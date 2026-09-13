//! `tdx-doc` — the TEDROX Documents command line interface.

mod doc_cmds;
mod file_cmds;
mod image_cmds;
mod pdf_cmds;
mod sheet_cmds;
mod support;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};

use support::{print_result, Ctx, OutputFormat};

#[derive(Parser)]
#[command(
    name = "tdx-doc",
    version,
    about = "TEDROX Documents CLI — local-first document toolbox",
    long_about = "Process PDFs, documents, spreadsheets, images and archives locally.\nNo file ever leaves this machine.\n\nRun `tdx-doc tools` to list every implemented operation."
)]
struct Cli {
    #[arg(long, global = true, help = "Machine-readable JSON output")]
    json: bool,
    #[arg(long, short, global = true, help = "Suppress progress output")]
    quiet: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// PDF page operations, metadata, compression and rendering
    Pdf(PdfArgs),
    /// Image conversion, geometry, SVG and tracing
    Image(ImageArgs),
    /// CSV and spreadsheet work
    Csv(CsvArgs),
    /// XLSX / XLS / ODS inspection and export
    Xlsx(XlsxArgs),
    /// Word documents, Markdown and text
    Doc(DocArgs),
    /// Conversion center (auto-detected routing)
    Convert(ConvertArgs),
    /// ZIP archives
    Archive(ArchiveArgs),
    /// File utilities: hashes, duplicates, batch rename
    File(FileArgs),
    /// Detect the type of any file
    Inspect(InspectArgs),
    /// List operation capabilities exposed by the core
    Tools(ToolsArgs),
}

#[derive(Args)]
struct PdfArgs {
    #[command(subcommand)]
    command: PdfCommand,
}

#[derive(Subcommand)]
enum PdfCommand {
    /// Merge two or more PDF files
    Merge {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Split a PDF into pages, chunks or explicit ranges
    Split {
        file: PathBuf,
        #[arg(long)]
        pages: Option<String>,
        #[arg(long)]
        chunk: Option<u32>,
        #[arg(long, default_value = ".")]
        out_dir: PathBuf,
    },
    /// Extract a page selection into a new PDF
    Extract {
        file: PathBuf,
        #[arg(long, default_value = "all")]
        pages: String,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Delete a page selection
    Delete {
        file: PathBuf,
        #[arg(long)]
        pages: String,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Reverse page order
    Reverse {
        file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Rewrite with an explicit page order
    Reorder {
        file: PathBuf,
        #[arg(long, help = "Comma-separated 1-based page order, e.g. 3,1,2")]
        order: String,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Rotate pages clockwise
    Rotate {
        file: PathBuf,
        #[arg(long, default_value_t = 90)]
        degrees: i32,
        #[arg(long)]
        pages: Option<String>,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Extract only odd or only even pages
    OddEven {
        file: PathBuf,
        #[arg(long, help = "Keep odd pages (default) instead of even")]
        even: bool,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Insert one PDF into another after page N (0 = beginning)
    Insert {
        base: PathBuf,
        insert: PathBuf,
        #[arg(long, default_value_t = 0)]
        after: u32,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Read document metadata
    Metadata { file: PathBuf },
    /// Remove metadata, XMP, JavaScript and embedded files
    Clean {
        file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Change title, author, subject and keywords
    SetMetadata {
        file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        author: Option<String>,
        #[arg(long)]
        subject: Option<String>,
        #[arg(long)]
        keywords: Option<String>,
    },
    /// Re-encode embedded images to reduce size
    Compress {
        file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long, default_value = "balanced", value_parser = ["screen", "balanced", "print"])]
        preset: String,
        #[arg(long)]
        quality: Option<u8>,
        #[arg(long)]
        max_dimension: Option<u32>,
    },
    /// Stamp a text watermark on every page
    Watermark {
        file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long, default_value = "CONFIDENTIAL")]
        text: String,
        #[arg(long, default_value_t = 48.0)]
        size: f32,
        #[arg(long, default_value_t = 0.15)]
        opacity: f32,
        #[arg(long, default_value_t = 45.0)]
        angle: f32,
    },
    /// Render PDF pages to images (requires the PDFium adapter)
    ToImages {
        file: PathBuf,
        #[arg(long, default_value = ".")]
        out_dir: PathBuf,
        #[arg(long, default_value = "png", value_parser = ["png", "jpg", "jpeg"])]
        format: String,
        #[arg(long, default_value_t = 150)]
        dpi: u32,
    },
    /// Build a PDF from images
    FromImages {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long, default_value = "a4", value_parser = ["a4", "letter", "fit"])]
        page_size: String,
        #[arg(long, default_value_t = 24.0)]
        margin: f32,
    },
    /// Structural summary of a PDF
    Inspect { file: PathBuf },
}

#[derive(Args)]
struct ImageArgs {
    #[command(subcommand)]
    command: ImageCommand,
}

#[derive(Subcommand)]
enum ImageCommand {
    /// Convert between bitmap formats
    Convert {
        file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long, value_parser = ["png", "jpg", "jpeg", "webp", "bmp", "tiff", "avif"])]
        to: String,
        #[arg(long, default_value_t = 90)]
        quality: u8,
        #[arg(long, help = "Background color as R,G,B for flattening transparency")]
        background: Option<String>,
    },
    /// Resize an image
    Resize {
        file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        width: Option<u32>,
        #[arg(long)]
        height: Option<u32>,
        #[arg(long, default_value = "contain", value_parser = ["contain", "cover", "exact"])]
        mode: String,
        #[arg(long)]
        to: Option<String>,
    },
    /// Crop a rectangular region
    Crop {
        file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        x: u32,
        #[arg(long)]
        y: u32,
        #[arg(long)]
        width: u32,
        #[arg(long)]
        height: u32,
        #[arg(long)]
        to: Option<String>,
    },
    /// Rotate and/or flip an image
    Rotate {
        file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long, default_value_t = 90)]
        degrees: i32,
        #[arg(long)]
        flip_horizontal: bool,
        #[arg(long)]
        flip_vertical: bool,
    },
    /// Re-encode without EXIF or auxiliary metadata
    Strip {
        file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Rasterize an SVG
    SvgRender {
        file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        width: Option<u32>,
        #[arg(long)]
        height: Option<u32>,
        #[arg(long, default_value = "png", value_parser = ["png", "jpg", "webp"])]
        to: String,
    },
    /// Wrap a raster image in an SVG container (embed, not tracing)
    SvgEmbed {
        file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Trace a raster image into vector paths
    Trace {
        file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long, default_value = "poster", value_parser = ["bw", "poster", "photo"])]
        preset: String,
        #[arg(long, default_value_t = 4)]
        speckle: u32,
        #[arg(long, default_value_t = 6)]
        color_precision: u8,
    },
    /// Create a Windows ICO icon
    Ico {
        file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long, default_value_t = 256)]
        size: u32,
    },
}

