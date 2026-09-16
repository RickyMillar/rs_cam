//! T2 — the slope-compensated residual treatment
//! (`planning/multitool_2026-08-23/ORCHESTRATION_PLAN.md` Phase T, task T2,
//! plan blocker B1).
//!
//! # The defect being pinned
//!
//! A drop-cutter Z is the tool-CENTRE offset surface. On a plain sloped plane
//! — which **either** ball machines perfectly, leaving nothing behind — two
//! ladder tools disagree by `(R_coarse − R_fine)·(sec θ − 1)`
//! (`T1_FINDINGS.md` §3.4). At 45° with the ladder used here that is
//! **0.518 mm against a 0.030 mm tolerance, 17×**. Untreated, a tier map hands
//! every slope on a terrain to the fine tool, which is the measured cause of
//! the T4 two-tier arm losing 5.5 h (plan §0).
//!
//! # The fixtures, and why each one is here
//!
//! | fixture | `Raw` | `SlopeCompensated` | what it would catch |
//! |---|---|---|---|
//! | 45° plane ramp | fine tier | **coarse tier** | the treatment doing nothing |
//! | hemispherical bowl tighter than the coarse tool | fine tier | fine tier | the treatment over-reaching and stealing genuinely unreachable pockets |
//! | flat plate | coarse tier | coarse tier, **bit-identical** | compensation leaking onto θ = 0 |
//! | 70° plane ramp | fine tier | coarse tier | the cap sitting lower than it says |
//! | 80° plane ramp | fine tier | fine tier | a capped `sec θ` handing near-vertical walls to the coarse tool |
//!
//! The ramps are exact planes (`z = x·tan θ` is linear, so every triangle of
//! the height field is coplanar), which is what makes the analytic law's
//! prediction checkable to `1e-6` rather than merely "about right".
//!
//! # Where the assertions are taken
//!
//! Only well inside the plate. The tier map does **no boundary erosion**
//! (module doc), so within roughly one coarse envelope of the rim the big tool
//! hangs off and reads a false-high residual; on a steep ramp the contact
//! point is offset `R·sin θ` — 1.48 mm at 80° — so the guard band has to clear
//! that too. [`INTERIOR_MM`] is the resulting margin.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use common::tools::ball_cutter;
use rs_cam_core::maps::tier_map::{
    MAX_COMPENSATED_SLOPE_DEG, NO_TIER, ResidualTreatment, TierLadder, TierMap, TierMapError,
    TierMapParams, cl_offset_bias_mm, compute_tier_map, drop_call_count, ladder_drops_at,
    reset_drop_call_count,
};
use rs_cam_core::maps::tier_map_cache::{cached_tier_map, clear};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh, make_test_flat};
use rs_cam_core::tool::MillingCutter;

/// Half-extent of every plate fixture (mm).
const HALF_MM: f64 = 6.0;
/// Height-field pitch for the ramps (mm). A plane needs no resolution — the
/// facets are coplanar at any pitch — so this is set for speed.
const RAMP_STEP_MM: f64 = 0.5;
/// Height-field pitch for the bowl (mm), which does need resolution.
const BOWL_STEP_MM: f64 = 0.2;
/// Bowl radius (mm) — tighter than the coarse tool's 1.5 mm radius, so the
/// coarse tool physically cannot enter. Same fixture as `tier_map_walk_t1`.
const BOWL_R_MM: f64 = 1.2;
/// Grid cell (mm).
const CELL_MM: f64 = 0.25;
/// Residual tolerance (mm) every fixture is judged at.
const TOLERANCE_MM: f64 = 0.03;
/// Assertions stay at least this far inside the plate edge (mm) — see the
/// module doc.
const INTERIOR_MM: f64 = 3.5;

const COARSE_DIA_MM: f64 = 3.0;
const FINE_DIA_MM: f64 = 0.5;

/// `drop_call_count` and the memo table are process-global, so every test in
/// this binary that builds a map takes this first. Same device as
/// `tier_map_cache_t3.rs::cache_lock`.
fn walk_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    #[allow(clippy::unwrap_used)] // poisoning would mean another test panicked
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
}

/// A plane inclined at `slope_deg` about the Y axis: `z = x · tan θ`.
fn ramp(slope_deg: f64) -> TriangleMesh {
    let gradient = slope_deg.to_radians().tan();
    common::meshes::height_field(HALF_MM, RAMP_STEP_MM, move |x, _y| x * gradient)
}

