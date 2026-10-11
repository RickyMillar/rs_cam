//! The stock panel's "Alignment pins" section (P7, 2026-10-12).
//!
//! The operator edits the pins AS A SET: a preset picker, the pattern
//! dimensions, "Move all", "Snap to flip" and one diameter. The single-pin
//! drags sit in a closed "Edit single pins" fold. Status chips, the table
//! and the keying warning read the ONE core report,
//! `ProjectSession::pin_set_report_for`, of the draft pins, so they follow
//! a drag before it commits.
//!
//! Every edit writes the stock draft. The inspector applies a finished
//! edit as one `SetAlignmentPins` (or `SetPinPartModels`) through
//! `panel_apply::apply_stock_draft`, which raises the pin-drill
//! synchronisation of the one pin door (`controller/events/pins.rs`).

mod controls;
pub mod edit;
mod status;

use rs_cam_core::compute::alignment_pins::{NOMINAL_DOWEL_MM, PinObstacle, PinPlacementError};
use rs_cam_core::session::PinSetReport;

use super::PanelEdit;
use crate::state::job::{AlignmentPin, FaceUp, StockConfig, ZRotation};
use crate::ui::AppEvent;

/// What the pin-drill operation looks like for the stock panel's chip.
#[derive(Debug, Clone, PartialEq)]
pub enum PinDrillState {
    /// The stock has no pins, so no pin drill is needed.
    NoPins,
    /// The pin-drill operation exists and drills `holes` holes.
    Exists { name: String, holes: usize },
    /// No pin-drill operation exists, and the door can create one.
    Missing,
    /// No pin-drill operation exists, and the door refuses with this text
    /// (`controller::pin_drill_blocker`).
    Refused(&'static str),
}

impl PinDrillState {
    /// The state of `session`.
    #[must_use]
    pub fn of(session: &rs_cam_core::session::ProjectSession) -> Self {
        use crate::state::toolpath::OperationConfig;
        if session.stock_config().alignment_pins.is_empty() {
            return Self::NoPins;
        }
        let op = session
            .toolpath_configs()
            .iter()
            .find_map(|tc| match &tc.operation {
                OperationConfig::AlignmentPinDrill(c) => Some((tc.name.clone(), c.holes.len())),
                _ => None,
            });
        match op {
            Some((name, holes)) => Self::Exists { name, holes },
            None => {
                crate::controller::pin_drill_blocker(session).map_or(Self::Missing, Self::Refused)
            }
        }
    }
}

/// One model row of the part-model chooser.
#[derive(Debug, Clone, PartialEq)]
pub struct PartModelRow {
    pub id: usize,
    pub name: String,
    /// The model counts as the part now.
    pub is_part: bool,
    /// The default rule counts the model as the part.
    pub default_part: bool,
}

/// Everything the section reads besides the draft.
pub struct PinSectionInputs<'a> {
    /// The in-plane map of each setup, in setup order.
    pub setups: &'a [(FaceUp, ZRotation)],
    /// The placer's obstacles and pin tool.
    pub keyed: &'a KeyedPinInputs,
    /// The one report of the DRAFT pins.
    pub report: &'a PinSetReport,
    /// The part-model chooser rows.
    pub models: &'a [PartModelRow],
    /// The diameters of the tools of the drilling operations, ascending.
    pub drill_diameters: &'a [f64],
    pub pin_drill: &'a PinDrillState,
    /// The pin the viewport marked.
    pub selected_pin: Option<usize>,
    /// The refusal of the last preset click, until the next finished edit.
    pub preset_refusal: Option<&'a str>,
}

/// What the section asks of the caller besides the draft edit.
#[derive(Debug, Default)]
pub struct PinSectionOut {
    pub edit: PanelEdit,
    /// The new marked pin, when the section changed it.
    pub selected_pin: Option<Option<usize>>,
    /// "Create the pin drill": run the pin door's synchronisation.
    pub sync_pin_drill: bool,
    /// The new preset refusal: `Some(Some(text))` sets it, `Some(None)`
    /// clears it.
    pub preset_refusal: Option<Option<String>>,
}

