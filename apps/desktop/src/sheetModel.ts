/**
 * Spreadsheet model and formula engine.
 *
 * The evaluator is a real tokenizer + recursive-descent parser over the Excel
 * grammar subset TEDROX Documents supports. Every function below is
 * implemented, not stubbed; the compatibility matrix lists the same set.
 */

export type CellAlign = "left" | "center" | "right" | "justify";

export type CellModel = {
  row: number;
  col: number;
  value: string;
  formula?: string | null;
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
  align?: CellAlign | null;
  color?: string | null;
  fill?: string | null;
  format?: string | null;
};

export type SheetModel = {
  name: string;
  cells: CellModel[];
};

export type WorkbookModel = {
  sheets: SheetModel[];
};

export type Sheet = {
  name: string;
  cells: Map<string, CellModel>;
  rows: number;
  cols: number;
};

export type Workbook = {
  sheets: Sheet[];
};

export const keyOf = (row: number, col: number) => `${row}:${col}`;

export function colName(col: number): string {
  let name = "";
  let value = col;
  while (value >= 0) {
    name = String.fromCharCode(65 + (value % 26)) + name;
    value = Math.floor(value / 26) - 1;
  }
  return name;
}

export function cellRef(row: number, col: number): string {
  return `${colName(col)}${row + 1}`;
}

export function parseRef(reference: string): { row: number; col: number } | null {
  const match = reference.trim().toUpperCase().match(/^\$?([A-Z]+)\$?(\d+)$/);
  if (!match) return null;
  let col = 0;
  for (const char of match[1]) col = col * 26 + (char.charCodeAt(0) - 64);
  const row = Number(match[2]) - 1;
  if (row < 0) return null;
  return { row, col: col - 1 };
}

/* ------------------------------------------------------------------ values */

export type Value =
  | { kind: "empty" }
  | { kind: "number"; value: number }
  | { kind: "text"; value: string }
  | { kind: "bool"; value: boolean }
  | { kind: "error"; code: string };

export const EMPTY: Value = { kind: "empty" };
export const num = (value: number): Value =>
  Number.isFinite(value) ? { kind: "number", value } : { kind: "error", code: "#NUM!" };
export const text = (value: string): Value => ({ kind: "text", value });
export const bool = (value: boolean): Value => ({ kind: "bool", value });
export const err = (code: string): Value => ({ kind: "error", code });

export function toNumber(value: Value): number | null {
  switch (value.kind) {
    case "number":
      return value.value;
    case "bool":
      return value.value ? 1 : 0;
    case "empty":
      return 0;
    case "text": {
      const trimmed = value.value.trim();
      if (trimmed === "") return 0;
      if (/^-?\d+(\.\d+)?([eE][+-]?\d+)?$/.test(trimmed)) return Number(trimmed);
      if (/^-?\d+(\.\d+)?%$/.test(trimmed)) return Number(trimmed.slice(0, -1)) / 100;
      return null;
    }
    default:
      return null;
  }
}

export function toText(value: Value): string {
  switch (value.kind) {
    case "empty":
      return "";
    case "text":
      return value.value;
    case "number":
      return formatGeneral(value.value);
    case "bool":
      return value.value ? "TRUE" : "FALSE";
    default:
      return value.code;
  }
}

export function formatGeneral(value: number): string {
  if (!Number.isFinite(value)) return "#NUM!";
  if (Number.isInteger(value) && Math.abs(value) < 1e15) return String(value);
  const rounded = Number(value.toPrecision(15));
  return String(rounded);
}

/* ---------------------------------------------------------------- tokenizer */

type Token =
  | { kind: "number"; value: number }
  | { kind: "string"; value: string }
  | { kind: "name"; value: string }
  | { kind: "ref"; sheet: string | null; row: number; col: number; abs: boolean }
  | { kind: "op"; value: string }
  | { kind: "paren"; value: "(" | ")" }
  | { kind: "comma" }
  | { kind: "colon" };

function tokenize(input: string): Token[] {
  const tokens: Token[] = [];
  let index = 0;
  while (index < input.length) {
    const char = input[index];
    if (/\s/.test(char)) {
      index += 1;
      continue;
    }
    if (char === '"') {
      let value = "";
      index += 1;
      while (index < input.length) {
        if (input[index] === '"') {
          if (input[index + 1] === '"') {
            value += '"';
            index += 2;
            continue;
          }
          index += 1;
          break;
        }
        value += input[index];
        index += 1;
      }
      tokens.push({ kind: "string", value });
      continue;
    }
    if (/[0-9]/.test(char) || (char === "." && /[0-9]/.test(input[index + 1] ?? ""))) {
      let value = "";
      while (index < input.length && /[0-9.]/.test(input[index])) {
        value += input[index];
        index += 1;
      }
      if (input[index] === "e" || input[index] === "E") {
        let exponent = input[index];
        let lookahead = index + 1;
        if (input[lookahead] === "+" || input[lookahead] === "-") {
          exponent += input[lookahead];
          lookahead += 1;
        }
        if (/[0-9]/.test(input[lookahead] ?? "")) {
          while (lookahead < input.length && /[0-9]/.test(input[lookahead])) {
            exponent += input[lookahead];
            lookahead += 1;
          }
          value += exponent;
          index = lookahead;
        }
      }
      tokens.push({ kind: "number", value: Number(value) });
      continue;
    }
    if (char === "'") {
      let name = "";
      index += 1;
      while (index < input.length) {
        if (input[index] === "'") {
          if (input[index + 1] === "'") {
            name += "'";
            index += 2;
            continue;
          }
          index += 1;
          break;
        }
        name += input[index];
        index += 1;
      }
      if (input[index] === "!") {
        index += 1;
        const ref = readRef(input, index);
        if (ref) {
          tokens.push({ kind: "ref", sheet: name, row: ref.row, col: ref.col, abs: ref.abs });
          index = ref.end;
          continue;
        }
      }
      tokens.push({ kind: "name", value: name });
      continue;
    }
    if (/[A-Za-z_$]/.test(char)) {
      const start = index;
      while (index < input.length && /[A-Za-z0-9_$.]/.test(input[index])) index += 1;
      const word = input.slice(start, index);
      if (input[index] === "!" && !word.startsWith("$")) {
        index += 1;
        const ref = readRef(input, index);
        if (ref) {
          tokens.push({ kind: "ref", sheet: word, row: ref.row, col: ref.col, abs: ref.abs });
          index = ref.end;
          continue;
        }
      }
      const ref = parseRef(word);
      if (ref) {
        tokens.push({ kind: "ref", sheet: null, row: ref.row, col: ref.col, abs: word.includes("$") });
        continue;
      }
      tokens.push({ kind: "name", value: word.toUpperCase() });
      continue;
    }
    if ("+-*/^&=<>".includes(char)) {
      const two = input.slice(index, index + 2);
      if (two === "<=" || two === ">=" || two === "<>") {
        tokens.push({ kind: "op", value: two });
        index += 2;
        continue;
      }
      tokens.push({ kind: "op", value: char });
      index += 1;
      continue;
    }
    if (char === "(" || char === ")") {
      tokens.push({ kind: "paren", value: char });
      index += 1;
      continue;
    }
    if (char === "," || char === ";") {
      tokens.push({ kind: "comma" });
      index += 1;
      continue;
    }
    if (char === ":") {
      tokens.push({ kind: "colon" });
      index += 1;
      continue;
    }
    if (char === "%") {
      tokens.push({ kind: "op", value: "%" });
      index += 1;
      continue;
    }
    index += 1;
  }
  return tokens;
}

