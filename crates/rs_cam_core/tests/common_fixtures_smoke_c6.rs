//! C6 — the shared test-fixture library is byte-identical to what it replaces.
//!
//! Oracle: `planning/review_2026-07-29/TECH_DEBT_RESEARCH_AND_FIX_PLAN.md`
//! Addendum C item C6, and `ANTIPATTERNS_BACKLOG.md` P6.
//!
//! `tests/common/` generalised five private copies of `grooved_block`, two
//! private height-field meshers and four private FNV fingerprints into one
//! parameterised set. The obvious hazard is that "generalise" quietly becomes
//! "subtly change": several consumers of these fixtures pin toolpath
//! fingerprints, so a mesh that differs in the last ulp moves a constant that
//! nobody can later attribute.
//!
//! This file is the guard. For every generator whose structure actually
//! changed, it carries the DONOR implementation verbatim — copied from the
//! test file it came from at the commit before migration — and asserts the
//! shared version reproduces it vertex-for-vertex and triangle-for-triangle,
//! at each donor's own parameters. It is deliberately not a quality test:
//! nothing here says the fixtures are good, only that they are the same.
//!
//! When the remaining private copies migrate (`reach_policy_pr4`,
//! `pencil_tip_float_channel_d1`, `coverage_routing_pr5`,
//! `generic_rest_routing_pr7`), their parameters are already covered below.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::fingerprint::{fnv1a_debug, move_fingerprint};
use common::meshes::{GroovedBlock, extrude_profile, grooved_block, height_field, sawtooth_plate};
use common::session::{
    generate, mesh_model, pinned_heights, polygon_model, single_op_session, single_op_session_with,
    square_polygon, stock_over, toolpath_config,
};
use common::tools::{ball_control, ball_tool_config, endmill_tool_config, wanaka_taper};

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{PocketConfig, PocketPattern, ScallopConfig};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::tool::MillingCutter;

// ── Donor implementations, verbatim ─────────────────────────────────────

/// From `checkpoint_a_valley_matrix.rs` / `reach_policy_pr4.rs` /
/// `pencil_tip_float_channel_d1.rs` (identical bodies; the three differed
/// only in `dense_half` and `dense_step`, which are parameters here).
fn donor_grooved_block_symmetric(
    rim_half_width: f64,
    wall_deg: f64,
    depth: f64,
    dense_half: f64,
    dense_step: f64,
) -> TriangleMesh {
    let tan = wall_deg.to_radians().tan();
    let floor_half = rim_half_width - depth / tan;
    assert!(floor_half > 0.0, "groove is a V, not a trapezoid");
    let z_at = |x: f64| -> f64 {
        let ax = x.abs();
        if ax >= rim_half_width {
            0.0
        } else if ax <= floor_half {
            -depth
        } else {
            -depth + (ax - floor_half) * tan
        }
    };

    let mut xs: Vec<f64> = Vec::new();
    let mut x = -20.0;
    while x < -dense_half {
        xs.push(x);
        x += 1.0;
    }
    let mut x = -dense_half;
    while x <= dense_half + 1e-9 {
        xs.push(x);
        x += dense_step;
    }
    for b in [-rim_half_width, -floor_half, floor_half, rim_half_width] {
        xs.push(b);
    }
    let mut x = dense_half + 1.0;
    while x <= 20.0 + 1e-9 {
        xs.push(x);
        x += 1.0;
    }
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    xs.dedup_by(|a, b| (*a - *b).abs() < 1e-9);

    let ys: Vec<f64> = (0..=24).map(|i| -12.0 + i as f64).collect();

    let mut verts = Vec::with_capacity(xs.len() * ys.len());
    for &yv in &ys {
        for &xv in &xs {
            verts.push(P3::new(xv, yv, z_at(xv)));
        }
    }
    let nx = xs.len();
    let mut tris: Vec<[u32; 3]> = Vec::new();
    for j in 0..ys.len() - 1 {
        for i in 0..nx - 1 {
            let a = (j * nx + i) as u32;
            let b = (j * nx + i + 1) as u32;
            let c = ((j + 1) * nx + i + 1) as u32;
            let d = ((j + 1) * nx + i) as u32;
            tris.push([a, b, c]);
            tris.push([a, c, d]);
        }
    }
    TriangleMesh::from_raw(verts, tris)
}

