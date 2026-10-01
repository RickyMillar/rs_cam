//! The app settings file: `~/.config/rs_cam/settings.toml` (B5).
//!
//! The loader lives in core, at [`rs_cam_core::budget::settings`], so the GUI
//! and the CLI read the file through ONE function. This module re-exports it
//! for the GUI call sites. Do not add a second parser here.

pub use rs_cam_core::budget::settings::{
    AppSettings, LoadedSettings, SETTINGS_ENV, UNLIMITED, load, load_from, parse, parse_limit_text,
    resolve_limit, settings_path, settings_path_from,
};
