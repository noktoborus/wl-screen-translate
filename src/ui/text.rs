//! Text drawn over the screenshot: paragraphs, the hint and errors.

use std::sync::Arc;

use eframe::egui::{Align2, Color32, FontId, Painter, Rect, Shape, vec2};
use eframe::epaint::{Galley, Mesh, Tessellator};

/// The share of a line's height the font takes.
const FONT_TO_LINE: f32 = 0.75;
/// The smallest font size, in points.
const MIN_FONT: f32 = 10.0;
/// The largest font size, in points.
const MAX_FONT: f32 = 64.0;
/// The plate under a paragraph.
const PLATE: Color32 = Color32::from_black_alpha(225);
/// The size of the hint and error text, in points.
const NOTICE_FONT: f32 = 20.0;
/// The distance of the hint from the top edge, in points.
const NOTICE_MARGIN: f32 = 24.0;
/// The space between a notice and the edge of its plate, in points.
const NOTICE_PADDING: f32 = 8.0;
/// The colour of an error.
const ERROR: Color32 = Color32::from_rgb(255, 110, 110);
/// The text laid out every frame to notice egui rebuilding its fonts.
const PROBE: &str = "·";

/// A paragraph to draw: `text` on a plate over `rect`, sized for lines
/// `line_height` points tall.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Paragraph<'a> {
    pub rect: Rect,
    pub line_height: f32,
    pub text: &'a str,
}

/// The paragraphs of a screen, laid out and tessellated once and drawn as the
/// same mesh while nothing it depends on changes.
///
/// The mesh holds the places of the glyphs in egui's font atlas: it is made
/// again when the paragraphs, the scale or the size of the atlas change, or
/// when egui rebuilds its fonts (a new font, the atlas nearly full), which
/// shows as a new galley of [`PROBE`], laid out every frame.
#[derive(Default)]
pub struct Plates {
    paragraphs: Vec<(Rect, f32, String)>,
    pixels_per_point: f32,
    atlas_size: [usize; 2],
    probe: Option<Arc<Galley>>,
    mesh: Arc<Mesh>,
}

