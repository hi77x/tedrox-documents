# Web application

`apps/web-app` is the browser build of TEDROX Documents. It renders the same
interface as the desktop application and runs entirely in the page.

## What works in the browser

| Area | In the browser | Desktop only |
| --- | --- | --- |
| PDF viewing | Continuous canvas, thumbnails, text layer, search, outline | — |
| PDF pages | Reorder, rotate, delete, duplicate through `pdf-lib` | — |
| PDF annotations | Highlights, ink, shapes, arrows, text boxes drawn into the page | Editable annotation objects with appearance streams |
| PDF metadata | Title, author, subject, producer can be cleared | Full privacy clean including JavaScript and embedded files |
| PDF forms | Fields are listed | Filling writes AcroForm values |
| Spreadsheets | CSV and TSV open, the shared formula engine evaluates, CSV saves | XLSX and ODS open and save |
| Documents | Markdown and text open in the paginated editor, export to Markdown | DOCX open, save and PDF export |
| Images | PNG, JPEG and WebP conversion and resizing through canvas | TIFF, AVIF, BMP, SVG tracing, ICO |
| Conversion | Image routes | DOCX, XLSX, PDF and archive routes |

Anything outside the first column reports that it needs the desktop
application instead of failing silently.

## Privacy

Files are read with the File API and never uploaded: there is no server in this
project. The service worker caches the application bundle for offline use and
never caches user documents.

## Building

```bash
cd apps/web-app
npm ci
npm run build          # writes apps/web-app/dist
npm run preview        # local check
```

## Verification

`scripts/verify-web-app.mjs` drives the real bundle in headless Chromium: it
opens a PDF and a CSV through the file chooser, asserts that the workspaces
render, and checks that the compact layout collapses the side panels. It runs
in CI and stores the web screenshots used in the README.
