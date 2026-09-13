//! Job runtime for TEDROX Documents.
//!
//! The engine owns a bounded worker pool, per-job cancellation tokens, progress
//! propagation and a sanitized job history. It never stores document contents —
//! only metadata, paths chosen by the user, sizes and timings.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Condvar, Mutex,
};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use tdx_core::{
    error::Result,
    progress::{CancelToken, ProgressEvent, ProgressSink, Stage},
    result::OperationResult,
};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Validating,
    Reading,
    Processing,
    Writing,
    Verifying,
    Completed,
    Cancelled,
    Failed,
    Partial,
}

impl JobState {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            JobState::Completed | JobState::Cancelled | JobState::Failed | JobState::Partial
        )
    }
}

/// Serializable snapshot of a job, safe to send to any UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRecord {
    pub id: Uuid,
    pub operation_id: String,
    pub created_at: SystemTime,
    pub started_at: Option<SystemTime>,
    pub finished_at: Option<SystemTime>,
    pub inputs: Vec<PathBuf>,
    pub outputs: Vec<PathBuf>,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub progress: f32,
    pub stage: Stage,
    pub state: JobState,
    pub warnings: Vec<String>,
    pub error: Option<String>,
    pub error_category: Option<String>,
}

impl JobRecord {
    fn new(id: Uuid, operation_id: String, inputs: Vec<PathBuf>) -> Self {
        Self {
            id,
            operation_id,
            created_at: SystemTime::now(),
            started_at: None,
            finished_at: None,
            inputs,
            outputs: Vec::new(),
            bytes_in: 0,
            bytes_out: 0,
            progress: 0.0,
            stage: Stage::Validating,
            state: JobState::Queued,
            warnings: Vec::new(),
            error: None,
            error_category: None,
        }
    }
}

pub struct JobContext {
    pub id: Uuid,
    pub cancel: CancelToken,
    pub progress: ProgressSink,
}

type JobTask = Box<dyn FnOnce(JobContext) -> Result<OperationResult> + Send + 'static>;
type Listener = Arc<dyn Fn(&JobRecord) + Send + Sync + 'static>;

struct Pending {
    id: Uuid,
    task: JobTask,
}

struct Inner {
    queue: Mutex<VecDeque<Pending>>,
    records: Mutex<HashMap<Uuid, JobRecord>>,
    tokens: Mutex<HashMap<Uuid, CancelToken>>,
    finished: Condvar,
    records_changed: Condvar,
    active: AtomicUsize,
    max_concurrent: usize,
    listener: Mutex<Option<Listener>>,
}

#[derive(Clone)]
pub struct JobEngine {
    inner: Arc<Inner>,
}

