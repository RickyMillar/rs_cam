//! The words of the pin state (round 2): the stock panel's one status
//! line and its one warning line, and the wizard's chips beside Apply.
//! Pure functions of the one core report (`PinSetReport`) and the drill
//! state, so a test reads the words the GUI paints.

use rs_cam_core::compute::alignment_pins::PIN_WALL_MM;
use rs_cam_core::session::PinSetReport;

use super::{PinDrillBlock, PinDrillState};
use crate::ui::components::Role;

/// One chip: its words and its role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chip {
    pub word: String,
    pub role: Role,
}

/// The wizard's chips beside Apply, in order: seating and keying, then the
/// walls.
#[must_use]
pub fn chip_words(report: &PinSetReport) -> Vec<Chip> {
    let flip = &report.flip;
    let mut chips = Vec::new();
    chips.push(if !flip.modelled_flip() {
        Chip {
            word: "No flip".to_owned(),
            role: Role::Unknown,
        }
    } else if !flip.seats() {
        Chip {
            word: format!(
                "Misses by {:.1} mm",
                flip.seat_mismatch_mm.unwrap_or(f64::INFINITY)
            ),
            role: Role::Danger,
        }
    } else if flip.keyed() {
        Chip {
            word: match flip.keying_margin_mm() {
                Some(margin) => format!("Keyed, margin {margin:.1} mm"),
                None => "Keyed".to_owned(),
            },
            role: Role::Ok,
        }
    } else {
        Chip {
            word: format!("Not keyed: seats turned {:.0} deg", turned_deg(report)),
            role: Role::Danger,
        }
    });
    if let Some(min) = report.min_wall_mm() {
        let role = if min < 0.0 {
            Role::Danger
        } else if min < PIN_WALL_MM {
            Role::Caution
        } else {
            Role::Ok
        };
        chips.push(Chip {
            word: format!("Wall {min:.1} mm"),
            role,
        });
    }
    chips
}

/// The turn of the closest wrong seating, in degrees.
fn turned_deg(report: &PinSetReport) -> f64 {
    report
        .flip
        .wrong_seating
        .map_or(180.0, |w| w.turn_deg.abs())
}

/// What the stock panel's pin block says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinSummary {
    /// The one status line: "3 pins · keyed", "No pins".
    pub status: String,
    /// At most one blocking problem, in the danger colour.
    pub warning: Option<String>,
    /// `true` when the stock has pins: the button reads "Edit pins…".
    pub has_pins: bool,
}

/// The pin block's words for `pin_count` pins with the report `report`
/// and the drill state `drills`.
///
/// The warning is the first blocking problem of: no flip, no seat, not
/// keyed, no pin drill, a thin wall.
#[must_use]
pub fn summary(report: &PinSetReport, drills: &PinDrillState, pin_count: usize) -> PinSummary {
    if pin_count == 0 {
        return PinSummary {
            status: "No pins".to_owned(),
            warning: None,
            has_pins: false,
        };
    }
    let flip = &report.flip;
    let state = if !flip.modelled_flip() {
        "no flip"
    } else if !flip.seats() {
        "do not seat"
    } else if flip.keyed() {
        "keyed"
    } else {
        "do not key"
    };
    let noun = if pin_count == 1 { "pin" } else { "pins" };
    let drill_block = drills.rows().iter().find_map(|row| match &row.block {
        Some(PinDrillBlock::NeedsTool) => {
            Some(format!("No drill for the {:.2} mm pins.", row.diameter))
        }
        Some(PinDrillBlock::NeedsTopSetup) => Some("No Top setup for the pin drill.".to_owned()),
        None => None,
    });
    let thin_wall = report
        .min_wall_mm()
        .filter(|w| *w < PIN_WALL_MM)
        .map(|w| format!("Thin wall: {w:.1} mm (need {PIN_WALL_MM:.1})."));
    let warning = if !flip.modelled_flip() {
        Some("No setup turns the part over.".to_owned())
    } else if !flip.seats() {
        Some(format!(
            "Does not seat: misses by {:.1} mm.",
            flip.seat_mismatch_mm.unwrap_or(f64::INFINITY)
        ))
    } else if !flip.keyed() {
        Some(format!(
            "Not keyed: the part also seats turned {:.0} deg.",
            turned_deg(report)
        ))
    } else {
        drill_block.or(thin_wall)
    };
    PinSummary {
        status: format!("{pin_count} {noun} \u{00B7} {state}"),
        warning,
        has_pins: true,
    }
}
