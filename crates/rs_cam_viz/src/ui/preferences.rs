//! File ▸ Preferences: the app settings window.
//!
//! Operator request 2026-10-02: every app setting that is not a project
//! value is in one window, with a category bar on the left. Operator ruling
//! the same day: the memory budget defaults to half of the RAM, and the
//! cut-trace file is not saved by default (a Diagnostics switch).
//!
//! The categories:
//!
//! | Category | Settings file section |
//! |---|---|
//! | General | `[general]` |
//! | Memory | `[memory]` |
//! | Display | `[display]` and `[display.overlays]` |
//! | Simulation | `[simulation]` |
//! | Files and libraries | `[paths]` |
//! | Diagnostics (advanced) | `[diagnostics]` |
//! | Automation | none: read-only status |
//!
//! The window owns no file IO. Apply pushes [`AppEvent::ApplyPreferences`].
//! The controller reads the draft through [`PreferencesState::chosen_settings`],
//! writes `settings.toml` through the ONE core writer
//! (`rs_cam_core::settings::save_to`), applies the live values and posts an
//! Info toast. Cancel, Escape and the title-bar cross discard the draft.
//!
//! The draft (every value and every typed text) lives in
//! [`AppState::preferences`], not in egui memory: it is a panel draft with
//! content (`state/CLAUDE.md`). The category is part of the draft, so a
//! category switch keeps every value that the operator typed. `None` means
//! the window is closed.
//!
//! What this window does NOT offer: a value that changes a computed number
//! (feed modulation, timing constants) or an instrument switch for a test
//! or a bench (`RS_CAM_STAMP_DISPATCH`, `RS_CAM_PLAYBACK_DISPATCH`). Those
//! stay in code or in the environment.

use std::ops::RangeInclusive;
use std::path::PathBuf;

use rs_cam_core::budget::{MemoryBudget, MemoryLimit, format_bytes, format_exact_size};
use rs_cam_core::settings::{
    AppSettings, LoadedSettings, LogLevel, PresentModeSetting, SettingToken, StockViewDefault,
    ToolpathColourDefault, limits,
};

use super::AppEvent;
use crate::state::AppState;
use crate::ui::components::{
    Banner, Button, ChoiceRow, KeyValue, KeyValueRow, Role, SectionHeader, text,
};
use crate::ui::tokens;

/// The default width of the window, in points. A layout value.
const WINDOW_WIDTH: f32 = 760.0;

/// The default height of the window, in points. A layout value.
const WINDOW_HEIGHT: f32 = 520.0;

/// The width of the category bar, in points. A layout value.
const CATEGORY_WIDTH: f32 = 170.0;

/// The label column of every row, in points: the column that `ChoiceRow`
/// uses, so the text rows and the choice rows line up.
const LABEL_WIDTH: f32 = tokens::LABEL_COL_WIDTH;

/// The width of a number field, in points.
const NUMBER_FIELD_WIDTH: f32 = tokens::WELL_MIN_WIDTH * 2.0;

/// The width of a folder field, in points.
const FOLDER_FIELD_WIDTH: f32 = 360.0;

/// The caption under a value that the app reads only at start.
const RESTART_NOTE: &str = "Takes effect after a restart.";

/// One category of the left bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreferencesCategory {
    General,
    Memory,
    Display,
    Simulation,
    Files,
    Diagnostics,
    Automation,
}

impl PreferencesCategory {
    /// Every category, in the order of the bar.
    pub const ALL: [Self; 7] = [
        Self::General,
        Self::Memory,
        Self::Display,
        Self::Simulation,
        Self::Files,
        Self::Diagnostics,
        Self::Automation,
    ];

    /// The text of the category in the bar and in its header.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Memory => "Memory",
            Self::Display => "Display",
            Self::Simulation => "Simulation",
            Self::Files => "Files and libraries",
            Self::Diagnostics => "Diagnostics",
            Self::Automation => "Automation (MCP)",
        }
    }

    /// An advanced category: the bar marks it.
    #[must_use]
    pub fn is_advanced(self) -> bool {
        self == Self::Diagnostics
    }
}

/// The memory limit the operator selects in the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryChoice {
    /// Half of the system memory (`budget::DEFAULT_SYSTEM_FRACTION`).
    Default,
    /// A size that the operator types.
    Custom,
    /// No limit.
    Unlimited,
}

/// Where the budget in the settings file comes from, when the window opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetSource {
    /// No file, no `limit` key, or `limit = "default"`.
    Default,
    /// The file gives a size or `"unlimited"`.
    SettingsFile,
    /// The file has a problem (see the notice), and it gives no limit, so
    /// the default applies.
    DefaultAfterBadFile,
}

impl BudgetSource {
    /// The text of the Source row.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Default => "Default: half of the system memory",
            Self::SettingsFile => "The settings file",
            Self::DefaultAfterBadFile => "Default: the settings file has a problem",
        }
    }
}

/// The value of one overlay in the Display category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayChoice {
    /// The default of each workspace (no key in the file).
    Workspace,
    /// On in every viewport workspace.
    On,
    /// Off in every viewport workspace.
    Off,
}

/// What the Automation category shows. Read when the window opens.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AutomationStatus {
    /// The app runs with `--mcp`: the embedded MCP server is on.
    pub mcp_active: bool,
    /// The present mode that the launch requested.
    pub present_requested: Option<String>,
    /// Where the requested mode came from.
    pub present_source: Option<&'static str>,
    /// The present mode that wgpu negotiated, when it logged it.
    pub present_negotiated: Option<String>,
}

