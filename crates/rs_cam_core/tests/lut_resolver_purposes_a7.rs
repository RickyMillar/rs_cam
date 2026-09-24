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
//!
//! **Ruling B4 (2026-09-25).** For a V-bit the recipe resolver now tries
//! the chipload-bearing rows first; an RPM anchor answers a V-bit only when
//! no chip row matches. On the embedded LUT no swept recipe rests on an RPM
//! anchor any more (`trace/vbit60` was the last cell; the two flat RPM rows
//! win no swept query). So the property runs twice: on the embedded LUT it
//! must find no silent fallback, no false positive and, since B4, no
//! disclosure; on a fixture LUT that holds only the two Whiteside V-bit RPM
//! anchors, the `trace/vbit60` cells rest on an anchor and must disclose it.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use rs_cam_core::feeds::vendor_lut::VendorLut;
use rs_cam_core::feeds::{
    ChiploadSource, FeedsInput, FeedsWarning, OperationFamily, PassRole, SetupContext,
    SpindleStrategy, ToolGeometryHint, calculate, embedded_vendor_lut,
};

/// The two Whiteside V-bit RPM anchors of the embedded LUT (60 deg, 1/4 in
/// and 1/2 in, hardwood Trace; RPM only, no chipload), alone. With no chip
/// row beside them, a 60 deg V-bit Trace recipe in solid wood rests on one
/// of them (ruling B4 keeps an RPM anchor for a V-bit when no chip row
/// matches).
fn rpm_anchor_lut() -> VendorLut {
    let observations: Vec<_> = embedded_vendor_lut()
        .observations
        .iter()
        .filter(|o| {
            o.observation_id == "whiteside-1540-vgroove-60deg-quarter-rpm"
                || o.observation_id == "whiteside-1550-vgroove-60deg-half-rpm"
        })
        .cloned()
        .collect();
    assert_eq!(observations.len(), 2, "the two Whiteside RPM anchors exist");
    assert!(
        observations
            .iter()
            .all(|o| o.chipload_min_mm_tooth.is_none() && o.chipload_max_mm_tooth.is_none()),
        "the fixture rows are RPM anchors"
    );
    VendorLut { observations }
}
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
///
/// Ruling B4: on the embedded LUT this cut now rests on the AMS-159 chip
/// row, so the fixture LUT of the two Whiteside RPM anchors carries the
/// case. The 3.175 mm key is 0.5x of the 1/4 in anchor.
#[test]
fn the_rpm_anchor_disclosure_renders_as_operator_text() {
    let lut = rpm_anchor_lut();
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
        vendor_lut: Some(&lut),
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
    assert!(
        text.contains("No chipload-bearing row matches this cut"),
        "the fixture has no chip row, so the text must not claim a gate row:\n{text}"
    );
}

/// The disclosure count of one sweep.
struct Sweep {
    total: usize,
    disclosed: usize,
    silent_fallbacks: Vec<String>,
    false_positives: Vec<String>,
}

fn sweep(lut: &VendorLut) -> Sweep {
    let machine = MachineProfile::generic_wood_router();
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

    Sweep {
        total,
        disclosed,
        silent_fallbacks,
        false_positives,
    }
}

/// No silent fallback and no false positive in one sweep.
fn assert_disclosed_exactly(label: &str, s: &Sweep) {
    println!(
        "a3 disclosure sweep ({label}): {} queries, {} rested on an RPM anchor and were \
         disclosed, {} silent, {} false positives",
        s.total,
        s.disclosed,
        s.silent_fallbacks.len(),
        s.false_positives.len()
    );
    assert!(
        s.silent_fallbacks.is_empty(),
        "**a3 regression** ({label}): {} recipe(s) fell back to the formula on a matched \
         vendor row with no disclosure: {:?}",
        s.silent_fallbacks.len(),
        &s.silent_fallbacks[..s.silent_fallbacks.len().min(5)]
    );
    assert!(
        s.false_positives.is_empty(),
        "**a3 over-reach** ({label}): the disclosure fired on {} recipe(s) that did NOT rest \
         on an RPM anchor: {:?}",
        s.false_positives.len(),
        &s.false_positives[..s.false_positives.len().min(5)]
    );
}

#[test]
fn rpm_anchor_fallback_is_disclosed_exactly_where_it_happens() {
    // The embedded LUT: the property holds, and since ruling B4 no swept
    // recipe rests on an RPM anchor (derived with python3 over the JSON
    // files and the scorer of `vendor_lookup.rs`). A non-zero count is a
    // new RPM-anchor recipe: name it before a re-pin.
    let embedded = sweep(embedded_vendor_lut());
    assert_disclosed_exactly("embedded LUT", &embedded);
    assert_eq!(
        embedded.disclosed, 0,
        "ruling B4 left no RPM-anchor recipe on the embedded LUT in this sweep; {} now rest \
         on one",
        embedded.disclosed
    );

    // The fixture LUT: only the two Whiteside RPM anchors. The trace/vbit60
    // cells rest on them and must disclose; every other cell finds no row.
    let fixture = sweep(&rpm_anchor_lut());
    assert_disclosed_exactly("RPM-anchor fixture", &fixture);
    assert!(
        fixture.disclosed > 0,
        "the fixture of two RPM anchors gave no RPM-anchor recipe, so this test asserts \
         nothing: the V-bit recipe no longer falls back to an RPM anchor when no chip row \
         matches"
    );
}
