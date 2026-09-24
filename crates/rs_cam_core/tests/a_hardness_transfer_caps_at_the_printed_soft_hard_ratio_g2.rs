//! G2 — a hardness transfer caps at the printed soft/hard ratio
//! (extrapolation P2 step 4, `planning/extrapolation_2026-09-24/P2_PLAN.md`
//! §4 "Softwood cap", orchestrator decision 4).
//!
//! Inside solid wood a matched row serves the query through the Janka law
//! `(row / query)^0.5`. For a hardwood row (1450) on a generic softwood
//! query (600) the law gives x1.55. No chart prints a softwood/hardwood
//! ratio that high: the largest printed ratio is 1.50 on a ball nose, 1.43
//! on a flat end mill, 1.42 on a V-bit, 1.30 on a facing bit and 1.00 on a
//! tapered ball (EXTRAPOLATION_G2 §1.2, table T2, the upper end of each
//! "softwood / hardwood" range). A3 step 4 (orchestrator decision 3) gives
//! the bull nose its own printed value, 1.33, from the Amana corner-radius
//! chart (EXTRAPOLATION_G3 §1.6: mid ratios 1.33 at 1/4 in and 1.25 at 1/2
//! in); before A3 it borrowed the flat-end 1.43.
//! `feeds::extrapolation::hardness_basis` clamps the scale
//! at the cap of the ROW's tool family. The raw ratio stays unchanged, so
//! the extrapolation flag stays set.
//!
//! The sentry pins:
//!
//! - the constants, the bull nose's printed cap 1.33 included;
//! - the 6.0 mm ball Scallop in generic softwood, on
//!   `amana-ball-hardwood-scallop-6000-2f`: scale 1.50, raw ratio 2.416667
//!   unchanged, the flag set; the recipe band, the gate band and the
//!   modulator's door agree;
//! - the 3.175 mm cell on the same row: a G1 form C claim and the cap on one
//!   row;
//! - a tapered hardwood row on a softwood query and on a softer hardwood
//!   query: capped at 1.00;
//! - a transfer under the cap, a transfer downward (Ipe) and a composite
//!   board keep their bases;
//! - the card text of a capped basis.
//!
//! How the numbers were derived (python3, from the rows in
//! `data/vendor_lut/observations/`):
//!
//! - raw ratio `1450 / 600 = 2.416666666667`; law `2.416666666667^0.5 =
//!   1.554563175515`;
//! - 6.0 mm ball: the row prints 0.025-0.04 mm/tooth at 6.0 mm (Exact), so
//!   the band is `0.025 * 1.50 = 0.0375` to `0.04 * 1.50 = 0.06` (was
//!   `0.038864 - 0.062183` under the law);
//! - 3.175 mm ball: form C scale `(3.175 / 6.0)^0.61 = 0.678252514240`, band
//!   `0.025 * 0.678252514240 * 1.50 = 0.025434469284` to
//!   `0.04 * 0.678252514240 * 1.50 = 0.040695150854`;
//! - tapered: `onsrud-hardwood-77-100-1_4-pocket` prints 0.127-0.1778 at
//!   6.35 mm with a per-row 1450. The cap 1.00 keeps the printed band. On a
//!   1000 lbf hardwood query the law gives `1.45^0.5 = 1.204159457879`,
//!   capped to 1.00 on the tapered row, and kept on the ball row (under
//!   1.50).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::extrapolation::{
    HardnessBasis, JankaFrom, SOFT_OVER_HARD_PRINTED_MAX, SoftHardCap, hardness_basis,
    soft_hard_cap,
};
use rs_cam_core::feeds::suggest::{
    StockContext, SuggestContext, SuggestParamsInput, SuggestedParams, feeds_input_for_operation,
    suggest_params,
};
use rs_cam_core::feeds::vendor_lookup::{
    LookupQuery, LookupResult, find_best_chip_envelope_row, lookup_best,
};
use rs_cam_core::feeds::vendor_lut::{
    HardnessKind, MaterialFamily, ToolFamily, VendorLut, VendorObservation,
};
use rs_cam_core::feeds::vendor_normalize::to_lookup_query;
use rs_cam_core::feeds::{EMBEDDED_LUT, FeedsSupport, SpindleStrategy};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, SheetGoodKind, WoodSpecies};
use rs_cam_core::tool_load::chipload_envelope_for_toolpath;

