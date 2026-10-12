//! The pin wizard (R4, 2026-10-12): "Pin setup..." of the stock panel's
//! pin section.
//!
//! Four steps:
//!
//! 1. The flip: which setup the pins are judged against and its flip axis.
//!    With no flipped setup, the step says so and offers "Add a Bottom
//!    setup" (the `AddSetup` row, at once).
//! 2. The dowel: the pattern (3 pins of one size, or 2 pins of two sizes)
//!    and the dowel sizes. A size with no drill offers "Add a <d> mm
//!    drill"; the wizard adds it on Apply.
//! 3. The placement: the large plan view, Auto-place, the nudges, the
//!    along and across values and a drag of the set, with the live keying
//!    margin and walls.
//! 4. The summary: keyed or not, the walls, and the tools and operations
//!    that Apply creates, moves or removes.
//!
//! The wizard works on its OWN draft (`PinWizardState::pins`). Cancel
//! changes nothing. Apply sends ONE `AppEvent::PinWizardApply`: the pin
//! door adds the queued drills, applies one `SetAlignmentPins` (one undo
//! step for the pins) and synchronises the pin-drill operations. Every
//! number the wizard shows comes from the one core report,
//! `ProjectSession::pin_set_report_for`, of the draft.

use rs_cam_core::compute::alignment_pins::{
    AlignmentPin, NOMINAL_DOWEL_MM, pin_diameters, same_pin_diameter,
};
use rs_cam_core::session::{PinPreset, PinSetReport, ProjectSession};

use super::AppEvent;
use super::components::{Banner, Button, ChoiceRow, Role, StatusChip, UiExt as _, ValueRow, text};
use super::properties::PanelEdit;
use super::properties::pins::{
    KeyedPinInputs, chip_words, edit, keyed_pin_plan, mixed_large_diameter,
    mixed_start_large_diameter, plan_view,
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
/// stepper, the two separators and the button row, in points.
const CHROME_HEIGHT: f32 = 150.0;
/// The smallest height of the step body, in points.
const MIN_BODY_HEIGHT: f32 = 200.0;
/// The largest height of the plan view of step 3, in points.
const PLAN_MAX_HEIGHT: f32 = 420.0;
/// The nudge steps of the set, in mm.
const NUDGE_STEPS_MM: [f64; 4] = [-1.0, -0.5, 0.5, 1.0];

/// The four steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WizardStep {
    Flip,
    Dowel,
    Placement,
    Summary,
}

impl WizardStep {
    const ALL: [Self; 4] = [Self::Flip, Self::Dowel, Self::Placement, Self::Summary];

    fn title(self) -> &'static str {
        match self {
            Self::Flip => "1. Flip",
            Self::Dowel => "2. Dowel",
            Self::Placement => "3. Placement",
            Self::Summary => "4. Summary",
        }
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    fn next(self) -> Self {
        Self::ALL.get(self.index() + 1).copied().unwrap_or(self)
    }

    fn back(self) -> Self {
        self.index()
            .checked_sub(1)
            .and_then(|i| Self::ALL.get(i).copied())
            .unwrap_or(self)
    }
}

/// The pin pattern of step 2.
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
                mixed_large_diameter(small_d, &drills),
            ),
        };
        Self {
            step: WizardStep::Flip,
            pattern,
            small_d,
            large_d,
            pins: stock.alignment_pins.clone(),
            add_drills: Vec::new(),
            selected: None,
            place_refusal: None,
        }
    }

    /// Place the pattern with the placer of the stock panel
    /// (`keyed_pin_plan`), at the wizard's dowel sizes.
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

    /// The pin sizes of the draft that no tool of the project drills and
    /// that Apply does not add.
    #[must_use]
    pub fn sizes_without_drill(&self, session: &ProjectSession) -> Vec<f64> {
        pin_diameters(&self.pins)
            .into_iter()
            .filter(|d| session.pin_tool_candidates(*d).is_empty())
            .filter(|d| !self.add_drills.iter().any(|a| same_pin_diameter(*a, *d)))
            .collect()
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
            stepper(ui, &mut wizard.step);
            ui.separator();
            // The step scrolls; the stepper and the buttons stay in view.
            egui::ScrollArea::vertical()
                .max_height((max_height - CHROME_HEIGHT).max(MIN_BODY_HEIGHT))
                .auto_shrink([false, true])
                .show(ui, |ui| match wizard.step {
                    WizardStep::Flip => step_flip(ui, session, &report, events),
                    WizardStep::Dowel => step_dowel(ui, session, &mut wizard),
                    WizardStep::Placement => step_placement(ui, session, &report, &mut wizard),
                    WizardStep::Summary => step_summary(ui, session, &report, &wizard),
                });
            ui.separator();
            close = nav(ui, &mut wizard, session);
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
        }),
    }
}

