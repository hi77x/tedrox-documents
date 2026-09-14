# Android notes

The Android application is **not built yet**. This document records the exact
state so nobody mistakes the repository for something it is not.

## What works today

| Item | State |
| --- | --- |
| Shared Rust core compiles for Android targets | Verified in CI (`android.yml`, manual trigger) |
| Browser application on Android | Works: `apps/web-app` is an installable PWA |
| Native Kotlin/Tauri shell | Not started |
| APK or AAB artifact | Not produced |

## Why there is no APK yet

The engine and the interface are ready for a mobile shell, but one blocker is
real and must be solved before an APK would be useful:

**Storage Access Framework.** On Android the system document picker returns
`content://` URIs, not filesystem paths. Every TEDROX command currently takes a
path and reads it with `std::fs`. Shipping an APK before a content-resolver
bridge exists would present a file picker that cannot open the chosen file,
which is worse than not shipping.

The intended fix, in order:

1. Add a Kotlin/JNI bridge (or `tauri-plugin-fs` document-URI support) that
   copies a chosen document into the application cache directory.
2. Resolve every command's input path through that bridge.
3. Write results back through `ACTION_CREATE_DOCUMENT`.
4. Then enable the shell.

## Planned shell

```text
apps/android (Tauri 2 mobile or Kotlin + Jetpack Compose)
        │
        ▼
crates/tdx-* (the same engine as the desktop and the CLI)
```

- Bottom sheets and contextual toolbars instead of a shrunken desktop ribbon.
- Page thumbnails in a drawer, selection-based formatting panels.
- Share target: "Open with TEDROX Documents".
- Files live in the app cache only while a job runs, then are removed.

## Building the core for Android today

```bash
rustup target add aarch64-linux-android
cargo check -p tdx-core --target aarch64-linux-android
```

A full native build additionally needs the NDK toolchain and linker
configuration. `arm64-v8a` and `armeabi-v7a` are the targets worth building
first; `x86_64` is useful for emulators only.

## Using TEDROX on a phone now

Open the web application in Chrome and use **Add to Home screen**. It runs
entirely in the browser, keeps files on the device and supports PDF viewing,
page organization, annotations, spreadsheets and text documents.