#[derive(Args)]
struct CsvArgs {
    #[command(subcommand)]
    command: CsvCommand,
}

#[derive(Subcommand)]
enum CsvCommand {
    /// Report delimiter, encoding, columns and row count
    Info { file: PathBuf },
    /// Convert CSV/TSV to XLSX
    ToXlsx {
        file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long, default_value = "Sheet1")]
        sheet: String,
        #[arg(long)]
        delimiter: Option<String>,
    },
    /// Convert CSV to a JSON array
    ToJson {
        file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        compact: bool,
        #[arg(long)]
        delimiter: Option<String>,
    },
    /// Convert a JSON array of objects to CSV
    FromJson {
        file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        delimiter: Option<String>,
    },
    /// Filter, sort, deduplicate, trim, rename and select columns
    Transform {
        file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        delimiter: Option<String>,
        #[arg(long)]
        output_delimiter: Option<String>,
        #[arg(long)]
        trim: bool,
        #[arg(long)]
        drop_empty: bool,
        #[arg(long)]
        dedupe: bool,
        #[arg(long, help = "Comma-separated column names to keep")]
        select: Option<String>,
        #[arg(long, help = "Rename columns: old=new,old2=new2")]
        rename: Option<String>,
        #[arg(long)]
        sort: Option<String>,
        #[arg(long)]
        desc: bool,
        #[arg(long)]
        filter_col: Option<String>,
        #[arg(long)]
        contains: Option<String>,
        #[arg(long)]
        equals: Option<String>,
        #[arg(long)]
        gt: Option<f64>,
        #[arg(long)]
        lt: Option<f64>,
    },
    /// Split by row count or by a column value
    Split {
        file: PathBuf,
        #[arg(long, default_value = ".")]
        out_dir: PathBuf,
        #[arg(long, conflicts_with = "by")]
        every: Option<u64>,
        #[arg(
            long,
            conflicts_with = "every",
            help = "Split by the value of this column"
        )]
        by: Option<String>,
    },
    /// Concatenate CSV files with the same header
    Join {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        #[arg(short, long)]
        output: PathBuf,
    },
}

#[derive(Args)]
struct XlsxArgs {
    #[command(subcommand)]
    command: XlsxCommand,
}

#[derive(Subcommand)]
enum XlsxCommand {
    /// List sheets and dimensions
    Info { file: PathBuf },
    /// Export one sheet to CSV
    ToCsv {
        file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        sheet: Option<String>,
        #[arg(long)]
        delimiter: Option<String>,
    },
}

#[derive(Args)]
struct DocArgs {
    #[command(subcommand)]
    command: DocCommand,
}

#[derive(Subcommand)]
enum DocCommand {
    /// Extract plain text (or Markdown) from a document
    Extract {
        file: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long)]
        markdown: bool,
    },
    /// Create a DOCX from Markdown or text
    Create {
        file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        author: Option<String>,
    },
    /// Document statistics
    Inspect { file: PathBuf },
    /// Convert a legacy DOC through the LibreOffice adapter
    LegacyImport {
        file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
}

