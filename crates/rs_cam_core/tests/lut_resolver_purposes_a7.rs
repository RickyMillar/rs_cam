//! **Checkpoint K (a3) — the two LUT resolvers keep their separate
//! purposes, and the one place they diverge now says so.**
//!
//! A-6's census (`planning/review_2026-08-08/LUT_BOUNDARY_EVIDENCE.md`
//! §1) measured the entry-point divergence at **141 of 18 144** swept
//! queries, every one of them the same class: the recipe resolver
//! ([`find_best_row_for_geometry`]) matched an RPM-only anchor row that
//! publishes no chipload column, so `feeds::calculate` fell back to the
//! empirical formula and returned `chipload_bounds: None`, while the
//! envelope resolver ([`find_best_chip_envelope_row`]) — the one the
//! gate uses — matched a banded row underneath it.
//!
//! Checkpoint K ruled **a3**: keep both resolvers, declare their
//! purposes, and make the fallback **visible**. This file pins the
//! visibility half as a property rather than a value, so it survives
//! LUT growth:
//!
//! > every recommendation that rests on an RPM anchor carries
//! > [`FeedsWarning::VendorRowPublishesNoChipload`] naming that row —
//! > and no recommendation that does not, carries it.
//!
//! **Nothing here moves a number.** The warning is report-only; the RPM
//! anchor is still used, which is precisely why option (a1) ("unify on
//! the envelope resolver") was rejected — it would take the anchor away
//! from those 141 queries.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use rs_cam_core::feeds::{
    ChiploadSource, FeedsInput, FeedsWarning, OperationFamily, PassRole, SetupContext,
    SpindleStrategy, ToolGeometryHint, calculate, embedded_vendor_lut,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

/// The three (op family, geometry) cells A-6 found the divergence in,
/// plus two control cells that do not diverge — so a regression that
/// made the warning fire everywhere fails here too.
fn sweep_inputs() -> Vec<(&'static str, OperationFamily, ToolGeometryHint)> {
    vec![
        (
            "adaptive/flat",
            OperationFamily::Adaptive,
            ToolGeometryHint::Flat,
        ),
        (
            "adaptive/bull",
            OperationFamily::Adaptive,
            ToolGeometryHint::Bull { corner_radius: 0.5 },
        ),
        (
            "trace/vbit60",
            OperationFamily::Trace,
            ToolGeometryHint::VBit {
                included_angle: 60.0,
                tip_diameter: 0.2,
            },
        ),
        (
            "pocket/flat",
            OperationFamily::Pocket,
            ToolGeometryHint::Flat,
        ),
        (
            "scallop/ball",
            OperationFamily::Scallop,
            ToolGeometryHint::Ball,
        ),
    ]
}

/// **Rendered evidence (rule 3)** — what the a3 disclosure actually
/// reads as, through the core diagnostics adapter that the CLI report
/// and MCP `get_diagnostics` both consume.
#[test]
fn the_rpm_anchor_disclosure_renders_as_operator_text() {
    let machine = MachineProfile::generic_wood_router();
    let result = calculate(&FeedsInput {
        tool_diameter: 3.175,
        flute_count: 2,
        flute_length: 20.0,
        shank_diameter: Some(6.0),
        tool_geometry: ToolGeometryHint::VBit {
            included_angle: 60.0,
            tip_diameter: 0.2,
        },
        material: &Material::SolidWood {
            species: WoodSpecies::HardMaple,
        },
        machine: &machine,
        operation: OperationFamily::Trace,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: Some(1.0),
        radial_width_mm: Some(1.2),
        target_scallop_mm: Some(0.01),
        vendor_lut: Some(embedded_vendor_lut()),
        setup: SetupContext::default(),
        spindle_strategy: SpindleStrategy::MatchChart,
    });
    let rendered: Vec<String> =
        rs_cam_core::diagnostics::adapters::from_feeds::diagnostics_from_feeds_result(
            rs_cam_core::ids::ToolpathId(0),
            &result,
        )
        .into_iter()
        .map(|d| format!("[{:?}] {:?}: {}", d.severity, d.id, d.message))
        .collect();
    println!("=== a3 disclosure, as the operator reads it ===");
    for line in &rendered {
        println!("{line}");
    }
    let text = rendered.join("\n");
    assert!(
        text.contains("is an RPM anchor and publishes no chipload column"),
        "the disclosure must render, not merely exist as a typed variant:\n{text}"
    );
    assert!(
        text.contains("this recommendation carries no band"),
        "and it must say the consequence, not only the cause:\n{text}"
    );
}

