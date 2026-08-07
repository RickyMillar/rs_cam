//! E9 — the zero-code falsification probe for the rest-grid refinement anomaly.
//!
//! `rest_grid_resolution_c9.rs` records an OPEN ANOMALY: refining
//! `RestFieldParams::cell_mm` from the shipped 0.5 mm to 0.10 mm makes the
//! shipped detector find LESS. On the tip-scale groove the coarse cell finds
//! one centreline of 19.0 mm and the fine cells find "nothing at all".
//!
//! This file exists to test whether "nothing at all" is true.
//!
//! # The hypothesis under test — DETECTED AND REFUSED
//!
//! `rest_grid_resolution_c9.rs` counts `rf.centerlines.len()`. That is a
//! **post-routing** list. In `detect_rest_valleys`:
//!
//! ```text
//! rest_field.rs:951   skeleton_length += len;                    // BEFORE routing
//! rest_field.rs:969   let verdict = branch_verdict(...);
//! rest_field.rs:970   if verdict == RoutingVerdict::Pencil {
//! rest_field.rs:982       centerlines.push(RestCenterline { .. });
//! rest_field.rs:986   } else if comp != usize::MAX {
//! rest_field.rs:991       clearing_comps.insert(comp);           // Clearing AND Refused
//! ```
//!
//! A branch that is detected, traced, measured, and then **refused** never
//! appears in `centerlines`. The c9 sentry therefore cannot distinguish *"not
//! detected"* from *"detected and refused"* — and it reads neither
//! `report.skeleton_length_mm` (accumulated PRE-routing) nor
//! `clearing_regions`, both of which already carry the distinction.
//!
//! **Pre-registered prediction** (W7, `REST_GRID_ANOMALY_STUDY.md` §4, written
//! before this probe was run): at `h = 0.1` on the tip-scale groove,
//!
//! > `report.skeleton_length_mm ≈ 19 mm` **and** `clearing_regions.len() ≥ 1`
//! > **while** `centerlines.is_empty()`.
//!
//! *Falsified if* `skeleton_length_mm ≈ 0` at `h = 0.1`, which would move the
//! stage of death back to ridge extraction for both phenomena.
//!
//! # RESULT — measured 2026-08-05, prediction CONFIRMED
//!
//! Tip-scale groove, the arm the anomaly's headline is about:
//!
//! | cell | centrelines | centreline mm | **skeleton mm** | **routed away** | **clearing regions** | rest vol mm³ |
//! |---|---|---|---|---|---|---|
//! | 0.50 | 1 | 19.000 | 19.000 | 0.000 | 0 | 12.9813 |
//! | 0.25 | **0** | 0.000 | **18.500** | **18.500** | **1** | 11.1530 |
//! | 0.10 | **0** | 0.000 | **18.200** | **18.200** | **1** | 10.3219 |
//!
//! **The 0.10 mm cell does not "find nothing at all". It finds 18.200 mm of
//! skeleton — 95.8% of the shipped cell's 19.000 mm — traces it, measures it,
//! and then routes every millimetre of it away.** `clearing_regions.len() == 1`
//! at both fine cells; per §4.1 the only reachable non-Pencil exit on these
//! dials is `Refused` (the Pencil/Clearing threshold is `8 × 0.25 = 2.0 mm` of
//! reach and this fixture's median reach is 0.155 mm even at the coarse cell).
//! The rest FIELD also survives — volume 12.98 → 10.32 mm³ — so the mask did
//! not die upstream either.
//!
//! **Consequence: the anomaly's headline is wrong and every downstream
//! statement of the form "the fine grid detects nothing" must be restated.**
//! Refinement does not find less feature. It routes the same feature
//! differently. Detection is essentially flat across a 5× refinement (19.000 →
//! 18.200 mm, −4.2%, monotone — the size of a boundary-resolution effect, not
//! of a detection loss).
//!
//! Wide control, same seams — and it corroborates W7's §2 finding that these
//! are **two anomalies, not one**:
//!
//! | cell | centrelines | centreline mm | skeleton mm | routed away | median reach |
//! |---|---|---|---|---|---|
//! | 0.50 | 2 | 38.000 | 38.000 | 0.000 | 0.5827 |
//! | 0.25 | 2 | 37.000 | 37.000 | 0.000 | 0.2417 |
//! | 0.10 | 2 | 36.400 | 36.400 | **0.000** | **0.0000** |
//!
//! **The control's routing never changes.** Nothing is routed away at any
//! cell; detection, tracing and routing are all stable while median reach
//! collapses to zero. The control's collapse is therefore a **measurement**
//! phenomenon located entirely in the cross-section/reach stages, with no
//! detection or routing component at all — which is exactly the separation
//! W7's §2 predicted and is why a single mechanism was never going to explain
//! both fixtures.
//!
//! What this probe does **not** establish: it does not prove *why* the fine
//! cell refuses. §3.1's ridge-displacement mechanism remains the leading
//! hypothesis and remains **unexecuted** — arm 2 of the study. This probe
//! relocates the stage of death for the tip-scale fixture from detection to
//! routing; it does not name the arithmetic that moved the routing verdict.
//!
//! # Why the routing exit is a binary refusal detector on these fixtures
//!
//! With `num_offset_passes_cap: 8` and `offset_stepover_mm: 0.25` the
//! Pencil/Clearing threshold is `8 × 0.25 = 2.0 mm` of reach, which neither
//! fixture's narrow side approaches. **On these fixtures the only reachable
//! non-Pencil exit is `Refused`** — so a branch that leaves `centerlines`
//! without leaving `skeleton_length_mm` was refused, not re-routed.
//!
//! # Scope
//!
//! **Zero production change. No cargo behaviour change. `RestFieldParams` are
//! byte-identical to `rest_grid_resolution_c9::measure`'s** — same mesh, same
//! reference, same tool, same thresholds, same cell ladder — so the two files
//! are reading the same runs through different seams. Any divergence in the
//! shared columns is itself a finding.
//!
//! `rest_grid_resolution_c9` stays green and untouched (Checkpoint E ruling
//! Q4). Production rest-grid resolution does not move.
//!
//! Owner of the anomaly, re-pointed at Checkpoint E (E10): the ridge-extraction
//! stages `rest_field::box_smooth_rest` / `nms_candidates`, **not**
//! `measure_cross_section`, which is downstream of the defect and whose own
//! contribution is bounded by one cell.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use rs_cam_core::geo::polyline_length;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::rest_field::{RestFieldParams, RestReference, detect_rest_valleys};
use rs_cam_core::tool::{BallEndmill, MillingCutter};

