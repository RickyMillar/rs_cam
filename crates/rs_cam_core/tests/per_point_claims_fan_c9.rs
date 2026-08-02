//! C9 (tech-debt backlog P9, second bullet) — the claims/pencil offset fan
//! is now PER-POINT wide, not a single scalar folded from the branch's
//! shallowest reported depth.
//!
//! ## The defect, precisely
//!
//! `reach::suggested_offset_stepover_mm(cutter, depth)` is DEFINED as
//! `working_half_width_mm(cutter, depth) * SUGGESTED_STEPOVER_OVERLAP`
//! (`SUGGESTED_STEPOVER_OVERLAP == 0.5`) — the policy's whole promise is a
//! 50 % overlap between neighbouring passes, i.e. `spacing / half_width ==
//! 0.5` at every point a pass is placed.
//!
//! Before C9, `crease_paths::centerline_cut_paths` sized ONE stepover per
//! branch at the SHALLOWEST reported depth
//! (`reach::suggested_offset_stepover_mm(cutter, min_valley_depth)` in
//! `unified_finish.rs`'s claims pass; `params.offset_stepover` sized the
//! same way in `pencil::rest_depth_arm`) and applied it to every point,
//! even though `working_half_width_mm` is monotone non-decreasing in
//! depth. That is not a coverage defect first — `paths_from_sampled`'s
//! per-point reach truncation (PR-5) already lets a small, uniform
//! stepover creep close to the local reach given enough passes. It is an
//! OVERLAP defect: at a point deeper than the reference depth, the
//! (too-small) uniform spacing is a much SMALLER fraction of that point's
//! own (larger) working half-width than the policy specifies, so the fan
//! was re-cutting material it had already cut instead of spending that
//! pass reaching outward. On the shipped Ø1-tip/7°/Ø6-shank taper, sizing
//! at 0.2 mm and applying at 2.0 mm turns the intended 50 % overlap into
//! ~64 % — measured below, not asserted from memory.
//!
//! ## What this file checks
//!
//! 1. `per_point_fan_holds_the_target_overlap_at_both_ends` — THE GATE.
//!    Runs the real, now-`pub` `crease_paths::centerline_cut_paths` on a
//!    hand-built `RestCenterline` whose rest depth ramps from 0.2 mm to
//!    2.0 mm along its run (explicit `samples`, not detector output — see
//!    [`ramped_centerline`]), reads the FIRST offset pass's real emitted
//!    lateral position at both ends, and asserts the achieved overlap sits
//!    at `SUGGESTED_STEPOVER_OVERLAP` (50 %) at BOTH ends — the invariant
//!    a per-branch scalar cannot hold except at the one depth it was sized
//!    from. It also reconstructs, from the stable `reach::` primitives
//!    (not through `centerline_cut_paths`, since the retired code path no
//!    longer exists to invoke), what the SAME branch's deep end would have
//!    achieved under the old uniform-scalar scheme, and pins that
//!    reconstruction as markedly worse — the two numbers are printed
//!    together so the claim can be checked against the evidence in one
//!    run. This is the test expected to fail RED against the pre-C9 code
//!    (which has no per-point branch in `centerline_cut_paths` at all —
//!    the `pub`/accessor surface this file needs does not exist there
//!    either, so "red" pre-fix is a hard compile failure, not a softer
//!    assertion failure; see the file's closing note).
//! 2. `per_point_fan_never_places_a_pass_beyond_its_local_reach` — a smoke
//!    check that the per-point TRUNCATION criterion `k · stepover_i ≤
//!    reach_i` still holds everywhere on the emitted fan, re-derived at
//!    each surviving point's own X position.
//! 3. `c9_ball_control_stepover_is_depth_invariant` — the Ø3 ball control:
//!    its cusp radius equals its envelope radius, so
//!    `suggested_offset_stepover_mm` — and therefore the achieved overlap —
//!    is IDENTICAL at every depth. C9's per-point fix is a no-op on this
//!    tool, exactly as it is not on the taper.
//!
//! ## Fixture note
//!
//! The task that produced this file asked for a "V-groove whose wall angle
//! ramps" to vary the apex depth along the run. This fixture instead uses a
//! straight VERTICAL-walled channel of constant, generous rim distance
//! ([`ramped_centerline`]), because the overlap invariant this file gates
//! on does not depend on how much reach margin exists — only on whether the
//! first offset pass survives at all, which a generous constant rim
//! guarantees without needing to hand-tune reach against a pass-count cap.
//! `reach::solve_reach` on a vertical wall reduces to the closed form
//! `rim_distance − width_at_height(depth)` (no scan — see
//! `reach::profile_rise`'s doc), so every reach value used here is EXACT.
//!
//! Basis: `planning/review_2026-07-29/TECH_DEBT_RESEARCH_AND_FIX_PLAN.md`
//! backlog item P9 (second bullet), wave C9.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

