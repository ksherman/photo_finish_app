use std::ffi::c_void;
use std::fs;
use std::os::raw::{c_char, c_int};
use std::path::Path;
use std::ptr;
use std::time::Instant;

use super::{
    fit_dimensions, GeneratedDerivatives, StageTimingsMs, PREVIEW_MAX_EDGE, PREVIEW_QUALITY,
    THUMBNAIL_MAX_EDGE, THUMBNAIL_QUALITY,
};
use crate::db::{BUNDLE_DIR_NAME, PREVIEWS_DIR_NAME, THUMBNAILS_DIR_NAME};
use crate::{Error, Result};

type CFTypeRef = *const c_void;
type CFAllocatorRef = *const c_void;
type CFStringRef = *const c_void;
type CFURLRef = *const c_void;
type CFDictionaryRef = *const c_void;
type CFNumberRef = *const c_void;
type CFBooleanRef = *const c_void;
type CGImageSourceRef = *const c_void;
type CGImageDestinationRef = *const c_void;
type CGImageRef = *const c_void;
type CGColorSpaceRef = *const c_void;
type CGContextRef = *const c_void;

const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
const K_CF_NUMBER_SINT32_TYPE: c_int = 3;
const K_CF_NUMBER_FLOAT64_TYPE: c_int = 6;
const K_CG_IMAGE_ALPHA_NONE_SKIP_LAST: u32 = 5;
const K_CG_BITMAP_BYTE_ORDER_DEFAULT: u32 = 0;
const K_CG_INTERPOLATION_HIGH: c_int = 3;

#[repr(C)]
#[derive(Copy, Clone)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct CGSize {
    width: f64,
    height: f64,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct CGRect {
    origin: CGPoint,
    size: CGSize,
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    static kCFBooleanTrue: CFBooleanRef;
    static kCFTypeDictionaryKeyCallBacks: c_void;
    static kCFTypeDictionaryValueCallBacks: c_void;
    fn CFRelease(cf: CFTypeRef);
    fn CFStringCreateWithBytes(
        alloc: CFAllocatorRef,
        bytes: *const u8,
        num_bytes: isize,
        encoding: u32,
        is_external: bool,
    ) -> CFStringRef;
    fn CFURLCreateFromFileSystemRepresentation(
        alloc: CFAllocatorRef,
        buffer: *const c_char,
        buf_len: isize,
        is_directory: bool,
    ) -> CFURLRef;
    fn CFNumberCreate(
        alloc: CFAllocatorRef,
        the_type: c_int,
        value_ptr: *const c_void,
    ) -> CFNumberRef;
    fn CFNumberGetValue(
        number: CFNumberRef,
        the_type: c_int,
        value_ptr: *mut c_void,
    ) -> bool;
    fn CFDictionaryCreate(
        alloc: CFAllocatorRef,
        keys: *const CFTypeRef,
        values: *const CFTypeRef,
        num_values: isize,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> CFDictionaryRef;
    fn CFDictionaryGetValue(dict: CFDictionaryRef, key: CFTypeRef) -> CFTypeRef;
}

#[link(name = "ImageIO", kind = "framework")]
extern "C" {
    static kCGImageSourceCreateThumbnailFromImageAlways: CFStringRef;
    static kCGImageSourceCreateThumbnailWithTransform: CFStringRef;
    static kCGImageSourceThumbnailMaxPixelSize: CFStringRef;
    static kCGImageSourceShouldCacheImmediately: CFStringRef;
    static kCGImageDestinationLossyCompressionQuality: CFStringRef;
    static kCGImagePropertyPixelWidth: CFStringRef;
    static kCGImagePropertyPixelHeight: CFStringRef;
    static kCGImagePropertyOrientation: CFStringRef;

    fn CGImageSourceCreateWithURL(url: CFURLRef, options: CFDictionaryRef) -> CGImageSourceRef;
    fn CGImageSourceCopyPropertiesAtIndex(
        isrc: CGImageSourceRef,
        index: usize,
        options: CFDictionaryRef,
    ) -> CFDictionaryRef;
    fn CGImageSourceCreateThumbnailAtIndex(
        isrc: CGImageSourceRef,
        index: usize,
        options: CFDictionaryRef,
    ) -> CGImageRef;
    fn CGImageDestinationCreateWithURL(
        url: CFURLRef,
        type_uti: CFStringRef,
        count: usize,
        options: CFDictionaryRef,
    ) -> CGImageDestinationRef;
    fn CGImageDestinationAddImage(
        idst: CGImageDestinationRef,
        image: CGImageRef,
        properties: CFDictionaryRef,
    );
    fn CGImageDestinationFinalize(idst: CGImageDestinationRef) -> bool;
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGImageRelease(image: CGImageRef);
    fn CGImageGetWidth(image: CGImageRef) -> usize;
    fn CGImageGetHeight(image: CGImageRef) -> usize;
    fn CGColorSpaceCreateDeviceRGB() -> CGColorSpaceRef;
    fn CGColorSpaceRelease(space: CGColorSpaceRef);
    fn CGBitmapContextCreate(
        data: *mut c_void,
        width: usize,
        height: usize,
        bits_per_component: usize,
        bytes_per_row: usize,
        space: CGColorSpaceRef,
        bitmap_info: u32,
    ) -> CGContextRef;
    fn CGContextSetInterpolationQuality(c: CGContextRef, quality: c_int);
    fn CGContextDrawImage(c: CGContextRef, rect: CGRect, image: CGImageRef);
    fn CGBitmapContextCreateImage(c: CGContextRef) -> CGImageRef;
    fn CGContextRelease(c: CGContextRef);
}

struct CfGuard(CFTypeRef);
impl Drop for CfGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) };
        }
    }
}