mod common;

use common::meshes::GroovedBlock;
use common::tools::wanaka_taper;

/// The c9 ladder, unchanged. `0.5` is `RestFieldParams::default`'s shipped
/// value.
const CELLS: [f64; 3] = [0.5, 0.25, 0.1];

/// Everything the c9 sentry reads, plus the four pre-routing seams it does
/// not.
#[derive(Debug, Clone)]
struct Probe {
    cell: f64,
    // --- what c9 reads (post-routing) --------------------------------------
    centerlines: usize,
    centerline_length_mm: f64,
    median_reach_mm: f64,
    refused_points: usize,
    sampled_points: usize,
    // --- what c9 does NOT read (pre-routing / routed-away) ------------------
    /// Accumulated BEFORE `branch_verdict` — every traced skeleton polyline.
    skeleton_length_mm: f64,
    /// Components that produced ≥1 pencil-routed polyline.
    pencil_region_count: usize,
    /// Components routed (wholly or partly) away from pencil.
    clearing_region_count: usize,
    clearing_regions: usize,
    /// Σ rest × cell² over the mask — did the FIELD survive refinement?
    total_rest_volume_mm3: f64,
    /// Length that survived the `min_cut_length` filter after pencil routing.
    traced_length_mm: f64,
    grid_nx: usize,
    grid_ny: usize,
}

impl Probe {
    /// The decisive quantity: skeleton traced but not emitted as pencil.
    fn routed_away_mm(&self) -> f64 {
        (self.skeleton_length_mm - self.centerline_length_mm).max(0.0)
    }
}