function readRef(input: string, start: number): { row: number; col: number; abs: boolean; end: number } | null {
  const match = input.slice(start).match(/^(\$?)([A-Za-z]{1,3})(\$?)(\d{1,7})/);
  if (!match) return null;
  const reference = parseRef(`${match[1]}${match[2]}${match[3]}${match[4]}`);
  if (!reference) return null;
  return { ...reference, abs: match[1] === "$" || match[3] === "$", end: start + match[0].length };
}

/* ------------------------------------------------------------------ parser */

export type RangeArg = { kind: "range"; sheet: string | null; cells: Value[][] };
export type Arg = Value | RangeArg;

type Resolver = {
  cell: (sheet: string | null, row: number, col: number) => Value;
  sheetIndex: number;
  row: number;
  col: number;
};

class Parser {
  private position = 0;

  constructor(
    private readonly tokens: Token[],
    private readonly resolve: Resolver,
  ) {}

  parse(): Value {
    const value = this.expression();
    return value;
  }

  private peek(): Token | undefined {
    return this.tokens[this.position];
  }

  private next(): Token | undefined {
    const token = this.tokens[this.position];
    this.position += 1;
    return token;
  }

  private expression(): Value {
    return this.comparison();
  }

  private comparison(): Value {
    let left = this.concatenation();
    for (;;) {
      const token = this.peek();
      if (token?.kind !== "op") break;
      if (!["=", "<>", "<", ">", "<=", ">="].includes(token.value)) break;
      this.next();
      const right = this.concatenation();
      left = compareValues(left, right, token.value);
    }
    return left;
  }

  private concatenation(): Value {
    let left = this.additive();
    while (this.peek()?.kind === "op" && (this.peek() as { value: string }).value === "&") {
      this.next();
      const right = this.additive();
      if (left.kind === "error") continue;
      if (right.kind === "error") {
        left = right;
        continue;
      }
      left = text(toText(left) + toText(right));
    }
    return left;
  }

  private additive(): Value {
    let left = this.multiplicative();
    for (;;) {
      const token = this.peek();
      if (token?.kind !== "op" || (token.value !== "+" && token.value !== "-")) break;
      this.next();
      const right = this.multiplicative();
      const a = toNumber(left);
      const b = toNumber(right);
      if (left.kind === "error") continue;
      if (right.kind === "error") {
        left = right;
        continue;
      }
      if (a === null || b === null) {
        left = err("#VALUE!");
        continue;
      }
      left = num(token.value === "+" ? a + b : a - b);
    }
    return left;
  }

  private multiplicative(): Value {
    let left = this.power();
    for (;;) {
      const token = this.peek();
      if (token?.kind !== "op" || (token.value !== "*" && token.value !== "/")) break;
      this.next();
      const right = this.power();
      const a = toNumber(left);
      const b = toNumber(right);
      if (left.kind === "error") continue;
      if (right.kind === "error") {
        left = right;
        continue;
      }
      if (a === null || b === null) {
        left = err("#VALUE!");
        continue;
      }
      left = token.value === "*" ? num(a * b) : b === 0 ? err("#DIV/0!") : num(a / b);
    }
    return left;
  }

  private power(): Value {
    const base = this.unary();
    const token = this.peek();
    if (token?.kind === "op" && token.value === "^") {
      this.next();
      const exponent = this.power();
      const a = toNumber(base);
      const b = toNumber(exponent);
      if (a === null || b === null) return err("#VALUE!");
      return num(Math.pow(a, b));
    }
    return base;
  }

  private unary(): Value {
    const token = this.peek();
    if (token?.kind === "op" && (token.value === "-" || token.value === "+")) {
      this.next();
      const inner = this.unary();
      const value = toNumber(inner);
      if (value === null) return inner.kind === "error" ? inner : err("#VALUE!");
      return num(token.value === "-" ? -value : value);
    }
    return this.postfix();
  }

  private postfix(): Value {
    const value = this.primary();
    if (this.peek()?.kind === "op" && (this.peek() as { value: string }).value === "%") {
      this.next();
      const numeric = toNumber(value);
      return numeric === null ? err("#VALUE!") : num(numeric / 100);
    }
    return value;
  }

  private primary(): Value {
    const token = this.next();
    if (!token) return err("#SYNTAX!");
    switch (token.kind) {
      case "number":
        return num(token.value);
      case "string":
        return text(token.value);
      case "ref": {
        const start = token;
        if (this.peek()?.kind === "colon") {
          this.next();
          const endToken = this.next();
          if (endToken?.kind !== "ref") return err("#SYNTAX!");
          return this.range(start.sheet, start, endToken) as unknown as Value;
        }
        return this.resolve.cell(token.sheet, token.row, token.col);
      }
      case "name":
        return this.name(token.value);
      case "op":
        if (token.value === "-" || token.value === "+") {
          this.position -= 1;
          return this.unary();
        }
        return err("#SYNTAX!");
      case "paren":
        if (token.value === "(") {
          const value = this.expression();
          if (this.peek()?.kind === "paren" && (this.peek() as { value: string }).value === ")") this.next();
          return value;
        }
        return err("#SYNTAX!");
      default:
        return err("#SYNTAX!");
    }
  }

  private name(name: string): Value {
    if (name === "TRUE") return bool(true);
    if (name === "FALSE") return bool(false);
    if (this.peek()?.kind === "paren" && (this.peek() as { value: string }).value === "(") {
      this.next();
      const args = this.arguments();
      return callFunction(name, args, this.resolve);
    }
    return err("#NAME?");
  }

  private arguments(): Arg[] {
    const args: Arg[] = [];
    if (this.peek()?.kind === "paren" && (this.peek() as { value: string }).value === ")") {
      this.next();
      return args;
    }
    for (;;) {
      const value = this.expression() as Arg;
      args.push(value);
      const token = this.next();
      if (!token) break;
      if (token.kind === "comma") continue;
      if (token.kind === "paren" && token.value === ")") break;
      break;
    }
    return args;
  }