fn stepper(ui: &mut egui::Ui, step: &mut WizardStep) {
    ui.horizontal_wrapped(|ui| {
        for s in WizardStep::ALL {
            let mut title = egui::RichText::new(s.title());
            if s == *step {
                title = title.strong();
            }
            if ui.selectable_label(s == *step, title).clicked() {
                *step = s;
            }
        }
    });
}

fn nav(ui: &mut egui::Ui, wizard: &mut PinWizardState, session: &ProjectSession) -> Close {
    let mut close = Close::Stay;
    ui.horizontal_wrapped(|ui| {
        if ui
            .add(Button::new("Back").enabled(wizard.step != WizardStep::Flip))
            .clicked()
        {
            wizard.step = wizard.step.back();
        }
        if wizard.step == WizardStep::Summary {
            let missing = wizard.sizes_without_drill(session);
            let apply = ui
                .add(Button::primary("Apply").enabled(!wizard.pins.is_empty()))
                .on_hover_text(
                    "Adds the drills of the list, writes the pins in one step and makes the \
                     pin-drill operations follow them.",
                );
            if apply.clicked() {
                close = Close::Apply;
            }
            if !missing.is_empty() {
                ui.label(text::caption(
                    "A pin size has no drill: its operation is refused until a drill of the \
                     size exists. Step 2 adds one.",
                ));
            }
        } else if ui.add(Button::primary("Next")).clicked() {
            if wizard.step == WizardStep::Dowel && wizard.pins.is_empty() {
                wizard.place(session);
            }
            wizard.step = wizard.step.next();
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

/// Step 1: the flip.
fn step_flip(
    ui: &mut egui::Ui,
    session: &ProjectSession,
    report: &PinSetReport,
    events: &mut Vec<AppEvent>,
) {
    ui.label(text::caption(
        "The pins register the part when it turns over. The check judges the pins against \
         the first setup that turns the part over in the plane of the pins.",
    ));
    for (i, setup) in session.list_setups().iter().enumerate() {
        let judged = report.judged_setup == Some(i);
        let line = format!(
            "{}: face {}, {}{}",
            setup.name,
            setup.face_up.label(),
            setup.z_rotation.label(),
            if judged { " (judged)" } else { "" }
        );
        ui.label(if judged {
            text::body_strong(line)
        } else {
            text::caption(line)
        });
    }
    ui.add_space(crate::ui::tokens::SPACE_2);
    match report.map.axis_label() {
        Some(axis) if report.judged_setup.is_some() => {
            ui.add(StatusChip::new(format!("Flip axis: {axis}"), Role::Ok));
        }
        _ => {
            Banner::new(
                Role::Danger,
                "No setup turns the part over in the plane of the pins, so the pins have no \
                 flip to key.",
            )
            .show(ui);
            if ui
                .add(Button::primary("Add a Bottom setup"))
                .on_hover_text(
                    "Adds a setup with face Bottom at 0 deg now. Cancel does not remove it.",
                )
                .clicked()
            {
                events.push(AppEvent::PinAddSetup(FaceUp::Bottom));
            }
        }
    }
    let has_top = session
        .list_setups()
        .iter()
        .any(|s| s.face_up == FaceUp::Top && s.z_rotation == crate::state::job::ZRotation::Deg0);
    if !has_top {
        ui.add_space(crate::ui::tokens::SPACE_2);
        Banner::new(
            Role::Caution,
            "No setup has face Top at 0 deg. The pin drill drills from the top of the stock, \
             so it needs one.",
        )
        .show(ui);
        if ui
            .add(Button::new("Add a Top setup"))
            .on_hover_text("Adds a setup with face Top at 0 deg now. Cancel does not remove it.")
            .clicked()
        {
            events.push(AppEvent::PinAddSetup(FaceUp::Top));
        }
    }
}

/// A row of quick size picks. A click writes the size into `value`.
fn size_picks(ui: &mut egui::Ui, label: &str, sizes: &[f64], value: &mut f64) -> bool {
    if sizes.is_empty() {
        return false;
    }
    let mut picked = false;
    ui.horizontal_wrapped(|ui| {
        ui.label(text::caption(label));
        for d in sizes {
            if ui.add(Button::quiet(format!("{d:.2} mm"))).clicked() {
                *value = *d;
                picked = true;
            }
        }
    });
    picked
}

/// Step 2: the dowel.
fn step_dowel(ui: &mut egui::Ui, session: &ProjectSession, wizard: &mut PinWizardState) {
    let mut changed = false;
    let before = wizard.pattern;
    ui.add(
        ChoiceRow::new("Pattern", &mut wizard.pattern, &WizardPattern::OPTIONS).hover(
            "3 pins, one size: one pin on the flip axis and a mirror pair off it; one drill. \
             2 pins, two sizes: two pins on the flip axis of different diameters; two drills.",
        ),
    );
    changed |= wizard.pattern != before;

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
    changed |= size_picks(ui, "Sizes of the project:", &known, &mut wizard.small_d);
    if wizard.pattern == WizardPattern::Mixed {
        let min_large = wizard.small_d + edit::MIN_DIAMETER_STEP_MM;
        if wizard.large_d < min_large {
            wizard.large_d = mixed_start_large_diameter(wizard.small_d);
        }
        ui.param_grid("pin_wizard_dowel_large", |ui| {
            let out = ValueRow::new(
                "Large dowel",
                &mut wizard.large_d,
                " mm",
                0.1,
                min_large..=25.0,
            )
            .show(ui);
            edit_out.drag(&out.value_response);
        });
        let larger: Vec<f64> = known.iter().copied().filter(|d| *d > min_large).collect();
        changed |= size_picks(
            ui,
            "Larger sizes of the project:",
            &larger,
            &mut wizard.large_d,
        );
        ui.label(text::caption(format!(
            "With no larger drill, the start value is the small dowel plus twice the hole slop \
             ({:.2} mm). Set it to the dowel you have.",
            mixed_start_large_diameter(wizard.small_d) - wizard.small_d
        )));
    }
    changed |= edit_out.committed;
    if changed {
        wizard.place(session);
    }
    if let Some(reason) = &wizard.place_refusal {
        ui.label(
            egui::RichText::new(reason)
                .small()
                .color(crate::ui::tokens::DANGER),
        );
    }

    // The drill of each size: the project's tool, a queued add, or the
    // button that queues one.
    ui.add_space(crate::ui::tokens::SPACE_2);
    ui.label(text::subhead("Drills"));
    let sizes = pin_diameters(&wizard.pins);
    for d in sizes {
        ui.push_id(("pin_wizard_drill", d.to_bits()), |ui| {
            let tools = session.pin_tool_candidates(d);
            let queued = wizard
                .add_drills
                .iter()
                .position(|a| same_pin_diameter(*a, d));
            if let Some(&tool_id) = tools.first() {
                let name = session
                    .tools()
                    .iter()
                    .find(|t| t.id.0 == tool_id)
                    .map_or_else(String::new, |t| t.name.clone());
                ui.add(StatusChip::new(format!("{d:.2} mm: {name}"), Role::Ok));
            } else if let Some(at) = queued {
                ui.add(StatusChip::new(
                    format!(
                        "{d:.2} mm: Apply adds '{}'",
                        session.pin_drill_tool_template(d).name
                    ),
                    Role::Info,
                ));
                if ui.add(Button::quiet("Do not add it")).clicked() {
                    wizard.add_drills.remove(at);
                }
            } else {
                ui.add(StatusChip::new(
                    format!("{d:.2} mm: no drill"),
                    Role::Danger,
                ));
                if ui
                    .add(Button::primary(format!("Add a {d:.2} mm drill")))
                    .on_hover_text(
                        "Apply adds a drill of this size: a copy of the project's drill with \
                         the new diameter.",
                    )
                    .clicked()
                {
                    wizard.add_drills.push(d);
                }
            }
        });
    }
    // A queued drill of a size the draft no longer has is dropped.
    let live = pin_diameters(&wizard.pins);
    wizard
        .add_drills
        .retain(|a| live.iter().any(|d| same_pin_diameter(*a, *d)));
}

/// Step 3: the placement.
fn step_placement(
    ui: &mut egui::Ui,
    session: &ProjectSession,
    report: &PinSetReport,
    wizard: &mut PinWizardState,
) {
    let stock = session.stock_config();
    for chip in chip_words(report) {
        ui.add(StatusChip::new(chip.word, chip.role));
    }
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
            size: plan_view::PlanSize::Large {
                max_height: PLAN_MAX_HEIGHT,
            },
        },
        &mut edit_out,
    );
    if let Some(selected) = view.selected_pin {
        wizard.selected = selected;
    }
    ui.horizontal_wrapped(|ui| {
        if ui
            .add(Button::new("Auto-place"))
            .on_hover_text(
                "Places the pattern of step 2 in the clear material, with a wall to the edge, \
                 the part, the keep-outs and the fixtures.",
            )
            .clicked()
        {
            wizard.place(session);
        }
        if ui
            .add(Button::new("Snap to flip"))
            .on_hover_text("Moves the pins the least distance that makes the set symmetric.")
            .clicked()
        {
            wizard.pins = edit::snap(&wizard.pins, &report.map);
        }
    });
    if let Some(reason) = &wizard.place_refusal {
        ui.label(
            egui::RichText::new(reason)
                .small()
                .color(crate::ui::tokens::DANGER),
        );
    }
    for (axis, along_x) in [("X", true), ("Y", false)] {
        ui.horizontal_wrapped(|ui| {
            ui.label(text::caption(format!("Nudge {axis}")));
            for step in NUDGE_STEPS_MM {
                if ui.add(Button::quiet(format!("{step:+}"))).clicked() {
                    let (dx, dy) = if along_x { (step, 0.0) } else { (0.0, step) };
                    wizard.pins = edit::move_all(&wizard.pins, dx, dy);
                }
            }
        });
    }
    // The along and across values of the preset the draft fits.
    let length = edit::axis_length(&report.map);
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
                .show(ui);
                ValueRow::new("Pair along", &mut p.pair_along, " mm", 0.5, 0.0..=length).show(ui);
                ValueRow::new(
                    "Pair off axis",
                    &mut p.pair_across,
                    " mm",
                    0.5,
                    edit::MIN_PAIR_ACROSS_MM..=length.max(edit::MIN_PAIR_ACROSS_MM),
                )
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
                .show(ui);
                ValueRow::new(
                    "Large pin along",
                    &mut p.large_along,
                    " mm",
                    0.5,
                    0.0..=length,
                )
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
    walls_caption(ui, report);
}

/// One caption with the thinnest walls of the report.
fn walls_caption(ui: &mut egui::Ui, report: &PinSetReport) {
    let part = |v: Option<f64>| v.map_or_else(|| "\u{2014}".to_owned(), |w| format!("{w:.1}"));
    ui.label(text::caption(format!(
        "Thinnest walls, mm: stock edge {}, part {}, keep-out or fixture {}.",
        part(report.wall_to_stock_edge_mm()),
        part(report.wall_to_part_mm()),
        part(report.wall_to_keep_out_mm()),
    )));
}

/// The lines of the summary: what Apply does. A pure function, so a test
/// reads the lines the wizard paints.
#[must_use]
pub fn summary_lines(session: &ProjectSession, wizard: &PinWizardState) -> Vec<(String, Role)> {
    let mut out = Vec::new();
    for d in &wizard.add_drills {
        out.push((
            format!(
                "Add the tool '{}'.",
                session.pin_drill_tool_template(*d).name
            ),
            Role::Info,
        ));
    }
    out.push((
        format!(
            "Write {} pins in one step (one undo step).",
            wizard.pins.len()
        ),
        Role::Info,
    ));
    let has_top = session
        .list_setups()
        .iter()
        .any(|s| s.face_up == FaceUp::Top && s.z_rotation == crate::state::job::ZRotation::Deg0);
    let sizes = pin_diameters(&wizard.pins);
    for d in &sizes {
        let count = wizard
            .pins
            .iter()
            .filter(|p| same_pin_diameter(p.diameter, *d))
            .count();
        let op = session
            .pin_drill_op_for(*d)
            .and_then(|i| session.toolpath_configs().get(i));
        let queued = wizard.add_drills.iter().any(|a| same_pin_diameter(*a, *d));
        let line = match op {
            Some(tc) => (
                format!("'{}' drills the {count} hole(s) of {d:.2} mm.", tc.name),
                Role::Ok,
            ),
            None if !has_top => (
                format!("The {d:.2} mm drill is refused: no setup has face Top at 0 deg."),
                Role::Danger,
            ),
            None if queued || session.pin_drill_tool_for(*d).is_some() => (
                format!("Create the pin-drill operation of {d:.2} mm ({count} hole(s))."),
                Role::Ok,
            ),
            None => (
                format!("The {d:.2} mm drill is refused: no tool of {d:.2} mm."),
                Role::Danger,
            ),
        };
        out.push(line);
    }
    for (index, d) in session.pin_drill_ops() {
        if !sizes.iter().any(|s| same_pin_diameter(*s, d))
            && let Some(tc) = session.toolpath_configs().get(index)
        {
            out.push((
                format!("Remove '{}': no pin is {d:.2} mm.", tc.name),
                Role::Caution,
            ));
        }
    }
    out
}

/// Step 4: the summary.
fn step_summary(
    ui: &mut egui::Ui,
    session: &ProjectSession,
    report: &PinSetReport,
    wizard: &PinWizardState,
) {
    for chip in chip_words(report) {
        ui.add(StatusChip::new(chip.word, chip.role));
    }
    walls_caption(ui, report);
    ui.add_space(crate::ui::tokens::SPACE_2);
    ui.label(text::subhead("Apply does this"));
    for (line, role) in summary_lines(session, wizard) {
        ui.add(StatusChip::new(line, role));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn the_steps_go_forward_and_back() {
        assert_eq!(WizardStep::Flip.next(), WizardStep::Dowel);
        assert_eq!(WizardStep::Summary.next(), WizardStep::Summary);
        assert_eq!(WizardStep::Flip.back(), WizardStep::Flip);
        assert_eq!(WizardStep::Summary.back(), WizardStep::Placement);
    }
}
