import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  CellModel,
  WorkbookModel,
  cellRef,
  cellsFromWorkbook,
  colName,
  evaluateWorkbook,
  keyOf,
  workbookFromCells,
} from "./sheetModel";
import { t } from "./i18n";

type Props = {
  path: string | null;
  workbook: WorkbookModel;
  displayName: string;
  onSaved: (path: string, kind: "sheet") => void;
};

const DEFAULT_ROWS = 200;
const MAX_ROWS = 400;
const DEFAULT_COLS = 26;
const MAX_COLS = 52;

export default function SpreadsheetEditor({ path: initialPath, workbook, displayName, onSaved }: Props) {
  const initial = useMemo(() => cellsFromWorkbook(workbook), [workbook]);
  const [path, setPath] = useState<string | null>(initialPath);
  const [name, setName] = useState(initial.sheets[0]?.name ?? "Sheet1");
  const [cells, setCells] = useState<Map<string, CellModel>>(
    () => new Map(initial.sheets[0]?.cells ?? []),
  );
  const [rows, setRows] = useState(DEFAULT_ROWS);
  const [cols, setCols] = useState(DEFAULT_COLS);
  const [selection, setSelection] = useState({ row: 0, col: 0 });
  const [editing, setEditing] = useState<{ row: number; col: number; value: string } | null>(null);
  const [formulaDraft, setFormulaDraft] = useState<string | null>(null);
  const [dirty, setDirty] = useState(false);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState("");
  const [error, setError] = useState<string | null>(null);
  const gridRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const loaded = cellsFromWorkbook(workbook);
    const sheet = loaded.sheets[0];
    if (sheet) {
      setCells(new Map(sheet.cells));
      setName(sheet.name);
      let maxRow = 0;
      let maxCol = 0;
      for (const cell of sheet.cells.values()) {
        maxRow = Math.max(maxRow, cell.row);
        maxCol = Math.max(maxCol, cell.col);
      }
      setRows(Math.min(MAX_ROWS, Math.max(DEFAULT_ROWS, maxRow + 20)));
      setCols(Math.min(MAX_COLS, Math.max(DEFAULT_COLS, maxCol + 3)));
      setSelection({ row: 0, col: 0 });
      setDirty(false);
    }
  }, [workbook]);

  const evaluation = useMemo(() => evaluateWorkbook(cells), [cells]);

  const rawValue = (row: number, col: number): string => {
    const cell = cells.get(keyOf(row, col));
    return cell?.formula ?? cell?.value ?? "";
  };

  const commit = (row: number, col: number, raw: string) => {
    setCells((previous) => {
      const next = new Map(previous);
      const key = keyOf(row, col);
      const existing: CellModel = next.get(key) ?? {
        row,
        col,
        value: "",
        formula: null,
        bold: false,
        italic: false,
      };
      if (raw.startsWith("=")) {
        existing.formula = raw;
        existing.value = "";
      } else {
        existing.formula = null;
        existing.value = raw;
      }
      if (!existing.value && !existing.formula && !existing.bold && !existing.italic) {
        next.delete(key);
      } else {
        next.set(key, existing);
      }
      return next;
    });
    setDirty(true);
  };

  const commitEditing = () => {
    if (!editing) return;
    commit(editing.row, editing.col, editing.value);
    setEditing(null);
    setFormulaDraft(null);
  };

  const startEdit = (row: number, col: number, seed?: string) => {
    const value = seed ?? rawValue(row, col);
    setEditing({ row, col, value });
    setFormulaDraft(null);
    gridRef.current?.focus();
  };

  const toggleFormat = (format: "bold" | "italic") => {
    setCells((previous) => {
      const next = new Map(previous);
      const key = keyOf(selection.row, selection.col);
      const existing: CellModel = next.get(key) ?? {
        row: selection.row,
        col: selection.col,
        value: "",
        formula: null,
        bold: false,
        italic: false,
      };
      if (format === "bold") existing.bold = !existing.bold;
      if (format === "italic") existing.italic = !existing.italic;
      if (!existing.value && !existing.formula && !existing.bold && !existing.italic) {
        next.delete(key);
      } else {
        next.set(key, existing);
      }
      return next;
    });
    setDirty(true);
  };

  const onGridKeyDown = (event: React.KeyboardEvent) => {
    if (editing) return;
    const { row, col } = selection;
    const move = (nextRow: number, nextCol: number) => {
      event.preventDefault();
      setSelection({
        row: Math.max(0, Math.min(rows - 1, nextRow)),
        col: Math.max(0, Math.min(cols - 1, nextCol)),
      });
    };
    switch (event.key) {
      case "ArrowUp":
        move(row - 1, col);
        break;
      case "ArrowDown":
        move(row + 1, col);
        break;
      case "ArrowLeft":
        move(row, col - 1);
        break;
      case "ArrowRight":
        move(row, col + 1);
        break;
      case "Tab":
        move(row, col + 1);
        break;
      case "Enter":
        event.preventDefault();
        startEdit(row, col);
        break;
      case "Delete":
      case "Backspace":
        event.preventDefault();
        commit(row, col, "");
        break;
      default:
        if (event.key.length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey) {
          event.preventDefault();
          startEdit(row, col, event.key);
        }
    }
  };

  const onEditKeyDown = (event: React.KeyboardEvent) => {
    if (!editing) return;
    if (event.key === "Enter") {
      event.preventDefault();
      const { row, col, value } = editing;
      commit(row, col, value);
      setEditing(null);
      setFormulaDraft(null);
      setSelection({ row: Math.min(rows - 1, row + 1), col });
      gridRef.current?.focus();
    } else if (event.key === "Tab") {
      event.preventDefault();
      const { row, col, value } = editing;
      commit(row, col, value);
      setEditing(null);
      setFormulaDraft(null);
      setSelection({ row, col: Math.min(cols - 1, col + 1) });
      gridRef.current?.focus();
    } else if (event.key === "Escape") {
      event.preventDefault();
      setEditing(null);
      setFormulaDraft(null);
      gridRef.current?.focus();
    }
  };

  const saveTo = async (target: string) => {
    setBusy(true);
    setError(null);
    try {
      const model = workbookFromCells(cells, name);
      await invoke("sheet_save", { path: target, model });
      setPath(target);
      setDirty(false);
      setStatus(t("doc.saved"));
      onSaved(target, "sheet");
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const saveAs = async () => {
    const target = await save({
      title: t("doc.saveAs"),
      defaultPath: path ?? `${displayName || "Workbook"}.xlsx`,
      filters: [
        { name: "XLSX", extensions: ["xlsx"] },
        { name: "CSV", extensions: ["csv"] },
      ],
    });
    if (target) await saveTo(target);
  };

  const saveDocument = async () => {
    if (path) {
      await saveTo(path);
    } else {
      await saveAs();
    }
  };

  const openFile = async () => {
    const selectionPaths = await open({
      multiple: false,
      title: t("sheet.open"),
      filters: [{ name: "Spreadsheets", extensions: ["xlsx", "csv", "tsv", "ods"] }],
    });
    if (!selectionPaths) return;
    const picked = Array.isArray(selectionPaths) ? selectionPaths[0] : selectionPaths;
    setBusy(true);
    setError(null);
    try {
      const loaded = await invoke<WorkbookModel>("sheet_open", { path: picked });
      const normalized = cellsFromWorkbook(loaded);
      const sheet = normalized.sheets[0];
      if (sheet) {
        setCells(new Map(sheet.cells));
        setName(sheet.name);
      }
      setPath(picked);
      setDirty(false);
      onSaved(picked, "sheet");
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const formulaBarValue = editing ? editing.value : formulaDraft ?? rawValue(selection.row, selection.col);

  return (
    <section className="editor-page">
      <div className="ribbon">
        <div className="ribbon-group">
          <button type="button" className="tb-btn wide" onMouseDown={(e) => e.preventDefault()} onClick={saveDocument} disabled={busy}>
            {t("doc.save")}
          </button>
          <button type="button" className="tb-btn wide ghost" onMouseDown={(e) => e.preventDefault()} onClick={saveAs} disabled={busy}>
            {t("doc.saveAs")}
          </button>
          <button type="button" className="tb-btn wide ghost" onMouseDown={(e) => e.preventDefault()} onClick={openFile} disabled={busy}>
            {t("sheet.open")}
          </button>
        </div>
        <div className="ribbon-group">
          <button
            type="button"
            className={`tb-btn bold ${cells.get(keyOf(selection.row, selection.col))?.bold ? "on" : ""}`}
            title={t("sheet.bold")}
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => toggleFormat("bold")}
          >
            B
          </button>
          <button
            type="button"
            className={`tb-btn italic ${cells.get(keyOf(selection.row, selection.col))?.italic ? "on" : ""}`}
            title={t("sheet.italic")}
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => toggleFormat("italic")}
          >
            I
          </button>
        </div>
        <div className="ribbon-group">
          <button type="button" className="tb-btn wide ghost" onMouseDown={(e) => e.preventDefault()} onClick={() => setRows((value) => Math.min(MAX_ROWS, value + 50))}>
            {t("sheet.addRow")}
          </button>
          <button type="button" className="tb-btn wide ghost" onMouseDown={(e) => e.preventDefault()} onClick={() => setCols((value) => Math.min(MAX_COLS, value + 1))}>
            {t("sheet.addCol")}
          </button>
        </div>
        <div className="ribbon-group">
          <input
            className="tb-input"
            value={name}
            title={t("sheet.sheetName")}
            onChange={(event) => {
              setName(event.target.value);
              setDirty(true);
            }}
          />
        </div>
      </div>

      <div className="formula-bar">
        <span className="cell-ref">{cellRef(selection.row, selection.col)}</span>
        <input
          value={formulaBarValue}
          placeholder={t("sheet.formula")}
          onChange={(event) => {
            if (editing) {
              setEditing({ ...editing, value: event.target.value });
            } else {
              setFormulaDraft(event.target.value);
            }
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              const value = formulaDraft ?? editing?.value ?? "";
              commit(selection.row, selection.col, value);
              setEditing(null);
              setFormulaDraft(null);
              gridRef.current?.focus();
            }
          }}
          onBlur={() => {
            if (formulaDraft !== null) {
              commit(selection.row, selection.col, formulaDraft);
              setFormulaDraft(null);
            }
          }}
        />
      </div>

      <div className="grid-wrap" ref={gridRef} tabIndex={0} onKeyDown={onGridKeyDown}>
        <table className="sheet-grid">
          <thead>
            <tr>
              <th className="corner" />
              {Array.from({ length: cols }, (_, col) => (
                <th key={col} className={selection.col === col ? "active" : ""}>
                  {colName(col)}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {Array.from({ length: rows }, (_, row) => (
              <tr key={row}>
                <th className={selection.row === row ? "active" : ""}>{row + 1}</th>
                {Array.from({ length: cols }, (_, col) => {
                  const key = keyOf(row, col);
                  const cell = cells.get(key);
                  const selected = selection.row === row && selection.col === col;
                  const editingThis = editing?.row === row && editing?.col === col;
                  const value = cell?.formula ? evaluation.display.get(key) ?? "" : cell?.value ?? "";
                  const hasError = evaluation.error.has(key);
                  return (
                    <td
                      key={col}
                      className={[
                        selected ? "selected" : "",
                        cell?.bold ? "cell-bold" : "",
                        cell?.italic ? "cell-italic" : "",
                        hasError ? "cell-error" : "",
                      ]
                        .filter(Boolean)
                        .join(" ")}
                      onClick={() => {
                        setSelection({ row, col });
                        setFormulaDraft(null);
                        gridRef.current?.focus();
                      }}
                      onDoubleClick={() => startEdit(row, col)}
                    >
                      {editingThis ? (
                        <input
                          autoFocus
                          value={editing?.value ?? ""}
                          onChange={(event) => setEditing({ row, col, value: event.target.value })}
                          onKeyDown={onEditKeyDown}
                          onBlur={commitEditing}
                        />
                      ) : (
                        value
                      )}
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div className="status-bar">
        <span>{t("sheet.cell")}: {cellRef(selection.row, selection.col)}</span>
        <span className="truncate">{path ?? t("sheet.untitled")}{dirty ? " •" : ""}</span>
        <span>{status}</span>
      </div>

      {error && <div className="alert error floating">{error}</div>}
    </section>
  );
}
