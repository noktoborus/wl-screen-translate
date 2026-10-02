//! The callbacks through which the library reads its model files.
//!
//! The library opens no file itself. It asks the host for the size and then
//! the content of each file, by a path relative to the component directory.

use std::ffi::{CStr, c_char};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The component directory, fixed by the first engine loaded in the process.
static DIRECTORY: OnceLock<PathBuf> = OnceLock::new();

/// Sets the component directory; returns the one already set when it differs.
pub fn set_directory(directory: &Path) -> Result<(), PathBuf> {
    let set = DIRECTORY.get_or_init(|| directory.to_path_buf());
    if set == directory {
        Ok(())
    } else {
        Err(set.clone())
    }
}

fn resolve(relative: *const c_char) -> Option<PathBuf> {
    if relative.is_null() {
        return None;
    }
    let relative = unsafe { CStr::from_ptr(relative) }.to_str().ok()?;
    Some(DIRECTORY.get()?.join(relative))
}

/// `GetFileContentSizeFn`: the size of a file, zero when it cannot be read.
pub extern "C" fn file_size(relative: *const c_char) -> u32 {
    resolve(relative)
        .and_then(|path| std::fs::metadata(path).ok())
        .map_or(0, |meta| meta.len() as u32)
}

/// `GetFileContentFn`: copies a file into a buffer of `size` bytes.
pub extern "C" fn file_content(relative: *const c_char, size: u32, buffer: *mut c_char) {
    let Some(content) = resolve(relative).and_then(|path| std::fs::read(path).ok()) else {
        return;
    };
    let count = content.len().min(size as usize);
    unsafe { std::ptr::copy_nonoverlapping(content.as_ptr(), buffer.cast::<u8>(), count) };
}
