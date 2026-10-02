//! The Tesseract models found at start, and the model of each language of
//! NLLB.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The model directory and its languages, found once by [`scan`].
static FOUND: OnceLock<Found> = OnceLock::new();

/// The model directory and its languages.
#[derive(Debug, Default)]
struct Found {
    directory: Option<PathBuf>,
    languages: Vec<String>,
}

/// The model of English, added to every other: screen text is full of it.
const ENGLISH: &str = "eng";

/// NLLB codes whose Tesseract model is not named by their first part.
const MODELS: &[(&str, &str)] = &[
    ("zho_Hans", "chi_sim"),
    ("zho_Hant", "chi_tra"),
    ("yue_Hant", "chi_tra"),
    ("arb_Arab", "ara"),
    ("ars_Arab", "ara"),
    ("acm_Arab", "ara"),
    ("acq_Arab", "ara"),
    ("aeb_Arab", "ara"),
    ("ajp_Arab", "ara"),
    ("apc_Arab", "ara"),
    ("ary_Arab", "ara"),
    ("arz_Arab", "ara"),
    ("pes_Arab", "fas"),
    ("prs_Arab", "fas"),
    ("zsm_Latn", "msa"),
    ("azj_Latn", "aze"),
    ("uzn_Latn", "uzb"),
    ("khk_Cyrl", "mon"),
    ("npi_Deva", "nep"),
    ("ory_Orya", "ori"),
    ("pbt_Arab", "pus"),
    ("quy_Latn", "que"),
    ("ydd_Hebr", "yid"),
    ("lvs_Latn", "lav"),
    ("als_Latn", "sqi"),
    ("swh_Latn", "swa"),
    ("nob_Latn", "nor"),
    ("nno_Latn", "nor"),
];

/// Looks for the models in `preferred`, else where the system keeps them.
/// Later calls keep the first answer.
pub fn scan(preferred: &Path) {
    FOUND.get_or_init(|| {
        let directory = tesseract::model_directory(preferred);
        let languages = directory
            .as_deref()
            .map(tesseract::languages)
            .unwrap_or_default();
        match &directory {
            Some(directory) => log::info!(
                "Tesseract models in {}: {}",
                directory.display(),
                languages.join(" ")
            ),
            None => log::info!("no Tesseract models"),
        }
        Found {
            directory,
            languages,
        }
    });
}

fn found() -> &'static Found {
    FOUND.get_or_init(Found::default)
}

/// The model directory found, if any.
pub fn directory() -> Option<&'static Path> {
    found().directory.as_deref()
}

/// The Tesseract model of `language`, an NLLB code.
fn model(language: &str) -> &str {
    MODELS
        .iter()
        .find(|(code, _)| *code == language)
        .map(|(_, model)| *model)
        .unwrap_or_else(|| language.split('_').next().unwrap_or(language))
}

/// Whether the model of `language` is installed; any language is when no
/// model is, so the menu is not empty and recognising tells what is missing.
pub fn reads(language: &str) -> bool {
    reads_in(&found().languages, language)
}

fn reads_in(installed: &[String], language: &str) -> bool {
    installed.is_empty() || installed.iter().any(|name| name == model(language))
}

/// The models to read `language` with: its own, and English when it is
/// installed.
pub fn models(language: &str) -> String {
    models_in(&found().languages, language)
}

fn models_in(installed: &[String], language: &str) -> String {
    let own = model(language);
    if own != ENGLISH && installed.iter().any(|name| name == ENGLISH) {
        format!("{own}+{ENGLISH}")
    } else {
        own.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn languages_are_named_as_tesseract_names_them() {
        assert_eq!(model("rus_Cyrl"), "rus");
        assert_eq!(model("zho_Hans"), "chi_sim");
        assert_eq!(model("arz_Arab"), "ara");
    }

    #[test]
    fn installed_models_are_read() {
        let installed = ["eng".to_owned(), "rus".to_owned()];
        assert!(reads_in(&installed, "rus_Cyrl"));
        assert!(!reads_in(&installed, "deu_Latn"));
        assert!(reads_in(&[], "deu_Latn"));
    }

    #[test]
    fn english_is_added_when_installed() {
        let installed = ["eng".to_owned(), "rus".to_owned()];
        assert_eq!(models_in(&installed, "rus_Cyrl"), "rus+eng");
        assert_eq!(models_in(&installed, "eng_Latn"), "eng");
        assert_eq!(models_in(&["rus".to_owned()], "rus_Cyrl"), "rus");
    }
}
