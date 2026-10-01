//! The app settings file: `settings.toml` in the `rs_cam` config folder.
//!
//! The GUI and the CLI read this file through this ONE loader, and
//! File ▸ Preferences writes it through this ONE writer. The file holds:
//!
//! ```toml
//! [memory]
//! limit = "12GiB"           # or a byte count, "unlimited", "default"
//!
//! [general]
//! window_width = 1400.0     # points; read at start
//! window_height = 900.0     # points; read at start
//! undo_depth = 100          # undo steps the GUI keeps
//! toast_info_seconds = 4.0
//! toast_warning_seconds = 6.0
//! toast_error_seconds = 8.0
//! confirm_unsaved_quit = true
//!
//! [display]
//! show_all_toolpaths = false        # operator ruling WP27: selected only
//! toolpath_colour_mode = "normal"   # "engagement", "advance_per_tooth"
//!
//! [display.overlays]                # an overlay id = on or off in every
//! grid = false                      # viewport workspace
//!
//! [simulation]
//! playback_speed = 500.0    # moves per second
//! stock_view = "solid"      # "deviation", "by_height"
//! stock_opacity = 1.0       # 0.0 to 1.0
//!
//! [paths]
//! tool_library = "/path"    # RS_CAM_TOOL_DIR overrides it
//! machine_library = "/path" # RS_CAM_MACHINE_DIR overrides it
//! screenshots = "/path"     # else the current folder
//!
//! [diagnostics]
//! save_cut_trace = false    # operator ruling 2026-10-02: off
//! cut_trace_retain = 5
//! artifact_dir = "/path"    # else <user cache>/rs_cam/artifacts
//! log_level = "info"        # RUST_LOG overrides it; read at start
//! present_mode = "fifo"     # RS_CAM_PRESENT_MODE overrides it; read at start
//! ```
//!
//! A missing key gives the default in [`AppSettings::default`], which is the
//! behaviour of the app before the key existed. The one change is
//! `save_cut_trace`: before 2026-10-02 the GUI always wrote the file.
//!
//! The `[memory] limit` vocabulary lives in `crate::budget::settings`; this
//! module only carries the value.
//!
//! A missing file gives the defaults and is never an error. A file that is
//! not TOML gives the defaults and a warning. A key with a bad value gives
//! the default for that key and a warning; the other keys stay.
//!
//! The writer ([`save_to`]) keeps every key and table that it does not own,
//! and it writes through a temporary file and a rename, so a reader never
//! sees half a file. It removes an owned key whose value is the default, so
//! the file names only what the operator changed. `[memory] limit` is the
//! exception: the writer always writes it, as it did before.

pub mod paths;

use std::collections::BTreeMap;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::budget::settings::{limit_text, parse_limit_text};
use crate::budget::{MemoryBudget, MemoryLimit};

pub use paths::{
    MACHINE_DIR_ENV, SETTINGS_ENV, TOOL_DIR_ENV, install_paths, settings_path, settings_path_from,
};

use paths::SETTINGS_FILE;

/// A closed set of values that the file stores as a token.
pub trait SettingToken: Copy + PartialEq + Sized + 'static {
    /// Every value, in the order a surface lists them.
    const ALL: &'static [Self];
    /// The token in the file.
    fn token(self) -> &'static str;
    /// The text that a surface shows.
    fn label(self) -> &'static str;
    /// The value of a token, in any case.
    fn from_token(text: &str) -> Option<Self> {
        let text = text.trim();
        Self::ALL
            .iter()
            .copied()
            .find(|value| value.token().eq_ignore_ascii_case(text))
    }
}

/// Declare a [`SettingToken`] enum: `Variant => ("token", "Label")`.
macro_rules! setting_token {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $( $(#[$vmeta:meta])* $variant:ident => ($token:literal, $label:literal) ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $( $(#[$vmeta])* $variant ),+
        }

        impl SettingToken for $name {
            const ALL: &'static [Self] = &[$(Self::$variant),+];
            fn token(self) -> &'static str {
                match self { $(Self::$variant => $token),+ }
            }
            fn label(self) -> &'static str {
                match self { $(Self::$variant => $label),+ }
            }
        }
    };
}

