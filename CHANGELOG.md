# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/).

## [0.1.1] — 2026-09-13

### Added

- CI now builds and publishes the Windows NSIS installer
  (`TEDROX-Documents-0.1.1-x64-setup.exe`) with the desktop application.
- The landing page and README clearly separate the desktop installer from the
  portable command line tool.

### Fixed

- `tdx-doc.exe` launched without arguments (for example by double-clicking the
  file) now explains that it is a command line tool and waits for Enter instead
  of closing the window immediately.

## [0.1.0] — 2026-09-13

First public milestone. The native engine and CLI are complete; desktop and
Android shells are in development.

### Added

- Rust workspace with `tdx-core`, `tdx-jobs`, `tdx-pdf`, `tdx-image`,
  `tdx-sheet`, `tdx-docx`, `tdx-archive`, `tdx-convert` and `tdx-cli`.
- File type detection by magic bytes, container inspection, extension and
  content sniffing.
- Job runtime with bounded concurrency, cancellation, staged progress and
  sanitized job history.
- PDF: merge, split, extract, delete, reorder, reverse, rotate, odd/even,
  insert, duplicate, images→PDF, metadata view/edit, privacy clean,
  image re-compression presets and text watermarking.
- Images: PNG/JPEG/WebP/BMP/TIFF/AVIF conversion, resize, crop, rotate, flip,
  metadata strip, SVG rasterization, PNG→SVG embed, raster→SVG tracing and ICO
  creation.
- Spreadsheets: CSV report (delimiter, encoding, types), CSV↔XLSX↔JSON,
  transform (filter, sort, dedupe, trim, select, rename), split by rows or
  field, join, XLSX/XLS/ODS reading.
- Documents: DOCX creation from Markdown/text, DOCX text/Markdown extraction,
  Markdown/HTML→PDF with system font embedding, legacy DOC import through the
  optional LibreOffice adapter.
- Archives: ZIP creation, safe extraction with zip-slip and bomb protection.
- Files: SHA-256 hashing, duplicate detection, batch rename with dry-run.
- `tdx-doc` CLI: full command coverage, `--json` everywhere, stable exit codes,
  cooperative Ctrl+C cancellation and progress suppression when piped.
- Bilingual EN/RU landing page with GitHub release integration.
- CI: format, clippy, tests, license audit, i18n validation, Windows/Linux
  builds, Android workflow scaffold and Pages deployment.
- Documentation: architecture, build, CLI, formats, localization,
  troubleshooting, benchmarks, privacy and security policies.