/// From `coverage_routing_pr5.rs` / `generic_rest_routing_pr7.rs` (identical
/// bodies). Note it inserts no breakpoints and samples a wider dense window.
fn donor_grooved_block_skewed(
    rim_half_width: f64,
    wall_deg: f64,
    depth: f64,
    skew: f64,
) -> TriangleMesh {
    let tan_l = wall_deg.to_radians().tan();
    let tan_r = (wall_deg * skew).to_radians().tan();
    let z_at = |x: f64| -> f64 {
        let floor_l = rim_half_width - depth / tan_l;
        let floor_r = rim_half_width - depth / tan_r;
        if x <= -rim_half_width || x >= rim_half_width {
            0.0
        } else if x < 0.0 {
            let ax = -x;
            if ax <= floor_l {
                -depth
            } else {
                -depth + (ax - floor_l) * tan_l
            }
        } else if x <= floor_r {
            -depth
        } else {
            -depth + (x - floor_r) * tan_r
        }
    };
    let mut xs: Vec<f64> = Vec::new();
    let mut x = -20.0;
    while x < -5.0 {
        xs.push(x);
        x += 1.0;
    }
    let mut x = -5.0;
    while x <= 5.0 + 1e-9 {
        xs.push(x);
        x += 0.05;
    }
    let mut x = 6.0;
    while x <= 20.0 + 1e-9 {
        xs.push(x);
        x += 1.0;
    }
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    xs.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    let ys: Vec<f64> = (0..=24).map(|i| -12.0 + i as f64).collect();
    let mut verts = Vec::with_capacity(xs.len() * ys.len());
    for &yv in &ys {
        for &xv in &xs {
            verts.push(P3::new(xv, yv, z_at(xv)));
        }
    }
    let nx = xs.len();
    let mut tris: Vec<[u32; 3]> = Vec::new();
    for j in 0..ys.len() - 1 {
        for i in 0..nx - 1 {
            let a = (j * nx + i) as u32;
            let b = (j * nx + i + 1) as u32;
            let c = ((j + 1) * nx + i + 1) as u32;
            let d = ((j + 1) * nx + i) as u32;
            tris.push([a, b, c]);
            tris.push([a, c, d]);
        }
    }
    TriangleMesh::from_raw(verts, tris)
}

/// From `standing_material_channel_am9.rs`.
fn donor_sawtooth_plate(half: f64, period: f64, amplitude: f64) -> TriangleMesh {
    let step_x = period / 16.0;
    let step_y = 2.0;
    let nx = ((2.0 * half) / step_x).round() as usize + 1;
    let ny = ((2.0 * half) / step_y).round() as usize + 1;
    let mut vertices = Vec::with_capacity(nx * ny);
    for j in 0..ny {
        let y = -half + j as f64 * step_y;
        for i in 0..nx {
            let x = -half + i as f64 * step_x;
            let phase = x / period - (x / period).floor();
            let ridge = 1.0 - (2.0 * phase - 1.0).abs();
            vertices.push(P3::new(x, y, amplitude * ridge));
        }
    }
    let mut triangles = Vec::with_capacity((nx - 1) * (ny - 1) * 2);
    for j in 0..(ny - 1) {
        for i in 0..(nx - 1) {
            let a = (j * nx + i) as u32;
            let b = a + 1;
            let c = ((j + 1) * nx + i) as u32;
            let d = c + 1;
            triangles.push([a, c, b]);
            triangles.push([b, c, d]);
        }
    }
    TriangleMesh::from_raw(vertices, triangles)
}

/// From `checkpoint_b_resolution_ab.rs` (`HALF = 8.0`, `MESH_STEP = 0.25`).
fn donor_height_field(z: impl Fn(f64, f64) -> f64) -> TriangleMesh {
    const HALF: f64 = 8.0;
    const MESH_STEP: f64 = 0.25;
    let n = ((2.0 * HALF) / MESH_STEP).round() as usize + 1;
    let mut vertices = Vec::with_capacity(n * n);
    for j in 0..n {
        let y = -HALF + j as f64 * MESH_STEP;
        for i in 0..n {
            let x = -HALF + i as f64 * MESH_STEP;
            vertices.push(P3::new(x, y, z(x, y)));
        }
    }
    let mut triangles = Vec::with_capacity((n - 1) * (n - 1) * 2);
    for j in 0..(n - 1) {
        for i in 0..(n - 1) {
            let a = (j * n + i) as u32;
            let b = a + 1;
            let c = ((j + 1) * n + i) as u32;
            let d = c + 1;
            triangles.push([a, c, b]);
            triangles.push([b, c, d]);
        }
    }
    TriangleMesh::from_raw(vertices, triangles)
}

