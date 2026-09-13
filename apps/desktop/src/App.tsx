import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open, save } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  FileInfo,
  OpResult,
  Progress,
  formatBytes,
  inspectFiles,
  pdfMetadata,
  runOperation,
  suggestOutput,
} from "./api";
import { Language, detectLanguage, setLanguage, t } from "./i18n";

type Field =
  | { kind: "text"; key: string; labelKey: string; placeholderKey?: string; defaultValue?: string }
  | { kind: "number"; key: string; labelKey: string; defaultValue?: number }
  | { kind: "select"; key: string; labelKey: string; options: { value: string; labelKey?: string }[]; defaultValue?: string };

type Tool = {
  id: string;
  titleKey: string;
  descKey: string;
  command: string;
  multiple: boolean;
  suffix: string;
  extension: string;
  outputIsDirectory?: boolean;
  fields?: Field[];
  buildArgs: (files: string[], output: string, values: Record<string, string>) => Record<string, unknown>;
};

const TOOLS: Record<string, Tool[]> = {
  pdf: [
    {
      id: "pdf.merge",
      titleKey: "pdf.merge",
      descKey: "pdf.mergeDesc",
      command: "pdf_merge",
      multiple: true,
      suffix: "merged",
      extension: "pdf",
      buildArgs: (files, output) => ({ files, output }),
    },
    {
      id: "pdf.split",
      titleKey: "pdf.split",
      descKey: "pdf.splitDesc",
      command: "pdf_split",
      multiple: false,
      suffix: "parts",
      extension: "pdf",
      outputIsDirectory: true,
      fields: [{ kind: "number", key: "chunk", labelKey: "pdf.chunk", defaultValue: 1 }],
      buildArgs: (files, output, values) => ({
        file: files[0],
        outputDir: output,
        chunk: Number(values.chunk || "1"),
      }),
    },
    {
      id: "pdf.extract",
      titleKey: "pdf.extract",
      descKey: "pdf.extractDesc",
      command: "pdf_extract",
      multiple: false,
      suffix: "extracted",
      extension: "pdf",
      fields: [{ kind: "text", key: "pages", labelKey: "common.pages", placeholderKey: "pdf.pagesPlaceholder", defaultValue: "1-" }],
      buildArgs: (files, output, values) => ({ file: files[0], pages: values.pages || "all", output }),
    },
    {
      id: "pdf.clean",
      titleKey: "pdf.clean",
      descKey: "pdf.cleanDesc",
      command: "pdf_clean",
      multiple: false,
      suffix: "clean",
      extension: "pdf",
      buildArgs: (files, output) => ({ file: files[0], output }),
    },
    {
      id: "pdf.compress",
      titleKey: "pdf.compress",
      descKey: "pdf.compressDesc",
      command: "pdf_compress",
      multiple: false,
      suffix: "compressed",
      extension: "pdf",
      fields: [
        {
          kind: "select",
          key: "preset",
          labelKey: "images.mode",
          options: [{ value: "screen" }, { value: "balanced" }, { value: "print" }],
          defaultValue: "balanced",
        },
      ],
      buildArgs: (files, output, values) => ({
        file: files[0],
        output,
        preset: values.preset || "balanced",
        quality: 75,
        maxDimension: 1600,
      }),
    },
    {
      id: "pdf.watermark",
      titleKey: "pdf.watermark",
      descKey: "pdf.watermarkDesc",
      command: "pdf_watermark",
      multiple: false,
      suffix: "stamped",
      extension: "pdf",
      fields: [
        { kind: "text", key: "text", labelKey: "pdf.watermarkText", defaultValue: "CONFIDENTIAL" },
        { kind: "number", key: "opacity", labelKey: "images.mode", defaultValue: 0.15 },
      ],
      buildArgs: (files, output, values) => ({
        file: files[0],
        output,
        text: values.text || "CONFIDENTIAL",
        opacity: Math.min(1, Math.max(0.02, Number(values.opacity || "0.15"))),
      }),
    },
  ],
  images: [
    {
      id: "image.convert",
      titleKey: "images.convert",
      descKey: "images.convertDesc",
      command: "image_convert",
      multiple: false,
      suffix: "converted",
      extension: "webp",
      fields: [
        {
          kind: "select",
          key: "to",
          labelKey: "common.output",
          options: [{ value: "png" }, { value: "jpg" }, { value: "webp" }, { value: "bmp" }, { value: "tiff" }, { value: "avif" }],
          defaultValue: "webp",
        },
        { kind: "number", key: "quality", labelKey: "images.mode", defaultValue: 90 },
      ],
      buildArgs: (files, output, values) => ({
        file: files[0],
        to: values.to || "webp",
        output,
        quality: Number(values.quality || "90"),
      }),
    },
    {
      id: "image.resize",
      titleKey: "images.resize",
      descKey: "images.resizeDesc",
      command: "image_resize",
      multiple: false,
      suffix: "resized",
      extension: "png",
      fields: [
        { kind: "number", key: "width", labelKey: "images.width", defaultValue: 1280 },
        { kind: "number", key: "height", labelKey: "images.height", defaultValue: 0 },
        {
          kind: "select",
          key: "mode",
          labelKey: "images.mode",
          options: [{ value: "contain" }, { value: "cover" }, { value: "exact" }],
          defaultValue: "contain",
        },
      ],
      buildArgs: (files, output, values) => ({
        file: files[0],
        output,
        width: Number(values.width || "0") || null,
        height: Number(values.height || "0") || null,
        mode: values.mode || "contain",
      }),
    },
  ],
  sheets: [
    {
      id: "sheet.csv_to_xlsx",
      titleKey: "sheets.csvToXlsx",
      descKey: "sheets.csvToXlsxDesc",
      command: "csv_to_xlsx",
      multiple: false,
      suffix: "data",
      extension: "xlsx",
      buildArgs: (files, output) => ({ file: files[0], output }),
    },
    {
      id: "sheet.xlsx_to_csv",
      titleKey: "sheets.xlsxToCsv",
      descKey: "sheets.xlsxToCsvDesc",
      command: "xlsx_to_csv",
      multiple: false,
      suffix: "sheet",
      extension: "csv",
      buildArgs: (files, output) => ({ file: files[0], output }),
    },
  ],
  convert: [
    {
      id: "convert.auto",
      titleKey: "convert.run",
      descKey: "convert.desc",
      command: "convert_auto",
      multiple: false,
      suffix: "converted",
      extension: "pdf",
      buildArgs: (files, output) => ({ file: files[0], output }),
    },
  ],
};