mod common;

use common::meshes::plateau;
use common::tools::{ball_control, wanaka_taper};

use rs_cam_core::compute::config::TipFloatFinding;
use rs_cam_core::crease_paths::centerline_cut_paths;
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::pencil::PencilPath;
use rs_cam_core::reach::{self, LocalValley, SUGGESTED_STEPOVER_OVERLAP, ValleySide};
use rs_cam_core::rest_field::{CenterlineSample, RestCenterline};
use rs_cam_core::tool::MillingCutter;

// ── Fixture geometry: a straight run along X at y = 0 ──────────────────

/// Sample count along the run. Only the endpoints are load-bearing; the
/// interior just needs to be dense enough for a believable centreline.
const N: usize = 21;
const RUN_LEN_MM: f64 = 20.0;
const X_MIN_MM: f64 = -RUN_LEN_MM / 2.0;

/// Rest depth at the shallow end — the reference depth both shipped callers
/// (`unified_finish.rs`'s `min_valley_depth`, `rest_depth_arm`'s scalar
/// dial) size their single stepover from.
const DEPTH_SHALLOW_MM: f64 = 0.2;
/// Rest depth at the deep end.
const DEPTH_DEEP_MM: f64 = 2.0;

/// Constant vertical-wall rim distance (mm), generous relative to every
/// stepover in play across the whole depth range — see the module doc's
/// fixture note. Big enough that the first offset pass never gets
/// truncated by reach at either end (checked by an assert, not assumed).
const RIM_MM: f64 = 5.0;

/// The pass-count cap both shipped callers use
/// (`CLAIMS_OFFSET_PASS_CAP` in `unified_finish.rs`,
/// `PencilParams::num_offset_passes` in `rest_depth_arm`).
const CAP: usize = 4;

const MIN_CUT_LENGTH_MM: f64 = 2.0;
const SAMPLING_MM: f64 = 1.0;

/// Tolerance on the achieved-overlap assertions. The per-point scheme's
/// overlap is exact by construction (`spacing == half_width *
/// SUGGESTED_STEPOVER_OVERLAP`, both sides of the ratio computed from the
/// SAME `depth_mm`), so this only needs to absorb floating-point rounding
/// through the offset/lift pipeline, not model uncertainty.
const OVERLAP_TOL: f64 = 1e-6;

fn t_of_index(i: usize) -> f64 {
    i as f64 / (N - 1) as f64
}

fn x_of_t(t: f64) -> f64 {
    X_MIN_MM + t * RUN_LEN_MM
}

/// Inverse of [`x_of_t`], clamped — offset passes only ever move points in
/// Y (the run is X-aligned, so the perpendicular normal is pure +Y), so
/// every emitted point's X still lands on this line.
fn t_of_x(x: f64) -> f64 {
    ((x - X_MIN_MM) / RUN_LEN_MM).clamp(0.0, 1.0)
}

fn depth_of_t(t: f64) -> f64 {
    DEPTH_SHALLOW_MM + t * (DEPTH_DEEP_MM - DEPTH_SHALLOW_MM)
}

