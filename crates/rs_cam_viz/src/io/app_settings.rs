//! The app settings file: `~/.config/rs_cam/settings.toml` (B5).
//!
//! The loader and the writer live in core, at
//! [`rs_cam_core::budget::settings`], so the GUI and the CLI read the file
//! through ONE function, and File ▸ Preferences writes it through ONE
//! function. This module re-exports them for the GUI call sites. Do not add
//! a second parser or writer here.

pub use rs_cam_core::budget::settings::{
    AppSettings, DEFAULT, LoadedSettings, SETTINGS_ENV, UNLIMITED, limit_text, load, load_from,
    parse, parse_limit_text, resolve_limit, save_limit_to, settings_path, settings_path_from,
};
