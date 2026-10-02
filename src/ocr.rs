//! The recognisers to choose from, each loaded on its first use.

use std::time::Instant;

use image::RgbaImage;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};
use crate::paths::Paths;
use crate::tessdata;

/// The recogniser of the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Ocr {
    /// Chrome Screen AI: built for x86-64 only.
    ScreenAi,
    /// PaddleOCR PP-OCRv5, through ONNX Runtime.
    #[serde(rename = "paddle-ocr")]
    Paddle,
    /// Tesseract, the library of the system.
    Tesseract,
}

impl Default for Ocr {
    /// Screen AI, but where there is no build of it.
    fn default() -> Self {
        if cfg!(any(target_arch = "arm", target_arch = "aarch64")) {
            Ocr::Paddle
        } else {
            Ocr::ScreenAi
        }
    }
}

impl Ocr {
    /// Every recogniser, in the order of the menu.
    pub const ALL: [Ocr; 3] = [Ocr::ScreenAi, Ocr::Paddle, Ocr::Tesseract];

    /// The name of the recogniser in the menu and in the settings.
    pub fn id(self) -> &'static str {
        match self {
            Ocr::ScreenAi => "screen-ai",
            Ocr::Paddle => "paddle-ocr",
            Ocr::Tesseract => "tesseract",
        }
    }

    /// The recogniser named `id`.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|ocr| ocr.id() == id)
    }

    /// The name shown.
    pub fn name(self) -> &'static str {
        match self {
            Ocr::ScreenAi => "Screen AI",
            Ocr::Paddle => "PaddleOCR",
            Ocr::Tesseract => "Tesseract",
        }
    }

    /// Whether the recogniser reads `language`, an NLLB code such as
    /// `rus_Cyrl`.
    pub fn reads(self, language: &str) -> bool {
        match self {
            Ocr::ScreenAi => true,
            Ocr::Paddle => paddle_ocr::recognizer_for(script(language)).is_some(),
            Ocr::Tesseract => tessdata::reads(language),
        }
    }
}

impl Ocr {
    /// The model the recogniser reads `language` with, when it has one for
    /// each language: another model reads the text otherwise.
    pub fn model(self, language: &str) -> Option<String> {
        match self {
            Ocr::ScreenAi => None,
            Ocr::Paddle => paddle_ocr::recognizer_for(script(language)).map(str::to_owned),
            Ocr::Tesseract => Some(tessdata::models(language)),
        }
    }
}

/// The ISO 15924 script of an NLLB code: the part after the underscore.
fn script(language: &str) -> &str {
    language.rsplit('_').next().unwrap_or(language)
}

/// One recognised line, in pixels of the image recognised.
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

/// A [`Line`] from the line of a recogniser, whose fields are alike.
macro_rules! line_from {
    ($line:ty) => {
        impl From<$line> for Line {
            fn from(line: $line) -> Self {
                Self {
                    text: line.text,
                    x: line.x,
                    y: line.y,
                    width: line.width,
                    height: line.height,
                    block: line.block,
                    paragraph: line.paragraph,
                }
            }
        }
    };
}

line_from!(screen_ai::Line);
line_from!(paddle_ocr::Line);
line_from!(tesseract::Line);

/// The recognisers loaded so far.
#[derive(Default)]
pub struct Recognizers {
    screen_ai: Option<screen_ai::Engine>,
    paddle_ocr: Option<paddle_ocr::Engine>,
    tesseract: Option<tesseract::Engine>,
}

impl Recognizers {
    /// Recognises the text of `image`, written in `language`, with `ocr`,
    /// which is loaded from `paths` first if it is not yet.
    pub fn recognize(
        &mut self,
        ocr: Ocr,
        paths: &Paths,
        image: &RgbaImage,
        language: &str,
    ) -> Result<Vec<Line>> {
        match ocr {
            Ocr::ScreenAi => {
                let engine = loaded(&mut self.screen_ai, ocr, &paths.screen_ai, |path| {
                    screen_ai::Engine::load(path).map_err(|source| AppError::OcrLoad { source })
                })?;
                let lines = engine
                    .recognize(image)
                    .map_err(|source| AppError::Ocr { source })?;
                Ok(lines.into_iter().map(Line::from).collect())
            }
            Ocr::Paddle => {
                let engine = loaded(&mut self.paddle_ocr, ocr, &paths.paddle_ocr, |path| {
                    paddle_ocr::Engine::load(path)
                        .map_err(|source| AppError::PaddleOcrLoad { source })
                })?;
                let lines =
                    engine
                        .recognize(image, script(language))
                        .map_err(|source| match source {
                            // A recogniser is loaded on its first use.
                            paddle_ocr::Error::Missing { .. }
                            | paddle_ocr::Error::Load { .. }
                            | paddle_ocr::Error::Alphabet { .. } => {
                                AppError::PaddleOcrLoad { source }
                            }
                            source => AppError::PaddleOcr { source },
                        })?;
                Ok(lines.into_iter().map(Line::from).collect())
            }
            Ocr::Tesseract => {
                let directory = tessdata::directory().unwrap_or(&paths.tesseract);
                let engine = loaded(&mut self.tesseract, ocr, directory, |path| {
                    let engine = tesseract::Engine::load(path)
                        .map_err(|source| AppError::TesseractLoad { source })?;
                    log::info!("Tesseract {}", engine.version());
                    Ok(engine)
                })?;
                let models = tessdata::models(language);
                log::info!("Tesseract models {models}");
                let lines = engine
                    .recognize(image, &models)
                    .map_err(|source| match source {
                        // A model is read on its first use.
                        tesseract::Error::Missing { .. } | tesseract::Error::Init { .. } => {
                            AppError::TesseractLoad { source }
                        }
                        source => AppError::Tesseract { source },
                    })?;
                Ok(lines.into_iter().map(Line::from).collect())
            }
        }
    }
}

/// The engine in `slot`, loaded from `path` by `load` if it is empty.
fn loaded<'a, T>(
    slot: &'a mut Option<T>,
    ocr: Ocr,
    path: &std::path::Path,
    load: impl FnOnce(&std::path::Path) -> Result<T>,
) -> Result<&'a mut T> {
    if let Some(engine) = slot {
        return Ok(engine);
    }
    log::info!("loading {} from {}", ocr.name(), path.display());
    let started = Instant::now();
    let engine = load(path)?;
    log::info!("{} loaded in {:?}", ocr.name(), started.elapsed());
    Ok(slot.insert(engine))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paddle_ocr_reads_only_some_scripts() {
        assert!(Ocr::Paddle.reads("rus_Cyrl"));
        assert!(Ocr::Paddle.reads("jpn_Jpan"));
        assert!(Ocr::Paddle.reads("zho_Hant"));
        assert!(!Ocr::Paddle.reads("amh_Ethi"));
        assert!(!Ocr::Paddle.reads("heb_Hebr"));
        assert!(Ocr::ScreenAi.reads("amh_Ethi"));
        // No Tesseract model found: every language, to tell what is missing.
        assert!(Ocr::Tesseract.reads("amh_Ethi"));
    }

    #[test]
    fn ids_round_trip() {
        for ocr in Ocr::ALL {
            assert_eq!(Ocr::from_id(ocr.id()), Some(ocr));
            let yaml = serde_yaml_ng::to_string(&ocr).unwrap();
            assert_eq!(yaml.trim(), ocr.id());
        }
    }
}
