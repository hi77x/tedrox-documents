export type CellModel = {
  row: number;
  col: number;
  value: string;
  formula?: string | null;
  bold?: boolean;
  italic?: boolean;
};

export type SheetModel = {
  name: string;
  cells: CellModel[];
};

export type WorkbookModel = {
  sheets: SheetModel[];
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
  const match = reference.trim().toUpperCase().match(/^([A-Z]+)(\d+)$/);
  if (!match) return null;
  const letters = match[1];
  let col = 0;
  for (const char of letters) {
    col = col * 26 + (char.charCodeAt(0) - 64);
  }
  const row = Number(match[2]) - 1;
  if (row < 0) return null;
  return { row, col: col - 1 };
}

function toNumber(value: string): number | null {
  const trimmed = value.trim();
  if (!trimmed) return null;
  const number = Number(trimmed);
  return Number.isFinite(number) ? number : null;
}

type CellLookup = (reference: string) => string;

function tokenize(expression: string): string[] {
  const tokens: string[] = [];
  let index = 0;
  while (index < expression.length) {
    const char = expression[index];
    if (/\s/.test(char)) {
      index += 1;
      continue;
    }
    if (/[0-9.]/.test(char)) {
      let number = "";
      while (index < expression.length && /[0-9.]/.test(expression[index])) {
        number += expression[index];
        index += 1;
      }
      tokens.push(number);
      continue;
    }
    if (/[A-Za-z]/.test(char)) {
      let word = "";
      while (index < expression.length && /[A-Za-z0-9_]/.test(expression[index])) {
        word += expression[index];
        index += 1;
      }
      tokens.push(word.toUpperCase());
      continue;
    }
    tokens.push(char);
    index += 1;
  }
  return tokens;
}

class FormulaParser {
  private position = 0;

  constructor(
    private readonly tokens: string[],
    private readonly lookup: CellLookup,
  ) {}

  parse(): number {
    const value = this.expression();
    return value;
  }

  private peek(): string | undefined {
    return this.tokens[this.position];
  }

  private next(): string | undefined {
    const token = this.tokens[this.position];
    this.position += 1;
    return token;
  }

  private expression(): number {
    let value = this.term();
    while (this.peek() === "+" || this.peek() === "-") {
      const operator = this.next();
      const right = this.term();
      value = operator === "+" ? value + right : value - right;
    }
    return value;
  }

  private term(): number {
    let value = this.power();
    while (this.peek() === "*" || this.peek() === "/") {
      const operator = this.next();
      const right = this.power();
      value = operator === "*" ? value * right : right === 0 ? Number.NaN : value / right;
    }
    return value;
  }

  private power(): number {
    const base = this.unary();
    if (this.peek() === "^") {
      this.next();
      const exponent = this.power();
      return Math.pow(base, exponent);
    }
    return base;
  }

  private unary(): number {
    const token = this.peek();
    if (token === "-") {
      this.next();
      return -this.unary();
    }
    if (token === "+") {
      this.next();
      return this.unary();
    }
    return this.primary();
  }

  private primary(): number {
    const token = this.next();
    if (token === undefined) return Number.NaN;
    if (/^[0-9.]+$/.test(token)) return Number(token);
    if (token === "(") {
      const value = this.expression();
      if (this.peek() === ")") this.next();
      return value;
    }
    if (/^[A-Z]+\d+$/.test(token)) {
      const value = toNumber(this.lookup(token));
      return value ?? 0;
    }
    if (this.peek() === "(") {
      this.next();
      const args: number[] = [];
      if (this.peek() !== ")") {
        args.push(...this.argumentValues());
        while (this.peek() === ",") {
          this.next();
          args.push(...this.argumentValues());
        }
      }
      if (this.peek() === ")") this.next();
      return this.functionValue(token, args);
    }
    return Number.NaN;
  }

  private argumentValues(): number[] {
    const save = this.position;
    const token = this.next();
    if (token && /^[A-Z]+\d+$/.test(token) && this.peek() === ":") {
      this.next();
      const end = this.next();
      if (end && /^[A-Z]+\d+$/.test(end)) {
        return this.rangeValues(token, end);
      }
    }
    this.position = save;
    return [this.expression()];
  }

