//! G1 — every consumer reads one claimed band (extrapolation P1 step 3,
//! `P1_PLAN.md` §2.3 "Where the claim plugs in", orchestrator decision 6).
//!
//! The G1 size claim runs inside `vendor_lookup::build_result`, the one place
//! where a matched row becomes a `LookupResult`. Both resolvers pass through
//! it: the recipe resolver (Suggest, explain) and the envelope resolver (the
//! gate, the optimizer, the viewport, the modulator and the advisor, through
//! `tool_load::chipload::matched_chip_envelope`). So a claimed cell has one
//! band, and a refused cell has none anywhere.
//!
//! The arms:
//!
//! - form A, B and C cells: Suggest's pre-derate band
//!   (`FeedsResult::matched_lut_row`), the envelope resolver's band on the
//!   same query, and `tool_load::chipload_envelope_for_toolpath` (the
//!   modulator's and the advisor's door, with no simulation, so no depth
//!   de-rate) are one band, and the scale on the row is the claim's scale;
//! - a refused cell (a 1.0 mm tapered Scallop in hardwood): Suggest refuses,
//!   the envelope row publishes no band, the modulator's door gives `None`,
//!   and a raw `calculate` ships the formula with no vendor RPM, no band and
//!   no `VendorRowPublishesNoChipload`;
//! - decision 6: the gate's verdict on a refused cell is
//!   `Unmodeled(NoVendorData)`, and its diagnostic reports under
//!   `load.chipload.unmodeled`, never under a `*.within` id. Before P1 an
//!   unmodelled chipload verdict reported under `load.chipload.within`
//!   (`from_tool_load::chipload_to_diagnostic`).
//!
//! How the numbers were derived (python3 over the JSON files in
//! `data/vendor_lut/observations/`, with the formulas of
//! `extrapolation/size.rs`):
//!
//! - form A, a 1.0 mm flat Pocket in softwood: anchor
//!   `amana-flat-softwood-pocket-0794-2f-spektra` (0.0254, min = max), claim
//!   scale `2^t = 1.286034051728` (see `a_size_claim_states_...`), hardness
//!   scale 1.0 (the row has no Janka, so it reads the query table:
//!   `WoodSpecies::GenericSoftwood`, 600 on a 600 query), band
//!   `0.0254 * 1.286034051728 = 0.032665264914` at both ends. Since A2 (point
//!   mode) the row is a point: min `None`, max 0.032665264914,
//!   `printed_chipload() == Point`, and the gate bounds are `{ min: None,
//!   max: v' }`. RE-PINNED
//!   2026-09-24 (extrapolation P2 step 3, one Janka table): was
//!   `0.0254 * 1.286034 * (500 / 600)^0.5 = 0.029819170734`, when the row
//!   default was a separate 500 lbf anchor;
//! - form B, a 2.5 mm flat Pocket in hardwood: anchor
//!   `amana-flat-hardwood-pocket-3175-2f-spektra` (max 0.1016 only, Janka
//!   1450); series 3.175 / 6.0 / 6.35 mm (mids 0.1016 / 0.127 / 0.127), slope
//!   0.333834426894, one step down to `3.175² / 6.0 = 1.680104166667`; scale
//!   `(2.5 / 3.175)^0.333834426894 = 0.923308309938`, max 0.093808124290;
//! - form C, a 3.175 mm ball Scallop in hardwood: anchor
//!   `amana-ball-hardwood-scallop-6000-2f` (0.025-0.04, Janka 1450), scale
//!   `(3.175 / 6.0)^0.61 = 0.678252514240`, band 0.016956312856 -
//!   0.027130100570.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::compute::{ToolConfig, ToolId, ToolType};
use rs_cam_core::diagnostics::adapters::from_tool_load::diagnostics_from_load_verdict;
use rs_cam_core::diagnostics::ids;
use rs_cam_core::feeds::extrapolation::{SizeBasis, SizeForm};
use rs_cam_core::feeds::geometry::doc_derating_scale;
use rs_cam_core::feeds::provenance::ValueProvenance;
use rs_cam_core::feeds::suggest::{
    StockContext, SuggestContext, SuggestParamsInput, SuggestedParams, feeds_input_for_operation,
    suggest_params,
};
use rs_cam_core::feeds::vendor_lookup::{
    LookupResult, PrintedChipload, find_best_chip_envelope_row,
};
use rs_cam_core::feeds::vendor_lut::{LutOperationFamily, LutPassRole};
use rs_cam_core::feeds::vendor_normalize::to_lookup_query;
use rs_cam_core::feeds::{
    ChiploadSource, EMBEDDED_LUT, FeedsError, FeedsSupport, FeedsWarning, SpindleStrategy,
    calculate,
};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::stock::simulation_cut::{
    CutKinematics, Engagement, SimulationCutSample, SimulationCutTrace,
};
use rs_cam_core::tool::{FlatEndmill, ToolDefinition};
use rs_cam_core::tool_load::chipload_envelope_for_toolpath;
use rs_cam_core::tool_load::verdict::{
    ChipBoundsSource, ChiploadVerdict, DeflectionVerdict, DepthVerdict, PowerVerdict,
    ToolpathLoadVerdict, UnmodeledReason,
};
use rs_cam_core::tool_load::{GateEnv, ToleranceBands, ToolpathLoadContext};

