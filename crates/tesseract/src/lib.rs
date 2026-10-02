//! Text recognition through the Tesseract library, loaded at run time from
//! the system: its C API, as `tesseract/capi.h` declares it.
#![deny(missing_docs)]

mod error;
mod prepare;

use std::ffi::{CStr, CString, c_char, c_float, c_int, c_uchar, c_void};
use std::path::{Path, PathBuf};

use image::RgbaImage;
use libloading::Library;

pub use error::{Error, Result};

/// The file names the library is looked for under, in order.
#[cfg(target_os = "linux")]
pub const LIBRARY_NAMES: &[&str] = &["libtesseract.so.5", "libtesseract.so"];
/// The file names the library is looked for under, in order.
#[cfg(target_os = "windows")]
pub const LIBRARY_NAMES: &[&str] = &["libtesseract-5.dll", "tesseract55.dll", "tesseract.dll"];
/// The extension of a language model.
pub const MODEL_EXTENSION: &str = "traineddata";
/// Models of a model directory that are not languages.
const NOT_LANGUAGES: &[&str] = &["osd", "equ"];
/// Where distributions put the models, looked in when the directory given
/// has none.
const SYSTEM_DIRECTORIES: &[&str] = &[
    "/usr/share/tesseract/tessdata",
    "/usr/share/tesseract-ocr/5/tessdata",
    "/usr/share/tesseract-ocr/4.00/tessdata",
    "/usr/share/tessdata",
    "/usr/local/share/tessdata",
];
/// The levels of the page iterator.
const BLOCK: c_int = 0;
const PARAGRAPH: c_int = 1;
const LINE: c_int = 2;
/// Page segmentation: a page of blocks and columns, without orientation.
const SEGMENT_AUTO: c_int = 3;
/// A line read with a lower confidence, out of 100, is dropped: mostly
/// icons and lines read as letters.
const MIN_CONFIDENCE: f32 = 30.0;

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
    /// The paragraph the line belongs to, counted over the page.
    pub paragraph: i32,
}

/// The model directory to use: `preferred` if it holds a model, else that
/// of `TESSDATA_PREFIX` or of the distribution.
pub fn model_directory(preferred: &Path) -> Option<PathBuf> {
    let prefix = std::env::var_os("TESSDATA_PREFIX").map(PathBuf::from);
    std::iter::once(preferred.to_path_buf())
        .chain(prefix)
        .chain(SYSTEM_DIRECTORIES.iter().map(PathBuf::from))
        .find(|directory| !languages(directory).is_empty())
}

/// The languages of the models in `directory`, such as `rus` or `chi_sim`,
/// sorted.
pub fn languages(directory: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut found: Vec<String> = entries
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            (path.extension()? == MODEL_EXTENSION).then_some(())?;
            let language = path.file_stem()?.to_str()?.to_owned();
            (!NOT_LANGUAGES.contains(&language.as_str())).then_some(language)
        })
        .collect();
    found.sort();
    found
}

type Handle = *mut c_void;

/// The functions of the C API this crate calls.
struct Api {
    version: unsafe extern "C" fn() -> *const c_char,
    create: unsafe extern "C" fn() -> Handle,
    delete: unsafe extern "C" fn(Handle),
    init: unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> c_int,
    set_mode: unsafe extern "C" fn(Handle, c_int),
    set_image: unsafe extern "C" fn(Handle, *const c_uchar, c_int, c_int, c_int, c_int),
    set_resolution: unsafe extern "C" fn(Handle, c_int),
    recognize: unsafe extern "C" fn(Handle, *mut c_void) -> c_int,
    clear: unsafe extern "C" fn(Handle),
    iterator: unsafe extern "C" fn(Handle) -> Handle,
    iterator_delete: unsafe extern "C" fn(Handle),
    page_iterator: unsafe extern "C" fn(Handle) -> Handle,
    text: unsafe extern "C" fn(Handle, c_int) -> *mut c_char,
    confidence: unsafe extern "C" fn(Handle, c_int) -> c_float,
    delete_text: unsafe extern "C" fn(*const c_char),
    bounding_box: unsafe extern "C" fn(
        Handle,
        c_int,
        *mut c_int,
        *mut c_int,
        *mut c_int,
        *mut c_int,
    ) -> c_int,
    beginning_of: unsafe extern "C" fn(Handle, c_int) -> c_int,
    next: unsafe extern "C" fn(Handle, c_int) -> c_int,
}

