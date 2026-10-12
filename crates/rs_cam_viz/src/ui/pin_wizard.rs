//! The pin wizard (R4; round 2, 2026-10-12): the one place that edits the
//! alignment pins. The stock panel's "Set up pins…" or "Edit pins…" opens
//! it.
//!
//! Two steps:
//!
//! 1. Dowels: the flip diagram of the judged setup (or the refusal and its
//!    fix), the pattern, the dowel sizes and one drill row for each size.
//! 2. Place: the plan view, Auto-place, a move along the flip axis, the
//!    pattern values, the part models, and the chips of the report beside
//!    Apply.
//!
//! Every pin control keeps the set symmetric under the flip: a set move
//! goes along the flip axis only, and the distance off the axis is the
//! pattern's "Pair off axis" value (`pins::edit`).
//!
//! The wizard works on its OWN draft (`PinWizardState::pins`, the drill
//! queue). Cancel changes nothing that the draft holds. Apply sends ONE
//! `AppEvent::PinWizardApply`; the pin door applies it. Three fixes apply
//! at once through the door, and their hover says so: "Add a Bottom
//! setup", "Add a Top setup" and the part-model choice. Every number comes
//! from the one core report, `ProjectSession::pin_set_report_for`, of the
//! draft.

use rs_cam_core::compute::alignment_pins::{AlignmentPin, NOMINAL_DOWEL_MM, same_pin_diameter};
use rs_cam_core::session::{PinPreset, PinSetReport, ProjectSession};

use super::AppEvent;
use super::components::{Banner, Button, ChoiceRow, Role, StatusChip, UiExt as _, ValueRow, text};
use super::properties::PanelEdit;
use super::properties::pins::{
    KeyedPinInputs, PartModelRow, chip_words, drills, edit, flip_view, keyed_pin_plan, plan_view,
};
use crate::state::AppState;
use crate::state::job::FaceUp;

/// The default size of the wizard window, in points. It fits a 1600 x 900
/// screen with room round it.
const DEFAULT_WIDTH: f32 = 720.0;
const DEFAULT_HEIGHT: f32 = 640.0;
/// The space the window keeps clear round it on the screen, in points.
const SCREEN_MARGIN: f32 = 48.0;
/// The height of the window parts round the step body: the title bar, the
/// stepper, the two separators, the chips and the button row, in points.
const CHROME_HEIGHT: f32 = 180.0;
/// The smallest height of the step body, in points.
const MIN_BODY_HEIGHT: f32 = 200.0;
/// The largest height of the plan view of step 2, in points.
const PLAN_MAX_HEIGHT: f32 = 420.0;
/// The steps of a move along the flip axis, in mm.
const NUDGE_STEPS_MM: [f64; 4] = [-1.0, -0.5, 0.5, 1.0];

/// The two steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WizardStep {
    Dowels,
    Place,
}

impl WizardStep {
    const ALL: [Self; 2] = [Self::Dowels, Self::Place];

    fn title(self) -> &'static str {
        match self {
            Self::Dowels => "1. Dowels",
            Self::Place => "2. Place",
        }
    }
}

/// The pin pattern of step 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WizardPattern {
    /// One pin on the flip axis and a mirror pair off it, one dowel size.
    AxisPair,
    /// Two pins of two dowel sizes on the flip axis.
    Mixed,
}

