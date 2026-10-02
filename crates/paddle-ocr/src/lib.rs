//! Text recognition with the PaddleOCR PP-OCRv5 models through ONNX Runtime.
//!
//! A detector finds the lines of text, a recogniser for the script reads
//! each, and the lines are grouped into paragraphs by their layout.
#![deny(missing_docs)]

mod detect;
mod error;
mod layout;
mod recognize;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use image::RgbaImage;
use ort::session::Session;
use ort::session::builder::GraphOptimizationLevel;

pub use error::{Error, Result};

/// The file name of ONNX Runtime in the model directory.
#[cfg(target_os = "linux")]
pub const RUNTIME_NAME: &str = "libonnxruntime.so";
/// The file name of ONNX Runtime in the model directory.
#[cfg(target_os = "windows")]
pub const RUNTIME_NAME: &str = "onnxruntime.dll";
/// The detector in the model directory.
pub const DETECTOR: &str = "det.onnx";
/// The directory of the recognisers in the model directory: `<name>.onnx`
/// and its alphabet `<name>.txt`.
pub const RECOGNIZERS: &str = "rec";

/// One recognised line, in pixels of the image given to [`Engine::recognize`].
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
    /// The block the line belongs to: here its paragraph.
    pub block: i32,
    /// The paragraph the line belongs to.
    pub paragraph: i32,
}

/// The name of the recogniser that reads `script`, an ISO 15924 code such as
/// `Cyrl`, if any does.
pub fn recognizer_for(script: &str) -> Option<&'static str> {
    Some(match script {
        "Latn" => "latin",
        "Cyrl" => "cyrillic",
        // The Chinese recogniser reads Japanese too.
        "Hans" | "Hant" | "Jpan" => "ch",
        "Hang" => "korean",
        "Arab" => "arabic",
        "Deva" => "devanagari",
        "Grek" => "el",
        "Taml" => "ta",
        "Telu" => "te",
        "Thai" => "th",
        _ => return None,
    })
}

/// The detector, and the recognisers loaded so far.
pub struct Engine {
    directory: PathBuf,
    detector: Session,
    recognizers: HashMap<&'static str, recognize::Recognizer>,
}

impl Engine {
    /// Loads ONNX Runtime and the detector from a model directory; a
    /// recogniser is loaded on its first use.
    ///
    /// One process loads ONNX Runtime once: a second engine uses the runtime
    /// of the first.
    pub fn load(directory: &Path) -> Result<Self> {
        let runtime = directory.join(RUNTIME_NAME);
        if !runtime.is_file() {
            return Err(Error::Missing { path: runtime });
        }
        ort::init_from(&runtime)
            .map_err(|source| Error::Runtime { source })?
            .commit();
        Ok(Self {
            directory: directory.to_path_buf(),
            detector: session(&directory.join(DETECTOR))?,
            recognizers: HashMap::new(),
        })
    }

    /// Recognises the text of an image written in `script`, an ISO 15924
    /// code.
    pub fn recognize(&mut self, image: &RgbaImage, script: &str) -> Result<Vec<Line>> {
        let name = recognizer_for(script).ok_or_else(|| Error::Script {
            script: script.to_owned(),
        })?;
        let recognizer = match self.recognizers.entry(name) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                let directory = self.directory.join(RECOGNIZERS);
                entry.insert(recognize::Recognizer::load(&directory, name)?)
            }
        };
        let boxes = detect::boxes(&mut self.detector, image)?;
        let texts = recognizer.read(image, &boxes)?;
        let lines = boxes
            .into_iter()
            .zip(texts)
            .filter_map(|(rect, text)| Some((rect, text?)))
            .collect();
        Ok(layout::paragraphs(lines))
    }
}

/// The most threads a model runs on: more only wait on each other, so the
/// small models of this crate run slower.
const MAX_THREADS: usize = 4;

/// Opens a model with half the cores of the machine, which leaves out their
/// hyper-threaded twins, at most [`MAX_THREADS`].
fn session(path: &Path) -> Result<Session> {
    if !path.is_file() {
        return Err(Error::Missing {
            path: path.to_path_buf(),
        });
    }
    let cores = std::thread::available_parallelism().map_or(1, usize::from);
    let threads = (cores / 2).clamp(1, MAX_THREADS);
    let load = |source: ort::Error| Error::Load {
        path: path.to_path_buf(),
        source,
    };
    Session::builder()
        .map_err(load)?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(|error| load(error.into()))?
        .with_intra_threads(threads)
        .map_err(|error| load(error.into()))?
        .commit_from_file(path)
        .map_err(load)
}

/// A box in pixels of the image.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Rect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl Rect {
    fn right(&self) -> f32 {
        self.x + self.width
    }

    fn bottom(&self) -> f32 {
        self.y + self.height
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripts_pick_their_recogniser() {
        assert_eq!(recognizer_for("Cyrl"), Some("cyrillic"));
        assert_eq!(recognizer_for("Latn"), Some("latin"));
        assert_eq!(recognizer_for("Jpan"), Some("ch"));
        assert_eq!(recognizer_for("Ethi"), None);
    }
}