const TOL: f64 = 1e-12;

/// The ball row that the softwood Scallop cells reach (derived grade c,
/// per-row Janka 1450, 0.025-0.04 mm/tooth at 6.0 mm).
const BALL_SCALLOP_ROW: &str = "amana-ball-hardwood-scallop-6000-2f";

/// A tapered hardwood row with a per-row Janka 1450 (Onsrud 77-100, 1/4 in,
/// 0.127-0.1778 mm/tooth).
const TAPERED_ROW: &str = "onsrud-hardwood-77-100-1_4-pocket";

/// `1450 / 600`.
const RAW: f64 = 1450.0 / 600.0;

/// `(1450 / 600)^0.5`.
const LAW: f64 = 1.554_563_175_515;

// ── Helpers ──────────────────────────────────────────────────────────

fn softwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    }
}

fn ball(diameter: f64) -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId(1), ToolType::BallNose);
    t.diameter = diameter;
    t.flute_count = 2;
    t.cutting_length = (diameter * 3.0).max(6.0);
    t.shank_diameter = diameter.max(3.175);
    t.shaft_diameter = diameter.max(3.175);
    t.stickout = t.cutting_length + 6.0;
    t
}

fn suggest(op: OperationType, tool: &ToolConfig, material: &Material) -> SuggestedParams {
    let machine = MachineProfile::default();
    let stock = StockContext {
        stock_top_z: 0.0,
        stock_bottom_z: -18.0,
        stock_z: 18.0,
        stock_padding: 2.0,
    };
    suggest_params(SuggestParamsInput {
        op_type: op,
        tool,
        machine: &machine,
        material,
        lut: &EMBEDDED_LUT,
        stock_ctx: &stock,
        spindle_strategy: SpindleStrategy::default(),
        context: SuggestContext::default(),
    })
    .unwrap_or_else(|e| panic!("{op:?} {} mm ships: {e}", tool.diameter))
}

/// The envelope resolver's row on the query that Suggest routes for this
/// cell (the gate's resolver).
fn envelope_row(op: OperationType, tool: &ToolConfig, material: &Material) -> LookupResult {
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
    find_best_chip_envelope_row(&EMBEDDED_LUT, &query, &input.tool_geometry)
        .expect("a chipload-bearing row matches")
}

/// The embedded row with this id.
fn row(id: &str) -> VendorObservation {
    EMBEDDED_LUT
        .observations
        .iter()
        .find(|o| o.observation_id == id)
        .unwrap_or_else(|| panic!("the embedded LUT has no row {id}"))
        .clone()
}

/// The query at the row's own tool, size, operation and pass role, in
/// `family` at `janka`.
fn query_at(obs: &VendorObservation, family: MaterialFamily, janka: f64) -> LookupQuery {
    LookupQuery {
        tool_family: obs.tool_family,
        tool_subfamily: obs.tool_subfamily.clone(),
        diameter_mm: obs.diameter_mm.expect("the row has a diameter"),
        flute_count: obs.flute_count,
        material_family: family,
        hardness_kind: Some(HardnessKind::Janka),
        hardness_value: Some(janka),
        operation_family: obs.operation_family,
        pass_role: obs.pass_role,
    }
}

/// The lookup on a LUT that holds only the row `id`.
fn alone(id: &str, family: MaterialFamily, janka: f64) -> LookupResult {
    let obs = row(id);
    let query = query_at(&obs, family, janka);
    let lut = VendorLut {
        observations: vec![obs],
    };
    lookup_best(&lut, &query)
        .unwrap_or_else(|| panic!("the one-row LUT must answer its own row {id}"))
}

