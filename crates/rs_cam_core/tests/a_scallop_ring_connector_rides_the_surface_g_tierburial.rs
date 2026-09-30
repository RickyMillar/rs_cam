//! G-TIERBURIAL (`planning/tiered_finish_2026-09-30/RESULTS.md`, "Fine-tier
//! burial"): a continuous-mode scallop joins ring i to ring i + 1 with a
//! "helical" connector feed. The connector was one straight 3D line from the
//! tool's position to the next ring's nearest kept point, up to
//! `3 x cusp_r` long, and nothing probed it against the surface. On convex
//! ground it passed under the drop-cutter surface: 0.836 mm deep on the
//! rivmap100 fine tier (the unified-finish mid-steep band runs
//! `continuous: true`), against a planned cusp of 0.03 mm.
//!
//! The fix refines the connector against the drop-cutter surface exactly as
//! a ring chord is refined, and retracts when it cannot track the surface.
//!
//! This sentry runs a continuous scallop over a dumbbell region whose rings
//! split into two loops, one per lobe, across a ridge, and checks two
//! things:
//!
//! - non-vacuity: at least one connector's STRAIGHT chord (tool position to
//!   the next ring's start, the pre-fix move) sits deeper under the
//!   drop-cutter surface than the op tolerance;
//! - the emitted path: no fed sample sits deeper under the drop-cutter
//!   surface than the op tolerance.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

mod common;

use common::meshes::height_field;
use rs_cam_core::dressup::entry_audit::buried_fed_chords;
use rs_cam_core::dressup::{EntrySurfaceProbe, OffMeshEntry};
use rs_cam_core::finish::scallop::{ScallopParams, ScallopRuntimeEvent};
use rs_cam_core::geo::{P2, P3};
use rs_cam_core::geometry::region_set::RegionSet;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::tool::BallEndmill;
use rs_cam_core::toolpath::{MoveType, Toolpath};

const HALF: f64 = 10.0;
const STEP: f64 = 0.1;
/// A ridge along Y at X = 0: height and Gaussian half-width (mm).
const RIDGE_H: f64 = 1.5;
const RIDGE_W: f64 = 0.8;
/// The region: two 8 x 8 mm lobes joined by a neck NECK_L long in X and
/// NECK_H high in Y. An inward offset past NECK_H / 2 splits the rings into
/// one loop per lobe, NECK_L + 2 x inset apart: under the connector limit
/// `3 x cusp_r` = 3 mm for insets below 1 mm. The connector between the
/// lobes crosses the ridge.
const LOBE: f64 = 8.0;
const NECK_L: f64 = 1.0;
const NECK_H: f64 = 0.6;
const BALL_D: f64 = 2.0;
/// The fine tier's scallop height and op tolerance (the rivmap100 fixture).
const SCALLOP_H: f64 = 0.03;
const TOL: f64 = 0.05;
const SAMPLE_MM: f64 = 0.05;

fn ridge() -> TriangleMesh {
    height_field(HALF, STEP, |x, _y| RIDGE_H * (-(x / RIDGE_W).powi(2)).exp())
}

fn dumbbell() -> Polygon2 {
    let (n, h, l) = (NECK_L / 2.0, NECK_H / 2.0, LOBE / 2.0);
    let x1 = n + LOBE;
    Polygon2::new(vec![
        P2::new(-x1, -l),
        P2::new(-n, -l),
        P2::new(-n, -h),
        P2::new(n, -h),
        P2::new(n, -l),
        P2::new(x1, -l),
        P2::new(x1, l),
        P2::new(n, l),
        P2::new(n, h),
        P2::new(-n, h),
        P2::new(-n, l),
        P2::new(-x1, l),
    ])
}

