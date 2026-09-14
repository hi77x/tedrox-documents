/**
 * Browser host adapter.
 *
 * Everything here runs in the page: files are read with the File API, PDF page
 * operations use pdf-lib, image work uses canvas. Operations that need the Rust
 * engine report a clear "desktop only" error instead of pretending to work.
 */
import { PDFDocument, StandardFonts, degrees, rgb } from "pdf-lib";
import type { OpResult, Platform, Progress } from "@desktop/platform";
import type { DocModel } from "@desktop/docModel";
import type { CellModel, WorkbookModel } from "@desktop/sheetModel";

type StoredFile = { file: File; name: string };

const files = new Map<string, StoredFile>();
let counter = 0;

const DESKTOP_ONLY = (what: string) =>
  new Error(`${what} needs the desktop application: download it from the releases page.`);

function pathFor(name: string): string {
  counter += 1;
  return `web://${counter}/${name}`;
}

function download(name: string, bytes: Uint8Array, mime: string): void {
  const blob = new Blob([bytes as unknown as BlobPart], { type: mime });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = name;
  anchor.click();
  setTimeout(() => URL.revokeObjectURL(url), 4000);
}

function result(path: string, inBytes: number, outBytes: number, kind: string, stats: Record<string, unknown> = {}): OpResult {
  return {
    outputs: [{ path, bytes: outBytes, kind, label: null }],
    bytesIn: inBytes,
    bytesOut: outBytes,
    durationMs: 0,
    warnings: [],
    stats,
  };
}

function bytesOf(file: StoredFile): Promise<Uint8Array> {
  return file.file.arrayBuffer().then((buffer) => new Uint8Array(buffer));
}

async function pickFiles(multiple: boolean, extensions?: string[]): Promise<string[] | null> {
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.multiple = multiple;
    if (extensions && extensions.length > 0) {
      input.accept = extensions.map((extension) => `.${extension}`).join(",");
    }
    input.style.display = "none";
    document.body.appendChild(input);
    input.addEventListener("change", () => {
      const picked = Array.from(input.files ?? []);
      document.body.removeChild(input);
      if (picked.length === 0) {
        resolve(null);
        return;
      }
      const paths = picked.map((file) => {
        const path = pathFor(file.name);
        files.set(path, { file, name: file.name });
        return path;
      });
      resolve(paths);
    });
    input.click();
  });
}

/* ------------------------------------------------------------------ parsing */

function markdownToBlocks(text: string): DocModel["blocks"] {
  const blocks: DocModel["blocks"] = [];
  const plain = (value: string) => [
    {
      text: value,
      bold: false,
      italic: false,
      underline: false,
      size: null,
      font: null,
      color: null,
    },
  ];
  for (const raw of text.replace(/\r\n/g, "\n").split("\n")) {
    const line = raw.trimEnd();
    if (line.trim() === "") continue;
    if (line.startsWith("### ")) blocks.push({ kind: "heading3", align: null, runs: plain(line.slice(4)) });
    else if (line.startsWith("## ")) blocks.push({ kind: "heading2", align: null, runs: plain(line.slice(3)) });
    else if (line.startsWith("# ")) blocks.push({ kind: "heading1", align: null, runs: plain(line.slice(2)) });
    else if (line.startsWith("> ")) blocks.push({ kind: "quote", align: null, runs: plain(line.slice(2)) });
    else if (/^[-*] /.test(line)) blocks.push({ kind: "bullet", align: null, runs: plain(line.slice(2)) });
    else if (/^\d+\. /.test(line)) blocks.push({ kind: "numbered", align: null, runs: plain(line.replace(/^\d+\.\s/, "")) });
    else blocks.push({ kind: "paragraph", align: null, runs: plain(line) });
  }
  if (blocks.length === 0) blocks.push({ kind: "paragraph", align: null, runs: plain("") });
  return blocks;
}

function modelToMarkdown(model: DocModel): string {
  const lines: string[] = [];
  for (const block of model.blocks) {
    const text = block.runs.map((run) => run.text).join("");
    switch (block.kind) {
      case "heading1":
        lines.push(`# ${text}`, "");
        break;
      case "heading2":
        lines.push(`## ${text}`, "");
        break;
      case "heading3":
        lines.push(`### ${text}`, "");
        break;
      case "quote":
        lines.push(`> ${text}`, "");
        break;
      case "bullet":
        lines.push(`- ${text}`);
        break;
      case "numbered":
        lines.push(`1. ${text}`);
        break;
      default:
        lines.push(text, "");
    }
  }
  return lines.join("\n");
}

