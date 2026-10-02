//! Lines of the recogniser joined into the paragraphs that are translated.

use eframe::egui::{Pos2, Rect, vec2};

use crate::ocr::Line;

/// A paragraph, in pixels of the screenshot.
#[derive(Debug, Clone, PartialEq)]
pub struct Paragraph {
    /// The lines joined by spaces.
    pub text: String,
    /// The union of the boxes of the lines.
    pub rect: Rect,
    /// The mean height of the lines.
    pub line_height: f32,
}

/// Groups lines by their block and paragraph, in order of first appearance.
///
/// `origin` is where the recognised image starts in the screenshot.
pub fn group(lines: Vec<Line>, origin: Pos2) -> Vec<Paragraph> {
    let mut keys: Vec<(i32, i32)> = Vec::new();
    let mut groups: Vec<Vec<Line>> = Vec::new();
    for line in lines {
        let key = (line.block, line.paragraph);
        match keys.iter().position(|known| *known == key) {
            Some(index) => groups[index].push(line),
            None => {
                keys.push(key);
                groups.push(vec![line]);
            }
        }
    }
    groups
        .into_iter()
        .map(|lines| {
            let rect = lines
                .iter()
                .map(|line| {
                    Rect::from_min_size(
                        origin + vec2(line.x, line.y),
                        vec2(line.width, line.height),
                    )
                })
                .reduce(|a, b| a.union(b))
                .unwrap_or(Rect::from_min_size(origin, vec2(0.0, 0.0)));
            let line_height =
                lines.iter().map(|line| line.height).sum::<f32>() / lines.len() as f32;
            let text = lines
                .into_iter()
                .map(|line| line.text)
                .collect::<Vec<_>>()
                .join(" ");
            Paragraph {
                text,
                rect,
                line_height,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::pos2;

    fn line(text: &str, y: f32, block: i32, paragraph: i32) -> Line {
        Line {
            text: text.into(),
            x: 0.0,
            y,
            width: 100.0,
            height: 10.0,
            block,
            paragraph,
        }
    }

    #[test]
    fn joins_lines_of_one_paragraph() {
        let lines = vec![
            line("a", 0.0, 0, 0),
            line("x", 50.0, 1, 0),
            line("b", 10.0, 0, 0),
        ];
        let paragraphs = group(lines, pos2(5.0, 5.0));
        assert_eq!(paragraphs.len(), 2);
        assert_eq!(paragraphs[0].text, "a b");
        assert_eq!(
            paragraphs[0].rect,
            Rect::from_min_max(pos2(5.0, 5.0), pos2(105.0, 25.0))
        );
        assert_eq!(paragraphs[1].text, "x");
    }
}
