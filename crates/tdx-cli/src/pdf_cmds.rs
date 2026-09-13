//! PDF subcommands.

use std::process::ExitCode;

use tdx_core::error::Result;

use crate::support::{Ctx, OutputFormat};
use crate::{run_and_report, PdfArgs, PdfCommand};

pub fn run(ctx: &Ctx, args: PdfArgs) -> Result<ExitCode> {
    match args.command {
        PdfCommand::Merge { files, output } => {
            run_and_report(ctx, "pdf.merge", &files, |sink, cancel| {
                tdx_pdf::pages::merge(&files, &output, sink, cancel)
            })
        }
        PdfCommand::Split {
            file,
            pages,
            chunk,
            out_dir,
        } => {
            let mode = if let Some(chunk) = chunk {
                tdx_pdf::SplitMode::Chunks(chunk)
            } else if let Some(pages) = pages {
                let selection = tdx_pdf::PageSelection::parse(&pages)?;
                let resolved = selection.resolve(first_page_count(&file)?)?;
                tdx_pdf::SplitMode::Ranges(ranges_from_pages(&resolved))
            } else {
                tdx_pdf::SplitMode::EveryPage
            };
            run_and_report(
                ctx,
                "pdf.split",
                std::slice::from_ref(&file),
                |sink, cancel| tdx_pdf::pages::split(&file, &out_dir, &mode, sink, cancel),
            )
        }
        PdfCommand::Extract {
            file,
            pages,
            output,
        } => {
            let selection = tdx_pdf::PageSelection::parse(&pages)?;
            run_and_report(
                ctx,
                "pdf.extract",
                std::slice::from_ref(&file),
                |sink, cancel| {
                    tdx_pdf::pages::extract_pages(&file, &output, &selection, sink, cancel)
                },
            )
        }
        PdfCommand::Delete {
            file,
            pages,
            output,
        } => {
            let selection = tdx_pdf::PageSelection::parse(&pages)?;
            run_and_report(
                ctx,
                "pdf.delete_pages",
                std::slice::from_ref(&file),
                |sink, cancel| {
                    tdx_pdf::pages::delete_pages(&file, &output, &selection, sink, cancel)
                },
            )
        }
        PdfCommand::Reverse { file, output } => run_and_report(
            ctx,
            "pdf.reverse",
            std::slice::from_ref(&file),
            |sink, cancel| tdx_pdf::pages::reverse(&file, &output, sink, cancel),
        ),
        PdfCommand::Reorder {
            file,
            order,
            output,
        } => {
            let order = crate::support::parse_page_list(&order)?;
            run_and_report(
                ctx,
                "pdf.reorder",
                std::slice::from_ref(&file),
                |sink, cancel| tdx_pdf::pages::reorder(&file, &output, &order, sink, cancel),
            )
        }
        PdfCommand::Rotate {
            file,
            degrees,
            pages,
            output,
        } => {
            let selection = pages
                .as_deref()
                .map(tdx_pdf::PageSelection::parse)
                .transpose()?;
            run_and_report(
                ctx,
                "pdf.rotate",
                std::slice::from_ref(&file),
                |sink, cancel| {
                    tdx_pdf::pages::rotate(
                        &file,
                        &output,
                        degrees,
                        selection.as_ref(),
                        sink,
                        cancel,
                    )
                },
            )
        }
        PdfCommand::OddEven { file, even, output } => run_and_report(
            ctx,
            "pdf.odd_even",
            std::slice::from_ref(&file),
            |sink, cancel| tdx_pdf::pages::extract_odd_even(&file, &output, !even, sink, cancel),
        ),
        PdfCommand::Insert {
            base,
            insert,
            after,
            output,
        } => {
            let inputs = vec![base.clone(), insert.clone()];
            run_and_report(ctx, "pdf.insert", &inputs, |sink, cancel| {
                tdx_pdf::pages::insert(&base, &insert, &output, after, sink, cancel)
            })
        }
        PdfCommand::Metadata { file } => {
            let metadata = tdx_pdf::metadata::read_metadata(&file)?;
            if ctx.format == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&metadata).unwrap_or_else(|_| "{}".into())
                );
            } else {
                println!(
                    "Title:            {}",
                    metadata.title.as_deref().unwrap_or("-")
                );
                println!(
                    "Author:           {}",
                    metadata.author.as_deref().unwrap_or("-")
                );
                println!(
                    "Subject:          {}",
                    metadata.subject.as_deref().unwrap_or("-")
                );
                println!(
                    "Keywords:         {}",
                    metadata.keywords.as_deref().unwrap_or("-")
                );
                println!(
                    "Creator:          {}",
                    metadata.creator.as_deref().unwrap_or("-")
                );
                println!(
                    "Producer:         {}",
                    metadata.producer.as_deref().unwrap_or("-")
                );
                println!(
                    "Created:          {}",
                    metadata.creation_date.as_deref().unwrap_or("-")
                );
                println!(
                    "Modified:         {}",
                    metadata.modification_date.as_deref().unwrap_or("-")
                );
                println!("Pages:            {}", metadata.page_count);
                println!("PDF version:      {}", metadata.version);
                println!(
                    "File size:        {}",
                    tdx_core::fsutil::format_bytes(metadata.file_size)
                );
                println!("XMP metadata:     {}", yes_no(metadata.has_xmp));
                println!("JavaScript:       {}", yes_no(metadata.has_javascript));
                println!("Embedded files:   {}", yes_no(metadata.has_embedded_files));
            }
            Ok(ExitCode::SUCCESS)
        }
        PdfCommand::Clean { file, output } => run_and_report(
            ctx,
            "pdf.metadata_clean",
            std::slice::from_ref(&file),
            |sink, cancel| tdx_pdf::metadata::privacy_clean(&file, &output, sink, cancel),
        ),
        PdfCommand::SetMetadata {
            file,
            output,
            title,
            author,
            subject,
            keywords,
        } => {
            let update = tdx_pdf::MetadataUpdate {
                title,
                author,
                subject,
                keywords,
            };
            run_and_report(
                ctx,
                "pdf.metadata_set",
                std::slice::from_ref(&file),
                |sink, cancel| {
                    tdx_pdf::metadata::set_metadata(&file, &output, &update, sink, cancel)
                },
            )
        }
        PdfCommand::Compress {
            file,
            output,
            preset,
            quality,
            max_dimension,
        } => {
            let preset = if quality.is_some() || max_dimension.is_some() {
                tdx_pdf::CompressPreset::Custom {
                    quality: quality.unwrap_or(75),
                    max_dimension: max_dimension.unwrap_or(1600),
                }
            } else {
                match preset.as_str() {
                    "screen" => tdx_pdf::CompressPreset::Screen,
                    "print" => tdx_pdf::CompressPreset::Print,
                    _ => tdx_pdf::CompressPreset::Balanced,
                }
            };
            run_and_report(
                ctx,
                "pdf.compress",
                std::slice::from_ref(&file),
                |sink, cancel| tdx_pdf::compress::compress(&file, &output, preset, sink, cancel),
            )
        }
        PdfCommand::Watermark {
            file,
            output,
            text,
            size,
            opacity,
            angle,
        } => {
            let options = tdx_pdf::watermark::WatermarkOptions {
                text,
                font_size: size,
                opacity,
                angle,
                color: (0.5, 0.5, 0.5),
            };
            run_and_report(
                ctx,
                "pdf.watermark",
                std::slice::from_ref(&file),
                |sink, cancel| {
                    tdx_pdf::watermark::watermark(&file, &output, &options, sink, cancel)
                },
            )
        }
        PdfCommand::ToImages {
            file,
            out_dir,
            format,
            dpi,
        } => {
            let raster = match format.as_str() {
                "jpg" | "jpeg" => tdx_pdf::RasterFormat::Jpeg,
                _ => tdx_pdf::RasterFormat::Png,
            };
            let options = tdx_pdf::images::PdfToImagesOptions {
                format: raster,
                dpi,
                quality: 90,
                pages: None,
            };
            run_and_report(
                ctx,
                "pdf.to_images",
                std::slice::from_ref(&file),
                |sink, cancel| {
                    tdx_pdf::images::pdf_to_images(&file, &out_dir, &options, sink, cancel)
                },
            )
        }
        PdfCommand::FromImages {
            files,
            output,
            page_size,
            margin,
        } => {
            let page_size = match page_size.as_str() {
                "letter" => tdx_pdf::PageSize::Letter,
                "fit" => tdx_pdf::PageSize::Fit,
                _ => tdx_pdf::PageSize::A4,
            };
            let options = tdx_pdf::images::ImagesToPdfOptions {
                page_size,
                margin,
                jpeg_quality: 88,
            };
            run_and_report(ctx, "pdf.images_to_pdf", &files, |sink, cancel| {
                tdx_pdf::images::images_to_pdf(&files, &output, &options, sink, cancel)
            })
        }
        PdfCommand::Inspect { file } => {
            let info = tdx_pdf::inspect(&file)?;
            if ctx.format == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&info).unwrap_or_else(|_| "{}".into())
                );
            } else {
                println!("Pages:        {}", info.page_count);
                println!("Version:      {}", info.version);
                println!("Objects:      {}", info.object_count);
                println!("Encrypted:    {}", yes_no(info.encrypted));
                println!(
                    "File size:    {}",
                    tdx_core::fsutil::format_bytes(info.file_size)
                );
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

fn first_page_count(path: &std::path::Path) -> Result<u32> {
    let document = tdx_pdf::load_pdf(path)?;
    Ok(document.get_pages().len() as u32)
}

fn ranges_from_pages(pages: &[u32]) -> Vec<(u32, u32)> {
    let mut ranges: Vec<(u32, u32)> = Vec::new();
    for page in pages {
        match ranges.last_mut() {
            Some((_, end)) if *end + 1 == *page => *end = *page,
            _ => ranges.push((*page, *page)),
        }
    }
    ranges
}
