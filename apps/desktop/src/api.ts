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

export async function inspectFiles(paths: string[]): Promise<FileInfo[]> {
  return invoke<FileInfo[]>("inspect_files", { paths });
}

export async function listTools(): Promise<unknown> {
  return invoke("list_tools");
}

export async function suggestOutput(input: string, suffix: string, extension: string): Promise<string> {
  return invoke<string>("suggest_output", { input, suffix, extension });
}

export async function pdfMetadata(path: string): Promise<Record<string, unknown>> {
  return invoke("pdf_metadata", { file: path });
}

export type RunOptions = {
  command: string;
  args: Record<string, unknown>;
  onProgress: (progress: Progress) => void;
};

export async function runOperation({ command, args, onProgress }: RunOptions): Promise<OpResult> {
  const channel = new Channel<Progress>();
  channel.onmessage = onProgress;
  return invoke<OpResult>(command, { ...args, onProgress: channel });
}

export function formatBytes(bytes: number): string {
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return unit === 0 ? `${bytes} B` : `${value.toFixed(1)} ${units[unit]}`;
}
