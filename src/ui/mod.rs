//! The fullscreen window: the screenshot, the region, the text and the languages.

mod languages;
mod region;
mod text;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Color32, ColorImage, Context, PointerButton, Rect, Sense, TextureHandle, TextureOptions,
    Vec2, ViewportBuilder, ViewportCommand, ViewportId, pos2,
};
use image::RgbaImage;
use image::imageops::FilterType;
use plate_menu::PlateMenu;
use rust_i18n::t;

use crate::APP_ID;
use crate::error::AppError;
use crate::fonts::Fonts;
use crate::gpu::ATLAS_SIDE;
use crate::ocr::Ocr;
use crate::paragraph::Paragraph;
use crate::settings::{Direction, Settings};
use crate::split::{self, Split, Unit};
use crate::worker::{Languages, Request, Response, Worker};

use self::languages::Side;

/// The side of the progress spinner, in points.
const SPINNER_SIZE: f32 = 48.0;
/// The smallest region side, in points, that is recognised.
const MIN_REGION_SIDE: f32 = 4.0;
/// The most entries the language menu shows at once; the rest scroll.
const MENU_ENTRIES: usize = 10;

/// The fullscreen window on the monitor at `monitor` in the order of `winit`.
///
/// The window on monitor 0 is the root one; the others are opened by it.
pub fn viewport(monitor: usize) -> ViewportBuilder {
    let builder = ViewportBuilder::default()
        .with_app_id(APP_ID)
        .with_title(APP_ID)
        .with_decorations(false)
        .with_fullscreen(true);
    // The screenshot on Windows is of one monitor, shown where the system puts
    // the window.
    #[cfg(target_os = "linux")]
    let builder = builder.with_monitor(monitor);
    #[cfg(not(target_os = "linux"))]
    let _ = monitor;
    builder
}

/// The part of the screenshot one window shows.
struct Screen {
    /// The connector of the monitor, such as `DP-1`, when it is known.
    monitor: Option<String>,
    image: RgbaImage,
    texture: TextureHandle,
}

/// The application state.
pub struct App {
    /// The screenshot of every monitor, until the first frame cuts it into
    /// `screens`: before it the toolkit does not know the monitors, nor the
    /// largest texture the device takes, and assumes 2048 pixels.
    desktop: Option<RgbaImage>,
    /// One per monitor, the first in the root window; one in all when the
    /// monitors do not match the screenshot.
    screens: Vec<Screen>,
    settings: Settings,
    settings_path: PathBuf,
    worker: Worker,
    fonts: Fonts,
    /// The paragraphs drawn on the active screen, kept tessellated.
    plates: text::Plates,
    /// The screen the language window is on: the last one the pointer was over.
    languages_on: usize,
    /// Where the language window was last seen at rest on that screen.
    language_window: Option<egui::Pos2>,
    /// The pointer was held since then: only a drag moves the window, egui
    /// itself shifts it while it learns its size.
    pointer_held: bool,
    /// The menu a language or the split is picked from, and which it is for.
    menu: PlateMenu,
    picking: Option<Pick>,
    job: u64,
    /// The direction and the split the job was sent with.
    job_cut: Option<(Direction, Split)>,
    busy: bool,
    /// The screen the region, the text and an error are on.
    active: usize,
    drag_start: Option<egui::Pos2>,
    region: Option<Rect>,
    /// Where the screenshot was drawn when the region was recognised, to
    /// recognise it again.
    region_image: Option<Rect>,
    paragraphs: Vec<Paragraph>,
    /// The paragraphs cut as the settings say: the plates and their pieces.
    units: Vec<Unit>,
    /// The translation of each piece of `units`, in order, as it arrives.
    translations: Vec<Option<String>>,
    stats: Stats,
    error: Option<AppError>,
}

/// What the menu is open for.
#[derive(Debug, Clone, Copy)]
enum Pick {
    Language(Side),
    Split,
    Ocr,
}

