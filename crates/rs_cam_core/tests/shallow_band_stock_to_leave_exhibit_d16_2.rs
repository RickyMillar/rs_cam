//! **EXHIBIT — D-16.2: `UnifiedFinish`'s Shallow band silently ignores
//! `stock_to_leave`.**
//!
//! ## READ THIS FIRST — what form this file is in
//!
//! **These are NOT a contract. Every assertion below pins the CURRENT,
//! DEFECTIVE behaviour, and is written so that it FAILS THE MOMENT THE FIX
//! LANDS.** A green run of this file means the defect is still present. A
//! red run of tests 2 or 4 means somebody honoured the dial and the
//! assertion needs INVERTING, in place, exactly as its own doc comment
//! directs — not deleting.
//!
//! This is the same red-first idiom wave W2 used in
//! `arcfit_intent_boundary_f1.rs`: deliberately NOT `#[ignore]`d, because an
//! ignored characterisation rots in silence and nobody finds out the day the
//! behaviour it described stopped being true.
//!
//! Two of the four tests are CONTROLS and are green both before and after
//! the fix (`d16_2_fixture_cuts_a_shallow_band_and_only_a_shallow_band`,
//! `control_mid_steep_band_honours_stock_to_leave_exactly`). They exist so
//! that a "no shift measured" reading can never be explained away as "the
//! measurement was broken" or "nothing was generated".
//!
//! ## The defect, as filed
//!
//! `unified_finish::unified_finish_toolpath_with_cancel` dispatches each
//! planned region on one `match region.band`:
//!
//! | band | generator | takes `stock_to_leave`? |
//! |---|---|---|
//! | `VerySteep` | `waterline_toolpath_with_cancel` | **NO** — `WaterlineParams` has no such field (see test 4) |
//! | `MidSteep` | `scallop_toolpath_structured_annotated_with_cancel` | yes — `ScallopParams::stock_to_leave` |
//! | `Shallow` | `raster_toolpath_from_grid` | **NO** — six params, none of them a leave allowance |
//!
//! The Shallow arm builds a raw drop-cutter grid with
//! `batch_drop_cutter_with_cancel(mesh, index, cutter, raster_stepover,
//! 0.0, effective_min_z, …)` — the `0.0` there is `direction_deg`, NOT a
//! leave allowance (`dropcutter.rs`'s `batch_drop_cutter_with_cancel`
//! signature) — and hands the grid straight to `raster_toolpath_from_grid`
//! (`toolpath.rs`), which performs no Z arithmetic at all: every emitted
//! cut target is `grid.get(row, col).position()` verbatim. So two
//! `UnifiedFinish` ops differing ONLY in `stock_to_leave` emit a
//! byte-identical shallow-band toolpath. The dial reports no error and does
//! nothing.
//!
//! The honouring paths all share ONE convention — a pure `+Z` shift on the
//! drop-cutter contact point, `cl.z + stock_to_leave`: `scallop.rs`'s
//! `ring_to_3d` (including its off-mesh sentinel), `pencil.rs`'s emit path,
//! and `surface_link.rs`'s `build_surface_link`. A fix for this exhibit is
//! therefore expected to be that same shift, which is why test 2's
//! inversion is a one-line edit.
//!
//! ## Why `intra_region_hookup_mm` is PINNED TO ZERO here
//!
//! The dial is not TOTALLY inert on the shallow band at shipped defaults.
//! `UnifiedFinishParams::intra_region_hookup_mm` ships at **6.0**, and the
//! relink it enables passes `params.stock_to_leave` through to
//! `surface_link::build_surface_link`, which DOES apply it. So at defaults
//! the dial is inert on CUTS and live on LINKS — a mixed population that
//! would make the measurement below un-attributable. Every arm in this file
//! pins `intra_region_hookup_mm: 0.0` so the exhibit is a statement about
//! the raster band and nothing else. For the same reason every arm passes
//! `machining_boundary`, `link_kinematics`, `claims` and `debug` as `None`:
//! those are the other three call sites that read `params.stock_to_leave`
//! (the router's link costing and the claims/crease pencil emission), and
//! with them out of scope the MidSteep scallop arm is the ONLY consumer
//! left in the whole function.
//!
//! ## Fixtures
//!
//! * Tests 1–2 use the smooth double bump copied verbatim from
//!   `zero_removal_rest_pass_a4.rs` (whose header already records this
//!   defect as the reason `stock_to_leave` was not usable as its variable).
//!   Its maximum slope is `atan(0.9 · π/20)` ≈ **8.05°**, so against the
//!   shipped `steep_threshold_deg = 45.0` it is 100% Shallow — test 1
//!   proves that rather than assuming it.
//! * Tests 3–4 use `make_test_hemisphere(20.0, 16)`, the fixture
//!   `capability_link_moves_safety.rs`'s
//!   `unified_finish_node_barriers_allow_intra_region_reorder_and_pin_depth`
//!   already asserts produces at least one **VerySteep** node at these exact
//!   dials. A hemisphere splits cleanly into all three bands by radius
//!   (shallow `r < R/√2`, mid-steep `R/√2 ≤ r < R·sin 75°`, very-steep
//!   above), so one fixture carries both the mid-steep control and the
//!   very-steep twin with a band population that is known non-empty rather
//!   than hoped to be.
//!
//! No production code is touched by this file. It is an instrument.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::meshes::height_field;
use rs_cam_core::finish_planner::{FinishBand, FinishPlannerParams};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh, make_test_hemisphere};
use rs_cam_core::tool::{BallEndmill, MillingCutter};
use rs_cam_core::toolpath::{MoveIntent, Toolpath};
use rs_cam_core::unified_finish::{
    UnifiedFinishParams, UnifiedFinishReport, unified_finish_toolpath_with_cancel,
};

