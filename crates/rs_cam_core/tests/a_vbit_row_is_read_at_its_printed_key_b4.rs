//! B4 (G5, V-bit key) — a V-bit row is read at its printed key: the
//! cutting diameter when the chart prints one, the included angle when it
//! prints none. Suggest and the gate read one row.
//!
//! Pre-registration: `planning/extrapolation_2026-09-24/B4_PLAN.md` and the
//! orchestrator's decisions at its top (step 1: data, key, basis and
//! resolver).
//!
//! The rules under test:
//!
//! - `feeds::geometry::lut_key_diameter_mm` and its cutter twin
//!   `lut_key_diameter_for_cutter` give the nominal diameter of a V-bit at
//!   every depth (before B4: the engaged width at the depth);
//! - the Amana AMS-159 and Spektra engraving rows are transcribed as
//!   printed: no cutting diameter, and the printed 18 000 RPM ("Operating
//!   RPM: 18,000"; "CNC Operating Spindle Speed: 18,000 RPM");
//! - a V-bit row with no diameter and a printed angle is
//!   `SizeBasis::AngleKey`: no claim, no size window, scale 1.0;
//! - the recipe resolver tries the chipload-bearing rows first for a
//!   V-bit, so Suggest's row is the gate's row (before B4 the Whiteside
//!   1540 / 1550 RPM anchors outscored the AMS-159 row);
//! - a V-bit row with a printed diameter takes the G1 size claim.
//!
//! How the numbers were derived (python3 over
//! `data/vendor_lut/observations/*.json`, with the scorer, the angle bonus
//! and the tie-break of `vendor_lookup.rs`):
//!
//! - 60 deg, 2 flutes, Trace finish, softwood or hardwood, at 6.35 mm or
//!   12.7 mm: the chip rows score 1825 for `amana-vgroove-{mat}-trace-60deg-2f`
//!   (1000 + 220 family + 120 exact + 60 grade a + 80 flutes + 45 role + 100
//!   material + 200 angle; no diameter, no Janka) and at most 1725 for any
//!   other chip row. The Whiteside RPM anchors no longer compete.
//! - The AMS-159 60 deg row prints 0.003 in (0.0762 mm) as one value, so it
//!   is a point. A pointed 60 deg cone at depth `ap` is `2 ap tan 30 deg`
//!   wide, so the gate's depth ratio is 0.866 and the de-rate is 1.0: the
//!   gate holds 0.0762.
//! - MDF, 60 deg, 2 flutes: the one row is `onsrud-mdf-37-80-1-trace`
//!   (25.4 mm, 0.1016-0.1524). At 6.35 mm the ratio is 0.25 (4.0x): refused.
//!   At 12.7 mm it is 0.5, the inclusive window edge: form C, scale
//!   0.5^0.61 = 0.655196701929, spread borrowed from the flat end mills.
//! - The MDF VCarve RPM at a 3 mm hint: the Onsrud row prints no RPM, so the
//!   engine rule runs at the engaged width, 0.1 + 2 x 3 x tan 30 deg =
//!   3.5641 mm: 170 m/min x 1000 / (pi x 3.5641) = 15 183 rpm. At the
//!   nominal 12.7 mm it would be 4 261 rpm, under the literature anti-pattern
//!   `rpm < 12000`.
//! - The parallel finish cells find no V-bit row (every V-bit row is Trace
//!   or Contour), so the R1 judgement refuses them with the `VBIT_PARALLEL`
//!   text.
//!
//! Step 2 of B4 (the band de-rate at the nominal diameter) adds its own
//! arm to this file.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::cutter::build_cutter;
use rs_cam_core::compute::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::extrapolation::{ClaimResidual, SizeBasis, SizeForm, SpreadFamily};
use rs_cam_core::feeds::geometry::{lut_key_diameter_for_cutter, lut_key_diameter_mm};
use rs_cam_core::feeds::suggest::{
    StockContext, SuggestContext, SuggestParamsInput, SuggestedParams, feeds_input_for_operation,
    suggest_params,
};
use rs_cam_core::feeds::vendor_lookup::{
    PrintedChipload, find_best_chip_envelope_row, vbit_key_text,
};
use rs_cam_core::feeds::vendor_lut::{LutOperationFamily, LutPassRole, ToolFamily};
use rs_cam_core::feeds::vendor_normalize::to_lookup_query;
use rs_cam_core::feeds::{
    EMBEDDED_LUT, FeedsError, FeedsInput, FeedsSupport, FeedsWarning, OperationFamily, PassRole,
    SetupContext, SpindleStrategy, ToolGeometryHint, calculate, embedded_vendor_lut, feeds_support,
};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::{MachineProfile, PowerModel};
use rs_cam_core::material::{Material, PlywoodGrade, SheetGoodKind, WoodSpecies};
use rs_cam_core::stock::simulation_cut::{
    CutKinematics, Engagement, SimulationCutSample, SimulationCutTrace,
};
use rs_cam_core::tool_load::verdict::{ChipBoundsSource, ChiploadVerdict};
use rs_cam_core::tool_load::{GateEnv, ToleranceBands, ToolpathLoadContext};

