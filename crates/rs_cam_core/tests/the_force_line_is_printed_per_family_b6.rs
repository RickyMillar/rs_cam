//! Sentry: **the force line is printed per family** (ruling B6,
//! 2026-09-25).
//!
//! ## What ruling B6 changed
//!
//! Until B6 every material carried one scalar `Kc`, and `feeds::force`
//! scaled one woodresearch.sk MDF fit by `Kc / 35.1`. Power also took a
//! 2.0 grain factor. B6 removed the scalar, the anchor and the factor. One
//! typed force line per material replaces them
//! (`Material::force_line() -> Result<ForceLine, ForceLineRefusal>`):
//!
//! - Solid wood: the Curti 2021 density law, Table 5, helix 0. Each term
//!   takes its upper envelope over the grain angle (0-180°) and over up-
//!   and down-milling. `Ks = Ks_n · ρ` and `F_edge = Int_n · ρ`.
//! - The density is `ρ = SG × 1000 × 1.12` from FPL Table 5-3a (12 % MC).
//!   A species with no Table 5-3a row reads the Table 5-5a basic SG `Gb`,
//!   converted by FPL Ch.4 Eq. (4-11), `G12 = Gb / [1 − 0.265 Gb (1 −
//!   12/30)]`.
//! - MDF: the Goli 2018 printed line.
//! - Every other material refuses with a named reason.
//!
//! Plan: `planning/extrapolation_2026-09-24/B6_PLAN.md` §7.
//!
//! ## How the expected numbers are made
//!
//! This file does not call the envelope function to get the expected
//! envelope. It re-declares the eight printed Curti coefficients and the
//! printed SGs, takes the maxima itself, and compares the crate against
//! that arithmetic and against the plan's printed tables.
//!
//! ## The arms (plan §7 items)
//!
//! 1. `the_envelope_is_the_printed_quadratics_maximum` — item 1.
//! 2. `each_species_line_is_the_density_law_at_its_fpl_density` — item 2,
//!    with radiata pine and jarrah (Table 5-5a) added.
//! 3. `mdf_is_the_goli_2018_printed_line` — item 3.
//! 4. `every_line_has_grain_factor_one_and_power_is_the_hand_formula` —
//!    item 4.
//! 5. `every_section_three_material_refuses_with_its_variant` — item 5.
//! 6. `the_density_range_holds_its_ends` — item 6.
//! 7. `the_chip_regime_names_its_side_and_power_still_answers` — item 7.
//! 8. `every_consumer_reads_one_line` and
//!    `every_consumer_abstains_on_plywood` — item 8.
//! 9. `no_removed_token_is_left_in_the_core_source` — item 9.
//! 10. `the_card_headlines_are_the_plan_strings` — item 10.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::cutter::build_cutter;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::efficiency::cut_efficiency;
use rs_cam_core::feeds::force::{immersion_angle, lateral_cutting_force};
use rs_cam_core::feeds::predict::{
    DeflectionUnmodeled, predict_peak_deflection_um, tip_deflection_from_engagement,
};
use rs_cam_core::feeds::{
    ChiploadBounds, ChiploadSource, FeedsDerates, FeedsResult, PowerUnmodeled, RampBasis,
    RampFallback, power_at_operating_point,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::force_line::{
    CURTI_2021_FORM_ID, CURTI_DOWN_HELIX0_INT_N, CURTI_DOWN_HELIX0_KS_N, CURTI_UP_HELIX0_INT_N,
    CURTI_UP_HELIX0_KS_N, ChipRegime, DensitySource, ForceLine, ForceLineRefusal, FplSgRow,
    GOLI_2018_FORM_ID, GrainQuadratic, WoodDensity, curti_helix0_envelope,
};
use rs_cam_core::material::wood_species_library::wood_species_library;
use rs_cam_core::material::{
    AluminumAlloy, FiberglassGrade, FoamDensity, Material, PlasticFamily, PlywoodGrade,
    SheetGoodKind, WoodSpecies,
};

// ── The printed constants, re-declared ──────────────────────────────────

/// Curti 2021 Table 5, helix 0, as printed: `(a, b, c)` of `a·GA² + b·GA +
/// c`. Units: N/mm² per kg/m³ (Ks) and N/mm per kg/m³ (Int).
///
/// - up, Ks: "−5 ⋅ 10−6 GA2 + 1⋅10−3 GA + 26⋅10−3"
/// - up, Int: "−1⋅10−7 GA2 + 1⋅10−5 GA + 55⋅10−4"
/// - down, Ks: "−3⋅10−6 GA2 + 5 ⋅ 10−4 GA + 56⋅10−3"
/// - down, Int: "−3⋅10−7 GA2 + 4⋅10−5 A + 47⋅10−4" (the source prints "A"
///   for "GA")
const UP_KS: (f64, f64, f64) = (-5e-6, 1e-3, 26e-3);
const UP_INT: (f64, f64, f64) = (-1e-7, 1e-5, 55e-4);
const DOWN_KS: (f64, f64, f64) = (-3e-6, 5e-4, 56e-3);
const DOWN_INT: (f64, f64, f64) = (-3e-7, 4e-5, 47e-4);

/// The plan's printed envelope (§2.1): Ks_n 0.0768333, Int_n 0.0060333.
const PLAN_KS_N: f64 = 0.076_833_3;
const PLAN_INT_N: f64 = 0.006_033_3;

/// FPL Table 5-3a is at 12 % MC; `ρ = SG × 1000 × 1.12`.
const RHO_PER_SG: f64 = 1000.0 * 1.12;

fn at(q: (f64, f64, f64), ga: f64) -> f64 {
    q.0 * ga * ga + q.1 * ga + q.2
}

/// The maximum of one printed quadratic over GA 0-180°: the two ends, and
/// the vertex `−b / 2a` of a downward parabola when it lies inside.
fn max_over_grain(q: (f64, f64, f64)) -> f64 {
    let mut best = at(q, 0.0).max(at(q, 180.0));
    if q.0 < 0.0 {
        let vertex = -q.1 / (2.0 * q.0);
        if (0.0..=180.0).contains(&vertex) {
            best = best.max(at(q, vertex));
        }
    }
    best
}

/// `(Ks_n, Int_n)`: per term, the larger of the up and down maxima.
fn local_envelope() -> (f64, f64) {
    (
        max_over_grain(UP_KS).max(max_over_grain(DOWN_KS)),
        max_over_grain(UP_INT).max(max_over_grain(DOWN_INT)),
    )
}

/// FPL Ch.4 Eq. (4-11) at 12 % MC with MCfs 30 %.
fn g12_from_basic(gb: f64) -> f64 {
    gb / (1.0 - 0.265 * gb * (1.0 - 12.0 / 30.0))
}

fn solid(species: WoodSpecies) -> Material {
    Material::SolidWood { species }
}

fn line_of(material: &Material) -> ForceLine {
    material
        .force_line()
        .unwrap_or_else(|r| panic!("{} must have a force line, got {r:?}", material.label()))
}

fn rel_close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol * a.abs().max(b.abs()).max(1e-300)
}