// ── Constants ────────────────────────────────────────────────────────────

/// The one variable. 0.5 mm is a real roughing-to-finish allowance, and it
/// is three orders of magnitude above anything floating-point noise in a
/// drop-cutter contact Z could manufacture.
const STOCK_TO_LEAVE_MM: f64 = 0.5;

/// Ø3 ball nose. `UnifiedFinish` enforces a ball tip at the op-adapter layer
/// (`compute::execute`), so a flat end mill is not a legal fixture here; this
/// matches `zero_removal_rest_pass_a4.rs`'s `ball_tool_config(3.0)`.
const TOOL_DIAMETER_MM: f64 = 3.0;

/// Cutting length of the fixture tool (mm) — matches
/// `capability_link_moves_safety.rs`'s `BallEndmill::new(3.0, 25.0)`.
const TOOL_CUTTING_LENGTH_MM: f64 = 25.0;

/// Half-extent (mm) of the bumpy fixture surface. From
/// `zero_removal_rest_pass_a4.rs`.
const HALF: f64 = 20.0;

/// Stock height (mm) of the bumpy fixture. The surface lives in
/// `[STOCK_Z - RELIEF, STOCK_Z]`. From `zero_removal_rest_pass_a4.rs`.
const STOCK_Z: f64 = 6.0;

/// Peak-to-trough of the bumpy fixture surface (mm). From
/// `zero_removal_rest_pass_a4.rs`.
const RELIEF: f64 = 2.0;

/// Hemisphere radius (mm) for the band fixtures — the value
/// `capability_link_moves_safety.rs` runs UnifiedFinish against.
const HEMI_RADIUS_MM: f64 = 20.0;

/// Path tolerance for the hemisphere arms. Pinned to the value the proven
/// capability-test configuration uses, because `tolerance` floors the
/// classification cell size
/// (`FinishResolutionPolicy::cusp_quarter` = `max(cusp_radius/4,
/// tolerance)`) and therefore decides which cells land in which band. This
/// is a fixture pin, not a quality choice.
const HEMI_TOLERANCE_MM: f64 = 0.5;