setting_token! {
    /// The toolpath colour mode the viewport starts in. The GUI maps it to
    /// its own `ToolpathColorMode`.
    pub enum ToolpathColourDefault {
        /// The palette colour of each toolpath.
        Normal => ("normal", "Normal"),
        /// The feed rate against the nominal feed.
        Engagement => ("engagement", "Engagement"),
        /// The achieved advance per tooth against the vendor band.
        AdvancePerTooth => ("advance_per_tooth", "Advance per tooth"),
    }
}

setting_token! {
    /// The stock view the simulation starts in. The GUI maps it to its own
    /// `StockVizMode`.
    pub enum StockViewDefault {
        /// The wood-tone stock.
        Solid => ("solid", "Solid"),
        /// The deviation from the model surface.
        Deviation => ("deviation", "Deviation"),
        /// The height gradient.
        ByHeight => ("by_height", "By height"),
    }
}

setting_token! {
    /// The log level of the GUI when `RUST_LOG` is not set.
    pub enum LogLevel {
        /// Errors only.
        Error => ("error", "Error"),
        /// Warnings and errors.
        Warn => ("warn", "Warning"),
        /// The default of the GUI.
        Info => ("info", "Info"),
        /// Debug records.
        Debug => ("debug", "Debug"),
        /// Every record.
        Trace => ("trace", "Trace"),
    }
}

setting_token! {
    /// The present mode that a launch requests when `RS_CAM_PRESENT_MODE`
    /// is not set. The tokens are the ones `RS_CAM_PRESENT_MODE` accepts
    /// (`rs_cam_viz::present_mode::decide`).
    pub enum PresentModeSetting {
        /// Vertical sync, the interactive default.
        AutoVsync => ("auto_vsync", "Auto (vsync)"),
        /// No vertical sync, the `--mcp` default.
        AutoNoVsync => ("auto_no_vsync", "Auto (no vsync)"),
        /// First in, first out.
        Fifo => ("fifo", "Fifo"),
        /// First in, first out, relaxed.
        FifoRelaxed => ("fifo_relaxed", "Fifo relaxed"),
        /// Mailbox.
        Mailbox => ("mailbox", "Mailbox"),
        /// Immediate.
        Immediate => ("immediate", "Immediate"),
    }
}

/// `[general]`.
#[derive(Debug, Clone, PartialEq)]
pub struct GeneralSettings {
    /// The initial window width, in points. Read at start.
    pub window_width: f32,
    /// The initial window height, in points. Read at start.
    pub window_height: f32,
    /// The number of undo steps the GUI keeps.
    pub undo_depth: usize,
    /// How long an Info toast stays, in seconds.
    pub toast_info_seconds: f64,
    /// How long a Warning toast stays, in seconds.
    pub toast_warning_seconds: f64,
    /// How long an Error toast stays, in seconds.
    pub toast_error_seconds: f64,
    /// Ask before the GUI quits with unsaved changes.
    pub confirm_unsaved_quit: bool,
}

/// The window size before this setting existed (`rs_cam_viz::run`).
pub const DEFAULT_WINDOW_SIZE: [f32; 2] = [1400.0, 900.0];

/// The undo depth before this setting existed (`UndoHistory::push`).
pub const DEFAULT_UNDO_DEPTH: usize = 100;

/// The toast durations before this setting existed (`Notification::ttl`):
/// Info, Warning, Error, in seconds.
pub const DEFAULT_TOAST_SECONDS: [f64; 3] = [4.0, 6.0, 8.0];

impl Default for GeneralSettings {
    fn default() -> Self {
        let [window_width, window_height] = DEFAULT_WINDOW_SIZE;
        let [info, warning, error] = DEFAULT_TOAST_SECONDS;
        Self {
            window_width,
            window_height,
            undo_depth: DEFAULT_UNDO_DEPTH,
            toast_info_seconds: info,
            toast_warning_seconds: warning,
            toast_error_seconds: error,
            confirm_unsaved_quit: true,
        }
    }
}