/// What the language window tells of the screenshot and the region, each
/// known once measured.
#[derive(Debug, Default)]
struct Stats {
    /// The width and height of the screenshot of every monitor, in pixels.
    total: Option<[u32; 2]>,
    /// The width and height of the region, in pixels.
    selected: Option<[u32; 2]>,
    recognition: Option<Duration>,
    translation: Option<Duration>,
}

impl App {
    /// Shows `screenshot` and sends jobs to `worker`.
    pub fn new(
        screenshot: RgbaImage,
        mut settings: Settings,
        settings_path: PathBuf,
        worker: Worker,
        fonts: Fonts,
    ) -> Self {
        settings.fit_source();
        settings.remember_direction();
        Self {
            desktop: Some(screenshot),
            screens: Vec::new(),
            settings,
            settings_path,
            worker,
            fonts,
            plates: text::Plates::default(),
            languages_on: 0,
            language_window: None,
            pointer_held: false,
            menu: PlateMenu::new().max_plates(MENU_ENTRIES),
            picking: None,
            job: 0,
            job_cut: None,
            busy: false,
            active: 0,
            drag_start: None,
            region: None,
            region_image: None,
            paragraphs: Vec::new(),
            units: Vec::new(),
            translations: Vec::new(),
            stats: Stats::default(),
            error: None,
        }
    }

    /// Cuts the screenshot into one image per monitor and loads them.
    fn split(&mut self, context: &Context, frame: &eframe::Frame) {
        let Some(desktop) = self.desktop.take() else {
            return;
        };
        self.stats.total = Some([desktop.width(), desktop.height()]);
        let started = Instant::now();
        #[cfg(target_os = "linux")]
        let images = frame
            .winit_window()
            .and_then(|window| per_monitor(&desktop, &distinct(window.available_monitors())))
            .unwrap_or_else(|| vec![(None, desktop)]);
        #[cfg(not(target_os = "linux"))]
        let images = {
            let _ = frame;
            vec![(None, desktop)]
        };
        self.screens = images
            .into_iter()
            .enumerate()
            .map(|(index, (monitor, image))| Screen {
                texture: load_texture(context, &format!("screenshot {index}"), &image),
                monitor,
                image,
            })
            .collect();
        log::info!(
            "{} window(s), one per monitor, textures loaded in {:?}",
            self.screens.len(),
            started.elapsed()
        );
    }

    fn save_settings(&mut self) {
        if let Err(error) = self.settings.save(&self.settings_path) {
            log::error!("{error}");
            self.error = Some(error);
        }
    }

    /// Keeps where the user left the language window, once the drag is over.
    fn place_languages(&mut self, context: &Context, monitor: String, position: egui::Pos2) {
        if context.input(|input| input.pointer.any_down()) {
            self.pointer_held = true;
            return;
        }
        let moved = self.pointer_held
            && self
                .language_window
                .is_some_and(|previous| previous != position);
        self.pointer_held = false;
        self.language_window = Some(position);
        if moved {
            self.settings
                .language_window
                .insert(monitor, [position.x, position.y]);
            self.save_settings();
        }
    }

    /// Follows the pointer to screen `index`: the language window is shown on
    /// the monitor the pointer is over. It stays put while the menu is open.
    fn follow_pointer(&mut self, context: &Context, index: usize) {
        if index == self.languages_on
            || self.menu.is_open()
            || !context.input(|input| input.pointer.has_pointer())
        {
            return;
        }
        self.languages_on = index;
        self.language_window = None;
        self.pointer_held = false;
    }

