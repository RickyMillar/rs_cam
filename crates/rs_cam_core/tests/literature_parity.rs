//! Literature parity sentries — pin the live `Material` accessors to
//! the verbatim-quoted reference values staged in
//! `planning/data_ingest_2026-05-29/kc.md` and `hardness.md` (with the
//! 2026-05-30 Wood Database extension at
//! `planning/data_ingest_2026-05-30/wood_database_species.md`).
//!
//! Why bother? The Phase 1+2B ingest moved several constants (sheet-
//! good Kc, Janka 870 / 710 corrections, GRAIN_ANISOTROPY_FACTOR 2.5
//! → 2.0). Future bumps that drift from the citations would slip past
//! every other test. These tests catch a drift on the very next
//! `cargo test`, and the failure message includes the citation so
//! whoever broke them sees what they fought.
//!
//! Tolerance: the literature has measurement spread, and the live
//! constants are midpoints / point estimates. A 5% band is generous
//! enough for an MDF Kc range of 25.81–35.58 vs the live 31.4, tight
//! enough to catch a slow drift before it compounds.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::material::{
    AluminumAlloy, Material, PlasticFamily, PlasticHardness, PlywoodGrade, SheetGoodKind,
    WoodSpecies,
};

/// Generous tolerance band for "live value matches literature midpoint".
/// Tight enough to catch a >5 % drift, loose enough to absorb the
/// natural spread in measured wood properties.
const KC_TOLERANCE_PCT: f64 = 0.05;
const JANKA_TOLERANCE_LBF: f64 = 50.0;

fn within_pct(observed: f64, expected: f64, tolerance_pct: f64, cite: &str) {
    let diff = (observed - expected).abs();
    let band = expected.abs() * tolerance_pct;
    assert!(
        diff <= band,
        "literature drift — observed {observed}, expected {expected} ±{band:.3} \
         (tolerance {:.0}%). Citation: {cite}",
        tolerance_pct * 100.0
    );
}

// ─── Kc (specific cutting force) ────────────────────────────────────

#[test]
fn sheet_good_kc_matches_palubicki_pmc6315737() {
    // Particleboard 35.0 N/mm² — Pałubicki 2021 midpoint of
    // peripheral-up-milling 32.0 (slow) and 37.6 (fast) at vc 40/60 m/s.
    // DOI: 10.3390/ma14092208.
    let pb = Material::SheetGood {
        kind: SheetGoodKind::Particleboard,
    };
    within_pct(
        pb.kc_n_per_mm2().expect("Particleboard Kc must be Some"),
        35.0,
        KC_TOLERANCE_PCT,
        "Pałubicki 2021, DOI 10.3390/ma14092208, midpoint of 32.0–37.6 N/mm² verbatim from kc.md",
    );

    // MDF 31.44 N/mm² — PMC6315737 round-shape Ks average (SD 2.68;
    // range 25.81–35.58).
    let mdf = Material::SheetGood {
        kind: SheetGoodKind::Mdf,
    };
    within_pct(
        mdf.kc_n_per_mm2().expect("MDF Kc must be Some"),
        31.4,
        KC_TOLERANCE_PCT,
        "PMC6315737 round-shape Ks: MDF avg 31.44 (SD 2.68; range 25.81–35.58) — kc.md",
    );
}

#[test]
fn hdpe_kc_matches_yang_2022_midpoint() {
    // HDPE Kc 40.0 N/mm² — Yang 2022 midpoint of measured cutting yield
    // stress 46.89 (15° rake) and 33.85 (30° rake). DOI 10.3390/polym14010189.
    let hdpe = Material::Plastic {
        family: PlasticFamily::Hdpe,
    };
    within_pct(
        hdpe.kc_n_per_mm2().expect("HDPE Kc must be Some"),
        40.0,
        KC_TOLERANCE_PCT,
        "Yang 2022, DOI 10.3390/polym14010189, midpoint of 33.85–46.89 N/mm² verbatim from kc.md",
    );
}

