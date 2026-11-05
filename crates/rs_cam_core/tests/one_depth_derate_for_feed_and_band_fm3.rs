//! FM3 — the vendor depth table holds its last printed point above 3 x D.
//!
//! Feeds matrix ruling R3 (2026-09-23) made one depth de-rate for the feed
//! and the band. Feeds contract P11b (L6, chip first) retired that claim:
//! neither the feed nor the band takes the table now; the table is a limit
//! of the envelope. This sentry keeps the claims that still hold:
//!
//! - (c) above 3 x D the scale is 0.50 and the static checks raise the
//!   `feeds.depth_beyond_published_table` Caution;
//! - (d) the id `geom.dpp_over_1_5x_diameter` is gone from `ids.rs`.

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::{ToolConfig, ToolId, ToolType};
use rs_cam_core::diagnostics::adapters::from_static_checks::diagnostics_from_static_checks;
use rs_cam_core::diagnostics::{Severity, ids};
use rs_cam_core::feeds::geometry::doc_derating_scale;
use rs_cam_core::ids::ToolpathId;

/// Ø9.525 (3/8 in), the tool of the static-check arm.
const D_MM: f64 = 9.525;
const TOL: f64 = 1e-12;

// (a) and (b) RETIRED in feeds contract P11b (L6, chip first): neither the
// feed nor the band takes the depth table now. The table is the envelope
// limit "vendor depth ceiling (width not printed)" (sentries
// `the_depth_table_no_longer_moves_the_chip_p11b`,
// `the_vendor_depth_table_is_a_limit_that_warns_p11b`).

fn pocket(dpp: f64) -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        floor: rs_cam_core::compute::floor::Floor::top(60.0),
        depth_per_pass: Some(dpp),
        ..PocketConfig::default()
    })
}

fn tool() -> ToolConfig {
    ToolConfig::builder(ToolId::from_raw(1), ToolType::EndMill)
        .diameter(D_MM)
        .flute_mm(60.0)
        .build()
}

fn beyond_table_severities(dpp: f64) -> Vec<Severity> {
    diagnostics_from_static_checks(ToolpathId::from_raw(1), &pocket(dpp), &tool(), None)
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
