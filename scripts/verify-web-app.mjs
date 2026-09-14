#!/usr/bin/env node
/**
 * Headless verification of the browser application.
 *
 * Drives the real bundle: opens a PDF and a CSV through the file chooser and
 * checks that the shared workspaces render. Also captures web screenshots.
 */
import { spawn } from "node:child_process";
import { mkdir } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const app = join(root, "apps", "web-app");
const output = join(root, "assets", "screenshots");
const PORT = Number(process.env.TDX_WEB_PORT ?? 4322);
const BASE = `http://127.0.0.1:${PORT}`;

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function waitForServer(attempts = 80) {
  for (let index = 0; index < attempts; index += 1) {
    try {
      const response = await fetch(`${BASE}/`);
      if (response.ok) return;
    } catch {
      /* not up yet */
    }
    await sleep(250);
  }
  throw new Error("The web preview server did not start");
}

async function openSample(page, samplePath, expected) {
  const chooser = page.waitForEvent("filechooser", { timeout: 15000 });
  await page.getByText("Open file", { exact: false }).first().click();
  const fileChooser = await chooser;
  await fileChooser.setFiles(samplePath);
  await page.waitForSelector(expected, { timeout: 30000 });
  await sleep(2200);
}

async function main() {
  await mkdir(output, { recursive: true });
  const server = spawn(
    process.execPath,
    [join(app, "node_modules", "vite", "bin", "vite.js"), "preview", "--port", String(PORT), "--strictPort", "--host", "127.0.0.1"],
    { cwd: app, stdio: "ignore" },
  );

  const channel = process.env.TDX_BROWSER_CHANNEL || undefined;
  let browser;
  const failures = [];
  try {
    await waitForServer();
    browser = await chromium.launch({ channel });
    const context = await browser.newContext({ deviceScaleFactor: 2, viewport: { width: 1440, height: 900 } });
    const page = await context.newPage();
    page.on("pageerror", (error) => failures.push(`page error: ${error.message}`));

    await page.goto(`${BASE}/`, { waitUntil: "domcontentloaded" });
    await page.waitForSelector(".app-shell", { timeout: 20000 });
    await page.screenshot({ path: join(output, "web-home.png"), animations: "disabled" });

    await openSample(page, join(root, "samples", "contract.pdf"), ".pdf-page-wrap canvas, .thumb canvas");
    const thumbs = await page.locator(".thumb canvas").count();
    if (thumbs === 0) failures.push("the PDF workspace rendered no thumbnails");
    await page.screenshot({ path: join(output, "web-pdf.png"), animations: "disabled" });

    await page.goto(`${BASE}/`, { waitUntil: "domcontentloaded" });
    await page.waitForSelector(".app-shell", { timeout: 20000 });
    await openSample(page, join(root, "samples", "dataset.csv"), ".sheet-tabs");
    const cells = await page.locator(".sheet-tabs").count();
    if (cells === 0) failures.push("the spreadsheet workspace did not render");
    await page.screenshot({ path: join(output, "web-sheet.png"), animations: "disabled" });

    // Narrow viewport: the compact layout must keep the workspace usable.
    await page.setViewportSize({ width: 414, height: 896 });
    await page.goto(`${BASE}/`, { waitUntil: "domcontentloaded" });
    await page.waitForSelector(".app-shell", { timeout: 20000 });
    const sidebarVisible = await page.locator(".side-panel").first().isVisible().catch(() => false);
    if (sidebarVisible) failures.push("the compact layout still shows the side panel");
    await page.screenshot({ path: join(output, "web-mobile.png"), animations: "disabled" });

    await context.close();
  } finally {
    if (browser) await browser.close();
    server.kill();
  }

  if (failures.length > 0) {
    console.error("Web application verification failed:");
    for (const failure of failures) console.error(`  - ${failure}`);
    process.exit(1);
  }
  console.log("Web application verified: home, PDF and spreadsheet render headlessly.");
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
