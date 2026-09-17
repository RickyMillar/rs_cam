//! Unit tests for the unified finishing pass. Moved out of
//! `finish/unified_finish.rs` by P4; the module body is unchanged.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use crate::geo::P3;

use super::region_routing::rest_grid_index;

use super::*;
use crate::mesh::{make_test_flat, make_test_hemisphere};
use crate::polygon::Polygon2;
use crate::tool::BallEndmill;
use std::sync::atomic::{AtomicBool, Ordering};

// ── C2 follow-up 1: window-clipped, phase-preserving lattices ───────
//
// ACCEPTANCE BAR. Clipping the SAMPLED window to a region must not move
// one byte of emitted motion: the cells only ever select points inside
// the region polygon, so sampling outside its padded bbox is pure waste.
// Anything else — a re-phased lattice, a flipped serpentine row, a
// dropped boundary point — shows up here as a move-list difference.
//
// Both windowed frames are covered, because they window in DIFFERENT
// coordinate systems: `direction_deg == 0.0` (which a gate-PASSING
// region reaches whenever its PCA-minor axis folds to 0 — a shape
// elongated along Y) takes `batch_drop_cutter`'s axis-aligned path and
// windows in world XY; a rotated frame windows on forward-rotated
// corners.

/// Byte-comparable fingerprint of a move list. Every `f64` travels as
/// its BIT pattern: `PartialEq` would let a `-0.0`/`0.0` divergence —
/// exactly what a re-derived lattice origin produces — pass as equal.
fn move_bits(tp: &Toolpath) -> Vec<String> {
    tp.moves
        .iter()
        .map(|m| {
            let (kind, i, j, feed) = match m.move_type {
                crate::toolpath::MoveType::Rapid => ("rapid", 0.0, 0.0, 0.0),
                crate::toolpath::MoveType::Linear { feed_rate } => ("linear", 0.0, 0.0, feed_rate),
                crate::toolpath::MoveType::ArcCW { i, j, feed_rate } => ("cw", i, j, feed_rate),
                crate::toolpath::MoveType::ArcCCW { i, j, feed_rate } => ("ccw", i, j, feed_rate),
            };
            format!(
                "{kind}|{:016x}|{:016x}|{:016x}|{:016x}|{:016x}|{:016x}|{:?}",
                m.target.x.to_bits(),
                m.target.y.to_bits(),
                m.target.z.to_bits(),
                i.to_bits(),
                j.to_bits(),
                feed.to_bits(),
                m.intent
            )
        })
        .collect()
}

/// A "U" region on a flat plate: joined along the base, two legs above
/// the notch. Swept at 0° that is the canonical boustrophedon SPLIT, so
/// the cell arm below emits more than one cell rather than degenerating
/// to the undivided case.
fn u_region() -> Polygon2 {
    Polygon2::new(vec![
        P2::new(-20.0, -10.0),
        P2::new(20.0, -10.0),
        P2::new(20.0, 10.0),
        P2::new(7.0, 10.0),
        P2::new(7.0, -2.0),
        P2::new(-7.0, -2.0),
        P2::new(-7.0, 10.0),
        P2::new(-20.0, 10.0),
    ])
}

/// What one region emits on one lattice: the undivided raster, the
/// per-cell rasters, the cell count and the membership-mismatch count —
/// exactly the four things the band arm derives from a grid.
struct ShallowEmission {
    undivided: Toolpath,
    cellwise: Toolpath,
    cells: usize,
    mismatches: usize,
}

fn shallow_emission_on(
    grid: &DropCutterGrid,
    region: &Polygon2,
    params: &UnifiedFinishParams,
    min_z: f64,
) -> ShallowEmission {
    let region_set = RegionSet::new(vec![region.clone()]);
    let undivided = raster_toolpath_from_grid(
        grid,
        params.feed_rate,
        params.plunge_rate,
        params.safe_z,
        Some(min_z),
        Some(&region_set),
    );
    let decomposed = crate::geometry::monotone_cells::lattice_monotone_cells(grid, region, min_z);
    let mismatches = crate::geometry::monotone_cells::cells_select_same_lattice(
        grid,
        region,
        &decomposed.cells,
        min_z,
    );
    let mut cellwise = Toolpath::new();
    for cell in &decomposed.cells {
        let cell_set = RegionSet::new(vec![cell.clone()]);
        let cell_tp = raster_toolpath_from_grid(
            grid,
            params.feed_rate,
            params.plunge_rate,
            params.safe_z,
            Some(min_z),
            Some(&cell_set),
        );
        cellwise.moves.extend(cell_tp.moves);
    }
    ShallowEmission {
        undivided,
        cellwise,
        cells: decomposed.cells.len(),
        mismatches,
    }
}

/// Compare two move lists bit-for-bit, reporting the FIRST divergence
/// rather than dumping two multi-thousand-entry vectors.
fn assert_same_moves(want: &Toolpath, got: &Toolpath, what: &str) {
    let (want_bits, got_bits) = (move_bits(want), move_bits(got));
    assert_eq!(
        want_bits.len(),
        got_bits.len(),
        "{what}: move COUNT diverged ({} vs {})",
        want_bits.len(),
        got_bits.len()
    );
    for (i, (a, b)) in want_bits.iter().zip(got_bits.iter()).enumerate() {
        assert_eq!(a, b, "{what}: move {i} of {} diverged", want_bits.len());
    }
}

fn windowed_lattice_is_byte_identical_at(direction_deg: f64) {
    let mesh = make_test_flat(60.0);
    let index = SpatialIndex::build(&mesh, 10.0);
    let cutter = ball(3.0);
    let params = UnifiedFinishParams {
        raster_stepover: 0.8,
        ..UnifiedFinishParams::default()
    };
    let never_cancel = || false;
    let region = u_region();
    let min_z = mesh.bbox.min.z - 0.1 + params.stock_to_leave;

    let whole = build_shallow_raster_grid(
        &mesh,
        &index,
        &cutter,
        &params,
        params.raster_stepover,
        direction_deg,
        None,
        &never_cancel,
    )
    .unwrap();
    let windowed = build_shallow_raster_grid(
        &mesh,
        &index,
        &cutter,
        &params,
        params.raster_stepover,
        direction_deg,
        Some(region_sampling_window(&region, params.raster_stepover)),
        &never_cancel,
    )
    .unwrap();

    // Non-vacuity: without this the equalities below would pass for
    // free if the window ever degenerated to the whole lattice.
    assert!(
        windowed.points.len() < whole.points.len(),
        "window sampled {} of {} points at {direction_deg}° — no saving, \
             so the byte-identity assertions below prove nothing",
        windowed.points.len(),
        whole.points.len()
    );
    // The lattice did not move: local (0,0) is a PARENT lattice point.
    let col_off = (windowed.u_start - whole.u_start) / whole.x_step;
    let row_off = (windowed.v_start - whole.v_start) / whole.y_step;
    assert!(
        (col_off - col_off.round()).abs() < 1e-9,
        "u_start off-lattice"
    );
    assert!(
        (row_off - row_off.round()).abs() < 1e-9,
        "v_start off-lattice"
    );
    assert_eq!(
        row_off.round() as usize % 2,
        0,
        "serpentine phase: v_start must sit on an EVEN parent row"
    );

    let want = shallow_emission_on(&whole, &region, &params, min_z);
    let got = shallow_emission_on(&windowed, &region, &params, min_z);

    // The fixture must actually reach the CELL arm — a membership
    // fallback would leave the per-cell comparison below testing two
    // copies of the undivided raster.
    assert_eq!(
        want.mismatches, 0,
        "fixture fell back to the undivided raster at {direction_deg}°"
    );
    assert_eq!(
        want.mismatches, got.mismatches,
        "the window changed the membership VERDICT at {direction_deg}° \
             ({} vs {}) — the fallback decision must be window-invariant too",
        want.mismatches, got.mismatches
    );
    assert!(
        !want.undivided.moves.is_empty(),
        "fixture emitted nothing at {direction_deg}°"
    );
    assert_eq!(
        want.cells, got.cells,
        "cell count diverged at {direction_deg}°"
    );
    assert_same_moves(
        &want.undivided,
        &got.undivided,
        &format!("undivided raster at {direction_deg}°"),
    );
    assert_same_moves(
        &want.cellwise,
        &got.cellwise,
        &format!("per-cell raster at {direction_deg}°"),
    );
}