#[test]
fn plastics_without_primary_source_refuse_kc() {
    // The honesty contract: only HDPE has a fetched primary Kc.
    // Every other plastic family must return `None` so the gates
    // refuse with MaterialUnvalidated rather than predict force from
    // a fabricated constant. Drift from this contract — e.g. someone
    // bumping Acrylic to Some(...) without a citation — fires this test.
    //
    // Phase D 2026-05-31 — six new families joined the refusal set
    // (UHMW-PE / PP / Nylon 6/6 / ABS / PETG / Rigid PVC). PMMA Round-2
    // recorded a 276.5 N/mm² nanoscale value (kc_extra.md) with an
    // explicit "do NOT promote — size-effect inflated" caveat. POM/PC
    // remain genuine gaps. None of the new families have any primary
    // milling-regime measurement, so they all refuse.
    for family in [
        PlasticFamily::Polycarbonate,
        PlasticFamily::Acrylic,
        PlasticFamily::Delrin,
        PlasticFamily::Generic,
        PlasticFamily::UhmwPe,
        PlasticFamily::Polypropylene,
        PlasticFamily::Nylon66,
        PlasticFamily::Abs,
        PlasticFamily::Petg,
        PlasticFamily::RigidPvc,
    ] {
        let m = Material::Plastic { family };
        assert_eq!(
            m.kc_n_per_mm2(),
            None,
            "{family:?}: no primary force study fetched (kc.md / kc_extra.md / \
             kc_gaps.md). Until one lands, kc_n_per_mm2() must return None and \
             the gates must refuse via UnmodeledReason::MaterialUnvalidated."
        );
    }
}

#[test]
fn aluminum_kc_matches_vdi_3323_group_22_kienzle_pair() {
    // Phase 4 promoted the Phase 3-F Kienzle pair into
    // `Material::Aluminum::kc_n_per_mm2()`:
    //   kc1.1 = 800 N/mm², mc = 0.25 (VDI 3323 group 22).
    // Evaluated at the representative chip thickness h = 0.1 mm,
    //   Kc = 800 · 0.1^(-0.25) ≈ 1422.8 N/mm²
    // Both alloys share the VDI group anchor until per-alloy data
    // lands.
    let expected = 800.0 * 0.1_f64.powf(-0.25);
    for alloy in [AluminumAlloy::Alloy6061T6, AluminumAlloy::Alloy7075T6] {
        let m = Material::Aluminum { alloy };
        let kc = m.kc_n_per_mm2().expect(
            "Phase 4 enables aluminum Kc — Material::Aluminum::kc_n_per_mm2 must be Some",
        );
        within_pct(
            kc,
            expected,
            0.01,
            "Machining Doctor VDI 3323 group-22 (Wayback 2024-08-13) — \
             planning/data_ingest_2026-05-30/aluminum_kc.md",
        );
        assert!(
            kc > 1000.0 && kc < 2000.0,
            "{alloy:?} Kc {kc} must land in the Sandvik aluminium-specific \
             order-of-magnitude band (350–700 raw, ~1000–2000 once h^-mc scaled)"
        );
    }
}

// ─── Wood species Janka (lbf @ 12% MC) ──────────────────────────────

#[test]
fn longleaf_pine_janka_matches_wood_database() {
    // Phase 2C corrected the trade-group misnomer SouthernYellowPine
    // (Janka 690 — guess at which species) to per-species LongleafPine
    // (Janka 870, Wood Database). This test pins the per-species
    // citation so a future blind revert can't slip past.
    within_pct(
        WoodSpecies::LongleafPine.janka_lbf(),
        870.0,
        JANKA_TOLERANCE_LBF / 870.0,
        "Wood Database Longleaf Pine entry — hardness.md verbatim quote",
    );
}

#[test]
fn radiata_pine_janka_matches_wood_database() {
    // Phase 2C corrected from the repo's pre-Phase-2 500 lbf to the
    // Wood Database value 710 lbf.
    within_pct(
        WoodSpecies::RadiataPine.janka_lbf(),
        710.0,
        JANKA_TOLERANCE_LBF / 710.0,
        "Wood Database Radiata Pine entry — hardness.md verbatim quote",
    );
}

