//! Burial audit for fed toolpath moves against the drop-cutter surface.
//!
//! G-RAMPTERRAIN (`planning/entry_moves_2026-09-03/`): the ramp-entry
//! dressup drew straight legs with no surface probe, and the legs cut
//! through standing terrain. The rapid-collision checker only audits
//! rapids, so a fed gouge was invisible to it. This module is the
//! standing check for that class: it samples every selected fed move
//! and compares each sample against the drop-cutter surface for the
//! operation's cutter.

use crate::arc_util::linearize_arc;
use crate::dressup::EntrySurfaceProbe;
use crate::geo::{P2, P3};
use crate::polygon::Polygon2;
use crate::toolpath::{MoveIntent, MoveType, Toolpath};

/// One fed move whose sampled position sinks below the protected surface.
#[derive(Debug, Clone, Copy)]
pub struct BuriedChord {
    /// Index of the move in `Toolpath::moves`.
    pub move_index: usize,
    /// The generator's intent tag for the move.
    pub intent: MoveIntent,
    /// The deepest burial found on the move, in mm.
    pub max_burial_mm: f64,
    /// The sample position at the deepest burial.
    pub worst: P3,
    /// XY chord length of the move, in mm.
    pub chord_len_mm: f64,
}

/// True for the intents that entry and lead dressups emit.
pub fn is_entry_intent(intent: MoveIntent) -> bool {
    matches!(
        intent,
        MoveIntent::EntryRamp
            | MoveIntent::EntryHelix
            | MoveIntent::EntryPlunge
            | MoveIntent::LeadIn
            | MoveIntent::LeadOut
    )
}

/// Find fed moves that sink below the drop-cutter surface.
///
/// The check samples each selected fed move every `sample_spacing_mm`.
/// It samples an arc move along the arc, not along the chord. At each
/// sample the protected floor is [`EntrySurfaceProbe::floor_z`] —
/// `cl_z + stock_to_leave` with the operation's cutter, the SAME
/// measure the entry emitters clip to. The burial at a sample is
/// `floor − sample_z`. The function reports each move whose deepest
/// burial exceeds `tolerance_mm`.
///
/// A sample outside the mesh footprint does not constrain the move:
/// the probe reports no floor there. The first move of a toolpath has
/// no start position and is skipped. `filter` selects the moves to
/// audit by intent — pass [`is_entry_intent`] for the entry sentry, or
/// `|_| true` for the all-feeds census.
pub fn buried_fed_chords(
    toolpath: &Toolpath,
    probe: &EntrySurfaceProbe<'_>,
    sample_spacing_mm: f64,
    tolerance_mm: f64,
    filter: impl Fn(MoveIntent) -> bool,
) -> Vec<BuriedChord> {
    let spacing = sample_spacing_mm.max(1e-3);
    let mut reports = Vec::new();

    let starts = toolpath.moves.iter();
    let ends = toolpath.moves.iter().skip(1);
    for (prev_index, (prev, m)) in starts.zip(ends).enumerate() {
        let move_index = prev_index + 1;
        if !filter(m.intent) {
            continue;
        }
        let samples: Vec<P3> = match m.move_type {
            MoveType::Rapid => continue,
            MoveType::Linear { .. } => {
                let d = P3::new(
                    m.target.x - prev.target.x,
                    m.target.y - prev.target.y,
                    m.target.z - prev.target.z,
                );
                let len = (d.x * d.x + d.y * d.y + d.z * d.z).sqrt();
                let n = (len / spacing).ceil().max(1.0) as usize;
                (0..=n)
                    .map(|s| {
                        let t = s as f64 / n as f64;
                        P3::new(
                            prev.target.x + d.x * t,
                            prev.target.y + d.y * t,
                            prev.target.z + d.z * t,
                        )
                    })
                    .collect()
            }
            MoveType::ArcCW { i, j, .. } => {
                linearize_arc(prev.target, m.target, i, j, true, spacing)
            }
            MoveType::ArcCCW { i, j, .. } => {
                linearize_arc(prev.target, m.target, i, j, false, spacing)
            }
        };

        let mut max_burial = f64::NEG_INFINITY;
        let mut worst = m.target;
        for p in &samples {
            let Some(floor) = probe.floor_z(p.x, p.y) else {
                continue;
            };
            let burial = floor - p.z;
            if burial > max_burial {
                max_burial = burial;
                worst = *p;
            }
        }

        if max_burial > tolerance_mm {
            let dx = m.target.x - prev.target.x;
            let dy = m.target.y - prev.target.y;
            reports.push(BuriedChord {
                move_index,
                intent: m.intent,
                max_burial_mm: max_burial,
                worst,
                chord_len_mm: (dx * dx + dy * dy).sqrt(),
            });
        }
    }
    reports
}