#[test]
fn a_windowed_shallow_lattice_emits_identical_moves_at_zero_degrees() {
    // The axis-aligned sampling path: the window is applied in world XY.
    windowed_lattice_is_byte_identical_at(0.0);
}

#[test]
fn a_windowed_shallow_lattice_emits_identical_moves_when_rotated() {
    // The rotated sampling path: the window's four world corners are
    // forward-rotated into (u, v) before the index ranges are taken.
    // 89.9° is `honest_raster_direction_deg`'s own nudge off the
    // dishonest 90° fast path; 37° is an ordinary oblique frame.
    windowed_lattice_is_byte_identical_at(89.9);
    windowed_lattice_is_byte_identical_at(37.0);
}

fn ball(diameter: f64) -> BallEndmill {
    BallEndmill::new(diameter, diameter * 5.0)
}

/// C4: `span_label` / `from_span_label` are inverses over the whole
/// vocabulary, so a consumer that asks for a kind by label breaks here —
/// loudly, once — rather than silently matching nothing at its own site.
#[test]
fn region_kind_span_labels_round_trip() {
    assert_eq!(RegionKind::ALL.len(), 4);
    for kind in RegionKind::ALL {
        let label = kind.span_label();
        assert_eq!(
            RegionKind::from_span_label(&label),
            Some(kind),
            "{label:?} did not round-trip"
        );
    }

    // The exact strings two harnesses used to spell out themselves.
    assert_eq!(
        RegionKind::from_span_label("Pencil claims"),
        Some(RegionKind::Crease)
    );
    assert_eq!(
        RegionKind::from_span_label("MidSteep band"),
        Some(RegionKind::Band(FinishBand::MidSteep))
    );

    // Sibling annotation spans are NOT region nodes and must not be
    // mistaken for one — `v3_cascade_ab.rs` measured 620 outer spans of
    // which 615 were rings.
    assert_eq!(RegionKind::from_span_label("Ring 3/9"), None);
    assert_eq!(RegionKind::from_span_label("Z level 2"), None);
    assert_eq!(RegionKind::from_span_label(""), None);

    // Distinct labels: a copy-pasted arm would make one kind unreachable.
    let mut labels: Vec<String> = RegionKind::ALL.iter().map(|k| k.span_label()).collect();
    labels.sort();
    labels.dedup();
    assert_eq!(labels.len(), RegionKind::ALL.len());
}

/// FIN-13: a band becomes text in ONE place, and the band findings carry
/// the enum rather than the text.
///
/// The finding types restated the producer's fields and took the band as a
/// `&'static str`, so the enum existed on one side of the boundary and a
/// token on the other. A typo in either table was unreachable from the
/// other.
#[test]
fn a_band_renders_through_one_label_table() {
    assert_eq!(FinishBand::ALL.len(), 3);

    // One table. `RegionKind` renders a band by asking the band.
    for band in FinishBand::ALL {
        assert_eq!(
            RegionKind::Band(band).band_label(),
            band.label(),
            "{band:?} renders two different tokens"
        );
        assert_eq!(
            RegionKind::from_span_label(&format!("{} band", band.label())),
            Some(RegionKind::Band(band)),
            "{band:?} did not round-trip through its own label"
        );
    }

    // Distinct labels: two bands that render alike make one unreachable.
    let mut labels: Vec<&str> = FinishBand::ALL.iter().map(|b| b.label()).collect();
    labels.sort_unstable();
    labels.dedup();
    assert_eq!(labels.len(), FinishBand::ALL.len(), "band labels collide");

    // The tokens every surface printed before FIN-13.
    assert_eq!(FinishBand::VerySteep.label(), "VerySteep");
    assert_eq!(FinishBand::MidSteep.label(), "MidSteep");
    assert_eq!(FinishBand::Shallow.label(), "Shallow");
    assert_eq!(
        crate::compute::config::HeightClip::BottomZ.label(),
        "bottom_z"
    );
    assert_eq!(crate::compute::config::HeightClip::TopZ.label(), "top_z");
}

/// `v3_cascade_ab.rs::strategy_of_span` was a four-arm label -> strategy
/// table. It is now `from_span_label(..).map(|k| k.strategy().label())`.
/// This pins that the composition returns exactly what the hand-written
/// table did, so the migration is provably value-preserving and the
/// harness's `UNFUSED_ORDER` tokens still match.
#[test]
fn span_label_to_strategy_reproduces_the_retired_harness_table() {
    let via_kind =
        |label: &str| RegionKind::from_span_label(label).map(|kind| kind.strategy().label());
    assert_eq!(via_kind("VerySteep band"), Some("waterline"));
    assert_eq!(via_kind("MidSteep band"), Some("scallop"));
    assert_eq!(via_kind("Shallow band"), Some("raster"));
    assert_eq!(via_kind("Pencil claims"), Some("pencil"));
    assert_eq!(via_kind("Ring 3/9"), None);
}

// ── fixture 1: flat plate (Shallow only) ────────────────────────────

#[test]
fn flat_plate_generates_raster_only() {
    let mesh = make_test_flat(60.0);
    let index = SpatialIndex::build(&mesh, 10.0);
    let cutter = ball(6.0); // radius 3.0
    let params = UnifiedFinishParams::default();
    let planner = FinishPlannerParams::for_tool(cutter.cusp_radius());
    let never_cancel = || false;

    let (tp, anns, report) = unified_finish_toolpath_with_cancel(
        &mesh,
        &index,
        &cutter,
        5.0,
        -5.0,
        &params,
        &planner,
        None,
        None,
        None,
        None,
        &never_cancel,
    )
    .unwrap();

    assert!(
        report.shallow.move_count > 0,
        "flat plate should produce shallow raster moves"
    );
    assert_eq!(report.mid_steep.move_count, 0);
    assert_eq!(report.very_steep.move_count, 0);
    assert!(!tp.moves.is_empty());
    assert!(
        anns.is_empty(),
        "raster-only run should carry no scallop annotations"
    );
}