/// `[display]`.
#[derive(Debug, Clone, PartialEq)]
pub struct DisplaySettings {
    /// The viewport starts with "All toolpaths" on. Default off: operator
    /// ruling WP27 (the selected toolpath only).
    pub show_all_toolpaths: bool,
    /// The toolpath colour mode the viewport starts in.
    pub toolpath_colour_mode: ToolpathColourDefault,
    /// `[display.overlays]`: an overlay id and the value it takes in every
    /// viewport workspace, in place of that workspace's default. An id that
    /// is not in the map keeps the workspace default. The map keeps an id
    /// that this build does not know, so a write does not delete it.
    pub overlays: BTreeMap<String, bool>,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            show_all_toolpaths: false,
            toolpath_colour_mode: ToolpathColourDefault::Normal,
            overlays: BTreeMap::new(),
        }
    }
}

/// `[simulation]`. Presentation only: nothing here changes a computed
/// number.
#[derive(Debug, Clone, PartialEq)]
pub struct SimulationSettings {
    /// The playback speed, in moves per second.
    pub playback_speed: f32,
    /// The stock view.
    pub stock_view: StockViewDefault,
    /// The stock opacity, 0.0 (clear) to 1.0 (solid).
    pub stock_opacity: f32,
}

/// The playback speed before this setting existed
/// (`SimulationState::new`), in moves per second.
pub const DEFAULT_PLAYBACK_SPEED: f32 = 500.0;

impl Default for SimulationSettings {
    fn default() -> Self {
        Self {
            playback_speed: DEFAULT_PLAYBACK_SPEED,
            stock_view: StockViewDefault::Solid,
            stock_opacity: 1.0,
        }
    }
}

/// `[paths]`. `None` is the default folder.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PathSettings {
    /// The tool-library folder. `RS_CAM_TOOL_DIR` overrides it.
    pub tool_library: Option<PathBuf>,
    /// The machine-library folder. `RS_CAM_MACHINE_DIR` overrides it.
    pub machine_library: Option<PathBuf>,
    /// The screenshot folder. `None` is the current folder of the process.
    pub screenshots: Option<PathBuf>,
}

/// `[diagnostics]`. Advanced: these change no computed number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticsSettings {
    /// Write the simulation cut-trace file. Default off: operator ruling
    /// 2026-10-02. The in-memory trace does not depend on it.
    pub save_cut_trace: bool,
    /// How many cut-trace files stay in the folder.
    pub cut_trace_retain: usize,
    /// The artifact folder. `None` is `<user cache>/rs_cam/artifacts`
    /// (`paths::artifact_dir_from`).
    pub artifact_dir: Option<PathBuf>,
    /// The log level when `RUST_LOG` is not set. Read at start.
    pub log_level: LogLevel,
    /// The present mode when `RS_CAM_PRESENT_MODE` is not set. `None` is the
    /// launch default. Read at start.
    pub present_mode: Option<PresentModeSetting>,
}

/// The cut-trace file count before this setting existed
/// (`SIM_CUT_ARTIFACT_RETAIN`, G-SIMDUMP).
pub const DEFAULT_CUT_TRACE_RETAIN: usize = 5;

impl Default for DiagnosticsSettings {
    fn default() -> Self {
        Self {
            save_cut_trace: false,
            cut_trace_retain: DEFAULT_CUT_TRACE_RETAIN,
            artifact_dir: None,
            log_level: LogLevel::Info,
            present_mode: None,
        }
    }
}

/// The settings the app reads.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AppSettings {
    /// `[memory] limit`.
    pub memory_limit: MemoryLimit,
    /// `[general]`.
    pub general: GeneralSettings,
    /// `[display]`.
    pub display: DisplaySettings,
    /// `[simulation]`.
    pub simulation: SimulationSettings,
    /// `[paths]`.
    pub paths: PathSettings,
    /// `[diagnostics]`.
    pub diagnostics: DiagnosticsSettings,
}

