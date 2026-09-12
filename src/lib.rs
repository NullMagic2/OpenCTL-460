//! Hardware-independent report decoding, pressure shaping, and pen lifecycle.
//! All regression tests live in debug/; Windows handles stay in the executable.

#[cfg(windows)]
pub mod broker;
pub mod button_actions;
pub mod calibration;
pub mod calibration_pad;
pub mod config;
pub mod device_selection;
pub mod double_click;
pub mod engine;
pub mod handwriting;
pub mod hid;
#[cfg(windows)]
#[path = "../shared/ipc.rs"]
pub mod ipc;
pub mod lifecycle;
pub mod line_smoothing;
pub mod live_config;
pub mod pen_control;
pub mod pressure_editor;
pub mod pressure_profiles;
pub mod protocol;
#[cfg(windows)]
pub mod status;
pub mod stroke;
pub mod tablet_mode;
pub mod trace;
#[path = "../shared/wire.rs"]
pub mod wire;
pub mod wobble;
pub mod writing_filter;

/// Product version shared by the settings UI and read-only CLI version query.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod smoothing_guard;

pub mod precision;
pub mod shape_assist;
#[cfg(windows)]
pub mod start_marker;
pub mod straight_assist;

pub mod spectral_smoothing;

pub mod startup_wait;

pub mod endpoint_settling;
