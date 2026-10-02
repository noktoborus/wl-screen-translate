//! The dimmed screen around the selected region, as slurp draws it.

use eframe::egui::{Color32, Painter, Rect, Stroke, StrokeKind, pos2};

/// The shade over everything outside the region.
const SHADE: Color32 = Color32::from_black_alpha(140);
/// The border of the region.
const BORDER: Stroke = Stroke {
    width: 2.0,
    color: Color32::WHITE,
};

/// Dims `screen` outside `region` and outlines the region.
pub fn paint(painter: &Painter, screen: Rect, region: Option<Rect>) {
    let Some(region) = region else {
        painter.rect_filled(screen, 0.0, SHADE);
        return;
    };
    let above = Rect::from_min_max(screen.min, pos2(screen.max.x, region.min.y));
    let below = Rect::from_min_max(pos2(screen.min.x, region.max.y), screen.max);
    let left = Rect::from_min_max(
        pos2(screen.min.x, region.min.y),
        pos2(region.min.x, region.max.y),
    );
    let right = Rect::from_min_max(
        pos2(region.max.x, region.min.y),
        pos2(screen.max.x, region.max.y),
    );
    for shade in [above, below, left, right] {
        painter.rect_filled(shade, 0.0, SHADE);
    }
    painter.rect_stroke(region, 0.0, BORDER, StrokeKind::Outside);
}