/// A flat plate at z = 0 with one hemispherical bowl of radius [`BOWL_R_MM`]
/// centred on the origin, its floor at `-BOWL_R_MM`. Copied from
/// `tier_map_walk_t1` on purpose: T2 must not move T1's answer.
fn plane_with_bowl() -> TriangleMesh {
    common::meshes::height_field(HALF_MM, BOWL_STEP_MM, |x, y| {
        let d = (x * x + y * y).sqrt();
        if d < BOWL_R_MM {
            -(BOWL_R_MM * BOWL_R_MM - d * d).sqrt()
        } else {
            0.0
        }
    })
}

fn params(treatment: ResidualTreatment) -> TierMapParams {
    TierMapParams {
        cell_mm: CELL_MM,
        tolerance_mm: TOLERANCE_MM,
        margin_mm: 0.5,
        treatment,
    }
}

fn never_cancel() -> impl Fn() -> bool + Send + Sync {
    || false
}

/// Build both arms of the two-ball ladder over one mesh.
fn both_arms(mesh: &TriangleMesh) -> (TierMap, TierMap) {
    let index = SpatialIndex::build_auto(mesh);
    let coarse = ball_cutter(COARSE_DIA_MM);
    let fine = ball_cutter(FINE_DIA_MM);
    let tools: [&dyn MillingCutter; 2] = [&coarse, &fine];
    let ladder = TierLadder::new(&tools).unwrap();
    let cancel = never_cancel();
    let untreated = params(ResidualTreatment::Raw);
    let treated = params(ResidualTreatment::SlopeCompensated);
    let raw = compute_tier_map(mesh, &index, &ladder, &untreated, &cancel).unwrap();
    let comp = compute_tier_map(mesh, &index, &ladder, &treated, &cancel).unwrap();
    (raw, comp)
}

/// Assert every cell of `map` well inside the plate carries `want`, and return
/// how many cells were checked so the caller can refuse a vacuous population.
fn assert_interior_label(map: &TierMap, want: u8, what: &str) -> usize {
    let mut checked = 0usize;
    for row in 0..map.ny {
        for col in 0..map.nx {
            let (x, y) = map.cell_center(row, col).unwrap();
            if x.abs() > INTERIOR_MM || y.abs() > INTERIOR_MM {
                continue;
            }
            checked += 1;
            assert_eq!(
                map.label_at(row, col),
                Some(want),
                "{what}: cell at ({x:.2}, {y:.2}) should be tier {want}"
            );
        }
    }
    assert!(
        checked > 200,
        "{what}: {checked} cells is not a real population"
    );
    checked
}

// ── 1. The killer fixture: a 45° plane both tools machine perfectly ──────

#[test]
fn the_slope_bias_is_measurably_present_on_a_plane_ramp() {
    // Measure the defect before asserting anything about labels: a sentry
    // that only reads labels cannot distinguish "the bias is gone" from
    // "the fixture never had one".
    let _guard = walk_lock();
    let mesh = ramp(45.0);
    let index = SpatialIndex::build_auto(&mesh);
    let coarse = ball_cutter(COARSE_DIA_MM);
    let fine = ball_cutter(FINE_DIA_MM);
    let tools: [&dyn MillingCutter; 2] = [&coarse, &fine];
    let ladder = TierLadder::new(&tools).unwrap();

    let drops = ladder_drops_at(0.0, 0.0, &mesh, &index, &ladder);
    assert!(drops[0].contacted && drops[1].contacted);
    let raw_residual = drops[0].z - drops[1].z;

    let excess = coarse.cusp_radius_mm() - fine.cusp_radius_mm();
    let predicted = cl_offset_bias_mm(excess, 45.0).unwrap();
    assert!(
        (raw_residual - predicted).abs() < 1e-6,
        "the analytic law must reproduce the measured residual: \
         measured {raw_residual}, law {predicted}"
    );
    assert!(
        raw_residual > 15.0 * TOLERANCE_MM,
        "the fixture is only interesting if the bias dwarfs the tolerance, \
         got {raw_residual} against {TOLERANCE_MM}"
    );
}

