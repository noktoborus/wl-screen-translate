//! Translations kept in a temporary directory, so a text translated once is
//! shown at once, without loading the model.
//!
//! A translation is a file `<source>-<target>/<hash>`, where the hash is the
//! 128-bit XXH3 of the text in hexadecimal. Hashes, hits, misses and writes
//! are logged; a failure to read or write is logged and taken for a miss.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use xxhash_rust::xxh3::xxh3_128;

use crate::worker::Languages;

/// The translations of one user.
pub struct Cache {
    directory: PathBuf,
}

impl Cache {
    /// The cache in `directory`, made when the first translation is put.
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    /// The place of the translation of `text` from and to `languages`, or
    /// none when a language code would leave the cache directory.
    pub fn entry(&self, languages: &Languages, text: &str) -> Option<Entry> {
        let code = |code: &str| {
            !code.is_empty()
                && code
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        };
        if !code(&languages.source) || !code(&languages.target) {
            log::warn!("language codes unfit for a file name: {languages:?}");
            return None;
        }
        let direction = format!("{}-{}", languages.source, languages.target);
        let started = Instant::now();
        let hash = format!("{:032x}", xxh3_128(text.as_bytes()));
        log::info!(
            "hash {hash} of {} bytes of text in {:?}",
            text.len(),
            started.elapsed()
        );
        let name = format!("{direction}/{hash}");
        let path = self.directory.join(direction).join(hash);
        Some(Entry { name, path })
    }
}

/// The place of one translation in the cache.
pub struct Entry {
    /// `<direction>/<hash>`, for the log.
    name: String,
    path: PathBuf,
}

impl Entry {
    /// The translation, if one was put.
    pub fn get(&self) -> Option<String> {
        match std::fs::read_to_string(&self.path) {
            Ok(translation) => {
                log::info!("cache hit {}", self.name);
                Some(translation)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                log::info!("cache miss {}", self.name);
                None
            }
            Err(error) => {
                log::warn!(
                    "cannot read the translation {}: {error}",
                    self.path.display()
                );
                None
            }
        }
    }

    /// Keeps `translation`.
    pub fn put(&self, translation: &str) {
        match write(&self.path, translation) {
            Ok(()) => log::info!("cached {}", self.name),
            Err(error) => {
                log::warn!(
                    "cannot keep the translation {}: {error}",
                    self.path.display()
                )
            }
        }
    }
}

/// Writes through a temporary file and a rename, so a reader never sees a
/// part of a translation.
fn write(path: &Path, text: &str) -> std::io::Result<()> {
    let directory = path.parent().unwrap_or(Path::new("."));
    create_private_dir(directory)?;
    let mut file = tempfile::NamedTempFile::new_in(directory)?;
    file.write_all(text.as_bytes())?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}

/// Makes `directory` and its parents, readable by the user alone: the texts
/// may be private.
fn create_private_dir(directory: &Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(directory)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn languages(source: &str, target: &str) -> Languages {
        Languages {
            source: source.into(),
            target: target.into(),
        }
    }

    #[test]
    fn gets_what_was_put() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path().join("cache"));
        let english = languages("eng_Latn", "rus_Cyrl");
        let entry = cache.entry(&english, "Hello").unwrap();
        assert_eq!(entry.get(), None);
        entry.put("Привет");
        let entry = cache.entry(&english, "Hello").unwrap();
        assert_eq!(entry.get().as_deref(), Some("Привет"));
    }

    #[test]
    fn keeps_directions_apart() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path().to_path_buf());
        let get = |source, target| cache.entry(&languages(source, target), "Hello")?.get();
        cache
            .entry(&languages("eng_Latn", "rus_Cyrl"), "Hello")
            .unwrap()
            .put("Привет");
        assert_eq!(get("eng_Latn", "deu_Latn"), None);
        assert_eq!(get("rus_Cyrl", "eng_Latn"), None);
    }

    #[test]
    fn refuses_codes_leaving_the_directory() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path().join("cache"));
        assert!(
            cache
                .entry(&languages("..", "x/../../y"), "Hello")
                .is_none()
        );
    }
}
