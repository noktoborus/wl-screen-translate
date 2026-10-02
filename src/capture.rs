//! Capture of the whole screen.

use image::RgbaImage;

use crate::error::{AppError, Result};

/// Takes a screenshot through the desktop portal, without its dialog.
///
/// The portal writes a file for the caller; it is removed once read.
#[cfg(target_os = "linux")]
pub fn screen() -> Result<RgbaImage> {
    use ashpd::desktop::screenshot::Screenshot;

    let portal = |source| AppError::Portal { source };
    let screenshot = pollster::block_on(async {
        Screenshot::request()
            .interactive(false)
            .modal(false)
            .send()
            .await
    })
    .and_then(|request| request.response())
    .map_err(portal)?;
    let uri = screenshot.uri().as_str();
    let path = url::Url::parse(uri)
        .ok()
        .and_then(|url| url.to_file_path().ok())
        .ok_or_else(|| AppError::ScreenshotUri { uri: uri.into() })?;
    let image = image::open(&path).map_err(|source| AppError::ScreenshotRead {
        path: path.clone(),
        source,
    })?;
    if let Err(error) = std::fs::remove_file(&path) {
        log::warn!("{}: {error}", path.display());
    }
    Ok(image.into_rgba8())
}

/// Takes a screenshot of the primary monitor.
#[cfg(target_os = "windows")]
pub fn screen() -> Result<RgbaImage> {
    let capture = |source| AppError::Capture { source };
    let monitors = xcap::Monitor::all().map_err(capture)?;
    let monitor = monitors
        .iter()
        .find(|monitor| monitor.is_primary().unwrap_or(false))
        .or(monitors.first())
        .ok_or(AppError::NoMonitor)?;
    monitor.capture_image().map_err(capture)
}
