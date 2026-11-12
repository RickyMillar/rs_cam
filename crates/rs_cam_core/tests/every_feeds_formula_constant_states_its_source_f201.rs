//! F201 (tech-debt review 2026-10-19), operator ruling 6 (2026-10-22),
//! option B: every constant on the feeds formula path states its source.
//!
//! The formula path is `feeds::calculate` and the `Material` methods that it
//! reads. `feeds::formula_constants()` is the registry. A row is either
//! `Cited` (a file in the repo prints the figure) or `Unsourced` (no file in
//! the repo prints it; the row records the claim of the code comment).
//!
//! The sentry checks four things:
//! 1. The registry holds the known F201 sites (non-vacuity anchor).
//! 2. A `Cited` path exists in the repo and contains the printed text.
//! 3. The material rows equal what the `Material` methods return, so a row
//!    cannot drift from the number that the formula reads.
//! 4. The count of `Unsourced` rows is pinned. The test prints the list. A
//!    new unsourced value fails the test until someone cites it or raises
//!    the pinned count on purpose. The citation task is
//!    `planning/tech_debt_2026-10-19/F201_UNSOURCED_FEEDS_CONSTANTS.md`.

#![expect(
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]
#![expect(
    clippy::print_stderr,
    reason = "the test prints the unsourced list on purpose (check 4)."
)]

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use rs_cam_core::feeds::formula_constants;
use rs_cam_core::material::{
    AluminumAlloy, ConstantSource, FiberglassGrade, FoamDensity, FormulaConstant, Material,
    PlasticFamily, PlywoodGrade, SheetGoodKind, WoodSpecies,
};

