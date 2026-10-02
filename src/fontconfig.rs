//! The few calls of the fontconfig library that find a font for a character.
//!
//! The library is loaded at run time: the program starts without it, and
//! fontconfig answers from the font cache the system keeps, without reading a
//! font file.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::path::PathBuf;

use libloading::Library;

/// The library of every Linux desktop.
const LIBRARY_NAME: &str = "libfontconfig.so.1";

/// `FcMatchPattern`: the substitutions of a pattern before a match.
const MATCH_PATTERN: c_int = 0;
/// `FcResultMatch`.
const RESULT_MATCH: c_int = 0;
/// `FcTrue` and `FcFalse`.
const TRUE: c_int = 1;
const FALSE: c_int = 0;

/// The formats egui reads: TrueType and OpenType outlines.
const READABLE_FORMATS: &[&str] = &["TrueType", "CFF"];

type Config = c_void;
type Pattern = c_void;
type CharSet = c_void;

type InitFn = unsafe extern "C" fn() -> *mut Config;
type ConfigDestroyFn = unsafe extern "C" fn(*mut Config);
type PatternCreateFn = unsafe extern "C" fn() -> *mut Pattern;
type PatternDestroyFn = unsafe extern "C" fn(*mut Pattern);
type AddStringFn = unsafe extern "C" fn(*mut Pattern, *const c_char, *const u8) -> c_int;
type AddBoolFn = unsafe extern "C" fn(*mut Pattern, *const c_char, c_int) -> c_int;
type AddCharSetFn = unsafe extern "C" fn(*mut Pattern, *const c_char, *const CharSet) -> c_int;
type GetStringFn =
    unsafe extern "C" fn(*const Pattern, *const c_char, c_int, *mut *mut u8) -> c_int;
type GetIntegerFn = unsafe extern "C" fn(*const Pattern, *const c_char, c_int, *mut c_int) -> c_int;
type GetCharSetFn =
    unsafe extern "C" fn(*const Pattern, *const c_char, c_int, *mut *mut CharSet) -> c_int;
type CharSetCreateFn = unsafe extern "C" fn() -> *mut CharSet;
type CharSetDestroyFn = unsafe extern "C" fn(*mut CharSet);
type CharSetAddFn = unsafe extern "C" fn(*mut CharSet, u32) -> c_int;
type CharSetHasFn = unsafe extern "C" fn(*const CharSet, u32) -> c_int;
type SubstituteFn = unsafe extern "C" fn(*mut Config, *mut Pattern, c_int) -> c_int;
type DefaultSubstituteFn = unsafe extern "C" fn(*mut Pattern);
type FontMatchFn = unsafe extern "C" fn(*mut Config, *mut Pattern, *mut c_int) -> *mut Pattern;

/// The library and its configuration with the font cache.
pub struct Fontconfig {
    config: *mut Config,
    config_destroy: ConfigDestroyFn,
    pattern_create: PatternCreateFn,
    pattern_destroy: PatternDestroyFn,
    add_string: AddStringFn,
    add_bool: AddBoolFn,
    add_char_set: AddCharSetFn,
    get_string: GetStringFn,
    get_integer: GetIntegerFn,
    get_char_set: GetCharSetFn,
    char_set_create: CharSetCreateFn,
    char_set_destroy: CharSetDestroyFn,
    char_set_add: CharSetAddFn,
    char_set_has: CharSetHasFn,
    substitute: SubstituteFn,
    default_substitute: DefaultSubstituteFn,
    font_match: FontMatchFn,
    _library: Library,
}

/// A font fontconfig matched; it lives as long as the match.
pub struct Face<'a> {
    fontconfig: &'a Fontconfig,
    pattern: *mut Pattern,
    /// The font file.
    pub path: PathBuf,
    /// The face in the file.
    pub index: u32,
}

