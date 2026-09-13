//! Document and conversion subcommands.

use std::path::PathBuf;
use std::process::ExitCode;

use tdx_core::{
    detect::{detect, FileKind},
    error::Result,
    fsutil,
    result::OperationResult,
    TdxError,
};

use crate::support::{Ctx, OutputFormat};
use crate::{run_and_report, ConvertArgs, DocArgs, DocCommand};

pub fn run(ctx: &Ctx, args: DocArgs) -> Result<ExitCode> {
    match args.command {
        DocCommand::Extract {
            file,
            output,
            markdown,
        } => {
            let text = if markdown {
                tdx_docx::extract_markdown(&file)?
            } else {
                tdx_docx::extract_text(&file)?
            };
            let output = match output {
                Some(path) => path,
                None => {
                    let extension = if markdown { "md" } else { "txt" };
                    fsutil::default_output(&file, "extracted", extension)
                }
            };
            fsutil::atomic_write(&output, |writer| {
                std::io::Write::write_all(writer, text.as_bytes()).map_err(TdxError::Io)
            })?;
            let kind = if markdown {
                FileKind::Markdown
            } else {
                FileKind::Txt
            };
            let mut result = OperationResult::single(output.clone(), kind);
            result.bytes_in = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
            result.set_stat("characters", text.chars().count());
            crate::support::print_result(
                ctx,
                if markdown {
                    "doc.to_markdown"
                } else {
                    "doc.extract_text"
                },
                &result.finalize(),
            );
            Ok(ExitCode::SUCCESS)
        }
        DocCommand::Create {
            file,
            output,
            title,
            author,
        } => {
            let options = tdx_docx::DocxOptions { title, author };
            run_and_report(
                ctx,
                "doc.create",
                std::slice::from_ref(&file),
                |sink, cancel| tdx_docx::create_from_file(&file, &output, &options, sink, cancel),
            )
        }
        DocCommand::Inspect { file } => {
            let detected = detect(&file)?;
            if ctx.format == OutputFormat::Json {
                let payload = if detected.kind == FileKind::Docx {
                    serde_json::to_value(tdx_docx::inspect(&file)?).unwrap_or_default()
                } else {
                    serde_json::json!({ "kind": detected.kind, "size": detected.size })
                };
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".into())
                );
                return Ok(ExitCode::SUCCESS);
            }
            println!("Type:      {}", detected.kind.display_name());
            println!("Size:      {}", fsutil::format_bytes(detected.size));
            if detected.kind == FileKind::Docx {
                let info = tdx_docx::inspect(&file)?;
                println!("Paragraphs: {}", info.paragraphs);
                println!("Headings:  {}", info.headings);
                println!("Words:     {}", info.words);
                println!("Chars:     {}", info.characters);
                if let Some(title) = info.title {
                    println!("Title:     {title}");
                }
                if let Some(author) = info.author {
                    println!("Author:    {author}");
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        DocCommand::LegacyImport { file, output } => run_and_report(
            ctx,
            "doc.legacy_import",
            std::slice::from_ref(&file),
            |_sink, _cancel| tdx_convert::legacy_import(&file, &output),
        ),
    }
}

pub fn run_convert(ctx: &Ctx, args: ConvertArgs) -> Result<ExitCode> {
    let ConvertArgs { file, output } = args;
    let detected = detect(&file)?;
    let target = output
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .ok_or_else(|| TdxError::InvalidInput("The output path needs a file extension".into()))?;

    match (&detected.kind, target.as_str()) {
        (FileKind::Csv | FileKind::Tsv, "xlsx") => {
            let options = tdx_sheet::csv_ops::CsvToXlsxOptions::default();
            run_and_report(
                ctx,
                "sheet.csv_to_xlsx",
                std::slice::from_ref(&file),
                |sink, cancel| {
                    tdx_sheet::csv_ops::csv_to_xlsx(&file, &output, &options, sink, cancel)
                },
            )
        }
        (FileKind::Csv | FileKind::Tsv, "json") => {
            let options = tdx_sheet::csv_ops::JsonOptions::default();
            run_and_report(
                ctx,
                "sheet.csv_to_json",
                std::slice::from_ref(&file),
                |sink, cancel| {
                    tdx_sheet::csv_ops::csv_to_json(&file, &output, &options, sink, cancel)
                },
            )
        }
        (FileKind::Xlsx | FileKind::Xls | FileKind::Ods, "csv") => {
            let options = tdx_sheet::csv_ops::XlsxToCsvOptions::default();
            run_and_report(
                ctx,
                "sheet.xlsx_to_csv",
                std::slice::from_ref(&file),
                |sink, cancel| {
                    tdx_sheet::csv_ops::xlsx_to_csv(&file, &output, &options, sink, cancel)
                },
            )
        }
        (FileKind::Json, "csv") => {
            let options = tdx_sheet::csv_ops::JsonOptions::default();
            run_and_report(
                ctx,
                "sheet.json_to_csv",
                std::slice::from_ref(&file),
                |sink, cancel| {
                    tdx_sheet::csv_ops::json_to_csv(&file, &output, &options, sink, cancel)
                },
            )
        }
        (kind, "pdf") if kind.is_image() => {
            let options = tdx_pdf::images::ImagesToPdfOptions::default();
            let inputs = vec![file.clone()];
            run_and_report(ctx, "pdf.images_to_pdf", &inputs, |sink, cancel| {
                tdx_pdf::images::images_to_pdf(&inputs, &output, &options, sink, cancel)
            })
        }
        (kind, format)
            if kind.is_image() && tdx_image::OutputFormat::from_extension(format).is_some() =>
        {
            let format = tdx_image::OutputFormat::from_extension(format).expect("checked");
            run_and_report(
                ctx,
                "image.convert",
                std::slice::from_ref(&file),
                |sink, cancel| tdx_image::convert(&file, &output, format, 92, None, sink, cancel),
            )
        }
        (FileKind::Svg, "png" | "jpg" | "webp") => {
            let raster = match target.as_str() {
                "jpg" => tdx_image::svg::RasterFormat::Jpeg,
                "webp" => tdx_image::svg::RasterFormat::Webp,
                _ => tdx_image::svg::RasterFormat::Png,
            };
            run_and_report(
                ctx,
                "image.svg_render",
                std::slice::from_ref(&file),
                |sink, cancel| {
                    tdx_image::svg::render_svg(
                        &file, &output, None, None, raster, 92, None, sink, cancel,
                    )
                },
            )
        }
        (FileKind::Pdf, "png" | "jpg" | "jpeg") => {
            let directory = output.parent().map(PathBuf::from).unwrap_or_default();
            let options = tdx_pdf::images::PdfToImagesOptions::default();
            run_and_report(
                ctx,
                "pdf.to_images",
                std::slice::from_ref(&file),
                |sink, cancel| {
                    tdx_pdf::images::pdf_to_images(&file, &directory, &options, sink, cancel)
                },
            )
        }
        _ => run_and_report(
            ctx,
            "convert.auto",
            std::slice::from_ref(&file),
            |sink, cancel| tdx_convert::convert_file(&file, &output, sink, cancel),
        ),
    }
}