struct CgImageGuard(CGImageRef);
impl Drop for CgImageGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CGImageRelease(self.0) };
        }
    }
}

fn make_cf_url(path: &Path) -> Result<CfGuard> {
    let bytes = path.as_os_str().as_encoded_bytes();
    let url = unsafe {
        CFURLCreateFromFileSystemRepresentation(
            ptr::null(),
            bytes.as_ptr() as *const c_char,
            bytes.len() as isize,
            false,
        )
    };
    if url.is_null() {
        Err(Error::Io(format!("invalid path for CFURL: {}", path.display())))
    } else {
        Ok(CfGuard(url))
    }
}

fn make_cf_string(s: &str) -> Result<CfGuard> {
    let cf = unsafe {
        CFStringCreateWithBytes(
            ptr::null(),
            s.as_ptr(),
            s.len() as isize,
            K_CF_STRING_ENCODING_UTF8,
            false,
        )
    };
    if cf.is_null() {
        Err(Error::Image("failed to allocate CFString".into()))
    } else {
        Ok(CfGuard(cf))
    }
}

fn get_dict_i32(dict: CFDictionaryRef, key: CFStringRef) -> Option<i32> {
    unsafe {
        let val = CFDictionaryGetValue(dict, key as CFTypeRef);
        if val.is_null() {
            return None;
        }
        let mut out: i32 = 0;
        CFNumberGetValue(
            val as CFNumberRef,
            K_CF_NUMBER_SINT32_TYPE,
            &mut out as *mut i32 as *mut c_void,
        )
        .then_some(out)
    }
}

fn write_cg_jpeg(cg_img: CGImageRef, dst: &Path, quality: f64) -> Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| Error::Io(format!("{}: {}", parent.display(), e)))?;
    }
    let uti = make_cf_string("public.jpeg")?;
    let url = make_cf_url(dst)?;
    let dest = CfGuard(unsafe { CGImageDestinationCreateWithURL(url.0, uti.0, 1, ptr::null()) });
    if dest.0.is_null() {
        return Err(Error::Image(format!(
            "CGImageDestinationCreateWithURL failed for {}",
            dst.display()
        )));
    }

    let q_num = CfGuard(unsafe {
        CFNumberCreate(
            ptr::null(),
            K_CF_NUMBER_FLOAT64_TYPE,
            &quality as *const f64 as *const c_void,
        )
    });
    let keys = unsafe { [kCGImageDestinationLossyCompressionQuality as CFTypeRef] };
    let vals = [q_num.0];
    let props = CfGuard(unsafe {
        CFDictionaryCreate(
            ptr::null(),
            keys.as_ptr(),
            vals.as_ptr(),
            1,
            &kCFTypeDictionaryKeyCallBacks,
            &kCFTypeDictionaryValueCallBacks,
        )
    });

    let ok = unsafe {
        CGImageDestinationAddImage(dest.0, cg_img, props.0);
        CGImageDestinationFinalize(dest.0)
    };
    if !ok {
        return Err(Error::Image(format!(
            "CGImageDestinationFinalize failed for {}",
            dst.display()
        )));
    }
    Ok(())
}

fn downscale_cg_image(src: CGImageRef, dst_w: usize, dst_h: usize) -> Result<CgImageGuard> {
    unsafe {
        let space = CGColorSpaceCreateDeviceRGB();
        if space.is_null() {
            return Err(Error::Image("CGColorSpaceCreateDeviceRGB failed".into()));
        }
        let ctx = CGBitmapContextCreate(
            ptr::null_mut(),
            dst_w,
            dst_h,
            8,
            dst_w * 4,
            space,
            K_CG_IMAGE_ALPHA_NONE_SKIP_LAST | K_CG_BITMAP_BYTE_ORDER_DEFAULT,
        );
        CGColorSpaceRelease(space);
        if ctx.is_null() {
            return Err(Error::Image("CGBitmapContextCreate failed".into()));
        }

        CGContextSetInterpolationQuality(ctx, K_CG_INTERPOLATION_HIGH);
        let rect = CGRect {
            origin: CGPoint { x: 0.0, y: 0.0 },
            size: CGSize {
                width: dst_w as f64,
                height: dst_h as f64,
            },
        };
        CGContextDrawImage(ctx, rect, src);
        let out = CGBitmapContextCreateImage(ctx);
        CGContextRelease(ctx);
        if out.is_null() {
            return Err(Error::Image("CGBitmapContextCreateImage failed".into()));
        }
        Ok(CgImageGuard(out))
    }
}