impl AppSettings {
    /// The memory budget these settings give.
    #[must_use]
    pub fn memory_budget(&self) -> MemoryBudget {
        MemoryBudget::from_setting(self.memory_limit)
    }
}

/// The ranges a numeric value must be in. A value outside gives the
/// default and a warning. These are input bounds, not measurements.
pub mod limits {
    use std::ops::RangeInclusive;

    /// A window side, in points.
    pub const WINDOW_SIDE: RangeInclusive<f64> = 320.0..=16384.0;
    /// The undo depth, in steps.
    pub const UNDO_DEPTH: RangeInclusive<u64> = 1..=10_000;
    /// A toast duration, in seconds.
    pub const TOAST_SECONDS: RangeInclusive<f64> = 1.0..=600.0;
    /// The playback speed, in moves per second. The timeline's own floor
    /// is 1 move per second (`ui/sim_timeline.rs`).
    pub const PLAYBACK_SPEED: RangeInclusive<f64> = 1.0..=10_000_000.0;
    /// The stock opacity.
    pub const STOCK_OPACITY: RangeInclusive<f64> = 0.0..=1.0;
    /// The cut-trace file count.
    pub const CUT_TRACE_RETAIN: RangeInclusive<u64> = 1..=1000;
}

/// The result of [`load`]: the settings, where they came from, and a
/// warning when the file, or a key in it, was not used.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LoadedSettings {
    /// The settings; the defaults for what the file did not give.
    pub settings: AppSettings,
    /// The path that was looked up, or `None` when no path resolved.
    pub path: Option<PathBuf>,
    /// Why the file, or a key in it, was not used, for a toast or a log
    /// line.
    pub warning: Option<String>,
}

// ── reading ────────────────────────────────────────────────────────────────

/// Reads the keys of one section and collects a warning per bad key.
struct SectionReader<'a> {
    name: &'static str,
    table: Option<&'a toml::Table>,
    warnings: &'a mut Vec<String>,
}

impl<'a> SectionReader<'a> {
    fn new(root: &'a toml::Table, name: &'static str, warnings: &'a mut Vec<String>) -> Self {
        let table = match root.get(name) {
            None => None,
            Some(toml::Value::Table(table)) => Some(table),
            Some(_) => {
                warnings.push(format!("[{name}] is not a table; its defaults apply"));
                None
            }
        };
        Self {
            name,
            table,
            warnings,
        }
    }

