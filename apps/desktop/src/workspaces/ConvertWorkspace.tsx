import { useMemo, useState } from "react";
import { classify, fileName, formatBytes, type FileInfo } from "../platform";
import { useStore } from "../state";
import {
  IconCompress,
  IconConvert,
  IconCrop,
  IconDoc,
  IconExtract,
  IconImage,
  IconMerge,
  IconPdf,
  IconRefresh,
  IconResize,
  IconSheet,
  IconShield,
  IconSplit,
  IconTrash,
} from "../design/icons";

export type ConvertOp = {
  id: string;
  label: string;
  kind: "document" | "sheet" | "pdf" | "image" | "any";
  command: string;
  /** Extension of the produced file; omit for directory outputs. */
  extension?: string;
  suffix: string;
  /** Ask for an output directory and apply to every matching file. */
  batch?: boolean;
  directory?: boolean;
  args?: Record<string, unknown>;
  hint?: string;
};

export const CONVERT_OPS: ConvertOp[] = [
  { id: "doc.to.pdf", label: "Export PDF", kind: "document", command: "convert_auto", extension: "pdf", suffix: "export" },
  { id: "doc.to.txt", label: "Extract text", kind: "document", command: "doc_extract", extension: "txt", suffix: "text", args: { markdown: false } },
  { id: "doc.to.md", label: "Extract Markdown", kind: "document", command: "doc_extract", extension: "md", suffix: "markdown", args: { markdown: true } },
  { id: "md.to.docx", label: "Create DOCX", kind: "document", command: "doc_from_markdown", extension: "docx", suffix: "document" },
  { id: "md.to.html", label: "Create HTML", kind: "document", command: "markdown_to_html", extension: "html", suffix: "page" },
  { id: "sheet.to.xlsx", label: "Convert to XLSX", kind: "sheet", command: "csv_to_xlsx", extension: "xlsx", suffix: "workbook" },
  { id: "sheet.to.csv", label: "Convert to CSV", kind: "sheet", command: "xlsx_to_csv", extension: "csv", suffix: "data" },
  { id: "pdf.compress", label: "Compress", kind: "pdf", command: "pdf_compress", extension: "pdf", suffix: "compressed", args: { preset: "balanced", quality: 75, maxDimension: 1600 } },
  { id: "pdf.clean", label: "Privacy clean", kind: "pdf", command: "pdf_clean", extension: "pdf", suffix: "clean" },
  { id: "pdf.split", label: "Split pages", kind: "pdf", command: "pdf_split", suffix: "parts", batch: true, directory: true, args: { chunk: 1 } },
  { id: "image.webp", label: "→ WebP", kind: "image", command: "image_convert", extension: "webp", suffix: "converted", args: { to: "webp", quality: 90 } },
  { id: "image.png", label: "→ PNG", kind: "image", command: "image_convert", extension: "png", suffix: "converted", args: { to: "png", quality: 90 } },
  { id: "image.jpg", label: "→ JPEG", kind: "image", command: "image_convert", extension: "jpg", suffix: "converted", args: { to: "jpg", quality: 90 } },
  { id: "image.avif", label: "→ AVIF", kind: "image", command: "image_convert", extension: "avif", suffix: "converted", args: { to: "avif", quality: 80 } },
  { id: "image.resize", label: "Resize", kind: "image", command: "image_resize", extension: "png", suffix: "resized", args: { width: 1280, height: null, mode: "contain" } },
  { id: "image.rotate", label: "Rotate 90°", kind: "image", command: "image_rotate", suffix: "rotated", args: { degrees: 90, flipHorizontal: false, flipVertical: false } },
  { id: "image.strip", label: "Strip metadata", kind: "image", command: "image_strip_metadata", suffix: "clean" },
];