/// The folders that apply when a folder field is empty, and the
/// environment overrides. Read when the window opens.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FolderFacts {
    /// `<config folder>/tools`.
    pub tool_default: Option<PathBuf>,
    /// `<config folder>/machines`.
    pub machine_default: Option<PathBuf>,
    /// `<user cache>/rs_cam/artifacts`.
    pub artifact_default: Option<PathBuf>,
    /// `RS_CAM_TOOL_DIR`, when it is set: it wins over the field.
    pub tool_env: Option<String>,
    /// `RS_CAM_MACHINE_DIR`, when it is set: it wins over the field.
    pub machine_env: Option<String>,
    /// `RUST_LOG`, when it is set: it wins over the log level.
    pub rust_log: Option<String>,
    /// `RS_CAM_PRESENT_MODE`, when it is set: it wins over the present mode.
    pub present_env: Option<String>,
}

impl FolderFacts {
    /// The facts of this process.
    #[must_use]
    pub fn of_process() -> Self {
        use rs_cam_core::settings::paths;

        let env = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
        let process = |name: &str| std::env::var(name).ok();
        // The defaults: the resolvers with no override and no file value.
        let no_override = |name: &str| match name {
            paths::TOOL_DIR_ENV | paths::MACHINE_DIR_ENV => None,
            _ => process(name),
        };
        Self {
            tool_default: paths::tool_library_dir_from(no_override, None),
            machine_default: paths::machine_library_dir_from(no_override, None),
            artifact_default: paths::artifact_dir_from(process, None),
            tool_env: env(paths::TOOL_DIR_ENV),
            machine_env: env(paths::MACHINE_DIR_ENV),
            rust_log: env("RUST_LOG"),
            present_env: env(crate::present_mode::PRESENT_MODE_ENV),
        }
    }
}

/// The typed texts of the window. Each is parsed at Apply, so a field that
/// the operator is typing never loses its focus to a reformat.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PreferenceTexts {
    pub window_width: String,
    pub window_height: String,
    pub undo_depth: String,
    pub toast_info: String,
    pub toast_warning: String,
    pub toast_error: String,
    pub playback_speed: String,
    pub cut_trace_retain: String,
    pub tool_library: String,
    pub machine_library: String,
    pub screenshots: String,
    pub artifact_dir: String,
}

/// A number as the window shows it: no decimals for a whole number.
fn number_text(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value}")
    }
}

/// A folder as the window shows it: empty for the default.
fn folder_text(path: Option<&PathBuf>) -> String {
    path.map(|path| path.display().to_string())
        .unwrap_or_default()
}

impl PreferenceTexts {
    fn of(settings: &AppSettings) -> Self {
        let g = &settings.general;
        Self {
            window_width: number_text(f64::from(g.window_width)),
            window_height: number_text(f64::from(g.window_height)),
            undo_depth: g.undo_depth.to_string(),
            toast_info: number_text(g.toast_info_seconds),
            toast_warning: number_text(g.toast_warning_seconds),
            toast_error: number_text(g.toast_error_seconds),
            playback_speed: number_text(f64::from(settings.simulation.playback_speed)),
            cut_trace_retain: settings.diagnostics.cut_trace_retain.to_string(),
            tool_library: folder_text(settings.paths.tool_library.as_ref()),
            machine_library: folder_text(settings.paths.machine_library.as_ref()),
            screenshots: folder_text(settings.paths.screenshots.as_ref()),
            artifact_dir: folder_text(settings.diagnostics.artifact_dir.as_ref()),
        }
    }
}

/// Parse a number field. The sentence names the field and the range.
fn parse_number(label: &str, text: &str, range: &RangeInclusive<f64>) -> Result<f64, String> {
    match text.trim().parse::<f64>() {
        Ok(value) if value.is_finite() && range.contains(&value) => Ok(value),
        _ => Err(format!(
            "{label}: type a number from {} to {}.",
            number_text(*range.start()),
            number_text(*range.end())
        )),
    }
}

/// Parse a whole-number field. The sentence names the field and the range.
fn parse_count(label: &str, text: &str, range: &RangeInclusive<u64>) -> Result<usize, String> {
    match text.trim().parse::<u64>() {
        Ok(value) if range.contains(&value) => {
            usize::try_from(value).map_err(|error| format!("{label}: {error}."))
        }
        _ => Err(format!(
            "{label}: type a whole number from {} to {}.",
            range.start(),
            range.end()
        )),
    }
}

/// A folder field: an empty field is the default (`None`).
fn parse_folder(text: &str) -> Option<PathBuf> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

/// Narrow a checked number to the `f32` the settings store. Every `f32`
/// setting has a range far inside `f32`.
fn narrow(value: f64) -> f32 {
    value as f32
}

/// The open Preferences window: what it shows and the operator's draft.
#[derive(Debug, Clone, PartialEq)]
pub struct PreferencesState {
    /// The category in the right-hand area.
    pub category: PreferencesCategory,
    /// The values that are not typed text: switches, choices, the overlay
    /// map, the opacity. The typed values are in [`Self::texts`];
    /// [`Self::chosen_settings`] joins the two.
    pub draft: AppSettings,
    /// The typed texts.
    pub texts: PreferenceTexts,
    /// The memory choice in the window.
    pub memory_choice: MemoryChoice,
    /// The text of the Custom size field.
    pub custom_text: String,
    /// The system memory, read ONCE when the window opens. `None` when the
    /// platform does not give it.
    pub system_bytes: Option<u64>,
    /// The budget that the compute backend enforces when the window opens.
    pub effective: MemoryBudget,
    /// The budget that the settings file gives now. It differs from
    /// `effective` when the file changed after the app started.
    pub file_budget: MemoryBudget,
    /// Where the file's budget comes from.
    pub source: BudgetSource,
    /// The settings file that Apply writes. `None` when no path resolves.
    pub settings_path: Option<PathBuf>,
    /// Why the file, or a key in it, was not used, from the loader.
    pub file_warning: Option<String>,
    /// Why the last Apply did not write the file.
    pub apply_error: Option<String>,
    /// The Automation category.
    pub automation: AutomationStatus,
    /// The default folders and the environment overrides.
    pub folders: FolderFacts,
}