/// Path tolerance for the bumpy arms — `UnifiedFinishParams::default()`'s
/// own value, so the shallow exhibit runs at shipped dials.
const BUMPY_TOLERANCE_MM: f64 = 0.05;

// ── Fixture ──────────────────────────────────────────────────────────────

/// Everything an arm needs EXCEPT the variable. Held in one struct so that
/// [`run`] takes exactly two things — the fixture and `stock_to_leave` —
/// and a reader can see at a glance that the two arms of every comparison
/// below differ in one field and no other.
struct Fixture {
    mesh: TriangleMesh,
    index: SpatialIndex,
    tolerance: f64,
    top_z: f64,
    bottom_z: f64,
}

/// The same smooth double bump `zero_removal_rest_pass_a4.rs` uses, copied
/// verbatim (that file's copy is private to it, and it is not this wave's to
/// edit). Occupies the TOP of the stock so the finish pass has real work.
fn bumpy_surface() -> TriangleMesh {
    height_field(HALF, 0.5, |x, y| {
        let sx = (x / HALF * std::f64::consts::PI).cos();
        let sy = (y / HALF * std::f64::consts::PI).cos();
        STOCK_Z - RELIEF * 0.5 * (1.0 - sx * sy * 0.9)
    })
}

/// 100%-Shallow fixture (test 1 proves the "100%" rather than assuming it).
fn bumpy_fixture() -> Fixture {
    let mesh = bumpy_surface();
    let index = SpatialIndex::build_auto(&mesh);
    Fixture {
        mesh,
        index,
        tolerance: BUMPY_TOLERANCE_MM,
        // The surface spans [4.1, 5.9]; a millimetre of clearance either
        // side so no height clamp can decide anything here. (Only the
        // VerySteep arm reads these at all — there is no such band on this
        // fixture — but leaving them wrong would be a trap for a later
        // editor.)
        top_z: STOCK_Z + 1.0,
        bottom_z: STOCK_Z - RELIEF - 1.0,
    }
}

/// All-three-bands fixture. Index cell size and Z bounds are pinned to
/// `capability_link_moves_safety.rs`'s proven configuration.
fn hemisphere_fixture() -> Fixture {
    let mesh = make_test_hemisphere(HEMI_RADIUS_MM, 16);
    let index = SpatialIndex::build(&mesh, 12.0);
    Fixture {
        mesh,
        index,
        tolerance: HEMI_TOLERANCE_MM,
        top_z: 25.0,
        bottom_z: -1.0,
    }
}

// ── The arm ──────────────────────────────────────────────────────────────

/// One generation's output.
struct Arm {
    toolpath: Toolpath,
    report: UnifiedFinishReport,
}

/// Run the shipped orchestrator once. **`stock_to_leave` is the ONLY thing
/// any caller in this file varies** — everything else comes from the
/// fixture or from `UnifiedFinishParams::default()`.
///
/// `intra_region_hookup_mm: 0.0` is pinned here rather than inherited: see
/// the module header. At its shipped 6.0 default the relink pass DOES honour
/// `stock_to_leave` (via `surface_link::build_surface_link`), which would
/// mix a honouring population into a measurement about the raster band.
fn run(fixture: &Fixture, stock_to_leave: f64) -> Arm {
    let cutter = BallEndmill::new(TOOL_DIAMETER_MM, TOOL_CUTTING_LENGTH_MM);
    let params = UnifiedFinishParams {
        // ── THE VARIABLE ───────────────────────────────────────────────
        stock_to_leave,
        // ── everything below is held identical across both arms ────────
        tolerance: fixture.tolerance,
        intra_region_hookup_mm: 0.0,
        ..UnifiedFinishParams::default()
    };
    // `for_tool`'s parameter is a CUSP radius, never `radius()` — the
    // finish-planner dials are all feature-scale (`finish_planner.rs`'s own
    // doc, and the 2026-07-29 radius programme). Inert on this ball fixture
    // where the two coincide; a wrong oracle the moment it is tapered.
    let planner = FinishPlannerParams::for_tool(cutter.cusp_radius_mm());
    let never_cancel = || false;

    let (toolpath, _anns, report) = unified_finish_toolpath_with_cancel(
        &fixture.mesh,
        &fixture.index,
        &cutter,
        fixture.top_z,
        fixture.bottom_z,
        &params,
        &planner,
        /* machining_boundary */ None,
        /* link_kinematics */ None,
        /* claims */ None,
        /* debug */ None,
        &never_cancel,
    )
    .expect("uncancelled generation");

    Arm { toolpath, report }
}

