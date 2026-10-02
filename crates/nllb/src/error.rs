//! Errors of the NLLB translator.

use std::path::PathBuf;

/// A boxed error of a lower layer that has no error type of its own.
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// A failure of loading the model or of translating.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// `tokenizer.json` could not be read.
    #[error("cannot load the tokenizer {path}")]
    Tokenizer {
        /// The tokenizer file.
        path: PathBuf,
        /// The tokenizer error.
        #[source]
        source: BoxError,
    },
    /// The CTranslate2 model could not be loaded.
    #[error("cannot load the model from {path}")]
    Model {
        /// The model directory.
        path: PathBuf,
        /// The CTranslate2 error.
        #[source]
        source: BoxError,
    },
    /// A language code is not a token of the model.
    #[error("unknown language {code}")]
    Language {
        /// The code asked for.
        code: String,
    },
    /// A text could not be split into tokens.
    #[error("cannot encode the text")]
    Encode {
        /// The tokenizer error.
        #[source]
        source: BoxError,
    },
    /// The model failed to translate.
    #[error("translation failed")]
    Translate {
        /// The CTranslate2 error.
        #[source]
        source: BoxError,
    },
    /// The translated tokens could not be joined into text.
    #[error("cannot decode the translation")]
    Decode {
        /// The tokenizer error.
        #[source]
        source: BoxError,
    },
}

/// The result type of this crate.
pub type Result<T> = std::result::Result<T, Error>;
