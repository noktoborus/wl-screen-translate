//! How the recognised text is cut into the pieces that are translated: whole,
//! by paragraph or by sentence. A piece is translated and cached alone, and
//! shown as soon as it is.

use eframe::egui::Rect;
use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

use crate::paragraph::{self, Paragraph};

/// How the text is cut.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Split {
    /// The whole text is one piece, drawn on one plate.
    Whole,
    /// Each paragraph is a piece.
    Paragraphs,
    /// Each sentence of a paragraph is a piece.
    #[default]
    Sentences,
}

impl Split {
    /// Every way, in the order of the menu.
    pub const ALL: [Split; 3] = [Split::Whole, Split::Paragraphs, Split::Sentences];

    /// The name of the way in the menu and in `locales/app.yml`.
    pub fn id(self) -> &'static str {
        match self {
            Split::Whole => "whole",
            Split::Paragraphs => "paragraphs",
            Split::Sentences => "sentences",
        }
    }

    /// The way named `id`.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|split| split.id() == id)
    }
}

/// What is drawn on one plate, and the pieces its text is translated in.
#[derive(Debug, Clone, PartialEq)]
pub struct Unit {
    /// The place and the recognised text of the plate.
    pub paragraph: Paragraph,
    /// The pieces, in order.
    pub pieces: Vec<String>,
}

/// The plates of `paragraphs` cut `split`.
pub fn units(paragraphs: &[Paragraph], split: Split) -> Vec<Unit> {
    match split {
        Split::Whole if paragraphs.is_empty() => Vec::new(),
        Split::Whole => {
            let texts = || paragraphs.iter().map(|p| p.text.as_str());
            let rect = paragraphs
                .iter()
                .map(|p| p.rect)
                .reduce(Rect::union)
                .unwrap_or(Rect::NOTHING);
            let line_height =
                paragraphs.iter().map(|p| p.line_height).sum::<f32>() / paragraphs.len() as f32;
            vec![Unit {
                paragraph: Paragraph {
                    text: texts().collect::<Vec<_>>().join("\n"),
                    rect,
                    line_height,
                    lines: paragraphs.iter().flat_map(|p| p.lines.clone()).collect(),
                },
                pieces: vec![texts().collect::<Vec<_>>().join(" ")],
            }]
        }
        Split::Paragraphs => paragraphs
            .iter()
            .map(|paragraph| Unit {
                paragraph: paragraph.clone(),
                pieces: vec![paragraph.text.clone()],
            })
            .collect(),
        Split::Sentences => paragraphs
            .iter()
            .map(|paragraph| Unit {
                paragraph: paragraph.clone(),
                pieces: sentences(&paragraph.text),
            })
            .collect(),
    }
}

/// The sentences of `text`, by the rules of Unicode (UAX #29).
fn sentences(text: &str) -> Vec<String> {
    text.split_sentence_bounds()
        .map(str::trim)
        .filter(|sentence| !sentence.is_empty())
        .map(str::to_owned)
        .collect()
}

/// `text` cut into chunks of at most `max` tokens, as `count` counts them.
///
/// A chunk ends at the word boundary nearest the limit: a word goes into
/// this chunk or the next. A word longer than the limit alone is cut between
/// its letters. A text within the limit is one chunk.
pub fn limit<E>(
    text: &str,
    max: usize,
    mut count: impl FnMut(&str) -> Result<usize, E>,
) -> Result<Vec<String>, E> {
    if count(text)? <= max {
        return Ok(vec![text.to_owned()]);
    }
    let mut chunks = Vec::new();
    let mut chunk = String::new();
    let close = |chunk: &mut String, chunks: &mut Vec<String>| {
        let done = std::mem::take(chunk);
        if !done.trim().is_empty() {
            chunks.push(done.trim().to_owned());
        }
    };
    for word in text.split_word_bounds() {
        if count(format!("{chunk}{word}").trim())? <= max {
            chunk.push_str(word);
            continue;
        }
        close(&mut chunk, &mut chunks);
        if count(word.trim())? <= max {
            chunk.push_str(word.trim_start());
            continue;
        }
        for letter in word.graphemes(true) {
            if !chunk.is_empty() && count(&format!("{chunk}{letter}"))? > max {
                close(&mut chunk, &mut chunks);
            }
            chunk.push_str(letter);
        }
    }
    close(&mut chunk, &mut chunks);
    Ok(chunks)
}

/// `units` with each piece longer than `max` tokens cut by [`limit`], and how
/// many pieces more that made.
///
/// A cut ends a plate: the plate of a piece cut in two is cut in two as well,
/// the pieces before the cut going with the first part, those after with the
/// second, each drawn over the lines its text covers.
pub fn limit_units<E>(
    units: Vec<Unit>,
    max: usize,
    mut count: impl FnMut(&str) -> Result<usize, E>,
) -> Result<(Vec<Unit>, usize), E> {
    let mut cuts = 0;
    let mut limited = Vec::with_capacity(units.len());
    for unit in units {
        // The pieces of each part of the plate.
        let mut parts = vec![Vec::new()];
        for piece in &unit.pieces {
            for (index, chunk) in limit(piece, max, &mut count)?.into_iter().enumerate() {
                if index > 0 {
                    cuts += 1;
                    parts.push(Vec::new());
                }
                parts.last_mut().unwrap().push(chunk);
            }
        }
        if parts.len() == 1 {
            limited.push(unit);
            continue;
        }
        let mut first = 0;
        for pieces in parts {
            let end = first + pieces.iter().map(|p| paragraph::letters(p)).sum::<usize>();
            limited.push(Unit {
                paragraph: unit.paragraph.part(first..end),
                pieces,
            });
            first = end;
        }
    }
    Ok((limited, cuts))
}