/// The pinned count of unsourced rows. Raise it only on purpose, in the same
/// change that adds the row to the planning note.
///
/// Feeds contract P5 (K2b, card rows 25 and 27) raised it from 146 to 150:
/// `feeds.suggest.STEPOVER_BACKOFF_TARGET_MOVES`,
/// `feeds.suggest.STEPOVER_BACKOFF_DIAMETER_FRACTION`,
/// `feeds.suggest.DEFLECTION_BACKOFF_TARGET_UM` and
/// `feeds.suggest.DEFLECTION_BACKOFF_DPP_FLOOR_MM`, the bases of the
/// `Runtime` and `Deflection` limits of the envelope. Card rows 4 (the drill
/// RPM tiers) and 17 (the L/D shares) had rows before P5.
///
/// The P5 chain ruling (lead, 2026-11-03) deleted the deflection back-off
/// (pass 6) and its floor, so `feeds.suggest.DEFLECTION_BACKOFF_DPP_FLOOR_MM`
/// left: 150 to 149.
///
/// Feeds contract P6 (K3, decision E4) raised it from 149 to 154: the legacy
/// project-loader default of each tool fact, at which an `Unknown` fact
/// plans: `compute.tool_config.LEGACY_DEFAULT_CUTTING_LENGTH_MM`,
/// `compute.tool_config.LEGACY_DEFAULT_TAPER_HALF_ANGLE_DEG`,
/// `compute.tool_config.LEGACY_DEFAULT_SHANK_DIAMETER_MM`,
/// `compute.tool_config.LEGACY_DEFAULT_STICKOUT_MM` and
/// `compute.tool_config.LEGACY_DEFAULT_FLUTE_COUNT`.
///
/// Feeds contract P10 (ruling D8, lead ruling on k) raised it from 154 to
/// 159: the axial geometry rule of the add door, one row per distinct old
/// default at the reference diameter 6 mm,
/// `compute.geometry_default.AXIAL_K_0_5_MM`,
/// `compute.geometry_default.AXIAL_K_1_0_MM`,
/// `compute.geometry_default.AXIAL_K_1_5_MM`,
/// `compute.geometry_default.AXIAL_K_2_0_MM` and
/// `compute.geometry_default.AXIAL_K_3_0_MM`. The radial rule reads the
/// family `ae_factor` rows, which were rows before P10.
///
/// Feeds contract P11c (D13 item 3) lowered it from 159 to 158:
/// `feeds.calculate.slotting_doc_cap_d` left the registry. The slot depth
/// cap is the machine's `RigidityProfile::slot_doc_factor` (per-machine
/// data, as the other rigidity values; the hobby grade default is the
/// Shapeoko port, Unsourced, and its basis says so).
///
/// Feeds contract P13a (architecture review 2026-11-05, H3b, L3, L4)
/// raised it from 158 to 162: four values that shipped with no row now
/// have one, `feeds.MIN_AP_MM` and `feeds.MIN_AE_MM` (the engagement
/// floor), `tool.DEFAULT_HELIX_DEG` (the helix that five cutter
/// constructors typed) and `compute.operation_configs.DRILL_DWELL_S`. No
/// value moved.
///
/// Feeds contract P13c (rulings O1, O3) lowered it from 162 to 161:
/// `feeds.operation_default_profile.Pocket.Roughing.ae_factor` is now the
/// derived corner-coverage bound `(1 + 1/sqrt 2) / 2` (Cited, the step 0
/// finding that prints it), not the typed 0.35. Two new rows are Cited:
/// `machine.RigidityProfile.hobby.force_budget_n` (the largest peak force of
/// the printed hobby slot rows, its chart row) and
/// `feeds.force_budget.corner_coverage_width_factor`. The deleted
/// `doc_roughing_factor` was machine data, not a row.
///
/// Feeds contract P16 (operator ruling D12) lowered it from 161 to 160:
/// `feeds.calculate.slotting_threshold_d` (0.85 D) left with the slot depth
/// cap; every reader of the threshold existed for the cap. The deleted
/// `slot_doc_factor` was machine data, not a row.
///
/// Feeds contract P20a (2026-11-08) lowered it from 160 to 152: the drill
/// RPM tier left. No source printed it, and the Amana Spektra chart that the
/// G6 drill claim reads prints 18000 RPM for the same tool. The eight rows
/// were `feeds.drill_rpm_envelope_for_diameter.{small_max_d_mm,
/// mid_max_d_mm, small_floor_rpm, small_ceiling_rpm, mid_floor_rpm,
/// mid_ceiling_rpm, large_floor_rpm, large_ceiling_rpm}`.
///
/// Feeds contract P21c (2026-11-10) lowered it from 152 to 142: the
/// constant-chip RPM stops at a named limit, so the speed-up factor and the
/// milling RPM tiers left. No file in the repo printed them. The ten rows
/// were `feeds.MAX_SPINDLE_SPEEDUP`,
/// `feeds.milling_rpm_ceiling_for_diameter.tier{1,2,3,4}_{max_d_mm,
/// ceiling_rpm}` and `feeds.milling_rpm_ceiling_for_diameter.large_ceiling_rpm`.
///
/// Feeds contract P29-4 commit C (2026-11-12) lowered it from 142 to 141:
/// `feeds.default_engagement.adaptive.rigidity_doc_divisor` (1.50) left with
/// the default-depth floor that read the machine `adaptive_doc_factor`. The
/// force budget caps the Adaptive depth (ruling R1). The deleted
/// `adaptive_doc_factor` was machine data, not a row.
///
/// Feeds contract P29-8 commit 8a (2026-11-12) raised it from 141 to 146:
/// the five entry numbers that were plain constants became private rows of
/// `feeds::entry_rule`, so every entry number is on the list. No file in
/// the repo prints them. The five rows are `entry_rule.ramp_angle_deg.{dressup,
/// adaptive3d}`, `entry_rule.helix_pitch_mm.{dressup, adaptive3d}` and
/// `entry_rule.helix_radius_over_d`. No number moved.
///
/// Feeds contract P29-8 commit 8b (2026-11-13) lowered it from 146 to 144:
/// the two door ramp-angle rows (`entry_rule.ramp_angle_deg.{dressup,
/// adaptive3d}`, 3 and 10 degrees) left. Operator ruling O2 (Easel) made
/// the ramp angle rule one table by the stock class, and each of its four
/// rows is `Cited` (`entry_rule.ramp_angle_deg.{hardwood,
/// softwood_and_composite, non_ferrous}` from the stored Easel page,
/// `entry_rule.ramp_angle_deg.plastic` from the R6 research note row A4).
const PINNED_UNSOURCED_COUNT: usize = 144;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn by_name(rows: &[FormulaConstant]) -> BTreeMap<String, FormulaConstant> {
    rows.iter()
        .map(|row| (row.name.to_string(), row.clone()))
        .collect()
}

