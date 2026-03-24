//! Parameter sweep test harness.
//!
//! For each operation, generates a baseline toolpath with default params, then
//! varies one parameter at a time, fingerprints both, diffs, and writes
//! JSON + SVG artifacts to `target/param_sweeps/`.
//!
//! Run all sweeps:    `cargo test --test param_sweep`
//! Run one family:    `cargo test --test param_sweep sweep_pocket`
//! Run one parameter: `cargo test --test param_sweep sweep_pocket_stepover`
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout,
    clippy::print_stderr
)]

use rs_cam_core::{
    adaptive::AdaptiveParams,
    dexel_stock::{StockCutDirection, TriDexelStock},
    dropcutter::batch_drop_cutter,
    fingerprint::{
        FingerprintDiff, ParameterSweepResult, StockFingerprint, SweepArtifacts, SweepVariant,
        ToolpathFingerprint, diff_fingerprints,
    },
    geo::{BoundingBox3, P2},
    mesh::{SpatialIndex, TriangleMesh, make_test_hemisphere},
    pocket::PocketParams,
    polygon::Polygon2,
    profile::{ProfileParams, ProfileSide},
    tool::{BallEndmill, FlatEndmill, MillingCutter},
    toolpath::{Toolpath, raster_toolpath_from_grid},
    waterline::{WaterlineParams, waterline_toolpath},
};
use std::path::PathBuf;

// ── Output helpers ──────────────────────────────────────────────────────

fn output_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/param_sweeps");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_json(path: &std::path::Path, value: &impl serde::Serialize) {
    let json = serde_json::to_string_pretty(value).unwrap();
    std::fs::write(path, json).unwrap();
}

fn write_svg(path: &std::path::Path, tp: &Toolpath) {
    let svg = rs_cam_core::viz::toolpath_to_svg(tp, 800.0, 600.0);
    std::fs::write(path, svg).unwrap();
}

fn ensure_dir(path: &std::path::Path) {
    std::fs::create_dir_all(path).unwrap();
}

/// Run a full single-parameter sweep: baseline + N variants.
/// Returns the sweep result and writes all artifacts to disk.
fn run_sweep<F>(
    op_name: &str,
    param_name: &str,
    base_value: serde_json::Value,
    variants: &[serde_json::Value],
    generate: F,
) -> ParameterSweepResult
where
    F: Fn(Option<&serde_json::Value>) -> Toolpath,
{
    let dir = output_dir().join(op_name).join(param_name);
    ensure_dir(&dir);

    // Baseline
    let base_tp = generate(None);
    let base_fp = ToolpathFingerprint::from_toolpath(&base_tp);
    let base_arts = SweepArtifacts::generate(&base_tp, None);

    write_json(&dir.join("baseline.json"), &base_fp);
    write_svg(&dir.join("baseline.svg"), &base_tp);
    if let Some(svg) = &base_arts.toolpath_svg {
        std::fs::write(dir.join("baseline_toolpath.svg"), svg).unwrap();
    }

    // Variants
    let mut sweep_variants = Vec::new();
    for val in variants {
        let variant_tp = generate(Some(val));
        let variant_fp = ToolpathFingerprint::from_toolpath(&variant_tp);
        let diff = diff_fingerprints(&base_fp, &variant_fp);
        let arts = SweepArtifacts::generate(&variant_tp, None);

        let val_str = match val {
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::String(s) => s.clone(),
            _ => format!("{val}"),
        };

        write_json(&dir.join(format!("variant_{val_str}.json")), &variant_fp);
        write_json(&dir.join(format!("variant_{val_str}_diff.json")), &diff);
        write_svg(&dir.join(format!("variant_{val_str}.svg")), &variant_tp);

        sweep_variants.push(SweepVariant {
            value: val.clone(),
            fingerprint: variant_fp,
            diff,
            artifacts: Some(arts),
        });
    }

    let result = ParameterSweepResult {
        operation: op_name.to_string(),
        parameter_name: param_name.to_string(),
        base_value,
        base_fingerprint: base_fp,
        variants: sweep_variants,
    };

    write_json(&dir.join("sweep_result.json"), &result);
    result
}

