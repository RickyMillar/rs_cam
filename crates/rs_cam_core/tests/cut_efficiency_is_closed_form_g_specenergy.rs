//! G-SPECENERGY — `feeds::efficiency::cut_efficiency` reports the closed
//! form of `ADVICE.md` §2, and reports it as an **efficiency** figure.
//!
//! ```text
//! u  =  P / MRR  =  Ks  +  (F_edge · D · ψ) / (2 · ae · fz)   [J/mm³]
//! ```
//!
//! ## What this file protects
//!
//! `u` divides the two-term power by the material removal rate, and in
//! that division the spindle speed `n` and the axial DOC `ap` cancel
//! exactly. That cancellation is the whole reason the number is worth
//! putting on screen: it separates *efficiency* from *rate*. RPM buys
//! rate. Depth buys rate. Neither buys efficiency. Only the chipload and
//! the radial engagement move `u`.
//!
//! A later "simplification" that reintroduces either term — for instance
//! one that reaches for `FeedsResult::power_kw` or `mrr_mm3_min` instead
//! of the closed form — turns `u` back into a rate figure while leaving
//! the field name and the units unchanged. Nothing else in the engine
//! would notice. So the invariance arms below are the point of the file,
//! and the sensitivity arms are there because invariance alone also
//! passes on a constant.
//!
//! ## The measured fixture
//!
//! 6 mm 2-flute flat, DOC 4.20, WOC 2.10, generic softwood. Measured in
//! `planning/load_model_2026-09-16/ADVICE.md` §1:
//!
//! | fz (mm/tooth) | u (J/mm³) | ploughing share |
//! |---|---:|---:|
//! | 0.0380 (running)    | 151 | 83 % |
//! | 0.0675 (vendor mid) |  96 | 74 % |
//! | 0.0850 (vendor max) |  81 | 69 % |
//!
//! Those three rows are pinned here against values this file derives
//! itself, from locally re-declared literature constants. A
//! literature-matrix cell must pin the published number independently of
//! whatever the crate currently believes it is.

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
use rs_cam_core::material::{Material, WoodSpecies};

// --- The fixture, as ADVICE.md §1 measured it ------------------------

const DIAMETER_MM: f64 = 6.0;
const FLUTES: u32 = 2;
const AXIAL_DOC_MM: f64 = 4.20;
const RADIAL_WOC_MM: f64 = 2.10;

const FZ_RUNNING: f64 = 0.0380;
const FZ_VENDOR_MID: f64 = 0.0675;
const FZ_VENDOR_MAX: f64 = 0.0850;

/// The vendor window whose midpoint is `FZ_VENDOR_MID` and whose ceiling
/// is `FZ_VENDOR_MAX` — the band ADVICE.md's two right-hand rows name.
const BAND_MIN_MM: f64 = 0.0500;
const BAND_MAX_MM: f64 = 0.0850;

// --- The literature constants, re-declared locally -------------------

/// Affine slope `Ks` (N/mm²) for generic softwood: the woodresearch.sk
/// 201905/12 anchor `49.95` scaled by this material's `Kc / 35.1`, i.e.
/// `(2.7 × 6.5) / (2.7 × 13.0) = 0.5`. Declared here so the expected `u`
/// below is derived from the published fit, not from the crate.
const SOFTWOOD_KS_N_PER_MM2: f64 = 49.95 * 0.5;

/// Affine edge intercept `F_edge` (N per mm of axial engagement) for the
/// same material — the `+5.30` term of the same fit, same scale.
const SOFTWOOD_F_EDGE_N_PER_MM: f64 = 5.30 * 0.5;

/// Relative tolerance for "the same number". The invariance arms vary
/// the spindle speed and the axial DOC, which reach the closed form
/// through nothing at all, so the two sides agree to the last bit or the
/// model has grown a dependency it must not have. A model that had
/// reintroduced `n` or `ap` would differ by tens of percent, not by
/// 1e-12.
const REL_TOL: f64 = 1e-12;

/// `u` from the closed form, computed here with plain arithmetic —
/// `ψ = arccos(1 − ae/r)` included. Independent of `feeds::force`.
fn expected_specific_energy(radial_woc_mm: f64, advance_per_tooth_mm: f64) -> f64 {
    let psi = (1.0 - radial_woc_mm / (DIAMETER_MM / 2.0))
        .clamp(-1.0, 1.0)
        .acos();
    SOFTWOOD_KS_N_PER_MM2
        + (SOFTWOOD_F_EDGE_N_PER_MM * DIAMETER_MM * psi)
            / (2.0 * radial_woc_mm * advance_per_tooth_mm)
}

fn softwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    }
}