    fn value(&self, key: &str) -> Option<&'a toml::Value> {
        self.table.and_then(|table| table.get(key))
    }

    fn warn(&mut self, key: &str, problem: &str) {
        self.warnings.push(format!(
            "[{}] {key}: {problem}; the default applies",
            self.name
        ));
    }

    fn bool(&mut self, key: &str, default: bool) -> bool {
        match self.value(key) {
            None => default,
            Some(toml::Value::Boolean(value)) => *value,
            Some(_) => {
                self.warn(key, "not true or false");
                default
            }
        }
    }

    fn float(&mut self, key: &str, default: f64, range: &RangeInclusive<f64>) -> f64 {
        let value = match self.value(key) {
            None => return default,
            Some(toml::Value::Float(value)) => *value,
            // An integer setting is far below 2^53, so the cast is exact.
            Some(toml::Value::Integer(value)) => *value as f64,
            Some(_) => {
                self.warn(key, "not a number");
                return default;
            }
        };
        if value.is_finite() && range.contains(&value) {
            value
        } else {
            self.warn(
                key,
                &format!("{value} is outside {} to {}", range.start(), range.end()),
            );
            default
        }
    }

    fn count(&mut self, key: &str, default: usize, range: &RangeInclusive<u64>) -> usize {
        let value = match self.value(key) {
            None => return default,
            Some(toml::Value::Integer(value)) => *value,
            Some(_) => {
                self.warn(key, "not a whole number");
                return default;
            }
        };
        match u64::try_from(value) {
            Ok(value) if range.contains(&value) => usize::try_from(value).unwrap_or(default),
            _ => {
                self.warn(
                    key,
                    &format!("{value} is outside {} to {}", range.start(), range.end()),
                );
                default
            }
        }
    }

    fn token<T: SettingToken>(&mut self, key: &str, default: T) -> T {
        self.optional_token(key).unwrap_or(default)
    }

    fn optional_token<T: SettingToken>(&mut self, key: &str) -> Option<T> {
        let text = match self.value(key) {
            None => return None,
            Some(toml::Value::String(text)) => text,
            Some(_) => {
                self.warn(key, "not a text value");
                return None;
            }
        };
        let parsed = T::from_token(text);
        if parsed.is_none() {
            let accepted: Vec<&str> = T::ALL.iter().map(|value| value.token()).collect();
            self.warn(
                key,
                &format!("\"{text}\" is not one of {}", accepted.join(", ")),
            );
        }
        parsed
    }

    /// A folder. An empty text is no value.
    fn path(&mut self, key: &str) -> Option<PathBuf> {
        match self.value(key) {
            None => None,
            Some(toml::Value::String(text)) if text.trim().is_empty() => None,
            Some(toml::Value::String(text)) => Some(PathBuf::from(text)),
            Some(_) => {
                self.warn(key, "not a text value");
                None
            }
        }
    }

    /// A table of booleans.
    fn bool_table(&mut self, key: &str) -> BTreeMap<String, bool> {
        let mut out = BTreeMap::new();
        match self.value(key) {
            None => {}
            Some(toml::Value::Table(table)) => {
                for (id, value) in table {
                    match value {
                        toml::Value::Boolean(on) => {
                            out.insert(id.clone(), *on);
                        }
                        _ => self.warn(&format!("{key}.{id}"), "not true or false"),
                    }
                }
            }
            Some(_) => self.warn(key, "not a table"),
        }
        out
    }
}

/// Narrow a checked `f64` setting to the `f32` the GUI stores. Every `f32`
/// setting has a range far inside `f32`, so the narrowing loses only digits
/// that no surface shows.
fn narrow(value: f64) -> f32 {
    value as f32
}