// ── fixture 2: hemisphere (spans all three bands) ───────────────────
//
// Radius/tool-radius/cell-size mirror `finish_planner`'s own
// `dome_decomposes_into_four_regions` test (radius 30, cell 1.0,
// `FinishPlannerParams::for_tool(3.0)`) as closely as possible — that
// test proves the decomposition dials produce a clean
// shallow/mid/very-steep split at this exact scale. The only new
// variable here is going through a real triangulated mesh + the
// classification-surface probe pipeline instead of a synthetic z-grid;
// R1's hysteresis/close/min-area conditioning is specifically built to
// absorb the resulting facet noise.
fn steep_cone_fixture() -> (TriangleMesh, SpatialIndex, BallEndmill, FinishPlannerParams) {
    let mesh = make_test_hemisphere(30.0, 24);
    let index = SpatialIndex::build(&mesh, 10.0);
    let cutter = ball(6.0); // radius 3.0
    let planner = FinishPlannerParams::for_tool(cutter.cusp_radius());
    (mesh, index, cutter, planner)
}

fn steep_cone_params() -> UnifiedFinishParams {
    UnifiedFinishParams {
        // Forces classification cell_size to exactly 1.0mm
        // (`(tool_radius / 4).max(tolerance)` with tool_radius = 3.0),
        // matching the mirrored `finish_planner` test's grid.
        tolerance: 1.0,
        ..UnifiedFinishParams::default()
    }
}

#[test]
fn steep_cone_generates_multiple_bands() {
    let (mesh, index, cutter, planner) = steep_cone_fixture();
    let params = steep_cone_params();
    let never_cancel = || false;

    let (tp, _anns, report) = unified_finish_toolpath_with_cancel(
        &mesh,
        &index,
        &cutter,
        35.0,
        0.0,
        &params,
        &planner,
        None,
        None,
        None,
        None,
        &never_cancel,
    )
    .unwrap();

    let populated = [report.very_steep, report.mid_steep, report.shallow]
        .iter()
        .filter(|b| b.move_count > 0)
        .count();
    assert!(
        populated >= 2,
        "hemisphere should span at least two bands, got very_steep={} mid_steep={} shallow={}",
        report.very_steep.move_count,
        report.mid_steep.move_count,
        report.shallow.move_count
    );
    let expected_total =
        report.very_steep.move_count + report.mid_steep.move_count + report.shallow.move_count;
    assert_eq!(tp.moves.len(), expected_total);
}

/// FIN-14: the height-clip finding says WHERE it was taken, so a clipped
/// `MidSteep` region reads as "not measured" and never as a clean band.
///
/// The fixture is the hemisphere, which bands by height: `VerySteep` below
/// z ≈ 7.8 mm, `MidSteep` between z ≈ 7.8 and z ≈ 21.2 mm, `Shallow` above.
/// A `bottom_z` of 15 mm therefore cuts straight through the `MidSteep`
/// span and takes the whole `VerySteep` ladder away.
///
/// The `MidSteep` arm reports nothing, and it cannot: `top_z` and
/// `bottom_z` reach the waterline arm alone. What the finding must carry is
/// that fact.
#[test]
fn a_clamped_mid_steep_band_reads_as_not_measured() {
    let (mesh, index, cutter, planner) = steep_cone_fixture();
    let params = steep_cone_params();
    let never_cancel = || false;

    let (_tp, _anns, report) = unified_finish_toolpath_with_cancel(
        &mesh,
        &index,
        &cutter,
        35.0,
        15.0,
        &params,
        &planner,
        None,
        None,
        None,
        None,
        &never_cancel,
    )
    .unwrap();

    assert!(
        report.mid_steep.region_count > 0,
        "the fixture must plan a MidSteep region for this case to mean \
         anything; got {:?}",
        report.mid_steep
    );
    assert!(
        report.height_clip_measured.very_steep,
        "the VerySteep arm compares its ladder with the resolved heights, \
         so it must enter the measured set"
    );
    assert!(
        !report.height_clip_measured.mid_steep,
        "the MidSteep arm reads no resolved height, so it must NOT read as \
         measured"
    );
    assert!(
        !report.height_clip_measured.shallow,
        "the Shallow arm reads no resolved height either"
    );

    let dropped = dropped_band_finding(&report);
    let clipped = clipped_band_finding(&report);
    let measured = dropped
        .map(|f| f.bands_measured)
        .or_else(|| clipped.map(|f| f.bands_measured))
        .expect("a bottom_z of 15 mm must clip the VerySteep ladder");
    assert!(
        measured.contains(FinishBand::VerySteep) && !measured.contains(FinishBand::MidSteep),
        "the finding must carry the measured set, not an empty claim: {measured:?}"
    );
    let described = measured.describe();
    assert!(
        described.contains("NOT measured on MidSteep"),
        "the operator sentence must name the unmeasured bands; got {described}"
    );
}

#[test]
fn annotations_shifted_by_concat_offset() {
    let (mesh, index, cutter, planner) = steep_cone_fixture();
    let params = steep_cone_params();
    let never_cancel = || false;

    let (tp, anns, report) = unified_finish_toolpath_with_cancel(
        &mesh,
        &index,
        &cutter,
        35.0,
        0.0,
        &params,
        &planner,
        None,
        None,
        None,
        None,
        &never_cancel,
    )
    .unwrap();

    assert!(
        !anns.is_empty(),
        "mid-steep band should carry scallop ring annotations"
    );
    for ann in &anns {
        assert!(
            ann.move_index < tp.moves.len(),
            "annotation move_index {} escaped the stitched toolpath ({} moves)",
            ann.move_index,
            tp.moves.len()
        );
        assert!(
            ann.move_index >= report.very_steep.move_count,
            "annotation move_index {} lands before the mid-steep band's concat offset ({})",
            ann.move_index,
            report.very_steep.move_count
        );
    }
}

#[test]
fn deterministic() {
    let (mesh, index, cutter, planner) = steep_cone_fixture();
    let params = steep_cone_params();
    let never_cancel = || false;

    let (tp_a, _anns_a, report_a) = unified_finish_toolpath_with_cancel(
        &mesh,
        &index,
        &cutter,
        35.0,
        0.0,
        &params,
        &planner,
        None,
        None,
        None,
        None,
        &never_cancel,
    )
    .unwrap();
    let (tp_b, _anns_b, report_b) = unified_finish_toolpath_with_cancel(
        &mesh,
        &index,
        &cutter,
        35.0,
        0.0,
        &params,
        &planner,
        None,
        None,
        None,
        None,
        &never_cancel,
    )
    .unwrap();

    assert_eq!(tp_a.moves.len(), tp_b.moves.len());
    assert_eq!(
        report_a.very_steep.move_count,
        report_b.very_steep.move_count
    );
    assert_eq!(report_a.mid_steep.move_count, report_b.mid_steep.move_count);
    assert_eq!(report_a.shallow.move_count, report_b.shallow.move_count);

    let first_a = tp_a
        .moves
        .first()
        .map(|m| (m.target.x, m.target.y, m.target.z));
    let first_b = tp_b
        .moves
        .first()
        .map(|m| (m.target.x, m.target.y, m.target.z));
    assert_eq!(first_a, first_b);
    let last_a = tp_a
        .moves
        .last()
        .map(|m| (m.target.x, m.target.y, m.target.z));
    let last_b = tp_b
        .moves
        .last()
        .map(|m| (m.target.x, m.target.y, m.target.z));
    assert_eq!(last_a, last_b);
}