fn probe(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    cell: f64,
) -> Probe {
    // Ø12 ball reference — identical to `rest_grid_resolution_c9::measure`,
    // chosen there because it cannot get anywhere near these grooves, so the
    // groove IS the rest field. Same reference at every cell size.
    let reference = BallEndmill::new(12.0, 25.0);
    let rf = detect_rest_valleys(
        mesh,
        index,
        cutter,
        RestReference::Cutter {
            tool: &reference as &dyn MillingCutter,
            is_surface_probe: false,
        },
        &RestFieldParams {
            cell_mm: cell,
            min_valley_depth: 0.05,
            offset_stepover_mm: 0.25,
            num_offset_passes_cap: 8,
            min_cut_length: 2.0,
            region_margin_mm: 0.5,
        },
    );

    let mut reaches: Vec<f64> = Vec::new();
    let mut refused = 0usize;
    for cl in rf.centerlines.iter() {
        for s in cl.samples.iter() {
            if s.reach.refused {
                refused += 1;
            }
            reaches.push(s.reach.min_mm());
        }
    }
    reaches.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = if reaches.is_empty() {
        0.0
    } else {
        reaches[reaches.len() / 2]
    };

    Probe {
        cell,
        centerlines: rf.centerlines.len(),
        centerline_length_mm: rf
            .centerlines
            .iter()
            .map(|cl| polyline_length(&cl.points))
            .sum(),
        median_reach_mm: median,
        refused_points: refused,
        sampled_points: reaches.len(),
        skeleton_length_mm: rf.report.skeleton_length_mm,
        pencil_region_count: rf.report.pencil_region_count,
        clearing_region_count: rf.report.clearing_region_count,
        clearing_regions: rf.clearing_regions.len(),
        total_rest_volume_mm3: rf.report.total_rest_volume_mm3,
        traced_length_mm: rf.report.traced_length_mm,
        grid_nx: rf.report.grid_nx,
        grid_ny: rf.report.grid_ny,
    }
}

fn report(label: &str, rows: &[Probe]) {
    println!();
    println!("== E9 routing probe — {label} ==");
    println!(
        "  {:>5} | {:>4} {:>9} | {:>9} {:>10} | {:>6} {:>6} {:>6} | {:>10} {:>9} | {:>9}",
        "cell",
        "cl#",
        "cl_len",
        "skeleton",
        "routed_away",
        "pencil",
        "clr#",
        "clrReg",
        "rest_vol",
        "traced",
        "med_reach",
    );
    for m in rows {
        println!(
            "  {:>5.2} | {:>4} {:>9.3} | {:>9.3} {:>10.3} | {:>6} {:>6} {:>6} | {:>10.4} \
             {:>9.3} | {:>9.4}   grid {}x{} pts={} refused={}",
            m.cell,
            m.centerlines,
            m.centerline_length_mm,
            m.skeleton_length_mm,
            m.routed_away_mm(),
            m.pencil_region_count,
            m.clearing_region_count,
            m.clearing_regions,
            m.total_rest_volume_mm3,
            m.traced_length_mm,
            m.median_reach_mm,
            m.grid_nx,
            m.grid_ny,
            m.sampled_points,
            m.refused_points,
        );
    }
    println!(
        "  columns: cl#/cl_len = what rest_grid_resolution_c9 reads (POST-routing); \
         skeleton/routed_away/clrReg = the pre-routing seams it does not."
    );
}

