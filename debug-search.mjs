import { build } from "esbuild";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const app = join(here, "apps", "desktop");
const bundle = await build({
  entryPoints: [join(app, "src", "sheetModel.ts")],
  bundle: true,
  format: "esm",
  platform: "neutral",
  write: false,
  logLevel: "silent",
});
const code = bundle.outputFiles[0].text;
const sheet = await import(`data:text/javascript;base64,${Buffer.from(code).toString("base64")}`);

const { evaluateWorkbook, keyOf, parseRef } = sheet;

function run(formula) {
  const cells = new Map();
  const parsed = parseRef("D1");
  cells.set(keyOf(parsed.row, parsed.col), { row: parsed.row, col: parsed.col, value: "", formula });
  const result = evaluateWorkbook({ sheets: [{ name: "Sheet1", cells, rows: 5, cols: 5 }] });
  return result.sheets[0].display.get(keyOf(parsed.row, parsed.col));
}

console.log("SEARCH plain:", run('=SEARCH("tro","tedrox")'));
console.log("SEARCH star :", run('=SEARCH("tro*","tedrox")'));
console.log("SEARCH quest:", run('=SEARCH("t?drox","tedrox")'));
console.log("FIND        :", run('=FIND("d","tedrox")'));
