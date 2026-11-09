//! G-ROUGHGOUGE (adaptive research Phase 1.5, 2026-10-23; B2 after the
//! evidence run, operator ruling: a default rough must export, and a graze
//! within the op's own tolerance is not a defect): the model gouge guard of
//! the 3D Rough reads the emitted moves against the mesh with the stock to
//! leave and the chord tolerance of the op.
//!
//! The row of the 3D Rough states [`ModelGougeRule::Allowance`]. With the
//! leave `L` and the chord tolerance `t` of the config
//! (`OperationConfig::model_gouge_allowance`), the probe is the tool less
//! `t` in radius, the limit is CL Z + L − t, and the model surface less the
//! tolerance is CL Z − t. The claims:
//!
//! 1. A rough-like path at the floor plus the leave, or at the limit, gives
//!    no finding. So does a path whose tool side grazes a wall by less than
//!    `t` (the Corne hex-hole case of the evidence run).
//! 2. A tip inside the leave band (below the limit, above the surface less
//!    `t`) is a Caution finding: export does not warn on it, and the
//!    finding gives the depth below the limit and the height above the
//!    surface. Feeds contract P2 commit B (`c6826ad05`, K6: no finding
//!    refuses a job) changed the text from "export does not refuse".
//! 3. A tip below the surface less `t` is Severe, with both depths, at
//!    the right place. One dip in one move gives the finding at the bottom
//!    of the dip, on that move.
//! 4. A tool side past `t` into a wall is Severe, and the finding names a
//!    wall: the depth is the wall height above the tip.
//! 5. The Pocket rule (Surface) is unchanged: its probe keeps the CAD
//!    tolerance and it reads no allowance. The 2.5D behaviour on a real
//!    model wall stays with `a_default_step_pocket_never_cuts_below_a_ring_ux7b`
//!    (`a_tool_tangent_to_a_wall_is_clean_and_a_tool_in_it_is_not_ux7b`).
//! 6. A row states `Allowance` exactly when its config gives an allowance,
//!    and the allowance is the leave and max(tolerance, merge tolerance).
//!
//! The meshes, the tool and the paths are synthetic and small. The Phase 0
//! saved projects are the evidence run.

#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::{ModelGougeRule, OperationConfig, OperationType};
use rs_cam_core::compute::model_gouge::{
    GougeProbe, MODEL_GOUGE_TOL_MM, MODEL_GOUGE_Z_TOL_MM, ModelGougeAllowance, ModelGougeFinding,
    measure, probe_radial_tolerance_mm,
};
use rs_cam_core::diagnostics::Severity;
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::tool::FlatEndmill;
use rs_cam_core::toolpath::Toolpath;

/// The Z of the floor of the model (mm).
const FLOOR_Z: f64 = 10.0;
/// The Z of the block top of the wall mesh (mm).
const BLOCK_Z: f64 = 20.0;
/// The X of the wall of the wall mesh (mm): floor below it, block above.
const WALL_X: f64 = 20.0;
/// The tool diameter (mm): a flat end mill, the default cutter of a rough.
const TOOL_D: f64 = 6.0;
const R: f64 = TOOL_D / 2.0;
/// The Y rows of the rough-like path (mm). X runs from -30 to 30.
const ROWS: [f64; 4] = [-20.0, -10.0, 0.0, 10.0];

/// A 100 x 100 mm flat floor at [`FLOOR_Z`].
fn floor_mesh() -> TriangleMesh {
    let vertices = vec![
        P3::new(-50.0, -50.0, FLOOR_Z),
        P3::new(50.0, -50.0, FLOOR_Z),
        P3::new(50.0, 50.0, FLOOR_Z),
        P3::new(-50.0, 50.0, FLOOR_Z),
    ];
    TriangleMesh::from_raw(vertices, vec![[0, 1, 2], [0, 2, 3]])
}