impl WizardPattern {
    const OPTIONS: [(Self, &'static str); 2] = [
        (Self::AxisPair, "3 pins, one size"),
        (Self::Mixed, "2 pins, two sizes"),
    ];
}

/// The wizard's own draft. `AppState::panels.pin_wizard` holds it while the
/// wizard is open.
#[derive(Debug, Clone)]
pub struct PinWizardState {
    pub step: WizardStep,
    pub pattern: WizardPattern,
    /// The dowel of the 3-pin pattern, and the small dowel of the mixed
    /// pattern, in mm.
    pub small_d: f64,
    /// The large dowel of the mixed pattern, in mm.
    pub large_d: f64,
    /// The draft pins, stock-local.
    pub pins: Vec<AlignmentPin>,
    /// The drill sizes that Apply adds as tools.
    pub add_drills: Vec<f64>,
    /// The tool that Apply binds to the pin-drill operation of a size.
    pub use_tools: Vec<(f64, usize)>,
    /// The pin marked on the plan view.
    pub selected: Option<usize>,
    /// The refusal of the last placement, until the next one.
    pub place_refusal: Option<String>,
}

impl PinWizardState {
    /// A wizard on the stored pins of `session`.
    #[must_use]
    pub fn open(session: &ProjectSession) -> Self {
        let stock = session.stock_config();
        let report = session.pin_set_report();
        let keyed = KeyedPinInputs::from_session(session);
        let small_d = keyed.plan_pin_diameter(stock);
        let drills = super::properties::drill_tool_diameters(session);
        let (pattern, small_d, large_d) = match report.preset {
            Some(PinPreset::TwoMixedDiameters(fit)) => {
                (WizardPattern::Mixed, fit.small_diameter, fit.large_diameter)
            }
            _ => (
                WizardPattern::AxisPair,
                small_d,
                edit::mixed_large_diameter(small_d, &drills),
            ),
        };
        Self {
            step: WizardStep::Dowels,
            pattern,
            small_d,
            large_d,
            pins: stock.alignment_pins.clone(),
            add_drills: Vec::new(),
            use_tools: Vec::new(),
            selected: None,
            place_refusal: None,
        }
    }

    /// Place the pattern with the placer (`keyed_pin_plan`), at the
    /// wizard's dowel sizes.
    pub fn place(&mut self, session: &ProjectSession) {
        let report = session.pin_set_report_for(&self.pins);
        let map = &report.map;
        // The placer sizes the pins from the stored pins first; an empty
        // set makes it take the wizard's dowel.
        let mut stock = session.stock_config().clone();
        stock.alignment_pins.clear();
        let inputs = KeyedPinInputs {
            pin_tool_diameter: Some(self.small_d),
            ..KeyedPinInputs::from_session(session)
        };
        let plan = keyed_pin_plan(&stock, map.face_up(), map.z_rotation(), &inputs);
        let placed = match self.pattern {
            WizardPattern::AxisPair => plan
                .map(|pins| pins.to_vec())
                .map_err(|e| format!("The placer refuses: {e}")),
            WizardPattern::Mixed => edit::mixed_preset_pins(
                &self.pins,
                map,
                plan.as_ref().ok(),
                self.small_d,
                self.large_d,
            ),
        };
        match placed {
            Ok(pins) => {
                self.pins = pins;
                self.place_refusal = None;
            }
            Err(reason) => self.place_refusal = Some(reason),
        }
    }

    /// Give the pins the wizard's dowel sizes, per pattern role. A draft
    /// that fits the chosen pattern keeps its positions; any other draft
    /// is placed again.
    pub fn resize(&mut self, session: &ProjectSession) {
        let report = session.pin_set_report_for(&self.pins);
        let fits = matches!(
            (self.pattern, report.preset),
            (
                WizardPattern::AxisPair,
                Some(PinPreset::AxisPlusMirrorPair(_))
            ) | (WizardPattern::Mixed, Some(PinPreset::TwoMixedDiameters(_)))
        );
        match edit::with_diameters(&report.map, report.preset, self.small_d, self.large_d) {
            Some(pins) if fits => {
                self.pins = pins;
                self.place_refusal = None;
            }
            _ => self.place(session),
        }
    }
}

/// What the operator did with the wizard this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Close {
    Stay,
    Apply,
    Cancel,
}

/// Draw the wizard when it is open.
pub fn draw(ctx: &egui::Context, state: &mut AppState, events: &mut Vec<AppEvent>) {
    let Some(mut wizard) = state.panels.pin_wizard.take() else {
        return;
    };
    let session = &state.session;
    let report = session.pin_set_report_for(&wizard.pins);
    let screen = ctx.content_rect();
    let max_width = (screen.width() - SCREEN_MARGIN).max(320.0);
    let max_height = (screen.height() - SCREEN_MARGIN).max(320.0);

    let mut close = Close::Stay;
    let mut still_open = true;
    egui::Window::new("Pin setup")
        .id(egui::Id::new("pin_wizard_window"))
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width(DEFAULT_WIDTH.min(max_width))
        .default_height(DEFAULT_HEIGHT.min(max_height))
        .max_width(max_width)
        .max_height(max_height)
        .open(&mut still_open)
        .show(ctx, |ui| {
            stepper(ui, &mut wizard, session);
            ui.separator();
            // The step scrolls; the stepper and the buttons stay in view.
            egui::ScrollArea::vertical()
                .max_height((max_height - CHROME_HEIGHT).max(MIN_BODY_HEIGHT))
                .auto_shrink([false, true])
                .show(ui, |ui| match wizard.step {
                    WizardStep::Dowels => step_dowels(ui, session, &report, &mut wizard, events),
                    WizardStep::Place => step_place(ui, session, &report, &mut wizard, events),
                });
            ui.separator();
            close = nav(ui, &mut wizard, session, &report);
        });
    if !still_open {
        close = Close::Cancel;
    }
    match close {
        Close::Stay => state.panels.pin_wizard = Some(wizard),
        Close::Cancel => {}
        Close::Apply => events.push(AppEvent::PinWizardApply {
            pins: wizard.pins,
            add_drills: wizard.add_drills,
            use_tools: wizard.use_tools,
        }),
    }
}

