#!/usr/bin/env node
/**
 * Refresh apps/web/releases.json from the latest GitHub release.
 * The landing page reads this file at runtime; failures fall back to the
 * releases page, so the script never breaks a deployment.
 */
import { writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const output = join(root, "apps", "web", "releases.json");
const repository = "hi77x/tedrox-documents";

async function fetchLatest() {
  const response = await fetch(`https://api.github.com/repos/${repository}/releases/latest`, {
    headers: { "User-Agent": "tedrox-documents-releases", Accept: "application/vnd.github+json" },
  });
  if (response.status === 404) return null;
  if (!response.ok) throw new Error(`GitHub API responded with ${response.status}`);
  return response.json();
}

const manifest = {
  version: null,
  releasedAt: null,
  repository: `https://github.com/${repository}`,
  assets: [],
};

try {
  const release = await fetchLatest();
  if (release) {
    manifest.version = String(release.tag_name || "").replace(/^v/, "") || null;
    manifest.releasedAt = release.published_at || null;
    manifest.assets = (release.assets || []).map((asset) => ({
      name: asset.name,
      url: asset.browser_download_url,
      bytes: asset.size,
    }));
  }
} catch (error) {
  console.warn(`Release manifest: ${error instanceof Error ? error.message : String(error)}`);
}

await writeFile(output, `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`Wrote ${output} (version: ${manifest.version ?? "none"})`);
