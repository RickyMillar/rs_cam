//! G3 — a printed cell serves every family through one claim.
//!
//! Extrapolation package A3 (`planning/extrapolation_2026-09-24/A3_PLAN.md`,
//! gap group G3, family transfer). Some charts print one chip load per tool
//! and material and name no operation. The LUT files such a row under one
//! home family, (Pocket, Roughing). A family rule
//! (`feeds::extrapolation::FAMILY_RULES`) carries the home row into each
//! family that it serves, at x1.00 (copy semantics).
//!
//! The tapered arms are from A3 steps 2 and 3. Step 4 adds the bull-nose
//! rule (the Amana corner-radius chart) and the arms (a), (c), (g) and (h).
//!
//! - (a) `FAMILY_RULES` holds exactly one rule for the bull nose and one for
//!   the tapered ball; there is no ball-nose rule (ruling B3).
//! - (b) For each routed operation of a served family (Profile, Waterline,
//!   SteepShallow, Trace, Pencil, Adaptive, DropCutter, Scallop), at both
//!   home sizes and in the four judged woods, the recipe and the envelope
//!   resolvers return the Onsrud 77-100 pocket row, marked `Transferred`,
//!   with a band identical to the home band. Step 3 deleted the copies in
//!   the Adaptive, Parallel and Scallop families, so these families read
//!   the home row too. One exception: in MDF a printed Parallel row wins
//!   (SpeTool at 3.175 mm on score, Amana ZrN v8 at 6.35 mm on the tie
//!   rule), and the arm pins that row as `Printed`.
//! - (c) A 6.0 mm bull nose on a DropCutter in hardwood reads the 6.35 mm
//!   Amana corner-radius pocket row through two claims: the family claim
//!   and a G1 form C size claim. The band is the home band x the size scale
//!   x the hardness scale, on both resolvers and at the modulator's door.
//! - (d) On a tapered Trace in hardwood, Suggest, the envelope resolver and
//!   the modulator's door read the same row and the same band, and the card
//!   states the rule and the row.
//! - (e) A synthetic LUT: on an equal score, a row printed in the queried
//!   family beats a transferred row, although the transferred row's id
//!   sorts first. With no printed row, the transferred row answers.
//! - (f) A3 step 3: each rule has one row per (source, material, diameter,
//!   flutes), filed under the rule's home. The LUT holds no copy of a
//!   printed cell in another family.
//! - (g) The B3 pin: a 3.175 mm ball nose on a DropCutter in hardwood still
//!   reads the derived `amana-ball-hardwood-parallel-3175-2f` row, printed
//!   in its family, with no transfer.
//! - (h) A bull-nose finish pass in plywood refuses with
//!   `support::BULL_FINISH_PLYWOOD` (the chart prints no plywood column). A
//!   `ProjectCurve` on a bull nose routes to (Parallel, Finish); on a
//!   facing bit it still refuses. Operator ruling 2026-09-25 ("a
//!   ProjectCurve on a V-bit routes ... as a trace") means a V-bit no
//!   longer belongs beside the facing bit here: it routes to
//!   (Trace, Finish) instead.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::extrapolation::{
    FAMILY_RULE_TEXT, FAMILY_RULES, FamilyBasis, HardnessBasis,
};
use rs_cam_core::feeds::suggest::{
    StockContext, SuggestContext, SuggestParamsInput, feeds_input_for_operation, suggest_params,
};
use rs_cam_core::feeds::support::BULL_FINISH_PLYWOOD;
use rs_cam_core::feeds::vendor_lookup::{
    LookupQuery, LookupResult, find_best_chip_envelope_row, find_best_row_for_geometry,
};
use rs_cam_core::feeds::vendor_lut::{
    HardnessKind, LutOperationFamily, LutPassRole, MaterialFamily, ToolFamily, VendorLut,
};
use rs_cam_core::feeds::vendor_normalize::{lut_query_for, to_lookup_query};
use rs_cam_core::feeds::{
    EMBEDDED_LUT, FeedsSupport, SpindleStrategy, ToolGeometryHint, feeds_support,
};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, PlywoodGrade, SheetGoodKind, WoodSpecies};
use rs_cam_core::tool_load::chipload_envelope_for_toolpath;