#[test]
fn rpm_anchor_fallback_is_disclosed_exactly_where_it_happens() {
    let machine = MachineProfile::generic_wood_router();
    let lut = embedded_vendor_lut();
    let mut disclosed = 0usize;
    let mut silent_fallbacks: Vec<String> = Vec::new();
    let mut false_positives: Vec<String> = Vec::new();
    let mut total = 0usize;

    for (label, operation, geometry) in sweep_inputs() {
        for species in [
            WoodSpecies::HardMaple,
            WoodSpecies::WhiteOak,
            WoodSpecies::GenericSoftwood,
        ] {
            let material = Material::SolidWood { species };
            for diameter in [1.0_f64, 3.175, 6.0, 12.0] {
                for flutes in [1u32, 2, 3] {
                    for pass_role in [PassRole::Roughing, PassRole::Finish] {
                        total += 1;
                        let result = calculate(&FeedsInput {
                            tool_diameter: diameter,
                            flute_count: flutes,
                            flute_length: 20.0,
                            shank_diameter: Some(6.0),
                            tool_geometry: geometry,
                            material: &material,
                            machine: &machine,
                            operation,
                            // a3's disclosure is about the resolver, not
                            // the a4 routing; no operation kind here.
                            operation_kind: None,
                            pass_role,
                            axial_depth_mm: Some(diameter * 0.3),
                            radial_width_mm: Some(diameter * 0.4),
                            target_scallop_mm: Some(0.01),
                            vendor_lut: Some(lut),
                            setup: SetupContext::default(),
                            spindle_strategy: SpindleStrategy::MatchChart,
                        });
                        // The condition the warning is *about*: a vendor
                        // row matched, and its chipload column was absent
                        // so the formula supplied the number.
                        let rested_on_rpm_anchor = result.vendor_source.is_some()
                            && matches!(result.chipload_source, ChiploadSource::FormulaFallback);
                        let warned = result.warnings.iter().find_map(|w| match w {
                            FeedsWarning::VendorRowPublishesNoChipload {
                                observation_id, ..
                            } => Some(observation_id.clone()),
                            _ => None,
                        });
                        let case = format!(
                            "{label} d={diameter} f={flutes} {species:?} {pass_role:?} \
                             row={:?}",
                            result.vendor_source
                        );
                        match (rested_on_rpm_anchor, warned) {
                            (true, Some(id)) => {
                                assert_eq!(
                                    Some(&id),
                                    result.vendor_source.as_ref(),
                                    "{case}: the warning must name the row the recipe \
                                     actually rested on"
                                );
                                assert!(
                                    result.chipload_bounds.is_none(),
                                    "{case}: an RPM-anchor recipe must carry no band — \
                                     if it does, the fallback stopped being the \
                                     bandless case this warning describes"
                                );
                                if disclosed < 4 {
                                    println!("  disclosed: {case}");
                                }
                                disclosed += 1;
                            }
                            (true, None) => silent_fallbacks.push(case),
                            (false, Some(_)) => false_positives.push(case),
                            (false, None) => {}
                        }
                    }
                }
            }
        }
    }

    println!(
        "a3 disclosure sweep: {total} queries, {disclosed} rested on an RPM anchor and were \
         disclosed, {} silent, {} false positives",
        silent_fallbacks.len(),
        false_positives.len()
    );
    assert!(
        silent_fallbacks.is_empty(),
        "**a3 regression**: {} recipe(s) fell back to the formula on a matched vendor row \
         with no disclosure: {:?}",
        silent_fallbacks.len(),
        &silent_fallbacks[..silent_fallbacks.len().min(5)]
    );
    assert!(
        false_positives.is_empty(),
        "**a3 over-reach**: the disclosure fired on {} recipe(s) that did NOT rest on an \
         RPM anchor: {:?}",
        false_positives.len(),
        &false_positives[..false_positives.len().min(5)]
    );
    assert!(
        disclosed > 0,
        "the sweep found no RPM-anchor fallback at all, so this test asserts nothing. Either \
         the LUT's RPM-only rows stopped winning any query (say so and re-scope), or the \
         sweep drifted off A-6's three divergent cells (adaptive/flat, adaptive/bull, \
         trace/vbit60)."
    );
}
