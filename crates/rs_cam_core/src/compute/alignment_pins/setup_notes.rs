//! The dowel notes for the operator at the machine (P10, 2026-10-12).
//!
//! The per-setup G-code header and the setup sheet print the same notes:
//! the dowel list, the setup that drills the dowel holes, the seat line of
//! a flipped setup and one orientation cue. This module makes the text
//! once, so the two surfaces cannot disagree.
//!
//! The dowels are numbered 1-based in the order of the stored pins
//! (`StockConfig::alignment_pins`). The positions are stock-local, which
//! is the X0 Y0 frame of every per-setup file (G-EXPORT-DATUM). The
//! dowels stay in the spoilboard at the flip, so one list is true in every
//! file.

use super::{AlignmentPin, PIN_MATCH_TOL_MM, PinFlipKind, PinSetupMap, validate_pins_for_flip};
use crate::compute::transform::{FaceUp, ZRotation};

/// The dowel notes of one setup. See [`pin_setup_notes`].
#[derive(Debug, Clone, PartialEq)]
pub struct PinSetupNotes {
    /// One lead line, then one line for each dowel.
    pub pin_lines: Vec<String>,
    /// The setup drills the dowel holes.
    pub drills_holes: bool,
    /// The dowel numbers as the operator reads them: `1` or `1-3`.
    pub dowel_range: String,
    /// `Seat the part on dowels 1-3` for a flipped setup whose holes land
    /// on the dowels. `None` for any other setup.
    pub seat: Option<String>,
    /// The orientation cue of a flipped setup whose holes land on the
    /// dowels.
    pub cue: Option<String>,
    /// One plain warning line of a flipped setup: the holes miss the
    /// dowels, or the dowels do not key the flip.
    pub warning: Option<String>,
}

/// The warning line when the dowels do not key the flip.
pub const NOT_KEYED_WARNING: &str =
    "WARNING: The dowels do not key the flip: check the orientation by eye.";

/// The dowel notes of one setup.
///
/// - `pins`: the stored pins, stock-local.
/// - `face_up`, `z_rotation`: the setup. A setup whose map is
///   [`PinFlipKind::InPlane`] (a `Bottom` setup) is a flipped setup; it
///   gets the seat line, the cue and the warning.
/// - `stock_w`, `stock_d`: the stock XY extents.
/// - `drills_holes`: the setup holds an enabled operation that drills the
///   dowel holes.
///
/// `None` when there are no pins. The caller decides if the project needs
/// the notes (a project with no flipped setup does not).
///
/// The cue comes from the map, not from a preset. A dowel whose hole the
/// flip puts back on itself is on the flip axis; the cue names it and its
/// side of the stock centre. The other dowels change holes in pairs, and
/// the cue names the pairs.
#[must_use]
pub fn pin_setup_notes(
    pins: &[AlignmentPin],
    face_up: FaceUp,
    z_rotation: ZRotation,
    stock_w: f64,
    stock_d: f64,
    drills_holes: bool,
) -> Option<PinSetupNotes> {
    if pins.is_empty() {
        return None;
    }
    let dowel_range = if pins.len() == 1 {
        "1".to_owned()
    } else {
        format!("1-{}", pins.len())
    };
    let mut pin_lines = Vec::with_capacity(pins.len() + 1);
    pin_lines.push(format!(
        "DOWELS: {} in the spoilboard, X Y from X0 Y0, diameter",
        pins.len()
    ));
    for (i, p) in pins.iter().enumerate() {
        pin_lines.push(format!(
            "Dowel {}: X {:.2} Y {:.2}, {:.2} mm",
            i + 1,
            p.at.x(),
            p.at.y(),
            p.diameter
        ));
    }

    let mut notes = PinSetupNotes {
        pin_lines,
        drills_holes,
        dowel_range,
        seat: None,
        cue: None,
        warning: None,
    };
    let map = PinSetupMap::new(face_up, z_rotation, stock_w, stock_d);
    if map.kind() != PinFlipKind::InPlane {
        return Some(notes);
    }
    let report = validate_pins_for_flip(pins, face_up, z_rotation, stock_w, stock_d);
    if !report.seats() {
        notes.warning = Some(format!(
            "WARNING: After this flip a hole misses its dowel by {:.2} mm: {}",
            report.seat_mismatch_mm.unwrap_or(f64::INFINITY),
            "the part does not seat on the dowels."
        ));
        return Some(notes);
    }
    notes.seat = Some(format!("Seat the part on dowels {}", notes.dowel_range));
    notes.cue = Some(orientation_cue(pins, &map, stock_w, stock_d));
    if !report.keyed() {
        notes.warning = Some(NOT_KEYED_WARNING.to_owned());
    }
    Some(notes)
}

