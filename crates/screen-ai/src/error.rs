//! Errors of the Screen AI wrapper.

use std::path::PathBuf;

/// A failure of loading the library or of recognising an image.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The shared library could not be loaded.
    #[error("cannot load {path}")]
    Load {
        /// The library file that was opened.
        path: PathBuf,
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
    /// An engine was already loaded from another directory in this process.
    #[error("engine already loaded from {loaded}")]
    OtherDirectory {
        /// The directory the engine was loaded from first.
        loaded: PathBuf,
    },
    /// `InitOCRUsingCallback` returned false, usually a model file is missing.
    #[error("the library failed to initialise")]
    Init,
    /// `PerformOCR` returned no result.
    #[error("the library returned no result")]
    Recognize,
    /// The result is not a `VisualAnnotation` message.
    #[error("cannot decode the result")]
    Decode {
        /// The protobuf error.
        #[source]
        source: prost::DecodeError,
    },
}

/// The result type of this crate.
pub type Result<T> = std::result::Result<T, Error>;
