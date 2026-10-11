//! Alignment pins: the keyed-pin construction and the flip audit.
//!
//! Physical dowels in the spoilboard register a part between a top setup
//! and its flipped bottom setup. The holes in the part locate it; the
//! stock edges do not.
//!
//! - `keying.rs` — the intrinsic keying check: the closest turn of the
//!   part that also puts every hole on a dowel ([`closest_wrong_seating`]).
//! - `placement.rs` — [`place_keyed_pins`]: one pin on the flip axis plus a
//!   mirror pair, in the clear strips beside the model.
//! - `pattern.rs` — pure set edits: move, diameter, snap to the flip, and
//!   the presets with their read-back (P4).
//! - this file — the pin type, the setup map ([`PinSetupMap`]), its flip
//!   axis ([`PinSetupMap::axis_segment`], [`pin_flip_map`]) and
//!   [`validate_pins_for_flip`], which judges a stored set.
//!
//! `stock_config.rs` holds the stock model and calls in here; it does not
//! hold this algorithm.

mod keying;
mod pattern;
mod placement;

pub use keying::{WrongSeating, closest_wrong_seating, keying_bar_mm, pattern_is_keyed};
pub use pattern::{
    AxisPairPattern, MixedPairPattern, axis_plus_mirror_pair, fit_axis_plus_mirror_pair,
    set_diameter, snap_to_flip, translate, two_mixed_diameters,
};
pub use placement::{PinPlacementError, PinPlacementRequest, PinSide, place_keyed_pins};

use serde::{Deserialize, Serialize};

use crate::compute::transform::{FaceUp, SetupTransformInfo, ZRotation};
use crate::geo::P3;

/// A physical alignment pin position for part registration between setups.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlignmentPin {
    pub x: f64,
    pub y: f64,
    pub diameter: f64,
}

impl AlignmentPin {
    pub fn new(x: f64, y: f64, diameter: f64) -> Self {
        Self { x, y, diameter }
    }
}

/// Clearance demanded between a pin's physical edge and any boundary —
/// the stock edge on one side, the model bbox on the other.
pub const PIN_WALL_MM: f64 = 2.0;

/// Two pin positions closer than this are the same physical hole — a
/// dowel will drop into either.
pub const PIN_MATCH_TOL_MM: f64 = 0.5;

/// Slop on the comparisons in this module, so a placement that achieves
/// exactly the required value is not rejected by its own rounding.
const PIN_GEOM_EPS_MM: f64 = 1e-9;

/// What the pin check can say about one setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinFlipKind {
    /// `Top`, at any turn: the part is not turned over. Seating and
    /// keying are not judged.
    NotAFlip,
    /// `Bottom` at 0 or 180 deg, or at 90 or 270 deg on a square stock:
    /// the footprint stays the same, and the check judges seating and
    /// keying.
    InPlane,
    /// `Bottom` at 90 or 270 deg on a stock that is not square: the turn
    /// changes the footprint. The check does not model it, and says so.
    QuarterTurnNotSquare,
    /// `Front/Back/Left/Right`: the part stands on an edge. Pins in the
    /// XY plane do not register it.
    EdgeUp,
}

/// The in-plane map of one setup: where the hole drilled at a stock-local
/// XY point lands on the board after the setup turns the part.
///
/// The map is the setup transform ([`SetupTransformInfo::world_to_local`],
/// `FaceUp` then `ZRotation`) with a zero stock origin, because pins are
/// stock-local. This type holds no second copy of that map. Examples:
///
/// - `Bottom` at 0 deg: `(x, y) -> (x, D - y)`;
/// - `Bottom` at 180 deg: `(x, y) -> (W - x, y)`;
/// - `Bottom` at 90 deg: `(x, y) -> (y, x)` (only on a square stock).
#[derive(Clone)]
pub struct PinSetupMap {
    transform: SetupTransformInfo,
    kind: PinFlipKind,
}

