//! PDF size optimization: downsample and re-encode embedded raster images.

use std::path::Path;

use image::{ExtendedColorType, GrayImage, ImageEncoder, RgbImage};
use lopdf::{Dictionary, Object, ObjectId};
use tdx_core::{
    detect::FileKind,
    error::Result,
    progress::{CancelToken, ProgressSink, Stage},
    result::OperationResult,
};

use crate::build::{load_pdf, prune_unreachable, save_document, verify_pdf};
use crate::CompressPreset;

fn filter_names(dict: &Dictionary) -> Vec<Vec<u8>> {
    match dict.get(b"Filter") {
        Ok(Object::Name(name)) => vec![name.clone()],
        Ok(Object::Array(items)) => items
            .iter()
            .filter_map(|item| item.as_name().ok().map(|n| n.to_vec()))
            .collect(),
        _ => vec![],
    }
}

fn dict_name(dict: &Dictionary, key: &[u8]) -> Option<Vec<u8>> {
    dict.get(key)
        .ok()
        .and_then(|object| object.as_name().ok().map(|n| n.to_vec()))
}

fn dict_int(dict: &Dictionary, key: &[u8]) -> Option<i64> {
    match dict.get(key).ok()? {
        Object::Integer(value) => Some(*value),
        Object::Real(value) => Some(*value as i64),
        _ => None,
    }
}

fn flatten_on_white(image: &image::DynamicImage) -> image::DynamicImage {
    if !image.color().has_alpha() {
        return image.clone();
    }
    let mut canvas = image::RgbaImage::from_pixel(
        image.width(),
        image.height(),
        image::Rgba([255, 255, 255, 255]),
    );
    image::imageops::overlay(&mut canvas, &image.to_rgba8(), 0, 0);
    image::DynamicImage::ImageRgba8(canvas)
}

struct Candidate {
    id: ObjectId,
    was_dct: bool,
    has_alpha: bool,
}

