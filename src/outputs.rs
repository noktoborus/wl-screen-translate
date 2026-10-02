//! The monitors as the Wayland compositor lays them out.
//!
//! `winit` reports the size of a monitor as its video mode: a monitor turned on
//! its side keeps the width and height of the panel, and a fractional scale is
//! rounded. The compositor's own layout, from xdg-output, is read here over a
//! connection of its own and matched to the monitors of `winit` by name.

use smithay_client_toolkit::output::{OutputHandler, OutputInfo, OutputState};
use smithay_client_toolkit::reexports::client::globals::registry_queue_init;
use smithay_client_toolkit::reexports::client::protocol::wl_output::{Transform, WlOutput};
use smithay_client_toolkit::reexports::client::{Connection, QueueHandle};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::{delegate_output, delegate_registry, registry_handlers};

use crate::monitor::Monitor;

/// A monitor of the compositor.
#[derive(Debug, Clone)]
pub struct Output {
    /// The connector, such as `DP-1`: what `winit` names the monitor too.
    pub name: Option<String>,
    /// Where it lies in the desktop.
    pub monitor: Monitor,
}

/// The monitors of the compositor, or `None` outside Wayland or on an error.
pub fn list() -> Option<Vec<Output>> {
    let connection = Connection::connect_to_env().ok()?;
    let (globals, mut queue) = registry_queue_init::<State>(&connection)
        .inspect_err(|error| log::warn!("wayland registry: {error}"))
        .ok()?;
    let handle = queue.handle();
    let mut state = State {
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &handle),
    };
    // The first round brings the outputs, the second their xdg-output layout.
    for _ in 0..2 {
        queue
            .roundtrip(&mut state)
            .inspect_err(|error| log::warn!("wayland roundtrip: {error}"))
            .ok()?;
    }
    let outputs = state
        .outputs
        .outputs()
        .filter_map(|output| state.outputs.info(&output))
        .map(|info| Output {
            monitor: layout(&info),
            name: info.name,
        })
        .collect();
    Some(outputs)
}

/// The logical rectangle of an output: xdg-output's, or one worked out from
/// the mode, the turn and the scale when the compositor lacks xdg-output.
fn layout(info: &OutputInfo) -> Monitor {
    let (x, y) = info.logical_position.unwrap_or(info.location);
    let (width, height) = info.logical_size.unwrap_or_else(|| {
        let (width, height) = info
            .modes
            .iter()
            .find(|mode| mode.current)
            .map_or((0, 0), |mode| mode.dimensions);
        let scale = info.scale_factor.max(1);
        let turned = matches!(
            info.transform,
            Transform::_90 | Transform::_270 | Transform::Flipped90 | Transform::Flipped270
        );
        let (width, height) = if turned {
            (height, width)
        } else {
            (width, height)
        };
        (width / scale, height / scale)
    });
    Monitor {
        x: f64::from(x),
        y: f64::from(y),
        width: f64::from(width),
        height: f64::from(height),
    }
}

struct State {
    registry: RegistryState,
    outputs: OutputState,
}

impl OutputHandler for State {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }

    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlOutput) {}

    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlOutput) {}

    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlOutput) {}
}

impl ProvidesRegistryState for State {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }

    registry_handlers!(OutputState);
}

delegate_registry!(State);
delegate_output!(State);
