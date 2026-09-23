//! G-SPECENERGY — `feeds::efficiency` abstains rather than fabricates.
//!
//! The specific-energy model is built entirely on `Ks` and `F_edge`, and
//! those two coefficients exist only for a material with a primary-source
//! `Kc`. Where the `Kc` is absent there is no force model, so there is no
//! answer — `cut_efficiency` returns `None`, the same refusal the engine
//! already makes through `UnmodeledReason::MaterialUnvalidated`.
//!
//! ## Why this file exists
//!
//! This repository's recurring defect is **absence rendered as success**:
//! an empty triage drawn as an all-clear, an empty chart frame that looks
//! like a chart, a hazard verdict built from a pointer that was not on
//! the chart. A specific energy of `0.0 J/mm³` would be the next one — it
//! reads as a perfectly efficient cut. A ploughing share of `0.0` reads
//! as a cut that spends nothing on rubbing. Both are the most flattering
//! possible lie about a material the engine knows nothing about.
//!
//! Every arm below is paired with a non-vacuity arm, because a module
//! that returned `None` for everything would pass the refusal arms alone.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::tool_config::{
    BitCutDirection, ToolConfig, ToolId, ToolMaterial, ToolType,
};
use rs_cam_core::feeds::efficiency::{ChipVerdict, cut_efficiency};
use rs_cam_core::feeds::{ChiploadBounds, ChiploadSource, FeedsDerates, FeedsResult};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, PlasticFamily};

const DIAMETER_MM: f64 = 6.0;
const FLUTES: u32 = 2;
const AXIAL_DOC_MM: f64 = 4.20;
const RADIAL_WOC_MM: f64 = 2.10;
const RPM: f64 = 17_000.0;
const FZ: f64 = 0.0380;

/// Acrylic has no fetched primary milling-regime force study, so
/// `Material::kc_n_per_mm2` returns `None`. See `material.rs` — the
/// historical generic 4.0 was a fabricated baseline and was removed.
fn material_without_kc() -> Material {
    Material::Plastic {
        family: PlasticFamily::Acrylic,
    }
}

/// HDPE carries a measured yield-stress midpoint (Yang 2022), so it has a
/// `Kc`. Same enum variant as the refusing material — only the primary
/// source differs, which is exactly the distinction under test.
fn material_with_kc() -> Material {
    Material::Plastic {
        family: PlasticFamily::Hdpe,
    }
}

fn endmill(stickout: f64) -> ToolConfig {
    ToolConfig {
        id: ToolId(0),
        name: "6 mm 2-flute flat".to_owned(),
        tool_number: 1,
        tool_type: ToolType::EndMill,
        diameter: DIAMETER_MM,
        cutting_length: 22.0,
        helix_deg: 30.0,
        corner_radius_mm: 0.0,
        corner_radius: 0.0,
        included_angle: 90.0,
        taper_half_angle: 15.0,
        shaft_diameter: DIAMETER_MM,
        holder_diameter: 25.0,
        shank_diameter: DIAMETER_MM,
        shank_length: 20.0,
        stickout,
        flute_count: FLUTES,
        tool_material: ToolMaterial::Carbide,
        cut_direction: BitCutDirection::UpCut,
        vendor: String::new(),
        product_id: String::new(),
    }
}

fn pocket_op() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        stepover: RADIAL_WOC_MM,
        depth: 12.0,
        depth_per_pass: AXIAL_DOC_MM,
        feed_rate: FZ * RPM * f64::from(FLUTES),
        spindle_rpm: Some(17_000),
        ..PocketConfig::default()
    })
}

fn operating_point(axial_doc_mm: f64, band: Option<ChiploadBounds>) -> FeedsResult {
    let feed = FZ * RPM * f64::from(FLUTES);
    FeedsResult {
        rpm: RPM,
        chip_load_mm: FZ,
        feed_rate_mm_min: feed,
        plunge_rate_mm_min: feed * 0.4,
        ramp_feed_mm_min: feed * 0.6,
        axial_depth_mm: axial_doc_mm,
        radial_width_mm: RADIAL_WOC_MM,
        power_kw: 0.0,
        available_power_kw: 0.8,
        power_limited: false,
        mrr_mm3_min: axial_doc_mm * RADIAL_WOC_MM * feed,
        warnings: Vec::new(),
        vendor_source: None,
        support: rs_cam_core::feeds::FeedsSupport::FormulaOnly {
            source: rs_cam_core::feeds::support::MILLING_FORMULA_SOURCE,
        },
        chipload_source: ChiploadSource::FormulaFallback,
        chipload_bounds: band,
        matched_lut_row: None,
        effective_diameter_mm: DIAMETER_MM,
        derates: FeedsDerates::default(),
    }
}

fn vendor_band() -> Option<ChiploadBounds> {
    Some(ChiploadBounds {
        min_mm_per_tooth: 0.0500,
        max_mm_per_tooth: 0.0850,
    })
}

