# Contributing

Thanks for helping improve TEDROX Documents. This guide keeps contributions
predictable and reviewable.

## Before you start

- Read [ARCHITECTURE.md](ARCHITECTURE.md) to understand the core/shell boundary.
- For anything larger than a bug fix, open an issue first so we can agree on the
  approach.
- Security issues go through [SECURITY.md](SECURITY.md), never public issues.

## Development setup

```bash
rustup toolchain install stable
cargo build --workspace
cargo test --workspace
```

The workspace targets Rust 1.80+ and is pinned by `rust-toolchain.toml`.

## Quality gates

Every pull request must pass:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
node scripts/sync-i18n.mjs
node scripts/license-audit.mjs
```

## Code guidelines

- No comments unless they explain *why*, not *what*.
- Engines live in `tdx-*` crates and never depend on UI code.
- User-facing text belongs in `packages/i18n`, never inline in UI components.
- New operations must register a descriptor in `tdx_core::ops` with honest
  capability flags; do not mark a feature supported before it is tested.
- File writes go through `tdx_core::fsutil` so atomicity is preserved.
- Add tests for behavior changes; fixtures are generated at runtime, never
  committed as large binaries.
- New dependencies must pass the license audit; prefer MIT/Apache-2.0/BSD/ISC/
  Zlib. MPL-2.0 is acceptable with a note. GPL/AGPL requires an external adapter
  instead of a direct dependency.

## Commit style

Conventional commits are appreciated:

```text
feat(pdf): add page duplication
fix(sheet): decode windows-1251 headers correctly
docs(cli): document --json output
```

## Localization

English is canonical. When you add UI strings:

1. add them to `packages/i18n/en.json`,
2. add Russian translations to `packages/i18n/ru.json`,
3. run `node scripts/sync-i18n.mjs` — CI fails on missing or orphan keys.

## Releasing

Maintainers follow `docs/build.md` and tag `vX.Y.Z`. The release workflow builds
artifacts, generates checksums and publishes the GitHub release.
