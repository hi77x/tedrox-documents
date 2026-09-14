/** Browser stand-ins for the Tauri modules the shared UI imports. */

export class Channel<T> {
  onmessage: ((message: T) => void) | null = null;
}

export function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  void command;
  void args;
  return Promise.reject(
    new Error("The Tauri bridge is not available in the browser build"),
  );
}

export function convertFileSrc(path: string): string {
  return path;
}
