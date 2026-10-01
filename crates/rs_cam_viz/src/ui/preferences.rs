//! File ▸ Preferences: the app settings window.
//!
//! Operator ruling 2026-10-02: "memory budget should be configurable in
//! File > Preferences but default to half" (half of the system RAM).
//!
//! The window has one section today, Memory. It shows the system memory,
//! the budget that the compute lanes enforce now, and where that budget
//! comes from. The operator selects Default (half of RAM), Custom (a binary
//! size) or Unlimited, and clicks Apply.
//!
//! The window owns no file IO and no budget. Apply pushes
//! [`AppEvent::ApplyPreferences`] with a validated [`MemoryLimit`]. The
//! controller writes `settings.toml` through the ONE core writer
//! (`rs_cam_core::budget::settings::save_limit_to`) and gives the new
//! budget to the running compute backend.
//!
//! The draft (the choice and the typed size) lives in
//! [`AppState::preferences`], not in egui memory: it is a panel draft with
//! content (`state/CLAUDE.md`). `None` means the window is closed.

use std::path::PathBuf;

use rs_cam_core::budget::settings::LoadedSettings;
use rs_cam_core::budget::{MemoryBudget, MemoryLimit, format_bytes, format_exact_size};

use super::AppEvent;
use crate::state::AppState;
use crate::ui::components::{
    Banner, Button, ChoiceRow, KeyValue, KeyValueRow, Role, SectionHeader, text,
};
use crate::ui::tokens;

/// The width of the window, in points. A layout value, not a measurement.
const WINDOW_WIDTH: f32 = 460.0;

/// The label column of the Memory rows, in points. One value for every row
/// of the panel (`KeyValueRow::label_width`).
const LABEL_WIDTH: f32 = 130.0;

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
    /// The file exists but did not read or parse, so the default applies.
    DefaultAfterBadFile,
}

impl BudgetSource {
    /// The text of the Source row.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Default => "Default: half of the system memory",
            Self::SettingsFile => "The settings file",
            Self::DefaultAfterBadFile => "Default: the settings file did not read",
        }
    }
}

/// The open Preferences window: what it shows and the operator's draft.
#[derive(Debug, Clone, PartialEq)]
pub struct PreferencesState {
    /// The choice in the window.
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
    /// Why the file was not used, from the loader.
    pub file_warning: Option<String>,
    /// Why the last Apply did not write the file.
    pub apply_error: Option<String>,
}

impl PreferencesState {
    /// The window state for the settings in `loaded`, the backend budget
    /// `effective` and the system memory `system_bytes`.
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
        let source = if loaded.warning.is_some() {
            BudgetSource::DefaultAfterBadFile
        } else if limit == MemoryLimit::Default {
            BudgetSource::Default
        } else {
            BudgetSource::SettingsFile
        };
        Self {
            memory_choice,
            custom_text,
            system_bytes,
            effective,
            file_budget: MemoryBudget::from_setting_on(limit, system_bytes),
            source,
            settings_path: loaded.path.clone(),
            file_warning: loaded.warning.clone(),
            apply_error: None,
        }
    }

    /// The limit that Apply writes, or why the draft is not valid.
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

    /// The budget that `limit` gives on this machine.
    #[must_use]
    pub fn budget_for(&self, limit: MemoryLimit) -> MemoryBudget {
        MemoryBudget::from_setting_on(limit, self.system_bytes)
    }
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
/// nothing. Apply, or Enter in the size field, pushes
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
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width(WINDOW_WIDTH)
        .open(&mut still_open)
        .show(ctx, |ui| {
            cancel = draw_content(ui, prefs, events);
        });
    let escape = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    if !still_open || cancel || escape {
        state.preferences = None;
    }
}