/// **The headline arm.** The tip-scale groove, where c9 records "the 0.25 mm
/// and 0.10 mm cells find nothing at all".
///
/// This test does not re-assert c9's pin. It asserts the *distinction* c9
/// cannot make, in whichever direction the machine reports it, and prints the
/// full table either way.
#[test]
fn tip_scale_groove_detection_loss_is_separated_from_routing_refusal() {
    let mesh = GroovedBlock::new(0.8, 70.0, 1.2).dense_step(0.1).build();
    let index = SpatialIndex::build(&mesh, 5.0);
    let cutter = wanaka_taper();
    let rows: Vec<Probe> = CELLS
        .iter()
        .map(|&c| probe(&mesh, &index, &cutter, c))
        .collect();
    report(
        "tip-scale groove (rim half-width 0.8 mm, 70 deg walls)",
        &rows,
    );

    let fine = rows.last().expect("swept at least one cell");
    let coarse = rows.first().expect("swept at least one cell");

    // Non-vacuity: the probe must be reading the same runs c9 reads. If the
    // coarse arm stopped detecting, this file is measuring something else and
    // no verdict below is admissible.
    assert!(
        coarse.centerlines > 0 && coarse.centerline_length_mm > 1.0,
        "the shipped 0.50 mm cell found nothing — this probe is not reading \
         the same runs rest_grid_resolution_c9 reads, so its verdict is void"
    );

    // The falsification bar, pre-registered in REST_GRID_ANOMALY_STUDY.md §4
    // and §7 arm 1: at h = 0.1 the hypothesis is DETECTED-AND-REFUSED, whose
    // signature is a non-zero pre-routing skeleton beside an empty
    // post-routing centreline list. The competing hypothesis is
    // NOT-DETECTED, whose signature is skeleton ≈ 0.
    //
    // This assertion states the DISJUNCTION — exactly one of the two must
    // hold, and which one is the finding. It cannot pass vacuously: an empty
    // `centerlines` with a zero skeleton and a zero rest volume would mean
    // the field itself vanished, and that fails the third clause.
    let detected_and_refused = fine.centerlines == 0 && fine.skeleton_length_mm > 1.0;
    let not_detected = fine.centerlines == 0 && fine.skeleton_length_mm <= 1.0;
    println!(
        "\n  VERDICT at cell {:.2}: centerlines={} skeleton={:.3} mm \
         clearing_regions={} rest_volume={:.4} mm3  ->  {}",
        fine.cell,
        fine.centerlines,
        fine.skeleton_length_mm,
        fine.clearing_regions,
        fine.total_rest_volume_mm3,
        if detected_and_refused {
            "DETECTED AND REFUSED (W7 prediction CONFIRMED)"
        } else if not_detected {
            "NOT DETECTED (W7 prediction REFUTED — stage of death is ridge extraction)"
        } else {
            "the fine cell now emits centrelines — the anomaly changed"
        }
    );

    // Whatever the routing outcome, the rest FIELD must survive refinement.
    // If it does not, neither hypothesis is the right question.
    assert!(
        fine.total_rest_volume_mm3 > 0.0,
        "the rest field itself is empty at cell {:.2} mm — the mask died \
         upstream of both ridge extraction and routing, which is a third \
         mechanism neither hypothesis covers",
        fine.cell
    );

    // ── The pin ────────────────────────────────────────────────────────────
    // Measured 2026-08-05: DETECTED AND REFUSED, and the numbers are in the
    // module doc. This assertion pins the mechanism, not the digits: the fine
    // cell must keep detecting essentially all of the feature the shipped
    // cell detects, while emitting none of it as pencil. It goes red if
    // either half of that changes — if detection genuinely starts failing
    // (the hypothesis this probe refuted returning), or if the routing
    // verdict flips back to Pencil (the anomaly being fixed). Both are news;
    // neither may be absorbed silently. Rewrite this file's doc, do not
    // delete the line.
    assert!(
        detected_and_refused,
        "the tip-scale groove's fine cell no longer reads DETECTED AND \
         REFUSED: centerlines={}, skeleton={:.3} mm. Measured 2026-08-05 as \
         0 centrelines beside 18.200 mm of skeleton. Re-read the module doc \
         and rewrite it rather than deleting this assertion",
        fine.centerlines, fine.skeleton_length_mm
    );
    assert!(
        fine.skeleton_length_mm >= 0.9 * coarse.skeleton_length_mm,
        "detected skeleton fell from {:.3} mm at cell {:.2} to {:.3} mm at \
         cell {:.2} — more than the 10% a boundary-resolution effect explains. \
         That would be a genuine DETECTION loss, which is the hypothesis this \
         probe refuted on 2026-08-05 (19.000 -> 18.200 mm, -4.2%)",
        coarse.skeleton_length_mm,
        coarse.cell,
        fine.skeleton_length_mm,
        fine.cell
    );
    assert!(
        fine.clearing_regions >= 1,
        "the fine cell emitted no centrelines AND no clearing region — the \
         branch vanished from both lists, which is the E11(b) reporting blind \
         spot (`clearing_comps.insert` is guarded by `comp != usize::MAX`), \
         not the routing refusal this probe measured"
    );
}

