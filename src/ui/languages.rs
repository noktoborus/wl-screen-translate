//! The floating window with the source and target languages, and the menu a
//! language is picked from.

use eframe::egui::{self, Align2, Context, Pos2, Rect, pos2, vec2};
use plate_menu::{MenuItem, PlateMenu};
use rust_i18n::t;

use crate::settings::{Direction, Settings};
use crate::split::Split;

/// The distance of the window from the top right corner, in points, until it
/// is moved.
const MARGIN: f32 = 16.0;
/// The width taken by the line between the languages and the recent
/// directions, in points.
const SEPARATOR_WIDTH: f32 = 6.0;

/// Which of the two languages a button or the menu stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Source,
    Target,
}

/// What the window did this frame.
pub struct Shown {
    /// The button of this language was pressed.
    pub pressed: Option<Side>,
    /// A recent direction other than the one in use was clicked.
    pub direction: Option<Direction>,
    /// The translate switch was clicked.
    pub toggled: bool,
    /// The copy-and-quit button was clicked.
    pub copy: bool,
    /// The split button was clicked.
    pub split: bool,
    /// The close button of the window was clicked.
    pub closed: bool,
    /// The top left corner of the window, once it has been laid out.
    pub position: Option<Pos2>,
}

/// Shows the window where it was left on the monitor `monitor`, or in the top
/// right corner of `screen`.
///
/// The position only seeds the window: egui keeps it while the program runs,
/// one for each monitor window, and the window is free to be dragged. The
/// copy button is enabled when there is `text` to copy; `stats` are lines of
/// a name and a value.
pub fn window(
    context: &Context,
    settings: &Settings,
    monitor: &str,
    screen: Rect,
    text: bool,
    stats: &[(String, String)],
) -> Shown {
    let mut pressed = None;
    let mut direction = None;
    let mut toggled = false;
    let mut copy = false;
    let mut split = false;
    let mut open = true;
    let window = egui::Window::new(t!("ui.languages"))
        .open(&mut open)
        .resizable(false)
        .collapsible(true);
    let window = match settings.language_window.get(monitor) {
        Some(&[x, y]) => window.default_pos(pos2(x, y)),
        None => window
            .pivot(Align2::RIGHT_TOP)
            .default_pos(screen.right_top() + vec2(-MARGIN, MARGIN)),
    };
    let shown = window.show(context, |ui| {
        // Every text on one line: the window grows to fit it, however narrow
        // egui would keep it.
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        let row = ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                // The switch and the copy button are as wide as the column,
                // known once all of it is laid out: their places are kept,
                // as wide as their text, and they are put there last.
                let height = ui.spacing().interact_size.y;
                let translate = t!("ui.translate");
                let (_, switch_slot) =
                    ui.allocate_space(vec2(button_width(ui, &translate), height));
                let languages = [
                    (Side::Source, t!("ui.source"), local_name(&settings.source)),
                    (Side::Target, t!("ui.target"), local_name(&settings.target)),
                ];
                // The language buttons are as wide as the widest.
                let width = languages
                    .iter()
                    .map(|(_, _, name)| button_width(ui, name))
                    .fold(0.0, f32::max);
                egui::Grid::new("languages").num_columns(2).show(ui, |ui| {
                    for (side, label, name) in languages {
                        ui.label(label);
                        if ui
                            .add_sized(vec2(width, height), egui::Button::new(name))
                            .clicked()
                        {
                            pressed = Some(side);
                        }
                        ui.end_row();
                    }
                });
                let copy_text = t!("ui.copy");
                let (_, copy_slot) = ui.allocate_space(vec2(button_width(ui, &copy_text), height));
                let split_name = t!(format!("ui.split.{}", settings.split.id()));
                // Named by the split, but kept open when it changes; closed at
                // start, as egui keeps no memory between runs here.
                egui::CollapsingHeader::new(split_name.as_ref())
                    .id_salt("split")
                    .default_open(false)
                    .show(ui, |ui| {
                        split = ui.button(split_name.as_ref()).clicked();
                        egui::Grid::new("stats").num_columns(2).show(ui, |ui| {
                            for (name, value) in stats {
                                ui.label(name);
                                ui.label(value);
                                ui.end_row();
                            }
                        });
                    });
                let full =
                    |slot: Rect| Rect::from_min_size(slot.min, vec2(ui.min_rect().width(), height));
                let (switch_slot, copy_slot) = (full(switch_slot), full(copy_slot));
                let switch = egui::Button::selectable(settings.translate, translate);
                toggled = ui.put(switch_slot, switch).clicked();
                copy = ui
                    .add_enabled_ui(text, |ui| ui.put(copy_slot, egui::Button::new(copy_text)))
                    .inner
                    .clicked();
            });
            if settings.recent.is_empty() {
                return None;
            }
            // Not a separator: one in a row takes the height left in the
            // window, which stretches it to the bottom of the screen.
            let (_, line) = ui.allocate_space(vec2(SEPARATOR_WIDTH, 0.0));
            direction = recent(ui, settings);
            Some(line.center().x)
        });
        if let Some(x) = row.inner {
            let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
            ui.painter().vline(x, row.response.rect.y_range(), stroke);
        }
    });
    Shown {
        pressed,
        direction,
        toggled,
        copy,
        split,
        closed: !open,
        position: shown.map(|shown| shown.response.rect.min),
    }
}

/// Lists the recent directions, the one in use selected, and returns the one
/// clicked unless it is in use.
fn recent(ui: &mut egui::Ui, settings: &Settings) -> Option<Direction> {
    let current = settings.direction();
    let mut clicked = None;
    ui.vertical(|ui| {
        for direction in &settings.recent {
            let text = format!(
                "{} / {}",
                local_name(&direction.source),
                local_name(&direction.target)
            );
            let selected = *direction == current;
            if ui.selectable_label(selected, text).clicked() && !selected {
                clicked = Some(direction.clone());
            }
        }
    });
    clicked
}