fn six_mm_flat() -> ToolConfig {
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
        stickout: 30.0,
        flute_count: FLUTES,
        tool_material: ToolMaterial::Carbide,
        cut_direction: BitCutDirection::UpCut,
        vendor: String::new(),
        product_id: String::new(),
        size_units: None,
    }
}

fn pocket_op() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        stepover: RADIAL_WOC_MM,
        depth: 12.0,
        depth_per_pass: AXIAL_DOC_MM,
        feed_rate: 1292.0,
        spindle_rpm: Some(17_000),
        ..PocketConfig::default()
    })
}

/// A `FeedsResult` describing exactly one operating point. The fields
/// `cut_efficiency` reads are set; the rest carry the neutral values the
/// calculator would leave for a formula-fallback recipe.
fn operating_point(
    rpm: f64,
    advance_per_tooth_mm: f64,
    axial_doc_mm: f64,
    radial_woc_mm: f64,
    band: Option<ChiploadBounds>,
) -> FeedsResult {
    let feed = advance_per_tooth_mm * rpm * f64::from(FLUTES);
    FeedsResult {
        rpm,
        chip_load_mm: advance_per_tooth_mm,
        feed_rate_mm_min: feed,
        plunge_rate_mm_min: feed * 0.4,
        ramp_feed_mm_min: feed * 0.6,
        axial_depth_mm: axial_doc_mm,
        radial_width_mm: radial_woc_mm,
        power_kw: 0.0,
        available_power_kw: 0.8,
        power_limited: false,
        mrr_mm3_min: axial_doc_mm * radial_woc_mm * feed,
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
        min_mm_per_tooth: BAND_MIN_MM,
        max_mm_per_tooth: BAND_MAX_MM,
    })
}

/// `u` at one operating point of the fixture.
fn u_at(rpm: f64, advance_per_tooth_mm: f64, axial_doc_mm: f64, radial_woc_mm: f64) -> f64 {
    let result = operating_point(
        rpm,
        advance_per_tooth_mm,
        axial_doc_mm,
        radial_woc_mm,
        vendor_band(),
    );
    cut_efficiency(
        &pocket_op(),
        &six_mm_flat(),
        &softwood(),
        &MachineProfile::default(),
        &result,
    )
    .expect("softwood carries a primary-source Kc, so the model answers")
    .specific_energy_j_per_mm3
}

fn assert_same(label: &str, a: f64, b: f64) {
    assert!(
        (a - b).abs() <= a.abs() * REL_TOL,
        "{label}: {a} vs {b} — the two must be the same number"
    );
}

#[test]
fn specific_energy_matches_the_closed_form_at_three_chiploads() {
    for fz in [FZ_RUNNING, FZ_VENDOR_MID, FZ_VENDOR_MAX] {
        let got = u_at(17_000.0, fz, AXIAL_DOC_MM, RADIAL_WOC_MM);
        let want = expected_specific_energy(RADIAL_WOC_MM, fz);
        assert_same(&format!("u at fz {fz}"), got, want);
    }
}

#[test]
fn specific_energy_reproduces_the_advice_table() {
    // ADVICE.md §1's measured column, to the precision it publishes.
    for (fz, published_u, published_ploughing) in [
        (FZ_RUNNING, 151.0, 0.83),
        (FZ_VENDOR_MID, 96.0, 0.74),
        (FZ_VENDOR_MAX, 81.0, 0.69),
    ] {
        let eff = cut_efficiency(
            &pocket_op(),
            &six_mm_flat(),
            &softwood(),
            &MachineProfile::default(),
            &operating_point(17_000.0, fz, AXIAL_DOC_MM, RADIAL_WOC_MM, vendor_band()),
        )
        .expect("softwood carries a primary-source Kc");
        let u = eff.specific_energy_j_per_mm3;
        assert!(
            (u - published_u).abs() < 0.5,
            "fz {fz}: ADVICE.md publishes u = {published_u} J/mm^3, got {u:.2}"
        );
        assert!(
            (eff.ploughing_share - published_ploughing).abs() < 0.005,
            "fz {fz}: ADVICE.md publishes a {published_ploughing} ploughing share, got {:.4}",
            eff.ploughing_share
        );
        // The share is the closed form's own decomposition, not a second
        // opinion: (u - Ks) / u.
        assert_same(
            "ploughing share",
            eff.ploughing_share,
            (u - SOFTWOOD_KS_N_PER_MM2) / u,
        );
    }
}

/// `n` cancels against `MRR`. Two spindle speeds, one chipload, one
/// answer.
#[test]
fn specific_energy_is_invariant_under_rpm() {
    let slow = u_at(12_000.0, FZ_RUNNING, AXIAL_DOC_MM, RADIAL_WOC_MM);
    let fast = u_at(24_000.0, FZ_RUNNING, AXIAL_DOC_MM, RADIAL_WOC_MM);
    assert_same("u across a 2x RPM change", slow, fast);
}