#[test]
fn a_material_without_a_primary_source_kc_gets_no_efficiency_answer() {
    let answer = cut_efficiency(
        &pocket_op(),
        &endmill(30.0),
        &material_without_kc(),
        &MachineProfile::default(),
        &operating_point(AXIAL_DOC_MM, vendor_band()),
    );
    assert!(
        answer.is_none(),
        "acrylic has no Kc, so it has no Ks and no F_edge; a number here would be fabricated"
    );
}

/// Non-vacuity. The same call on a material that *does* carry a `Kc`
/// answers, and answers with a usable number — so the arm above is not
/// passing because `cut_efficiency` refuses everything.
#[test]
fn a_material_with_a_primary_source_kc_gets_an_answer() {
    let eff = cut_efficiency(
        &pocket_op(),
        &endmill(30.0),
        &material_with_kc(),
        &MachineProfile::default(),
        &operating_point(AXIAL_DOC_MM, vendor_band()),
    )
    .expect("HDPE carries a measured Kc, so the affine model applies");
    assert!(
        eff.specific_energy_j_per_mm3.is_finite() && eff.specific_energy_j_per_mm3 > 0.0,
        "u must be a real positive energy density, got {}",
        eff.specific_energy_j_per_mm3
    );
    assert!(
        (0.0..1.0).contains(&eff.ploughing_share),
        "the ploughing share is a fraction of the cutting power, got {}",
        eff.ploughing_share
    );
}

/// Each `Option` field refuses for its own reason. Without a band there
/// is no midpoint, so neither ratio has a second operand — and neither
/// reports one.
#[test]
fn without_a_band_the_ratios_abstain_and_the_verdict_says_so() {
    let eff = cut_efficiency(
        &pocket_op(),
        &endmill(30.0),
        &material_with_kc(),
        &MachineProfile::default(),
        &operating_point(AXIAL_DOC_MM, None),
    )
    .expect("the material still carries a Kc; only the band is missing");
    assert_eq!(eff.verdict, ChipVerdict::NoBand);
    assert!(eff.band.is_none());
    assert!(
        eff.wear_ratio_vs_band_mid.is_none(),
        "no band midpoint means no wear ratio, not a ratio of 1.0"
    );
    assert!(
        eff.time_ratio_vs_band_mid.is_none(),
        "no band midpoint means no time ratio, not a ratio of 1.0"
    );
    // The band-independent half of the answer still stands.
    assert!(eff.specific_energy_j_per_mm3 > 0.0);
    assert!(eff.rubbing_floor_mm > 0.0);
}

/// Non-vacuity for the arm above: with a band, both ratios are `Some`.
#[test]
fn with_a_band_both_ratios_are_answerable() {
    let eff = cut_efficiency(
        &pocket_op(),
        &endmill(30.0),
        &material_with_kc(),
        &MachineProfile::default(),
        &operating_point(AXIAL_DOC_MM, vendor_band()),
    )
    .expect("HDPE carries a measured Kc");
    assert!(eff.band.is_some());
    assert!(eff.wear_ratio_vs_band_mid.is_some());
    assert!(eff.time_ratio_vs_band_mid.is_some());
}

/// The deflection ceiling is its own refusal. A tool with no stickout
/// carries no cantilever, so there is no compliance and no ceiling — and
/// an absent axial DOC removes the ceiling for the same kind of reason.
#[test]
fn the_deflection_ceiling_abstains_when_its_own_inputs_are_absent() {
    let machine = MachineProfile::default();
    let no_stickout = cut_efficiency(
        &pocket_op(),
        &endmill(0.0),
        &material_with_kc(),
        &machine,
        &operating_point(AXIAL_DOC_MM, vendor_band()),
    )
    .expect("the material is unchanged; only the tool geometry is degenerate");
    assert!(
        no_stickout.deflection_ceiling_mm.is_none(),
        "no stickout means no cantilever and no ceiling, not a ceiling of 0"
    );

    let no_axial = cut_efficiency(
        &pocket_op(),
        &endmill(30.0),
        &material_with_kc(),
        &machine,
        &operating_point(0.0, vendor_band()),
    )
    .expect("the material is unchanged; only the axial DOC is absent");
    assert!(
        no_axial.deflection_ceiling_mm.is_none(),
        "no axial engagement means no deflection budget to invert"
    );
}

/// Non-vacuity for the arm above: given a stickout and an axial DOC, the
/// ceiling is a real positive chipload.
#[test]
fn the_deflection_ceiling_is_a_number_when_the_model_applies() {
    let eff = cut_efficiency(
        &pocket_op(),
        &endmill(30.0),
        &material_with_kc(),
        &MachineProfile::default(),
        &operating_point(AXIAL_DOC_MM, vendor_band()),
    )
    .expect("HDPE carries a measured Kc");
    let ceiling = eff
        .deflection_ceiling_mm
        .expect("a 6 mm carbide endmill at 30 mm stickout has a solvable deflection budget");
    assert!(
        ceiling.is_finite() && ceiling > 0.0,
        "the ceiling is an advance per tooth, got {ceiling}"
    );
}
