import { StrictMode, useEffect, useRef } from "react";
import { createRoot } from "react-dom/client";
import { AppStateProvider, useStore, type TabKind } from "../state";
import { installMockPlatform } from "../platform";
import { Shell } from "../App";
import { SAMPLE_DOCUMENT, SAMPLE_PDF, SAMPLE_WORKBOOK } from "./fixtures";
import "../design/tokens.css";

type Scene =
  | "home"
  | "writer"
  | "sheets"
  | "pdf-view"
  | "pdf-organize"
  | "pdf-annotate"
  | "pdf-tools"
  | "convert"
  | "images"
  | "settings"
  | "palette";

const SCENES: Scene[] = [
  "home",
  "writer",
  "sheets",
  "pdf-view",
  "pdf-organize",
  "pdf-annotate",
  "pdf-tools",
  "convert",
  "images",
  "settings",
  "palette",
];

function sceneFromUrl(): { scene: Scene; theme: "light" | "dark" } {
  const params = new URLSearchParams(window.location.search);
  const scene = (params.get("scene") ?? "home") as Scene;
  const theme = params.get("theme") === "dark" ? "dark" : "light";
  return { scene: SCENES.includes(scene) ? scene : "home", theme };
}

function Bootstrap({ scene }: { scene: Scene }) {
  const { openTab, activateTab, setSettings, jobs, startJob, finishJob } = useStore();
  const started = useRef(false);
  const target = useRef<string | null>(null);

  useEffect(() => {
    setSettings({ theme: sceneFromUrl().theme, language: "en" });
  }, [setSettings]);

  useEffect(() => {
    if (started.current) return;
    started.current = true;
    const open = (kind: TabKind, title: string, path: string | null) => {
      target.current = openTab({ kind, title, path });
    };

    if (scene === "home") {
      // The default Home tab is enough.
    } else if (scene === "writer") {
      open("document", "annual-report.docx", "/samples/annual-report.docx");
    } else if (scene === "sheets") {
      open("sheet", "budget.xlsx", "/samples/budget.xlsx");
    } else if (scene.startsWith("pdf")) {
      open("pdf", "annual-report.pdf", SAMPLE_PDF);
    } else if (scene === "convert") {
      open("convert", "Convert", null);
    } else if (scene === "images") {
      open("images", "Images", null);
    } else if (scene === "settings") {
      open("settings", "Settings", null);
    } else if (scene === "palette") {
      open("document", "annual-report.docx", "/samples/annual-report.docx");
    }

    if (scene === "pdf-organize" || scene === "pdf-annotate" || scene === "pdf-tools") {
      window.setTimeout(() => {
        document.querySelectorAll<HTMLButtonElement>(".pdf-tool-strip .seg button").forEach((button) => {
          const label = button.textContent?.trim().toLowerCase() ?? "";
          if (scene === "pdf-organize" && label === "organize") button.click();
          if (scene === "pdf-annotate" && label === "annotate") button.click();
          if (scene === "pdf-tools" && label === "tools") button.click();
        });
      }, 1200);
    }

    window.setTimeout(() => {
      if (scene === "palette") {
        window.dispatchEvent(new KeyboardEvent("keydown", { key: "k", ctrlKey: true, bubbles: true }));
      }
      if (target.current) activateTab(target.current);
      document.body.dataset.ready = "true";
    }, 1500);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scene]);

  useEffect(() => {
    if (scene !== "convert" && scene !== "images") return;
    const id = startJob("Compress PDF", "invoice.pdf");
    finishJob(id, "done", {
      outputs: [{ path: "invoice-compressed.pdf", bytes: 91234, kind: "pdf", label: null }],
      bytesIn: 402133,
      bytesOut: 91234,
      durationMs: 1840,
      warnings: [],
      stats: {},
    });
  }, [finishJob, scene, startJob]);

  void jobs;
  return null;
}

function Harness() {
  const { scene } = sceneFromUrl();
  return (
    <AppStateProvider>
      <Bootstrap scene={scene} />
      <Shell />
    </AppStateProvider>
  );
}

const { theme } = sceneFromUrl();
document.documentElement.dataset.theme = theme;

installMockPlatform({
  document: SAMPLE_DOCUMENT,
  workbook: SAMPLE_WORKBOOK,
  pdfInfo: { pageCount: 10, version: "1.7", encrypted: false, objectCount: 0, fileSize: 17505 },
});

createRoot(document.getElementById("root") as HTMLElement).render(
  <StrictMode>
    <Harness />
  </StrictMode>,
);
