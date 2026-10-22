//! Sentry (K3): **the obstacle plane is local. A move rises to it only
//! where it crosses a clamp, and an op plans around a clamp instead of
//! lifting over it.**
//!
//! ## The defect this pins
//!
//! K1 raised every retract and clearance of a setup with an obstacle to the
//! obstacle plane. Every lift then went to the plane, also far from a
//! clamp: on testpiece_mini the total rose from about 2673 s to 3338 s.
//! And a raster beside a clamp lifted over it at each row end, because only
//! the step after the dressups knew the clamp.
//!
//! ## The fix this pins
//!
//! 1. The heights do not read the obstacles. The per-op obstacle step lifts
//!    a move to the plane only when the check forbids it, and starts each
//!    toolpath at the plane (the join rule).
//! 2. The territory resolver removes the obstacle footprints from the
//!    territory of each op before it plans, so a raster links its rows on
//!    one side of the clamp.
//!
//! The claims:
//!
//! - (a) a link far from the clamp stays at the op's own retract;
//! - (b) a link that crosses the clamp rises to the plane;
//! - (c) a raster beside a clamp makes fewer lifts than the K1 pipeline
//!   (the op without the clamp, then the obstacle step), and no move
//!   crosses the clamp: the service check and the simulation find no hit;
//! - (d) the pre-clip reads the CUTTER band only. With a long-stickout
//!   tool whose holder reaches the clamp only below the surface, the K3
//!   path cuts every cell the K1 path cuts outside the cutter footprint.
//!   (The first K3 build read every band at the op's lowest Z and cut the
//!   holder footprint out of the territory: testpiece_mini front finish,
//!   12918 mm of cut fell to 7601 mm.)

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::print_stderr,
    reason = "the two raster tests print their lift counts for the report."
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::meshes::height_field;
use common::op_fixtures::simulate;
use common::session::{mesh_model, polygon_model, single_op_session, stock_under};
use common::tools::{ball_tool_config, endmill_tool_config};

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::SAFE_Z_CLEARANCE_MM;
use rs_cam_core::compute::obstacle::{JoinStart, clip_annotated_to_obstacles};
use rs_cam_core::compute::operation_configs::{DropCutterConfig, FaceConfig, TraceConfig};
use rs_cam_core::compute::tool_config::ToolConfig;
use rs_cam_core::ids::ObstacleId;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{AddObstacleArgs, Command, Obstacle, ObstacleGeometry, ProjectSession};
use rs_cam_core::toolpath::{MoveType, Toolpath};
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;
use rs_cam_core::trace::transform_provenance::ReconcileSet;
use std::collections::BTreeSet;

/// The stock half-width (mm) and depth: top at Z 0.
const HALF: f64 = 30.0;
const DEPTH: f64 = 6.0;
/// The clamp top and its clearance: the grown top is 21 mm.
const CLAMP_TOP: f64 = 20.0;
const CLEARANCE: f64 = 1.0;

fn plane() -> f64 {
    CLAMP_TOP + CLEARANCE + SAFE_Z_CLEARANCE_MM
}

fn add_clamp(session: &mut ProjectSession, min: [f64; 3], max: [f64; 3]) {
    let _ = session
        .apply(Command::AddObstacle(AddObstacleArgs {
            setup_index: 0,
            obstacle: Box::new(Obstacle {
                id: ObstacleId::from_raw(0),
                name: "Clamp".to_owned(),
                enabled: true,
                clearance: CLEARANCE,
                geometry: ObstacleGeometry::Box { min, max },
            }),
        }))
        .expect("a valid clamp");
}

fn generate(session: &mut ProjectSession) -> Toolpath {
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generation must succeed");
    session.result_at(0).expect("a result").toolpath().clone()
}

/// A lift: a rapid that rises.
fn lifts(tp: &Toolpath) -> usize {
    tp.moves
        .windows(2)
        .filter(|w| w[1].move_type == MoveType::Rapid && w[1].target.z > w[0].target.z + 1e-9)
        .count()
}

