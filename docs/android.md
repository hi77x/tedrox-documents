# Android notes

The Android shell is in development. The shared Rust core is already
platform-neutral, and no business logic will be duplicated in Kotlin.

## Planned architecture

```text
apps/android (Kotlin + Jetpack Compose)
        │  UniFFI bindings
        ▼
crates/tdx-core + engines (same crates as desktop)
```

- Document access through the Storage Access Framework; no broad storage
  permissions.
- Share targets for "Open with TEDROX Documents".
- Jobs screen backed by the same `tdx-jobs` runtime.
- Files are copied into the app cache only while a job runs, then removed.

## Current status

| Item | State |
| --- | --- |
| `tdx-core` compiles for Android targets | Verified at the crate level |
| UniFFI boundary | Not implemented yet |
| Kotlin shell | Not started |
| CI workflow | Scaffolded in `.github/workflows/android.yml` (manual trigger) |

## Building the core for Android (today)

```bash
rustup target add aarch64-linux-android
cargo check -p tdx-core --target aarch64-linux-android
```

A full build additionally needs the Android NDK toolchain and a Cargo linker
configuration; the CI workflow documents the exact variables. Do not expect an
installable APK from this repository yet — the release page states this
explicitly instead of shipping a broken artifact.
