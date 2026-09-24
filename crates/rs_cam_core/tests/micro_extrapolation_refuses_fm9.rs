//! FM9 — a micro tool does not ship a chipload extrapolated from a far
//! larger chart row (ruling R1 applied to size, operator 2026-09-24).
//!
//! The probe: tapered ball tips of 0.5 / 0.8 / 1.0 mm on a Scallop in
//! hardwood shipped 0.0347 / 0.0454 / 0.0520 mm/tooth from the Onsrud
//! 77-100 1/8 in and 1/4 in rows, scaled 6-8x down by the `(d/D)^0.61` law
//! that was fitted on 3-12 mm rows. No chart publishes a figure at that size.
//!
//! The rule (`feeds::support::micro_extrapolation_refusal`): a tool whose
//! lookup diameter is under `MICRO_TOOL_DIAMETER_MM` (1.5 mm) ships only
//! from a row whose diameter is `MICRO_ROW_RATIO_MIN` (0.5x) to
//! `MICRO_ROW_RATIO_MAX` (2x) of its own. Otherwise `feeds_support`
//! refuses with a reason that names both sizes, and
//! `validate_tool_for_operation` raises `FeedsError::Unbacked`.
//!
//! Ruling A1 (2026-09-24): the lookup key of a tapered ball is its tip,
//! not the engaged cone diameter. So a tapered refusal names the tip only,
//! with no "(engaged ...)" note. Ruling B1: a tapered tip under
//! `TAPERED_MIN_TIP_MM` (0.5 mm) refuses before the size rule runs.
//!
//! The arms:
//!
//! - a 0.5 mm tapered ball Scallop in hardwood refuses, and the text names
//!   0.5 mm and the row's 3.175 mm;
//! - a 0.3 mm tapered ball Parallel (DropCutter) in hardwood refuses on the
//!   tip floor (ruling B1), though the 0.5 mm SpeTool row is inside the 2x
//!   window; a 0.3 mm tapered Scallop, which matches no row at all (every
//!   Scallop row is more than 10x the tip), refuses on the same floor and
//!   does not fall to the formula;
//! - a 1.0 mm tapered ball Scallop in hardwood refuses;
//! - a 1.0 mm ball nose Scallop in hardwood refuses: the only ball Scallop
//!   rows are the Ø6 hardwood rows (6x); the 1 mm ball rows are Parallel
//!   rows, and the MDF one is another material category;
//! - a 3.175 mm tapered ball Scallop in hardwood ships (not a micro tool);
//! - a 1.0 mm flat end mill pocket in SOFTWOOD ships: the printed Spektra
//!   0.794 mm softwood row is 0.79x the tool, inside the window. Since
//!   extrapolation P1 step 3 it ships `Extrapolated` through a G1 form A
//!   claim between the printed 0.79375 mm and 1.5 mm rows.
//!
//! Since extrapolation P1 step 3 the size rule runs inside the lookup
//! (`feeds::extrapolation::SizeLaw`, `LookupResult::size_basis`), and the
//! refusal texts above are its texts for a tool under 1.5 mm.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::suggest::{
    StockContext, SuggestContext, SuggestParamsInput, SuggestedParams, suggest_params,
};
use rs_cam_core::feeds::support::{
    MICRO_ROW_RATIO_MAX, MICRO_ROW_RATIO_MIN, MICRO_TOOL_DIAMETER_MM, TAPERED_MIN_TIP_MM,
    micro_extrapolation_refusal, tapered_tip_floor_refusal,
};
use rs_cam_core::feeds::vendor_lut::ToolFamily;
use rs_cam_core::feeds::{EMBEDDED_LUT, FeedsError, FeedsSupport, SpindleStrategy};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

fn tool_of(kind: ToolType, diameter: f64) -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId(1), kind);
    t.diameter = diameter;
    t.flute_count = 2;
    t.cutting_length = (diameter * 3.0).max(6.0);
    t.shank_diameter = diameter.max(3.175);
    t.shaft_diameter = diameter.max(3.175);
    t.stickout = t.cutting_length + 6.0;
    if matches!(kind, ToolType::TaperedBallNose) {
        t.taper_half_angle = 7.0;
        t.shank_diameter = (diameter + 3.0).max(6.0);
        t.shaft_diameter = t.shank_diameter;
    }
    t
}

fn stock_ctx() -> StockContext {
    StockContext {
        stock_top_z: 0.0,
        stock_bottom_z: -18.0,
        stock_z: 18.0,
        stock_padding: 2.0,
    }
}

fn suggest(
    op: OperationType,
    tool: &ToolConfig,
    material: &Material,
) -> Result<SuggestedParams, FeedsError> {
    let machine = MachineProfile::default();
    let stock = stock_ctx();
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
}

fn hardwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    }
}

fn softwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    }
}

