import { useEffect, useMemo, useRef, useState } from "react";
import { fileName, formatBytes, type Progress } from "../platform";
import { useStore, type Job } from "../state";
import {
  IconActivity,
  IconArrowRight,
  IconClose,
  IconCommand,
  IconConvert,
  IconDoc,
  IconImage,
  IconMoon,
  IconPdf,
  IconPlus,
  IconSettings,
  IconSheet,
  IconSun,
  IconWarn,
} from "../design/icons";

export type Command = {
  id: string;
  title: string;
  group: string;
  shortcut?: string;
  run: () => void | Promise<void>;
};

export function TopBar({
  onCommandPalette,
  onJobs,
  jobsOpen,
  onSettings,
}: {
  onCommandPalette: () => void;
  onJobs: () => void;
  jobsOpen: boolean;
  onSettings: () => void;
}) {
  const { jobs, settings, setSettings, tabs, activeId } = useStore();
  const active = jobs.filter((job) => job.state === "running" || job.state === "queued").length;
  void tabs;
  void activeId;
  const theme =
    settings.theme === "system"
      ? window.matchMedia?.("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light"
      : settings.theme;

  return (
    <header className="titlebar">
      <div className="brand">
        <svg className="brand-mark" viewBox="0 0 24 24" aria-hidden="true">
          <path d="M6 3h8l4 4v14H6z" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinejoin="round" />
          <path d="M14 3v4h4" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinejoin="round" />
          <path d="M9 12h6M9 15.5h6" stroke="currentColor" strokeWidth="1.4" />
        </svg>
        <span>TEDROX Documents</span>
      </div>

      <button className="btn ghost" onClick={onCommandPalette} style={{ gap: 8, color: "var(--text-muted)" }}>
        <IconCommand />
        <span>Search commands</span>
        <span className="mono small">Ctrl K</span>
      </button>

      <span className="titlebar-spacer" />

      <div className="titlebar-actions">
        <button className={`icon-btn${jobsOpen ? " on" : ""}`} title="Jobs" onClick={onJobs} style={{ position: "relative" }}>
          <IconActivity />
          {active > 0 ? (
            <span
              style={{
                position: "absolute",
                top: 2,
                right: 2,
                width: 7,
                height: 7,
                borderRadius: "50%",
                background: "var(--accent)",
              }}
            />
          ) : null}
        </button>
        <button
          className="icon-btn"
          title={theme === "dark" ? "Switch to light theme" : "Switch to dark theme"}
          onClick={() => setSettings({ theme: theme === "dark" ? "light" : "dark" })}
        >
          {theme === "dark" ? <IconSun /> : <IconMoon />}
        </button>
        <button className="icon-btn" title="Settings" onClick={onSettings}>
          <IconSettings />
        </button>
      </div>
    </header>
  );
}

export function TabStrip({ onNewTab }: { onNewTab: () => void }) {
  const { tabs, activeId, activateTab, closeTab } = useStore();

  return (
    <div className="tabstrip">
      {tabs.map((tab) => (
        <div
          key={tab.id}
          className={`tab${tab.id === activeId ? " active" : ""}`}
          onMouseDown={(event) => {
            if (event.button === 1) {
              event.preventDefault();
              closeTab(tab.id);
              return;
            }
            activateTab(tab.id);
          }}
          onAuxClick={(event) => {
            if (event.button === 1) closeTab(tab.id);
          }}
        >
          {tab.kind === "document" && <IconDoc className="tab-icon" />}
          {tab.kind === "sheet" && <IconSheet className="tab-icon" />}
          {tab.kind === "pdf" && <IconPdf className="tab-icon" />}
          {tab.kind === "convert" && <IconConvert className="tab-icon" />}
          {tab.kind === "images" && <IconImage className="tab-icon" />}
          {tab.kind === "home" && <IconCommand className="tab-icon" />}
          {tab.kind === "settings" && <IconSettings className="tab-icon" />}
          <span className="tab-label" title={tab.path ?? tab.title}>
            {tab.title}
            {tab.subtitle ? ` · ${tab.subtitle}` : ""}
          </span>
          {tab.dirty ? <span className="tab-dirty" title="Unsaved changes" /> : null}
          <button
            className="tab-close"
            title="Close tab (Ctrl+W)"
            onClick={(event) => {
              event.stopPropagation();
              closeTab(tab.id);
            }}
          >
            <IconClose width={12} height={12} />
          </button>
        </div>
      ))}
      <button className="icon-btn tab-new" onClick={onNewTab} title="New document (Ctrl+N)">
        <IconPlus />
      </button>
    </div>
  );
}

export function CommandPalette({
  commands,
  onClose,
}: {
  commands: Command[];
  onClose: () => void;
}) {
  const [query, setQuery] = useState("");
  const [index, setIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const list = needle
      ? commands.filter((command) =>
          `${command.group} ${command.title}`.toLowerCase().includes(needle),
        )
      : commands;
    return list.slice(0, 40);
  }, [commands, query]);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    setIndex(0);
  }, [query]);

  const run = (command: Command) => {
    onClose();
    void command.run();
  };

  return (
    <div className="overlay" onMouseDown={onClose}>
      <div className="palette" onMouseDown={(event) => event.stopPropagation()}>
        <input
          ref={inputRef}
          className="palette-input"
          placeholder="Search commands, files and tools"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Escape") onClose();
            if (event.key === "ArrowDown") {
              event.preventDefault();
              setIndex((value) => Math.min(value + 1, filtered.length - 1));
            }
            if (event.key === "ArrowUp") {
              event.preventDefault();
              setIndex((value) => Math.max(value - 1, 0));
            }
            if (event.key === "Enter" && filtered[index]) {
              event.preventDefault();
              run(filtered[index]);
            }
          }}
        />
        <div className="palette-list">
          {filtered.length === 0 && <p className="palette-section">No matching commands</p>}
          {filtered.map((command, position) => (
            <button
              key={command.id}
              className={`palette-item${position === index ? " active" : ""}`}
              onMouseEnter={() => setIndex(position)}
              onClick={() => run(command)}
            >
              <span>{command.title}</span>
              <span className="hint">{command.shortcut ?? command.group}</span>
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}

export function JobCenter({ onClose }: { onClose: () => void }) {
  const { jobs, cancelJob, platform } = useStore();
  return (
    <div className="popover" style={{ position: "fixed", top: 42, right: 12 }}>
      <div className="popover-head">
        <span>Jobs</span>
        <button className="icon-btn" onClick={onClose}>
          <IconClose />
        </button>
      </div>
      <div className="popover-body">
        {jobs.length === 0 && <p className="small muted" style={{ padding: 8 }}>No jobs yet.</p>}
        {jobs.map((job: Job) => (
          <div className="job-row" key={job.id}>
            <span className={`job-state ${job.state}`} />
            <div className="job-main">
              <div className="job-title">{job.title}</div>
              <div className="job-meta">
                <span>{job.stage}</span>
                {job.bytesOut ? <span>{formatBytes(job.bytesOut)}</span> : null}
                {job.error ? <span style={{ color: "var(--danger)" }}>{job.error}</span> : null}
              </div>
              {job.state === "running" && (
                <div className="progress-track">
                  <div className="progress-fill" style={{ width: `${Math.round(job.progress * 100)}%` }} />
                </div>
              )}
            </div>
            {job.state === "running" || job.state === "queued" ? (
              <button className="icon-btn" onClick={() => cancelJob(job.id)} title="Cancel">
                <IconClose />
              </button>
            ) : job.outputs[0] ? (
              <button className="icon-btn" title="Show in folder" onClick={() => void platform.reveal(job.outputs[0])}>
                <IconArrowRight />
              </button>
            ) : null}
          </div>
        ))}
      </div>
    </div>
  );
}

export function Toasts() {
  const { toasts, dismissToast } = useStore();
  if (toasts.length === 0) return null;
  return (
    <div className="toast-stack">
      {toasts.map((toast) => (
        <div className={`toast ${toast.kind}`} key={toast.id}>
          <div style={{ flex: 1 }}>
            <div>{toast.message}</div>
            {toast.detail ? <div className="small muted">{toast.detail}</div> : null}
          </div>
          <button onClick={() => dismissToast(toast.id)} title="Dismiss">
            <IconClose width={12} height={12} />
          </button>
        </div>
      ))}
    </div>
  );
}

export function ProgressIndicator({ progress }: { progress: Progress | null }) {
  if (!progress) return null;
  return (
    <span className="status-pill">
      <IconWarn /> {progress.stage} {Math.round(progress.progress * 100)}%
    </span>
  );
}

export function pathLabel(path: string | null): string {
  return path ? fileName(path) : "Untitled";
}