// ── cancellation ─────────────────────────────────────────────────────

#[test]
fn cancellation_propagates() {
    let mesh = make_test_flat(60.0);
    let index = SpatialIndex::build(&mesh, 10.0);
    let cutter = ball(6.0);
    let params = UnifiedFinishParams::default();
    let planner = FinishPlannerParams::for_tool(cutter.cusp_radius());

    // False on the very first check, true on every check after —
    // guarantees at least one check succeeds (so classification can
    // start) but the run cannot complete without observing cancel.
    let already_checked = AtomicBool::new(false);
    let cancel_after_first = || already_checked.swap(true, Ordering::SeqCst);

    let result = unified_finish_toolpath_with_cancel(
        &mesh,
        &index,
        &cutter,
        5.0,
        -5.0,
        &params,
        &planner,
        None,
        None,
        None,
        None,
        &cancel_after_first,
    );
    assert!(
        result.is_err(),
        "expected cancellation to propagate as Err(Cancelled)"
    );
}

// ── P2.d router ──────────────────────────────────────────────────────

#[test]
fn preamble_and_retract_detection() {
    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(P3::new(1.0, 2.0, 30.0), MoveIntent::Linking);
    tp.feed_to_with_intent(P3::new(1.0, 2.0, 0.5), 500.0, MoveIntent::EntryPlunge);
    tp.feed_to_with_intent(P3::new(5.0, 2.0, 0.4), 1000.0, MoveIntent::FinishingCut);
    tp.feed_to_with_intent(P3::new(9.0, 2.0, 0.3), 1000.0, MoveIntent::FinishingCut);
    tp.rapid_to_with_intent(P3::new(9.0, 2.0, 30.0), MoveIntent::Retract);

    let (k, entry) = strippable_preamble(&tp).expect("canonical preamble");
    assert_eq!(k, 2);
    assert_eq!((entry.x, entry.y, entry.z), (1.0, 2.0, 0.5));

    let (n, exit) = trailing_retracts(&tp);
    assert_eq!(n, 1);
    let exit = exit.expect("exit point");
    assert_eq!((exit.x, exit.y, exit.z), (9.0, 2.0, 0.3));

    // A toolpath opening with a cut (no preamble) is not strippable.
    let mut bare = Toolpath::new();
    bare.feed_to_with_intent(P3::new(0.0, 0.0, 0.0), 1000.0, MoveIntent::FinishingCut);
    assert!(strippable_preamble(&bare).is_none());

    // An unrecognized preamble shape (e.g. a Retract before the first
    // cut) is kept verbatim rather than guessed at.
    let mut odd = Toolpath::new();
    odd.rapid_to_with_intent(P3::new(0.0, 0.0, 30.0), MoveIntent::Retract);
    odd.feed_to_with_intent(P3::new(0.0, 0.0, 0.0), 500.0, MoveIntent::EntryPlunge);
    odd.feed_to_with_intent(P3::new(1.0, 0.0, 0.0), 1000.0, MoveIntent::FinishingCut);
    assert!(strippable_preamble(&odd).is_none());
}

fn test_link_kinematics() -> crate::machine::kinematics::LinkKinematics {
    crate::machine::kinematics::LinkKinematics {
        kinematics: crate::machine::kinematics::MachineKinematics::default(),
        max_feed_mm_min: 3000.0,
        rapid_feed_mm_min: 5000.0,
    }
}

#[test]
fn router_orders_regions_and_reports_links() {
    let (mesh, index, cutter, planner) = steep_cone_fixture();
    let params = steep_cone_params();
    let lk = test_link_kinematics();
    let never_cancel = || false;

    let (tp, anns, report) = unified_finish_toolpath_with_cancel(
        &mesh,
        &index,
        &cutter,
        35.0,
        0.0,
        &params,
        &planner,
        None,
        Some(&lk),
        None,
        None,
        &never_cancel,
    )
    .unwrap();

    assert!(!tp.moves.is_empty());
    assert!(!report.route.is_empty(), "route must list every cut region");
    assert_eq!(
        report.links.len(),
        report.route.len() - 1,
        "one costed junction per consecutive route pair"
    );
    for link in &report.links {
        assert!(
            link.cost_s.is_finite() && link.cost_s >= 0.0,
            "junction cost must be a real integrated time, got {}",
            link.cost_s
        );
        if let Some(alt) = link.alt_cost_s {
            assert!(
                link.cost_s <= alt,
                "winning candidate ({:.3}s) must not cost more than the loser ({alt:.3}s)",
                link.cost_s
            );
        }
    }
    for ann in &anns {
        assert!(
            ann.move_index < tp.moves.len(),
            "annotation move_index {} escaped the stitched toolpath ({} moves)",
            ann.move_index,
            tp.moves.len()
        );
    }
}

#[test]
fn router_is_deterministic() {
    let (mesh, index, cutter, planner) = steep_cone_fixture();
    let params = steep_cone_params();
    let lk = test_link_kinematics();
    let never_cancel = || false;

    let run = || {
        unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            35.0,
            0.0,
            &params,
            &planner,
            None,
            Some(&lk),
            None,
            None,
            &never_cancel,
        )
        .unwrap()
    };
    let (tp_a, _, report_a) = run();
    let (tp_b, _, report_b) = run();
    assert_eq!(tp_a.moves.len(), tp_b.moves.len());
    assert_eq!(report_a.route, report_b.route);
    assert_eq!(report_a.links.len(), report_b.links.len());
    for (a, b) in report_a.links.iter().zip(report_b.links.iter()) {
        assert_eq!(a.surface, b.surface);
        assert_eq!(a.from_region, b.from_region);
        assert_eq!(a.to_region, b.to_region);
    }
}

/// A synthetic region path at `x_center` on a flat surface (z = 0):
/// the canonical `Linking rapid → EntryPlunge → cuts → Retract` shape
/// every strategy in this module emits.
fn synthetic_region(region_index: usize, band: FinishBand, x_center: f64) -> RegionPath {
    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(P3::new(x_center - 3.0, 0.0, 30.0), MoveIntent::Linking);
    tp.feed_to_with_intent(
        P3::new(x_center - 3.0, 0.0, 0.0),
        500.0,
        MoveIntent::EntryPlunge,
    );
    tp.feed_to_with_intent(
        P3::new(x_center + 3.0, 0.0, 0.0),
        1000.0,
        MoveIntent::FinishingCut,
    );
    tp.rapid_to_with_intent(P3::new(x_center + 3.0, 0.0, 30.0), MoveIntent::Retract);
    let (head_strip, entry) = strippable_preamble(&tp).map(|(k, p)| (k, Some(p))).unwrap();
    let (tail_strip, exit) = trailing_retracts(&tp);
    RegionPath {
        region_index,
        band,
        tp,
        anns: Vec::new(),
        head_strip,
        entry,
        tail_strip,
        exit,
    }
}

