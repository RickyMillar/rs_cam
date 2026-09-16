//! M3 sentries — the engagement denominator is the ENGAGED diameter, not the
//! shank.
//!
//! `dexel_stock::stamping`'s `StampPartial::finish` used to form
//! `radial_woc_fraction` as `(perp_max − perp_min) / (2 · radius)`, where
//! `radius` is the caller's `envelope_radius_mm()` — the same scalar that sizes
//! the stamp's bounding box. The bbox genuinely wants the envelope and still
//! gets it; the engagement fraction wants the diameter that is actually cutting
//! at the sample's own axial DOC, and was getting the shank instead
//! (`planning/review_2026-07-29/RADIUS_AUDIT.md` U3;
//! `planning/rapid_safety_2026-08-28/PLAN.md` Phase M).
//!
//! On the shipped R1.0 tapered ball at 0.5 mm DOC the two differ by
//! `3.0 / 0.8660 = 3.4641×`, and M1 measured the consequence on a real wanaka
//! trace: time-weighted mean engagement **3.69× low** on op 7 and **5.74× low**
//! on op 8, with the two Ø6 flat-endmill roughs reading a correction factor of
//! exactly 1.000 (`M1_RESULTS.md` §7).
//!
//! What these four tests pin:
//!
//! 1. the analytic anchor — a full-immersion pass at 0.5 mm DOC on the shipped
//!    taper now reads near 1.0, where the shank denominator gave ~0.27;
//! 2. the falsification control — a flat endmill is untouched, and by
//!    *identity* rather than by a branch on tool type;
//! 3. the censoring semantics — a corrected fraction past full immersion still
//!    reads exactly 1.0, as it always did;
//! 4. the summary axis (`per_kinematics`) carries the corrected magnitude.
//!
//! ```text
//! cargo test -p rs_cam_core --test engagement_denominator_m3
//! ```
//!
//! **Every band below is stated as an absolute floor, never as a diff against a
//! recomputed "old" value** — reconstructing the pre-fix reading as
//! `new × engaged/envelope` would just be the patch grading itself. The floors
//! are chosen so the pre-fix denominator cannot reach them; each test says by
//! what factor.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::geo::P3;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::stock::radial_profile::{LUT_SAMPLES, RadialProfileLUT};
use rs_cam_core::stock::simulation_cut::{CutKinematics, SimulationCutSample, SummaryAccumulator};
use rs_cam_core::tool::{FlatEndmill, MillingCutter, TaperedBallEndmill};
use rs_cam_core::toolpath::Toolpath;

// ── The fixture board ───────────────────────────────────────────────────────

/// Fine enough that the perpendicular extent of a 1.73 mm engaged diameter is
/// resolved to ~1.5 %. The taper's corrected fraction is bounded below by
/// `(2·w − 2·CELL) / (2·engaged)`, so the cell size is what sets the lower end
/// of every band quoted here.
const CELL: f64 = 0.025;
/// Stock top. 2D-convention frame: the cut happens at negative Z.
const TOP_Z: f64 = 0.0;
const BOTTOM_Z: f64 = -6.0;
const BOARD_X: f64 = 16.0;
const BOARD_Y: f64 = 10.0;
/// The pass runs down the middle in Y, so the full envelope (3 mm) fits either
/// side of it inside the board.
const PATH_Y: f64 = 5.0;
const PATH_X0: f64 = 3.0;
const PATH_X1: f64 = 13.0;
const FEED: f64 = 1200.0;
const SAMPLE_STEP: f64 = 0.25;

/// The shipped R1.0 tapered ball: Ø2 ball tip, 5.7° taper, Ø6 shaft. Its
/// `radius()` is the SHAFT radius, 3.0 — which is exactly why U3 was invisible.
fn shipped_taper() -> TaperedBallEndmill {
    TaperedBallEndmill::new(2.0, 5.7, 6.0, 20.0)
}

/// A virgin block, top at `TOP_Z`.
fn fresh_block(cell: f64) -> TriDexelStock {
    TriDexelStock::from_stock(0.0, 0.0, BOARD_X, BOARD_Y, BOTTOM_Z, TOP_Z, cell)
}