/// Parse the text of a settings file. A key with a bad value gives its
/// default and one warning. Pure, so a test can give it any text.
///
/// # Errors
/// A sentence when the text is not TOML. The caller then uses the defaults.
pub fn parse_lenient(text: &str) -> Result<(AppSettings, Vec<String>), String> {
    let root: toml::Table = text
        .parse()
        .map_err(|error| format!("{SETTINGS_FILE} is not valid: {error}"))?;
    let mut warnings = Vec::new();
    let defaults = AppSettings::default();

    let memory_limit = {
        let mut memory = SectionReader::new(&root, "memory", &mut warnings);
        match memory.value("limit") {
            None => MemoryLimit::Default,
            Some(toml::Value::Integer(bytes)) => match u64::try_from(*bytes) {
                Ok(bytes) => MemoryLimit::Bytes(bytes),
                Err(_) => {
                    memory.warn("limit", &format!("{bytes} is not a size"));
                    MemoryLimit::Default
                }
            },
            Some(toml::Value::String(text)) => match parse_limit_text(text) {
                Ok(limit) => limit,
                Err(error) => {
                    memory.warn("limit", &error.to_string());
                    MemoryLimit::Default
                }
            },
            Some(_) => {
                memory.warn("limit", "not a size");
                MemoryLimit::Default
            }
        }
    };

    let general = {
        let d = &defaults.general;
        let mut s = SectionReader::new(&root, "general", &mut warnings);
        GeneralSettings {
            window_width: narrow(s.float(
                "window_width",
                f64::from(d.window_width),
                &limits::WINDOW_SIDE,
            )),
            window_height: narrow(s.float(
                "window_height",
                f64::from(d.window_height),
                &limits::WINDOW_SIDE,
            )),
            undo_depth: s.count("undo_depth", d.undo_depth, &limits::UNDO_DEPTH),
            toast_info_seconds: s.float(
                "toast_info_seconds",
                d.toast_info_seconds,
                &limits::TOAST_SECONDS,
            ),
            toast_warning_seconds: s.float(
                "toast_warning_seconds",
                d.toast_warning_seconds,
                &limits::TOAST_SECONDS,
            ),
            toast_error_seconds: s.float(
                "toast_error_seconds",
                d.toast_error_seconds,
                &limits::TOAST_SECONDS,
            ),
            confirm_unsaved_quit: s.bool("confirm_unsaved_quit", d.confirm_unsaved_quit),
        }
    };

    let display = {
        let d = &defaults.display;
        let mut s = SectionReader::new(&root, "display", &mut warnings);
        DisplaySettings {
            show_all_toolpaths: s.bool("show_all_toolpaths", d.show_all_toolpaths),
            toolpath_colour_mode: s.token("toolpath_colour_mode", d.toolpath_colour_mode),
            overlays: s.bool_table("overlays"),
        }
    };

    let simulation = {
        let d = &defaults.simulation;
        let mut s = SectionReader::new(&root, "simulation", &mut warnings);
        SimulationSettings {
            playback_speed: narrow(s.float(
                "playback_speed",
                f64::from(d.playback_speed),
                &limits::PLAYBACK_SPEED,
            )),
            stock_view: s.token("stock_view", d.stock_view),
            stock_opacity: narrow(s.float(
                "stock_opacity",
                f64::from(d.stock_opacity),
                &limits::STOCK_OPACITY,
            )),
        }
    };

    let paths = {
        let mut s = SectionReader::new(&root, "paths", &mut warnings);
        PathSettings {
            tool_library: s.path("tool_library"),
            machine_library: s.path("machine_library"),
            screenshots: s.path("screenshots"),
        }
    };

    let diagnostics = {
        let d = &defaults.diagnostics;
        let mut s = SectionReader::new(&root, "diagnostics", &mut warnings);
        DiagnosticsSettings {
            save_cut_trace: s.bool("save_cut_trace", d.save_cut_trace),
            cut_trace_retain: s.count(
                "cut_trace_retain",
                d.cut_trace_retain,
                &limits::CUT_TRACE_RETAIN,
            ),
            artifact_dir: s.path("artifact_dir"),
            log_level: s.token("log_level", d.log_level),
            present_mode: s.optional_token("present_mode"),
        }
    };

    Ok((
        AppSettings {
            memory_limit,
            general,
            display,
            simulation,
            paths,
            diagnostics,
        },
        warnings,
    ))
}

/// Parse the text of a settings file, strictly.
///
/// # Errors
/// A sentence that names the problem: bad TOML, or every key with a bad
/// value (for example `[memory] limit`).
pub fn parse(text: &str) -> Result<AppSettings, String> {
    let (settings, warnings) = parse_lenient(text)?;
    if warnings.is_empty() {
        Ok(settings)
    } else {
        Err(warnings.join("; "))
    }
}

/// Read the settings of this process. Never fails; see [`LoadedSettings`].
#[must_use]
pub fn load() -> LoadedSettings {
    match settings_path() {
        Some(path) => load_from(&path),
        None => LoadedSettings::default(),
    }
}

/// Read the settings from `path`. A missing file gives the defaults with no
/// warning.
#[must_use]
pub fn load_from(path: &Path) -> LoadedSettings {
    let used_path = Some(path.to_path_buf());
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return LoadedSettings {
                path: used_path,
                ..LoadedSettings::default()
            };
        }
        Err(error) => {
            return LoadedSettings {
                path: used_path,
                warning: Some(format!(
                    "{} was not read ({error}); the defaults apply",
                    path.display()
                )),
                ..LoadedSettings::default()
            };
        }
    };
    match parse_lenient(&text) {
        Ok((settings, warnings)) => LoadedSettings {
            settings,
            path: used_path,
            warning: (!warnings.is_empty())
                .then(|| format!("{}: {}", path.display(), warnings.join("; "))),
        },
        Err(problem) => LoadedSettings {
            path: used_path,
            warning: Some(format!("{}: {problem}; the defaults apply", path.display())),
            ..LoadedSettings::default()
        },
    }
}