impl PinSetupMap {
    /// The map of a setup with `face_up` and `z_rotation` on a stock of
    /// `stock_w` x `stock_d`.
    ///
    /// A stock is square when its sides differ by at most
    /// [`PIN_MATCH_TOL_MM`]: a smaller footprint change is less than the
    /// hole slop, so a dowel does not feel it.
    #[must_use]
    pub fn new(face_up: FaceUp, z_rotation: ZRotation, stock_w: f64, stock_d: f64) -> Self {
        let square = (stock_w - stock_d).abs() <= PIN_MATCH_TOL_MM;
        let kind = match (face_up, z_rotation) {
            (FaceUp::Top, _) => PinFlipKind::NotAFlip,
            (FaceUp::Bottom, ZRotation::Deg0 | ZRotation::Deg180) => PinFlipKind::InPlane,
            (FaceUp::Bottom, ZRotation::Deg90 | ZRotation::Deg270) if square => {
                PinFlipKind::InPlane
            }
            (FaceUp::Bottom, ZRotation::Deg90 | ZRotation::Deg270) => {
                PinFlipKind::QuarterTurnNotSquare
            }
            (FaceUp::Front | FaceUp::Back | FaceUp::Left | FaceUp::Right, _) => PinFlipKind::EdgeUp,
        };
        Self {
            transform: SetupTransformInfo {
                face_up,
                z_rotation,
                stock_x: stock_w,
                stock_y: stock_d,
                stock_z: 0.0,
                ..SetupTransformInfo::default()
            },
            kind,
        }
    }

    #[must_use]
    pub fn kind(&self) -> PinFlipKind {
        self.kind
    }

    #[must_use]
    pub fn face_up(&self) -> FaceUp {
        self.transform.face_up
    }

    #[must_use]
    pub fn z_rotation(&self) -> ZRotation {
        self.transform.z_rotation
    }

    /// Where the stock-local point `(x, y)` lands after the setup. Only
    /// meaningful when [`Self::kind`] is [`PinFlipKind::InPlane`].
    #[must_use]
    pub fn map_xy(&self, x: f64, y: f64) -> (f64, f64) {
        let p = self.transform.world_to_local(P3::new(x, y, 0.0));
        (p.x, p.y)
    }

    /// The flip axis of the setup: the stock-local segment that the map
    /// keeps in place, from one stock edge to the other. `None` unless
    /// [`Self::kind`] is [`PinFlipKind::InPlane`].
    ///
    /// An in-plane flip is a mirror, so the midpoint of a point and its
    /// image is on the axis. The method takes the midpoints of the four
    /// stock corners. The map moves the stock onto itself, so the two
    /// midpoints farthest apart are the ends of the axis inside the stock:
    ///
    /// - `Bottom` at 0 deg: `y = D/2`, from `(0, D/2)` to `(W, D/2)`;
    /// - `Bottom` at 180 deg: `x = W/2`, from `(W/2, 0)` to `(W/2, D)`;
    /// - `Bottom` at 90 or 270 deg on a square stock: a diagonal.
    ///
    /// The ends are in a fixed order: the end with the smaller X first,
    /// and on a tie the end with the smaller Y first.
    #[must_use]
    pub fn axis_segment(&self) -> Option<[(f64, f64); 2]> {
        if self.kind != PinFlipKind::InPlane {
            return None;
        }
        let (w, d) = (self.transform.stock_x, self.transform.stock_y);
        let mids: Vec<(f64, f64)> = [(0.0, 0.0), (w, 0.0), (w, d), (0.0, d)]
            .into_iter()
            .map(|(x, y)| {
                let (mx, my) = self.map_xy(x, y);
                (0.5 * (x + mx), 0.5 * (y + my))
            })
            .collect();
        // (length, first end, second end) of the longest chord.
        type Chord = (f64, (f64, f64), (f64, f64));
        let mut best: Option<Chord> = None;
        for (i, a) in mids.iter().enumerate() {
            for b in mids.iter().skip(i + 1) {
                let len = (b.0 - a.0).hypot(b.1 - a.1);
                if best.is_none_or(|(l, _, _)| len > l) {
                    best = Some((len, *a, *b));
                }
            }
        }
        let (len, a, b) = best?;
        if len <= PIN_MATCH_TOL_MM {
            return None;
        }
        let a_first =
            a.0 < b.0 - PIN_GEOM_EPS_MM || ((a.0 - b.0).abs() <= PIN_GEOM_EPS_MM && a.1 <= b.1);
        Some(if a_first { [a, b] } else { [b, a] })
    }