/// Rapid to depth (rapids are sampled but never stamped on the metric walk),
/// then one straight fed pass at constant Z.
fn straight_pass(tip_z: f64) -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(PATH_X0, PATH_Y, TOP_Z + 5.0));
    tp.rapid_to(P3::new(PATH_X0, PATH_Y, tip_z));
    tp.feed_to(P3::new(PATH_X1, PATH_Y, tip_z), FEED);
    tp
}

/// Run the production metric seam and hand back the cutting samples of the
/// fed pass.
fn linear_cut_samples(
    stock: &mut TriDexelStock,
    cutter: &dyn MillingCutter,
    tip_z: f64,
) -> Vec<SimulationCutSample> {
    let tp = straight_pass(tip_z);
    let lut = RadialProfileLUT::from_cutter(cutter, LUT_SAMPLES);
    let never_cancel = || false;
    stock
        .simulate_toolpath_with_lut_metrics_rapid_checked(
            &tp,
            &lut,
            cutter,
            cutter.radius(),
            StockCutDirection::FromTop,
            ToolpathId(0),
            18_000,
            2,
            5000.0,
            SAMPLE_STEP,
            None,
            &[],
            &[],
            true,
            &never_cancel,
            None,
        )
        .expect("never cancelled")
        .into_iter()
        .filter(|s| s.is_cutting && s.cut_kinematics == CutKinematics::Linear)
        .collect()
}

/// The steady-state middle half of a pass — the ends of any pass see a partial
/// footprint, and this is a claim about the denominator, not about lead-in.
fn steady(samples: &[SimulationCutSample]) -> &[SimulationCutSample] {
    assert!(
        samples.len() >= 8,
        "fixture produced only {} cutting samples",
        samples.len()
    );
    let q = samples.len() / 4;
    &samples[q..samples.len() - q]
}

fn mean_radial(samples: &[SimulationCutSample]) -> f64 {
    samples
        .iter()
        .map(|s| s.engagement.radial_woc_fraction)
        .sum::<f64>()
        / samples.len() as f64
}

// ── (a) the analytic anchor ─────────────────────────────────────────────────

/// **The 3.4641× anchor.** A full-immersion pass at 0.5 mm DOC on the shipped
/// R1.0 taper.
///
/// The arithmetic, all of it checkable by hand:
///
/// * envelope radius = shaft radius = **3.0** — the OLD denominator's half;
/// * `engagement_radius_mm(0.5)` = `sqrt(2·R·h − h²)` = `sqrt(0.75)` =
///   **0.8660254** — the NEW denominator's half;
/// * their ratio is **3.4641016**, reproducing `PLAN.md`'s analytic figure and
///   M1's measured anchor to seven digits.
///
/// The numerator is the perpendicular extent of cells that carried more than
/// `FRESH_MATERIAL_THRESHOLD_MM = 0.05` of fresh material above the cutter
/// surface, so it reaches out to `width_at_height(0.5 − 0.05)` =
/// `sqrt(0.6975)` = **0.8351646**, not to the full 0.8660254 — the fraction is
/// discretisation-limited from below at
/// `(2·0.8351646 − 2·CELL) / 1.7320508 = 0.9356` and bounded above by
/// `2·0.8351646 / 1.7320508 = 0.9644`.
///
/// **Red against the pre-fix code by 3.46×:** the same numerator over the
/// 6.0 mm shank gives 0.270–0.278, so the 0.90 floor below is unreachable
/// without the fix. (It is also the reason M1 found the taper finish ops
/// reading "a tenth of the shank" where they were taking a third of the
/// engaged diameter.)
#[test]
fn tapered_ball_full_slot_is_normalised_by_the_engaged_diameter() {
    let cutter = shipped_taper();
    const DOC: f64 = 0.5;

    // Preamble: the two denominators, stated before anything is simulated.
    let envelope = cutter.envelope_radius_mm();
    let engaged = cutter.engagement_radius_mm(DOC);
    assert!(
        (envelope - 3.0).abs() < 1e-12,
        "shipped taper envelope radius moved: {envelope}"
    );
    assert!(
        (engaged - 0.866_025_403_784_438_6).abs() < 1e-9,
        "engagement_radius_mm(0.5) is not sqrt(0.75): {engaged}"
    );
    assert!(
        (envelope / engaged - 3.464_101_615_137_754).abs() < 1e-6,
        "the U3 anchor ratio moved: {}",
        envelope / engaged
    );

    let mut stock = fresh_block(CELL);
    let samples = linear_cut_samples(&mut stock, &cutter, TOP_Z - DOC);
    let mid = steady(&samples);

    // The stamp must agree that this is a 0.5 mm pass — the denominator is
    // evaluated at exactly the depth published as `axial_doc_mm`, so a wrong
    // depth here would silently produce a right-looking fraction.
    let axial = mid.iter().map(|s| s.axial_doc_mm).fold(0.0_f64, f64::max);
    assert!(
        (axial - DOC).abs() < 0.05,
        "measured axial DOC {axial} is not the commanded {DOC}"
    );

    let mean = mean_radial(mid);
    assert!(
        (0.90..=1.0).contains(&mean),
        "corrected radial engagement {mean} is outside the \
         discretisation-limited full-slot band [0.9356, 0.9644]; the pre-fix \
         shank denominator reads ~0.27 here"
    );
    // Nothing may saturate: this pass is genuinely just short of full
    // immersion, and a clamp would hide a denominator that had gone too small.
    let peak = mid
        .iter()
        .map(|s| s.engagement.radial_woc_fraction)
        .fold(0.0_f64, f64::max);
    assert!(
        peak < 1.0,
        "a 0.5 mm pass saturated at 1.0 — the denominator is too small, not \
         merely corrected"
    );
}