/// The floor at [`FLOOR_Z`] for X < [`WALL_X`], a vertical wall at
/// [`WALL_X`], and a block top at [`BLOCK_Z`] for X > [`WALL_X`].
fn wall_mesh() -> TriangleMesh {
    let vertices = vec![
        P3::new(-50.0, -50.0, FLOOR_Z),
        P3::new(WALL_X, -50.0, FLOOR_Z),
        P3::new(WALL_X, 50.0, FLOOR_Z),
        P3::new(-50.0, 50.0, FLOOR_Z),
        P3::new(WALL_X, -50.0, BLOCK_Z),
        P3::new(50.0, -50.0, BLOCK_Z),
        P3::new(50.0, 50.0, BLOCK_Z),
        P3::new(WALL_X, 50.0, BLOCK_Z),
    ];
    let triangles = vec![
        [0, 1, 2],
        [0, 2, 3],
        [4, 5, 6],
        [4, 6, 7],
        [1, 4, 7],
        [1, 7, 2],
    ];
    TriangleMesh::from_raw(vertices, triangles)
}

/// The default 3D Rough config.
fn rough_config() -> OperationConfig {
    OperationConfig::new_default(OperationType::Adaptive3d)
}

/// The axial leave and the op tolerance of `cfg`.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "test: an unexpected variant panics"
)]
fn leave_and_tolerance(cfg: &OperationConfig) -> (f64, f64) {
    match cfg {
        OperationConfig::Adaptive3d(c) => (c.stock_to_leave_axial, c.tolerance),
        _ => panic!("not a 3D Rough: {cfg:?}"),
    }
}

/// The rule of the 3D Rough row, and the allowance of its default config
/// with no segment merge.
fn rough() -> (ModelGougeRule, ModelGougeAllowance) {
    let rule = OperationType::Adaptive3d.capabilities().model_gouge;
    assert!(
        matches!(rule, ModelGougeRule::Allowance { .. }),
        "the 3D Rough row states Allowance: {rule:?}"
    );
    (rule, rough_config().model_gouge_allowance(None).unwrap())
}

/// A zigzag over [`ROWS`] at `z`: one rapid in, one plunge, the rows, and
/// the steps between them. Every cutting move is at `z` after the plunge.
fn zigzag_at(z: f64) -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(-30.0, ROWS[0], FLOOR_Z + 20.0));
    tp.feed_to(P3::new(-30.0, ROWS[0], z), 300.0);
    let mut x_end = 30.0;
    for (k, y) in ROWS.iter().enumerate() {
        if k > 0 {
            tp.feed_to(P3::new(-x_end, *y, z), 1000.0);
        }
        tp.feed_to(P3::new(x_end, *y, z), 1000.0);
        x_end = -x_end;
    }
    tp
}

/// One pass along Y beside the wall, with the tool centre at `x` and the
/// tip at `z`.
fn pass_beside_the_wall(x: f64, z: f64) -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(x, -30.0, BLOCK_Z + 10.0));
    tp.feed_to(P3::new(x, -30.0, z), 300.0);
    tp.feed_to(P3::new(x, 30.0, z), 1000.0);
    tp
}

/// Run the guard as the session does: the probe radius is the tool radius
/// less [`probe_radial_tolerance_mm`].
fn run(
    mesh: &TriangleMesh,
    tp: &Toolpath,
    rule: ModelGougeRule,
    allowance: ModelGougeAllowance,
) -> ModelGougeFinding {
    let index = SpatialIndex::build_auto(mesh);
    let radial = probe_radial_tolerance_mm(rule, allowance, R);
    let reduced = FlatEndmill::new(TOOL_D - 2.0 * radial, 25.0);
    let probe = GougeProbe {
        mesh,
        index: &index,
        reduced_tool: &reduced,
        tool_radius: R,
        bevel_depth_mm: 0.0,
        allowance,
        depth_bases: &[],
    };
    measure(tp, rule, &probe, &AtomicBool::new(false))
        .unwrap()
        .unwrap()
}

/// Two lengths are one value.
fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

/// Claim 1: at the leave, at the limit, and with a graze inside the
/// tolerance, the guard judges samples and finds nothing.
#[test]
fn a_rough_at_its_leave_or_grazing_within_its_tolerance_reads_clean_g_roughgouge() {
    let (rule, a) = rough();
    let floor = floor_mesh();
    for z in [FLOOR_Z + a.leave_mm, FLOOR_Z + a.leave_mm - a.tolerance_mm] {
        let f = run(&floor, &zigzag_at(z), rule, a);
        assert!(f.judged > 0, "the guard judged nothing at Z {z}: {f:?}");
        assert!(!f.gouges(), "a rough at Z {z} reads clean: {}", f.message());
        assert_eq!(f.severity(), None, "{f:?}");
    }
    // The tool side half the tolerance into the wall, the tip at the leave.
    let wall = wall_mesh();
    let graze = WALL_X - R + a.tolerance_mm / 2.0;
    let f = run(
        &wall,
        &pass_beside_the_wall(graze, FLOOR_Z + a.leave_mm),
        rule,
        a,
    );
    assert!(
        f.judged > 0 && !f.gouges(),
        "a graze within the tolerance: {f:?}"
    );
}

