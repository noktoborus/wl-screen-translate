//! Errors of the PaddleOCR engine.

use std::path::PathBuf;

/// A failure of loading a model or of recognising an image.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// ONNX Runtime could not be loaded.
    #[error("cannot load ONNX Runtime")]
    Runtime {
        /// The loader error.
        #[source]
        source: ort::LoadDynamicError,
    },
    /// No recogniser reads the script.
    #[error("no recogniser reads {script}")]
    Script {
        /// The ISO 15924 code of the script.
        script: String,
    },
    /// A model file or an alphabet is missing.
    #[error("missing {path}")]
    Missing {
        /// The file looked for.
        path: PathBuf,
    },
    /// An alphabet could not be read.
    #[error("cannot read {path}")]
    Alphabet {
        /// The alphabet file.
        path: PathBuf,
        /// The file system error.
        #[source]
        source: std::io::Error,
    },
    /// ONNX Runtime could not load a model.
    #[error("cannot load {path}")]
    Load {
        /// The model file.
        path: PathBuf,
        /// The runtime error.
        #[source]
        source: ort::Error,
    },
    /// ONNX Runtime failed to run a model.
    #[error("inference failed")]
    Run {
        /// The runtime error.
        #[source]
        source: ort::Error,
    },
    /// A model answered with a shape this crate does not read.
    #[error("unexpected output shape {shape:?}")]
    Shape {
        /// The shape of the output.
        shape: Vec<i64>,
    },
}

impl From<ort::Error> for Error {
    fn from(source: ort::Error) -> Self {
        Self::Run { source }
    }
}

/// The result type of this crate.
pub type Result<T> = std::result::Result<T, Error>;
