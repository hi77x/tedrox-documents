import { Channel, invoke } from "@tauri-apps/api/core";

export type OpOutput = {
  path: string;
  bytes: number;
  kind: string;
  label: string | null;
};

export type OpResult = {
  outputs: OpOutput[];
  bytesIn: number;
  bytesOut: number;
  durationMs: number;
  warnings: string[];
  stats: Record<string, unknown>;
};

export type Progress = {
  stage: string;
  progress: number;
  message: string | null;
  done: boolean;
};

export type FileInfo = {
  path: string;
  name: string;
  kind: string;
  mime: string;
  category: string;
  size: number;
  confidence: string;
};

export type OpenOptions = {
  multiple?: boolean;
  directory?: boolean;
  title?: string;
  extensions?: string[];
};

/** Everything the UI needs from its host platform. */
export type Platform = {
  kind: "desktop" | "web" | "mock";
  openPaths(options: OpenOptions): Promise<string[] | null>;
  savePath(suggested: string, extension: string, label: string): Promise<string | null>;
  invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
  run(command: string, args: Record<string, unknown>, onProgress: (p: Progress) => void): Promise<OpResult>;
  readBinary(path: string): Promise<Uint8Array>;
  fileSize(path: string): Promise<number>;
  reveal(path: string): Promise<void>;
  suggestOutput(input: string, suffix: string, extension: string): Promise<string>;
};

export const isDesktop = (): boolean =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

function desktopPlatform(): Platform {
  return {
    kind: "desktop",
    async openPaths(options) {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const selection = await open({
        multiple: options.multiple ?? false,
        directory: options.directory ?? false,
        title: options.title,
        filters: options.extensions
          ? [{ name: options.extensions.join(", ").toUpperCase(), extensions: options.extensions }]
          : undefined,
      });
      if (!selection) return null;
      return Array.isArray(selection) ? selection : [selection];
    },
    async savePath(suggested, extension, label) {
      const { save } = await import("@tauri-apps/plugin-dialog");
      const chosen = await save({
        defaultPath: suggested,
        filters: [{ name: label, extensions: [extension] }],
      });
      return chosen ?? null;
    },
    async invoke<T>(command: string, args?: Record<string, unknown>) {
      return invoke<T>(command, args);
    },
    async run(command, args, onProgress) {
      const channel = new Channel<Progress>();
      channel.onmessage = onProgress;
      return invoke<OpResult>(command, { ...args, onProgress: channel });
    },
    async readBinary(path) {
      return invoke<Uint8Array>("read_binary", { path });
    },
    async fileSize(path) {
      return invoke<number>("file_size", { path });
    },
    async reveal(path) {
      const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
      await revealItemInDir(path);
    },
    async suggestOutput(input, suffix, extension) {
      return invoke<string>("suggest_output", { input, suffix, extension });
    },
  };
}

export type MockFixtures = {
  document?: unknown;
  workbook?: unknown;
  pdfInfo?: unknown;
};

/**
 * Fallback host used by the headless screenshot harness and by the browser
 * preview build. Nothing here pretends to touch a real filesystem.
 */
function mockPlatform(fixtures: MockFixtures): Platform {
  return {
    kind: "mock",
    async openPaths() {
      return null;
    },
    async savePath() {
      return null;
    },
    async invoke<T>(command: string, args?: Record<string, unknown>) {
      const request = args ?? {};
      const path = String(request.path ?? request.file ?? "");
      if (command === "docx_open") return (fixtures.document ?? { title: null, blocks: [] }) as T;
      if (command === "sheet_open") return (fixtures.workbook ?? { sheets: [] }) as T;
      if (command === "sheet_probe_styles") return { has_formatting: false, format_count: 1 } as unknown as T;
      if (command === "pdf_info") {
        return (fixtures.pdfInfo ?? { pageCount: 0, version: "1.7", encrypted: false, objectCount: 0, fileSize: 0 }) as T;
      }
      if (command === "list_tools") return [] as unknown as T;
      if (command === "pdf_metadata") {
        return { title: "Annual Report 2026", author: "TEDROX Documents", page_count: 10 } as unknown as T;
      }
      if (command === "pdf_form_fields" || command === "pdf_list_annotations") {
        return [] as unknown as T;
      }
      if (command === "read_binary") {
        const response = await fetch(path);
        if (!response.ok) throw new Error(`The sample ${path} is not available`);
        return new Uint8Array(await response.arrayBuffer()) as unknown as T;
      }
      if (command === "file_size") return 0 as unknown as T;
      throw new Error(`The ${command} command is not available outside the desktop app`);
    },
    async run(command) {
      await new Promise((resolve) => setTimeout(resolve, 320));
      return {
        outputs: [{ path: `out/${command}`, bytes: 1024, kind: "pdf", label: null }],
        bytesIn: 4096,
        bytesOut: 1024,
        durationMs: 320,
        warnings: [],
        stats: {},
      };
    },
    async readBinary(path) {
      const response = await fetch(path);
      if (!response.ok) throw new Error(`The sample ${path} is not available`);
      return new Uint8Array(await response.arrayBuffer());
    },
    async fileSize() {
      return 0;
    },
    async reveal() {},
    async suggestOutput(input, suffix, extension) {
      return `${input.replace(/\.[^.]+$/, "")}-${suffix}.${extension}`;
    },
  };
}

let cached: Platform | null = null;

export function platform(): Platform {
  if (!cached) {
    cached = isDesktop() ? desktopPlatform() : mockPlatform({});
  }
  return cached;
}

export function installMockPlatform(fixtures: MockFixtures): void {
  cached = mockPlatform(fixtures);
}

/** Replace the host adapter entirely (used by the browser build). */
export function setPlatform(adapter: Platform): void {
  cached = adapter;
}

export function formatBytes(bytes: number): string {
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return unit === 0 ? `${Math.round(bytes)} B` : `${value.toFixed(1)} ${units[unit]}`;
}

export function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

export type FileKindLabel = "document" | "sheet" | "pdf" | "image" | "convert" | "home";

export function classify(path: string): FileKindLabel {
  const lower = path.toLowerCase();
  if (lower.endsWith(".pdf")) return "pdf";
  if (/\.(xlsx|csv|tsv|ods)$/.test(lower)) return "sheet";
  if (/\.(docx|md|txt|rtf|odt|html?)$/.test(lower)) return "document";
  if (/\.(png|jpe?g|webp|bmp|tiff?|avif|svg|ico)$/.test(lower)) return "image";
  return "convert";
}