/// The wide control, read through the same seams. `rest_grid_resolution_c9`
/// pins that detected *length* is flat here while median reach collapses
/// 0.583 → 0.242 → 0.000 across the sweep.
///
/// The pre-routing seams say whether that collapse is accompanied by any
/// change in what was traced. **Measured 2026-08-05: it is not.** Nothing is
/// routed away at any cell — detection, tracing and routing are all stable
/// while median reach falls to zero. The control's collapse is therefore a
/// **measurement** phenomenon located entirely in the cross-section/reach
/// stages, with no detection or routing component at all, which is exactly the
/// two-anomalies separation W7's §2 predicted.
#[test]
fn wide_control_reach_collapse_is_read_through_the_pre_routing_seams() {
    let mesh = GroovedBlock::new(2.5, 70.0, 1.2).dense_step(0.1).build();
    let index = SpatialIndex::build(&mesh, 5.0);
    let cutter = wanaka_taper();
    let rows: Vec<Probe> = CELLS
        .iter()
        .map(|&c| probe(&mesh, &index, &cutter, c))
        .collect();
    report(
        "wide groove control (rim half-width 2.5 mm, 70 deg walls)",
        &rows,
    );

    let coarse = rows.first().expect("swept at least one cell");
    let fine = rows.last().expect("swept at least one cell");

    // Non-vacuity: the control must still be tracing at both ends.
    assert!(
        coarse.skeleton_length_mm > 0.0 && fine.skeleton_length_mm > 0.0,
        "the control traced no skeleton at one end of the sweep ({:.3} coarse \
         / {:.3} fine) — it is no longer controlling for anything",
        coarse.skeleton_length_mm,
        fine.skeleton_length_mm
    );

    // The pin: the control has NO routing component. If routing ever starts
    // moving here, the two phenomena have stopped being separable and this
    // file's headline separation must be re-read.
    for m in &rows {
        assert!(
            m.routed_away_mm() < 1e-9,
            "the control routed {:.3} mm away at cell {:.2} — measured \
             2026-08-05 as ZERO at every cell. The control's reach collapse is \
             supposed to have no detection or routing component; if it now has \
             one, the two-anomalies separation in this file's module doc must \
             be re-read, not silently absorbed",
            m.routed_away_mm(),
            m.cell
        );
    }
    println!(
        "\n  control: skeleton {:.3} -> {:.3} mm, centreline {:.3} -> {:.3} mm, \
         routed away {:.3} -> {:.3} mm, median reach {:.4} -> {:.4} mm",
        coarse.skeleton_length_mm,
        fine.skeleton_length_mm,
        coarse.centerline_length_mm,
        fine.centerline_length_mm,
        coarse.routed_away_mm(),
        fine.routed_away_mm(),
        coarse.median_reach_mm,
        fine.median_reach_mm,
    );
}

