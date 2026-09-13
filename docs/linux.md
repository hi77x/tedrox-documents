# Linux notes

## Building

```bash
git clone https://github.com/hi77x/tedrox-documents
cd tedrox-documents
cargo build --release -p tdx-cli
./target/release/tdx-doc tools
```

The core has no system-library requirements beyond a standard build toolchain;
`flate2` uses the pure-Rust backend and `printpdf` writes PDF without native
dependencies.

## Desktop shell

The Tauri 2 shell (in development) needs WebKitGTK when it lands:

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev
```

## Packaging status

AppImage and `.deb` packages are on the roadmap and not produced yet. Until
then, build from source and copy `target/release/tdx-doc` into `~/.local/bin`
or `/usr/local/bin`.

```bash
install -m 755 target/release/tdx-doc ~/.local/bin/tdx-doc
```

## System fonts for PDF generation

Non-Latin document text needs a TrueType font. The converter looks for:

- `/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf`
- `/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf`
- `/usr/share/fonts/TTF/DejaVuSans.ttf`

Install `fonts-dejavu-core` (or `fonts-liberation`) if none of these exist.
Latin-only documents use the built-in Helvetica font and need nothing.

## Qt/GTK themes

The CLI is unaffected by desktop themes. The future Tauri shell will follow the
system light/dark preference automatically.