const TOL: f64 = 1e-12;

/// The printed AMS-159 60 deg chip load, 0.003 in.
const AMS_60_POINT_MM: f64 = 0.0762;

/// `0.5^0.61`: form C from the 1 in Onsrud row to a 1/2 in tool.
const SCALE_HALF: f64 = 0.655_196_701_929;

/// The G1 refusal of the 1 in Onsrud row for a 1/4 in V-bit.
const REFUSAL_1_4_IN: &str = "no published figure for a 6.35 mm V-bit; the nearest chart row is \
     25.4 mm, 4.0x the tool, outside the 0.5x to 2x window of the generic size law and past one \
     printed step of the row's chart series (extrapolation G1)";

/// The R1 judgement for a V-bit parallel finish (`support::VBIT_PARALLEL`).
const VBIT_PARALLEL: &str = "No published figure backs the formula for a V-bit on parallel \
     finish passes: at the engaged width the chip is below half of every V-bit figure.";

/// A 60 deg 2-flute pointed V-bit of `diameter` mm.
fn vbit(diameter: f64) -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId(1), ToolType::VBit);
    t.diameter = diameter;
    t.flute_count = 2;
    t.included_angle = 60.0;
    t.cutting_length = 19.05;
    t.shank_diameter = diameter;
    t.shaft_diameter = diameter;
    t.stickout = 27.05;
    t
}

/// The generic wood router with the feed and power ceilings lifted, so no
/// ceiling moves the RPM this file pins (ruling R4 Q10: the RPM follows a
/// binding feed ceiling down).
fn open_machine() -> MachineProfile {
    let mut m = MachineProfile::generic_wood_router();
    m.max_feed_mm_min = 100_000.0;
    m.max_cutting_feed_mm_min = Some(100_000.0);
    m.power = PowerModel::ConstantPower { power_kw: 100.0 };
    m
}

fn stock() -> StockContext {
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
    let machine = open_machine();
    suggest_params(SuggestParamsInput {
        op_type: op,
        tool,
        machine: &machine,
        material,
        lut: &EMBEDDED_LUT,
        stock_ctx: &stock(),
        spindle_strategy: SpindleStrategy::MatchChart,
        context: SuggestContext::default(),
    })
}

fn softwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    }
}

fn hardwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    }
}

fn mdf() -> Material {
    Material::SheetGood {
        kind: SheetGoodKind::Mdf,
    }
}

/// (a) The hint key equals the cutter key equals the nominal diameter at
/// every depth, from a shallow engraving to past the cone.
#[test]
fn the_vbit_key_is_the_nominal_diameter_at_every_depth_b4() {
    for d in [6.35, 12.7] {
        let tool = vbit(d);
        let cutter = build_cutter(&tool);
        let hint = ToolGeometryHint::VBit {
            included_angle: 60.0,
            tip_diameter: 0.0,
        };
        for ap in [0.1, 0.381, 5.0, 50.0] {
            let hint_key = lut_key_diameter_mm(hint, ap, d, d);
            let cutter_key = lut_key_diameter_for_cutter(&cutter, ap);
            assert_eq!(hint_key, d, "hint key at {d} mm, ap {ap}");
            assert_eq!(cutter_key, d, "cutter key at {d} mm, ap {ap}");
        }
    }
}

