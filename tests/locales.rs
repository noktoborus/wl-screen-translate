//! Every key of the files of `locales/` has an English and a Russian value,
//! and every language of the model has a name.

use std::collections::BTreeMap;
use std::path::Path;

const LOCALES: &[&str] = &["en", "ru"];

fn keys(file: &Path) -> BTreeMap<String, serde_yaml_ng::Value> {
    let text = std::fs::read_to_string(file).expect("readable locale file");
    serde_yaml_ng::from_str(&text).expect("valid yaml")
}

fn files() -> Vec<std::path::PathBuf> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("locales");
    std::fs::read_dir(directory)
        .expect("locales/")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "yml"))
        .collect()
}

#[test]
fn every_key_has_every_locale() {
    for file in files() {
        for (key, values) in keys(&file).iter().filter(|(key, _)| !key.starts_with('_')) {
            for locale in LOCALES {
                let value = values.get(locale).and_then(|value| value.as_str());
                assert!(
                    value.is_some_and(|value| !value.is_empty()),
                    "{key} has no {locale} value in {}",
                    file.display()
                );
            }
        }
    }
}

#[test]
fn every_language_has_a_name() {
    let known: Vec<String> = files()
        .iter()
        .flat_map(|file| keys(file).into_keys())
        .collect();
    for code in nllb::LANGUAGES {
        let key = format!("language.{code}");
        assert!(known.contains(&key), "{key} is missing");
    }
}
