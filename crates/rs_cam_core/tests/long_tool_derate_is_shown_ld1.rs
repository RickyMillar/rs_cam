//! **LD1 — the long-tool de-rate is visible when it fires.**
//!
//! Ruling R4 WP1 (2026-09-23,
//! `planning/feeds_matrix_2026-09-23/R4_AGGRESSIVENESS_SPEC.md` §3.2): the
//! long-tool (L/D) de-rate "may stay as a minor cut but must show in the UI
//! when it fires". The calculator multiplies the feed by 0.88 above 4 x D
//! stickout and by 0.75 above 6 x D. The rule is a repo rule with no source.
//! Until WP1 only a hover sentence named it.
//!
//! Ruling R4 Q7 (2026-09-24) moved the share into the aggressiveness dial:
//! Suggest pass 6b aims at `k_eff = aggressiveness x share`, so a long tool
//! gets a smaller engagement, not a thinner chip. The feed does not take the
//! share any more. The warning stays as the visible record, reworded
//! "load target x0.75".
//!
//! What this sentry pins:
//!
//! 1. `FeedsWarning::LongToolDerate` fires if and only if
//!    `derates.ld_overhang < 1.0`, over stickouts on both sides of both
//!    thresholds and with no stickout at all.
//! 2. The warning carries the share, the stickout, the diameter and the
//!    ratio.
//! 3. The diagnostics adapter turns it into the Info finding
//!    `feeds.long_tool_derate`, and the id is in `ids::ALL`.
//! 4. The factor on the warning is the share in the derate record, and the
//!    feed is the same at every stickout (Q7: no feed cut).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::diagnostics::adapters::from_feeds::diagnostics_from_feeds_result;
use rs_cam_core::diagnostics::{Severity, ids};
use rs_cam_core::feeds::{
    FeedsInput, FeedsResult, FeedsWarning, OperationFamily, PassRole, SetupContext,
    SpindleStrategy, ToolGeometryHint, WorkholdingRigidity, calculate, embedded_vendor_lut,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

const DIAMETER_MM: f64 = 6.0;

/// Ø6 flat 2F pocket rough in white oak, with the given stickout.
fn pocket(stickout_mm: Option<f64>) -> FeedsResult {
    let machine = MachineProfile::generic_wood_router();
    let material = Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    };
    calculate(&FeedsInput {
        tool_diameter: DIAMETER_MM,
        flute_count: 2,
        flute_length: 22.0,
        shank_diameter: Some(DIAMETER_MM),
        tool_geometry: ToolGeometryHint::Flat,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Pocket,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: Some(3.0),
        radial_width_mm: Some(2.0),
        target_scallop_mm: None,
        vendor_lut: Some(embedded_vendor_lut()),
        setup: SetupContext {
            tool_overhang_mm: stickout_mm,
            workholding_rigidity: WorkholdingRigidity::Medium,
        },
        spindle_strategy: SpindleStrategy::MatchChart,
    })
}

/// The warning's fields, when it fired.
fn long_tool_warning(result: &FeedsResult) -> Option<(f64, f64, f64, f64)> {
    result.warnings.iter().find_map(|w| match w {
        FeedsWarning::LongToolDerate {
            stickout_mm,
            diameter_mm,
            ratio,
            factor,
        } => Some((*stickout_mm, *diameter_mm, *ratio, *factor)),
        _ => None,
    })
}

#[test]
fn the_warning_fires_exactly_when_the_feed_takes_the_derate_ld1() {
    // No stickout, 3 x D, 5 x D, 7.5 x D (the GUI default 45 mm on Ø6), 10 x D.
    let cases: [(Option<f64>, f64); 5] = [
        (None, 1.0),
        (Some(18.0), 1.0),
        (Some(30.0), 0.88),
        (Some(45.0), 0.75),
        (Some(60.0), 0.75),
    ];
    let mut fired = 0usize;
    let no_stickout_feed = pocket(None).feed_rate_mm_min;
    for (stickout, expected_factor) in cases {
        let result = pocket(stickout);
        assert!(
            (result.feed_rate_mm_min - no_stickout_feed).abs() <= no_stickout_feed * 1e-12,
            "stickout {stickout:?}: the feed {} moved from the no-stickout feed \
             {no_stickout_feed}. Ruling R4 Q7: the long-tool share is a load target, \
             not a feed factor.",
            result.feed_rate_mm_min
        );
        let ld = result.derates.ld_overhang;
        assert!(
            (ld - expected_factor).abs() < 1e-12,
            "stickout {stickout:?}: the feed took L/D factor {ld}, expected \
             {expected_factor}. This sentry pins visibility, not the value; if the \
             rule moved on purpose, update the table."
        );
        let warning = long_tool_warning(&result);
        assert_eq!(
            warning.is_some(),
            ld < 1.0,
            "stickout {stickout:?}: the warning must fire if and only if the feed \
             took the de-rate (ld_overhang {ld}). Warnings: {:?}",
            result.warnings
        );
        if let Some((stickout_mm, diameter_mm, ratio, factor)) = warning {
            fired += 1;
            let stickout = stickout.expect("the warning fired, so a stickout was given");
            assert_eq!(stickout_mm, stickout);
            assert_eq!(diameter_mm, DIAMETER_MM);
            assert!(
                (ratio - stickout / DIAMETER_MM).abs() < 1e-12,
                "the ratio {ratio} is not stickout / diameter"
            );
            assert_eq!(
                factor, ld,
                "the warning's factor must be the factor in the derate record"
            );
        }
    }
    assert_eq!(
        fired, 3,
        "non-vacuity: three of the five cases take the de-rate"
    );
}

#[test]
fn the_diagnostic_is_an_info_finding_with_its_own_id_ld1() {
    assert!(
        ids::ALL.contains(&ids::FEEDS_LONG_TOOL_DERATE),
        "the id must be registered in ids::ALL"
    );
    assert_eq!(ids::FEEDS_LONG_TOOL_DERATE, "feeds.long_tool_derate");

    let result = pocket(Some(45.0));
    let diags = diagnostics_from_feeds_result(rs_cam_core::ids::ToolpathId(0), &result);
    let finding = diags
        .iter()
        .find(|d| d.id.as_str() == ids::FEEDS_LONG_TOOL_DERATE)
        .unwrap_or_else(|| {
            panic!(
                "no feeds.long_tool_derate finding. Findings: {:?}",
                diags.iter().map(|d| d.id.as_str()).collect::<Vec<_>>()
            )
        });
    assert_eq!(finding.severity, Severity::Info);
    assert!(
        finding.message.contains("load target x0.75") && finding.message.contains("7.5 x D"),
        "the finding must carry the factor and the ratio: {:?}",
        finding.message
    );

    let short = pocket(Some(18.0));
    assert!(
        diagnostics_from_feeds_result(rs_cam_core::ids::ToolpathId(0), &short)
            .iter()
            .all(|d| d.id.as_str() != ids::FEEDS_LONG_TOOL_DERATE),
        "a 3 x D tool takes no de-rate and must raise no finding"
    );
}