#[test]
fn route_greedy_seeds_at_steepest_and_chains_nearest() {
    let mesh = make_test_flat(60.0);
    let index = SpatialIndex::build(&mesh, 10.0);
    let cutter = ball(6.0);
    let params = UnifiedFinishParams::default();
    let lk = test_link_kinematics();
    let never_cancel = || false;

    // Planned order (Shallow first, as `decompose` emits): a Shallow
    // region on each flank, the lone MidSteep in the middle.
    let paths = vec![
        synthetic_region(0, FinishBand::Shallow, -20.0),
        synthetic_region(1, FinishBand::Shallow, 20.0),
        synthetic_region(2, FinishBand::MidSteep, 0.0),
    ];

    let (order, junctions) = route_greedy(
        &paths,
        &mesh,
        &index,
        &cutter,
        &params,
        None,
        &lk,
        &never_cancel,
    )
    .unwrap();

    assert_eq!(
        order.first(),
        Some(&2),
        "route must seed at the steepest band present, got {order:?}"
    );
    // The MidSteep region exits at x = +3: the east flank's entry
    // (x = 17) is 14 mm away, the west flank's (x = -23) is 26 mm —
    // greedy must take the near one first.
    assert_eq!(order, vec![2, 1, 0]);
    assert_eq!(junctions.len(), 2);
    for j in junctions.iter().flatten() {
        assert!(j.cost_s.is_finite() && j.cost_s > 0.0);
    }
}

// ── machining boundary ───────────────────────────────────────────────

#[test]
fn boundary_restricts_output() {
    let mesh = make_test_flat(60.0);
    let index = SpatialIndex::build(&mesh, 10.0);
    let cutter = ball(6.0);
    let params = UnifiedFinishParams::default();
    let planner = FinishPlannerParams::for_tool(cutter.cusp_radius());
    let never_cancel = || false;

    let (unrestricted, _anns, _report) = unified_finish_toolpath_with_cancel(
        &mesh,
        &index,
        &cutter,
        5.0,
        -5.0,
        &params,
        &planner,
        None,
        None,
        None,
        None,
        &never_cancel,
    )
    .unwrap();

    let square = Polygon2::rectangle(-10.0, -10.0, 10.0, 10.0);
    let regions = vec![square];
    let boundary = RegionSet::from_slice(&regions);

    let (restricted, _anns2, _report2) = unified_finish_toolpath_with_cancel(
        &mesh,
        &index,
        &cutter,
        5.0,
        -5.0,
        &params,
        &planner,
        Some(&boundary),
        None,
        None,
        None,
        &never_cancel,
    )
    .unwrap();

    assert!(
        restricted.moves.len() < unrestricted.moves.len(),
        "boundary-restricted run should produce fewer moves ({} vs {})",
        restricted.moves.len(),
        unrestricted.moves.len()
    );

    // Stepover slack at the region edge: a raster row point can sit up
    // to one stepover past the boundary before the run-splitter drops
    // it.
    let margin = params.raster_stepover.max(cutter.radius());
    let mut saw_cut = false;
    for m in &restricted.moves {
        if let crate::toolpath::MoveType::Linear { .. } = m.move_type
            && m.intent == crate::toolpath::MoveIntent::FinishingCut
        {
            saw_cut = true;
            assert!(
                m.target.x >= -10.0 - margin
                    && m.target.x <= 10.0 + margin
                    && m.target.y >= -10.0 - margin
                    && m.target.y <= 10.0 + margin,
                "cutting move ({:.2},{:.2}) escaped the boundary square",
                m.target.x,
                m.target.y
            );
        }
    }
    assert!(
        saw_cut,
        "expected at least one cutting move inside the boundary"
    );
}

// ── claims pipeline (v3 S1) ──────────────────────────────────────────

/// A Gaussian-profile trench running along X, centered at `y = 0`.
/// Mirrors `rest_field::tests::make_trench` (duplicated here — small and
/// test-only, each module's fixture stays independently readable): a
/// hard-edged plane-wall V has a CONSTANT rest depth along its length
/// (a plateau, no local maximum) and the RestDepth detector's
/// NMS-based ridge extraction never latches onto one (see
/// `rest_field::tests::v_valley_yields_one_centerline`'s doc comment) —
/// only a curved profile like this one is genuinely detectable.
fn make_trench_mesh(
    len_x: f64,
    half_y: f64,
    depth: f64,
    sigma: f64,
    nx: usize,
    ny: usize,
) -> TriangleMesh {
    let mut verts = Vec::new();
    for iy in 0..=ny {
        let y = -half_y + 2.0 * half_y * iy as f64 / ny as f64;
        let z = -depth * (-(y / sigma).powi(2)).exp();
        for ix in 0..=nx {
            let x = len_x * ix as f64 / nx as f64;
            verts.push(P3::new(x, y, z));
        }
    }
    let mut tris = Vec::new();
    let stride = nx + 1;
    for iy in 0..ny {
        for ix in 0..nx {
            let a = (iy * stride + ix) as u32;
            let b = a + 1;
            let c = a + stride as u32;
            let d = c + 1;
            tris.push([a, b, d]);
            tris.push([a, d, c]);
        }
    }
    TriangleMesh::from_raw(verts, tris)
}

/// Shared claims-on fixture: the trench mesh, a small pencil/finishing
/// tool, and a bigger analytic reference tool that cannot reach the
/// trench floor — same tool sizes as `rest_field`'s own
/// `v_valley_yields_one_centerline` proof.
fn trench_claims_fixture() -> (
    TriangleMesh,
    SpatialIndex,
    BallEndmill,
    UnifiedFinishParams,
    FinishPlannerParams,
) {
    let mesh = make_trench_mesh(30.0, 6.0, 1.5, 1.2, 30, 48);
    let index = SpatialIndex::build_auto(&mesh);
    // Crease detection is the analytic SELF-probe (rest = where the
    // op's own cutter floats above the bare surface), so the fixture
    // cutter must BRIDGE the 1.5 mm trench: radius 1.0 > half-width
    // 0.75 floats ~0.86 mm above the floor — a detectable rest ridge
    // along the trench axis.
    let cutter = ball(2.0);
    let params = UnifiedFinishParams {
        tolerance: 0.5,
        ..UnifiedFinishParams::default()
    };
    let planner = FinishPlannerParams::for_tool(cutter.cusp_radius());
    (mesh, index, cutter, params, planner)
}

