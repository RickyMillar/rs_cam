//! Literature parity sentries — pin the live `Material` accessors to
//! the verbatim-quoted reference values staged in
//! `planning/data_ingest_2026-05-29/kc.md` and `hardness.md` (with the
//! 2026-05-30 Wood Database extension at
//! `planning/data_ingest_2026-05-30/wood_database_species.md`).
//!
//! Why bother? The Phase 1+2B ingest moved several constants (Janka
//! 870 / 710 corrections among them). Future bumps that drift from the
//! citations would slip past every other test. These tests catch a
//! drift on the very next `cargo test`, and the failure message
//! includes the citation so whoever broke them sees what they fought.
//!
//! Ruling B6 (2026-09-24) replaced the scalar `Kc` with one typed force
//! line per material (`Material::force_line`). The force arms now pin
//! the Goli 2018 MDF line and the named refusals. The solid-wood lines
//! are pinned in `the_force_line_is_printed_per_family_b6.rs`.
//!
//! Tolerance: the literature has measurement spread, and the live
//! constants are midpoints / point estimates. A 5% band is generous
//! enough for the MDF Ks range 25.81–35.58 around the printed 31.44,
//! tight enough to catch a slow drift before it compounds.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::material::force_line::ForceLineRefusal;
use rs_cam_core::material::{
    AluminumAlloy, Material, PlasticFamily, PlasticHardness, PlywoodGrade, SheetGoodKind,
    WoodSpecies,
};

/// Generous tolerance band for "live value matches literature midpoint".
/// Tight enough to catch a >5 % drift, loose enough to absorb the
/// natural spread in measured wood properties.
const FORCE_TOLERANCE_PCT: f64 = 0.05;
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

// ─── The force line (ruling B6) ─────────────────────────────────────
//
// Ruling B6 (2026-09-24) replaced the scalar `Kc` with one typed force
// line per material (`Material::force_line`). The sheet-good, HDPE and
// aluminium `Kc` literals are gone. These arms pin the one printed line
// that stays (MDF, Goli 2018) and the refusals that replace the others.

#[test]
fn mdf_force_line_is_goli_2018_and_other_sheet_goods_refuse() {
    // MDF: Goli 2018 Table 3, up-milling: "Ks [N mm−2] | 31.44 (2.68)" and
    // "Int [N mm−1] | 3.36 (0.27)". doi:10.3390/ma11122575 (PMC6315737).
    let mdf = Material::SheetGood {
        kind: SheetGoodKind::Mdf,
    }
    .force_line()
    .expect("MDF must carry the Goli 2018 line");
    within_pct(
        mdf.ks_n_per_mm2(),
        31.44,
        FORCE_TOLERANCE_PCT,
        "Goli 2018 Table 3, MDF Ks 31.44 (SD 2.68; range 25.81–35.58)",
    );
    within_pct(
        mdf.f_edge_n_per_mm(),
        3.36,
        FORCE_TOLERANCE_PCT,
        "Goli 2018 Table 3, MDF Int 3.36 (SD 0.27; range 2.96–3.83)",
    );
    assert_eq!(
        mdf.chip_range_mm(),
        (0.041, 0.091),
        "Goli 2018 mean chip range"
    );
    assert!(
        (mdf.grain_factor() - 1.0).abs() < 1e-12,
        "MDF is isotropic in plane, so its grain factor is 1.0"
    );

    // Particleboard: Pałubicki 2021 prints a total kc at 40-60 m/s, which
    // is a different quantity from the per-edge line. HDF: no source.
    assert_eq!(
        Material::SheetGood {
            kind: SheetGoodKind::Particleboard,
        }
        .force_line(),
        Err(ForceLineRefusal::Particleboard),
        "ruling B6 retired the Pałubicki 2021 particleboard value from the force model"
    );
    assert_eq!(
        Material::SheetGood {
            kind: SheetGoodKind::Hdf,
        }
        .force_line(),
        Err(ForceLineRefusal::Hdf),
        "no fetched source measures HDF"
    );
}