/// A lift to the obstacle plane: a rapid that rises to the plane.
fn plane_lifts(tp: &Toolpath) -> usize {
    tp.moves
        .windows(2)
        .filter(|w| {
            w[1].move_type == MoveType::Rapid
                && w[1].target.z > w[0].target.z + 1e-9
                && w[1].target.z >= plane() - 1e-9
        })
        .count()
}

/// The service check and the simulation both find no obstacle hit.
fn assert_no_hit(session: &mut ProjectSession, tp: &Toolpath, label: &str) {
    let set = session.obstacle_set_for_toolpath(0).expect("the clamp");
    let tool = rs_cam_core::compute::cutter::build_cutter(&session.tools()[0]).unwrap();
    let view = set
        .for_tool(
            &tool.to_assembly(&rs_cam_core::machine::head::HeadFit::default()),
            None,
        )
        .expect("a tool view");
    let hits = view.check_toolpath(tp, JoinStart::Unknown);
    assert!(hits.is_empty(), "{label}: {hits:?}");
    simulate(session);
    let sim = session.simulation_result().expect("a simulation");
    assert!(
        sim.obstacle_collisions.is_empty(),
        "{label}: {:?}",
        sim.obstacle_collisions
    );
}

/// (a) and (b): one Trace over three outlines. A and B are left of a clamp
/// across the stock, C is right of it.
#[test]
fn a_link_rises_to_the_plane_only_when_it_crosses_the_clamp_k3() {
    let outlines = vec![
        Polygon2::rectangle(-28.0, -5.0, -22.0, 5.0),
        Polygon2::rectangle(-18.0, -5.0, -12.0, 5.0),
        Polygon2::rectangle(12.0, -5.0, 20.0, 5.0),
    ];
    let mut session = single_op_session(
        stock_under(HALF, DEPTH),
        endmill_tool_config(6.0),
        polygon_model(outlines, "Three outlines"),
        "Trace",
        OperationConfig::Trace(TraceConfig {
            depth: 1.0,
            depth_per_pass: 1.0,
            ..TraceConfig::default()
        }),
    );
    add_clamp(&mut session, [-3.0, -40.0, -DEPTH], [3.0, 40.0, CLAMP_TOP]);
    let tp = generate(&mut session);
    let tc = &session.toolpath_configs()[0];
    let retract = session
        .toolpath_heights(tc)
        .expect("the heights resolve")
        .heights
        .retract_z;
    assert!(
        retract < plane() - 1.0,
        "the op's own retract {retract} must be below the plane {}",
        plane()
    );
    // The cutter keeps 3 + 1 + 3 mm from the clamp centre line.
    let keep_out = 3.0 + CLEARANCE + 3.0;
    let mut local_links = 0;
    let mut crossings = 0;
    for w in tp.moves.windows(2) {
        let (a, b) = (w[0].target, w[1].target);
        let traverse = w[1].move_type == MoveType::Rapid && (b.x - a.x).hypot(b.y - a.y) > 1e-6;
        if a.x.min(b.x) < keep_out && a.x.max(b.x) > -keep_out {
            assert!(
                a.z.min(b.z) >= plane() - 1e-9,
                "(b) a move from {a:?} to {b:?} meets the clamp below the plane"
            );
            if traverse && a.x.min(b.x) < -keep_out && a.x.max(b.x) > keep_out {
                crossings += 1;
            }
        } else if traverse && a.x.max(b.x) < -keep_out {
            // (a) far from the clamp: the op's own retract, not the plane.
            assert!(
                (a.z - retract).abs() < 1e-9 && (b.z - retract).abs() < 1e-9,
                "(a) a link from {a:?} to {b:?} far from the clamp left the retract {retract}"
            );
            local_links += 1;
        }
    }
    assert!(local_links >= 1, "(a) no link between A and B: {tp:?}");
    assert!(crossings >= 1, "(b) no link crosses the clamp: {tp:?}");
    assert_no_hit(&mut session, &tp, "trace");
}

