//! P6.0c (geometry selection, SAFETY): the side of travel that a lead arc
//! turns to, from the material side that the op states for each closed
//! pass.
//!
//! # The defect
//!
//! [`super::apply_lead_in_out`] curled every lead arc to the LEFT of travel.
//! On a pass with the material on its left, both lead arcs cut into the
//! part. Measured on HEAD 52cf29db (tool radius 3, lead radius 2,
//! `tests/a_lead_curls_away_from_the_part_p6_0c.rs`): Trace Left and
//! Profile Outside conventional cut 1.837 mm into the band round the part.
//! (These labels are from before P6.2. The O-1 ruling flipped the Trace
//! sign, so the old Trace Left is the new Trace Right.)
//!
//! # The rule
//!
//! The op states where the material is beside each closed pass
//! ([`RingMaterial`], at
//! [`crate::toolpath::Toolpath::emit_closed_contour_with_intent`]). The
//! Layered runner puts the statements in
//! [`crate::trace::produced::Produced::closed_passes`], keyed by the plunge
//! target. The dressup reads the travel direction from the pass moves (the
//! signed area of the pass ring), so the stated side converts to a side of
//! travel without a guess:
//!
//! - `Inside` or `Outside`: the arc turns away from the material. Each
//!   lead sample must also lie on the air side of the pass ring (or on
//!   it). The ring is the tool-centre path, `R` from the material, so a
//!   sample on the material side of it is a gouge. A sample there vetoes
//!   the lead, and the pass keeps its plunge or retract. This catches the
//!   second mechanism: a ring that starts at a concave corner (an inward
//!   offset keeps sharp corners), where the tangent approach goes back
//!   along the previous edge into its wall (2.000 mm on HEAD for Profile
//!   Inside, the Pocket wall, Trace Right and Chamfer Inside, on either
//!   side).
//! - `BothSides` (Trace None on the line): no arc is safe. The pass keeps
//!   its plunge and retract. A refusal would fire on the product's own
//!   default (the Finish role turns the lead on for Trace), and P1b chose
//!   the same for an open path.
//! - No statement: under a surface probe (a 3D op), the arc turns left as
//!   before. The probe lifts each sample onto the model, so the side does
//!   not decide a gouge. Without a probe (a 2.5D raster, a facing row, an
//!   open path), no lead: the dressup does not guess a side.

use crate::geo::{P2, P3};
use crate::toolpath::{ClosedPassSide, Move, MoveType, RingMaterial};

/// The side of travel a lead arc turns to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LeadTurn {
    Left,
    Right,
}

impl LeadTurn {
    /// The unit normal of travel `(ux, uy)` on this side.
    pub(super) fn normal(self, ux: f64, uy: f64) -> (f64, f64) {
        match self {
            Self::Left => (-uy, ux),
            Self::Right => (uy, -ux),
        }
    }
}

/// The air side of one closed pass: its ring and where the air is.
pub(super) struct AirSide {
    ring: Vec<P2>,
    inside: bool,
}

impl AirSide {
    /// True when every sample lies on the air side of the ring, or on it.
    pub(super) fn holds(&self, samples: &[P3]) -> bool {
        samples.iter().all(|q| {
            let p = P2::new(q.x, q.y);
            near_ring(&self.ring, p)
                || crate::polygon::point_in_polygon(&p, &self.ring) == self.inside
        })
    }
}

/// The lead of one pass end: the turn, and the air check of a pass with a
/// stated side.
pub(super) struct PassLead {
    pub(super) turn: LeadTurn,
    pub(super) air: Option<AirSide>,
}

/// The tolerance (mm) of the key match and of "on the ring". The key is a
/// copied coordinate, so it matches exactly; the slack covers a translated
/// frame (`Produced::translated`) only.
const KEY_EPS_MM: f64 = 1e-6;

/// The lead of the pass whose plunge or last cut is `moves[at]`. `ring` is
/// the move range of the pass (the plunge to the last cut). `None`: the
/// pass gets no lead.
pub(super) fn pass_lead(
    moves: &[Move],
    at: usize,
    ring: std::ops::Range<usize>,
    closed_passes: &[ClosedPassSide],
    probed: bool,
) -> Option<PassLead> {
    let key = moves.get(at)?.target;
    let stated = closed_passes.iter().find(|pass| {
        (pass.start.x - key.x).abs() < KEY_EPS_MM
            && (pass.start.y - key.y).abs() < KEY_EPS_MM
            && (pass.start.z - key.z).abs() < KEY_EPS_MM
    });
    let material = match stated.map(|pass| pass.material) {
        Some(RingMaterial::BothSides) => return None,
        Some(m @ (RingMaterial::Inside | RingMaterial::Outside)) => m,
        // `NotStated` is never recorded; read it as no statement.
        Some(RingMaterial::NotStated) | None => {
            return probed.then_some(PassLead {
                turn: LeadTurn::Left,
                air: None,
            });
        }
    };
    let points: Vec<P2> = moves
        .get(ring)?
        .iter()
        .filter(|m| m.move_type != MoveType::Rapid)
        .map(|m| P2::new(m.target.x, m.target.y))
        .collect();
    let area = signed_area(&points);
    if points.len() < 3 || area.abs() < KEY_EPS_MM {
        // No ring to read the travel direction from.
        return None;
    }
    let ccw = area > 0.0;
    // The material is on the left of travel when it is inside a CCW ring
    // or outside a CW ring.
    let material_left = (material == RingMaterial::Inside) == ccw;
    Some(PassLead {
        turn: if material_left {
            LeadTurn::Right
        } else {
            LeadTurn::Left
        },
        air: Some(AirSide {
            ring: points,
            inside: material == RingMaterial::Outside,
        }),
    })
}

/// The closed edges of `ring`: each point to the next, the last to the
/// first.
fn edges(ring: &[P2]) -> impl Iterator<Item = (P2, P2)> + '_ {
    ring.iter()
        .copied()
        .zip(ring.iter().copied().cycle().skip(1))
}

/// The signed area (mm²) of `ring`: positive for CCW.
fn signed_area(ring: &[P2]) -> f64 {
    edges(ring).map(|(a, b)| a.x * b.y - b.x * a.y).sum::<f64>() / 2.0
}

/// True when `p` lies within [`KEY_EPS_MM`] of an edge of `ring`.
fn near_ring(ring: &[P2], p: P2) -> bool {
    edges(ring).any(|(a, b)| {
        let ab = b - a;
        let len_sq = ab.norm_squared();
        let t = if len_sq > 0.0 {
            ((p - a).dot(&ab) / len_sq).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (p - (a + ab * t)).norm() < KEY_EPS_MM
    })
}
