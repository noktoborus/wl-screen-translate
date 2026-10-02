//! The DB text detector: a map of the likelihood of text, cut into boxes.
//!
//! The steps follow PaddleOCR's `DBPostProcess`, with upright boxes: text on
//! a screen is not rotated.

use image::RgbaImage;
use image::imageops::FilterType;
use ort::session::Session;
use ort::value::Tensor;

use crate::Rect;
use crate::error::{Error, Result};

/// The largest side given to the detector, in pixels.
const MAX_SIDE: u32 = 2048;
/// Images with a shorter longest side are enlarged: the detector misses
/// letters a dozen pixels high.
const MIN_SIDE: u32 = 960;
/// The most an image is enlarged.
const MAX_ENLARGE: f32 = 2.0;
/// The sides of the input are multiples of this.
const STRIDE: u32 = 32;
/// The mean and the deviation of each channel, blue, green, red, the model
/// was trained with.
const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const STD: [f32; 3] = [0.229, 0.224, 0.225];
/// A pixel of the map above this is text.
const THRESHOLD: f32 = 0.3;
/// A region whose mean likelihood is below this is dropped.
const BOX_THRESHOLD: f32 = 0.6;
/// How much a region is grown back: the detector finds the core of the text.
const UNCLIP_RATIO: f32 = 1.5;
/// A region with a shorter side, in pixels of the map, is dropped.
const MIN_REGION: f32 = 3.0;

/// The boxes of text in `image`, in its pixels.
pub fn boxes(detector: &mut Session, image: &RgbaImage) -> Result<Vec<Rect>> {
    let longest = image.width().max(image.height()).max(1);
    let scale = if longest > MAX_SIDE {
        MAX_SIDE as f32 / longest as f32
    } else if longest < MIN_SIDE {
        (MIN_SIDE as f32 / longest as f32).min(MAX_ENLARGE)
    } else {
        1.0
    };
    let width = ((image.width() as f32 * scale).round() as u32).max(1);
    let height = ((image.height() as f32 * scale).round() as u32).max(1);
    let scaled;
    let input = if (width, height) == image.dimensions() {
        image
    } else {
        scaled = image::imageops::resize(image, width, height, FilterType::Triangle);
        &scaled
    };
    // Padded to the stride, not stretched, so the map is in scaled pixels.
    let padded = [width.div_ceil(STRIDE) * STRIDE, height.div_ceil(STRIDE) * STRIDE];
    let tensor = Tensor::from_array((
        [1usize, 3, padded[1] as usize, padded[0] as usize],
        planes(input, padded),
    ))?;
    let outputs = detector.run(ort::inputs![tensor])?;
    let (shape, map) = outputs[0].try_extract_tensor::<f32>()?;
    let [_, _, rows, columns] = shape[..] else {
        return Err(Error::Shape {
            shape: shape.to_vec(),
        });
    };
    let map = Map {
        values: map,
        width: columns as usize,
        height: rows as usize,
    };
    Ok(regions(&map, width as usize, height as usize)
        .into_iter()
        .map(|rect| Rect {
            x: rect.x / scale,
            y: rect.y / scale,
            width: rect.width / scale,
            height: rect.height / scale,
        })
        .map(|rect| clamp(rect, image.width() as f32, image.height() as f32))
        .filter(|rect| rect.width >= 1.0 && rect.height >= 1.0)
        .collect())
}

/// The image as normalised blue, green and red planes of `padded` size.
fn planes(image: &RgbaImage, padded: [u32; 2]) -> Box<[f32]> {
    let [width, height] = padded.map(|side| side as usize);
    let plane = width * height;
    let mut data = vec![0.0; 3 * plane].into_boxed_slice();
    for (x, y, pixel) in image.enumerate_pixels() {
        let at = y as usize * width + x as usize;
        for (channel, value) in [pixel[2], pixel[1], pixel[0]].into_iter().enumerate() {
            data[channel * plane + at] = (value as f32 / 255.0 - MEAN[channel]) / STD[channel];
        }
    }
    data
}