// ── (b) the flat-endmill falsification control ──────────────────────────────

/// **The correction is the IDENTITY on a flat endmill, by construction.**
///
/// `FlatEndmill::width_at_height` returns `self.radius()` at every height — the
/// same `diameter() / 2.0` expression the old `radius` argument was built from
/// — so `2.0 * engagement_radius_mm(doc)` and the pre-fix `2.0 * radius` are
/// the *same `f64`*, bit for bit, and the whole division is unchanged. This is
/// asserted on the bits rather than with a tolerance, and it is why the fix
/// needed no branch on tool type.
///
/// The depth spread deliberately includes `0.0` (where every ball-tipped shape
/// returns a zero engaged radius) and a depth past the flute length.
#[test]
fn flat_endmill_denominator_is_the_envelope_radius() {
    for (name, cutter) in [
        ("flat_6", FlatEndmill::new(6.0, 25.0)),
        ("flat_0p5", FlatEndmill::new(0.5, 6.0)),
        ("flat_12p7", FlatEndmill::new(12.7, 40.0)),
    ] {
        let envelope = cutter.envelope_radius_mm();
        for doc in [0.0, 1e-9, 0.02, 0.5, 3.0, 12.0, 1000.0] {
            let engaged = cutter.engagement_radius_mm(doc);
            assert_eq!(
                engaged.to_bits(),
                envelope.to_bits(),
                "{name}: engagement_radius_mm({doc}) = {engaged} is not \
                 bit-identical to the envelope radius {envelope}; the M3 \
                 denominator would no longer be a no-op on flat tools"
            );
        }
    }
}

/// The same physical cut, read through two tools of the same 6 mm envelope.
///
/// Pre-fix these two disagreed by the full 3.46×: the flat read a genuine
/// ~0.98 full slot and the taper read ~0.27 of the *shank*. Post-fix both are
/// reading a full slot of their own engaged diameter, and the residual gap is
/// discretisation only — the taper's 1.73 mm engaged diameter is 3.46× more
/// sensitive to the 0.025 mm cell than the flat's 6.0 mm.
///
/// The flat side of this pairing is the falsification control M1 §9 names: its
/// correction factor is identically 1.000, so a flat reading that moves means
/// the patch touched something other than the denominator.
#[test]
fn flat_and_taper_now_agree_that_a_full_slot_is_a_full_slot() {
    const DOC: f64 = 0.5;
    let flat = FlatEndmill::new(6.0, 25.0);
    let taper = shipped_taper();

    let mut flat_stock = fresh_block(CELL);
    let flat_mean = mean_radial(steady(&linear_cut_samples(
        &mut flat_stock,
        &flat,
        TOP_Z - DOC,
    )));
    let mut taper_stock = fresh_block(CELL);
    let taper_mean = mean_radial(steady(&linear_cut_samples(
        &mut taper_stock,
        &taper,
        TOP_Z - DOC,
    )));

    // The flat's own band. CLAUDE.md records that a genuine full slot reads
    // ~0.95 rather than 1.0 under the F.a sub-cell coverage gate; at this cell
    // size it lands near 0.98.
    assert!(
        (0.90..=1.0).contains(&flat_mean),
        "the Ø6 flat control moved off the full-slot band: {flat_mean}"
    );
    assert!(
        (flat_mean - taper_mean).abs() < 0.10,
        "flat reads {flat_mean} and taper reads {taper_mean} on the same \
         full-slot fixture; pre-fix the gap was the full 3.46×"
    );
}