#[test]
fn solid_wood_jankas_match_repo_anchored_values() {
    // The species the repo currently models, with their anchor Janka
    // sources. Hard Maple / Walnut / Red Oak class / White Oak / Birch
    // values come from the FPL Wood Handbook anchors and Wood Database
    // cross-checks. Any future bump should refresh the citation in
    // hardness.md and this test message.
    for (species, expected, source) in [
        (
            WoodSpecies::HardMaple,
            1450.0,
            "FPL / Wood Database 'Hard Maple' (sugar maple Acer saccharum)",
        ),
        (
            WoodSpecies::Walnut,
            1010.0,
            "Wood Database Black Walnut entry — hardness.md",
        ),
        (
            WoodSpecies::Birch,
            1260.0,
            "Wood Database Yellow Birch entry — hardness.md",
        ),
        (
            WoodSpecies::WhiteOak,
            1360.0,
            "Wood Database White Oak entry — hardness.md",
        ),
        (
            WoodSpecies::Jarrah,
            1910.0,
            "Wood Database Jarrah entry — hardness.md",
        ),
        (
            WoodSpecies::Ipe,
            3510.0,
            "Wood Database Ipe entry — hardness.md",
        ),
    ] {
        within_pct(
            species.janka_lbf(),
            expected,
            JANKA_TOLERANCE_LBF / expected,
            source,
        );
    }
}

// ─── Plastic hardness (canonical scale preserved) ───────────────────

#[test]
fn plastic_hardness_preserves_scale_with_citation() {
    // PlasticFamily::hardness() must NEVER silently convert between
    // Shore D and Rockwell M — they measure different things on
    // different ranges. PMMA is reported in Rockwell M natively
    // (MakeItFrom), HDPE / PC / Delrin in Shore D (ISO 868 / ASTM D2240).
    assert!(
        matches!(
            PlasticFamily::Hdpe.hardness(),
            Some(PlasticHardness::ShoreD(v)) if (v - 64.0).abs() < 0.5
        ),
        "HDPE must report Shore D 64 per Direct Plastics / ISO 868 (hardness.md). \
         Was: {:?}",
        PlasticFamily::Hdpe.hardness()
    );
    assert!(
        matches!(
            PlasticFamily::Acrylic.hardness(),
            Some(PlasticHardness::RockwellM(v)) if (v - 93.0).abs() < 0.5
        ),
        "Acrylic (PMMA) must report Rockwell M 93 per MakeItFrom (hardness.md). \
         Was: {:?}",
        PlasticFamily::Acrylic.hardness()
    );
    assert_eq!(
        PlasticFamily::Generic.hardness(),
        None,
        "Generic plastic has no fetched hardness datum — must be None"
    );
}

#[test]
fn uhmw_pe_hardness_matches_tivar_1000_shore_d_anchor() {
    // Phase D 2026-05-31. Mitsubishi Chemical TIVAR 1000 Natural Virgin
    // datasheet, ASTM D2240. Verbatim from `hardness_extra.md` H.1:
    // "Hardness, Shore D ... 66 ... 66 ... ASTM D2240".
    assert!(
        matches!(
            PlasticFamily::UhmwPe.hardness(),
            Some(PlasticHardness::ShoreD(v)) if (v - 66.0).abs() < 0.5
        ),
        "UHMW-PE must report Shore D 66 per Mitsubishi TIVAR 1000 / ASTM D2240 \
         (hardness_extra.md H.1). Was: {:?}",
        PlasticFamily::UhmwPe.hardness()
    );
}

#[test]
fn polypropylene_hardness_matches_simona_shore_d_anchor() {
    // Phase D 2026-05-31. SIMONA PP-H datasheet (ASTM D2240) +
    // Direct Plastics PP-H datasheet (ISO 868), both report Shore D 70.
    // The two-source corroboration is what makes this a Grade A anchor.
    assert!(
        matches!(
            PlasticFamily::Polypropylene.hardness(),
            Some(PlasticHardness::ShoreD(v)) if (v - 70.0).abs() < 0.5
        ),
        "Polypropylene must report Shore D 70 per SIMONA PP-H / ASTM D2240 + \
         Direct Plastics / ISO 868 corroboration (hardness_extra.md H.1). \
         Was: {:?}",
        PlasticFamily::Polypropylene.hardness()
    );
}

#[test]
fn nylon66_hardness_matches_nylatron_gs_shore_d_anchor() {
    // Phase D 2026-05-31. Mitsubishi Chemical Nylatron GS (MoS2-filled
    // cast machinable grade — the CAM-relevant nylon form) datasheet:
    // Shore D 85 / Rockwell M 85 / Rockwell R 115 ALL on the same row
    // (ASTM D2240 / D785). We surface Shore D 85 because it is the
    // primary scale shared by the rest of the plastic enum and the
    // value is corroborated by three independent scales.
    assert!(
        matches!(
            PlasticFamily::Nylon66.hardness(),
            Some(PlasticHardness::ShoreD(v)) if (v - 85.0).abs() < 0.5
        ),
        "Nylon 6/6 must report Shore D 85 per Mitsubishi Nylatron GS / ASTM D2240 \
         (hardness_extra.md H.1). Was: {:?}",
        PlasticFamily::Nylon66.hardness()
    );
}