impl PreferencesState {
    /// The window state for the settings in `loaded`, the backend budget
    /// `effective` and the system memory `system_bytes`. The window opens
    /// on the General category; the Automation status and the folder facts
    /// are empty until the controller fills them.
    #[must_use]
    pub fn new(
        loaded: &LoadedSettings,
        effective: MemoryBudget,
        system_bytes: Option<u64>,
    ) -> Self {
        let limit = loaded.settings.memory_limit;
        let (memory_choice, custom_text) = match limit {
            MemoryLimit::Default => (MemoryChoice::Default, String::new()),
            MemoryLimit::Unlimited => (MemoryChoice::Unlimited, String::new()),
            MemoryLimit::Bytes(bytes) => (MemoryChoice::Custom, format_exact_size(bytes)),
        };
        let source = if limit != MemoryLimit::Default {
            BudgetSource::SettingsFile
        } else if loaded.warning.is_some() {
            BudgetSource::DefaultAfterBadFile
        } else {
            BudgetSource::Default
        };
        Self {
            category: PreferencesCategory::General,
            draft: loaded.settings.clone(),
            texts: PreferenceTexts::of(&loaded.settings),
            memory_choice,
            custom_text,
            system_bytes,
            effective,
            file_budget: MemoryBudget::from_setting_on(limit, system_bytes),
            source,
            settings_path: loaded.path.clone(),
            file_warning: loaded.warning.clone(),
            apply_error: None,
            automation: AutomationStatus::default(),
            folders: FolderFacts::default(),
        }
    }

    /// The memory limit that Apply writes, or why the draft is not valid.
    ///
    /// # Errors
    /// The sentence of [`validate_custom_size`] for a bad Custom size.
    pub fn chosen_limit(&self) -> Result<MemoryLimit, String> {
        match self.memory_choice {
            MemoryChoice::Default => Ok(MemoryLimit::Default),
            MemoryChoice::Unlimited => Ok(MemoryLimit::Unlimited),
            MemoryChoice::Custom => validate_custom_size(&self.custom_text),
        }
    }

    /// The settings that Apply writes: the draft with every typed text
    /// parsed, or the first problem.
    ///
    /// # Errors
    /// A sentence that names the field and the values it takes.
    pub fn chosen_settings(&self) -> Result<AppSettings, String> {
        let t = &self.texts;
        let mut settings = self.draft.clone();
        settings.memory_limit = self.chosen_limit()?;

        let g = &mut settings.general;
        g.window_width = narrow(parse_number(
            "Window width",
            &t.window_width,
            &limits::WINDOW_SIDE,
        )?);
        g.window_height = narrow(parse_number(
            "Window height",
            &t.window_height,
            &limits::WINDOW_SIDE,
        )?);
        g.undo_depth = parse_count("Undo steps", &t.undo_depth, &limits::UNDO_DEPTH)?;
        g.toast_info_seconds = parse_number("Info toast", &t.toast_info, &limits::TOAST_SECONDS)?;
        g.toast_warning_seconds =
            parse_number("Warning toast", &t.toast_warning, &limits::TOAST_SECONDS)?;
        g.toast_error_seconds =
            parse_number("Error toast", &t.toast_error, &limits::TOAST_SECONDS)?;

        settings.simulation.playback_speed = narrow(parse_number(
            "Playback speed",
            &t.playback_speed,
            &limits::PLAYBACK_SPEED,
        )?);
        settings.simulation.stock_opacity = settings.simulation.stock_opacity.clamp(0.0, 1.0);

        settings.paths.tool_library = parse_folder(&t.tool_library);
        settings.paths.machine_library = parse_folder(&t.machine_library);
        settings.paths.screenshots = parse_folder(&t.screenshots);

        let d = &mut settings.diagnostics;
        d.cut_trace_retain = parse_count(
            "Cut-trace files to keep",
            &t.cut_trace_retain,
            &limits::CUT_TRACE_RETAIN,
        )?;
        d.artifact_dir = parse_folder(&t.artifact_dir);
        Ok(settings)
    }

    /// The budget that `limit` gives on this machine.
    #[must_use]
    pub fn budget_for(&self, limit: MemoryLimit) -> MemoryBudget {
        MemoryBudget::from_setting_on(limit, self.system_bytes)
    }

    /// The draft value of overlay `id`.
    #[must_use]
    pub fn overlay_choice(&self, id: &str) -> OverlayChoice {
        match self.draft.display.overlays.get(id) {
            None => OverlayChoice::Workspace,
            Some(true) => OverlayChoice::On,
            Some(false) => OverlayChoice::Off,
        }
    }

    /// Set the draft value of overlay `id`.
    pub fn set_overlay_choice(&mut self, id: &str, choice: OverlayChoice) {
        let overlays = &mut self.draft.display.overlays;
        match choice {
            OverlayChoice::Workspace => {
                overlays.remove(id);
            }
            OverlayChoice::On => {
                overlays.insert(id.to_owned(), true);
            }
            OverlayChoice::Off => {
                overlays.insert(id.to_owned(), false);
            }
        }
    }
}

/// True when `next` changes a value that the app reads only at start: the
/// window size, the log level, the present mode.
#[must_use]
pub fn needs_restart(current: &AppSettings, next: &AppSettings) -> bool {
    current.general.window_width != next.general.window_width
        || current.general.window_height != next.general.window_height
        || current.diagnostics.log_level != next.diagnostics.log_level
        || current.diagnostics.present_mode != next.diagnostics.present_mode
}

