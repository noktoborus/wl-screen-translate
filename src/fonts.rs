//! Fonts for the letters egui's own fonts lack, found when a text needs them.
//!
//! Nothing is looked for at start. When a text shown has letters no font has,
//! they go to a thread that asks fontconfig for a sans-serif face with each;
//! a face found has the other letters it covers struck off, so one face is
//! taken for a script. The face's file is memory-mapped and egui borrows the
//! mapping: only the pages of the glyphs drawn are loaded, and they are clean
//! page cache shared with other programs. A mapping lives until the program
//! exits.

use std::collections::HashSet;
use std::sync::mpsc::{Receiver, Sender, channel};

use eframe::egui::{Context, FontFamily, FontId};
use eframe::epaint::text::{FontInsert, FontPriority, InsertFontFamily};

/// The size letters are looked up at: any will do, the glyphs are the same.
const PROBE_SIZE: f32 = 14.0;

/// The handle of the font thread.
pub struct Fonts {
    requests: Sender<Vec<char>>,
    found: Receiver<FontInsert>,
    /// The letters sent once: each is looked for at most once.
    asked: HashSet<char>,
}

impl Fonts {
    /// Starts the thread, idle until a letter is missing; it wakes the
    /// interface after each font found.
    pub fn spawn(context: Context) -> Self {
        let (requests, letters) = channel();
        let (fonts, found) = channel();
        std::thread::spawn(move || serve(&letters, &fonts, &context));
        Self {
            requests,
            found,
            asked: HashSet::new(),
        }
    }

    /// Asks for fonts for the letters of `texts` no font has.
    pub fn cover<'a>(&mut self, context: &Context, texts: impl IntoIterator<Item = &'a str>) {
        let font = FontId::proportional(PROBE_SIZE);
        let missing: Vec<char> = context.fonts_mut(|fonts| {
            texts
                .into_iter()
                .flat_map(str::chars)
                .filter(|letter| !letter.is_whitespace() && !letter.is_control())
                .filter(|letter| self.asked.insert(*letter))
                .filter(|letter| !fonts.has_glyph(&font, *letter))
                .collect()
        });
        if !missing.is_empty() {
            log::info!(
                "{} letters without a font, looking for fonts",
                missing.len()
            );
            let _ = self.requests.send(missing);
        }
    }

    /// Adds the fonts found since the last call, after egui's own.
    pub fn install(&self, context: &Context) {
        while let Ok(font) = self.found.try_recv() {
            context.add_font(font);
        }
    }
}

/// Finds a face for each letter sent, until the interface goes.
#[cfg(target_os = "linux")]
fn serve(letters: &Receiver<Vec<char>>, fonts: &Sender<FontInsert>, context: &Context) {
    use std::collections::HashMap;
    use std::time::Instant;

    use crate::fontconfig::Fontconfig;

    let mut fontconfig = None;
    // The faces taken, by file and index, and the mapping of each file.
    let mut taken = HashSet::new();
    let mut files: HashMap<std::path::PathBuf, &'static [u8]> = HashMap::new();
    for mut left in letters {
        let fontconfig = match &fontconfig {
            Some(fontconfig) => fontconfig,
            None => {
                let started = Instant::now();
                match Fontconfig::load() {
                    Ok(loaded) => {
                        log::info!("fontconfig loaded in {:?}", started.elapsed());
                        fontconfig.insert(loaded)
                    }
                    Err(error) => {
                        log::warn!("no fonts beyond egui's own: {error}");
                        return;
                    }
                }
            }
        };
        while let Some(&letter) = left.first() {
            let started = Instant::now();
            let Some(face) = fontconfig.face_for(letter) else {
                log::info!("no font has {letter:?} (U+{:04X})", u32::from(letter));
                left.remove(0);
                continue;
            };
            left.retain(|letter| !face.has(*letter));
            if !taken.insert((face.path.clone(), face.index)) {
                continue;
            }
            let bytes = match files.get(&face.path) {
                Some(bytes) => *bytes,
                None => match map(&face.path) {
                    Ok(bytes) => *files.entry(face.path.clone()).or_insert(bytes),
                    Err(error) => {
                        log::warn!("cannot map the font {}: {error}", face.path.display());
                        continue;
                    }
                },
            };
            let name = format!("{}#{}", face.path.display(), face.index);
            log::info!(
                "font {name} for {letter:?} (U+{:04X}) in {:?}",
                u32::from(letter),
                started.elapsed()
            );
            let mut data = eframe::egui::FontData::from_static(bytes);
            data.index = face.index;
            let lowest = |family| InsertFontFamily {
                family,
                priority: FontPriority::Lowest,
            };
            let font = FontInsert::new(
                &name,
                data,
                vec![
                    lowest(FontFamily::Proportional),
                    lowest(FontFamily::Monospace),
                ],
            );
            if fonts.send(font).is_err() {
                return;
            }
            context.request_repaint();
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn serve(letters: &Receiver<Vec<char>>, _fonts: &Sender<FontInsert>, _context: &Context) {
    for _ in letters {
        log::info!("fonts beyond egui's own are looked for on Linux only");
    }
}

/// Maps the file at `path` for the rest of the program.
#[cfg(target_os = "linux")]
fn map(path: &std::path::Path) -> std::io::Result<&'static [u8]> {
    let file = std::fs::File::open(path)?;
    // SAFETY: font files are not written while installed; a file truncated
    // while mapped would fault on a read.
    let mmap = unsafe { memmap2::Mmap::map(&file)? };
    // Glyphs are read here and there: reading ahead would load pages of
    // glyphs never drawn.
    if let Err(error) = mmap.advise(memmap2::Advice::Random) {
        log::debug!("cannot advise the mapping of {}: {error}", path.display());
    }
    Ok(Box::leak(Box::new(mmap)))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use std::time::Duration;

    use super::*;

    /// A letter of a script egui's fonts lack.
    const THAI: char = 'ก';

    fn frame(context: &Context) {
        let mut output = context.run_ui(Default::default(), |_| {});
        output.textures_delta.clear();
    }

    fn has_glyph(context: &Context, letter: char) -> bool {
        context.fonts_mut(|fonts| fonts.has_glyph(&FontId::proportional(PROBE_SIZE), letter))
    }

    #[test]
    fn adds_a_font_for_a_missing_letter() {
        let system_has_it = crate::fontconfig::Fontconfig::load()
            .is_ok_and(|fontconfig| fontconfig.face_for(THAI).is_some());
        if !system_has_it {
            eprintln!("skipped: no font of the system has {THAI}");
            return;
        }
        let context = Context::default();
        frame(&context);
        assert!(!has_glyph(&context, THAI));
        let mut fonts = Fonts::spawn(context.clone());
        fonts.cover(&context, ["สวัสดี"]);
        let font = fonts.found.recv_timeout(Duration::from_secs(5)).unwrap();
        context.add_font(font);
        frame(&context);
        assert!(has_glyph(&context, THAI));
    }

    #[test]
    fn asks_for_a_letter_once() {
        let context = Context::default();
        frame(&context);
        let mut fonts = Fonts::spawn(context.clone());
        fonts.cover(&context, ["\u{E000}"]);
        fonts.cover(&context, ["\u{E000} a"]);
        assert_eq!(fonts.asked, HashSet::from(['\u{E000}', 'a']));
    }
}
