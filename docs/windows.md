# Windows notes

## Building

```powershell
git clone https://github.com/hi77x/tedrox-documents
cd tedrox-documents
cargo build --release -p tdx-cli
.\target\release\tdx-doc.exe tools
```

Requirements: Rust stable with the MSVC toolchain (the default on
windows-msvc), Windows 10 or newer.

## Packaging

```powershell
powershell -ExecutionPolicy Bypass -File scripts/package-windows.ps1
```

Artifacts land in `dist/`:

- `tdx-doc-windows-x86_64.zip` — portable CLI (plus README, license and
  third-party notices),
- `SHA256SUMS.txt`.

## Running without installation

The ZIP is portable: unpack anywhere and run `tdx-doc.exe` from a terminal or
call it from scripts. No registry writes, no services.

## SmartScreen

Builds are not code-signed yet. On first run Windows may show "Windows
protected your PC"; choose **More info → Run anyway**, or verify the SHA256
checksum against `SHA256SUMS.txt` first:

```powershell
Get-FileHash .\tdx-doc-windows-x86_64.zip -Algorithm SHA256
```

## Long paths and Unicode

The engine uses Windows wide-character APIs through Rust's standard library;
Unicode names and long paths are handled as long as the system has long-path
support enabled (`LongPathsEnabled`). If a path is too long, the operation
fails with a clear I/O error rather than truncating.
