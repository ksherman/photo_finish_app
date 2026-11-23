use fast_image_resize::images::{Image, ImageRef};
use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};
use image::codecs::jpeg::{JpegDecoder, JpegEncoder};
use image::metadata::Orientation;
use image::{DynamicImage, ExtendedColorType, ImageDecoder};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::Path;

use crate::db::{BUNDLE_DIR_NAME, PREVIEWS_DIR_NAME, THUMBNAILS_DIR_NAME};
use crate::event::EventBundle;
use crate::{Error, Result};

pub const THUMBNAIL_MAX_EDGE: u32 = 320;
pub const PREVIEW_MAX_EDGE: u32 = 1280;
pub const THUMBNAIL_QUALITY: u8 = 82;
pub const PREVIEW_QUALITY: u8 = 86;

#[derive(Debug, Clone)]
pub struct GeneratedDerivatives {
    pub thumbnail_rel_path: String,
    pub preview_rel_path: String,
    pub original_width: u32,
    pub original_height: u32,
}

/// Decodes a JPEG with EXIF orientation applied and generates both a 320px thumbnail
/// and a 1280px preview using SIMD-accelerated resizing (`fast_image_resize`).
/// Pure Rust — zero external libvips / C library packaging dependencies.
pub fn generate_derivatives(
    event_root: &Path,
    photo_id: &str,
    source_rel_path: &str,
) -> Result<GeneratedDerivatives> {
    let source_path = event_root.join(source_rel_path);
    let file =
        File::open(&source_path).map_err(|e| Error::Io(format!("{}: {}", source_path.display(), e)))?;
    let mut decoder =
        JpegDecoder::new(BufReader::new(file)).map_err(|e| Error::Image(e.to_string()))?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);

    let mut img = DynamicImage::from_decoder(decoder).map_err(|e| Error::Image(e.to_string()))?;
    img.apply_orientation(orientation);

    let rgb = img.to_rgb8();
    let (orig_w, orig_h) = rgb.dimensions();

    let thumb_rel = format!("{}/{}/{}.jpg", BUNDLE_DIR_NAME, THUMBNAILS_DIR_NAME, photo_id);
    let prev_rel = format!("{}/{}/{}.jpg", BUNDLE_DIR_NAME, PREVIEWS_DIR_NAME, photo_id);

    write_resized_jpeg(
        rgb.as_raw(),
        orig_w,
        orig_h,
        THUMBNAIL_MAX_EDGE,
        THUMBNAIL_QUALITY,
        &event_root.join(&thumb_rel),
    )?;
    write_resized_jpeg(
        rgb.as_raw(),
        orig_w,
        orig_h,
        PREVIEW_MAX_EDGE,
        PREVIEW_QUALITY,
        &event_root.join(&prev_rel),
    )?;

    Ok(GeneratedDerivatives {
        thumbnail_rel_path: thumb_rel,
        preview_rel_path: prev_rel,
        original_width: orig_w,
        original_height: orig_h,
    })
}

/// Processes a photo record in `bundle`, updating its status to `ready` or `error`.
pub fn process_photo_in_bundle(
    bundle: &EventBundle,
    photo_id: &str,
    relative_path: &str,
) -> Result<GeneratedDerivatives> {
    match generate_derivatives(bundle.root(), photo_id, relative_path) {
        Ok(deriv) => {
            bundle.mark_photo_ready(
                photo_id,
                &deriv.thumbnail_rel_path,
                &deriv.preview_rel_path,
                deriv.original_width,
                deriv.original_height,
            )?;
            Ok(deriv)
        }
        Err(err) => {
            let _ = bundle.mark_photo_error(photo_id, &err.to_string());
            Err(err)
        }
    }
}

fn write_resized_jpeg(
    src_rgb: &[u8],
    src_w: u32,
    src_h: u32,
    max_edge: u32,
    quality: u8,
    output_path: &Path,
) -> Result<()> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|e| Error::Io(format!("{}: {}", parent.display(), e)))?;
    }

    let (dst_w, dst_h) = fit_dimensions(src_w, src_h, max_edge);

    let encoded_buf: Vec<u8> = if dst_w == src_w && dst_h == src_h {
        src_rgb.to_vec()
    } else {
        let src_view = ImageRef::new(src_w, src_h, src_rgb, PixelType::U8x3)
            .map_err(|e| Error::Image(e.to_string()))?;
        let mut dst_image = Image::new(dst_w, dst_h, PixelType::U8x3);
        let mut resizer = Resizer::new();
        let options = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3));
        resizer
            .resize(&src_view, &mut dst_image, Some(&options))
            .map_err(|e| Error::Image(e.to_string()))?;
        dst_image.into_vec()
    };

    let out_file = File::create(output_path)
        .map_err(|e| Error::Io(format!("{}: {}", output_path.display(), e)))?;
    let mut writer = BufWriter::new(out_file);
    let mut encoder = JpegEncoder::new_with_quality(&mut writer, quality);
    encoder
        .encode(&encoded_buf, dst_w, dst_h, ExtendedColorType::Rgb8)
        .map_err(|e| Error::Image(e.to_string()))?;

    Ok(())
}

fn fit_dimensions(w: u32, h: u32, max_edge: u32) -> (u32, u32) {
    let longest = w.max(h);
    if longest <= max_edge || longest == 0 {
        return (w.max(1), h.max(1));
    }
    let scale = max_edge as f64 / longest as f64;
    let new_w = ((w as f64 * scale).round() as u32).max(1);
    let new_h = ((h as f64 * scale).round() as u32).max(1);
    (new_w, new_h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};
    use tempfile::tempdir;

    #[test]
    fn generates_thumbnail_and_preview_from_jpeg() {
        let dir = tempdir().unwrap();
        let rel = "Gym A/Session 1A/sample.jpg";
        let src_path = dir.path().join(rel);
        fs::create_dir_all(src_path.parent().unwrap()).unwrap();

        let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_fn(2000, 1000, |x, y| Rgb([(x % 255) as u8, (y % 255) as u8, 128]));
        img.save(&src_path).unwrap();

        let deriv = generate_derivatives(dir.path(), "pho_test0001aa", rel).unwrap();
        assert_eq!(deriv.original_width, 2000);
        assert_eq!(deriv.original_height, 1000);

        let thumb = image::open(dir.path().join(&deriv.thumbnail_rel_path)).unwrap();
        assert_eq!(thumb.width(), 320);
        assert_eq!(thumb.height(), 160);

        let prev = image::open(dir.path().join(&deriv.preview_rel_path)).unwrap();
        assert_eq!(prev.width(), 1280);
        assert_eq!(prev.height(), 640);
    }
}