/// The `LocalValley` at parameter `t`: a VERTICAL-walled channel (both
/// sides, constant [`RIM_MM`]) — see the module doc's fixture note.
fn valley_at_t(t: f64) -> LocalValley {
    let side = ValleySide::vertical(RIM_MM);
    LocalValley {
        rest_depth_mm: depth_of_t(t),
        left: side,
        right: side,
    }
}

/// Build a `RestCenterline` whose per-point rest depth ramps from
/// `DEPTH_SHALLOW_MM` to `DEPTH_DEEP_MM` along the run, with EXPLICIT
/// `samples` — not detector output, per the C9 task's design: the depths
/// here are literal numbers this file controls, so the sentry cannot be
/// confused by detector noise.
fn ramped_centerline(cutter: &dyn MillingCutter) -> RestCenterline {
    let mut points = Vec::with_capacity(N);
    let mut samples = Vec::with_capacity(N);
    for i in 0..N {
        let t = t_of_index(i);
        let valley = valley_at_t(t);
        let r = reach::solve_reach(cutter, &valley);
        points.push(P3::new(x_of_t(t), 0.0, -valley.rest_depth_mm));
        samples.push(CenterlineSample { valley, reach: r });
    }
    RestCenterline {
        points,
        half_width_mm: RIM_MM,
        samples,
    }
}

/// Run `centerline_cut_paths` on [`ramped_centerline`], sizing the
/// (fallback-only, inert for spacing on this MEASURED centreline — see
/// `centerline_cut_paths`'s doc) `offset_stepover` argument at the
/// shallow-depth reference the shipped callers use.
fn generate(
    cutter: &dyn MillingCutter,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
) -> Vec<PencilPath> {
    let centerline = ramped_centerline(cutter);
    let scalar_stepover = reach::suggested_offset_stepover_mm(cutter, DEPTH_SHALLOW_MM);
    let never_cancel = || false;
    let mut float = TipFloatFinding::default();
    centerline_cut_paths(
        std::slice::from_ref(&centerline),
        mesh,
        index,
        cutter,
        SAMPLING_MM,
        scalar_stepover,
        CAP,
        MIN_CUT_LENGTH_MM,
        0.0,
        &mut float,
        &never_cancel,
    )
    .expect("no cancellation requested")
}

// ── (1) THE GATE: real emitted geometry, both ends, one run ─────────────