#[test]
fn every_plastic_refuses_a_force_line() {
    // The honesty contract: no plastic has a printed cutting-force line.
    // HDPE joined the refusal set in ruling B6: Yang 2022
    // (doi:10.3390/polym14010189) prints a cutting yield stress, not a
    // cutting force. Every other family had no primary force study
    // (kc.md / kc_extra.md / kc_gaps.md).
    for family in [
        PlasticFamily::Hdpe,
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
        assert_eq!(
            Material::Plastic { family }.force_line(),
            Err(ForceLineRefusal::Plastic),
            "{family:?}: no source prints a cutting-force line for a plastic, so the \
             gates must refuse via UnmodeledReason::MaterialUnvalidated"
        );
    }
}

#[test]
fn aluminum_refuses_a_force_line() {
    // The only pair is a Kienzle kc1.1 800 / mc 0.25 from an aggregator
    // (VDI 3323 group 22, planning/data_ingest_2026-05-30/aluminum_kc.md).
    // It is a power law with no printed chip range, not an affine
    // per-edge line, so ruling B6 refuses it.
    for alloy in [
        AluminumAlloy::Alloy6061T6,
        AluminumAlloy::Alloy7075T6,
        AluminumAlloy::Alloy2024T3,
        AluminumAlloy::Alloy5052H32,
        AluminumAlloy::Alloy3003H14,
        AluminumAlloy::Alloy1100O,
        AluminumAlloy::Alloy7050T7651,
    ] {
        assert_eq!(
            Material::Aluminum { alloy }.force_line(),
            Err(ForceLineRefusal::Aluminum),
            "{alloy:?}: the Kienzle pair is not a per-edge line (ruling B6)"
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

// ─── Parametric solid wood: the library row's FPL density ───────────

#[test]
fn solid_wood_by_janka_reads_the_fpl_row_density() {
    // Ruling B6 removed the `janka_to_kc_n_per_mm2` helper. The force line
    // of `Material::SolidWoodByJanka` reads the FPL Table 5-3a SG of its
    // library row (`specific_gravity_12`), and never the Janka value.
    // FPL: "Cherry, black 12% 0.50 85,000 ..." gives ρ = 0.50 × 1120 = 560
    // kg/m³. The two Janka values below differ, and the line must not.
    let cherry = |janka_lbf: f64| Material::SolidWoodByJanka {
        janka_lbf,
        label: "Cherry, black".to_owned(),
        source_id: "fpl_ch5_2010".to_owned(),
    };
    let line = cherry(950.0)
        .force_line()
        .expect("an FPL library row with an SG has a force line");
    let rho = 0.50 * 1000.0 * 1.12;
    assert!(
        line.density_kg_m3().is_some_and(|d| (d - rho).abs() < 1e-9),
        "black cherry ρ {:?}, want {rho} (FPL Table 5-3a SG 0.50 × 1120)",
        line.density_kg_m3()
    );
    assert_eq!(
        cherry(3000.0).force_line(),
        Ok(line),
        "the force line must not read the Janka value"
    );
}

#[test]
fn solid_wood_by_janka_refuses_without_a_printed_density() {
    // A Wood Database row carries Janka only, and an FPL row where FPL
    // prints "—" for the SG carries no density. Both refuse with
    // `NoDensity`, and the gates refuse via MaterialUnvalidated.
    let wood_database_row = Material::SolidWoodByJanka {
        janka_lbf: 950.0,
        label: "Cherry, black".to_owned(),
        source_id: "wood_database_2026-05-30".to_owned(),
    };
    let fpl_row_with_no_sg = Material::SolidWoodByJanka {
        janka_lbf: 1580.0,
        label: "Honeylocust".to_owned(),
        source_id: "fpl_ch5_2010".to_owned(),
    };
    assert_eq!(
        wood_database_row.force_line(),
        Err(ForceLineRefusal::NoDensity)
    );
    assert_eq!(
        fpl_row_with_no_sg.force_line(),
        Err(ForceLineRefusal::NoDensity)
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