fn value(rows: &BTreeMap<String, FormulaConstant>, name: &str) -> f64 {
    rows.get(name)
        .unwrap_or_else(|| panic!("F201: the registry has no row `{name}`"))
        .value
}

#[test]
fn every_feeds_formula_constant_states_its_source_f201() {
    let rows = formula_constants();
    let named = by_name(&rows);

    // Names are unique: one row per constant.
    let mut seen = HashSet::new();
    for row in &rows {
        assert!(
            seen.insert(row.name.to_string()),
            "F201: the registry names `{}` twice",
            row.name
        );
    }

    // 1. Non-vacuity: the sites that F201 names are in the registry.
    for anchor in [
        "material.base_cutting_speed_m_min.solid_wood",
        "material.base_cutting_speed_m_min.aluminum",
        "material.base_cutting_speed_m_min.fiberglass",
        "material.plunge_rate_base.solid_wood",
        "material.plunge_rate_base.reference_diameter_mm",
        "material.feed_scale_factor.exponent",
        "feeds.operation_default_profile.Adaptive.Roughing.ap_factor",
        "feeds.operation_default_profile.Face.Finish.ae_factor",
        "feeds.default_engagement.adaptive.flat_bull_ap",
        "machine.ChipLoadFormula.default.k0",
        // K4 (feeds contract P3, D5): the dressup entry plunge feed rule.
        "dressup.entry_plunge_rate.first_linear_feed_fraction",
        "dressup.entry_plunge_rate.fallback_mm_min",
        // P29-8 8a: every entry number of THE entry rule.
        // P29-8 8b: the ramp angle rule by the stock class (Easel).
        "entry_rule.ramp_angle_deg.hardwood",
        "entry_rule.ramp_angle_deg.softwood_and_composite",
        "entry_rule.ramp_angle_deg.non_ferrous",
        "entry_rule.ramp_angle_deg.plastic",
        "entry_rule.helix_pitch_mm.dressup",
        "entry_rule.helix_pitch_mm.adaptive3d",
        "entry_rule.helix_radius_over_d",
    ] {
        assert!(
            named.contains_key(anchor),
            "F201: the registry lost the known site `{anchor}`"
        );
    }
    // The profile table: 7 families × 3 roles × (ap, ae); Drill is out.
    let profile_rows = rows
        .iter()
        .filter(|r| r.name.starts_with("feeds.operation_default_profile."))
        .count();
    assert_eq!(profile_rows, 42, "F201: operation_default_profile rows");

    // 2. A Cited path exists in the repo and prints the figure.
    let mut cited = Vec::new();
    for row in &rows {
        if let ConstantSource::Cited { path, printed } = row.source {
            let full = repo_root().join(path);
            let text = std::fs::read_to_string(&full).unwrap_or_else(|e| {
                panic!(
                    "F201: `{}` cites `{path}`, which the repo does not hold: {e}",
                    row.name
                )
            });
            assert!(
                text.contains(printed),
                "F201: `{}` cites `{path}`, but the file does not contain {printed:?}",
                row.name
            );
            cited.push(row.name.to_string());
        }
    }
    // The cited drill ceilings follow from the printed ramp-down figures
    // (in/min at the 3 mm row, P20a): 72.5 × 25.4 / 3.0 and 90 × 25.4 / 3.0.
    let row_mm = 3.0;
    for (name, printed_in_min) in [
        ("material.DRILL_PLUNGE_CEILING_WOOD_PER_MM", 72.5),
        ("material.DRILL_PLUNGE_CEILING_PLYWOOD_PER_MM", 72.5),
        ("material.DRILL_PLUNGE_CEILING_SHEET_PER_MM", 90.0),
    ] {
        let derived = printed_in_min * 25.4 / row_mm;
        assert!(
            (value(&named, name) - derived).abs() < 1e-9,
            "F201: `{name}` is not {printed_in_min} in/min over 3 mm ({derived})"
        );
    }

    // 3. The material rows equal what the formula reads.
    material_rows_match_the_methods(&named);

    // 4. Print the unsourced list and pin its count.
    let unsourced: Vec<(String, f64, &str)> = rows
        .iter()
        .filter_map(|row| match row.source {
            ConstantSource::Unsourced { claim } => Some((row.name.to_string(), row.value, claim)),
            ConstantSource::Cited { .. } => None,
        })
        .collect();
    eprintln!(
        "F201: {} unsourced feeds formula constants ({} cited):",
        unsourced.len(),
        cited.len()
    );
    for (name, v, claim) in &unsourced {
        eprintln!("  {name} = {v}  [{claim}]");
    }
    assert_eq!(
        unsourced.len(),
        PINNED_UNSOURCED_COUNT,
        "F201: the unsourced count moved. Cite the new value (a file in the repo that prints it), \
         or raise PINNED_UNSOURCED_COUNT on purpose and add the row to \
         the F201 citation note (F201_UNSOURCED_FEEDS_CONSTANTS.md). Unsourced: {:?}",
        unsourced
            .iter()
            .map(|(n, _, _)| n.as_str())
            .collect::<Vec<_>>()
    );
}

