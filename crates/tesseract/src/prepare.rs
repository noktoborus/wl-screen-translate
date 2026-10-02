//! The image made fit for Tesseract, which reads dark print on a light page
//! at a few hundred dots per inch: grey, dark on light, enlarged.

use image::imageops::FilterType;
use image::{GrayImage, RgbaImage};

/// How much a screen image is enlarged: screen letters are a dozen pixels
/// high, Tesseract reads best from about thirty.
const SCALE: f32 = 2.5;
/// The longest side of the enlarged image, in pixels.
const MAX_SIDE: f32 = 8000.0;
/// The resolution the enlarged image is said to have, in dots per inch.
pub const RESOLUTION: i32 = 300;

/// The image given to Tesseract and how much it was enlarged.
pub struct Prepared {
    pub image: GrayImage,
    pub scale: f32,
}

/// `image` in grey, inverted when it is light text on a dark ground, and
/// enlarged.
pub fn grey(image: &RgbaImage) -> Prepared {
    let mut grey = image::imageops::grayscale(image);
    let pixels = grey.as_raw().len().max(1) as u64;
    let mean = grey.as_raw().iter().map(|&value| value as u64).sum::<u64>() / pixels;
    if mean < 128 {
        image::imageops::invert(&mut grey);
    }
    let longest = image.width().max(image.height()).max(1) as f32;
    let scale = SCALE.min(MAX_SIDE / longest).max(1.0);
    if scale == 1.0 {
        return Prepared { image: grey, scale };
    }
    let width = (image.width() as f32 * scale).round() as u32;
    let height = (image.height() as f32 * scale).round() as u32;
    Prepared {
        image: image::imageops::resize(&grey, width, height, FilterType::CatmullRom),
        scale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dark_ground_is_made_light() {
        let mut image = RgbaImage::from_pixel(10, 10, image::Rgba([20, 20, 20, 255]));
        image.put_pixel(5, 5, image::Rgba([240, 240, 240, 255]));
        let prepared = grey(&image);
        assert_eq!(prepared.scale, SCALE);
        assert_eq!(prepared.image.width(), 25);
        assert!(prepared.image.get_pixel(0, 0)[0] > 200);
    }
}
