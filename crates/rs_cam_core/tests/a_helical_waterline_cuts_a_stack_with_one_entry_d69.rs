//! D69 §2 (W1, G-WLCONTINUOUS): the helical Waterline.
//!
//! Pre-registration: `planning/adaptive_research_2026-10-23/phase3/d69/README.md`
//! §2 and the operator ruling of 2026-10-28 (on by default, holes and
//! walls). Two rings of consecutive levels chain when one lies inside the
//! other, no ring of the two levels lies between them, and the match is
//! one to one. A chain is ONE entry: the tool descends along the LOWER
//! ring, so it cannot gouge in 3-axis.
//!
//! The fixture is a plate with a drafted outer wall and a vertical square
//! hole. Each level has two rings: the outer ring (it grows as Z falls)
//! and the hole ring (the same at each level). The rule chains each over
//! the whole ladder, so the op cuts two entries, not two per level.
//!
//! Claims:
//!
//! 1. A chain descends in one run of feed moves: no retract between its
//!    levels, and the run cuts at every level of the ladder.
//! 2. No cutting move goes below the drop-cutter CL Z of the surface
//!    under it (the model gouge guard's probe and tolerances), and none
//!    goes below the lowest level.
//! 3. A ring that matches no ring of the next level (rule 3 fails) keeps
//!    today's stacked form, move for move.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use common::op_fixtures::no_dressups;
use common::session::{mesh_model, pinned_heights, stock_over};
use common::tools::endmill_tool_config;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::model_gouge::{MODEL_GOUGE_TOL_MM, MODEL_GOUGE_Z_TOL_MM};
use rs_cam_core::compute::operation_configs::WaterlineConfig;
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh, make_test_hemisphere};
use rs_cam_core::ops::waterline::{
    ChainLinkProbe, LevelRings, LevelRun, WaterlineParams, helical_waterline,
};
use rs_cam_core::surface::dropcutter::point_drop_cutter;
use rs_cam_core::tool::FlatEndmill;
use rs_cam_core::toolpath::{MoveIntent, MoveType, RingMaterial, Toolpath};

/// Outer half width of the plate at the bottom and at the top (a drafted
/// wall), the half width of the hole, the plate height (mm).
const OUTER_BOTTOM: f64 = 20.0;
const OUTER_TOP: f64 = 18.0;
const HOLE: f64 = 5.0;
const PLATE: f64 = 6.0;
/// The pinned ladder: top, bottom and step (mm): levels 5.5 .. 0.5.
const TOP: f64 = 5.5;
const BOTTOM: f64 = 0.5;
const Z_STEP: f64 = 1.0;
const TOOL_D: f64 = 6.0;

/// A plate with a drafted outer wall and a vertical square hole, closed,
/// each face wound outward.
fn holed_plate() -> TriangleMesh {
    let mut vertices: Vec<P3> = Vec::new();
    let mut square = |half: f64, z: f64| -> [u32; 4] {
        let mut ids = [0_u32; 4];
        for (k, (sx, sy)) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
            .into_iter()
            .enumerate()
        {
            vertices.push(P3::new(sx * half, sy * half, z));
            ids[k] = u32::try_from(vertices.len() - 1).expect("few vertices");
        }
        ids
    };
    let ob = square(OUTER_BOTTOM, 0.0);
    let ot = square(OUTER_TOP, PLATE);
    let hb = square(HOLE, 0.0);
    let ht = square(HOLE, PLATE);
    let mut triangles: Vec<[u32; 3]> = Vec::new();
    let mut quad = |a: u32, b: u32, c: u32, d: u32| {
        triangles.push([a, b, c]);
        triangles.push([a, c, d]);
    };
    for k in 0..4 {
        let n = (k + 1) % 4;
        // Outer wall, out; hole wall, into the hole; top, up; bottom, down.
        quad(ob[k], ob[n], ot[n], ot[k]);
        quad(hb[n], hb[k], ht[k], ht[n]);
        quad(ot[k], ot[n], ht[n], ht[k]);
        quad(ob[k], hb[k], hb[n], ob[n]);
    }
    TriangleMesh::from_raw(vertices, triangles)
}

/// The op output (no dressup) on the plate, with the dial at `helical`.
fn waterline(helical: bool) -> Toolpath {
    let annotated = common::op_fixtures::generated_op(
        stock_over(OUTER_BOTTOM + 3.0, PLATE + 2.0),
        endmill_tool_config(TOOL_D),
        mesh_model(holed_plate(), "d69_holed_plate"),
        OperationConfig::Waterline(WaterlineConfig {
            z_step: Z_STEP,
            sampling: 0.5,
            helical,
            ..WaterlineConfig::default()
        }),
        |tc| {
            tc.dressups = no_dressups();
            tc.heights = pinned_heights(TOP, BOTTOM);
        },
    );
    annotated.toolpath
}

fn levels() -> Vec<f64> {
    rs_cam_core::ops::waterline::waterline_z_levels(TOP, BOTTOM, Z_STEP)
}

fn entries(tp: &Toolpath) -> usize {
    tp.moves
        .iter()
        .filter(|m| m.intent == MoveIntent::EntryPlunge)
        .count()
}