const TOL: f64 = 1e-12;

/// A tapered ball with the tip `tip_mm` (the lookup key, ruling A1).
fn tapered(tip_mm: f64) -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId(1), ToolType::TaperedBallNose);
    t.diameter = tip_mm;
    t.flute_count = 2;
    t.cutting_length = (tip_mm * 3.0).max(12.0);
    t.taper_half_angle = 7.0;
    t.shank_diameter = (tip_mm + 3.0).max(6.0);
    t.shaft_diameter = t.shank_diameter;
    t.stickout = t.cutting_length + 8.0;
    t
}

/// A bull nose of diameter `d` with a corner radius of `d / 4` (the Amana
/// 46460 and 46462 proportion; derived, see the row notes).
fn bull(d: f64) -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId(1), ToolType::BullNose);
    t.diameter = d;
    t.flute_count = 2;
    t.corner_radius = d / 4.0;
    t.corner_radius_mm = d / 4.0;
    t.cutting_length = (d * 3.0).max(12.0);
    t.shank_diameter = d.max(6.0);
    t.shaft_diameter = t.shank_diameter;
    t.stickout = t.cutting_length + 8.0;
    t
}

/// The four judged woods, with the material key of the Onsrud row ids.
fn woods() -> [(&'static str, Material); 4] {
    [
        (
            "hardwood",
            Material::SolidWood {
                species: WoodSpecies::GenericHardwood,
            },
        ),
        (
            "softwood",
            Material::SolidWood {
                species: WoodSpecies::GenericSoftwood,
            },
        ),
        (
            "mdf",
            Material::SheetGood {
                kind: SheetGoodKind::Mdf,
            },
        ),
        (
            "plywood-hardwood",
            Material::Plywood {
                grade: PlywoodGrade::BalticBirch,
            },
        ),
    ]
}

/// The recipe and the envelope rows on the query that Suggest routes.
fn both_rows(op: OperationType, tool: &ToolConfig, material: &Material) -> [LookupResult; 2] {
    let machine = MachineProfile::default();
    let operation = OperationConfig::new_default(op);
    let input = feeds_input_for_operation(
        &operation,
        tool,
        material,
        &machine,
        &EMBEDDED_LUT,
        SpindleStrategy::MatchChart,
    );
    let query = to_lookup_query(&input).expect("the routing answers");
    let recipe = find_best_row_for_geometry(&EMBEDDED_LUT, &query, &input.tool_geometry)
        .unwrap_or_else(|| panic!("{op:?}: the recipe resolver found no row"));
    let envelope = find_best_chip_envelope_row(&EMBEDDED_LUT, &query, &input.tool_geometry)
        .unwrap_or_else(|| panic!("{op:?}: the envelope resolver found no row"));
    [recipe, envelope]
}

/// The printed MDF Parallel rows that beat the transfer (arm (b)): the
/// SpeTool 3.175 mm 2-flute row (exact, 2 flutes: 1875 against 1855) and
/// the Amana ZrN v8 6.35 mm 2-flute row (1905 against 1905; the tie rule
/// gives it to the printed row).
fn printed_mdf_parallel_row(tip: f64) -> &'static str {
    if tip < 5.0 {
        "spetool-tapered-mdf-parallel-3175-2f"
    } else {
        "amana-tapered-mdf-parallel-6350-2f-zrn-v8"
    }
}

