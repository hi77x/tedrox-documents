import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useStore } from "../state";
import { fileName } from "../platform";
import {
  cellRef,
  colName,
  detectFormatKind,
  evaluateWorkbook,
  keyOf,
  modelFromWorkbook,
  parseRef,
  type CellAlign,
  type CellModel,
  type Workbook,
} from "../sheetModel";
import {
  IconAlignCenter,
  IconAlignLeft,
  IconAlignRight,
  IconBold,
  IconChart,
  IconCopy,
  IconFilter,
  IconItalic,
  IconPaste,
  IconRefresh,
  IconSearch,
  IconSheet,
  IconSort,
  IconTrash,
  IconUnderline,
} from "../design/icons";

const ROW_HEIGHT = 26;
const HEADER_HEIGHT = 26;
const ROW_HEADER_WIDTH = 48;
const COL_WIDTH = 108;

const NUMBER_FORMATS: { label: string; value: string }[] = [
  { label: "General", value: "" },
  { label: "Number 1,234.00", value: "#,##0.00" },
  { label: "Number 1,234", value: "#,##0" },
  { label: "Currency", value: "$#,##0.00" },
  { label: "Percent 12.34%", value: "0.00%" },
  { label: "Date 2026-09-14", value: "yyyy-mm-dd" },
  { label: "Date 14/09/2026", value: "dd/mm/yyyy" },
  { label: "Time 13:45", value: "hh:mm" },
  { label: "Scientific", value: "0.00E+00" },
];

type Range = { r0: number; c0: number; r1: number; c1: number };