    /// Shows the language window and the menu on screen `index`.
    fn show_languages(&mut self, context: &Context, index: usize, screen: Rect) {
        let monitor = self.screens[index]
            .monitor
            .clone()
            .unwrap_or_else(|| languages::monitor_key(context, screen));
        let shown = languages::window(
            context,
            &self.settings,
            &monitor,
            screen,
            self.copied_text().is_some(),
            &self.stats_rows(),
        );
        if shown.closed {
            context.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Close);
        }
        if let Some(side) = shown.pressed {
            let names = nllb::LANGUAGES
                .iter()
                .filter_map(|code| nllb::autonym(code));
            self.fonts.cover(context, names);
            languages::open(&mut self.menu, &self.settings, side);
            self.picking = Some(Pick::Language(side));
        }
        if shown.split {
            languages::open_split(&mut self.menu, &self.settings);
            self.picking = Some(Pick::Split);
        }
        if shown.ocr {
            languages::open_ocr(&mut self.menu, &self.settings);
            self.picking = Some(Pick::Ocr);
        }
        if let Some(position) = shown.position {
            self.place_languages(context, monitor, position);
        }
        if shown.toggled {
            self.toggle_translate();
        }
        if shown.copy
            && let Some(text) = self.copied_text()
        {
            #[cfg(target_os = "linux")]
            crate::clipboard::copy(text);
            // The system keeps the text after the program quits.
            #[cfg(not(target_os = "linux"))]
            context.copy_text(text);
            context.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Close);
        }
        if let Some(direction) = shown.direction {
            self.set_direction(direction);
        }
        if let Some(chosen) = self.menu.show(context) {
            self.pick(chosen.id);
        }
    }

    /// Sets what the menu was opened for to `id`: a language code, a split
    /// or a recogniser.
    fn pick(&mut self, id: String) {
        match self.picking.take() {
            Some(Pick::Language(side)) => {
                let mut direction = self.settings.direction();
                match side {
                    Side::Source => direction.source = id,
                    Side::Target => direction.target = id,
                }
                if direction != self.settings.direction() {
                    self.set_direction(direction);
                }
            }
            Some(Pick::Split) => {
                if let Some(split) = Split::from_id(&id)
                    && split != self.settings.split
                {
                    self.settings.split = split;
                    self.cut();
                    self.retranslate();
                }
            }
            Some(Pick::Ocr) => {
                if let Some(ocr) = Ocr::from_id(&id)
                    && ocr != self.settings.ocr
                {
                    self.settings.ocr = ocr;
                    if self.settings.fit_source() {
                        self.settings.remember_direction();
                    }
                    self.save_settings();
                    self.rerecognize();
                }
            }
            None => {}
        }
    }

    /// Takes `direction`, remembers it and translates with it; the region is
    /// recognised again when PaddleOCR reads the new language of the text
    /// with another model.
    fn set_direction(&mut self, direction: Direction) {
        let reread = self.settings.ocr == Ocr::PaddleOcr
            && paddle_ocr_model(&self.settings.source) != paddle_ocr_model(&direction.source);
        self.settings.source = direction.source;
        self.settings.target = direction.target;
        self.settings.remember_direction();
        if reread {
            self.save_settings();
            self.rerecognize();
        } else {
            self.retranslate();
        }
    }

    /// Recognises the region again, as the recogniser or its model changed.
    fn rerecognize(&mut self) {
        if let (Some(region), Some(image_rect)) = (self.region, self.region_image) {
            self.recognize(image_rect, region);
        }
    }

    /// Turns the translation on, which translates the text shown, or off,
    /// which shows the recognised text and stops a translation under way.
    fn toggle_translate(&mut self) {
        self.settings.translate = !self.settings.translate;
        self.worker.set_translate(self.settings.translate);
        if self.settings.translate {
            self.retranslate();
            return;
        }
        self.save_settings();
        self.translations.clear();
        // Paragraphs while busy: the job is past recognition, translating.
        if self.busy && !self.paragraphs.is_empty() {
            self.cancel_job("by turning the translation off");
        }
    }

    /// The text of each plate: the translation of the pieces that have one,
    /// the recognised text of the others; the recognised text of the plates
    /// none of whose pieces has one, and of all when not translating.
    fn plate_texts(&self) -> Vec<String> {
        let mut first = 0;
        self.units
            .iter()
            .map(|unit| {
                let range = first..first + unit.pieces.len();
                first = range.end;
                let translated = self.translations.get(range).unwrap_or_default();
                if !self.settings.translate || translated.iter().all(Option::is_none) {
                    return unit.paragraph.text.clone();
                }
                let pieces = translated
                    .iter()
                    .zip(&unit.pieces)
                    .map(|(translation, piece)| translation.as_deref().unwrap_or(piece));
                split::join(pieces, &self.settings.target)
            })
            .collect()
    }

    /// The text the copy button copies, one plate a line: the translated one
    /// once every piece is translated when translating, else the recognised
    /// one.
    fn copied_text(&self) -> Option<String> {
        let pieces: usize = self.units.iter().map(|unit| unit.pieces.len()).sum();
        let translated =
            self.translations.len() == pieces && self.translations.iter().all(Option::is_some);
        if self.units.is_empty() || (self.settings.translate && !translated) {
            return None;
        }
        Some(self.plate_texts().join("\n"))
    }

    /// Cuts the paragraphs into plates and pieces as the settings say.
    fn cut(&mut self) {
        self.units = split::units(&self.paragraphs, self.settings.split);
        self.translations.clear();
    }

    /// The lines of the statistics: those measured, and not zero.
    fn stats_rows(&self) -> Vec<(String, String)> {
        let mut rows = Vec::new();
        let mut row = |key: &str, value: String| rows.push((t!(key).into_owned(), value));
        let pixels = |[width, height]: [u32; 2]| {
            t!("ui.stats.pixels", width = width, height = height).into_owned()
        };
        if let Some(size) = self.stats.total {
            row("ui.stats.total", pixels(size));
        }
        if let Some(size) = self.stats.selected {
            row("ui.stats.selected", pixels(size));
        }
        if let Some(elapsed) = self.stats.recognition {
            row("ui.stats.recognition", duration(elapsed));
        }
        let letters = self
            .paragraphs
            .iter()
            .flat_map(|p| p.text.chars())
            .filter(|c| !c.is_whitespace())
            .count();
        if letters > 0 {
            row("ui.stats.letters", letters.to_string());
        }
        let count = match self.settings.split {
            Split::Whole => None,
            Split::Paragraphs => Some(("ui.stats.paragraphs", self.paragraphs.len())),
            Split::Sentences => Some((
                "ui.stats.sentences",
                self.units.iter().map(|unit| unit.pieces.len()).sum(),
            )),
        };
        if let Some((key, count)) = count.filter(|(_, count)| *count > 0) {
            row(key, count.to_string());
        }
        if let Some(elapsed) = self.stats.translation {
            row("ui.stats.translation", duration(elapsed));
        }
        rows
    }

    fn languages(&self) -> Languages {
        Languages {
            source: self.settings.source.clone(),
            target: self.settings.target.clone(),
        }
    }

    fn start_job(&mut self) -> u64 {
        self.job += 1;
        self.busy = true;
        self.error = None;
        self.translations.clear();
        self.stats.translation = None;
        self.job_cut = Some((self.settings.direction(), self.settings.split));
        self.job
    }

    /// Drops the job under way: its answers no longer match `job`. It is
    /// logged as cancelled `reason`.
    fn cancel_job(&mut self, reason: &str) {
        log::info!("job {} cancelled {reason}", self.job);
        self.job += 1;
        self.busy = false;
        self.worker.cancel();
    }

    /// Takes the answers of the worker; the letters of a text shown get fonts
    /// if egui's lack them.
    fn receive(&mut self, context: &Context) {
        while let Some(response) = self.worker.poll() {
            match response {
                Response::Recognized {
                    id,
                    paragraphs,
                    elapsed,
                    translating,
                } if id == self.job => {
                    self.fonts
                        .cover(context, paragraphs.iter().map(|p| p.text.as_str()));
                    self.paragraphs = paragraphs;
                    self.stats.recognition = Some(elapsed);
                    self.cut();
                    let cut = Some((self.settings.direction(), self.settings.split));
                    if !translating {
                        self.busy = false;
                        // Turned on after the worker had passed it by.
                        if self.settings.translate {
                            self.retranslate();
                        }
                    } else if self.job_cut != cut {
                        // The languages or the split changed during recognition.
                        self.retranslate();
                    }
                }
                Response::Translated { id, index, text } if id == self.job => {
                    // Turned off after the worker had started it.
                    if self.settings.translate {
                        self.fonts.cover(context, [text.as_str()]);
                        if self.translations.len() <= index {
                            self.translations.resize(index + 1, None);
                        }
                        self.translations[index] = Some(text);
                    }
                }
                Response::Done { id, elapsed } if id == self.job => {
                    self.stats.translation = Some(elapsed);
                    self.busy = false;
                }
                Response::Failed { id, error } if id == self.job => {
                    log::error!("{error}: {:?}", std::error::Error::source(&error));
                    self.error = Some(error);
                    self.busy = false;
                }
                _ => {}
            }
        }
    }

    fn recognize(&mut self, image_rect: Rect, region: Rect) {
        self.region_image = Some(image_rect);
        let screenshot = &self.screens[self.active].image;
        let scale = screenshot.width() as f32 / image_rect.width();
        let bounds = Rect::from_min_size(
            pos2(0.0, 0.0),
            Vec2::new(screenshot.width() as f32, screenshot.height() as f32),
        );
        let pixels = Rect::from_min_max(
            pos2(0.0, 0.0) + (region.min - image_rect.min) * scale,
            pos2(0.0, 0.0) + (region.max - image_rect.min) * scale,
        )
        .intersect(bounds);
        if pixels.width() < 1.0 || pixels.height() < 1.0 {
            return;
        }
        log::info!(
            "region on {}: {}x{} pixels at {}, {}",
            self.screens[self.active]
                .monitor
                .as_deref()
                .unwrap_or("the screen"),
            pixels.width() as u32,
            pixels.height() as u32,
            pixels.min.x as u32,
            pixels.min.y as u32,
        );
        let image = image::imageops::crop_imm(
            screenshot,
            pixels.min.x as u32,
            pixels.min.y as u32,
            pixels.width() as u32,
            pixels.height() as u32,
        )
        .to_image();
        self.paragraphs.clear();
        self.cut();
        self.stats = Stats {
            total: self.stats.total,
            selected: Some([pixels.width() as u32, pixels.height() as u32]),
            ..Stats::default()
        };
        let id = self.start_job();
        self.worker.send(Request::Recognize {
            id,
            image,
            origin: pixels.min,
            ocr: self.settings.ocr,
            split: self.settings.split,
            languages: self.languages(),
        });
    }

    fn retranslate(&mut self) {
        self.save_settings();
        if !self.settings.translate || self.paragraphs.is_empty() {
            return;
        }
        let pieces = self
            .units
            .iter()
            .flat_map(|unit| unit.pieces.iter().cloned())
            .collect();
        let id = self.start_job();
        self.worker.send(Request::Translate {
            id,
            pieces,
            languages: self.languages(),
        });
    }

    /// Draws screen `index`, takes the region dragged on it and, when the
    /// pointer is over it, shows the language window.
    fn show_screen(&mut self, index: usize, ui: &mut egui::Ui) {
        log_frame(ui.ctx(), index);
        let context = ui.ctx().clone();
        // Esc closes the menu first, the program only when no menu is open.
        let menu_open = self.menu.is_open();
        let quit = context.input(|input| {
            (input.key_pressed(egui::Key::Escape) && !menu_open)
                || input.viewport().close_requested()
        });
        if quit {
            context.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Close);
        }
        let screen = ui.max_rect();
        let image = &self.screens[index].image;
        let image_size = Vec2::new(image.width() as f32, image.height() as f32);
        let fit = (screen.width() / image_size.x).min(screen.height() / image_size.y);
        let image_rect = Rect::from_center_size(screen.center(), image_size * fit);
        self.paint(index, ui.painter(), screen, image_rect, fit);
        self.select(index, ui, screen, image_rect);
        if self.busy && index == self.active {
            let spinner = Rect::from_center_size(screen.center(), Vec2::splat(SPINNER_SIZE));
            ui.put(spinner, egui::Spinner::new().size(SPINNER_SIZE));
        }
        self.follow_pointer(&context, index);
        if index == self.languages_on {
            self.show_languages(&context, index, screen);
        }
    }

    fn paint(
        &mut self,
        index: usize,
        painter: &egui::Painter,
        screen: Rect,
        image_rect: Rect,
        fit: f32,
    ) {
        painter.rect_filled(screen, 0.0, Color32::BLACK);
        let whole = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        let texture = self.screens[index].texture.id();
        painter.image(texture, image_rect, whole, Color32::WHITE);
        let active = index == self.active;
        if active {
            region::paint(painter, screen, self.region);
            let texts = self.plate_texts();
            let paragraphs: Vec<_> = self
                .units
                .iter()
                .zip(&texts)
                .map(|(unit, text)| text::Paragraph {
                    rect: Rect::from_min_max(
                        image_rect.min + unit.paragraph.rect.min.to_vec2() * fit,
                        image_rect.min + unit.paragraph.rect.max.to_vec2() * fit,
                    ),
                    line_height: unit.paragraph.line_height * fit,
                    text,
                })
                .collect();
            self.plates.paint(painter, &paragraphs);
        } else {
            region::paint(painter, screen, None);
        }
        match &self.error {
            Some(error) if active => text::error(painter, screen, &t!(error.message_key())),
            _ if self.region.is_none() => text::hint(painter, screen, &t!("ui.hint")),
            _ => {}
        }
    }

    /// Drops the region, its text and the job under way, logged as cancelled
    /// `reason`.
    fn drop_region(&mut self, reason: &str) {
        if self.busy {
            self.cancel_job(reason);
        }
        self.drag_start = None;
        self.region = None;
        self.region_image = None;
        self.paragraphs.clear();
        self.cut();
        self.stats = Stats {
            total: self.stats.total,
            ..Stats::default()
        };
        self.error = None;
    }

    fn select(&mut self, index: usize, ui: &egui::Ui, screen: Rect, image_rect: Rect) {
        let response = ui.interact(screen, ui.id().with(("region", index)), Sense::drag());
        if self.menu.is_open() {
            return;
        }
        // Over the screenshot, not the language window, a right click drops
        // the region, or quits when there is none.
        let right_click = ui.input(|input| input.pointer.secondary_clicked());
        if right_click && response.hovered() {
            if self.region.is_some() {
                self.drop_region("by a right click");
            } else {
                ui.ctx()
                    .send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Close);
            }
        }
        if response.drag_started_by(PointerButton::Primary) {
            self.drop_region("by a new region");
            self.active = index;
            self.drag_start = response.interact_pointer_pos();
        }
        if index != self.active {
            return;
        }
        if let (Some(start), Some(pointer)) = (self.drag_start, response.interact_pointer_pos()) {
            self.region = Some(Rect::from_two_pos(start, pointer).intersect(image_rect));
        }
        if response.drag_stopped_by(PointerButton::Primary) {
            self.drag_start = None;
            match self.region {
                Some(region) if region.width().min(region.height()) >= MIN_REGION_SIDE => {
                    self.recognize(image_rect, region);
                }
                _ => self.region = None,
            }
        }
    }
}