#[test]
fn a_45_degree_ramp_is_fine_tier_raw_and_coarse_tier_compensated() {
    // THE fixture. Both arms are kept in one test so the contrast cannot be
    // half-deleted: the `Raw` half is the defect, the compensated half is the
    // fix, and they run over byte-identical inputs.
    let _guard = walk_lock();
    let (raw, compensated) = both_arms(&ramp(45.0));

    assert_interior_label(&raw, 1, "raw 45 deg ramp");
    assert_interior_label(&compensated, 0, "compensated 45 deg ramp");

    // The treatments differ in LABELS only. The reference plane is the same
    // drop taken the same way in both walks, so it must survive bit for bit —
    // otherwise "same map, one term different" is not what shipped.
    let raw_bits: Vec<u32> = raw.finest_z.iter().map(|z| z.to_bits()).collect();
    let comp_bits: Vec<u32> = compensated.finest_z.iter().map(|z| z.to_bits()).collect();
    assert_eq!(
        raw_bits, comp_bits,
        "the reference drop plane must be identical across treatments"
    );
    assert_eq!(raw.treatment, ResidualTreatment::Raw);
    assert_eq!(compensated.treatment, ResidualTreatment::SlopeCompensated);
}

// ── 2. Compensation must not steal a genuinely unreachable pocket ────────

#[test]
fn the_bowl_floor_stays_fine_tier_under_compensation() {
    // T1's fixture, T1's answer. The bowl (R1.2) is tighter than the coarse
    // ball (R1.5), so no amount of slope reasoning may hand it over: the
    // coarse tool cannot physically enter. This is the over-reach sentry.
    let _guard = walk_lock();
    let (raw, compensated) = both_arms(&plane_with_bowl());

    for map in [&raw, &compensated] {
        let (row, col) = map.nearest_cell(0.0, 0.0).expect("bowl centre is on grid");
        assert_eq!(
            map.label_at(row, col),
            Some(1),
            "{:?}: the bowl floor is out of the coarse tool's reach",
            map.treatment
        );

        let mut inner = 0usize;
        for r in 0..map.ny {
            for c in 0..map.nx {
                let (x, y) = map.cell_center(r, c).unwrap();
                if (x * x + y * y).sqrt() > BOWL_R_MM * 0.5 {
                    continue;
                }
                inner += 1;
                assert_eq!(
                    map.label_at(r, c),
                    Some(1),
                    "{:?}: bowl interior at ({x:.2}, {y:.2}) must stay fine-tier",
                    map.treatment
                );
            }
        }
        assert!(inner >= 4, "bowl interior sample too small: {inner} cells");
    }

    // And the surrounding flat ground is still the coarse tool's, under both.
    for map in [&raw, &compensated] {
        let (row, col) = map.nearest_cell(4.0, 4.0).expect("flat ground is on grid");
        assert_eq!(
            map.label_at(row, col),
            Some(0),
            "{:?}: flat ground away from the bowl is coarse-tier",
            map.treatment
        );
    }
}

// ── 3. θ = 0 — the treatment must be exactly inert ───────────────────────

#[test]
fn on_flat_ground_the_two_treatments_agree_bit_for_bit() {
    // sec 0 − 1 == 0, so the compensated comparison is `residual − 0.0`, not
    // `residual − something_small`. Anything less than exact equality here
    // means the treatment perturbs surfaces it was never meant to touch.
    let _guard = walk_lock();
    let (raw, compensated) = both_arms(&make_test_flat(20.0));

    assert_eq!((raw.nx, raw.ny), (compensated.nx, compensated.ny));
    assert_eq!(
        raw.labels, compensated.labels,
        "on θ = 0 the compensation must change no label at all"
    );
    let raw_bits: Vec<u32> = raw.finest_z.iter().map(|z| z.to_bits()).collect();
    let comp_bits: Vec<u32> = compensated.finest_z.iter().map(|z| z.to_bits()).collect();
    assert_eq!(raw_bits, comp_bits);

    // Non-vacuous: the plate really is claimed, and the padded ring really is
    // sentinel, in both arms.
    assert!(raw.tier_cell_counts()[0] > 200);
    assert_eq!(raw.tier_cell_counts(), compensated.tier_cell_counts());
    assert!(compensated.unassigned_cells() > 0);
}

// ── 4/5. The cap, from both sides ────────────────────────────────────────

#[test]
fn a_70_degree_ramp_is_still_compensated_below_the_cap() {
    // The lower side of the cap. If someone quietly drops the cap to 60° to
    // make a wall behave, this fails: 70° is mid-steep terrain the coarse
    // tool machines perfectly and must keep.
    let _guard = walk_lock();
    // Deliberate constant tripwire: this test's meaning depends on 70° being
    // BELOW the cap; if the cap moves under it, fail here, not mysteriously.
    #[allow(clippy::assertions_on_constants)]
    {
        assert!(70.0 < MAX_COMPENSATED_SLOPE_DEG);
    }
    let (raw, compensated) = both_arms(&ramp(70.0));
    assert_interior_label(&raw, 1, "raw 70 deg ramp");
    assert_interior_label(&compensated, 0, "compensated 70 deg ramp");
}

