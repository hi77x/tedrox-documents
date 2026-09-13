#!/usr/bin/env node
/** Copy canonical dictionaries from packages/i18n into the static landing. */
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const source = join(root, "packages", "i18n");
const target = join(root, "apps", "web", "i18n");

const languages = ["en", "ru"];
await mkdir(target, { recursive: true });

const dictionaries = {};
for (const language of languages) {
  const content = await readFile(join(source, `${language}.json`), "utf8");
  dictionaries[language] = JSON.parse(content);
  await writeFile(join(target, `${language}.json`), `${JSON.stringify(dictionaries[language], null, 2)}\n`);
}

const english = Object.keys(dictionaries.en).sort();
const problems = [];
for (const language of languages) {
  const keys = Object.keys(dictionaries[language]).sort();
  const missing = english.filter((key) => !keys.includes(key));
  const orphan = keys.filter((key) => !english.includes(key));
  for (const key of missing) problems.push(`${language}: missing key ${key}`);
  for (const key of orphan) problems.push(`${language}: orphan key ${key}`);
}

if (problems.length) {
  console.error("Dictionary validation failed:");
  for (const problem of problems) console.error(`  - ${problem}`);
  process.exit(1);
}
console.log(`i18n OK: ${languages.length} languages, ${english.length} keys each.`);
