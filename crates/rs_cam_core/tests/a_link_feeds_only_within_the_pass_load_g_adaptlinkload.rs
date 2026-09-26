//! G-ADAPTLINKLOAD — a 2D Adaptive keep-down link feeds only within the
//! load the planner holds its cutting passes to.
//!
//! Operator ruling 2026-09-26: "Don't plough unless the link can genuinely
//! do a legit cutting move with defined load to get there." The planner
//! holds a pass step to `target_engagement_fraction(stepover, R) × 1.05`
//! (`adaptive/search.rs`, `pass_engagement_ceiling`), public as
//! `rs_cam_core::adaptive::pass_engagement_limit`. A keep-down `Link` is now
//! walked in pass steps on the planner's material grid, admitted only when
//! no step exceeds that ceiling, and stamped; otherwise the planner
//! retracts and re-enters with its normal entry.
//!
//! Before the ruling, on this fixture (G-ADAPTORDER instrument at
//! a5916499): 22 fed links, 20 in material, 172 samples, 1056 mm^3, peak
//! radial 0.90, 32 retracts.
//!
//! Mop chain hops (the second arm). The residue mop used to walk tool-down
//! across cleared gaps of up to 6 R to the next residue INSIDE its `Cut`
//! (intent `ClearingCut`), so this sentry never saw them. A hop is now a
//! keep-down link under the same rule: the traverse through cleared cells
//! to the last pass-step position short of the next material is a fed
//! `Link` (intent `Linking`), admitted by the same pass-load walk, and a
//! refused hop ends the chain for a retract and re-entry. The bite that
//! follows is the mop's ordinary step. Measured at 91bb006a with the hops
//! labelled by a throw-away probe: 48 hops, every one ending in material,
//! peak radial 0.493 (within limit + tolerance; 12 over the bare limit),
//! 255 mm^3 including the bite. After: 14 -> 82 fed links, the 68 added
//! being the hops; every `Linking` sample, hops included, is held by the
//! assertion below.
//!
//! The fixture is the six-island pocket of G-ADAPTORDER
//! (`common::adaptive_islands`). The oracle is the session simulation of the
//! emitted path (dexel stock, sim cell 0.5 mm), not the planner.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

mod common;

use common::adaptive_islands::{
    SIM_RESOLUTION_MM, STEPOVER_MM, TOLERANCE_MM, TOOL_RADIUS_MM, adaptive_session, toolpath_of,
    trace_of,
};

use rs_cam_core::adaptive::pass_engagement_limit;
use rs_cam_core::ops::adaptive_shared::radial_woc_fraction_from_leading_arc;
use rs_cam_core::toolpath::{MoveIntent, MoveType};

/// Retracts (rapids that climb in Z) in the emitted path before the ruling,
/// measured by the G-ADAPTORDER instrument at a5916499.
const PRE_RULING_RETRACTS: usize = 32;

/// Fed keep-down links in the emitted path at 91bb006a, before the mop's
/// chain hops were emitted as links (measured by this file's `measure`).
const PRE_HOP_FED_LINKS: usize = 14;

/// The planner's material-grid cell: `max(R / 6, tolerance)`
/// (`adaptive/path.rs`, `adaptive_segments_with_debug`).
fn planner_cell_mm() -> f64 {
    (TOOL_RADIUS_MM / 6.0).max(TOLERANCE_MM)
}

/// The pass ceiling as the simulator's radial width-of-cut fraction a_e/D.
fn pass_limit_radial() -> f64 {
    radial_woc_fraction_from_leading_arc(pass_engagement_limit(STEPOVER_MM, TOOL_RADIUS_MM))
}

/// What the simulator may read above the planner on the same cut, as a
/// fraction of D. The sim radial is the perpendicular extent of fresh sim
/// cells over D (`dexel_stock/stamping.rs`). Three widths separate it from
/// the planner's grid: a planner cell is cleared when its lattice point is
/// inside a stamp disc, so up to one planner cell (0.5 mm) of real material
/// can sit where the planner reads cleared; the sim measures extent at sim
/// cell centres (0.5 mm); and the emitted path is the planner path
/// simplified to the operation tolerance (0.1 mm). (0.5 + 0.5 + 0.1) / 6 =
/// 0.183.
fn discretisation_tolerance() -> f64 {
    (planner_cell_mm() + SIM_RESOLUTION_MM + TOLERANCE_MM) / (2.0 * TOOL_RADIUS_MM)
}

