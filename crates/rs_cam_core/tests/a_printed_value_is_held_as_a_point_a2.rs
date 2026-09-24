//! A2 (point mode, G4) — a printed value is held as a point.
//!
//! A vendor row that prints one value (a maximum only, or two equal limits)
//! gives a POINT, not a band. No consumer derives a band from it. The rule
//! and its consumers:
//!
//! - `vendor_lookup::build_result` stores a point as "maximum present, no
//!   minimum", and `LookupResult::printed_chipload()` classifies it as
//!   `PrintedChipload::Point`;
//! - Suggest carries it as `FeedsResult::chipload_point_mm`, and
//!   `chipload_bounds` stays two-limit only (`None` for a point);
//! - the gate holds `{ min: None, max: v' }` with the source
//!   `VendorLutPointPreset`: hard above the point, a burn ADVISORY below it,
//!   never `Exceeds(Low)` (decision Q1 (a));
//! - the envelope resolver gives `ChipTarget::Point`; the band-only envelope
//!   (`chipload_envelope_for_toolpath`, the viewport) gives `None` (Q3); the
//!   modulator reads `ChipTarget::modulation_band()`, which is a point with no
//!   floor.
//!
//! Pre-registration: `planning/extrapolation_2026-09-24/A2_PLAN.md` §5.
//!
//! How the numbers were derived (python3 over
//! `data/vendor_lut/observations/*.json`, the 26 files the embedded LUT
//! includes):
//!
//! - 496 rows; 344 print two limits with `min < max`; 148 print one value
//!   (100 a maximum only, 48 two equal limits, 0 a minimum only); 4 print no
//!   chipload. Every one-value row carries a `diameter_mm` except the three
//!   AMS-159 60 and 90 deg rows: since ruling B4 (2026-09-25) they carry
//!   none, because the chart prints none, and they are keyed at their angle
//!   (`SizeBasis::AngleKey`). Test (a) queries them at 6.35 mm; an
//!   angle-keyed row is not scaled at any key.
//! - The AMS-159 60 deg hardwood Trace cell (a 12.7 mm 2-flute V-bit):
//!   `amana-vgroove-hardwood-trace-60deg-2f`, min = max = 0.0762, no Janka
//!   on the row (the hardness scale reads the query table: 1.0), no
//!   diameter (`AngleKey`, scale 1.0). The V-bit depth-to-diameter ratio is
//!   `ap / (2 ap tan 30 deg) = 0.866 <= 1`, so the depth de-rate is 1.0 at
//!   every depth and v' = 0.0762. The FM1 matrix (`matrix_2026-09-23.csv`)
//!   reads `chipload_point_mm = 0.0762` here. Since B4 the gate keys the
//!   V-bit at its nominal 12.7 mm and the envelope resolver gives the same
//!   AMS-159 row (score 1825; the next chip row is the softwood 60 deg row
//!   at 1725), so the gate holds the point too.
//! - The Spektra 6 mm hardwood Pocket cell (a 6.0 mm 2-flute end mill):
//!   `amana-flat-hardwood-pocket-6000-2f-spektra`, max 0.127 only, Janka
//!   1450 on a 1450 query (`WoodSpecies::GenericHardwood`), diameter ratio
//!   1.0, so v = 0.127. The gate samples cut at 2.0 mm (ratio 2 / 6 <= 1), so
//!   v' = 0.127.

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
use rs_cam_core::feeds::ToolGeometryHint;
use rs_cam_core::feeds::geometry::doc_derating_scale;
use rs_cam_core::feeds::suggest::{
    StockContext, SuggestContext, SuggestParamsInput, SuggestedParams, suggest_params,
};
use rs_cam_core::feeds::vendor_lookup::{
    LookupQuery, LookupResult, PrintedChipload, find_best_chip_envelope_row,
};
use rs_cam_core::feeds::vendor_lut::{LutOperationFamily, LutPassRole, VendorObservation};
use rs_cam_core::feeds::{EMBEDDED_LUT, SpindleStrategy, embedded_vendor_lut};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::stock::simulation_cut::{
    CutKinematics, Engagement, SimulationCutSample, SimulationCutTrace,
};
use rs_cam_core::tool_load::verdict::{ChipBoundsSource, ChiploadVerdict};
use rs_cam_core::tool_load::{
    ChipTarget, GateEnv, ToleranceBands, ToolpathLoadContext, chip_target_for_toolpath,
    chipload_envelope_for_toolpath,
};