// ── writing ────────────────────────────────────────────────────────────────

/// A process-wide counter that gives every write its own temporary file
/// name, as `crate::session::save` does for a project file. The process id
/// keeps two processes apart.
static SETTINGS_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Writes the owned keys of one section. The writer takes the section out
/// of the file's table and [`SectionWriter::close`] puts it back, unless
/// it is empty.
struct SectionWriter {
    name: &'static str,
    table: toml::Table,
}

impl SectionWriter {
    /// Take the section `name` out of `root`. A value that is not a table
    /// is replaced by a table.
    fn open(root: &mut toml::Table, name: &'static str) -> Self {
        let table = match root.remove(name) {
            Some(toml::Value::Table(table)) => table,
            _ => toml::Table::new(),
        };
        Self { name, table }
    }

    /// Put the section back into `root`, unless the writer left it empty.
    fn close(self, root: &mut toml::Table) {
        if !self.table.is_empty() {
            root.insert(self.name.to_owned(), toml::Value::Table(self.table));
        }
    }

    /// Write `value`, or remove the key when the value is the default.
    fn put(&mut self, key: &str, value: toml::Value, is_default: bool) {
        if is_default {
            self.table.remove(key);
        } else {
            self.table.insert(key.to_owned(), value);
        }
    }

    fn put_float(&mut self, key: &str, value: f64, default: f64) {
        self.put(key, toml::Value::Float(value), value == default);
    }

    fn put_count(&mut self, key: &str, value: usize, default: usize) {
        let stored = i64::try_from(value).unwrap_or(i64::MAX);
        self.put(key, toml::Value::Integer(stored), value == default);
    }

    fn put_bool(&mut self, key: &str, value: bool, default: bool) {
        self.put(key, toml::Value::Boolean(value), value == default);
    }

    fn put_token<T: SettingToken>(&mut self, key: &str, value: T, default: T) {
        self.put(
            key,
            toml::Value::String(value.token().to_owned()),
            value == default,
        );
    }

    fn put_optional_token<T: SettingToken>(&mut self, key: &str, value: Option<T>) {
        match value {
            Some(value) => self.put(key, toml::Value::String(value.token().to_owned()), false),
            None => {
                self.table.remove(key);
            }
        }
    }

    fn put_path(&mut self, key: &str, value: Option<&Path>) {
        match value {
            Some(path) => self.put(
                key,
                toml::Value::String(path.to_string_lossy().into_owned()),
                false,
            ),
            None => {
                self.table.remove(key);
            }
        }
    }
}