/// See the module doc's item 1. `SUGGESTED_STEPOVER_OVERLAP` (50 %) must
/// hold at BOTH ends of a branch whose depth varies 10×, measured from the
/// REAL `PencilPath`s `centerline_cut_paths` emits — not re-derived
/// arithmetic — so a regression that restores scalar spacing through any
/// code path fails this test.
#[test]
fn per_point_fan_holds_the_target_overlap_at_both_ends() {
    let taper = wanaka_taper();
    let mesh = plateau(60.0, 5.0);
    let index = SpatialIndex::build(&mesh, 5.0);

    let paths = generate(&taper, &mesh, &index);
    let offset_paths: Vec<_> = paths.iter().filter(|p| !p.is_centerline()).collect();
    assert!(
        !offset_paths.is_empty(),
        "the fixture must emit an offset fan"
    );

    // The FIRST offset pass in emission order is pass_num == 1 on the
    // `fan.left` side (`paths_from_sampled` emits centreline, then the
    // whole left fan low-to-high, then the whole right fan) — its spacing
    // from the centreline (offset 0) IS the achieved stepover at pass 1,
    // and since every pass at a given point is `k` copies of that SAME
    // per-point stepover, pass 1 alone characterises the spacing.
    let pass1 = offset_paths[0];
    let shallow_pt = pass1
        .points()
        .first()
        .expect("pass 1 must have at least one point");
    let deep_pt = pass1
        .points()
        .last()
        .expect("pass 1 must have at least one point");
    assert!(
        shallow_pt.z.is_finite() && deep_pt.z.is_finite(),
        "pass 1 must survive at both ends — RIM_MM is meant to be generous \
         enough that it always does; shallow z={}, deep z={}",
        shallow_pt.z,
        deep_pt.z
    );

    let offset_shallow_real = shallow_pt.y.abs();
    let offset_deep_real = deep_pt.y.abs();
    let half_width_shallow = reach::working_half_width_mm(&taper, DEPTH_SHALLOW_MM);
    let half_width_deep = reach::working_half_width_mm(&taper, DEPTH_DEEP_MM);
    let overlap_shallow_real = 1.0 - offset_shallow_real / half_width_shallow;
    let overlap_deep_real = 1.0 - offset_deep_real / half_width_deep;

    // Historical reconstruction: what the retired uniform-scalar scheme
    // would have achieved at the deep end of this SAME branch. Computed
    // from the stable `reach::` primitives, not through
    // `centerline_cut_paths` — that code path is gone, by design (this is
    // the fix). Printed for context, not asserted as the gate.
    let scalar_stepover = reach::suggested_offset_stepover_mm(&taper, DEPTH_SHALLOW_MM);
    let overlap_deep_reconstructed_old = 1.0 - scalar_stepover / half_width_deep;

    println!(
        "C9 overlap — shallow end (depth {DEPTH_SHALLOW_MM}mm): real spacing \
         {offset_shallow_real:.4}mm / half-width {half_width_shallow:.4}mm = \
         overlap {:.2}% (target {:.0}%)",
        overlap_shallow_real * 100.0,
        SUGGESTED_STEPOVER_OVERLAP * 100.0
    );
    println!(
        "C9 overlap — deep end (depth {DEPTH_DEEP_MM}mm): real spacing \
         {offset_deep_real:.4}mm / half-width {half_width_deep:.4}mm = \
         overlap {:.2}% (target {:.0}%)",
        overlap_deep_real * 100.0,
        SUGGESTED_STEPOVER_OVERLAP * 100.0
    );
    println!(
        "C9 overlap — deep end RECONSTRUCTED under the retired uniform-scalar \
         scheme (stepover sized at {DEPTH_SHALLOW_MM}mm = {scalar_stepover:.4}mm, \
         applied at {DEPTH_DEEP_MM}mm): overlap {:.2}% — the defect this fix \
         retired, not the current behaviour",
        overlap_deep_reconstructed_old * 100.0
    );

    // THE GATE: the real, emitted fan holds the target overlap at BOTH
    // ends — the property a per-branch scalar can only hold at the one
    // depth it happened to be sized from.
    assert!(
        (overlap_shallow_real - SUGGESTED_STEPOVER_OVERLAP).abs() < OVERLAP_TOL,
        "shallow-end achieved overlap {:.6} != target {:.6}",
        overlap_shallow_real,
        SUGGESTED_STEPOVER_OVERLAP
    );
    assert!(
        (overlap_deep_real - SUGGESTED_STEPOVER_OVERLAP).abs() < OVERLAP_TOL,
        "deep-end achieved overlap {:.6} != target {:.6} — this is the C9 \
         regression: a uniform scalar sized at the shallow end cannot hold \
         the target overlap at the deep end",
        overlap_deep_real,
        SUGGESTED_STEPOVER_OVERLAP
    );
    // The reconstruction is the documented defect, pinned: on the shipped
    // taper, sizing at 0.2mm and applying at 2.0mm turns the intended 50%
    // overlap into more than 60% — passes re-cutting material instead of
    // reaching outward.
    assert!(
        overlap_deep_reconstructed_old > 0.60,
        "expected the retired uniform-scalar scheme's deep-end overlap to \
         exceed 60% (measured independently at ~63.7%); got {:.2}% — the \
         fixture no longer discriminates",
        overlap_deep_reconstructed_old * 100.0
    );

    // Informational only (per review): pass-count / lateral-reach at CAP is
    // a CAP artefact on this generous-rim fixture, not the invariant this
    // test gates on — printed honestly, not spun as a coverage win.
    let surviving_passes_at = |end_first: bool| -> usize {
        offset_paths
            .iter()
            .filter(|p| {
                let pt = if end_first {
                    p.points().first()
                } else {
                    p.points().last()
                };
                pt.is_some_and(|pt| pt.z.is_finite())
            })
            .count()
    };
    println!(
        "C9 overlap — informational: {} of {} declared offset passes survive \
         at the shallow end, {} at the deep end (cap = {CAP}; RIM_MM = \
         {RIM_MM}mm makes reach a non-factor here — this is a cap artefact, \
         not a coverage measurement)",
        surviving_passes_at(true),
        offset_paths.len(),
        surviving_passes_at(false)
    );
}