const TOL: f64 = 1e-9;

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

fn suggest(
    op: OperationType,
    tool: &ToolConfig,
    material: &Material,
) -> Result<SuggestedParams, FeedsError> {
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
}

/// The envelope resolver's row on the query that Suggest routes for this
/// cell (the gate's resolver, through the public entry point).
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

fn close(a: Option<f64>, b: Option<f64>) -> bool {
    match (a, b) {
        (Some(x), Some(y)) => (x - y).abs() < TOL,
        (None, None) => true,
        _ => false,
    }
}

/// One claimed cell: Suggest, the envelope resolver and the modulator's
/// door read one band, and the row carries the claim's scale.
fn assert_one_claimed_band(
    op: OperationType,
    tool: &ToolConfig,
    material: &Material,
    anchor: &str,
    form: &str,
    scale: f64,
    band: (Option<f64>, f64),
) {
    let label = format!("{op:?} {:?} {} mm", tool.tool_type, tool.diameter);
    let s = suggest(op, tool, material).unwrap_or_else(|e| panic!("{label}: {e}"));
    let FeedsSupport::Extrapolated { claim } = &s.feeds_result.support else {
        panic!(
            "{label}: expected a G1 claim, got {:?}",
            s.feeds_result.support
        );
    };
    assert_eq!(claim.form.name(), form, "{label}: {claim:?}");
    assert!((claim.scale - scale).abs() < TOL, "{label}: {claim:?}");

    let recipe = s
        .feeds_result
        .matched_lut_row
        .as_ref()
        .expect("a vendor row answered");
    assert_eq!(recipe.observation_id, anchor, "{label}");
    assert_eq!(recipe.size_basis.claim(), Some(claim.as_ref()), "{label}");
    assert!(
        (recipe.chipload_diameter_scale - claim.scale).abs() < 1e-15,
        "{label}"
    );
    assert!(
        close(recipe.chip_load_min_mm, band.0),
        "{label}: {recipe:?}"
    );
    assert!(
        close(recipe.chip_load_max_mm, Some(band.1)),
        "{label}: {recipe:?}"
    );

    // The gate's resolver: the same row, the same basis, the same band.
    let env = envelope_row(op, tool, material);
    assert_eq!(env.observation_id, recipe.observation_id, "{label}");
    assert_eq!(env.size_basis, recipe.size_basis, "{label}");
    assert!(
        close(env.chip_load_min_mm, recipe.chip_load_min_mm),
        "{label}"
    );
    assert!(
        close(env.chip_load_max_mm, recipe.chip_load_max_mm),
        "{label}"
    );

    // The modulator's and the advisor's door. With no simulation the depth
    // is 0, so the de-rate is 1.0 and the band is the row's band. It needs
    // both bounds (a `Range`); a max-only row gives `None` here, as it
    // gives Suggest no `chipload_bounds`.
    let door = chipload_envelope_for_toolpath(
        material,
        tool,
        &OperationConfig::new_default(op),
        ToolpathId(0),
        None,
    );
    match band.0 {
        Some(min) => {
            let range = door.unwrap_or_else(|| panic!("{label}: the door gave no band"));
            assert!((range.start - min).abs() < TOL, "{label}: {range:?}");
            assert!((range.end - band.1).abs() < TOL, "{label}: {range:?}");
        }
        None => {
            // A2 (point mode): a row that prints one value is a claimed
            // POINT. No consumer makes a band from it; every consumer reads
            // the one claimed value.
            assert!(door.is_none(), "{label}: {door:?}");
            assert!(s.feeds_result.chipload_bounds.is_none(), "{label}");
            for (who, row) in [("recipe", recipe), ("envelope", &env)] {
                match row.printed_chipload() {
                    PrintedChipload::Point { value_mm } => assert!(
                        (value_mm - band.1).abs() < TOL,
                        "{label} {who}: point {value_mm} vs claimed {}",
                        band.1
                    ),
                    other => panic!("{label} {who}: expected a point, got {other:?}"),
                }
            }
            // Suggest's point takes the depth de-rate of the band at the
            // calculator's depth (flat end: the ratio divides by D).
            let ap = s.feeds_result.axial_depth_mm;
            let expected = band.1 * doc_derating_scale(ap / tool.diameter);
            let point = s
                .feeds_result
                .chipload_point_mm
                .unwrap_or_else(|| panic!("{label}: Suggest carries no point"));
            assert!(
                (point - expected).abs() < TOL,
                "{label}: Suggest point {point} vs {expected} (ap {ap})"
            );
        }
    }
}