#[derive(Debug, Default)]
struct Links {
    /// Fed `Linking` moves (keep-down links).
    fed: usize,
    fed_in_material: usize,
    samples_in_material: usize,
    volume_mm3: f64,
    /// Peak radial over every `Linking` sample, rapids included.
    peak_radial: f64,
    worst_move: usize,
    retracts: usize,
    cycle_s: f64,
}

fn measure(session: &rs_cam_core::session::ProjectSession) -> Links {
    let tp = toolpath_of(session);
    let trace = trace_of(session);
    let mut out = Links {
        cycle_s: trace.summary.total_runtime_s,
        ..Links::default()
    };
    for s in &trace.samples {
        if s.source_intent == Some(MoveIntent::Linking)
            && s.engagement.radial_woc_fraction > out.peak_radial
        {
            out.peak_radial = s.engagement.radial_woc_fraction;
            out.worst_move = s.move_index;
        }
    }
    for (i, mv) in tp.moves.iter().enumerate() {
        if mv.intent != MoveIntent::Linking || matches!(mv.move_type, MoveType::Rapid) || i == 0 {
            continue;
        }
        out.fed += 1;
        let mut n = 0;
        for s in trace.samples.iter().filter(|s| s.move_index == i) {
            if s.removed_volume_est_mm3 > 1e-3 {
                n += 1;
                out.volume_mm3 += s.removed_volume_est_mm3;
            }
        }
        out.fed_in_material += usize::from(n > 0);
        out.samples_in_material += n;
    }
    out.retracts = tp
        .moves
        .windows(2)
        .filter(|w| matches!(w[1].move_type, MoveType::Rapid) && w[1].target.z > w[0].target.z)
        .count();
    out
}

/// Every link the simulator sees, mop hops included, stays within the pass
/// load; the planner still links keep-down where it can, and retracts where
/// it ploughed.
#[test]
fn a_link_feeds_only_within_the_pass_load() {
    let session = adaptive_session(false);
    let links = measure(&session);
    let limit = pass_limit_radial();
    let tol = discretisation_tolerance();
    eprintln!(
        "G-ADAPTLINKLOAD: {links:?}; limit {limit:.4} + tol {tol:.4}; cycle {:.1} s",
        links.cycle_s
    );
    assert!(
        links.fed >= 1,
        "no keep-down link left: the fixture tests nothing"
    );
    assert!(
        links.peak_radial <= limit + tol,
        "a Linking sample on move {} reads radial {:.4}, above the pass limit {limit:.4} + \
         {tol:.4}",
        links.worst_move,
        links.peak_radial
    );
    assert!(
        links.fed > PRE_HOP_FED_LINKS,
        "{} fed links, {PRE_HOP_FED_LINKS} before the mop hops became links: a mop hop is \
         still hidden inside a ClearingCut, out of reach of the load check above",
        links.fed
    );
    assert!(
        links.retracts > PRE_RULING_RETRACTS,
        "{} retracts, {PRE_RULING_RETRACTS} before the ruling: no ploughing link was turned \
         into a retract",
        links.retracts
    );
}

/// Per-link evidence: prints each fed link's length and simulated load.
#[test]
#[ignore = "instrument: prints each keep-down link"]
fn print_each_keep_down_link() {
    let session = adaptive_session(false);
    let tp = toolpath_of(&session);
    let trace = trace_of(&session);
    for (i, mv) in tp.moves.iter().enumerate() {
        if mv.intent != MoveIntent::Linking || i == 0 {
            continue;
        }
        let from = tp.moves[i - 1].target;
        let len = (mv.target.x - from.x).hypot(mv.target.y - from.y);
        let (mut r, mut v) = (0.0_f64, 0.0_f64);
        for s in trace.samples.iter().filter(|s| s.move_index == i) {
            r = r.max(s.engagement.radial_woc_fraction);
            v += s.removed_volume_est_mm3;
        }
        eprintln!(
            "move {i:5} {:?} len {len:7.2} z {:6.2}->{:6.2} peak radial {r:.3} volume {v:8.2}",
            mv.move_type, from.z, mv.target.z
        );
    }
    eprintln!("{:?}", measure(&session));
}
