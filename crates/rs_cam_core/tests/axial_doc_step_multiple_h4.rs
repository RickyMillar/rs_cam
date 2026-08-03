//! **H4 intake (b)** — peak axial DOC reading an exact MULTIPLE of the
//! commanded Z step.
//!
//! The live validation of 2026-07-30 (`ORCHESTRATION_LOG.md`, CONCERN 3)
//! recorded three readings and named no mechanism, on purpose:
//!
//! | op | commanded step | `peak_axial_doc_mm` | ratio |
//! |---|---|---|---|
//! | 10 `3D Rough 6`, pass 1 @ z 22.4 | 2.6 mm | 2.6 | exactly 1x |
//! | 10 `3D Rough 6`, pass 2 @ z 19.8 | 2.6 mm | **5.200000762939453** | **exactly 2x** |
//! | 4 `Back Rough`, all five passes | 3.0 mm | 3.0 | exactly 1x |
//! | 8 (surface finish) | `z_step` 0.3 mm | 1.86 | ~6x |
//!
//! The brief for this wave is explicit: *five prior mechanisms here were
//! wrong, so build a discriminating probe before naming a mechanism.*
//! Narration's own suggestions were arc-fit overshoot, lift bridging and
//! uncleared stock; arc-fit had already been exonerated for the related
//! Rivers 6.07 mm spike (the fourth time a confirmed mechanism was not the
//! cause).
//!
//! # The probe
//!
//! This file does not run a generator at all. It hand-builds a toolpath so
//! that COVERAGE is known in closed form, drives the shipped simulator
//! (`TriDexelStock::simulate_toolpath_with_metrics_with_cancel` — the same
//! entry point the session uses) and reads `axial_engagement_mm` per
//! sample. Every other variable — generator, arc fitting, lead-ins, lift
//! bridges, depth-pass planning, model geometry — is absent by
//! construction, so a reproduction here cannot be attributed to any of
//! them.
//!
//! Three arms on one stock, cut at the SAME commanded step:
//!
//! * **CLEARED** — pass 2 runs over ground pass 1 already took down one
//!   step.
//! * **VIRGIN** — pass 2 runs over ground no earlier pass visited.
//! * **DEEPER** — pass 3 runs over ground passes 1 and 2 both took down.
//!
//! If the reading tracks the material standing above the cutter, CLEARED
//! and DEEPER both read 1x and VIRGIN reads exactly 2x. If the reading
//! grows with pass index, or with depth, or with anything other than what
//! is actually there, DEEPER separates it — which is why DEEPER is in the
//! probe and not just the two arms the live report showed.
//!
//! # Result (measured, this file)
//!
//! `peak_axial_doc_mm` is `max(pre_ray_length - post_ray_length)` over the
//! cells under the cutter's midpoint disc
//! (`dexel_stock::stamping::stamp_segment_with_metrics`). It is the height
//! of material this stamp REMOVED. It has never been a reading of the
//! commanded step and cannot be one: nothing in the stamping kernel knows
//! what the step was. An exact `n x step` reading means the column carried
//! `n` steps of stock when the pass arrived — a COVERAGE fact about the
//! toolpath, faithfully measured.
//!
//! **The measurement is not the defect. The COMPARISON is.** Narration
//! renders these numbers against a "commanded `depth_per_pass`" — and for
//! op 8 the operation is a surface finish that has no depth-per-pass at
//! all, which is why that line printed *"commanded depth_per_pass is
//! unknown"* and then invited the reader to compare anyway. See the
//! wave-15 entry in the orchestration log for which prior conclusions this
//! touches.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::simulation_cut::SimulationCutSample;
use rs_cam_core::tool::FlatEndmill;
use rs_cam_core::toolpath::{MoveIntent, Toolpath};

/// The live op-10 numbers, kept so the probe reproduces the operator's
/// arithmetic rather than a convenient one.
const STOCK_TOP_Z: f64 = 10.0;
const Z_STEP_MM: f64 = 2.6;
const PASS1_Z: f64 = STOCK_TOP_Z - Z_STEP_MM; // 7.4
const PASS2_Z: f64 = PASS1_Z - Z_STEP_MM; // 4.8
const PASS3_Z: f64 = PASS2_Z - Z_STEP_MM; // 2.2