/// Claim 2: a tip inside the leave band is Caution, with the depth below
/// the limit and the height above the surface less the tolerance.
#[test]
fn a_cut_into_the_leave_is_a_caution_g_roughgouge() {
    let (rule, a) = rough();
    let z = FLOOR_Z + a.leave_mm - a.tolerance_mm - 2.0 * MODEL_GOUGE_Z_TOL_MM;
    let f = run(&floor_mesh(), &zigzag_at(z), rule, a);
    assert!(f.gouges(), "{f:?}");
    assert_eq!(f.severity(), Some(Severity::Caution), "{f:?}");
    assert!(!f.warns_at_export(), "{f:?}");
    assert!(near(f.deepest_mm, 2.0 * MODEL_GOUGE_Z_TOL_MM), "{f:?}");
    assert!(
        near(f.below_surface_mm, 2.0 * MODEL_GOUGE_Z_TOL_MM - a.leave_mm),
        "{f:?}"
    );
    assert!(!f.at_wall, "{f:?}");
    let m = f.message();
    assert!(
        m.contains("not into the model") && m.contains("mm above"),
        "{m}"
    );
    // K6 (`c6826ad05`): no finding refuses a job, so the text says the
    // export does not warn.
    assert!(m.contains("export does not warn on it"), "{m}");
}

/// Claim 3: a tip below the surface less the tolerance is Severe, with
/// both depths; a dip gives the finding at its bottom, on its move.
#[test]
fn a_cut_below_the_surface_is_severe_at_the_right_place_g_roughgouge() {
    let (rule, a) = rough();
    let floor = floor_mesh();
    let z = FLOOR_Z - a.tolerance_mm - 2.0 * MODEL_GOUGE_Z_TOL_MM;
    let f = run(&floor, &zigzag_at(z), rule, a);
    assert_eq!(f.severity(), Some(Severity::Severe), "{f:?}");
    assert!(f.warns_at_export(), "{f:?}");
    // The plunge starts in the air: its first sample is the one clean one.
    assert_eq!(f.gouging + 1, f.judged, "{f:?}");
    assert!(
        near(f.deepest_mm, a.leave_mm + 2.0 * MODEL_GOUGE_Z_TOL_MM),
        "{f:?}"
    );
    assert!(
        near(f.below_surface_mm, 2.0 * MODEL_GOUGE_Z_TOL_MM),
        "{f:?}"
    );
    assert!(
        near(f.surface_z, FLOOR_Z + a.leave_mm - a.tolerance_mm),
        "limit Z: {f:?}"
    );
    assert!(!f.at_wall, "{f:?}");
    let m = f.message();
    assert!(
        m.contains("cuts into the model") && m.contains("mm below the model"),
        "{m}"
    );

    // One dip of 1 mm below the floor in the middle of the second row.
    let at_leave = FLOOR_Z + a.leave_mm;
    let bottom = P3::new(5.0, ROWS[1], FLOOR_Z - 1.0);
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(-30.0, ROWS[0], FLOOR_Z + 20.0));
    tp.feed_to(P3::new(-30.0, ROWS[0], at_leave), 300.0);
    tp.feed_to(P3::new(30.0, ROWS[0], at_leave), 1000.0);
    tp.feed_to(P3::new(30.0, ROWS[1], at_leave), 1000.0);
    tp.feed_to(bottom, 1000.0);
    let dip_move = tp.moves.len() - 1;
    tp.feed_to(P3::new(-30.0, ROWS[1], at_leave), 1000.0);
    let f = run(&floor, &tp, rule, a);
    assert_eq!(f.severity(), Some(Severity::Severe), "{f:?}");
    assert!(
        near(f.deepest_mm, 1.0 + a.leave_mm - a.tolerance_mm),
        "{f:?}"
    );
    assert!(near(f.below_surface_mm, 1.0 - a.tolerance_mm), "{f:?}");
    assert_eq!(f.move_index, dip_move, "{f:?}");
    let [x, y, z] = f.at;
    assert!(
        near(x, bottom.x) && near(y, bottom.y) && near(z, bottom.z),
        "the deepest sample is the bottom of the dip: {f:?}"
    );
}