/// (b) The AMS-159 and Spektra engraving rows are transcribed as printed:
/// no cutting diameter, the printed 18 000 RPM, and a printed angle.
#[test]
fn the_ams159_and_spektra_rows_carry_the_printed_key_and_rpm_b4() {
    let rows: Vec<_> = EMBEDDED_LUT
        .observations
        .iter()
        .filter(|o| {
            o.tool_family == ToolFamily::ChamferVbit
                && (o.source_id == "amana_ams159_vgroove_v2"
                    || o.source_id == "amana_spektra_engraving_v4")
        })
        .collect();
    // 16 wood rows (amana_vgroove_engraving.json) and 3 acrylic / aluminium
    // rows (amana_vgroove_aluminum_acrylic.json).
    assert_eq!(rows.len(), 19);
    for o in rows {
        assert_eq!(o.diameter_mm, None, "{}", o.observation_id);
        assert_eq!(o.rpm_nominal, Some(18_000.0), "{}", o.observation_id);
        assert!(o.included_angle_deg.is_some(), "{}", o.observation_id);
    }
}

/// A steady cut on toolpath 0 at `doc` mm deep: 12 samples, 0.02 mm/tooth
/// at 18 000 rpm x 2 flutes (720 mm/min), under the printed point.
fn steady_trace(doc: f64) -> SimulationCutTrace {
    let feed = 720.0;
    let chip = feed / (18_000.0 * 2.0);
    let samples: Vec<SimulationCutSample> = (0..12)
        .map(|idx| SimulationCutSample {
            toolpath_id: ToolpathId(0),
            move_index: idx,
            sample_index: idx,
            position: [0.0, 0.0, -doc],
            cumulative_time_s: 0.1 * idx as f64,
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: CutKinematics::Linear,
            feed_rate_mm_min: feed,
            spindle_rpm: 18_000,
            flute_count: 2,
            axial_doc_mm: doc,
            axial_engagement_mm: doc,
            arc_engagement_radians: Some(std::f64::consts::FRAC_PI_2),
            chipload_mm_per_tooth: chip,
            effective_chip_thickness_mm: Some(chip),
            engagement: Engagement::with_radial_woc(0.5),
            removed_volume_est_mm3: 0.1,
            mrr_mm3_s: 1.0,
            in_transit_span: false,
            ..SimulationCutSample::test_fixture()
        })
        .collect();
    SimulationCutTrace {
        sample_step_mm: 1.0,
        samples,
        ..SimulationCutTrace::test_fixture()
    }
}

/// The gate's bounds for `tool` on a Trace-family cut sampled at `doc` mm.
fn gate_bounds(
    op: OperationType,
    tool: &ToolConfig,
    material: &Material,
    doc: f64,
) -> rs_cam_core::tool_load::verdict::ChipBounds {
    let trace = steady_trace(doc);
    let tool_def = build_cutter(tool);
    let machine = open_machine();
    let tolerance = ToleranceBands::default();
    let ctx = ToolpathLoadContext {
        toolpath_id: ToolpathId(0),
        tool: &tool_def,
        material,
        operation_family: LutOperationFamily::Trace,
        pass_role: LutPassRole::Finish,
        operation_feed_rate_mm_min: 720.0,
        operation_kind: op,
        spans: None,
        drill_op: None,
    };
    let env = GateEnv {
        sim_trace: Some(&trace),
        machine: Some(&machine),
        tolerance: &tolerance,
    };
    match rs_cam_core::tool_load::chipload::evaluate(&ctx, &env) {
        ChiploadVerdict::Within {
            approach_to_max, ..
        } => approach_to_max.bounds,
        ChiploadVerdict::Exceeds { triggering, .. } => triggering.bounds,
        other => panic!(
            "{op:?} {} mm: the gate must judge the cut: {other:?}",
            tool.diameter
        ),
    }
}