/// Regression sentry: the region-node ranges must TILE the stitched
/// toolpath, leaving no move between two nodes.
///
/// A move outside every node range is not a cosmetic gap. Surface links
/// are *feed* moves and `tsp::split_into_segments` only splits on
/// `MoveType::Rapid`, so an orphaned link is glued into a cutting
/// segment straddling the node boundary; the reorder relocates that
/// segment into the region it just left, and
/// `tsp::remap_spans`'s foreign-intrusion guard then DROPS the region
/// span for containing a foreign move.
///
/// Measured on wanaka ×2 before the fix: five region nodes dropped,
/// each tripped by exactly one intruder — in every case the single
/// `MoveIntent::Linking` move sitting at `span.end_move`. Those five
/// carried 87% of the operation's cutting length, so the survivors
/// described 12.6% of the op while `spans_valid` still read `true`,
/// and `narrate_toolpath` reported `regions 0` on the live GUI.
/// See `planning/unified_v3_design.md` §14c/§14d/§14g.
#[test]
fn region_node_ranges_tile_the_stitched_toolpath() {
    let (mesh, index, cutter, params, planner) = trench_claims_fixture();
    let never_cancel = || false;

    let (tp, _anns, report) = unified_finish_toolpath_with_cancel(
        &mesh,
        &index,
        &cutter,
        5.0,
        -5.0,
        &params,
        &planner,
        None,
        None,
        None,
        None,
        &never_cancel,
    )
    .unwrap();

    assert!(
        !report.region_table.is_empty(),
        "fixture must route at least one region"
    );
    for pair in report.region_table.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        assert_eq!(
            a.move_range.end,
            b.move_range.start,
            "region nodes must be contiguous — {:?} ends at {} but {:?} \
                 starts at {}, orphaning {} move(s) that the rapid reorder \
                 can relocate into the previous node and so trip the \
                 foreign-intrusion guard",
            a.kind,
            a.move_range.end,
            b.kind,
            b.move_range.start,
            b.move_range.start.saturating_sub(a.move_range.end),
        );
    }
    if let Some(last) = report.region_table.last() {
        assert_eq!(
            last.move_range.end,
            tp.moves.len(),
            "the last region node must reach the end of the stitched \
                 toolpath"
        );
    }
}

#[test]
fn claims_off_matches_legacy_band_only_output() {
    let (mesh, index, cutter, params, planner) = trench_claims_fixture();
    let never_cancel = || false;

    let (tp, _anns, report) = unified_finish_toolpath_with_cancel(
        &mesh,
        &index,
        &cutter,
        5.0,
        -5.0,
        &params,
        &planner,
        None,
        None,
        None,
        None,
        &never_cancel,
    )
    .unwrap();

    assert!(
        report.claims.is_none(),
        "claims pipeline must not run when claims=None"
    );
    assert!(
        report
            .region_table
            .iter()
            .all(|e| matches!(e.kind, RegionKind::Band(_))),
        "no crease node should appear when claims=None, even on a \
             crease-bearing mesh: {:?}",
        report.region_table
    );
    let banded_total: usize = report
        .region_table
        .iter()
        .map(|e| e.move_range.end - e.move_range.start)
        .sum();
    assert_eq!(
        tp.moves.len(),
        banded_total,
        "toolpath must be exactly the band regions with claims off"
    );
}

#[test]
fn claims_on_emits_crease_node_after_bands() {
    let (mesh, index, cutter, params, planner) = trench_claims_fixture();
    let claims_cfg = ClaimsConfig {
        territory_stock: None,
        // Build-list item 3's default arm — not under test here.
        crease_reference: CreaseReference::SelfProbe,
        rest_field_params: RestFieldParams {
            cell_mm: 0.5,
            min_valley_depth: 0.05,
            // Force pencil routing over clearing (mirrors
            // `rest_field::tests::v_valley_yields_one_centerline`) so
            // the detected ridge survives as a centerline to claim.
            num_offset_passes_cap: 64,
            ..RestFieldParams::default()
        },
        min_rest_depth_mm: 0.02,
        // S4 not under test here — off, the safe/default choice.
        territory_clip: false,
        crease_hookup_mm: 5.0,
    };
    let never_cancel = || false;

    let (tp, _anns, report) = unified_finish_toolpath_with_cancel(
        &mesh,
        &index,
        &cutter,
        5.0,
        -5.0,
        &params,
        &planner,
        None,
        None,
        Some(&claims_cfg),
        None,
        &never_cancel,
    )
    .unwrap();

    let claims = report.claims.expect("claims pipeline must have run");
    assert!(
        claims.crease_path_count > 0,
        "expected claimed crease cut paths, got {claims:?}"
    );
    assert!(claims.crease_path_length_mm > 0.0);
    // S1 additive claims: the pencil node cuts, but corridors are NOT
    // carved out of the bands (width-honest carving is future work),
    // so decompose sees no creases.
    assert_eq!(report.decompose.claimed_creases, 0);
    // No territory stock: territory stays full (design doc §2.1
    // step 4), so no cells are measured skippable.
    assert_eq!(claims.territory_mode, ClaimTerritoryMode::Full);

    let crease_entry = report
        .region_table
        .last()
        .expect("region table must be non-empty");
    assert_eq!(crease_entry.kind, RegionKind::Crease);
    assert_eq!(
        crease_entry.move_range.end,
        tp.moves.len(),
        "crease node must be the tail of the stitched toolpath"
    );
    for entry in &report.region_table[..report.region_table.len() - 1] {
        assert!(
            matches!(entry.kind, RegionKind::Band(_)),
            "every non-trailing entry must be a band, got {entry:?}"
        );
        assert!(
            entry.move_range.end <= crease_entry.move_range.start,
            "crease node must come after every routed band"
        );
    }
}

#[test]
fn region_table_ranges_are_within_bounds_and_non_overlapping() {
    let (mesh, index, cutter, params, planner) = trench_claims_fixture();
    let claims_cfg = ClaimsConfig {
        territory_stock: None,
        // Not under test here (region-table range invariants); the
        // default arm is the safe choice.
        crease_reference: CreaseReference::SelfProbe,
        rest_field_params: RestFieldParams {
            cell_mm: 0.5,
            min_valley_depth: 0.05,
            // Force pencil routing over clearing (mirrors
            // `rest_field::tests::v_valley_yields_one_centerline`) so
            // the detected ridge survives as a centerline to claim.
            num_offset_passes_cap: 64,
            ..RestFieldParams::default()
        },
        min_rest_depth_mm: 0.02,
        // S4 not under test here — off, the safe/default choice.
        territory_clip: false,
        crease_hookup_mm: 5.0,
    };
    let never_cancel = || false;

    let (tp, _anns, report) = unified_finish_toolpath_with_cancel(
        &mesh,
        &index,
        &cutter,
        5.0,
        -5.0,
        &params,
        &planner,
        None,
        None,
        Some(&claims_cfg),
        None,
        &never_cancel,
    )
    .unwrap();

    assert!(
        !report.region_table.is_empty(),
        "expected at least the crease node in the region table"
    );
    let mut prev_end = 0usize;
    for entry in &report.region_table {
        assert!(
            entry.move_range.start <= entry.move_range.end,
            "malformed range {:?}",
            entry.move_range
        );
        assert!(
            entry.move_range.start >= prev_end,
            "region ranges must not overlap: {:?} starts before the \
                 previous entry ended at {prev_end}",
            entry.move_range
        );
        assert!(
            entry.move_range.end <= tp.moves.len(),
            "region range {:?} escaped the {}-move toolpath",
            entry.move_range,
            tp.moves.len()
        );
        prev_end = entry.move_range.end;
    }
}

