//! The graphics device the window is drawn with: `wgpu`, its adapter and limits.

use std::sync::Arc;

use eframe::egui::ViewportBuilder;
use eframe::egui_wgpu::{RenderState, WgpuConfiguration, WgpuSetup, WgpuSetupCreateNew};
use eframe::wgpu;

/// Widest texture the toolkit is told it may create.
///
/// The device is asked for the largest texture it grants, but the glyph atlas of
/// the toolkit is cut into rows as wide as the side the window reports, and its
/// height doubles as glyphs arrive: a row of sixteen thousand pixels makes every
/// step of that doubling cost four times what a row of four thousand would.
/// Eight thousand still holds a screenshot of a 4K screen whole.
pub const ATLAS_SIDE: usize = 8192;

/// The label the device carries in the messages of `wgpu`.
const WINDOW_DEVICE: &str = "window device";

/// The options the window is started with.
pub fn native_options(viewport: ViewportBuilder) -> eframe::NativeOptions {
    let mut setup = WgpuSetupCreateNew::without_display_handle();
    setup.power_preference = wgpu::PowerPreference::LowPower;
    setup.native_adapter_selector = Some(Arc::new(choose_adapter));
    setup.device_descriptor = Arc::new(|adapter| wgpu::DeviceDescriptor {
        label: Some(WINDOW_DEVICE),
        required_limits: limits(&adapter.limits()),
        ..Default::default()
    });

    eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: WgpuConfiguration {
            wgpu_setup: WgpuSetup::CreateNew(setup),
            ..WgpuConfiguration::default()
        },
        ..eframe::NativeOptions::default()
    }
}

/// Writes the adapter the window ended up on into the log.
pub fn log_adapter(state: &RenderState) {
    let info = state.adapter.get_info();
    log::info!(
        "adapter: {} [{:?}] type={:?} driver={} {}",
        info.name,
        info.backend,
        info.device_type,
        info.driver,
        info.driver_info
    );
    log::info!(
        "max texture side: {}",
        state.device.limits().max_texture_dimension_2d
    );
    if is_software(&info) {
        log::warn!("the window is drawn by the processor, no graphics chip was usable");
    }
}

/// What is asked of the device: the defaults cut down to the adapter, and every
/// texture size raised to what the adapter grants.
///
/// The defaults of `eframe` ask a GL adapter for no more than the WebGL 2 limits,
/// whose 2048 pixels do not hold a screenshot, and the defaults of `wgpu` exceed
/// what a small chip offers, so a device request asking for them fails outright —
/// hence the cut. The texture sizes go the other way: what the adapter offers is
/// what is asked of it.
fn limits(offered: &wgpu::Limits) -> wgpu::Limits {
    let mut asked = wgpu::Limits::default().or_worse_values_from(offered);
    asked.max_texture_dimension_1d = offered.max_texture_dimension_1d;
    asked.max_texture_dimension_2d = offered.max_texture_dimension_2d;
    asked.max_texture_dimension_3d = offered.max_texture_dimension_3d;
    asked
}

/// Picks the adapter the window is drawn with, hardware before software.
fn choose_adapter(
    adapters: &[wgpu::Adapter],
    surface: Option<&wgpu::Surface<'_>>,
) -> std::result::Result<wgpu::Adapter, String> {
    let serves_window = |adapter: &&wgpu::Adapter| {
        surface.is_none_or(|surface| adapter.is_surface_supported(surface))
    };

    for adapter in adapters {
        let info = adapter.get_info();
        log::debug!(
            "adapter offered: {} [{:?}] type={:?} software={}",
            info.name,
            info.backend,
            info.device_type,
            is_software(&info)
        );
    }

    adapters
        .iter()
        .filter(serves_window)
        .find(|adapter| !is_software(&adapter.get_info()))
        .or_else(|| adapters.iter().find(serves_window))
        .cloned()
        .ok_or_else(|| "no adapter serves the window".to_owned())
}

/// True for an adapter that draws on the processor instead of a graphics chip.
fn is_software(info: &wgpu::AdapterInfo) -> bool {
    let name = info.name.to_ascii_lowercase();
    info.device_type == wgpu::DeviceType::Cpu
        || ["llvmpipe", "softpipe", "swrast", "lavapipe"]
            .iter()
            .any(|marker| name.contains(marker))
}