/// (b) The routed operations of the served families read the home row
/// through one claim, with the home band.
#[test]
fn the_home_row_serves_the_unprinted_families_through_one_claim_g3() {
    let transfers = [
        (OperationType::Profile, LutOperationFamily::Contour),
        (OperationType::Waterline, LutOperationFamily::Contour),
        (OperationType::SteepShallow, LutOperationFamily::Contour),
        (OperationType::Trace, LutOperationFamily::Trace),
        (OperationType::Pencil, LutOperationFamily::Trace),
        // A3 step 3: the copies in these families are gone.
        (OperationType::Adaptive, LutOperationFamily::Adaptive),
        (OperationType::DropCutter, LutOperationFamily::Parallel),
        (OperationType::Scallop, LutOperationFamily::Scallop),
    ];
    let mut checked = 0;
    let mut printed_wins = 0;
    for (tip, column) in [(3.175, "1_8"), (6.35, "1_4")] {
        let tool = tapered(tip);
        for (key, material) in &woods() {
            let home_id = format!("onsrud-{key}-77-100-{column}-pocket");
            let [home, _] = both_rows(OperationType::Pocket, &tool, material);
            assert_eq!(
                home.observation_id, home_id,
                "the Pocket cell reads the home row"
            );
            assert_eq!(home.family_basis, FamilyBasis::Printed, "{home_id}");
            for (op, family) in transfers {
                let label = format!("{op:?} {tip} mm {key}");
                if *key == "mdf" && family == LutOperationFamily::Parallel {
                    // A row printed in the family beats the transfer.
                    for row in both_rows(op, &tool, material) {
                        assert_eq!(row.observation_id, printed_mdf_parallel_row(tip), "{label}");
                        assert_eq!(row.family_basis, FamilyBasis::Printed, "{label}");
                        printed_wins += 1;
                    }
                    continue;
                }
                for row in both_rows(op, &tool, material) {
                    assert_eq!(row.observation_id, home_id, "{label}");
                    let claim = row
                        .family_basis
                        .claim()
                        .unwrap_or_else(|| panic!("{label}: the row must be Transferred"));
                    assert_eq!(claim.query.0, family, "{label}");
                    assert_eq!(
                        claim.home,
                        (LutOperationFamily::Pocket, LutPassRole::Roughing),
                        "{label}"
                    );
                    assert_eq!(claim.source_rows, vec![home_id.clone()], "{label}");
                    assert_eq!(row.size_basis, home.size_basis, "{label}");
                    assert_eq!(row.chip_load_min_mm, home.chip_load_min_mm, "{label}");
                    assert_eq!(row.chip_load_max_mm, home.chip_load_max_mm, "{label}");
                    assert_eq!(row.chip_load_mm, home.chip_load_mm, "{label}");
                    let (_, detail) = claim.card_text();
                    assert!(detail.contains(FAMILY_RULE_TEXT), "{label}: {detail}");
                    assert!(detail.contains(&home_id), "{label}: {detail}");
                    checked += 1;
                }
            }
        }
    }
    // 2 sizes x 4 woods x 8 operations x 2 resolvers = 128, of which the
    // MDF DropCutter cells (2 sizes x 2 resolvers = 4) read a printed row.
    assert_eq!(checked, 124);
    assert_eq!(printed_wins, 4);
}

/// (f) A3 step 3: one row per printed cell. For each rule, the rows of its
/// sources and subfamily have one row per (source, material, diameter,
/// flutes), and that row is filed under the rule's home. A copy of a
/// printed cell in another family would fail here.
#[test]
fn a_rule_has_one_row_per_printed_cell_g3() {
    use std::collections::BTreeMap;
    for rule in FAMILY_RULES {
        let mut cells: BTreeMap<(String, String, u64, u32), Vec<String>> = BTreeMap::new();
        for obs in EMBEDDED_LUT.observations.iter().filter(|o| {
            o.tool_family == rule.tool_family
                && rule.source_ids.contains(&o.source_id.as_str())
                && o.tool_subfamily.as_deref() == Some(rule.tool_subfamily)
        }) {
            assert_eq!(
                (obs.operation_family, obs.pass_role),
                rule.home,
                "{}: a rule row is filed under the rule's home",
                obs.observation_id
            );
            let key = (
                obs.source_id.clone(),
                format!("{:?}", obs.material_family),
                obs.diameter_mm
                    .expect("a rule row has a diameter")
                    .to_bits(),
                obs.flute_count,
            );
            cells
                .entry(key)
                .or_default()
                .push(obs.observation_id.clone());
        }
        assert!(!cells.is_empty(), "{rule:?} has no rows");
        for (key, ids) in &cells {
            assert_eq!(
                ids.len(),
                1,
                "{key:?}: one row per printed cell, got {ids:?}"
            );
        }
        if rule.tool_family == ToolFamily::TaperedBallNose {
            // 4 sheets x 2 tip diameters.
            assert_eq!(cells.len(), 8, "{cells:?}");
        }
        if rule.tool_family == ToolFamily::BullNose {
            // 2 diameters (1/4 and 1/2 in) x softwood, hardwood and MDF.
            assert_eq!(cells.len(), 6, "{cells:?}");
        }
    }
}

