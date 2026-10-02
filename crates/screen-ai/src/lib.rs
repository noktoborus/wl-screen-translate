//! Text recognition through the Chrome Screen AI library.
//!
//! The library is the component Chrome downloads for its own OCR. It has no
//! public interface; the calls and the bitmap layout used here are the ones
//! Chromium uses in `services/screen_ai/screen_ai_library_wrapper_impl.cc`.
#![deny(missing_docs)]

mod bitmap;
mod error;
mod files;
mod proto;

use std::ffi::c_char;
use std::path::Path;

use image::RgbaImage;
use image::imageops::FilterType;
use libloading::Library;
use prost::Message;

pub use error::{Error, Result};

/// The file name of the library in the component directory.
#[cfg(target_os = "linux")]
pub const LIBRARY_NAME: &str = "libchromescreenai.so";
/// The file name of the library in the component directory.
#[cfg(target_os = "windows")]
pub const LIBRARY_NAME: &str = "chrome_screen_ai.dll";

/// The largest side `PerformOCR` accepts when the library does not say.
const DEFAULT_MAX_DIMENSION: u32 = 2048;

type SizeFn = extern "C" fn(*const c_char) -> u32;
type ContentFn = extern "C" fn(*const c_char, u32, *mut c_char);
type SetFileContentFunctionsFn = unsafe extern "C" fn(SizeFn, ContentFn);
type InitFn = unsafe extern "C" fn() -> bool;
type SetLightModeFn = unsafe extern "C" fn(bool);
type MaxDimensionFn = unsafe extern "C" fn() -> u32;
type PerformOcrFn = unsafe extern "C" fn(*const bitmap::SkBitmap, *mut u32) -> *mut c_char;
type FreeFn = unsafe extern "C" fn(*mut c_char);

/// One recognised line, in pixels of the image given to [`Engine::recognize`].
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// The text of the line.
    pub text: String,
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
    /// The block the line belongs to.
    pub block: i32,
    /// The paragraph of the block the line belongs to.
    pub paragraph: i32,
}

/// A loaded and initialised library.
pub struct Engine {
    perform: PerformOcrFn,
    free: FreeFn,
    max_dimension: u32,
    _library: Library,
}

impl Engine {
    /// Loads the library and its models from a component directory.
    ///
    /// One process serves one directory: the library keeps global state.
    pub fn load(directory: &Path) -> Result<Self> {
        files::set_directory(directory).map_err(|loaded| Error::OtherDirectory { loaded })?;
        let path = directory.join(LIBRARY_NAME);
        let library = unsafe { Library::new(&path) }.map_err(|source| Error::Load {
            path: path.clone(),
            source,
        })?;
        let set_files: SetFileContentFunctionsFn = symbol(&library, "SetFileContentFunctions")?;
        let init: InitFn = symbol(&library, "InitOCRUsingCallback")?;
        let set_light: SetLightModeFn = symbol(&library, "SetOCRLightMode")?;
        let max_dimension: MaxDimensionFn = symbol(&library, "GetMaxImageDimension")?;
        let perform = symbol(&library, "PerformOCR")?;
        let free = symbol(&library, "FreeLibraryAllocatedCharArray")?;
        unsafe { set_files(files::file_size, files::file_content) };
        if !unsafe { init() } {
            return Err(Error::Init);
        }
        unsafe { set_light(false) };
        let max_dimension = match unsafe { max_dimension() } {
            0 => DEFAULT_MAX_DIMENSION,
            value => value,
        };
        Ok(Self {
            perform,
            free,
            max_dimension,
            _library: library,
        })
    }

    /// Recognises the text of an image.
    pub fn recognize(&self, image: &RgbaImage) -> Result<Vec<Line>> {
        let longest = image.width().max(image.height());
        let scale = if longest > self.max_dimension {
            self.max_dimension as f32 / longest as f32
        } else {
            1.0
        };
        let scaled;
        let input = if scale < 1.0 {
            let width = ((image.width() as f32 * scale) as u32).max(1);
            let height = ((image.height() as f32 * scale) as u32).max(1);
            scaled = image::imageops::resize(image, width, height, FilterType::Triangle);
            &scaled
        } else {
            image
        };
        let bytes = self.perform(input)?;
        let annotation = proto::VisualAnnotation::decode(bytes.as_slice())
            .map_err(|source| Error::Decode { source })?;
        Ok(annotation
            .lines
            .into_iter()
            .filter(|line| !line.utf8_string.trim().is_empty())
            .map(|line| {
                let rect = line.bounding_box.unwrap_or_default();
                Line {
                    text: line.utf8_string.trim().to_owned(),
                    x: rect.x as f32 / scale,
                    y: rect.y as f32 / scale,
                    width: rect.width as f32 / scale,
                    height: rect.height as f32 / scale,
                    block: line.block_id,
                    paragraph: line.paragraph_id,
                }
            })
            .collect())
    }

    fn perform(&self, image: &RgbaImage) -> Result<Vec<u8>> {
        let bitmap = bitmap::Bitmap::new(image.as_raw(), image.width(), image.height());
        let mut length = 0u32;
        let output = unsafe { (self.perform)(bitmap.as_ptr(), &mut length) };
        if output.is_null() {
            return Err(Error::Recognize);
        }
        let bytes =
            unsafe { std::slice::from_raw_parts(output.cast::<u8>(), length as usize) }.to_vec();
        unsafe { (self.free)(output) };
        Ok(bytes)
    }
}

fn symbol<T: Copy>(library: &Library, name: &'static str) -> Result<T> {
    let found = unsafe { library.get::<T>(name.as_bytes()) }
        .map_err(|source| Error::Symbol { name, source })?;
    Ok(*found)
}