/// (c) 6.35 mm and 12.7 mm, softwood and hardwood, Trace and VCarve: the
/// recipe row, the envelope row and the gate's row are the AMS-159 60 deg
/// row, the basis is `AngleKey { 60 }`, and Suggest ships the printed
/// 18 000 RPM with no RPM-anchor warning.
#[test]
fn suggest_and_the_gate_read_the_ams159_row_b4() {
    for (label, material) in [("softwood", softwood()), ("hardwood", hardwood())] {
        let expected = format!("amana-vgroove-{label}-trace-60deg-2f");
        for d in [6.35, 12.7] {
            for op in [OperationType::Trace, OperationType::VCarve] {
                let cell = format!("{op:?} {d} mm in {label}");
                let tool = vbit(d);
                let s = suggest(op, &tool, &material).unwrap_or_else(|e| panic!("{cell}: {e}"));
                let recipe = s
                    .feeds_result
                    .matched_lut_row
                    .as_ref()
                    .unwrap_or_else(|| panic!("{cell}: no recipe row"));
                assert_eq!(recipe.observation_id, expected, "{cell}: recipe row");
                assert_eq!(
                    recipe.size_basis,
                    SizeBasis::AngleKey { angle_deg: 60.0 },
                    "{cell}"
                );
                assert_eq!(s.feeds_result.support, FeedsSupport::VendorBacked, "{cell}");
                assert_eq!(
                    recipe.printed_chipload(),
                    PrintedChipload::Point {
                        value_mm: AMS_60_POINT_MM
                    },
                    "{cell}: the printed point, unscaled"
                );
                assert_eq!(recipe.rpm_nominal, Some(18_000.0), "{cell}");
                assert!(
                    (s.feeds_result.rpm - 18_000.0).abs() < 1e-9,
                    "{cell}: Suggest RPM {}",
                    s.feeds_result.rpm
                );
                assert!(
                    !s.feeds_result
                        .warnings
                        .iter()
                        .any(|w| matches!(w, FeedsWarning::VendorRowPublishesNoChipload { .. })),
                    "{cell}: the recipe rests on a chip row, not on an RPM anchor"
                );
                assert_eq!(
                    vbit_key_text(ToolFamily::ChamferVbit, recipe).as_deref(),
                    Some(
                        "keyed at the printed angle 60 deg (the chart prints no cutting diameter)"
                    ),
                    "{cell}"
                );

                // The envelope resolver on the query Suggest routes.
                let operation = OperationConfig::new_default(op);
                let machine = open_machine();
                let input = feeds_input_for_operation(
                    &operation,
                    &tool,
                    &material,
                    &machine,
                    &EMBEDDED_LUT,
                    SpindleStrategy::MatchChart,
                );
                let query = to_lookup_query(&input).expect("the routing answers");
                assert_eq!(query.diameter_mm, d, "{cell}: the Suggest key");
                let env = find_best_chip_envelope_row(&EMBEDDED_LUT, &query, &input.tool_geometry)
                    .unwrap_or_else(|| panic!("{cell}: no envelope row"));
                assert_eq!(env.observation_id, expected, "{cell}: envelope row");

                // The gate itself, at a shallow and a deep sample: it holds the
                // AMS-159 point (a point preset, not a band from another row).
                for doc in [0.5, 3.0] {
                    let bounds = gate_bounds(op, &tool, &material, doc);
                    assert_eq!(
                        bounds.source,
                        ChipBoundsSource::VendorLutPointPreset,
                        "{cell}, gate at {doc} mm"
                    );
                    assert_eq!(bounds.min_mm_per_tooth, None, "{cell}, gate at {doc} mm");
                    assert!(
                        (bounds.max_mm_per_tooth - AMS_60_POINT_MM).abs() < TOL,
                        "{cell}, gate at {doc} mm: {}",
                        bounds.max_mm_per_tooth
                    );
                }
            }
        }
    }
}

/// (d) MDF: the 6.35 mm V-bit Trace refuses on the 25.4 mm Onsrud row
/// (4.0x), and the 12.7 mm one ships form C from it.
#[test]
fn the_onsrud_mdf_row_is_read_at_its_printed_diameter_b4() {
    match suggest(OperationType::Trace, &vbit(6.35), &mdf()) {
        Err(FeedsError::Unbacked { reason, .. }) => assert_eq!(reason, REFUSAL_1_4_IN),
        Err(other) => panic!("expected Unbacked, got {other:?}"),
        Ok(p) => panic!(
            "a 6.35 mm V-bit Trace in MDF must refuse, got {:?}",
            p.feeds_result.support
        ),
    }

    let s = suggest(OperationType::Trace, &vbit(12.7), &mdf())
        .expect("a 12.7 mm V-bit Trace in MDF ships form C");
    let FeedsSupport::Extrapolated { claim } = &s.feeds_result.support else {
        panic!("expected the G1 claim, got {:?}", s.feeds_result.support);
    };
    assert_eq!(claim.form, SizeForm::GenericFallback { exponent: 0.61 });
    assert!((claim.scale - SCALE_HALF).abs() < TOL, "{}", claim.scale);
    assert_eq!(claim.anchor_diameter_mm, 25.4);
    assert!(matches!(
        claim.residual,
        ClaimResidual::VendorSpread {
            family: SpreadFamily::BorrowedFromFlatEnd,
            ..
        }
    ));
    let row = s
        .feeds_result
        .matched_lut_row
        .as_ref()
        .expect("the recipe names its row");
    assert_eq!(row.observation_id, "onsrud-mdf-37-80-1-trace");
    assert_eq!(
        vbit_key_text(ToolFamily::ChamferVbit, row).as_deref(),
        Some("keyed at the printed cutting diameter 25.4 mm")
    );
    // Not a V-bit: no V-bit key line.
    assert_eq!(vbit_key_text(ToolFamily::FlatEnd, row), None);
}