/// (d) Suggest, the envelope resolver and the modulator's door read one row
/// and one band on a tapered Trace in hardwood (Ø3.175 tip, the printed
/// 1/8 in column 0.003-0.005 in/tooth = 0.0762-0.127 mm/tooth, row Janka
/// 1450 = query Janka 1450, so no scale).
#[test]
fn every_consumer_reads_the_transferred_band_g3() {
    const HOME: &str = "onsrud-hardwood-77-100-1_8-pocket";
    let tool = tapered(3.175);
    let material = Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    };
    let machine = MachineProfile::default();
    let stock = StockContext {
        stock_top_z: 0.0,
        stock_bottom_z: -18.0,
        stock_z: 18.0,
        stock_padding: 2.0,
    };
    let s = suggest_params(SuggestParamsInput {
        op_type: OperationType::Trace,
        tool: &tool,
        machine: &machine,
        material: &material,
        lut: &EMBEDDED_LUT,
        stock_ctx: &stock,
        spindle_strategy: SpindleStrategy::default(),
        context: SuggestContext::default(),
    })
    .expect("a tapered Trace in hardwood ships through the family claim");
    let FeedsSupport::FamilyTransferred { family, size } = &s.feeds_result.support else {
        panic!(
            "expected FamilyTransferred, got {:?}",
            s.feeds_result.support
        );
    };
    assert!(size.is_none(), "the tip is the printed size: {size:?}");
    assert_eq!(family.source_rows, vec![HOME.to_owned()]);
    let (headline, detail) = s.feeds_result.support.card_text();
    assert_eq!(
        headline,
        "vendor row, family transferred (G3 family): the pocket row serves this trace pass"
    );
    assert!(detail.starts_with(FAMILY_RULE_TEXT), "{detail}");
    assert!(detail.contains(HOME), "{detail}");

    let recipe = s
        .feeds_result
        .matched_lut_row
        .as_ref()
        .expect("a vendor row answered");
    assert_eq!(recipe.observation_id, HOME);
    assert!((recipe.chip_load_min_mm.expect("min") - 0.0762).abs() < TOL);
    assert!((recipe.chip_load_max_mm.expect("max") - 0.127).abs() < TOL);

    let [_, envelope] = both_rows(OperationType::Trace, &tool, &material);
    assert_eq!(envelope.observation_id, recipe.observation_id);
    assert_eq!(envelope.family_basis, recipe.family_basis);
    assert_eq!(envelope.chip_load_min_mm, recipe.chip_load_min_mm);
    assert_eq!(envelope.chip_load_max_mm, recipe.chip_load_max_mm);

    // With no simulation the depth is 0, so the de-rate is 1.0 and the
    // door's band is the row's band.
    let door = chipload_envelope_for_toolpath(
        &material,
        &tool,
        &OperationConfig::new_default(OperationType::Trace),
        ToolpathId(0),
        None,
    )
    .expect("the modulator's door reads the transferred band");
    assert!((door.start - 0.0762).abs() < TOL, "{door:?}");
    assert!((door.end - 0.127).abs() < TOL, "{door:?}");
}

