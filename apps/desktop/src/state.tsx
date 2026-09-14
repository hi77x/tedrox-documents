import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { Platform, Progress, OpResult, formatBytes, platform as getPlatform } from "./platform";

export type TabKind = "home" | "settings" | "document" | "sheet" | "pdf" | "convert" | "images";

export type Tab = {
  id: string;
  kind: TabKind;
  title: string;
  subtitle?: string;
  path: string | null;
  dirty: boolean;
};

export type JobState = "queued" | "running" | "done" | "failed" | "cancelled";

export type Job = {
  id: string;
  title: string;
  detail: string;
  stage: string;
  progress: number;
  state: JobState;
  error?: string;
  startedAt: number;
  finishedAt?: number;
  bytesIn?: number;
  bytesOut?: number;
  outputs: string[];
  warnings: string[];
};

export type ToastKind = "info" | "success" | "warn" | "error";

export type Toast = {
  id: string;
  kind: ToastKind;
  message: string;
  detail?: string;
};

export type Theme = "system" | "light" | "dark";
export type Language = "en" | "ru";

export type RecentEntry = {
  path: string;
  kind: TabKind;
  at: number;
};

type Settings = {
  theme: Theme;
  language: Language;
  autosave: boolean;
  restoreSession: boolean;
  defaultZoom: number;
  spellcheck: boolean;
};

type Store = {
  platform: Platform;
  tabs: Tab[];
  activeId: string;
  jobs: Job[];
  toasts: Toast[];
  recent: RecentEntry[];
  pinned: string[];
  settings: Settings;
  workspaceVisible: boolean;
  openTab: (tab: Omit<Tab, "id" | "dirty"> & { id?: string; dirty?: boolean }) => string;
  closeTab: (id: string) => void;
  closeOtherTabs: (id: string) => void;
  activateTab: (id: string) => void;
  updateTab: (id: string, patch: Partial<Tab>) => void;
  togglePinned: (path: string) => void;
  remember: (path: string, kind: TabKind) => void;
  clearRecent: () => void;
  setSettings: (patch: Partial<Settings>) => void;
  pushToast: (toast: Omit<Toast, "id">) => void;
  dismissToast: (id: string) => void;
  startJob: (title: string, detail: string) => string;
  updateJob: (id: string, patch: Partial<Job>) => void;
  finishJob: (id: string, state: JobState, result?: OpResult, error?: string) => void;
  runJob: (
    title: string,
    detail: string,
    command: string,
    args: Record<string, unknown>,
    options?: { quiet?: boolean },
  ) => Promise<OpResult>;
  cancelJob: (id: string) => void;
};

const StoreContext = createContext<Store | null>(null);

const RECENT_KEY = "tdx.recent.v2";
const PINNED_KEY = "tdx.pinned.v2";
const SETTINGS_KEY = "tdx.settings.v2";
const SESSION_KEY = "tdx.session.v2";

function readJson<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    if (!raw) return fallback;
    return JSON.parse(raw) as T;
  } catch {
    return fallback;
  }
}

function detectLanguage(): Language {
  const stored = localStorage.getItem("tdx-lang");
  if (stored === "en" || stored === "ru") return stored;
  return navigator.language.slice(0, 2).toLowerCase() === "ru" ? "ru" : "en";
}