#[test]
fn above_the_cap_a_near_vertical_wall_is_not_handed_to_the_coarse_tool() {
    // The upper side. At 80° a CLAMPED sec θ would still subtract its full
    // capped term (`1.25 × (sec 75° − 1)` = 3.58 mm) from a 5.95 mm residual
    // and keep going negative as the wall steepens, passing any cell. The
    // shipped policy is to ABSTAIN instead, so the wall keeps its raw
    // residual and stays fine-tier — the waterline band's territory anyway.
    let _guard = walk_lock();
    // Deliberate constant tripwire: this test's meaning depends on 80° being
    // ABOVE the cap; if the cap moves over it, fail here, not mysteriously.
    #[allow(clippy::assertions_on_constants)]
    {
        assert!(80.0 > MAX_COMPENSATED_SLOPE_DEG);
    }
    let (raw, compensated) = both_arms(&ramp(80.0));

    let checked = assert_interior_label(&raw, 1, "raw 80 deg ramp");
    assert_eq!(
        checked,
        assert_interior_label(&compensated, 1, "compensated 80 deg ramp"),
        "both arms must judge the same population"
    );

    // Abstention is exact, not approximate: above the cap the compensated
    // walk compares the same number the raw one does, so the labels match
    // cell for cell across the interior.
    for row in 0..raw.ny {
        for col in 0..raw.nx {
            let (x, y) = raw.cell_center(row, col).unwrap();
            if x.abs() > INTERIOR_MM || y.abs() > INTERIOR_MM {
                continue;
            }
            assert_eq!(
                raw.label_at(row, col),
                compensated.label_at(row, col),
                "abstention must reproduce the raw verdict at ({x:.2}, {y:.2})"
            );
        }
    }
    // The law itself refuses to answer up there, rather than clamping.
    assert_eq!(cl_offset_bias_mm(1.25, 80.0), None);
}

// ── 6. Determinism, cost, cancellation, and the memo ─────────────────────

#[test]
fn the_compensated_walk_is_deterministic() {
    let _guard = walk_lock();
    let mesh = ramp(45.0);
    let index = SpatialIndex::build_auto(&mesh);
    let coarse = ball_cutter(COARSE_DIA_MM);
    let fine = ball_cutter(FINE_DIA_MM);
    let tools: [&dyn MillingCutter; 2] = [&coarse, &fine];
    let ladder = TierLadder::new(&tools).unwrap();
    let treated = params(ResidualTreatment::SlopeCompensated);

    let a = compute_tier_map(&mesh, &index, &ladder, &treated, &never_cancel()).unwrap();
    let b = compute_tier_map(&mesh, &index, &ladder, &treated, &never_cancel()).unwrap();
    assert_eq!(a.labels, b.labels, "two-pass labels must be deterministic");
    let a_bits: Vec<u32> = a.finest_z.iter().map(|z| z.to_bits()).collect();
    let b_bits: Vec<u32> = b.finest_z.iter().map(|z| z.to_bits()).collect();
    assert_eq!(a_bits, b_bits, "the reference plane must be deterministic");
}

#[test]
fn the_second_pass_buys_slope_not_drops() {
    // The compensated arm is two passes but the SAME drop-cutter work: pass 1
    // takes the reference drop, pass 2 takes the coarse drops. On a two-tool
    // ladder every owned cell costs exactly one coarse drop in either arm, so
    // the counter must land on the same number. (On a longer ladder the two
    // arms early-out at different tiers and this equality does not hold — the
    // claim is about the pass split, not about the ladder.)
    let _guard = walk_lock();
    let mesh = ramp(45.0);
    let index = SpatialIndex::build_auto(&mesh);
    let coarse = ball_cutter(COARSE_DIA_MM);
    let fine = ball_cutter(FINE_DIA_MM);
    let tools: [&dyn MillingCutter; 2] = [&coarse, &fine];
    let ladder = TierLadder::new(&tools).unwrap();

    reset_drop_call_count();
    let raw = params(ResidualTreatment::Raw);
    compute_tier_map(&mesh, &index, &ladder, &raw, &never_cancel()).unwrap();
    let raw_drops = drop_call_count();

    reset_drop_call_count();
    let treated = params(ResidualTreatment::SlopeCompensated);
    compute_tier_map(&mesh, &index, &ladder, &treated, &never_cancel()).unwrap();
    let compensated_drops = drop_call_count();

    assert!(raw_drops > 0, "the walk must actually run the drop cutter");
    assert_eq!(
        raw_drops, compensated_drops,
        "the second pass must not double the drop-cutter bill"
    );
}