  private rangeValues(start: string, end: string): number[] {
    const from = parseRef(start);
    const to = parseRef(end);
    if (!from || !to) return [];
    const values: number[] = [];
    const rowStart = Math.min(from.row, to.row);
    const rowEnd = Math.max(from.row, to.row);
    const colStart = Math.min(from.col, to.col);
    const colEnd = Math.max(from.col, to.col);
    for (let row = rowStart; row <= rowEnd; row += 1) {
      for (let col = colStart; col <= colEnd; col += 1) {
        const value = toNumber(this.lookup(cellRef(row, col)));
        if (value !== null) values.push(value);
      }
    }
    return values;
  }

  private functionValue(name: string, args: number[]): number {
    switch (name) {
      case "SUM":
        return args.reduce((total, value) => total + value, 0);
      case "AVERAGE":
        return args.length ? args.reduce((total, value) => total + value, 0) / args.length : Number.NaN;
      case "MIN":
        return args.length ? Math.min(...args) : Number.NaN;
      case "MAX":
        return args.length ? Math.max(...args) : Number.NaN;
      case "COUNT":
        return args.length;
      case "ABS":
        return Math.abs(args[0] ?? Number.NaN);
      case "ROUND": {
        const digits = args[1] ?? 0;
        const factor = Math.pow(10, digits);
        return Math.round((args[0] ?? Number.NaN) * factor) / factor;
      }
      default:
        return Number.NaN;
    }
  }
}

export function formatNumber(value: number): string {
  if (!Number.isFinite(value)) return "#VALUE!";
  if (Number.isInteger(value)) return String(value);
  return String(Math.round(value * 1e10) / 1e10);
}

export function evaluateWorkbook(
  cells: Map<string, CellModel>,
): { display: Map<string, string>; error: Map<string, string> } {
  const display = new Map<string, string>();
  const error = new Map<string, string>();
  const visiting = new Set<string>();

  const lookup = (reference: string): string => {
    const parsed = parseRef(reference);
    if (!parsed) return "0";
    return displayValue(keyOf(parsed.row, parsed.col));
  };

  const displayValue = (key: string): string => {
    if (display.has(key)) return display.get(key) ?? "";
    const cell = cells.get(key);
    if (!cell) return "0";
    if (!cell.formula) {
      display.set(key, cell.value);
      return cell.value;
    }
    if (visiting.has(key)) {
      error.set(key, "#CYCLE!");
      return "#CYCLE!";
    }
    visiting.add(key);
    try {
      const expression = cell.formula.replace(/^=/, "");
      const parser = new FormulaParser(tokenize(expression), lookup);
      const value = parser.parse();
      if (Number.isNaN(value)) {
        error.set(key, "#ERROR!");
        display.set(key, "#ERROR!");
        return "#ERROR!";
      }
      const formatted = formatNumber(value);
      display.set(key, formatted);
      return formatted;
    } catch {
      error.set(key, "#ERROR!");
      display.set(key, "#ERROR!");
      return "#ERROR!";
    } finally {
      visiting.delete(key);
    }
  };

  for (const key of cells.keys()) displayValue(key);
  return { display, error };
}

export function workbookFromCells(
  cells: Map<string, CellModel>,
  name: string,
): WorkbookModel {
  const list: CellModel[] = [];
  for (const cell of cells.values()) {
    if (!cell.value && !cell.formula && !cell.bold && !cell.italic) continue;
    list.push({
      row: cell.row,
      col: cell.col,
      value: cell.value,
      formula: cell.formula ?? null,
      bold: Boolean(cell.bold),
      italic: Boolean(cell.italic),
    });
  }
  return { sheets: [{ name, cells: list }] };
}

export function cellsFromWorkbook(workbook: WorkbookModel): {
  sheets: { name: string; cells: Map<string, CellModel> }[];
} {
  return {
    sheets: (workbook.sheets ?? []).map((sheet) => {
      const cells = new Map<string, CellModel>();
      for (const cell of sheet.cells ?? []) {
        const key = keyOf(cell.row, cell.col);
        cells.set(key, {
          row: cell.row,
          col: cell.col,
          value: cell.value ?? "",
          formula: cell.formula ?? null,
          bold: Boolean(cell.bold),
          italic: Boolean(cell.italic),
        });
      }
      return { name: sheet.name || "Sheet1", cells };
    }),
  };
}
