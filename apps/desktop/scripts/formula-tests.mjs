#!/usr/bin/env node
/**
 * Formula engine tests.
 *
 * The engine is TypeScript, so it is bundled with the project's own esbuild and
 * imported as an ES module; no separate test runner is required.
 */
import { build } from "esbuild";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const app = join(here, "..");

const bundle = await build({
  entryPoints: [join(app, "src", "sheetModel.ts")],
  bundle: true,
  format: "esm",
  platform: "neutral",
  write: false,
  logLevel: "silent",
});

const code = bundle.outputFiles[0].text;
const module64 = Buffer.from(`${code}\n//# sourceURL=sheetModel.mjs`).toString("base64");
const sheet = await import(`data:text/javascript;base64,${module64}`);

const { evaluateWorkbook, keyOf } = sheet;

let passed = 0;
const failures = [];

function check(name, actual, expected) {
  if (actual === expected) {
    passed += 1;
  } else {
    failures.push(`${name}: expected ${JSON.stringify(expected)}, received ${JSON.stringify(actual)}`);
  }
}

/** Build a workbook from a list of [ref, value] pairs on a single sheet. */
function toCell(reference, value) {
  const parsed = sheet.parseRef(reference);
  const isFormula = typeof value === "string" && value.startsWith("=");
  return {
    key: keyOf(parsed.row, parsed.col),
    cell: {
      row: parsed.row,
      col: parsed.col,
      value: isFormula ? "" : String(value),
      formula: isFormula ? value : null,
    },
  };
}

function book(values, extraSheets = []) {
  const cells = new Map();
  for (const [reference, value] of Object.entries(values)) {
    const entry = toCell(reference, value);
    cells.set(entry.key, entry.cell);
  }
  const sheets = [{ name: "Sheet1", cells, rows: 20, cols: 12 }];
  for (const extra of extraSheets) {
    const other = new Map();
    for (const [reference, value] of Object.entries(extra.values)) {
      const entry = toCell(reference, value);
      other.set(entry.key, entry.cell);
    }
    sheets.push({ name: extra.name, cells: other, rows: 20, cols: 12 });
  }
  return { sheets };
}

function valueAt(values, reference, extraSheets) {
  const result = evaluateWorkbook(book(values, extraSheets));
  const parsed = sheet.parseRef(reference);
  return result.sheets[0].display.get(keyOf(parsed.row, parsed.col)) ?? "";
}

const grid = {
  A1: "10",
  A2: "20",
  A3: "30",
  A4: "40",
  B1: "1",
  B2: "2",
  B3: "3",
  C1: "apple",
  C2: "banana",
  C3: "apple",
};