// ── 1. The envelope ─────────────────────────────────────────────────────

#[test]
fn the_envelope_is_the_printed_quadratics_maximum() {
    // The crate stores the eight printed coefficients, not a derived value.
    for (name, crate_q, printed) in [
        ("up Ks", CURTI_UP_HELIX0_KS_N, UP_KS),
        ("up Int", CURTI_UP_HELIX0_INT_N, UP_INT),
        ("down Ks", CURTI_DOWN_HELIX0_KS_N, DOWN_KS),
        ("down Int", CURTI_DOWN_HELIX0_INT_N, DOWN_INT),
    ] {
        assert_eq!(
            crate_q,
            GrainQuadratic::new(printed.0, printed.1, printed.2),
            "{name}: the crate must store the printed Curti 2021 Table 5 coefficients"
        );
    }

    let (ks_n, int_n) = local_envelope();
    assert!(
        (ks_n - PLAN_KS_N).abs() < 1e-7 && (int_n - PLAN_INT_N).abs() < 1e-7,
        "the maxima of the printed quadratics are ({ks_n}, {int_n}), not the plan's \
         ({PLAN_KS_N}, {PLAN_INT_N})"
    );
    let (crate_ks_n, crate_int_n) = curti_helix0_envelope();
    assert!(
        (crate_ks_n - ks_n).abs() < 1e-12 && (crate_int_n - int_n).abs() < 1e-12,
        "the crate envelope ({crate_ks_n}, {crate_int_n}) is not the maximum of the printed \
         quadratics ({ks_n}, {int_n})"
    );

    // The per-term envelope costs at most 0.5 % above the worst single
    // (mode, grain angle) state at h 0.05 and 0.10 mm (plan: 0.28 % and
    // 0.31 %).
    for h in [0.05_f64, 0.10] {
        let envelope = ks_n * h + int_n;
        let mut worst = 0.0_f64;
        for (ks_q, int_q) in [(UP_KS, UP_INT), (DOWN_KS, DOWN_INT)] {
            for step in 0..=1800_u32 {
                let ga = f64::from(step) / 10.0;
                worst = worst.max(at(ks_q, ga) * h + at(int_q, ga));
            }
        }
        let excess = envelope / worst - 1.0;
        assert!(
            (0.0..=0.005).contains(&excess),
            "h {h} mm: the envelope {envelope} is {:.3} % above the worst single state {worst}",
            100.0 * excess
        );
    }
}