// ── Build-list item 3: stock-referenced crease claims ───────────────

/// [`CreaseReference::MachinedStock`] sentry (process-proof build-list
/// item 3): the orchestrator must actually swap the crease detector's
/// reference to the supplied stock (not silently keep using the
/// self-probe), and must fall back to the self-probe cleanly — no
/// panic, a `ClaimsReport` still comes back — when the caller opts
/// into `MachinedStock` without a `territory_stock` in scope.
///
/// Reuses `trench_claims_fixture` with a fresh, UNCUT solid-brick
/// stock (`TriDexelStock::from_bounds`, top = the mesh's own bbox
/// ceiling) rather than a hand-built stock matched to the mesh exactly
/// — sidesteps needing byte-exact agreement with the detector's own
/// probe-drop math. The brick's flat top sits at the SAME height the tip
/// cutter itself contacts outside the trench, so this assigns a
/// *smaller* rest magnitude at the trench than the self-probe (whose
/// reference reaches the true floor) — assertions therefore stay off
/// magnitude comparisons between arms (fixture-dependent, not a
/// general property of `MachinedStock`) and instead check the
/// structural contract: the swap actually ran, and the fallback
/// actually fell back.
#[test]
fn crease_reference_machined_stock_runs_and_falls_back_without_stock() {
    let (mesh, index, cutter, params, planner) = trench_claims_fixture();
    let never_cancel = || false;
    let rf_params = || RestFieldParams {
        cell_mm: 0.5,
        min_valley_depth: 0.05,
        // Force pencil routing over clearing, same as the other claims
        // tests, so the detected ridge survives as a centerline.
        num_offset_passes_cap: 64,
        ..RestFieldParams::default()
    };

    // Self-probe baseline (S1/S2 default arm) — establishes what "the
    // detector ran at all" looks like on this fixture.
    let self_probe_cfg = ClaimsConfig {
        territory_stock: None,
        crease_reference: CreaseReference::SelfProbe,
        rest_field_params: rf_params(),
        min_rest_depth_mm: 0.02,
        // S4 not under test here — off, the safe/default choice.
        territory_clip: false,
        crease_hookup_mm: 5.0,
    };
    let (_tp, _anns, report_self) = unified_finish_toolpath_with_cancel(
        &mesh,
        &index,
        &cutter,
        5.0,
        -5.0,
        &params,
        &planner,
        None,
        None,
        Some(&self_probe_cfg),
        None,
        &never_cancel,
    )
    .expect("self-probe crease detection must complete");
    let claims_self = report_self.claims.expect("claims pipeline must have run");

    // MachinedStock arm, stock present: must complete, must still read
    // `RestIslands` territory (driven by `territory_stock`, independent
    // of which reference fed the detector), and must actually have
    // claimed the trench crease under the STOCK reference (proves the
    // swap ran — a silently-ignored reference would still claim
    // nothing-changed output, which this mesh's single crease can't
    // distinguish from "swap didn't happen" on its own, so the real
    // proof is the fallback-vs-stock split below).
    let stock = crate::dexel_stock::TriDexelStock::from_bounds(&mesh.bbox, 1.0);
    let stock_cfg = ClaimsConfig {
        territory_stock: Some(&stock),
        crease_reference: CreaseReference::MachinedStock,
        rest_field_params: rf_params(),
        min_rest_depth_mm: 0.02,
        // S4 not under test here — off, the safe/default choice.
        territory_clip: false,
        crease_hookup_mm: 5.0,
    };
    let (_tp2, _anns2, report_stock) = unified_finish_toolpath_with_cancel(
        &mesh,
        &index,
        &cutter,
        5.0,
        -5.0,
        &params,
        &planner,
        None,
        None,
        Some(&stock_cfg),
        None,
        &never_cancel,
    )
    .expect("machined-stock crease detection must complete");
    let claims_stock = report_stock.claims.expect("claims pipeline must have run");
    assert_eq!(
        claims_stock.territory_mode,
        ClaimTerritoryMode::RestIslands,
        "a territory_stock in scope must read RestIslands territory mode \
             regardless of which arm fed the crease detector"
    );
    // Direct evidence the swap actually fed `detect_rest_valleys`: at
    // the trench center, the solid brick's flat top sits well above
    // where the tip cutter can descend into the (too-narrow-to-bridge)
    // Gaussian dip, so `stock_top - pencil_drop` must read a
    // comfortably-over-threshold rest value there — independent of
    // whatever the ridge-tracing pipeline does with it downstream.
    let rest_grid_stock = report_stock
        .rest_grid
        .as_ref()
        .expect("rest grid must be carried through");
    let trench_center_rest = rest_grid_index(rest_grid_stock, 15.0, 0.0)
        .and_then(|i| rest_grid_stock.rest.get(i))
        .copied()
        .expect("trench center must be a trusted grid cell");
    assert!(
        trench_center_rest.is_finite() && f64::from(trench_center_rest) > 0.05,
        "expected the stock-referenced field to read material at the \
             trench center, got {trench_center_rest}"
    );
    assert!(
        claims_stock.crease_path_count > 0,
        "expected the trench crease to still claim cut paths under the \
             stock-referenced detector, got {claims_stock:?}"
    );
    assert!(claims_stock.crease_path_length_mm > 0.0);

    // No-stock fallback: `MachinedStock` with `territory_stock: None`
    // has no stock to reference, so it must fall back to the
    // self-probe rather than error or panic — and the fallback must
    // reproduce the self-probe run byte-for-byte (same code path),
    // proving the fallback actually engaged rather than, say, silently
    // detecting nothing.
    let fallback_cfg = ClaimsConfig {
        territory_stock: None,
        crease_reference: CreaseReference::MachinedStock,
        rest_field_params: rf_params(),
        min_rest_depth_mm: 0.02,
        // S4 not under test here — off, the safe/default choice.
        territory_clip: false,
        crease_hookup_mm: 5.0,
    };
    let (_tp3, _anns3, report_fallback) = unified_finish_toolpath_with_cancel(
        &mesh,
        &index,
        &cutter,
        5.0,
        -5.0,
        &params,
        &planner,
        None,
        None,
        Some(&fallback_cfg),
        None,
        &never_cancel,
    )
    .expect("MachinedStock with no territory_stock must fall back cleanly, not error");
    let claims_fallback = report_fallback
        .claims
        .expect("claims pipeline must have run even on fallback");
    assert_eq!(
        claims_fallback.territory_mode,
        ClaimTerritoryMode::Full,
        "no territory_stock means territory stays Full even under MachinedStock"
    );
    assert_eq!(
        claims_fallback.crease_path_count, claims_self.crease_path_count,
        "the no-stock fallback must reproduce the self-probe run exactly"
    );
    assert!(
        (claims_fallback.crease_path_length_mm - claims_self.crease_path_length_mm).abs() < 1e-9,
        "fallback length {} must match self-probe length {}",
        claims_fallback.crease_path_length_mm,
        claims_self.crease_path_length_mm
    );
}