export function AppStateProvider({
  children,
}: {
  children: ReactNode;
}) {
  const platform = useMemo(() => getPlatform(), []);
  const [tabs, setTabs] = useState<Tab[]>(() => [
    { id: "home", kind: "home", title: "Home", path: null, dirty: false },
  ]);
  const [activeId, setActiveId] = useState("home");
  const [jobs, setJobs] = useState<Job[]>([]);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [recent, setRecent] = useState<RecentEntry[]>(() => readJson(RECENT_KEY, []));
  const [pinned, setPinned] = useState<string[]>(() => readJson(PINNED_KEY, []));
  const [settings, setSettingsState] = useState<Settings>(() => ({
    theme: "system",
    language: detectLanguage(),
    autosave: false,
    restoreSession: false,
    defaultZoom: 100,
    spellcheck: false,
    ...readJson<Partial<Settings>>(SETTINGS_KEY, {}),
  }));
  const [workspaceVisible, setWorkspaceVisible] = useState(false);
  const cancelled = useRef(new Set<string>());

  useEffect(() => {
    document.documentElement.dataset.theme = settings.theme;
  }, [settings.theme]);

  useEffect(() => {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings));
    localStorage.setItem("tdx-lang", settings.language);
    document.documentElement.lang = settings.language;
  }, [settings]);

  useEffect(() => {
    localStorage.setItem(RECENT_KEY, JSON.stringify(recent.slice(0, 40)));
  }, [recent]);

  useEffect(() => {
    localStorage.setItem(PINNED_KEY, JSON.stringify(pinned));
  }, [pinned]);

  useEffect(() => {
    if (!settings.restoreSession) {
      localStorage.removeItem(SESSION_KEY);
      return;
    }
    const restorable = tabs
      .filter((tab) => tab.path && (tab.kind === "document" || tab.kind === "sheet" || tab.kind === "pdf"))
      .map((tab) => ({ kind: tab.kind, path: tab.path as string, title: tab.title }));
    localStorage.setItem(SESSION_KEY, JSON.stringify(restorable));
  }, [tabs, settings.restoreSession]);

  const openTab = useCallback<Store["openTab"]>((tab) => {
    const id = tab.id ?? `${tab.kind}:${tab.path ?? Date.now()}:${Math.random().toString(36).slice(2, 7)}`;
    setTabs((current) => {
      const existing = current.find((item) => item.id === id);
      if (existing) {
        return current.map((item) => (item.id === id ? { ...item, ...tab, id } : item));
      }
      return [...current, { ...tab, id, dirty: tab.dirty ?? false }];
    });
    setActiveId(id);
    setWorkspaceVisible(true);
    return id;
  }, []);

  const closeTab = useCallback((id: string) => {
    setTabs((current) => {
      const index = current.findIndex((tab) => tab.id === id);
      if (index === -1) return current;
      const next = current.filter((tab) => tab.id !== id);
      if (next.length === 0) {
        return [{ id: "home", kind: "home", title: "Home", path: null, dirty: false }];
      }
      setActiveId((active) => {
        if (active !== id) return active;
        const fallback = next[Math.min(index, next.length - 1)];
        return fallback.id;
      });
      return next;
    });
  }, []);

  const closeOtherTabs = useCallback((id: string) => {
    setTabs((current) => {
      const keep = current.filter((tab) => tab.id === id || tab.kind === "home");
      setActiveId(id);
      return keep.length ? keep : current;
    });
  }, []);

  const updateTab = useCallback((id: string, patch: Partial<Tab>) => {
    setTabs((current) => current.map((tab) => (tab.id === id ? { ...tab, ...patch } : tab)));
  }, []);

  const remember = useCallback((path: string, kind: TabKind) => {
    setRecent((current) => [
      { path, kind, at: Date.now() },
      ...current.filter((entry) => entry.path !== path),
    ].slice(0, 40));
  }, []);

  const togglePinned = useCallback((path: string) => {
    setPinned((current) =>
      current.includes(path) ? current.filter((item) => item !== path) : [path, ...current],
    );
  }, []);

  const pushToast = useCallback((toast: Omit<Toast, "id">) => {
    const id = Math.random().toString(36).slice(2, 9);
    setToasts((current) => [...current, { ...toast, id }].slice(-4));
    if (toast.kind !== "error") {
      window.setTimeout(() => setToasts((current) => current.filter((item) => item.id !== id)), 5200);
    }
  }, []);

  const dismissToast = useCallback((id: string) => {
    setToasts((current) => current.filter((item) => item.id !== id));
  }, []);

  const startJob = useCallback<Store["startJob"]>((title, detail) => {
    const id = Math.random().toString(36).slice(2, 9);
    const record: Job = {
      id,
      title,
      detail,
      stage: "queued",
      progress: 0,
      state: "queued",
      startedAt: Date.now(),
      outputs: [],
      warnings: [],
    };
    setJobs((current) => [record, ...current].slice(0, 40));
    return id;
  }, []);

  const updateJob = useCallback<Store["updateJob"]>((id, patch) => {
    setJobs((current) => current.map((job) => (job.id === id ? { ...job, ...patch } : job)));
  }, []);

  const finishJob = useCallback<Store["finishJob"]>((id, state, result, error) => {
    setJobs((current) =>
      current.map((job) =>
        job.id === id
          ? {
              ...job,
              state,
              progress: state === "done" ? 1 : job.progress,
              finishedAt: Date.now(),
              error,
              bytesIn: result?.bytesIn ?? job.bytesIn,
              bytesOut: result?.bytesOut ?? job.bytesOut,
              outputs: result?.outputs.map((output) => output.path) ?? job.outputs,
              warnings: result?.warnings ?? job.warnings,
              stage: state,
            }
          : job,
      ),
    );
  }, []);

  const runJob = useCallback<Store["runJob"]>(
    async (title, detail, command, args, options) => {
      const id = startJob(title, detail);
      try {
        const result = await platform.run(command, args, (progress: Progress) => {
          updateJob(id, {
            state: cancelled.current.has(id) ? "cancelled" : "running",
            stage: progress.stage,
            progress: progress.progress,
          });
        });
        finishJob(id, cancelled.current.has(id) ? "cancelled" : "done", result);
        if (!options?.quiet) {
          const summary =
            result.bytesOut > 0
              ? `${formatBytes(result.bytesIn)} → ${formatBytes(result.bytesOut)}`
              : `${result.outputs.length} output${result.outputs.length === 1 ? "" : "s"}`;
          pushToast({ kind: "success", message: title, detail: summary });
        }
        return result;
      } catch (error) {
        const message = String(error);
        finishJob(id, cancelled.current.has(id) ? "cancelled" : "failed", undefined, message);
        if (!options?.quiet) {
          pushToast({ kind: "error", message: title, detail: message });
        }
        throw error;
      } finally {
        cancelled.current.delete(id);
      }
    },
    [finishJob, pushToast, startJob, updateJob],
  );

  const cancelJob = useCallback((id: string) => {
    cancelled.current.add(id);
    setJobs((current) =>
      current.map((job) => (job.id === id ? { ...job, state: "cancelled", stage: "cancelled" } : job)),
    );
  }, []);

  const activateTab = useCallback((id: string) => setActiveId(id), []);
  const clearRecent = useCallback(() => setRecent([]), []);
  const setSettings = useCallback(
    (patch: Partial<Settings>) => setSettingsState((current) => ({ ...current, ...patch })),
    [],
  );

  const value: Store = useMemo(
    () => ({
      platform,
      tabs,
      activeId,
      jobs,
      toasts,
      recent,
      pinned,
      settings,
      workspaceVisible,
      openTab,
      closeTab,
      closeOtherTabs,
      activateTab,
      updateTab,
      togglePinned,
      remember,
      clearRecent,
      setSettings,
      pushToast,
      dismissToast,
      startJob,
      updateJob,
      finishJob,
      runJob,
      cancelJob,
    }),
    [
      activeId,
      activateTab,
      cancelJob,
      clearRecent,
      closeOtherTabs,
      closeTab,
      dismissToast,
      finishJob,
      jobs,
      openTab,
      pinned,
      platform,
      pushToast,
      recent,
      remember,
      runJob,
      setSettings,
      settings,
      startJob,
      tabs,
      toasts,
      togglePinned,
      updateJob,
      updateTab,
      workspaceVisible,
    ],
  );

  return <StoreContext.Provider value={value}>{children}</StoreContext.Provider>;
}

export function useStore(): Store {
  const store = useContext(StoreContext);
  if (!store) throw new Error("useStore must be used inside AppStateProvider");
  return store;
}