/// Draw the "Alignment pins" section.
pub fn draw(
    ui: &mut egui::Ui,
    stock: &mut StockConfig,
    inputs: &PinSectionInputs<'_>,
    plan_memo: &mut crate::state::panels::KeyedPinPlanMemo,
    events: &mut Vec<AppEvent>,
) -> PinSectionOut {
    let report = inputs.report;
    let map = report.map.clone();
    let has_flip = report.judged_setup.is_some();
    let header = egui::RichText::new("Alignment pins")
        .strong()
        .color(crate::ui::tokens::TEXT_STRONG);

    egui::CollapsingHeader::new(header)
        .id_salt("alignment_pins_section")
        .default_open(!stock.alignment_pins.is_empty() || has_flip)
        .show(ui, |ui| {
            let mut out = PinSectionOut::default();

            if !has_flip && stock.alignment_pins.is_empty() {
                if ui
                    .add(crate::ui::components::Button::new("Two-sided setup"))
                    .on_hover_text(
                        "Adds a flipped Setup 2 and places three keyed pins in the clear \
                         material.",
                    )
                    .clicked()
                {
                    events.push(AppEvent::SetupTwoSided);
                }
                ui.add_space(crate::ui::tokens::SPACE_2);
            }

            // The flip axis comes from the setups (P3).
            ui.label(crate::ui::components::text::caption(format!(
                "Flip axis: {}",
                map.axis_label()
                    .unwrap_or_else(|| "none (no flipped Bottom setup)".to_owned())
            )));

            // The plan of the placer, cached on its inputs (P6).
            let (plan_face, plan_turn) = (map.face_up(), map.z_rotation());
            let fingerprint = keyed_pin_plan_fingerprint(stock, plan_face, plan_turn, inputs.keyed);
            let plan = plan_memo.get_or_plan(fingerprint, || {
                keyed_pin_plan(stock, plan_face, plan_turn, inputs.keyed)
            });

            if !stock.alignment_pins.is_empty() {
                status::chips(ui, report, inputs.pin_drill, &mut out, events);
                status::keying_warning(ui, stock, report, &plan, &mut out.edit);
            }

            ui.add_space(crate::ui::tokens::SPACE_2);
            controls::preset_picker(ui, stock, inputs, &plan, &mut out);
            controls::pattern_controls(ui, stock, report, &mut out.edit);
            controls::set_controls(ui, stock, report, &mut out.edit);
            controls::auto_place(ui, stock, has_flip, &plan, &mut out.edit);
            controls::part_models(ui, stock, inputs.models, &mut out.edit);

            if !stock.alignment_pins.is_empty() {
                status::pin_table(ui, stock, report, inputs.selected_pin, &mut out);
            }
            controls::single_pins(ui, stock, inputs.keyed, &mut out.edit);
            status::findings(ui, report);
            if out.edit.committed && inputs.preset_refusal.is_some() {
                out.preset_refusal = Some(None);
            }
            out
        })
        .body_returned
        .unwrap_or_default()
}

/// The session values the keyed-pin planner reads.
///
/// ONE constructor for both doors: the stock panel's "Auto-place" and the
/// controller's "Two-sided setup". Before G-PINPANEL the panel took its
/// clear strips from `padding` and a pin diameter from the stored pins. A
/// hand-sized stock (380 mm wide, origin x -15, model x 0..350, padding 5)
/// then read a 5 mm strip where the real strip is 15 mm, and the panel
/// refused a pair that the other door placed.
#[derive(Debug, Clone, Default)]
pub struct KeyedPinInputs {
    /// The world-frame outlines a pin keeps a wall to: the part models, the
    /// keep-out zones and the fixtures
    /// ([`rs_cam_core::session::ProjectSession::pin_obstacles`] of
    /// [`rs_cam_core::session::ProjectSession::pin_part_models`]).
    pub obstacles: Vec<PinObstacle>,
    /// The diameter of the tool that drills the pin holes
    /// ([`rs_cam_core::session::ProjectSession::pin_drill_tool_diameter`]).
    /// `None` when no pin-drill or drill operation exists.
    pub pin_tool_diameter: Option<f64>,
}

impl KeyedPinInputs {
    #[must_use]
    pub fn from_session(session: &rs_cam_core::session::ProjectSession) -> Self {
        Self {
            obstacles: session.pin_obstacles(&session.pin_part_models()),
            pin_tool_diameter: session.pin_drill_tool_diameter(),
        }
    }

    /// The diameter the placer gives the pins: the tool that drills the
    /// holes, else the first stored pin, else [`NOMINAL_DOWEL_MM`]. A
    /// stored pin that differs from the tool is a finding
    /// (`pin_tool_finding`), not a silent change.
    #[must_use]
    pub fn plan_pin_diameter(&self, stock: &StockConfig) -> f64 {
        self.pin_tool_diameter
            .or_else(|| stock.alignment_pins.first().map(|p| p.diameter))
            .unwrap_or(NOMINAL_DOWEL_MM)
    }
}

/// The keyed pins for `stock` under the flipped setup (`face_up` and
/// `z_rotation`).
///
/// Both doors call this function with [`KeyedPinInputs::from_session`], so
/// they cannot disagree about a placement. The clear area comes from the
/// real stock extent and the obstacles through
/// [`StockConfig::plan_keyed_pins`], never from `padding`.
pub fn keyed_pin_plan(
    stock: &StockConfig,
    face_up: FaceUp,
    z_rotation: ZRotation,
    inputs: &KeyedPinInputs,
) -> Result<[AlignmentPin; 3], PinPlacementError> {
    stock.plan_keyed_pins(
        face_up,
        z_rotation,
        &inputs.obstacles,
        inputs.plan_pin_diameter(stock),
    )
}

