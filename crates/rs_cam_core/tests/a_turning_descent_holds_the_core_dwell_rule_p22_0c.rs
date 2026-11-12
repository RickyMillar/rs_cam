//! P22-0c-2 (`planning/feeds_contract_2026-11-03/phases/P22-0c-2.md`): the
//! dwell rule of the core zone holds on a path that turns while it
//! descends, and on every lap of a helical Waterline.
//!
//! A cell under the core keeps the top it had when the blade ring last
//! passed over it. So along every piece of path with an XY diameter of at
//! most the core diameter, the descent must be at most the blade
//! protrusion (`CoreZone`, `tool/body.rs`).
//!
//! 1. A cut descent at the straight-line limit that turns 90 degrees
//!    strikes (the wanaka read, `P22-0b_wanaka.md` section 3.2: two cut
//!    descents at 7.125 deg, 0.03-0.05 mm past the protrusion). The same
//!    path after `CoreZone::clamp_descents` (the rule
//!    `adaptive3d::path::clamp_descents_to_limit` calls) gives zero core
//!    findings. The clamp only raises points.
//! 2. A helical Waterline of a non-centre-cutting tool whose laps descend
//!    at the dressup ramp angle (3 deg, the centre-cutting rule) strikes.
//!    With the tool's core zone (`helical_waterline(.., core, ..)`) each
//!    lap holds `CoreZone::max_ring_slope`, and no core finding remains.
//!
//! The probe tool of `a_split_descent_strikes_like_one_move_g_simcore`:
//! 18 mm, an 8 mm core, a 1 mm protrusion (the Waterline arm: 0.3 mm, so
//! the core rule binds under 3 deg on a ring of r 10), over a 12 mm board,
//! cell 0.25 mm.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::tool_config::{
    CentreCutting, NonCentreCuttingGeometry, ToolConfig, ToolFact, ToolId, ToolType,
};
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::mesh::{SpatialIndex, make_test_hemisphere};
use rs_cam_core::ops::waterline::{
    ChainLinkProbe, LevelRings, LevelRun, WaterlineParams, helical_waterline,
};
use rs_cam_core::stock::contact::WalkChecks;
use rs_cam_core::stock::radial_profile::LUT_SAMPLES;
use rs_cam_core::tool::body::{CoreZone, core_of, cut_table};
use rs_cam_core::tool::{FlatEndmill, MillingCutter, ToolDefinition};
use rs_cam_core::toolpath::Toolpath;

const DIAMETER_MM: f64 = 18.0;
const CORE_MM: f64 = 8.0;
const HALF: f64 = 40.0;
const STOCK_TOP_Z: f64 = 12.0;
const FLOOR_Z: f64 = 7.0;
const CELL_MM: f64 = 0.25;

fn bit(protrusion_mm: f64) -> ToolDefinition {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.name = "18 mm 2F".to_owned();
    tool.diameter = DIAMETER_MM;
    tool.set_cutting_length(ToolFact::Stated(30.0));
    tool.set_stickout(ToolFact::Stated(45.0));
    tool.set_shank_diameter(ToolFact::Stated(12.0));
    tool.set_flute_count(ToolFact::Stated(2));
    tool.set_centre_cutting(ToolFact::Stated(CentreCutting::No(
        NonCentreCuttingGeometry {
            centre_core_diameter_mm: CORE_MM,
            blade_protrusion_mm: protrusion_mm,
        },
    )));
    rs_cam_core::compute::cutter::build_cutter(&tool).expect("a valid tool")
}

fn board() -> TriDexelStock {
    TriDexelStock::from_bounds(
        &BoundingBox3 {
            min: P3::new(-HALF, -HALF, 0.0),
            max: P3::new(HALF, HALF, STOCK_TOP_Z),
        },
        CELL_MM,
    )
}