#[track_caller]
fn assert_same_mesh(label: &str, got: &TriangleMesh, want: &TriangleMesh) {
    assert_eq!(
        got.vertices.len(),
        want.vertices.len(),
        "{label}: vertex count"
    );
    assert_eq!(
        got.triangles.len(),
        want.triangles.len(),
        "{label}: triangle count"
    );
    for (i, (g, w)) in got.vertices.iter().zip(&want.vertices).enumerate() {
        assert_eq!(
            (g.x.to_bits(), g.y.to_bits(), g.z.to_bits()),
            (w.x.to_bits(), w.y.to_bits(), w.z.to_bits()),
            "{label}: vertex {i} differs — {g:?} vs {w:?}"
        );
    }
    assert_eq!(got.triangles, want.triangles, "{label}: winding");
}

// ── Bit-identity of the generalised generators ──────────────────────────

/// Every parameter set the five donor copies of `grooved_block` are called
/// with, symmetric and skewed, reproduced exactly by the one builder.
#[test]
fn grooved_block_builder_reproduces_all_five_donor_copies() {
    // (rim, wall_deg, depth, dense_half, dense_step) — call sites from
    // checkpoint_a (1.5/70/1.2 @ 3.0/0.1), reach_policy_pr4 (1.5 and 2.5 @
    // 4.0/0.05) and pencil_tip_float_channel_d1 (0.6/80/2.5 and 2.0/45/1.0 @
    // 3.0/0.05).
    for (rim, wall, depth, dense_half, dense_step) in [
        (1.5, 70.0, 1.2, 3.0, 0.1),
        (1.5, 70.0, 1.2, 4.0, 0.05),
        (2.5, 70.0, 1.2, 4.0, 0.05),
        (0.6, 80.0, 2.5, 3.0, 0.05),
        (2.0, 45.0, 1.0, 3.0, 0.05),
    ] {
        let got = GroovedBlock::new(rim, wall, depth)
            .dense_half_width(dense_half)
            .dense_step(dense_step)
            .build();
        let want = donor_grooved_block_symmetric(rim, wall, depth, dense_half, dense_step);
        assert_same_mesh(
            &format!("symmetric groove {rim}/{wall}/{depth} @ {dense_half}/{dense_step}"),
            &got,
            &want,
        );
    }

    // coverage_routing_pr5 (2.5/70/1.2 skew 1.0 and 3.0/80/1.0 skew 0.45) and
    // generic_rest_routing_pr7 (8.0/50/5.0 skew 1.0).
    for (rim, wall, depth, skew) in [
        (2.5, 70.0, 1.2, 1.0),
        (3.0, 80.0, 1.0, 0.45),
        (8.0, 50.0, 5.0, 1.0),
    ] {
        let got = GroovedBlock::new(rim, wall, depth)
            .skew(skew)
            .dense_half_width(5.0)
            .breakpoints(false)
            .build();
        let want = donor_grooved_block_skewed(rim, wall, depth, skew);
        assert_same_mesh(
            &format!("skewed groove {rim}/{wall}/{depth} skew {skew}"),
            &got,
            &want,
        );
    }
}

/// The symmetric default and the `skew(1.0)` path must be the same mesh:
/// unifying two donor bodies into one is only sound if the asymmetric
/// formulation degenerates exactly.
#[test]
fn symmetric_default_and_unit_skew_agree() {
    let a = grooved_block(2.5, 70.0, 1.2);
    let b = GroovedBlock::new(2.5, 70.0, 1.2).skew(1.0).build();
    assert_same_mesh("skew(1.0) degenerates to symmetric", &b, &a);
}

/// `sawtooth_plate` was re-expressed on top of `height_field_grid`; the plate
/// it produces must not have moved.
#[test]
fn sawtooth_plate_reproduces_its_donor() {
    let got = sawtooth_plate(25.0, 2.0, 1.0);
    let want = donor_sawtooth_plate(25.0, 2.0, 1.0);
    assert_same_mesh("A/M9 corrugation", &got, &want);
}

/// The checkpoint-B height fields, at their own `HALF` / `MESH_STEP`, through
/// the shared square-grid convenience.
#[test]
fn height_field_reproduces_its_donor() {
    let narrow_ridge = |x: f64, _y: f64| 4.0 * (1.0 - x.abs()).max(0.0);
    assert_same_mesh(
        "narrow ridge",
        &height_field(8.0, 0.25, narrow_ridge),
        &donor_height_field(narrow_ridge),
    );

    let narrow_valley = |x: f64, _y: f64| 4.0 - 4.0 * (1.0 - x.abs()).max(0.0);
    assert_same_mesh(
        "narrow valley",
        &height_field(8.0, 0.25, narrow_valley),
        &donor_height_field(narrow_valley),
    );
}