/// The likelihood of text at each pixel of the scaled image.
struct Map<'a> {
    values: &'a [f32],
    width: usize,
    height: usize,
}

/// The regions of the map above [`THRESHOLD`] within `width` by `height`,
/// grown back to the size of their text.
fn regions(map: &Map, width: usize, height: usize) -> Vec<Rect> {
    let (width, height) = (width.min(map.width), height.min(map.height));
    let mut seen = vec![false; width * height];
    let mut stack = Vec::new();
    let mut found = Vec::new();
    for start in 0..width * height {
        if seen[start] || map.values[(start / width) * map.width + start % width] <= THRESHOLD {
            continue;
        }
        seen[start] = true;
        stack.push(start);
        let (mut left, mut top, mut right, mut bottom) = (usize::MAX, usize::MAX, 0, 0);
        let (mut sum, mut count) = (0.0, 0usize);
        while let Some(at) = stack.pop() {
            let (x, y) = (at % width, at / width);
            (left, right, top, bottom) = (left.min(x), right.max(x), top.min(y), bottom.max(y));
            sum += map.values[y * map.width + x];
            count += 1;
            let neighbours = [
                (x > 0).then(|| at - 1),
                (x + 1 < width).then_some(at + 1),
                (y > 0).then(|| at - width),
                (y + 1 < height).then_some(at + width),
            ];
            for next in neighbours.into_iter().flatten() {
                if !seen[next] && map.values[(next / width) * map.width + next % width] > THRESHOLD
                {
                    seen[next] = true;
                    stack.push(next);
                }
            }
        }
        let rect = Rect {
            x: left as f32,
            y: top as f32,
            width: (right - left + 1) as f32,
            height: (bottom - top + 1) as f32,
        };
        if rect.width.min(rect.height) < MIN_REGION || sum / (count as f32) < BOX_THRESHOLD {
            continue;
        }
        found.push(unclip(rect));
    }
    found
}

/// Grows a region on every side by its area times [`UNCLIP_RATIO`] over its
/// perimeter, as PaddleOCR does.
fn unclip(rect: Rect) -> Rect {
    let grow = rect.width * rect.height * UNCLIP_RATIO / (2.0 * (rect.width + rect.height));
    Rect {
        x: rect.x - grow,
        y: rect.y - grow,
        width: rect.width + 2.0 * grow,
        height: rect.height + 2.0 * grow,
    }
}

/// `rect` cut to the image.
fn clamp(rect: Rect, width: f32, height: f32) -> Rect {
    let (left, top) = (rect.x.max(0.0), rect.y.max(0.0));
    let (right, bottom) = (rect.right().min(width), rect.bottom().min(height));
    Rect {
        x: left,
        y: top,
        width: (right - left).max(0.0),
        height: (bottom - top).max(0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regions_are_found_and_grown() {
        let (width, height) = (20, 10);
        let mut values = vec![0.0; width * height];
        // A bar of 10 by 4 pixels, and a speck too small to keep.
        for y in 3..7 {
            for x in 2..12 {
                values[y * width + x] = 0.9;
            }
        }
        values[width + 17] = 0.9;
        let map = Map {
            values: &values,
            width,
            height,
        };
        let found = regions(&map, width, height);
        assert_eq!(found.len(), 1);
        let grow = 10.0 * 4.0 * UNCLIP_RATIO / 28.0;
        assert_eq!(
            found[0],
            Rect {
                x: 2.0 - grow,
                y: 3.0 - grow,
                width: 10.0 + 2.0 * grow,
                height: 4.0 + 2.0 * grow,
            }
        );
    }

    #[test]
    fn faint_regions_are_dropped() {
        let values = vec![0.4; 100];
        let map = Map {
            values: &values,
            width: 10,
            height: 10,
        };
        assert!(regions(&map, 10, 10).is_empty());
    }
}