// ── 2. The species lines ────────────────────────────────────────────────

#[test]
fn each_species_line_is_the_density_law_at_its_fpl_density() {
    let (ks_n, int_n) = local_envelope();

    // Plan §2.3 and §2.4: (species, the printed SG, ρ, Ks, F_edge). The
    // generic SGs are the means of the FPL rows the old `Kc` comments
    // named.
    let generic_softwood_sg = (0.40 + 0.36 + 0.32) / 3.0;
    let generic_hardwood_sg = (0.64 + 0.54 + 0.63) / 3.0;
    let table_5_3a = [
        (
            WoodSpecies::GenericSoftwood,
            generic_softwood_sg,
            403.2,
            30.98,
            2.433,
        ),
        (
            WoodSpecies::GenericHardwood,
            generic_hardwood_sg,
            675.7,
            51.92,
            4.077,
        ),
        (WoodSpecies::LongleafPine, 0.59, 660.8, 50.77, 3.987),
        (WoodSpecies::HardMaple, 0.63, 705.6, 54.21, 4.257),
        (WoodSpecies::Walnut, 0.55, 616.0, 47.33, 3.717),
        (WoodSpecies::Birch, 0.62, 694.4, 53.35, 4.190),
        (WoodSpecies::WhiteOak, 0.68, 761.6, 58.52, 4.595),
    ];
    for (species, sg, rho, ks, fe) in table_5_3a {
        let line = line_of(&solid(species));
        let local_rho = sg * RHO_PER_SG;
        assert!(
            (local_rho - rho).abs() < 0.1,
            "{species:?}: SG {sg} gives ρ {local_rho}"
        );
        assert!(
            line.specific_gravity()
                .is_some_and(|got| (got - sg).abs() < 1e-9),
            "{species:?}: SG {:?}, want {sg}",
            line.specific_gravity()
        );
        assert!(
            line.density_kg_m3()
                .is_some_and(|got| (got - rho).abs() < 0.1),
            "{species:?}: ρ {:?}, want {rho}",
            line.density_kg_m3()
        );
        assert!(
            (line.ks_n_per_mm2() - ks).abs() < 0.01
                && rel_close(line.ks_n_per_mm2(), ks_n * local_rho, 1e-12),
            "{species:?}: Ks {}, want {ks} (= {ks_n} × {local_rho})",
            line.ks_n_per_mm2()
        );
        assert!(
            (line.f_edge_n_per_mm() - fe).abs() < 0.001
                && rel_close(line.f_edge_n_per_mm(), int_n * local_rho, 1e-12),
            "{species:?}: F_edge {}, want {fe} (= {int_n} × {local_rho})",
            line.f_edge_n_per_mm()
        );
        assert_eq!(line.form_id(), CURTI_2021_FORM_ID);
        assert_eq!(
            line.chip_range_mm(),
            (0.04, 0.10),
            "Curti 2021 mean chip range"
        );
    }

    // The two generic species name their three rows.
    for species in [WoodSpecies::GenericSoftwood, WoodSpecies::GenericHardwood] {
        let source = species.fpl_density().expect("generic density").source();
        assert!(
            matches!(source, DensitySource::FplMean(rows) if rows.len() == 3),
            "{species:?} must be the mean of three FPL rows"
        );
    }

    // No Table 5-3a row: FPL Table 5-5a basic SG, by Eq. (4-11). Radiata
    // pine ("Pine, radiata ... Green 0.42") and jarrah ("Jarrah ... Green
    // 0.67") land inside the Curti range: 504.1 and 839.9 kg/m³.
    for (species, gb, rho, ks, fe) in [
        (WoodSpecies::RadiataPine, 0.42, 504.1, 38.73, 3.041),
        (WoodSpecies::Jarrah, 0.67, 839.9, 64.53, 5.067),
    ] {
        let line = line_of(&solid(species));
        let local_rho = g12_from_basic(gb) * RHO_PER_SG;
        assert!(
            (local_rho - rho).abs() < 0.1,
            "{species:?}: Gb {gb} gives ρ {local_rho}"
        );
        assert_eq!(line.basic_specific_gravity(), Some(gb), "{species:?}");
        assert!(
            line.specific_gravity()
                .is_some_and(|got| (got - g12_from_basic(gb)).abs() < 1e-12),
            "{species:?}: G12 {:?}",
            line.specific_gravity()
        );
        assert!(
            line.density_kg_m3()
                .is_some_and(|got| (got - rho).abs() < 0.1),
            "{species:?}: ρ {:?}, want {rho}",
            line.density_kg_m3()
        );
        assert!(
            (line.ks_n_per_mm2() - ks).abs() < 0.01
                && rel_close(line.ks_n_per_mm2(), ks_n * local_rho, 1e-12),
            "{species:?}: Ks {}, want {ks}",
            line.ks_n_per_mm2()
        );
        assert!(
            (line.f_edge_n_per_mm() - fe).abs() < 0.001
                && rel_close(line.f_edge_n_per_mm(), int_n * local_rho, 1e-12),
            "{species:?}: F_edge {}, want {fe}",
            line.f_edge_n_per_mm()
        );
        assert!(
            matches!(
                species.fpl_density().expect("5-5a density").source(),
                DensitySource::FplBasicRow(_)
            ),
            "{species:?} must read a Table 5-5a row"
        );
    }
}