// ── Measurement ──────────────────────────────────────────────────────────

/// Cut Z values inside `range`, selected BY MOVE INTENT.
///
/// The intent set mirrors the contract `pencil.rs`'s own
/// `stock_to_leave` test pins — `FinishingCut | EntryPlunge` — so that this
/// exhibit and the honouring path it is contrasted against are measuring
/// the same population. Retracts and linking rapids sit at `safe_z` and are
/// not cut positions; including them would dilute any shift by a constant
/// that never moves.
fn cut_zs_in(tp: &Toolpath, range: std::ops::Range<usize>) -> Vec<f64> {
    let Some(slice) = tp.moves.get(range) else {
        return Vec::new();
    };
    slice
        .iter()
        .filter(|m| matches!(m.intent, MoveIntent::FinishingCut | MoveIntent::EntryPlunge))
        .map(|m| m.target.z)
        .collect()
}

/// Every cut Z in the whole toolpath, in emission order.
fn cut_zs(tp: &Toolpath) -> Vec<f64> {
    cut_zs_in(tp, 0..tp.moves.len())
}

/// Cut Z values belonging to `band`, selected **BY REGION SPAN** — the
/// `move_range` of every `RegionTableEntry` whose `kind.band()` is `band`.
///
/// Never by a Z heuristic and never by a label literal. A Z heuristic would
/// be circular here (the thing under measurement IS a Z shift), and a label
/// literal would re-introduce exactly the failure mode
/// `arcfit_intent_boundary_f1.rs` documents: selecting a population by a
/// string that a downstream transform is free to rewrite.
fn band_cut_zs(arm: &Arm, band: FinishBand) -> Vec<f64> {
    let mut zs = Vec::new();
    for entry in &arm.report.region_table {
        if entry.kind.band() == Some(band) {
            zs.extend(cut_zs_in(&arm.toolpath, entry.move_range.clone()));
        }
    }
    zs
}

/// Largest `|b − a|` over the paired sequences. Callers must have already
/// established that the two have equal length.
fn max_abs_shift(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (y - x).abs())
        .fold(0.0_f64, f64::max)
}

/// Largest departure of the measured shift from `expected`.
fn max_shift_error(a: &[f64], b: &[f64], expected: f64) -> f64 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| ((y - x) - expected).abs())
        .fold(0.0_f64, f64::max)
}

// ── Test 1: non-vacuity ──────────────────────────────────────────────────