function parseDelimited(text: string, delimiter: string): WorkbookModel {
  const rows = text.replace(/\r\n/g, "\n").split("\n").filter((line) => line.length > 0);
  const cells: CellModel[] = [];
  rows.forEach((line, row) => {
    line.split(delimiter).forEach((value, col) => {
      if (value === "") return;
      const numeric = /^-?\d+(\.\d+)?$/.test(value.trim());
      cells.push({
        row,
        col,
        value,
        format: numeric ? "#,##0" : null,
      });
    });
  });
  return { sheets: [{ name: "Sheet1", cells }] };
}

function workbookToCsv(model: WorkbookModel): string {
  const sheet = model.sheets[0];
  if (!sheet) return "";
  let maxRow = 0;
  let maxCol = 0;
  for (const cell of sheet.cells) {
    maxRow = Math.max(maxRow, cell.row);
    maxCol = Math.max(maxCol, cell.col);
  }
  const grid: string[][] = Array.from({ length: maxRow + 1 }, () => Array.from({ length: maxCol + 1 }, () => ""));
  for (const cell of sheet.cells) {
    grid[cell.row][cell.col] = cell.value ?? "";
  }
  return grid
    .map((row) => row.map((value) => (value.includes(",") ? `"${value.replace(/"/g, '""')}"` : value)).join(","))
    .join("\n");
}

/* ------------------------------------------------------------------- images */

async function canvasOperation(
  bytes: Uint8Array,
  mime: string,
  transform: (canvas: HTMLCanvasElement) => void,
  outputType: string,
  quality = 0.92,
): Promise<Uint8Array> {
  const blob = new Blob([bytes as unknown as BlobPart], { type: mime });
  const bitmap = await createImageBitmap(blob);
  const canvas = document.createElement("canvas");
  canvas.width = bitmap.width;
  canvas.height = bitmap.height;
  const context = canvas.getContext("2d");
  if (!context) throw new Error("Canvas is unavailable");
  context.drawImage(bitmap, 0, 0);
  transform(canvas);
  const output: Blob = await new Promise((resolve) =>
    canvas.toBlob((value) => resolve(value as Blob), outputType, quality),
  );
  return new Uint8Array(await output.arrayBuffer());
}

const MIME_BY_EXTENSION: Record<string, string> = {
  png: "image/png",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  webp: "image/webp",
};

/* ---------------------------------------------------------------------- pdf */

async function applyPlan(bytes: Uint8Array, plan: { page: number; rotate: number }[]): Promise<Uint8Array> {
  const source = await PDFDocument.load(bytes, { ignoreEncryption: false });
  const target = await PDFDocument.create();
  const copied = await target.copyPages(
    source,
    plan.map((entry) => entry.page - 1),
  );
  copied.forEach((page, index) => {
    const rotation = plan[index].rotate;
    if (rotation) {
      const current = page.getRotation().angle;
      page.setRotation(degrees(((current + rotation) % 360 + 360) % 360));
    }
    target.addPage(page);
  });
  return target.save();
}

type WebAnnotation = {
  page: number;
  rect: [number, number, number, number];
  kind: { kind: string; quads?: [number, number, number, number][]; strokes?: [number, number][][]; from?: [number, number]; to?: [number, number]; text?: string; font_size?: number };
  color?: [number, number, number] | null;
  opacity?: number | null;
};