// ── 3. MDF ──────────────────────────────────────────────────────────────

#[test]
fn mdf_is_the_goli_2018_printed_line() {
    let line = line_of(&Material::SheetGood {
        kind: SheetGoodKind::Mdf,
    });
    // Goli 2018 Table 3, MDF, up-milling: "Ks [N mm−2] | 31.44 (2.68)" and
    // "Int [N mm−1] | 3.36 (0.27)"; mean chip 0.041-0.091 mm.
    assert!(
        (line.ks_n_per_mm2() - 31.44).abs() < 1e-12,
        "Ks {}",
        line.ks_n_per_mm2()
    );
    assert!(
        (line.f_edge_n_per_mm() - 3.36).abs() < 1e-12,
        "F_edge {}",
        line.f_edge_n_per_mm()
    );
    assert_eq!(line.chip_range_mm(), (0.041, 0.091));
    assert!(
        (line.grain_factor() - 1.0).abs() < 1e-15,
        "MDF is isotropic in plane"
    );
    assert_eq!(line.form_id(), GOLI_2018_FORM_ID);
    assert_eq!(
        line.density_kg_m3(),
        None,
        "the Goli line is printed, not scaled"
    );
    assert_eq!(line, ForceLine::goli_2018_mdf());
}

// ── 4. Grain factor, and the power terms ────────────────────────────────

#[test]
fn every_line_has_grain_factor_one_and_power_is_the_hand_formula() {
    let mut ok_lines = 0_usize;
    let catalog = Material::catalog();
    let library = wood_species_library()
        .iter()
        .map(|e| Material::SolidWoodByJanka {
            janka_lbf: e.janka_lbf,
            label: e.display_name.clone(),
            source_id: e.source_id.clone(),
        });
    for material in catalog.into_iter().map(|(_, m)| m).chain(library) {
        if let Ok(line) = material.force_line() {
            ok_lines += 1;
            assert!(
                (line.grain_factor() - 1.0).abs() < 1e-15,
                "{}: grain factor {} — every shipped line has 1.0 (ruling B6)",
                material.label(),
                line.grain_factor()
            );
        }
    }
    assert!(
        ok_lines > 10,
        "non-vacuity: only {ok_lines} materials carry a line"
    );

    // `PowerTerms` (crate-private) through its public door: the figure at
    // the operation's feed, and at a zero feed (the edge floor), against
    // `P = A · (Ks · ap·ae · feed + F_edge · ap · π·D·n · z·ψ/2π) / 60e6`.
    let material = solid(WoodSpecies::GenericHardwood);
    let line = line_of(&material);
    let (ap, ae, feed, rpm) = (2.0, 3.0, 1700.0, 18_000.0);
    let figure = power_at_operating_point(
        &pocket(ap, ae, feed, rpm),
        &endmill(),
        &material,
        &MachineProfile::generic_wood_router(),
        None,
    )
    .expect("a hardwood pocket is a modelled cut");
    let psi = (1.0 - ae / (DIAMETER_MM / 2.0)).acos();
    // The grain factor rides on both terms; the loop above pins it at 1.0.
    let a = line.grain_factor();
    let shear = a * line.ks_n_per_mm2() * ap * ae * feed / 60_000_000.0;
    let edge = a
        * line.f_edge_n_per_mm()
        * ap
        * (std::f64::consts::PI * DIAMETER_MM * rpm)
        * (f64::from(FLUTES) * psi / std::f64::consts::TAU)
        / 60_000_000.0;
    assert!(
        rel_close(figure.required_kw, shear + edge, 1e-12),
        "power {} kW, hand formula {} kW",
        figure.required_kw,
        shear + edge
    );
    assert!(
        rel_close(figure.required_kw_at_feed(0.0), edge, 1e-12),
        "edge floor {} kW, hand formula {edge} kW",
        figure.required_kw_at_feed(0.0)
    );
    assert_eq!(
        figure.force.line, line,
        "the figure names the line it was built from"
    );
}

