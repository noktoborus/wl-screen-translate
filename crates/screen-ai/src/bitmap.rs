//! The in-memory layout of the Skia `SkBitmap` that `PerformOCR` reads.
//!
//! Only the fields the library reads are filled. The pixel reference is a stub
//! whose one purpose is to be non-null, because `SkBitmap::isNull` tests it.

use std::ffi::c_void;
use std::marker::PhantomData;

/// `kRGBA_8888_SkColorType`.
const COLOR_TYPE_RGBA: i32 = 4;
/// `kOpaque_SkAlphaType`.
const ALPHA_TYPE_OPAQUE: i32 = 1;
/// The bytes of one RGBA pixel.
const PIXEL_BYTES: usize = 4;

/// An empty virtual table for the stub pixel reference; the library calls none of it.
static STUB_VTABLE: [usize; 16] = [0; 16];

#[repr(C)]
struct SkImageInfo {
    color_space: *const c_void,
    color_type: i32,
    alpha_type: i32,
    width: i32,
    height: i32,
}

#[repr(C)]
struct SkPixmap {
    pixels: *const c_void,
    row_bytes: usize,
    info: SkImageInfo,
}

/// The `SkBitmap` as the library was compiled against it.
#[repr(C)]
pub struct SkBitmap {
    pixel_ref: *const StubPixelRef,
    pixmap: SkPixmap,
    flags: u32,
}

#[repr(C)]
struct StubPixelRef {
    vtable: *const [usize; 16],
    ref_count: i32,
    padding: i32,
    width: i32,
    height: i32,
    pixels: *const c_void,
    row_bytes: usize,
    reserve: [u8; 64],
}

/// A bitmap borrowing the pixels of an RGBA image.
pub struct Bitmap<'a> {
    raw: SkBitmap,
    _pixel_ref: Box<StubPixelRef>,
    _pixels: PhantomData<&'a [u8]>,
}

impl<'a> Bitmap<'a> {
    /// Describes `pixels`, rows of `width` RGBA pixels, `height` rows.
    pub fn new(pixels: &'a [u8], width: u32, height: u32) -> Self {
        let row_bytes = width as usize * PIXEL_BYTES;
        let data = pixels.as_ptr().cast::<c_void>();
        let pixel_ref = Box::new(StubPixelRef {
            vtable: &STUB_VTABLE,
            ref_count: 1,
            padding: 0,
            width: width as i32,
            height: height as i32,
            pixels: data,
            row_bytes,
            reserve: [0; 64],
        });
        let raw = SkBitmap {
            pixel_ref: &*pixel_ref,
            pixmap: SkPixmap {
                pixels: data,
                row_bytes,
                info: SkImageInfo {
                    color_space: std::ptr::null(),
                    color_type: COLOR_TYPE_RGBA,
                    alpha_type: ALPHA_TYPE_OPAQUE,
                    width: width as i32,
                    height: height as i32,
                },
            },
            flags: 0,
        };
        Self {
            raw,
            _pixel_ref: pixel_ref,
            _pixels: PhantomData,
        }
    }

    /// The pointer `PerformOCR` takes.
    pub fn as_ptr(&self) -> *const SkBitmap {
        &self.raw
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_matches_the_library() {
        assert_eq!(std::mem::offset_of!(SkBitmap, pixmap), 8);
        assert_eq!(
            std::mem::offset_of!(SkBitmap, pixmap) + std::mem::offset_of!(SkPixmap, info),
            24
        );
        assert_eq!(std::mem::offset_of!(SkImageInfo, color_type), 8);
        assert_eq!(std::mem::offset_of!(SkImageInfo, width), 16);
        assert_eq!(std::mem::offset_of!(SkBitmap, flags), 48);
    }
}
