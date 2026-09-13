# Architecture (deep dive)

The summary lives in [`ARCHITECTURE.md`](../ARCHITECTURE.md) at the repository
root. This document adds the data-flow and dependency detail needed to extend
the engine.

## Dependency graph

```text
tdx-cli ──────────────┐
tedrox-documents-desktop (Tauri) ─┤
                      ▼
              tdx-convert ── tdx-docx ── zip
                  │            │
                  │            └─ (text/markdown extraction)
                  ▼
     tdx-pdf ── tdx-jobs ── tdx-core
     tdx-image         │
     tdx-sheet ────────┘
     tdx-archive ──────┘
```

Rules enforced in review:

- `tdx-core` depends on nothing in the workspace.
- Engines never depend on `tdx-convert`, `tdx-cli` or the desktop shell.
- The desktop shell only calls engine APIs; it contains no document logic.
- `tdx-convert` orchestrates engines for cross-format routing.

## Operation lifecycle

1. The caller detects the input (`tdx_core::detect`).
2. The engine validates options and runs the disk-space pre-flight.
3. Reading is streamed where the format allows (CSV) or bounded by the file
   size (PDF, images).
4. Work units check `CancelToken::check()` between pages, images and rows.
5. Progress events are emitted through `ProgressSink` with a stage and a
   0..1 fraction.
6. Output is encoded to a buffer or a temp file and written atomically.
7. PDF outputs are re-opened and page counts compared before success is
   reported; mismatches downgrade to warnings.

## Job runtime

`JobEngine` runs tasks on a bounded worker pool:

```text
queued → validating → reading → processing → writing → verifying → completed
                                                                 ↘ failed / cancelled
```

- `JobRecord` snapshots are serializable and safe for UI transfer.
- `JobEngine::set_listener` receives an update on every state change.
- Cancellation is cooperative; long engine calls receive the same token the
  UI holds.

## Adding an operation

1. Implement the engine function in the correct `tdx-*` crate, accepting a
   `ProgressSink` and `CancelToken` when the work is non-trivial.
2. Describe it in `tdx_core::ops::OperationRegistry::builtin` with honest
   capability flags.
3. Expose it in `tdx-cli` (one match arm) and, when ready, in the desktop
   shell (one command plus one UI entry).
4. Add an integration test that generates its own fixture.
5. Update `docs/formats.md`, `docs/cli.md` and the changelog.