type Tab = "home" | "pdf" | "images" | "sheets" | "convert" | "settings";

type ResultEntry = { title: string; result: OpResult };

export default function App() {
  const [language, setLanguageState] = useState<Language>(() => detectLanguage());
  const [tab, setTab] = useState<Tab>("home");
  const [theme, setTheme] = useState<"system" | "light" | "dark">("system");
  const [files, setFiles] = useState<string[]>([]);
  const [inspected, setInspected] = useState<FileInfo[]>([]);
  const [metadata, setMetadata] = useState<Record<string, unknown> | null>(null);
  const [values, setValues] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [results, setResults] = useState<ResultEntry[]>([]);
  const resultsRef = useRef<ResultEntry[]>([]);

  useEffect(() => {
    setLanguage(language);
  }, [language]);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  const tool = useMemo(() => {
    if (tab === "home" || tab === "settings") return null;
    return TOOLS[tab]?.[0] ?? null;
  }, [tab]);

  const selectFiles = useCallback(async () => {
    const selection = await open({ multiple: true, title: t("home.open") });
    if (!selection) return;
    const paths = Array.isArray(selection) ? selection : [selection];
    setFiles((previous) => [...new Set([...previous, ...paths])]);
  }, []);

  useEffect(() => {
    if (files.length === 0) {
      setInspected([]);
      setMetadata(null);
      return;
    }
    inspectFiles(files.slice(0, 40))
      .then(setInspected)
      .catch((err) => setError(String(err)));
    const first = files[0];
    if (first.toLowerCase().endsWith(".pdf")) {
      pdfMetadata(first)
        .then(setMetadata)
        .catch(() => setMetadata(null));
    } else {
      setMetadata(null);
    }
  }, [files]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "drop") {
          const paths = event.payload.paths;
          if (paths.length > 0) {
            setFiles((previous) => [...new Set([...previous, ...paths])]);
            setTab((current) => (current === "settings" ? "home" : current));
          }
        }
      })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {});
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  const runTool = useCallback(
    async (definition: Tool) => {
      if (files.length === 0) return;
      setBusy(true);
      setError(null);
      setProgress(null);
      try {
        let output: string | null = null;
        if (definition.outputIsDirectory) {
          output = await open({ directory: true, title: t("common.chooseOutput") });
        } else {
          const suggested = await suggestOutput(files[0], definition.suffix, definition.extension);
          output = await save({
            defaultPath: suggested,
            title: t("common.chooseOutput"),
            filters: [{ name: definition.extension.toUpperCase(), extensions: [definition.extension] }],
          });
        }
        if (!output) return;
        const result = await runOperation({
          command: definition.command,
          args: definition.buildArgs(files, output as string, values),
          onProgress: setProgress,
        });
        const entry = { title: t(definition.titleKey), result };
        resultsRef.current = [entry, ...resultsRef.current].slice(0, 30);
        setResults(resultsRef.current);
      } catch (err) {
        setError(String(err));
      } finally {
        setBusy(false);
        setProgress(null);
      }
    },
    [files, values],
  );

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand">
          <img src="icon.svg" alt="" width="26" height="26" />
          <span>TEDROX Documents</span>
        </div>
        <nav>
          {(
            [
              ["home", "nav.home"],
              ["pdf", "PDF"],
              ["images", "nav.images"],
              ["sheets", "nav.sheets"],
              ["convert", "nav.convert"],
              ["settings", "nav.settings"],
            ] as [Tab, string][]
          ).map(([id, label]) => (
            <button
              key={id}
              className={tab === id ? "nav-item active" : "nav-item"}
              onClick={() => {
                setTab(id);
                setFiles([]);
                setValues({});
                setError(null);
              }}
            >
              {label === "PDF" ? "PDF" : t(label)}
            </button>
          ))}
        </nav>
        <p className="privacy-badge">{t("hero.trust.local")}</p>
      </aside>

      <main className="content">
        {tab === "home" && (
          <section>
            <h1>{t("home.title")}</h1>
            <p className="lede">{t("home.subtitle")}</p>
            <div className="actions-row">
              <button className="btn primary" onClick={selectFiles}>
                {t("home.open")}
              </button>
              {files.length > 0 && (
                <button className="btn" onClick={() => setFiles([])}>
                  {t("common.clear")}
                </button>
              )}
            </div>
            {inspected.length > 0 && (
              <div className="panel">
                <h2>{t("home.inspect")}</h2>
                <table className="table">
                  <thead>
                    <tr>
                      <th>File</th>
                      <th>{t("common.detected")}</th>
                      <th>{t("common.size")}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {inspected.map((item) => (
                      <tr key={item.path}>
                        <td className="truncate">{item.name}</td>
                        <td>
                          <span className="chip">{item.kind}</span>
                        </td>
                        <td>{formatBytes(item.size)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
            <div className="panel">
              <h2>{t("home.tools")}</h2>
              <div className="tool-grid">
                {Object.entries(TOOLS).flatMap(([group, list]) =>
                  list.map((definition) => (
                    <button
                      key={definition.id}
                      className="tool-card"
                      onClick={() => {
                        setTab(group as Tab);
                        setFiles([]);
                        setValues({});
                      }}
                    >
                      <strong>{t(definition.titleKey)}</strong>
                      <span>{t(definition.descKey)}</span>
                    </button>
                  )),
                )}
              </div>
            </div>
            <div className="panel">
              <h2>{t("home.recent")}</h2>
              {results.length === 0 && <p className="muted">{t("home.empty")}</p>}
              {results.map((entry, index) => (
                <ResultCard key={index} entry={entry} />
              ))}
              {results.length > 0 && <p className="muted small">{t("jobs.localNote")}</p>}
            </div>
          </section>
        )}

        {tab !== "home" && tab !== "settings" && tool && (
          <section>
            <h1>{t(tool.titleKey)}</h1>
            <p className="lede">{t(tool.descKey)}</p>

            <div className="actions-row">
              <button className="btn primary" onClick={selectFiles} disabled={busy}>
                {t("common.addFiles")}
              </button>
              {files.length > 0 && (
                <button className="btn" onClick={() => setFiles([])} disabled={busy}>
                  {t("common.clear")}
                </button>
              )}
            </div>

            {files.length > 0 && (
              <ul className="file-list">
                {files.map((file) => (
                  <li key={file}>
                    <span className="truncate">{file}</span>
                    <button className="link" onClick={() => setFiles((previous) => previous.filter((item) => item !== file))}>
                      {t("common.clear")}
                    </button>
                  </li>
                ))}
              </ul>
            )}

            {tool.fields && tool.fields.length > 0 && (
              <div className="panel fields">
                {tool.fields.map((field) => (
                  <label key={field.key}>
                    <span>{t(field.labelKey)}</span>
                    {field.kind === "select" ? (
                      <select
                        value={values[field.key] ?? field.defaultValue ?? ""}
                        onChange={(event) => setValues((previous) => ({ ...previous, [field.key]: event.target.value }))}
                      >
                        {field.options.map((option) => (
                          <option key={option.value} value={option.value}>
                            {option.labelKey ? t(option.labelKey) : option.value}
                          </option>
                        ))}
                      </select>
                    ) : (
                      <input
                        type={field.kind === "number" ? "number" : "text"}
                        step={field.kind === "number" ? "any" : undefined}
                        placeholder={field.kind === "text" && field.placeholderKey ? t(field.placeholderKey) : undefined}
                        value={values[field.key] ?? (field.defaultValue !== undefined ? String(field.defaultValue) : "")}
                        onChange={(event) => setValues((previous) => ({ ...previous, [field.key]: event.target.value }))}
                      />
                    )}
                  </label>
                ))}
              </div>
            )}

            <div className="actions-row">
              <button className="btn primary" onClick={() => runTool(tool)} disabled={busy || files.length === 0}>
                {busy ? t("common.running") : t("common.run")}
              </button>
            </div>

            {progress && (
              <div className="progress-wrap">
                <div className="progress-bar" style={{ width: `${Math.round(progress.progress * 100)}%` }} />
                <span className="muted small">
                  {progress.stage} · {Math.round(progress.progress * 100)}% {progress.message ?? ""}
                </span>
              </div>
            )}

            {error && <div className="alert error">{error}</div>}

            {metadata && tab === "pdf" && files[0]?.toLowerCase().endsWith(".pdf") && (
              <div className="panel">
                <h2>{t("home.inspect")}</h2>
                <dl className="meta-grid">
                  {Object.entries(metadata).map(([key, value]) => (
                    <div key={key}>
                      <dt>{key}</dt>
                      <dd>{String(value ?? "—")}</dd>
                    </div>
                  ))}
                </dl>
              </div>
            )}

            <div className="panel">
              <h2>{t("home.recent")}</h2>
              {results.length === 0 && <p className="muted">{t("home.empty")}</p>}
              {results.map((entry, index) => (
                <ResultCard key={index} entry={entry} />
              ))}
            </div>
          </section>
        )}

        {tab === "settings" && (
          <section>
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
              <p>
                {t("settings.version")}: 0.1.0 · MIT · space.tedrox.documents
              </p>
              <p className="muted">{t("settings.privacyNote")}</p>
            </div>
          </section>
        )}
      </main>
    </div>
  );
}

function ResultCard({ entry }: { entry: ResultEntry }) {
  return (
    <div className="result-card">
      <div className="result-head">
        <strong>{entry.title}</strong>
        <span className="muted small">
          {entry.result.durationMs} ms · {formatBytes(entry.result.bytesOut)}
        </span>
      </div>
      <ul>
        {entry.result.outputs.map((output) => (
          <li key={output.path}>
            <span className="truncate">{output.path}</span>
            <button className="link" onClick={() => revealItemInDir(output.path)}>
              {t("common.save")}
            </button>
          </li>
        ))}
      </ul>
      {entry.result.warnings.map((warning, index) => (
        <p key={index} className="alert warning">
          {warning}
        </p>
      ))}
    </div>
  );
}