    /// The flip axis as the operator reads it, for example `y = 32.5 mm`.
    /// `None` when [`Self::axis_segment`] is `None`.
    #[must_use]
    pub fn axis_label(&self) -> Option<String> {
        let [a, b] = self.axis_segment()?;
        Some(if (a.1 - b.1).abs() <= PIN_GEOM_EPS_MM {
            format!("y = {:.1} mm", a.1)
        } else if (a.0 - b.0).abs() <= PIN_GEOM_EPS_MM {
            format!("x = {:.1} mm", a.0)
        } else {
            format!(
                "the diagonal from ({:.1}, {:.1}) to ({:.1}, {:.1}) mm",
                a.0, a.1, b.0, b.1
            )
        })
    }

    /// The setup as the operator reads it: `'Bottom'` or
    /// `'Bottom' turned 180 deg`.
    fn label(&self) -> String {
        match self.transform.z_rotation {
            ZRotation::Deg0 => format!("'{}'", self.transform.face_up.label()),
            rotation => format!(
                "'{}' turned {}",
                self.transform.face_up.label(),
                rotation.label()
            ),
        }
    }
}

/// What kind of problem a [`PinFinding`] states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinFindingKind {
    /// A pin body is not wholly inside the stock.
    OutOfBounds,
    /// The check does not model this setup (a quarter turn on a stock
    /// that is not square).
    NotModelled,
    /// After the flip, a hole misses its dowel.
    DoesNotSeat,
    /// The part also seats turned on the dowels.
    NotKeyed,
}

impl PinFindingKind {
    /// What the operator does about it.
    #[must_use]
    pub fn fix_hint(&self) -> &'static str {
        match self {
            PinFindingKind::OutOfBounds => {
                "Move each pin so that its whole body is on the stock, with a wall to the edge."
            }
            PinFindingKind::NotModelled => {
                "Turn the flipped setup 0 or 180 deg, or make the stock square. The pin check \
                 does not model a quarter turn that changes the footprint."
            }
            PinFindingKind::DoesNotSeat => {
                "Give every pin a partner across the flip axis, or put it on the axis. The axis \
                 is y = D/2 for a Bottom setup at 0 deg and x = W/2 for a Bottom setup turned \
                 180 deg."
            }
            PinFindingKind::NotKeyed => {
                "Two pins of one diameter never key: a half turn about their midpoint puts each \
                 hole on the other dowel. Use one pin on the flip axis plus a mirror pair off \
                 the axis, or two pins of different diameters. Do not put the three pins in a \
                 line or in an equilateral triangle. The stock panel's \"Auto-place keyed \
                 pins\" places the first pattern."
            }
        }
    }
}

/// One problem with a stored pin set, as a sentence and a kind.
#[derive(Debug, Clone, PartialEq)]
pub struct PinFinding {
    pub kind: PinFindingKind,
    pub text: String,
}

/// What a stored pin set actually does under a given setup.
///
/// Reports **millimetres**, not booleans, so a caller can tell "off by
/// 0.2 mm, a dowel will still find it" from "off by 145 mm, there is no
/// hole there".
#[derive(Debug, Clone, PartialEq)]
pub struct PinFlipReport {
    /// The setup the pins were judged against.
    pub face_up: FaceUp,
    pub z_rotation: ZRotation,
    /// What the check can say about that setup.
    pub kind: PinFlipKind,
    pub stock_w: f64,
    pub stock_d: f64,
    pub pin_count: usize,
    /// Worst distance, over the dowels, from a dowel to the nearest hole
    /// of its size after the setup. `0` means every hole lands on a dowel,
    /// i.e. the part re-seats. `None` unless [`Self::kind`] is
    /// [`PinFlipKind::InPlane`].
    pub seat_mismatch_mm: Option<f64>,
    /// The closest wrong seating of the pattern (see
    /// [`closest_wrong_seating`]). `None` when no turn of the part seats.
    pub wrong_seating: Option<WrongSeating>,
    /// The miss a wrong seating needs to be blocked ([`keying_bar_mm`]).
    pub keying_bar_mm: f64,
    /// Pin indices whose body is not wholly inside the stock.
    pub out_of_bounds: Vec<usize>,
}

