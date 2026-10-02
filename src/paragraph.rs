//! Lines of the recogniser joined into the paragraphs that are translated.

use std::ops::Range;

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
    /// The lines, in the order of `text`.
    pub lines: Vec<LineBox>,
}

/// Where a line of a paragraph is, and how much of its text it holds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineBox {
    /// The box of the line.
    pub rect: Rect,
    /// The letters of the line: its characters but spaces.
    pub letters: usize,
}

/// The characters of `text` but spaces: the measure of a part of a paragraph
/// that its pieces, however cut and trimmed, keep.
pub fn letters(text: &str) -> usize {
    text.chars().filter(|c| !c.is_whitespace()).count()
}

impl Paragraph {
    /// The part of the paragraph from its letter `range.start` to the one
    /// before `range.end`: its text, and the lines it covers. A line covered
    /// in part is narrowed to the share of its letters; the box of a
    /// paragraph without lines is kept whole.
    pub fn part(&self, range: Range<usize>) -> Paragraph {
        let mut lines = Vec::new();
        let mut first = 0;
        for line in &self.lines {
            let end = first + line.letters;
            let (start, stop) = (range.start.max(first), range.end.min(end));
            if start < stop {
                let share = |at: usize| (at - first) as f32 / line.letters as f32;
                let (left, width) = (line.rect.min.x, line.rect.width());
                lines.push(LineBox {
                    rect: Rect::from_x_y_ranges(
                        left + width * share(start)..=left + width * share(stop),
                        line.rect.y_range(),
                    ),
                    letters: stop - start,
                });
            }
            first = end;
        }
        let rect = lines
            .iter()
            .map(|line| line.rect)
            .reduce(Rect::union)
            .unwrap_or(self.rect);
        let line_height = if lines.is_empty() {
            self.line_height
        } else {
            lines.iter().map(|line| line.rect.height()).sum::<f32>() / lines.len() as f32
        };
        Paragraph {
            text: slice_letters(&self.text, range).to_owned(),
            rect,
            line_height,
            lines,
        }
    }
}

/// The text of `text` from its letter `range.start` to the one before
/// `range.end`, spaces counted not.
fn slice_letters(text: &str, range: Range<usize>) -> &str {
    let mut letters = text
        .char_indices()
        .filter(|(_, c)| !c.is_whitespace())
        .map(|(at, c)| (at, at + c.len_utf8()));
    if range.is_empty() {
        return "";
    }
    let Some((start, mut end)) = letters.nth(range.start) else {
        return "";
    };
    if range.end > range.start + 1 {
        end = letters
            .nth(range.end - range.start - 2)
            .map_or(text.len(), |(_, end)| end);
    }
    &text[start..end]
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
            let boxes = lines
                .iter()
                .map(|line| LineBox {
                    rect: Rect::from_min_size(
                        origin + vec2(line.x, line.y),
                        vec2(line.width, line.height),
                    ),
                    letters: letters(&line.text),
                })
                .collect();
            let text = lines
                .into_iter()
                .map(|line| line.text)
                .collect::<Vec<_>>()
                .join(" ");
            Paragraph {
                text,
                rect,
                line_height,
                lines: boxes,
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

    #[test]
    fn a_part_covers_its_lines() {
        let lines = vec![line("ab cd", 0.0, 0, 0), line("efgh", 10.0, 0, 0)];
        let paragraph = &group(lines, pos2(0.0, 0.0))[0];
        assert_eq!(paragraph.text, "ab cd efgh");
        let head = paragraph.part(0..2);
        assert_eq!(head.text, "ab");
        assert_eq!(
            head.rect,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(50.0, 10.0))
        );
        let tail = paragraph.part(2..6);
        assert_eq!(tail.text, "cd ef");
        assert_eq!(
            tail.rect,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 20.0))
        );
        assert_eq!(tail.lines.len(), 2);
        assert_eq!(paragraph.part(6..8).text, "gh");
    }
}