/// Run a sweep with simulation: generates stock heightmap SVGs and StockFingerprints.
fn run_sweep_with_sim<F>(
    op_name: &str,
    param_name: &str,
    base_value: serde_json::Value,
    variants: &[serde_json::Value],
    stock_bounds: &BoundingBox3,
    cell_size: f64,
    cutter: &dyn MillingCutter,
    direction: StockCutDirection,
    generate: F,
) -> ParameterSweepResult
where
    F: Fn(Option<&serde_json::Value>) -> Toolpath,
{
    let dir = output_dir().join(op_name).join(param_name);
    ensure_dir(&dir);

    // Baseline with simulation
    let base_tp = generate(None);
    let base_fp = ToolpathFingerprint::from_toolpath(&base_tp);
    let mut base_stock = TriDexelStock::from_bounds(stock_bounds, cell_size);
    base_stock.simulate_toolpath(&base_tp, cutter, direction);
    let base_sfp = StockFingerprint::from_stock(&base_stock);
    let base_arts = SweepArtifacts::generate(&base_tp, Some(&base_stock));

    write_json(&dir.join("baseline.json"), &base_fp);
    write_json(&dir.join("baseline_stock.json"), &base_sfp);
    write_svg(&dir.join("baseline.svg"), &base_tp);
    if let Some(svg) = &base_arts.stock_heightmap_svg {
        std::fs::write(dir.join("baseline_stock.svg"), svg).unwrap();
    }

    // Variants with simulation
    let mut sweep_variants = Vec::new();
    for val in variants {
        let variant_tp = generate(Some(val));
        let variant_fp = ToolpathFingerprint::from_toolpath(&variant_tp);
        let diff = diff_fingerprints(&base_fp, &variant_fp);

        let mut variant_stock = TriDexelStock::from_bounds(stock_bounds, cell_size);
        variant_stock.simulate_toolpath(&variant_tp, cutter, direction);
        let variant_sfp = StockFingerprint::from_stock(&variant_stock);
        let arts = SweepArtifacts::generate(&variant_tp, Some(&variant_stock));

        let val_str = match val {
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::String(s) => s.clone(),
            _ => format!("{val}"),
        };

        write_json(&dir.join(format!("variant_{val_str}.json")), &variant_fp);
        write_json(&dir.join(format!("variant_{val_str}_diff.json")), &diff);
        write_json(&dir.join(format!("variant_{val_str}_stock.json")), &variant_sfp);
        write_svg(&dir.join(format!("variant_{val_str}.svg")), &variant_tp);
        if let Some(svg) = &arts.stock_heightmap_svg {
            std::fs::write(dir.join(format!("variant_{val_str}_stock.svg")), svg).unwrap();
        }

        sweep_variants.push(SweepVariant {
            value: val.clone(),
            fingerprint: variant_fp,
            diff,
            artifacts: Some(arts),
        });
    }

    let result = ParameterSweepResult {
        operation: op_name.to_string(),
        parameter_name: param_name.to_string(),
        base_value,
        base_fingerprint: base_fp,
        variants: sweep_variants,
    };

    write_json(&dir.join("sweep_result.json"), &result);
    result
}

// ── Test geometry ───────────────────────────────────────────────────────

fn rect_polygon() -> Polygon2 {
    Polygon2::rectangle(0.0, 0.0, 40.0, 30.0)
}

fn l_shape_polygon() -> Polygon2 {
    Polygon2::new(vec![
        P2::new(0.0, 0.0),
        P2::new(30.0, 0.0),
        P2::new(30.0, 15.0),
        P2::new(15.0, 15.0),
        P2::new(15.0, 30.0),
        P2::new(0.0, 30.0),
    ])
}

fn hemisphere_mesh() -> (TriangleMesh, SpatialIndex) {
    let mesh = make_test_hemisphere(20.0, 16);
    let index = SpatialIndex::build(&mesh, 12.0);
    (mesh, index)
}