// ── 5. The refusals ─────────────────────────────────────────────────────

#[test]
fn every_section_three_material_refuses_with_its_variant() {
    let mut cases: Vec<(Material, ForceLineRefusal)> = Vec::new();
    for grade in [
        PlywoodGrade::Softwood,
        PlywoodGrade::BalticBirch,
        PlywoodGrade::HardwoodFaced,
    ] {
        cases.push((Material::Plywood { grade }, ForceLineRefusal::Plywood));
    }
    for family in [
        PlasticFamily::Generic,
        PlasticFamily::Acrylic,
        PlasticFamily::Hdpe,
        PlasticFamily::Delrin,
        PlasticFamily::Polycarbonate,
        PlasticFamily::UhmwPe,
        PlasticFamily::Polypropylene,
        PlasticFamily::Nylon66,
        PlasticFamily::Abs,
        PlasticFamily::Petg,
        PlasticFamily::RigidPvc,
    ] {
        cases.push((Material::Plastic { family }, ForceLineRefusal::Plastic));
    }
    for alloy in [
        AluminumAlloy::Alloy6061T6,
        AluminumAlloy::Alloy7075T6,
        AluminumAlloy::Alloy2024T3,
        AluminumAlloy::Alloy5052H32,
        AluminumAlloy::Alloy3003H14,
        AluminumAlloy::Alloy1100O,
        AluminumAlloy::Alloy7050T7651,
    ] {
        cases.push((Material::Aluminum { alloy }, ForceLineRefusal::Aluminum));
    }
    for density in [FoamDensity::Low, FoamDensity::Medium, FoamDensity::High] {
        cases.push((Material::Foam { density }, ForceLineRefusal::Foam));
    }
    for grade in [FiberglassGrade::G10Fr4, FiberglassGrade::Generic] {
        cases.push((Material::Fiberglass { grade }, ForceLineRefusal::Fiberglass));
    }
    cases.push((
        Material::SheetGood {
            kind: SheetGoodKind::Hdf,
        },
        ForceLineRefusal::Hdf,
    ));
    cases.push((
        Material::SheetGood {
            kind: SheetGoodKind::Particleboard,
        },
        ForceLineRefusal::Particleboard,
    ));
    cases.push((
        Material::Custom {
            name: "operator entry".to_owned(),
            feed_scale_factor: 1.0,
        },
        ForceLineRefusal::Custom,
    ));
    // One Wood Database row: Janka only, no density.
    let wood_database_row = wood_species_library()
        .iter()
        .find(|e| e.source_id == "wood_database_2026-05-30")
        .expect("the library holds Wood Database rows");
    cases.push((
        Material::SolidWoodByJanka {
            janka_lbf: wood_database_row.janka_lbf,
            label: wood_database_row.display_name.clone(),
            source_id: wood_database_row.source_id.clone(),
        },
        ForceLineRefusal::NoDensity,
    ));

    for (material, want) in &cases {
        assert_eq!(
            material.force_line(),
            Err(*want),
            "{} must refuse with {want:?}",
            material.label()
        );
    }
    assert_eq!(
        cases.len(),
        3 + 11 + 7 + 3 + 2 + 4,
        "every §3 case is listed"
    );

    // Ipe: FPL Table 5-5a "Ipe ... Green 0.92" → G12 1.0776, ρ 1207.0
    // kg/m³, above the Curti range, so it refuses with the density.
    let ipe_rho = g12_from_basic(0.92) * RHO_PER_SG;
    assert!((ipe_rho - 1207.0).abs() < 0.1, "Ipe ρ {ipe_rho}");
    match solid(WoodSpecies::Ipe).force_line() {
        Err(ForceLineRefusal::DensityOutOfRange { rho_kg_m3 }) => assert!(
            (rho_kg_m3 - ipe_rho).abs() < 1e-9,
            "Ipe refuses at ρ {rho_kg_m3}, want {ipe_rho}"
        ),
        other => panic!("Ipe must refuse DensityOutOfRange, got {other:?}"),
    }

    // Non-vacuity: radiata pine and jarrah now have a line (the fetch of
    // FPL Table 5-5a inside B6), so the refusal arm is not a blanket.
    for species in [WoodSpecies::RadiataPine, WoodSpecies::Jarrah] {
        assert!(
            solid(species).force_line().is_ok(),
            "{species:?} must have a line"
        );
    }
}