  private range(
    sheetName: string | null,
    start: { sheet: string | null; row: number; col: number },
    end: { sheet: string | null; row: number; col: number },
  ): RangeArg {
    const sheet = sheetName ?? start.sheet ?? end.sheet;
    const rowStart = Math.min(start.row, end.row);
    const rowEnd = Math.max(start.row, end.row);
    const colStart = Math.min(start.col, end.col);
    const colEnd = Math.max(start.col, end.col);
    const cells: Value[][] = [];
    for (let row = rowStart; row <= rowEnd; row += 1) {
      const line: Value[] = [];
      for (let col = colStart; col <= colEnd; col += 1) line.push(this.resolve.cell(sheet, row, col));
      cells.push(line);
    }
    return { kind: "range", sheet, cells };
  }
}

function compareValues(left: Value, right: Value, operator: string): Value {
  if (left.kind === "error") return left;
  if (right.kind === "error") return right;
  const a = left.kind === "text" ? left.value.toLowerCase() : left.kind === "empty" ? "" : toNumber(left);
  const b = right.kind === "text" ? right.value.toLowerCase() : right.kind === "empty" ? "" : toNumber(right);
  let comparison: number;
  if (typeof a === "number" && typeof b === "number") comparison = a === b ? 0 : a < b ? -1 : 1;
  else if (typeof a === "string" && typeof b === "string") comparison = a === b ? 0 : a < b ? -1 : 1;
  else comparison = typeof a === "number" ? -1 : 1;
  switch (operator) {
    case "=":
      return bool(comparison === 0);
    case "<>":
      return bool(comparison !== 0);
    case "<":
      return bool(comparison < 0);
    case ">":
      return bool(comparison > 0);
    case "<=":
      return bool(comparison <= 0);
    default:
      return bool(comparison >= 0);
  }
}

/* --------------------------------------------------------------- functions */

function flatten(args: Arg[]): Value[] {
  const out: Value[] = [];
  for (const arg of args) {
    if (arg.kind === "range") {
      for (const row of arg.cells) for (const cell of row) out.push(cell);
    } else {
      out.push(arg as Value);
    }
  }
  return out;
}

function numbers(args: Arg[]): number[] {
  const out: number[] = [];
  for (const value of flatten(args)) {
    const numeric = toNumber(value);
    if (numeric !== null && value.kind !== "empty") out.push(numeric);
  }
  return out;
}

function firstRange(args: Arg[]): RangeArg | null {
  for (const arg of args) if (arg.kind === "range") return arg;
  return null;
}

function criteriaMatcher(criteria: Value | RangeArg): (value: Value) => boolean {
  const target = criteria.kind === "range" ? criteria.cells[0]?.[0] ?? EMPTY : criteria;
  if (target.kind === "text") {
    const match = target.value.match(/^(<=|>=|<>|=|<|>)\s*(.*)$/);
    if (match) {
      const operator = match[1];
      const operand = match[2];
      const numeric = operand.trim() !== "" && !Number.isNaN(Number(operand)) ? Number(operand) : null;
      return (value: Value) => {
        const other = numeric === null ? toText(value).toLowerCase() : toNumber(value);
        if (other === null) return false;
        const reference = numeric === null ? operand.toLowerCase() : numeric;
        switch (operator) {
          case "=":
            return other === reference;
          case "<>":
            return other !== reference;
          case "<":
            return other < reference;
          case ">":
            return other > reference;
          case "<=":
            return other <= reference;
          default:
            return other >= reference;
        }
      };
    }
    const needle = target.value.toLowerCase();
    if (needle.includes("*") || needle.includes("?")) {
      const pattern = new RegExp(`^${needle.replace(/[.+^${}()|[\]\\]/g, "\\$&").replace(/\*/g, ".*").replace(/\?/g, ".")}$`);
      return (value: Value) => pattern.test(toText(value).toLowerCase());
    }
    return (value: Value) => toText(value).toLowerCase() === needle;
  }
  const wanted = target.kind === "number" ? target.value : toNumber(target);
  return (value: Value) => toNumber(value) === wanted;
}

function excelDateToSerial(date: Date): number {
  const epoch = Date.UTC(1899, 11, 30);
  return Math.floor((date.getTime() - epoch) / 86_400_000);
}

function serialToDate(serial: number): Date {
  const epoch = Date.UTC(1899, 11, 30);
  return new Date(epoch + Math.round(serial) * 86_400_000);
}

function todaySerial(): number {
  const now = new Date();
  return excelDateToSerial(new Date(Date.UTC(now.getFullYear(), now.getMonth(), now.getDate())));
}

export type FunctionContext = {
  resolve: Resolver;
  sheetName: (index: number) => string;
};