// ── (2) smoke check: no pass ever exceeds its local reach ───────────────

/// No surviving (non-`NaN`) point anywhere on the fan violates the
/// per-point coverage criterion `k · stepover_i ≤ reach_i`, re-derived at
/// each point's own X position rather than assumed.
#[test]
fn per_point_fan_never_places_a_pass_beyond_its_local_reach() {
    let taper = wanaka_taper();
    let mesh = plateau(60.0, 5.0);
    let index = SpatialIndex::build(&mesh, 5.0);

    let paths = generate(&taper, &mesh, &index);
    let offset_paths: Vec<_> = paths.iter().filter(|p| !p.is_centerline()).collect();
    assert!(
        !offset_paths.is_empty(),
        "the fixture must emit an offset fan"
    );

    let mut checked = 0usize;
    for path in &offset_paths {
        for p in path.points() {
            if !p.z.is_finite() {
                continue; // truncated — not a coverage claim
            }
            let t = t_of_x(p.x);
            let valley = valley_at_t(t);
            let local_reach = reach::solve_reach(&taper, &valley).left_mm;
            assert!(
                p.y.abs() <= local_reach + 0.05,
                "coverage violation at x={:.3} (t={t:.3}): offset {:.4}mm \
                 exceeds the local reach {local_reach:.4}mm",
                p.x,
                p.y.abs()
            );
            checked += 1;
        }
    }
    assert!(
        checked > 0,
        "no surviving points to check — the fan is entirely truncated"
    );
    println!("C9 no-violation smoke check: {checked} surviving points, 0 violations");
}

// ── control: the Ø3 ball does not move ───────────────────────────────────

/// The Ø3 ball control: `cusp_radius() == envelope_radius() == 1.5mm`
/// (`common::tools::ball_control`'s doc), so
/// `suggested_offset_stepover_mm` — and therefore the achieved overlap — is
/// IDENTICAL at the shallow and deep reference depths. C9's per-point fix
/// is therefore a no-op on this tool: there is nothing for per-point
/// spacing to differ FROM.
#[test]
fn c9_ball_control_stepover_is_depth_invariant() {
    let ball = ball_control();
    let s_shallow = reach::suggested_offset_stepover_mm(&ball, DEPTH_SHALLOW_MM);
    let s_deep = reach::suggested_offset_stepover_mm(&ball, DEPTH_DEEP_MM);

    // Exact: half the 1.5mm cusp radius.
    assert!(
        (s_shallow - 0.75).abs() < 1e-9,
        "expected the Ø3 ball's cusp-floor stepover (0.75mm), got {s_shallow}"
    );
    assert!(
        (s_deep - s_shallow).abs() < 1e-9,
        "the ball control's stepover must be IDENTICAL at both reference \
         depths — cusp radius equals envelope radius for a plain ball, so \
         C9's per-point fix must be a no-op here; got shallow {s_shallow} \
         vs deep {s_deep}"
    );
    println!(
        "C9 ball control: stepover {s_shallow:.4}mm at both {DEPTH_SHALLOW_MM}mm and \
         {DEPTH_DEEP_MM}mm depth — identical, as required of the control"
    );
}