// ── 6. The density range ────────────────────────────────────────────────

#[test]
fn the_density_range_holds_its_ends() {
    let density = |rho: f64| {
        WoodDensity::from_density_kg_m3(
            rho,
            DensitySource::FplRow(FplSgRow::new("range edge", 0.5)),
        )
    };
    for rho in [287.0, 1080.0] {
        assert!(
            ForceLine::curti_density_law(density(rho)).is_ok(),
            "ρ {rho} is an end of the Curti range and must be inside"
        );
    }
    for rho in [286.9, 1080.1] {
        assert_eq!(
            ForceLine::curti_density_law(density(rho)),
            Err(ForceLineRefusal::DensityOutOfRange { rho_kg_m3: rho }),
            "ρ {rho} is outside 287-1080 kg/m³"
        );
    }
}

// ── 7. The chip regime ──────────────────────────────────────────────────

#[test]
fn the_chip_regime_names_its_side_and_power_still_answers() {
    let material = solid(WoodSpecies::GenericHardwood);
    let line = line_of(&material);
    assert_eq!(
        line.chip_regime(0.03),
        ChipRegime::BelowMeasured { mean_chip_mm: 0.03 }
    );
    assert_eq!(line.chip_regime(0.05), ChipRegime::Measured);
    assert_eq!(
        line.chip_regime(0.12),
        ChipRegime::AboveMeasured { mean_chip_mm: 0.12 }
    );

    // A chip below the printed range does not refuse. Half immersion on a
    // Ø6 two-flute tool (ψ = π/2) at 18 000 rev/min and 1 700 mm/min:
    // fz = 0.04722 mm and the mean chip `fz · (1 − cos ψ) / ψ` = 0.0301 mm.
    let (ap, ae, feed, rpm) = (2.0, 3.0, 1700.0, 18_000.0);
    let figure = power_at_operating_point(
        &pocket(ap, ae, feed, rpm),
        &endmill(),
        &material,
        &MachineProfile::generic_wood_router(),
        None,
    )
    .expect("a mean chip below the measured range extends the line; it does not refuse");
    let psi = std::f64::consts::FRAC_PI_2;
    let fz = feed / (rpm * f64::from(FLUTES));
    let mean_chip = fz * (1.0 - psi.cos()) / psi;
    assert!(
        (mean_chip - 0.03).abs() < 0.001,
        "fixture: the mean chip is {mean_chip}"
    );
    assert!(
        matches!(
            figure.force.chip_regime,
            ChipRegime::BelowMeasured { mean_chip_mm } if (mean_chip_mm - mean_chip).abs() < 1e-9
        ),
        "the figure must name the chip below the range: {:?}",
        figure.force.chip_regime
    );
    assert!(figure.required_kw > 0.0 && figure.required_kw.is_finite());
    assert!(
        figure.force.extrapolation_text().is_some(),
        "the card states the extrapolation"
    );
}

// ── 8. One line for every consumer ──────────────────────────────────────

const DIAMETER_MM: f64 = 6.0;
const FLUTES: u32 = 2;

fn endmill() -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = DIAMETER_MM;
    tool.shaft_diameter = DIAMETER_MM;
    tool.shank_diameter = DIAMETER_MM;
    tool.cutting_length = 22.0;
    tool.stickout = 30.0;
    tool.flute_count = FLUTES;
    tool
}

fn pocket(ap: f64, ae: f64, feed: f64, rpm: f64) -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        stepover: ae,
        depth: 4.0 * ap,
        depth_per_pass: ap,
        feed_rate: feed,
        plunge_rate: 400.0,
        spindle_rpm: Some(rpm as u32),
        ..PocketConfig::default()
    })
}