impl JobEngine {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            inner: Arc::new(Inner {
                queue: Mutex::new(VecDeque::new()),
                records: Mutex::new(HashMap::new()),
                tokens: Mutex::new(HashMap::new()),
                finished: Condvar::new(),
                records_changed: Condvar::new(),
                active: AtomicUsize::new(0),
                max_concurrent: max_concurrent.max(1),
                listener: Mutex::new(None),
            }),
        }
    }

    /// Register a listener that receives a record snapshot on every update.
    pub fn set_listener<F>(&self, listener: F)
    where
        F: Fn(&JobRecord) + Send + Sync + 'static,
    {
        *self.inner.listener.lock().expect("listener lock") = Some(Arc::new(listener));
    }

    pub fn submit<F>(
        &self,
        operation_id: impl Into<String>,
        inputs: Vec<PathBuf>,
        task: F,
    ) -> JobHandle
    where
        F: FnOnce(JobContext) -> Result<OperationResult> + Send + 'static,
    {
        let id = Uuid::new_v4();
        let operation_id = operation_id.into();
        let record = JobRecord::new(id, operation_id, inputs);
        let token = CancelToken::new();

        self.inner
            .records
            .lock()
            .expect("records lock")
            .insert(id, record.clone());
        self.inner
            .tokens
            .lock()
            .expect("tokens lock")
            .insert(id, token.clone());
        self.notify(&record);

        let pending = Pending {
            id,
            task: Box::new(task),
        };
        {
            let mut queue = self.inner.queue.lock().expect("queue lock");
            queue.push_back(pending);
        }
        self.dispatch();

        JobHandle {
            id,
            engine: self.clone(),
            cancel: token,
        }
    }

    pub fn status(&self, id: Uuid) -> Option<JobRecord> {
        self.inner
            .records
            .lock()
            .expect("records lock")
            .get(&id)
            .cloned()
    }

    pub fn history(&self) -> Vec<JobRecord> {
        let records = self.inner.records.lock().expect("records lock");
        let mut items: Vec<JobRecord> = records.values().cloned().collect();
        items.sort_by_key(|record| record.created_at);
        items.reverse();
        items
    }

    /// Wait until a job reaches a terminal state. `timeout` of `None` waits
    /// forever.
    pub fn wait(&self, id: Uuid, timeout: Option<Duration>) -> Option<JobRecord> {
        let deadline = timeout.map(|t| std::time::Instant::now() + t);
        let mut records = self.inner.records.lock().expect("records lock");
        loop {
            let record = records.get(&id).cloned();
            match &record {
                None => return None,
                Some(r) if r.state.is_terminal() => return record,
                _ => {}
            }
            records = match deadline {
                Some(deadline) => {
                    let now = std::time::Instant::now();
                    if now >= deadline {
                        return records.get(&id).cloned();
                    }
                    let (guard, _) = self
                        .inner
                        .finished
                        .wait_timeout(records, deadline - now)
                        .expect("condvar wait");
                    guard
                }
                None => self.inner.finished.wait(records).expect("condvar wait"),
            };
        }
    }

    pub fn cancel(&self, id: Uuid) {
        if let Some(token) = self
            .inner
            .tokens
            .lock()
            .expect("tokens lock")
            .get(&id)
            .cloned()
        {
            token.cancel();
            self.update(id, |record| {
                if !record.state.is_terminal() {
                    record.warnings.push("Cancellation requested".to_string());
                }
            });
        }
    }

    pub fn active_count(&self) -> usize {
        self.inner.active.load(Ordering::SeqCst)
    }

    fn dispatch(&self) {
        loop {
            let pending = {
                let mut queue = self.inner.queue.lock().expect("queue lock");
                if queue.is_empty() {
                    return;
                }
                if self.inner.active.load(Ordering::SeqCst) >= self.inner.max_concurrent {
                    return;
                }
                queue.pop_front()
            };
            let Some(pending) = pending else { return };
            self.inner.active.fetch_add(1, Ordering::SeqCst);
            let inner = self.inner.clone();
            std::thread::spawn(move || {
                run_job(inner.clone(), pending);
                inner.active.fetch_sub(1, Ordering::SeqCst);
                // Chain: after finishing, try to start the next queued job.
                let engine = JobEngine { inner };
                engine.dispatch();
            });
        }
    }

    fn update<F>(&self, id: Uuid, mutate: F)
    where
        F: FnOnce(&mut JobRecord),
    {
        let snapshot = {
            let mut records = self.inner.records.lock().expect("records lock");
            let Some(record) = records.get_mut(&id) else {
                return;
            };
            mutate(record);
            record.clone()
        };
        self.notify(&snapshot);
        self.inner.records_changed.notify_all();
    }

    fn notify(&self, record: &JobRecord) {
        if let Some(listener) = self.inner.listener.lock().expect("listener lock").clone() {
            listener(record);
        }
    }
}

fn state_for_stage(stage: Stage) -> JobState {
    match stage {
        Stage::Validating => JobState::Validating,
        Stage::Reading => JobState::Reading,
        Stage::Processing => JobState::Processing,
        Stage::Writing => JobState::Writing,
        Stage::Verifying => JobState::Verifying,
        Stage::Completed => JobState::Completed,
    }
}

