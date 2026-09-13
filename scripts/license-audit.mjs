#!/usr/bin/env node
/**
 * Audit every crate dependency in Cargo.lock against the permissive allowlist
 * and regenerate THIRD_PARTY_LICENSES.md.
 *
 * Optional external adapters (LibreOffice, PDFium) are not Rust dependencies
 * and are documented separately in the generated file.
 */
import { execFileSync } from "node:child_process";
import { writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const allowed = new Set([
  "MIT",
  "MIT-0",
  "Apache-2.0",
  "Apache-2.0-WITH-LLVM-exception",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "0BSD",
  "ISC",
  "Zlib",
  "Unicode-3.0",
  "Unicode-DFS-2016",
  "CC0-1.0",
  "Unlicense",
  "MPL-2.0",
  "NCSA",
]);

const raw = execFileSync("cargo", ["metadata", "--format-version", "1", "--locked"], {
  cwd: root,
  maxBuffer: 1024 * 1024 * 1024,
}).toString();
const metadata = JSON.parse(raw);

const packages = new Map();
for (const pkg of metadata.packages) {
  if (!pkg.source || !pkg.source.includes("crates.io")) continue;
  packages.set(`${pkg.name}@${pkg.version}`, (pkg.license || "UNKNOWN").trim());
}

function isAllowed(expression) {
  const normalized = expression.replace(/[()]/g, " ").replace(/\//g, " OR ");
  const orGroups = normalized.split(/\s+OR\s+/i);
  return orGroups.some((group) =>
    group
      .split(/\s+AND\s+/i)
      .map((token) => token.trim())
      .filter(Boolean)
      .every((token) => allowed.has(token) || token.includes("LLVM-exception"))
  );
}

const violations = [];
const rows = [];
for (const [name, license] of [...packages.entries()].sort()) {
  rows.push({ name, license });
  if (!isAllowed(license)) violations.push(`${name}: ${license}`);
}

const header = `# Third-party licenses

TEDROX Documents is MIT licensed. The native core only links dependencies
whose licenses permit redistribution under MIT terms. This file is generated
by \`scripts/license-audit.mjs\` from \`Cargo.lock\`; run it after dependency
changes.

## Rust dependencies

| Crate | License |
| --- | --- |
`;
const body = rows.map((row) => `| ${row.name} | ${row.license} |`).join("\n");
const footer = `

## Optional external adapters (not bundled)

| Adapter | Purpose | License | Distribution |
| --- | --- | --- | --- |
| LibreOffice | Legacy .doc/.xls import | MPL-2.0 | Detected on the user's machine; never bundled |
| PDFium | PDF page rasterization | BSD-3-Clause | Loaded from \`TDX_PDFIUM_PATH\` when provided; never bundled |

## Notes

- \`resvg\`/\`usvg\` are MPL-2.0: MPL applies at file level and does not
  relicense this project.
- No AGPL or commercial-only PDF engine is linked.
`;

await writeFile(join(root, "THIRD_PARTY_LICENSES.md"), header + body + footer + "\n");

if (violations.length) {
  console.error("License audit failed:");
  for (const violation of violations) console.error(`  - ${violation}`);
  process.exit(1);
}
console.log(`License audit OK: ${rows.length} crates, all permissive.`);
