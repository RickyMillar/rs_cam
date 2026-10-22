//! LH-1, the viz half — every viz surface that shows an air-cut percentage
//! names its denominator.
//!
//! W14a (F171) moved the three viz rows of
//! `rs_cam_core/tests/air_cut_denominators_lh1.rs`
//! (`no_surface_divides_air_cut_by_a_runtime_without_naming_it`) here, so
//! the viz gate runs them. The core file keeps the CLI and core rows.
//!
//! A source-text check is the honest instrument: the sites are `format!`
//! strings inside egui closures and `serde_json::json!` literals, none of
//! which a unit test reaches. If this test fails because a file moved,
//! retarget it; do not delete it.

#![expect(
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

#[test]
fn no_viz_surface_divides_air_cut_by_a_runtime_without_naming_it_lh1() {
    let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

    // (file, what its air-cut label must say)
    let surfaces: [(&str, &[&str]); 3] = [
        (
            "src/ui/sim_diagnostics.rs",
            &["% of total runtime)", "{pct:.0}% of total runtime"],
        ),
        // F181: the MCP `get_diagnostics` reply is the serialized core
        // report, so its denominators are the names of the core fields
        // (the core file's `diagnostics_types.rs` row).
        (
            "src/controller/events/compute.rs",
            &["serde_json::to_value(&core_diagnostics)"],
        ),
        (
            "src/app/mcp/simulation.rs",
            &[
                "air_cut_pct_of_total_runtime",
                "air_cut_pct_of_cutting_time",
            ],
        ),
    ];

    for (rel, required) in surfaces {
        let path = crate_root.join(rel);
        let src = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("LH-1 sentry needs retargeting: {rel}: {e}"));

        assert!(
            !src.contains("air_cut_time_s / "),
            "{rel} divides air_cut_time_s by hand — use \
             AirCutRatios::air_cut_pct_of_total_runtime / _of_cutting_time so the \
             denominator is named at the call site (MEASUREMENT_DOMAINS.md LH-1)"
        );
        for needle in required {
            assert!(
                src.contains(needle),
                "{rel} must name its air-cut denominator; expected to find {needle:?}"
            );
        }
    }
}