/// **The correction can only RAISE a reading, on every shipped shape.**
///
/// `engagement_radius_mm(d) <= envelope_radius_mm()` for all `d >= 0` is what
/// makes M2's capture readable: engagement, the arc derived from it, and the
/// power/deflection figures downstream either rise or stay put, and air-cut
/// time can only fall. A fixture that moves the other way after this commit is
/// evidence about the patch, not about the tool.
///
/// This also covers the transit-span case (CLAUDE.md P3): where the dexel
/// over-reads axial DOC, the engaged radius saturates at the envelope and the
/// fraction is left exactly as it was, rather than being pushed down.
#[test]
fn the_engaged_radius_never_exceeds_the_envelope_on_any_shipped_shape() {
    use rs_cam_core::tool::{BallEndmill, BullNoseEndmill, VBitEndmill};

    let shapes: Vec<(&str, Box<dyn MillingCutter>)> = vec![
        ("flat_6", Box::new(FlatEndmill::new(6.0, 25.0))),
        ("ball_6", Box::new(BallEndmill::new(6.0, 25.0))),
        ("ball_1", Box::new(BallEndmill::new(1.0, 8.0))),
        (
            "bullnose_6r1",
            Box::new(BullNoseEndmill::new(6.0, 1.0, 25.0)),
        ),
        ("vbit_60", Box::new(VBitEndmill::new(12.7, 60.0, 20.0))),
        ("vbit_20", Box::new(VBitEndmill::new(5.5, 20.0, 20.0))),
        ("taper_r1_5p7", Box::new(shipped_taper())),
        (
            "taper_r0p5_7p1",
            Box::new(TaperedBallEndmill::new(1.0, 7.1, 6.0, 25.0)),
        ),
        (
            "taper_r1p5_2p8",
            Box::new(TaperedBallEndmill::new(3.0, 2.8, 6.0, 25.0)),
        ),
    ];
    for (name, cutter) in shapes {
        let envelope = cutter.envelope_radius_mm();
        let mut prev = f64::NEG_INFINITY;
        for i in 0..=4000 {
            let doc = i as f64 * 0.02;
            let engaged = cutter.engagement_radius_mm(doc);
            assert!(
                engaged <= envelope + 1e-12,
                "{name}: engagement_radius_mm({doc}) = {engaged} exceeds the \
                 envelope radius {envelope}; the M3 denominator would LOWER a \
                 reading and M2's before/after capture would stop being \
                 one-directional"
            );
            assert!(
                engaged >= prev - 1e-12,
                "{name}: engaged radius fell from {prev} to {engaged} at \
                 doc {doc}"
            );
            prev = engaged;
        }
    }
}

// ── (c) the clamp is kept, deliberately ─────────────────────────────────────