#[test]
fn the_compensated_walk_still_polls_the_cancel_token() {
    // The two-pass restructure must not lose the per-row poll — a tier map on
    // a real board is minutes of work and the GUI has to be able to stop it.
    let _guard = walk_lock();
    let mesh = ramp(45.0);
    let index = SpatialIndex::build_auto(&mesh);
    let coarse = ball_cutter(COARSE_DIA_MM);
    let fine = ball_cutter(FINE_DIA_MM);
    let tools: [&dyn MillingCutter; 2] = [&coarse, &fine];
    let ladder = TierLadder::new(&tools).unwrap();

    let polls = AtomicUsize::new(0);
    let cancel_at_once = || {
        polls.fetch_add(1, Ordering::Relaxed);
        true
    };
    let treated = params(ResidualTreatment::SlopeCompensated);
    let err = compute_tier_map(&mesh, &index, &ladder, &treated, &cancel_at_once)
        .expect_err("an always-cancelling walk must not return a map");
    assert!(matches!(err, TierMapError::Cancelled));
    assert!(polls.load(Ordering::Relaxed) > 0, "no poll happened");
}

#[test]
fn the_memo_never_serves_one_treatment_out_of_the_other() {
    // `ResidualTreatment` rides the cache key as a bare discriminant. If it
    // ever stopped doing so, a compensated plan would silently be served the
    // raw map — the two disagree about the entire ramp, so this fixture makes
    // that failure loud rather than subtle.
    let _guard = walk_lock();
    clear();
    let mesh = Arc::new(ramp(45.0));
    let index = SpatialIndex::build_auto(&mesh);
    let coarse = ball_cutter(COARSE_DIA_MM);
    let fine = ball_cutter(FINE_DIA_MM);
    let tools: [&dyn MillingCutter; 2] = [&coarse, &fine];
    let ladder = TierLadder::new(&tools).unwrap();
    let raw = params(ResidualTreatment::Raw);
    let treated = params(ResidualTreatment::SlopeCompensated);
    let cancel = never_cancel();

    let raw_map = cached_tier_map(&mesh, &index, &ladder, &raw, &cancel).unwrap();
    let comp_map = cached_tier_map(&mesh, &index, &ladder, &treated, &cancel).unwrap();
    assert!(
        !Arc::ptr_eq(&raw_map, &comp_map),
        "a compensated map must never be served from a raw entry"
    );
    assert_eq!(raw_map.treatment, ResidualTreatment::Raw);
    assert_eq!(comp_map.treatment, ResidualTreatment::SlopeCompensated);
    assert_ne!(
        raw_map.labels, comp_map.labels,
        "on this fixture the two answers differ, so identical labels would \
         mean one arm was served for the other"
    );

    // Same key twice is still one Arc and zero fresh drop work, per arm.
    reset_drop_call_count();
    let raw_again = cached_tier_map(&mesh, &index, &ladder, &raw, &cancel).unwrap();
    let comp_again = cached_tier_map(&mesh, &index, &ladder, &treated, &cancel).unwrap();
    assert!(Arc::ptr_eq(&raw_map, &raw_again));
    assert!(Arc::ptr_eq(&comp_map, &comp_again));
    assert_eq!(drop_call_count(), 0, "a cache hit must do no drop work");
    clear();
}

// ── 7. The sentinel is still the sentinel ────────────────────────────────

#[test]
fn compensation_does_not_invent_owners_outside_the_part() {
    // A gradient stencil that read `NaN` neighbours as numbers, or that
    // wrapped around a row, would show up here as a labelled cell in the pad.
    let _guard = walk_lock();
    let (_, compensated) = both_arms(&ramp(45.0));
    assert_eq!(
        compensated.label_at(0, 0),
        Some(NO_TIER),
        "the padded corner is off the part"
    );
    for (label, z) in compensated.labels.iter().zip(compensated.finest_z.iter()) {
        if *label == NO_TIER {
            assert!(z.is_nan(), "an unowned cell must not publish a height");
        } else {
            assert!(z.is_finite());
        }
    }
    assert_eq!(
        compensated.tier_cell_counts().iter().sum::<usize>() + compensated.unassigned_cells(),
        compensated.labels.len(),
        "every cell is either a tier or the sentinel"
    );
}
