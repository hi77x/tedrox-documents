import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useStore } from "../state";
import { fileName } from "../platform";
import {
  FONT_FAMILIES,
  FONT_SIZES,
  fontSizeCommandLevel,
  modelToHtml,
  parseEditor,
  type DocModel,
} from "../docModel";
import {
  IconAlignCenter,
  IconAlignJustify,
  IconAlignLeft,
  IconAlignRight,
  IconBold,
  IconClose,
  IconDoc,
  IconItalic,
  IconListBullet,
  IconListNumber,
  IconRedo,
  IconSearch,
  IconUnderline,
  IconUndo,
  IconZoomIn,
  IconZoomOut,
} from "../design/icons";

type Props = {
  tabId: string;
  path: string | null;
  model: DocModel;
  displayName: string;
};

const PAGE_HEIGHT = 1123;
const PAGE_MARGIN_Y = 76;
const PAGE_MARGIN_X = 84;
const PAGE_CONTENT_HEIGHT = PAGE_HEIGHT - PAGE_MARGIN_Y * 2;

/** Narrow windows start zoomed out so the page fits without horizontal scrolling. */
function initialZoom(): number {
  if (typeof window === "undefined") return 1;
  if (window.innerWidth < 560) return 0.45;
  if (window.innerWidth < 900) return 0.65;
  return 1;
}