/// **A corrected fraction past full immersion still reads exactly 1.0.**
///
/// The metric has always been CENSORED at full immersion rather than
/// extrapolated past it, and M3 keeps that: M1 §8 L1 measured 203,360 samples
/// project-wide that saturate under the corrected denominator, and L2 records
/// that 80,656 were already censored before the fix. Changing the censoring
/// would confound the magnitude evidence M2 is collecting.
///
/// The fixture is a finish pass re-tracking its own flank — the regime M1's
/// ops 7/8 live in. The stock surface is the tool's own profile raised by
/// 0.10 mm inside a ±1.0 mm band and cut well clear of the tool outside it, so
/// every engaged cell removes 0.10 mm:
///
/// * `max_penetration` = 0.10 → denominator `2 · sqrt(0.2 − 0.01)` =
///   **0.8718 mm**;
/// * the perpendicular extent runs the width of the band, **1.90–2.00 mm**
///   (cell-quantised);
/// * uncensored that is **2.18–2.29**, so the reading must be exactly `1.0`.
///
/// **Red against the pre-fix code by ~6.7×:** the same extent over the 6.0 mm
/// shank is 0.317–0.333, nowhere near saturation.
#[test]
fn a_corrected_fraction_past_full_immersion_is_censored_at_one() {
    // Coarser than the other fixtures because this one shapes every cell, and
    // the clamp has 2.2× of headroom rather than 3 %.
    const SHAPE_CELL: f64 = 0.05;
    const TIP_Z: f64 = -1.2;
    const STANDING_MM: f64 = 0.10;
    const BAND_HALF_WIDTH: f64 = 1.0;

    let cutter = shipped_taper();
    let mut stock = fresh_block(SHAPE_CELL);
    {
        let rows = stock.z_grid.rows;
        let cols = stock.z_grid.cols;
        let origin_v = stock.z_grid.origin_v;
        let cs = stock.z_grid.cell_size;
        for row in 0..rows {
            let dy = (origin_v + row as f64 * cs - PATH_Y).abs();
            // Inside the band: the cutter's own flank, plus the standing skim.
            // Outside: below the tip, so the pass cannot reach it and it
            // contributes neither perpendicular extent nor penetration.
            let top = if dy <= BAND_HALF_WIDTH {
                let h = cutter
                    .height_at_radius(dy)
                    .expect("inside the envelope radius");
                TIP_Z + h + STANDING_MM
            } else {
                TIP_Z - 0.5
            };
            assert!(top < TOP_Z, "shaped surface {top} escaped the block top");
            for col in 0..cols {
                stock.clear_above_at(row, col, top as f32);
            }
        }
    }

    let samples = linear_cut_samples(&mut stock, &cutter, TIP_Z);
    let mid = steady(&samples);

    // The DOC that sets the denominator really is the thin skim, not the
    // 1.2 mm the tip sits below the original block top.
    let axial = mid.iter().map(|s| s.axial_doc_mm).fold(0.0_f64, f64::max);
    assert!(
        axial < 0.2,
        "measured axial DOC {axial} is not the {STANDING_MM} mm skim this \
         fixture builds; the clamp claim below would be about a different cut"
    );

    let saturated = mid
        .iter()
        .filter(|s| s.engagement.radial_woc_fraction == 1.0)
        .count();
    assert!(
        saturated * 2 > mid.len(),
        "only {saturated} of {} steady samples censored at exactly 1.0; \
         over-immersion must clamp, not extrapolate",
        mid.len()
    );
    let peak = mid
        .iter()
        .map(|s| s.engagement.radial_woc_fraction)
        .fold(0.0_f64, f64::max);
    assert_eq!(
        peak.to_bits(),
        1.0_f64.to_bits(),
        "the censored value is {peak}, not exactly 1.0"
    );
}

// ── (d) the summary axis carries the corrected magnitude ────────────────────

/// `KinematicsSummary::average_radial_woc_fraction` is the axis-aware reading
/// CLAUDE.md points agents at, and it is time-weighted over the same
/// per-sample fraction. It must carry the correction through.
///
/// Floor rather than delta, for the reason stated in the module docs: 0.80 is
/// 2.9× the ~0.27 the shank denominator produces on this fixture, so the
/// assertion is red pre-fix and cannot be satisfied by a recomputation of the
/// value it is grading.
#[test]
fn per_kinematics_average_carries_the_corrected_engagement() {
    let cutter = shipped_taper();
    let mut stock = fresh_block(CELL);
    let samples = linear_cut_samples(&mut stock, &cutter, TOP_Z - 0.5);

    let mut acc = SummaryAccumulator::default();
    for sample in &samples {
        acc.observe(sample);
    }
    let summary = acc.finish_toolpath(ToolpathId(0));
    let linear = summary
        .per_kinematics
        .get(&CutKinematics::Linear)
        .expect("the fed pass is Linear kinematics");

    assert!(
        linear.cutting_runtime_s > 0.0,
        "the Linear class has an empty population — a gate handed no samples \
         passes and looks healthy"
    );
    assert!(
        linear.average_radial_woc_fraction >= 0.80,
        "per_kinematics Linear average is {}; the shank denominator reads \
         ~0.27 on this fixture",
        linear.average_radial_woc_fraction
    );
    // The whole-toolpath headline moves with it.
    assert!(
        summary.average_engagement >= 0.80,
        "average_engagement is {}",
        summary.average_engagement
    );
}