/// Validate the text of the Custom size field. It reads through the
/// settings parser (`parse_limit_text`), so the window, the file and the
/// CLI flag accept the same sizes.
///
/// # Errors
/// A sentence for an empty or bad size, a decimal unit, zero, or a word
/// that the Default or Unlimited choice owns.
pub fn validate_custom_size(text: &str) -> Result<MemoryLimit, String> {
    if text.trim().is_empty() {
        return Err("Type a size, for example 24GiB.".to_owned());
    }
    match crate::io::app_settings::parse_limit_text(text) {
        Ok(MemoryLimit::Bytes(0)) => Err("The limit must be larger than 0 B.".to_owned()),
        Ok(MemoryLimit::Bytes(bytes)) => Ok(MemoryLimit::Bytes(bytes)),
        Ok(MemoryLimit::Unlimited) => Err("For no limit, select Unlimited.".to_owned()),
        Ok(MemoryLimit::Default) => Err("For the default, select Default.".to_owned()),
        Err(error) => Err(format!("{error}.")),
    }
}

/// The text of a budget: its limit, or "No limit".
#[must_use]
pub fn budget_text(budget: MemoryBudget) -> String {
    budget
        .limit_bytes
        .map_or_else(|| "No limit".to_owned(), format_bytes)
}

/// The value slot of a budget row: a measured size, or the text "No limit".
fn budget_value(budget: MemoryBudget) -> KeyValue {
    match budget.limit_bytes {
        Some(bytes) => KeyValue::Measured(format_bytes(bytes)),
        None => KeyValue::Text("No limit".to_owned()),
    }
}

/// Draw the window. Does nothing when it is closed.
///
/// Escape, the title-bar cross and Cancel close the window and write
/// nothing. Apply, or Enter in the memory size field, pushes
/// [`AppEvent::ApplyPreferences`]; the controller closes the window when
/// the write succeeds.
pub fn draw(ctx: &egui::Context, state: &mut AppState, events: &mut Vec<AppEvent>) {
    let Some(prefs) = state.preferences.as_mut() else {
        return;
    };
    let mut still_open = true;
    let mut cancel = false;
    egui::Window::new("Preferences")
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width(WINDOW_WIDTH)
        .default_height(WINDOW_HEIGHT)
        .open(&mut still_open)
        .show(ctx, |ui| {
            cancel = draw_content(ui, prefs, events);
        });
    let escape = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    if !still_open || cancel || escape {
        state.preferences = None;
    }
}

/// The window body: the action bar, the category bar, the category.
/// Returns true when the operator clicked Cancel.
fn draw_content(
    ui: &mut egui::Ui,
    prefs: &mut PreferencesState,
    events: &mut Vec<AppEvent>,
) -> bool {
    let mut cancel = false;
    let mut enter_applies = false;

    egui::Panel::bottom("prefs_actions").show(ui, |ui| {
        cancel = draw_actions(ui, prefs, events);
    });

    egui::Panel::left("prefs_categories")
        .resizable(false)
        .default_size(CATEGORY_WIDTH)
        .show(ui, |ui| {
            draw_category_bar(ui, prefs);
        });

    egui::CentralPanel::default().show(ui, |ui| {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                SectionHeader::new(prefs.category.label()).show(ui);
                match prefs.category {
                    PreferencesCategory::General => draw_general(ui, prefs),
                    PreferencesCategory::Memory => enter_applies = draw_memory(ui, prefs),
                    PreferencesCategory::Display => draw_display(ui, prefs),
                    PreferencesCategory::Simulation => draw_simulation(ui, prefs),
                    PreferencesCategory::Files => draw_files(ui, prefs),
                    PreferencesCategory::Diagnostics => draw_diagnostics(ui, prefs),
                    PreferencesCategory::Automation => draw_automation(ui, prefs),
                }
            });
    });

    if enter_applies && prefs.chosen_settings().is_ok() && prefs.settings_path.is_some() {
        events.push(AppEvent::ApplyPreferences);
    }
    cancel
}

/// The category bar: one selectable row per category.
fn draw_category_bar(ui: &mut egui::Ui, prefs: &mut PreferencesState) {
    ui.add_space(tokens::SPACE_3);
    for category in PreferencesCategory::ALL {
        let label = if category.is_advanced() {
            format!("{} (advanced)", category.label())
        } else {
            category.label().to_owned()
        };
        let selected = prefs.category == category;
        let response = ui.add_sized(
            [ui.available_width(), tokens::ROW_DENSE],
            egui::Button::selectable(selected, label),
        );
        if response.clicked() {
            prefs.category = category;
        }
    }
}

/// Apply and Cancel, the settings path and the first problem of the draft.
/// Returns true when the operator clicked Cancel.
fn draw_actions(ui: &mut egui::Ui, prefs: &PreferencesState, events: &mut Vec<AppEvent>) -> bool {
    ui.add_space(tokens::SPACE_2);
    let path_text = prefs.settings_path.as_ref().map_or_else(
        || "No settings path: set RS_CAM_SETTINGS, XDG_CONFIG_HOME or HOME.".to_owned(),
        |path| format!("Settings file: {}", path.display()),
    );
    ui.label(text::caption(path_text));
    if let Some(warning) = &prefs.file_warning {
        Banner::new(Role::Caution, warning.clone()).show(ui);
    }
    if let Some(error) = &prefs.apply_error {
        Banner::new(Role::Danger, error.clone()).show(ui);
    }
    let chosen = prefs.chosen_settings();
    if let Err(problem) = &chosen {
        ui.label(egui::RichText::new(problem).color(tokens::CAUTION));
    }
    let can_apply = chosen.is_ok() && prefs.settings_path.is_some();
    let mut cancel = false;
    ui.horizontal(|ui| {
        if ui
            .add(Button::primary("Apply").enabled(can_apply))
            .clicked()
        {
            events.push(AppEvent::ApplyPreferences);
        }
        cancel = ui.add(Button::new("Cancel")).clicked();
    });
    ui.add_space(tokens::SPACE_2);
    cancel
}