/// Form A on a point row (A2): the anchor
/// `amana-flat-softwood-pocket-0794-2f-spektra` prints one value (0.0254,
/// min = max), so the claim is a claimed POINT, 0.0254 x 1.286034051728 =
/// 0.032665264914, and no consumer holds a minimum.
#[test]
fn a_form_a_cell_has_one_point_g1() {
    assert_one_claimed_band(
        OperationType::Pocket,
        &tool_of(ToolType::EndMill, 1.0),
        &softwood(),
        "amana-flat-softwood-pocket-0794-2f-spektra",
        "A",
        1.286_034_051_728,
        (None, FORM_A_POINT_MM_TOOTH),
    );
}

/// The form A claimed point (see [`a_form_a_cell_has_one_point_g1`]).
const FORM_A_POINT_MM_TOOTH: f64 = 0.032_665_264_914;

/// The gate on the form A cell (A2): the bounds are `{ min: None, max: v' }`
/// with the source `VendorLutPointPreset`. The samples cut at 0.5 mm on a
/// 1.0 mm tool (DOC/D = 0.5), so the de-rate is 1.0 and v' is the claimed
/// point. The advance 0.02 mm/tooth (720 mm/min at 18 000 rpm x 2 flutes)
/// is below the point, so the verdict is `Within` with a burn advisory
/// against the printed point, never `Exceeds(Low)` (decision Q1 (a)).
#[test]
fn the_gate_holds_the_form_a_point_with_no_minimum_g1() {
    const RPM: u32 = 18_000;
    const FEED: f64 = 720.0;
    let tool = ToolDefinition::new(
        Box::new(FlatEndmill::new(1.0, 6.0)),
        1.0,
        12.0,
        6.0,
        12.0,
        2,
        ToolMaterial::Carbide,
    );
    let material = softwood();
    let samples: Vec<SimulationCutSample> = (0..12)
        .map(|idx| SimulationCutSample {
            toolpath_id: ToolpathId(0),
            move_index: idx,
            sample_index: idx,
            position: [0.0, 0.0, -0.5],
            cumulative_time_s: 0.1 * idx as f64,
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: CutKinematics::Linear,
            feed_rate_mm_min: FEED,
            spindle_rpm: RPM,
            flute_count: 2,
            axial_doc_mm: 0.5,
            axial_engagement_mm: 0.5,
            arc_engagement_radians: Some(std::f64::consts::FRAC_PI_2),
            chipload_mm_per_tooth: FEED / (f64::from(RPM) * 2.0),
            effective_chip_thickness_mm: Some(FEED / (f64::from(RPM) * 2.0)),
            engagement: Engagement::with_radial_woc(0.5),
            removed_volume_est_mm3: 0.1,
            mrr_mm3_s: 1.0,
            in_transit_span: false,
            ..SimulationCutSample::test_fixture()
        })
        .collect();
    let trace = SimulationCutTrace {
        sample_step_mm: 1.0,
        samples,
        ..SimulationCutTrace::test_fixture()
    };
    let machine = MachineProfile::default();
    let tolerance = ToleranceBands::default();
    let ctx = ToolpathLoadContext {
        toolpath_id: ToolpathId(0),
        tool: &tool,
        material: &material,
        operation_family: LutOperationFamily::Pocket,
        pass_role: LutPassRole::Roughing,
        operation_feed_rate_mm_min: FEED,
        operation_kind: OperationType::Pocket,
        spans: None,
        drill_op: None,
    };
    let env = GateEnv {
        sim_trace: Some(&trace),
        machine: Some(&machine),
        tolerance: &tolerance,
    };
    let verdict = rs_cam_core::tool_load::chipload::evaluate(&ctx, &env);
    let ChiploadVerdict::Within {
        approach_to_min,
        approach_to_max,
        burn_advisory,
        ..
    } = &verdict
    else {
        panic!("a chip below the point is advisory, never Exceeds(Low): {verdict:?}");
    };
    let bounds = &approach_to_max.bounds;
    assert_eq!(bounds.source, ChipBoundsSource::VendorLutPointPreset);
    assert_eq!(bounds.min_mm_per_tooth, None, "a point is not a band");
    assert!(
        (bounds.max_mm_per_tooth - FORM_A_POINT_MM_TOOTH).abs() < TOL,
        "gate max {} vs the claimed point {FORM_A_POINT_MM_TOOTH}",
        bounds.max_mm_per_tooth
    );
    assert!(approach_to_min.is_none(), "{approach_to_min:?}");
    let advisory = burn_advisory
        .as_deref()
        .expect("a chip below the point gives a burn advisory");
    assert_eq!(
        advisory.bounds.burn_reference_mm(),
        Some(bounds.max_mm_per_tooth)
    );
    assert_eq!(advisory.bounds.burn_reference_label(), "printed point");
}