/// The PaddleOCR model that reads `language`, an NLLB code.
fn paddle_ocr_model(language: &str) -> Option<&'static str> {
    paddle_ocr::recognizer_for(language.rsplit('_').next().unwrap_or(language))
}

/// `elapsed` in milliseconds below a second, else in seconds.
fn duration(elapsed: Duration) -> String {
    if elapsed < Duration::from_secs(1) {
        t!("ui.stats.ms", n = elapsed.as_millis()).into_owned()
    } else {
        let seconds = format!("{:.1}", elapsed.as_secs_f32());
        t!("ui.stats.s", n = seconds).into_owned()
    }
}

/// Writes why the window on screen `index` is drawn: a frame drawn with
/// nothing happening shows here.
fn log_frame(context: &Context, index: usize) {
    if log::log_enabled!(log::Level::Debug) {
        log::debug!(
            "screen {index}: frame {}, repaint asked by {:?}",
            context.cumulative_frame_nr(),
            context.repaint_causes()
        );
    }
}

/// Each monitor of `monitors` once, in the order it first appears.
///
/// Under Wayland the list of a window repeats every monitor: `winit` 0.30 fills
/// it with the outputs it knows at start and adds each again as the compositor
/// announces it. `with_monitor` counts the monitors of the event loop, which
/// are listed once, in the order of their first appearance here.
#[cfg(target_os = "linux")]
fn distinct(
    monitors: impl Iterator<Item = winit::monitor::MonitorHandle>,
) -> Vec<winit::monitor::MonitorHandle> {
    let mut seen = Vec::new();
    for monitor in monitors {
        if !seen.contains(&monitor) {
            seen.push(monitor);
        }
    }
    seen
}