/// The row carries the ball-nose cap on the law 1450 -> 600.
fn assert_ball_capped(r: &LookupResult, label: &str) {
    assert_eq!(r.observation_id, BALL_SCALLOP_ROW, "{label}");
    let HardnessBasis::Capped {
        ratio_raw,
        law_scale,
        row_hardness,
        from,
        cap,
    } = &r.hardness_basis
    else {
        panic!(
            "{label}: expected a capped basis, got {:?}",
            r.hardness_basis
        );
    };
    assert!((ratio_raw - RAW).abs() < TOL, "{label}: {ratio_raw}");
    assert!((law_scale - LAW).abs() < 1e-11, "{label}: {law_scale}");
    assert!((row_hardness - 1450.0).abs() < TOL, "{label}");
    assert_eq!(*from, JankaFrom::Row, "{label}");
    assert_eq!(
        *cap,
        SoftHardCap {
            family: ToolFamily::BallNose,
            ratio: 1.50,
        },
        "{label}"
    );
    // The raw ratio is unchanged, so the flag reads the uncapped transfer;
    // the applied scale is the cap. The flag reads the COMBINED raw ratio
    // (diameter x hardness): at 6.0 mm it is 1.0 x 2.42 (flagged); at
    // 3.175 mm it is 0.529 x 2.42 = 1.28, under 1.4 (not flagged).
    assert!((r.chipload_hardness_ratio_raw - RAW).abs() < TOL, "{label}");
    assert!((r.chipload_hardness_scale - 1.50).abs() < TOL, "{label}");
    let combined = r.chipload_diameter_ratio_raw * r.chipload_hardness_ratio_raw;
    assert_eq!(
        r.is_extrapolated,
        combined.ln().abs() > 1.4_f64.ln(),
        "{label}: the flag must read the combined raw ratio {combined}"
    );
}

/// Recipe, gate and the modulator's door read one band.
fn assert_one_capped_band(op: OperationType, tool: &ToolConfig, band: (f64, f64)) -> LookupResult {
    let material = softwood();
    let label = format!("{op:?} ball {} mm softwood", tool.diameter);
    let s = suggest(op, tool, &material);
    let recipe = s
        .feeds_result
        .matched_lut_row
        .expect("a vendor row answered");
    assert_ball_capped(&recipe, &format!("{label} (recipe)"));
    let min = recipe.chip_load_min_mm.expect("the row prints a min");
    let max = recipe.chip_load_max_mm.expect("the row prints a max");
    assert!((min - band.0).abs() < 1e-11, "{label}: min {min}");
    assert!((max - band.1).abs() < 1e-11, "{label}: max {max}");

    let env = envelope_row(op, tool, &material);
    assert_ball_capped(&env, &format!("{label} (gate)"));
    assert_eq!(env.hardness_basis, recipe.hardness_basis, "{label}");
    assert_eq!(env.size_basis, recipe.size_basis, "{label}");
    assert_eq!(env.chip_load_min_mm, recipe.chip_load_min_mm, "{label}");
    assert_eq!(env.chip_load_max_mm, recipe.chip_load_max_mm, "{label}");

    // The modulator's and the advisor's door, with no simulation (no depth
    // de-rate).
    let range = chipload_envelope_for_toolpath(
        &material,
        tool,
        &OperationConfig::new_default(op),
        ToolpathId(0),
        None,
    )
    .unwrap_or_else(|| panic!("{label}: the door gave no band"));
    assert!((range.start - band.0).abs() < 1e-11, "{label}: {range:?}");
    assert!((range.end - band.1).abs() < 1e-11, "{label}: {range:?}");
    recipe
}

// ── The constants ────────────────────────────────────────────────────