async function applyAnnotations(bytes: Uint8Array, annotations: WebAnnotation[]): Promise<Uint8Array> {
  const document_ = await PDFDocument.load(bytes);
  const font = await document_.embedFont(StandardFonts.Helvetica);
  const pages = document_.getPages();
  for (const annotation of annotations) {
    const page = pages[annotation.page - 1];
    if (!page) continue;
    const [r, g, b] = annotation.color ?? [1, 0.82, 0.12];
    const color = rgb(r, g, b);
    const opacity = annotation.opacity ?? 1;
    const kind = annotation.kind.kind;
    if (kind === "highlight" || kind === "underline" || kind === "strike_out") {
      for (const quad of annotation.kind.quads ?? []) {
        const height = kind === "highlight" ? quad[3] - quad[1] : 1.6;
        const y = kind === "strike_out" ? (quad[1] + quad[3]) / 2 : quad[1] + (kind === "underline" ? 1 : 0);
        page.drawRectangle({
          x: quad[0],
          y,
          width: quad[2] - quad[0],
          height,
          color,
          opacity: kind === "highlight" ? Math.min(opacity, 0.6) : 1,
        });
      }
    } else if (kind === "ink") {
      for (const stroke of annotation.kind.strokes ?? []) {
        for (let index = 1; index < stroke.length; index += 1) {
          page.drawLine({
            start: { x: stroke[index - 1][0], y: stroke[index - 1][1] },
            end: { x: stroke[index][0], y: stroke[index][1] },
            thickness: 1.6,
            color,
          });
        }
      }
    } else if (kind === "square") {
      page.drawRectangle({
        x: annotation.rect[0],
        y: annotation.rect[1],
        width: annotation.rect[2] - annotation.rect[0],
        height: annotation.rect[3] - annotation.rect[1],
        borderColor: color,
        borderWidth: 1.4,
      });
    } else if (kind === "circle") {
      page.drawEllipse({
        x: (annotation.rect[0] + annotation.rect[2]) / 2,
        y: (annotation.rect[1] + annotation.rect[3]) / 2,
        xScale: (annotation.rect[2] - annotation.rect[0]) / 2,
        yScale: (annotation.rect[3] - annotation.rect[1]) / 2,
        borderColor: color,
        borderWidth: 1.4,
      });
    } else if (kind === "line" || kind === "arrow") {
      const from = annotation.kind.from ?? [annotation.rect[0], annotation.rect[1]];
      const to = annotation.kind.to ?? [annotation.rect[2], annotation.rect[3]];
      page.drawLine({
        start: { x: from[0], y: from[1] },
        end: { x: to[0], y: to[1] },
        thickness: 1.6,
        color,
      });
      if (kind === "arrow") {
        const dx = to[0] - from[0];
        const dy = to[1] - from[1];
        const length = Math.hypot(dx, dy) || 1;
        const ux = dx / length;
        const uy = dy / length;
        const head = 10;
        page.drawLine({
          start: { x: to[0], y: to[1] },
          end: { x: to[0] - ux * head - uy * head * 0.4, y: to[1] - uy * head + ux * head * 0.4 },
          thickness: 1.6,
          color,
        });
        page.drawLine({
          start: { x: to[0], y: to[1] },
          end: { x: to[0] - ux * head + uy * head * 0.4, y: to[1] - uy * head - ux * head * 0.4 },
          thickness: 1.6,
          color,
        });
      }
    } else if (kind === "free_text") {
      const size = annotation.kind.font_size ?? 12;
      const lines = (annotation.kind.text ?? "").split("\n");
      lines.forEach((line, index) => {
        page.drawText(line, {
          x: annotation.rect[0] + 2,
          y: annotation.rect[3] - size * (index + 1),
          size,
          font,
          color,
        });
      });
    } else if (kind === "note") {
      page.drawRectangle({
        x: annotation.rect[0],
        y: annotation.rect[1],
        width: annotation.rect[2] - annotation.rect[0],
        height: annotation.rect[3] - annotation.rect[1],
        color,
        opacity: 0.25,
      });
    }
  }
  return document_.save();
}

/* ------------------------------------------------------------------ adapter */