check("SUM range", valueAt({ ...grid, D1: "=SUM(A1:A4)" }, "D1"), "100");
check("SUM with literal", valueAt({ ...grid, D1: "=SUM(A1,10,5)" }, "D1"), "25");
check("AVERAGE", valueAt({ ...grid, D1: "=AVERAGE(A1:A4)" }, "D1"), "25");
check("COUNT", valueAt({ ...grid, D1: "=COUNT(A1:A4)" }, "D1"), "4");
check("COUNTA", valueAt({ ...grid, D1: "=COUNTA(C1:C3)" }, "D1"), "3");
check("MIN/MAX", valueAt({ ...grid, D1: "=MAX(A1:A4)-MIN(A1:A4)" }, "D1"), "30");
check("MEDIAN", valueAt({ ...grid, D1: "=MEDIAN(A1:A4)" }, "D1"), "25");
check("ROUND", valueAt({ D1: "=ROUND(3.14159,2)" }, "D1"), "3.14");
check("ROUNDUP", valueAt({ D1: "=ROUNDUP(3.001,1)" }, "D1"), "3.1");
check("INT/MOD", valueAt({ D1: "=MOD(7,3)" }, "D1"), "1");
check("POWER", valueAt({ D1: "=POWER(2,10)" }, "D1"), "1024");
check("SQRT", valueAt({ D1: "=SQRT(144)" }, "D1"), "12");
check("Division by zero", valueAt({ D1: "=1/0" }, "D1"), "#DIV/0!");
check("IF true", valueAt({ ...grid, D1: "=IF(A1>5,\"big\",\"small\")" }, "D1"), "big");
check("IF false", valueAt({ ...grid, D1: "=IF(A1>50,\"big\",\"small\")" }, "D1"), "small");
check("IFS", valueAt({ ...grid, D1: "=IFS(A1>50,\"high\",A1>5,\"mid\",TRUE,\"low\")" }, "D1"), "mid");
check("AND/OR", valueAt({ ...grid, D1: "=AND(A1>5,OR(A2>100,FALSE))" }, "D1"), "FALSE");
check("IFERROR", valueAt({ D1: "=IFERROR(1/0,\"n/a\")" }, "D1"), "n/a");
check("COUNTIF", valueAt({ ...grid, D1: "=COUNTIF(C1:C3,\"apple\")" }, "D1"), "2");
check("COUNTIF operator", valueAt({ ...grid, D1: "=COUNTIF(A1:A4,\">15\")" }, "D1"), "3");
check("SUMIF", valueAt({ ...grid, D1: "=SUMIF(C1:C3,\"apple\",A1:A3)" }, "D1"), "40");
check("AVERAGEIF", valueAt({ ...grid, D1: "=AVERAGEIF(C1:C3,\"apple\",A1:A3)" }, "D1"), "20");
check("SUMPRODUCT", valueAt({ ...grid, D1: "=SUMPRODUCT(A1:A3,B1:B3)" }, "D1"), "140");
check("LEFT", valueAt({ D1: '=LEFT("tedrox",3)' }, "D1"), "ted");
check("RIGHT", valueAt({ D1: '=RIGHT("tedrox",3)' }, "D1"), "rox");
check("MID", valueAt({ D1: '=MID("tedrox",2,2)' }, "D1"), "ed");
check("LEN", valueAt({ D1: '=LEN("tedrox")' }, "D1"), "6");
check("UPPER/PROPER", valueAt({ D1: '=UPPER(PROPER("tedrox documents"))' }, "D1"), "TEDROX DOCUMENTS");
check("TRIM", valueAt({ D1: '=TRIM("  a   b  ")' }, "D1"), "a b");
check("CONCAT", valueAt({ D1: '=CONCAT("a","-",1)' }, "D1"), "a-1");
check("TEXTJOIN", valueAt({ D1: '=TEXTJOIN(",",TRUE,"a","","b")' }, "D1"), "a,b");
check("FIND", valueAt({ D1: '=FIND("d","tedrox")' }, "D1"), "3");
check("SEARCH wildcard", valueAt({ D1: '=SEARCH("e*rox","tedrox")' }, "D1"), "2");
check("SUBSTITUTE", valueAt({ D1: '=SUBSTITUTE("a-b-c","-","+")' }, "D1"), "a+b+c");
check("REPLACE", valueAt({ D1: '=REPLACE("tedrox",1,3,"TED")' }, "D1"), "TEDrox");
check("VALUE", valueAt({ D1: '=VALUE("42.5")*2' }, "D1"), "85");
check("TEXT number format", valueAt({ D1: '=TEXT(1234.5,"#,##0.00")' }, "D1"), "1,234.50");
check("VLOOKUP", valueAt({ ...grid, D1: "=VLOOKUP(\"banana\",C1:C3,1,FALSE)" }, "D1"), "banana");
check("XLOOKUP", valueAt({ ...grid, D1: "=XLOOKUP(\"banana\",C1:C3,A1:A3)" }, "D1"), "20");
check("INDEX/MATCH", valueAt({ ...grid, D1: "=INDEX(A1:A4,MATCH(\"banana\",C1:C3,0))" }, "D1"), "20");
check("ROWS/COLUMNS", valueAt({ ...grid, D1: "=ROWS(A1:A4)*COLUMNS(A1:B4)" }, "D1"), "8");
check("ROW/COLUMN", valueAt({ D1: "=ROW()+COLUMN()" }, "D1"), "5");
check("Cross-sheet reference", valueAt({ D1: "=Data!A1*2" }, "D1", [{ name: "Data", values: { A1: "21" } }]), "42");
check("Circular reference", valueAt({ A1: "=A1+1" }, "A1"), "#CYCLE!");
check("Unknown function", valueAt({ D1: "=NOPE(1)" }, "D1"), "#NAME?");
check("Percent literal", valueAt({ D1: "=50%*200" }, "D1"), "100");
check("Comparison operator", valueAt({ D1: "=3<>4" }, "D1"), "TRUE");

const formatted = evaluateWorkbook({
  sheets: [
    {
      name: "Sheet1",
      rows: 4,
      cols: 4,
      cells: new Map([
        [keyOf(0, 0), { row: 0, col: 0, value: "1234.5", format: "#,##0.00" }],
        [keyOf(1, 0), { row: 1, col: 0, value: "0.2567", format: "0.00%" }],
      ]),
    },
  ],
});
check("Number format grouping", formatted.sheets[0].display.get(keyOf(0, 0)), "1,234.50");
check("Percent format", formatted.sheets[0].display.get(keyOf(1, 0)), "25.67%");

if (failures.length > 0) {
  console.error(`Formula engine tests failed (${failures.length}):`);
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log(`Formula engine tests passed: ${passed} assertions.`);