/// Claim 4: the tool side past the tolerance into a wall is Severe, and
/// the finding names a wall (the depth is the wall height above the tip).
#[test]
fn a_tool_past_its_tolerance_into_a_wall_is_blocking_and_named_a_wall_g_roughgouge() {
    let (rule, a) = rough();
    let into = WALL_X - R + a.tolerance_mm + 0.1;
    let tip = FLOOR_Z + a.leave_mm;
    let f = run(&wall_mesh(), &pass_beside_the_wall(into, tip), rule, a);
    assert_eq!(f.severity(), Some(Severity::Severe), "{f:?}");
    assert!(f.at_wall, "{f:?}");
    let limit = BLOCK_Z + a.leave_mm - a.tolerance_mm;
    assert!(near(f.deepest_mm, limit - tip), "{f:?}");
    assert!(
        near(f.below_surface_mm, BLOCK_Z - a.tolerance_mm - tip),
        "{f:?}"
    );
    assert!(
        f.message().contains("wall steeper than 45 degrees"),
        "{}",
        f.message()
    );
}

/// Claim 5: the Pocket rule keeps the CAD tolerance in its probe, reads no
/// allowance, and every finding it gives is Severe.
#[test]
fn the_pocket_rule_reads_no_allowance_g_roughgouge() {
    let (rule, a) = rough();
    let pocket = OperationType::Pocket.capabilities().model_gouge;
    assert!(
        matches!(pocket, ModelGougeRule::Surface { .. }),
        "{pocket:?}"
    );
    assert!(near(
        probe_radial_tolerance_mm(pocket, a, R),
        MODEL_GOUGE_TOL_MM
    ));
    assert!(near(probe_radial_tolerance_mm(rule, a, R), a.tolerance_mm));
    let floor = floor_mesh();

    // Inside the leave band: Caution under the rough, clean under Pocket.
    let band = zigzag_at(FLOOR_Z + a.leave_mm - a.tolerance_mm - 2.0 * MODEL_GOUGE_Z_TOL_MM);
    let surface = run(&floor, &band, pocket, a);
    assert!(surface.judged > 0 && !surface.gouges(), "{surface:?}");

    // Below the floor: the Pocket depth is the depth below the floor alone.
    let below = zigzag_at(FLOOR_Z - 2.0 * MODEL_GOUGE_Z_TOL_MM);
    let surface = run(&floor, &below, pocket, a);
    assert!(
        near(surface.deepest_mm, 2.0 * MODEL_GOUGE_Z_TOL_MM),
        "{surface:?}"
    );
    assert!(
        near(surface.below_surface_mm, surface.deepest_mm),
        "{surface:?}"
    );
    assert_eq!(surface.severity(), Some(Severity::Severe), "{surface:?}");
    assert!(!surface.at_wall, "{surface:?}");
}

/// Claim 6: a row states `Allowance` exactly when its default config gives
/// an allowance, and the 3D Rough allowance is its leave and the larger of
/// its tolerance and the segment merge tolerance.
#[test]
fn an_allowance_row_has_an_allowance_g_roughgouge() {
    for &op in OperationType::ALL {
        let is_allowance = matches!(
            op.capabilities().model_gouge,
            ModelGougeRule::Allowance { .. }
        );
        let allowance = OperationConfig::new_default(op).model_gouge_allowance(None);
        assert_eq!(is_allowance, allowance.is_some(), "{op:?}");
    }
    let cfg = rough_config();
    let (leave, tol) = leave_and_tolerance(&cfg);
    let none = cfg.model_gouge_allowance(None).unwrap();
    assert!(
        near(none.leave_mm, leave) && near(none.tolerance_mm, tol),
        "{none:?}"
    );
    let wide = cfg.model_gouge_allowance(Some(tol + 0.2)).unwrap();
    assert!(near(wide.tolerance_mm, tol + 0.2), "{wide:?}");
    let narrow = cfg.model_gouge_allowance(Some(tol / 2.0)).unwrap();
    assert!(near(narrow.tolerance_mm, tol), "{narrow:?}");
}
