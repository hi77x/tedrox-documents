import type { DocModel } from "../docModel";
import type { WorkbookModel } from "../sheetModel";

export const SAMPLE_DOCUMENT: DocModel = {
  title: "Annual Report 2026",
  blocks: [
    { kind: "heading1", align: null, runs: [{ text: "Annual Report 2026", bold: false, italic: false, underline: false, size: null, font: null, color: null }] },
    { kind: "paragraph", align: null, runs: [{ text: "Prepared by the TEDROX Documents team for the 2026 operating cycle.", bold: false, italic: true, underline: false, size: null, font: null, color: null }] },
    { kind: "heading2", align: null, runs: [{ text: "Overview", bold: false, italic: false, underline: false, size: null, font: null, color: null }] },
    {
      kind: "paragraph",
      align: null,
      runs: [
        { text: "The engine processed ", bold: false, italic: false, underline: false, size: null, font: null, color: null },
        { text: "4.2 million", bold: true, italic: false, underline: false, size: null, font: null, color: null },
        { text: " local operations — documents, spreadsheets and PDFs — without sending a single byte to a remote service.", bold: false, italic: false, underline: false, size: null, font: null, color: null },
      ],
    },
    { kind: "heading2", align: null, runs: [{ text: "Highlights", bold: false, italic: false, underline: false, size: null, font: null, color: null }] },
    { kind: "bullet", align: null, runs: [{ text: "Every release ships with checksums and reproducible builds.", bold: false, italic: false, underline: false, size: null, font: null, color: null }] },
    { kind: "bullet", align: null, runs: [{ text: "The CLI and the desktop application share one engine.", bold: false, italic: false, underline: false, size: null, font: null, color: null }] },
    { kind: "bullet", align: null, runs: [{ text: "Format support is described by automated tests, not by marketing copy.", bold: false, italic: false, underline: false, size: null, font: null, color: null }] },
    { kind: "heading2", align: null, runs: [{ text: "Regional results", bold: false, italic: false, underline: false, size: null, font: null, color: null }] },
    { kind: "numbered", align: null, runs: [{ text: "Europe — 1,840,000 operations", bold: false, italic: false, underline: false, size: null, font: null, color: null }] },
    { kind: "numbered", align: null, runs: [{ text: "Americas — 1,320,000 operations", bold: false, italic: false, underline: false, size: null, font: null, color: null }] },
    { kind: "numbered", align: null, runs: [{ text: "Asia-Pacific — 1,040,000 operations", bold: false, italic: false, underline: false, size: null, font: null, color: null }] },
    { kind: "quote", align: null, runs: [{ text: "Local-first is not a slogan here: it is the only mode of operation.", bold: false, italic: false, underline: false, size: null, font: null, color: null }] },
    { kind: "heading2", align: null, runs: [{ text: "Outlook", bold: false, italic: false, underline: false, size: null, font: null, color: null }] },
    { kind: "paragraph", align: null, runs: [{ text: "The next cycle focuses on deeper OOXML fidelity, a real paginated editor and portable releases for Linux and Android.", bold: false, italic: false, underline: false, size: null, font: null, color: null }] },
  ],
};

export const SAMPLE_WORKBOOK: WorkbookModel = {
  sheets: [
    {
      name: "Budget",
      cells: [
        { row: 0, col: 0, value: "Team", bold: true, italic: false, underline: false, align: null, color: null, fill: "E8EEFC", format: null },
        { row: 0, col: 1, value: "Revenue", bold: true, italic: false, underline: false, align: null, color: null, fill: "E8EEFC", format: null },
        { row: 0, col: 2, value: "Expenses", bold: true, italic: false, underline: false, align: null, color: null, fill: "E8EEFC", format: null },
        { row: 0, col: 3, value: "Margin", bold: true, italic: false, underline: false, align: null, color: null, fill: "E8EEFC", format: null },
        { row: 1, col: 0, value: "Europe", formula: null },
        { row: 1, col: 1, value: "412000", formula: null, format: "#,##0" },
        { row: 1, col: 2, value: "318000", formula: null, format: "#,##0" },
        { row: 1, col: 3, value: "", formula: "=B2-C2", format: "#,##0" },
        { row: 2, col: 0, value: "Americas", formula: null },
        { row: 2, col: 1, value: "388400", formula: null, format: "#,##0" },
        { row: 2, col: 2, value: "301200", formula: null, format: "#,##0" },
        { row: 2, col: 3, value: "", formula: "=B3-C3", format: "#,##0" },
        { row: 3, col: 0, value: "Asia-Pacific", formula: null },
        { row: 3, col: 1, value: "296800", formula: null, format: "#,##0" },
        { row: 3, col: 2, value: "244500", formula: null, format: "#,##0" },
        { row: 3, col: 3, value: "", formula: "=B4-C4", format: "#,##0" },
        { row: 4, col: 0, value: "Total", bold: true, italic: false, underline: false, align: null, color: null, fill: null, format: null },
        { row: 4, col: 1, value: "", formula: "=SUM(B2:B4)", bold: true, format: "#,##0" },
        { row: 4, col: 2, value: "", formula: "=SUM(C2:C4)", bold: true, format: "#,##0" },
        { row: 4, col: 3, value: "", formula: "=SUM(D2:D4)", bold: true, format: "#,##0" },
        { row: 6, col: 0, value: "Margin %", formula: null, italic: true },
        { row: 6, col: 1, value: "", formula: "=D5/B5", format: "0.00%" },
        { row: 7, col: 0, value: "Average revenue", formula: null, italic: true },
        { row: 7, col: 1, value: "", formula: "=AVERAGE(B2:B4)", format: "#,##0" },
        { row: 8, col: 0, value: "Best quarter", formula: null, italic: true },
        { row: 8, col: 1, value: "", formula: "=MAX(B2:B4)", format: "#,##0" },
      ],
    },
    { name: "Notes", cells: [{ row: 0, col: 0, value: "Figures are generated sample data." }] },
  ],
};

export const SAMPLE_PDF = "/samples/annual-report.pdf";
export const SAMPLE_PDF_SCAN = "/samples/scan-sample.pdf";
