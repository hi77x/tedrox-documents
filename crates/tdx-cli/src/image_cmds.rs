//! Image subcommands.

use std::process::ExitCode;

use tdx_core::error::{Result, TdxError};

use crate::support::{parse_background, Ctx};
use crate::{run_and_report, ImageArgs, ImageCommand};

fn parse_format(value: &str) -> Result<tdx_image::OutputFormat> {
    tdx_image::OutputFormat::from_extension(value)
        .ok_or_else(|| TdxError::InvalidInput(format!("Unsupported image format: {value}")))
}

pub fn run(ctx: &Ctx, args: ImageArgs) -> Result<ExitCode> {
    match args.command {
        ImageCommand::Convert {
            file,
            output,
            to,
            quality,
            background,
        } => {
            let format = parse_format(&to)?;
            let background = background.as_deref().map(parse_background).transpose()?;
            run_and_report(
                ctx,
                "image.convert",
                std::slice::from_ref(&file),
                |sink, cancel| {
                    tdx_image::convert(&file, &output, format, quality, background, sink, cancel)
                },
            )
        }
        ImageCommand::Resize {
            file,
            output,
            width,
            height,
            mode,
            to,
        } => {
            let format = match to {
                Some(value) => parse_format(&value)?,
                None => preserve_format(&file)?,
            };
            let mode = match mode.as_str() {
                "cover" => tdx_image::ResizeMode::Cover,
                "exact" => tdx_image::ResizeMode::Exact,
                _ => tdx_image::ResizeMode::Contain,
            };
            run_and_report(
                ctx,
                "image.resize",
                std::slice::from_ref(&file),
                |sink, cancel| {
                    tdx_image::resize(
                        &file, &output, width, height, mode, format, 92, sink, cancel,
                    )
                },
            )
        }
        ImageCommand::Crop {
            file,
            output,
            x,
            y,
            width,
            height,
            to,
        } => {
            let format = match to {
                Some(value) => parse_format(&value)?,
                None => preserve_format(&file)?,
            };
            run_and_report(
                ctx,
                "image.crop",
                std::slice::from_ref(&file),
                |sink, cancel| {
                    tdx_image::crop(
                        &file, &output, x, y, width, height, format, 92, sink, cancel,
                    )
                },
            )
        }
        ImageCommand::Rotate {
            file,
            output,
            degrees,
            flip_horizontal,
            flip_vertical,
        } => run_and_report(
            ctx,
            "image.rotate_flip",
            std::slice::from_ref(&file),
            |sink, cancel| {
                tdx_image::rotate_flip(
                    &file,
                    &output,
                    degrees,
                    flip_horizontal,
                    flip_vertical,
                    None,
                    sink,
                    cancel,
                )
            },
        ),
        ImageCommand::Strip { file, output } => run_and_report(
            ctx,
            "image.strip_metadata",
            std::slice::from_ref(&file),
            |sink, cancel| tdx_image::strip_metadata(&file, &output, sink, cancel),
        ),
        ImageCommand::SvgRender {
            file,
            output,
            width,
            height,
            to,
        } => {
            let format = match to.as_str() {
                "jpg" | "jpeg" => tdx_image::svg::RasterFormat::Jpeg,
                "webp" => tdx_image::svg::RasterFormat::Webp,
                _ => tdx_image::svg::RasterFormat::Png,
            };
            run_and_report(
                ctx,
                "image.svg_render",
                std::slice::from_ref(&file),
                |sink, cancel| {
                    tdx_image::svg::render_svg(
                        &file, &output, width, height, format, 92, None, sink, cancel,
                    )
                },
            )
        }
        ImageCommand::SvgEmbed { file, output } => run_and_report(
            ctx,
            "image.png_to_svg_embed",
            std::slice::from_ref(&file),
            |sink, cancel| tdx_image::svg::png_to_svg_embed(&file, &output, sink, cancel),
        ),
        ImageCommand::Trace {
            file,
            output,
            preset,
            speckle,
            color_precision,
        } => {
            let preset = match preset.as_str() {
                "bw" => tdx_image::svg::TracePreset::Bw,
                "photo" => tdx_image::svg::TracePreset::Photo,
                _ => tdx_image::svg::TracePreset::Poster,
            };
            let options = tdx_image::svg::TraceOptions {
                preset,
                filter_speckle: speckle,
                color_precision,
                ..tdx_image::svg::TraceOptions::default()
            };
            run_and_report(
                ctx,
                "image.png_to_svg_trace",
                std::slice::from_ref(&file),
                |sink, cancel| tdx_image::svg::trace(&file, &output, &options, sink, cancel),
            )
        }
        ImageCommand::Ico { file, output, size } => run_and_report(
            ctx,
            "image.to_ico",
            std::slice::from_ref(&file),
            |sink, cancel| tdx_image::to_ico(&file, &output, size, sink, cancel),
        ),
    }
}

fn preserve_format(path: &std::path::Path) -> Result<tdx_image::OutputFormat> {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("png");
    tdx_image::OutputFormat::from_extension(extension)
        .ok_or_else(|| TdxError::InvalidInput("Specify --to for this input format".into()))
}