/// Go to `step`. The Place step with no pins places the pattern first.
fn go_to(wizard: &mut PinWizardState, session: &ProjectSession, step: WizardStep) {
    if step == WizardStep::Place && wizard.pins.is_empty() {
        wizard.place(session);
    }
    wizard.step = step;
}

fn stepper(ui: &mut egui::Ui, wizard: &mut PinWizardState, session: &ProjectSession) {
    ui.horizontal_wrapped(|ui| {
        for s in WizardStep::ALL {
            let mut title = egui::RichText::new(s.title());
            if s == wizard.step {
                title = title.strong();
            }
            if ui.selectable_label(s == wizard.step, title).clicked() {
                go_to(wizard, session, s);
            }
        }
    });
}

fn nav(
    ui: &mut egui::Ui,
    wizard: &mut PinWizardState,
    session: &ProjectSession,
    report: &PinSetReport,
) -> Close {
    let mut close = Close::Stay;
    if wizard.step == WizardStep::Place && !wizard.pins.is_empty() {
        ui.horizontal_wrapped(|ui| {
            for chip in chip_words(report) {
                ui.add(StatusChip::new(chip.word, chip.role));
            }
        });
    }
    let valid = !wizard.pins.is_empty() && report.flip.seats() && report.flip.keyed();
    ui.horizontal_wrapped(|ui| {
        if ui
            .add(Button::new("Back").enabled(wizard.step == WizardStep::Place))
            .clicked()
        {
            wizard.step = WizardStep::Dowels;
        }
        if wizard.step == WizardStep::Dowels && ui.add(Button::primary("Next")).clicked() {
            go_to(wizard, session, WizardStep::Place);
        }
        let can_apply = !wizard.pins.is_empty() && (wizard.step == WizardStep::Place || valid);
        if ui
            .add(Button::primary("Apply").enabled(can_apply))
            .on_hover_text("Writes the pins and the drills (one undo step).")
            .clicked()
        {
            close = Close::Apply;
        }
        if ui
            .add(Button::quiet("Cancel"))
            .on_hover_text("Closes the wizard. The pins and the tools do not change.")
            .clicked()
        {
            close = Close::Cancel;
        }
    });
    close
}

/// The flip diagram and its one-line caption, or the refusal and its fix.
fn flip_block(
    ui: &mut egui::Ui,
    session: &ProjectSession,
    report: &PinSetReport,
    pins: &[AlignmentPin],
    events: &mut Vec<AppEvent>,
) {
    let judged = report
        .judged_setup
        .and_then(|i| session.list_setups().get(i))
        .map(|s| s.name.clone());
    match (judged, report.map.axis_label()) {
        (Some(name), Some(axis)) => {
            let stock = session.stock_config();
            let _ = flip_view::show(ui, report, pins, stock.x, stock.y);
            ui.label(text::caption(format!("Flip: {name}, about {axis}")));
        }
        _ => {
            Banner::new(
                Role::Danger,
                "No setup turns the part over in the pin plane.",
            )
            .show(ui);
            if ui
                .add(Button::primary("Add a Bottom setup"))
                .on_hover_text("Adds it now. Cancel does not remove it.")
                .clicked()
            {
                events.push(AppEvent::PinAddSetup(FaceUp::Bottom));
            }
        }
    }
}

