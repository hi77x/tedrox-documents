# Troubleshooting

## "This document is encrypted or password protected"

The PDF has an owner or user password. The core never attempts to break
encryption. Remove the protection with the tool that created it, then retry.
For viewing only, most PDF readers can still open the file.

## "Not enough disk space"

The operation estimates the required space (including a 64 MB safety margin)
before writing. Free space or choose another output location with `-o`.

## "No system TrueType font was found"

Markdown/HTML/text → PDF needs a system font only for non-Latin characters.
Install one of:

- Windows: any of Arial, Segoe UI, Calibri (present by default)
- Linux: `fonts-dejavu-core` or `fonts-liberation`
- macOS: Arial from the system fonts

Latin-only documents always work without external fonts.

## "PDF rendering requires the optional PDFium adapter"

`pdf to-images` is intentionally disabled until PDFium is configured. See
[formats.md](formats.md) for the setup and licensing rationale.

## "This operation requires an optional external adapter"

You hit a legacy format (`.doc`, `.xls`) without LibreOffice installed. Either
install LibreOffice or export the file to DOCX/XLSX with the application that
created it.

## CSV columns or delimiters look wrong

The detector chooses the delimiter that stays consistent across the first 20
lines and treats the first row as a header when it looks like unique labels.
Override explicitly:

```bash
tdx-doc csv transform data.csv -o out.csv --delimiter ";"
# or parse without header handling
tdx-doc csv to-xlsx data.csv -o data.xlsx --delimiter "\t"
```

Windows-1251, UTF-16 and other encodings are decoded automatically; the report
from `csv info` shows which encoding was used.

## Output is larger than expected

- Markdown/DOCX → PDF embeds a full TrueType font (~1–2 MB) when non-Latin
  characters are present; font subsetting is on the roadmap.
- PDF compression only re-encodes images. Vector-heavy documents may not
  shrink; the command says so instead of pretending.
- PNG is lossless; convert to WebP or JPEG when size matters.

## CLI prints escape codes in logs

Progress uses carriage returns and is disabled automatically when stderr is
not a terminal, or explicitly with `--quiet`. `--json` never emits progress.

## Windows SmartScreen warning

Builds are unsigned. Verify `SHA256SUMS.txt` from the release page and choose
"More info → Run anyway". Code signing is tracked on the roadmap.

## Where do I report a bug?

Open an issue with the command you ran, the tool version (`tdx-doc --version`)
and the sanitized error category from `--json` output. Never paste document
contents.