/// The move indices of the core findings of `tp` on a fresh board, with
/// the simulation's own table (`tool::body::cut_table`).
fn core_findings(tool: &ToolDefinition, tp: &Toolpath) -> Vec<usize> {
    let lut = cut_table(tool, LUT_SAMPLES);
    let mut check = WalkChecks::new(tool);
    board()
        .simulate_toolpath_with_lut_metrics_rapid_checked(
            tp,
            &lut,
            tool,
            tool.radius(),
            StockCutDirection::FromTop,
            ToolpathId::from_raw(0),
            18_000,
            2,
            5000.0,
            CELL_MM,
            None,
            &[],
            &[],
            true,
            &rs_cam_core::interrupt::NeverCancel,
            Some(&mut check),
        )
        .expect("never cancelled");
    let core = check.into_findings().core;
    core.into_iter().map(|hit| hit.move_index).collect()
}

/// A toolpath that rapids over the first point, rapids down to it, and
/// feeds through the rest.
fn fed(points: &[P3]) -> Toolpath {
    let mut tp = Toolpath::new();
    let first = points.first().copied().expect("a point");
    tp.rapid_to(P3::new(first.x, first.y, STOCK_TOP_Z + 5.0));
    tp.rapid_to(first);
    for p in points.iter().skip(1) {
        tp.feed_to(*p, 1000.0);
    }
    tp
}

/// A cut descent from 0.5 mm over the board at `slope`, 22 mm along +X,
/// then a 90 degree turn and 22 mm along +Y, in 0.5 mm steps (a draped
/// cut). The floor stops it.
fn turning_descent(slope: f64) -> Vec<P3> {
    let mut points = vec![P3::new(-20.0, -20.0, STOCK_TOP_Z + 0.5)];
    let mut z = STOCK_TOP_Z + 0.5;
    for k in 1..=44 {
        z = (z - slope * 0.5).max(FLOOR_Z);
        points.push(P3::new(-20.0 + 0.5 * f64::from(k), -20.0, z));
    }
    for k in 1..=44 {
        z = (z - slope * 0.5).max(FLOOR_Z);
        points.push(P3::new(2.0, -20.0 + 0.5 * f64::from(k), z));
    }
    points
}

#[test]
fn a_cut_descent_that_turns_holds_the_core_dwell_rule_p22_0c() {
    let tool = bit(1.0);
    let core: CoreZone = core_of(&tool).expect("a non-centre-cutting tool");
    let raw = turning_descent(core.max_descent_slope());
    let before = core_findings(&tool, &fed(&raw));
    assert!(
        !before.is_empty(),
        "control: the straight-line limit through a 90 deg turn strikes"
    );
    let mut clamped = raw.clone();
    core.clamp_descents(&mut clamped, None);
    let mut raised = 0;
    for (a, b) in raw.iter().zip(&clamped) {
        assert_eq!((a.x, a.y), (b.x, b.y), "the clamp moves only Z");
        assert!(b.z >= a.z, "the clamp only raises: {a:?} -> {b:?}");
        raised += usize::from(b.z > a.z + 1e-9);
    }
    assert!(raised > 0, "the dwell rule raised the turn");
    let after = core_findings(&tool, &fed(&clamped));
    assert!(after.is_empty(), "after the dwell rule: findings {after:?}");
    // A straight descent at the straight-line limit keeps its bits.
    let straight: Vec<P3> = (0..=40)
        .map(|k| {
            let x = 0.5 * f64::from(k);
            P3::new(x, 0.0, 12.5 - core.max_descent_slope() * x)
        })
        .collect();
    let mut kept = straight.clone();
    core.clamp_descents(&mut kept, None);
    assert_eq!(
        kept, straight,
        "a straight descent at the limit is unchanged"
    );
}

/// A circle of `radius` about the origin at `z`, 360 points.
fn circle(radius: f64, z: f64) -> Vec<P3> {
    (0..360)
        .map(|k| {
            let a = f64::from(k).to_radians();
            P3::new(radius * a.cos(), radius * a.sin(), z)
        })
        .collect()
}