impl Api {
    fn load(library: &Library) -> Result<Self> {
        Ok(Self {
            version: symbol(library, "TessVersion")?,
            create: symbol(library, "TessBaseAPICreate")?,
            delete: symbol(library, "TessBaseAPIDelete")?,
            init: symbol(library, "TessBaseAPIInit3")?,
            set_mode: symbol(library, "TessBaseAPISetPageSegMode")?,
            set_image: symbol(library, "TessBaseAPISetImage")?,
            set_resolution: symbol(library, "TessBaseAPISetSourceResolution")?,
            recognize: symbol(library, "TessBaseAPIRecognize")?,
            clear: symbol(library, "TessBaseAPIClear")?,
            iterator: symbol(library, "TessBaseAPIGetIterator")?,
            iterator_delete: symbol(library, "TessResultIteratorDelete")?,
            page_iterator: symbol(library, "TessResultIteratorGetPageIterator")?,
            text: symbol(library, "TessResultIteratorGetUTF8Text")?,
            confidence: symbol(library, "TessResultIteratorConfidence")?,
            delete_text: symbol(library, "TessDeleteText")?,
            bounding_box: symbol(library, "TessPageIteratorBoundingBox")?,
            beginning_of: symbol(library, "TessPageIteratorIsAtBeginningOf")?,
            next: symbol(library, "TessPageIteratorNext")?,
        })
    }
}

fn symbol<T: Copy>(library: &Library, name: &'static str) -> Result<T> {
    let found = unsafe { library.get::<T>(name.as_bytes()) }
        .map_err(|source| Error::Symbol { name, source })?;
    Ok(*found)
}

/// The library, and a handle initialised for one language at a time.
pub struct Engine {
    api: Api,
    handle: Handle,
    directory: PathBuf,
    /// The language the handle is initialised for.
    language: Option<String>,
    _library: Library,
}

// The handle is used by one thread at a time, as `&mut self` ensures.
unsafe impl Send for Engine {}

impl Engine {
    /// Loads the library of the system; the models are read from
    /// `directory` when a language is first recognised.
    pub fn load(directory: &Path) -> Result<Self> {
        let mut error = None;
        let library = LIBRARY_NAMES.iter().find_map(|name| {
            unsafe { Library::new(*name) }
                .map_err(|source| error = Some(Error::Load { name, source }))
                .ok()
        });
        let Some(library) = library else {
            return Err(error.expect("a library name is tried"));
        };
        let api = Api::load(&library)?;
        let handle = unsafe { (api.create)() };
        Ok(Self {
            api,
            handle,
            directory: directory.to_path_buf(),
            language: None,
            _library: library,
        })
    }

    /// The version of the library, such as `5.3.0`.
    pub fn version(&self) -> String {
        let version = unsafe { (self.api.version)() };
        if version.is_null() {
            return String::new();
        }
        unsafe { CStr::from_ptr(version) }
            .to_string_lossy()
            .into_owned()
    }