impl Plates {
    /// Draws `paragraphs`, tessellating them only when they or the fonts
    /// changed.
    pub fn paint(&mut self, painter: &Painter, paragraphs: &[Paragraph<'_>]) {
        let context = painter.ctx();
        let probe = painter.layout_no_wrap(
            PROBE.to_owned(),
            FontId::proportional(MIN_FONT),
            Color32::WHITE,
        );
        let pixels_per_point = context.pixels_per_point();
        let atlas_size = context.fonts(|fonts| fonts.font_image_size());
        let unchanged = self
            .probe
            .as_ref()
            .is_some_and(|old| Arc::ptr_eq(old, &probe))
            && self.pixels_per_point == pixels_per_point
            && self.atlas_size == atlas_size
            && self.paragraphs.len() == paragraphs.len()
            && self.paragraphs.iter().zip(paragraphs).all(|(old, new)| {
                old.0 == new.rect && old.1 == new.line_height && old.2 == new.text
            });
        if !unchanged {
            self.tessellate(painter, paragraphs);
            self.probe = Some(probe);
        }
        if !self.mesh.is_empty() {
            painter.add(Shape::mesh(Arc::clone(&self.mesh)));
        }
    }

    fn tessellate(&mut self, painter: &Painter, paragraphs: &[Paragraph<'_>]) {
        let shapes: Vec<Shape> = paragraphs
            .iter()
            .flat_map(|paragraph| plate(painter, paragraph))
            .collect();
        // Laying out may have grown the atlas: its size is read after.
        let context = painter.ctx();
        self.pixels_per_point = context.pixels_per_point();
        self.atlas_size = context.fonts(|fonts| fonts.font_image_size());
        let options = context.tessellation_options(|options| *options);
        let mut tessellator =
            Tessellator::new(self.pixels_per_point, options, self.atlas_size, Vec::new());
        let mut mesh = Mesh::default();
        for shape in shapes {
            tessellator.tessellate_shape(shape, &mut mesh);
        }
        log::debug!(
            "{} paragraphs tessellated into {} vertices",
            paragraphs.len(),
            mesh.vertices.len()
        );
        self.mesh = Arc::new(mesh);
        self.paragraphs = paragraphs
            .iter()
            .map(|paragraph| {
                (
                    paragraph.rect,
                    paragraph.line_height,
                    paragraph.text.to_owned(),
                )
            })
            .collect();
    }
}

/// The plate and the text of `paragraph`.
fn plate(painter: &Painter, paragraph: &Paragraph<'_>) -> [Shape; 2] {
    let Paragraph {
        rect,
        line_height,
        text,
    } = *paragraph;
    let size = (line_height * FONT_TO_LINE).clamp(MIN_FONT, MAX_FONT);
    let galley = painter.layout(
        text.to_owned(),
        FontId::proportional(size),
        Color32::WHITE,
        rect.width().max(size),
    );
    let plate = Rect::from_min_size(
        rect.min,
        vec2(
            rect.width().max(galley.size().x),
            rect.height().max(galley.size().y),
        ),
    );
    [
        Shape::rect_filled(plate, 2.0, PLATE),
        Shape::galley(plate.min, galley, Color32::WHITE),
    ]
}

/// Draws the usage hint at the top of the screen.
pub fn hint(painter: &Painter, screen: Rect, text: &str) {
    notice(painter, screen, text, Color32::WHITE);
}

/// Draws an error at the top of the screen.
pub fn error(painter: &Painter, screen: Rect, text: &str) {
    notice(painter, screen, text, ERROR);
}

fn notice(painter: &Painter, screen: Rect, text: &str, color: Color32) {
    let anchor = screen.center_top() + vec2(0.0, NOTICE_MARGIN);
    let galley = painter.layout_no_wrap(text.to_owned(), FontId::proportional(NOTICE_FONT), color);
    let rect = Align2::CENTER_TOP.anchor_size(anchor, galley.size());
    painter.rect_filled(rect.expand(NOTICE_PADDING), 4.0, PLATE);
    painter.galley(rect.min, galley, color);
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Context, FontData, LayerId, pos2};
    use eframe::epaint::text::{FontInsert, FontPriority, InsertFontFamily};

    use super::*;

    /// Draws `text` with `plates` in a frame and returns the mesh drawn.
    fn draw(context: &Context, plates: &mut Plates, text: &str) -> Arc<Mesh> {
        let paragraph = Paragraph {
            rect: Rect::from_min_size(pos2(10.0, 10.0), vec2(200.0, 40.0)),
            line_height: 20.0,
            text,
        };
        let mut output = context.run_ui(Default::default(), |ui| {
            let painter = ui.ctx().layer_painter(LayerId::background());
            plates.paint(&painter, &[paragraph]);
        });
        output.textures_delta.clear();
        Arc::clone(&plates.mesh)
    }

    #[test]
    fn keeps_the_mesh_while_nothing_changes() {
        let context = Context::default();
        let mut plates = Plates::default();
        let first = draw(&context, &mut plates, "Привет");
        assert!(!first.is_empty());
        let second = draw(&context, &mut plates, "Привет");
        assert!(Arc::ptr_eq(&first, &second));
        let other = draw(&context, &mut plates, "Hello");
        assert!(!Arc::ptr_eq(&second, &other));
    }

    #[test]
    fn makes_the_mesh_again_when_the_fonts_are_rebuilt() {
        let context = Context::default();
        let mut plates = Plates::default();
        let first = draw(&context, &mut plates, "Привет");
        let definitions = eframe::egui::FontDefinitions::default();
        let (_, font) = definitions.font_data.first_key_value().unwrap();
        context.add_font(FontInsert::new(
            "again",
            FontData::clone(font),
            vec![InsertFontFamily {
                family: eframe::egui::FontFamily::Proportional,
                priority: FontPriority::Lowest,
            }],
        ));
        let _ = draw(&context, &mut plates, "Привет");
        let rebuilt = draw(&context, &mut plates, "Привет");
        assert!(!Arc::ptr_eq(&first, &rebuilt));
    }
}
