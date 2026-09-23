//! FM3 — one depth de-rate for the feed and for the chipload band.
//!
//! Feeds matrix ruling R3 (2026-09-23). The operator ruled: "yes, delete.
//! 1.5 is arbitrary." Onsrud, Freud and Amana print one depth rule at three
//! points: 1 x D full, 2 x D minus 25 %, 3 x D minus 50 %. Before R3 the
//! engine held two readings of it. The feed used a step (0.75 at 1.5 x D,
//! 0.45 above 3 x D) and the band used a linear scale (0.875 at 1.5 x D,
//! 0.50 above 3 x D). The band that `feeds::calculate` returned also carried
//! no de-rate on a roughing cell with no depth hint (EVIDENCE 5.2-6, 5.2-7).
//!
//! This sentry pins the ruled state:
//!
//! - (a) the feed multiplier and the band multiplier are the same number at
//!   1.5, 2.5 and 4 x D, in the functions and in one `calculate` result;
//! - (b) at 1, 2 and 3 x D they are the printed 1.00, 0.75 and 0.50;
//! - (c) above 3 x D the scale is 0.50 and the static checks raise the
//!   `feeds.depth_beyond_published_table` Caution;
//! - (d) the id `geom.dpp_over_1_5x_diameter` is gone from `ids.rs`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::{ToolConfig, ToolId, ToolType};
use rs_cam_core::diagnostics::adapters::from_static_checks::diagnostics_from_static_checks;
use rs_cam_core::diagnostics::{Severity, ids};
use rs_cam_core::feeds::geometry::{depth_tier_multiplier, doc_derating_scale};
use rs_cam_core::feeds::{
    FeedsInput, OperationFamily, PassRole, SetupContext, SpindleStrategy, ToolGeometryHint,
    calculate, embedded_vendor_lut,
};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

/// Ø9.525 (3/8 in). Feeds matrix R5 (2026-09-23) made the 6 mm flat adaptive
/// cell resolve to a printed single-value Spektra row with no band; the 3/8 in
/// softwood adaptive cell resolves to a printed Onsrud 60-series range, so the
/// band arm below has a two-bound row to read.
const D_MM: f64 = 9.525;
const TOL: f64 = 1e-12;

/// (a) and (b) on the functions: the feed's multiplier IS the band's scale.
#[test]
fn the_feed_and_the_band_read_one_scale_fm3() {
    for ratio in [1.5, 2.5, 4.0] {
        let feed = depth_tier_multiplier(ratio * D_MM, D_MM);
        let band = doc_derating_scale(ratio);
        assert!(
            (feed - band).abs() < TOL,
            "at {ratio} x D the feed multiplier {feed} and the band scale {band} differ"
        );
    }
    for (ratio, printed) in [(1.0, 1.00), (2.0, 0.75), (3.0, 0.50)] {
        let feed = depth_tier_multiplier(ratio * D_MM, D_MM);
        let band = doc_derating_scale(ratio);
        assert!(
            (feed - printed).abs() < TOL && (band - printed).abs() < TOL,
            "at {ratio} x D the vendors print {printed}; feed {feed}, band {band}"
        );
    }
}

/// (a) on one `calculate` result: a 2D adaptive rough with NO depth hint.
/// The band it returns is the raw row band times the feed's own multiplier.
#[test]
fn the_returned_band_carries_the_feed_depth_derate_fm3() {
    let machine = MachineProfile::generic_wood_router();
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let input = FeedsInput {
        tool_diameter: D_MM,
        flute_count: 2,
        flute_length: 30.0,
        shank_diameter: None,
        tool_geometry: ToolGeometryHint::Flat,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Adaptive,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: Some(embedded_vendor_lut()),
        setup: SetupContext::default(),
        spindle_strategy: SpindleStrategy::MatchChart,
    };
    let result = calculate(&input);
    let ratio = result.axial_depth_mm / D_MM;
    // Non-vacuity: the cell must sit between printed points, where the two
    // old readings disagreed.
    assert!(
        ratio > 1.0 && ratio < 2.0,
        "fixture depth {:.3} mm ({ratio:.3} x D) is not between 1 and 2 x D",
        result.axial_depth_mm
    );
    let row = result
        .matched_lut_row
        .as_ref()
        .expect("the fixture needs a matched vendor row");
    let raw_max = row.chip_load_max_mm.expect("the row publishes a band max");
    let raw_min = row.chip_load_min_mm.expect("the row publishes a band min");
    let band = result
        .chipload_bounds
        .expect("the fixture needs a returned band");
    let band_scale_max = band.max_mm_per_tooth / raw_max;
    let band_scale_min = band.min_mm_per_tooth / raw_min;
    let feed_scale = result.derates.depth_tier;
    assert!(
        (band_scale_max - feed_scale).abs() < 1e-9 && (band_scale_min - feed_scale).abs() < 1e-9,
        "band scale {band_scale_max}/{band_scale_min} and feed depth de-rate {feed_scale} differ \
         at {ratio:.3} x D"
    );
    assert!(
        (feed_scale - doc_derating_scale(ratio)).abs() < 1e-9,
        "feed de-rate {feed_scale} is not doc_derating_scale({ratio:.3}) = {}",
        doc_derating_scale(ratio)
    );
    assert!(
        feed_scale < 1.0,
        "non-vacuity: the band must carry a de-rate"
    );
}

fn pocket(dpp: f64) -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        depth: 60.0,
        depth_per_pass: dpp,
        ..PocketConfig::default()
    })
}

fn tool() -> ToolConfig {
    ToolConfig {
        diameter: D_MM,
        cutting_length: 60.0,
        ..ToolConfig::new_default(ToolId(1), ToolType::EndMill)
    }
}

fn beyond_table_severities(dpp: f64) -> Vec<Severity> {
    diagnostics_from_static_checks(ToolpathId(1), &pocket(dpp), &tool(), None)
        .into_iter()
        .filter(|d| d.id.as_str() == ids::FEEDS_DEPTH_BEYOND_PUBLISHED_TABLE)
        .map(|d| d.severity)
        .collect()
}

/// (c) above 3 x D the scale holds 0.50 and the Caution fires; at 3 x D it
/// does not.
#[test]
fn above_the_table_holds_the_last_point_and_cautions_fm3() {
    assert!((doc_derating_scale(4.0) - 0.50).abs() < TOL);
    assert!((depth_tier_multiplier(4.0 * D_MM, D_MM) - 0.50).abs() < TOL);
    assert_eq!(
        beyond_table_severities(4.0 * D_MM),
        vec![Severity::Caution],
        "at 4 x D the static checks must raise one Caution"
    );
    assert!(
        beyond_table_severities(3.0 * D_MM).is_empty(),
        "at exactly 3 x D the depth is on the printed table"
    );
    assert!(
        ids::ALL.contains(&ids::FEEDS_DEPTH_BEYOND_PUBLISHED_TABLE),
        "the new id is in the registry"
    );
}

/// (d) the deleted hint id no longer exists.
#[test]
fn the_one_and_a_half_diameter_hint_id_is_gone_fm3() {
    let ids_src = include_str!("../src/diagnostics/ids.rs");
    // Non-vacuity: the file read is the id registry.
    assert!(
        ids_src.contains("\"geom.dpp_exceeds_cutting_length\""),
        "the anchor id is missing; this test no longer reads the registry"
    );
    assert!(
        !ids_src.contains("dpp_over_1_5x_diameter"),
        "`geom.dpp_over_1_5x_diameter` is back in ids.rs (ruling R3 deleted it)"
    );
    assert!(!ids::ALL.contains(&"geom.dpp_over_1_5x_diameter"));
}