/// The ring radius of the Waterline arm (mm).
const RING_MM: f64 = 10.0;

/// A stack of equal circles of radius [`RING_MM`]: one level in air over
/// the board, then two levels 3.2 mm apart. A lap of the ring descends at
/// most `2 pi r tan 3 deg` = 3.29 mm, so one lap per level at nearly the
/// 3 deg ramp angle for a centre-cutting tool.
fn ring_stack() -> Vec<LevelRings> {
    [STOCK_TOP_Z + 0.2, 9.0, 5.8]
        .into_iter()
        .map(|z| {
            let ring = circle(RING_MM, z);
            LevelRings {
                runs: vec![LevelRun {
                    path: ring.clone(),
                    whole: true,
                    contour: 0,
                }],
                contours: vec![ring],
                rest_kept: false,
                rest_dropped: false,
                outer_rings_skipped: 0,
            }
        })
        .collect()
}

fn waterline(core: Option<CoreZone>) -> Toolpath {
    let params = WaterlineParams {
        sampling: 0.5,
        feed_rate: 1000.0,
        plunge_rate: 300.0,
        safe_z: STOCK_TOP_Z + 5.0,
        stock_to_leave: 0.0,
    };
    // A small dome far below the rings: every link reads clear.
    let mesh = make_test_hemisphere(1.0, 8);
    let index = SpatialIndex::build(&mesh, 1.0);
    let reduced = FlatEndmill::new(DIAMETER_MM - 0.1, 25.0);
    let probe = ChainLinkProbe {
        mesh: &mesh,
        index: &index,
        reduced_tool: &reduced,
    };
    rs_cam_core::interrupt::run_uncancellable(|cancel| {
        helical_waterline(&ring_stack(), &params, &probe, None, core, cancel)
    })
}

#[test]
fn a_helical_waterline_lap_holds_the_core_dwell_rule_p22_0c() {
    let tool = bit(0.3);
    let core = core_of(&tool).expect("a non-centre-cutting tool");
    let ring = circle(RING_MM, 0.0);
    let slope = core.max_ring_slope(&ring);
    let ramp = rs_cam_core::ops::waterline::WATERLINE_RING_DESCENT_DEG
        .to_radians()
        .tan();
    assert!(
        slope < ramp,
        "the core rule binds: {slope:.4} under the 3 deg ramp {ramp:.4}"
    );
    // The one rule: the dwell on a circle is the helix arc of
    // `max_helix_pitch_mm` (`2 r asin(c / r)`).
    let arc = 2.0 * RING_MM * (core.radius_mm / RING_MM).asin();
    let dwell = core
        .max_dwell_mm(&ring)
        .expect("the ring is wider than the core");
    assert!(
        (dwell - arc).abs() < 0.05,
        "dwell {dwell:.3} vs arc {arc:.3}"
    );

    let centre_cutting_laps = waterline(None);
    let before = core_findings(&tool, &centre_cutting_laps);
    assert!(
        !before.is_empty(),
        "control: laps at the 3 deg ramp angle strike under the core"
    );
    let laps = waterline(Some(core));
    // Every fed lap move descends no faster than the ring slope.
    for pair in laps.moves.windows(2) {
        let (a, b) = (pair[0].target, pair[1].target);
        let dxy = (b.x - a.x).hypot(b.y - a.y);
        if dxy > 1e-9 {
            assert!(
                a.z - b.z <= slope * dxy * (1.0 + 1e-9) + 1e-9,
                "a lap move from {a:?} to {b:?} descends faster than {slope:.4}"
            );
        }
    }
    assert!(
        laps.moves.len() > centre_cutting_laps.moves.len(),
        "the core rule adds laps"
    );
    let after = core_findings(&tool, &laps);
    assert!(after.is_empty(), "core findings on the laps: {after:?}");
}
