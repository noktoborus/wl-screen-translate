//! Translation with an NLLB-200 model converted for CTranslate2.
#![deny(missing_docs)]

mod error;
mod languages;

use std::path::Path;

use ct2rs::sys::{Config, TranslationOptions, Translator as Model};
use tokenizers::Tokenizer;

pub use error::{BoxError, Error, Result};
pub use languages::{LANGUAGES, autonym};

/// The tokenizer file in the model directory.
pub const TOKENIZER_FILE: &str = "tokenizer.json";
/// The token that ends a source sentence.
const END_OF_SENTENCE: &str = "</s>";

/// A loaded model and its tokenizer.
pub struct Translator {
    model: Model,
    tokenizer: Tokenizer,
}

impl Translator {
    /// Loads a CTranslate2 model directory that also holds `tokenizer.json`.
    pub fn load(directory: &Path) -> Result<Self> {
        let path = directory.join(TOKENIZER_FILE);
        let tokenizer =
            Tokenizer::from_file(&path).map_err(|source| Error::Tokenizer { path, source })?;
        let model = Model::new(directory, &Config::default()).map_err(|source| Error::Model {
            path: directory.to_path_buf(),
            source: source.into(),
        })?;
        Ok(Self { model, tokenizer })
    }

    /// Translates every text from the `source` language to the `target` one.
    ///
    /// Languages are codes of [`LANGUAGES`]. The result keeps the order of `texts`.
    pub fn translate(&self, texts: &[String], source: &str, target: &str) -> Result<Vec<String>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        for code in [source, target] {
            if self.tokenizer.token_to_id(code).is_none() {
                return Err(Error::Language { code: code.into() });
            }
        }
        let sources = texts
            .iter()
            .map(|text| self.tokens(text, source))
            .collect::<Result<Vec<_>>>()?;
        let prefixes = vec![vec![target]; texts.len()];
        let results = self
            .model
            .translate_batch_with_target_prefix(
                &sources,
                &prefixes,
                &TranslationOptions::default(),
                None,
            )
            .map_err(|source| Error::Translate {
                source: source.into(),
            })?;
        results
            .into_iter()
            .map(|result| {
                let tokens = result.hypotheses.into_iter().next().unwrap_or_default();
                self.text(&tokens)
            })
            .collect()
    }

    fn tokens(&self, text: &str, language: &str) -> Result<Vec<String>> {
        let encoding = self
            .tokenizer
            .encode(text, false)
            .map_err(|source| Error::Encode { source })?;
        let mut tokens = Vec::with_capacity(encoding.len() + 2);
        tokens.push(language.to_owned());
        tokens.extend_from_slice(encoding.get_tokens());
        tokens.push(END_OF_SENTENCE.to_owned());
        Ok(tokens)
    }

    fn text(&self, tokens: &[String]) -> Result<String> {
        let ids: Vec<u32> = tokens
            .iter()
            .filter_map(|token| self.tokenizer.token_to_id(token))
            .collect();
        self.tokenizer
            .decode(&ids, true)
            .map_err(|source| Error::Decode { source })
    }
}