/// A `FeedsResult` at one operating point, with the fields
/// `cut_efficiency` reads.
fn operating_point(ap: f64, ae: f64, fz: f64, rpm: f64) -> FeedsResult {
    let feed = fz * rpm * f64::from(FLUTES);
    FeedsResult {
        rpm,
        chip_load_mm: fz,
        feed_rate_mm_min: feed,
        plunge_rate_mm_min: feed * 0.4,
        plunge: rs_cam_core::feeds::PlungeBasis::DrillCycle,
        ramp: RampBasis::NoChip {
            reason: RampFallback::NoLut,
        },
        axial_depth_mm: ap,
        radial_width_mm: ae,
        power_kw: 0.0,
        available_power_kw: 0.8,
        power_limited: false,
        mrr_mm3_min: ap * ae * feed,
        warnings: Vec::new(),
        vendor_source: None,
        support: rs_cam_core::feeds::FeedsSupport::FormulaOnly {
            source: rs_cam_core::feeds::support::MILLING_FORMULA_SOURCE,
        },
        chipload_source: ChiploadSource::FormulaFallback,
        chipload_bounds: Some(ChiploadBounds {
            min_mm_per_tooth: 0.05,
            max_mm_per_tooth: 0.085,
        }),
        chipload_point_mm: None,
        matched_lut_row: None,
        effective_diameter_mm: DIAMETER_MM,
        derates: FeedsDerates::default(),
    }
}

#[test]
fn every_consumer_reads_one_line() {
    let (ap, ae, rpm) = (2.0, 2.1, 18_000.0);
    let fz = 0.05;
    let feed = fz * rpm * f64::from(FLUTES);
    let psi = (1.0 - ae / (DIAMETER_MM / 2.0)).acos();
    assert!(
        (psi - immersion_angle(ae, DIAMETER_MM / 2.0)).abs() < 1e-12,
        "fixture: one immersion definition"
    );
    let machine = MachineProfile::generic_wood_router();
    let tool = endmill();

    for material in [
        solid(WoodSpecies::GenericHardwood),
        solid(WoodSpecies::RadiataPine),
        Material::SheetGood {
            kind: SheetGoodKind::Mdf,
        },
    ] {
        let line = line_of(&material);
        let (ks, fe) = (line.ks_n_per_mm2(), line.f_edge_n_per_mm());
        let label = material.label();

        // Deflection force: `F = ap · (Ks · fz · sin θ_peak + F_edge)`, with
        // θ_peak = min(ψ, π/2).
        let hand_force = ap * (ks * fz * psi.min(std::f64::consts::FRAC_PI_2).sin() + fe);
        let force = lateral_cutting_force(&material, ap, psi, fz).expect("modelled force");
        assert!(
            rel_close(force, hand_force, 1e-12),
            "{label}: force {force} N, hand {hand_force} N"
        );

        // The deflection predictor bends the tool under the same force.
        let tool_def = build_cutter(&tool);
        let e = tool_def.tool_material.youngs_modulus_n_per_mm2();
        let delta = tip_deflection_from_engagement(&tool_def, &material, ap, psi, fz)
            .expect("modelled deflection");
        let hand_delta = tool_def.tip_deflection_mm(hand_force, ap, e);
        assert!(
            rel_close(delta, hand_delta, 1e-12) && delta > 0.0,
            "{label}: deflection {delta} mm, hand {hand_delta} mm"
        );

        // Power: shear and edge from the same pair (grain factor 1.0).
        let figure =
            power_at_operating_point(&pocket(ap, ae, feed, rpm), &tool, &material, &machine, None)
                .expect("modelled power");
        let hand_kw = (ks * ap * ae * feed
            + fe * ap
                * (std::f64::consts::PI * DIAMETER_MM * rpm)
                * (f64::from(FLUTES) * psi / std::f64::consts::TAU))
            / 60_000_000.0;
        assert!(
            rel_close(figure.required_kw, hand_kw, 1e-12),
            "{label}: power {} kW, hand {hand_kw} kW",
            figure.required_kw
        );

        // Cut efficiency: `u = Ks + F_edge · D · ψ / (2 · ae · fz)`, and the
        // ploughing share `(u − Ks) / u` returns the same Ks.
        let eff = cut_efficiency(
            &pocket(ap, ae, feed, rpm),
            &tool,
            &material,
            &machine,
            &operating_point(ap, ae, fz, rpm),
        )
        .expect("modelled efficiency");
        let hand_u = ks + fe * DIAMETER_MM * psi / (2.0 * ae * fz);
        assert!(
            rel_close(eff.specific_energy_j_per_mm3, hand_u, 1e-12),
            "{label}: u {}, hand {hand_u}",
            eff.specific_energy_j_per_mm3
        );
        let ks_from_share = eff.specific_energy_j_per_mm3 * (1.0 - eff.ploughing_share);
        assert!(
            rel_close(ks_from_share, ks, 1e-9),
            "{label}: the ploughing share implies Ks {ks_from_share}, the line says {ks}"
        );
    }
}