/// `ap` cancels against `MRR`. Two depths, one chipload, one answer.
#[test]
fn specific_energy_is_invariant_under_axial_doc() {
    let shallow = u_at(17_000.0, FZ_RUNNING, AXIAL_DOC_MM, RADIAL_WOC_MM);
    let deep = u_at(17_000.0, FZ_RUNNING, AXIAL_DOC_MM * 2.0, RADIAL_WOC_MM);
    assert_same("u across a 2x DOC change", shallow, deep);
}

/// Non-vacuity for both invariance arms: `u` is not a constant. It falls
/// hyperbolically with the chipload and it moves with the radial
/// engagement. Without this arm, a `cut_efficiency` that returned a fixed
/// number would satisfy every assertion above.
#[test]
fn specific_energy_moves_with_chipload_and_with_radial_width() {
    let thin = u_at(17_000.0, FZ_RUNNING, AXIAL_DOC_MM, RADIAL_WOC_MM);
    let mid = u_at(17_000.0, FZ_VENDOR_MID, AXIAL_DOC_MM, RADIAL_WOC_MM);
    let heavy = u_at(17_000.0, FZ_VENDOR_MAX, AXIAL_DOC_MM, RADIAL_WOC_MM);
    assert!(
        thin > mid && mid > heavy,
        "u must fall as the chip thickens: {thin:.2} / {mid:.2} / {heavy:.2}"
    );
    assert!(
        thin / heavy > 1.5,
        "the fixture spends {:.2}x more energy per mm^3 at fz {FZ_RUNNING} than at fz \
         {FZ_VENDOR_MAX}; ADVICE.md measures 1.86x",
        thin / heavy
    );

    let narrow = u_at(17_000.0, FZ_RUNNING, AXIAL_DOC_MM, RADIAL_WOC_MM);
    let wide = u_at(17_000.0, FZ_RUNNING, AXIAL_DOC_MM, RADIAL_WOC_MM * 2.0);
    assert!(
        (narrow - wide).abs() > narrow * 0.05,
        "u must move with the radial engagement: {narrow:.2} vs {wide:.2}"
    );
}

/// The two ratios are the operator-facing form of the same closed form,
/// both measured against the band midpoint. ADVICE.md's running point
/// reads "1.6x tool wear, 1.8x the time" against this band.
#[test]
fn the_ratios_compare_the_operating_point_with_the_band_midpoint() {
    let eff = cut_efficiency(
        &pocket_op(),
        &six_mm_flat(),
        &softwood(),
        &MachineProfile::default(),
        &operating_point(
            17_000.0,
            FZ_RUNNING,
            AXIAL_DOC_MM,
            RADIAL_WOC_MM,
            vendor_band(),
        ),
    )
    .expect("softwood carries a primary-source Kc");

    let band_mid = (BAND_MIN_MM + BAND_MAX_MM) / 2.0;
    let wear = eff
        .wear_ratio_vs_band_mid
        .expect("a band is present, so the wear ratio is answerable");
    let time = eff
        .time_ratio_vs_band_mid
        .expect("a band is present, so the time ratio is answerable");
    assert_same(
        "wear ratio",
        wear,
        expected_specific_energy(RADIAL_WOC_MM, FZ_RUNNING)
            / expected_specific_energy(RADIAL_WOC_MM, band_mid),
    );
    assert_same("time ratio", time, band_mid / FZ_RUNNING);
    assert!(
        (wear - 1.6).abs() < 0.05 && (time - 1.8).abs() < 0.05,
        "the Phase A row reads 1.6x wear and 1.8x time; got {wear:.2}x and {time:.2}x"
    );
    assert_eq!(eff.verdict, ChipVerdict::Thin);
    assert_same("advance per tooth", eff.advance_per_tooth_mm, FZ_RUNNING);
}

/// The verdict reads the band, and it reads all three sides of it.
#[test]
fn the_verdict_places_the_chipload_against_the_band() {
    for (fz, want) in [
        (FZ_RUNNING, ChipVerdict::Thin),
        (FZ_VENDOR_MID, ChipVerdict::InBand),
        (0.1100, ChipVerdict::Heavy),
    ] {
        let eff = cut_efficiency(
            &pocket_op(),
            &six_mm_flat(),
            &softwood(),
            &MachineProfile::default(),
            &operating_point(17_000.0, fz, AXIAL_DOC_MM, RADIAL_WOC_MM, vendor_band()),
        )
        .expect("softwood carries a primary-source Kc");
        assert_eq!(eff.verdict, want, "fz {fz}");
    }
}
