//! Lines put in reading order and grouped into paragraphs by their places:
//! the detector knows nothing of paragraphs.

use crate::{Line, Rect};

/// How far apart the middles of two boxes of one row may be, in parts of the
/// lower box.
const ROW_SPREAD: f32 = 0.5;
/// The widest gap between two lines of a paragraph, in parts of the lower
/// line.
const LINE_GAP: f32 = 0.5;
/// How much two lines of a paragraph may overlap upwards, in parts of the
/// lower line.
const LINE_OVERLAP: f32 = 0.3;
/// The greatest ratio of the heights of two lines of a paragraph.
const HEIGHT_RATIO: f32 = 1.4;

/// The lines in reading order, rows from the top and boxes of a row from the
/// left, each numbered by its paragraph.
pub fn paragraphs(mut lines: Vec<(Rect, String)>) -> Vec<Line> {
    lines.sort_by(|a, b| middle(&a.0).total_cmp(&middle(&b.0)));
    let mut rows: Vec<Vec<(Rect, String)>> = Vec::new();
    for line in lines {
        match rows.last_mut() {
            Some(row) if same_row(&row[0].0, &line.0) => row.push(line),
            _ => rows.push(vec![line]),
        }
    }
    // The last line of each paragraph so far.
    let mut ends: Vec<Rect> = Vec::new();
    let mut out = Vec::new();
    for mut row in rows {
        row.sort_by(|a, b| a.0.x.total_cmp(&b.0.x));
        let row_ends = ends.clone();
        for (rect, text) in row {
            let found = row_ends
                .iter()
                .enumerate()
                .filter(|(_, end)| continues(end, &rect))
                .min_by(|a, b| (rect.y - a.1.bottom()).total_cmp(&(rect.y - b.1.bottom())))
                .map(|(index, _)| index);
            let paragraph = match found {
                Some(index) => {
                    ends[index] = rect;
                    index
                }
                None => {
                    ends.push(rect);
                    ends.len() - 1
                }
            } as i32;
            out.push(Line {
                text,
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: rect.height,
                block: paragraph,
                paragraph,
            });
        }
    }
    out
}

fn middle(rect: &Rect) -> f32 {
    rect.y + rect.height / 2.0
}

/// Whether `b` is on the row `a` starts.
fn same_row(a: &Rect, b: &Rect) -> bool {
    (middle(b) - middle(a)).abs() < ROW_SPREAD * a.height.min(b.height)
}

/// Whether `line` is the next line of the paragraph ending with `end`: just
/// below it, of a like height, and beside it.
fn continues(end: &Rect, line: &Rect) -> bool {
    let gap = line.y - end.bottom();
    let height = end.height.min(line.height);
    let ratio = end.height.max(line.height) / height.max(1.0);
    let overlap = end.right().min(line.right()) - end.x.max(line.x);
    gap < LINE_GAP * height
        && gap > -LINE_OVERLAP * height
        && ratio <= HEIGHT_RATIO
        && overlap > 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(x: f32, y: f32, width: f32, height: f32, text: &str) -> (Rect, String) {
        (
            Rect {
                x,
                y,
                width,
                height,
            },
            text.to_owned(),
        )
    }

    fn found(lines: Vec<(Rect, String)>) -> Vec<(String, i32)> {
        paragraphs(lines)
            .into_iter()
            .map(|line| (line.text, line.paragraph))
            .collect()
    }

    #[test]
    fn close_lines_make_a_paragraph() {
        let lines = vec![
            line(0.0, 22.0, 180.0, 20.0, "second"),
            line(0.0, 0.0, 200.0, 20.0, "first"),
            line(0.0, 44.0, 90.0, 20.0, "third"),
        ];
        assert_eq!(
            found(lines),
            [
                ("first".into(), 0),
                ("second".into(), 0),
                ("third".into(), 0)
            ]
        );
    }

    #[test]
    fn a_gap_a_column_or_a_heading_starts_a_paragraph() {
        let lines = vec![
            line(0.0, 0.0, 200.0, 30.0, "heading"),
            line(0.0, 34.0, 200.0, 16.0, "body"),
            line(300.0, 35.0, 100.0, 16.0, "column"),
            line(0.0, 52.0, 200.0, 16.0, "body 2"),
            line(0.0, 100.0, 200.0, 16.0, "after a gap"),
        ];
        assert_eq!(
            found(lines),
            [
                ("heading".into(), 0),
                ("body".into(), 1),
                ("column".into(), 2),
                ("body 2".into(), 1),
                ("after a gap".into(), 3),
            ]
        );
    }
}