fn size_reason(err: &FeedsError) -> String {
    match err {
        FeedsError::Unbacked { reason, .. } => reason.to_string(),
        other => panic!("expected Unbacked, got {other:?}"),
    }
}

#[test]
fn the_rule_by_hand_fm9() {
    assert_eq!(MICRO_TOOL_DIAMETER_MM, 1.5);
    assert_eq!(MICRO_ROW_RATIO_MAX, 2.0);
    assert_eq!(MICRO_ROW_RATIO_MIN, 0.5);
    // 3.175 / 0.5 = 6.35x: refuse, and the text names both sizes.
    // The function reads the key that it is given. Nominal 0.5 mm, key
    // 0.5651 mm: 3.175 / 0.5651 = 5.6x, so refuse, and the text names the
    // nominal diameter, the engaged key and the row. Since ruling A1 a
    // tapered caller passes its tip as the key, so this pair of diameters
    // now comes only from a V-bit caller (key = engaged width). The raw
    // call keeps the text format under test.
    let r = micro_extrapolation_refusal(ToolFamily::TaperedBallNose, 0.5, 0.5651, 3.175)
        .expect("5.6x off a 0.57 mm engaged diameter refuses");
    assert!(
        r.starts_with(
            "no published figure for a 0.50 mm tapered ball nose (engaged 0.57 mm at the cut depth); \
             the nearest chart row is 3.175 mm, 5.6x the tool"
        ),
        "{r}"
    );
    // The engaged note is left out when the two diameters agree.
    let flat = micro_extrapolation_refusal(ToolFamily::FlatEnd, 1.0, 1.0, 6.0).unwrap();
    assert!(!flat.contains("engaged"), "{flat}");
    // 0.794 / 1.0 = 0.794x: inside the window.
    assert!(micro_extrapolation_refusal(ToolFamily::FlatEnd, 1.0, 1.0, 0.794).is_none());
    // 2.0x exactly is inside; just over is not.
    assert!(micro_extrapolation_refusal(ToolFamily::FlatEnd, 1.0, 1.0, 2.0).is_none());
    assert!(micro_extrapolation_refusal(ToolFamily::FlatEnd, 1.0, 1.0, 2.01).is_some());
    // The rule reads the LOOKUP diameter: a key of 1.6 mm is not a micro
    // tool, whatever the nominal diameter. (Since ruling A1 a tapered caller
    // passes its tip as the key; a V-bit caller passes its engaged width.)
    assert!(micro_extrapolation_refusal(ToolFamily::TaperedBallNose, 1.4, 1.6, 6.35).is_none());
    // A 1.5 mm tool is not a micro tool.
    assert!(micro_extrapolation_refusal(ToolFamily::FlatEnd, 1.5, 1.5, 6.0).is_none());

    // Ruling B1: the tapered tip floor. Under 0.5 mm refuses; 0.5 mm exactly
    // passes to the size rule; other families are not judged by it.
    assert_eq!(TAPERED_MIN_TIP_MM, 0.5);
    assert_eq!(
        tapered_tip_floor_refusal(ToolFamily::TaperedBallNose, 0.3).as_deref(),
        Some(
            "no published figure for a 0.30 mm tapered ball nose: no wood chart prints a tip \
             under 0.5 mm (ruling B1)"
        )
    );
    assert!(tapered_tip_floor_refusal(ToolFamily::TaperedBallNose, 0.5).is_none());
    assert!(tapered_tip_floor_refusal(ToolFamily::BallNose, 0.3).is_none());
}

#[test]
fn a_half_millimetre_tapered_scallop_refuses_with_both_sizes_fm9() {
    let err = suggest(
        OperationType::Scallop,
        &tool_of(ToolType::TaperedBallNose, 0.5),
        &hardwood(),
    )
    .expect_err("a 0.5 mm tapered ball has no Scallop chart row near its size");
    let reason = size_reason(&err);
    // Ruling A1: the key is the 0.5 mm tip, so the text carries no
    // "(engaged ...)" note. The tip is not under 0.5 mm, so the size rule,
    // not the B1 tip floor, refuses: the nearest Scallop row is 3.175 mm,
    // 6.35x the tip.
    assert!(
        reason.starts_with("no published figure for a 0.50 mm tapered ball nose; ")
            && reason.contains("the nearest chart row is 3.175 mm")
            && !reason.contains("engaged"),
        "the reason must name the tip and the nearest row size: {reason:?}"
    );
}

