//! The alignment pins (P7; round 2, 2026-10-12).
//!
//! The stock panel shows ONE compact block ([`draw`]): a status line, at
//! most one warning line and one button that opens the pin wizard
//! (`ui/pin_wizard.rs`). The wizard edits the pins; this folder holds its
//! parts: the pure set edits (`edit.rs`), the drill rows (`drills.rs`),
//! the plan view (`plan_view.rs`), the flip diagram (`flip_view.rs`) and
//! the words of the pin state (`status.rs`).
//!
//! Every number comes from the ONE core report,
//! `ProjectSession::pin_set_report_for`. Every edit reaches the session
//! through the one pin door (`controller/events/pins.rs`).

pub mod drills;
pub mod edit;
pub mod flip_view;
pub mod plan_view;
mod status;

pub use status::{Chip, PinSummary, chip_words, summary};

use rs_cam_core::compute::alignment_pins::{NOMINAL_DOWEL_MM, PinObstacle, PinPlacementError};

use crate::state::job::{AlignmentPin, FaceUp, StockConfig, ZRotation};
use crate::ui::components::{Button, text};

/// Why the door cannot create the pin-drill operation of one pin size
/// (`controller::pin_drill_blocker`, the door's own check).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinDrillBlock {
    /// No setup is a `Top` setup at 0 deg.
    NeedsTopSetup,
    /// No tool of the pin size is in the project.
    NeedsTool,
}

/// One pin size and the drill that makes its holes, as the pin wizard
/// shows it (R1, R2). Every value comes from the core bindings
/// (`ProjectSession::pin_tool_bindings_for`) and the door's check.
#[derive(Debug, Clone, PartialEq)]
pub struct PinDrillRow {
    /// The pin diameter of the size, in mm.
    pub diameter: f64,
    /// The tool that drills (or will drill) the holes: id and name.
    pub tool: Option<(usize, String)>,
    /// The tool diameter, when a tool exists.
    pub tool_diameter: Option<f64>,
    /// Every tool of the size: id and name (`pin_tool_candidates`).
    pub candidates: Vec<(usize, String)>,
    /// Why the door cannot create the operation, when no operation exists
    /// and the door cannot create one.
    pub block: Option<PinDrillBlock>,
}

impl PinDrillRow {
    /// `true` when the tool diameter is not the pin diameter.
    #[must_use]
    pub fn tool_mismatch(&self) -> bool {
        self.tool_diameter.is_some_and(|d| {
            !rs_cam_core::compute::alignment_pins::same_pin_diameter(d, self.diameter)
        })
    }
}

/// What the pin-drill operations look like for the pin block and the wizard.
#[derive(Debug, Clone, PartialEq)]
pub enum PinDrillState {
    /// The pin set is empty, so no pin drill is needed.
    NoPins,
    /// One row for each pin size, in ascending order of diameter.
    Sizes(Vec<PinDrillRow>),
}

impl PinDrillState {
    /// The state of `pins` (a draft) on `session`.
    #[must_use]
    pub fn of(session: &rs_cam_core::session::ProjectSession, pins: &[AlignmentPin]) -> Self {
        if pins.is_empty() {
            return Self::NoPins;
        }
        let tool_name = |id: usize| {
            session
                .tools()
                .iter()
                .find(|t| t.id.0 == id)
                .map(|t| t.name.clone())
                .unwrap_or_else(|| format!("tool {id}"))
        };
        let rows = session
            .pin_tool_bindings_for(pins)
            .into_iter()
            .map(|b| {
                let block = if b.op_index.is_some() {
                    None
                } else {
                    crate::controller::pin_drill_blocker(session, b.pin_diameter).map(|blocker| {
                        match blocker {
                            crate::controller::PinDrillBlocker::NeedsTopSetup => {
                                PinDrillBlock::NeedsTopSetup
                            }
                            crate::controller::PinDrillBlocker::NeedsTool { .. } => {
                                PinDrillBlock::NeedsTool
                            }
                        }
                    })
                };
                PinDrillRow {
                    diameter: b.pin_diameter,
                    tool: b.tool_id.map(|id| (id, tool_name(id))),
                    tool_diameter: b.tool_diameter,
                    candidates: session
                        .pin_tool_candidates(b.pin_diameter)
                        .into_iter()
                        .map(|id| (id, tool_name(id)))
                        .collect(),
                    block,
                }
            })
            .collect();
        Self::Sizes(rows)
    }