/// One labelled text field.
fn text_row(ui: &mut egui::Ui, label: &str, value: &mut String, width: f32, hint: &str) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [LABEL_WIDTH, tokens::ROW_DENSE],
            egui::Label::new(label).truncate(),
        );
        ui.add(
            egui::TextEdit::singleline(value)
                .desired_width(width)
                .hint_text(hint),
        );
    });
}

/// One on-or-off row.
fn switch_row(ui: &mut egui::Ui, label: &str, value: &mut bool, hover: &str) {
    let options = [(true, "On"), (false, "Off")];
    ui.add(ChoiceRow::new(label, value, &options).hover(hover));
}

/// One closed choice over a [`SettingToken`].
fn token_row<T: SettingToken>(ui: &mut egui::Ui, label: &str, value: &mut T, hover: &str) {
    let options: Vec<(T, &str)> = T::ALL.iter().map(|value| (*value, value.label())).collect();
    ui.add(ChoiceRow::new(label, value, &options).hover(hover));
}

fn caption(ui: &mut egui::Ui, sentence: &str) {
    ui.label(text::caption(sentence));
}

fn draw_general(ui: &mut egui::Ui, prefs: &mut PreferencesState) {
    let t = &mut prefs.texts;
    text_row(ui, "Window width", &mut t.window_width, NUMBER_FIELD_WIDTH, "1400");
    text_row(ui, "Window height", &mut t.window_height, NUMBER_FIELD_WIDTH, "900");
    caption(ui, "The size of the window at start, in points.");
    caption(ui, RESTART_NOTE);

    ui.add_space(tokens::SPACE_3);
    text_row(ui, "Undo steps", &mut t.undo_depth, NUMBER_FIELD_WIDTH, "100");
    caption(ui, "The number of edits that Undo can reverse. Applies at once.");

    ui.add_space(tokens::SPACE_3);
    text_row(ui, "Info toast", &mut t.toast_info, NUMBER_FIELD_WIDTH, "4");
    text_row(ui, "Warning toast", &mut t.toast_warning, NUMBER_FIELD_WIDTH, "6");
    text_row(ui, "Error toast", &mut t.toast_error, NUMBER_FIELD_WIDTH, "8");
    caption(
        ui,
        "How long each toast stays, in seconds. Applies to the next toast.",
    );

    ui.add_space(tokens::SPACE_3);
    switch_row(
        ui,
        "Ask before quit",
        &mut prefs.draft.general.confirm_unsaved_quit,
        "On: a quit with unsaved changes asks first. Off: the app quits at once.",
    );
    if !prefs.draft.general.confirm_unsaved_quit {
        Banner::new(
            Role::Caution,
            "A quit with unsaved changes loses them without a question.",
        )
        .show(ui);
    }
}

/// The Memory category. Returns true when Enter in the size field asks
/// for an Apply.
fn draw_memory(ui: &mut egui::Ui, prefs: &mut PreferencesState) -> bool {
    let system_row = match prefs.system_bytes {
        Some(bytes) => KeyValueRow::new("System memory", KeyValue::Measured(format_bytes(bytes))),
        None => KeyValueRow::not_measured("System memory"),
    };
    ui.add(system_row.label_width(LABEL_WIDTH));
    ui.add(KeyValueRow::new("Budget now", budget_value(prefs.effective)).label_width(LABEL_WIDTH));
    ui.add(
        KeyValueRow::new("Source", KeyValue::Text(prefs.source.label().to_owned()))
            .label_width(LABEL_WIDTH),
    );
    if prefs.file_budget != prefs.effective {
        caption(
            ui,
            &format!(
                "The settings file gives {}. The app reads the file only at start.",
                budget_text(prefs.file_budget)
            ),
        );
    }

    ui.add_space(tokens::SPACE_3);
    let options = [
        (MemoryChoice::Default, "Default (half of RAM)"),
        (MemoryChoice::Custom, "Custom"),
        (MemoryChoice::Unlimited, "Unlimited"),
    ];
    let choice_changed = ui
        .add(
            ChoiceRow::new("Memory limit", &mut prefs.memory_choice, &options).hover(concat!(
                "Default: half of the system memory. ",
                "Custom: a size that you type. ",
                "Unlimited: no limit.",
            )),
        )
        .changed();
    if choice_changed {
        prefs.apply_error = None;
    }

    let mut enter_applies = false;
    match prefs.memory_choice {
        MemoryChoice::Default => {
            let half = prefs.budget_for(MemoryLimit::Default);
            caption(
                ui,
                &format!("The limit is {} on this machine.", budget_text(half)),
            );
        }
        MemoryChoice::Custom => {
            let response = ui.add(
                egui::TextEdit::singleline(&mut prefs.custom_text)
                    .desired_width(NUMBER_FIELD_WIDTH)
                    .hint_text("24GiB"),
            );
            enter_applies = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            caption(
                ui,
                "Binary units: B, KiB, MiB, GiB, TiB. A bare number is a byte count.",
            );
            // Read after the edit, so the message matches the text this frame.
            match &prefs.chosen_limit() {
                Err(problem) => {
                    ui.label(egui::RichText::new(problem).color(tokens::CAUTION));
                }
                Ok(MemoryLimit::Bytes(bytes)) => {
                    if prefs.system_bytes.is_some_and(|total| *bytes > total) {
                        ui.label(
                            egui::RichText::new(concat!(
                                "This limit is larger than the system memory. ",
                                "The system can stop the app before the budget does.",
                            ))
                            .color(tokens::CAUTION),
                        );
                    }
                }
                Ok(_) => {}
            }
        }
        MemoryChoice::Unlimited => {
            ui.add_space(tokens::SPACE_2);
            Banner::new(
                Role::Caution,
                "The app can use all memory. The system may stop it.",
            )
            .show(ui);
        }
    }

    ui.add_space(tokens::SPACE_3);
    caption(
        ui,
        concat!(
            "The CLI flag --memory-limit does not apply to the GUI. ",
            "In a CLI run, it overrides this file.",
        ),
    );
    caption(
        ui,
        "A job that runs keeps its old limit. The next job uses the new limit.",
    );
    enter_applies
}