#[test]
fn every_consumer_abstains_on_plywood() {
    let (ap, ae, rpm) = (2.0, 2.1, 18_000.0);
    let fz = 0.05;
    let feed = fz * rpm * f64::from(FLUTES);
    let psi = immersion_angle(ae, DIAMETER_MM / 2.0);
    let machine = MachineProfile::generic_wood_router();
    let tool = endmill();
    let op = pocket(ap, ae, feed, rpm);

    for grade in [
        PlywoodGrade::Softwood,
        PlywoodGrade::BalticBirch,
        PlywoodGrade::HardwoodFaced,
    ] {
        let plywood = Material::Plywood { grade };
        assert_eq!(
            lateral_cutting_force(&plywood, ap, psi, fz),
            None,
            "{grade:?}"
        );
        assert_eq!(
            tip_deflection_from_engagement(&build_cutter(&tool), &plywood, ap, psi, fz),
            None,
            "{grade:?}"
        );
        assert_eq!(
            power_at_operating_point(&op, &tool, &plywood, &machine, None).map(|p| p.required_kw),
            Err(PowerUnmodeled::MaterialUnvalidated),
            "{grade:?}"
        );
        assert!(
            cut_efficiency(
                &op,
                &tool,
                &plywood,
                &machine,
                &operating_point(ap, ae, fz, rpm)
            )
            .is_none(),
            "{grade:?}"
        );
        assert_eq!(
            predict_peak_deflection_um(&op, &tool, &plywood, &machine).map(|p| p.predicted_um),
            Err(DeflectionUnmodeled::MaterialUnvalidated),
            "{grade:?}"
        );
    }

    // Non-vacuity: the same calls answer on hardwood.
    let hardwood = solid(WoodSpecies::GenericHardwood);
    assert!(lateral_cutting_force(&hardwood, ap, psi, fz).is_some());
    assert!(power_at_operating_point(&op, &tool, &hardwood, &machine, None).is_ok());
    assert!(predict_peak_deflection_um(&op, &tool, &hardwood, &machine).is_ok());
}

// ── 9. The source scan ──────────────────────────────────────────────────

/// Every `.rs` file under `dir`, recursively.
fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|x| x == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn no_removed_token_is_left_in_the_core_source() {
    const REMOVED: [&str; 8] = [
        "LIT_KS",
        "LIT_FEDGE",
        "LIT_ANCHOR",
        "MILLING_KC_FACTOR",
        "GRAIN_ANISOTROPY_FACTOR",
        "janka_to_kc",
        "kc_n_per_mm2",
        "affine_coefficients",
    ];
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_sources(&src, &mut files);
    assert!(
        files.len() > 100,
        "non-vacuity: the scan found only {} source files under {}",
        files.len(),
        src.display()
    );
    let mut hits = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
        for (n, line) in text.lines().enumerate() {
            for token in REMOVED {
                if line.contains(token) {
                    hits.push(format!("{}:{}: {token}", file.display(), n + 1));
                }
            }
        }
    }
    assert!(
        hits.is_empty(),
        "ruling B6 removed these names; no shim, no comment that points at them:\n{}",
        hits.join("\n")
    );
}

// ── 10. The card headlines ──────────────────────────────────────────────

#[test]
fn the_card_headlines_are_the_plan_strings() {
    assert_eq!(
        line_of(&solid(WoodSpecies::GenericHardwood)).card_text(),
        "Force line: Curti 2021 density law, ρ 676 kg/m³ (FPL SG 0.60), upper envelope"
    );
    assert_eq!(
        line_of(&Material::SheetGood {
            kind: SheetGoodKind::Mdf,
        })
        .card_text(),
        "Force line: Goli 2018 MDF, printed"
    );
    let plywood = Material::Plywood {
        grade: PlywoodGrade::BalticBirch,
    }
    .force_line()
    .expect_err("plywood refuses");
    assert_eq!(plywood.headline(), "No force line: plywood (ruling B6)");
    assert_eq!(
        plywood.detail(),
        "The only plywood measurement is a figure read for poplar plywood (Goli 2023). No \
         source measures Baltic birch or softwood plywood, and the density law does not fit \
         plywood (G7 T5)."
    );
}