// ── S4 rest-territory confinement (mask-AND) ────────────────────────

/// (a) `territory_clip: false` must be a total no-op: even with a
/// territory stock in scope and a saturating `min_rest_depth_mm` (so
/// every covered cell would read measured-skippable if the mask-AND
/// ran), leaving `territory_clip` off must leave
/// `territory_masked_cells`/`territory_masked_area_mm2` at zero and
/// `post_territory_region_count` exactly the decompose count — the
/// mask-AND never ran.
#[test]
fn territory_clip_off_is_a_total_no_op() {
    let (mesh, index, cutter, params, planner) = trench_claims_fixture();
    let stock = crate::dexel_stock::TriDexelStock::from_bounds(&mesh.bbox, 1.0);
    let never_cancel = || false;

    let cfg = ClaimsConfig {
        territory_stock: Some(&stock),
        crease_reference: CreaseReference::MachinedStock,
        rest_field_params: RestFieldParams {
            cell_mm: 0.5,
            min_valley_depth: 0.05,
            num_offset_passes_cap: 64,
            ..RestFieldParams::default()
        },
        min_rest_depth_mm: 1.0e6,
        territory_clip: false,
        crease_hookup_mm: 5.0,
    };
    let (_tp, _anns, report) = unified_finish_toolpath_with_cancel(
        &mesh,
        &index,
        &cutter,
        5.0,
        -5.0,
        &params,
        &planner,
        None,
        None,
        Some(&cfg),
        None,
        &never_cancel,
    )
    .unwrap();
    let claims = report.claims.expect("claims pipeline must have run");
    assert_eq!(
        claims.territory_masked_cells, 0,
        "territory_clip: false must never mask coverage"
    );
    assert_eq!(claims.territory_masked_area_mm2, 0.0);
    assert_eq!(
        claims.post_territory_region_count, report.decompose.region_count,
        "with S4 off, post_territory_region_count is exactly the decompose count"
    );
}

/// (b) `territory_clip: true` WITHOUT a `territory_stock` is inert —
/// the per-cell rest keep-mask never exists, so there is nothing
/// to AND (`ClaimsConfig::territory_clip` doc: requires a
/// territory_stock). Completes normally under either crease
/// reference; `territory_masked_cells` stays 0.
#[test]
fn territory_clip_without_stock_is_inert() {
    let (mesh, index, cutter, params, planner) = trench_claims_fixture();
    let never_cancel = || false;

    let cfg = ClaimsConfig {
        territory_stock: None,
        crease_reference: CreaseReference::SelfProbe,
        rest_field_params: RestFieldParams {
            cell_mm: 0.5,
            min_valley_depth: 0.05,
            num_offset_passes_cap: 64,
            ..RestFieldParams::default()
        },
        min_rest_depth_mm: 0.02,
        territory_clip: true,
        crease_hookup_mm: 5.0,
    };
    let (_tp, _anns, report) = unified_finish_toolpath_with_cancel(
        &mesh,
        &index,
        &cutter,
        5.0,
        -5.0,
        &params,
        &planner,
        None,
        None,
        Some(&cfg),
        None,
        &never_cancel,
    )
    .expect("territory_clip without a territory_stock must complete, not error");
    let claims = report.claims.expect("claims pipeline must have run");
    assert_eq!(
        claims.territory_masked_cells, 0,
        "no territory_stock -> no rest verdict grid -> nothing to AND"
    );
    assert_eq!(claims.territory_masked_area_mm2, 0.0);
}

/// (c) `territory_clip: true` under `CreaseReference::MachinedStock`
/// with a real (not saturating) rest-depth threshold: the same uncut
/// solid-brick stock as the build-list-3 test with `min_rest_depth_mm`
/// at a real value (0.05) — the per-cell verdict keeps only cells
/// where the brick top truly sits ≥0.05mm above the pencil drop (the
/// trench, not the flat surface flanking it), so the mask-AND confines
/// coverage to a genuinely smaller-than-the-band rest island rather
/// than "the whole stock". Compares against the SAME config with
/// `territory_clip: false` to prove the confinement actually shrank
/// the emitted toolpath, not just changed telemetry.
#[test]
fn territory_clip_confines_bands_to_rest_islands_and_shrinks_the_toolpath() {
    let (mesh, index, cutter, params, planner) = trench_claims_fixture();
    let stock = crate::dexel_stock::TriDexelStock::from_bounds(&mesh.bbox, 1.0);
    let never_cancel = || false;
    let rf_params = || RestFieldParams {
        cell_mm: 0.5,
        min_valley_depth: 0.05,
        num_offset_passes_cap: 64,
        ..RestFieldParams::default()
    };

    let unclipped_cfg = ClaimsConfig {
        territory_stock: Some(&stock),
        crease_reference: CreaseReference::MachinedStock,
        rest_field_params: rf_params(),
        min_rest_depth_mm: 0.05,
        territory_clip: false,
        crease_hookup_mm: 5.0,
    };
    let (tp_unclipped, _anns, report_unclipped) = unified_finish_toolpath_with_cancel(
        &mesh,
        &index,
        &cutter,
        5.0,
        -5.0,
        &params,
        &planner,
        None,
        None,
        Some(&unclipped_cfg),
        None,
        &never_cancel,
    )
    .unwrap();
    let claims_unclipped = report_unclipped
        .claims
        .expect("claims pipeline must have run");
    assert_eq!(
        claims_unclipped.territory_masked_cells, 0,
        "the unconfined baseline run must not itself mask anything"
    );
    let rest_regions = report_unclipped
        .rest_regions
        .as_ref()
        .expect("rest_regions must be carried through under MachinedStock");
    assert!(
        !rest_regions.is_empty(),
        "expected the detector to find a non-empty rest island on this \
             fixture — the fixture is meaningless for S4 otherwise"
    );

    let clipped_cfg = ClaimsConfig {
        territory_stock: Some(&stock),
        crease_reference: CreaseReference::MachinedStock,
        rest_field_params: rf_params(),
        min_rest_depth_mm: 0.05,
        territory_clip: true,
        crease_hookup_mm: 5.0,
    };
    let (tp_clipped, _anns2, report_clipped) = unified_finish_toolpath_with_cancel(
        &mesh,
        &index,
        &cutter,
        5.0,
        -5.0,
        &params,
        &planner,
        None,
        None,
        Some(&clipped_cfg),
        None,
        &never_cancel,
    )
    .unwrap();
    let claims_clipped = report_clipped
        .claims
        .expect("claims pipeline must have run");

    assert!(
        claims_clipped.territory_masked_cells >= 1,
        "expected the S4 mask-AND to remove at least one \
             measured-skippable covered cell, got {claims_clipped:?}"
    );
    assert!(
        tp_clipped.moves.len() < tp_unclipped.moves.len(),
        "confining bands to rest islands must shrink the emitted \
             toolpath: confined={} unconfined={}",
        tp_clipped.moves.len(),
        tp_unclipped.moves.len()
    );
}