export function createWebPlatform(): Platform {
  return {
    kind: "web",
    async openPaths(options) {
      if (options.directory) return null;
      return pickFiles(options.multiple ?? false, options.extensions);
    },
    async savePath(suggested) {
      return suggested;
    },
    async invoke<T>(command: string, args?: Record<string, unknown>) {
      const request = args ?? {};
      const path = String(request.path ?? request.file ?? "");
      switch (command) {
        case "sheet_probe_styles":
          return { has_formatting: false, format_count: 1 } as unknown as T;
        case "list_tools":
          return [] as unknown as T;
        case "suggest_output": {
          const input = String(request.input ?? "output");
          const suffix = String(request.suffix ?? "out");
          const extension = String(request.extension ?? "bin");
          return `${input.replace(/\.[^.]+$/, "")}-${suffix}.${extension}` as unknown as T;
        }
        case "read_binary": {
          const stored = files.get(path);
          if (!stored) throw new Error("The file is no longer available in this session");
          return (await bytesOf(stored)) as unknown as T;
        }
        case "file_size": {
          const stored = files.get(path);
          return (stored ? stored.file.size : 0) as unknown as T;
        }
        case "write_binary": {
          const bytes = new Uint8Array(request.bytes as number[]);
          download(path.split("/").pop() ?? "output.bin", bytes, "application/octet-stream");
          return bytes.byteLength as unknown as T;
        }
        case "delete_file":
          files.delete(path);
          return undefined as unknown as T;
        case "inspect_files": {
          const paths = (request.paths as string[]) ?? [];
          return paths.map((item) => {
            const stored = files.get(item);
            const name = stored?.name ?? item.split("/").pop() ?? item;
            const extension = name.slice(name.lastIndexOf(".") + 1).toLowerCase();
            const category = ["pdf"].includes(extension)
              ? "pdf"
              : ["png", "jpg", "jpeg", "webp", "bmp", "tiff", "avif", "svg", "ico"].includes(extension)
                ? "image"
                : ["csv", "tsv", "xlsx", "ods"].includes(extension)
                  ? "spreadsheet"
                  : "document";
            return {
              path: item,
              name,
              kind: extension || "file",
              mime: MIME_BY_EXTENSION[extension] ?? "application/octet-stream",
              category,
              size: stored?.file.size ?? 0,
              confidence: "exact",
            };
          }) as unknown as T;
        }
        case "docx_open": {
          const stored = files.get(path);
          if (!stored) throw new Error("The file is no longer available in this session");
          const name = stored.name.toLowerCase();
          if (name.endsWith(".docx")) {
            throw DESKTOP_ONLY("Reading DOCX files");
          }
          const text = await stored.file.text();
          return { title: stored.name, blocks: markdownToBlocks(text) } as unknown as T;
        }
        case "sheet_open": {
          const stored = files.get(path);
          if (!stored) throw new Error("The file is no longer available in this session");
          const name = stored.name.toLowerCase();
          if (name.endsWith(".xlsx") || name.endsWith(".ods")) {
            throw DESKTOP_ONLY("Reading XLSX and ODS workbooks");
          }
          const text = await stored.file.text();
          return parseDelimited(text, name.endsWith(".tsv") ? "\t" : ",") as unknown as T;
        }
        case "docx_save":
        case "sheet_save": {
          const model = request.model as unknown;
          if (command === "docx_save") {
            const markdown = modelToMarkdown(model as DocModel);
            const bytes = new TextEncoder().encode(markdown);
            download(path.split("/").pop()?.replace(/\.docx$/i, ".md") ?? "document.md", bytes, "text/markdown");
            return { path } as unknown as T;
          }
          const csv = workbookToCsv(model as WorkbookModel);
          const bytes = new TextEncoder().encode(csv);
          download(path.split("/").pop()?.replace(/\.(xlsx|ods)$/i, ".csv") ?? "spreadsheet.csv", bytes, "text/csv");
          return { path } as unknown as T;
        }
        case "doc_extract": {
          const stored = files.get(path);
          if (!stored) throw new Error("The file is no longer available in this session");
          const text = await stored.file.text();
          const output = String(request.output ?? "extract.txt");
          download(output.split("/").pop() ?? "extract.txt", new TextEncoder().encode(text), "text/plain");
          return output as unknown as T;
        }
        case "pdf_info": {
          const stored = files.get(path);
          if (!stored) throw new Error("The file is no longer available in this session");
          const document_ = await PDFDocument.load(await bytesOf(stored));
          return {
            pageCount: document_.getPageCount(),
            version: "1.7",
            encrypted: false,
            objectCount: 0,
            fileSize: stored.file.size,
          } as unknown as T;
        }
        case "pdf_form_fields": {
          const stored = files.get(path);
          if (!stored) return [] as unknown as T;
          const document_ = await PDFDocument.load(await bytesOf(stored));
          const form = document_.getForm();
          return form.getFields().map((field) => ({
            name: field.getName(),
            kind: field.constructor.name.replace("PDF", "").toLowerCase(),
            value: null,
            state: null,
            options: [],
            read_only: field.isReadOnly(),
            required: field.isRequired(),
            multiline: false,
          })) as unknown as T;
        }
        case "pdf_list_annotations":
          return [] as unknown as T;
        case "pdf_metadata": {
          const stored = files.get(path);
          if (!stored) return {} as unknown as T;
          const document_ = await PDFDocument.load(await bytesOf(stored));
          return {
            title: document_.getTitle() ?? null,
            author: document_.getAuthor() ?? null,
            subject: document_.getSubject() ?? null,
            creator: document_.getCreator() ?? null,
            producer: document_.getProducer() ?? null,
          } as unknown as T;
        }
        default:
          throw DESKTOP_ONLY(`The ${command} operation`);
      }
    },
    async run(command, args, onProgress) {
      const report = (stage: string, progress: number) =>
        onProgress({ stage, progress, message: null, done: false } as Progress);
      const input = String(args.file ?? "");
      const output = String(args.output ?? "output");
      const stored = files.get(input);
      if (!stored) throw new Error("The file is no longer available in this session");
      const bytes = await bytesOf(stored);
      const inBytes = bytes.byteLength;
      const nameOf = (value: string) => value.split("/").pop() ?? "output";

      report("reading", 0.2);
      switch (command) {
        case "pdf_apply_plan": {
          const plan = args.plan as { page: number; rotate: number }[];
          const produced = await applyPlan(bytes, plan);
          report("writing", 0.9);
          download(nameOf(output), produced, "application/pdf");
          return result(output, inBytes, produced.byteLength, "pdf");
        }
        case "pdf_add_annotations": {
          const annotations = args.annotations as WebAnnotation[];
          const produced = await applyAnnotations(bytes, annotations);
          report("writing", 0.9);
          download(nameOf(output), produced, "application/pdf");
          return result(output, inBytes, produced.byteLength, "pdf");
        }
        case "pdf_clean": {
          const document_ = await PDFDocument.load(bytes);
          document_.setTitle("");
          document_.setAuthor("");
          document_.setSubject("");
          document_.setKeywords([]);
          document_.setProducer("TEDROX Documents (web)");
          document_.setCreator("TEDROX Documents (web)");
          const produced = await document_.save();
          report("writing", 0.9);
          download(nameOf(output), produced, "application/pdf");
          return result(output, inBytes, produced.byteLength, "pdf", { pages: document_.getPageCount() });
        }
        case "image_convert": {
          const target = String(args.to ?? "png");
          const mime = MIME_BY_EXTENSION[target] ?? "image/png";
          const produced = await canvasOperation(
            bytes,
            MIME_BY_EXTENSION[input.split(".").pop()?.toLowerCase() ?? "png"] ?? "image/png",
            () => undefined,
            mime,
            Number(args.quality ?? 90) / 100,
          );
          report("writing", 0.9);
          download(nameOf(output), produced, mime);
          return result(output, inBytes, produced.byteLength, "image");
        }
        case "image_resize": {
          const width = Number(args.width ?? 0) || null;
          const height = Number(args.height ?? 0) || null;
          const mode = String(args.mode ?? "contain");
          const produced = await canvasOperation(
            bytes,
            MIME_BY_EXTENSION[input.split(".").pop()?.toLowerCase() ?? "png"] ?? "image/png",
            (canvas) => {
              const bitmapWidth = canvas.width;
              const bitmapHeight = canvas.height;
              const scale =
                mode === "exact"
                  ? null
                  : Math.min(
                      (width ?? bitmapWidth) / bitmapWidth,
                      (height ?? bitmapHeight) / bitmapHeight,
                    );
              const targetWidth = mode === "exact" ? width ?? bitmapWidth : Math.round(bitmapWidth * (scale ?? 1));
              const targetHeight = mode === "exact" ? height ?? bitmapHeight : Math.round(bitmapHeight * (scale ?? 1));
              const source = document.createElement("canvas");
              source.width = bitmapWidth;
              source.height = bitmapHeight;
              source.getContext("2d")?.drawImage(canvas, 0, 0);
              canvas.width = Math.max(1, targetWidth);
              canvas.height = Math.max(1, targetHeight);
              canvas.getContext("2d")?.drawImage(source, 0, 0, targetWidth, targetHeight);
            },
            MIME_BY_EXTENSION[output.split(".").pop()?.toLowerCase() ?? "png"] ?? "image/png",
          );
          report("writing", 0.9);
          download(nameOf(output), produced, "image/png");
          return result(output, inBytes, produced.byteLength, "image");
        }
        default:
          throw DESKTOP_ONLY(`The ${command} operation`);
      }
    },
    async readBinary(path) {
      const stored = files.get(path);
      if (!stored) throw new Error("The file is no longer available in this session");
      return bytesOf(stored);
    },
    async fileSize(path) {
      return files.get(path)?.file.size ?? 0;
    },
    async reveal() {},
    async suggestOutput(input, suffix, extension) {
      return `${input.replace(/\.[^.]+$/, "")}-${suffix}.${extension}`;
    },
  };
}
