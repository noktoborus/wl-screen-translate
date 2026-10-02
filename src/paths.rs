//! Where the settings and the model files live.

use std::path::PathBuf;

use directories::ProjectDirs;

use crate::APP_ID;
use crate::error::{AppError, Result};

/// The settings file in the configuration directory.
const SETTINGS_FILE: &str = "settings.yaml";
/// The Screen AI component directory in the data directory.
const SCREEN_AI_DIR: &str = "screen-ai";
/// The ONNX Runtime and PaddleOCR model directory in the data directory.
const PADDLE_OCR_DIR: &str = "paddle-ocr";
/// The Tesseract model directory in the data directory.
const TESSERACT_DIR: &str = "tesseract";
/// The NLLB model directory in the data directory.
const NLLB_DIR: &str = "nllb";
/// The translation cache directory in the temporary directory.
const TRANSLATIONS_DIR: &str = "translations";

/// The resolved locations.
#[derive(Debug, Clone)]
pub struct Paths {
    /// The settings file.
    pub settings: PathBuf,
    /// The Screen AI library and its models.
    pub screen_ai: PathBuf,
    /// ONNX Runtime and the PaddleOCR models.
    pub paddle_ocr: PathBuf,
    /// The Tesseract models, unless the system's are used.
    pub tesseract: PathBuf,
    /// The CTranslate2 NLLB model and its tokenizer.
    pub nllb: PathBuf,
    /// The translations already made, see [`crate::cache`].
    pub translations: PathBuf,
}

impl Paths {
    /// Resolves the platform directories of the application.
    pub fn resolve() -> Result<Self> {
        let dirs = ProjectDirs::from("", "", APP_ID).ok_or(AppError::NoHome)?;
        Ok(Self {
            settings: dirs.config_dir().join(SETTINGS_FILE),
            screen_ai: dirs.data_dir().join(SCREEN_AI_DIR),
            paddle_ocr: dirs.data_dir().join(PADDLE_OCR_DIR),
            tesseract: dirs.data_dir().join(TESSERACT_DIR),
            nllb: dirs.data_dir().join(NLLB_DIR),
            translations: temporary_dir(&dirs).join(TRANSLATIONS_DIR),
        })
    }
}

/// The user's runtime directory, which is private and emptied at logout, or
/// the shared temporary directory where there is none.
fn temporary_dir(dirs: &ProjectDirs) -> PathBuf {
    match dirs.runtime_dir() {
        Some(runtime) => runtime.to_path_buf(),
        None => std::env::temp_dir().join(APP_ID),
    }
}
