import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import DocumentEditor from "./DocumentEditor";
import SpreadsheetEditor from "./SpreadsheetEditor";
import { CONVERT_TOOLS, IMAGE_TOOLS, PDF_TOOLS, ToolsPage } from "./ToolsPage";
import { DocModel, emptyDocument } from "./docModel";
import { WorkbookModel } from "./sheetModel";
import { runOperation } from "./api";
import { Language, detectLanguage, setLanguage, t } from "./i18n";

type View = "home" | "doc" | "sheet" | "pdf" | "images" | "convert" | "settings";
type Recent = { path: string; kind: "doc" | "sheet"; at: number };

const RECENT_KEY = "tdx-recent";

function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

export default function App() {
  const [language, setLanguageState] = useState<Language>(() => detectLanguage());
  const [theme, setTheme] = useState<"system" | "light" | "dark">("system");
  const [view, setView] = useState<View>("home");
  const [docState, setDocState] = useState<{
    key: number;
    path: string | null;
    model: DocModel;
    name: string;
  } | null>(null);
  const [sheetState, setSheetState] = useState<{
    key: number;
    path: string | null;
    workbook: WorkbookModel;
    name: string;
  } | null>(null);
  const [recent, setRecent] = useState<Recent[]>(() => {
    try {
      const stored = JSON.parse(localStorage.getItem(RECENT_KEY) ?? "[]");
      return Array.isArray(stored) ? stored : [];
    } catch {
      return [];
    }
  });
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setLanguage(language);
  }, [language]);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  const remember = (path: string, kind: "doc" | "sheet") => {
    setRecent((previous) => {
      const next = [{ path, kind, at: Date.now() }, ...previous.filter((item) => item.path !== path)].slice(0, 12);
      localStorage.setItem(RECENT_KEY, JSON.stringify(next));
      return next;
    });
  };

  const newDocument = () => {
    setDocState({ key: Date.now(), path: null, model: emptyDocument(), name: t("doc.untitled") });
    setView("doc");
  };

  const newSpreadsheet = () => {
    setSheetState({
      key: Date.now(),
      path: null,
      workbook: { sheets: [{ name: "Sheet1", cells: [] }] },
      name: t("sheet.untitled"),
    });
    setView("sheet");
  };

  const openPath = async (path: string) => {
    const lower = path.toLowerCase();
    setError(null);
    try {
      if (lower.endsWith(".docx")) {
        const model = await invoke<DocModel>("docx_open", { path });
        setDocState({
          key: Date.now(),
          path,
          model,
          name: model.title ?? fileName(path),
        });
        setView("doc");
        remember(path, "doc");
      } else if (/\.(xlsx|csv|tsv|ods)$/.test(lower)) {
        const workbook = await invoke<WorkbookModel>("sheet_open", { path });
        setSheetState({ key: Date.now(), path, workbook, name: fileName(path) });
        setView("sheet");
        remember(path, "sheet");
      } else if (lower.endsWith(".md") || lower.endsWith(".txt")) {
        const output = path.replace(/\.(md|txt)$/i, "") + ".imported.docx";
        await runOperation({
          command: "convert_auto",
          args: { file: path, output },
          onProgress: () => {},
        });
        const model = await invoke<DocModel>("docx_open", { path: output });
        setDocState({ key: Date.now(), path: output, model, name: fileName(output) });
        setView("doc");
        remember(output, "doc");
      } else {
        setError(t("app.unsupported"));
      }
    } catch (err) {
      setError(String(err));
    }
  };

  const openDialog = async () => {
    const selection = await open({
      multiple: false,
      title: t("app.openFile"),
      filters: [
        {
          name: t("app.documentsAndSheets"),
          extensions: ["docx", "xlsx", "csv", "tsv", "ods", "md", "txt"],
        },
      ],
    });
    if (!selection) return;
    await openPath(Array.isArray(selection) ? selection[0] : selection);
  };

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "drop" && event.payload.paths.length > 0) {
          void openPath(event.payload.paths[0]);
        }
      })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {});
    return () => {
      if (unlisten) unlisten();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    import("@tauri-apps/api/event")
      .then(({ listen }) =>
        listen<string>("open-file", (event) => {
          if (event.payload) void openPath(event.payload);
        }),
      )
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {});
    return () => {
      if (unlisten) unlisten();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const navItems: { id: View; label: string }[] = [
    { id: "home", label: t("nav.home") },
    { id: "doc", label: t("nav.document") },
    { id: "sheet", label: t("nav.spreadsheet") },
    { id: "pdf", label: "PDF" },
    { id: "images", label: t("nav.images") },
    { id: "convert", label: t("nav.convert") },
    { id: "settings", label: t("nav.settings") },
  ];

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand">
          <img src="icon.svg" alt="" width="26" height="26" />
          <span>TEDROX Documents</span>
        </div>
        <nav>
          {navItems.map((item) => (
            <button
              key={item.id}
              className={view === item.id ? "nav-item active" : "nav-item"}
              onClick={() => {
                if (item.id === "doc" && !docState) {
                  newDocument();
                  return;
                }
                if (item.id === "sheet" && !sheetState) {
                  newSpreadsheet();
                  return;
                }
                setView(item.id);
              }}
            >
              {item.label}
            </button>
          ))}
        </nav>
        <p className="privacy-badge">{t("hero.trust.local")}</p>
      </aside>

      <main className="content">
        {view === "home" && (
          <section className="page">
            <h1>{t("app.homeTitle")}</h1>
            <p className="lede">{t("app.homeSubtitle")}</p>

            <div className="start-grid">
              <button className="start-card" onClick={newDocument}>
                <span className="start-icon" aria-hidden>
                  <svg viewBox="0 0 24 24" width="26" height="26">
                    <path
                      d="M6 2h8l4 4v16H6z"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="1.6"
                      strokeLinejoin="round"
                    />
                    <path d="M9 12h6M9 15h6M9 9h3" stroke="currentColor" strokeWidth="1.4" />
                  </svg>
                </span>
                <strong>{t("app.newDoc")}</strong>
                <span className="muted">{t("app.newDocDesc")}</span>
              </button>
              <button className="start-card" onClick={newSpreadsheet}>
                <span className="start-icon" aria-hidden>
                  <svg viewBox="0 0 24 24" width="26" height="26">
                    <rect x="3" y="4" width="18" height="16" rx="1.5" fill="none" stroke="currentColor" strokeWidth="1.6" />
                    <path d="M3 9h18M3 14h18M9 4v16M15 4v16" stroke="currentColor" strokeWidth="1.2" />
                  </svg>
                </span>
                <strong>{t("app.newSheet")}</strong>
                <span className="muted">{t("app.newSheetDesc")}</span>
              </button>
              <button className="start-card" onClick={openDialog}>
                <span className="start-icon" aria-hidden>
                  <svg viewBox="0 0 24 24" width="26" height="26">
                    <path
                      d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="1.6"
                      strokeLinejoin="round"
                    />
                  </svg>
                </span>
                <strong>{t("app.openFile")}</strong>
                <span className="muted">{t("app.openFileDesc")}</span>
              </button>
            </div>

            <div className="panel">
              <h2>{t("app.recent")}</h2>
              {recent.length === 0 && <p className="muted">{t("app.recentEmpty")}</p>}
              <ul className="recent-list">
                {recent.map((item) => (
                  <li key={item.path}>
                    <button className="link" onClick={() => openPath(item.path)}>
                      <span className="recent-kind">{item.kind === "doc" ? "DOCX" : "XLSX"}</span>
                      <span className="truncate">{fileName(item.path)}</span>
                    </button>
                    <span className="muted small truncate">{item.path}</span>
                  </li>
                ))}
              </ul>
            </div>

            <div className="panel">
              <h2>{t("app.extraTools")}</h2>
              <div className="chip-row">
                <button className="chip-btn" onClick={() => setView("pdf")}>
                  {t("pdf.title")}
                </button>
                <button className="chip-btn" onClick={() => setView("images")}>
                  {t("images.title")}
                </button>
                <button className="chip-btn" onClick={() => setView("convert")}>
                  {t("convert.title")}
                </button>
              </div>
            </div>
          </section>
        )}

        {view === "doc" && docState && (
          <DocumentEditor
            key={docState.key}
            path={docState.path}
            model={docState.model}
            displayName={docState.name}
            onSaved={remember}
          />
        )}

        {view === "sheet" && sheetState && (
          <SpreadsheetEditor
            key={sheetState.key}
            path={sheetState.path}
            workbook={sheetState.workbook}
            displayName={sheetState.name}
            onSaved={remember}
          />
        )}

        {view === "pdf" && <ToolsPage tools={PDF_TOOLS} titleKey="pdf.title" descKey="tools.pdfDesc" />}
        {view === "images" && (
          <ToolsPage tools={IMAGE_TOOLS} titleKey="images.title" descKey="tools.imagesDesc" />
        )}
        {view === "convert" && (
          <ToolsPage tools={CONVERT_TOOLS} titleKey="convert.title" descKey="convert.desc" />
        )}

        {view === "settings" && (
          <section className="page">
            <h1>{t("settings.title")}</h1>
            <div className="panel fields">
              <label>
                <span>{t("settings.language")}</span>
                <select value={language} onChange={(event) => setLanguageState(event.target.value as Language)}>
                  <option value="en">English</option>
                  <option value="ru">Русский</option>
                </select>
              </label>
              <label>
                <span>{t("settings.theme")}</span>
                <select value={theme} onChange={(event) => setTheme(event.target.value as typeof theme)}>
                  <option value="system">{t("settings.theme.system")}</option>
                  <option value="light">{t("settings.theme.light")}</option>
                  <option value="dark">{t("settings.theme.dark")}</option>
                </select>
              </label>
            </div>
            <div className="panel">
              <h2>{t("settings.about")}</h2>
              <p>{t("settings.version")}: 0.2.0 · MIT · space.tedrox.documents</p>
              <p className="muted">{t("settings.privacyNote")}</p>
            </div>
          </section>
        )}
      </main>

      {error && (
        <div className="alert error floating" onClick={() => setError(null)}>
          {error}
        </div>
      )}
    </div>
  );
}
