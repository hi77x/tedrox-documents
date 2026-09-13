# tdx-doc CLI reference

Every command processes files locally. Add `--json` for machine-readable
output, `--quiet` to suppress progress. Exit codes: `0` success, `1` error,
`130` cancelled.

## Global

```bash
tdx-doc --help
tdx-doc tools [--category pdf|documents|sheets|csv|images|convert|files]
tdx-doc inspect FILE [--json]
```

## PDF

```bash
tdx-doc pdf merge a.pdf b.pdf -o merged.pdf
tdx-doc pdf split book.pdf --chunk 25 --out-dir out/
tdx-doc pdf split book.pdf --pages 1-5,8 --out-dir out/
tdx-doc pdf extract book.pdf --pages 10-12 -o chapter.pdf
tdx-doc pdf delete book.pdf --pages 1,3 -o trimmed.pdf
tdx-doc pdf reorder book.pdf --order 3,1,2 -o reordered.pdf
tdx-doc pdf reverse book.pdf -o reversed.pdf
tdx-doc pdf rotate scan.pdf --degrees 90 [--pages all] -o fixed.pdf
tdx-doc pdf odd-even scan.pdf [--even] -o split.pdf
tdx-doc pdf insert base.pdf extra.pdf --after 2 -o combined.pdf
tdx-doc pdf metadata file.pdf [--json]
tdx-doc pdf set-metadata file.pdf -o out.pdf --title "Report" --author "Team"
tdx-doc pdf clean file.pdf -o clean.pdf
tdx-doc pdf compress file.pdf -o small.pdf --preset screen|balanced|print
tdx-doc pdf compress file.pdf -o small.pdf --quality 60 --max-dimension 1200
tdx-doc pdf watermark file.pdf -o stamped.pdf --text DRAFT --opacity 0.2
tdx-doc pdf from-images page1.png page2.jpg -o album.pdf [--page-size a4|letter|fit]
tdx-doc pdf to-images file.pdf --out-dir pages/ --dpi 150   # requires PDFium adapter
tdx-doc pdf inspect file.pdf [--json]
```

Page selections accept `all`, `odd`, `even`, single pages (`7`), ranges (`1-5`)
and open ranges (`10-`), combined with commas.

## Images

```bash
tdx-doc image convert in.png --to webp -o out.webp [--quality 90] [--background 255,255,255]
tdx-doc image resize in.png -o out.png --width 1280 [--height 720] [--mode contain|cover|exact]
tdx-doc image crop in.png -o out.png --x 0 --y 0 --width 512 --height 512
tdx-doc image rotate in.png -o out.png --degrees 90 [--flip-horizontal] [--flip-vertical]
tdx-doc image strip in.jpg -o clean.jpg
tdx-doc image svg-render icon.svg -o icon.png [--width 512] [--height 512] [--to png|jpg|webp]
tdx-doc image svg-embed raster.png -o wrapped.svg
tdx-doc image trace logo.png -o logo.svg [--preset bw|poster|photo] [--speckle 4]
tdx-doc image ico icon.png -o icon.ico [--size 256]
```

`svg-embed` wraps raster pixels in SVG — it is not vector tracing. Use
`image trace` for real vectors.

## CSV and spreadsheets

```bash
tdx-doc csv info data.csv [--json]
tdx-doc csv to-xlsx data.csv -o data.xlsx [--sheet Sheet1]
tdx-doc csv to-json data.csv -o data.json [--compact]
tdx-doc csv from-json data.json -o data.csv
tdx-doc csv transform data.csv -o clean.csv \
  [--trim] [--drop-empty] [--dedupe] \
  [--select name,score] [--rename old=new] \
  [--sort score] [--desc] \
  [--filter-col team --contains core | --equals infra | --gt 10 | --lt 100]
tdx-doc csv split data.csv --every 1000 --out-dir parts/
tdx-doc csv split data.csv --by team --out-dir groups/
tdx-doc csv join a.csv b.csv -o all.csv

tdx-doc xlsx info book.xlsx [--json]
tdx-doc xlsx to-csv book.xlsx -o sheet.csv [--sheet "Q1"] [--delimiter ";"]
```

## Documents

```bash
tdx-doc doc create notes.md -o notes.docx [--title T] [--author A]
tdx-doc doc extract notes.docx -o notes.txt [--markdown]
tdx-doc doc inspect notes.docx [--json]
tdx-doc doc legacy-import old.doc -o converted.docx   # optional LibreOffice adapter
tdx-doc convert notes.md -o notes.pdf
tdx-doc convert page.html -o page.pdf
tdx-doc convert data.csv -o data.xlsx
tdx-doc convert photo.png -o photo.webp
```

`convert` detects the input type from content and routes to the right engine;
unsupported pairs fail with an explicit message instead of guessing.

## Archives and files

```bash
tdx-doc archive zip folder/ a.txt -o bundle.zip
tdx-doc archive list bundle.zip [--json]
tdx-doc archive unzip bundle.zip --out-dir extracted/
tdx-doc file hash *.pdf [--json]
tdx-doc file duplicates folder/ [--json]
tdx-doc file rename *.png --prefix img_ --start 1 [--apply]
```

Batch rename is a dry run until `--apply` is passed and refuses to overwrite
existing files.

## JSON output

All commands that produce a result emit:

```json
{
  "ok": true,
  "operation": "pdf.merge",
  "outputs": [{ "path": "...", "bytes": 191234, "kind": "pdf", "label": null }],
  "bytesIn": 188000,
  "bytesOut": 191234,
  "durationMs": 1291,
  "warnings": [],
  "stats": { "pages": 2, "files": 2 }
}
```

Errors use `{"ok": false, "error": {"category": "...", "message": "..."}}`.
Progress is written to stderr and disabled automatically when piped.
