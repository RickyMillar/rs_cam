//! G-ROUGHGOUGE (adaptive research Phase 1.5, 2026-10-23): the model gouge
//! guard of the 3D Rough reads the emitted moves against the mesh with the
//! stock to leave of the op.
//!
//! The row of the 3D Rough states [`ModelGougeRule::Allowance`]. A sample
//! gouges when the tip goes below the drop-cutter CL Z plus the limit
//! offset of the config (`OperationConfig::model_gouge_limit_offset_mm`:
//! the axial leave less the chord tolerance of the planner). The claims:
//!
//! 1. A rough-like path at the floor plus the leave reads clean, and so
//!    does a path at the floor plus the offset (the promise of the
//!    planner: it may simplify a draped chord by its chord tolerance).
//! 2. The same path shifted down by the leave plus two Z tolerances gives a
//!    finding on every sample, with depth `offset + 2 * MODEL_GOUGE_Z_TOL_MM`
//!    and the limit Z in `surface_z`. One dip in one move gives the finding
//!    at the bottom of the dip, on that move.
//! 3. A tip between the floor and the limit gouges under the rule of the
//!    3D Rough and is clean under the rule of a Pocket: the Surface rule
//!    does not read the offset. The 2.5D behaviour on a model wall stays
//!    with `a_default_step_pocket_never_cuts_below_a_ring_ux7b`
//!    (`a_tool_tangent_to_a_wall_is_clean_and_a_tool_in_it_is_not_ux7b`).
//! 4. A row states `Allowance` exactly when its config gives an offset,
//!    and the offset is the leave less the chord tolerance.
//!
//! The mesh, the tool and the paths are synthetic and small. The Phase 0
//! saved projects are the evidence run.

#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::{ModelGougeRule, OperationConfig, OperationType};
use rs_cam_core::compute::model_gouge::{
    GougeProbe, MODEL_GOUGE_TOL_MM, MODEL_GOUGE_Z_TOL_MM, ModelGougeFinding, measure,
};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::tool::FlatEndmill;
use rs_cam_core::toolpath::Toolpath;

/// The Z of the flat floor of the model (mm).
const FLOOR_Z: f64 = 10.0;
/// The tool diameter (mm): a flat end mill, the default cutter of a rough.
const TOOL_D: f64 = 6.0;
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

/// The default 3D Rough config: its leave and its tolerance.
fn rough_config() -> OperationConfig {
    OperationConfig::new_default(OperationType::Adaptive3d)
}

/// The axial leave and the op tolerance of `cfg`.
fn leave_and_tolerance(cfg: &OperationConfig) -> (f64, f64) {
    match cfg {
        OperationConfig::Adaptive3d(c) => (c.stock_to_leave_axial, c.tolerance),
        _ => panic!("not a 3D Rough: {cfg:?}"),
    }
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

/// Run the guard with `rule` and the limit offset `offset_mm`.
fn run(tp: &Toolpath, rule: ModelGougeRule, offset_mm: f64) -> ModelGougeFinding {
    let mesh = floor_mesh();
    let index = SpatialIndex::build_auto(&mesh);
    // The probe of the session: the tool less the radial tolerance.
    let reduced = FlatEndmill::new(TOOL_D - 2.0 * MODEL_GOUGE_TOL_MM, 25.0);
    let probe = GougeProbe {
        mesh: &mesh,
        index: &index,
        reduced_tool: &reduced,
        tool_radius: TOOL_D / 2.0,
        bevel_depth_mm: 0.0,
        limit_offset_mm: offset_mm,
        depth_bases: &[],
    };
    measure(tp, rule, &probe, &AtomicBool::new(false))
        .unwrap()
        .unwrap()
}

/// The rule of the 3D Rough row, and the offset of its default config.
fn rough_rule_and_offset() -> (ModelGougeRule, f64) {
    let rule = OperationType::Adaptive3d.capabilities().model_gouge;
    assert!(
        matches!(rule, ModelGougeRule::Allowance { .. }),
        "the 3D Rough row states Allowance: {rule:?}"
    );
    let offset = rough_config().model_gouge_limit_offset_mm(None).unwrap();
    (rule, offset)
}

/// Claim 1: a path at the floor plus the leave, and at the floor plus the
/// offset, reads clean, and the guard judged samples on it.
#[test]
fn a_rough_at_its_leave_reads_clean_g_roughgouge() {
    let (rule, offset) = rough_rule_and_offset();
    let (leave, _) = leave_and_tolerance(&rough_config());
    for z in [FLOOR_Z + leave, FLOOR_Z + offset] {
        let f = run(&zigzag_at(z), rule, offset);
        assert!(f.judged > 0, "the guard judged nothing at Z {z}: {f:?}");
        assert!(!f.gouges(), "a rough at Z {z} reads clean: {}", f.message());
    }
}

/// Claim 2: the path shifted down by the leave plus two Z tolerances
/// gouges on every judged sample, by the offset plus two Z tolerances,
/// and the finding names the limit Z. A dip in one move gives the finding
/// at the bottom of the dip, on that move.
#[test]
fn a_rough_below_its_leave_reads_the_depth_and_the_place_g_roughgouge() {
    let (rule, offset) = rough_rule_and_offset();
    let (leave, _) = leave_and_tolerance(&rough_config());

    let shifted_z = FLOOR_Z + leave - (leave + 2.0 * MODEL_GOUGE_Z_TOL_MM);
    let f = run(&zigzag_at(shifted_z), rule, offset);
    assert!(f.gouges(), "{f:?}");
    // The plunge starts in the air: its first sample is the one clean one.
    assert_eq!(f.gouging + 1, f.judged, "{f:?}");
    let want = offset + 2.0 * MODEL_GOUGE_Z_TOL_MM;
    assert!(
        (f.deepest_mm - want).abs() < 1e-9,
        "depth {} != {want}",
        f.deepest_mm
    );
    assert!(
        (f.surface_z - (FLOOR_Z + offset)).abs() < 1e-9,
        "limit Z: {f:?}"
    );
    assert!(f.message().contains("stock to leave"), "{}", f.message());

    // One dip of 1 mm below the floor in the middle of the second row.
    let at_leave = FLOOR_Z + leave;
    let bottom = P3::new(5.0, ROWS[1], FLOOR_Z - 1.0);
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(-30.0, ROWS[0], FLOOR_Z + 20.0));
    tp.feed_to(P3::new(-30.0, ROWS[0], at_leave), 300.0);
    tp.feed_to(P3::new(30.0, ROWS[0], at_leave), 1000.0);
    tp.feed_to(P3::new(30.0, ROWS[1], at_leave), 1000.0);
    tp.feed_to(bottom, 1000.0);
    let dip_move = tp.moves.len() - 1;
    tp.feed_to(P3::new(-30.0, ROWS[1], at_leave), 1000.0);
    let f = run(&tp, rule, offset);
    assert!(f.gouges(), "{f:?}");
    assert!((f.deepest_mm - (1.0 + offset)).abs() < 1e-9, "{f:?}");
    assert_eq!(f.move_index, dip_move, "{f:?}");
    let [x, y, z] = f.at;
    assert!(
        (x - bottom.x).abs() < 1e-9 && (y - bottom.y).abs() < 1e-9 && (z - bottom.z).abs() < 1e-9,
        "the deepest sample is the bottom of the dip: {f:?}"
    );
}

