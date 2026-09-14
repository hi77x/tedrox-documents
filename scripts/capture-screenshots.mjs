#!/usr/bin/env node
/**
 * Headless screenshot capture.
 *
 * Starts the built harness with `vite preview`, then captures the real product
 * UI for every scene. Nothing is hand-painted: each image is the application
 * rendered from production components with deterministic fixtures.
 */
import { spawn } from "node:child_process";
import { mkdir, rm } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const app = join(root, "apps", "desktop");
const output = join(root, "assets", "screenshots");
const PORT = Number(process.env.TDX_SCREENSHOT_PORT ?? 4319);
const BASE = `http://127.0.0.1:${PORT}`;

const SCENES = [
  { name: "home", scene: "home", width: 1440, height: 900, theme: "light" },
  { name: "document", scene: "writer", width: 1440, height: 900, theme: "light" },
  { name: "spreadsheet", scene: "sheets", width: 1440, height: 900, theme: "light" },
  { name: "pdf-view", scene: "pdf-view", width: 1440, height: 900, theme: "light" },
  { name: "pdf-organize", scene: "pdf-organize", width: 1440, height: 900, theme: "light" },
  { name: "pdf-annotate", scene: "pdf-annotate", width: 1440, height: 900, theme: "light" },
  { name: "pdf-tools", scene: "pdf-tools", width: 1440, height: 900, theme: "light" },
  { name: "convert", scene: "convert", width: 1440, height: 900, theme: "light" },
  { name: "images", scene: "images", width: 1440, height: 900, theme: "light" },
  { name: "settings", scene: "settings", width: 1440, height: 900, theme: "light" },
  { name: "command-palette", scene: "palette", width: 1440, height: 900, theme: "light" },
  { name: "home-dark", scene: "home", width: 1440, height: 900, theme: "dark" },
  { name: "document-dark", scene: "writer", width: 1440, height: 900, theme: "dark" },
  { name: "pdf-view-dark", scene: "pdf-view", width: 1440, height: 900, theme: "dark" },
  { name: "hero", scene: "writer", width: 1600, height: 1000, theme: "light" },
  { name: "og", scene: "pdf-view", width: 1200, height: 630, theme: "light" },
  { name: "mobile-pdf", scene: "pdf-view", width: 420, height: 880, theme: "light" },
];

async function waitForServer(attempts = 80) {
  for (let index = 0; index < attempts; index += 1) {
    try {
      const response = await fetch(`${BASE}/screenshot.html`);
      if (response.ok) return;
    } catch {
      /* not up yet */
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error("The preview server did not start");
}

async function main() {
  await rm(output, { recursive: true, force: true });
  await mkdir(output, { recursive: true });

  const viteBin = join(app, "node_modules", "vite", "bin", "vite.js");
  const server = spawn(
    process.execPath,
    [viteBin, "preview", "--port", String(PORT), "--strictPort", "--host", "127.0.0.1"],
    { cwd: app, stdio: "ignore" },
  );

  const channel = process.env.TDX_BROWSER_CHANNEL || undefined;
  let browser;
  try {
    await waitForServer();
    browser = await chromium.launch({ channel, args: ["--disable-lcd-text"] });
    const context = await browser.newContext({ deviceScaleFactor: 2 });
    const page = await context.newPage();
    await page.emulateMedia({ reducedMotion: "reduce" });

    for (const item of SCENES) {
      await page.setViewportSize({ width: item.width, height: item.height });
      await page.goto(`${BASE}/screenshot.html?scene=${item.scene}&theme=${item.theme}`, {
        waitUntil: "domcontentloaded",
        timeout: 60000,
      });
      await page.waitForFunction(() => document.body.dataset.ready === "true", null, { timeout: 45000 });
      await page.evaluate(() => document.fonts.ready);
      await page.waitForTimeout(1400);
      await page.screenshot({
        path: join(output, `${item.name}.png`),
        animations: "disabled",
      });
      console.log(`captured ${item.name} (${item.width}x${item.height}, ${item.theme})`);
    }

    await context.close();
  } finally {
    if (browser) await browser.close();
    server.kill();
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