/// The new text of a settings file: `existing` with every key of
/// `settings` written. Every key and table that this module does not own
/// stays. Pure, so a test can give it any text.
///
/// `toml` reads the file into a `toml::Table`, so the comments of the file
/// do not stay.
///
/// # Errors
/// A sentence when `existing` is not valid TOML. The writer then does not
/// write, so it never deletes keys that it could not read.
pub fn merged_text(existing: &str, settings: &AppSettings) -> Result<String, String> {
    let mut root: toml::Table = existing.parse().map_err(|error| {
        format!("{SETTINGS_FILE} is not valid TOML ({error}); correct or delete it first")
    })?;
    let defaults = AppSettings::default();

    // `[memory] limit` is always written, as before this writer existed.
    let mut w = SectionWriter::open(&mut root, "memory");
    w.put(
        "limit",
        toml::Value::String(limit_text(settings.memory_limit)),
        false,
    );
    w.close(&mut root);

    {
        let (s, d) = (&settings.general, &defaults.general);
        let mut w = SectionWriter::open(&mut root, "general");
        w.put_float(
            "window_width",
            f64::from(s.window_width),
            f64::from(d.window_width),
        );
        w.put_float(
            "window_height",
            f64::from(s.window_height),
            f64::from(d.window_height),
        );
        w.put_count("undo_depth", s.undo_depth, d.undo_depth);
        w.put_float(
            "toast_info_seconds",
            s.toast_info_seconds,
            d.toast_info_seconds,
        );
        w.put_float(
            "toast_warning_seconds",
            s.toast_warning_seconds,
            d.toast_warning_seconds,
        );
        w.put_float(
            "toast_error_seconds",
            s.toast_error_seconds,
            d.toast_error_seconds,
        );
        w.put_bool(
            "confirm_unsaved_quit",
            s.confirm_unsaved_quit,
            d.confirm_unsaved_quit,
        );
        w.close(&mut root);
    }

    {
        let (s, d) = (&settings.display, &defaults.display);
        let mut w = SectionWriter::open(&mut root, "display");
        w.put_bool(
            "show_all_toolpaths",
            s.show_all_toolpaths,
            d.show_all_toolpaths,
        );
        w.put_token(
            "toolpath_colour_mode",
            s.toolpath_colour_mode,
            d.toolpath_colour_mode,
        );
        let overlays: toml::Table = s
            .overlays
            .iter()
            .map(|(id, on)| (id.clone(), toml::Value::Boolean(*on)))
            .collect();
        let empty = overlays.is_empty();
        w.put("overlays", toml::Value::Table(overlays), empty);
        w.close(&mut root);
    }

    {
        let (s, d) = (&settings.simulation, &defaults.simulation);
        let mut w = SectionWriter::open(&mut root, "simulation");
        w.put_float(
            "playback_speed",
            f64::from(s.playback_speed),
            f64::from(d.playback_speed),
        );
        w.put_token("stock_view", s.stock_view, d.stock_view);
        w.put_float(
            "stock_opacity",
            f64::from(s.stock_opacity),
            f64::from(d.stock_opacity),
        );
        w.close(&mut root);
    }

    {
        let s = &settings.paths;
        let mut w = SectionWriter::open(&mut root, "paths");
        w.put_path("tool_library", s.tool_library.as_deref());
        w.put_path("machine_library", s.machine_library.as_deref());
        w.put_path("screenshots", s.screenshots.as_deref());
        w.close(&mut root);
    }

    {
        let (s, d) = (&settings.diagnostics, &defaults.diagnostics);
        let mut w = SectionWriter::open(&mut root, "diagnostics");
        w.put_bool("save_cut_trace", s.save_cut_trace, d.save_cut_trace);
        w.put_count("cut_trace_retain", s.cut_trace_retain, d.cut_trace_retain);
        w.put_path("artifact_dir", s.artifact_dir.as_deref());
        w.put_token("log_level", s.log_level, d.log_level);
        w.put_optional_token("present_mode", s.present_mode);
        w.close(&mut root);
    }

    toml::to_string(&root).map_err(|error| format!("{SETTINGS_FILE} was not written: {error}"))
}

/// Write `settings` into the settings file at `path`.
///
/// - The writer makes the folder when it does not exist.
/// - A missing file gives a new file.
/// - The other keys and tables of an existing file stay ([`merged_text`]).
/// - The writer writes a temporary file in the same folder and renames it
///   over `path`. A rename cannot cross a file system, so the temporary
///   file is not in the system temporary folder.
///
/// # Errors
/// A sentence that names the path and the problem. On an error the file at
/// `path` does not change.
pub fn save_to(path: &Path, settings: &AppSettings) -> Result<(), String> {
    let existing = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("{} was not read: {error}", path.display())),
    };
    let text = merged_text(&existing, settings)
        .map_err(|problem| format!("{}: {problem}", path.display()))?;
    let parent = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("{} was not made: {error}", parent.display()))?;
    let seq = SETTINGS_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temp_path = parent.join(format!(".rs_cam_settings_{}_{seq}.tmp", std::process::id()));
    if let Err(error) = std::fs::write(&temp_path, text) {
        // A failed write can leave a part of the file.
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!("{} was not written: {error}", path.display()));
    }
    if let Err(error) = std::fs::rename(&temp_path, path) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!("{} was not written: {error}", path.display()));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