fn draw_display(ui: &mut egui::Ui, prefs: &mut PreferencesState) {
    switch_row(
        ui,
        "All toolpaths",
        &mut prefs.draft.display.show_all_toolpaths,
        "On: the viewport starts with every generated toolpath drawn. \
         Off (the default): it draws the selected toolpath only.",
    );
    token_row::<ToolpathColourDefault>(
        ui,
        "Toolpath colour",
        &mut prefs.draft.display.toolpath_colour_mode,
        "The toolpath colour mode the viewport starts in.",
    );
    caption(ui, RESTART_NOTE);

    ui.add_space(tokens::SPACE_3);
    SectionHeader::new("Overlay defaults").show(ui);
    caption(
        ui,
        concat!(
            "Workspace: each workspace sets its own default. ",
            "On or Off: the same value in every viewport workspace. ",
            "Applies at the next workspace switch.",
        ),
    );
    let options = [
        (OverlayChoice::Workspace, "Workspace"),
        (OverlayChoice::On, "On"),
        (OverlayChoice::Off, "Off"),
    ];
    for id in crate::ui::overlays::registry::PREFERENCE_OVERLAYS {
        let Some(row) = crate::ui::overlays::registry::row(id) else {
            continue;
        };
        if !crate::ui::overlays::registry::is_preference_overlay(row) {
            continue;
        }
        let mut choice = prefs.overlay_choice(row.id);
        let changed = ui
            .add(ChoiceRow::new(row.label, &mut choice, &options).hover(row.hover))
            .changed();
        if changed {
            prefs.set_overlay_choice(row.id, choice);
        }
    }
}

fn draw_simulation(ui: &mut egui::Ui, prefs: &mut PreferencesState) {
    text_row(
        ui,
        "Playback speed",
        &mut prefs.texts.playback_speed,
        NUMBER_FIELD_WIDTH,
        "500",
    );
    caption(ui, "Moves per second.");
    token_row::<StockViewDefault>(
        ui,
        "Stock view",
        &mut prefs.draft.simulation.stock_view,
        "The colour of the simulated stock.",
    );
    ui.horizontal(|ui| {
        ui.add_sized(
            [LABEL_WIDTH, tokens::ROW_DENSE],
            egui::Label::new("Stock opacity").truncate(),
        );
        ui.add(egui::Slider::new(
            &mut prefs.draft.simulation.stock_opacity,
            0.0..=1.0,
        ));
    });
    caption(
        ui,
        "These are the values at start and after a project opens. They change no computed number.",
    );
}

/// One folder row: the field, the default it falls back to, and the
/// environment variable that wins over it.
fn folder_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
    default: Option<&PathBuf>,
    env: Option<(&str, &String)>,
) {
    let hint = default.map_or_else(
        || "No default folder".to_owned(),
        |path| path.display().to_string(),
    );
    text_row(ui, label, value, FOLDER_FIELD_WIDTH, &hint);
    if let Some((name, value)) = env {
        Banner::new(
            Role::Info,
            format!("{name} is set to {value}. It wins over this field."),
        )
        .show(ui);
    }
}

fn draw_files(ui: &mut egui::Ui, prefs: &mut PreferencesState) {
    let f = &prefs.folders;
    let t = &mut prefs.texts;
    caption(
        ui,
        "An empty field is the default folder. The CLI reads the same folders.",
    );
    folder_row(
        ui,
        "Tool library",
        &mut t.tool_library,
        f.tool_default.as_ref(),
        f.tool_env.as_ref().map(|v| ("RS_CAM_TOOL_DIR", v)),
    );
    folder_row(
        ui,
        "Machine library",
        &mut t.machine_library,
        f.machine_default.as_ref(),
        f.machine_env.as_ref().map(|v| ("RS_CAM_MACHINE_DIR", v)),
    );
    text_row(
        ui,
        "Screenshots",
        &mut t.screenshots,
        FOLDER_FIELD_WIDTH,
        "The current folder",
    );
    caption(ui, "F12 saves a screenshot here. Applies at once.");

    ui.add_space(tokens::SPACE_3);
    let path = prefs.settings_path.as_ref().map_or_else(
        || "No settings path".to_owned(),
        |path| path.display().to_string(),
    );
    ui.add(KeyValueRow::new("Settings file", KeyValue::Text(path)).label_width(LABEL_WIDTH));
    caption(
        ui,
        "RS_CAM_SETTINGS names another settings file. This window cannot move it.",
    );
}

