export type DocRun = {
  text: string;
  bold: boolean;
  italic: boolean;
  underline: boolean;
  size: number | null;
  font: string | null;
  color: string | null;
};

export type DocBlock = {
  kind: string;
  align: string | null;
  runs: DocRun[];
};

export type DocModel = {
  title: string | null;
  blocks: DocBlock[];
};

const DEFAULT_RUN: DocRun = {
  text: "",
  bold: false,
  italic: false,
  underline: false,
  size: null,
  font: null,
  color: null,
};

const KEYWORD_SIZES: Record<string, number> = {
  "x-small": 10,
  small: 13,
  medium: 16,
  large: 18,
  "x-large": 24,
  "xx-large": 32,
  "xxx-large": 48,
};

const FONT_SIZE_LEVELS: Record<string, string> = {
  "10": "1",
  "13": "2",
  "16": "3",
  "18": "4",
  "24": "5",
  "32": "6",
  "48": "7",
};

export const FONT_SIZES = [10, 13, 16, 18, 24, 32, 48];
export const FONT_FAMILIES = [
  { label: "Segoe UI", value: "Segoe UI" },
  { label: "Arial", value: "Arial" },
  { label: "Calibri", value: "Calibri" },
  { label: "Georgia", value: "Georgia" },
  { label: "Times New Roman", value: "Times New Roman" },
  { label: "Courier New", value: "Courier New" },
];

export function emptyDocument(): DocModel {
  return { title: null, blocks: [{ kind: "paragraph", align: null, runs: [{ ...DEFAULT_RUN }] }] };
}

function colorToHex(value: string): string | null {
  const rgb = value.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/i);
  if (rgb) {
    return [1, 2, 3]
      .map((index) => Number(rgb[index]).toString(16).padStart(2, "0"))
      .join("");
  }
  const hex = value.match(/^#([0-9a-f]{6})$/i);
  return hex ? hex[1].toLowerCase() : null;
}

function parseFontSize(value: string): number | null {
  const trimmed = value.trim().toLowerCase();
  if (!trimmed) return null;
  if (KEYWORD_SIZES[trimmed]) return KEYWORD_SIZES[trimmed];
  const px = trimmed.match(/^([\d.]+)px$/);
  if (px) return Math.round(Number(px[1]) * 0.75);
  const pt = trimmed.match(/^([\d.]+)pt$/);
  if (pt) return Number(pt[1]);
  return null;
}

type Inherited = Omit<DocRun, "text">;

function mergeRun(runs: DocRun[], run: DocRun) {
  const last = runs[runs.length - 1];
  if (
    last &&
    last.bold === run.bold &&
    last.italic === run.italic &&
    last.underline === run.underline &&
    last.size === run.size &&
    last.font === run.font &&
    last.color === run.color
  ) {
    last.text += run.text;
    return;
  }
  runs.push(run);
}

