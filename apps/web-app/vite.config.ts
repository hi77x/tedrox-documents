import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const here = dirname(fileURLToPath(import.meta.url));
const desktopSrc = resolve(here, "../desktop/src");

/** The browser build shares the desktop UI; Tauri modules are stubbed out. */
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: [
      { find: /^@desktop\//, replacement: `${desktopSrc}/` },
      { find: "@tauri-apps/api/core", replacement: resolve(here, "src/shims/tauri-core.ts") },
      { find: "@tauri-apps/api/event", replacement: resolve(here, "src/shims/tauri-event.ts") },
      { find: "@tauri-apps/api/webview", replacement: resolve(here, "src/shims/tauri-webview.ts") },
      { find: "@tauri-apps/plugin-dialog", replacement: resolve(here, "src/shims/tauri-dialog.ts") },
      { find: "@tauri-apps/plugin-opener", replacement: resolve(here, "src/shims/tauri-opener.ts") },
    ],
  },
  build: {
    target: "es2022",
    sourcemap: false,
    chunkSizeWarningLimit: 1200,
  },
  server: {
    port: 1421,
    strictPort: true,
  },
});
