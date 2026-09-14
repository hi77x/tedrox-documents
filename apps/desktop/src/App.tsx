import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AppStateProvider, useStore, type Tab, type TabKind } from "./state";
import { classify, fileName } from "./platform";
import { CommandPalette, JobCenter, TabStrip, Toasts, TopBar, type Command } from "./shell/Chrome";
import { HomeView } from "./workspaces/HomeView";
import { SettingsView } from "./workspaces/SettingsView";
import { PdfWorkspace } from "./workspaces/pdf/PdfWorkspace";
import { WriterWorkspace } from "./workspaces/WriterWorkspace";
import { SheetsWorkspace } from "./workspaces/SheetsWorkspace";
import { ConvertWorkspace } from "./workspaces/ConvertWorkspace";
import { emptyDocument, type DocModel } from "./docModel";
import { emptyWorkbook, workbookFromModel, type Workbook } from "./sheetModel";
import "./design/tokens.css";

type Payload =
  | { kind: "document"; model: DocModel | null; error?: string }
  | { kind: "sheet"; workbook: Workbook | null; error?: string };

export function Shell() {
  const {
    platform,
    tabs,
    activeId,
    openTab,
    closeTab,
    activateTab,
    updateTab,
    settings,
    pushToast,
    runJob,
  } = useStore();
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [jobsOpen, setJobsOpen] = useState(false);
  const [payloads, setPayloads] = useState<Record<string, Payload>>({});
  const payloadsRef = useRef(payloads);
  payloadsRef.current = payloads;
  const inFlight = useRef(new Set<string>());

  const loadPayload = useCallback(
    async (id: string, kind: TabKind, path: string) => {
      if (kind === "document") {
        try {
          const model = await platform.invoke<DocModel>("docx_open", { path });
          setPayloads((current) => ({ ...current, [id]: { kind: "document", model } }));
        } catch (error) {
          setPayloads((current) => ({ ...current, [id]: { kind: "document", model: null, error: String(error) } }));
          pushToast({ kind: "error", message: "Cannot open the document", detail: String(error) });
        }
      } else if (kind === "sheet") {
        try {
          const raw = await platform.invoke<Parameters<typeof workbookFromModel>[0]>("sheet_open", { path });
          const workbook = workbookFromModel(raw);
          setPayloads((current) => ({ ...current, [id]: { kind: "sheet", workbook } }));
        } catch (error) {
          setPayloads((current) => ({ ...current, [id]: { kind: "sheet", workbook: null, error: String(error) } }));
          pushToast({ kind: "error", message: "Cannot open the spreadsheet", detail: String(error) });
        }
      }
    },
    [platform, pushToast],
  );

  const ensurePayload = useCallback(
    (id: string, kind: TabKind, path: string | null) => {
      if (!path || (kind !== "document" && kind !== "sheet")) return;
      if (payloadsRef.current[id]) return;
      if (inFlight.current.has(id)) return;
      inFlight.current.add(id);
      void loadPayload(id, kind, path).finally(() => inFlight.current.delete(id));
    },
    [loadPayload],
  );

  useEffect(() => {
    for (const tab of tabs) {
      if (tab.path && (tab.kind === "document" || tab.kind === "sheet")) {
        ensurePayload(tab.id, tab.kind, tab.path);
      }
    }
  }, [ensurePayload, tabs]);

  const tabId = useCallback(
    (kind: TabKind, path: string | null, title: string) =>
      openTab({ kind, title, path }),
    [openTab],
  );

  const importAsDocument = useCallback(
    async (path: string, payloadId: string) => {
      try {
        const output = await platform.suggestOutput(path, "imported", "docx");
        await runJob(
          "Import document",
          fileName(path),
          "doc_from_markdown",
          { file: path, output, title: fileName(path).replace(/\.[^.]+$/, "") },
          { quiet: true },
        );
        await loadPayload(payloadId, "document", output);
        updateTab(payloadId, { path: output });
      } catch (error) {
        pushToast({ kind: "error", message: "Import failed", detail: String(error) });
      }
    },
    [loadPayload, platform, pushToast, runJob, updateTab],
  );

  const openPath = useCallback(
    (path: string) => {
      const kind = classify(path);
      if (kind === "pdf") {
        const id = tabId("pdf", path, fileName(path));
        ensurePayload(id, "pdf", path);
        return;
      }
      if (kind === "sheet") {
        const id = tabId("sheet", path, fileName(path));
        ensurePayload(id, "sheet", path);
        return;
      }
      if (kind === "image") {
        openTab({ kind: "images", title: "Images", path: null });
        return;
      }
      if (kind === "document") {
        const id = tabId("document", path, fileName(path));
        if (path.toLowerCase().endsWith(".docx")) {
          ensurePayload(id, "document", path);
        } else {
          void importAsDocument(path, id);
        }
        return;
      }
      openTab({ kind: "convert", title: "Convert", path: null });
    },
    [ensurePayload, importAsDocument, openTab, tabId],
  );

  const openFiles = useCallback(async () => {
    const picked = await platform.openPaths({
      multiple: true,
      title: "Open file",
      extensions: [
        "docx", "doc", "odt", "rtf", "xlsx", "csv", "tsv", "ods", "pdf", "md", "txt", "html",
        "png", "jpg", "jpeg", "webp", "bmp", "tiff", "avif", "svg",
      ],
    });
    if (!picked) return;
    for (const path of picked) openPath(path);
  }, [openPath, platform]);

  const newDocument = useCallback(() => {
    const id = openTab({ kind: "document", title: "Untitled document", path: null });
    setPayloads((current) => ({ ...current, [id]: { kind: "document", model: emptyDocument() } }));
  }, [openTab]);

  const newSpreadsheet = useCallback(() => {
    const id = openTab({ kind: "sheet", title: "Untitled spreadsheet", path: null });
    setPayloads((current) => ({ ...current, [id]: { kind: "sheet", workbook: emptyWorkbook() } }));
  }, [openTab]);

  const openHome = useCallback(() => {
    const home = tabs.find((tab) => tab.kind === "home");
    if (home) activateTab(home.id);
    else openTab({ kind: "home", title: "Home", path: null });
  }, [activateTab, openTab, tabs]);

  const commands: Command[] = useMemo(() => {
    const list: Command[] = [
      { id: "file.open", title: "Open file…", group: "File", shortcut: "Ctrl O", run: openFiles },
      { id: "file.new.document", title: "New document", group: "File", shortcut: "Ctrl N", run: newDocument },
      { id: "file.new.sheet", title: "New spreadsheet", group: "File", run: newSpreadsheet },
      { id: "nav.home", title: "Go to Home", group: "Navigate", run: openHome },
      { id: "nav.pdf", title: "Open the PDF workspace", group: "Navigate", run: () => openTab({ kind: "pdf", title: "PDF", path: null }) },
      { id: "nav.convert", title: "Open Convert", group: "Navigate", run: () => openTab({ kind: "convert", title: "Convert", path: null }) },
      { id: "nav.images", title: "Open Images", group: "Navigate", run: () => openTab({ kind: "images", title: "Images", path: null }) },
      { id: "tab.close", title: "Close the current tab", group: "View", shortcut: "Ctrl W", run: () => closeTab(activeId) },
    ];
    if (activeId.startsWith("document")) {
      list.push({
        id: "doc.export",
        title: "Export the document to PDF",
        group: "Document",
        run: async () => {
          const tab = tabs.find((item) => item.id === activeId);
          if (!tab?.path) {
            pushToast({ kind: "warn", message: "Save the document first" });
            return;
          }
          const suggested = await platform.suggestOutput(tab.path, "export", "pdf");
          const target = await platform.savePath(suggested, "pdf", "PDF");
          if (!target) return;
          await runJob("Export PDF", fileName(target), "convert_auto", { file: tab.path, output: target });
        },
      });
    }
    for (const tab of tabs) {
      if (!tab.path) continue;
      list.push({
        id: `open.${tab.id}`,
        title: `Switch to ${tab.title}`,
        group: "Open files",
        run: () => activateTab(tab.id),
      });
    }
    return list;
  }, [
    activateTab,
    activeId,
    closeTab,
    newDocument,
    newSpreadsheet,
    openFiles,
    openHome,
    openTab,
    platform,
    pushToast,
    runJob,
    tabs,
  ]);

  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      const meta = event.ctrlKey || event.metaKey;
      if (!meta) return;
      const key = event.key.toLowerCase();
      if (key === "k") {
        event.preventDefault();
        setPaletteOpen(true);
      }
      if (key === "o") {
        event.preventDefault();
        void openFiles();
      }
      if (key === "n") {
        event.preventDefault();
        newDocument();
      }
      if (key === "w") {
        event.preventDefault();
        closeTab(activeId);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [activeId, closeTab, newDocument, openFiles]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void import("@tauri-apps/api/webview")
      .then(({ getCurrentWebview }) =>
        getCurrentWebview().onDragDropEvent((event) => {
          if (event.payload.type === "drop" && event.payload.paths.length > 0) {
            for (const path of event.payload.paths) openPath(path);
          }
        }),
      )
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => undefined);
    return () => unlisten?.();
  }, [openPath]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void import("@tauri-apps/api/event")
      .then(({ listen }) =>
        listen<string>("open-file", (event) => {
          if (event.payload) openPath(event.payload);
        }),
      )
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => undefined);
    return () => unlisten?.();
  }, [openPath]);

  const themeClass = settings.theme;

  return (
    <div className={`app-shell theme-${themeClass}`}>
      <TopBar
        onCommandPalette={() => setPaletteOpen(true)}
        onJobs={() => setJobsOpen((value) => !value)}
        jobsOpen={jobsOpen}
        onSettings={() => openTab({ kind: "settings", title: "Settings", path: null })}
      />
      <TabStrip onNewTab={newDocument} />

      <div style={{ position: "relative", minHeight: 0, overflow: "hidden" }}>
        {tabs.map((tab) => (
          <div
            key={tab.id}
            style={{
              position: "absolute",
              inset: 0,
              display: tab.id === activeId ? "block" : "none",
              minHeight: 0,
            }}
          >
            <TabBody
              tab={tab}
              payload={payloads[tab.id]}
              onOpenPath={openPath}
              onOpenFiles={openFiles}
              onNewDocument={newDocument}
              onNewSpreadsheet={newSpreadsheet}
              onOpenTab={(kind) => openTab({ kind, title: titleFor(kind), path: null })}
            />
          </div>
        ))}
      </div>

      {paletteOpen && <CommandPalette commands={commands} onClose={() => setPaletteOpen(false)} />}
      {jobsOpen && <JobCenter onClose={() => setJobsOpen(false)} />}
      <Toasts />
    </div>
  );
}