/// A hash of every input of [`keyed_pin_plan`]: the stock extent and
/// origin, the first stored pin's diameter, the setup map, the pin tool and
/// every obstacle point. The stock panel's memo
/// ([`crate::state::panels::KeyedPinPlanMemo`]) keys on it.
fn keyed_pin_plan_fingerprint(
    stock: &StockConfig,
    face_up: FaceUp,
    z_rotation: ZRotation,
    inputs: &KeyedPinInputs,
) -> u64 {
    use std::hash::{Hash as _, Hasher as _};

    let mut h = std::collections::hash_map::DefaultHasher::new();
    for v in [stock.x, stock.y, stock.origin_x, stock.origin_y] {
        v.to_bits().hash(&mut h);
    }
    // The first stored pin sets the diameter when no tool drills the holes.
    stock
        .alignment_pins
        .first()
        .map(|p| p.diameter.to_bits())
        .hash(&mut h);
    face_up.hash(&mut h);
    z_rotation.hash(&mut h);
    inputs.pin_tool_diameter.map(f64::to_bits).hash(&mut h);
    for o in &inputs.obstacles {
        o.kind.hash(&mut h);
        o.name.hash(&mut h);
        o.closed.hash(&mut h);
        for (x, y) in &o.points {
            x.to_bits().hash(&mut h);
            y.to_bits().hash(&mut h);
        }
    }
    h.finish()
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use rs_cam_core::compute::alignment_pins::PinObstacleKind;

    fn stock(x: f64, y: f64) -> StockConfig {
        StockConfig {
            x,
            y,
            ..StockConfig::default()
        }
    }

    /// The pin-drill inputs for a model at `x_min..x_max` and a tool of
    /// `pin_diameter`.
    fn inputs(x_min: f64, x_max: f64, y: f64, pin_diameter: f64) -> KeyedPinInputs {
        KeyedPinInputs {
            obstacles: vec![PinObstacle::rectangle(
                PinObstacleKind::Part,
                "model",
                (x_min, 0.0),
                (x_max, y),
            )],
            pin_tool_diameter: Some(pin_diameter),
        }
    }

    /// The live wanaka geometry. A 5 mm clear ring cannot hold a 6 mm
    /// dowel at ANY offset — 6 mm of pin plus 2 mm of wall each side
    /// needs 10 mm — so the only correct answer is a refusal. The
    /// pre-2026-08-22 controller placed the pin at x = 2.5, spanning
    /// -0.5..5.5 relative to the stock edge: hanging off the blank.
    #[test]
    fn panel_refuses_o6_pin_in_a_5mm_clear_ring() {
        let s = stock(140.0, 150.0);
        match keyed_pin_plan(
            &s,
            FaceUp::Bottom,
            ZRotation::Deg0,
            &inputs(5.0, 135.0, 150.0, 6.0),
        ) {
            Err(PinPlacementError::NoRoomOnAxis { need_mm, .. }) => {
                assert!((need_mm - 10.0).abs() < 1e-9);
            }
            other => panic!("expected a strip-too-narrow refusal, got {other:?}"),
        }
    }

    /// With room to work in, the panel places one pin on the mirror line
    /// and a mirror pair off it. The pattern seats after the flip and, by
    /// the intrinsic check, seats in no turned position (P1).
    #[test]
    fn panel_plan_seats_the_flip_and_keys_it() {
        let s = stock(140.0, 150.0);
        let pins = keyed_pin_plan(
            &s,
            FaceUp::Bottom,
            ZRotation::Deg0,
            &inputs(20.0, 120.0, 150.0, 6.0),
        )
        .unwrap();
        assert!(
            (pins[0].y - s.y * 0.5).abs() < 1e-9,
            "the axis pin is off the mirror line at y={}",
            pins[0].y
        );
        assert!(
            (pins[1].y + pins[2].y - s.y).abs() < 1e-9 && (pins[1].x - pins[2].x).abs() < 1e-9,
            "pins 2 and 3 are not a mirror pair: {pins:?}"
        );
        let mut placed = s;
        placed.alignment_pins = pins.to_vec();
        let report = placed.validate_pins_for_flip(FaceUp::Bottom, ZRotation::Deg0);
        assert!(report.seats(), "{report:?}");
        assert!(report.keyed(), "{report:?}");
        assert!(report.out_of_bounds.is_empty(), "{report:?}");
    }

    /// No flipped setup means no flip to key, and the panel says so
    /// rather than inventing a pattern.
    #[test]
    fn panel_refuses_when_no_flip_is_programmed() {
        let s = stock(140.0, 150.0);
        assert!(matches!(
            keyed_pin_plan(
                &s,
                FaceUp::Top,
                ZRotation::Deg0,
                &inputs(20.0, 120.0, 150.0, 6.0)
            ),
            Err(PinPlacementError::UnsupportedFlip { .. })
        ));
    }

    /// No tool drills the holes: the placer keeps the stored pin size, and
    /// with no stored pin it uses the stated nominal dowel. It does not
    /// refuse, and it never takes a tool that does not drill the holes.
    #[test]
    fn panel_without_a_pin_tool_keeps_the_stored_pin_size() {
        let no_tool = KeyedPinInputs {
            pin_tool_diameter: None,
            ..inputs(20.0, 120.0, 150.0, 6.0)
        };
        let mut s = stock(140.0, 150.0);
        assert!((no_tool.plan_pin_diameter(&s) - NOMINAL_DOWEL_MM).abs() < 1e-12);
        s.alignment_pins = vec![AlignmentPin::new(10.0, 75.0, 8.0)];
        assert!((no_tool.plan_pin_diameter(&s) - 8.0).abs() < 1e-12);
        let pins = keyed_pin_plan(&s, FaceUp::Bottom, ZRotation::Deg0, &no_tool).unwrap();
        assert!(
            pins.iter().all(|p| (p.diameter - 8.0).abs() < 1e-12),
            "{pins:?}"
        );
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod command_tests {
    use rs_cam_core::compute::alignment_pins::PinSetupMap;
    use rs_cam_core::session::Command;

    use super::edit;
    use crate::state::job::{AlignmentPin, FaceUp, StockConfig, ZRotation};

    fn stored() -> StockConfig {
        StockConfig {
            x: 65.0,
            y: 65.0,
            alignment_pins: vec![
                AlignmentPin::new(59.0, 32.5, 6.0),
                AlignmentPin::new(6.0, 32.5, 6.0),
            ],
            ..StockConfig::default()
        }
    }

    /// A pattern edit of the draft reaches the session as ONE
    /// `SetAlignmentPins` with the edited set, not a `SetStockConfig`
    /// that drops every result.
    #[test]
    fn a_pattern_edit_is_one_set_alignment_pins() {
        let stored = stored();
        let map = PinSetupMap::new(FaceUp::Bottom, ZRotation::Deg0, 65.0, 65.0);
        for pins in [
            edit::pair_off_axis(&stored.alignment_pins, &map, 26.5).unwrap(),
            edit::move_all(&stored.alignment_pins, 0.5, 0.0),
            edit::snap(&stored.alignment_pins, &map),
        ] {
            let mut draft = stored.clone();
            draft.alignment_pins = pins.clone();
            match crate::ui::properties::stock_edit_command(&stored, draft) {
                Some(Command::SetAlignmentPins(args)) => assert_eq!(args.pins, pins),
                None if pins == stored.alignment_pins => {}
                other => panic!("expected SetAlignmentPins, got {other:?}"),
            }
        }
    }

    /// A part-model choice alone is `SetPinPartModels`, which stales
    /// nothing.
    #[test]
    fn a_part_model_choice_is_its_own_row() {
        let stored = stored();
        let mut draft = stored.clone();
        draft.pin_part_models = Some(vec![1]);
        match crate::ui::properties::stock_edit_command(&stored, draft) {
            Some(Command::SetPinPartModels(args)) => assert_eq!(args.models, Some(vec![1])),
            other => panic!("expected SetPinPartModels, got {other:?}"),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod split_tests {
    use rs_cam_core::session::Command;

    use crate::state::job::{AlignmentPin, StockConfig};

    /// A draft that moves the pins AND the part-model choice takes the
    /// choice row first; the rest is then a pin-only edit. Neither half
    /// is a `SetStockConfig` that drops every result.
    #[test]
    fn a_two_field_draft_splits_into_two_narrow_rows() {
        let stored = StockConfig {
            alignment_pins: vec![AlignmentPin::new(1.0, 2.0, 6.0)],
            ..StockConfig::default()
        };
        let mut draft = stored.clone();
        draft.alignment_pins = vec![AlignmentPin::new(3.0, 2.0, 6.0)];
        draft.pin_part_models = Some(vec![0]);

        let first = crate::ui::properties::part_models_first(&stored, &draft);
        assert!(
            matches!(first, Some(Command::SetPinPartModels(_))),
            "{first:?}"
        );
        let mut after_first = stored.clone();
        after_first.pin_part_models = Some(vec![0]);
        assert!(matches!(
            crate::ui::properties::stock_edit_command(&after_first, draft.clone()),
            Some(Command::SetAlignmentPins(_))
        ));
        // A draft that also changes a dimension is a stock edit; no split.
        let mut wider = draft;
        wider.x += 1.0;
        assert!(crate::ui::properties::part_models_first(&stored, &wider).is_none());
    }
}