const TOOL_DIAMETER_MM: f64 = 2.0;
const CELL_MM: f64 = 0.25;

/// Y lanes, far enough apart that the discs never overlap.
const LANE_CLEARED: f64 = 0.0;
const LANE_VIRGIN: f64 = 8.0;
const LANE_DEEPER: f64 = 16.0;

const X_START: f64 = 0.0;
const X_END: f64 = 10.0;

/// The dexel rays are `f32`, so an "exact" step survives as an exact `f32`
/// difference, not an exact `f64` one. One part in 10^6 of the step is far
/// tighter than any mechanism that could produce a spurious multiple, and
/// far looser than `f32` rounding at this magnitude.
const EXACTNESS_TOL_MM: f64 = 1e-5;

fn stock() -> TriDexelStock {
    let bbox = BoundingBox3 {
        min: P3::new(X_START - 5.0, LANE_CLEARED - 5.0, 0.0),
        max: P3::new(X_END + 5.0, LANE_DEEPER + 5.0, STOCK_TOP_Z),
    };
    TriDexelStock::from_bounds(&bbox, CELL_MM)
}

/// One straight cut along `+X` in a lane, at a level, preceded by a rapid
/// to its start so the feed move is a pure lateral cut with no plunge.
fn lane_pass(tp: &mut Toolpath, y: f64, z: f64) {
    tp.rapid_to_with_intent(P3::new(X_START, y, STOCK_TOP_Z + 5.0), MoveIntent::Linking);
    tp.rapid_to_with_intent(P3::new(X_START, y, z), MoveIntent::Linking);
    tp.feed_to_with_intent(P3::new(X_END, y, z), 1000.0, MoveIntent::ClearingCut);
}

fn simulate(tp: &Toolpath, stock: &mut TriDexelStock) -> Vec<SimulationCutSample> {
    let cutter = FlatEndmill::new(TOOL_DIAMETER_MM, 25.0);
    let never_cancel = || false;
    stock
        .simulate_toolpath_with_metrics_with_cancel(
            tp,
            &cutter,
            StockCutDirection::FromTop,
            ToolpathId(0),
            18_000,
            2,
            5000.0,
            0.5,
            None,
            &[],
            &[],
            true,
            &never_cancel,
        )
        .expect("simulation completes")
}

/// Peak `axial_engagement_mm` among cutting samples whose Y is in `lane`
/// and whose Z is at `z`. Selecting by geometry rather than by move index
/// keeps the probe honest if the move stream is ever reshaped.
fn peak_axial_in(samples: &[SimulationCutSample], lane: f64, z: f64) -> f64 {
    samples
        .iter()
        .filter(|s| s.is_cutting)
        .filter(|s| (s.position[1] - lane).abs() < 1.0)
        .filter(|s| (s.position[2] - z).abs() < 1e-6)
        .map(|s| s.axial_engagement_mm)
        .fold(0.0_f64, f64::max)
}

