//! The persisted choice of languages.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};
use crate::ocr::Ocr;
use crate::split::Split;

/// The extension a settings file this version cannot read is kept aside under.
const UNREADABLE_SUFFIX: &str = "unreadable";
/// How many directions of translation are remembered.
pub const RECENT_DIRECTIONS: usize = 4;

/// A direction of translation: from a language to another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Direction {
    /// The language of the text.
    pub source: String,
    /// The language to translate to.
    pub target: String,
}

/// The settings of the application.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// The language of the recognised text.
    #[serde(default = "default_source")]
    pub source: String,
    /// The language to translate to.
    #[serde(default = "default_target")]
    pub target: String,
    /// Where the language window was left, in points, by the connector of the
    /// monitor, such as `DP-1`, or by its size in pixels, such as `2560x1440`,
    /// when the connector is not known.
    #[serde(default)]
    pub language_window: BTreeMap<String, [f32; 2]>,
    /// The directions last chosen, the latest first, at most
    /// [`RECENT_DIRECTIONS`].
    #[serde(default)]
    pub recent: Vec<Direction>,
    /// Whether the recognised text is translated.
    #[serde(default = "default_translate")]
    pub translate: bool,
    /// How the text is cut for translation.
    #[serde(default)]
    pub split: Split,
    /// The recogniser of the text.
    #[serde(default)]
    pub ocr: Ocr,
    /// Whether a piece longer than [`Settings::token_limit`] is cut.
    #[serde(default)]
    pub limit_tokens: bool,
    /// The most tokens of a piece given to the translator, kept while the
    /// limit is off.
    #[serde(default = "default_token_limit")]
    pub token_limit: usize,
}

fn default_source() -> String {
    "eng_Latn".into()
}

fn default_target() -> String {
    "rus_Cyrl".into()
}

fn default_token_limit() -> usize {
    512
}

fn default_translate() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            source: default_source(),
            target: default_target(),
            language_window: BTreeMap::new(),
            recent: Vec::new(),
            translate: default_translate(),
            split: Split::default(),
            ocr: Ocr::default(),
            limit_tokens: false,
            token_limit: default_token_limit(),
        }
    }
}

impl Settings {
    /// The direction in use.
    pub fn direction(&self) -> Direction {
        Direction {
            source: self.source.clone(),
            target: self.target.clone(),
        }
    }

    /// Puts the direction in use first among the recent ones, unless it is
    /// from a language to itself.
    pub fn remember_direction(&mut self) {
        if self.source == self.target {
            return;
        }
        let direction = self.direction();
        self.recent.retain(|recent| *recent != direction);
        self.recent.insert(0, direction);
        self.recent.truncate(RECENT_DIRECTIONS);
    }

    /// The most tokens of a piece, when they are limited.
    pub fn token_limit(&self) -> Option<usize> {
        self.limit_tokens.then_some(self.token_limit)
    }

    /// Takes the default language of the text when the recogniser does not
    /// read the one chosen; true if it did.
    pub fn fit_source(&mut self) -> bool {
        if self.ocr.reads(&self.source) {
            return false;
        }
        self.source = default_source();
        true
    }

    /// Reads the settings, or starts from the defaults.
    pub fn load(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        serde_yaml_ng::from_str(&text).unwrap_or_else(|error| {
            log::warn!("{}: {error}", path.display());
            start_over(path)
        })
    }

    /// Writes the settings through a temporary file and a rename.
    pub fn save(&self, path: &Path) -> Result<()> {
        let text =
            serde_yaml_ng::to_string(self).map_err(|source| AppError::SettingsEncode { source })?;
        let write_error = |source| AppError::SettingsWrite {
            path: path.to_path_buf(),
            source,
        };
        let directory = path.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(directory).map_err(write_error)?;
        let mut file = tempfile::NamedTempFile::new_in(directory).map_err(write_error)?;
        file.write_all(text.as_bytes()).map_err(write_error)?;
        file.persist(path)
            .map_err(|error| write_error(error.error))?;
        Ok(())
    }
}

/// Keeps an unreadable file aside and returns the defaults.
fn start_over(path: &Path) -> Settings {
    let aside = path.with_extension(UNREADABLE_SUFFIX);
    if let Err(error) = std::fs::rename(path, &aside) {
        log::warn!("{}: {error}", aside.display());
    }
    Settings::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.yaml");
        let settings = Settings {
            source: "deu_Latn".into(),
            target: "ukr_Cyrl".into(),
            language_window: BTreeMap::from([("2560x1440".into(), [100.0, 200.5])]),
            recent: vec![Direction {
                source: "deu_Latn".into(),
                target: "ukr_Cyrl".into(),
            }],
            translate: false,
            split: Split::Paragraphs,
            ocr: Ocr::PaddleOcr,
            limit_tokens: true,
            token_limit: 200,
        };
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path), settings);
    }

    #[test]
    fn remembers_the_latest_directions_once() {
        let mut settings = Settings::default();
        for (source, target) in [
            ("eng_Latn", "rus_Cyrl"),
            ("deu_Latn", "rus_Cyrl"),
            ("fra_Latn", "rus_Cyrl"),
            ("eng_Latn", "rus_Cyrl"),
            ("spa_Latn", "rus_Cyrl"),
            ("ita_Latn", "rus_Cyrl"),
        ] {
            settings.source = source.into();
            settings.target = target.into();
            settings.remember_direction();
        }
        let sources: Vec<_> = settings.recent.iter().map(|d| d.source.as_str()).collect();
        assert_eq!(sources, ["ita_Latn", "spa_Latn", "eng_Latn", "fra_Latn"]);
    }

    #[test]
    fn a_language_to_itself_is_not_remembered() {
        let mut settings = Settings::default();
        settings.target = settings.source.clone();
        settings.remember_direction();
        assert!(settings.recent.is_empty());
    }

    #[test]
    fn missing_key_reads_as_default() {
        let settings: Settings = serde_yaml_ng::from_str("target: fra_Latn").unwrap();
        assert_eq!(settings.source, default_source());
        assert_eq!(settings.target, "fra_Latn");
        assert_eq!(settings.split, Split::Sentences);
        assert_eq!(settings.ocr, Ocr::default());
        assert_eq!(settings.token_limit(), None);
        assert_eq!(settings.token_limit, 512);
    }

    #[test]
    fn a_language_the_recogniser_does_not_read_is_replaced() {
        let mut settings = Settings {
            source: "amh_Ethi".into(),
            ocr: Ocr::ScreenAi,
            ..Settings::default()
        };
        assert!(!settings.fit_source());
        settings.ocr = Ocr::PaddleOcr;
        assert!(settings.fit_source());
        assert_eq!(settings.source, default_source());
    }

    #[test]
    fn unreadable_file_is_kept_aside() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.yaml");
        std::fs::write(&path, "source: [").unwrap();
        assert_eq!(Settings::load(&path), Settings::default());
        assert!(path.with_extension(UNREADABLE_SUFFIX).exists());
    }
}