#[test]
fn abs_hardness_matches_makeitfrom_rockwell_r_anchor() {
    // Phase D 2026-05-31. MakeItFrom material-group page reports a
    // Rockwell R range "100 to 110". The live accessor returns the
    // midpoint 105.0. ABS is reported in Rockwell R natively (the
    // softer R-scale fits softer engineering plastics); a future
    // drift that silently flipped to Shore D would fail the `matches!`
    // pattern.
    assert!(
        matches!(
            PlasticFamily::Abs.hardness(),
            Some(PlasticHardness::RockwellR(v)) if (v - 105.0).abs() < 0.5
        ),
        "ABS must report Rockwell R 105 (midpoint of MakeItFrom 100-110 range) \
         (hardness_extra.md H.1). Was: {:?}",
        PlasticFamily::Abs.hardness()
    );
}

#[test]
fn petg_hardness_matches_vivak_rockwell_r_anchor() {
    // Phase D 2026-05-31. Plaskolite VIVAK Sheet (Sheffield Plastics)
    // datasheet, ASTM D-785. Verbatim from `hardness_extra.md` H.1:
    // "Rockwell Hardness    115    R Scale    ASTM D-785".
    assert!(
        matches!(
            PlasticFamily::Petg.hardness(),
            Some(PlasticHardness::RockwellR(v)) if (v - 115.0).abs() < 0.5
        ),
        "PETG must report Rockwell R 115 per Plaskolite VIVAK / ASTM D-785 \
         (hardness_extra.md H.1). Was: {:?}",
        PlasticFamily::Petg.hardness()
    );
}

#[test]
fn rigid_pvc_hardness_matches_interstate_shore_d_anchor() {
    // Phase D 2026-05-31. Interstate Advanced Materials White PVC
    // Sheet product page (ASTM D-1784 class 12454-B): "74 (Shore D)".
    // Scale-only citation per `hardness_extra.md` H.1 caveat — the
    // source lists the Shore D value but does NOT echo ASTM D2240
    // explicitly. D2240 is the universal Shore D method.
    assert!(
        matches!(
            PlasticFamily::RigidPvc.hardness(),
            Some(PlasticHardness::ShoreD(v)) if (v - 74.0).abs() < 0.5
        ),
        "Rigid PVC Type 1 must report Shore D 74 per Interstate AM (scale-only) \
         (hardness_extra.md H.1). Was: {:?}",
        PlasticFamily::RigidPvc.hardness()
    );
}

// ─── Aluminum Brinell anchors ───────────────────────────────────────

#[test]
fn aluminum_brinell_matches_asm_anchors() {
    within_pct(
        AluminumAlloy::Alloy6061T6.brinell_hb(),
        95.0,
        0.05,
        "ASM Aerospace Specification Metals 6061-T6 Brinell — hardness.md",
    );
    within_pct(
        AluminumAlloy::Alloy7075T6.brinell_hb(),
        150.0,
        0.05,
        "ASM Aerospace Specification Metals 7075-T6 Brinell — hardness.md",
    );
}

#[test]
fn aluminum_brinell_2024_t3_matches_asm_anchor() {
    // Phase C 2026-05-31. ASM matweb 2024-T3 entry, verbatim
    // "Hardness, Brinell  120  120  AA; Typical; 500 g load; 10 mm ball".
    // Source: asm.matweb.com/search/SpecificMaterial.asp?bassnum=ma2024t3
    // (fetched via curl -sk due to TLS chain; content matches the
    // public ASM page). See planning/data_ingest_2026-05-30/
    // hardness_extra.md H.2.
    within_pct(
        AluminumAlloy::Alloy2024T3.brinell_hb(),
        120.0,
        0.05,
        "ASM Aerospace Specification Metals 2024-T3 Brinell — hardness_extra.md H.2",
    );
}