#[test]
fn a_form_b_cell_has_one_band_g1() {
    assert_one_claimed_band(
        OperationType::Pocket,
        &tool_of(ToolType::EndMill, 2.5),
        &hardwood(),
        "amana-flat-hardwood-pocket-3175-2f-spektra",
        "B",
        0.923_308_309_938,
        (None, 0.093_808_124_290),
    );
}

#[test]
fn a_form_c_cell_has_one_band_g1() {
    assert_one_claimed_band(
        OperationType::Scallop,
        &tool_of(ToolType::BallNose, 3.175),
        &hardwood(),
        "amana-ball-hardwood-scallop-6000-2f",
        "C",
        0.678_252_514_240,
        (Some(0.016_956_312_856), 0.027_130_100_570),
    );
}

/// A refused cell has no band anywhere.
#[test]
fn a_refused_cell_has_no_band_anywhere_g1() {
    let tool = tool_of(ToolType::TaperedBallNose, 1.0);
    let material = hardwood();
    let op = OperationType::Scallop;

    // Suggest refuses with the size rule's text.
    let err = suggest(op, &tool, &material).expect_err("no chart brackets a 1.0 mm tapered tip");
    let FeedsError::Unbacked { reason, .. } = &err else {
        panic!("expected Unbacked, got {err:?}");
    };
    assert!(
        reason.starts_with(
            "no published figure for a 1.00 mm tapered ball nose; the nearest chart row is 6.35 mm"
        ),
        "{reason}"
    );

    // The envelope resolver still matches the row, and the row carries the
    // refusal and publishes no band.
    let env = envelope_row(op, &tool, &material);
    assert_eq!(env.observation_id, "onsrud-hardwood-77-100-1_4-scallop");
    assert!(env.size_basis.is_refused(), "{:?}", env.size_basis);
    assert_eq!(env.chip_load_mm, 0.0);
    assert_eq!(env.chip_load_min_mm, None);
    assert_eq!(env.chip_load_max_mm, None);

    // The modulator's and the advisor's door (and so the gate and the
    // viewport, which share `matched_chip_envelope`) gives no band.
    assert!(
        chipload_envelope_for_toolpath(
            &material,
            &tool,
            &OperationConfig::new_default(op),
            ToolpathId(0),
            None,
        )
        .is_none()
    );

    // A raw `calculate` (no validation) ships the formula: no vendor RPM, no
    // band, and not the RPM-only warning. The row stays visible with its
    // refusal, and it labels no value `VendorLut`.
    let machine = MachineProfile::default();
    let operation = OperationConfig::new_default(op);
    let input = feeds_input_for_operation(
        &operation,
        &tool,
        &material,
        &machine,
        &EMBEDDED_LUT,
        SpindleStrategy::MatchChart,
    );
    let r = calculate(&input);
    assert!(
        matches!(&r.support, FeedsSupport::Refuse { reason } if reason.starts_with("no published figure for a 1.00 mm tapered ball nose")),
        "{:?}",
        r.support
    );
    assert_eq!(r.chipload_source, ChiploadSource::FormulaFallback);
    assert_eq!(r.vendor_source, None);
    assert!(r.chipload_bounds.is_none());
    assert!(
        !r.warnings
            .iter()
            .any(|w| matches!(w, FeedsWarning::VendorRowPublishesNoChipload { .. })),
        "{:?}",
        r.warnings
    );
    let row = r
        .matched_lut_row
        .as_ref()
        .expect("the refused row stays visible");
    assert!(matches!(row.size_basis, SizeBasis::Refused { .. }));
    assert_eq!(r.provenance().spindle_rpm, Some(ValueProvenance::formula()));
}