/// **Arm 3 — the rim-sensitivity contract. The study's prediction is REFUTED
/// as literally written, and the corrected statement is sharper.**
///
/// `REST_GRID_ANOMALY_STUDY.md` §7 arm 3 pre-registered: *"Take a
/// detector-produced `LocalValley`, perturb `rim_distance_mm` by ±h, re-solve
/// with `solve_reach`. **Prediction: `dX/d(rim) = 1` exactly** — reach inherits
/// rim error one-for-one."*
///
/// The reasoning behind it was
/// `X = rim_distance − profile_rise(cutter, cot θ, δ)`, with `profile_rise`
/// *"essentially h-independent"*. **`profile_rise` is not rim-independent.**
/// `ValleySide::wall_run()` (`reach.rs:194-203`) computes the wall angle as
///
/// ```text
/// slope = wall_rise_mm / rim_distance_mm      // then run = 1/slope
/// ```
///
/// so moving `rim_distance_mm` while holding `wall_rise_mm` fixed **also moves
/// the inferred wall angle**, which moves `profile_rise`, which partly cancels
/// the rim change.
///
/// **Measured 2026-08-05** on the detector-produced valley with the most
/// clamp headroom (depth 1.0095 mm, left rim 1.500 / rise 2.3645, wall
/// 57.61°), perturbing by ±0.1457 mm:
///
/// | case | gain `dX/d(rim)` |
/// |---|---|
/// | 1 — angle-preserving | **1.000000000** |
/// | 2 — rise-preserving (the literal wording) | **0.6753 / 0.6674** |
/// | 3 — vertical wall | **1.000000000** |
///
/// **A third of the rim error cancels itself** under the literal
/// perturbation. The study's `= 1 exactly` is not what a rise-preserving rim
/// perturbation does.
///
/// # The corrected contract, in three cases
///
/// 1. **Angle-preserving perturbation** (`ValleySide::from_wall_angle`, rim and
///    rise move together): `dX/d(rim) = 1` **exactly**. This is the form in
///    which the study's claim is TRUE, and it is the one a rim *quantisation*
///    argument needs — quantising the walk length does not change the wall the
///    walk climbed.
/// 2. **Rise-preserving perturbation** (rim alone, the literal wording):
///    `0 < dX/d(rim) < 1`. Partly self-cancelling.
/// 3. **Vertical wall** (`wall_run() == 0`): `profile_rise` reduces to
///    `width_at_height(δ)`, which reads no rim at all, so `dX/d(rim) = 1`
///    **exactly** again.
///
/// # Why this matters to the anomaly, stated carefully
///
/// It does **not** overturn §3. The reach collapse still needs a mechanism
/// that *manufactured* reach at the coarse cell, and rim bias is still it. But
/// §3's arithmetic converts rim bias into reach at a gain of 1, and the true
/// gain depends on which perturbation the coarse grid actually performs — a
/// question §3 never asks. **`measure_cross_section` reports rim and rise off
/// the same walk**, so a coarse walk moves both, which is nearer case 1 than
/// case 2. The study's number therefore survives; its derivation does not, and
/// anyone re-deriving the ridge-displacement arithmetic (§7 arm 2, still
/// unrun) must state which perturbation they are assuming — the answer differs
/// by a factor of 1.5.
///
/// Note this is a **second** cancellation, independent of the one §3.3 already
/// names (`max_slope`'s single-cell secant, which inverts the other way). Two
/// separate terms partially cancel the rim bias, which is consistent with §3's
/// own observation that the observed curve is not a clean `X ∝ h`.
///
/// This is a permanent contract sentry either way: if case 1 or case 3 ever
/// stops reading exactly 1, `REST_GRID_ANOMALY_STUDY.md` §3's whole argument
/// is void.
#[test]
fn reach_sensitivity_to_rim_distance_is_gain_one_only_when_the_wall_angle_is_held() {
    use rs_cam_core::reach::{LocalValley, ValleySide, solve_reach};

    let mesh = GroovedBlock::new(2.5, 70.0, 1.2).dense_step(0.1).build();
    let index = SpatialIndex::build(&mesh, 5.0);
    let cutter = wanaka_taper();
    let reference = BallEndmill::new(12.0, 25.0);
    let rf = detect_rest_valleys(
        &mesh,
        &index,
        &cutter,
        RestReference::Cutter {
            tool: &reference as &dyn MillingCutter,
            is_surface_probe: false,
        },
        &RestFieldParams {
            cell_mm: 0.5,
            min_valley_depth: 0.05,
            offset_stepover_mm: 0.25,
            num_offset_passes_cap: 8,
            min_cut_length: 2.0,
            region_margin_mm: 0.5,
        },
    );

    // The detector-produced valley with the MOST headroom above the
    // `.max(0.0)` clamp, so every derivative below is measured in the linear
    // region rather than against the floor. Taking the best sample rather
    // than a fixed bar keeps this arm alive as the fixture's absolute reach
    // moves; the perturbation is then sized to that headroom.
    let seed_sample = rf
        .centerlines
        .iter()
        .flat_map(|cl| cl.samples.iter())
        .filter(|s| !s.reach.refused && s.reach.min_mm() > 0.0)
        .max_by(|a, b| {
            a.reach
                .min_mm()
                .partial_cmp(&b.reach.min_mm())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .copied()
        .expect(
            "the wide control produced no unrefused sample with non-zero reach \
             on both sides — the fixture, not the contract, changed",
        );
    let seed = seed_sample.valley;
    // Stay well inside the clamp on both sides: a quarter of the available
    // headroom, capped at one cell (the magnitude §3.2's quantisation reaches).
    let h = (0.25 * seed_sample.reach.min_mm()).min(0.5);
    assert!(
        h > 1e-6,
        "no usable perturbation magnitude: best reach was {:.6} mm",
        seed_sample.reach.min_mm()
    );

    let base = solve_reach(&cutter, &seed);
    let theta = seed.left.wall_angle_rad();
    println!(
        "\n== E9 arm 3 — rim sensitivity ==\n  seed valley: depth={:.4} mm, \
         left rim={:.4}/rise={:.4} (wall {:.2} deg), right rim={:.4}/rise={:.4} \
         -> reach L={:.4} R={:.4} refused={}",
        seed.rest_depth_mm,
        seed.left.rim_distance_mm,
        seed.left.wall_rise_mm,
        theta.to_degrees(),
        seed.right.rim_distance_mm,
        seed.right.wall_rise_mm,
        base.left_mm,
        base.right_mm,
        base.refused,
    );

    // ── Case 1: angle-preserving. The study's claim, in the form where it is
    //    true. Gain must be EXACTLY 1.
    for delta in [-h, h] {
        let perturbed = LocalValley {
            rest_depth_mm: seed.rest_depth_mm,
            left: ValleySide::from_wall_angle(seed.left.rim_distance_mm + delta, theta),
            right: seed.right,
        };
        // Re-solve the unperturbed side through the same constructor so the
        // baseline and the perturbation differ ONLY in rim distance.
        let control = solve_reach(
            &cutter,
            &LocalValley {
                rest_depth_mm: seed.rest_depth_mm,
                left: ValleySide::from_wall_angle(seed.left.rim_distance_mm, theta),
                right: seed.right,
            },
        );
        let gain = (solve_reach(&cutter, &perturbed).left_mm - control.left_mm) / delta;
        println!("  [case 1] angle-preserving, rim {delta:+.4} mm -> gain {gain:.9}");
        assert!(
            (gain - 1.0).abs() < 1e-9,
            "angle-preserving rim perturbation must pass through at gain \
             EXACTLY 1 (profile_rise reads only the wall RUN and the depth, \
             neither of which moves here); got {gain:.9}. If this is not 1, \
             REST_GRID_ANOMALY_STUDY.md §3's argument is void"
        );
    }

    // ── Case 2: rise-preserving — the study's LITERAL wording. Refuted.
    let mut case2 = Vec::new();
    for delta in [-h, h] {
        let perturbed = LocalValley {
            rest_depth_mm: seed.rest_depth_mm,
            left: ValleySide::new(seed.left.rim_distance_mm + delta, seed.left.wall_rise_mm),
            right: seed.right,
        };
        let gain = (solve_reach(&cutter, &perturbed).left_mm - base.left_mm) / delta;
        println!("  [case 2] rise-preserving, rim {delta:+.4} mm -> gain {gain:.9}");
        case2.push(gain);
        assert!(
            gain > 0.0 && gain < 1.0,
            "perturbing rim ALONE changes the inferred wall angle (wall_run = \
             rise/rim), so profile_rise moves too and the gain must be strictly \
             inside (0, 1); got {gain:.9}. Gain >= 1 would mean wall_run had \
             stopped reading rim_distance and the refutation recorded in this \
             test's doc no longer applies"
        );
        assert!(
            (gain - 1.0).abs() > 1e-6,
            "case 2 read gain {gain:.9}, i.e. one-for-one. That is the \
             pre-registered prediction this test REFUTES; if it now holds, \
             ValleySide::wall_run has changed and the doc must be rewritten"
        );
    }
    println!(
        "  => the study's 'dX/d(rim) = 1 exactly' holds for case 1 and is \
         REFUTED for case 2 (measured {:.4} / {:.4})",
        case2[0], case2[1]
    );

    // ── Case 3: vertical wall — profile_rise reads no rim at all.
    let vert_base = solve_reach(
        &cutter,
        &LocalValley {
            rest_depth_mm: seed.rest_depth_mm,
            left: ValleySide::vertical(seed.left.rim_distance_mm),
            right: seed.right,
        },
    );
    for delta in [-h, h] {
        let got = solve_reach(
            &cutter,
            &LocalValley {
                rest_depth_mm: seed.rest_depth_mm,
                left: ValleySide::vertical(seed.left.rim_distance_mm + delta),
                right: seed.right,
            },
        );
        let gain = (got.left_mm - vert_base.left_mm) / delta;
        println!("  [case 3] vertical wall, rim {delta:+.4} mm -> gain {gain:.9}");
        assert!(
            (gain - 1.0).abs() < 1e-9,
            "on a VERTICAL wall profile_rise is width_at_height(depth), which \
             reads no rim at all, so the gain is 1 by construction; got \
             {gain:.9}"
        );
        // And the two sides stay decoupled except through the refusal sum.
        assert!(
            (got.right_mm - vert_base.right_mm).abs() < 1e-9,
            "perturbing the LEFT rim moved the RIGHT reach by {:+.6} mm — the \
             two sides are coupled only through the `refused` sum, never \
             through the reach magnitudes",
            got.right_mm - vert_base.right_mm
        );
    }
}