/// The screenshot cut into one image per monitor of `monitors`, each with the
/// connector of its monitor, or `None` when the monitors do not match it.
#[cfg(target_os = "linux")]
fn per_monitor(
    desktop: &RgbaImage,
    monitors: &[winit::monitor::MonitorHandle],
) -> Option<Vec<(Option<String>, RgbaImage)>> {
    use crate::monitor::{self, Monitor};

    // Under X11 there is no compositor layout, and the one of `winit` is right.
    let outputs = crate::outputs::list().unwrap_or_default();
    let layout: Vec<Monitor> = monitors
        .iter()
        .map(|handle| {
            let name = handle.name();
            outputs
                .iter()
                .find(|output| name.is_some() && output.name == name)
                .map_or_else(|| Monitor::from_winit(handle), |output| output.monitor)
        })
        .collect();
    let size = desktop.dimensions();
    let crops = layout
        .iter()
        .map(|&current| monitor::locate(&layout, current, size))
        .collect::<Option<Vec<_>>>();
    let Some(crops) = crops.filter(|crops| !crops.is_empty()) else {
        log::warn!(
            "the monitors {layout:?} do not match the {}x{} screenshot, it is shown whole",
            size.0,
            size.1
        );
        return None;
    };
    let images = monitors
        .iter()
        .zip(crops)
        .map(|(handle, crop)| {
            log::info!(
                "monitor {:?}: {}x{} at {},{} of the {}x{} screenshot",
                handle.name(),
                crop.width,
                crop.height,
                crop.x,
                crop.y,
                size.0,
                size.1
            );
            let image = image::imageops::crop_imm(desktop, crop.x, crop.y, crop.width, crop.height)
                .to_image();
            (handle.name(), image)
        })
        .collect();
    Some(images)
}

