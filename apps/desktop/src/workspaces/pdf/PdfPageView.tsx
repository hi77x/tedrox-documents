import { useCallback, useEffect, useRef, useState } from "react";
import type { PdfDocument } from "../../lib/pdf";
import { renderPage } from "../../lib/pdf";

export type HighlightBox = {
  page: number;
  rect: [number, number, number, number];
  color: string;
  opacity: number;
  current?: boolean;
};

type Props = {
  document: PdfDocument;
  pageNumber: number;
  scale: number;
  shouldRender: boolean;
  textLayer: boolean;
  highlights?: HighlightBox[];
  onVisible: (page: number) => void;
  overlay?: (size: { width: number; height: number; scale: number }) => React.ReactNode;
};

export function PdfPageView({
  document,
  pageNumber,
  scale,
  shouldRender,
  textLayer,
  highlights,
  onVisible,
  overlay,
}: Props) {
  const wrapper = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const textRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState<{ width: number; height: number }>({ width: 0, height: 0 });

  useEffect(() => {
    const node = wrapper.current;
    if (!node) return;
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) onVisible(pageNumber);
        }
      },
      { rootMargin: "-45% 0px -45% 0px" },
    );
    observer.observe(node);
    return () => observer.disconnect();
  }, [pageNumber, onVisible]);

  useEffect(() => {
    if (!shouldRender) return;
    let cancelled = false;
    let layer: { cancel: () => void } | null = null;

    void (async () => {
      const page = await document.getPage(pageNumber);
      if (cancelled || !canvasRef.current) return;
      const rendered = await renderPage(page, scale, canvasRef.current);
      setSize(rendered);
      if (cancelled || !textRef.current || !textLayer) return;
      const pdfjs = await import("pdfjs-dist");
      textRef.current.replaceChildren();
      const textLayerInstance = new pdfjs.TextLayer({
        textContentSource: page.streamTextContent(),
        container: textRef.current,
        viewport: page.getViewport({ scale }),
      });
      layer = textLayerInstance as unknown as { cancel: () => void };
      await textLayerInstance.render();
      if (cancelled) textRef.current.replaceChildren();
    })();

    return () => {
      cancelled = true;
      layer?.cancel();
    };
  }, [document, pageNumber, scale, shouldRender, textLayer]);

  const highlightSpans = useCallback(() => undefined, []);

  return (
    <div
      ref={wrapper}
      className="pdf-page-wrap"
      style={{ width: size.width || undefined, height: size.height || undefined }}
      data-page={pageNumber}
    >
      {shouldRender ? <canvas ref={canvasRef} /> : <div style={{ width: size.width || 600, height: size.height || 800 }} />}
      {shouldRender && textLayer ? <div className="textLayer" ref={textRef} onLoad={highlightSpans} /> : null}
      {shouldRender && highlights
        ? highlights
            .filter((box) => box.page === pageNumber)
            .map((box, index) => (
              <div
                key={index}
                style={{
                  position: "absolute",
                  left: box.rect[0],
                  top: box.rect[1],
                  width: Math.max(1, box.rect[2] - box.rect[0]),
                  height: Math.max(1, box.rect[3] - box.rect[1]),
                  background: box.color,
                  opacity: box.opacity,
                  pointerEvents: "none",
                  zIndex: 4,
                }}
              />
            ))
        : null}
      {shouldRender && overlay ? (
        <div style={{ position: "absolute", inset: 0, zIndex: 5 }}>{overlay({ ...size, scale })}</div>
      ) : null}
    </div>
  );
}

export type ThumbProps = {
  document: PdfDocument;
  pageNumber: number;
  width: number;
  active: boolean;
  selected: boolean;
  onClick: (page: number, event: React.MouseEvent) => void;
};

export function PdfThumb({ document, pageNumber, width, active, selected, onClick }: ThumbProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const page = await document.getPage(pageNumber);
      const base = page.getViewport({ scale: 1 });
      if (cancelled || !canvasRef.current) return;
      await renderPage(page, width / base.width, canvasRef.current);
    })();
    return () => {
      cancelled = true;
    };
  }, [document, pageNumber, width]);

  return (
    <button
      type="button"
      className={`thumb${active ? " active" : ""}${selected ? " selected" : ""}`}
      onClick={(event) => onClick(pageNumber, event)}
    >
      <canvas ref={canvasRef} />
      <span className="thumb-label">
        <span>{pageNumber}</span>
      </span>
    </button>
  );
}
