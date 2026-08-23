//! F2 sentry — the finish planner's island-filter dials are reachable from
//! `UnifiedFinishConfig`, and reaching for none of them changes nothing.
//!
//! Background (`planning/multitool_2026-08-23/T3_FINDINGS.md` §4.e): the
//! operator's asked-for dials — "min island 50 mm²", "merge radius 5 mm" —
//! have existed all along as `FinishPlannerParams::{min_region_area_mm2,
//! close_radius_mm, hysteresis_deg}`, derived per tool by `for_tool` and
//! reachable from no config, no TOML key and no MCP param. F2 exposes them as
//! three `Option<f64>` overrides.
//!
//! The contract, and what each test pins:
//!
//! * `None` = today's `for_tool` derivation, **byte-identical output**. A
//!   project written before F2 must plan exactly as it did — that is
//!   `none_reproduces_the_for_tool_derivation_exactly` plus the TOML
//!   round-trip, which also pins that a saved project grows no new keys.
//! * `Some(v)` overrides that ONE dial and leaves the other two derived —
//!   `some_overrides_only_the_dial_it_names`.
//! * The override actually reaches `decompose`, measured on its own output
//!   rather than asserted on the struct — `the_min_area_override_reaches_
//!   decompose`.
//! * The schema publishes them, so MCP and the GUI can see them at all.
//!
//! Report-free: none of this is a measurement of a part, and no gate consumes
//! any of it.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::operation_configs::UnifiedFinishConfig;
use rs_cam_core::finish_planner::{FinishBand, FinishPlannerParams, PlannedRegions, decompose};
use rs_cam_core::geo::V3;
use rs_cam_core::slope::SlopeMap;

/// A Ø1-tip tapered ball's cusp radius — the case `for_tool`'s doc warns
/// about, where the envelope radius would be 3.0 and every derived dial would
/// be off by 36× in area.
const CUSP_MM: f64 = 0.5;

/// The params block of a project file written BEFORE F2 existed: every
/// required key, none of the three new ones.
const PRE_F2_PARAMS_TOML: &str = r#"
steep_threshold_deg = 45.0
waterline_threshold_deg = 75.0
overlap_mm = 2.0
scallop_height = 0.03
tolerance = 0.05
raster_stepover = 0.6
z_step = 0.5
sampling = 0.3
stock_to_leave = 0.0
feed_rate = 1062.0
plunge_rate = 180.0
pencil_claims = false
min_rest_depth_mm = 0.02
claims_reference = "auto"
territory_clip = false
intra_region_hookup_mm = 6.0
crease_hookup_mm = 5.0
"#;

fn pre_f2_config() -> UnifiedFinishConfig {
    toml::from_str(PRE_F2_PARAMS_TOML).expect("a pre-F2 params block must still load")
}

/// (a) A project file that never heard of these dials loads them as `None`,
/// and saving it back adds no keys.
///
/// The second half is the one worth stating: `skip_serializing_if` is what
/// keeps a re-saved project byte-comparable with the one on disk. A plain
/// `#[serde(default)]` would have written three `null`-equivalent keys into
/// every project in the repo the first time it was opened.
#[test]
fn a_pre_f2_project_loads_as_none_and_saves_without_the_keys() {
    let cfg = pre_f2_config();

    assert_eq!(cfg.min_region_area_mm2, None);
    assert_eq!(cfg.close_radius_mm, None);
    assert_eq!(cfg.hysteresis_deg, None);

    let round_tripped = toml::to_string(&cfg).expect("serialize");
    for key in ["min_region_area_mm2", "close_radius_mm", "hysteresis_deg"] {
        assert!(
            !round_tripped.contains(key),
            "an untouched dial must not grow a key on save; got:\n{round_tripped}"
        );
    }

    // And the values that WERE on disk survive, so this is a round-trip and
    // not merely a defaulted struct.
    let reloaded: UnifiedFinishConfig = toml::from_str(&round_tripped).expect("reload");
    assert!((reloaded.raster_stepover - 0.6).abs() < 1e-12);
    assert!((reloaded.overlap_mm - 2.0).abs() < 1e-12);
    assert_eq!(reloaded.min_region_area_mm2, None);
    assert_eq!(reloaded.close_radius_mm, None);
    assert_eq!(reloaded.hysteresis_deg, None);
}

