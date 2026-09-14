import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useStore } from "../../state";
import { fileName, formatBytes } from "../../platform";
import { findMatches, loadOutline, openPdf, toBytes, type OutlineNode, type PdfDocument } from "../../lib/pdf";
import { PdfPageView, PdfThumb } from "./PdfPageView";
import {
  IconArrowAnnot,
  IconArrowDown,
  IconArrowRight,
  IconArrowUp,
  IconCircle,
  IconCompress,
  IconEye,
  IconForm,
  IconGrid,
  IconHighlight,
  IconLine,
  IconMerge,
  IconNote,
  IconPage,
  IconPdf,
  IconPen,
  IconRotateLeft,
  IconRotateRight,
  IconSearch,
  IconSelect,
  IconShield,
  IconSplit,
  IconSquare,
  IconTextTool,
  IconTrash,
  IconZoomIn,
  IconZoomOut,
} from "../../design/icons";

type Mode = "view" | "organize" | "annotate" | "forms" | "tools";
type Tool =
  | "select"
  | "highlight"
  | "underline"
  | "strikeout"
  | "ink"
  | "text"
  | "note"
  | "square"
  | "circle"
  | "line"
  | "arrow";

type Draft =
  | {
      id: string;
      page: number;
      kind: "highlight" | "underline" | "strikeout";
      quads: [number, number, number, number][];
      color: string;
      opacity: number;
    }
  | { id: string; page: number; kind: "ink"; strokes: [number, number][][]; color: string; opacity: number }
  | {
      id: string;
      page: number;
      kind: "square" | "circle" | "line" | "arrow";
      rect: [number, number, number, number];
      color: string;
      opacity: number;
    }
  | {
      id: string;
      page: number;
      kind: "text";
      rect: [number, number, number, number];
      text: string;
      color: string;
      opacity: number;
      fontSize: number;
    }
  | { id: string; page: number; kind: "note"; rect: [number, number, number, number]; text: string; color: string; opacity: number };

type FormField = {
  name: string;
  kind: string;
  value: string | null;
  state: string | null;
  options: string[];
  read_only: boolean;
  required: boolean;
  multiline: boolean;
};

type PlanEntry = { page: number; rotate: number };

const RENDER_WINDOW = 3;

let draftSeq = 0;
const nextId = () => `${Date.now().toString(36)}-${(draftSeq += 1)}`;