fn draw_diagnostics(ui: &mut egui::Ui, prefs: &mut PreferencesState) {
    caption(
        ui,
        "Advanced. These settings write files for fault-finding. They change no computed number.",
    );
    ui.add_space(tokens::SPACE_2);
    switch_row(
        ui,
        "Save cut traces",
        &mut prefs.draft.diagnostics.save_cut_trace,
        "On: each simulation writes its cut trace as a JSON file. \
         Off (the default): no file. The app keeps the trace in memory either way.",
    );
    if prefs.draft.diagnostics.save_cut_trace {
        caption(ui, "One file can be several GB on a fine simulation.");
    }
    text_row(
        ui,
        "Files to keep",
        &mut prefs.texts.cut_trace_retain,
        NUMBER_FIELD_WIDTH,
        "5",
    );
    let hint = prefs.folders.artifact_default.as_ref().map_or_else(
        || "No default folder".to_owned(),
        |path| path.display().to_string(),
    );
    text_row(
        ui,
        "Artifact folder",
        &mut prefs.texts.artifact_dir,
        FOLDER_FIELD_WIDTH,
        &hint,
    );
    caption(
        ui,
        "Cut traces go to simulation_metrics and debug traces to toolpath_debug in this folder. \
         Applies to the next job.",
    );

    ui.add_space(tokens::SPACE_3);
    token_row::<LogLevel>(
        ui,
        "Log level",
        &mut prefs.draft.diagnostics.log_level,
        "The log level when RUST_LOG is not set.",
    );
    if let Some(value) = &prefs.folders.rust_log {
        Banner::new(
            Role::Info,
            format!("RUST_LOG is set to {value}. It wins over this setting."),
        )
        .show(ui);
    }
    let mut options: Vec<(Option<PresentModeSetting>, &str)> = vec![(None, "Launch default")];
    options.extend(
        PresentModeSetting::ALL
            .iter()
            .map(|mode| (Some(*mode), mode.label())),
    );
    ui.add(
        ChoiceRow::new(
            "Present mode",
            &mut prefs.draft.diagnostics.present_mode,
            &options,
        )
        .hover(
            "Launch default: vsync for a plain launch, no vsync for --mcp. \
             An explicit mode that the display does not support stops the app at start.",
        ),
    );
    if let Some(value) = &prefs.folders.present_env {
        Banner::new(
            Role::Info,
            format!("RS_CAM_PRESENT_MODE is set to {value}. It wins over this setting."),
        )
        .show(ui);
    }
    caption(ui, RESTART_NOTE);
}

fn draw_automation(ui: &mut egui::Ui, prefs: &PreferencesState) {
    let a = &prefs.automation;
    let mcp = if a.mcp_active {
        "On: this app runs with --mcp"
    } else {
        "Off: start the app with --mcp to turn it on"
    };
    ui.add(
        KeyValueRow::new("MCP server", KeyValue::Text(mcp.to_owned())).label_width(LABEL_WIDTH),
    );
    let text_or_unknown = |value: Option<String>, label: &str| match value {
        Some(value) => KeyValueRow::new(label, KeyValue::Text(value)),
        None => KeyValueRow::not_measured(label),
    };
    ui.add(
        text_or_unknown(a.present_requested.clone(), "Present mode asked").label_width(LABEL_WIDTH),
    );
    ui.add(
        text_or_unknown(a.present_source.map(str::to_owned), "Asked by").label_width(LABEL_WIDTH),
    );
    ui.add(
        text_or_unknown(a.present_negotiated.clone(), "Present mode in use")
            .label_width(LABEL_WIDTH),
    );
    caption(
        ui,
        "Read-only. The MCP server starts only from the command line. \
         Diagnostics sets the present mode for the next start.",
    );
}

#[cfg(test)]
mod tests {
    // SAFETY: test module; a failed unwrap is a failed test.
    #![allow(clippy::unwrap_used)]

    use super::*;

    const GIB: u64 = 1 << 30;

    fn loaded(limit: MemoryLimit, warning: Option<&str>) -> LoadedSettings {
        LoadedSettings {
            settings: AppSettings {
                memory_limit: limit,
                ..AppSettings::default()
            },
            path: Some(PathBuf::from("/x/settings.toml")),
            warning: warning.map(str::to_owned),
        }
    }

    #[test]
    fn a_custom_size_reads_binary_units_and_refuses_bad_sizes() {
        assert_eq!(
            validate_custom_size("24GiB"),
            Ok(MemoryLimit::Bytes(24 * GIB))
        );
        assert_eq!(
            validate_custom_size(" 1.5 GiB "),
            Ok(MemoryLimit::Bytes(3 * GIB / 2))
        );
        assert_eq!(validate_custom_size("4096"), Ok(MemoryLimit::Bytes(4096)));
        for bad in [
            "",
            "   ",
            "24GB",
            "lots",
            "-1GiB",
            "0",
            "0GiB",
            "unlimited",
            "default",
        ] {
            assert!(validate_custom_size(bad).is_err(), "{bad:?}");
        }
        let error = validate_custom_size("24GB").unwrap_err();
        assert!(error.contains("binary units"), "{error}");
    }

    #[test]
    fn the_window_opens_on_the_value_of_the_file() {
        let total = Some(64 * GIB);
        let half = MemoryBudget::with_limit(32 * GIB);

        let state = PreferencesState::new(&loaded(MemoryLimit::Default, None), half, total);
        assert_eq!(state.category, PreferencesCategory::General);
        assert_eq!(state.memory_choice, MemoryChoice::Default);
        assert_eq!(state.source, BudgetSource::Default);
        assert_eq!(state.file_budget, half);
        assert_eq!(state.chosen_limit(), Ok(MemoryLimit::Default));
        assert_eq!(state.chosen_settings(), Ok(AppSettings::default()));

        let state = PreferencesState::new(
            &loaded(MemoryLimit::Bytes(24 * GIB), None),
            MemoryBudget::with_limit(24 * GIB),
            total,
        );
        assert_eq!(state.memory_choice, MemoryChoice::Custom);
        assert_eq!(state.custom_text, "24GiB");
        assert_eq!(state.source, BudgetSource::SettingsFile);
        assert_eq!(state.chosen_limit(), Ok(MemoryLimit::Bytes(24 * GIB)));

        let state = PreferencesState::new(
            &loaded(MemoryLimit::Unlimited, None),
            MemoryBudget::UNLIMITED,
            total,
        );
        assert_eq!(state.memory_choice, MemoryChoice::Unlimited);
        assert_eq!(state.chosen_limit(), Ok(MemoryLimit::Unlimited));

        let state =
            PreferencesState::new(&loaded(MemoryLimit::Default, Some("bad file")), half, total);
        assert_eq!(state.source, BudgetSource::DefaultAfterBadFile);
    }