/// (b) `None` ≡ the pre-F2 planner, dial for dial.
///
/// The right-hand side is the exact expression `generate_unified_finish`
/// carried before F2: `for_tool(cusp)` with three assignments on top. If the
/// override plumbing ever leaks a value into an unset dial, the emitted
/// toolpath moves, and this is where that shows up first.
#[test]
fn none_reproduces_the_for_tool_derivation_exactly() {
    let cfg = pre_f2_config();
    let got = cfg.planner_params(CUSP_MM);

    let mut expected = FinishPlannerParams::for_tool(CUSP_MM);
    expected.steep_threshold_deg = cfg.steep_threshold_deg;
    expected.waterline_threshold_deg = cfg.waterline_threshold_deg;
    expected.overlap_mm = cfg.overlap_mm;

    assert_planner_eq(&got, &expected);

    // Spelled out, so a reader does not have to trust `for_tool` either:
    // these are the formulas the field docs promise.
    let want_area = (2.0 * CUSP_MM).powi(2) * 4.0;
    let area = got.min_region_area_mm2;
    assert!((area - want_area).abs() < 1e-12, "{area}");
    assert!((got.close_radius_mm - CUSP_MM * 0.5).abs() < 1e-12);
    assert!((got.hysteresis_deg - 10.0).abs() < 1e-12);
}

/// (c) `Some(v)` moves exactly one dial.
#[test]
fn some_overrides_only_the_dial_it_names() {
    let derived = pre_f2_config().planner_params(CUSP_MM);
    let d_area = derived.min_region_area_mm2;
    let d_close = derived.close_radius_mm;
    let d_hyst = derived.hysteresis_deg;

    let mut cfg = pre_f2_config();
    cfg.min_region_area_mm2 = Some(50.0);
    let got = cfg.planner_params(CUSP_MM);
    assert!((got.min_region_area_mm2 - 50.0).abs() < 1e-12);
    assert!((got.close_radius_mm - d_close).abs() < 1e-12);
    assert!((got.hysteresis_deg - d_hyst).abs() < 1e-12);

    let mut cfg = pre_f2_config();
    cfg.close_radius_mm = Some(5.0);
    let got = cfg.planner_params(CUSP_MM);
    assert!((got.close_radius_mm - 5.0).abs() < 1e-12);
    assert!((got.min_region_area_mm2 - d_area).abs() < 1e-12);
    assert!((got.hysteresis_deg - d_hyst).abs() < 1e-12);

    let mut cfg = pre_f2_config();
    cfg.hysteresis_deg = Some(2.5);
    let got = cfg.planner_params(CUSP_MM);
    assert!((got.hysteresis_deg - 2.5).abs() < 1e-12);
    assert!((got.min_region_area_mm2 - d_area).abs() < 1e-12);
    assert!((got.close_radius_mm - d_close).abs() < 1e-12);

    // All three at once, since a per-field `if let` chain is exactly the
    // shape that can drop the last one.
    let mut cfg = pre_f2_config();
    cfg.min_region_area_mm2 = Some(1.0);
    cfg.close_radius_mm = Some(0.25);
    cfg.hysteresis_deg = Some(4.0);
    let got = cfg.planner_params(CUSP_MM);
    assert!((got.min_region_area_mm2 - 1.0).abs() < 1e-12);
    assert!((got.close_radius_mm - 0.25).abs() < 1e-12);
    assert!((got.hysteresis_deg - 4.0).abs() < 1e-12);
}

