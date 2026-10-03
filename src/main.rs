//! Translates the text of a screen region: capture, select, recognise, translate.
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod cache;
mod capture;
#[cfg(target_os = "linux")]
mod clipboard;
mod error;
#[cfg(target_os = "linux")]
mod fontconfig;
mod fonts;
mod gpu;
#[cfg(target_os = "linux")]
mod monitor;
mod ocr;
#[cfg(target_os = "linux")]
mod outputs;
mod paragraph;
mod paths;
mod settings;
mod split;
mod tessdata;
mod ui;
mod worker;

use std::process::ExitCode;

use rust_i18n::t;

use crate::error::{AppError, Result};
use crate::fonts::Fonts;
use crate::paths::Paths;
use crate::settings::Settings;
use crate::worker::Worker;

rust_i18n::i18n!("locales", fallback = "en");

/// The application identifier: window class and directory name.
pub const APP_ID: &str = "wl-screen-translate";
/// The level logged when `RUST_LOG` names none.
const LOG_LEVEL: &str = "info";

fn main() -> ExitCode {
    let level = std::env::var("RUST_LOG").unwrap_or_else(|_| LOG_LEVEL.to_owned());
    if let Err(error) = simple_log::console(level.as_str()) {
        eprintln!("cannot start the log at level {level:?}: {error}");
    }
    #[cfg(target_os = "linux")]
    if std::env::args().nth(1).as_deref() == Some(clipboard::SERVE_ARG) {
        return match clipboard::serve() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                log::error!("clipboard: {error}");
                ExitCode::FAILURE
            }
        };
    }
    let russian = sys_locale::get_locale().is_some_and(|locale| locale.starts_with("ru"));
    rust_i18n::set_locale(if russian { "ru" } else { "en" });
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            log::error!("{error}: {:?}", std::error::Error::source(&error));
            eprintln!("{}", t!(error.message_key()));
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let paths = Paths::resolve()?;
    tessdata::scan(&paths.tesseract);
    let settings = Settings::load(&paths.settings);
    let started = std::time::Instant::now();
    let screenshot = capture::screen()?;
    log::info!(
        "screenshot {}x{} taken in {:?}",
        screenshot.width(),
        screenshot.height(),
        started.elapsed()
    );
    let options = gpu::native_options(ui::viewport(0));
    eframe::run_native(
        APP_ID,
        options,
        Box::new(move |creation| {
            if let Some(state) = &creation.wgpu_render_state {
                gpu::log_adapter(state);
            }
            let context = creation.egui_ctx.clone();
            let settings_path = paths.settings.clone();
            let fonts = Fonts::spawn(context.clone());
            let worker = Worker::spawn(paths, context, settings.translate);
            Ok(Box::new(ui::App::new(
                screenshot,
                settings,
                settings_path,
                worker,
                fonts,
            )))
        }),
    )
    .map_err(|source| AppError::Window { source })
}
