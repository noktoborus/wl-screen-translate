//! Errors of the application and the text the user is shown for each.

use std::path::PathBuf;

/// A failure anywhere in the application.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// No home directory, so no place for settings and models.
    #[error("no home directory")]
    NoHome,
    /// The desktop portal refused or failed the screenshot.
    #[cfg(target_os = "linux")]
    #[error("screenshot portal failed")]
    Portal {
        /// The portal error.
        #[source]
        source: ashpd::Error,
    },
    /// The portal answered with a location that is not a local file.
    #[cfg(target_os = "linux")]
    #[error("screenshot is not a local file: {uri}")]
    ScreenshotUri {
        /// The location the portal gave.
        uri: String,
    },
    /// The screen could not be captured.
    #[cfg(target_os = "windows")]
    #[error("screen capture failed")]
    Capture {
        /// The capture error.
        #[source]
        source: xcap::XCapError,
    },
    /// No monitor was found to capture.
    #[cfg(target_os = "windows")]
    #[error("no monitor")]
    NoMonitor,
    /// The screenshot file could not be decoded.
    #[error("cannot read the screenshot {path}")]
    ScreenshotRead {
        /// The screenshot file.
        path: PathBuf,
        /// The decoder error.
        #[source]
        source: image::ImageError,
    },
    /// The settings could not be encoded.
    #[error("cannot encode the settings")]
    SettingsEncode {
        /// The encoder error.
        #[source]
        source: serde_yaml_ng::Error,
    },
    /// The settings file could not be written.
    #[error("cannot write the settings {path}")]
    SettingsWrite {
        /// The settings file.
        path: PathBuf,
        /// The file system error.
        #[source]
        source: std::io::Error,
    },
    /// The OCR library or its models could not be loaded.
    #[error("cannot load Screen AI")]
    OcrLoad {
        /// The library error.
        #[source]
        source: screen_ai::Error,
    },
    /// Recognition failed.
    #[error("recognition failed")]
    Ocr {
        /// The library error.
        #[source]
        source: screen_ai::Error,
    },
    /// The translation model could not be loaded.
    #[error("cannot load the translation model")]
    TranslatorLoad {
        /// The translator error.
        #[source]
        source: nllb::Error,
    },
    /// Translation failed.
    #[error("translation failed")]
    Translate {
        /// The translator error.
        #[source]
        source: nllb::Error,
    },
    /// The window could not be opened.
    #[error("cannot open the window")]
    Window {
        /// The toolkit error.
        #[source]
        source: eframe::Error,
    },
}

impl AppError {
    /// The key of the text shown for this error in `locales/app.yml`.
    pub fn message_key(&self) -> &'static str {
        match self {
            Self::NoHome => "error.no_home",
            #[cfg(target_os = "linux")]
            Self::Portal { .. } | Self::ScreenshotUri { .. } => "error.capture",
            #[cfg(target_os = "windows")]
            Self::Capture { .. } | Self::NoMonitor => "error.capture",
            Self::ScreenshotRead { .. } => "error.capture",
            Self::SettingsEncode { .. } | Self::SettingsWrite { .. } => "error.settings",
            Self::OcrLoad { .. } => "error.ocr_load",
            Self::Ocr { .. } => "error.ocr",
            Self::TranslatorLoad { .. } => "error.translator_load",
            Self::Translate { .. } => "error.translate",
            Self::Window { .. } => "error.window",
        }
    }
}

/// The result type of the application.
pub type Result<T> = std::result::Result<T, AppError>;