export function WriterWorkspace({ tabId, path, model, displayName }: Props) {
  const { platform, runJob, updateTab, remember, pushToast, settings } = useStore();
  const editor = useRef<HTMLDivElement>(null);
  const [dirty, setDirty] = useState(false);
  const [zoom, setZoom] = useState(() => initialZoom());
  const [pageCount, setPageCount] = useState(1);
  const [stats, setStats] = useState({ words: 0, characters: 0 });
  const [findOpen, setFindOpen] = useState(false);
  const [findText, setFindText] = useState("");
  const [replaceText, setReplaceText] = useState("");
  const [busy, setBusy] = useState(false);

  const repaginate = useCallback(() => {
    const root = editor.current;
    if (!root) return;
    root.querySelectorAll<HTMLElement>("[data-page-break]").forEach((node) => node.remove());
    const blocks = Array.from(root.children) as HTMLElement[];
    let used = 0;
    let pages = 1;
    for (const block of blocks) {
      const style = getComputedStyle(block);
      const height =
        block.offsetHeight + (parseFloat(style.marginBottom) || 0) + (parseFloat(style.marginTop) || 0);
      if (used > 0 && used + height > PAGE_CONTENT_HEIGHT) {
        const spacer = globalThis.document.createElement("div");
        spacer.dataset.pageBreak = "true";
        spacer.contentEditable = "false";
        spacer.className = "page-break";
        spacer.style.height = `${Math.max(30, PAGE_CONTENT_HEIGHT - used)}px`;
        root.insertBefore(spacer, block);
        used = height;
        pages += 1;
      } else {
        used += height;
      }
    }
    setPageCount(pages);
  }, []);

  const updateStats = useCallback(() => {
    const text = editor.current?.textContent ?? "";
    setStats({ words: text.split(/\s+/).filter(Boolean).length, characters: text.length });
  }, []);

  useEffect(() => {
    const root = editor.current;
    if (!root || root.dataset.initialized === "true") return;
    root.innerHTML = modelToHtml(model);
    root.dataset.initialized = "true";
    const first = root.querySelector<HTMLElement>("p, h1, h2, h3, blockquote");
    if (first) {
      const range = globalThis.document.createRange();
      range.selectNodeContents(first);
      range.collapse(false);
    }
    updateStats();
    requestAnimationFrame(() => repaginate());
  }, [model, repaginate, updateStats]);

  useEffect(() => {
    updateTab(tabId, { title: displayName, dirty });
  }, [dirty, displayName, tabId, updateTab]);

  const onChange = useCallback(() => {
    setDirty(true);
    updateStats();
    requestAnimationFrame(() => repaginate());
  }, [repaginate, updateStats]);

  const save = useCallback(
    async (target: string) => {
      const current = parseEditor(editor.current as HTMLElement);
      await platform.invoke("docx_save", { path: target, model: current });
      setDirty(false);
      updateTab(tabId, { title: fileName(target), path: target, dirty: false });
      remember(target, "document");
      pushToast({ kind: "success", message: "Saved", detail: fileName(target) });
    },
    [platform, pushToast, remember, tabId, updateTab],
  );

  const saveCurrent = useCallback(async () => {
    setBusy(true);
    try {
      if (path) {
        await save(path);
        return;
      }
      const suggested = `${displayName.replace(/\.docx$/i, "") || "Document"}.docx`;
      const target = await platform.savePath(suggested, "docx", "Word document");
      if (!target) return;
      await save(target);
    } catch (err) {
      pushToast({ kind: "error", message: "Save failed", detail: String(err) });
    } finally {
      setBusy(false);
    }
  }, [displayName, path, platform, pushToast, save]);

  const saveAs = useCallback(async () => {
    const suggested = `${(path ? fileName(path) : displayName).replace(/\.docx$/i, "")}-copy.docx`;
    const target = await platform.savePath(suggested, "docx", "Word document");
    if (!target) return;
    setBusy(true);
    try {
      await save(target);
    } catch (err) {
      pushToast({ kind: "error", message: "Save failed", detail: String(err) });
    } finally {
      setBusy(false);
    }
  }, [displayName, path, platform, pushToast, save]);

  const exportPdf = useCallback(async () => {
    if (!path) {
      pushToast({ kind: "warn", message: "Save the document first" });
      return;
    }
    const suggested = await platform.suggestOutput(path, "export", "pdf");
    const target = await platform.savePath(suggested, "pdf", "PDF");
    if (!target) return;
    setBusy(true);
    try {
      await runJob("Export PDF", fileName(target), "convert_auto", { file: path, output: target });
    } finally {
      setBusy(false);
    }
  }, [path, platform, pushToast, runJob]);

  const exec = useCallback(
    (command: string, value?: string) => {
      editor.current?.focus();
      globalThis.document.execCommand(command, false, value);
      onChange();
    },
    [onChange],
  );

  const setBlock = useCallback(
    (kind: string) => {
      const map: Record<string, string> = {
        paragraph: "p",
        heading1: "h1",
        heading2: "h2",
        heading3: "h3",
        quote: "blockquote",
      };
      exec("formatBlock", map[kind] ?? "p");
    },
    [exec],
  );

  const selectText = useCallback((node: Text, start: number, length: number) => {
    const range = globalThis.document.createRange();
    range.setStart(node, start);
    range.setEnd(node, start + length);
    const selection = window.getSelection();
    selection?.removeAllRanges();
    selection?.addRange(range);
    node.parentElement?.scrollIntoView({ block: "center", behavior: "smooth" });
  }, []);

  const textNodes = useCallback((): Text[] => {
    const root = editor.current;
    if (!root) return [];
    const walker = globalThis.document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    const nodes: Text[] = [];
    while (walker.nextNode()) {
      const node = walker.currentNode as Text;
      if (node.data.trim()) nodes.push(node);
    }
    return nodes;
  }, []);

  const findNext = useCallback(() => {
    const needle = findText.trim().toLowerCase();
    if (!needle) return;
    const selection = window.getSelection();
    const anchor = selection?.anchorNode ?? null;
    const offset = selection?.anchorOffset ?? 0;
    const nodes = textNodes();
    let started = false;
    for (const node of nodes) {
      if (!started && anchor && node !== anchor) continue;
      started = true;
      const haystack = node.data.toLowerCase();
      const from = !started || node === anchor ? offset : 0;
      const index = haystack.indexOf(needle, node === anchor ? from : 0);
      if (index !== -1) {
        selectText(node, index, findText.length);
        return;
      }
    }
    for (const node of nodes) {
      const index = node.data.toLowerCase().indexOf(needle);
      if (index !== -1) {
        selectText(node, index, findText.length);
        return;
      }
    }
    pushToast({ kind: "info", message: "Not found", detail: findText });
  }, [findText, pushToast, selectText, textNodes]);

  const replaceAll = useCallback(() => {
    const needle = findText.trim();
    if (!needle) return;
    const regex = new RegExp(needle.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "gi");
    let changed = 0;
    for (const node of textNodes()) {
      const next = node.data.replace(regex, replaceText);
      if (next !== node.data) {
        node.data = next;
        changed += 1;
      }
    }
    if (changed > 0) onChange();
    pushToast({
      kind: changed > 0 ? "success" : "info",
      message: changed > 0 ? `Replaced in ${changed} run${changed === 1 ? "" : "s"}` : "Nothing to replace",
    });
  }, [findText, onChange, pushToast, replaceText, textNodes]);

  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      const meta = event.ctrlKey || event.metaKey;
      if (!meta) return;
      if (event.key.toLowerCase() === "s") {
        event.preventDefault();
        void (event.shiftKey ? saveAs() : saveCurrent());
      }
      if (event.key.toLowerCase() === "f") {
        event.preventDefault();
        setFindOpen(true);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [saveAs, saveCurrent]);

  const wordLabel = useMemo(
    () => `${stats.words} words · ${stats.characters} characters · ${pageCount} page${pageCount === 1 ? "" : "s"}`,
    [pageCount, stats],
  );

  return (
    <div className="workspace">
      <div className="ribbon">
        <div className="ribbon-group">
          <div className="ribbon-row">
            <button className="icon-btn" onClick={() => exec("undo")} title="Undo (Ctrl+Z)">
              <IconUndo />
            </button>
            <button className="icon-btn" onClick={() => exec("redo")} title="Redo (Ctrl+Y)">
              <IconRedo />
            </button>
          </div>
          <div className="ribbon-label">History</div>
        </div>

        <div className="ribbon-group">
          <div className="ribbon-row">
            <select className="select" defaultValue="paragraph" onChange={(event) => setBlock(event.target.value)} title="Paragraph style">
              <option value="paragraph">Normal</option>
              <option value="heading1">Heading 1</option>
              <option value="heading2">Heading 2</option>
              <option value="heading3">Heading 3</option>
              <option value="quote">Quote</option>
            </select>
            <select className="select" defaultValue="Calibri" onChange={(event) => exec("fontName", event.target.value)} title="Font family">
              {FONT_FAMILIES.map((family) => (
                <option key={family.value} value={family.value}>
                  {family.label}
                </option>
              ))}
            </select>
            <select
              className="select narrow"
              defaultValue="16"
              onChange={(event) => exec("fontSize", fontSizeCommandLevel(Number(event.target.value)))}
              title="Font size"
            >
              {FONT_SIZES.map((size) => (
                <option key={size} value={size}>
                  {size}
                </option>
              ))}
            </select>
          </div>
          <div className="ribbon-label">Font</div>
        </div>

        <div className="ribbon-group">
          <div className="ribbon-row">
            <button className="icon-btn" onClick={() => exec("bold")} title="Bold (Ctrl+B)">
              <IconBold />
            </button>
            <button className="icon-btn" onClick={() => exec("italic")} title="Italic (Ctrl+I)">
              <IconItalic />
            </button>
            <button className="icon-btn" onClick={() => exec("underline")} title="Underline (Ctrl+U)">
              <IconUnderline />
            </button>
            <input className="color-swatch" type="color" defaultValue="#14181f" onChange={(event) => exec("foreColor", event.target.value)} title="Text colour" />
            <input className="color-swatch" type="color" defaultValue="#fff3b0" onChange={(event) => exec("hiliteColor", event.target.value)} title="Highlight" />
            <button className="icon-btn" onClick={() => exec("removeFormat")} title="Clear formatting">
              <IconClose />
            </button>
          </div>
          <div className="ribbon-label">Text</div>
        </div>

        <div className="ribbon-group">
          <div className="ribbon-row">
            <button className="icon-btn" onClick={() => exec("justifyLeft")} title="Align left">
              <IconAlignLeft />
            </button>
            <button className="icon-btn" onClick={() => exec("justifyCenter")} title="Centre">
              <IconAlignCenter />
            </button>
            <button className="icon-btn" onClick={() => exec("justifyRight")} title="Align right">
              <IconAlignRight />
            </button>
            <button className="icon-btn" onClick={() => exec("justifyFull")} title="Justify">
              <IconAlignJustify />
            </button>
          </div>
          <div className="ribbon-label">Paragraph</div>
        </div>

        <div className="ribbon-group">
          <div className="ribbon-row">
            <button className="icon-btn" onClick={() => exec("insertUnorderedList")} title="Bulleted list">
              <IconListBullet />
            </button>
            <button className="icon-btn" onClick={() => exec("insertOrderedList")} title="Numbered list">
              <IconListNumber />
            </button>
          </div>
          <div className="ribbon-label">Lists</div>
        </div>

        <div className="ribbon-group">
          <div className="ribbon-row">
            <button className={`icon-btn${findOpen ? " on" : ""}`} onClick={() => setFindOpen((value) => !value)} title="Find and replace (Ctrl+F)">
              <IconSearch />
            </button>
          </div>
          <div className="ribbon-label">Editing</div>
        </div>

        <div className="ribbon-group" style={{ marginLeft: "auto", borderRight: 0 }}>
          <div className="ribbon-row">
            <button className="btn" onClick={saveCurrent} disabled={busy}>
              Save
            </button>
            <button className="btn" onClick={saveAs} disabled={busy}>
              Save as…
            </button>
            <button className="btn primary" onClick={exportPdf} disabled={busy || !path}>
              Export PDF
            </button>
          </div>
          <div className="ribbon-label">File</div>
        </div>
      </div>

      <div className="workspace-body no-side">
        <div className="canvas-area">
          {findOpen && (
            <div className="panel" style={{ margin: 12, display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
              <input className="text-input" placeholder="Find" value={findText} onChange={(event) => setFindText(event.target.value)} style={{ width: 180 }} />
              <input className="text-input" placeholder="Replace with" value={replaceText} onChange={(event) => setReplaceText(event.target.value)} style={{ width: 180 }} />
              <button className="btn" onClick={findNext}>
                Find next
              </button>
              <button className="btn" onClick={replaceAll}>
                Replace all
              </button>
              <button className="btn ghost" onClick={() => setFindOpen(false)}>
                Close
              </button>
            </div>
          )}

          <div className="writer-canvas" style={{ transform: `scale(${zoom})`, transformOrigin: "top center" }}>
            <div
              className="doc-page"
              ref={editor}
              contentEditable
              suppressContentEditableWarning
              spellCheck={settings.spellcheck}
              onInput={onChange}
              onKeyUp={onChange}
              style={{ width: 794, padding: `${PAGE_MARGIN_Y}px ${PAGE_MARGIN_X}px` }}
            />
          </div>
        </div>
      </div>

      <div className="statusbar">
        <span className="status-pill">
          <IconDoc /> {fileName(path ?? displayName)}
        </span>
        <span className="sep" />
        <span>{wordLabel}</span>
        <span className="spacer" />
        <button className="icon-btn" onClick={() => setZoom((value) => Math.max(0.35, value - 0.1))} title="Zoom out">
          <IconZoomOut />
        </button>
        <span>{Math.round(zoom * 100)}%</span>
        <button className="icon-btn" onClick={() => setZoom((value) => Math.min(2, value + 0.1))} title="Zoom in">
          <IconZoomIn />
        </button>
        {dirty && <span style={{ color: "var(--accent)" }}>Unsaved changes</span>}
      </div>
    </div>
  );
}
