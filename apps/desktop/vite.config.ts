import { readFileSync, readdirSync, existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";

const here = dirname(fileURLToPath(import.meta.url));
const samplesDir = resolve(here, "../../samples");

const MIME: Record<string, string> = {
  ".pdf": "application/pdf",
  ".png": "image/png",
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".webp": "image/webp",
  ".svg": "image/svg+xml",
  ".csv": "text/csv",
  ".json": "application/json",
  ".txt": "text/plain; charset=utf-8",
  ".md": "text/markdown; charset=utf-8",
};

function sampleFiles(): { name: string; data: Buffer }[] {
  if (!existsSync(samplesDir)) return [];
  return readdirSync(samplesDir)
    .filter((name) => !name.startsWith("_") && /\.(pdf|png|jpg|jpeg|webp|svg|csv|json|docx|xlsx)$/i.test(name))
    .map((name) => ({ name, data: readFileSync(join(samplesDir, name)) }));
}

/** Serve the shared sample documents in dev and copy them into the build. */
function samplesPlugin(): Plugin {
  return {
    name: "tedrox-samples",
    configureServer(server) {
      server.middlewares.use("/samples", (request, response, next) => {
        const name = decodeURIComponent((request.url ?? "/").split("?")[0]).replace(/^\//, "");
        if (!name || name.includes("..")) {
          next();
          return;
        }
        const file = join(samplesDir, name);
        if (!existsSync(file)) {
          next();
          return;
        }
        const extension = name.slice(name.lastIndexOf("."));
        response.setHeader("Content-Type", MIME[extension] ?? "application/octet-stream");
        response.end(readFileSync(file));
      });
    },
    generateBundle() {
      for (const file of sampleFiles()) {
        this.emitFile({ type: "asset", fileName: `samples/${file.name}`, source: file.data });
      }
    },
  };
}

export default defineConfig({
  plugins: [react(), samplesPlugin()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    target: "chrome105",
    sourcemap: false,
    rollupOptions: {
      input: {
        main: resolve(here, "index.html"),
        screenshot: resolve(here, "screenshot.html"),
      },
    },
  },
});
