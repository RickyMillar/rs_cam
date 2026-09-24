//! G3 — a printed cell serves every family through one claim.
//!
//! Extrapolation package A3 (`planning/extrapolation_2026-09-24/A3_PLAN.md`,
//! gap group G3, family transfer). Some charts print one chip load per tool
//! and material and name no operation. The LUT files such a row under one
//! home family, (Pocket, Roughing). A family rule
//! (`feeds::extrapolation::FAMILY_RULES`) carries the home row into each
//! family that it serves, at x1.00 (copy semantics).
//!
//! This file holds the tapered arms of the sentry (A3 step 2). Step 3 adds
//! arm (f), and step 4 adds the arms (a), (c), (g) and (h).
//!
//! - (b) For each routed operation of a served family with no printed
//!   tapered row (Profile, Waterline, SteepShallow, Trace, Pencil), at both
//!   home sizes and in the four judged woods, the recipe and the envelope
//!   resolvers return the Onsrud 77-100 pocket row, marked `Transferred`,
//!   with a band identical to the home band. Step 3 widens this arm to
//!   Adaptive, Parallel and Scallop: until the copies in those families are
//!   deleted, the tie rule keeps the copies, which are `Printed`.
//! - (d) On a tapered Trace in hardwood, Suggest, the envelope resolver and
//!   the modulator's door read the same row and the same band, and the card
//!   states the rule and the row.
//! - (e) A synthetic LUT: on an equal score, a row printed in the queried
//!   family beats a transferred row, although the transferred row's id
//!   sorts first. With no printed row, the transferred row answers.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::extrapolation::{FAMILY_RULE_TEXT, FamilyBasis};
use rs_cam_core::feeds::suggest::{
    StockContext, SuggestContext, SuggestParamsInput, feeds_input_for_operation, suggest_params,
};
use rs_cam_core::feeds::vendor_lookup::{
    LookupQuery, LookupResult, find_best_chip_envelope_row, find_best_row_for_geometry,
};
use rs_cam_core::feeds::vendor_lut::{
    HardnessKind, LutOperationFamily, LutPassRole, MaterialFamily, ToolFamily, VendorLut,
};
use rs_cam_core::feeds::vendor_normalize::to_lookup_query;
use rs_cam_core::feeds::{EMBEDDED_LUT, FeedsSupport, SpindleStrategy, ToolGeometryHint};
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

/// (b) The routed operations with no printed tapered row read the home row
/// through one claim, with the home band.
#[test]
fn the_home_row_serves_the_unprinted_families_through_one_claim_g3() {
    let transfers = [
        (OperationType::Profile, LutOperationFamily::Contour),
        (OperationType::Waterline, LutOperationFamily::Contour),
        (OperationType::SteepShallow, LutOperationFamily::Contour),
        (OperationType::Trace, LutOperationFamily::Trace),
        (OperationType::Pencil, LutOperationFamily::Trace),
    ];
    let mut checked = 0;
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
    // 2 sizes x 4 woods x 5 operations x 2 resolvers.
    assert_eq!(checked, 80);
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