/// (d) The override reaches `decompose` and changes what it plans.
///
/// Asserted on the planner's OUTPUT, not on the struct: the standing rule
/// here is that a dial is not exposed until something downstream demonstrably
/// moves (T4's whole lesson — a config value that reaches no consumer reads
/// exactly like one that does).
///
/// The fixture is a 40 × 40 mm flat plane at 1 mm cells carrying one 6 × 6 mm
/// patch at 60°. At the Ø6-ball derivation (`for_tool(3.0)`,
/// `min_region_area_mm2` = 144 mm²) that 36 mm² patch is well below the floor
/// and is absorbed into the surrounding shallow band; at 1 mm² it survives as
/// its own mid-steep region.
#[test]
fn the_min_area_override_reaches_decompose() {
    let slope_map = plane_with_a_small_steep_patch();
    let covered = vec![true; slope_map.rows * slope_map.cols];

    let mut cfg = UnifiedFinishConfig::default();
    // Ø6 ball nose — the common finishing tool, and the radius `for_tool`'s
    // own `Default` uses.
    let derived = cfg.planner_params(3.0);
    assert!(
        (derived.min_region_area_mm2 - 144.0).abs() < 1e-9,
        "fixture assumes the derived floor is 144 mm²; got {}",
        derived.min_region_area_mm2
    );

    let absorbed = decompose(&slope_map, &covered, &[], &derived);
    assert!(
        absorbed.stats.absorbed_regions > 0,
        "the derived floor must absorb the patch: {:?}",
        absorbed.stats
    );
    assert_eq!(
        mid_steep_regions(&absorbed),
        0,
        "an absorbed patch leaves no mid-steep region"
    );

    cfg.min_region_area_mm2 = Some(1.0);
    let kept = decompose(&slope_map, &covered, &[], &cfg.planner_params(3.0));
    assert_eq!(
        kept.stats.absorbed_regions, 0,
        "at a 1 mm² floor nothing is small enough to absorb: {:?}",
        kept.stats
    );
    assert!(
        mid_steep_regions(&kept) >= 1,
        "the override must let the patch survive as its own region: {:?}",
        kept.stats
    );
}

/// (e) The three dials are on the published schema, optional, defaulting to
/// null, and each carries the prose an agent needs (the cusp-vs-envelope
/// footgun has no other surface).
#[test]
fn the_schema_publishes_the_three_dials() {
    let schema = OperationConfig::schema_for_type(OperationType::UnifiedFinish);
    for name in ["min_region_area_mm2", "close_radius_mm", "hysteresis_deg"] {
        let param = schema
            .params
            .iter()
            .find(|p| p.name == name)
            .unwrap_or_else(|| panic!("schema must publish `{name}`"));
        assert!(param.optional, "`{name}` must be optional");
        assert_eq!(
            param.default,
            serde_json::Value::Null,
            "`{name}` must default to null = derive from the tool"
        );
        assert!(
            param.description.is_some(),
            "`{name}` must carry its unit and its derivation on the wire"
        );
    }
}

// ── fixtures ─────────────────────────────────────────────────────────────

/// Exhaustive by construction: `FinishPlannerParams` derives `Debug` and
/// every field is a plain scalar, so the rendered struct IS the dial set. A
/// per-field comparison would need updating whenever a dial is added, and the
/// one it forgot is exactly the one this sentry exists to catch.
fn assert_planner_eq(got: &FinishPlannerParams, expected: &FinishPlannerParams) {
    assert_eq!(
        format!("{got:?}"),
        format!("{expected:?}"),
        "an unset override must leave the derivation untouched, dial for dial"
    );
}

fn mid_steep_regions(planned: &PlannedRegions) -> usize {
    planned
        .regions
        .iter()
        .filter(|r| r.band == FinishBand::MidSteep)
        .count()
}

/// 40 × 40 cells at 1 mm, everything horizontal except a 6 × 6 patch at 60°
/// well inside the border (step 0 erodes coverage by one cell).
fn plane_with_a_small_steep_patch() -> SlopeMap {
    let rows = 40;
    let cols = 40;
    let total = rows * cols;
    let mut angles = vec![0.0_f64; total];
    for r in 17..23 {
        for c in 17..23 {
            angles[r * cols + c] = 60.0_f64.to_radians();
        }
    }
    SlopeMap {
        // Neither of these is read by `decompose` (no creases are supplied
        // and extraction works off the label grid), but the struct is
        // all-public and carries no constructor for a synthetic grid.
        normals: vec![V3::new(0.0, 0.0, 1.0); total],
        angles,
        curvatures: vec![0.0; total],
        rows,
        cols,
        origin_x: 0.0,
        origin_y: 0.0,
        cell_size: 1.0,
    }
}