/// (e) MDF VCarve at a 3 mm hint: the Onsrud row prints no RPM, so the
/// engine RPM runs at the engaged width (15 183 rpm), not at the nominal
/// diameter (4 261 rpm, clamped to the 8 000 rpm spindle floor). The key
/// moved in B4; the surface-speed RPM did not.
#[test]
fn the_mdf_vcarve_rpm_stays_at_the_engaged_width_b4() {
    let machine = open_machine();
    let material = mdf();
    let result = calculate(&FeedsInput {
        tool_diameter: 12.7,
        flute_count: 2,
        flute_length: 19.05,
        shank_diameter: Some(12.7),
        tool_geometry: ToolGeometryHint::VBit {
            included_angle: 60.0,
            tip_diameter: 0.1,
        },
        material: &material,
        machine: &machine,
        operation: OperationFamily::Trace,
        operation_kind: Some(OperationType::VCarve),
        pass_role: PassRole::Finish,
        axial_depth_mm: Some(3.0),
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: Some(embedded_vendor_lut()),
        setup: SetupContext::default(),
        spindle_strategy: SpindleStrategy::MatchChart,
    });
    let row = result
        .matched_lut_row
        .as_ref()
        .expect("the Onsrud row answers");
    assert_eq!(row.observation_id, "onsrud-mdf-37-80-1-trace");
    assert_eq!(row.rpm_nominal, None, "the Onsrud sheet prints no RPM");
    eprintln!(
        "a_vbit_row_is_read_at_its_printed_key_b4: MDF VCarve 12.7 mm at 3 mm, rpm {}",
        result.rpm
    );
    assert!(
        result.rpm >= 12_000.0,
        "the engine RPM must stay at the engaged width (about 15 183 rpm); got {}",
        result.rpm
    );
}

/// (f) The parallel finish cells still refuse with the `VBIT_PARALLEL`
/// judgement: no V-bit row is filed under Parallel.
#[test]
fn the_vbit_parallel_finish_cells_refuse_b4() {
    let machine = open_machine();
    let materials = [
        softwood(),
        hardwood(),
        mdf(),
        Material::Plywood {
            grade: PlywoodGrade::BalticBirch,
        },
    ];
    let mut checked = 0;
    for kind in [
        OperationType::DropCutter,
        OperationType::RampFinish,
        OperationType::RadialFinish,
        OperationType::HorizontalFinish,
    ] {
        for material in &materials {
            for d in [6.35, 12.7] {
                let support = feeds_support(&FeedsInput {
                    tool_diameter: d,
                    flute_count: 2,
                    flute_length: 19.05,
                    shank_diameter: Some(d),
                    tool_geometry: ToolGeometryHint::VBit {
                        included_angle: 60.0,
                        tip_diameter: 0.0,
                    },
                    material,
                    machine: &machine,
                    operation: OperationFamily::Parallel,
                    operation_kind: Some(kind),
                    pass_role: PassRole::Finish,
                    axial_depth_mm: Some(1.0),
                    radial_width_mm: None,
                    target_scallop_mm: None,
                    vendor_lut: Some(embedded_vendor_lut()),
                    setup: SetupContext::default(),
                    spindle_strategy: SpindleStrategy::MatchChart,
                });
                match support {
                    FeedsSupport::Refuse { reason } => {
                        assert_eq!(reason, VBIT_PARALLEL, "{kind:?} {d} mm in {material:?}");
                    }
                    other => {
                        panic!("{kind:?} {d} mm in {material:?}: expected Refuse, got {other:?}")
                    }
                }
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 32);
}