function walk(node: Node, inherited: Inherited, runs: DocRun[]) {
  for (const child of Array.from(node.childNodes)) {
    if (child.nodeType === Node.TEXT_NODE) {
      const text = child.textContent ?? "";
      if (text) mergeRun(runs, { ...inherited, text });
      continue;
    }
    if (child.nodeType !== Node.ELEMENT_NODE) continue;
    const element = child as HTMLElement;
    const tag = element.tagName.toLowerCase();
    if (tag === "br") {
      mergeRun(runs, { ...inherited, text: "\n" });
      continue;
    }
    const next: Inherited = { ...inherited };
    if (tag === "b" || tag === "strong") next.bold = true;
    if (tag === "i" || tag === "em") next.italic = true;
    if (tag === "u") next.underline = true;
    const face = element.getAttribute("face");
    if (face) next.font = face;
    const style = element.style;
    if (style.fontWeight === "bold" || Number(style.fontWeight) >= 600) next.bold = true;
    if (style.fontStyle === "italic") next.italic = true;
    if (style.textDecorationLine?.includes("underline") || style.textDecoration?.includes("underline")) {
      next.underline = true;
    }
    if (style.color) {
      const hex = colorToHex(style.color);
      if (hex) next.color = hex;
    }
    if (style.fontSize) {
      const size = parseFontSize(style.fontSize);
      if (size) next.size = size;
    }
    if (style.fontFamily) {
      const family = style.fontFamily.split(",")[0].replace(/["']/g, "").trim();
      if (family) next.font = family;
    }
    walk(child, next, runs);
  }
}

export function parseEditor(root: HTMLElement): DocModel {
  const blocks: DocBlock[] = [];
  const items: { element: HTMLElement; list: "bullet" | "numbered" | null }[] = [];
  for (const child of Array.from(root.children)) {
    const tag = child.tagName.toLowerCase();
    if (tag === "ul" || tag === "ol") {
      for (const item of Array.from(child.children)) {
        items.push({ element: item as HTMLElement, list: tag === "ul" ? "bullet" : "numbered" });
      }
    } else {
      items.push({ element: child as HTMLElement, list: null });
    }
  }
  for (const { element, list } of items) {
    const tag = element.tagName.toLowerCase();
    let kind = "paragraph";
    if (list) kind = list;
    else if (/^h[1-6]$/.test(tag)) kind = `heading${Math.min(3, Number(tag[1]))}`;
    else if (tag === "blockquote" || tag === "pre") kind = "quote";
    const align = element.style.textAlign || null;
    const runs: DocRun[] = [];
    walk(element, { ...DEFAULT_RUN }, runs);
    if (runs.length === 0) runs.push({ ...DEFAULT_RUN });
    blocks.push({ kind, align, runs });
  }
  if (blocks.length === 0) {
    const runs: DocRun[] = [];
    walk(root, { ...DEFAULT_RUN }, runs);
    if (runs.length === 0) runs.push({ ...DEFAULT_RUN });
    blocks.push({ kind: "paragraph", align: null, runs });
  }
  return { title: null, blocks };
}

function escapeHtml(value: string): string {
  return value.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

function runHtml(run: DocRun): string {
  let style = "";
  if (run.bold) style += "font-weight:700;";
  if (run.italic) style += "font-style:italic;";
  if (run.underline) style += "text-decoration:underline;";
  if (run.color) style += `color:#${run.color};`;
  if (run.size) style += `font-size:${run.size / 2}pt;`;
  if (run.font) style += `font-family:${run.font};`;
  const text = escapeHtml(run.text).replace(/\n/g, "<br>");
  if (!style) return text;
  return `<span style="${style}">${text}</span>`;
}

function blockHtml(block: DocBlock): string {
  const align = block.align ? ` style="text-align:${block.align}"` : "";
  const inner = block.runs.map(runHtml).join("");
  switch (block.kind) {
    case "heading1":
      return `<h1${align}>${inner || "<br>"}</h1>`;
    case "heading2":
      return `<h2${align}>${inner || "<br>"}</h2>`;
    case "heading3":
      return `<h3${align}>${inner || "<br>"}</h3>`;
    case "bullet":
      return `<ul><li${align}>${inner || "<br>"}</li></ul>`;
    case "numbered":
      return `<ol><li${align}>${inner || "<br>"}</li></ol>`;
    case "quote":
      return `<blockquote${align}>${inner || "<br>"}</blockquote>`;
    default:
      return `<p${align}>${inner || "<br>"}</p>`;
  }
}

export function modelToHtml(model: DocModel): string {
  return model.blocks.map(blockHtml).join("");
}

export function documentWordCount(root: HTMLElement): number {
  const text = root.textContent ?? "";
  return text.split(/\s+/).filter(Boolean).length;
}

export function fontSizeCommandLevel(size: number): string {
  return FONT_SIZE_LEVELS[String(size)] ?? "3";
}
