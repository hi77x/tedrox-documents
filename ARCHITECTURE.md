# Architecture

TEDROX Documents is a Rust workspace with a strict separation between the
document engine and its shells. Nothing in the engine knows about a UI.

```text
apps/
  desktop/        Tauri 2 shell (in development)
  android/        Kotlin shell over the same core (planned)
  web/            Static bilingual landing page
crates/
  tdx-core/       File detection, capability registry, errors, atomic IO, progress
  tdx-jobs/       Job runtime: queue, lifecycle, cancellation, sanitized history
  tdx-pdf/        PDF page operations, metadata, compression, watermark, images→PDF
  tdx-image/      Bitmap conversion, geometry, SVG rasterization, tracing
  tdx-sheet/      CSV workbench and XLSX/XLS/ODS reading/writing
  tdx-docx/       Native OOXML writer and text/Markdown reader
  tdx-archive/    ZIP creation and safe extraction
  tdx-convert/    Conversion routing (Markdown/HTML/DOCX → PDF, auto-dispatch)
  tdx-cli/        tdx-doc command line interface
packages/
  i18n/           Canonical EN/RU dictionaries
```

## Core boundary

Every user-facing operation is described once in `tdx_core::ops`:

```rust
OperationDescriptor {
    id, name, category, summary,
    accepted, outputs,
    cost, streaming, cancellable, platforms, experimental,
}
```

The registry is the single source of truth for menus, the conversion center,
the command palette and `tdx-doc tools`. Unsupported conversions are absent
from the registry rather than failing at runtime.

## Job runtime

Long operations run through `tdx_jobs::JobEngine`:

- bounded concurrency (`JobEngine::new(max_concurrent)`),
- cooperative cancellation through `CancelToken` checked between work units,
- staged progress (`validating → reading → processing → writing → verifying`),
- sanitized failure categories (`CancelToken` and `TdxError::category`),
- serializable `JobRecord` snapshots safe for any UI.

The CLI runs operations synchronously but still uses the same progress sinks
and cancellation tokens, so engines cannot tell the difference.

## Files and safety

`tdx_core::fsutil` centralizes the safety rules:

- **Atomic writes** — output goes to a sibling temp file, is flushed, synced
  and only then renamed into place.
- **No overwrites by default** — callers decide whether to use
  `unique_path` or an explicit output path.
- **Disk space pre-flight** — `ensure_free_space` reports a typed error with
  required versus available megabytes.
- **Zip-slip protection** — `safe_join` normalizes archive paths and rejects
  escapes; `tdx-archive` also bounds entry count and expansion ratios.

## PDF engine

Built on `lopdf` (MIT). Merging, splitting and page extraction share one
object-import routine (`tdx_pdf::build`):

1. every object of the source document is copied with remapped references,
2. inherited page attributes (`MediaBox`, `CropBox`, `Resources`, `Rotate`)
   are resolved from the source page tree,
3. a fresh `Pages` tree and `Catalog` are assembled,
4. unreachable objects are pruned so removed pages do not leak content,
5. the result is compressed, written atomically and re-opened for verification.

Page rasterization (`pdf.to_images`) requires an optional PDFium build; the
command reports `adapter_required` until `TDX_PDFIUM_PATH` is configured.

## Conversion engine

`tdx-convert` renders text-first PDFs with `printpdf` (MIT). Latin-only
documents use the built-in Helvetica font (tiny output); when non-Latin
characters appear, a system TrueType font is embedded. Font subsetting is a
roadmap item.

## Testing strategy

- unit tests in every crate (detection, selection parsing, XML escaping,
  dictionary validation, job lifecycle, zip-slip),
- integration tests per engine generating fixtures at runtime (no binary
  fixtures in git): PDF page operations, image pipelines, CSV/XLSX/JSON
  round-trips, DOCX round-trips, conversion smoke tests,
- CLI end-to-end runs are part of release QA.

## Dependency policy

The MIT core links only permissively licensed crates. `scripts/license-audit.mjs`
enforces an allowlist against `Cargo.lock` and regenerates
`THIRD_PARTY_LICENSES.md`. LibreOffice and PDFium stay external adapters and
are never bundled.