function titleFor(kind: TabKind): string {
  switch (kind) {
    case "pdf":
      return "PDF";
    case "convert":
      return "Convert";
    case "images":
      return "Images";
    case "settings":
      return "Settings";
    case "sheet":
      return "Spreadsheet";
    case "document":
      return "Document";
    default:
      return "Home";
  }
}

function TabBody({
  tab,
  payload,
  onOpenPath,
  onOpenFiles,
  onNewDocument,
  onNewSpreadsheet,
  onOpenTab,
}: {
  tab: Tab;
  payload: Payload | undefined;
  onOpenPath: (path: string) => void;
  onOpenFiles: () => void;
  onNewDocument: () => void;
  onNewSpreadsheet: () => void;
  onOpenTab: (kind: "pdf" | "convert" | "images") => void;
}) {
  const { updateTab } = useStore();

  if (tab.kind === "home") {
    return (
      <HomeView
        onOpenFiles={onOpenFiles}
        onNewDocument={onNewDocument}
        onNewSpreadsheet={onNewSpreadsheet}
        onOpenPath={onOpenPath}
        onOpenTab={onOpenTab}
      />
    );
  }
  if (tab.kind === "settings") return <SettingsView />;
  if (tab.kind === "pdf") {
    return <PdfWorkspace path={tab.path} tabId={tab.id} />;
  }
  if (tab.kind === "convert") return <ConvertWorkspace mode="convert" />;
  if (tab.kind === "images") return <ConvertWorkspace mode="images" />;

  if (tab.kind === "document") {
    if (!payload) {
      return (
        <div className="empty-state">
          <h2>Opening…</h2>
          <p>{tab.path ?? tab.title}</p>
        </div>
      );
    }
    if (payload.kind === "document" && payload.model) {
      return (
        <WriterWorkspace tabId={tab.id} path={tab.path} model={payload.model} displayName={tab.title} />
      );
    }
    return (
      <div className="empty-state">
        <h2>Could not open this document</h2>
        <p>{payload.error ?? "Unknown error"}</p>
        <button className="btn" onClick={() => updateTab(tab.id, { dirty: false })}>
          Continue without the file
        </button>
      </div>
    );
  }

  if (tab.kind === "sheet") {
    if (!payload || payload.kind !== "sheet" || !payload.workbook) {
      return (
        <div className="empty-state">
          <h2>{payload?.error ? "Could not open this spreadsheet" : "Opening…"}</h2>
          <p>{payload?.error ?? tab.path ?? tab.title}</p>
        </div>
      );
    }
    return (
      <SheetsWorkspace tabId={tab.id} path={tab.path} workbook={payload.workbook} displayName={tab.title} />
    );
  }

  return null;
}

export default function App() {
  return (
    <AppStateProvider>
      <Shell />
    </AppStateProvider>
  );
}
