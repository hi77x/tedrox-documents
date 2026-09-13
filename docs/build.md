# Build and release

## Prerequisites

| Tool | Version | Notes |
| --- | --- | --- |
| Rust | stable, 1.80+ | `rust-toolchain.toml` pins the channel |
| Node.js | 20+ | For landing tools, i18n sync and license audit only |
| Git | any recent | |

Cargo dependencies are pinned by `Cargo.lock`; do not build with
`--ignore-rust-version` on older compilers.

## Build

```bash
cargo build --workspace            # debug
cargo build --release -p tdx-cli   # tdx-doc binary in target/release
```

The workspace profile enables thin LTO and strips release binaries.

## Test

```bash
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

Tests generate their own fixtures in temporary directories; no large binaries
are stored in the repository.

## Repository checks

```bash
node scripts/sync-i18n.mjs      # dictionary completeness (EN/RU)
node scripts/license-audit.mjs  # dependency license allowlist + THIRD_PARTY_LICENSES.md
node scripts/generate-releases.mjs  # refresh apps/web/releases.json
```

## Package (Windows)

```powershell
powershell -ExecutionPolicy Bypass -File scripts/package-windows.ps1
```

Creates `dist/tdx-doc-windows-x86_64.zip` plus `SHA256SUMS.txt`.

## Deploy the landing

Set the deployment token in the environment, never in a file:

```powershell
$env:TEDROX_API_KEY = "..."   # Windows PowerShell
./scripts/deploy-landing.ps1
```

```bash
export TEDROX_API_KEY="..."   # bash
./scripts/deploy-landing.sh
```

The script syncs dictionaries, refreshes the release manifest, reuses or
creates the "TEDROX Documents" project and deploys `apps/web` with `--wait`.
GitHub Pages is configured as a fallback (`.github/workflows/pages.yml`).

## Release checklist

1. Update `CHANGELOG.md` and the version in `Cargo.toml`.
2. `cargo test --workspace` and the repository checks pass.
3. Commit, tag `vX.Y.Z`, push the tag.
4. `release.yml` builds artifacts and publishes the GitHub release with
   checksums.
5. Run `node scripts/generate-releases.mjs` and deploy the landing.
