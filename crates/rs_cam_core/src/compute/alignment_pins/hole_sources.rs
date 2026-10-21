//! One source of hole positions (P8, 2026-10-12).
//!
//! The stock pins give the dowel holes, and the pin-drill operation drills
//! them. A second operation can also drill holes at the same positions (a
//! drill operation on a drawing of the dowel holes, or a pocket on its
//! circles). Two copies of one fact then exist:
//!
//! - when both copies agree, the machine drills each dowel hole twice;
//! - when one copy moves and the other does not, the holes and the dowels
//!   disagree, and the part does not seat at the flip.
//!
//! This module judges the copies. It reads hole positions that the caller
//! gives in the stock-local frame (the frame of the pins). It does not read
//! an operation name.

use super::{AlignmentPin, PIN_MATCH_TOL_MM, same_pin_diameter};
use crate::selection::CAD_ROUNDING_MM;

/// One hole that an operation drills, in the stock-local frame.
#[derive(Debug, Clone, PartialEq)]
pub struct DrilledHole {
    pub x: f64,
    pub y: f64,
    /// The hole diameter when the source states it (a drawn circle).
    /// `None` for a bare point.
    pub diameter: Option<f64>,
}

/// How an operation makes its holes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoleOpKind {
    /// A drill cycle at each hole centre.
    Drill,
    /// A pocket that clears each drawn circle.
    Pocket,
}

/// The holes of one operation that is not the pin-drill operation.
#[derive(Debug, Clone, PartialEq)]
pub struct HoleOp {
    /// The toolpath id.
    pub toolpath_id: usize,
    /// The toolpath name, for the sentence only.
    pub name: String,
    pub kind: HoleOpKind,
    pub holes: Vec<DrilledHole>,
}

/// What kind of problem a [`HoleSourceFinding`] states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoleSourceKind {
    /// A pin-drill operation exists, and another operation also drills a
    /// hole at a stock pin.
    DrilledTwice,
    /// A drill operation drills holes of a pin diameter that are not at a
    /// stock pin.
    DowelHolesDisagree,
}

impl HoleSourceKind {
    /// What the operator does about it.
    #[must_use]
    pub fn fix_hint(&self) -> &'static str {
        match self {
            HoleSourceKind::DrilledTwice => {
                "Keep one source of the dowel holes. Delete or disable the other operation and \
                 let the pin-drill operation drill the holes from the stock pins, or remove the \
                 stock pins."
            }
            HoleSourceKind::DowelHolesDisagree => {
                "Move the stock pins onto the drawn holes, or change the drawing to match the \
                 pins. Better: delete the drawn dowel holes and let the pin-drill operation \
                 drill the holes from the stock pins."
            }
        }
    }
}

/// One problem with the hole sources, as a sentence, a kind and the
/// operation it names.
#[derive(Debug, Clone, PartialEq)]
pub struct HoleSourceFinding {
    pub kind: HoleSourceKind,
    pub toolpath_id: usize,
    pub text: String,
}

/// The distance from `(x, y)` to the nearest pin, in mm, over `pins`.
fn nearest_pin_mm<'a>(pins: impl IntoIterator<Item = &'a AlignmentPin>, x: f64, y: f64) -> f64 {
    pins.into_iter()
        .map(|p| (p.at.x() - x).hypot(p.at.y() - y))
        .fold(f64::INFINITY, f64::min)
}