/// The width of a button with `text`, in points.
fn button_width(ui: &egui::Ui, text: &str) -> f32 {
    let galley = egui::WidgetText::from(text).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Button,
    );
    galley.size().x + 2.0 * ui.spacing().button_padding.x
}

/// Opens `menu` on the ways to split the text, the one in use selected.
pub fn open_split(menu: &mut PlateMenu, settings: &Settings) {
    let items = Split::ALL
        .iter()
        .map(|split| MenuItem::new(split.id(), t!(format!("ui.split.{}", split.id()))))
        .collect();
    match menu.open_at(items, settings.split.id()) {
        Ok(()) => menu.notice(t!("ui.pick_split")),
        Err(error) => log::error!("split menu: {error}"),
    }
}

/// Opens `menu` on the languages of [`items`], the one in use for `side`
/// selected.
pub fn open(menu: &mut PlateMenu, settings: &Settings, side: Side) {
    let (current, notice) = match side {
        Side::Source => (&settings.source, t!("ui.pick_source")),
        Side::Target => (&settings.target, t!("ui.pick_target")),
    };
    match menu.open_at(items(), current) {
        Ok(()) => menu.notice(notice),
        Err(error) => log::error!("language menu: {error}"),
    }
}

/// An entry for each language: its name in its own script, with its name in
/// the language of the interface and its code beside it; found by any of the
/// three.
fn items() -> Vec<MenuItem> {
    nllb::LANGUAGES
        .iter()
        .map(|code| {
            let name = nllb::autonym(code).unwrap_or(code);
            let local = local_name(code);
            MenuItem::new(*code, name)
                .detail(format!("{local} · {code}"))
                .search(format!("{local} {code}"))
        })
        .collect()
}

/// The name of the language `code` in the language of the interface, from
/// `locales/languages.yml`.
fn local_name(code: &str) -> String {
    t!(format!("language.{code}")).into_owned()
}

/// The key of a monitor in the settings when its connector is not known: its
/// size in pixels.
pub fn monitor_key(context: &Context, screen: Rect) -> String {
    let points = context
        .input(|input| input.viewport().monitor_size)
        .unwrap_or(screen.size());
    let pixels = points * context.pixels_per_point();
    format!("{}x{}", pixels.x.round(), pixels.y.round())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Direction;

    /// The language chosen after typing `query` into the menu.
    fn found(query: &str) -> Option<String> {
        let mut state = plate_menu::MenuState::new(items());
        state.push_query(query);
        state.accept()
    }

    #[test]
    fn languages_are_found_by_name_and_code() {
        assert_eq!(found("Deutsch").as_deref(), Some("deu_Latn"));
        assert_eq!(found("German").as_deref(), Some("deu_Latn"));
        assert_eq!(found("deu_Latn").as_deref(), Some("deu_Latn"));
        assert_eq!(found("русский").as_deref(), Some("rus_Cyrl"));
        assert_eq!(found("日本語").as_deref(), Some("jpn_Jpan"));
        assert_eq!(found("zho_Hant").as_deref(), Some("zho_Hant"));
    }

    #[test]
    fn entries_name_the_language_in_the_interface_language() {
        let german = items()
            .into_iter()
            .find(|item| item.id == "deu_Latn")
            .unwrap();
        assert_eq!(german.label, "Deutsch");
        assert_eq!(german.detail, "German · deu_Latn");
    }

    #[test]
    fn window_fits_its_content() {
        let context = Context::default();
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 2000.0));
        let mut settings = Settings::default();
        settings.recent.push(Direction {
            source: settings.source.clone(),
            target: settings.target.clone(),
        });
        let input = || egui::RawInput {
            screen_rect: Some(screen),
            ..Default::default()
        };
        for _ in 0..3 {
            let mut output = context.run_ui(input(), |ui| {
                window(ui.ctx(), &settings, "test", screen, true, &[]);
            });
            output.textures_delta.clear();
        }
        // egui names a window by its title, as `Atoms::text` gives it.
        let id = egui::Id::new(Some(t!("ui.languages")));
        let rect = context.memory(|memory| memory.area_rect(id)).unwrap();
        assert!(rect.height() < 200.0, "{rect:?}");
    }

    /// Every text of the window laid out with `shapes`, with its lines.
    fn texts(shape: &eframe::epaint::Shape, out: &mut Vec<(String, usize)>) {
        use eframe::epaint::Shape;
        match shape {
            Shape::Text(text) => out.push((text.galley.text().to_owned(), text.galley.rows.len())),
            Shape::Vec(shapes) => shapes.iter().for_each(|shape| texts(shape, out)),
            _ => {}
        }
    }

    #[test]
    fn no_text_wraps() {
        let context = Context::default();
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 2000.0));
        // The longest names, in English.
        let mut settings = Settings {
            source: "tzm_Tfng".into(),
            target: "apc_Arab".into(),
            ..Settings::default()
        };
        settings.remember_direction();
        let stats = [("Selected".to_owned(), "640x200 pixels".to_owned())];
        let mut wrapped = Vec::new();
        for _ in 0..3 {
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    ..Default::default()
                },
                |ui| {
                    window(ui.ctx(), &settings, "test", screen, true, &stats);
                },
            );
            output.textures_delta.clear();
            let mut laid_out = Vec::new();
            for clipped in &output.shapes {
                texts(&clipped.shape, &mut laid_out);
            }
            wrapped = laid_out.into_iter().filter(|(_, rows)| *rows > 1).collect();
        }
        assert!(wrapped.is_empty(), "{wrapped:?}");
    }
}