/// Claim 3: a tip between the floor and the limit gouges under the rule
/// of the 3D Rough, and is clean under the rule of a Pocket with the same
/// probe (the Surface rule does not read the offset). Below the floor, the
/// Pocket rule reads the depth below the floor alone.
#[test]
fn the_offset_moves_only_the_allowance_rule_g_roughgouge() {
    let (rule, offset) = rough_rule_and_offset();
    let pocket = OperationType::Pocket.capabilities().model_gouge;
    assert!(
        matches!(pocket, ModelGougeRule::Surface { .. }),
        "{pocket:?}"
    );

    let between = zigzag_at(FLOOR_Z + offset - 2.0 * MODEL_GOUGE_Z_TOL_MM);
    let rough = run(&between, rule, offset);
    assert!(rough.gouges(), "{rough:?}");
    assert!(
        (rough.deepest_mm - 2.0 * MODEL_GOUGE_Z_TOL_MM).abs() < 1e-9,
        "{rough:?}"
    );
    let surface = run(&between, pocket, offset);
    assert!(surface.judged > 0 && !surface.gouges(), "{surface:?}");

    let below = zigzag_at(FLOOR_Z - 2.0 * MODEL_GOUGE_Z_TOL_MM);
    let surface = run(&below, pocket, offset);
    assert!(
        (surface.deepest_mm - 2.0 * MODEL_GOUGE_Z_TOL_MM).abs() < 1e-9,
        "{surface:?}"
    );
}

/// Claim 4: a row states `Allowance` exactly when its default config gives
/// a limit offset, and the 3D Rough offset is its leave less the larger of
/// its tolerance and the segment merge tolerance.
#[test]
fn an_allowance_row_has_an_offset_g_roughgouge() {
    for &op in OperationType::ALL {
        let is_allowance = matches!(
            op.capabilities().model_gouge,
            ModelGougeRule::Allowance { .. }
        );
        let offset = OperationConfig::new_default(op).model_gouge_limit_offset_mm(None);
        assert_eq!(is_allowance, offset.is_some(), "{op:?}");
    }
    let cfg = rough_config();
    let (leave, tol) = leave_and_tolerance(&cfg);
    let none = cfg.model_gouge_limit_offset_mm(None).unwrap();
    assert!((none - (leave - tol)).abs() < 1e-12, "{none}");
    let wide = cfg.model_gouge_limit_offset_mm(Some(tol + 0.2)).unwrap();
    assert!((wide - (leave - tol - 0.2)).abs() < 1e-12, "{wide}");
    let narrow = cfg.model_gouge_limit_offset_mm(Some(tol / 2.0)).unwrap();
    assert!((narrow - (leave - tol)).abs() < 1e-12, "{narrow}");
}