#[test]
fn the_caps_are_the_largest_printed_soft_hard_ratios_g2() {
    assert_eq!(
        SOFT_OVER_HARD_PRINTED_MAX,
        &[
            (ToolFamily::BallNose, 1.50),
            (ToolFamily::FlatEnd, 1.43),
            (ToolFamily::ChamferVbit, 1.42),
            (ToolFamily::TaperedBallNose, 1.00),
            (ToolFamily::FacingBit, 1.30),
            (ToolFamily::BullNose, 1.33),
        ]
    );
    for &(family, ratio) in SOFT_OVER_HARD_PRINTED_MAX {
        assert_eq!(soft_hard_cap(family), SoftHardCap { family, ratio });
    }
    // A3 step 4: the bull nose reads its own printed pair (Amana corner
    // radius, 0.008 / 0.006 in at 1/4 in), not the flat-end 1.43.
    assert_eq!(
        soft_hard_cap(ToolFamily::BullNose),
        SoftHardCap {
            family: ToolFamily::BullNose,
            ratio: 1.33,
        },
        "a bull nose caps at its own printed ratio"
    );
    // Every tool family has an entry.
    for family in [
        ToolFamily::FlatEnd,
        ToolFamily::BallNose,
        ToolFamily::BullNose,
        ToolFamily::ChamferVbit,
        ToolFamily::TaperedBallNose,
        ToolFamily::FacingBit,
    ] {
        assert!(
            SOFT_OVER_HARD_PRINTED_MAX.iter().any(|(f, _)| *f == family),
            "{family:?} has no printed cap"
        );
    }
}

// ── The ball Scallop cells ───────────────────────────────────────────

/// The 6.0 mm cell: the row's own size, so the cap is the only scale.
#[test]
fn the_softwood_ball_scallop_caps_at_1_50_g2() {
    let r = assert_one_capped_band(OperationType::Scallop, &ball(6.0), (0.0375, 0.06));
    assert!(
        (r.chipload_diameter_scale - 1.0).abs() < TOL,
        "the row prints 6.0 mm: {r:?}"
    );
    assert!(r.size_basis.claim().is_none(), "{:?}", r.size_basis);
    assert!((r.chip_load_mm - 0.04875).abs() < 1e-11, "{r:?}");
}

/// The 3.175 mm cell: a G1 form C claim and the cap on one row. The
/// support arm is the claim; the cap is on the row.
#[test]
fn a_size_claim_and_a_cap_stack_on_one_row_g2() {
    let tool = ball(3.175);
    let r = assert_one_capped_band(
        OperationType::Scallop,
        &tool,
        (0.025_434_469_284, 0.040_695_150_854),
    );
    let claim = r
        .size_basis
        .claim()
        .expect("3.175 on the 6.0 row is form C");
    assert_eq!(claim.form.name(), "C");
    assert!((claim.scale - 0.678_252_514_240).abs() < 1e-11, "{claim:?}");
    let s = suggest(OperationType::Scallop, &tool, &softwood());
    assert!(
        matches!(&s.feeds_result.support, FeedsSupport::Extrapolated { .. }),
        "{:?}",
        s.feeds_result.support
    );
}

// ── The tapered cap ──────────────────────────────────────────────────

/// Tapered-ball charts print one band for softwood and hardwood, so the
/// cap 1.00 blocks every upward transfer on a tapered row: a softwood query
/// and a softer hardwood query read the printed band.
#[test]
fn a_tapered_hardwood_row_on_a_softer_query_caps_at_1_00_g2() {
    let obs = row(TAPERED_ROW);
    assert_eq!(obs.tool_family, ToolFamily::TaperedBallNose);
    assert_eq!(obs.hardness_value, Some(1450.0), "premise: per-row 1450");

    let soft = alone(
        TAPERED_ROW,
        MaterialFamily::Softwood,
        WoodSpecies::GenericSoftwood.janka_lbf(),
    );
    let HardnessBasis::Capped {
        ratio_raw,
        law_scale,
        cap,
        ..
    } = &soft.hardness_basis
    else {
        panic!("expected a capped basis: {:?}", soft.hardness_basis);
    };
    assert!((ratio_raw - RAW).abs() < TOL);
    assert!((law_scale - LAW).abs() < 1e-11);
    assert_eq!(cap.family, ToolFamily::TaperedBallNose);
    assert!((cap.ratio - 1.0).abs() < TOL);
    assert!((soft.chipload_hardness_scale - 1.0).abs() < TOL, "{soft:?}");
    assert!((soft.chipload_hardness_ratio_raw - RAW).abs() < TOL);
    assert!(soft.is_extrapolated, "the raw ratio 2.42 keeps the flag");
    assert!((soft.chip_load_min_mm.unwrap() - 0.127).abs() < TOL);
    assert!((soft.chip_load_max_mm.unwrap() - 0.1778).abs() < TOL);

    // A softer hardwood (1000 lbf): the law gives 1.45^0.5 = 1.2042, above
    // the tapered cap.
    let softer = alone(TAPERED_ROW, MaterialFamily::Hardwood, 1000.0);
    assert_eq!(softer.hardness_basis.name(), "Capped", "{softer:?}");
    assert!((softer.chipload_hardness_scale - 1.0).abs() < TOL);
}

