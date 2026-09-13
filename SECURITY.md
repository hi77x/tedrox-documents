# Security policy

## Reporting a vulnerability

Open a private security advisory on GitHub
(`Security → Advisories → Report a vulnerability`) instead of a public issue.
Include the affected version, a minimal reproducer and the impact you observed.
Reports are acknowledged within 7 days.

## Threat model

Document processing handles untrusted input. The engine treats every file as
hostile by default:

| Threat | Mitigation |
| --- | --- |
| Malformed PDFs | Parser errors map to a `corrupt` category; the input is never modified |
| Decompression bombs | ZIP extraction enforces entry count, expansion ratio and an 8 GB total budget |
| Zip slip / path traversal | `fsutil::safe_join` rejects absolute paths and `..` escapes |
| Crafted XML in DOCX | The reader extracts text only; it never executes content or resolves external entities |
| Embedded JavaScript / actions | `pdf clean` removes `/JavaScript`, `/OpenAction`, `/AA` and embedded files |
| Unsafe HTML preview | HTML is converted to text; scripts and styles are ignored, no web engine loads it |
| SVG script or external loads | SVG rasterization uses `usvg`, which ignores scripts and external resources |
| Password handling | The core can report encryption but never brute-forces or stores passwords |
| Supply chain | CI runs the license audit; releases are built from tagged commits with `Cargo.lock` pinned |

## Guarantees

- No document contents are logged. Log lines contain operation ids, file types,
  sizes, durations and sanitized error categories only.
- No telemetry, analytics or background network calls exist in the core.
- The only network access in the repository is the optional release check and
  the landing page's GitHub API request for version information.
- Release signing keys and API tokens are never stored in the repository;
  workflows read them from GitHub Actions secrets.

## Known limitations

- Windows builds are not code-signed yet; SmartScreen may warn.
- PDF page rasterization is disabled until the optional PDFium adapter is
  configured; the operation fails with a clear message rather than pretending.
- Redaction is intentionally unavailable; drawing black rectangles over
  recoverable text would be unsafe. A verified redaction pipeline is on the
  roadmap.
