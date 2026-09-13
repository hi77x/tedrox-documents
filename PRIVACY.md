# Privacy

TEDROX Documents is designed so that using it cannot leak your documents.

## What stays on your machine

Everything. Files are read, processed and written by the local native core.
There is no server component, no account, no cloud cache and no temporary
upload. The application works with the network disabled.

## What is never collected

- document contents, extracted text or rendered pixels,
- passwords or encryption keys,
- file paths outside your own machine,
- analytics events, device identifiers or crash dumps.

## Logs

If logging is enabled, entries contain operation metadata only:

```text
operation=pdf.merge files=2 bytes_in=481234 duration_ms=1291 status=ok
operation=pdf.extract error_category=encrypted status=failed
```

File names may appear in interactive output, but never file contents. Error
categories are stable identifiers (`encrypted`, `corrupt`, `insufficient_space`)
that are safe to share in bug reports.

## Recent files and indexing

The CLI is stateless between invocations. Desktop history, when introduced,
stores paths only, is disabled until you enable it, and can be cleared
completely. Content indexing is opt-in, stored locally and deletable.

## Network access

The core makes no network requests. The landing page fetches `releases.json`
from its own origin to display the latest version; no user data is sent.

## Third parties

LibreOffice and PDFium are optional external adapters. If you install them,
they run locally like any other program; TEDROX Documents neither bundles nor
updates them.