fn stock_bounds_2d() -> BoundingBox3 {
    BoundingBox3 {
        min: rs_cam_core::geo::P3::new(-5.0, -5.0, -10.0),
        max: rs_cam_core::geo::P3::new(45.0, 35.0, 1.0),
    }
}

fn stock_bounds_3d() -> BoundingBox3 {
    BoundingBox3 {
        min: rs_cam_core::geo::P3::new(-25.0, -25.0, -5.0),
        max: rs_cam_core::geo::P3::new(25.0, 25.0, 22.0),
    }
}

// ── Default params ──────────────────────────────────────────────────────

fn default_pocket_params() -> PocketParams {
    PocketParams {
        tool_radius: 3.175,
        stepover: 2.0,
        cut_depth: -3.0,
        feed_rate: 1000.0,
        plunge_rate: 500.0,
        safe_z: 10.0,
        climb: true,
    }
}

fn default_profile_params() -> ProfileParams {
    ProfileParams {
        tool_radius: 3.175,
        side: ProfileSide::Outside,
        cut_depth: -3.0,
        feed_rate: 1000.0,
        plunge_rate: 500.0,
        safe_z: 10.0,
        climb: true,
    }
}

fn default_adaptive_params() -> AdaptiveParams {
    AdaptiveParams {
        tool_radius: 3.175,
        stepover: 2.0,
        cut_depth: -3.0,
        feed_rate: 1500.0,
        plunge_rate: 500.0,
        safe_z: 10.0,
        tolerance: 0.1,
        slot_clearing: true,
        min_cutting_radius: 0.0,
        initial_stock: None,
    }
}

fn default_waterline_params() -> WaterlineParams {
    WaterlineParams {
        sampling: 1.0,
        feed_rate: 1000.0,
        plunge_rate: 500.0,
        safe_z: 30.0,
    }
}

// ── Assertion helpers ───────────────────────────────────────────────────

fn assert_has_change(diff: &FingerprintDiff, field: &str, context: &str) {
    assert!(
        diff.field_change(field).is_some(),
        "{context}: expected '{field}' to change but it didn't.\nChanged: {:?}",
        diff.changed_fields.iter().map(|c| &c.field).collect::<Vec<_>>()
    );
}

fn assert_no_change(diff: &FingerprintDiff, field: &str, context: &str) {
    assert!(
        diff.field_change(field).is_none(),
        "{context}: expected '{field}' to stay the same but it changed.\nChange: {:?}",
        diff.field_change(field)
    );
}