/// **Control — the bumpy fixture cuts a shallow band, and ONLY a shallow
/// band.**
///
/// GREEN BEFORE AND AFTER THE FIX. Without it, test 2's `max_shift == 0`
/// reading has a second explanation — "nothing was generated" — and a
/// zero measured over an empty population is not a measurement.
///
/// It also pins the band SPLIT, not just non-emptiness: if a future change
/// to classification let even one mid-steep region onto this fixture, test 2
/// would start averaging a honouring population (scallop) into a statement
/// about the non-honouring one (raster), and would go quietly wrong rather
/// than loudly red.
#[test]
fn d16_2_fixture_cuts_a_shallow_band_and_only_a_shallow_band() {
    let fixture = bumpy_fixture();
    let arm = run(&fixture, 0.0);

    assert!(
        !arm.toolpath.moves.is_empty(),
        "the bumpy fixture must generate a toolpath at all — every other \
         assertion in this file is vacuous otherwise"
    );
    assert!(
        arm.report.shallow.move_count > 0,
        "the SHALLOW band must emit moves (max slope ≈ 8.05° against a \
         shipped steep_threshold_deg of 45.0, so this fixture is 100% \
         shallow by construction); got shallow {:?}, mid_steep {:?}, \
         very_steep {:?}",
        arm.report.shallow,
        arm.report.mid_steep,
        arm.report.very_steep
    );
    assert_eq!(
        arm.report.mid_steep.move_count, 0,
        "no MID-STEEP moves may appear on this fixture: the mid-steep band \
         runs scallop, which DOES honour stock_to_leave, so a single \
         mid-steep region here would silently turn the D-16.2 exhibit into \
         a mixed-population measurement; got {:?}",
        arm.report.mid_steep
    );
    assert_eq!(
        arm.report.very_steep.move_count, 0,
        "no VERY-STEEP moves may appear on this fixture — same reason, \
         except that waterline does not honour the dial either, so this one \
         would flatter the exhibit rather than break it; got {:?}",
        arm.report.very_steep
    );
    assert!(
        !cut_zs(&arm.toolpath).is_empty(),
        "the toolpath must contain moves tagged FinishingCut/EntryPlunge — \
         the exhibit measures cut Z by INTENT, so an emission that stopped \
         tagging cuts would empty the population without emptying the \
         toolpath ({} moves present)",
        arm.toolpath.moves.len()
    );
}

// ── Test 2: THE EXHIBIT ──────────────────────────────────────────────────

/// **THE EXHIBIT — D-16.2: the Shallow band emits identical cut Z at
/// `stock_to_leave = 0.0` and `stock_to_leave = 0.5`.**
///
/// **THIS TEST PINS A DEFECT. IT IS EXPECTED TO FAIL WHEN THE FIX LANDS.**
///
/// Two arms, one fixture, one variable. Everything else — tool, planner,
/// tolerance, stepover, Z bounds, `intra_region_hookup_mm: 0.0` — is held
/// identical by [`run`], which takes `stock_to_leave` as its only
/// per-arm argument.
///
/// Root cause: `unified_finish.rs`'s `FinishBand::Shallow` arm calls
/// `raster_toolpath_from_grid(grid, feed_rate, plunge_rate, safe_z, min_z,
/// boundary_regions)` — six parameters, none of them a leave allowance —
/// on a raw `batch_drop_cutter_with_cancel` grid. That function does no Z
/// arithmetic; each emitted target is `grid.get(row, col).position()`
/// verbatim. There is nowhere in that call chain for the dial to act.
///
/// **To invert: replace `max_shift < 1e-9` with
/// `(max_shift - STOCK_TO_LEAVE_MM).abs() < 1e-9`. Nothing else in this
/// test changes.**
#[test]
fn exhibit_shallow_band_ignores_stock_to_leave_entirely() {
    let fixture = bumpy_fixture();

    let arm0 = run(&fixture, 0.0);
    let arm_half = run(&fixture, STOCK_TO_LEAVE_MM);

    // The exhibit must stay a statement about the raster band alone.
    assert_eq!(
        (
            arm0.report.mid_steep.move_count,
            arm0.report.very_steep.move_count
        ),
        (0, 0),
        "fixture precondition (see the sibling control test): this exhibit \
         is only about the shallow band, so no other band may contribute \
         moves; got mid_steep {:?}, very_steep {:?}",
        arm0.report.mid_steep,
        arm0.report.very_steep
    );

    let zs0 = cut_zs(&arm0.toolpath);
    let zs_half = cut_zs(&arm_half.toolpath);

    assert!(
        !zs0.is_empty(),
        "no cut positions were emitted — the exhibit would be measuring an \
         empty population, which is not evidence of anything"
    );
    assert_eq!(
        zs0.len(),
        zs_half.len(),
        "stock_to_leave must never change which points are emitted as cuts"
    );

    let max_shift = max_abs_shift(&zs0, &zs_half);

    assert!(
        max_shift < 1e-9,
        "INVERT THIS LINE WHEN THE FIX LANDS.\n\
         \n\
         D-16.2 exhibit: UnifiedFinish's SHALLOW (raster) band ignores \
         `stock_to_leave`. Two ops differing ONLY in that dial (0.0 vs \
         {STOCK_TO_LEAVE_MM} mm) emitted {n} cut positions each whose Z \
         differed by at most {max_shift:.12} mm — i.e. not at all.\n\
         \n\
         Root cause: `unified_finish.rs:1919` (the `FinishBand::Shallow` \
         arm) calls `toolpath.rs:607` `raster_toolpath_from_grid(grid, \
         feed_rate, plunge_rate, safe_z, min_z, boundary_regions)`. That \
         signature has no stock-to-leave parameter and the body performs no \
         Z arithmetic — every emitted target is \
         `grid.get(row, col).position()` verbatim off a raw \
         `batch_drop_cutter_with_cancel` grid (whose `0.0` argument is \
         `direction_deg`, not a leave allowance). The dial reports no error \
         and does nothing.\n\
         \n\
         A FAILURE HERE IS GOOD NEWS: it means the shallow band now shifts \
         its cut Z. Expected shift is exactly +{STOCK_TO_LEAVE_MM} mm, the \
         same pure `cl.z + stock_to_leave` convention `scallop.rs`'s \
         `ring_to_3d`, `pencil.rs` and `surface_link.rs` already use. \
         Replace `max_shift < 1e-9` with \
         `(max_shift - STOCK_TO_LEAVE_MM).abs() < 1e-9` and change nothing \
         else.",
        n = zs0.len(),
    );
}