#[test]
fn aluminum_brinell_5052_h32_matches_asm_anchor() {
    // Phase C 2026-05-31. ASM matweb 5052-H32 entry, verbatim
    // "Hardness, Brinell  60  60  AA; Typical; 500 g load; 10 mm ball".
    within_pct(
        AluminumAlloy::Alloy5052H32.brinell_hb(),
        60.0,
        0.05,
        "ASM Aerospace Specification Metals 5052-H32 Brinell — hardness_extra.md H.2",
    );
}

#[test]
fn aluminum_brinell_3003_h14_matches_makeitfrom_anchor() {
    // Phase C 2026-05-31. MakeItFrom 3003-H14 entry — ASM matweb
    // returns HTTP 500 for ma3003*, so MakeItFrom is the primary
    // source. Verbatim "Brinell Hardness    42". ASTM B647 implied
    // by the Brinell scale name but not echoed on per-alloy page.
    within_pct(
        AluminumAlloy::Alloy3003H14.brinell_hb(),
        42.0,
        0.05,
        "MakeItFrom 3003-H14 Brinell — hardness_extra.md H.2 (ASM not hosted)",
    );
}

#[test]
fn aluminum_brinell_1100_o_matches_makeitfrom_anchor() {
    // Phase C 2026-05-31. MakeItFrom 1100-O entry — like 3003,
    // ASM matweb returns HTTP 500 for ma1100*. Verbatim
    // "Brinell Hardness    23". Softest production aluminum
    // (commercially pure, annealed).
    within_pct(
        AluminumAlloy::Alloy1100O.brinell_hb(),
        23.0,
        0.05,
        "MakeItFrom 1100-O Brinell — hardness_extra.md H.2 (ASM not hosted)",
    );
}

#[test]
fn aluminum_brinell_7050_t7651_matches_asm_anchor() {
    // Phase C 2026-05-31. ASM matweb 7050-T7651 entry, verbatim
    // "Hardness, Brinell  147  147  500 kg load with 10 mm ball.
    // Calculated value." Kaiser Aluminum mill datasheet reports
    // 150 on the same alloy/temper (Mechanical Properties table
    // Brinell column); 147 vs 150 agree within source rounding.
    // We use the ASM-calculated 147 to stay consistent with the
    // matweb-primary pattern used for 6061/7075.
    within_pct(
        AluminumAlloy::Alloy7050T7651.brinell_hb(),
        147.0,
        0.05,
        "ASM Aerospace Specification Metals 7050-T7651 Brinell (calc) — hardness_extra.md H.2",
    );
}

// ─── Plywood / sheet-good Janka anchors used by vendor_normalize ─────

#[test]
fn plywood_effective_janka_matches_substrate_anchors() {
    // The Phase 1E refactor moved `vendor_normalize::material_to_lut`
    // off its inline table onto these accessors. Pin them so a future
    // bump to PlywoodGrade::effective_janka_lbf is traceable.
    within_pct(
        PlywoodGrade::Softwood.effective_janka_lbf(),
        600.0,
        JANKA_TOLERANCE_LBF / 600.0,
        "Softwood plywood substrate = Generic softwood baseline (600 lbf, repo anchor)",
    );
    within_pct(
        PlywoodGrade::BalticBirch.effective_janka_lbf(),
        1200.0,
        JANKA_TOLERANCE_LBF / 1200.0,
        "Baltic Birch ply effective = ~1200 lbf (between yellow birch 1260 and engineered substrate)",
    );
    within_pct(
        PlywoodGrade::HardwoodFaced.effective_janka_lbf(),
        1000.0,
        JANKA_TOLERANCE_LBF / 1000.0,
        "Hardwood-faced ply effective = ~1000 lbf substrate-weighted",
    );
}

#[test]
fn sheet_good_effective_janka_matches_anchors() {
    within_pct(
        SheetGoodKind::Mdf.effective_janka_lbf(),
        1100.0,
        JANKA_TOLERANCE_LBF / 1100.0,
        "MDF substrate density proxy — repo anchor",
    );
    within_pct(
        SheetGoodKind::Hdf.effective_janka_lbf(),
        1300.0,
        JANKA_TOLERANCE_LBF / 1300.0,
        "HDF substrate density proxy — repo anchor",
    );
    within_pct(
        SheetGoodKind::Particleboard.effective_janka_lbf(),
        750.0,
        JANKA_TOLERANCE_LBF / 750.0,
        "Particleboard substrate density proxy — repo anchor",
    );
}
