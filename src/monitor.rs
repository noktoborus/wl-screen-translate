//! The monitors, found in a screenshot of the whole desktop.
//!
//! The screenshot portal has no choice of monitor: it returns every monitor in
//! one image. Each monitor gets a fullscreen window that shows its own part.

/// A monitor in the logical coordinates of the desktop.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Monitor {
    /// The left edge.
    pub x: f64,
    /// The top edge.
    pub y: f64,
    /// The width.
    pub width: f64,
    /// The height.
    pub height: f64,
}

/// A rectangle of the screenshot, in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Crop {
    /// The left edge.
    pub x: u32,
    /// The top edge.
    pub y: u32,
    /// The width.
    pub width: u32,
    /// The height.
    pub height: u32,
}

/// The largest difference, in pixels, between the screenshot and the layout
/// scaled to its width that still counts as the same shape: rounding of
/// fractional scales.
const SHAPE_TOLERANCE: f64 = 2.0;

impl Monitor {
    /// The monitor `winit` reports, from physical to logical coordinates.
    pub fn from_winit(monitor: &winit::monitor::MonitorHandle) -> Self {
        let scale = monitor.scale_factor();
        let position = monitor.position();
        let size = monitor.size();
        Self {
            x: f64::from(position.x) / scale,
            y: f64::from(position.y) / scale,
            width: f64::from(size.width) / scale,
            height: f64::from(size.height) / scale,
        }
    }
}

/// Where `current` lies in a screenshot of `size` pixels that shows `monitors`.
///
/// The compositors lay the monitors out in logical coordinates and draw the
/// screenshot at one scale for all of them: GNOME at the largest scale of a
/// monitor, others at 1. The scale is therefore taken from the widths of the
/// layout and the image; `None` when the heights do not agree with it, which
/// means the layout is not the one the screenshot was taken of.
pub fn locate(monitors: &[Monitor], current: Monitor, size: (u32, u32)) -> Option<Crop> {
    let left = monitors.iter().map(|m| m.x).fold(f64::INFINITY, f64::min);
    let top = monitors.iter().map(|m| m.y).fold(f64::INFINITY, f64::min);
    let right = monitors
        .iter()
        .map(|m| m.x + m.width)
        .fold(f64::NEG_INFINITY, f64::max);
    let bottom = monitors
        .iter()
        .map(|m| m.y + m.height)
        .fold(f64::NEG_INFINITY, f64::max);
    let (width, height) = (f64::from(size.0), f64::from(size.1));
    if !(right > left && bottom > top) || width == 0.0 {
        return None;
    }
    let scale = width / (right - left);
    if ((bottom - top) * scale - height).abs() > SHAPE_TOLERANCE {
        return None;
    }
    let x = ((current.x - left) * scale).round().clamp(0.0, width);
    let y = ((current.y - top) * scale).round().clamp(0.0, height);
    let crop_width = (current.width * scale).round().min(width - x);
    let crop_height = (current.height * scale).round().min(height - y);
    if crop_width < 1.0 || crop_height < 1.0 {
        return None;
    }
    Some(Crop {
        x: x as u32,
        y: y as u32,
        width: crop_width as u32,
        height: crop_height as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(x: f64, y: f64, width: f64, height: f64) -> Monitor {
        Monitor {
            x,
            y,
            width,
            height,
        }
    }

    fn crop(x: u32, y: u32, width: u32, height: u32) -> Option<Crop> {
        Some(Crop {
            x,
            y,
            width,
            height,
        })
    }

    #[test]
    fn single_monitor_is_the_whole_image() {
        let only = monitor(0.0, 0.0, 1920.0, 1080.0);
        assert_eq!(locate(&[only], only, (1920, 1080)), crop(0, 0, 1920, 1080));
    }

    #[test]
    fn side_by_side_at_scale_one() {
        let left = monitor(0.0, 0.0, 1080.0, 1920.0);
        let right = monitor(1080.0, 0.0, 1920.0, 1080.0);
        let all = [left, right];
        assert_eq!(locate(&all, left, (3000, 1920)), crop(0, 0, 1080, 1920));
        assert_eq!(locate(&all, right, (3000, 1920)), crop(1080, 0, 1920, 1080));
    }

    #[test]
    fn layout_starting_off_origin() {
        let left = monitor(-1920.0, 0.0, 1920.0, 1080.0);
        let right = monitor(0.0, 0.0, 1920.0, 1080.0);
        assert_eq!(
            locate(&[left, right], right, (3840, 1080)),
            crop(1920, 0, 1920, 1080)
        );
    }

    #[test]
    fn mixed_scales_drawn_at_the_largest() {
        // A 4K monitor at 200 % and a 1080p one at 100 %, drawn at scale 2.
        let large = monitor(0.0, 0.0, 1920.0, 1080.0);
        let small = monitor(1920.0, 0.0, 1920.0, 1080.0);
        assert_eq!(
            locate(&[large, small], small, (7680, 2160)),
            crop(3840, 0, 3840, 2160)
        );
    }

    #[test]
    fn foreign_layout_is_refused() {
        let left = monitor(0.0, 0.0, 1920.0, 1080.0);
        let right = monitor(1920.0, 0.0, 1920.0, 1080.0);
        assert_eq!(locate(&[left, right], right, (3000, 1920)), None);
    }

    #[test]
    fn no_monitors_is_refused() {
        let only = monitor(0.0, 0.0, 1920.0, 1080.0);
        assert_eq!(locate(&[], only, (1920, 1080)), None);
    }
}