// ── Test 3: the MidSteep control ─────────────────────────────────────────

/// **Control — the MID-STEEP band shifts every cut Z by exactly
/// `stock_to_leave`.**
///
/// GREEN BEFORE AND AFTER THE FIX, and it carries two jobs.
///
/// 1. **It proves the measurement works.** Test 2's "no shift" is only
///    evidence if the identical instrument — same two-arm helper, same
///    intent filter, same `max`-over-pairs arithmetic — registers a shift
///    where one genuinely exists.
/// 2. **It guards the band that is already correct.** The mid-steep arm
///    passes `params.stock_to_leave` into `ScallopParams`
///    (`unified_finish.rs`, the `FinishBand::MidSteep` arm), and
///    `scallop::ring_to_3d` applies it as `cl.z + stock_to_leave` on every
///    lifted ring point, off-mesh sentinel included. A fix for the shallow
///    band must not disturb that.
///
/// The mid-steep population is selected BY REGION SPAN
/// (`RegionTableEntry::move_range` filtered on
/// `kind.band() == Some(FinishBand::MidSteep)`), never by a Z heuristic and
/// never by a label literal. The non-empty assert comes FIRST and on
/// purpose: a control that passes over an empty population is worse than no
/// control, because it reports confidence it has not earned.
#[test]
fn control_mid_steep_band_honours_stock_to_leave_exactly() {
    let fixture = hemisphere_fixture();

    let arm0 = run(&fixture, 0.0);
    let arm_half = run(&fixture, STOCK_TO_LEAVE_MM);

    // `FinishPlannerParams::for_tool`'s own absorption formula
    // (`(2·cusp_radius)² · 4`), restated here so the vacuity message can
    // name the bar the band had to clear rather than asserting into the dark.
    let cusp_r = BallEndmill::new(TOOL_DIAMETER_MM, TOOL_CUTTING_LENGTH_MM).cusp_radius_mm();
    let min_region_area_mm2 = (2.0 * cusp_r).powi(2) * 4.0;

    assert!(
        arm0.report.mid_steep.move_count > 0,
        "THE CONTROL IS VACUOUS: the hemisphere fixture emitted NO \
         mid-steep moves, so this test would 'pass' without measuring \
         anything. A hemisphere of radius {HEMI_RADIUS_MM} splits by radius \
         — mid-steep is the annulus between r = R/√2 ({:.2} mm) and \
         r = R·sin 75° ({:.2} mm), roughly 542 mm² against a \
         min_region_area_mm2 of {:.1} — so an empty band means \
         classification or region conditioning changed, NOT that the \
         control is unnecessary. Fix the fixture, do not delete the \
         assertion. Report: shallow {:?}, mid_steep {:?}, very_steep {:?}",
        HEMI_RADIUS_MM / std::f64::consts::SQRT_2,
        HEMI_RADIUS_MM * 75.0_f64.to_radians().sin(),
        min_region_area_mm2,
        arm0.report.shallow,
        arm0.report.mid_steep,
        arm0.report.very_steep,
    );

    let zs0 = band_cut_zs(&arm0, FinishBand::MidSteep);
    let zs_half = band_cut_zs(&arm_half, FinishBand::MidSteep);

    assert!(
        !zs0.is_empty(),
        "THE CONTROL IS VACUOUS: the mid-steep region spans exist \
         ({} moves reported) but contain no moves tagged \
         FinishingCut/EntryPlunge, so the by-intent selection came back \
         empty. Investigate the intent tagging or the span ranges — do not \
         weaken this assertion.",
        arm0.report.mid_steep.move_count,
    );
    assert_eq!(
        zs0.len(),
        zs_half.len(),
        "stock_to_leave must never change which points are emitted as cuts \
         (mid-steep band, selected by region span)"
    );

    let worst = max_shift_error(&zs0, &zs_half, STOCK_TO_LEAVE_MM);
    assert!(
        worst < 1e-9,
        "the MID-STEEP band must shift EVERY cut Z by exactly \
         +{STOCK_TO_LEAVE_MM} mm across {n} positions; worst departure from \
         that was {worst:.12} mm. This is the already-correct band \
         (`ScallopParams::stock_to_leave` → `scallop::ring_to_3d`'s \
         `cl.z + stock_to_leave`). A failure here means either the \
         instrument this file measures D-16.2 with is broken — in which \
         case test 2's reading proves nothing — or a fix for the shallow \
         band disturbed the band that was already right.",
        n = zs0.len(),
    );
}