const TOL: f64 = 1e-9;

/// The one-value rows in the 26 embedded observation files (see the module
/// doc for the derivation).
const ONE_VALUE_ROWS: usize = 148;

/// `true` when the raw row prints one value: a maximum only, a minimum
/// only, or two equal limits.
fn prints_one_value(obs: &VendorObservation) -> bool {
    match (obs.chipload_min_mm_tooth, obs.chipload_max_mm_tooth) {
        (Some(lo), Some(hi)) => lo == hi,
        (None, Some(_)) | (Some(_), None) => true,
        (None, None) => false,
    }
}

/// `true` when the raw row prints two limits with `min < max`.
fn prints_a_band(obs: &VendorObservation) -> bool {
    matches!(
        (obs.chipload_min_mm_tooth, obs.chipload_max_mm_tooth),
        (Some(lo), Some(hi)) if lo < hi
    )
}

/// The resolved row's shape must follow its raw row: one value gives a
/// point with no minimum, two limits give a band.
fn assert_shape_follows_the_raw_row(row: &LookupResult) {
    let lut = embedded_vendor_lut();
    let raw = lut
        .observations
        .iter()
        .find(|obs| obs.observation_id == row.observation_id)
        .expect("a resolved row is in the embedded LUT");
    let printed = row.printed_chipload();
    if prints_one_value(raw) {
        let PrintedChipload::Point { value_mm } = printed else {
            panic!(
                "{}: the raw row prints one value, so it must resolve as a point; got \
                 {printed:?}",
                row.observation_id
            );
        };
        assert_eq!(
            row.chip_load_min_mm, None,
            "{}: a point carries no minimum",
            row.observation_id
        );
        assert_eq!(
            row.chip_load_max_mm,
            Some(value_mm),
            "{}: the point is the stored maximum",
            row.observation_id
        );
    } else if prints_a_band(raw) {
        assert!(
            matches!(printed, PrintedChipload::Band { .. }),
            "{}: the raw row prints two limits, so it must resolve as a band; got \
             {printed:?}",
            row.observation_id
        );
    }
}

/// (a) Every LUT row that prints one value resolves as a point with no
/// minimum. Each one-value row is queried at its own tool, size, flute
/// count, material, hardness, operation and pass role. The row that wins
/// must follow its own raw shape. A one-value row that loses its own query
/// to another row is named on stderr; the winner is still checked.
#[test]
fn every_one_value_row_resolves_as_a_point_a2() {
    let lut = embedded_vendor_lut();
    let one_value: Vec<&VendorObservation> = lut
        .observations
        .iter()
        .filter(|obs| prints_one_value(obs))
        .collect();
    assert_eq!(
        one_value.len(),
        ONE_VALUE_ROWS,
        "the embedded LUT must carry {ONE_VALUE_ROWS} one-value rows (python3 over \
         observations/*.json); re-derive the count if the LUT moved"
    );

    let mut self_resolved = 0usize;
    let mut lost: Vec<(String, String)> = Vec::new();
    for obs in &one_value {
        // Ruling B4: an angle-keyed V-bit row prints no diameter and is not
        // scaled at any key, so it is queried at a 1/4 in tool.
        let diameter = obs.diameter_mm.unwrap_or_else(|| {
            assert!(
                obs.included_angle_deg.is_some(),
                "{}: a one-value row with no diameter and no angle",
                obs.observation_id
            );
            6.35
        });
        let query = LookupQuery {
            tool_family: obs.tool_family,
            tool_subfamily: obs.tool_subfamily.clone(),
            diameter_mm: diameter,
            flute_count: obs.flute_count,
            material_family: obs.material_family,
            hardness_kind: obs.hardness_kind,
            hardness_value: obs.hardness_value,
            operation_family: obs.operation_family,
            pass_role: obs.pass_role,
        };
        let hint = match obs.included_angle_deg {
            Some(angle) => ToolGeometryHint::VBit {
                included_angle: angle,
                tip_diameter: obs.tip_diameter_mm.unwrap_or(0.0),
            },
            None => ToolGeometryHint::Flat,
        };
        let row = find_best_chip_envelope_row(lut, &query, &hint).unwrap_or_else(|| {
            panic!(
                "{}: the row's own query resolves no chipload row",
                obs.observation_id
            )
        });
        assert_shape_follows_the_raw_row(&row);
        if row.observation_id == obs.observation_id {
            self_resolved += 1;
        } else {
            lost.push((obs.observation_id.clone(), row.observation_id.clone()));
        }
    }
    eprintln!(
        "A2 (a): {self_resolved} of {ONE_VALUE_ROWS} one-value rows win their own query; \
         {} lose it to another row: {lost:?}",
        lost.len()
    );
    assert!(
        self_resolved > 0,
        "no one-value row won its own query, so the point check is vacuous"
    );
}

