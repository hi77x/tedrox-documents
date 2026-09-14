import { useMemo, useState } from "react";
import { classify, fileName, formatBytes, type FileInfo } from "../platform";
import { useStore } from "../state";
import {
  IconConvert,
  IconDoc,
  IconFolderOpen,
  IconImage,
  IconMerge,
  IconPdf,
  IconSheet,
  IconSplit,
  IconStar,
} from "../design/icons";

export function HomeView({
  onOpenFiles,
  onNewDocument,
  onNewSpreadsheet,
  onOpenPath,
  onOpenTab,
}: {
  onOpenFiles: () => void;
  onNewDocument: () => void;
  onNewSpreadsheet: () => void;
  onOpenPath: (path: string) => void;
  onOpenTab: (kind: "pdf" | "convert" | "images") => void;
}) {
  const { recent, pinned, togglePinned, platform, pushToast, jobs } = useStore();
  const [over, setOver] = useState(false);
  const [inspection, setInspection] = useState<FileInfo[]>([]);

  const pinnedEntries = useMemo(() => recent.filter((entry) => pinned.includes(entry.path)), [recent, pinned]);

  const handleDrop = async (event: React.DragEvent) => {
    event.preventDefault();
    setOver(false);
    const paths = Array.from(event.dataTransfer.files)
      .map((file) => (file as File & { path?: string }).path)
      .filter((value): value is string => Boolean(value));
    if (paths.length === 0) {
      pushToast({
        kind: "warn",
        message: "Could not read the dropped files",
        detail: "Drop files onto the window from Explorer, or use Open file.",
      });
      return;
    }
    if (paths.length === 1) {
      onOpenPath(paths[0]);
      return;
    }
    try {
      const info = await platform.invoke<FileInfo[]>("inspect_files", { paths });
      setInspection(info);
    } catch (error) {
      pushToast({ kind: "error", message: "Cannot inspect files", detail: String(error) });
    }
  };

  const kindIcon = (kind: string) => {
    if (kind === "document") return <IconDoc />;
    if (kind === "sheet") return <IconSheet />;
    if (kind === "pdf") return <IconPdf />;
    return <IconImage />;
  };

  return (
    <div
      className="home"
      onDragOver={(event) => {
        event.preventDefault();
        setOver(true);
      }}
      onDragLeave={() => setOver(false)}
      onDrop={handleDrop}
    >
      <div className="home-inner">
        <h1>One app for documents.</h1>
        <p className="home-lede">
          Open, edit, convert and organize PDF, Word, spreadsheets and images — locally, without uploading
          anything.
        </p>

        <div className="home-grid">
          <button className="action-card" onClick={onNewDocument}>
            <span className="glyph">
              <IconDoc />
            </span>
            <span>
              <strong>New document</strong>
              <span>Word document with real OOXML output</span>
            </span>
          </button>
          <button className="action-card" onClick={onNewSpreadsheet}>
            <span className="glyph">
              <IconSheet />
            </span>
            <span>
              <strong>New spreadsheet</strong>
              <span>Workbook with formulas and formatting</span>
            </span>
          </button>
          <button className="action-card" onClick={onOpenFiles}>
            <span className="glyph">
              <IconFolderOpen />
            </span>
            <span>
              <strong>Open file</strong>
              <span>DOCX, XLSX, CSV, PDF, images</span>
            </span>
          </button>
          <button className="action-card" onClick={() => onOpenTab("convert")}>
            <span className="glyph">
              <IconConvert />
            </span>
            <span>
              <strong>Convert files</strong>
              <span>Batch conversion with real progress</span>
            </span>
          </button>
        </div>

        <div className="template-row">
          <button className="template-card" onClick={() => onOpenTab("pdf")}>
            <strong>
              <IconMerge /> Merge PDFs
            </strong>
            <span>Combine several files, keep the order</span>
          </button>
          <button className="template-card" onClick={() => onOpenTab("pdf")}>
            <strong>
              <IconSplit /> Split PDF
            </strong>
            <span>Every page, or fixed-size chunks</span>
          </button>
          <button className="template-card" onClick={() => onOpenTab("images")}>
            <strong>
              <IconImage /> Convert images
            </strong>
            <span>PNG, JPEG, WebP, TIFF, BMP, AVIF</span>
          </button>
          <button className="template-card" onClick={() => onOpenTab("convert")}>
            <strong>
              <IconConvert /> Batch queue
            </strong>
            <span>Many files, one operation</span>
          </button>
        </div>

        <div className={`dropzone${over ? " over" : ""}`}>
          Drop files here, or use <strong>Open file</strong>. Everything stays on this machine.
        </div>

        {inspection.length > 1 && (
          <div className="panel" style={{ marginBottom: 22 }}>
            <p className="panel-title">Dropped files</p>
            <div className="table-shell">
              <table>
                <thead>
                  <tr>
                    <th>File</th>
                    <th>Type</th>
                    <th>Size</th>
                    <th />
                  </tr>
                </thead>
                <tbody>
                  {inspection.map((item) => (
                    <tr key={item.path}>
                      <td className="truncate" style={{ maxWidth: 320 }}>
                        {item.name}
                      </td>
                      <td>
                        <span className="file-chip">{item.kind}</span>
                      </td>
                      <td>{formatBytes(item.size)}</td>
                      <td>
                        <button className="btn" onClick={() => onOpenPath(item.path)}>
                          Open
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <button className="btn" style={{ marginTop: 10 }} onClick={() => onOpenTab("convert")}>
              Send to batch conversion
            </button>
          </div>
        )}

        {pinnedEntries.length > 0 && (
          <>
            <h2>
              <IconStar /> Pinned
            </h2>
            <div className="table-shell" style={{ marginBottom: 22 }}>
              <table className="recent-table">
                <tbody>
                  {pinnedEntries.map((entry) => (
                    <tr className="recent-row" key={entry.path} onClick={() => onOpenPath(entry.path)}>
                      <td style={{ width: 32 }}>{kindIcon(classify(entry.path))}</td>
                      <td>{fileName(entry.path)}</td>
                      <td className="truncate" style={{ maxWidth: 380 }}>
                        {entry.path}
                      </td>
                      <td style={{ width: 40 }}>
                        <button
                          className="icon-btn"
                          title="Unpin"
                          onClick={(event) => {
                            event.stopPropagation();
                            togglePinned(entry.path);
                          }}
                        >
                          <IconStar />
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </>
        )}

        <h2>Recent</h2>
        {recent.length === 0 ? (
          <p className="muted small">Files you open appear here. Nothing is uploaded or indexed remotely.</p>
        ) : (
          <div className="table-shell">
            <table className="recent-table">
              <thead>
                <tr>
                  <th style={{ width: 32 }} />
                  <th>Name</th>
                  <th>Location</th>
                  <th style={{ width: 80 }}>Pinned</th>
                </tr>
              </thead>
              <tbody>
                {recent.slice(0, 14).map((entry) => (
                  <tr className="recent-row" key={entry.path} onClick={() => onOpenPath(entry.path)}>
                    <td>{kindIcon(classify(entry.path))}</td>
                    <td>{fileName(entry.path)}</td>
                    <td className="truncate" style={{ maxWidth: 380 }}>
                      {entry.path}
                    </td>
                    <td>
                      <button
                        className="icon-btn"
                        title={pinned.includes(entry.path) ? "Unpin" : "Pin"}
                        onClick={(event) => {
                          event.stopPropagation();
                          togglePinned(entry.path);
                        }}
                      >
                        <IconStar />
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

        {jobs.length > 0 && (
          <>
            <h2 style={{ marginTop: 24 }}>Last jobs</h2>
            <p className="muted small">
              {jobs.filter((job) => job.state === "done").length} completed ·{" "}
              {jobs.filter((job) => job.state === "failed").length} failed
            </p>
          </>
        )}
      </div>
    </div>
  );
}