/// (e) The tie rule: a row printed in the queried family beats a
/// transferred row on an equal score, and the id does not decide.
#[test]
fn a_printed_row_wins_a_tie_over_a_transfer_g3() {
    const HOME: &str = "onsrud-hardwood-77-100-1_8-pocket";
    const PRINTED: &str = "zz-synthetic-printed-trace";
    let home = EMBEDDED_LUT
        .observations
        .iter()
        .find(|o| o.observation_id == HOME)
        .expect("the home row is in the LUT")
        .clone();
    // The same printed cell, filed in the trace family with the query's
    // role. Its id sorts AFTER the home row's id, so only the tie rule can
    // make it win.
    let mut printed = home.clone();
    printed.observation_id = PRINTED.to_owned();
    printed.operation_family = LutOperationFamily::Trace;
    printed.pass_role = LutPassRole::Finish;
    assert!(
        HOME < PRINTED,
        "the id order must favour the transferred row"
    );

    let query = LookupQuery {
        tool_family: ToolFamily::TaperedBallNose,
        tool_subfamily: None,
        diameter_mm: 3.175,
        flute_count: 2,
        material_family: MaterialFamily::Hardwood,
        hardness_kind: Some(HardnessKind::Janka),
        hardness_value: Some(1450.0),
        operation_family: LutOperationFamily::Trace,
        pass_role: LutPassRole::Finish,
    };
    let geometry = ToolGeometryHint::TaperedBall {
        tip_radius: 1.5875,
        taper_angle_deg: 7.0,
    };

    // The transferred row alone answers, and scores as its copy would.
    let alone = VendorLut {
        observations: vec![home.clone()],
    };
    let transferred = find_best_row_for_geometry(&alone, &query, &geometry).expect("a row");
    assert_eq!(transferred.observation_id, HOME);
    assert_eq!(transferred.family_basis.name(), "Transferred");
    let copy_only = VendorLut {
        observations: vec![printed.clone()],
    };
    let copy = find_best_row_for_geometry(&copy_only, &query, &geometry).expect("a row");
    assert_eq!(copy.family_basis, FamilyBasis::Printed);
    assert_eq!(
        transferred.score, copy.score,
        "copy semantics: the transfer scores as its copy"
    );

    // Both rows: the tie goes to the printed row, in either file order and
    // through both resolvers.
    for observations in [vec![home.clone(), printed.clone()], vec![printed, home]] {
        let lut = VendorLut { observations };
        for row in [
            find_best_row_for_geometry(&lut, &query, &geometry).expect("a row"),
            find_best_chip_envelope_row(&lut, &query, &geometry).expect("a row"),
        ] {
            assert_eq!(row.observation_id, PRINTED);
            assert_eq!(row.family_basis, FamilyBasis::Printed);
        }
    }
}

/// (a) One rule per tool family that has a chart with no operation: the
/// bull nose and the tapered ball. No ball-nose rule (ruling B3).
#[test]
fn the_rules_are_the_bull_and_the_tapered_ball_g3() {
    let families: Vec<ToolFamily> = FAMILY_RULES.iter().map(|r| r.tool_family).collect();
    assert_eq!(families.len(), 2, "{families:?}");
    assert!(families.contains(&ToolFamily::BullNose), "{families:?}");
    assert!(
        families.contains(&ToolFamily::TaperedBallNose),
        "{families:?}"
    );
    let bull = FAMILY_RULES
        .iter()
        .find(|r| r.tool_family == ToolFamily::BullNose)
        .expect("a bull rule");
    assert_eq!(bull.source_ids, &["amana_corner_radius_spiral_plunge_2f"]);
    assert_eq!(bull.tool_subfamily, "corner_radius");
    assert_eq!(
        bull.home,
        (LutOperationFamily::Pocket, LutPassRole::Roughing)
    );
    assert_eq!(
        bull.serves,
        &[
            LutOperationFamily::Adaptive,
            LutOperationFamily::Contour,
            LutOperationFamily::Parallel,
            LutOperationFamily::Scallop,
            LutOperationFamily::Trace,
        ]
    );
    assert_eq!(bull.printed_mm, (6.35, 12.7));
}