/// The K1 pipeline for comparison: the op on the same session without the
/// clamp, then the obstacle step of the clamp.
fn k1_pipeline(mut free: ProjectSession, clamped: &ProjectSession) -> Toolpath {
    let tp = generate(&mut free);
    let set = clamped.obstacle_set_for_toolpath(0).expect("the clamp");
    let tool = rs_cam_core::compute::cutter::build_cutter(&clamped.tools()[0]).unwrap();
    let view = set
        .for_tool(
            &tool.to_assembly(&rs_cam_core::machine::head::HeadFit::default()),
            None,
        )
        .expect("a tool view");
    let tc = &free.toolpath_configs()[0];
    let retract = free
        .toolpath_heights(tc)
        .expect("the heights resolve")
        .heights
        .retract_z;
    let mut channels = ReconcileSet::empty();
    clip_annotated_to_obstacles(AnnotatedToolpath::new(tp), &view, retract, Some(300.0))
        .reconcile(&mut channels)
        .into_inner()
        .toolpath
}

/// (c) a 3D Finish raster (rows along X) beside a clamp at the +X edge,
/// across the middle of the rows' Y range.
#[test]
fn a_raster_beside_a_clamp_links_its_rows_on_one_side_k3() {
    let session = || {
        single_op_session(
            stock_under(HALF, DEPTH),
            ball_tool_config(6.0),
            mesh_model(height_field(HALF, 1.0, |_, _| -1.0), "Flat"),
            "3D Finish",
            OperationConfig::DropCutter(DropCutterConfig {
                stepover: 1.0,
                ..DropCutterConfig::default()
            }),
        )
    };
    let clamp = ([26.0, -8.0, -DEPTH], [40.0, 8.0, CLAMP_TOP]);
    let mut clamped = session();
    add_clamp(&mut clamped, clamp.0, clamp.1);
    let after = generate(&mut clamped);
    let before = k1_pipeline(session(), &clamped);
    let (n_before, n_after) = (lifts(&before), lifts(&after));
    assert!(
        n_before >= 5,
        "non-vacuity: the K1 pipeline lifts over the clamp ({n_before} lifts)"
    );
    assert!(
        n_after < n_before,
        "(c) the pre-clip must save lifts: {n_after} after, {n_before} before"
    );
    assert!(
        plane_lifts(&after) < plane_lifts(&before),
        "(c) the pre-clip must save lifts to the plane: {} after, {} before",
        plane_lifts(&after),
        plane_lifts(&before)
    );
    eprintln!(
        "3D Finish lifts: {n_after} after ({} to the plane), {n_before} before ({} to the plane)",
        plane_lifts(&after),
        plane_lifts(&before)
    );
    assert_no_hit(&mut clamped, &after, "3D Finish");
}

/// (c) the area path: a Face beside the same clamp. Face retracts after
/// each row in any case, so the claim is on the lifts to the plane: the
/// pre-clipped rows end before the clamp, and their links stay at the op's
/// own retract.
#[test]
fn a_face_beside_a_clamp_plans_around_it_k3() {
    let session = || {
        single_op_session(
            stock_under(HALF, DEPTH),
            endmill_tool_config(6.0),
            polygon_model(vec![Polygon2::rectangle(-HALF, -HALF, HALF, HALF)], "Stock"),
            "Face",
            OperationConfig::Face(FaceConfig {
                stepover: 3.0,
                depth: 1.0,
                depth_per_pass: 1.0,
                ..FaceConfig::default()
            }),
        )
    };
    let clamp = ([26.0, -8.0, -DEPTH], [40.0, 8.0, CLAMP_TOP]);
    let mut clamped = session();
    add_clamp(&mut clamped, clamp.0, clamp.1);
    let after = generate(&mut clamped);
    let before = k1_pipeline(session(), &clamped);
    let (n_before, n_after) = (plane_lifts(&before), plane_lifts(&after));
    assert!(
        n_before >= 2,
        "non-vacuity: the K1 pipeline lifts over the clamp ({n_before} lifts to the plane)"
    );
    assert!(
        n_after < n_before,
        "(c) the pre-clip must save lifts to the plane: {n_after} after, {n_before} before"
    );
    eprintln!("Face lifts to the plane: {n_after} after, {n_before} before");
    assert_no_hit(&mut clamped, &after, "Face");
}