#[derive(Args)]
struct ConvertArgs {
    /// Input file (type is detected from content)
    file: PathBuf,
    /// Output path; the extension decides the target format
    #[arg(short, long)]
    output: PathBuf,
}

#[derive(Args)]
struct ArchiveArgs {
    #[command(subcommand)]
    command: ArchiveCommand,
}

#[derive(Subcommand)]
enum ArchiveCommand {
    /// Create a ZIP archive
    Zip {
        #[arg(required = true)]
        items: Vec<PathBuf>,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Extract a ZIP archive safely
    Unzip {
        file: PathBuf,
        #[arg(long, default_value = ".")]
        out_dir: PathBuf,
    },
    /// List archive entries
    List { file: PathBuf },
}

#[derive(Args)]
struct FileArgs {
    #[command(subcommand)]
    command: FileCommand,
}

#[derive(Subcommand)]
enum FileCommand {
    /// Compute SHA-256 hashes
    Hash {
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
    /// Find duplicate files by content hash
    Duplicates {
        #[arg(required = true)]
        paths: Vec<PathBuf>,
    },
    /// Batch rename with prefix/suffix/numbering/replacement
    Rename {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        #[arg(long)]
        prefix: Option<String>,
        #[arg(long)]
        suffix: Option<String>,
        #[arg(long, help = "Exact replacement: from=to")]
        replace: Option<String>,
        #[arg(long = "start")]
        start_number: Option<u32>,
        #[arg(long, help = "Write changes (default is a dry run)")]
        apply: bool,
    },
}

#[derive(Args)]
struct InspectArgs {
    file: PathBuf,
}

#[derive(Args)]
struct ToolsArgs {
    #[arg(
        long,
        help = "Filter by category: pdf, documents, sheets, csv, images, convert, files"
    )]
    category: Option<String>,
}

fn main() -> ExitCode {
    if std::env::args_os().len() == 1 {
        welcome();
        return ExitCode::SUCCESS;
    }
    let cli = Cli::parse();
    let format = if cli.json {
        OutputFormat::Json
    } else {
        OutputFormat::Human
    };
    let ctx = match Ctx::new(format, cli.quiet) {
        Ok(ctx) => ctx,
        Err(err) => {
            eprintln!("Error: {err}");
            return ExitCode::from(1);
        }
    };

    let outcome = match cli.command {
        Command::Pdf(args) => pdf_cmds::run(&ctx, args),
        Command::Image(args) => image_cmds::run(&ctx, args),
        Command::Csv(args) => sheet_cmds::run_csv(&ctx, args),
        Command::Xlsx(args) => sheet_cmds::run_xlsx(&ctx, args),
        Command::Doc(args) => doc_cmds::run(&ctx, args),
        Command::Convert(args) => doc_cmds::run_convert(&ctx, args),
        Command::Archive(args) => file_cmds::run_archive(&ctx, args),
        Command::File(args) => file_cmds::run_file(&ctx, args),
        Command::Inspect(args) => file_cmds::run_inspect(&ctx, args),
        Command::Tools(args) => support::run_tools(&ctx, args.category.as_deref()),
    };

    match outcome {
        Ok(exit) => exit,
        Err(err) => {
            ctx.report_error(&err);
            match err {
                tdx_core::TdxError::Cancelled => ExitCode::from(130),
                _ => ExitCode::from(1),
            }
        }
    }
}

/// Friendly output when the executable is launched without arguments
/// (typically a double-click from Explorer). Keeps the window open so the
/// message stays readable.
fn welcome() {
    println!("TEDROX Documents {} — command line tool", tdx_core::VERSION);
    println!("Your document toolbox. Offline. Fast. Open.");
    println!();
    println!("This is a command-line program. Open PowerShell or Windows Terminal and run:");
    println!();
    println!("  tdx-doc tools                       list every operation");
    println!("  tdx-doc pdf merge a.pdf b.pdf -o merged.pdf");
    println!("  tdx-doc convert notes.md -o notes.pdf");
    println!("  tdx-doc csv to-xlsx data.csv -o data.xlsx");
    println!();
    println!("Looking for the desktop application? Install TEDROX-Documents-*-x64-setup.exe");
    println!("from https://github.com/hi77x/tedrox-documents/releases");
    println!();
    println!("Это консольная утилита: запускайте её из PowerShell или терминала.");
    println!("Нажмите Enter, чтобы закрыть это окно.");
    let _ = std::io::stdin().read_line(&mut String::new());
}

pub(crate) fn run_and_report(
    ctx: &Ctx,
    operation: &str,
    inputs: &[PathBuf],
    run: impl FnOnce(
        &tdx_core::ProgressSink,
        &tdx_core::CancelToken,
    ) -> tdx_core::Result<tdx_core::OperationResult>,
) -> tdx_core::Result<ExitCode> {
    let result = ctx.execute(operation, inputs, run)?;
    print_result(ctx, operation, &result);
    Ok(ExitCode::SUCCESS)
}