pub fn generate_derivatives_imageio(
    event_root: &Path,
    photo_id: &str,
    source_rel_path: &str,
) -> Result<GeneratedDerivatives> {
    let total_start = Instant::now();
    let source_path = event_root.join(source_rel_path);
    if !source_path.is_file() {
        return Err(Error::Io(format!("file not found: {}", source_path.display())));
    }

    let s0 = Instant::now();
    let url = make_cf_url(&source_path)?;
    let src = CfGuard(unsafe { CGImageSourceCreateWithURL(url.0, ptr::null()) });
    if src.0.is_null() {
        return Err(Error::Image(format!(
            "CGImageSourceCreateWithURL failed: {}",
            source_path.display()
        )));
    }

    let (mut orig_w, mut orig_h) = (0u32, 0u32);
    let props = CfGuard(unsafe { CGImageSourceCopyPropertiesAtIndex(src.0, 0, ptr::null()) });
    if !props.0.is_null() {
        let raw_w = unsafe { get_dict_i32(props.0, kCGImagePropertyPixelWidth) }.unwrap_or(0);
        let raw_h = unsafe { get_dict_i32(props.0, kCGImagePropertyPixelHeight) }.unwrap_or(0);
        let orient = unsafe { get_dict_i32(props.0, kCGImagePropertyOrientation) }.unwrap_or(1);
        if matches!(orient, 5..=8) {
            orig_w = raw_h.max(0) as u32;
            orig_h = raw_w.max(0) as u32;
        } else {
            orig_w = raw_w.max(0) as u32;
            orig_h = raw_h.max(0) as u32;
        }
    }

    let max_px = PREVIEW_MAX_EDGE as i32;
    let max_num = CfGuard(unsafe {
        CFNumberCreate(
            ptr::null(),
            K_CF_NUMBER_SINT32_TYPE,
            &max_px as *const i32 as *const c_void,
        )
    });
    let keys = unsafe {
        [
            kCGImageSourceCreateThumbnailFromImageAlways as CFTypeRef,
            kCGImageSourceCreateThumbnailWithTransform as CFTypeRef,
            kCGImageSourceThumbnailMaxPixelSize as CFTypeRef,
            kCGImageSourceShouldCacheImmediately as CFTypeRef,
        ]
    };
    let vals = unsafe {
        [
            kCFBooleanTrue as CFTypeRef,
            kCFBooleanTrue as CFTypeRef,
            max_num.0,
            kCFBooleanTrue as CFTypeRef,
        ]
    };
    let opts = CfGuard(unsafe {
        CFDictionaryCreate(
            ptr::null(),
            keys.as_ptr(),
            vals.as_ptr(),
            4,
            &kCFTypeDictionaryKeyCallBacks,
            &kCFTypeDictionaryValueCallBacks,
        )
    });

    let prev_img = CgImageGuard(unsafe { CGImageSourceCreateThumbnailAtIndex(src.0, 0, opts.0) });
    if prev_img.0.is_null() {
        return Err(Error::Image(format!(
            "CGImageSourceCreateThumbnailAtIndex failed: {}",
            source_path.display()
        )));
    }
    let pw = unsafe { CGImageGetWidth(prev_img.0) } as u32;
    let ph = unsafe { CGImageGetHeight(prev_img.0) } as u32;
    if orig_w == 0 || orig_h == 0 {
        orig_w = pw;
        orig_h = ph;
    }
    let decode_preview_ms = s0.elapsed().as_secs_f64() * 1000.0;

    let thumb_rel = format!("{}/{}/{}.jpg", BUNDLE_DIR_NAME, THUMBNAILS_DIR_NAME, photo_id);
    let prev_rel = format!("{}/{}/{}.jpg", BUNDLE_DIR_NAME, PREVIEWS_DIR_NAME, photo_id);

    let s1 = Instant::now();
    write_cg_jpeg(prev_img.0, &event_root.join(&prev_rel), PREVIEW_QUALITY as f64 / 100.0)?;
    let encode_preview_ms = s1.elapsed().as_secs_f64() * 1000.0;

    let s2 = Instant::now();
    let (tw, th) = fit_dimensions(pw, ph, THUMBNAIL_MAX_EDGE);
    if tw == pw && th == ph {
        write_cg_jpeg(prev_img.0, &event_root.join(&thumb_rel), THUMBNAIL_QUALITY as f64 / 100.0)?;
    } else {
        let thumb_img = downscale_cg_image(prev_img.0, tw as usize, th as usize)?;
        write_cg_jpeg(thumb_img.0, &event_root.join(&thumb_rel), THUMBNAIL_QUALITY as f64 / 100.0)?;
    }
    let thumb_ms = s2.elapsed().as_secs_f64() * 1000.0;

    Ok(GeneratedDerivatives {
        thumbnail_rel_path: thumb_rel,
        preview_rel_path: prev_rel,
        original_width: orig_w,
        original_height: orig_h,
        timings: StageTimingsMs {
            decode_preview_ms,
            encode_preview_ms,
            thumb_ms,
            total_ms: total_start.elapsed().as_secs_f64() * 1000.0,
        },
    })
}