/// The fingerprint helper must be the same FNV-1a the pinned constants were
/// captured with — spelled out here rather than cross-checked against another
/// copy of itself.
#[test]
fn fingerprint_is_the_pinned_fnv1a() {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in "the quick brown fox".bytes() {
        h ^= u64::from(byte);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    assert_eq!(fnv1a_debug("the quick brown fox"), {
        // `Debug` on `&str` adds the surrounding quotes, so hash the rendering
        // the helper actually hashes.
        let mut q: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in "\"the quick brown fox\"".bytes() {
            q ^= u64::from(byte);
            q = q.wrapping_mul(0x0000_0100_0000_01b3);
        }
        q
    });
    assert_ne!(h, 0, "the offset basis must not be zeroed");
}

// ── The builders wire up a session that really generates ────────────────

/// The 17-field `ToolpathConfig` builder, the model/stock helpers and the
/// one-op session builder produce something the REAL generation entry point
/// accepts — and the struct-update override idiom keeps working.
#[test]
fn session_builders_drive_a_real_generation() {
    let mut session = single_op_session(
        stock_over(25.0, 1.0),
        ball_tool_config(3.0),
        mesh_model(sawtooth_plate(25.0, 2.0, 1.0), "corrugation"),
        "Scallop",
        OperationConfig::Scallop(ScallopConfig {
            scallop_height: 0.1,
            tolerance: 0.1,
            ..ScallopConfig::default()
        }),
    );
    generate(&mut session, 0);
    let result = session.get_result(0).expect("a generated result");
    assert!(
        result.stats.move_count > 0,
        "the shared fixtures must produce a real toolpath, not an empty one"
    );
    let (moves, hash) = move_fingerprint(result.toolpath());
    assert_eq!(moves, result.toolpath().moves.len());
    assert_ne!(hash, 0);

    // 2D family: polygon model, and a tweak closure reaching the config.
    let mut pocket = single_op_session_with(
        stock_over(15.0, 10.0),
        endmill_tool_config(3.0),
        polygon_model(vec![square_polygon(10.0)], "square"),
        "Pocket",
        OperationConfig::Pocket(PocketConfig {
            stepover: 2.0,
            depth: 2.0,
            depth_per_pass: 2.0,
            feed_rate: 800.0,
            plunge_rate: 400.0,
            climb: true,
            pattern: PocketPattern::Contour,
            angle: 0.0,
            finishing_passes: 0,
            spindle_rpm: Some(18_000),
        }),
        |cfg| cfg.name = "Renamed By Tweak".to_owned(),
    );
    assert_eq!(
        pocket
            .get_toolpath_config(0)
            .expect("the tweaked config was added")
            .name,
        "Renamed By Tweak"
    );
    generate(&mut pocket, 0);
    assert!(pocket.get_result(0).expect("result").stats.move_count > 0);

    // The struct-update override idiom the module documents.
    let cfg = rs_cam_core::session::ToolpathConfig {
        heights: pinned_heights(0.0, -9.0),
        enabled: false,
        ..toolpath_config(
            "Pinned",
            OperationConfig::Scallop(ScallopConfig::default()),
            0,
            0,
        )
    };
    assert!(!cfg.enabled);
    assert_eq!(cfg.name, "Pinned");
}

/// The tool fixtures are the tools their docs claim: the 6× envelope-vs-cusp
/// split is the whole reason the taper is the shared finishing fixture.
#[test]
fn tool_fixtures_have_the_radii_their_docs_claim() {
    let taper = wanaka_taper();
    assert!((taper.envelope_radius_mm() - 3.0).abs() < 1e-9);
    assert!((taper.cusp_radius_mm() - 0.5).abs() < 1e-9);

    let ball = ball_control();
    assert!((ball.envelope_radius_mm() - 1.5).abs() < 1e-9);
    assert_eq!(ball.envelope_radius_mm(), ball.cusp_radius_mm());
}

/// `extrude_profile` and `plateau` were copied verbatim; a shape check is
/// enough to catch a botched move.
#[test]
fn copied_generators_keep_their_shape() {
    let ribbon = extrude_profile(&[(-1.0, 0.0), (0.0, -1.0), (1.0, 0.0)], -5.0, 5.0);
    assert_eq!(ribbon.vertices.len(), 6);
    assert_eq!(ribbon.triangles.len(), 4);

    let block = common::meshes::plateau(20.0, 5.0);
    assert_eq!(block.vertices.len(), 8);
    assert_eq!(block.triangles.len(), 12);
    assert!((block.bbox.min.z - -5.0).abs() < 1e-9);
    assert!((block.bbox.max.z - 0.0).abs() < 1e-9);
}