impl PinFlipReport {
    /// `true` when the check judges seating and keying for this setup.
    pub fn modelled_flip(&self) -> bool {
        self.kind == PinFlipKind::InPlane
    }

    /// Does the part re-seat after the flip it is programmed for?
    pub fn seats(&self) -> bool {
        self.pin_count > 0 && self.seat_mismatch_mm.is_some_and(|m| m <= PIN_MATCH_TOL_MM)
    }

    /// How far the closest wrong seating misses, in mm. `None` when no
    /// turn of the part seats at all.
    pub fn keying_margin_mm(&self) -> Option<f64> {
        self.wrong_seating.map(|w| w.miss_mm)
    }

    /// Is every wrong seating blocked? Fewer than two pins never key.
    pub fn keyed(&self) -> bool {
        self.pin_count >= 2
            && self
                .wrong_seating
                .is_none_or(|w| w.miss_mm >= self.keying_bar_mm - PIN_GEOM_EPS_MM)
    }

    /// Loader-ready warning lines. Empty when there is nothing to say —
    /// including when there are no pins at all, which is a different
    /// question than whether the pins that exist are correct.
    pub fn warnings(&self) -> Vec<String> {
        self.findings().into_iter().map(|f| f.text).collect()
    }

    /// The warning lines with their kinds, so a diagnostic can give each
    /// one its own fix hint.
    pub fn findings(&self) -> Vec<PinFinding> {
        let mut out = Vec::new();
        if self.pin_count == 0 {
            return out;
        }
        let map = PinSetupMap::new(self.face_up, self.z_rotation, self.stock_w, self.stock_d);
        // Bounds are a property of the pins alone, so they are reported
        // whatever the setup does; seating and keying only mean anything
        // against a flip the CAM actually models.
        if !self.out_of_bounds.is_empty() {
            out.push(PinFinding {
                kind: PinFindingKind::OutOfBounds,
                text: format!(
                    "{} alignment pin(s) are not wholly inside the stock.",
                    self.out_of_bounds.len()
                ),
            });
        }
        if self.kind == PinFlipKind::QuarterTurnNotSquare {
            out.push(PinFinding {
                kind: PinFindingKind::NotModelled,
                text: format!(
                    "Alignment pins are not judged for the {} setup: on a {:.1} x {:.1} mm stock \
                     that turn changes the footprint, and the pin check does not model it.",
                    map.label(),
                    self.stock_w,
                    self.stock_d
                ),
            });
        }
        if !self.modelled_flip() {
            return out;
        }
        if !self.seats() {
            out.push(PinFinding {
                kind: PinFindingKind::DoesNotSeat,
                text: format!(
                    "Alignment pins do not survive the flip: after the {} flip the worst pin \
                     misses its hole by {:.1} mm. The part cannot be re-seated on these dowels.",
                    map.label(),
                    self.seat_mismatch_mm.unwrap_or(f64::INFINITY)
                ),
            });
        }
        if !self.keyed() {
            out.push(PinFinding {
                kind: PinFindingKind::NotKeyed,
                text: self.not_keyed_text(),
            });
        }
        out
    }

    fn not_keyed_text(&self) -> String {
        const ADVICE: &str = "Use one pin on the flip axis plus a mirror pair off the axis, or \
                              two pins of different diameters.";
        if self.pin_count == 1 {
            return format!(
                "Alignment pins do not key the flip: one pin never keys, because the part can \
                 turn about it. {ADVICE}"
            );
        }
        let (turn, miss) = self
            .wrong_seating
            .map_or((180.0, 0.0), |w| (w.turn_deg.abs(), w.miss_mm));
        let lead = if self.pin_count == 2 {
            "Alignment pins do not key the flip: two pins of one diameter never key."
        } else {
            "Alignment pins do not key the flip."
        };
        format!(
            "{lead} The part also seats on the dowels turned {turn:.0} deg. The worst pin then \
             misses its hole by only {miss:.1} mm; a wrong seating is blocked only at {bar:.1} \
             mm. {ADVICE}",
            bar = self.keying_bar_mm
        )
    }
}