/// (c) A 6.0 mm bull nose on a DropCutter in hardwood: the family claim and
/// a G1 form C size claim on the 6.35 mm home row. The row prints Janka
/// 1450 and the query is `GenericHardwood` 1450, so the hardness scale is
/// 1.0. The band is 0.127-0.1778 x (6.0 / 6.35)^0.61 = 0.122683-0.171756.
#[test]
fn a_bull_finish_reads_the_home_row_through_two_claims_g3() {
    const HOME: &str = "amana-bull-hardwood-pocket-6350-2f-cr";
    let tool = bull(6.0);
    let material = Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    };
    let machine = MachineProfile::default();
    let stock = StockContext {
        stock_top_z: 0.0,
        stock_bottom_z: -18.0,
        stock_z: 18.0,
        stock_padding: 2.0,
    };
    let s = suggest_params(SuggestParamsInput {
        op_type: OperationType::DropCutter,
        tool: &tool,
        machine: &machine,
        material: &material,
        lut: &EMBEDDED_LUT,
        stock_ctx: &stock,
        spindle_strategy: SpindleStrategy::default(),
        context: SuggestContext::default(),
    })
    .expect("a bull DropCutter in hardwood ships through the family claim");
    let FeedsSupport::FamilyTransferred { family, size } = &s.feeds_result.support else {
        panic!(
            "expected FamilyTransferred, got {:?}",
            s.feeds_result.support
        );
    };
    assert_eq!(family.source_rows, vec![HOME.to_owned()]);
    assert_eq!(
        family.query,
        (LutOperationFamily::Parallel, LutPassRole::Finish)
    );
    let size = size
        .as_ref()
        .expect("6.0 on the 6.35 mm row is a size claim");
    assert_eq!(size.form.name(), "C", "{size:?}");
    let expected_scale = (6.0_f64 / 6.35).powf(0.61);
    assert!((size.scale - expected_scale).abs() < 1e-12, "{size:?}");
    let (headline, detail) = s.feeds_result.support.card_text();
    assert!(
        headline.starts_with(
            "vendor row, family transferred (G3 family): the pocket row serves this parallel \
             pass; "
        ),
        "{headline}"
    );
    assert!(detail.starts_with(FAMILY_RULE_TEXT), "{detail}");
    assert!(detail.contains(HOME), "{detail}");

    let recipe = s
        .feeds_result
        .matched_lut_row
        .as_ref()
        .expect("a vendor row answered");
    assert_eq!(recipe.observation_id, HOME);
    assert_eq!(recipe.hardness_basis, HardnessBasis::Unscaled, "{recipe:?}");
    let hardness = recipe.chipload_hardness_scale;
    assert!((hardness - 1.0).abs() < TOL, "{recipe:?}");
    let min = 0.127 * size.scale * hardness;
    let max = 0.1778 * size.scale * hardness;
    assert!((min - 0.122_683).abs() < 1e-6, "{min}");
    assert!((max - 0.171_756).abs() < 1e-6, "{max}");
    assert!((recipe.chip_load_min_mm.expect("min") - min).abs() < 1e-12);
    assert!((recipe.chip_load_max_mm.expect("max") - max).abs() < 1e-12);

    let [recipe2, envelope] = both_rows(OperationType::DropCutter, &tool, &material);
    for row in [&recipe2, &envelope] {
        assert_eq!(row.observation_id, HOME);
        assert_eq!(row.family_basis, recipe.family_basis);
        assert_eq!(row.size_basis, recipe.size_basis);
        assert_eq!(row.chip_load_min_mm, recipe.chip_load_min_mm);
        assert_eq!(row.chip_load_max_mm, recipe.chip_load_max_mm);
    }
    let door = chipload_envelope_for_toolpath(
        &material,
        &tool,
        &OperationConfig::new_default(OperationType::DropCutter),
        ToolpathId(0),
        None,
    )
    .expect("the modulator's door reads the transferred band");
    assert!((door.start - min).abs() < 1e-12, "{door:?}");
    assert!((door.end - max).abs() < 1e-12, "{door:?}");
}