/// The pieces of one plate joined back, as written in the language `code`:
/// Chinese and Japanese put no space between sentences.
pub fn join<'a>(pieces: impl IntoIterator<Item = &'a str>, code: &str) -> String {
    let unspaced = ["_Hans", "_Hant", "_Jpan"]
        .iter()
        .any(|script| code.ends_with(script));
    pieces
        .into_iter()
        .collect::<Vec<_>>()
        .join(if unspaced { "" } else { " " })
}

#[cfg(test)]
mod tests {
    use eframe::egui::{pos2, vec2};

    use super::*;

    fn paragraph(text: &str, y: f32) -> Paragraph {
        Paragraph {
            text: text.into(),
            rect: Rect::from_min_size(pos2(0.0, y), vec2(100.0, 10.0)),
            line_height: 10.0,
            lines: vec![paragraph::LineBox {
                rect: Rect::from_min_size(pos2(0.0, y), vec2(100.0, 10.0)),
                letters: paragraph::letters(text),
            }],
        }
    }

    #[test]
    fn sentences_are_cut_at_their_ends() {
        assert_eq!(
            sentences("Hello there. How are you? Fine!"),
            ["Hello there.", "How are you?", "Fine!"]
        );
        assert_eq!(sentences("你好。再见。"), ["你好。", "再见。"]);
    }

    #[test]
    fn the_whole_text_is_one_plate() {
        let paragraphs = [paragraph("One. Two.", 0.0), paragraph("Three.", 20.0)];
        let units = units(&paragraphs, Split::Whole);
        assert_eq!(units.len(), 1);
        assert_eq!(units[0].pieces, ["One. Two. Three."]);
        assert_eq!(units[0].paragraph.text, "One. Two.\nThree.");
        assert_eq!(units[0].paragraph.rect.height(), 30.0);
        assert_eq!(
            pieces(&paragraphs, Split::Sentences),
            ["One.", "Two.", "Three."]
        );
    }

    /// Every piece of `paragraphs` cut `split`, in order.
    fn pieces(paragraphs: &[Paragraph], split: Split) -> Vec<String> {
        units(paragraphs, split)
            .into_iter()
            .flat_map(|unit| unit.pieces)
            .collect()
    }

    /// One token a letter, as a tokenizer of letters would count.
    fn letters(text: &str) -> Result<usize, ()> {
        Ok(text.chars().count())
    }

    #[test]
    fn a_text_within_the_limit_is_kept() {
        assert_eq!(limit("one two", 7, letters), Ok(vec!["one two".to_owned()]));
    }

    #[test]
    fn chunks_end_at_the_word_nearest_the_limit() {
        assert_eq!(
            limit("one two three four", 9, letters),
            Ok(vec!["one two".into(), "three".into(), "four".into()])
        );
        assert_eq!(
            limit("aa bb cc dd", 5, letters),
            Ok(vec!["aa bb".into(), "cc dd".into()])
        );
    }

    #[test]
    fn a_word_longer_than_the_limit_is_cut_between_letters() {
        assert_eq!(
            limit("ab abcdefgh cd", 3, letters),
            Ok(vec![
                "ab".into(),
                "abc".into(),
                "def".into(),
                "gh".into(),
                "cd".into()
            ])
        );
    }

    #[test]
    fn a_cut_ends_a_plate() {
        let paragraphs = [paragraph("Ab cdef. Gh.", 0.0), paragraph("Four.", 20.0)];
        let units = units(&paragraphs, Split::Sentences);
        let (limited, cuts) = limit_units(units, 6, letters).unwrap();
        assert_eq!(cuts, 1);
        let plates: Vec<_> = limited
            .iter()
            .map(|unit| (unit.paragraph.text.as_str(), unit.pieces.clone()))
            .collect();
        assert_eq!(
            plates,
            [
                ("Ab", vec!["Ab".to_owned()]),
                ("cdef. Gh.", vec!["cdef.".into(), "Gh.".into()]),
                ("Four.", vec!["Four.".into()]),
            ]
        );
        // "Ab" is 2 of the 10 letters of the line, 100 points wide.
        assert_eq!(limited[0].paragraph.rect.width(), 20.0);
        assert_eq!(limited[1].paragraph.rect.min.x, 20.0);
        assert_eq!(limited[2], units_of(&paragraphs[1]));
    }

    /// The plate of `paragraph` alone, cut by sentence.
    fn units_of(paragraph: &Paragraph) -> Unit {
        units(std::slice::from_ref(paragraph), Split::Sentences).remove(0)
    }

    #[test]
    fn pieces_are_joined_as_the_language_writes() {
        assert_eq!(join(["Раз.", "Два."], "rus_Cyrl"), "Раз. Два.");
        assert_eq!(join(["一。", "二。"], "zho_Hans"), "一。二。");
    }
}