/// Loads `screenshot` as a texture, scaled down to the largest side the device takes.
///
/// The texture is only what is shown: recognition crops the screenshot itself,
/// so a scaled texture costs sharpness on screen and nothing in the text.
fn load_texture(context: &Context, name: &str, screenshot: &RgbaImage) -> TextureHandle {
    let max_side = context.input(|input| input.max_texture_side) as u32;
    let (width, height) = screenshot.dimensions();
    let side = width.max(height);
    let image = if side > max_side {
        log::warn!("{name} {width}x{height} exceeds the texture side {max_side}, scaled down");
        let scaled_width = (u64::from(width) * u64::from(max_side) / u64::from(side)).max(1) as u32;
        let scaled_height =
            (u64::from(height) * u64::from(max_side) / u64::from(side)).max(1) as u32;
        let scaled = image::imageops::resize(
            screenshot,
            scaled_width,
            scaled_height,
            FilterType::Triangle,
        );
        let size = [scaled.width() as usize, scaled.height() as usize];
        ColorImage::from_rgba_unmultiplied(size, scaled.as_raw())
    } else {
        let size = [width as usize, height as usize];
        ColorImage::from_rgba_unmultiplied(size, screenshot.as_raw())
    };
    context.load_texture(name, image, TextureOptions::LINEAR)
}

impl eframe::App for App {
    fn raw_input_hook(&mut self, _context: &Context, raw_input: &mut egui::RawInput) {
        raw_input.max_texture_side = raw_input.max_texture_side.map(|side| side.min(ATLAS_SIDE));
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let context = ui.ctx().clone();
        self.fonts.install(&context);
        self.receive(&context);
        self.split(&context, frame);
        for index in 1..self.screens.len() {
            context.show_viewport_immediate(
                ViewportId::from_hash_of(("monitor", index)),
                viewport(index),
                |ui, _class| self.show_screen(index, ui),
            );
        }
        self.show_screen(0, ui);
    }
}