/// The side of the stock centre that `(x, y)` is on, from the larger of
/// its two offsets: `+X`, `-X`, `+Y` or `-Y`.
fn side_of_centre(x: f64, y: f64, stock_w: f64, stock_d: f64) -> &'static str {
    let dx = x - 0.5 * stock_w;
    let dy = y - 0.5 * stock_d;
    if dx.abs() <= PIN_MATCH_TOL_MM && dy.abs() <= PIN_MATCH_TOL_MM {
        "at the stock centre"
    } else if dx.abs() >= dy.abs() {
        if dx > 0.0 {
            "on the +X side"
        } else {
            "on the -X side"
        }
    } else if dy > 0.0 {
        "on the +Y side"
    } else {
        "on the -Y side"
    }
}

/// `1`, `1 and 2`, `1, 2 and 4`.
fn number_list(numbers: &[usize]) -> String {
    match numbers {
        [] => String::new(),
        [one] => one.to_string(),
        [head @ .., last] => format!(
            "{} and {last}",
            head.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// The orientation cue of a flipped setup whose holes seat.
///
/// For each dowel, the hole that the flip puts on it is the hole of the
/// nearest dowel to its image. A dowel that gets its own hole back is on
/// the axis. Example (the 3-pin preset on a 65 x 65 stock, `Bottom` at 0
/// deg): "The part turns over about y = 32.5 mm. Dowel 1 (the axis pin)
/// is on the +X side and keeps its hole; dowels 2 and 3 change holes."
fn orientation_cue(pins: &[AlignmentPin], map: &PinSetupMap, stock_w: f64, stock_d: f64) -> String {
    let partner: Vec<usize> = pins
        .iter()
        .map(|p| {
            let (mx, my) = map.map_xy(p.at.x(), p.at.y());
            pins.iter()
                .enumerate()
                .map(|(j, q)| (j, (q.at.x() - mx).hypot(q.at.y() - my)))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map_or(0, |(j, _)| j)
        })
        .collect();
    let mut sentences = Vec::new();
    sentences.push(match map.axis_label() {
        Some(axis) => format!("The part turns over about {axis}."),
        None => "The part turns over.".to_owned(),
    });

    // (dowel index, the dowel) for each dowel that keeps its own hole.
    let fixed: Vec<(usize, &AlignmentPin)> = pins
        .iter()
        .enumerate()
        .zip(&partner)
        .filter(|((i, _), j)| *i == **j)
        .map(|(ij, _)| ij)
        .collect();
    let mut clauses = Vec::new();
    match fixed.as_slice() {
        [] => {}
        [(i, p)] => {
            clauses.push(format!(
                "Dowel {} (the axis pin) is {} and keeps its hole",
                i + 1,
                side_of_centre(p.at.x(), p.at.y(), stock_w, stock_d)
            ));
        }
        many => {
            let numbers: Vec<usize> = many.iter().map(|(i, _)| i + 1).collect();
            let sides: Vec<String> = many
                .iter()
                .map(|(i, p)| {
                    format!(
                        "{} {}",
                        i + 1,
                        side_of_centre(p.at.x(), p.at.y(), stock_w, stock_d)
                    )
                })
                .collect();
            clauses.push(format!(
                "Dowels {} are on the axis ({}) and keep their holes",
                number_list(&numbers),
                sides.join(", ")
            ));
        }
    }
    // A pair of dowels whose holes the flip swaps, each pair once.
    let swaps = partner
        .iter()
        .enumerate()
        .filter(|&(i, &j)| j > i && partner.get(j) == Some(&i))
        .map(|(i, &j)| format!("dowels {} and {} change holes", i + 1, j + 1));
    clauses.extend(swaps);
    if let Some((first, rest)) = clauses.split_first() {
        let mut sentence = first.clone();
        for clause in rest {
            sentence.push_str("; ");
            sentence.push_str(clause);
        }
        // Capitalise the first letter of a sentence that starts with a
        // swap clause.
        let mut chars = sentence.chars();
        let sentence = chars.next().map_or_else(String::new, |c| {
            c.to_ascii_uppercase().to_string() + chars.as_str()
        });
        sentences.push(format!("{sentence}."));
    }
    sentences.join(" ")
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;

    fn three_pin() -> Vec<AlignmentPin> {
        vec![
            AlignmentPin::new(59.0, 32.5, 6.0),
            AlignmentPin::new(7.5, 9.0, 6.0),
            AlignmentPin::new(7.5, 56.0, 6.0),
        ]
    }

    #[test]
    fn the_three_pin_preset_names_the_axis_pin_and_its_side() {
        let notes = pin_setup_notes(
            &three_pin(),
            FaceUp::Bottom,
            ZRotation::Deg0,
            65.0,
            65.0,
            false,
        )
        .expect("notes");
        assert_eq!(notes.pin_lines.len(), 4, "{:?}", notes.pin_lines);
        assert_eq!(notes.pin_lines[1], "Dowel 1: X 59.00 Y 32.50, 6.00 mm");
        assert_eq!(notes.seat.as_deref(), Some("Seat the part on dowels 1-3"));
        assert_eq!(
            notes.cue.as_deref(),
            Some(concat!(
                "The part turns over about y = 32.5 mm. Dowel 1 (the axis pin) is on the +X ",
                "side and keeps its hole; dowels 2 and 3 change holes."
            ))
        );
        assert_eq!(notes.warning, None);
    }

    #[test]
    fn a_half_turned_flip_moves_the_axis_and_the_side() {
        // `Bottom` turned 180 deg mirrors X: the axis is x = W/2. The same
        // pattern turned a quarter keys about that axis.
        let pins: Vec<AlignmentPin> = three_pin()
            .into_iter()
            .map(|p| AlignmentPin::new(p.at.y(), p.at.x(), p.diameter))
            .collect();
        let notes = pin_setup_notes(&pins, FaceUp::Bottom, ZRotation::Deg180, 65.0, 65.0, false)
            .expect("notes");
        let cue = notes.cue.expect("a cue");
        assert!(cue.contains("about x = 32.5 mm"), "{cue}");
        assert!(
            cue.contains("Dowel 1 (the axis pin) is on the +Y side"),
            "{cue}"
        );
        assert_eq!(notes.warning, None);
    }

    #[test]
    fn an_equal_pair_seats_but_does_not_key() {
        let pins = vec![
            AlignmentPin::new(59.0, 32.5, 6.0),
            AlignmentPin::new(6.0, 32.5, 6.0),
        ];
        let notes = pin_setup_notes(&pins, FaceUp::Bottom, ZRotation::Deg0, 65.0, 65.0, false)
            .expect("notes");
        assert_eq!(notes.seat.as_deref(), Some("Seat the part on dowels 1-2"));
        assert_eq!(notes.warning.as_deref(), Some(NOT_KEYED_WARNING));
    }

    #[test]
    fn a_pattern_that_does_not_seat_gets_a_warning_and_no_seat_line() {
        let pins = vec![AlignmentPin::new(10.0, 10.0, 6.0)];
        let notes = pin_setup_notes(&pins, FaceUp::Bottom, ZRotation::Deg0, 65.0, 65.0, false)
            .expect("notes");
        assert_eq!(notes.seat, None);
        assert_eq!(notes.cue, None);
        let warning = notes.warning.expect("a warning");
        assert!(warning.contains("45.00 mm"), "{warning}");
    }

    #[test]
    fn a_top_setup_lists_the_dowels_only() {
        let notes = pin_setup_notes(&three_pin(), FaceUp::Top, ZRotation::Deg0, 65.0, 65.0, true)
            .expect("notes");
        assert!(notes.drills_holes);
        assert_eq!(notes.dowel_range, "1-3");
        assert_eq!((notes.seat, notes.cue, notes.warning), (None, None, None));
        assert!(pin_setup_notes(&[], FaceUp::Top, ZRotation::Deg0, 65.0, 65.0, true).is_none());
    }
}
