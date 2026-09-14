import * as pdfjs from "pdfjs-dist";
import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";

pdfjs.GlobalWorkerOptions.workerSrc = workerUrl;

export type PdfDocument = pdfjs.PDFDocumentProxy;
export type PdfPage = pdfjs.PDFPageProxy;

export type PageText = {
  text: string;
  items: { text: string; transform: number[]; width: number; height: number }[];
};

export async function openPdf(bytes: Uint8Array): Promise<PdfDocument> {
  const copy = bytes.slice();
  return pdfjs.getDocument({ data: copy, isEvalSupported: false }).promise;
}

export function toBytes(value: unknown): Uint8Array {
  if (value instanceof Uint8Array) return value;
  if (value instanceof ArrayBuffer) return new Uint8Array(value);
  if (ArrayBuffer.isView(value)) {
    const view = value as ArrayBufferView;
    return new Uint8Array(view.buffer, view.byteOffset, view.byteLength);
  }
  throw new Error("Unsupported binary payload");
}

export async function renderPage(
  page: PdfPage,
  scale: number,
  canvas: HTMLCanvasElement,
): Promise<{ width: number; height: number }> {
  const viewport = page.getViewport({ scale });
  const ratio = Math.min(window.devicePixelRatio || 1, 2);
  const context = canvas.getContext("2d");
  if (!context) throw new Error("Canvas 2D context unavailable");
  canvas.width = Math.floor(viewport.width * ratio);
  canvas.height = Math.floor(viewport.height * ratio);
  canvas.style.width = `${Math.floor(viewport.width)}px`;
  canvas.style.height = `${Math.floor(viewport.height)}px`;
  const task = page.render({
    canvasContext: context,
    viewport,
    transform: ratio === 1 ? undefined : [ratio, 0, 0, ratio, 0, 0],
  });
  await task.promise;
  return { width: viewport.width, height: viewport.height };
}

export async function pageText(page: PdfPage): Promise<PageText> {
  const content = await page.getTextContent();
  const items = content.items
    .filter((item) => "str" in item)
    .map((item) => {
      const textItem = item as unknown as {
        str: string;
        transform: number[];
        width: number;
        height: number;
      };
      return {
        text: textItem.str,
        transform: textItem.transform,
        width: textItem.width,
        height: textItem.height,
      };
    });
  return { text: items.map((item) => item.text).join(" "), items };
}

export type OutlineNode = {
  title: string;
  page: number | null;
  children: OutlineNode[];
};

async function outlinePage(document: PdfDocument, destination: unknown): Promise<number | null> {
  try {
    let dest = destination;
    if (typeof dest === "string") dest = await document.getDestination(dest);
    if (!Array.isArray(dest) || dest.length === 0) return null;
    const index = await document.getPageIndex(dest[0] as never);
    return index + 1;
  } catch {
    return null;
  }
}

export async function loadOutline(document: PdfDocument): Promise<OutlineNode[]> {
  const outline = await document.getOutline();
  if (!outline) return [];
  const walk = async (nodes: typeof outline): Promise<OutlineNode[]> => {
    const result: OutlineNode[] = [];
    for (const node of nodes) {
      result.push({
        title: node.title,
        page: await outlinePage(document, node.dest),
        children: node.items?.length ? await walk(node.items as typeof outline) : [],
      });
    }
    return result;
  };
  return walk(outline);
}

export async function findMatches(document: PdfDocument, query: string, limit = 400): Promise<number[]> {
  const needle = query.trim().toLowerCase();
  if (needle.length < 2) return [];
  const pages: number[] = [];
  for (let index = 1; index <= document.numPages; index += 1) {
    const page = await document.getPage(index);
    const content = await page.getTextContent();
    const text = content.items
      .filter((item) => "str" in item)
      .map((item) => (item as unknown as { str: string }).str)
      .join(" ")
      .toLowerCase();
    if (text.includes(needle)) {
      pages.push(index);
      if (pages.length >= limit) break;
    }
  }
  return pages;
}