export function PdfWorkspace({ path, tabId }: { path: string | null; tabId: string }) {
  const { platform, runJob, updateTab, pushToast } = useStore();
  const [document, setDocument] = useState<PdfDocument | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [mode, setMode] = useState<Mode>("view");
  const [zoom, setZoom] = useState(() => {
    if (typeof window === "undefined") return 1;
    if (window.innerWidth < 560) return 0.5;
    if (window.innerWidth < 900) return 0.75;
    return 1;
  });
  const [currentPage, setCurrentPage] = useState(1);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [plan, setPlan] = useState<PlanEntry[]>([]);
  const [drafts, setDrafts] = useState<Draft[]>([]);
  const [tool, setTool] = useState<Tool>("select");
  const [color, setColor] = useState("#ffd21e");
  const [opacity, setOpacity] = useState(0.45);
  const [fontSize, setFontSize] = useState(12);
  const [outline, setOutline] = useState<OutlineNode[]>([]);
  const [query, setQuery] = useState("");
  const [matches, setMatches] = useState<number[]>([]);
  const [matchIndex, setMatchIndex] = useState(0);
  const [forms, setForms] = useState<FormField[]>([]);
  const [formValues, setFormValues] = useState<Record<string, string>>({});
  const [watermark, setWatermark] = useState("CONFIDENTIAL");
  const [compressPreset, setCompressPreset] = useState("balanced");
  const [busy, setBusy] = useState(false);
  const [drawPreview, setDrawPreview] = useState<{ page: number; rect: [number, number, number, number] } | null>(null);
  const [fileBytes, setFileBytes] = useState(0);

  const pageElements = useRef(new Map<number, HTMLElement>());
  const drawStart = useRef<[number, number] | null>(null);
  const inkStroke = useRef<[number, number][]>([]);

  const pageCount = document?.numPages ?? 0;
  const visiblePlan = useMemo(
    () => (plan.length ? plan : Array.from({ length: pageCount }, (_, index) => ({ page: index + 1, rotate: 0 }))),
    [plan, pageCount],
  );
  const planDirty = useMemo(
    () => visiblePlan.some((entry, index) => entry.page !== index + 1 || entry.rotate !== 0),
    [visiblePlan],
  );
  const dirty = planDirty || drafts.length > 0;

  const load = useCallback(
    async (target: string) => {
      setLoading(true);
      setError(null);
      try {
        const raw = await platform.readBinary(target);
        const bytes = toBytes(raw);
        setFileBytes(bytes.byteLength);
        const pdf = await openPdf(bytes);
        setDocument(pdf);
        setPlan(Array.from({ length: pdf.numPages }, (_, index) => ({ page: index + 1, rotate: 0 })));
        setSelected(new Set());
        setDrafts([]);
        setCurrentPage(1);
        void loadOutline(pdf).then(setOutline).catch(() => setOutline([]));
        updateTab(tabId, { title: fileName(target), subtitle: `${pdf.numPages} pages` });
      } catch (err) {
        setError(String(err));
      } finally {
        setLoading(false);
      }
    },
    [platform, tabId, updateTab],
  );

  useEffect(() => {
    if (path) void load(path);
  }, [path, load]);

  useEffect(() => {
    updateTab(tabId, { dirty });
  }, [dirty, tabId, updateTab]);

  useEffect(() => {
    if (!document) return;
    let cancelled = false;
    void findMatches(document, query).then((found) => {
      if (!cancelled) {
        setMatches(found);
        setMatchIndex(0);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [document, query]);

  useEffect(() => {
    const clean = query.trim().toLowerCase();
    const spans = globalThis.document.querySelectorAll<HTMLElement>(".textLayer span");
    spans.forEach((span) => span.classList.remove("highlight", "current-match"));
    if (clean.length < 2) return;
    spans.forEach((span) => {
      if ((span.textContent ?? "").toLowerCase().includes(clean)) span.classList.add("highlight");
    });
  }, [query, matches, currentPage, mode]);

  useEffect(() => {
    if (mode !== "forms" || !path) return;
    void platform
      .invoke<FormField[]>("pdf_form_fields", { file: path })
      .then((fields) => {
        setForms(fields);
        const values: Record<string, string> = {};
        for (const field of fields) values[field.name] = field.value ?? "";
        setFormValues(values);
      })
      .catch(() => setForms([]));
  }, [mode, path, platform]);

  const renderWindow = useMemo(() => {
    const keep = new Set<number>();
    for (let page = currentPage - RENDER_WINDOW; page <= currentPage + RENDER_WINDOW; page += 1) {
      if (page >= 1 && page <= pageCount) keep.add(page);
    }
    return keep;
  }, [currentPage, pageCount]);

  const scrollToPage = useCallback((page: number) => {
    pageElements.current.get(page)?.scrollIntoView({ behavior: "smooth", block: "start" });
    setCurrentPage(page);
  }, []);

  const toPdfPoint = useCallback(
    async (page: number, localX: number, localY: number): Promise<[number, number]> => {
      if (!document) return [0, 0];
      const pdfPage = await document.getPage(page);
      const viewport = pdfPage.getViewport({ scale: zoom });
      const point = viewport.convertToPdfPoint(localX, localY);
      return [point[0], point[1]];
    },
    [document, zoom],
  );

  const localPoint = (page: number, event: React.MouseEvent): [number, number] | null => {
    const element = pageElements.current.get(page);
    if (!element) return null;
    const rect = element.getBoundingClientRect();
    return [event.clientX - rect.left, event.clientY - rect.top];
  };

  const handleMouseDown = async (page: number, event: React.MouseEvent) => {
    if (tool === "select") return;
    const local = localPoint(page, event);
    if (!local) return;
    const point = await toPdfPoint(page, local[0], local[1]);
    drawStart.current = point;
    if (tool === "ink") inkStroke.current = [point];
  };

  const handleMouseMove = async (page: number, event: React.MouseEvent) => {
    if (!drawStart.current) return;
    const local = localPoint(page, event);
    if (!local) return;
    const point = await toPdfPoint(page, local[0], local[1]);
    if (tool === "ink") {
      inkStroke.current.push(point);
      return;
    }
    const start = drawStart.current;
    setDrawPreview({
      page,
      rect: [
        Math.min(start[0], point[0]),
        Math.min(start[1], point[1]),
        Math.max(start[0], point[0]),
        Math.max(start[1], point[1]),
      ],
    });
  };

  const handleMouseUp = async (page: number, event: React.MouseEvent) => {
    const start = drawStart.current;
    if (!start) return;
    drawStart.current = null;
    const local = localPoint(page, event);
    if (!local) return;
    const point = await toPdfPoint(page, local[0], local[1]);
    setDrawPreview(null);

    if (tool === "ink") {
      const strokes = [...inkStroke.current, point];
      inkStroke.current = [];
      if (strokes.length > 2) {
        setDrafts((current) => [
          ...current,
          { id: nextId(), page, kind: "ink", strokes: [strokes], color, opacity: Math.max(opacity, 0.7) },
        ]);
      }
      return;
    }

    const rect: [number, number, number, number] = [
      Math.min(start[0], point[0]),
      Math.min(start[1], point[1]),
      Math.max(start[0], point[0]),
      Math.max(start[1], point[1]),
    ];
    if (tool === "text" || tool === "note") {
      const text = window.prompt(tool === "note" ? "Comment" : "Text", "") ?? "";
      if (!text.trim()) return;
      const box: [number, number, number, number] =
        rect[2] - rect[0] < 8 || rect[3] - rect[1] < 8
          ? [rect[0], rect[1] - 24, rect[0] + 200, rect[1] + 6]
          : rect;
      setDrafts((current) => [
        ...current,
        tool === "text"
          ? { id: nextId(), page, kind: "text", rect: box, text, color, opacity: 1, fontSize }
          : { id: nextId(), page, kind: "note", rect: box, text, color, opacity: 1 },
      ]);
      return;
    }
    if (rect[2] - rect[0] < 3 || rect[3] - rect[1] < 3) return;
    if (tool === "square" || tool === "circle" || tool === "line" || tool === "arrow") {
      setDrafts((current) => [...current, { id: nextId(), page, kind: tool, rect, color, opacity }]);
    }
  };

  const markSelection = useCallback(
    async (page: number) => {
      if (tool !== "highlight" && tool !== "underline" && tool !== "strikeout") return;
      const element = pageElements.current.get(page);
      const selection = window.getSelection();
      if (!element || !selection || selection.isCollapsed || selection.rangeCount === 0) return;
      const range = selection.getRangeAt(0);
      if (!element.contains(range.commonAncestorContainer)) return;
      const pageRect = element.getBoundingClientRect();
      const quads: [number, number, number, number][] = [];
      for (const rect of Array.from(range.getClientRects())) {
        if (rect.width < 1 || rect.height < 1) continue;
        const a = await toPdfPoint(page, rect.left - pageRect.left, rect.top - pageRect.top);
        const b = await toPdfPoint(page, rect.right - pageRect.left, rect.bottom - pageRect.top);
        quads.push([Math.min(a[0], b[0]), Math.min(a[1], b[1]), Math.max(a[0], b[0]), Math.max(a[1], b[1])]);
      }
      selection.removeAllRanges();
      if (quads.length === 0) return;
      const kind = tool === "highlight" ? "highlight" : tool === "underline" ? "underline" : "strikeout";
      setDrafts((current) => [...current, { id: nextId(), page, kind, quads, color, opacity }]);
    },
    [color, opacity, tool, toPdfPoint],
  );

  useEffect(() => {
    const handler = () => {
      if (mode === "annotate" && (tool === "highlight" || tool === "underline" || tool === "strikeout")) {
        void markSelection(currentPage);
      }
    };
    globalThis.document.addEventListener("mouseup", handler);
    return () => globalThis.document.removeEventListener("mouseup", handler);
  }, [markSelection, mode, tool, currentPage]);

  const organize = (mutate: (entries: PlanEntry[]) => PlanEntry[]) => {
    setPlan((current) => {
      const base = current.length ? current.map((entry) => ({ ...entry })) : visiblePlan.map((entry) => ({ ...entry }));
      return mutate(base);
    });
  };

  const toggleSelect = (page: number, event: React.MouseEvent) => {
    setSelected((current) => {
      const next = new Set(current);
      if (event.shiftKey && next.size > 0) {
        const anchor = Math.min(...next);
        for (let index = Math.min(anchor, page); index <= Math.max(anchor, page); index += 1) next.add(index);
        return next;
      }
      if (event.ctrlKey || event.metaKey) {
        if (next.has(page)) next.delete(page);
        else next.add(page);
        return next;
      }
      return new Set([page]);
    });
    setCurrentPage(page);
  };

  const writeAnnotations = async (target: string) => {
    if (drafts.length === 0) return;
    await runJob("Write annotations", fileName(target), "pdf_add_annotations", {
      file: target,
      output: target,
      annotations: drafts.map(toAnnotationInput),
    });
  };

  const applyPlan = async (target: string) => {
    if (!planDirty) return;
    await runJob("Apply page changes", fileName(target), "pdf_apply_plan", {
      file: target,
      output: target,
      plan: visiblePlan,
    });
  };

  const saveInPlace = async () => {
    if (!path) return;
    setBusy(true);
    try {
      await applyPlan(path);
      await writeAnnotations(path);
      await load(path);
    } finally {
      setBusy(false);
    }
  };

  const saveAs = async () => {
    if (!path) return;
    const suggested = await platform.suggestOutput(path, "edited", "pdf");
    const target = await platform.savePath(suggested, "pdf", "PDF");
    if (!target) return;
    setBusy(true);
    try {
      if (planDirty) {
        await runJob("Apply page changes", fileName(target), "pdf_apply_plan", {
          file: path,
          output: target,
          plan: visiblePlan,
        });
      } else {
        await runJob("Copy", fileName(target), "pdf_clean", { file: path, output: target });
      }
      if (drafts.length > 0 && target) {
        await runJob("Write annotations", fileName(target), "pdf_add_annotations", {
          file: target,
          output: target,
          annotations: drafts.map(toAnnotationInput),
        });
      }
      pushToast({ kind: "success", message: "Saved copy", detail: target });
    } finally {
      setBusy(false);
    }
  };

  const runOutputTool = async (
    command: string,
    suffix: string,
    extension: string,
    extra: Record<string, unknown>,
  ) => {
    if (!path) return;
    const suggested = await platform.suggestOutput(path, suffix, extension);
    const target = await platform.savePath(suggested, extension, extension.toUpperCase());
    if (!target) return;
    setBusy(true);
    try {
      await runJob(labelFor(command), fileName(target), command, {
        file: path,
        output: target,
        ...extra,
      });
    } finally {
      setBusy(false);
    }
  };

  const splitDocument = async () => {
    if (!path) return;
    const directory = await platform.openPaths({ directory: true, title: "Choose an output folder" });
    if (!directory?.[0]) return;
    setBusy(true);
    try {
      await runJob("Split PDF", fileName(path), "pdf_split", {
        file: path,
        outputDir: directory[0],
        chunk: 1,
      });
    } finally {
      setBusy(false);
    }
  };

  const mergeAnother = async () => {
    if (!path) return;
    const picked = await platform.openPaths({
      multiple: false,
      title: "Choose a PDF to merge",
      extensions: ["pdf"],
    });
    if (!picked?.[0]) return;
    const suggested = await platform.suggestOutput(path, "merged", "pdf");
    const target = await platform.savePath(suggested, "pdf", "PDF");
    if (!target) return;
    setBusy(true);
    try {
      await runJob("Merge PDFs", fileName(target), "pdf_merge", { files: [path, picked[0]], output: target });
    } finally {
      setBusy(false);
    }
  };

  const exportImages = async () => {
    if (!path || !document) return;
    const directory = await platform.openPaths({ directory: true, title: "Choose an output folder" });
    if (!directory?.[0]) return;
    setBusy(true);
    try {
      const { renderPage } = await import("../../lib/pdf");
      const stem = fileName(path).replace(/\.pdf$/i, "");
      const separator = directory[0].includes("\\") ? "\\" : "/";
      for (let page = 1; page <= document.numPages; page += 1) {
        const pdfPage = await document.getPage(page);
        const canvas = globalThis.document.createElement("canvas");
        await renderPage(pdfPage, 2, canvas);
        const blob: Blob = await new Promise((resolve) =>
          canvas.toBlob((value) => resolve(value as Blob), "image/png"),
        );
        const bytes = new Uint8Array(await blob.arrayBuffer());
        await platform.invoke("write_binary", {
          path: `${directory[0]}${separator}${stem}-page-${String(page).padStart(3, "0")}.png`,
          bytes: Array.from(bytes),
        });
      }
      pushToast({ kind: "success", message: "Pages exported", detail: `${document.numPages} PNG files` });
    } catch (err) {
      pushToast({ kind: "error", message: "Export failed", detail: String(err) });
    } finally {
      setBusy(false);
    }
  };

  const applyForms = async () => {
    if (!path) return;
    setBusy(true);
    try {
      await runJob("Fill form", fileName(path), "pdf_fill_form", {
        file: path,
        output: path,
        values: formValues,
      });
      await load(path);
    } finally {
      setBusy(false);
    }
  };

  if (!path) {
    return (
      <div className="empty-state">
        <IconPdf width={28} height={28} />
        <h2>No PDF open</h2>
        <p>Open a PDF from Home to read, organize, annotate and protect it locally.</p>
      </div>
    );
  }

  const toolButtons: { id: Tool; icon: React.ReactNode; label: string }[] = [
    { id: "select", icon: <IconSelect />, label: "Select text" },
    { id: "highlight", icon: <IconHighlight />, label: "Highlight" },
    { id: "underline", icon: <IconTextTool />, label: "Underline" },
    { id: "strikeout", icon: <IconLine />, label: "Strike out" },
    { id: "ink", icon: <IconPen />, label: "Draw" },
    { id: "text", icon: <IconTextTool />, label: "Text box" },
    { id: "note", icon: <IconNote />, label: "Comment" },
    { id: "square", icon: <IconSquare />, label: "Rectangle" },
    { id: "circle", icon: <IconCircle />, label: "Ellipse" },
    { id: "arrow", icon: <IconArrowAnnot />, label: "Arrow" },
  ];

  return (
    <div className="workspace">
      <div className="pdf-tool-strip">
        <div className="seg">
          {(["view", "organize", "annotate", "forms", "tools"] as Mode[]).map((item) => (
            <button key={item} className={mode === item ? "on" : ""} onClick={() => setMode(item)}>
              {item === "view" ? <IconPage /> : null}
              {item === "organize" ? <IconGrid /> : null}
              {item === "annotate" ? <IconPen /> : null}
              {item === "forms" ? <IconForm /> : null}
              {item === "tools" ? <IconCompress /> : null}
              <span style={{ textTransform: "capitalize" }}>{item}</span>
            </button>
          ))}
        </div>
        <span className="sep" style={{ width: 1, height: 18, background: "var(--border)" }} />
        {mode === "view" && (
          <>
            <div className="row" style={{ gap: 2 }}>
              <button className="icon-btn" onClick={() => setZoom((value) => Math.max(0.25, value - 0.1))}>
                <IconZoomOut />
              </button>
              <span className="small muted" style={{ width: 42, textAlign: "center" }}>
                {Math.round(zoom * 100)}%
              </span>
              <button className="icon-btn" onClick={() => setZoom((value) => Math.min(4, value + 0.1))}>
                <IconZoomIn />
              </button>
            </div>
            <input
              className="text-input"
              style={{ width: 200 }}
              placeholder="Search in document"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
            />
            {matches.length > 0 && (
              <span className="small muted">
                {matchIndex + 1}/{matches.length}
              </span>
            )}
          </>
        )}
        {mode === "organize" && (
          <>
            <button className="btn" onClick={() => organize((entries) => rotateSelected(entries, selected, -90))} disabled={selected.size === 0}>
              <IconRotateLeft /> Rotate left
            </button>
            <button className="btn" onClick={() => organize((entries) => rotateSelected(entries, selected, 90))} disabled={selected.size === 0}>
              <IconRotateRight /> Rotate right
            </button>
            <button
              className="btn danger"
              onClick={() => {
                organize((entries) => entries.filter((_, index) => !selected.has(index + 1)));
                setSelected(new Set());
              }}
              disabled={selected.size === 0 || selected.size >= visiblePlan.length}
            >
              <IconTrash /> Delete
            </button>
            <button
              className="btn"
              onClick={() =>
                organize((entries) =>
                  [...entries, ...entries.filter((_, index) => selected.has(index + 1)).map((entry) => ({ ...entry }))],
                )
              }
              disabled={selected.size === 0}
            >
              Duplicate
            </button>
            <span className="spacer" />
            <button className="btn" onClick={mergeAnother} disabled={busy}>
              <IconMerge /> Merge another PDF
            </button>
            <button className="btn primary" onClick={saveInPlace} disabled={busy || !planDirty}>
              Apply to document
            </button>
          </>
        )}
        {mode === "annotate" && (
          <>
            {toolButtons.map((item) => (
              <button
                key={item.id}
                className={`icon-btn${tool === item.id ? " on" : ""}`}
                title={item.label}
                onClick={() => setTool(item.id)}
              >
                {item.icon}
              </button>
            ))}
            <span className="sep" style={{ width: 1, height: 18, background: "var(--border)" }} />
            <input className="color-swatch" type="color" value={color} onChange={(event) => setColor(event.target.value)} />
            <input
              type="range"
              min={0.1}
              max={1}
              step={0.05}
              value={opacity}
              onChange={(event) => setOpacity(Number(event.target.value))}
              style={{ width: 90 }}
              title="Opacity"
            />
            <label className="row small muted" style={{ gap: 4 }}>
              Text
              <input
                className="number-input"
                type="number"
                min={6}
                max={72}
                value={fontSize}
                onChange={(event) => setFontSize(Number(event.target.value))}
                style={{ width: 60 }}
                title="Text size for text boxes"
              />
            </label>
            <span className="spacer" />
            <span className="small muted">Select text with the highlight tool, then click a style.</span>
            <button className="btn primary" onClick={saveInPlace} disabled={busy || drafts.length === 0}>
              Write {drafts.length > 0 ? drafts.length : ""} into PDF
            </button>
          </>
        )}
        {mode === "forms" && (
          <>
            <span className="small muted">{forms.length} fields</span>
            <span className="spacer" />
            <button className="btn primary" onClick={applyForms} disabled={busy || forms.length === 0}>
              Save form values
            </button>
          </>
        )}
        {mode === "tools" && (
          <>
            <button className="btn" onClick={() => runOutputTool("pdf_clean", "clean", "pdf", {})} disabled={busy}>
              <IconShield /> Privacy clean
            </button>
            <button className="btn" onClick={splitDocument} disabled={busy}>
              <IconSplit /> Split every page
            </button>
            <button className="btn" onClick={exportImages} disabled={busy}>
              <IconEye /> Export PNG
            </button>
            <span className="spacer" />
            <span className="small muted">{formatBytes(fileBytes)}</span>
          </>
        )}
        {dirty && <span className="small" style={{ color: "var(--accent)" }}>Unsaved</span>}
      </div>

      <div className="workspace-body with-left with-right">
        <div className="side-panel">
          <div className="thumb-list">
            {document
              ? Array.from({ length: pageCount }, (_, index) => index + 1).map((page) => (
                  <PdfThumb
                    key={page}
                    document={document}
                    pageNumber={page}
                    width={150}
                    active={page === currentPage}
                    selected={selected.has(page)}
                    onClick={toggleSelect}
                  />
                ))
              : null}
          </div>
        </div>

        <div className="canvas-area">
          {loading && <div className="empty-state">Opening…</div>}
          {error && (
            <div className="empty-state">
              <h2>Could not open this PDF</h2>
              <p>{error}</p>
            </div>
          )}

          {document && mode !== "organize" && (
            <div className="writer-canvas" style={{ gap: 14 }}>
              {Array.from({ length: pageCount }, (_, index) => index + 1).map((page) => (
                <div
                  key={page}
                  ref={(node) => {
                    if (node) pageElements.current.set(page, node);
                    else pageElements.current.delete(page);
                  }}
                  style={{ position: "relative" }}
                >
                  <PdfPageView
                    document={document}
                    pageNumber={page}
                    scale={zoom}
                    shouldRender={renderWindow.has(page)}
                    textLayer={mode === "view" || mode === "annotate"}
                    onVisible={setCurrentPage}
                    overlay={
                      mode === "annotate"
                        ? () => (
                            <div
                              className="annotation-layer"
                              onMouseDown={(event) => void handleMouseDown(page, event)}
                              onMouseMove={(event) => void handleMouseMove(page, event)}
                              onMouseUp={(event) => void handleMouseUp(page, event)}
                              style={{
                                cursor: tool === "select" ? "text" : "crosshair",
                                pointerEvents: tool === "select" ? "none" : "auto",
                              }}
                            />
                          )
                        : undefined
                    }
                  />
                  {drawPreview && drawPreview.page === page ? (
                    <div
                      style={{
                        position: "absolute",
                        border: "1.5px dashed var(--accent)",
                        left: drawPreview.rect[0] * zoom,
                        top: drawPreview.rect[1] * zoom,
                        width: (drawPreview.rect[2] - drawPreview.rect[0]) * zoom,
                        height: (drawPreview.rect[3] - drawPreview.rect[1]) * zoom,
                        pointerEvents: "none",
                      }}
                    />
                  ) : null}
                </div>
              ))}
            </div>
          )}

          {document && mode === "organize" && (
            <div className="page-grid">
              {visiblePlan.map((entry, index) => (
                <div
                  key={`${entry.page}-${index}`}
                  className={`page-card${selected.has(index + 1) ? " selected" : ""}`}
                  draggable
                  onDragStart={(event) => event.dataTransfer.setData("text/plain", String(index))}
                  onDragOver={(event) => event.preventDefault()}
                  onDrop={(event) => {
                    event.preventDefault();
                    const from = Number(event.dataTransfer.getData("text/plain"));
                    if (Number.isNaN(from) || from === index) return;
                    organize((entries) => {
                      const [moved] = entries.splice(from, 1);
                      entries.splice(index, 0, moved);
                      return entries;
                    });
                  }}
                  onClick={(event) => toggleSelect(index + 1, event)}
                >
                  <span className="badge">{entry.page}</span>
                  <PdfThumb
                    document={document}
                    pageNumber={entry.page}
                    width={140}
                    active={false}
                    selected={false}
                    onClick={() => setCurrentPage(entry.page)}
                  />
                  <div className="row" style={{ justifyContent: "center", marginTop: 4 }}>
                    <span>{index + 1}</span>
                    {entry.rotate !== 0 ? <span className="mono">{entry.rotate}°</span> : null}
                  </div>
                </div>
              ))}
            </div>
          )}

          {document && mode === "forms" && (
            <div style={{ padding: 20, maxWidth: 720, margin: "0 auto" }}>
              {forms.length === 0 ? (
                <div className="empty-state">
                  <IconForm width={26} height={26} />
                  <h2>No interactive fields</h2>
                  <p>This PDF does not contain an AcroForm. Fillable fields appear here when present.</p>
                </div>
              ) : (
                <div className="panel col">
                  <p className="panel-title">Form fields</p>
                  {forms.map((field) => (
                    <label className="field" key={field.name}>
                      <span>
                        {field.name} · {field.kind}
                        {field.required ? " · required" : ""}
                        {field.read_only ? " · read-only" : ""}
                      </span>
                      {field.kind === "checkbox" || field.kind === "radio" ? (
                        <input
                          type="checkbox"
                          checked={/^(yes|on|1|true)$/i.test(formValues[field.name] ?? "")}
                          disabled={field.read_only}
                          onChange={(event) =>
                            setFormValues((current) => ({
                              ...current,
                              [field.name]: event.target.checked ? "true" : "false",
                            }))
                          }
                        />
                      ) : field.kind === "choice" && field.options.length > 0 ? (
                        <select
                          className="select"
                          value={formValues[field.name] ?? ""}
                          disabled={field.read_only}
                          onChange={(event) =>
                            setFormValues((current) => ({ ...current, [field.name]: event.target.value }))
                          }
                        >
                          <option value="">—</option>
                          {field.options.map((option) => (
                            <option key={option} value={option}>
                              {option}
                            </option>
                          ))}
                        </select>
                      ) : (
                        <textarea
                          className="text-input"
                          style={{ height: field.multiline ? 76 : 30, padding: "6px 8px" }}
                          value={formValues[field.name] ?? ""}
                          disabled={field.read_only}
                          onChange={(event) =>
                            setFormValues((current) => ({ ...current, [field.name]: event.target.value }))
                          }
                        />
                      )}
                    </label>
                  ))}
                  <button className="btn primary" onClick={applyForms} disabled={busy}>
                    Save into the PDF
                  </button>
                </div>
              )}
            </div>
          )}
        </div>

        <div className="inspector">
          {mode === "view" && (
            <>
              <div className="inspector-section">
                <h3>Find</h3>
                <input
                  className="search-input"
                  placeholder="Search text"
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                />
                {matches.length > 0 && (
                  <div className="row" style={{ marginTop: 8, justifyContent: "space-between" }}>
                    <span className="small muted">
                      {matchIndex + 1} of {matches.length} pages
                    </span>
                    <div className="row">
                      <button
                        className="icon-btn"
                        onClick={() => {
                          const next = (matchIndex - 1 + matches.length) % matches.length;
                          setMatchIndex(next);
                          scrollToPage(matches[next]);
                        }}
                      >
                        <IconArrowUp />
                      </button>
                      <button
                        className="icon-btn"
                        onClick={() => {
                          const next = (matchIndex + 1) % matches.length;
                          setMatchIndex(next);
                          scrollToPage(matches[next]);
                        }}
                      >
                        <IconArrowDown />
                      </button>
                    </div>
                  </div>
                )}
                {matches.slice(0, 30).map((page, index) => (
                  <button
                    key={page}
                    className="menu-item"
                    onClick={() => {
                      setMatchIndex(index);
                      scrollToPage(page);
                    }}
                  >
                    <IconSearch /> Page {page}
                  </button>
                ))}
              </div>
              <div className="inspector-section">
                <h3>Outline</h3>
                {outline.length === 0 && <p className="small muted">No bookmarks in this document.</p>}
                {outline.map((node, index) => (
                  <OutlineItem key={index} node={node} onGo={scrollToPage} depth={0} />
                ))}
              </div>
            </>
          )}

          {mode === "organize" && (
            <>
              <div className="inspector-section">
                <h3>Selection</h3>
                <p className="small muted">
                  {selected.size === 0 ? "No pages selected" : `${selected.size} pages selected`}
                </p>
                <div className="col">
                  <button className="btn" onClick={() => setSelected(new Set(visiblePlan.map((_, index) => index + 1)))}>
                    Select all
                  </button>
                  <button className="btn" onClick={() => setSelected(new Set())}>
                    Clear selection
                  </button>
                </div>
              </div>
              <div className="inspector-section">
                <h3>Order</h3>
                <div className="col">
                  <button className="btn" onClick={() => organize((entries) => moveSelected(entries, selected, -1))} disabled={selected.size === 0}>
                    <IconArrowUp /> Move earlier
                  </button>
                  <button className="btn" onClick={() => organize((entries) => moveSelected(entries, selected, 1))} disabled={selected.size === 0}>
                    <IconArrowDown /> Move later
                  </button>
                  <button className="btn" onClick={() => organize((entries) => [...entries].reverse())}>
                    Reverse order
                  </button>
                </div>
              </div>
              <div className="inspector-section">
                <h3>Save</h3>
                <p className="small muted">
                  {planDirty
                    ? `${visiblePlan.length} pages will be written back`
                    : "No page changes yet. Drag cards or use the toolbar."}
                </p>
                <div className="col">
                  <button className="btn primary" onClick={saveInPlace} disabled={busy || !planDirty}>
                    Apply to document
                  </button>
                  <button className="btn" onClick={saveAs} disabled={busy}>
                    Save a copy…
                  </button>
                </div>
              </div>
            </>
          )}

          {mode === "annotate" && (
            <>
              <div className="inspector-section">
                <h3>Pending annotations</h3>
                {drafts.length === 0 && <p className="small muted">Nothing added yet.</p>}
                <div className="col">
                  {drafts
                    .slice()
                    .reverse()
                    .map((draft) => (
                      <div className="row" key={draft.id} style={{ justifyContent: "space-between" }}>
                        <span className="small">
                          p.{draft.page} · {draft.kind}
                        </span>
                        <button
                          className="icon-btn"
                          onClick={() => setDrafts((current) => current.filter((item) => item.id !== draft.id))}
                        >
                          <IconTrash />
                        </button>
                      </div>
                    ))}
                  <button className="btn primary" onClick={saveInPlace} disabled={busy || drafts.length === 0}>
                    Write into the PDF
                  </button>
                  <button className="btn" onClick={() => setDrafts([])} disabled={drafts.length === 0}>
                    Discard all
                  </button>
                </div>
              </div>
              <div className="inspector-section">
                <h3>How it works</h3>
                <p className="small muted">
                  Annotations are written as real PDF annotation objects with appearance streams. Other
                  readers can edit or remove them.
                </p>
              </div>
            </>
          )}

          {mode === "tools" && (
            <>
              <div className="inspector-section">
                <h3>Compress</h3>
                <label className="field">
                  <span>Preset</span>
                  <select className="select" value={compressPreset} onChange={(event) => setCompressPreset(event.target.value)}>
                    <option value="screen">Screen · smallest</option>
                    <option value="balanced">Balanced</option>
                    <option value="print">Print · highest quality</option>
                  </select>
                </label>
                <button
                  className="btn"
                  style={{ marginTop: 8 }}
                  disabled={busy}
                  onClick={() =>
                    runOutputTool("pdf_compress", "compressed", "pdf", {
                      preset: compressPreset,
                      quality: 75,
                      maxDimension: 1600,
                    })
                  }
                >
                  <IconCompress /> Compress and save as…
                </button>
              </div>
              <div className="inspector-section">
                <h3>Watermark</h3>
                <input className="text-input" value={watermark} onChange={(event) => setWatermark(event.target.value)} />
                <button
                  className="btn"
                  style={{ marginTop: 8 }}
                  disabled={busy || !watermark.trim()}
                  onClick={() => runOutputTool("pdf_watermark", "stamped", "pdf", { text: watermark, opacity: 0.15 })}
                >
                  Add watermark
                </button>
              </div>
              <div className="inspector-section">
                <h3>Privacy</h3>
                <p className="small muted">
                  Removes document Info and XMP metadata, JavaScript, embedded files and launch actions.
                </p>
                <button className="btn" onClick={() => runOutputTool("pdf_clean", "clean", "pdf", {})} disabled={busy}>
                  <IconShield /> Save a clean copy
                </button>
              </div>
            </>
          )}
        </div>
      </div>

      <div className="statusbar">
        <span className="status-pill">
          <IconPage /> {currentPage} / {pageCount || "—"}
        </span>
        <span className="sep" />
        <span className="status-pill">
          <IconZoomOut /> {Math.round(zoom * 100)}% <IconZoomIn />
        </span>
        <span className="sep" />
        <span className="status-pill">{formatBytes(fileBytes)}</span>
        <span className="spacer" />
        {selected.size > 0 && <span className="status-pill">{selected.size} pages selected</span>}
        {dirty && <span className="status-pill" style={{ color: "var(--accent)" }}>Unsaved changes</span>}
      </div>
    </div>
  );
}

function labelFor(command: string): string {
  const map: Record<string, string> = {
    pdf_compress: "Compress PDF",
    pdf_clean: "Remove metadata",
    pdf_watermark: "Add watermark",
    pdf_merge: "Merge PDFs",
    pdf_split: "Split PDF",
    pdf_apply_plan: "Apply page changes",
    pdf_add_annotations: "Write annotations",
    pdf_fill_form: "Fill form",
  };
  return map[command] ?? command;
}

function toAnnotationInput(draft: Draft) {
  const rect =
    "rect" in draft
      ? draft.rect
      : "quads" in draft
        ? union(draft.quads)
        : ([0, 0, 1, 1] as [number, number, number, number]);
  const base = {
    page: draft.page,
    rect,
    color: hexToRgb(draft.color),
    opacity: draft.opacity,
    author: "TEDROX Documents",
    contents: null as string | null,
  };
  switch (draft.kind) {
    case "highlight":
      return { ...base, kind: { kind: "highlight", quads: draft.quads } };
    case "underline":
      return { ...base, kind: { kind: "underline", quads: draft.quads } };
    case "strikeout":
      return { ...base, kind: { kind: "strike_out", quads: draft.quads } };
    case "ink":
      return { ...base, kind: { kind: "ink", strokes: draft.strokes } };
    case "square":
      return { ...base, kind: { kind: "square" } };
    case "circle":
      return { ...base, kind: { kind: "circle" } };
    case "line":
      return {
        ...base,
        kind: { kind: "line", from: [draft.rect[0], draft.rect[3]], to: [draft.rect[2], draft.rect[1]] },
      };
    case "arrow":
      return {
        ...base,
        kind: { kind: "arrow", from: [draft.rect[0], draft.rect[3]], to: [draft.rect[2], draft.rect[1]] },
      };
    case "text":
      return { ...base, kind: { kind: "free_text", text: draft.text, font_size: draft.fontSize }, contents: draft.text };
    case "note":
      return { ...base, kind: { kind: "note", text: draft.text }, contents: draft.text };
    default:
      return base;
  }
}

function union(quads: [number, number, number, number][]): [number, number, number, number] {
  let x0 = Infinity;
  let y0 = Infinity;
  let x1 = -Infinity;
  let y1 = -Infinity;
  for (const quad of quads) {
    x0 = Math.min(x0, quad[0]);
    y0 = Math.min(y0, quad[1]);
    x1 = Math.max(x1, quad[2]);
    y1 = Math.max(y1, quad[3]);
  }
  if (!Number.isFinite(x0)) return [0, 0, 1, 1];
  return [x0, y0, x1, y1];
}

function hexToRgb(hex: string): [number, number, number] {
  const match = hex.replace("#", "").match(/^([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i);
  if (!match) return [1, 0.82, 0.12];
  return [parseInt(match[1], 16) / 255, parseInt(match[2], 16) / 255, parseInt(match[3], 16) / 255];
}

function moveSelected(entries: PlanEntry[], selected: Set<number>, delta: number): PlanEntry[] {
  const next = [...entries];
  const indices = [...selected].map((value) => value - 1).sort((a, b) => (delta < 0 ? a - b : b - a));
  const moved = new Set<number>();
  for (const index of indices) {
    const target = index + delta;
    if (target < 0 || target >= next.length) continue;
    if (selected.has(target + 1) || moved.has(target)) continue;
    [next[index], next[target]] = [next[target], next[index]];
    moved.add(target);
  }
  return next;
}

function rotateSelected(entries: PlanEntry[], selected: Set<number>, delta: number): PlanEntry[] {
  return entries.map((entry, index) =>
    selected.has(index + 1) ? { ...entry, rotate: (entry.rotate + delta + 360) % 360 } : entry,
  );
}

function OutlineItem({
  node,
  onGo,
  depth,
}: {
  node: OutlineNode;
  onGo: (page: number) => void;
  depth: number;
}) {
  const [open, setOpen] = useState(depth < 1);
  return (
    <div style={{ paddingLeft: depth * 12 }}>
      <div className="row" style={{ gap: 4 }}>
        {node.children.length > 0 ? (
          <button className="icon-btn" onClick={() => setOpen((value) => !value)} style={{ width: 18, height: 18 }}>
            {open ? <IconArrowDown /> : <IconArrowRight />}
          </button>
        ) : (
          <span style={{ width: 18 }} />
        )}
        <button className="link" style={{ fontSize: 12 }} onClick={() => node.page && onGo(node.page)} disabled={!node.page}>
          {node.title}
        </button>
      </div>
      {open &&
        node.children.map((child, index) => (
          <OutlineItem key={index} node={child} onGo={onGo} depth={depth + 1} />
        ))}
    </div>
  );
}