fn assert_row(named: &BTreeMap<String, FormulaConstant>, name: &str, read: f64) {
    let row = value(named, name);
    assert!(
        row.to_bits() == read.to_bits(),
        "F201: row `{name}` = {row}, but the formula reads {read}"
    );
}

fn material_rows_match_the_methods(named: &BTreeMap<String, FormulaConstant>) {
    let softwood = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let hardwood = Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    };
    let plywood = Material::Plywood {
        grade: PlywoodGrade::BalticBirch,
    };
    let mdf = Material::SheetGood {
        kind: SheetGoodKind::Mdf,
    };
    let plastic = Material::Plastic {
        family: PlasticFamily::Generic,
    };
    let aluminum = Material::Aluminum {
        alloy: AluminumAlloy::Alloy6061T6,
    };
    let foam = |density| Material::Foam { density };
    let fiberglass = Material::Fiberglass {
        grade: FiberglassGrade::Generic,
    };
    let custom = Material::Custom {
        name: "f201".to_owned(),
        feed_scale_factor: 1.0,
    };

    // base_cutting_speed_m_min
    let speed = "material.base_cutting_speed_m_min";
    for (case, m) in [
        ("solid_wood", &softwood),
        ("plywood", &plywood),
        ("sheet_good", &mdf),
        ("plastic", &plastic),
        ("aluminum", &aluminum),
        ("fiberglass", &fiberglass),
        ("custom", &custom),
    ] {
        assert_row(
            named,
            &format!("{speed}.{case}"),
            m.base_cutting_speed_m_min(),
        );
    }
    assert_row(
        named,
        &format!("{speed}.foam"),
        foam(FoamDensity::Low).base_cutting_speed_m_min(),
    );

    // feed_scale_factor
    let fs = "material.feed_scale_factor";
    let reference = value(named, &format!("{fs}.janka_reference_lbf"));
    let exponent = value(named, &format!("{fs}.exponent"));
    assert_eq!(
        hardwood.feed_scale_factor().to_bits(),
        (WoodSpecies::GenericHardwood.janka_lbf() / reference)
            .powf(exponent)
            .to_bits(),
        "F201: the Janka law rows do not give the hardwood feed scale"
    );
    assert_eq!(
        aluminum.feed_scale_factor().to_bits(),
        (AluminumAlloy::Alloy6061T6.brinell_hb()
            / value(named, &format!("{fs}.aluminum_brinell_reference_hb")))
        .powf(exponent)
        .to_bits(),
        "F201: the Brinell rows do not give the aluminium feed scale"
    );
    assert_row(named, &format!("{fs}.plastic"), plastic.feed_scale_factor());
    assert_row(
        named,
        &format!("{fs}.fiberglass"),
        fiberglass.feed_scale_factor(),
    );
    for (case, density) in [
        ("foam_low", FoamDensity::Low),
        ("foam_medium", FoamDensity::Medium),
        ("foam_high", FoamDensity::High),
    ] {
        assert_row(
            named,
            &format!("{fs}.{case}"),
            foam(density).feed_scale_factor(),
        );
    }

    // Board hardness proxies.
    for (case, grade) in [
        ("softwood", PlywoodGrade::Softwood),
        ("baltic_birch", PlywoodGrade::BalticBirch),
        ("hardwood_faced", PlywoodGrade::HardwoodFaced),
    ] {
        assert_row(
            named,
            &format!("material.plywood_effective_janka_lbf.{case}"),
            grade.effective_janka_lbf(),
        );
    }
    for (case, kind) in [
        ("mdf", SheetGoodKind::Mdf),
        ("hdf", SheetGoodKind::Hdf),
        ("particleboard", SheetGoodKind::Particleboard),
    ] {
        assert_row(
            named,
            &format!("material.sheet_good_effective_janka_lbf.{case}"),
            kind.effective_janka_lbf(),
        );
    }

    // plunge_rate_base at the reference diameter: the baseline, divided by
    // the feed scale where the arm does so.
    let pr = "material.plunge_rate_base";
    let d_ref = value(named, &format!("{pr}.reference_diameter_mm"));
    for (case, m, divides) in [
        ("solid_wood", &hardwood, true),
        ("plywood_and_sheet_good", &plywood, true),
        ("plywood_and_sheet_good", &mdf, true),
        ("plastic", &plastic, false),
        ("aluminum", &aluminum, true),
        ("fiberglass", &fiberglass, true),
        ("custom", &custom, true),
    ] {
        let base = value(named, &format!("{pr}.{case}"));
        let expected = if divides {
            base / m.feed_scale_factor()
        } else {
            base
        };
        assert_eq!(
            m.plunge_rate_base(d_ref).to_bits(),
            expected.to_bits(),
            "F201: row `{pr}.{case}` does not give the plunge of {m:?}"
        );
    }
    assert_eq!(
        foam(FoamDensity::Medium).plunge_rate_base(d_ref).to_bits(),
        value(named, &format!("{pr}.foam")).to_bits(),
        "F201: row `{pr}.foam` does not give the foam plunge"
    );
    // The diameter-scale clamp.
    let scale_min = value(named, &format!("{pr}.diameter_scale_min"));
    let scale_max = value(named, &format!("{pr}.diameter_scale_max"));
    let base = plastic.plunge_rate_base(d_ref);
    assert_eq!(
        plastic.plunge_rate_base(d_ref * 1e-3).to_bits(),
        (base * scale_min).to_bits()
    );
    assert_eq!(
        plastic.plunge_rate_base(d_ref * 1e3).to_bits(),
        (base * scale_max).to_bits()
    );

    // drill_plunge_feed_envelope_per_mm
    let env = "material.drill_plunge_feed_envelope_per_mm";
    for (case, m) in [
        ("plastic", &plastic),
        ("aluminum", &aluminum),
        ("fiberglass", &fiberglass),
        ("custom", &custom),
    ] {
        let (lo, hi) = m.drill_plunge_feed_envelope_per_mm();
        assert_row(named, &format!("{env}.{case}_floor"), lo);
        assert_row(named, &format!("{env}.{case}_ceiling"), hi);
    }
    let (lo, hi) = foam(FoamDensity::High).drill_plunge_feed_envelope_per_mm();
    assert_row(named, &format!("{env}.foam_floor"), lo);
    assert_row(named, &format!("{env}.foam_ceiling"), hi);
    let (lo, hi) = softwood.drill_plunge_feed_envelope_per_mm();
    assert_row(named, "material.DRILL_PLUNGE_FLOOR_WOOD_PER_MM", lo);
    assert_row(named, "material.DRILL_PLUNGE_CEILING_WOOD_PER_MM", hi);
    let (lo, hi) = plywood.drill_plunge_feed_envelope_per_mm();
    assert_row(named, "material.DRILL_PLUNGE_FLOOR_BOARD_PER_MM", lo);
    assert_row(named, "material.DRILL_PLUNGE_CEILING_PLYWOOD_PER_MM", hi);
    let (_, hi) = mdf.drill_plunge_feed_envelope_per_mm();
    assert_row(named, "material.DRILL_PLUNGE_CEILING_SHEET_PER_MM", hi);
}