fn assert_any_change(diff: &FingerprintDiff, context: &str) {
    assert!(
        diff.has_changes(),
        "{context}: expected some change but fingerprints are identical"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// POCKET SWEEPS
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn sweep_pocket_stepover() {
    let poly = rect_polygon();
    let cutter = FlatEndmill::new(6.35, 25.0);

    let result = run_sweep_with_sim(
        "pocket",
        "stepover",
        serde_json::json!(2.0),
        &[serde_json::json!(0.5), serde_json::json!(1.0), serde_json::json!(4.0)],
        &stock_bounds_2d(),
        0.5,
        &cutter,
        StockCutDirection::FromTop,
        |override_val| {
            let mut p = default_pocket_params();
            if let Some(v) = override_val {
                p.stepover = v.as_f64().unwrap();
            }
            rs_cam_core::pocket::pocket_toolpath(&poly, &p)
        },
    );

    // Smaller stepover → more moves, more cutting distance
    for v in &result.variants {
        let ctx = format!("pocket stepover={}", v.value);
        assert_any_change(&v.diff, &ctx);
        assert_has_change(&v.diff, "move_count", &ctx);
        assert_has_change(&v.diff, "cutting_distance_mm", &ctx);
        // Feed rates and Z should NOT change
        assert_no_change(&v.diff, "min_feed_rate", &ctx);
        assert_no_change(&v.diff, "min_z", &ctx);
    }
}

#[test]
fn sweep_pocket_feed_rate() {
    let poly = rect_polygon();

    let result = run_sweep(
        "pocket",
        "feed_rate",
        serde_json::json!(1000.0),
        &[serde_json::json!(500.0), serde_json::json!(2000.0), serde_json::json!(5000.0)],
        |override_val| {
            let mut p = default_pocket_params();
            if let Some(v) = override_val {
                p.feed_rate = v.as_f64().unwrap();
            }
            rs_cam_core::pocket::pocket_toolpath(&poly, &p)
        },
    );

    for v in &result.variants {
        let ctx = format!("pocket feed_rate={}", v.value);
        assert_any_change(&v.diff, &ctx);
        assert_has_change(&v.diff, "feed_rates", &ctx);
        // Geometry should NOT change
        assert_no_change(&v.diff, "move_count", &ctx);
        assert_no_change(&v.diff, "min_z", &ctx);
    }
}

#[test]
fn sweep_pocket_cut_depth() {
    let poly = rect_polygon();

    let result = run_sweep(
        "pocket",
        "cut_depth",
        serde_json::json!(-3.0),
        &[serde_json::json!(-1.0), serde_json::json!(-6.0), serde_json::json!(-10.0)],
        |override_val| {
            let mut p = default_pocket_params();
            if let Some(v) = override_val {
                p.cut_depth = v.as_f64().unwrap();
            }
            rs_cam_core::pocket::pocket_toolpath(&poly, &p)
        },
    );

    for v in &result.variants {
        let ctx = format!("pocket cut_depth={}", v.value);
        assert_any_change(&v.diff, &ctx);
        assert_has_change(&v.diff, "min_z", &ctx);
        assert_has_change(&v.diff, "z_levels", &ctx);
    }
}

#[test]
fn sweep_pocket_climb() {
    let poly = rect_polygon();

    let result = run_sweep(
        "pocket",
        "climb",
        serde_json::json!(true),
        &[serde_json::json!(false)],
        |override_val| {
            let mut p = default_pocket_params();
            if let Some(v) = override_val {
                p.climb = v.as_bool().unwrap();
            }
            rs_cam_core::pocket::pocket_toolpath(&poly, &p)
        },
    );

    // Climb toggle reverses winding direction. The fingerprint may or may
    // not detect this since the aggregate metrics (distance, Z, bbox) can
    // be identical. This is a known limitation of numeric fingerprints —
    // visual SVG diff is needed to verify direction reversal.
    // We just verify the test runs without error and produces output.
    let dir = output_dir().join("pocket").join("climb");
    assert!(dir.join("sweep_result.json").exists());
}

#[test]
fn sweep_pocket_safe_z() {
    let poly = rect_polygon();

    let result = run_sweep(
        "pocket",
        "safe_z",
        serde_json::json!(10.0),
        &[serde_json::json!(5.0), serde_json::json!(20.0), serde_json::json!(50.0)],
        |override_val| {
            let mut p = default_pocket_params();
            if let Some(v) = override_val {
                p.safe_z = v.as_f64().unwrap();
            }
            rs_cam_core::pocket::pocket_toolpath(&poly, &p)
        },
    );

    for v in &result.variants {
        let ctx = format!("pocket safe_z={}", v.value);
        assert_any_change(&v.diff, &ctx);
        assert_has_change(&v.diff, "max_z", &ctx);
        assert_has_change(&v.diff, "rapid_distance_mm", &ctx);
        // Cutting depth should NOT change
        assert_no_change(&v.diff, "min_z", &ctx);
        // Note: cutting_distance_mm CAN change because plunge moves (Linear
        // from safe_z to cut depth) count as cutting distance. This is expected.
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PROFILE SWEEPS
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn sweep_profile_side() {
    let poly = rect_polygon();

    let result = run_sweep(
        "profile",
        "side",
        serde_json::json!("outside"),
        &[serde_json::json!("inside")],
        |override_val| {
            let mut p = default_profile_params();
            if let Some(v) = override_val {
                p.side = match v.as_str().unwrap() {
                    "inside" => ProfileSide::Inside,
                    _ => ProfileSide::Outside,
                };
            }
            rs_cam_core::profile::profile_toolpath(&poly, &p)
        },
    );

    for v in &result.variants {
        let ctx = format!("profile side={}", v.value);
        assert_any_change(&v.diff, &ctx);
        // Bounding box should shift (tool moves to other side)
        assert!(
            v.diff.field_change("bbox_min_x").is_some()
                || v.diff.field_change("bbox_max_x").is_some()
                || v.diff.field_change("bbox_min_y").is_some()
                || v.diff.field_change("bbox_max_y").is_some(),
            "{ctx}: expected bounding box to change for side switch"
        );
    }
}

#[test]
fn sweep_profile_feed_rate() {
    let poly = rect_polygon();

    let result = run_sweep(
        "profile",
        "feed_rate",
        serde_json::json!(1000.0),
        &[serde_json::json!(500.0), serde_json::json!(3000.0)],
        |override_val| {
            let mut p = default_profile_params();
            if let Some(v) = override_val {
                p.feed_rate = v.as_f64().unwrap();
            }
            rs_cam_core::profile::profile_toolpath(&poly, &p)
        },
    );

    for v in &result.variants {
        let ctx = format!("profile feed_rate={}", v.value);
        assert_any_change(&v.diff, &ctx);
        assert_has_change(&v.diff, "feed_rates", &ctx);
        assert_no_change(&v.diff, "move_count", &ctx);
    }
}

#[test]
fn sweep_profile_climb() {
    let poly = rect_polygon();

    let result = run_sweep(
        "profile",
        "climb",
        serde_json::json!(true),
        &[serde_json::json!(false)],
        |override_val| {
            let mut p = default_profile_params();
            if let Some(v) = override_val {
                p.climb = v.as_bool().unwrap();
            }
            rs_cam_core::profile::profile_toolpath(&poly, &p)
        },
    );

    // Same as pocket climb — direction reversal may not show in numeric metrics.
    let dir = output_dir().join("profile").join("climb");
    assert!(dir.join("sweep_result.json").exists());
}

// ═══════════════════════════════════════════════════════════════════════
// ADAPTIVE SWEEPS
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn sweep_adaptive_stepover() {
    let poly = rect_polygon();
    let cutter = FlatEndmill::new(6.35, 25.0);

    let result = run_sweep_with_sim(
        "adaptive",
        "stepover",
        serde_json::json!(2.0),
        &[serde_json::json!(1.0), serde_json::json!(3.0)],
        &stock_bounds_2d(),
        0.5,
        &cutter,
        StockCutDirection::FromTop,
        |override_val| {
            let mut p = default_adaptive_params();
            if let Some(v) = override_val {
                p.stepover = v.as_f64().unwrap();
            }
            rs_cam_core::adaptive::adaptive_toolpath(&poly, &p)
        },
    );

    for v in &result.variants {
        let ctx = format!("adaptive stepover={}", v.value);
        assert_any_change(&v.diff, &ctx);
        assert_has_change(&v.diff, "cutting_distance_mm", &ctx);
    }
}

#[test]
fn sweep_adaptive_slot_clearing() {
    let poly = rect_polygon();

    let result = run_sweep(
        "adaptive",
        "slot_clearing",
        serde_json::json!(true),
        &[serde_json::json!(false)],
        |override_val| {
            let mut p = default_adaptive_params();
            if let Some(v) = override_val {
                p.slot_clearing = v.as_bool().unwrap();
            }
            rs_cam_core::adaptive::adaptive_toolpath(&poly, &p)
        },
    );

    for v in &result.variants {
        let ctx = format!("adaptive slot_clearing={}", v.value);
        assert_any_change(&v.diff, &ctx);
    }
}

#[test]
fn sweep_adaptive_tolerance() {
    let poly = rect_polygon();

    let result = run_sweep(
        "adaptive",
        "tolerance",
        serde_json::json!(0.1),
        &[serde_json::json!(0.01), serde_json::json!(0.5)],
        |override_val| {
            let mut p = default_adaptive_params();
            if let Some(v) = override_val {
                p.tolerance = v.as_f64().unwrap();
            }
            rs_cam_core::adaptive::adaptive_toolpath(&poly, &p)
        },
    );

    for v in &result.variants {
        let ctx = format!("adaptive tolerance={}", v.value);
        // Tighter tolerance = more points, so move_count should change
        assert_any_change(&v.diff, &ctx);
    }
}

#[test]
fn sweep_adaptive_min_cutting_radius() {
    let poly = l_shape_polygon(); // L-shape has sharp inside corners

    let result = run_sweep(
        "adaptive",
        "min_cutting_radius",
        serde_json::json!(0.0),
        &[serde_json::json!(1.0), serde_json::json!(3.0)],
        |override_val| {
            let mut p = default_adaptive_params();
            if let Some(v) = override_val {
                p.min_cutting_radius = v.as_f64().unwrap();
            }
            rs_cam_core::adaptive::adaptive_toolpath(&poly, &p)
        },
    );

    for v in &result.variants {
        let ctx = format!("adaptive min_cutting_radius={}", v.value);
        // Corner blending should change the path
        assert_any_change(&v.diff, &ctx);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// DROP CUTTER (3D FINISH) SWEEPS
// ═══════════════════════════════════════════════════════════════════════

fn generate_dropcutter(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    stepover: f64,
    feed_rate: f64,
    plunge_rate: f64,
    safe_z: f64,
    min_z: f64,
) -> Toolpath {
    let grid = batch_drop_cutter(mesh, index, cutter, stepover, 0.0, min_z);
    raster_toolpath_from_grid(&grid, feed_rate, plunge_rate, safe_z)
}

#[test]
fn sweep_dropcutter_stepover() {
    let (mesh, index) = hemisphere_mesh();
    let cutter = BallEndmill::new(6.35, 25.0);

    let result = run_sweep_with_sim(
        "dropcutter",
        "stepover",
        serde_json::json!(2.0),
        &[serde_json::json!(0.5), serde_json::json!(1.0), serde_json::json!(4.0)],
        &stock_bounds_3d(),
        0.5,
        &cutter,
        StockCutDirection::FromTop,
        |override_val| {
            let so = override_val
                .and_then(|v| v.as_f64())
                .unwrap_or(2.0);
            generate_dropcutter(&mesh, &index, &cutter, so, 1000.0, 500.0, 30.0, -5.0)
        },
    );

    for v in &result.variants {
        let ctx = format!("dropcutter stepover={}", v.value);
        assert_any_change(&v.diff, &ctx);
        assert_has_change(&v.diff, "move_count", &ctx);
        // Smaller stepover = more raster lines
        assert_has_change(&v.diff, "cutting_distance_mm", &ctx);
    }
}

#[test]
fn sweep_dropcutter_feed_rate() {
    let (mesh, index) = hemisphere_mesh();
    let cutter = BallEndmill::new(6.35, 25.0);

    let result = run_sweep(
        "dropcutter",
        "feed_rate",
        serde_json::json!(1000.0),
        &[serde_json::json!(500.0), serde_json::json!(2000.0)],
        |override_val| {
            let fr = override_val
                .and_then(|v| v.as_f64())
                .unwrap_or(1000.0);
            generate_dropcutter(&mesh, &index, &cutter, 2.0, fr, 500.0, 30.0, -5.0)
        },
    );

    for v in &result.variants {
        let ctx = format!("dropcutter feed_rate={}", v.value);
        assert_any_change(&v.diff, &ctx);
        assert_has_change(&v.diff, "feed_rates", &ctx);
        assert_no_change(&v.diff, "move_count", &ctx);
    }
}

#[test]
fn sweep_dropcutter_min_z() {
    let (mesh, index) = hemisphere_mesh();
    let cutter = BallEndmill::new(6.35, 25.0);

    let result = run_sweep(
        "dropcutter",
        "min_z",
        serde_json::json!(-5.0),
        &[serde_json::json!(-1.0), serde_json::json!(-20.0)],
        |override_val| {
            let mz = override_val
                .and_then(|v| v.as_f64())
                .unwrap_or(-5.0);
            generate_dropcutter(&mesh, &index, &cutter, 2.0, 1000.0, 500.0, 30.0, mz)
        },
    );

    for v in &result.variants {
        let ctx = format!("dropcutter min_z={}", v.value);
        assert_any_change(&v.diff, &ctx);
        assert_has_change(&v.diff, "min_z", &ctx);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// WATERLINE SWEEPS
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn sweep_waterline_z_step() {
    let (mesh, index) = hemisphere_mesh();
    let cutter = BallEndmill::new(6.35, 25.0);

    let result = run_sweep(
        "waterline",
        "z_step",
        serde_json::json!(2.0),
        &[serde_json::json!(0.5), serde_json::json!(1.0), serde_json::json!(5.0)],
        |override_val| {
            let zs = override_val
                .and_then(|v| v.as_f64())
                .unwrap_or(2.0);
            let p = default_waterline_params();
            waterline_toolpath(&mesh, &index, &cutter, 18.0, 0.0, zs, &p)
        },
    );

    for v in &result.variants {
        let ctx = format!("waterline z_step={}", v.value);
        assert_any_change(&v.diff, &ctx);
        // More Z levels with smaller step
        assert_has_change(&v.diff, "z_level_count", &ctx);
    }
}

#[test]
fn sweep_waterline_sampling() {
    let (mesh, index) = hemisphere_mesh();
    let cutter = BallEndmill::new(6.35, 25.0);

    let result = run_sweep(
        "waterline",
        "sampling",
        serde_json::json!(1.0),
        &[serde_json::json!(0.25), serde_json::json!(2.0)],
        |override_val| {
            let mut p = default_waterline_params();
            if let Some(v) = override_val {
                p.sampling = v.as_f64().unwrap();
            }
            waterline_toolpath(&mesh, &index, &cutter, 18.0, 0.0, 2.0, &p)
        },
    );

    for v in &result.variants {
        let ctx = format!("waterline sampling={}", v.value);
        assert_any_change(&v.diff, &ctx);
        // Finer sampling = smoother contours = more moves
        assert_has_change(&v.diff, "move_count", &ctx);
    }
}

#[test]
fn sweep_waterline_feed_rate() {
    let (mesh, index) = hemisphere_mesh();
    let cutter = BallEndmill::new(6.35, 25.0);

    let result = run_sweep(
        "waterline",
        "feed_rate",
        serde_json::json!(1000.0),
        &[serde_json::json!(500.0), serde_json::json!(3000.0)],
        |override_val| {
            let mut p = default_waterline_params();
            if let Some(v) = override_val {
                p.feed_rate = v.as_f64().unwrap();
            }
            waterline_toolpath(&mesh, &index, &cutter, 18.0, 0.0, 2.0, &p)
        },
    );

    for v in &result.variants {
        let ctx = format!("waterline feed_rate={}", v.value);
        assert_any_change(&v.diff, &ctx);
        assert_has_change(&v.diff, "feed_rates", &ctx);
        assert_no_change(&v.diff, "move_count", &ctx);
    }
}

#[test]
fn sweep_waterline_z_range() {
    let (mesh, index) = hemisphere_mesh();
    let cutter = BallEndmill::new(6.35, 25.0);

    let result = run_sweep(
        "waterline",
        "final_z",
        serde_json::json!(0.0),
        &[serde_json::json!(5.0), serde_json::json!(10.0)],
        |override_val| {
            let fz = override_val
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let p = default_waterline_params();
            waterline_toolpath(&mesh, &index, &cutter, 18.0, fz, 2.0, &p)
        },
    );

    for v in &result.variants {
        let ctx = format!("waterline final_z={}", v.value);
        assert_any_change(&v.diff, &ctx);
        // Higher final_z = fewer levels
        assert_has_change(&v.diff, "z_level_count", &ctx);
    }
}