    /// Recognises the text of an image written in `language`, a model name
    /// such as `rus`; models may be joined, as `eng+rus`.
    pub fn recognize(&mut self, image: &RgbaImage, language: &str) -> Result<Vec<Line>> {
        self.initialise(language)?;
        let prepared = prepare::grey(image);
        let (width, height) = (prepared.image.width(), prepared.image.height());
        unsafe {
            (self.api.set_mode)(self.handle, SEGMENT_AUTO);
            (self.api.set_image)(
                self.handle,
                prepared.image.as_raw().as_ptr(),
                width as c_int,
                height as c_int,
                1,
                width as c_int,
            );
            (self.api.set_resolution)(self.handle, prepare::RESOLUTION);
        }
        if unsafe { (self.api.recognize)(self.handle, std::ptr::null_mut()) } != 0 {
            unsafe { (self.api.clear)(self.handle) };
            return Err(Error::Recognize);
        }
        let lines = self.lines(prepared.scale);
        unsafe { (self.api.clear)(self.handle) };
        Ok(lines)
    }

    /// Initialises the handle for `language` unless it is already.
    fn initialise(&mut self, language: &str) -> Result<()> {
        if self.language.as_deref() == Some(language) {
            return Ok(());
        }
        for name in language.split('+') {
            let path = self.directory.join(format!("{name}.{MODEL_EXTENSION}"));
            if !path.is_file() {
                return Err(Error::Missing { path });
            }
        }
        let init = || Error::Init {
            directory: self.directory.clone(),
            language: language.to_owned(),
        };
        let directory =
            CString::new(self.directory.to_string_lossy().as_bytes()).map_err(|_| init())?;
        let name = CString::new(language).map_err(|_| init())?;
        self.language = None;
        if unsafe { (self.api.init)(self.handle, directory.as_ptr(), name.as_ptr()) } != 0 {
            return Err(init());
        }
        self.language = Some(language.to_owned());
        Ok(())
    }

    /// The lines of the last recognition, their boxes divided by `scale`.
    fn lines(&self, scale: f32) -> Vec<Line> {
        let api = &self.api;
        let iterator = unsafe { (api.iterator)(self.handle) };
        if iterator.is_null() {
            return Vec::new();
        }
        let page = unsafe { (api.page_iterator)(iterator) };
        let mut lines = Vec::new();
        let (mut block, mut paragraph) = (-1, -1);
        loop {
            if unsafe { (api.beginning_of)(page, BLOCK) } != 0 {
                block += 1;
            }
            if unsafe { (api.beginning_of)(page, PARAGRAPH) } != 0 {
                paragraph += 1;
            }
            let text = unsafe { (api.text)(iterator, LINE) };
            if !text.is_null() {
                let read = unsafe { CStr::from_ptr(text) }
                    .to_string_lossy()
                    .trim()
                    .to_owned();
                unsafe { (api.delete_text)(text) };
                let confidence = unsafe { (api.confidence)(iterator, LINE) };
                let [mut left, mut top, mut right, mut bottom] = [0; 4];
                let boxed = unsafe {
                    (api.bounding_box)(page, LINE, &mut left, &mut top, &mut right, &mut bottom)
                } != 0;
                if boxed && !read.is_empty() && confidence >= MIN_CONFIDENCE {
                    lines.push(Line {
                        text: read,
                        x: left as f32 / scale,
                        y: top as f32 / scale,
                        width: (right - left) as f32 / scale,
                        height: (bottom - top) as f32 / scale,
                        block,
                        paragraph,
                    });
                }
            }
            if unsafe { (api.next)(page, LINE) } == 0 {
                break;
            }
        }
        unsafe { (api.iterator_delete)(iterator) };
        lines
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        unsafe { (self.api.delete)(self.handle) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn languages_are_the_models_but_osd() {
        let temporary = tempfile::tempdir().unwrap();
        let dir = temporary.path().to_path_buf();
        for name in [
            "rus.traineddata",
            "eng.traineddata",
            "osd.traineddata",
            "notes.txt",
        ] {
            std::fs::write(dir.join(name), b"").unwrap();
        }
        assert_eq!(languages(&dir), ["eng", "rus"]);
        assert_eq!(model_directory(&dir), Some(dir));
    }
}