#[test]
fn a_tip_under_half_a_millimetre_refuses_on_the_tip_floor_b1() {
    // DropCutter routes to Parallel / Finish, where the 0.5 mm SpeTool row
    // matches (ratio 0.6); Scallop matches no row. Both refuse on the floor.
    for op in [OperationType::DropCutter, OperationType::Scallop] {
        let err = suggest(op, &tool_of(ToolType::TaperedBallNose, 0.3), &hardwood())
            .expect_err("no wood chart prints a tapered tip under 0.5 mm");
        assert_eq!(
            size_reason(&err),
            "no published figure for a 0.30 mm tapered ball nose: no wood chart prints a tip \
             under 0.5 mm (ruling B1)",
            "{op:?}"
        );
    }
}

#[test]
fn a_one_millimetre_tapered_scallop_refuses_fm9() {
    let err = suggest(
        OperationType::Scallop,
        &tool_of(ToolType::TaperedBallNose, 1.0),
        &hardwood(),
    )
    .expect_err("a 1.0 mm tapered ball has no chart row within 2x");
    assert!(
        size_reason(&err).starts_with("no published figure for a 1.00 mm tapered ball nose"),
        "{err}"
    );
}

#[test]
fn a_one_millimetre_ball_scallop_in_hardwood_refuses_fm9() {
    let err = suggest(
        OperationType::Scallop,
        &tool_of(ToolType::BallNose, 1.0),
        &hardwood(),
    )
    .expect_err("the nearest hardwood ball row is far larger than 1 mm");
    assert!(
        size_reason(&err).starts_with("no published figure for a 1.00 mm ball nose"),
        "{err}"
    );
}

#[test]
fn a_standard_tapered_scallop_ships_fm9() {
    let s = suggest(
        OperationType::Scallop,
        &tool_of(ToolType::TaperedBallNose, 3.175),
        &hardwood(),
    )
    .expect("a 3.175 mm tapered ball is not a micro tool");
    assert_eq!(s.feeds_result.support, FeedsSupport::VendorBacked);
    assert!(s.operation.feed_rate() > 0.0);
}

/// Extrapolation P1 step 3: the 1.0 mm tool sits between two printed sizes
/// of the anchor's own Spektra series, so it ships through a G1 form A claim
/// (log-log interpolation), not through the generic 0.61 law.
///
/// The numbers, from `amana_flat_end.json` (the Spektra softwood pocket 2F
/// series of `amana_spektra_spiral_plunge_v24`): the 0.79375 mm row prints
/// 0.0254 mm/tooth, the 1.5 mm row 0.0508 (both min = max, so the mid is the
/// printed value). The anchor is the 0.79375 mm row (score 1758 against 1708
/// for the 1.5 mm row: the diameter term is 133 against 83). With
/// `t = ln(1.0 / 0.79375) / ln(1.5 / 0.79375) = 0.362928842984`, the mid at
/// 1.0 mm is `0.0254 * 2^t = 0.0326652649139`, so the scale on the anchor is
/// `2^t = 1.286034051728` (python3 over the JSON; the ratio 0.0508 / 0.0254
/// is exactly 2).
#[test]
fn a_micro_tool_with_a_near_row_ships_fm9() {
    use rs_cam_core::feeds::extrapolation::{ClaimResidual, SizeForm};
    let s = suggest(
        OperationType::Pocket,
        &tool_of(ToolType::EndMill, 1.0),
        &softwood(),
    )
    .expect("the printed 0.794 mm softwood Spektra row is within 2x of 1 mm");
    let FeedsSupport::Extrapolated { claim } = &s.feeds_result.support else {
        panic!(
            "a 1.0 mm tool between the 0.79375 and 1.5 mm printed rows ships through a \
             form A claim, got {:?}",
            s.feeds_result.support
        );
    };
    assert_eq!(
        claim.form,
        SizeForm::Interpolated {
            lo_mm: 0.79375,
            hi_mm: 1.5
        }
    );
    assert!((claim.scale - 1.286_034_051_728).abs() < 1e-9, "{claim:?}");
    assert_eq!(
        claim.source_rows,
        vec![
            "amana-flat-softwood-pocket-0794-2f-spektra".to_owned(),
            "amana-flat-softwood-pocket-1500-2f-spektra".to_owned(),
        ]
    );
    assert_eq!(
        claim.residual,
        ClaimResidual::Bracket {
            lo_value: 0.0254,
            hi_value: 0.0508
        }
    );
    let row = s
        .feeds_result
        .matched_lut_row
        .as_ref()
        .expect("a vendor row answered");
    assert_eq!(
        row.observation_id,
        "amana-flat-softwood-pocket-0794-2f-spektra"
    );
    // The row carries the claimed scale, the one number every consumer reads.
    assert!((row.chipload_diameter_scale - claim.scale).abs() < 1e-12);
    let ratio = row.row_diameter_mm / 1.0;
    assert!(
        (MICRO_ROW_RATIO_MIN..=MICRO_ROW_RATIO_MAX).contains(&ratio),
        "matched {} at {} mm",
        row.observation_id,
        row.row_diameter_mm
    );
}