    /// Every typed text and every choice of the draft reaches the settings
    /// that Apply writes.
    #[test]
    fn the_draft_becomes_the_settings_that_apply_writes() {
        let mut state = PreferencesState::new(
            &loaded(MemoryLimit::Default, None),
            MemoryBudget::UNLIMITED,
            None,
        );
        state.texts.window_width = "1920".to_owned();
        state.texts.window_height = " 1080 ".to_owned();
        state.texts.undo_depth = "250".to_owned();
        state.texts.toast_info = "2.5".to_owned();
        state.texts.toast_warning = "10".to_owned();
        state.texts.toast_error = "20".to_owned();
        state.texts.playback_speed = "2000".to_owned();
        state.texts.cut_trace_retain = "12".to_owned();
        state.texts.tool_library = "/lib/tools".to_owned();
        state.texts.machine_library = "  ".to_owned();
        state.texts.screenshots = "/shots".to_owned();
        state.texts.artifact_dir = "/artifacts".to_owned();
        state.draft.general.confirm_unsaved_quit = false;
        state.draft.display.show_all_toolpaths = true;
        state.draft.diagnostics.save_cut_trace = true;
        state.draft.diagnostics.log_level = LogLevel::Debug;
        state.set_overlay_choice("grid", OverlayChoice::Off);
        state.set_overlay_choice("rapids", OverlayChoice::On);
        state.set_overlay_choice("rapids", OverlayChoice::Workspace);
        state.memory_choice = MemoryChoice::Custom;
        state.custom_text = "8GiB".to_owned();

        let s = state.chosen_settings().unwrap();
        assert_eq!(s.memory_limit, MemoryLimit::Bytes(8 * GIB));
        assert_eq!([s.general.window_width, s.general.window_height], [1920.0, 1080.0]);
        assert_eq!(s.general.undo_depth, 250);
        assert_eq!(s.general.toast_info_seconds, 2.5);
        assert_eq!(s.general.toast_warning_seconds, 10.0);
        assert_eq!(s.general.toast_error_seconds, 20.0);
        assert!(!s.general.confirm_unsaved_quit);
        assert!(s.display.show_all_toolpaths);
        assert_eq!(s.display.overlays.get("grid"), Some(&false));
        assert_eq!(s.display.overlays.get("rapids"), None, "back to the workspace");
        assert_eq!(s.simulation.playback_speed, 2000.0);
        assert_eq!(s.paths.tool_library, Some(PathBuf::from("/lib/tools")));
        assert_eq!(s.paths.machine_library, None, "an empty field is the default");
        assert_eq!(s.paths.screenshots, Some(PathBuf::from("/shots")));
        assert!(s.diagnostics.save_cut_trace);
        assert_eq!(s.diagnostics.cut_trace_retain, 12);
        assert_eq!(s.diagnostics.artifact_dir, Some(PathBuf::from("/artifacts")));
        assert_eq!(s.diagnostics.log_level, LogLevel::Debug);
        assert!(needs_restart(&AppSettings::default(), &s));
    }

    /// A bad text refuses Apply with a sentence that names the field.
    #[test]
    fn a_bad_text_is_not_a_setting() {
        let mut state = PreferencesState::new(
            &loaded(MemoryLimit::Default, None),
            MemoryBudget::UNLIMITED,
            None,
        );
        for (field, bad, name) in [
            (0, "wide", "Window width"),
            (1, "10", "Window height"),
            (2, "0", "Undo steps"),
            (3, "-1", "Info toast"),
            (4, "1.5", "Cut-trace files to keep"),
            (5, "0", "Playback speed"),
        ] {
            let mut draft = state.clone();
            let text = match field {
                0 => &mut draft.texts.window_width,
                1 => &mut draft.texts.window_height,
                2 => &mut draft.texts.undo_depth,
                3 => &mut draft.texts.toast_info,
                4 => &mut draft.texts.cut_trace_retain,
                _ => &mut draft.texts.playback_speed,
            };
            *text = bad.to_owned();
            let error = draft.chosen_settings().unwrap_err();
            assert!(error.starts_with(name), "{name}: {error}");
        }
        state.memory_choice = MemoryChoice::Custom;
        state.custom_text = "12GB".to_owned();
        assert!(state.chosen_settings().is_err(), "a bad size refuses too");
        // No system total: the default has no limit.
        assert_eq!(
            state.budget_for(MemoryLimit::Default),
            MemoryBudget::UNLIMITED
        );
        assert_eq!(budget_text(MemoryBudget::UNLIMITED), "No limit");
        assert_eq!(budget_text(MemoryBudget::with_limit(12 * GIB)), "12.00 GiB");
    }

    /// The category is part of the draft: a switch keeps every value.
    #[test]
    fn a_category_switch_keeps_the_draft() {
        let mut state = PreferencesState::new(
            &loaded(MemoryLimit::Default, None),
            MemoryBudget::UNLIMITED,
            None,
        );
        state.texts.undo_depth = "42".to_owned();
        state.draft.diagnostics.save_cut_trace = true;
        state.custom_text = "3GiB".to_owned();
        let before = state.clone();
        for category in PreferencesCategory::ALL {
            state.category = category;
        }
        state.category = before.category;
        assert_eq!(state, before);
        assert_eq!(state.chosen_settings().unwrap().general.undo_depth, 42);
    }

    /// The number texts read back to the same value.
    #[test]
    fn a_number_text_reads_back() {
        assert_eq!(number_text(1400.0), "1400");
        assert_eq!(number_text(2.5), "2.5");
        let texts = PreferenceTexts::of(&AppSettings::default());
        assert_eq!(texts.window_width, "1400");
        assert_eq!(texts.toast_error, "8");
        assert_eq!(texts.playback_speed, "500");
        assert_eq!(texts.tool_library, "");
    }
}