// ── What the cap does not touch ──────────────────────────────────────

/// Under the cap the law stands: the ball row on a 1000 lbf hardwood query
/// gives `1.45^0.5 = 1.204159457879`, under 1.50.
#[test]
fn a_transfer_under_the_cap_keeps_the_law_g2() {
    let r = alone(BALL_SCALLOP_ROW, MaterialFamily::Hardwood, 1000.0);
    let HardnessBasis::Law {
        ratio_raw,
        scale,
        from,
        ..
    } = &r.hardness_basis
    else {
        panic!("expected the law: {:?}", r.hardness_basis);
    };
    assert!((ratio_raw - 1.45).abs() < TOL);
    assert!((scale - 1.204_159_457_879).abs() < 1e-11, "{scale}");
    assert_eq!(*from, JankaFrom::Row);
    assert!((r.chipload_hardness_scale - scale).abs() < TOL);
}

/// A harder query (Ipe, 3510) derates by the law, `(1450 / 3510)^0.5 =
/// 0.642732769590`; the cap acts only upward.
#[test]
fn a_transfer_downward_is_not_capped_g2() {
    let r = alone(
        BALL_SCALLOP_ROW,
        MaterialFamily::Hardwood,
        WoodSpecies::Ipe.janka_lbf(),
    );
    assert_eq!(r.hardness_basis.name(), "Law", "{r:?}");
    assert!(
        (r.chipload_hardness_scale - 0.642_732_769_590).abs() < 1e-11,
        "{r:?}"
    );
}

/// A composite-board query has no hardness scale (P2 step 3), so no cap.
#[test]
fn a_composite_board_query_is_not_capped_g2() {
    let obs = row("amana-ball-mdf-pocket-6350-2f-v7");
    let query = query_at(
        &obs,
        MaterialFamily::Mdf,
        SheetGoodKind::Mdf.effective_janka_lbf(),
    );
    assert_eq!(hardness_basis(&query, &obs), HardnessBasis::CompositeBoard);
}

// ── The card text ────────────────────────────────────────────────────

#[test]
fn a_capped_basis_states_its_cap_on_the_card_g2() {
    let r = alone(
        BALL_SCALLOP_ROW,
        MaterialFamily::Softwood,
        WoodSpecies::GenericSoftwood.janka_lbf(),
    );
    let (headline, detail) = r
        .hardness_basis
        .card_text()
        .expect("a capped basis has a card text");
    assert_eq!(headline, "extrapolated (G2 hardness): capped at x1.50");
    assert_eq!(
        detail,
        "hardness transfer capped at x1.50 (the largest softwood/hardwood ratio that ball \
         nose charts print); the Janka law gives x1.55 (raw ratio 2.42, row 1450 lbf, printed \
         on the row)"
    );

    // A bull nose names its own printed value (A3 step 4).
    let bull = HardnessBasis::Capped {
        ratio_raw: RAW,
        law_scale: LAW,
        row_hardness: 1450.0,
        from: JankaFrom::FamilyGeneric,
        cap: soft_hard_cap(ToolFamily::BullNose),
    };
    let (_, bull_detail) = bull.card_text().expect("capped");
    assert!(
        bull_detail.starts_with(
            "hardness transfer capped at x1.33 (the largest softwood/hardwood ratio that bull \
             nose charts print); the Janka law gives x1.55"
        ),
        "{bull_detail}"
    );
    assert!(
        bull_detail.ends_with("the generic species of the row's family)"),
        "{bull_detail}"
    );

    // A basis that is not capped has no card line.
    assert_eq!(HardnessBasis::Unscaled.card_text(), None);
    assert_eq!(HardnessBasis::CompositeBoard.card_text(), None);
}
