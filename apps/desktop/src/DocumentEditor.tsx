import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { runOperation } from "./api";
import {
  DocModel,
  FONT_FAMILIES,
  FONT_SIZES,
  documentWordCount,
  fontSizeCommandLevel,
  modelToHtml,
  parseEditor,
} from "./docModel";
import { t } from "./i18n";

type Props = {
  path: string | null;
  model: DocModel;
  displayName: string;
  onSaved: (path: string, kind: "doc") => void;
};

export default function DocumentEditor({ path: initialPath, model, displayName, onSaved }: Props) {
  const editor = useRef<HTMLDivElement>(null);
  const [path, setPath] = useState<string | null>(initialPath);
  const [title] = useState(displayName);
  const [words, setWords] = useState(0);
  const [dirty, setDirty] = useState(false);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (editor.current) {
      editor.current.innerHTML = modelToHtml(model);
      setWords(documentWordCount(editor.current));
    }
  }, [model]);

  const exec = (command: string, value?: string) => {
    document.execCommand("styleWithCSS", false, "true");
    document.execCommand(command, false, value);
    editor.current?.focus();
    setDirty(true);
  };

  const block = (value: string) => exec("formatBlock", value);

  const currentModel = (): DocModel => {
    const parsed = parseEditor(editor.current as HTMLElement);
    parsed.title = title || null;
    return parsed;
  };

  const saveTo = async (target: string) => {
    setBusy(true);
    setError(null);
    try {
      await invoke("docx_save", { path: target, model: currentModel() });
      setPath(target);
      setDirty(false);
      setStatus(t("doc.saved"));
      onSaved(target, "doc");
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const saveAs = async () => {
    const target = await save({
      title: t("doc.saveAs"),
      defaultPath: path ?? `${title || t("doc.untitled")}.docx`,
      filters: [{ name: "DOCX", extensions: ["docx"] }],
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

  const exportPdf = async () => {
    if (!path) {
      setError(t("doc.saveFirst"));
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const output = path.replace(/\.docx$/i, "") + ".pdf";
      await runOperation({
        command: "convert_auto",
        args: { file: path, output },
        onProgress: () => {},
      });
      setStatus(t("doc.exported"));
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const toolbarButton = (label: string, command: string, value: string | undefined, titleKey: string, className = "") => (
    <button
      type="button"
      className={`tb-btn ${className}`}
      title={t(titleKey)}
      onMouseDown={(event) => event.preventDefault()}
      onClick={() => exec(command, value)}
    >
      {label}
    </button>
  );

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
          <button type="button" className="tb-btn wide ghost" onMouseDown={(e) => e.preventDefault()} onClick={exportPdf} disabled={busy}>
            PDF
          </button>
        </div>

        <div className="ribbon-group">
          {toolbarButton("↶", "undo", undefined, "tb.undo")}
          {toolbarButton("↷", "redo", undefined, "tb.redo")}
        </div>

        <div className="ribbon-group">
          <select
            className="tb-select"
            title={t("tb.style")}
            defaultValue="P"
            onChange={(event) => block(event.target.value)}
          >
            <option value="P">{t("tb.paragraph")}</option>
            <option value="H1">{t("tb.heading1")}</option>
            <option value="H2">{t("tb.heading2")}</option>
            <option value="H3">{t("tb.heading3")}</option>
            <option value="BLOCKQUOTE">{t("tb.quote")}</option>
          </select>
          <select
            className="tb-select"
            title={t("tb.font")}
            defaultValue="Segoe UI"
            onChange={(event) => exec("fontName", event.target.value)}
          >
            {FONT_FAMILIES.map((font) => (
              <option key={font.value} value={font.value}>
                {font.label}
              </option>
            ))}
          </select>
          <select
            className="tb-select narrow"
            title={t("tb.size")}
            defaultValue="16"
            onChange={(event) => exec("fontSize", fontSizeCommandLevel(Number(event.target.value)))}
          >
            {FONT_SIZES.map((size) => (
              <option key={size} value={size}>
                {size}
              </option>
            ))}
          </select>
        </div>

        <div className="ribbon-group">
          {toolbarButton("B", "bold", undefined, "tb.bold", "bold")}
          {toolbarButton("I", "italic", undefined, "tb.italic", "italic")}
          {toolbarButton("U", "underline", undefined, "tb.underline", "underline")}
          <label className="tb-color" title={t("tb.color")}>
            <input type="color" defaultValue="#1f2328" onChange={(event) => exec("foreColor", event.target.value)} />
          </label>
        </div>

        <div className="ribbon-group">
          {toolbarButton("⯇", "justifyLeft", undefined, "tb.alignLeft")}
          {toolbarButton("≡", "justifyCenter", undefined, "tb.alignCenter")}
          {toolbarButton("⯈", "justifyRight", undefined, "tb.alignRight")}
          {toolbarButton("☰", "justifyFull", undefined, "tb.alignJustify")}
        </div>

        <div className="ribbon-group">
          {toolbarButton("•", "insertUnorderedList", undefined, "tb.bullets")}
          {toolbarButton("1.", "insertOrderedList", undefined, "tb.numbering")}
        </div>
      </div>

      <div className="editor-canvas">
        <div
          ref={editor}
          className="paper"
          contentEditable
          suppressContentEditableWarning
          spellCheck
          onInput={() => {
            setDirty(true);
            if (editor.current) setWords(documentWordCount(editor.current));
          }}
        />
      </div>

      <div className="status-bar">
        <span>{t("doc.words")}: {words}</span>
        <span className="truncate">{path ?? t("doc.untitled")}{dirty ? " •" : ""}</span>
        <span>{status}</span>
      </div>

      {error && <div className="alert error floating">{error}</div>}
    </section>
  );
}
