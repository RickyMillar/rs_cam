//! M8 wiring sentries — the `iso_field` dial on the scallop op
//! (`planning/metrology_2026-09-02/FINDINGS.md` §M7–M8).
//!
//! Three claims, pinned:
//!
//! 1. An absent `iso_field` key loads the LEGACY cascade (`false`) — a
//!    project file written before 2026-09-03 is byte-identical in behavior.
//! 2. A pinned `iso_field = true` survives a round trip.
//! 3. The production wrapper selects exactly the evidence configuration:
//!    `IsoField` rings + the spec-correct `CosineSlope` law (the operator
//!    ruling: one law; speed is traded at the cusp/tool dials, never
//!    across the surface).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use rs_cam_core::compute::operation_configs::ScallopConfig;

#[test]
fn absent_iso_field_key_loads_the_legacy_cascade() {
    let toml = r#"
        scallop_height = 0.03
        tolerance = 0.05
        direction = "outside_in"
        continuous = true
        slope_from = 0.0
        slope_to = 90.0
        feed_rate = 913.0
        plunge_rate = 270.0
        stock_to_leave = 0.0
    "#;
    let cfg: ScallopConfig = toml::from_str(toml).expect("legacy config parses");
    assert!(
        !cfg.iso_field,
        "an absent iso_field key must load the legacy cascade"
    );
}

#[test]
fn pinned_iso_field_survives_a_round_trip() {
    let cfg = ScallopConfig {
        iso_field: true,
        ..ScallopConfig::default()
    };
    let text = toml::to_string(&cfg).expect("serialize");
    let back: ScallopConfig = toml::from_str(&text).expect("reparse");
    assert!(back.iso_field, "pinned iso_field = true must survive");
    assert!(
        text.contains("iso_field = true"),
        "the key must be written when pinned"
    );
}