/// One point cell, built from the FM1 tool builder.
struct PointCell {
    label: &'static str,
    op: OperationType,
    tool: ToolConfig,
    expected_row: &'static str,
    /// The printed point on the resolved row (scaled; see the module doc).
    point_mm: f64,
    operation_family: LutOperationFamily,
    pass_role: LutPassRole,
    /// The axial depth the gate samples cut at.
    sample_doc_mm: f64,
    /// True when the gate resolves the same point row as Suggest. Both
    /// cells are true since ruling B4: the gate keys a V-bit at its nominal
    /// diameter, as Suggest does, and the recipe resolver tries the chip
    /// rows first for a V-bit.
    gate_reads_the_point: bool,
}

fn hardwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    }
}

/// The tool builder of `feeds_matrix_instrument_fm1.rs`, for the two tool
/// types this file uses.
fn tool_of(kind: ToolType, diameter: f64) -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId(1), kind);
    t.diameter = diameter;
    t.flute_count = 2;
    t.cutting_length = (diameter * 3.0).max(12.0);
    t.shank_diameter = diameter.max(3.0);
    t.shaft_diameter = diameter.max(3.0);
    t.stickout = t.cutting_length + 8.0;
    if matches!(kind, ToolType::VBit) {
        t.included_angle = 60.0;
    }
    t
}

fn ams159_trace() -> PointCell {
    PointCell {
        label: "AMS-159 60 deg V-bit 12.7 mm / hardwood / Trace",
        op: OperationType::Trace,
        tool: tool_of(ToolType::VBit, 12.7),
        expected_row: "amana-vgroove-hardwood-trace-60deg-2f",
        point_mm: 0.0762,
        operation_family: LutOperationFamily::Trace,
        pass_role: LutPassRole::Finish,
        sample_doc_mm: 0.762,
        gate_reads_the_point: true,
    }
}

fn spektra_pocket() -> PointCell {
    PointCell {
        label: "Spektra 6 mm 2F end mill / hardwood / Pocket",
        op: OperationType::Pocket,
        tool: tool_of(ToolType::EndMill, 6.0),
        expected_row: "amana-flat-hardwood-pocket-6000-2f-spektra",
        point_mm: 0.127,
        operation_family: LutOperationFamily::Pocket,
        pass_role: LutPassRole::Roughing,
        sample_doc_mm: 2.0,
        gate_reads_the_point: true,
    }
}

fn suggest(cell: &PointCell) -> SuggestedParams {
    let machine = MachineProfile::default();
    let stock = StockContext {
        stock_top_z: 0.0,
        stock_bottom_z: -18.0,
        stock_z: 18.0,
        stock_padding: 2.0,
    };
    suggest_params(SuggestParamsInput {
        op_type: cell.op,
        tool: &cell.tool,
        machine: &machine,
        material: &hardwood(),
        lut: &EMBEDDED_LUT,
        stock_ctx: &stock,
        spindle_strategy: SpindleStrategy::default(),
        context: SuggestContext::default(),
    })
    .unwrap_or_else(|e| panic!("{}: Suggest refused: {e}", cell.label))
}