function callFunction(name: string, args: Arg[], resolve: Resolver): Value {
  const values = flatten(args);
  const valueArgs = values.filter((value) => value.kind !== "empty");
  const nums = numbers(args);
  const firstError = values.find((value) => value.kind === "error");
  const math = (compute: (list: number[]) => number | string): Value => {
    if (firstError && firstError.kind === "error") return firstError;
    const result = compute(nums);
    return typeof result === "string" ? err(result) : num(result);
  };

  switch (name) {
    case "SUM":
      return math((list) => list.reduce((total, value) => total + value, 0));
    case "PRODUCT":
      return math((list) => (list.length ? list.reduce((total, value) => total * value, 1) : 0));
    case "COUNT":
      return num(nums.length);
    case "COUNTA":
      return num(valueArgs.length);
    case "COUNTBLANK": {
      const range = firstRange(args);
      if (!range) return err("#VALUE!");
      let blanks = 0;
      for (const row of range.cells) for (const cell of row) if (cell.kind === "empty") blanks += 1;
      return num(blanks);
    }
    case "AVERAGE":
      return math((list) => (list.length ? list.reduce((total, value) => total + value, 0) / list.length : "#DIV/0!"));
    case "MIN":
      return math((list) => (list.length ? Math.min(...list) : 0));
    case "MAX":
      return math((list) => (list.length ? Math.max(...list) : 0));
    case "MEDIAN": {
      if (nums.length === 0) return err("#NUM!");
      const sorted = [...nums].sort((a, b) => a - b);
      const middle = Math.floor(sorted.length / 2);
      return num(sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2);
    }
    case "STDEV.S":
    case "STDEV": {
      if (nums.length < 2) return err("#DIV/0!");
      const mean = nums.reduce((total, value) => total + value, 0) / nums.length;
      const variance = nums.reduce((total, value) => total + (value - mean) ** 2, 0) / (nums.length - 1);
      return num(Math.sqrt(variance));
    }
    case "COUNTIF": {
      const range = firstRange(args);
      if (!range || args.length < 2) return err("#VALUE!");
      const matches = criteriaMatcher(args[1]);
      let count = 0;
      for (const row of range.cells) for (const cell of row) if (matches(cell)) count += 1;
      return num(count);
    }
    case "COUNTIFS": {
      const ranges = args.filter((arg) => arg.kind === "range") as RangeArg[];
      const criteria = args.filter((arg) => arg.kind !== "range") as Value[];
      for (const arg of args) if (arg.kind !== "range" && (arg as Value).kind === "error") return arg as Value;
      if (ranges.length === 0 || criteria.length < ranges.length) return err("#VALUE!");
      let count = 0;
      const height = ranges[0].cells.length;
      const width = ranges[0].cells[0]?.length ?? 0;
      for (let row = 0; row < height; row += 1) {
        for (let col = 0; col < width; col += 1) {
          let ok = true;
          for (let index = 0; index < ranges.length; index += 1) {
            const cell = ranges[index].cells[row]?.[col] ?? EMPTY;
            if (!criteriaMatcher(criteria[index])(cell)) {
              ok = false;
              break;
            }
          }
          if (ok) count += 1;
        }
      }
      return num(count);
    }
    case "SUMIF": {
      const range = firstRange(args);
      if (!range || args.length < 2) return err("#VALUE!");
      const matches = criteriaMatcher(args[1]);
      const sumRange = (args[2] && args[2].kind === "range" ? (args[2] as RangeArg) : range).cells;
      let total = 0;
      for (let row = 0; row < range.cells.length; row += 1) {
        for (let col = 0; col < (range.cells[row]?.length ?? 0); col += 1) {
          if (matches(range.cells[row][col])) {
            const numeric = toNumber(sumRange[row]?.[col] ?? EMPTY);
            if (numeric !== null) total += numeric;
          }
        }
      }
      return num(total);
    }
    case "SUMIFS": {
      const sumRange = firstRange(args);
      const ranges = args.filter((arg) => arg.kind === "range") as RangeArg[];
      const criteria = args.filter((arg) => arg.kind !== "range") as Value[];
      if (!sumRange || ranges.length < 2) return err("#VALUE!");
      const pairRanges = ranges.slice(1);
      let total = 0;
      for (let row = 0; row < sumRange.cells.length; row += 1) {
        for (let col = 0; col < (sumRange.cells[row]?.length ?? 0); col += 1) {
          let ok = true;
          for (let index = 0; index < pairRanges.length; index += 1) {
            if (!criteriaMatcher(criteria[index] ?? EMPTY)(pairRanges[index].cells[row]?.[col] ?? EMPTY)) {
              ok = false;
              break;
            }
          }
          if (ok) {
            const numeric = toNumber(sumRange.cells[row][col]);
            if (numeric !== null) total += numeric;
          }
        }
      }
      return num(total);
    }
    case "AVERAGEIF": {
      const range = firstRange(args);
      if (!range || args.length < 2) return err("#VALUE!");
      const matches = criteriaMatcher(args[1]);
      const sumRange = (args[2] && args[2].kind === "range" ? (args[2] as RangeArg) : range).cells;
      let total = 0;
      let count = 0;
      for (let row = 0; row < range.cells.length; row += 1) {
        for (let col = 0; col < (range.cells[row]?.length ?? 0); col += 1) {
          if (matches(range.cells[row][col])) {
            const numeric = toNumber(sumRange[row]?.[col] ?? EMPTY);
            if (numeric !== null) {
              total += numeric;
              count += 1;
            }
          }
        }
      }
      return count === 0 ? err("#DIV/0!") : num(total / count);
    }
    case "AVERAGEIFS": {
      const averageRange = firstRange(args);
      const ranges = args.filter((arg) => arg.kind === "range") as RangeArg[];
      const criteria = args.filter((arg) => arg.kind !== "range") as Value[];
      if (!averageRange || ranges.length < 2) return err("#VALUE!");
      let total = 0;
      let count = 0;
      for (let row = 0; row < averageRange.cells.length; row += 1) {
        for (let col = 0; col < (averageRange.cells[row]?.length ?? 0); col += 1) {
          let ok = true;
          for (let index = 0; index < ranges.length - 1; index += 1) {
            if (!criteriaMatcher(criteria[index] ?? EMPTY)(ranges[index + 1].cells[row]?.[col] ?? EMPTY)) {
              ok = false;
              break;
            }
          }
          if (ok) {
            const numeric = toNumber(averageRange.cells[row][col]);
            if (numeric !== null) {
              total += numeric;
              count += 1;
            }
          }
        }
      }
      return count === 0 ? err("#DIV/0!") : num(total / count);
    }

    case "IF": {
      if (args.length < 2) return err("#VALUE!");
      const condition = args[0];
      if (condition.kind === "error") return condition;
      const truthy = condition.kind === "bool" ? condition.value : (toNumber(condition as Value) ?? 0) !== 0;
      const branch = truthy ? args[1] : args[2] ?? bool(false);
      return branch.kind === "range" ? branch.cells[0]?.[0] ?? EMPTY : (branch as Value);
    }
    case "IFS": {
      for (let index = 0; index + 1 < args.length; index += 2) {
        const condition = args[index];
        if (condition.kind === "error") return condition;
        const truthy = condition.kind === "bool" ? condition.value : (toNumber(condition as Value) ?? 0) !== 0;
        if (truthy) {
          const branch = args[index + 1];
          return branch.kind === "range" ? branch.cells[0]?.[0] ?? EMPTY : (branch as Value);
        }
      }
      return err("#N/A");
    }
    case "AND":
    case "OR":
    case "XOR": {
      const flags = values.map((value) =>
        value.kind === "bool" ? value.value : (toNumber(value) ?? 0) !== 0,
      );
      if (firstError && firstError.kind === "error") return firstError;
      if (name === "AND") return bool(flags.every(Boolean));
      if (name === "OR") return bool(flags.some(Boolean));
      return bool(flags.filter(Boolean).length % 2 === 1);
    }
    case "NOT": {
      const first = values[0] ?? EMPTY;
      if (first.kind === "error") return first;
      const truthy = first.kind === "bool" ? first.value : (toNumber(first) ?? 0) !== 0;
      return bool(!truthy);
    }
    case "IFERROR": {
      const value = args[0];
      const resolved = value?.kind === "range" ? value.cells[0]?.[0] ?? EMPTY : (value as Value) ?? EMPTY;
      if (resolved.kind !== "error") return resolved;
      const fallback = args[1];
      return fallback?.kind === "range" ? fallback.cells[0]?.[0] ?? EMPTY : (fallback as Value) ?? EMPTY;
    }
    case "IFNA": {
      const value = args[0];
      const resolved = value?.kind === "range" ? value.cells[0]?.[0] ?? EMPTY : (value as Value) ?? EMPTY;
      if (resolved.kind !== "error" || resolved.code !== "#N/A") return resolved;
      const fallback = args[1];
      return fallback?.kind === "range" ? fallback.cells[0]?.[0] ?? EMPTY : (fallback as Value) ?? EMPTY;
    }
    case "SWITCH": {
      const subject = args[0];
      for (let index = 1; index + 1 < args.length; index += 2) {
        const candidate = args[index];
        if (candidate.kind === "range") continue;
        const comparison = compareValues(subject.kind === "range" ? subject.cells[0]?.[0] ?? EMPTY : subject, candidate, "=");
        if (comparison.kind === "bool" && comparison.value) {
          const branch = args[index + 1];
          return branch.kind === "range" ? branch.cells[0]?.[0] ?? EMPTY : (branch as Value);
        }
      }
      const fallbackIndex = args.length % 2 === 0 ? args.length - 1 : -1;
      if (fallbackIndex > 0) {
        const fallback = args[fallbackIndex];
        return fallback.kind === "range" ? fallback.cells[0]?.[0] ?? EMPTY : (fallback as Value);
      }
      return err("#N/A");
    }

    case "ABS":
      return math((list) => Math.abs(list[0] ?? 0));
    case "SIGN":
      return math((list) => Math.sign(list[0] ?? 0));
    case "ROUND":
      return math((list) => {
        const digits = list[1] ?? 0;
        const factor = Math.pow(10, digits);
        return Math.round((list[0] ?? 0) * factor) / factor;
      });
    case "ROUNDUP":
      return math((list) => {
        const digits = list[1] ?? 0;
        const factor = Math.pow(10, digits);
        const value = (list[0] ?? 0) * factor;
        return (value < 0 ? Math.floor(value) : Math.ceil(value)) / factor;
      });
    case "ROUNDDOWN":
      return math((list) => {
        const digits = list[1] ?? 0;
        const factor = Math.pow(10, digits);
        const value = (list[0] ?? 0) * factor;
        return (value < 0 ? Math.ceil(value) : Math.floor(value)) / factor;
      });
    case "INT":
      return math((list) => Math.floor(list[0] ?? 0));
    case "TRUNC":
      return math((list) => {
        const digits = list[1] ?? 0;
        const factor = Math.pow(10, digits);
        return Math.trunc((list[0] ?? 0) * factor) / factor;
      });
    case "MOD":
      return math((list) => (list[1] ? ((list[0] % list[1]) + list[1]) % list[1] : "#DIV/0!"));
    case "POWER":
      return math((list) => Math.pow(list[0] ?? 0, list[1] ?? 1));
    case "SQRT":
      return math((list) => (list[0] < 0 ? "#NUM!" : Math.sqrt(list[0] ?? 0)));
    case "EXP":
      return math((list) => Math.exp(list[0] ?? 0));
    case "LN":
      return math((list) => (list[0] <= 0 ? "#NUM!" : Math.log(list[0])));
    case "LOG10":
      return math((list) => (list[0] <= 0 ? "#NUM!" : Math.log10(list[0])));
    case "LOG":
      return math((list) => Math.log(list[0] ?? 0) / Math.log(list[1] ?? 10));
    case "PI":
      return num(Math.PI);
    case "CEILING":
    case "CEILING.MATH":
      return math((list) => {
        const step = Math.abs(list[1] ?? 1) || 1;
        return Math.ceil((list[0] ?? 0) / step) * step;
      });
    case "FLOOR":
    case "FLOOR.MATH":
      return math((list) => {
        const step = Math.abs(list[1] ?? 1) || 1;
        return Math.floor((list[0] ?? 0) / step) * step;
      });

    case "LEFT": {
      const source = toText(values[0] ?? EMPTY);
      const count = values[1] ? (toNumber(values[1]) ?? 1) : 1;
      return text(source.slice(0, Math.max(0, count)));
    }
    case "RIGHT": {
      const source = toText(values[0] ?? EMPTY);
      const count = values[1] ? (toNumber(values[1]) ?? 1) : 1;
      return text(count <= 0 ? "" : source.slice(-count));
    }
    case "MID": {
      const source = toText(values[0] ?? EMPTY);
      const start = toNumber(values[1] ?? EMPTY) ?? 1;
      const count = toNumber(values[2] ?? EMPTY) ?? 0;
      if (start < 1 || count < 0) return err("#VALUE!");
      return text(source.slice(start - 1, start - 1 + count));
    }
    case "LEN":
      return num(toText(values[0] ?? EMPTY).length);
    case "TRIM":
      return text(toText(values[0] ?? EMPTY).trim().replace(/\s+/g, " "));
    case "CLEAN":
      return text(toText(values[0] ?? EMPTY).replace(/[\u0000-\u001f]/g, ""));
    case "LOWER":
      return text(toText(values[0] ?? EMPTY).toLowerCase());
    case "UPPER":
      return text(toText(values[0] ?? EMPTY).toUpperCase());
    case "PROPER":
      return text(
        toText(values[0] ?? EMPTY).replace(/\w\S*/g, (word) => word[0].toUpperCase() + word.slice(1).toLowerCase()),
      );
    case "CONCAT":
    case "CONCATENATE":
      return text(values.map(toText).join(""));
    case "TEXTJOIN": {
      const delimiter = toText(values[0] ?? EMPTY);
      const ignoreEmpty = values[1]?.kind === "bool" ? (values[1] as { value: boolean }).value : true;
      const parts = values.slice(2).map(toText);
      return text((ignoreEmpty ? parts.filter((part) => part !== "") : parts).join(delimiter));
    }
    case "FIND": {
      const needle = toText(values[0] ?? EMPTY);
      const haystack = toText(values[1] ?? EMPTY);
      const start = values[2] ? (toNumber(values[2]) ?? 1) : 1;
      const index = haystack.indexOf(needle, start - 1);
      return index === -1 ? err("#VALUE!") : num(index + 1);
    }
    case "SEARCH": {
      const needle = toText(values[0] ?? EMPTY).toLowerCase();
      const haystack = toText(values[1] ?? EMPTY).toLowerCase();
      const start = values[2] ? (toNumber(values[2]) ?? 1) : 1;
      if (needle.includes("*") || needle.includes("?")) {
        const pattern = new RegExp(
          needle.replace(/[.+^${}()|[\]\\]/g, "\\$&").replace(/\*/g, ".*").replace(/\?/g, "."),
        );
        const slice = haystack.slice(start - 1);
        const match = slice.match(pattern);
        return match && match.index !== undefined ? num(start + match.index) : err("#VALUE!");
      }
      const index = haystack.indexOf(needle, start - 1);
      return index === -1 ? err("#VALUE!") : num(index + 1);
    }
    case "SUBSTITUTE": {
      const source = toText(values[0] ?? EMPTY);
      const search = toText(values[1] ?? EMPTY);
      const replacement = toText(values[2] ?? EMPTY);
      if (search === "") return text(source);
      return text(source.split(search).join(replacement));
    }
    case "REPLACE": {
      const source = toText(values[0] ?? EMPTY);
      const start = toNumber(values[1] ?? EMPTY) ?? 1;
      const count = toNumber(values[2] ?? EMPTY) ?? 0;
      const replacement = toText(values[3] ?? EMPTY);
      return text(source.slice(0, start - 1) + replacement + source.slice(start - 1 + count));
    }
    case "REPT": {
      const source = toText(values[0] ?? EMPTY);
      const count = Math.max(0, Math.floor(toNumber(values[1] ?? EMPTY) ?? 0));
      return text(source.repeat(Math.min(count, 4096)));
    }
    case "VALUE": {
      const numeric = toNumber(values[0] ?? EMPTY);
      return numeric === null ? err("#VALUE!") : num(numeric);
    }
    case "TEXT": {
      const source = values[0] ?? EMPTY;
      const pattern = toText(values[1] ?? EMPTY);
      const numeric = toNumber(source);
      if (numeric === null) return text(toText(source));
      return text(applyNumberFormat(numeric, pattern));
    }

    case "DATE": {
      const year = toNumber(values[0] ?? EMPTY) ?? 1900;
      const month = (toNumber(values[1] ?? EMPTY) ?? 1) - 1;
      const day = toNumber(values[2] ?? EMPTY) ?? 1;
      const date = new Date(Date.UTC(year < 1900 ? 1900 + year : year, month, day));
      return num(excelDateToSerial(date));
    }
    case "TIME": {
      const hours = toNumber(values[0] ?? EMPTY) ?? 0;
      const minutes = toNumber(values[1] ?? EMPTY) ?? 0;
      const seconds = toNumber(values[2] ?? EMPTY) ?? 0;
      return num((hours * 3600 + minutes * 60 + seconds) / 86_400);
    }
    case "TODAY":
      return num(todaySerial());
    case "NOW": {
      const now = new Date();
      const serial = excelDateToSerial(new Date(Date.UTC(now.getFullYear(), now.getMonth(), now.getDate())));
      const fraction = (now.getHours() * 3600 + now.getMinutes() * 60 + now.getSeconds()) / 86_400;
      return num(serial + fraction);
    }
    case "YEAR":
      return num(serialToDate(toNumber(values[0] ?? EMPTY) ?? 0).getUTCFullYear());
    case "MONTH":
      return num(serialToDate(toNumber(values[0] ?? EMPTY) ?? 0).getUTCMonth() + 1);
    case "DAY":
      return num(serialToDate(toNumber(values[0] ?? EMPTY) ?? 0).getUTCDate());
    case "HOUR": {
      const fraction = toNumber(values[0] ?? EMPTY) ?? 0;
      return num(Math.floor((fraction % 1) * 24));
    }
    case "MINUTE": {
      const fraction = toNumber(values[0] ?? EMPTY) ?? 0;
      return num(Math.floor(((fraction % 1) * 1440) % 60));
    }
    case "WEEKDAY": {
      const serial = toNumber(values[0] ?? EMPTY) ?? 0;
      const type = toNumber(values[1] ?? EMPTY) ?? 1;
      const day = serialToDate(serial).getUTCDay();
      if (type === 2) return num(day === 0 ? 7 : day);
      if (type === 3) return num(day === 0 ? 6 : day - 1);
      return num(day + 1);
    }
    case "DAYS": {
      const end = toNumber(values[0] ?? EMPTY) ?? 0;
      const start = toNumber(values[1] ?? EMPTY) ?? 0;
      return num(Math.round(end - start));
    }
    case "EDATE": {
      const serial = toNumber(values[0] ?? EMPTY) ?? 0;
      const months = toNumber(values[1] ?? EMPTY) ?? 0;
      const date = serialToDate(serial);
      const next = new Date(Date.UTC(date.getUTCFullYear(), date.getUTCMonth() + months, date.getUTCDate()));
      return num(excelDateToSerial(next));
    }

    case "ROW":
      return num(resolve.row + 1);
    case "COLUMN":
      return num(resolve.col + 1);
    case "ROWS": {
      const range = firstRange(args);
      return num(range ? range.cells.length : 1);
    }
    case "COLUMNS": {
      const range = firstRange(args);
      return num(range ? range.cells[0]?.length ?? 1 : 1);
    }
    case "INDEX": {
      const range = firstRange(args);
      if (!range) return err("#REF!");
      const rowIndex = toNumber((args.find((arg) => arg.kind !== "range") as Value) ?? EMPTY) ?? 1;
      const colArg = args.filter((arg) => arg.kind !== "range")[1] as Value | undefined;
      const colIndex = colArg ? toNumber(colArg) ?? 1 : range.cells[0]?.length === 1 ? 1 : 1;
      if (range.cells.length === 1 && !colArg) {
        const cell = range.cells[0][Math.floor(rowIndex) - 1];
        return cell ?? err("#REF!");
      }
      const row = range.cells[Math.floor(rowIndex) - 1];
      if (!row) return err("#REF!");
      const cell = row[Math.floor(colIndex) - 1];
      return cell ?? err("#REF!");
    }
    case "MATCH": {
      const needle = values[0] ?? EMPTY;
      const range = firstRange(args);
      if (!range) return err("#N/A");
      const matchType = toNumber(values[values.length - 1] ?? EMPTY) ?? 1;
      const line = range.cells[0];
      const column = range.cells.map((row) => row[0]);
      const list = line.length >= column.length ? line : column;
      for (let index = 0; index < list.length; index += 1) {
        const comparison = compareValues(list[index], needle, "=");
        if (comparison.kind === "bool" && comparison.value) return num(index + 1);
      }
      if (matchType === 1) {
        let best = -1;
        for (let index = 0; index < list.length; index += 1) {
          const cell = toNumber(list[index]);
          const target = toNumber(needle);
          if (cell !== null && target !== null && cell <= target) best = index;
        }
        return best >= 0 ? num(best + 1) : err("#N/A");
      }
      return err("#N/A");
    }
    case "VLOOKUP": {
      const needle = values[0] ?? EMPTY;
      const range = firstRange(args);
      if (!range || args.length < 3) return err("#N/A");
      const columnIndex = Math.floor(toNumber(args[1] as Value) ?? 1);
      for (const row of range.cells) {
        const comparison = compareValues(row[0] ?? EMPTY, needle, "=");
        if (comparison.kind === "bool" && comparison.value) {
          return row[columnIndex - 1] ?? err("#REF!");
        }
      }
      return err("#N/A");
    }
    case "HLOOKUP": {
      const needle = values[0] ?? EMPTY;
      const range = firstRange(args);
      if (!range || args.length < 3) return err("#N/A");
      const rowIndex = Math.floor(toNumber(args[1] as Value) ?? 1);
      const header = range.cells[0] ?? [];
      for (let index = 0; index < header.length; index += 1) {
        const comparison = compareValues(header[index], needle, "=");
        if (comparison.kind === "bool" && comparison.value) {
          return range.cells[rowIndex - 1]?.[index] ?? err("#REF!");
        }
      }
      return err("#N/A");
    }
    case "XLOOKUP": {
      const needle = values[0] ?? EMPTY;
      const lookupRange = args[1]?.kind === "range" ? (args[1] as RangeArg) : null;
      const returnRange = args[2]?.kind === "range" ? (args[2] as RangeArg) : null;
      if (!lookupRange || !returnRange) return err("#VALUE!");
      const lookupColumn = lookupRange.cells.map((row) => row[0]);
      const returnsColumn = returnRange.cells.map((row) => row[0]);
      for (let index = 0; index < lookupColumn.length; index += 1) {
        const comparison = compareValues(lookupColumn[index], needle, "=");
        if (comparison.kind === "bool" && comparison.value) {
          return returnsColumn[index] ?? err("#REF!");
        }
      }
      const fallback = args[3];
      return fallback && fallback.kind !== "range" ? (fallback as Value) : err("#N/A");
    }
    case "OFFSET": {
      const range = firstRange(args);
      if (!range) return err("#VALUE!");
      const rowOffset = toNumber(values[0] ?? EMPTY) ?? 0;
      const colOffset = values[1] ? toNumber(values[1]) ?? 0 : 0;
      const cell = range.cells[Math.max(0, rowOffset)]?.[Math.max(0, colOffset)];
      return cell ?? err("#REF!");
    }
    case "SUMX2MY2": {
      const a = args.filter((arg) => arg.kind === "range") as RangeArg[];
      if (a.length < 2) return err("#VALUE!");
      const left = a[0].cells.flat().map((value) => toNumber(value) ?? 0);
      const right = a[1].cells.flat().map((value) => toNumber(value) ?? 0);
      let total = 0;
      for (let index = 0; index < Math.min(left.length, right.length); index += 1) {
        total += left[index] ** 2 - right[index] ** 2;
      }
      return num(total);
    }
    case "SUMPRODUCT": {
      const ranges = args.filter((arg) => arg.kind === "range") as RangeArg[];
      if (ranges.length === 0) return err("#VALUE!");
      const lists = ranges.map((range) => range.cells.flat().map((value) => toNumber(value) ?? 0));
      const length = Math.min(...lists.map((list) => list.length));
      let total = 0;
      for (let index = 0; index < length; index += 1) {
        total += lists.reduce((product, list) => product * list[index], 1);
      }
      return num(total);
    }
    default:
      return err("#NAME?");
  }
}