export function ConvertWorkspace({ mode }: { mode: "convert" | "images" }) {
  const { platform, runJob, pushToast } = useStore();
  const [files, setFiles] = useState<string[]>([]);
  const [info, setInfo] = useState<FileInfo[]>([]);
  const [over, setOver] = useState(false);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<{ file: string; value: number } | null>(null);
  const [results, setResults] = useState<{ file: string; text: string; ok: boolean }[]>([]);
  const [crop, setCrop] = useState({ x: 0, y: 0, width: 512, height: 512 });
  const [removeSource, setRemoveSource] = useState(false);

  const visibleOps = useMemo(
    () => CONVERT_OPS.filter((op) => (mode === "images" ? op.kind === "image" : op.kind !== "image")),
    [mode],
  );

  const addPaths = async (paths: string[]) => {
    if (paths.length === 0) return;
    setFiles((current) => [...new Set([...current, ...paths])]);
    try {
      const detected = await platform.invoke<FileInfo[]>("inspect_files", { paths });
      setInfo((current) => [...current.filter((item) => !paths.includes(item.path)), ...detected]);
    } catch {
      /* detection is best-effort; classify() still works from the extension */
    }
  };

  const addViaDialog = async () => {
    const picked = await platform.openPaths({ multiple: true, title: "Add files" });
    if (picked) await addPaths(picked);
  };

  const kindOf = (path: string) => {
    const detected = info.find((item) => item.path === path);
    if (detected) {
      if (detected.category === "pdf") return "pdf";
      if (detected.category === "image") return "image";
      if (detected.category === "spreadsheet") return "sheet";
      if (detected.category === "document") return "document";
    }
    return classify(path) === "pdf"
      ? "pdf"
      : classify(path) === "image"
        ? "image"
        : classify(path) === "sheet"
          ? "sheet"
          : "document";
  };

  const runOp = async (op: ConvertOp) => {
    const matching = files.filter((file) => kindOf(file) === op.kind);
    if (matching.length === 0) {
      pushToast({ kind: "warn", message: `No ${op.kind} files selected` });
      return;
    }

    if (op.batch || matching.length > 1) {
      const directory = await platform.openPaths({ directory: true, title: "Choose an output folder" });
      if (!directory?.[0]) return;
      const separator = directory[0].includes("\\") ? "\\" : "/";
      setBusy(true);
      try {
        for (const file of matching) {
          const stem = fileName(file).replace(/\.[^.]+$/, "");
          const extension = op.extension ?? "pdf";
          const output = `${directory[0]}${separator}${stem}-${op.suffix}.${extension}`;
          setProgress({ file: fileName(file), value: 0 });
          await runJob(op.label, fileName(file), op.command, {
            file,
            output,
            ...(op.directory ? { outputDir: directory[0] } : {}),
            ...(op.args ?? {}),
            ...(op.command === "pdf_split"
              ? { outputDir: directory[0], chunk: 1 }
              : {}),
          }, { quiet: true });
          setResults((current) => [{ file: fileName(file), text: output, ok: true }, ...current].slice(0, 60));
          setProgress(null);
          if (removeSource) {
            try {
              await platform.invoke("delete_file", { path: file });
            } catch {
              /* deletion stays opt-in and best-effort */
            }
          }
        }
        pushToast({ kind: "success", message: `${op.label} finished`, detail: `${matching.length} files` });
      } catch (error) {
        pushToast({ kind: "error", message: `${op.label} failed`, detail: String(error) });
      } finally {
        setBusy(false);
        setProgress(null);
      }
      return;
    }

    const file = matching[0];
    if (op.command === "doc_extract") {
      const extension = op.extension ?? "txt";
      const suggested = await platform.suggestOutput(file, op.suffix, extension);
      const target = await platform.savePath(suggested, extension, extension.toUpperCase());
      if (!target) return;
      setBusy(true);
      try {
        await platform.invoke("doc_extract", {
          file,
          output: target,
          markdown: Boolean(op.args?.markdown),
        });
        pushToast({ kind: "success", message: op.label, detail: target });
        setResults((current) => [{ file: fileName(file), text: target, ok: true }, ...current].slice(0, 60));
      } catch (error) {
        pushToast({ kind: "error", message: op.label, detail: String(error) });
      } finally {
        setBusy(false);
      }
      return;
    }

    if (op.command === "markdown_to_html") {
      const suggested = await platform.suggestOutput(file, op.suffix, "html");
      const target = await platform.savePath(suggested, "html", "HTML");
      if (!target) return;
      setBusy(true);
      try {
        await platform.invoke("markdown_to_html", { file, output: target });
        pushToast({ kind: "success", message: op.label, detail: target });
      } catch (error) {
        pushToast({ kind: "error", message: op.label, detail: String(error) });
      } finally {
        setBusy(false);
      }
      return;
    }

    const extension = op.extension ?? "pdf";
    const suggested = await platform.suggestOutput(file, op.suffix, extension);
    const target = await platform.savePath(suggested, extension, extension.toUpperCase());
    if (!target) return;
    setBusy(true);
    try {
      await runJob(op.label, fileName(file), op.command, {
        file,
        output: target,
        ...(op.args ?? {}),
      });
      setResults((current) => [{ file: fileName(file), text: target, ok: true }, ...current].slice(0, 60));
    } catch (error) {
      setResults((current) => [{ file: fileName(file), text: String(error), ok: false }, ...current].slice(0, 60));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="home" style={{ padding: 22 }}>
      <div className="home-inner">
        <h1 style={{ fontSize: 20 }}>{mode === "images" ? "Images" : "Convert"}</h1>
        <p className="home-lede" style={{ marginBottom: 16 }}>
          {mode === "images"
            ? "Convert, resize, rotate and clean images locally."
            : "Detect file types, choose an operation and process many files at once. Nothing leaves this machine."}
        </p>

        <div
          className={`dropzone${over ? " over" : ""}`}
          onDragOver={(event) => {
            event.preventDefault();
            setOver(true);
          }}
          onDragLeave={() => setOver(false)}
          onDrop={async (event) => {
            event.preventDefault();
            setOver(false);
            const paths = Array.from(event.dataTransfer.files)
              .map((file) => (file as File & { path?: string }).path)
              .filter((value): value is string => Boolean(value));
            await addPaths(paths);
          }}
        >
          Drop files here, or{" "}
          <button className="link" onClick={addViaDialog}>
            choose files
          </button>
          . Detected types appear below with the operations that are really implemented.
        </div>

        {files.length > 0 && (
          <>
            <div className="row" style={{ marginBottom: 10 }}>
              <span className="muted small">{files.length} file(s)</span>
              <button className="btn" onClick={() => { setFiles([]); setInfo([]); setResults([]); }}>
                Clear
              </button>
              <label className="checkbox">
                <input type="checkbox" checked={removeSource} onChange={(event) => setRemoveSource(event.target.checked)} />
                Delete source after success
              </label>
              {busy && progress ? (
                <span className="muted small">
                  Working on {progress.file}…
                </span>
              ) : null}
            </div>

            <div className="convert-grid">
              {files.map((file) => {
                const kind = kindOf(file);
                const detected = info.find((item) => item.path === file);
                const ops = visibleOps.filter((op) => op.kind === kind);
                return (
                  <div className="file-card" key={file}>
                    <header>
                      {kind === "pdf" ? <IconPdf /> : kind === "image" ? <IconImage /> : kind === "sheet" ? <IconSheet /> : <IconDoc />}
                      <div style={{ minWidth: 0, flex: 1 }}>
                        <div className="name" title={file}>
                          {fileName(file)}
                        </div>
                        <div className="path" title={file}>
                          {detected ? `${detected.kind} · ${formatBytes(detected.size)}` : kind}
                        </div>
                      </div>
                      <button className="icon-btn" title="Remove" onClick={() => setFiles((current) => current.filter((item) => item !== file))}>
                        <IconTrash />
                      </button>
                    </header>
                    <div className="op-row">
                      {ops.map((op) => (
                        <button key={op.id} className="op-chip" onClick={() => void runOp(op)} disabled={busy} title={op.hint}>
                          {op.label}
                        </button>
                      ))}
                      {kind === "image" && (
                        <button
                          className="op-chip"
                          onClick={async () => {
                            const suggested = await platform.suggestOutput(file, "cropped", "png");
                            const target = await platform.savePath(suggested, "png", "PNG");
                            if (!target) return;
                            setBusy(true);
                            try {
                              await runJob("Crop image", fileName(file), "image_crop", {
                                file,
                                output: target,
                                x: crop.x,
                                y: crop.y,
                                width: crop.width,
                                height: crop.height,
                              });
                            } finally {
                              setBusy(false);
                            }
                          }}
                          disabled={busy}
                        >
                          <IconCrop /> Crop {crop.width}×{crop.height}
                        </button>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>

            {files.some((file) => kindOf(file) === "image") && mode === "images" && (
              <div className="panel" style={{ marginTop: 16 }}>
                <p className="panel-title">Crop settings (used by the Crop chip)</p>
                <div className="row wrap">
                  {(["x", "y", "width", "height"] as const).map((field) => (
                    <label className="field" key={field}>
                      <span>{field}</span>
                      <input
                        className="number-input"
                        type="number"
                        min={0}
                        value={crop[field]}
                        onChange={(event) => setCrop((current) => ({ ...current, [field]: Number(event.target.value) }))}
                      />
                    </label>
                  ))}
                </div>
              </div>
            )}
          </>
        )}

        {mode === "images" && (
          <div className="home-grid" style={{ marginTop: 20 }}>
            <div className="action-card">
              <span className="glyph">
                <IconResize />
              </span>
              <span>
                <strong>Resize</strong>
                <span>Contain, cover or exact pixels</span>
              </span>
            </div>
            <div className="action-card">
              <span className="glyph">
                <IconRefresh />
              </span>
              <span>
                <strong>Rotate and flip</strong>
                <span>90° steps, mirrored output</span>
              </span>
            </div>
            <div className="action-card">
              <span className="glyph">
                <IconShield />
              </span>
              <span>
                <strong>Strip metadata</strong>
                <span>Remove EXIF without losing pixels</span>
              </span>
            </div>
          </div>
        )}

        {mode === "convert" && (
          <div className="home-grid" style={{ marginTop: 20 }}>
            <div className="action-card">
              <span className="glyph">
                <IconConvert />
              </span>
              <span>
                <strong>Real conversions only</strong>
                <span>Unimplemented routes are absent, not stubbed</span>
              </span>
            </div>
            <div className="action-card">
              <span className="glyph">
                <IconMerge />
              </span>
              <span>
                <strong>Merge PDFs</strong>
                <span>Open the PDF workspace to combine files</span>
              </span>
            </div>
            <div className="action-card">
              <span className="glyph">
                <IconSplit />
              </span>
              <span>
                <strong>Split or extract</strong>
                <span>Every page, chunks or page ranges</span>
              </span>
            </div>
            <div className="action-card">
              <span className="glyph">
                <IconCompress />
              </span>
              <span>
                <strong>Compress</strong>
                <span>Screen, balanced or print presets</span>
              </span>
            </div>
          </div>
        )}

        {results.length > 0 && (
          <div className="panel" style={{ marginTop: 20 }}>
            <p className="panel-title">Results</p>
            <div className="table-shell">
              <table>
                <thead>
                  <tr>
                    <th>File</th>
                    <th>Outcome</th>
                  </tr>
                </thead>
                <tbody>
                  {results.map((entry, index) => (
                    <tr key={index}>
                      <td>{entry.file}</td>
                      <td style={{ color: entry.ok ? "var(--ok)" : "var(--danger)" }}>
                        {entry.ok ? (
                          <span className="row">
                            <IconExtract /> {entry.text}
                          </span>
                        ) : (
                          entry.text
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

export function opCount(): number {
  return CONVERT_OPS.length;
}
