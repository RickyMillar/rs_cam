//! G-FLUTETOP (2026-10-02, the T5 arm of `planning/tier_trial_2026-10-01/`).
//!
//! The push cutter modelled the tool as a body `cutter.length()` tall (the
//! flute length) with nothing above it. A vertex, edge or facet more than
//! that height over the fiber was skipped. Under a hill that stands more
//! than the flute length over a waterline level, every fiber came back free,
//! and the weave closed a contour inside the hill. The drop cutter has no
//! such top, so the two disagreed by the whole excess height.
//!
//! On the rivmap board at scale 3 (relief 36 mm, R2 tapered ball with a
//! 20 mm flute) the UnifiedFinish VerySteep band cut 852 fed moves more than
//! 1 mm under the drop-cutter surface, the deepest 28.9 mm. On the 350 board
//! (relief 42 mm) the simulation cut columns to the stock bottom.
//!
//! The fix: material above the flutes still blocks (the shank and the holder
//! stand there), so no push test has a top.

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
use rs_cam_core::geometry::fiber::Fiber;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::ops::waterline::{WaterlineParams, waterline_contours, waterline_toolpath};
use rs_cam_core::surface::pushcutter::push_cutter_fiber;
use rs_cam_core::tool::{BallEndmill, MillingCutter};

/// Flute length of the test cutter (mm).
const FLUTE_MM: f64 = 20.0;
/// Hill height (mm): more than the flute length over the level below.
const HILL_H: f64 = 30.0;
/// Hill flank angle from horizontal (degrees).
const FLANK_DEG: f64 = 60.0;
/// The waterline level (mm).
const LEVEL: f64 = 2.0;
const SAMPLING: f64 = 0.5;

/// A cone hill on a plane at z = 0.
fn hill() -> TriangleMesh {
    let slope = FLANK_DEG.to_radians().tan();
    height_field(25.0, 0.5, |x, y| {
        (HILL_H - (x * x + y * y).sqrt() * slope).max(0.0)
    })
}

fn cutter() -> BallEndmill {
    BallEndmill::new(4.0, FLUTE_MM)
}

#[test]
fn a_fiber_under_a_hill_taller_than_the_flutes_is_blocked() {
    let mesh = hill();
    let index = SpatialIndex::build_auto(&mesh);
    let tool = cutter();
    // Non-vacuity: the hill top stands more than the flute length over the
    // level, so a top at the flute length would hide it.
    assert!(HILL_H - LEVEL > tool.length());
    // A fiber through the hill's axis at the level.
    let mut fiber = Fiber::new_x(0.0, LEVEL, -25.0, 25.0);
    push_cutter_fiber(&mut fiber, &mesh, &index, &tool);
    // The point under the hill top (x = 0, t = 0.5) must be blocked.
    let blocked = fiber
        .intervals()
        .iter()
        .any(|iv| iv.lower <= 0.5 && 0.5 <= iv.upper);
    assert!(
        blocked,
        "the fiber under the hill top reads free: intervals {:?}",
        fiber.intervals()
    );
}

#[test]
fn a_waterline_level_closes_no_contour_inside_a_hill() {
    let mesh = hill();
    let index = SpatialIndex::build_auto(&mesh);
    let tool = cutter();

    // One loop: the hill's foot at the level, offset by the tool. Before the
    // fix a second loop ran inside the hill, where it stands FLUTE_MM over
    // the level.
    let contours = waterline_contours(&mesh, &index, &tool, LEVEL, SAMPLING);
    let radii: Vec<f64> = contours
        .iter()
        .map(|c| {
            c.iter().map(|p| (p.x * p.x + p.y * p.y).sqrt()).sum::<f64>() / c.len() as f64
        })
        .collect();
    eprintln!("contour mean radii {radii:?}");
    assert_eq!(
        contours.len(),
        1,
        "one contour expected at z {LEVEL}; mean radii {radii:?}"
    );

    // No fed move of the level passes under the drop-cutter surface.
    let params = WaterlineParams {
        sampling: SAMPLING,
        feed_rate: 1000.0,
        plunge_rate: 500.0,
        safe_z: HILL_H + 5.0,
        stock_to_leave: 0.0,
    };
    let tp = waterline_toolpath(&mesh, &index, &tool, LEVEL, LEVEL, 1.0, &params);
    assert!(!tp.moves.is_empty(), "non-vacuity: the level emits moves");
    let probe = EntrySurfaceProbe {
        mesh: &mesh,
        index: &index,
        cutter: &tool,
        stock_to_leave: 0.0,
        off_mesh: OffMeshEntry::PlungeFallback,
        rest_stock: None,
    };
    let buried = buried_fed_chords(&tp, &probe, 0.1, 0.05, |_| true);
    let worst = buried.iter().map(|b| b.max_burial_mm).fold(0.0, f64::max);
    assert!(
        buried.is_empty(),
        "{} fed moves pass more than 0.05 mm under the drop-cutter surface, the deepest {worst:.3} mm",
        buried.len()
    );
}