fn run_job(inner: Arc<Inner>, pending: Pending) {
    let id = pending.id;
    let token = inner
        .tokens
        .lock()
        .expect("tokens lock")
        .get(&id)
        .cloned()
        .unwrap_or_default();

    let engine = JobEngine {
        inner: inner.clone(),
    };
    engine.update(id, |record| {
        record.state = JobState::Validating;
        record.started_at = Some(SystemTime::now());
        record.stage = Stage::Validating;
    });

    let progress_engine = engine.clone();
    let sink = ProgressSink::new(move |event: ProgressEvent| {
        progress_engine.update(id, |record| {
            record.stage = event.stage;
            record.progress = event.progress;
            if !event.stage.is_terminal() {
                record.state = state_for_stage(event.stage);
            }
        });
    });

    let context = JobContext {
        id,
        cancel: token.clone(),
        progress: sink,
    };
    let outcome = (pending.task)(context);

    match outcome {
        Ok(result) => {
            if token.is_cancelled() {
                engine.update(id, |record| {
                    record.state = JobState::Cancelled;
                    record.finished_at = Some(SystemTime::now());
                });
            } else {
                engine.update(id, |record| {
                    record.state = JobState::Completed;
                    record.stage = Stage::Completed;
                    record.progress = 1.0;
                    record.finished_at = Some(SystemTime::now());
                    record.outputs = result.outputs.iter().map(|o| o.path.clone()).collect();
                    record.bytes_in = result.bytes_in;
                    record.bytes_out = result.bytes_out;
                    record.warnings = result.warnings.clone();
                });
            }
        }
        Err(err) => {
            let cancelled = matches!(err, tdx_core::error::TdxError::Cancelled);
            engine.update(id, |record| {
                record.state = if cancelled {
                    JobState::Cancelled
                } else {
                    JobState::Failed
                };
                record.finished_at = Some(SystemTime::now());
                if !cancelled {
                    record.error = Some(err.to_string());
                    record.error_category = Some(err.category().as_str().to_string());
                }
            });
        }
    }

    inner.tokens.lock().expect("tokens lock").remove(&id);
    inner.finished.notify_all();
    inner.records_changed.notify_all();
}

/// Handle returned to callers. Dropping it does not cancel the job.
pub struct JobHandle {
    pub id: Uuid,
    engine: JobEngine,
    cancel: CancelToken,
}

impl JobHandle {
    pub fn cancel(&self) {
        self.cancel.cancel();
        self.engine.cancel(self.id);
    }

    pub fn wait(&self) -> Option<JobRecord> {
        self.engine.wait(self.id, None)
    }

    pub fn wait_timeout(&self, timeout: Duration) -> Option<JobRecord> {
        self.engine.wait(self.id, Some(timeout))
    }

    pub fn status(&self) -> Option<JobRecord> {
        self.engine.status(self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn runs_jobs_and_reports_success() {
        let engine = JobEngine::new(2);
        let handle = engine.submit("test.noop", vec![], |context| {
            context.progress.stage(Stage::Processing, 0.5);
            Ok(OperationResult::empty())
        });
        let record = handle.wait().expect("record");
        assert_eq!(record.state, JobState::Completed);
    }

    #[test]
    fn reports_failures_with_category() {
        let engine = JobEngine::new(1);
        let handle = engine.submit("test.fail", vec![], |_| {
            Err(tdx_core::error::TdxError::InvalidInput("bad option".into()))
        });
        let record = handle.wait().expect("record");
        assert_eq!(record.state, JobState::Failed);
        assert_eq!(record.error_category.as_deref(), Some("invalid_input"));
    }

    #[test]
    fn cancels_cooperatively() {
        let engine = JobEngine::new(1);
        let started = Arc::new(AtomicBool::new(false));
        let started_clone = started.clone();
        let handle = engine.submit("test.cancel", vec![], move |context| {
            started_clone.store(true, Ordering::SeqCst);
            for _ in 0..100 {
                context.cancel.check()?;
                std::thread::sleep(Duration::from_millis(5));
            }
            Ok(OperationResult::empty())
        });
        while !started.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(1));
        }
        handle.cancel();
        let record = handle.wait().expect("record");
        assert_eq!(record.state, JobState::Cancelled);
    }

    #[test]
    fn enforces_concurrency_limit() {
        use std::sync::atomic::AtomicUsize;
        let engine = JobEngine::new(1);
        let running = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();
        for _ in 0..4 {
            let running = running.clone();
            let peak = peak.clone();
            handles.push(engine.submit("test.concurrency", vec![], move |_| {
                let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(20));
                running.fetch_sub(1, Ordering::SeqCst);
                Ok(OperationResult::empty())
            }));
        }
        for handle in handles {
            assert_eq!(handle.wait().expect("record").state, JobState::Completed);
        }
        assert_eq!(peak.load(Ordering::SeqCst), 1);
    }
}
