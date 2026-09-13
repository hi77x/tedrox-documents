import { useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { OpResult, Progress, formatBytes, runOperation, suggestOutput } from "./api";
import { t } from "./i18n";

export type ToolField =
  | { kind: "text"; key: string; labelKey: string; placeholderKey?: string; defaultValue?: string }
  | { kind: "number"; key: string; labelKey: string; defaultValue?: number }
  | {
      kind: "select";
      key: string;
      labelKey: string;
      options: { value: string; labelKey?: string }[];
      defaultValue?: string;
    };

export type Tool = {
  id: string;
  titleKey: string;
  descKey: string;
  command: string;
  multiple: boolean;
  suffix: string;
  extension: string;
  outputIsDirectory?: boolean;
  fields?: ToolField[];
  buildArgs: (files: string[], output: string, values: Record<string, string>) => Record<string, unknown>;
};

export const PDF_TOOLS: Tool[] = [
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
    fields: [
      {
        kind: "text",
        key: "pages",
        labelKey: "common.pages",
        placeholderKey: "pdf.pagesPlaceholder",
        defaultValue: "1-",
      },
    ],
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
];

export const IMAGE_TOOLS: Tool[] = [
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
        options: [
          { value: "png" },
          { value: "jpg" },
          { value: "webp" },
          { value: "bmp" },
          { value: "tiff" },
          { value: "avif" },
        ],
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
];

export const CONVERT_TOOLS: Tool[] = [
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
];

type ResultEntry = { title: string; result: OpResult };

export function ToolsPage({ tools, titleKey, descKey }: { tools: Tool[]; titleKey: string; descKey: string }) {
  const [activeTool, setActiveTool] = useState<Tool>(tools[0]);
  const [files, setFiles] = useState<string[]>([]);
  const [values, setValues] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [results, setResults] = useState<ResultEntry[]>([]);

  const selectFiles = async () => {
    const selection = await open({ multiple: true, title: t("common.addFiles") });
    if (!selection) return;
    const paths = Array.isArray(selection) ? selection : [selection];
    setFiles((previous) => [...new Set([...previous, ...paths])]);
  };

  const runTool = async (tool: Tool) => {
    if (files.length === 0) return;
    setBusy(true);
    setError(null);
    setProgress(null);
    try {
      let output: string | null = null;
      if (tool.outputIsDirectory) {
        output = await open({ directory: true, title: t("common.chooseOutput") });
      } else {
        const suggested = await suggestOutput(files[0], tool.suffix, tool.extension);
        output = await save({
          defaultPath: suggested,
          title: t("common.chooseOutput"),
          filters: [{ name: tool.extension.toUpperCase(), extensions: [tool.extension] }],
        });
      }
      if (!output) return;
      const result = await runOperation({
        command: tool.command,
        args: tool.buildArgs(files, output, values),
        onProgress: setProgress,
      });
      setResults((previous) => [{ title: t(tool.titleKey), result }, ...previous].slice(0, 20));
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
      setProgress(null);
    }
  };

  return (
    <section className="page">
      <h1>{t(titleKey)}</h1>
      <p className="lede">{t(descKey)}</p>

      <div className="tool-tabs">
        {tools.map((tool) => (
          <button
            key={tool.id}
            className={tool.id === activeTool.id ? "tool-tab active" : "tool-tab"}
            onClick={() => {
              setActiveTool(tool);
              setFiles([]);
              setValues({});
              setError(null);
            }}
          >
            {t(tool.titleKey)}
          </button>
        ))}
      </div>

      <div className="panel">
        <p className="muted tool-desc">{t(activeTool.descKey)}</p>
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
                <button
                  className="link"
                  onClick={() => setFiles((previous) => previous.filter((item) => item !== file))}
                >
                  {t("common.clear")}
                </button>
              </li>
            ))}
          </ul>
        )}

        {activeTool.fields && activeTool.fields.length > 0 && (
          <div className="fields">
            {activeTool.fields.map((field) => (
              <label key={field.key}>
                <span>{t(field.labelKey)}</span>
                {field.kind === "select" ? (
                  <select
                    value={values[field.key] ?? field.defaultValue ?? ""}
                    onChange={(event) =>
                      setValues((previous) => ({ ...previous, [field.key]: event.target.value }))
                    }
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
                    onChange={(event) =>
                      setValues((previous) => ({ ...previous, [field.key]: event.target.value }))
                    }
                  />
                )}
              </label>
            ))}
          </div>
        )}

        <div className="actions-row">
          <button className="btn primary" onClick={() => runTool(activeTool)} disabled={busy || files.length === 0}>
            {busy ? t("common.running") : t("common.run")}
          </button>
        </div>

        {progress && (
          <div className="progress-wrap">
            <div className="progress-bar" style={{ width: `${Math.round(progress.progress * 100)}%` }} />
            <span className="muted small">
              {progress.stage} · {Math.round(progress.progress * 100)}%
            </span>
          </div>
        )}

        {error && <div className="alert error">{error}</div>}
      </div>

      {results.length > 0 && (
        <div className="panel">
          <h2>{t("home.recent")}</h2>
          {results.map((entry, index) => (
            <div className="result-card" key={index}>
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
          ))}
        </div>
      )}
    </section>
  );
}