/// The runs of cutting moves between rapids, as move index ranges.
fn cut_runs(tp: &Toolpath) -> Vec<std::ops::Range<usize>> {
    let mut runs = Vec::new();
    let mut start = None;
    for (i, m) in tp.moves.iter().enumerate() {
        match (m.move_type.is_cutting(), start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                runs.push(s..i);
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        runs.push(s..tp.moves.len());
    }
    runs
}

#[test]
fn a_chained_stack_descends_with_one_entry_and_no_retract() {
    let ladder = levels();
    assert_eq!(ladder.len(), 6, "the ladder 5.5 .. 0.5: {ladder:?}");
    let stacked = waterline(false);
    assert_eq!(
        entries(&stacked),
        2 * ladder.len(),
        "the stacked form: one entry per ring and level"
    );
    let helical = waterline(true);
    assert_eq!(
        entries(&helical),
        2,
        "two chains (the outer wall and the hole), one entry each"
    );
    let runs = cut_runs(&helical);
    assert_eq!(runs.len(), 2, "one run of feed moves per chain");
    for run in runs {
        for &z in &ladder {
            assert!(
                helical.moves[run.clone()]
                    .iter()
                    .any(|m| (m.target.z - z).abs() < 1e-9),
                "the chain cuts at level {z} with no retract"
            );
        }
    }
}

#[test]
fn no_move_of_the_helix_goes_below_the_surface() {
    let helical = waterline(true);
    let mesh = holed_plate();
    let index = SpatialIndex::build(&mesh, 5.0);
    // The model gouge guard's probe: the tool less its radial tolerance.
    let probe = FlatEndmill::new(TOOL_D - 2.0 * MODEL_GOUGE_TOL_MM, 25.0);
    let mut judged = 0;
    for pair in helical.moves.windows(2) {
        let (from, to) = (pair[0].target, pair[1].target);
        if !pair[1].move_type.is_cutting() {
            continue;
        }
        assert!(
            to.z >= BOTTOM - 1e-9,
            "a move goes below the lowest level: {to:?}"
        );
        let len = (to.x - from.x).hypot(to.y - from.y);
        let n = (len / 0.25).ceil().max(1.0) as usize;
        for k in 0..=n {
            let t = k as f64 / n as f64;
            let p = from + (to - from) * t;
            let cl = point_drop_cutter(p.x, p.y, &mesh, &index, &probe);
            if cl.contacted {
                judged += 1;
                assert!(
                    p.z >= cl.z - MODEL_GOUGE_Z_TOL_MM,
                    "the helix cuts {:.4} mm below the surface at ({:.3}, {:.3}, {:.3})",
                    cl.z - p.z,
                    p.x,
                    p.y,
                    p.z
                );
            }
        }
    }
    assert!(judged > 0, "the check judged no sample");
}

/// A square ring of half width `half` round `(cx, cy)` at `z`.
fn square_ring(cx: f64, cy: f64, half: f64, z: f64) -> Vec<P3> {
    vec![
        P3::new(cx - half, cy - half, z),
        P3::new(cx + half, cy - half, z),
        P3::new(cx + half, cy + half, z),
        P3::new(cx - half, cy + half, z),
    ]
}

fn whole(contours: &[Vec<P3>]) -> Vec<LevelRun> {
    contours
        .iter()
        .enumerate()
        .map(|(c, path)| LevelRun {
            path: path.clone(),
            whole: true,
            contour: c,
        })
        .collect()
}

#[test]
fn an_unmatched_ring_keeps_the_stacked_form() {
    // Upper level: one ring. Lower level: two rings inside it. The upper
    // ring has two matches (rule 3 fails), so nothing chains.
    let upper = vec![square_ring(0.0, 0.0, 10.0, 2.0)];
    let lower = vec![
        square_ring(-5.0, 0.0, 3.0, 1.0),
        square_ring(5.0, 0.0, 3.0, 1.0),
    ];
    let levels = vec![
        LevelRings {
            runs: whole(&upper),
            contours: upper,
            rest_kept: false,
            rest_dropped: false,
        },
        LevelRings {
            runs: whole(&lower),
            contours: lower,
            rest_kept: false,
            rest_dropped: false,
        },
    ];
    let params = WaterlineParams {
        sampling: 0.5,
        feed_rate: 1000.0,
        plunge_rate: 300.0,
        safe_z: 20.0,
        stock_to_leave: 0.0,
    };
    let mesh = make_test_hemisphere(5.0, 8);
    let index = SpatialIndex::build(&mesh, 5.0);
    let tool = FlatEndmill::new(TOOL_D, 25.0);
    let probe = ChainLinkProbe {
        mesh: &mesh,
        index: &index,
        reduced_tool: &tool,
    };
    let helical = rs_cam_core::interrupt::run_uncancellable(|cancel| {
        helical_waterline(&levels, &params, &probe, cancel)
    });
    let mut stacked = Toolpath::new();
    for level in &levels {
        for run in &level.runs {
            let _ = stacked.emit_closed_contour_with_intent(
                &run.path,
                params.safe_z,
                params.feed_rate,
                params.plunge_rate,
                MoveIntent::FinishingCut,
                RingMaterial::NotStated,
            );
        }
    }
    assert_eq!(helical.moves.len(), stacked.moves.len());
    for (a, b) in helical.moves.iter().zip(&stacked.moves) {
        assert_eq!(a.target, b.target);
        assert_eq!(a.intent, b.intent);
        assert!(matches!(
            (a.move_type, b.move_type),
            (MoveType::Rapid, MoveType::Rapid) | (MoveType::Linear { .. }, MoveType::Linear { .. })
        ));
    }
}
