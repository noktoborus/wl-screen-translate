//! The CRNN recogniser: a line cut from the image, read by CTC.

use std::path::Path;

use image::RgbaImage;
use image::imageops::FilterType;
use ort::session::Session;
use ort::value::Tensor;

use crate::Rect;
use crate::error::{Error, Result};

/// The height of a line given to the model, in pixels.
const HEIGHT: u32 = 48;
/// A narrower line is padded to this width, in pixels, as PaddleOCR pads it.
const MIN_WIDTH: u32 = 320;
/// The widest line given to the model, in pixels.
const MAX_WIDTH: u32 = 3200;
/// A line read with a lower mean confidence is dropped.
const MIN_SCORE: f32 = 0.5;

/// A recogniser and the letters of its classes.
pub struct Recognizer {
    session: Session,
    /// The letter of each class but the blank, which is class 0.
    alphabet: Vec<String>,
}

impl Recognizer {
    /// Loads `<name>.onnx` and its alphabet `<name>.txt` from `directory`.
    pub fn load(directory: &Path, name: &str) -> Result<Self> {
        let path = directory.join(format!("{name}.txt"));
        if !path.is_file() {
            return Err(Error::Missing { path });
        }
        let text =
            std::fs::read_to_string(&path).map_err(|source| Error::Alphabet { path, source })?;
        let mut alphabet: Vec<String> = text
            .lines()
            .map(|line| line.trim_end_matches('\r').to_owned())
            .collect();
        // The class after the alphabet is the space.
        alphabet.push(" ".to_owned());
        Ok(Self {
            session: crate::session(&directory.join(format!("{name}.onnx")))?,
            alphabet,
        })
    }

    /// The text of each box, `None` where the reading is unsure or empty.
    ///
    /// The lines are read one by one: a batch is padded to its widest line,
    /// which costs more than it saves.
    pub fn read(&mut self, image: &RgbaImage, boxes: &[Rect]) -> Result<Vec<Option<String>>> {
        boxes
            .iter()
            .map(|rect| self.run(&line(image, *rect)))
            .collect()
    }

    fn run(&mut self, line: &RgbaImage) -> Result<Option<String>> {
        let width = line.width().max(MIN_WIDTH) as usize;
        let height = line.height() as usize;
        let plane = width * height;
        // Padding is 0, the middle grey after normalisation.
        let mut data = vec![0.0f32; 3 * plane];
        for (x, y, pixel) in line.enumerate_pixels() {
            let at = y as usize * width + x as usize;
            for (channel, value) in [pixel[2], pixel[1], pixel[0]].into_iter().enumerate() {
                data[at + channel * plane] = value as f32 / 127.5 - 1.0;
            }
        }
        let tensor = Tensor::from_array(([1, 3, height, width], data))?;
        let outputs = self.session.run(ort::inputs![tensor])?;
        let (shape, values) = outputs[0].try_extract_tensor::<f32>()?;
        let [1, _, classes] = shape[..] else {
            return Err(Error::Shape {
                shape: shape.to_vec(),
            });
        };
        Ok(decode(values, classes as usize, &self.alphabet))
    }
}

/// The box cut from the image and scaled to [`HEIGHT`].
fn line(image: &RgbaImage, rect: Rect) -> RgbaImage {
    let (x, y) = (rect.x as u32, rect.y as u32);
    let width = (rect.width.round() as u32).clamp(1, image.width() - x);
    let height = (rect.height.round() as u32).clamp(1, image.height() - y);
    let cut = image::imageops::crop_imm(image, x, y, width, height).to_image();
    let scaled = (HEIGHT as f32 * width as f32 / height as f32).ceil() as u32;
    image::imageops::resize(
        &cut,
        scaled.clamp(1, MAX_WIDTH),
        HEIGHT,
        FilterType::Triangle,
    )
}

/// The text of one line by greedy CTC: the likeliest class at each step,
/// repeats and blanks dropped. `None` when empty or unsure.
fn decode(probabilities: &[f32], classes: usize, alphabet: &[String]) -> Option<String> {
    let mut text = String::new();
    let mut scores = Vec::new();
    let mut previous = 0;
    for step in probabilities.chunks_exact(classes) {
        let (class, score) =
            step.iter()
                .copied()
                .enumerate()
                .fold((0, f32::MIN), |best, (class, score)| {
                    if score > best.1 { (class, score) } else { best }
                });
        if class != 0
            && class != previous
            && let Some(letter) = alphabet.get(class - 1)
        {
            text.push_str(letter);
            scores.push(score);
        }
        previous = class;
    }
    let text = text.trim();
    let mean = scores.iter().sum::<f32>() / scores.len().max(1) as f32;
    (!text.is_empty() && mean >= MIN_SCORE).then(|| text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steps(classes: &[usize], count: usize) -> Vec<f32> {
        classes
            .iter()
            .flat_map(|&class| (0..count).map(move |c| if c == class { 0.9 } else { 0.01 }))
            .collect()
    }

    #[test]
    fn ctc_drops_blanks_and_repeats() {
        let alphabet: Vec<String> = ["a", "b", " "].map(String::from).to_vec();
        let probabilities = steps(&[1, 1, 0, 1, 2, 3, 2, 0], 4);
        assert_eq!(
            decode(&probabilities, 4, &alphabet).as_deref(),
            Some("aab b")
        );
    }

    #[test]
    fn unsure_lines_are_dropped() {
        let alphabet: Vec<String> = ["a"].map(String::from).to_vec();
        let probabilities = [0.6, 0.4, 0.6, 0.4];
        assert_eq!(decode(&probabilities, 2, &alphabet), None);
        assert_eq!(decode(&[0.3, 0.3], 2, &alphabet), None);
    }
}