    /// The rows, or none.
    #[must_use]
    pub fn rows(&self) -> &[PinDrillRow] {
        match self {
            PinDrillState::NoPins => &[],
            PinDrillState::Sizes(rows) => rows,
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

impl PartModelRow {
    /// One row for each model of `session`, in model order.
    #[must_use]
    pub fn of(session: &rs_cam_core::session::ProjectSession) -> Vec<Self> {
        let part = session.pin_part_models();
        let default_part = session.default_pin_part_models();
        session
            .models()
            .iter()
            .map(|m| Self {
                id: m.id,
                name: m.name.clone(),
                is_part: part.contains(&rs_cam_core::ids::ModelId(m.id)),
                default_part: default_part.contains(&rs_cam_core::ids::ModelId(m.id)),
            })
            .collect()
    }
}

/// Draw the stock panel's "Alignment pins" block: the status line, at
/// most one warning line, and the button that opens the wizard. Returns
/// `true` when the operator clicks the button.
pub fn draw(ui: &mut egui::Ui, summary: &PinSummary) -> bool {
    ui.label(text::body_strong("Alignment pins"));
    wrapped(ui, text::caption(&summary.status));
    if let Some(warning) = &summary.warning {
        wrapped(
            ui,
            egui::RichText::new(warning)
                .small()
                .color(crate::ui::tokens::DANGER),
        );
    }
    let label = if summary.has_pins {
        "Edit pins\u{2026}"
    } else {
        "Set up pins\u{2026}"
    };
    ui.add(Button::new(label))
        .on_hover_text("Opens the pin wizard.")
        .clicked()
}

/// A label that wraps at the panel width (R5b).
///
/// The inspector sits in a scroll area that scrolls both ways
/// (`app::side_panel`), and a plain label there does not wrap.
pub(crate) fn wrapped(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) -> egui::Response {
    ui.add(egui::Label::new(text).wrap())
}

/// The session values the keyed-pin planner reads.
///
/// ONE constructor for both doors: the pin wizard's "Auto-place" and the
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
    /// The pin diameter of a NEW set when no pin is stored: the tool of
    /// the first pin-drill or drill operation
    /// ([`rs_cam_core::session::ProjectSession::default_pin_tool_diameter`]).
    /// `None` when no pin-drill or drill operation exists.
    pub pin_tool_diameter: Option<f64>,
}

impl KeyedPinInputs {
    #[must_use]
    pub fn from_session(session: &rs_cam_core::session::ProjectSession) -> Self {
        Self {
            obstacles: session.pin_obstacles(&session.pin_part_models()),
            pin_tool_diameter: session.default_pin_tool_diameter(),
        }
    }

    /// The diameter the placer gives the pins: the smallest stored pin
    /// size (the dowel the operator has), else the tool of the first
    /// drilling operation, else [`NOMINAL_DOWEL_MM`]. Since R1 each pin
    /// size has its own drill, so the pins set the size and the drill
    /// follows. A size with no drill shows in the wizard with "Add a
    /// <d> mm drill".
    #[must_use]
    pub fn plan_pin_diameter(&self, stock: &StockConfig) -> f64 {
        rs_cam_core::compute::alignment_pins::pin_diameters(&stock.alignment_pins)
            .first()
            .copied()
            .or(self.pin_tool_diameter)
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
            edit::move_along_axis(&stored.alignment_pins, &map, 0.5),
            edit::move_along_axis(&stored.alignment_pins, &map, -2.0),
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