/// A steady cut at `feed` on toolpath 0: 12 samples at the cell's depth.
fn steady_trace(cell: &PointCell, rpm: u32, feed: f64) -> SimulationCutTrace {
    let chip = feed / (f64::from(rpm) * 2.0);
    let samples: Vec<SimulationCutSample> = (0..12)
        .map(|idx| SimulationCutSample {
            toolpath_id: ToolpathId(0),
            move_index: idx,
            sample_index: idx,
            position: [0.0, 0.0, -cell.sample_doc_mm],
            cumulative_time_s: 0.1 * idx as f64,
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: CutKinematics::Linear,
            feed_rate_mm_min: feed,
            spindle_rpm: rpm,
            flute_count: 2,
            axial_doc_mm: cell.sample_doc_mm,
            axial_engagement_mm: cell.sample_doc_mm,
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

/// (b) One point cell: Suggest, the gate, the envelope resolver and the
/// modulator all hold the printed value as a point.
fn assert_the_cell_is_held_as_a_point(cell: &PointCell) {
    let label = cell.label;

    // Suggest: no band, the point.
    let s = suggest(cell);
    let recipe = s
        .feeds_result
        .matched_lut_row
        .as_ref()
        .unwrap_or_else(|| panic!("{label}: no vendor row answered"));
    assert_eq!(recipe.observation_id, cell.expected_row, "{label}");
    assert_eq!(
        recipe.printed_chipload(),
        PrintedChipload::Point {
            value_mm: cell.point_mm
        },
        "{label}: the recipe row is the printed point"
    );
    assert_eq!(recipe.chip_load_min_mm, None, "{label}: no minimum");
    assert!(
        s.feeds_result.chipload_bounds.is_none(),
        "{label}: Suggest derives no band from a point; got {:?}",
        s.feeds_result.chipload_bounds
    );
    let suggest_point = s
        .feeds_result
        .chipload_point_mm
        .unwrap_or_else(|| panic!("{label}: Suggest carries no point"));
    let ap = s.feeds_result.axial_depth_mm;
    if matches!(cell.tool.tool_type, ToolType::EndMill) {
        // Flat end: the de-rate ratio divides by D (the `g1` pattern).
        let expected = cell.point_mm * doc_derating_scale(ap / cell.tool.diameter);
        assert!(
            (suggest_point - expected).abs() < TOL,
            "{label}: Suggest point {suggest_point} vs {expected} (ap {ap})"
        );
    } else {
        // A 60 deg V-bit: the ratio is 0.866 at every depth, so no de-rate.
        assert!(
            (suggest_point - cell.point_mm).abs() < TOL,
            "{label}: Suggest point {suggest_point} vs the printed {} (ap {ap})",
            cell.point_mm
        );
    }

    // The gate: `{ min: None, max: v' }`, `VendorLutPointPreset`, and a burn
    // advisory below the point. 0.02 mm/tooth (720 mm/min at 18 000 rpm x 2
    // flutes) is below both points.
    const RPM: u32 = 18_000;
    const FEED: f64 = 720.0;
    let trace = steady_trace(cell, RPM, FEED);
    let tool_def = build_cutter(&cell.tool);
    let material = hardwood();
    let machine = MachineProfile::default();
    let tolerance = ToleranceBands::default();
    let ctx = ToolpathLoadContext {
        toolpath_id: ToolpathId(0),
        tool: &tool_def,
        material: &material,
        operation_family: cell.operation_family,
        pass_role: cell.pass_role,
        operation_feed_rate_mm_min: FEED,
        operation_kind: cell.op,
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
        panic!("{label}: a chip below the point is advisory, never Exceeds(Low): {verdict:?}");
    };
    let bounds = &approach_to_max.bounds;
    if !cell.gate_reads_the_point {
        // A cell whose gate reads another row. None since ruling B4 (before
        // B4 the gate keyed the AMS-159 V-bit at its engaged width and read
        // a banded row flagged extrapolated). The gate still never trips low.
        assert!(
            bounds.source.low_side_is_advisory(),
            "{label}: {:?}",
            bounds.source
        );
        return;
    }
    assert_eq!(
        bounds.source,
        ChipBoundsSource::VendorLutPointPreset,
        "{label}"
    );
    assert_eq!(
        bounds.min_mm_per_tooth, None,
        "{label}: a point is not a band"
    );
    assert!(approach_to_min.is_none(), "{label}: {approach_to_min:?}");
    let advisory = burn_advisory
        .as_deref()
        .unwrap_or_else(|| panic!("{label}: a chip below the point gives a burn advisory"));
    assert_eq!(
        advisory.bounds.burn_reference_mm(),
        Some(bounds.max_mm_per_tooth),
        "{label}"
    );
    assert_eq!(
        advisory.bounds.burn_reference_label(),
        "printed point",
        "{label}"
    );

    // The envelope resolver on the same trace: a point, the same v' as the
    // gate (one resolver, one de-rate).
    let operation = OperationConfig::new_default(cell.op);
    let target = chip_target_for_toolpath(
        &material,
        &cell.tool,
        &operation,
        ToolpathId(0),
        Some(&trace),
    );
    let target_point = match &target {
        Some(ChipTarget::Point(value)) => *value,
        other => panic!("{label}: the chip target must be a point; got {other:?}"),
    };
    assert!(
        (target_point - bounds.max_mm_per_tooth).abs() < TOL,
        "{label}: the chip target {target_point} and the gate max {} must be one v'",
        bounds.max_mm_per_tooth
    );
    if (target_point - cell.point_mm).abs() >= TOL {
        // Not derived: the gate de-rates the point at the sample depth. Both
        // cells here are at a depth ratio <= 1, so this is not expected.
        eprintln!(
            "{label}: gate v' {target_point} differs from the printed point {} at the \
             sample depth {} mm (a2 sentry `the_*_cell_is_held_as_a_point_a2`)",
            cell.point_mm, cell.sample_doc_mm
        );
    }
    if matches!(cell.tool.tool_type, ToolType::EndMill) {
        // Derived: key 6.0 on a 6.0 row, Janka 1450 on 1450, ratio 2 / 6.
        assert!(
            (target_point - cell.point_mm).abs() < TOL,
            "{label}: v' {target_point} vs the printed {}",
            cell.point_mm
        );
    }

    // The band-only envelope (the viewport, Q3) carries no point.
    let envelope = chipload_envelope_for_toolpath(
        &material,
        &cell.tool,
        &operation,
        ToolpathId(0),
        Some(&trace),
    );
    assert!(
        envelope.is_none(),
        "{label}: the band-only envelope must not carry a point; got {envelope:?}"
    );

    // The modulator's band is a point: a cap at v' with no floor.
    let band = ChipTarget::Point(target_point)
        .modulation_band()
        .unwrap_or_else(|| panic!("{label}: a valid point gives a modulation band"));
    assert!(band.is_point(), "{label}: {band:?}");
    assert!(
        (band.max_mm_per_tooth - target_point).abs() < TOL,
        "{label}"
    );
}

/// (b) The AMS-159 60 deg hardwood Trace cell (two equal printed limits).
#[test]
fn the_ams159_trace_cell_is_held_as_a_point_a2() {
    assert_the_cell_is_held_as_a_point(&ams159_trace());
}

/// (b) The Spektra 6 mm hardwood Pocket cell (a printed maximum only).
#[test]
fn the_spektra_pocket_cell_is_held_as_a_point_a2() {
    assert_the_cell_is_held_as_a_point(&spektra_pocket());
}

/// (c) Ruling B5 (G6): the drill claim scales a printed point by 1 / Z, and
/// the scaled value is still a point. A 6.0 mm 2-flute flat plunge in
/// hardwood reads `amana-flat-hardwood-pocket-6000-2f-spektra` (a maximum
/// 0.127 only) at x0.5: a point at 0.0635 with no minimum, on both
/// resolvers.
#[test]
fn a_drill_claim_keeps_a_printed_point_a_point_a2() {
    use rs_cam_core::feeds::vendor_lookup::find_best_row_for_geometry;
    use rs_cam_core::feeds::vendor_lut::{HardnessKind, MaterialFamily, ToolFamily};
    let lut = embedded_vendor_lut();
    let query = LookupQuery {
        tool_family: ToolFamily::FlatEnd,
        tool_subfamily: None,
        diameter_mm: 6.0,
        flute_count: 2,
        material_family: MaterialFamily::Hardwood,
        hardness_kind: Some(HardnessKind::Janka),
        hardness_value: Some(1450.0),
        operation_family: LutOperationFamily::Drill,
        pass_role: LutPassRole::Roughing,
    };
    let recipe = find_best_row_for_geometry(lut, &query, &ToolGeometryHint::Flat)
        .expect("the drill claim reads the Spektra side row");
    let gate = find_best_chip_envelope_row(lut, &query, &ToolGeometryHint::Flat)
        .expect("the envelope resolver reads the same row");
    for row in [&recipe, &gate] {
        assert_eq!(
            row.observation_id,
            "amana-flat-hardwood-pocket-6000-2f-spektra"
        );
        assert!(row.drill_basis.claim().is_some(), "{row:?}");
        assert_eq!(row.chip_load_min_mm, None, "a point carries no minimum");
        let PrintedChipload::Point { value_mm } = row.printed_chipload() else {
            panic!("a scaled point stays a point: {row:?}");
        };
        assert!((value_mm - 0.0635).abs() < TOL, "0.127 / 2, got {value_mm}");
    }
}