/// The discriminator.
#[test]
fn peak_axial_doc_counts_standing_stock_not_the_commanded_step() {
    let mut tp = Toolpath::new();

    // Lane CLEARED: pass 1 at one step, then pass 2 at two steps over the
    // same ground.
    lane_pass(&mut tp, LANE_CLEARED, PASS1_Z);
    lane_pass(&mut tp, LANE_CLEARED, PASS2_Z);

    // Lane VIRGIN: pass 2's level ONLY. No earlier pass has been here, and
    // this is the whole of the difference from the CLEARED lane.
    lane_pass(&mut tp, LANE_VIRGIN, PASS2_Z);

    // Lane DEEPER: all three levels in order.
    lane_pass(&mut tp, LANE_DEEPER, PASS1_Z);
    lane_pass(&mut tp, LANE_DEEPER, PASS2_Z);
    lane_pass(&mut tp, LANE_DEEPER, PASS3_Z);

    let mut st = stock();
    let samples = simulate(&tp, &mut st);
    assert!(
        samples.iter().any(|s| s.is_cutting),
        "the probe cut nothing — the stock or the lanes are misplaced"
    );

    let first_cut = peak_axial_in(&samples, LANE_CLEARED, PASS1_Z);
    let cleared = peak_axial_in(&samples, LANE_CLEARED, PASS2_Z);
    let virgin = peak_axial_in(&samples, LANE_VIRGIN, PASS2_Z);
    let deeper = peak_axial_in(&samples, LANE_DEEPER, PASS3_Z);

    eprintln!("\n== H4 intake (b): peak axial DOC vs commanded step ==");
    eprintln!("  commanded step             {Z_STEP_MM:.4} mm");
    eprintln!(
        "  pass 1, virgin ground      {first_cut:.6} mm  ({:.4}x)",
        first_cut / Z_STEP_MM
    );
    eprintln!(
        "  pass 2 over CLEARED ground {cleared:.6} mm  ({:.4}x)",
        cleared / Z_STEP_MM
    );
    eprintln!(
        "  pass 2 over VIRGIN ground  {virgin:.6} mm  ({:.4}x)",
        virgin / Z_STEP_MM
    );
    eprintln!(
        "  pass 3 over CLEARED ground {deeper:.6} mm  ({:.4}x)",
        deeper / Z_STEP_MM
    );

    // A pass over ground the previous pass took down removes exactly one
    // step — no matter how deep it is or how many passes preceded it.
    assert!(
        (cleared - Z_STEP_MM).abs() < EXACTNESS_TOL_MM,
        "pass 2 over cleared ground read {cleared} mm against a {Z_STEP_MM} mm step"
    );
    assert!(
        (deeper - Z_STEP_MM).abs() < EXACTNESS_TOL_MM,
        "pass 3 over cleared ground read {deeper} mm against a {Z_STEP_MM} mm step — the reading \
         tracks depth or pass index, which is NOT the standing-stock mechanism this probe was \
         built to test. Do not name a mechanism until this is understood."
    );

    // The same commanded step over ground nobody visited removes two.
    let expected_virgin = STOCK_TOP_Z - PASS2_Z; // 5.2 == 2 x 2.6
    assert!(
        (virgin - expected_virgin).abs() < EXACTNESS_TOL_MM,
        "pass 2 over virgin ground read {virgin} mm; the stock top is {STOCK_TOP_Z} and the pass \
         is at {PASS2_Z}, so {expected_virgin} mm was standing there"
    );
    assert!(
        (virgin / cleared - 2.0).abs() < 1e-4,
        "the two arms differ by {:.6}x, not the 2x the live report showed",
        virgin / cleared
    );

    eprintln!(
        "\nMECHANISM: `peak_axial_doc_mm` is the height of material REMOVED by the stamp \
         (pre_ray_len - post_ray_len over the midpoint disc). An exact n x step reading is n \
         steps of standing stock, faithfully measured. It is not, and has never been, a \
         reading of the commanded step."
    );
}

/// The negative control for the *reporting* half: an operation with no
/// commanded step at all still produces a peak axial DOC, because the
/// number is about the stock, not about the plan.
///
/// This is op 8's case in miniature — a single lateral pass with no depth
/// stepping whatsoever reads the full standing height. Narration renders
/// such a number beside "commanded depth_per_pass is unknown" and invites
/// a ratio that has no denominator.
#[test]
fn an_operation_with_no_commanded_step_still_reports_a_peak_axial_doc() {
    let mut tp = Toolpath::new();
    lane_pass(&mut tp, LANE_CLEARED, PASS2_Z);
    let mut st = stock();
    let samples = simulate(&tp, &mut st);
    let peak = peak_axial_in(&samples, LANE_CLEARED, PASS2_Z);
    let standing = STOCK_TOP_Z - PASS2_Z;
    assert!(
        (peak - standing).abs() < EXACTNESS_TOL_MM,
        "a single pass through {standing} mm of standing stock read {peak} mm"
    );
    eprintln!(
        "single pass, no depth stepping: peak axial DOC {peak:.4} mm = the {standing:.4} mm that \
         was standing. There is no commanded step to divide by."
    );
}
