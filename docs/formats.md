# Format support and adapters

This document is the honest capability reference. If a format is not listed
here, treat it as unsupported until a release note says otherwise.

## Capability matrix

| Format | Open | Edit | Convert | Merge/Split | Batch | Engine |
| --- | --- | --- | --- | --- | --- | --- |
| PDF | Yes | Pages, annotations, forms | Yes | Yes | Yes | lopdf (MIT) + PDF.js rendering |
| DOCX | Yes | Yes (editor) | Yes | — | — | native OOXML |
| DOC (legacy) | Adapter | — | Adapter | — | — | LibreOffice |
| Markdown | Yes | Yes | Yes | — | — | native |
| TXT | Yes | Yes | Yes | — | — | native |
| HTML | Yes | — | Yes | — | — | text-first renderer |
| RTF | Detect | — | — | — | — | — |
| CSV / TSV | Yes | Yes | Yes | Split | Yes | csv (MIT) |
| XLSX | Yes | Yes (editor) | Yes | — | — | calamine + rust_xlsxwriter |
| XLS (legacy) | Adapter | — | Adapter | — | — | LibreOffice |
| ODS | Read | — | — | — | — | calamine |
| JSON | Yes | — | Yes | — | — | serde_json |
| PNG / JPEG / WebP / BMP / TIFF / GIF | Yes | Yes | Yes | — | Yes | image (MIT/Apache) |
| AVIF | Decode/Encode | — | Yes | — | Yes | image (ravif) |
| SVG | Render | — | Raster | — | — | resvg (MPL-2.0) |
| ICO | Create | — | — | — | — | image |
| ZIP | Yes | — | — | — | — | zip (MIT) |
| 7z / TAR / GZ | Detect | — | — | — | — | — |

## PDF editing

| Capability | State | Notes |
| --- | --- | --- |
| Reorder, rotate, delete, duplicate, reverse | Yes | One atomic page plan, re-opened and verified after writing |
| Merge, insert, split, extract, odd/even | Yes | Shared object-import routine with fresh object ids |
| Highlight, underline, strike out | Yes | `/Highlight`, `/Underline`, `/StrikeOut` with `/QuadPoints` |
| Ink, rectangle, ellipse, line, arrow | Yes | Generated appearance streams, honoured by other readers |
| Free text and comments | Yes | Base-14 Helvetica appearance; non-Latin text raises a warning |
| AcroForm reading and filling | Yes | Text, choice, check box and radio; `/NeedAppearances` is set |
| Compression presets | Yes | Screen, balanced, print and custom DPI/quality |
| Privacy clean | Yes | Info, XMP, JavaScript, embedded files and launch actions |
| Encryption and digital signatures | No | Not exposed in the interface |
| Redaction | No | Not implemented and deliberately not offered |

New annotations are appended to the existing `/Annots` array; annotations that
were already in the document are preserved.

## Spreadsheet model

| Capability | State | Notes |
| --- | --- | --- |
| Values and formulas | Yes | Formulas are re-evaluated on load and saved as formulas |
| Multiple sheets | Yes | Names, order and per-sheet cells |
| Bold, italic, underline, alignment | Yes | Round-trips through TEDROX-created workbooks |
| Text colour, fill colour | Yes | |
| Number formats | Yes | Excel format codes such as `#,##0.00`, `0.00%`, `yyyy-mm-dd` |
| Formatting written by other applications | Not read back | The editor probes `xl/styles.xml` and warns before overwriting |
| Charts | Rendered in the editor | Charts are drawn from the live sheet data, not stored in the file |

## PDF rendering adapter (PDFium)

`tdx-doc pdf to-images` and the desktop page preview need a PDFium build, which
is **not bundled**. PDFium is BSD-3-Clause; keeping it external avoids shipping
a large native binary with the MIT core.

To enable it:

1. Obtain a PDFium build for your platform (for example from the
   `bblanchon/pdfium-binaries` project).
2. Point the application at the library:

   ```powershell
   $env:TDX_PDFIUM_PATH = "C:\path\to\pdfium.dll"
   ```

   ```bash
   export TDX_PDFIUM_PATH=/usr/lib/libpdfium.so
   ```

3. Restart `tdx-doc`. Until then the command returns a clear
   `adapter_required` error.

## LibreOffice adapter (legacy Office)

Legacy binary `.doc` and `.xls` are not OOXML and are not parsed by the native
core. If LibreOffice is installed, `tdx-doc doc legacy-import` converts the
file in a temporary workspace and continues with the native engine.

```bash
tdx-doc doc legacy-import contract.doc -o contract.docx
```

Detection checks `soffice` on `PATH` and the usual install locations on
Windows, Linux and macOS. LibreOffice is MPL-2.0, is never bundled and stays
under the user's control.

## Compression presets

PDF compression re-encodes embedded raster images; text and vector content is
untouched.

| Preset | JPEG quality | Max dimension | Intended use |
| --- | --- | --- | --- |
| Screen | 55 | 1024 px | E-mail, quick preview |
| Balanced | 72 | 1600 px | Default |
| Print | 85 | 2400 px | Pre-press drafts |
| Custom | 20–100 | 256–10000 px | `--quality` / `--max-dimension` |

Transparency is flattened onto white when an image is converted to JPEG; the
result warns when that happens. Documents that cannot shrink report it instead
of pretending.

## Known limitations

- Document editor: text-level formatting (headings, fonts, bold/italic/
  underline, colour, alignment, lists). Tables, images and page breaks are on
  the roadmap; complex layouts from third-party DOCX files are flattened when
  exported to PDF.
- Spreadsheet editor: values, formulas created in the editor and cell
  bold/italic round-trip through XLSX. Formulas that already exist inside
  third-party files are shown as their cached values because the reader does
  not expose formula text.
- Text-first HTML rendering ignores CSS layout, images and scripts by design.
- Font subsetting is not implemented; documents that embed a system font grow
  by roughly 1–2 MB until it lands.
- 7z, TAR and GZ are detected but not processed.
- Redaction is intentionally absent until a verified pipeline exists.
