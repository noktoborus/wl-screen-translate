//! Errors of the Tesseract wrapper.

use std::path::PathBuf;

/// A failure of loading the library or of recognising an image.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// No file name of the library could be loaded.
    #[error("cannot load {name}")]
    Load {
        /// The last file name tried.
        name: &'static str,
        /// The loader error.
        #[source]
        source: libloading::Error,
    },
    /// The library does not export a function this crate calls.
    #[error("missing symbol {name}")]
    Symbol {
        /// The name of the function.
        name: &'static str,
        /// The loader error.
        #[source]
        source: libloading::Error,
    },
    /// The model of a language is not in the model directory.
    #[error("missing {path}")]
    Missing {
        /// The model file looked for.
        path: PathBuf,
    },
    /// `TessBaseAPIInit3` failed for a language.
    #[error("cannot initialise {language} from {directory}")]
    Init {
        /// The model directory.
        directory: PathBuf,
        /// The language.
        language: String,
    },
    /// `TessBaseAPIRecognize` failed.
    #[error("recognition failed")]
    Recognize,
}

/// The result type of this crate.
pub type Result<T> = std::result::Result<T, Error>;