impl Fontconfig {
    /// Loads the library and the configuration of the system.
    pub fn load() -> Result<Self, String> {
        let library = unsafe { Library::new(LIBRARY_NAME) }
            .map_err(|error| format!("{LIBRARY_NAME}: {error}"))?;
        let init: InitFn = symbol(&library, "FcInitLoadConfigAndFonts")?;
        let mut fontconfig = Self {
            config: std::ptr::null_mut(),
            config_destroy: symbol(&library, "FcConfigDestroy")?,
            pattern_create: symbol(&library, "FcPatternCreate")?,
            pattern_destroy: symbol(&library, "FcPatternDestroy")?,
            add_string: symbol(&library, "FcPatternAddString")?,
            add_bool: symbol(&library, "FcPatternAddBool")?,
            add_char_set: symbol(&library, "FcPatternAddCharSet")?,
            get_string: symbol(&library, "FcPatternGetString")?,
            get_integer: symbol(&library, "FcPatternGetInteger")?,
            get_char_set: symbol(&library, "FcPatternGetCharSet")?,
            char_set_create: symbol(&library, "FcCharSetCreate")?,
            char_set_destroy: symbol(&library, "FcCharSetDestroy")?,
            char_set_add: symbol(&library, "FcCharSetAddChar")?,
            char_set_has: symbol(&library, "FcCharSetHasChar")?,
            substitute: symbol(&library, "FcConfigSubstitute")?,
            default_substitute: symbol(&library, "FcDefaultSubstitute")?,
            font_match: symbol(&library, "FcFontMatch")?,
            _library: library,
        };
        fontconfig.config = unsafe { init() };
        if fontconfig.config.is_null() {
            return Err("no fontconfig configuration".into());
        }
        Ok(fontconfig)
    }

    /// The sans-serif face, in a format egui reads, the configuration
    /// prefers for `letter`, if any has it.
    pub fn face_for(&self, letter: char) -> Option<Face<'_>> {
        unsafe {
            let pattern = (self.pattern_create)();
            let set = (self.char_set_create)();
            (self.char_set_add)(set, letter as u32);
            (self.add_char_set)(pattern, c"charset".as_ptr(), set);
            (self.char_set_destroy)(set);
            (self.add_string)(pattern, c"family".as_ptr(), c"sans-serif".as_ptr().cast());
            (self.add_bool)(pattern, c"scalable".as_ptr(), TRUE);
            (self.add_bool)(pattern, c"color".as_ptr(), FALSE);
            (self.substitute)(self.config, pattern, MATCH_PATTERN);
            (self.default_substitute)(pattern);
            let mut result = 0;
            let matched = (self.font_match)(self.config, pattern, &mut result);
            (self.pattern_destroy)(pattern);
            if matched.is_null() {
                return None;
            }
            let mut index = 0;
            (self.get_integer)(matched, c"index".as_ptr(), 0, &mut index);
            let path = self.string(matched, c"file");
            let format = self.string(matched, c"fontformat");
            let face = Face {
                fontconfig: self,
                pattern: matched,
                path: path.unwrap_or_default().into(),
                // The high bits name an instance of a variable font.
                index: (index as u32) & 0xffff,
            };
            let readable = format.is_some_and(|format| READABLE_FORMATS.contains(&format.as_str()));
            (readable && !face.path.as_os_str().is_empty() && face.has(letter)).then_some(face)
        }
    }
}

impl Fontconfig {
    /// The first string `name` of `pattern`.
    fn string(&self, pattern: *const Pattern, name: &CStr) -> Option<String> {
        let mut value = std::ptr::null_mut();
        let found = unsafe { (self.get_string)(pattern, name.as_ptr(), 0, &mut value) };
        (found == RESULT_MATCH && !value.is_null()).then(|| {
            unsafe { CStr::from_ptr(value.cast()) }
                .to_string_lossy()
                .into_owned()
        })
    }
}

impl Drop for Fontconfig {
    fn drop(&mut self) {
        if !self.config.is_null() {
            unsafe { (self.config_destroy)(self.config) };
        }
    }
}

impl Face<'_> {
    /// Whether the face has a glyph for `letter`.
    pub fn has(&self, letter: char) -> bool {
        let fontconfig = self.fontconfig;
        let mut set = std::ptr::null_mut();
        unsafe {
            (fontconfig.get_char_set)(self.pattern, c"charset".as_ptr(), 0, &mut set)
                == RESULT_MATCH
                && (fontconfig.char_set_has)(set, letter as u32) == TRUE
        }
    }
}

impl Drop for Face<'_> {
    fn drop(&mut self) {
        unsafe { (self.fontconfig.pattern_destroy)(self.pattern) };
    }
}

fn symbol<T: Copy>(library: &Library, name: &str) -> Result<T, String> {
    unsafe { library.get::<T>(name.as_bytes()) }
        .map(|symbol| *symbol)
        .map_err(|error| format!("{name}: {error}"))
}