/// Judge the hole sources of a project.
///
/// - `pins`: the stock pins, stock-local.
/// - `pin_drill_sizes`: the pin diameter of each enabled pin-drill
///   operation (R1: one operation for each pin size).
/// - `ops`: every other enabled operation that drills holes, with its
///   holes stock-local.
///
/// The rules:
///
/// 1. "Drilled twice": the operation drills a hole within
///    [`PIN_MATCH_TOL_MM`] of a stock pin whose size a pin-drill operation
///    in `pin_drill_sizes` drills.
/// 2. "Disagree": a [`HoleOpKind::Drill`] operation drills a hole whose
///    stated diameter is a pin diameter (within
///    [`CAD_ROUNDING_MM`]), and no pin of that diameter is within
///    [`PIN_MATCH_TOL_MM`] of it. A bare point has no diameter, so it is
///    not judged a dowel hole.
///
/// With no pins there is nothing to compare, and the result is empty.
#[must_use]
pub fn hole_source_findings(
    pins: &[AlignmentPin],
    pin_drill_sizes: &[f64],
    ops: &[HoleOp],
) -> Vec<HoleSourceFinding> {
    let mut out = Vec::new();
    if pins.is_empty() {
        return out;
    }
    for op in ops {
        let drilled: Vec<&AlignmentPin> = pins
            .iter()
            .filter(|p| {
                pin_drill_sizes
                    .iter()
                    .any(|d| same_pin_diameter(*d, p.diameter))
            })
            .collect();
        if !drilled.is_empty() {
            let at_pins = op
                .holes
                .iter()
                .filter(|h| nearest_pin_mm(drilled.iter().copied(), h.x, h.y) <= PIN_MATCH_TOL_MM)
                .count();
            if at_pins > 0 {
                out.push(HoleSourceFinding {
                    kind: HoleSourceKind::DrilledTwice,
                    toolpath_id: op.toolpath_id,
                    text: format!(
                        "Dowel holes are drilled twice: TP{id} '{name}' drills {at_pins} \
                         hole(s) at the stock pins, and the pin-drill operation drills the \
                         same holes.",
                        id = op.toolpath_id,
                        name = op.name,
                    ),
                });
            }
        }
        if op.kind != HoleOpKind::Drill {
            continue;
        }
        // (hole, distance to the nearest pin of its diameter)
        let mut misses: Vec<(&DrilledHole, f64)> = Vec::new();
        for hole in &op.holes {
            let Some(diameter) = hole.diameter else {
                continue;
            };
            let same_size: Vec<&AlignmentPin> = pins
                .iter()
                .filter(|p| (p.diameter - diameter).abs() <= CAD_ROUNDING_MM)
                .collect();
            if same_size.is_empty() {
                continue;
            }
            let miss = nearest_pin_mm(same_size, hole.x, hole.y);
            if miss > PIN_MATCH_TOL_MM {
                misses.push((hole, miss));
            }
        }
        let worst = misses
            .iter()
            .copied()
            .fold(None, |best: Option<(&DrilledHole, f64)>, m| match best {
                Some(b) if b.1 >= m.1 => Some(b),
                _ => Some(m),
            });
        if let Some((hole, miss)) = worst {
            out.push(HoleSourceFinding {
                kind: HoleSourceKind::DowelHolesDisagree,
                toolpath_id: op.toolpath_id,
                text: format!(
                    "Dowel holes disagree with the stock pins: TP{id} '{name}' drills {count} \
                     hole(s) of a pin diameter that are not at a stock pin. The worst is at \
                     ({x:.1}, {y:.1}) mm, {miss:.1} mm from the nearest pin of its size. The \
                     part does not seat on the dowels at the flip.",
                    id = op.toolpath_id,
                    name = op.name,
                    count = misses.len(),
                    x = hole.x,
                    y = hole.y,
                ),
            });
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn pins() -> Vec<AlignmentPin> {
        vec![
            AlignmentPin::new(59.0, 32.5, 6.0),
            AlignmentPin::new(6.0, 32.5, 6.0),
        ]
    }

    fn drill(holes: &[(f64, f64, Option<f64>)]) -> HoleOp {
        HoleOp {
            toolpath_id: 4,
            name: "Dowel holes".to_owned(),
            kind: HoleOpKind::Drill,
            holes: holes
                .iter()
                .map(|&(x, y, diameter)| DrilledHole { x, y, diameter })
                .collect(),
        }
    }

    #[test]
    fn a_drill_on_the_pins_is_drilled_twice_only_with_a_pin_drill() {
        let op = drill(&[(6.0, 32.5, Some(6.0)), (59.0, 32.5, Some(6.0))]);
        assert!(hole_source_findings(&pins(), &[], std::slice::from_ref(&op)).is_empty());
        let found = hole_source_findings(&pins(), &[6.0], &[op]);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].kind, HoleSourceKind::DrilledTwice);
        assert!(found[0].text.contains("2 hole(s)"), "{}", found[0].text);
    }

    #[test]
    fn a_pocket_on_the_pins_is_drilled_twice_but_never_disagrees() {
        let mut op = drill(&[(6.0, 32.5, Some(6.0)), (30.0, 10.0, Some(6.0))]);
        op.kind = HoleOpKind::Pocket;
        let found = hole_source_findings(&pins(), &[6.0], &[op]);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].kind, HoleSourceKind::DrilledTwice);
    }

    #[test]
    fn dowel_sized_holes_off_the_pins_disagree() {
        let op = drill(&[(6.0, 22.5, Some(6.0)), (59.0, 32.5, Some(6.0))]);
        let found = hole_source_findings(&pins(), &[], &[op]);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].kind, HoleSourceKind::DowelHolesDisagree);
        assert!(found[0].text.contains("1 hole(s)"), "{}", found[0].text);
        assert!(found[0].text.contains("10.0 mm"), "{}", found[0].text);
    }

    #[test]
    fn other_sizes_points_and_matching_holes_do_not_disagree() {
        // An 8 mm hole is not a 6 mm dowel hole; a bare point has no size;
        // a hole within the match tolerance is on its pin.
        let op = drill(&[
            (30.0, 10.0, Some(8.0)),
            (30.0, 50.0, None),
            (6.0 + 0.5 * PIN_MATCH_TOL_MM, 32.5, Some(6.0)),
        ]);
        assert!(hole_source_findings(&pins(), &[], &[op]).is_empty());
    }

    #[test]
    fn no_pins_means_no_finding() {
        let op = drill(&[(6.0, 22.5, Some(6.0))]);
        assert!(hole_source_findings(&[], &[6.0], &[op]).is_empty());
    }
}
