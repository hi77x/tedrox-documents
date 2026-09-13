# Benchmarks

Measurements from a real machine, not estimates. Re-run them locally with the
commands listed below; results depend on hardware and storage.

## Test machine

| Property | Value |
| --- | --- |
| CPU | Intel Xeon E3-1240 @ 3.30 GHz (4 cores / 8 threads) |
| RAM | 15.5 GB |
| OS | Windows 10 Pro |
| Storage | SATA SSD |
| Build | `tdx-doc` v0.1.0, `cargo build --release` (thin LTO, stripped) |
| Date | 2026-09-13 |

## CLI startup

| Command | Time |
| --- | --- |
| `tdx-doc --version` | 20 ms |
| `tdx-doc tools --json` (full registry, 50 operations) | 27 ms |

## PDF

Fixtures: a 40-paragraph Markdown document rendered to one page per section,
duplicated to 40 single-page PDFs; merged file 80 pages.

| Operation | Input | Time |
| --- | --- | --- |
| Markdown → PDF (5 blocks) | 105 B | 208 ms |
| Merge 40 files → 40 pages | 80 KB | 218 ms |
| Merge 40 files + 40 files → 80 pages | 113 KB | 225 ms |
| Split 40 pages into 40 files | 40 pages | 735 ms |
| Rotate 40 pages (90°) | 40 pages | 137 ms |
| Privacy clean 40 pages | 40 pages | 138 ms |

Peak memory (sampled): **7.1 MB** while merging 80 pages.

Reproduce:

```bash
tdx-doc pdf merge page*.pdf -o merged.pdf
tdx-doc pdf split merged.pdf --chunk 1 --out-dir parts/
tdx-doc pdf clean merged.pdf -o cleaned.pdf
```

## CSV

Fixture: 250,000 rows × 5 columns, 11.0 MB UTF-8, mixed integer, text and float
columns.

| Operation | Time | Peak RAM |
| --- | --- | --- |
| `csv info` (full scan, type inference) | 162 ms | 4.3 MB |
| `csv transform --filter-col --equals --sort --desc` | 360 ms | — |
| `csv split --every 25000` (10 files) | 221 ms | — |
| `csv to-xlsx` | 3.3 s | 305.7 MB |

The reader is fully streaming — 11 MB of CSV scanned in 4.3 MB of RAM. The XLSX
writer keeps the sheet in memory by design (`rust_xlsxwriter`); a
constant-memory mode is on the roadmap.

Reproduce:

```bash
tdx-doc csv info big.csv
tdx-doc csv transform big.csv -o filtered.csv --filter-col team --equals team3 --sort score --desc
tdx-doc csv to-xlsx big.csv -o big.xlsx
```

## Images

Fixture: a 3840×2160 PNG generated from SVG (gradient plus 400 translucent
shapes).

| Operation | Time |
| --- | --- |
| SVG → PNG 3840×2160 | 222 ms |
| PNG → WebP 3840×2160 | 159 ms |
| WebP → WebP resize to 1280 px wide | 323 ms |
| Raster → SVG trace (photo preset) 3840×2160 | 3.8 s |

Reproduce:

```bash
tdx-doc image svg-render large.svg -o large.png
tdx-doc image convert large.png --to webp -o large.webp
tdx-doc image resize large.webp -o small.webp --width 1280
tdx-doc image trace large.png -o traced.svg --preset photo
```

## Methodology

- Wall-clock time from the CLI's own report (`Done in …`), plus
  `Measure-Command` for cross-checks; process spawn overhead (~10–20 ms) is
  included in startup numbers only.
- Peak memory sampled at 15 ms intervals via `Get-Process` while the process
  runs.
- Fixtures were generated on the machine; none of them are committed to the
  repository.
- Numbers are updated when the engine changes; do not copy them into marketing
  material without re-running.