/// The window body. Returns true when the operator clicked Cancel.
fn draw_content(
    ui: &mut egui::Ui,
    prefs: &mut PreferencesState,
    events: &mut Vec<AppEvent>,
) -> bool {
    SectionHeader::new("Memory").show(ui);

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
        ui.label(text::caption(format!(
            "The settings file gives {}. The app reads the file only at start.",
            budget_text(prefs.file_budget)
        )));
    }
    if let Some(warning) = &prefs.file_warning {
        ui.add_space(tokens::SPACE_2);
        Banner::new(Role::Caution, warning.clone()).show(ui);
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
            ui.label(text::caption(format!(
                "The limit is {} on this machine.",
                budget_text(half)
            )));
        }
        MemoryChoice::Custom => {
            let response = ui.add(
                egui::TextEdit::singleline(&mut prefs.custom_text)
                    .desired_width(tokens::WELL_MIN_WIDTH * 2.0)
                    .hint_text("24GiB"),
            );
            enter_applies = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            ui.label(text::caption(
                "Binary units: B, KiB, MiB, GiB, TiB. A bare number is a byte count.",
            ));
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
    let path_text = prefs.settings_path.as_ref().map_or_else(
        || "No settings path: set RS_CAM_SETTINGS, XDG_CONFIG_HOME or HOME.".to_owned(),
        |path| format!("Settings file: {}", path.display()),
    );
    ui.label(text::caption(path_text));
    ui.label(text::caption(concat!(
        "The CLI flag --memory-limit does not apply to the GUI. ",
        "In a CLI run, it overrides this file.",
    )));
    ui.label(text::caption(
        "A job that runs keeps its old limit. The next job uses the new limit.",
    ));

    if let Some(error) = &prefs.apply_error {
        ui.add_space(tokens::SPACE_2);
        Banner::new(Role::Danger, error.clone()).show(ui);
    }

    ui.add_space(tokens::SPACE_3);
    let chosen = prefs.chosen_limit();
    let can_apply = chosen.is_ok() && prefs.settings_path.is_some();
    let mut cancel = false;
    ui.horizontal(|ui| {
        let apply = ui
            .add(Button::primary("Apply").enabled(can_apply))
            .clicked();
        if (apply || (enter_applies && can_apply))
            && let Ok(limit) = &chosen
        {
            events.push(AppEvent::ApplyPreferences(*limit));
        }
        cancel = ui.add(Button::new("Cancel")).clicked();
    });
    cancel
}

#[cfg(test)]
mod tests {
    // SAFETY: test module; a failed unwrap is a failed test.
    #![allow(clippy::unwrap_used)]

    use super::*;

    const GIB: u64 = 1 << 30;

    fn loaded(limit: MemoryLimit, warning: Option<&str>) -> LoadedSettings {
        LoadedSettings {
            settings: rs_cam_core::budget::settings::AppSettings {
                memory_limit: limit,
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
        assert_eq!(state.memory_choice, MemoryChoice::Default);
        assert_eq!(state.source, BudgetSource::Default);
        assert_eq!(state.file_budget, half);
        assert_eq!(state.chosen_limit(), Ok(MemoryLimit::Default));

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

    #[test]
    fn a_bad_custom_draft_is_not_a_limit() {
        let mut state = PreferencesState::new(
            &loaded(MemoryLimit::Default, None),
            MemoryBudget::UNLIMITED,
            None,
        );
        state.memory_choice = MemoryChoice::Custom;
        state.custom_text = "12GB".to_owned();
        assert!(state.chosen_limit().is_err());
        state.custom_text = "12GiB".to_owned();
        assert_eq!(state.chosen_limit(), Ok(MemoryLimit::Bytes(12 * GIB)));
        // No system total: the default has no limit.
        assert_eq!(
            state.budget_for(MemoryLimit::Default),
            MemoryBudget::UNLIMITED
        );
        assert_eq!(budget_text(MemoryBudget::UNLIMITED), "No limit");
        assert_eq!(budget_text(MemoryBudget::with_limit(12 * GIB)), "12.00 GiB");
    }
}