/// Decision 6: the gate's verdict on a refused cell is
/// `Unmodeled(NoVendorData)` (`matched_chip_envelope` gives `None`), and the
/// diagnostic reports under `load.chipload.unmodeled`, not under a
/// `*.within` id.
#[test]
fn a_refused_cell_is_not_reported_within_g1() {
    let verdict = ToolpathLoadVerdict {
        toolpath_id: ToolpathId(3),
        chipload: ChiploadVerdict::Unmodeled {
            reason: UnmodeledReason::NoVendorData,
        },
        power: PowerVerdict::Unmodeled {
            reason: UnmodeledReason::SimulationRequired,
        },
        deflection: DeflectionVerdict::Unmodeled {
            reason: UnmodeledReason::SimulationRequired,
        },
        depth: DepthVerdict::Unmodeled {
            reason: UnmodeledReason::SimulationRequired,
        },
        drill_gates: None,
        modulation_summary: None,
        feed_explanation: None,
        kinematic_utilization: None,
    };
    let rows = diagnostics_from_load_verdict(&verdict);
    let chip: Vec<&str> = rows
        .iter()
        .map(|d| d.id.as_str())
        .filter(|id| id.starts_with("load.chipload."))
        .collect();
    // Non-vacuity: the chipload gate published one row.
    assert_eq!(chip, vec![ids::LOAD_CHIPLOAD_UNMODELED]);
    assert!(!ids::LOAD_CHIPLOAD_UNMODELED.ends_with(".within"));
    assert!(
        !rows
            .iter()
            .any(|d| d.id.as_str() == ids::LOAD_CHIPLOAD_WITHIN),
        "an abstention must not report under the within id"
    );
    assert!(ids::ALL.contains(&ids::LOAD_CHIPLOAD_UNMODELED));
}

/// The form names the FM1 columns print.
#[test]
fn the_form_names_are_a_b_c_g1() {
    assert_eq!(
        SizeForm::Interpolated {
            lo_mm: 1.0,
            hi_mm: 2.0
        }
        .name(),
        "A"
    );
    assert_eq!(
        SizeForm::SeriesSlope {
            slope: 0.4,
            r2: 1.0,
            sizes: 3
        }
        .name(),
        "B"
    );
    assert_eq!(SizeForm::GenericFallback { exponent: 0.61 }.name(), "C");
}