/// (g) The B3 pin: the ball nose has no family rule, so a 3.175 mm ball on a
/// DropCutter in hardwood keeps the derived row printed in its family.
#[test]
fn the_ball_finish_row_is_not_transferred_b3_pin_g3() {
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::BallNose);
    tool.diameter = 3.175;
    tool.flute_count = 2;
    tool.cutting_length = 12.0;
    tool.shank_diameter = 3.175;
    tool.shaft_diameter = 3.175;
    tool.stickout = 20.0;
    let material = Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    };
    for row in both_rows(OperationType::DropCutter, &tool, &material) {
        assert_eq!(row.observation_id, "amana-ball-hardwood-parallel-3175-2f");
        assert_eq!(row.family_basis, FamilyBasis::Printed);
    }
}

/// (h) A bull-nose finish in plywood refuses with the plywood text, on a
/// DropCutter (declared Parallel) and on a `ProjectCurve` (declared Trace,
/// routed to Parallel). A `ProjectCurve` on a V-bit routes to
/// (Trace, Finish) and ships the printed AMS-159 row (operator ruling
/// 2026-09-25); on a facing bit — the only cutter class the LUT truly has
/// no `ProjectCurve` rows for — the routing still refuses.
#[test]
fn a_plywood_bull_finish_refuses_and_a_vbit_curve_routes_to_trace_g3() {
    let machine = MachineProfile::default();
    let plywood = Material::Plywood {
        grade: PlywoodGrade::BalticBirch,
    };
    let tool = bull(6.0);
    for op in [OperationType::DropCutter, OperationType::ProjectCurve] {
        let input = feeds_input_for_operation(
            &OperationConfig::new_default(op),
            &tool,
            &plywood,
            &machine,
            &EMBEDDED_LUT,
            SpindleStrategy::MatchChart,
        );
        let query = to_lookup_query(&input).expect("a bull nose routes on every operation");
        assert_eq!(
            (query.operation_family, query.pass_role),
            (LutOperationFamily::Parallel, LutPassRole::Finish),
            "{op:?}"
        );
        assert!(
            find_best_row_for_geometry(&EMBEDDED_LUT, &query, &input.tool_geometry).is_none(),
            "{op:?}: no bull row serves a plywood finish"
        );
        match feeds_support(&input) {
            FeedsSupport::Refuse { reason } => assert_eq!(reason, BULL_FINISH_PLYWOOD, "{op:?}"),
            other => panic!("{op:?}: expected the plywood refusal, got {other:?}"),
        }
    }

    let mut vbit = ToolConfig::new_default(ToolId(1), ToolType::VBit);
    vbit.diameter = 6.35;
    vbit.flute_count = 2;
    vbit.included_angle = 60.0;
    let hardwood = Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    };
    let input = feeds_input_for_operation(
        &OperationConfig::new_default(OperationType::ProjectCurve),
        &vbit,
        &hardwood,
        &machine,
        &EMBEDDED_LUT,
        SpindleStrategy::MatchChart,
    );
    // Operator ruling 2026-09-25 ("a ProjectCurve on a V-bit routes ... as
    // a trace"): the tool follows the curve at a set depth, the same
    // engagement a v-carve or trace pass has, so the query routes to the
    // printed V-bit Trace rows instead of refusing.
    let query =
        to_lookup_query(&input).expect("a ProjectCurve on a V-bit now routes to Trace/Finish");
    assert_eq!(
        (query.operation_family, query.pass_role),
        (LutOperationFamily::Trace, LutPassRole::Finish)
    );
    assert!(
        find_best_row_for_geometry(&EMBEDDED_LUT, &query, &input.tool_geometry).is_some(),
        "a 60 deg V-bit ProjectCurve in hardwood must read the printed AMS-159 trace row"
    );
    match feeds_support(&input) {
        FeedsSupport::VendorBacked => {}
        other => panic!("expected VendorBacked, got {other:?}"),
    }

    // A facing bit is the one cutter class the LUT truly has no
    // `ProjectCurve` rows for (no `ToolGeometryHint` maps to it — there is
    // no facing-bit `FeedsInput` to build — so the routing itself is
    // exercised directly).
    assert_eq!(
        lut_query_for(
            OperationType::ProjectCurve,
            ToolFamily::FacingBit,
            LutOperationFamily::Trace,
            LutPassRole::Finish,
        ),
        None,
        "a ProjectCurve on a facing bit still refuses the routing"
    );
}