// ── Test 4: the twin, filed SEPARATELY ───────────────────────────────────

/// **EXHIBIT (SEPARATE DEFECT) — the VERY-STEEP (waterline) band drops
/// `stock_to_leave` too, and for a different reason.**
///
/// **THIS TEST PINS A DEFECT. IT IS EXPECTED TO FAIL WHEN THAT DEFECT IS
/// FIXED.**
///
/// **This is NOT D-16.2 as filed.** D-16.2 is about the Shallow/raster
/// band, where the dial exists on both sides of the call and simply is not
/// threaded. Here the dial has nowhere to go: `waterline::WaterlineParams`
/// has exactly four fields — `sampling`, `feed_rate`, `plunge_rate`,
/// `safe_z` — and no `stock_to_leave` at all, so
/// `unified_finish.rs`'s `FinishBand::VerySteep` arm could not pass one if
/// it wanted to. The standalone waterline operation has the same gap, which
/// is why `UnifiedFinishParams::stock_to_leave`'s own doc comment already
/// says "scallop path only (raster and waterline don't take one today;
/// parity with the standalone ops)".
///
/// **Inverting this one is a wider change than test 2's.** Test 2 becomes
/// true the moment the shallow arm shifts its grid Z. This one needs a new
/// field on `WaterlineParams`, threaded through
/// `waterline_toolpath_with_cancel` to wherever the contour CL points are
/// lifted, plus a decision about whether the Z LEVELS of the ladder move
/// with the allowance or only the contours computed at each level — a
/// question the raster band does not have to answer. File and fix it
/// separately; do not fold it into D-16.2 just because the two exhibits
/// share a file.
#[test]
fn exhibit_very_steep_band_ignores_stock_to_leave_separate_defect() {
    let fixture = hemisphere_fixture();

    let arm0 = run(&fixture, 0.0);
    let arm_half = run(&fixture, STOCK_TO_LEAVE_MM);

    assert!(
        arm0.report.very_steep.move_count > 0,
        "THE EXHIBIT IS VACUOUS: the hemisphere emitted NO very-steep \
         moves, so 'no shift' below would be a statement about an empty \
         set. This fixture is the one \
         `capability_link_moves_safety::\
         unified_finish_node_barriers_allow_intra_region_reorder_and_pin_depth` \
         asserts produces at least one VerySteep node at these same dials \
         (radius {HEMI_RADIUS_MM}, 16 divisions, tolerance \
         {HEMI_TOLERANCE_MM}, waterline_threshold_deg 75.0) — so an empty \
         band means classification moved, and BOTH tests should be \
         investigated together. Report: shallow {:?}, mid_steep {:?}, \
         very_steep {:?}",
        arm0.report.shallow,
        arm0.report.mid_steep,
        arm0.report.very_steep,
    );

    let zs0 = band_cut_zs(&arm0, FinishBand::VerySteep);
    let zs_half = band_cut_zs(&arm_half, FinishBand::VerySteep);

    assert!(
        !zs0.is_empty(),
        "THE EXHIBIT IS VACUOUS: very-steep region spans exist ({} moves \
         reported) but contain no moves tagged FinishingCut/EntryPlunge. \
         Waterline tags its contour feeds `MoveIntent::FinishingCut` \
         (`waterline.rs`), so an empty by-intent selection means the \
         tagging changed — investigate it rather than relaxing the filter.",
        arm0.report.very_steep.move_count,
    );
    assert_eq!(
        zs0.len(),
        zs_half.len(),
        "stock_to_leave must never change which points are emitted as cuts \
         (very-steep band, selected by region span)"
    );

    let max_shift = max_abs_shift(&zs0, &zs_half);

    assert!(
        max_shift < 1e-9,
        "INVERT THIS LINE WHEN THE FIX LANDS — but note this is a SEPARATE \
         defect from D-16.2 as filed.\n\
         \n\
         VerySteep (waterline) exhibit: two ops differing ONLY in \
         `stock_to_leave` (0.0 vs {STOCK_TO_LEAVE_MM} mm) emitted {n} \
         very-steep cut positions each whose Z differed by at most \
         {max_shift:.12} mm — i.e. not at all.\n\
         \n\
         Root cause: `waterline::WaterlineParams` has four fields \
         (`sampling`, `feed_rate`, `plunge_rate`, `safe_z`) and no \
         stock-to-leave field at all, so `unified_finish.rs`'s \
         `FinishBand::VerySteep` arm has nothing to pass. Unlike the \
         shallow band this cannot be fixed by threading an existing dial \
         one level deeper: `WaterlineParams` needs the field first, and \
         whoever adds it must also rule on whether the Z LADDER LEVELS move \
         with the allowance or only the contours at each level.\n\
         \n\
         A FAILURE HERE IS GOOD NEWS. Expected shift is \
         +{STOCK_TO_LEAVE_MM} mm if the fix follows the `cl.z + \
         stock_to_leave` convention every honouring path uses; replace \
         `max_shift < 1e-9` with \
         `(max_shift - STOCK_TO_LEAVE_MM).abs() < 1e-9` in that case. If \
         the ruling was 'levels move too', the shift may be exact on the \
         contours and quantised on the ladder — re-derive the assertion \
         from the ruling rather than pattern-matching test 2.",
        n = zs0.len(),
    );
}