/// A row of quick size picks. A click writes the size into `value`.
fn size_picks(ui: &mut egui::Ui, sizes: &[f64], value: &mut f64) -> bool {
    if sizes.is_empty() {
        return false;
    }
    let mut picked = false;
    ui.horizontal_wrapped(|ui| {
        ui.label(text::caption("Sizes:"));
        for d in sizes {
            if ui.add(Button::quiet(format!("{d:.2} mm"))).clicked() {
                *value = *d;
                picked = true;
            }
        }
    });
    picked
}

/// The one-line refusal of the last placement.
fn refusal_line(ui: &mut egui::Ui, wizard: &PinWizardState) {
    if let Some(reason) = &wizard.place_refusal {
        ui.label(
            egui::RichText::new(reason)
                .small()
                .color(crate::ui::tokens::DANGER),
        );
    }
}

/// Step 1: the dowels.
fn step_dowels(
    ui: &mut egui::Ui,
    session: &ProjectSession,
    report: &PinSetReport,
    wizard: &mut PinWizardState,
    events: &mut Vec<AppEvent>,
) {
    flip_block(ui, session, report, &wizard.pins, events);
    ui.add_space(crate::ui::tokens::SPACE_2);

    let before = wizard.pattern;
    ui.add(
        ChoiceRow::new("Pattern", &mut wizard.pattern, &WizardPattern::OPTIONS).hover(
            "3 pins: one on the flip axis and a mirror pair, one drill. 2 pins: two sizes on \
             the axis, two drills.",
        ),
    );
    let pattern_changed = wizard.pattern != before;

    // The sizes the project can drill, and the nominal dowel.
    let mut known = super::properties::drill_tool_diameters(session);
    if !known
        .iter()
        .any(|d| same_pin_diameter(*d, NOMINAL_DOWEL_MM))
    {
        known.push(NOMINAL_DOWEL_MM);
        known.sort_by(f64::total_cmp);
    }
    let small_label = match wizard.pattern {
        WizardPattern::AxisPair => "Dowel",
        WizardPattern::Mixed => "Small dowel",
    };
    let mut edit_out = PanelEdit::default();
    ui.param_grid("pin_wizard_dowel", |ui| {
        let out = ValueRow::new(small_label, &mut wizard.small_d, " mm", 0.1, 1.0..=25.0).show(ui);
        edit_out.drag(&out.value_response);
    });
    let mut resized = size_picks(ui, &known, &mut wizard.small_d);
    if wizard.pattern == WizardPattern::Mixed {
        let min_large = wizard.small_d + edit::MIN_DIAMETER_STEP_MM;
        if wizard.large_d < min_large {
            wizard.large_d = edit::mixed_start_large_diameter(wizard.small_d);
        }
        ui.param_grid("pin_wizard_dowel_large", |ui| {
            let out = ValueRow::new(
                "Large dowel",
                &mut wizard.large_d,
                " mm",
                0.1,
                min_large..=25.0,
            )
            .tooltip(Some("Set it to the dowel you have."))
            .show(ui);
            edit_out.drag(&out.value_response);
        });
        let larger: Vec<f64> = known.iter().copied().filter(|d| *d > min_large).collect();
        resized |= size_picks(ui, &larger, &mut wizard.large_d);
    }
    resized |= edit_out.committed;
    if pattern_changed {
        wizard.place(session);
    } else if resized {
        wizard.resize(session);
    }
    refusal_line(ui, wizard);

    ui.add_space(crate::ui::tokens::SPACE_2);
    ui.label(text::subhead("Drills"));
    let mut queue = drills::DrillQueue {
        add_drills: &mut wizard.add_drills,
        use_tools: &mut wizard.use_tools,
    };
    drills::drill_rows(ui, session, &wizard.pins, &mut queue, events);
}