/* ------------------------------------------------------------ number format */

export type FormatKind = "general" | "number" | "percent" | "currency" | "date" | "time" | "text" | "scientific";

export function detectFormatKind(pattern: string | null | undefined): FormatKind {
  if (!pattern) return "general";
  const code = pattern.toLowerCase();
  if (code.includes("%")) return "percent";
  if (/[ymd]{2,}/.test(code) && !code.includes("0.0")) return "date";
  if (/h{1,2}:?m{1,2}/.test(code)) return "time";
  if (/^@$/.test(code)) return "text";
  if (code.includes("e+") || code.includes("e-")) return "scientific";
  if (/[$€£¥]/.test(pattern)) return "currency";
  return "number";
}

export function applyNumberFormat(value: number, pattern: string | null | undefined): string {
  if (!Number.isFinite(value)) return "#NUM!";
  if (!pattern || pattern === "General") return formatGeneral(value);
  const kind = detectFormatKind(pattern);
  switch (kind) {
    case "percent": {
      const decimals = (pattern.split(".")[1] ?? "").replace(/[^0#]/g, "").length;
      return `${addThousands((value * 100).toFixed(decimals))}%`;
    }
    case "currency": {
      const symbol = pattern.match(/[$€£¥]/)?.[0] ?? "";
      const decimals = (pattern.split(".")[1] ?? "").replace(/[^0#]/g, "").length;
      const negative = value < 0;
      const body = addThousands(Math.abs(value).toFixed(decimals));
      if (pattern.includes("(") ) return `${negative ? "(" : ""}${symbol}${body}${negative ? ")" : ""}`;
      return `${negative ? "-" : ""}${symbol}${body}`;
    }
    case "date": {
      return formatDate(serialToDate(value), pattern);
    }
    case "time": {
      const totalSeconds = Math.round((value % 1) * 86400);
      const hours = Math.floor(totalSeconds / 3600);
      const minutes = Math.floor((totalSeconds % 3600) / 60);
      const seconds = totalSeconds % 60;
      if (/h{1,2}:mm:ss/i.test(pattern)) {
        return `${String(hours).padStart(2, "0")}:${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
      }
      const hour12 = pattern.includes("AM/PM") || pattern.includes("am/pm");
      const display = hour12 ? ((hours + 11) % 12) + 1 : hours;
      const suffix = hour12 ? (hours < 12 ? " AM" : " PM") : "";
      return `${hour12 ? display : String(hours).padStart(2, "0")}:${String(minutes).padStart(2, "0")}${suffix}`;
    }
    case "scientific":
      return value.toExponential((pattern.split(".")[1] ?? "").replace(/[^0#]/g, "").length);
    case "text":
      return formatGeneral(value);
    default: {
      const decimals = (pattern.split(".")[1] ?? "").replace(/[^0#]/g, "").length;
      const grouped = pattern.includes(",");
      const body = decimals > 0 ? value.toFixed(decimals) : String(Math.round(value));
      return grouped ? addThousands(body) : body;
    }
  }
}

function addThousands(value: string): string {
  const [whole, fraction] = value.split(".");
  const sign = whole.startsWith("-") ? "-" : "";
  const digits = sign ? whole.slice(1) : whole;
  const grouped = digits.replace(/\B(?=(\d{3})+(?!\d))/g, ",");
  return `${sign}${grouped}${fraction ? `.${fraction}` : ""}`;
}

function formatDate(date: Date, pattern: string): string {
  const monthsShort = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
  const monthsLong = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
  const day = String(date.getUTCDate()).padStart(2, "0");
  const month = String(date.getUTCMonth() + 1).padStart(2, "0");
  const year = String(date.getUTCFullYear());
  if (/mm\/dd\/yyyy/i.test(pattern)) return `${month}/${day}/${year}`;
  if (/dd\.mm\.yyyy/i.test(pattern)) return `${day}.${month}.${year}`;
  if (/dd\/mm\/yyyy/i.test(pattern)) return `${day}/${month}/${year}`;
  if (/yyyy-mm-dd/i.test(pattern)) return `${year}-${month}-${day}`;
  if (/d\s+mmm\s+yyyy/i.test(pattern)) return `${date.getUTCDate()} ${monthsShort[date.getUTCMonth()]} ${year}`;
  if (/mmm\s+d,?\s+yyyy/i.test(pattern)) return `${monthsShort[date.getUTCMonth()]} ${date.getUTCDate()}, ${year}`;
  if (/mmmm/i.test(pattern)) return `${monthsLong[date.getUTCMonth()]} ${date.getUTCDate()}, ${year}`;
  return `${year}-${month}-${day}`;
}

/* -------------------------------------------------------------- evaluation */

export type SheetResult = {
  values: Map<string, Value>;
  display: Map<string, string>;
};

export type WorkbookResult = {
  sheets: SheetResult[];
};

export function emptyWorkbook(): Workbook {
  return { sheets: [{ name: "Sheet1", cells: new Map(), rows: 0, cols: 0 }] };
}

export function workbookFromModel(model: WorkbookModel): Workbook {
  const sheets = (model.sheets ?? []).map((sheet) => {
    const cells = new Map<string, CellModel>();
    let rows = 0;
    let cols = 0;
    for (const cell of sheet.cells ?? []) {
      cells.set(keyOf(cell.row, cell.col), {
        row: cell.row,
        col: cell.col,
        value: cell.value ?? "",
        formula: cell.formula ?? null,
        bold: Boolean(cell.bold),
        italic: Boolean(cell.italic),
        underline: Boolean(cell.underline),
        align: (cell.align as CellAlign) ?? null,
        color: cell.color ?? null,
        fill: cell.fill ?? null,
        format: cell.format ?? null,
      });
      rows = Math.max(rows, cell.row);
      cols = Math.max(cols, cell.col);
    }
    return { name: sheet.name || "Sheet1", cells, rows: rows + 1, cols: cols + 1 };
  });
  return { sheets: sheets.length ? sheets : emptyWorkbook().sheets };
}

export function modelFromWorkbook(workbook: Workbook): WorkbookModel {
  return {
    sheets: workbook.sheets.map((sheet) => ({
      name: sheet.name,
      cells: [...sheet.cells.values()].filter(
        (cell) =>
          cell.value !== "" ||
          cell.formula ||
          cell.bold ||
          cell.italic ||
          cell.underline ||
          cell.align ||
          cell.color ||
          cell.fill ||
          cell.format,
      ),
    })),
  };
}

export function evaluateWorkbook(workbook: Workbook): WorkbookResult {
  const memo: Map<string, Value>[] = workbook.sheets.map(() => new Map());
  const visiting: Set<string>[] = workbook.sheets.map(() => new Set());
  const results: SheetResult[] = workbook.sheets.map(() => ({
    values: new Map(),
    display: new Map(),
  }));

  const sheetNames = workbook.sheets.map((sheet) => sheet.name.toLowerCase());

  const valueAt = (sheetIndex: number, row: number, col: number): Value => {
    const sheet = workbook.sheets[sheetIndex];
    if (!sheet) return err("#REF!");
    const key = keyOf(row, col);
    const cached = memo[sheetIndex].get(key);
    if (cached) return cached;
    const cell = sheet.cells.get(key);
    if (!cell) {
      memo[sheetIndex].set(key, EMPTY);
      return EMPTY;
    }
    if (visiting[sheetIndex].has(key)) return err("#CYCLE!");
    if (!cell.formula) {
      const value = literalValue(cell.value);
      memo[sheetIndex].set(key, value);
      results[sheetIndex].values.set(key, value);
      results[sheetIndex].display.set(key, displayValue(cell, value));
      return value;
    }
    visiting[sheetIndex].add(key);
    const expression = cell.formula.replace(/^=/, "");
    let value: Value;
    try {
      const resolver: Resolver = {
        cell: (sheetName, rowIndex, colIndex) => {
          if (sheetName === null) return valueAt(sheetIndex, rowIndex, colIndex);
          const target = sheetNames.indexOf(sheetName.replace(/^'|'$/g, "").toLowerCase());
          if (target === -1) return err("#REF!");
          return valueAt(target, rowIndex, colIndex);
        },
        sheetIndex,
        row,
        col,
      };
      const parser = new Parser(tokenize(expression), resolver);
      value = parser.parse();
      if (value && (value as unknown as RangeArg).kind === "range") {
        value = (value as unknown as RangeArg).cells[0]?.[0] ?? EMPTY;
      }
    } catch {
      value = err("#ERROR!");
    }
    visiting[sheetIndex].delete(key);
    memo[sheetIndex].set(key, value);
    results[sheetIndex].values.set(key, value);
    results[sheetIndex].display.set(key, displayValue(cell, value));
    return value;
  };

  for (let sheetIndex = 0; sheetIndex < workbook.sheets.length; sheetIndex += 1) {
    for (const cell of workbook.sheets[sheetIndex].cells.values()) {
      valueAt(sheetIndex, cell.row, cell.col);
    }
  }

  return { sheets: results };
}

function literalValue(raw: string): Value {
  const trimmed = raw.trim();
  if (trimmed === "") return EMPTY;
  if (trimmed === "TRUE") return bool(true);
  if (trimmed === "FALSE") return bool(false);
  if (/^-?\d+(\.\d+)?([eE][+-]?\d+)?$/.test(trimmed)) return num(Number(trimmed));
  if (/^-?\d+(\.\d+)?%$/.test(trimmed)) return num(Number(trimmed.slice(0, -1)) / 100);
  if (/^-?\d{1,3}(,\d{3})+(\.\d+)?$/.test(trimmed)) return num(Number(trimmed.replace(/,/g, "")));
  return text(raw);
}

export function displayValue(cell: CellModel, value: Value): string {
  if (value.kind === "error") return value.code;
  if (value.kind === "empty") return "";
  if (value.kind === "number") {
    if (cell.format) return applyNumberFormat(value.value, cell.format);
    return formatGeneral(value.value);
  }
  return toText(value);
}

export function guessFormat(pattern: string): string | null {
  const trimmed = pattern.trim();
  if (!trimmed) return null;
  if (/^\d+(\.\d+)?%$/.test(trimmed)) return "0.00%";
  if (/^\d{4}-\d{2}-\d{2}$/.test(trimmed)) return "yyyy-mm-dd";
  if (/^\d{1,2}\/\d{1,2}\/\d{4}$/.test(trimmed)) return "dd/mm/yyyy";
  if (/\d,\d{3}/.test(trimmed) && trimmed.includes(".")) return "#,##0.00";
  if (/\d,\d{3}/.test(trimmed)) return "#,##0";
  if (/[$€£¥]\s?\d/.test(trimmed)) return "$#,##0.00";
  return null;
}