/// Worst-case, over the dowels, of the distance from a dowel to the
/// nearest hole that it fits, after `map` moves the holes.
///
/// For the blank to seat under `map`, EVERY dowel has to find a hole, so
/// the worst offender decides it. A dowel fits a hole of its own diameter
/// or larger; a bigger dowel does not enter a smaller hole. The dowel's
/// own hole always fits it, so the result is finite.
fn reseat_mismatch_mm(pins: &[AlignmentPin], map: impl Fn(&AlignmentPin) -> (f64, f64)) -> f64 {
    let holes: Vec<((f64, f64), f64)> = pins.iter().map(|p| (map(p), p.diameter)).collect();
    let mut worst: f64 = 0.0;
    for dowel in pins {
        let nearest = holes
            .iter()
            .filter(|(_, hole_d)| dowel.diameter <= hole_d + PIN_GEOM_EPS_MM)
            .map(|((hx, hy), _)| (hx - dowel.x).hypot(hy - dowel.y))
            .fold(f64::INFINITY, f64::min);
        if nearest.is_finite() && nearest > worst {
            worst = nearest;
        }
    }
    worst
}

/// Judge an existing pin set against one setup.
///
/// See [`PinFlipReport`]. `stock_w` / `stock_d` are the stock XY extents;
/// pins are stock-local. The setup is its full in-plane map: `face_up`
/// AND `z_rotation` (C3: a `Bottom` setup turned 180 deg mirrors X, not
/// Y).
pub fn validate_pins_for_flip(
    pins: &[AlignmentPin],
    face_up: FaceUp,
    z_rotation: ZRotation,
    stock_w: f64,
    stock_d: f64,
) -> PinFlipReport {
    let map = PinSetupMap::new(face_up, z_rotation, stock_w, stock_d);
    let out_of_bounds = pins
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            let r = p.diameter * 0.5;
            p.x - r < 0.0 || p.x + r > stock_w || p.y - r < 0.0 || p.y + r > stock_d
        })
        .map(|(i, _)| i)
        .collect();
    let seat_mismatch_mm = (map.kind() == PinFlipKind::InPlane)
        .then(|| reseat_mismatch_mm(pins, |p| map.map_xy(p.x, p.y)));

    PinFlipReport {
        face_up,
        z_rotation,
        kind: map.kind(),
        stock_w,
        stock_d,
        pin_count: pins.len(),
        seat_mismatch_mm,
        wrong_seating: closest_wrong_seating(pins),
        keying_bar_mm: keying_bar_mm(pins),
        out_of_bounds,
    }
}

/// The flip that the pins of a project register: the map of the first
/// setup whose flip is in plane ([`PinFlipKind::InPlane`]).
///
/// This is the one selector. The viewport axis line, the overlay
/// precondition and the stock panel all read it, so they cannot disagree.
/// A `Top` setup does not turn the part over, and an edge-up setup or a
/// quarter turn on a stock that is not square has no flip axis in the pin
/// plane. For those setups the result is `None`; [`pin_findings_for_setups`]
/// still states what the check does not model.
#[must_use]
pub fn pin_flip_map(
    setups: impl IntoIterator<Item = (FaceUp, ZRotation)>,
    stock_w: f64,
    stock_d: f64,
) -> Option<PinSetupMap> {
    setups
        .into_iter()
        .map(|(face_up, z_rotation)| PinSetupMap::new(face_up, z_rotation, stock_w, stock_d))
        .find(|map| map.kind() == PinFlipKind::InPlane)
}

/// The pin findings for every setup of a project, deduplicated by text and
/// in first-seen order.
///
/// A `Top` setup contributes only the bounds line, and a project with
/// several setups repeats it. Every surface (the core loader, the project
/// diagnostics, the GUI) reads this one list, so they cannot disagree.
pub fn pin_findings_for_setups(
    pins: &[AlignmentPin],
    setups: impl IntoIterator<Item = (FaceUp, ZRotation)>,
    stock_w: f64,
    stock_d: f64,
) -> Vec<PinFinding> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for (face_up, z_rotation) in setups {
        for finding in
            validate_pins_for_flip(pins, face_up, z_rotation, stock_w, stock_d).findings()
        {
            if seen.insert(finding.text.clone()) {
                out.push(finding);
            }
        }
    }
    out
}