/// Step 2: the placement.
fn step_place(
    ui: &mut egui::Ui,
    session: &ProjectSession,
    report: &PinSetReport,
    wizard: &mut PinWizardState,
    events: &mut Vec<AppEvent>,
) {
    let stock = session.stock_config();
    let mut edit_out = PanelEdit::default();
    let view = plan_view::show(
        ui,
        "pin_wizard",
        &mut wizard.pins,
        plan_view::PlanInputs {
            stock_w: stock.x,
            stock_d: stock.y,
            report,
            selected: wizard.selected,
            max_height: PLAN_MAX_HEIGHT,
        },
        &mut edit_out,
    );
    if let Some(selected) = view.selected_pin {
        wizard.selected = selected;
    }
    let has_axis = edit::axis_unit(&report.map).is_some();
    ui.horizontal_wrapped(|ui| {
        if ui
            .add(Button::new("Auto-place"))
            .on_hover_text("Places the pattern in the clear material.")
            .clicked()
        {
            wizard.place(session);
        }
        if has_axis {
            ui.label(text::caption("Move along the axis"));
            for step in NUDGE_STEPS_MM {
                if ui.add(Button::quiet(format!("{step:+}"))).clicked() {
                    wizard.pins = edit::move_along_axis(&wizard.pins, &report.map, step);
                }
            }
        }
    });
    refusal_line(ui, wizard);

    // The values of the pattern the draft fits.
    let length = edit::axis_length(&report.map);
    let along_hover = Some("The distance along the flip axis from its first end.");
    match report.preset {
        Some(PinPreset::AxisPlusMirrorPair(fit)) => {
            let mut p = fit;
            ui.param_grid("pin_wizard_axis_pair", |ui| {
                ValueRow::new(
                    "Axis pin along",
                    &mut p.axis_along,
                    " mm",
                    0.5,
                    0.0..=length,
                )
                .tooltip(along_hover)
                .show(ui);
                ValueRow::new("Pair along", &mut p.pair_along, " mm", 0.5, 0.0..=length)
                    .tooltip(along_hover)
                    .show(ui);
                ValueRow::new(
                    "Pair off axis",
                    &mut p.pair_across,
                    " mm",
                    0.5,
                    edit::MIN_PAIR_ACROSS_MM..=length.max(edit::MIN_PAIR_ACROSS_MM),
                )
                .tooltip(Some("The distance of each pair pin from the flip axis."))
                .show(ui);
            });
            if p != fit
                && let Some(pins) = edit::axis_pair_pins(&report.map, p)
            {
                wizard.pins = pins;
            }
        }
        Some(PinPreset::TwoMixedDiameters(fit)) => {
            let mut p = fit;
            ui.param_grid("pin_wizard_mixed", |ui| {
                ValueRow::new(
                    "Small pin along",
                    &mut p.small_along,
                    " mm",
                    0.5,
                    0.0..=length,
                )
                .tooltip(along_hover)
                .show(ui);
                ValueRow::new(
                    "Large pin along",
                    &mut p.large_along,
                    " mm",
                    0.5,
                    0.0..=length,
                )
                .tooltip(along_hover)
                .show(ui);
            });
            if p != fit
                && let Some(pins) = edit::mixed_pins(&report.map, p)
            {
                wizard.pins = pins;
            }
        }
        Some(PinPreset::Custom) | None => {}
    }
    part_models(ui, session, events);
}

/// The part-model chooser: which models the placer and the wall check call
/// the part. A choice applies at once through the pin door, so the plan
/// view and the placer read it.
fn part_models(ui: &mut egui::Ui, session: &ProjectSession, events: &mut Vec<AppEvent>) {
    let models = PartModelRow::of(session);
    if models.is_empty() {
        return;
    }
    let count = models.iter().filter(|m| m.is_part).count();
    let chosen = session.stock_config().pin_part_models.is_some();
    let title = format!("Part models: {count} of {}", models.len());
    ui.disclosure("pin_wizard_part_models", &title, false, |ui| {
        let mut picked: Vec<usize> = models.iter().filter(|m| m.is_part).map(|m| m.id).collect();
        let mut changed = false;
        for m in &models {
            let mut on = m.is_part;
            let label = if m.default_part {
                m.name.clone()
            } else {
                format!("{} (not part by default)", m.name)
            };
            if ui
                .checkbox(&mut on, label)
                .on_hover_text("Applies now.")
                .changed()
            {
                changed = true;
                if on {
                    picked.push(m.id);
                } else {
                    picked.retain(|id| *id != m.id);
                }
            }
        }
        if changed {
            picked.sort_unstable();
            events.push(AppEvent::PinPartModels(Some(picked)));
        }
        if chosen && ui.add(Button::quiet("Use the default")).clicked() {
            events.push(AppEvent::PinPartModels(None));
        }
    });
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn the_wizard_has_two_steps() {
        assert_eq!(WizardStep::ALL, [WizardStep::Dowels, WizardStep::Place]);
        assert_eq!(WizardStep::Place.title(), "2. Place");
    }
}