/// The 1 mm XY cells that the cutting moves of `tp` touch on the surface
/// (Z at or below `surface_z + 0.5`), sampled every 0.25 mm.
fn cut_cells(tp: &Toolpath, surface_z: f64) -> BTreeSet<(i64, i64)> {
    let mut cells = BTreeSet::new();
    for w in tp.moves.windows(2) {
        if w[1].move_type == MoveType::Rapid {
            continue;
        }
        let (a, b) = (w[0].target, w[1].target);
        let n = ((b - a).norm() / 0.25).ceil().max(1.0) as usize;
        for k in 0..=n {
            let p = a + (b - a) * (k as f64 / n as f64);
            if p.z <= surface_z + 0.5 {
                cells.insert((p.x.floor() as i64, p.y.floor() as i64));
            }
        }
    }
    cells
}

/// (d) a 3D Finish beside a low clamp with a long-stickout tool. The
/// holder (r 12.5, 10 mm above the tip) reaches the clamp (grown top 6)
/// only with the tip below Z -4. The surface is at Z -1 and the stock
/// bottom at Z -6: a pre-clip that read every band at the stock bottom
/// would cut the holder footprint out; the K1 path cuts there.
#[test]
fn the_pre_clip_keeps_the_cells_the_holder_does_not_reach_k3() {
    const SURFACE_Z: f64 = -1.0;
    const LOW_CLAMP_TOP: f64 = 5.0;
    const HOLDER_R: f64 = 12.5;
    let tool = ToolConfig {
        cutting_length: 8.0,
        shank_diameter: 6.0,
        stickout: 10.0,
        ..ball_tool_config(6.0)
    };
    let session = || {
        single_op_session(
            stock_under(HALF, DEPTH),
            tool.clone(),
            mesh_model(height_field(HALF, 1.0, |_, _| SURFACE_Z), "Flat"),
            "3D Finish",
            OperationConfig::DropCutter(DropCutterConfig {
                stepover: 1.0,
                ..DropCutterConfig::default()
            }),
        )
    };
    let clamp_min = [26.0, -8.0, -DEPTH];
    let clamp_max = [40.0, 8.0, LOW_CLAMP_TOP];
    let mut clamped = session();
    add_clamp(&mut clamped, clamp_min, clamp_max);
    let k3 = generate(&mut clamped);
    let k1 = k1_pipeline(session(), &clamped);
    let (k3_cells, k1_cells) = (cut_cells(&k3, SURFACE_Z), cut_cells(&k1, SURFACE_Z));
    // The XY distance from a cell centre to the grown clamp box.
    let gap = |(i, j): (i64, i64)| {
        let (x, y) = (i as f64 + 0.5, j as f64 + 0.5);
        let dx = (clamp_min[0] - CLEARANCE - x)
            .max(0.0)
            .max(x - clamp_max[0] - CLEARANCE);
        let dy = (clamp_min[1] - CLEARANCE - y)
            .max(0.0)
            .max(y - clamp_max[1] - CLEARANCE);
        dx.hypot(dy)
    };
    // The cutter footprint: radius 3, and one cell diagonal of slack for
    // the cell grid and the row lattice.
    let cutter_keep_out = 3.0 + std::f64::consts::SQRT_2;
    // Non-vacuity: the K1 path cuts inside the holder footprint, where a
    // pre-clip of every band would remove the cut.
    let in_holder_band = k1_cells
        .iter()
        .filter(|&&c| gap(c) > cutter_keep_out && gap(c) < HOLDER_R)
        .count();
    assert!(
        in_holder_band >= 20,
        "non-vacuity: the K1 path cuts only {in_holder_band} cells in the holder band"
    );
    let lost: Vec<_> = k1_cells
        .iter()
        .filter(|&&c| gap(c) > cutter_keep_out && !k3_cells.contains(&c))
        .collect();
    assert!(
        lost.is_empty(),
        "(d) the K3 path loses {} cells that the K1 path cuts outside the cutter \
         footprint, first {:?}",
        lost.len(),
        lost.first()
    );
    eprintln!(
        "holder-band cells: {in_holder_band}; cut cells: {} K3, {} K1",
        k3_cells.len(),
        k1_cells.len()
    );
    assert_no_hit(&mut clamped, &k3, "3D Finish, long stickout");
}
