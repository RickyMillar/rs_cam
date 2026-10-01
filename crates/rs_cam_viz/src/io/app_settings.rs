//! The app settings file: `~/.config/rs_cam/settings.toml`.
//!
//! The loader and the writer live in core, at [`rs_cam_core::settings`], so
//! the GUI and the CLI read the file through ONE function, and
//! File ▸ Preferences writes it through ONE function. The `[memory] limit`
//! text lives in [`rs_cam_core::budget::settings`]. This module re-exports
//! them for the GUI call sites. Do not add a second parser or writer here.

pub use rs_cam_core::budget::settings::{
    DEFAULT, UNLIMITED, limit_text, parse_limit_text, resolve_limit,
};
pub use rs_cam_core::settings::{
    AppSettings, DiagnosticsSettings, DisplaySettings, GeneralSettings, LoadedSettings, LogLevel,
    PathSettings, PresentModeSetting, SETTINGS_ENV, SettingToken, SimulationSettings,
    StockViewDefault, ToolpathColourDefault, install_paths, load, load_from, parse, save_to,
    settings_path, settings_path_from,
};