/// One fed move whose tool DISC reaches outside the operation's region.
///
/// The sibling of [`BuriedChord`] in the other axis: that one asks how deep
/// a fed move sank below the protected surface, this one asks how far it
/// reached past the wall.
///
/// Stays `pub`: `fed_moves_outside_region` returns it, so a crate-private
/// form raises `private_interfaces` (S29, 2026-09-16).
#[derive(Debug, Clone, Copy)]
pub struct OutsideRegionChord {
    /// Index of the move in `Toolpath::moves`.
    pub move_index: usize,
    /// The generator's intent tag for the move.
    pub intent: MoveIntent,
    /// How far the tool DISC reaches past the region at the worst sample,
    /// in mm. Always positive on a reported move.
    pub max_outside_mm: f64,
    /// The sample position at the worst reading.
    pub worst: P3,
}

/// How far the tool disc at `p` reaches outside one polygon, in mm.
///
/// Zero where the whole disc is inside. Inside the polygon the answer is
/// `radius - distance_to_boundary`; outside it is `radius + distance`,
/// because the centre itself has already left.
fn disc_outside_polygon(region: &Polygon2, p: &P2, tool_radius_mm: f64) -> f64 {
    let d = region.distance_to_boundary(p);
    if region.contains_point(p) {
        (tool_radius_mm - d).max(0.0)
    } else {
        tool_radius_mm + d
    }
}

/// Find fed moves whose tool disc leaves the operation's region.
///
/// G-RAMPCONTAIN (`planning/ui_fix_2026-09-09/research/R0.2.md`): the ramp
/// dressup drew straight legs with no region in scope, and on a small
/// pocket a leg ran past the wall and cut the surrounding stock. Neither
/// the rapid-collision checker (rapids only) nor
/// [`buried_fed_chords`] (Z only) can see that.
///
/// The check samples each selected fed move every `sample_spacing_mm`,
/// arcs along the arc. At each sample it measures how far the tool disc
/// reaches past the NEAREST region polygon, and reports a move whose worst
/// sample exceeds `tolerance_mm`. Measuring the disc against the raw
/// polygon is the same question as "inside the region offset by the tool
/// radius" and needs no offset call, so it cannot collapse or fail on a
/// thin region.
///
/// `region` holds the operation's own region, holes included. An EMPTY
/// region reports nothing — there is no region to be outside of, which is
/// "not measured", not "clean". The first move of a toolpath has no start
/// position and is skipped. `filter` selects the moves to audit by intent
/// — pass [`is_entry_intent`] for the entry sentry.
pub fn fed_moves_outside_region(
    toolpath: &Toolpath,
    region: &[Polygon2],
    tool_radius_mm: f64,
    sample_spacing_mm: f64,
    tolerance_mm: f64,
    filter: impl Fn(MoveIntent) -> bool,
) -> Vec<OutsideRegionChord> {
    let mut reports = Vec::new();
    if region.is_empty() {
        return reports;
    }
    let spacing = sample_spacing_mm.max(1e-3);

    let starts = toolpath.moves.iter();
    let ends = toolpath.moves.iter().skip(1);
    for (prev_index, (prev, m)) in starts.zip(ends).enumerate() {
        let move_index = prev_index + 1;
        if !filter(m.intent) {
            continue;
        }
        let samples: Vec<P3> = match m.move_type {
            MoveType::Rapid => continue,
            MoveType::Linear { .. } => {
                let d = P3::new(
                    m.target.x - prev.target.x,
                    m.target.y - prev.target.y,
                    m.target.z - prev.target.z,
                );
                let len = (d.x * d.x + d.y * d.y + d.z * d.z).sqrt();
                let n = (len / spacing).ceil().max(1.0) as usize;
                (0..=n)
                    .map(|s| {
                        let t = s as f64 / n as f64;
                        P3::new(
                            prev.target.x + d.x * t,
                            prev.target.y + d.y * t,
                            prev.target.z + d.z * t,
                        )
                    })
                    .collect()
            }
            MoveType::ArcCW { i, j, .. } => {
                linearize_arc(prev.target, m.target, i, j, true, spacing)
            }
            MoveType::ArcCCW { i, j, .. } => {
                linearize_arc(prev.target, m.target, i, j, false, spacing)
            }
        };

        let mut max_outside = f64::NEG_INFINITY;
        let mut worst = m.target;
        for p in &samples {
            let xy = P2::new(p.x, p.y);
            // The region is a SET: a sample inside any one polygon of it is
            // inside the region, so the answer is the smallest reading.
            let outside = region
                .iter()
                .map(|poly| disc_outside_polygon(poly, &xy, tool_radius_mm))
                .fold(f64::INFINITY, f64::min);
            if outside > max_outside {
                max_outside = outside;
                worst = *p;
            }
        }

        if max_outside > tolerance_mm {
            reports.push(OutsideRegionChord {
                move_index,
                intent: m.intent,
                max_outside_mm: max_outside,
                worst,
            });
        }
    }
    reports
}