/// Compress embedded images with the selected preset.
pub fn compress(
    input: &Path,
    output: &Path,
    preset: CompressPreset,
    sink: &ProgressSink,
    cancel: &CancelToken,
) -> Result<OperationResult> {
    sink.stage(Stage::Reading, 0.0);
    cancel.check()?;
    let mut document = load_pdf(input)?;
    let bytes_before = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    let page_count = document.get_pages().len() as u32;

    let mut candidates: Vec<Candidate> = Vec::new();
    for (id, object) in document.objects.iter() {
        if let Object::Stream(stream) = object {
            let is_image =
                dict_name(&stream.dict, b"Subtype").as_deref() == Some(b"Image".as_slice());
            if !is_image {
                continue;
            }
            let filters = filter_names(&stream.dict);
            let is_dct = filters.iter().any(|f| f == b"DCTDecode");
            let is_flate = filters.iter().any(|f| f == b"FlateDecode");
            if !is_dct && !is_flate {
                continue;
            }
            let color = dict_name(&stream.dict, b"ColorSpace");
            let supported_color =
                matches!(color.as_deref(), Some(b"DeviceRGB") | Some(b"DeviceGray"));
            if !supported_color {
                continue;
            }
            if dict_int(&stream.dict, b"BitsPerComponent") != Some(8) {
                continue;
            }
            let has_alpha = stream.dict.has(b"SMask") || stream.dict.has(b"Mask");
            candidates.push(Candidate {
                id: *id,
                was_dct: is_dct,
                has_alpha,
            });
        }
    }

    let quality = preset.jpeg_quality();
    let max_dimension = preset.max_dimension();
    let total = candidates.len().max(1);
    let mut processed = 0usize;
    let mut skipped = 0usize;
    let mut flattened = 0usize;
    let mut bytes_reclaimed: u64 = 0;

    for (index, candidate) in candidates.iter().enumerate() {
        cancel.check()?;
        sink.stage(Stage::Processing, index as f32 / total as f32);

        let Some(Object::Stream(stream)) = document.objects.get(&candidate.id) else {
            continue;
        };
        let width = dict_int(&stream.dict, b"Width").unwrap_or(0) as u32;
        let height = dict_int(&stream.dict, b"Height").unwrap_or(0) as u32;
        if width == 0 || height == 0 {
            skipped += 1;
            continue;
        }
        let color_space = dict_name(&stream.dict, b"ColorSpace");
        let is_gray = color_space.as_deref() == Some(b"DeviceGray".as_slice());

        let decoded: image::DynamicImage = if candidate.was_dct {
            match image::load_from_memory(&stream.content) {
                Ok(image) => image,
                Err(_) => {
                    skipped += 1;
                    continue;
                }
            }
        } else {
            let raw = match stream.decompressed_content() {
                Ok(raw) => raw,
                Err(_) => {
                    skipped += 1;
                    continue;
                }
            };
            let channels: usize = if is_gray { 1 } else { 3 };
            let expected = width as usize * height as usize * channels;
            if raw.len() < expected {
                skipped += 1;
                continue;
            }
            if is_gray {
                match GrayImage::from_raw(width, height, raw[..expected].to_vec()) {
                    Some(buffer) => image::DynamicImage::ImageLuma8(buffer),
                    None => {
                        skipped += 1;
                        continue;
                    }
                }
            } else {
                match RgbImage::from_raw(width, height, raw[..expected].to_vec()) {
                    Some(buffer) => image::DynamicImage::ImageRgb8(buffer),
                    None => {
                        skipped += 1;
                        continue;
                    }
                }
            }
        };

        if candidate.has_alpha {
            flattened += 1;
        }
        let mut working = flatten_on_white(&decoded);
        let original_max = working.width().max(working.height());
        if original_max > max_dimension {
            let scale = max_dimension as f32 / original_max as f32;
            let new_width = ((working.width() as f32 * scale).round() as u32).max(1);
            let new_height = ((working.height() as f32 * scale).round() as u32).max(1);
            working =
                working.resize_exact(new_width, new_height, image::imageops::FilterType::Lanczos3);
        }

        let mut encoded: Vec<u8> = Vec::new();
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, quality);
        let encode_result = if is_gray {
            let gray = working.to_luma8();
            encoder.write_image(
                gray.as_raw(),
                gray.width(),
                gray.height(),
                ExtendedColorType::L8,
            )
        } else {
            let rgb = working.to_rgb8();
            encoder.write_image(
                rgb.as_raw(),
                rgb.width(),
                rgb.height(),
                ExtendedColorType::Rgb8,
            )
        };
        if encode_result.is_err() {
            skipped += 1;
            continue;
        }

        let unchanged_size = encoded.len() >= stream.content.len()
            && working.width() == width
            && working.height() == height;
        if unchanged_size {
            skipped += 1;
            continue;
        }

        let Some(Object::Stream(stream)) = document.objects.get_mut(&candidate.id) else {
            continue;
        };
        bytes_reclaimed = bytes_reclaimed
            .saturating_add(stream.content.len().saturating_sub(encoded.len()) as u64);
        let new_width = working.width();
        let new_height = working.height();
        stream.set_content(encoded);
        stream.allows_compression = false;
        let dict = &mut stream.dict;
        dict.set("Filter", Object::Name(b"DCTDecode".to_vec()));
        dict.set("Width", new_width as i64);
        dict.set("Height", new_height as i64);
        dict.set("BitsPerComponent", 8);
        dict.set(
            "ColorSpace",
            Object::Name(if is_gray {
                b"DeviceGray".to_vec()
            } else {
                b"DeviceRGB".to_vec()
            }),
        );
        dict.remove(b"DecodeParms");
        dict.remove(b"SMask");
        dict.remove(b"Mask");
        processed += 1;
    }

    if let Ok(root) = document.trailer.get(b"Root").and_then(Object::as_reference) {
        prune_unreachable(&mut document, &[root]);
    }
    sink.stage(Stage::Writing, 0.9);
    save_document(&mut document, output)?;

    let mut result = OperationResult::single(output.to_path_buf(), FileKind::Pdf);
    result.bytes_in = bytes_before;
    result.set_stat("pages", page_count);
    result.set_stat("images_reencoded", processed);
    result.set_stat("images_skipped", skipped);
    result.set_stat("bytes_reclaimed", bytes_reclaimed);
    if flattened > 0 {
        result.warn(format!(
            "{flattened} image(s) with transparency were flattened onto a white background"
        ));
    }
    if processed == 0 {
        result.warn(
            "No embedded images could be compressed further; the document structure was kept",
        );
    }
    for warning in verify_pdf(output, page_count as usize)? {
        result.warn(warning);
    }
    // Guarantee the pre-flight check reported by the UI matches reality.
    let bytes_after = std::fs::metadata(output).map(|m| m.len()).unwrap_or(0);
    if bytes_after >= bytes_before {
        result.warn(
            "The output is not smaller than the input; the original may already be optimized",
        );
    }
    Ok(result.finalize())
}