export function SheetsWorkspace({
  tabId,
  path,
  workbook: initial,
  displayName,
}: {
  tabId: string;
  path: string | null;
  workbook: Workbook;
  displayName: string;
}) {
  const { platform, updateTab, remember, pushToast } = useStore();
  const [workbook, setWorkbook] = useState<Workbook>(initial);
  const [sheetIndex, setSheetIndex] = useState(0);
  const [active, setActive] = useState<{ row: number; col: number }>({ row: 0, col: 0 });
  const [anchor, setAnchor] = useState<{ row: number; col: number }>({ row: 0, col: 0 });
  const [selection, setSelection] = useState<Range>({ r0: 0, c0: 0, r1: 0, c1: 0 });
  const [editing, setEditing] = useState<{ row: number; col: number; draft: string } | null>(null);
  const [dirty, setDirty] = useState(false);
  const [scroll, setScroll] = useState({ top: 0, left: 0, width: 900, height: 600 });
  const [formattingRisk, setFormattingRisk] = useState(false);
  const [findText, setFindText] = useState("");
  const [replaceText, setReplaceText] = useState("");
  const [chartRange, setChartRange] = useState("A1:B10");
  const [chartType, setChartType] = useState<"bar" | "line" | "pie">("bar");
  const [filterValue, setFilterValue] = useState("");
  const [busy, setBusy] = useState(false);

  const wrapRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const gridRef = useRef(workbook);
  gridRef.current = workbook;

  const sheet = workbook.sheets[sheetIndex] ?? workbook.sheets[0];
  const result = useMemo(() => evaluateWorkbook(workbook) , [workbook]);
  const sheetResult = result.sheets[sheetIndex];

  useEffect(() => {
    updateTab(tabId, { title: displayName, dirty });
  }, [dirty, displayName, tabId, updateTab]);

  useEffect(() => {
    if (!path) return;
    void platform
      .invoke<{ has_formatting: boolean }>("sheet_probe_styles", { path })
      .then((probe) => setFormattingRisk(Boolean(probe?.has_formatting)))
      .catch(() => setFormattingRisk(false));
  }, [path, platform]);

  const rows = Math.max(sheet?.rows ?? 0, 60) + 20;
  const cols = Math.max(sheet?.cols ?? 0, 12) + 4;
  const totalWidth = cols * COL_WIDTH;
  const totalHeight = rows * ROW_HEIGHT;

  const firstRow = Math.max(0, Math.floor((scroll.top - HEADER_HEIGHT) / ROW_HEIGHT) - 2);
  const lastRow = Math.min(rows - 1, firstRow + Math.ceil(scroll.height / ROW_HEIGHT) + 6);
  const firstCol = Math.max(0, Math.floor((scroll.left - ROW_HEADER_WIDTH) / COL_WIDTH) - 1);
  const lastCol = Math.min(cols - 1, firstCol + Math.ceil(scroll.width / COL_WIDTH) + 3);

  const cellValue = useCallback(
    (row: number, col: number): CellModel | undefined => sheet?.cells.get(keyOf(row, col)),
    [sheet],
  );

  const displayAt = useCallback(
    (row: number, col: number): string => {
      const key = keyOf(row, col);
      return sheetResult?.display.get(key) ?? cellValue(row, col)?.value ?? "";
    },
    [cellValue, sheetResult],
  );

  const mutateCells = useCallback(
    (mutator: (cell: CellModel) => CellModel | null) => {
      setWorkbook((current) => {
        const sheets = current.sheets.map((item, index) => {
          if (index !== sheetIndex) return item;
          const cells = new Map(item.cells);
          let rowsMax = item.rows;
          let colsMax = item.cols;
          for (let row = selection.r0; row <= selection.r1; row += 1) {
            for (let col = selection.c0; col <= selection.c1; col += 1) {
              const key = keyOf(row, col);
              const existing: CellModel = cells.get(key) ?? { row, col, value: "" };
              const next = mutator(existing);
              if (next === null) cells.delete(key);
              else {
                cells.set(key, next);
                rowsMax = Math.max(rowsMax, row + 1);
                colsMax = Math.max(colsMax, col + 1);
              }
            }
          }
          return { ...item, cells, rows: rowsMax, cols: colsMax };
        });
        return { sheets };
      });
      setDirty(true);
    },
    [selection],
  );

  const setCell = useCallback(
    (row: number, col: number, patch: Partial<CellModel>) => {
      setWorkbook((current) => {
        const sheets = current.sheets.map((item, index) => {
          if (index !== sheetIndex) return item;
          const cells = new Map(item.cells);
          const key = keyOf(row, col);
          const existing: CellModel = cells.get(key) ?? { row, col, value: "" };
          const next = { ...existing, ...patch, row, col };
          const empty =
            !next.value && !next.formula && !next.bold && !next.italic && !next.underline && !next.align && !next.color && !next.fill && !next.format;
          if (empty) cells.delete(key);
          else cells.set(key, next);
          return { ...item, cells, rows: Math.max(item.rows, row + 1), cols: Math.max(item.cols, col + 1) };
        });
        return { sheets };
      });
      setDirty(true);
    },
    [sheetIndex],
  );

  const commitEdit = useCallback(
    (move: "down" | "right" | "none" = "down") => {
      if (!editing) return;
      const raw = editing.draft;
      const isFormula = raw.startsWith("=");
      setCell(editing.row, editing.col, { value: isFormula ? "" : raw, formula: isFormula ? raw : null });
      setEditing(null);
      if (move === "down") setActive((current) => ({ ...current, row: current.row + 1 }));
      if (move === "right") setActive((current) => ({ ...current, col: current.col + 1 }));
    },
    [editing, setCell],
  );

  const onKeyDown = (event: React.KeyboardEvent) => {
    if (editing) {
      if (event.key === "Escape") {
        setEditing(null);
        event.preventDefault();
      }
      if (event.key === "Enter") {
        commitEdit("down");
        event.preventDefault();
      }
      if (event.key === "Tab") {
        commitEdit("right");
        event.preventDefault();
      }
      return;
    }
    const meta = event.ctrlKey || event.metaKey;
    if (meta && event.key.toLowerCase() === "s") {
      event.preventDefault();
      void saveInPlace();
      return;
    }
    if (meta && event.key.toLowerCase() === "c") {
      void copySelection();
      return;
    }
    if (meta && event.key.toLowerCase() === "v") {
      void pasteClipboard();
      return;
    }
    if (event.key === "F2") {
      setEditing({ ...active, draft: cellValue(active.row, active.col)?.formula ?? cellValue(active.row, active.col)?.value ?? "" });
      event.preventDefault();
      return;
    }
    if (event.key === "Delete" || event.key === "Backspace") {
      mutateCells((cell) => (cell.formula ? { ...cell, value: "", formula: null } : { ...cell, value: "" }));
      event.preventDefault();
      return;
    }
    if (event.key === "Enter" && !meta) {
      setEditing({ ...active, draft: "" });
      event.preventDefault();
      return;
    }
    const moves: Record<string, [number, number]> = {
      ArrowUp: [-1, 0],
      ArrowDown: [1, 0],
      ArrowLeft: [0, -1],
      ArrowRight: [0, 1],
      Tab: [0, 1],
    };
    const move = moves[event.key];
    if (move) {
      event.preventDefault();
      const next = {
        row: Math.max(0, active.row + move[0]),
        col: Math.max(0, active.col + move[1]),
      };
      setActive(next);
      if (event.shiftKey) setSelection((current) => ({ ...current, r1: next.row, c1: next.col }));
      else {
        setAnchor(next);
        setSelection({ r0: next.row, c0: next.col, r1: next.row, c1: next.col });
      }
    }
  };

  const copySelection = async () => {
    const lines: string[] = [];
    for (let row = selection.r0; row <= selection.r1; row += 1) {
      const line: string[] = [];
      for (let col = selection.c0; col <= selection.c1; col += 1) {
        line.push(cellValue(row, col)?.formula ?? cellValue(row, col)?.value ?? "");
      }
      lines.push(line.join("\t"));
    }
    try {
      await navigator.clipboard.writeText(lines.join("\n"));
      pushToast({ kind: "success", message: "Copied", detail: `${lines.length} row(s)` });
    } catch {
      pushToast({ kind: "warn", message: "Clipboard is unavailable" });
    }
  };

  const pasteClipboard = async () => {
    try {
      const text = await navigator.clipboard.readText();
      const lines = text.split(/\r?\n/).filter((line) => line.length > 0);
      let count = 0;
      lines.forEach((line, rowOffset) => {
        line.split("\t").forEach((value, colOffset) => {
          const isFormula = value.startsWith("=");
          setCell(active.row + rowOffset, active.col + colOffset, {
            value: isFormula ? "" : value,
            formula: isFormula ? value : null,
          });
          count += 1;
        });
      });
      pushToast({ kind: "success", message: "Pasted", detail: `${count} cells` });
    } catch {
      pushToast({ kind: "warn", message: "Clipboard is unavailable" });
    }
  };

  const saveInPlace = useCallback(async () => {
    setBusy(true);
    try {
      if (formattingRisk) {
        pushToast({
          kind: "warn",
          message: "Formatting from the original file is not preserved",
          detail: "Values, formulas and TEDROX formatting are saved.",
        });
      }
      const model = modelFromWorkbook(gridRef.current);
      if (path) {
        await platform.invoke("sheet_save", { path, model });
        setDirty(false);
        pushToast({ kind: "success", message: "Saved", detail: fileName(path) });
      }
    } catch (err) {
      pushToast({ kind: "error", message: "Save failed", detail: String(err) });
    } finally {
      setBusy(false);
    }
  }, [formattingRisk, path, platform, pushToast]);

  const saveAs = useCallback(async () => {
    const suggested = `${(path ? fileName(path) : displayName).replace(/\.(xlsx|csv|tsv|ods)$/i, "")}-copy.xlsx`;
    const target = await platform.savePath(suggested, "xlsx", "Excel workbook");
    if (!target) return;
    setBusy(true);
    try {
      const model = modelFromWorkbook(gridRef.current);
      await platform.invoke("sheet_save", { path: target, model });
      remember(target, "sheet");
      pushToast({ kind: "success", message: "Saved copy", detail: fileName(target) });
    } catch (err) {
      pushToast({ kind: "error", message: "Save failed", detail: String(err) });
    } finally {
      setBusy(false);
    }
  }, [displayName, path, platform, pushToast, remember]);

  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
        event.preventDefault();
        void saveInPlace();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [saveInPlace]);

  const selectionStats = useMemo(() => {
    let count = 0;
    let sum = 0;
    let numeric = 0;
    for (let row = selection.r0; row <= selection.r1; row += 1) {
      for (let col = selection.c0; col <= selection.c1; col += 1) {
        const raw = sheetResult?.values.get(keyOf(row, col));
        if (!raw || raw.kind === "empty") continue;
        count += 1;
        if (raw.kind === "number") {
          numeric += 1;
          sum += raw.value;
        }
      }
    }
    return { count, numeric, sum, average: numeric ? sum / numeric : 0 };
  }, [selection, sheetResult]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const context = canvas.getContext("2d");
    if (!context) return;
    const parsed = chartRange.match(/^([A-Z]+\d+):([A-Z]+\d+)$/i);
    let series: { label: string; value: number }[] = [];
    if (parsed) {
      const start = parseRef(parsed[1]);
      const end = parseRef(parsed[2]);
      if (start && end) {
        for (let row = Math.min(start.row, end.row); row <= Math.max(start.row, end.row); row += 1) {
          const label = displayAt(row, Math.min(start.col, end.col)) || `Row ${row + 1}`;
          const value = Number(displayAt(row, Math.max(start.col, end.col)).replace(/,/g, ""));
          if (Number.isFinite(value)) series.push({ label, value });
        }
      }
    }
    const width = canvas.width;
    const height = canvas.height;
    context.clearRect(0, 0, width, height);
    context.font = "11px system-ui";
    if (series.length === 0) {
      context.fillStyle = "#8a93a3";
      context.fillText("Select a range like A1:B10 with labels and values", 10, 20);
      return;
    }
    const max = Math.max(...series.map((item) => item.value), 1);
    if (chartType === "bar") {
      const barWidth = Math.max(6, (width - 40) / series.length - 6);
      series.forEach((item, index) => {
        const barHeight = (item.value / max) * (height - 40);
        const x = 30 + index * (barWidth + 6);
        context.fillStyle = "#2f66d6";
        context.fillRect(x, height - 20 - barHeight, barWidth, barHeight);
        context.fillStyle = "#6b7280";
        context.save();
        context.translate(x + barWidth / 2, height - 8);
        context.fillText(item.label.slice(0, 8), -12, 0);
        context.restore();
      });
    } else if (chartType === "line") {
      context.strokeStyle = "#2f66d6";
      context.lineWidth = 2;
      context.beginPath();
      series.forEach((item, index) => {
        const x = 30 + (index / Math.max(1, series.length - 1)) * (width - 50);
        const y = height - 24 - (item.value / max) * (height - 44);
        if (index === 0) context.moveTo(x, y);
        else context.lineTo(x, y);
      });
      context.stroke();
    } else {
      let angle = -Math.PI / 2;
      const total = series.reduce((sum, item) => sum + item.value, 0) || 1;
      const palette = ["#2f66d6", "#f0a30a", "#1f7a4d", "#c8362f", "#7d5bd6", "#2aa3b8"];
      series.forEach((item, index) => {
        const slice = (item.value / total) * Math.PI * 2;
        context.beginPath();
        context.moveTo(width / 2, height / 2);
        context.arc(width / 2, height / 2, Math.min(width, height) / 2 - 16, angle, angle + slice);
        context.closePath();
        context.fillStyle = palette[index % palette.length];
        context.fill();
        angle += slice;
      });
    }
  }, [chartRange, chartType, sheetResult, displayAt]);

  const sheetNames = workbook.sheets.map((item) => item.name);

  return (
    <div className="workspace">
      <div className="ribbon">
        <div className="ribbon-group">
          <div className="ribbon-row">
            <button className={`icon-btn${activeCellIs("bold") ? " on" : ""}`} onClick={() => toggleStyle("bold")} title="Bold">
              <IconBold />
            </button>
            <button className="icon-btn" onClick={() => toggleStyle("italic")} title="Italic">
              <IconItalic />
            </button>
            <button className="icon-btn" onClick={() => toggleStyle("underline")} title="Underline">
              <IconUnderline />
            </button>
            <input className="color-swatch" type="color" defaultValue="#14181f" onChange={(event) => setAlignColor("color", event.target.value)} title="Text colour" />
            <input className="color-swatch" type="color" defaultValue="#fff3b0" onChange={(event) => setAlignColor("fill", event.target.value)} title="Fill colour" />
          </div>
          <div className="ribbon-label">Font</div>
        </div>

        <div className="ribbon-group">
          <div className="ribbon-row">
            <select className="select" onChange={(event) => applyNumberFormat(event.target.value)} defaultValue="" title="Number format">
              {NUMBER_FORMATS.map((format) => (
                <option key={format.label} value={format.value}>
                  {format.label}
                </option>
              ))}
            </select>
          </div>
          <div className="ribbon-label">Number</div>
        </div>

        <div className="ribbon-group">
          <div className="ribbon-row">
            <button className="icon-btn" onClick={() => setAlignment("left")} title="Align left">
              <IconAlignLeft />
            </button>
            <button className="icon-btn" onClick={() => setAlignment("center")} title="Centre">
              <IconAlignCenter />
            </button>
            <button className="icon-btn" onClick={() => setAlignment("right")} title="Align right">
              <IconAlignRight />
            </button>
          </div>
          <div className="ribbon-label">Alignment</div>
        </div>

        <div className="ribbon-group">
          <div className="ribbon-row">
            <button className="icon-btn" onClick={sortAscending} title="Sort ascending">
              <IconSort />
            </button>
            <button className="icon-btn" onClick={sortDescending} title="Sort descending">
              <IconSort style={{ transform: "scaleY(-1)" }} />
            </button>
            <button className="icon-btn" onClick={removeDuplicates} title="Remove duplicate rows">
              <IconTrash />
            </button>
          </div>
          <div className="ribbon-label">Data</div>
        </div>

        <div className="ribbon-group">
          <div className="ribbon-row">
            <button className="icon-btn" onClick={() => addSheet()} title="Add sheet">
              <IconSheet />
            </button>
            <button className="icon-btn" onClick={() => renameSheet()} title="Rename sheet">
              <IconRefresh />
            </button>
          </div>
          <div className="ribbon-label">Sheets</div>
        </div>

        <div className="ribbon-group" style={{ marginLeft: "auto", borderRight: 0 }}>
          <div className="ribbon-row">
            <button className="btn" onClick={saveInPlace} disabled={busy}>
              Save
            </button>
            <button className="btn" onClick={saveAs} disabled={busy}>
              Save as…
            </button>
          </div>
          <div className="ribbon-label">File</div>
        </div>
      </div>

      <div className="formula-bar">
        <input
          className="text-input"
          style={{ width: 90 }}
          value={cellRef(active.row, active.col)}
          onChange={(event) => {
            const parsed = parseRef(event.target.value);
            if (parsed) {
              setActive(parsed);
              setSelection({ r0: parsed.row, c0: parsed.col, r1: parsed.row, c1: parsed.col });
            }
          }}
        />
        <span className="mono small muted">fx</span>
        <input
          className="text-input"
          style={{ flex: 1 }}
          value={
            editing && editing.row === active.row && editing.col === active.col
              ? editing.draft
              : cellValue(active.row, active.col)?.formula ?? cellValue(active.row, active.col)?.value ?? ""
          }
          onChange={(event) => setEditing({ ...active, draft: event.target.value })}
          onKeyDown={(event) => {
            if (event.key === "Enter") commitEdit("down");
            if (event.key === "Escape") setEditing(null);
          }}
          placeholder="Value or =formula"
        />
      </div>

      <div className="workspace-body with-right">
        <div
          className="canvas-area"
          ref={wrapRef}
          onScroll={(event) => {
            const target = event.currentTarget;
            setScroll({
              top: target.scrollTop,
              left: target.scrollLeft,
              width: target.clientWidth,
              height: target.clientHeight,
            });
          }}
          tabIndex={0}
          onKeyDown={onKeyDown}
        >
          <div style={{ position: "relative", width: totalWidth + ROW_HEADER_WIDTH, height: totalHeight + HEADER_HEIGHT }}>
            <div
              style={{
                position: "sticky",
                top: 0,
                height: HEADER_HEIGHT,
                zIndex: 4,
                background: "var(--bg-sunken)",
                width: totalWidth + ROW_HEADER_WIDTH,
              }}
            >
              <div
                style={{
                  position: "sticky",
                  left: 0,
                  width: ROW_HEADER_WIDTH,
                  height: HEADER_HEIGHT,
                  display: "inline-block",
                  background: "var(--bg-sunken)",
                  borderRight: "1px solid var(--border)",
                  borderBottom: "1px solid var(--border)",
                  zIndex: 5,
                }}
              />
              {Array.from({ length: lastCol - firstCol + 1 }, (_, index) => firstCol + index).map((col) => (
                <div
                  key={col}
                  style={{
                    position: "absolute",
                    left: ROW_HEADER_WIDTH + col * COL_WIDTH,
                    top: 0,
                    width: COL_WIDTH,
                    height: HEADER_HEIGHT,
                    display: "grid",
                    placeItems: "center",
                    borderRight: "1px solid var(--border)",
                    borderBottom: "1px solid var(--border)",
                    fontSize: 11,
                    color: col >= selection.c0 && col <= selection.c1 ? "var(--accent)" : "var(--text-muted)",
                    background: col >= selection.c0 && col <= selection.c1 ? "var(--accent-soft)" : "var(--bg-sunken)",
                    fontWeight: 600,
                  }}
                >
                  {colName(col)}
                </div>
              ))}
            </div>

            {Array.from({ length: lastRow - firstRow + 1 }, (_, index) => firstRow + index).map((row) => (
              <div
                key={row}
                style={{
                  position: "absolute",
                  top: HEADER_HEIGHT + row * ROW_HEIGHT,
                  left: 0,
                  width: totalWidth + ROW_HEADER_WIDTH,
                  height: ROW_HEIGHT,
                }}
              >
                <div
                  style={{
                    position: "sticky",
                    left: 0,
                    width: ROW_HEADER_WIDTH,
                    height: ROW_HEIGHT,
                    display: "grid",
                    placeItems: "center",
                    background: row >= selection.r0 && row <= selection.r1 ? "var(--accent-soft)" : "var(--bg-sunken)",
                    color: row >= selection.r0 && row <= selection.r1 ? "var(--accent)" : "var(--text-muted)",
                    borderRight: "1px solid var(--border)",
                    borderBottom: "1px solid var(--border)",
                    fontSize: 11,
                    fontWeight: 600,
                    zIndex: 2,
                  }}
                >
                  {row + 1}
                </div>
                {Array.from({ length: lastCol - firstCol + 1 }, (_, index) => firstCol + index).map((col) => {
                  const cell = cellValue(row, col);
                  const isActive = row === active.row && col === active.col;
                  const inRange =
                    row >= selection.r0 && row <= selection.r1 && col >= selection.c0 && col <= selection.c1;
                  const value = displayAt(row, col);
                  const numeric = cell?.format ? detectFormatKind(cell.format) : "general";
                  const align = cell?.align ?? (numeric === "general" ? "left" : "right");
                  return (
                    <div
                      key={col}
                      onMouseDown={(event) => {
                        if (event.shiftKey) {
                          setSelection({ r0: anchor.row, c0: anchor.col, r1: row, c1: col });
                          setActive({ row, col });
                          return;
                        }
                        setActive({ row, col });
                        setAnchor({ row, col });
                        setSelection({ r0: row, c0: col, r1: row, c1: col });
                      }}
                      onDoubleClick={() =>
                        setEditing({ row, col, draft: cell?.formula ?? cell?.value ?? "" })
                      }
                      style={{
                        position: "absolute",
                        left: ROW_HEADER_WIDTH + col * COL_WIDTH,
                        top: 0,
                        width: COL_WIDTH,
                        height: ROW_HEIGHT,
                        padding: "0 6px",
                        display: "flex",
                        alignItems: "center",
                        justifyContent:
                          align === "center" ? "center" : align === "right" ? "flex-end" : "flex-start",
                        borderRight: "1px solid var(--border-soft)",
                        borderBottom: "1px solid var(--border-soft)",
                        background: inRange && !isActive ? "color-mix(in srgb, var(--accent) 8%, var(--bg-surface))" : "var(--bg-surface)",
                        outline: isActive ? "2px solid var(--accent)" : undefined,
                        outlineOffset: isActive ? -2 : undefined,
                        zIndex: isActive ? 3 : 1,
                        overflow: "hidden",
                        whiteSpace: "nowrap",
                        textOverflow: "ellipsis",
                        fontWeight: cell?.bold ? 700 : 400,
                        fontStyle: cell?.italic ? "italic" : "normal",
                        textDecoration: cell?.underline ? "underline" : "none",
                        color: cell?.color ?? (value.startsWith("#") ? "var(--danger)" : "var(--text)"),
                        backgroundColor: cell?.fill ?? undefined,
                      }}
                      title={cell?.formula ?? undefined}
                    >
                      {isActive && editing ? (
                        <input
                          autoFocus
                          value={editing.draft}
                          onChange={(event) => setEditing({ ...editing, draft: event.target.value })}
                          onKeyDown={(event) => {
                            if (event.key === "Enter") commitEdit("down");
                            if (event.key === "Tab") commitEdit("right");
                            if (event.key === "Escape") setEditing(null);
                          }}
                          style={{
                            width: "100%",
                            border: 0,
                            outline: "none",
                            background: "transparent",
                            font: "inherit",
                            padding: 0,
                          }}
                        />
                      ) : (
                        value
                      )}
                    </div>
                  );
                })}
              </div>
            ))}
          </div>
        </div>

        <div className="inspector">
          <div className="inspector-section">
            <h3>Selection</h3>
            <p className="small muted">
              {cellRef(selection.r0, selection.c0)}
              {selection.r0 !== selection.r1 || selection.c0 !== selection.c1
                ? `:${cellRef(selection.r1, selection.c1)}`
                : ""}
            </p>
            <p className="small muted">
              Sum {Math.round(selectionStats.sum * 100) / 100} · Average{" "}
              {selectionStats.numeric ? Math.round(selectionStats.average * 100) / 100 : "—"} · Count{" "}
              {selectionStats.count}
            </p>
          </div>

          <div className="inspector-section">
            <h3>Filter</h3>
            <input className="text-input" placeholder="Value contains" value={filterValue} onChange={(event) => setFilterValue(event.target.value)} />
            <div className="col" style={{ marginTop: 8 }}>
              <button className="btn" onClick={() => hideRowsNotMatching(filterValue)} disabled={!filterValue}>
                <IconFilter /> Hide rows that do not match
              </button>
              <button className="btn" onClick={() => setWorkbook((current) => ({ ...current }))}>
                Restore rows
              </button>
            </div>
          </div>

          <div className="inspector-section">
            <h3>Find and replace</h3>
            <input className="text-input" placeholder="Find" value={findText} onChange={(event) => setFindText(event.target.value)} />
            <input
              className="text-input"
              placeholder="Replace with"
              value={replaceText}
              onChange={(event) => setReplaceText(event.target.value)}
              style={{ marginTop: 6 }}
            />
            <button className="btn" style={{ marginTop: 8 }} onClick={replaceAll}>
              <IconSearch /> Replace in sheet
            </button>
          </div>

          <div className="inspector-section">
            <h3>Chart</h3>
            <input className="text-input" value={chartRange} onChange={(event) => setChartRange(event.target.value.toUpperCase())} />
            <div className="seg" style={{ marginTop: 8 }}>
              {(["bar", "line", "pie"] as const).map((type) => (
                <button key={type} className={chartType === type ? "on" : ""} onClick={() => setChartType(type)}>
                  {type}
                </button>
              ))}
            </div>
            <canvas ref={canvasRef} width={240} height={150} style={{ marginTop: 10, width: 240, height: 150, background: "var(--bg-surface)", borderRadius: 6 }} />
            <p className="small muted" style={{ marginTop: 6 }}>
              <IconChart /> Charts are drawn from the live sheet data.
            </p>
          </div>

          <div className="inspector-section">
            <h3>Clipboard</h3>
            <div className="row">
              <button className="btn" onClick={copySelection}>
                <IconCopy /> Copy
              </button>
              <button className="btn" onClick={pasteClipboard}>
                <IconPaste /> Paste
              </button>
            </div>
          </div>

          {formattingRisk && (
            <div className="inspector-section">
              <div className="alert warn">
                This workbook contains cell formatting that the editor cannot read back. Values, formulas and
                TEDROX formatting are preserved; other styling may be lost when saving.
              </div>
            </div>
          )}
        </div>
      </div>

      <div className="sheet-tabs">
        {workbook.sheets.map((item, index) => (
          <button
            key={`${item.name}-${index}`}
            className={`sheet-tab${index === sheetIndex ? " active" : ""}`}
            onClick={() => {
              setSheetIndex(index);
              setActive({ row: 0, col: 0 });
              setSelection({ r0: 0, c0: 0, r1: 0, c1: 0 });
            }}
          >
            {item.name}
          </button>
        ))}
        <button className="sheet-tab" onClick={() => addSheet()}>
          +
        </button>
        <span className="spacer" />
        <span className="small muted">
          {sheetNames.length} sheet{sheetNames.length === 1 ? "" : "s"}
        </span>
      </div>

      <div className="statusbar">
        <span className="status-pill">
          <IconSheet /> {fileName(path ?? displayName)}
        </span>
        <span className="sep" />
        <span>{cellRef(active.row, active.col)}</span>
        <span className="sep" />
        <span>{sheet?.cells.size ?? 0} cells with content</span>
        <span className="spacer" />
        {dirty && <span style={{ color: "var(--accent)" }}>Unsaved changes</span>}
      </div>
    </div>
  );

  function activeCellIs(style: "bold" | "italic" | "underline"): boolean {
    return Boolean(cellValue(active.row, active.col)?.[style]);
  }

  function toggleStyle(style: "bold" | "italic" | "underline") {
    const current = Boolean(cellValue(active.row, active.col)?.[style]);
    mutateCells((cell) => ({ ...cell, [style]: !current }));
  }

  function setAlignColor(key: "color" | "fill", value: string) {
    mutateCells((cell) => ({ ...cell, [key]: value.replace("#", "").toUpperCase() }));
  }

  function setAlignment(align: CellAlign) {
    mutateCells((cell) => ({ ...cell, align: cell.align === align ? null : align }));
  }

  function applyNumberFormat(format: string) {
    mutateCells((cell) => ({ ...cell, format: format || null }));
  }

  function addSheet() {
    setWorkbook((current) => ({
      sheets: [...current.sheets, { name: `Sheet${current.sheets.length + 1}`, cells: new Map(), rows: 0, cols: 0 }],
    }));
    setDirty(true);
  }

  function renameSheet() {
    const current = workbook.sheets[sheetIndex];
    const next = window.prompt("Sheet name", current?.name ?? "Sheet1");
    if (!next) return;
    setWorkbook((value) => ({
      sheets: value.sheets.map((item, index) => (index === sheetIndex ? { ...item, name: next } : item)),
    }));
    setDirty(true);
  }

  function sortAscending() {
    sortSelection(1);
  }

  function sortDescending() {
    sortSelection(-1);
  }

  function sortSelection(direction: number) {
    if (!sheet) return;
    const col = selection.c0;
    const rowsList: CellModel[][] = [];
    const rowIndices: number[] = [];
    for (let row = selection.r0; row <= selection.r1; row += 1) {
      const line: CellModel[] = [];
      for (let c = 0; c < Math.max(sheet.cols, selection.c1 + 1); c += 1) {
        line.push(sheet.cells.get(keyOf(row, c)) ?? { row, col: c, value: "" });
      }
      rowsList.push(line);
      rowIndices.push(row);
    }
    const key = (row: CellModel[]) => {
      const raw = row[col];
      const numeric = Number((raw?.value ?? "").replace(/,/g, ""));
      return Number.isFinite(numeric) && (raw?.value ?? "").trim() !== "" ? numeric : Number.NaN;
    };
    const sorted = rowsList
      .map((row, index) => ({ row, index }))
      .sort((a, b) => {
        const left = key(a.row);
        const right = key(b.row);
        if (Number.isNaN(left) && Number.isNaN(right)) {
          return direction * (a.row[col]?.value ?? "").localeCompare(b.row[col]?.value ?? "");
        }
        if (Number.isNaN(left)) return 1;
        if (Number.isNaN(right)) return -1;
        return direction * (left - right);
      });
    setWorkbook((current) => {
      const sheets = current.sheets.map((item, index) => {
        if (index !== sheetIndex) return item;
        const cells = new Map(item.cells);
        sorted.forEach((entry, position) => {
          const targetRow = rowIndices[position];
          entry.row.forEach((cell, c) => {
            const existing = cells.get(keyOf(targetRow, c));
            if (cell.value === "" && !cell.formula && !existing) return;
            cells.set(keyOf(targetRow, c), { ...cell, row: targetRow, col: c });
          });
        });
        return { ...item, cells };
      });
      return { sheets };
    });
    setDirty(true);
    pushToast({ kind: "success", message: direction > 0 ? "Sorted ascending" : "Sorted descending" });
  }

  function removeDuplicates() {
    if (!sheet) return;
    const signature = (row: number) =>
      Array.from({ length: Math.max(sheet.cols, selection.c1 + 1) }, (_, col) =>
        sheet.cells.get(keyOf(row, col))?.value ?? "",
      ).join("\u0001");
    const seen = new Set<string>();
    const keep: number[] = [];
    for (let row = selection.r0; row <= selection.r1; row += 1) {
      const key = signature(row);
      if (seen.has(key)) continue;
      seen.add(key);
      keep.push(row);
    }
    const removed = selection.r1 - selection.r0 + 1 - keep.length;
    if (removed === 0) {
      pushToast({ kind: "info", message: "No duplicate rows found" });
      return;
    }
    setWorkbook((current) => {
      const sheets = current.sheets.map((item, index) => {
        if (index !== sheetIndex) return item;
        const cells = new Map(item.cells);
        keep.forEach((sourceRow, position) => {
          const targetRow = selection.r0 + position;
          const width = Math.max(item.cols, selection.c1 + 1);
          for (let col = 0; col < width; col += 1) {
            const value = cells.get(keyOf(sourceRow, col));
            const targetKey = keyOf(targetRow, col);
            if (value) cells.set(targetKey, { ...value, row: targetRow, col });
            else cells.delete(targetKey);
          }
        });
        for (let row = selection.r0 + keep.length; row <= selection.r1; row += 1) {
          const width = Math.max(item.cols, selection.c1 + 1);
          for (let col = 0; col < width; col += 1) cells.delete(keyOf(row, col));
        }
        return { ...item, cells };
      });
      return { sheets };
    });
    setDirty(true);
    pushToast({ kind: "success", message: `Removed ${removed} duplicate row${removed === 1 ? "" : "s"}` });
  }

  function hideRowsNotMatching(needle: string) {
    if (!sheet) return;
    const lower = needle.toLowerCase();
    let kept = 0;
    setWorkbook((current) => {
      const sheets = current.sheets.map((item, index) => {
        if (index !== sheetIndex) return item;
        const cells = new Map(item.cells);
        for (let row = 0; row < item.rows; row += 1) {
          const matches = Array.from({ length: item.cols }, (_, col) =>
            (cells.get(keyOf(row, col))?.value ?? "").toLowerCase(),
          ).some((value) => value.includes(lower));
          if (!matches) {
            for (let col = 0; col < item.cols; col += 1) cells.delete(keyOf(row, col));
          } else kept += 1;
        }
        return { ...item, cells };
      });
      return { sheets };
    });
    setDirty(true);
    pushToast({ kind: "success", message: `${kept} matching rows kept`, detail: "Use Reload to restore the file" });
  }

  function replaceAll() {
    if (!findText) return;
    let changed = 0;
    setWorkbook((current) => {
      const sheets = current.sheets.map((item, index) => {
        if (index !== sheetIndex) return item;
        const cells = new Map(item.cells);
        for (const [key, cell] of cells) {
          if (cell.formula) continue;
          if (cell.value.includes(findText)) {
            cells.set(key, { ...cell, value: cell.value.split(findText).join(replaceText) });
            changed += 1;
          }
        }
        return { ...item, cells };
      });
      return { sheets };
    });
    setDirty(true);
    pushToast({ kind: "success", message: `${changed} cells updated` });
  }
}