/// Deepest burial of the straight chord `a -> b` under the probe's floor.
fn chord_burial(a: P3, b: P3, probe: &EntrySurfaceProbe<'_>) -> f64 {
    let len = (b.x - a.x).hypot(b.y - a.y);
    let n = (len / SAMPLE_MM).ceil().max(1.0) as usize;
    (0..=n)
        .filter_map(|k| {
            let t = k as f64 / n as f64;
            let (x, y) = (a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
            let z = a.z + (b.z - a.z) * t;
            probe.floor_z(x, y).map(|f| f - z)
        })
        .fold(f64::NEG_INFINITY, f64::max)
}

/// The end of the connector that starts at move `k`: the last move whose
/// XY lies on the segment from the anchor (`moves[k - 1]`) that every move
/// in between also lies on. The fix inserts refined points on that XY
/// segment; before the fix the connector is the one move `k`.
fn connector_end(tp: &Toolpath, k: usize) -> usize {
    let a = tp.moves[k - 1].target;
    let mut end = k;
    for j in (k + 1)..tp.moves.len().min(k + 64) {
        if !matches!(tp.moves[j].move_type, MoveType::Linear { .. }) {
            break;
        }
        let e = tp.moves[j].target;
        let (dx, dy) = (e.x - a.x, e.y - a.y);
        let len = dx.hypot(dy);
        if len < 1e-9 {
            break;
        }
        let on_line = (k..j).all(|i| {
            let p = tp.moves[i].target;
            let t = ((p.x - a.x) * dx + (p.y - a.y) * dy) / (len * len);
            let perp = ((p.x - a.x) * dy - (p.y - a.y) * dx).abs() / len;
            perp < 1e-6 && t > 0.0 && t < 1.0
        });
        if !on_line {
            break;
        }
        end = j;
    }
    end
}

#[test]
fn a_continuous_scallop_connector_does_not_pass_under_the_surface() {
    let mesh = ridge();
    let region = RegionSet::new(vec![dumbbell()]);
    let index = SpatialIndex::build_auto(&mesh);
    let cutter = BallEndmill::new(BALL_D, 25.0);
    let params = ScallopParams {
        scallop_height: SCALLOP_H,
        tolerance: TOL,
        continuous: true,
        ..ScallopParams::default()
    };
    let (tp, anns, _) = rs_cam_core::interrupt::run_uncancellable(|cancel| {
        rs_cam_core::finish::scallop::scallop_toolpath_structured_annotated_with_cancel(
            &mesh,
            &index,
            &cutter,
            &params,
            None,
            Some(&region),
            cancel,
        )
    });
    let probe = EntrySurfaceProbe {
        mesh: &mesh,
        index: &index,
        cutter: &cutter,
        stock_to_leave: 0.0,
        off_mesh: OffMeshEntry::PlungeFallback,
        rest_stock: None,
    };

    // Non-vacuity: the straight connector chords this fixture calls for.
    let mut connectors = 0usize;
    let mut worst_straight = (0.0f64, 0.0f64);
    for ann in &anns {
        let ScallopRuntimeEvent::Ring { continuous, .. } = ann.event;
        let k = ann.move_index;
        if !continuous || k == 0 || k >= tp.moves.len() {
            continue;
        }
        if !matches!(tp.moves[k].move_type, MoveType::Linear { .. })
            || matches!(tp.moves[k - 1].move_type, MoveType::Rapid)
        {
            continue; // a retract, not a connector
        }
        connectors += 1;
        let end = connector_end(&tp, k);
        let (a, b) = (tp.moves[k - 1].target, tp.moves[end].target);
        let straight = chord_burial(a, b, &probe);
        if straight > worst_straight.0 {
            worst_straight = (straight, (b.x - a.x).hypot(b.y - a.y));
        }
    }
    eprintln!(
        "{} moves, {connectors} connectors; worst straight connector {:.4} mm deep \
         ({:.3} mm long)",
        tp.moves.len(),
        worst_straight.0,
        worst_straight.1
    );
    assert!(connectors > 0, "non-vacuity: the spiral joins rings");
    assert!(
        worst_straight.0 > TOL,
        "non-vacuity: a straight connector would pass {:.4} mm under the surface, \
         not more than the tolerance {TOL}",
        worst_straight.0
    );

    let reports = buried_fed_chords(&tp, &probe, SAMPLE_MM, f64::NEG_INFINITY, |_| true);
    let worst = reports
        .iter()
        .max_by(|a, b| a.max_burial_mm.total_cmp(&b.max_burial_mm))
        .unwrap();
    eprintln!(
        "emitted: worst fed burial {:.4} mm at move {} ({:?}, chord {:.3} mm)",
        worst.max_burial_mm, worst.move_index, worst.intent, worst.chord_len_mm
    );
    assert!(
        worst.max_burial_mm <= TOL,
        "move {} ({:?}, chord {:.3} mm) passes {:.4} mm under the drop-cutter surface; \
         the op tolerance is {TOL}",
        worst.move_index,
        worst.intent,
        worst.chord_len_mm,
        worst.max_burial_mm
    );
}
