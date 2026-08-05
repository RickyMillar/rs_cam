//! Reference-fixture repeatability — the pre-registered study, executed.
//!
//! Companion to `common::reference_plate`. The specification is
//! `planning/review_2026-08-04/REFERENCE_FIXTURE_REPEATABILITY.md`; its bars
//! B-1..B-5 were written **before** any arm ran and are reproduced verbatim in
//! the doc comment of each arm below.
//!
//! # Standing: this banks evidence. It re-opens nothing.
//!
//! Checkpoint E ruled (Q2 / ask E4) that **B1/B2 stay closed this programme**.
//! The execution pass may run and bank evidence for a *future* programme; no
//! re-measurement re-opens anything here, and **no arm in this file makes a
//! strategy claim or names a quality winner**.
//!
//! The rule this file exists to enforce, from the plan's acceptance gates:
//! **bins come from repeated runs, never from a desired dial.**
//!
//! # Four things that get called "repeatability", and must not be pooled
//!
//! The prior campaign's single biggest instrument error was mixing these.
//!
//! | # | source | operation | expected | if non-zero |
//! |---|---|---|---|---|
//! | **V1** | re-run variance | same binary, same inputs, twice | **exactly 0** | a **determinism defect**, not a bin. Fix the code; do not widen the bin. |
//! | **V2** | regeneration variance | regenerate the toolpath, re-simulate | small, > 0 | **this sets the bin.** |
//! | **V3** | discretisation sensitivity | change sim cell or mesh step | large, systematic | **not variance at all.** Never pool with V1/V2; never compare across cells. |
//! | **V4** | alias / phase sensitivity | same surface, shifted against the lattice | `cell·tan θ` | sets a **floor** the bin must clear, independent of V1/V2. |
//!
//! **The minimum reportable bin = max(V2 bar, V4 bound).** V3 is excluded by
//! construction. V1 must be zero or the study stops and reports a defect.
//!
//! # Execution status, stated per arm
//!
//! | arm | bar | status |
//! |---|---|---|
//! | V1 generator determinism | B-1 | **RUN** — [`v1_generator_and_evaluators_are_bit_deterministic`] |
//! | V4 realised alias | B-3 | **RUN** — [`v4_realised_alias_bound_holds_on_every_slope_band`], analytically |
//! | B-5 non-reportable region | B-5 | **RUN** — [`b5_non_reportable_region_map`] |
//! | V2 regeneration variance | B-2 | **NOT RUN** — [`v2_regeneration_variance`], `#[ignore]`d |
//! | V3 cell sensitivity | B-4 | **NOT RUN** — [`v3_cell_sensitivity`], `#[ignore]`d |
//!
//! **V2 is the arm that sets the bin, and it is the one still NOT RUN.** So
//! **no bin is adopted by this file either** — see [`v2_regeneration_variance`]
//! for the owner and the exact command. What *is* banked is stated in
//! [`b5_non_reportable_region_map`], and it is sharper than the study's own
//! prediction.
//!
//! # Why V4 is executed analytically rather than through the simulator
//!
//! B-3's bar is *"translate the fixture by half a cell in X and re-simulate
//! without regenerating; the realised per-column difference must be
//! `≤ cell·tan θ` in every slope band."* The quantity under test is a property
//! of **the sampling lattice against the surface** — no toolpath and no
//! machining enter it. ARP-1 has an exact `z_at`, so the arm can be run
//! directly on the analytic surface, which is **stricter** than the simulated
//! version: it isolates the alias from every other source of difference
//! instead of measuring them summed. The simulated form remains available and
//! is what [`v3_cell_sensitivity`] would exercise.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

mod common;

use common::reference_plate::ReferencePlate;
use common::scallop_oracle::{SlopeBand, quantile};

/// The smallest *interesting* quality difference, from B-5: **20 µm, one fifth
/// of the shipped 0.1 mm `scallop_height` default** (`src/scallop.rs:128`), on
/// the grounds that a difference smaller than a fifth of the dial is not worth
/// a campaign.
const INTERESTING_UM: f64 = 20.0;

/// The cell ladder the study sweeps. `0.25` is the **prior** campaign's cell,
/// kept only to reproduce its alias.
const CELLS_MM: [f64; 6] = [0.50, 0.25, 0.10, 0.05, 0.02, 0.01];

// ===========================================================================
// B-1 — V1, determinism
// ===========================================================================

/// **B-1 (V1, determinism).** *"Two runs of the identical binary on identical
/// inputs must produce byte-identical output — all values, in order. Bar: `0`
/// differing entries out of the full population. Any non-zero count is
/// reported as a determinism defect with the differing indices, and the study
/// stops setting a bin until it is explained."*
///
/// Scope note, stated rather than glossed: this arm runs the bar against the
/// **fixture and its evaluators**, which is what this programme added. The
/// full-pipeline form of B-1 (two separate processes, PIDs recorded, over
/// `column_deviations`) belongs with [`v2_regeneration_variance`] and is NOT
/// RUN. A determinism defect *upstream* of the simulator would invalidate
/// every arm, so this is the right thing to run first regardless.
///
/// *Vacuity guard:* the comparison must be over a large population, and the
/// evaluators must actually be exercised at points where they do work — a
/// probe set that lands entirely on datum gutter would pass trivially.
#[test]
fn v1_generator_and_evaluators_are_bit_deterministic() {
    let plate = ReferencePlate::arp1();
    let half = plate.half_extent_mm();
    let n = 320;

    let sample = || -> Vec<u64> {
        let p = ReferencePlate::arp1();
        let mut bits = Vec::with_capacity(n * n);
        for i in 0..n {
            let x = -half + 2.0 * half * (i as f64 + 0.5) / n as f64;
            for k in 0..n {
                let y = -half + 2.0 * half * (k as f64 + 0.5) / n as f64;
                bits.push(p.z_at(x, y).to_bits());
                if let Some(s) = p.slope_deg_at(x, y) {
                    bits.push(s.to_bits());
                }
            }
        }
        bits
    };

    let a = sample();
    let b = sample();
    assert_eq!(
        a.len(),
        b.len(),
        "the two runs produced different population sizes — not a bin, a defect"
    );
    let differing: Vec<usize> = a
        .iter()
        .zip(b.iter())
        .enumerate()
        .filter(|(_, (x, y))| x != y)
        .map(|(i, _)| i)
        .collect();
    assert!(
        differing.is_empty(),
        "V1 is NOT zero: {} of {} values differ (first indices {:?}). Per B-1 \
         this is a DETERMINISM DEFECT, not a bin — fix the code, do not widen \
         the bin, and no bin may be set until it is explained",
        differing.len(),
        a.len(),
        &differing[..differing.len().min(8)]
    );

    // Vacuity guard: the probe set must actually reach the zones.
    let mut on_zone = 0usize;
    for i in 0..n {
        let x = -half + 2.0 * half * (i as f64 + 0.5) / n as f64;
        for k in 0..n {
            let y = -half + 2.0 * half * (k as f64 + 0.5) / n as f64;
            if plate.zone_at(x, y).is_some() {
                on_zone += 1;
            }
        }
    }
    println!(
        "V1: {} values compared over a {n}x{n} lattice, {} probes on zone support \
         ({:.1}%), 0 differing",
        a.len(),
        on_zone,
        100.0 * on_zone as f64 / (n * n) as f64
    );
    assert!(
        on_zone > n * n / 10,
        "only {on_zone} of {} probes landed on zone support — the population is \
         mostly datum gutter and the bar passes vacuously",
        n * n
    );
}

// ===========================================================================
// B-3 — V4, realised alias
// ===========================================================================

/// **B-3 (V4, alias, realised).** *"Translate the fixture by half a cell in X
/// and re-simulate without regenerating. The realised per-column difference
/// must be `≤ cell·tan θ` in every slope band. Bar: ≤ 1% of columns exceed the
/// bound. A larger exceedance means the bound is wrong and §1 must be
/// rewritten before any bin is adopted."*
///
/// Run analytically on ARP-1's exact surface — see the module doc for why that
/// is the stricter form. The band and the slope come from the plate's **exact
/// analytic normal**, not from a finite difference.
///
/// # Result — the bound holds, and the study's §1 wording needs one qualifier
///
/// **What is confirmed, and it is the substantive claim: the realised alias is
/// exactly proportional to the cell.** Per-band p50, halving the cell:
///
/// | band | 0.10 mm | 0.05 mm | 0.02 mm | 0.01 mm |
/// |---|---|---|---|---|
/// | shallow | 14.60 | 7.35 | 2.94 | 1.47 |
/// | mid | 49.47 | 24.72 | 9.92 | 4.97 |
/// | steep | 161.60 | 80.80 | 32.32 | 16.16 |
///
/// Every row is linear in the cell to three digits. (The 0.5 and 0.25 mm rows
/// deviate — there the lattice itself under-samples the relief, which is the
/// same effect that put the `λ_min/8` term in the tessellation rule.)
///
/// **What needed correcting — twice, and both are findings:**
///
/// 1. **`θ` is the sample's OWN slope, not the band's.** A first cut of this
///    arm compared every sample in a band against `cell·tan` of the band's
///    *lower edge* and read a 4.4% exceedance. That is an artefact of the
///    comparison: a 70° sample sitting in the 45–75° band is bounded by
///    `tan 70°`, not `tan 45°`. The bound is per-sample and this arm now
///    applies it that way.
/// 2. **The bound is a SMOOTH-GROUND bound.** It is derived from a slope, so
///    it says nothing across a **step**. On ARP-1 the step cases are the
///    terrace risers (a 1 mm jump between two surfaces whose slope is exactly
///    0 — the unmistakable signature that showed up as `max = 1000.00 µm` in
///    every shallow row) and the cone's outer rim. Those pairs are excluded
///    **structurally**, not by a threshold: a pair is excluded when its two
///    ends are in different zones, or when both ends read slope 0 and the
///    heights differ, which is a step and cannot be anything else.
///
/// **`REFERENCE_FIXTURE_REPEATABILITY.md` §1 should gain that qualifier.** It
/// is not a retraction — the alias floor stands, and the prior ±10 µm bin is
/// still below it by 25–93× — but a COLUMNS instrument run on real relief has
/// no way to make the smooth/step distinction this arm makes analytically, so
/// **its realised alias will exceed `cell·tan θ` wherever the part has a
/// step.** For a gate that is the conservative direction (the true floor is
/// higher than the formula), and it is one more reason the reportable region
/// in [`b5_non_reportable_region_map`] is a floor rather than a bin.
#[test]
fn v4_realised_alias_bound_holds_on_every_slope_band() {
    let plate = ReferencePlate::arp1();
    let half = plate.half_extent_mm();

    println!(
        "\n{:>6} {:>10} | {:>10} {:>10} {:>10} {:>8} {:>9}",
        "cell", "band", "p50 um", "p99 um", "max um", "n", "exceed"
    );
    let mut total_exceed = 0usize;
    let mut total_cols = 0usize;
    let mut cross_zone = 0usize;
    let mut risers = 0usize;

    for cell in CELLS_MM {
        // Two lattices over the same surface, offset by half a cell in X.
        // Everything else — the surface, the zones, the rotations — identical.
        let mut per_band: Vec<Vec<f64>> = vec![Vec::new(); 3];
        let mut band_exceed = [0usize; 3];
        let nx = ((2.0 * half) / cell).floor() as usize;
        // Cap the population so the coarse and fine cells cost comparably;
        // the quantiles are stable long before this bites.
        let stride = (nx / 400).max(1);
        let ny = 120;
        for i in (0..nx).step_by(stride) {
            let x = -half + (i as f64 + 0.5) * cell;
            for k in 0..ny {
                let y = -half + 2.0 * half * (k as f64 + 0.5) / ny as f64;
                let xs = x + 0.5 * cell;
                let (Some(band), Some(s0)) = (plate.band_at(x, y), plate.slope_deg_at(x, y)) else {
                    continue;
                };
                let Some(s1) = plate.slope_deg_at(xs, y) else {
                    continue;
                };
                let (z0, z1) = (plate.z_at(x, y), plate.z_at(xs, y));
                if !z0.is_finite() || !z1.is_finite() {
                    continue;
                }

                // POPULATION, stated. The `cell·tan θ` bound is derived from a
                // SLOPE and says nothing across a STEP, so pairs that straddle
                // a discontinuity are excluded — structurally, never by a
                // threshold on the very quantity under test.
                if plate.zone_at(x, y) != plate.zone_at(xs, y) {
                    cross_zone += 1;
                    continue;
                }
                if s0 < 1e-9 && s1 < 1e-9 && (z1 - z0).abs() > 1e-9 {
                    // Two surfaces of slope exactly zero at different heights:
                    // a terrace riser. This cannot be anything but a step.
                    risers += 1;
                    continue;
                }

                let d_um = (z1 - z0).abs() * 1000.0;
                // The bound at THIS sample's own slope. Mean value theorem
                // over the half-cell shift: |Δz| ≤ (cell/2)·max|∇z| on the
                // segment, so `cell·tan(max endpoint slope)` carries a factor
                // of 2 in hand.
                let bound_um = cell * s0.max(s1).to_radians().tan() * 1000.0;
                let idx = match band {
                    SlopeBand::Shallow => 0,
                    SlopeBand::MidSteep => 1,
                    SlopeBand::VerySteep => 2,
                };
                if d_um > bound_um {
                    band_exceed[idx] += 1;
                    total_exceed += 1;
                }
                per_band[idx].push(d_um);
                total_cols += 1;
            }
        }

        for (idx, band) in SlopeBand::ALL.iter().enumerate() {
            let v = &mut per_band[idx];
            if v.is_empty() {
                continue;
            }
            v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            println!(
                "{cell:>6.2} {:>10} | {:>10.2} {:>10.2} {:>10.2} {:>8} {:>9}",
                match band {
                    SlopeBand::Shallow => "shallow",
                    SlopeBand::MidSteep => "mid",
                    SlopeBand::VerySteep => "steep",
                },
                quantile(v, 0.50),
                quantile(v, 0.99),
                v.last().copied().unwrap_or(0.0),
                v.len(),
                band_exceed[idx],
            );
        }
    }

    let rate = 100.0 * total_exceed as f64 / total_cols.max(1) as f64;
    println!(
        "\nB-3: {total_exceed} of {total_cols} in-population samples exceeded \
         their OWN cell*tan(theta) ({rate:.3}%), bar is <= 1%."
    );
    println!(
        "     Excluded as straddling a discontinuity, structurally: \
         {cross_zone} cross-zone pairs and {risers} terrace risers. The bound \
         is slope-derived and does not apply across a step — a COLUMNS \
         instrument on real relief cannot make that distinction, so its \
         realised alias WILL exceed cell*tan(theta) at every step in the part. \
         That is the conservative direction for a gate."
    );
    assert!(
        rate <= 1.0,
        "B-3 FAILED: {rate:.3}% of smooth-ground samples exceeded their own \
         cell*tan(theta) bound (bar <= 1%). Per the pre-registered bar this \
         means THE BOUND IS WRONG and REFERENCE_FIXTURE_REPEATABILITY.md §1 \
         must be rewritten before any bin is adopted"
    );
    assert!(
        risers > 0,
        "no terrace risers were excluded — the step population is empty, so \
         this arm is no longer demonstrating the smooth-ground qualifier it \
         exists to report"
    );
}

// ===========================================================================
// B-5 — the non-reportable region
// ===========================================================================

/// **B-5 (non-repeatable region).** *"Report the slope band and cell
/// combinations where the required bin exceeds the smallest interesting
/// quality difference — taken as 20 µm, one fifth of the shipped 0.1 mm
/// `scallop_height` default. Those combinations are declared non-reportable
/// and no gate may be written on them. Prediction, from §1 and to be checked:
/// **VerySteep at every cell ≥ 0.05 mm is non-reportable**, and MidSteep is
/// non-reportable at ≥ 0.10 mm."*
///
/// # Result — both predictions hold, and VerySteep's is UNDER-stated
///
/// The floor is `cell·tan θ`, which varies **within** a band, so a per-band
/// verdict has to say at which angle. This arm reports **both edges** of every
/// band and declares a band non-reportable only when even its **most
/// favourable** (shallowest) angle exceeds the interesting difference — the
/// conservative direction.
///
/// | | most favourable angle | non-reportable at |
/// |---|---|---|
/// | Shallow | 0° (floor 0) | never, at its shallowest; ≥ 0.05 mm at its 45° edge |
/// | MidSteep | 45° | **every cell ≥ 0.05 mm** |
/// | VerySteep | 75° | **EVERY cell in the range, down to 0.01 mm** |
///
/// * **VerySteep is non-reportable at every cell studied**, including 0.01 mm,
///   where its most favourable angle still gives a 37 µm floor. W7 predicted
///   "≥ 0.05 mm"; the true statement is stronger and has no upper end in the
///   range anyone would run.
/// * **MidSteep** is non-reportable at ≥ 0.05 mm at its shallowest edge, which
///   subsumes the predicted ≥ 0.10 mm. At its 75° edge it is non-reportable
///   everywhere, like VerySteep.
/// * The only combinations that clear a 20 µm bin are **shallow ground at a
///   cell of 0.02 mm or finer** — and **nothing in this repo has ever
///   simulated finer than 0.1 mm.**
///
/// This is the study's most useful banked result and it is a **floor**, not a
/// bin: it holds independently of V2, because `max(V2, V4) ≥ V4`. A future
/// programme that measures V2 can only make the reportable region smaller.
///
/// The consequence — a recommendation, not a ruling: any future fine-quality
/// gate must either run a much finer cell than has ever been run here, or
/// restrict itself to shallow ground, or **use the analytic envelope oracle
/// instead of COLUMNS**. The third option is the cheap one, and ARP-1 now
/// makes it available.
#[test]
fn b5_non_reportable_region_map() {
    println!(
        "\nB-5 non-reportable region (bin floor = cell*tan(theta), interesting \
         difference = {INTERESTING_UM:.0} um = 1/5 of the shipped 0.1 mm scallop dial)\n"
    );
    // (label, most-favourable angle in the band, steepest angle in the band)
    let bands = [
        ("shallow  0-45", 0.0_f64, 45.0_f64),
        ("mid     45-75", 45.0, 75.0),
        ("steep   75-90", 75.0, 85.0),
    ];
    println!(
        "{:>6} | {:^30} | {:^30} | {:^30}",
        "cell", bands[0].0, bands[1].0, bands[2].0
    );
    println!(
        "{:>6} | {:^30} | {:^30} | {:^30}",
        "", "favourable / steepest", "favourable / steepest", "favourable / steepest"
    );

    for cell in CELLS_MM {
        let mut cols = Vec::new();
        for (_, lo_ang, hi_ang) in bands {
            let lo = cell * lo_ang.to_radians().tan() * 1000.0;
            let hi = cell * hi_ang.to_radians().tan() * 1000.0;
            cols.push(format!(
                "{lo:>8.1} /{hi:>9.1} um {:>7}",
                if lo <= INTERESTING_UM { "part" } else { "NONE" }
            ));
        }
        println!("{cell:>6.2} | {} | {} | {}", cols[0], cols[1], cols[2]);
    }
    println!(
        "  'part' = SOME of the band clears the bin at this cell; 'NONE' = even \
         the band's shallowest ground does not."
    );

    // ── The two pre-registered predictions, adjudicated at the band's most
    //    favourable angle (the conservative direction).
    for cell in CELLS_MM.iter().filter(|c| **c >= 0.05) {
        let steep = cell * 75.0_f64.to_radians().tan() * 1000.0;
        assert!(
            steep > INTERESTING_UM,
            "B-5 prediction FAILED: VerySteep at cell {cell} has a floor of \
             {steep:.1} um at its SHALLOWEST angle, inside the \
             {INTERESTING_UM:.0} um interesting difference — the prediction \
             said it would be non-reportable"
        );
    }
    for cell in CELLS_MM.iter().filter(|c| **c >= 0.10) {
        let mid = cell * 45.0_f64.to_radians().tan() * 1000.0;
        assert!(
            mid > INTERESTING_UM,
            "B-5 prediction FAILED: MidSteep at cell {cell} has a floor of \
             {mid:.1} um at its SHALLOWEST angle — the prediction said it \
             would be non-reportable"
        );
    }

    // ── The stronger statement this arm actually measured: VerySteep never
    //    clears the bin anywhere in the studied range.
    let finest = CELLS_MM.iter().copied().fold(f64::INFINITY, f64::min);
    let steep_at_finest = finest * 75.0_f64.to_radians().tan() * 1000.0;
    println!(
        "\nBANKED: VerySteep's floor at the FINEST cell studied ({finest:.2} mm) \
         is {steep_at_finest:.1} um, still {:.1}x the {INTERESTING_UM:.0} um \
         interesting difference.",
        steep_at_finest / INTERESTING_UM
    );
    assert!(
        steep_at_finest > INTERESTING_UM,
        "VerySteep became reportable at cell {finest} ({steep_at_finest:.1} um) \
         — this arm's banked conclusion has changed and its doc must be \
         rewritten"
    );
    println!(
        "  => MidSteep and VerySteep are non-reportable at EVERY cell >= 0.05 mm, \
         which SUBSUMES the pre-registered >= 0.05 / >= 0.10 prediction; \
         VerySteep additionally never clears the bin at ANY cell in the range. \
         The reportable region at a {INTERESTING_UM:.0} um bin is shallow ground \
         at <= 0.02 mm, and nothing in this repo has ever simulated that fine."
    );

    // ── And the fixture is NOT the limit. This is the row that says effort
    //    belongs on the instrument's cell, not on the mesh.
    let plate = ReferencePlate::arp1().with_tess_epsilon(0.001);
    let worst = common::reference_plate::Zone::ALL
        .iter()
        .filter_map(|z| plate.measured_tess_error(*z))
        .fold(0.0_f64, |m, e| m.max(e.p99_um));
    // The tightest floor anyone would actually run against: 0.05 mm cell on
    // 45-degree ground. (0.01 mm has never been simulated in this repo.)
    let practical_floor = 0.05 * 45.0_f64.to_radians().tan() * 1000.0;
    println!(
        "\n  fixture check: worst zone tessellation p99 = {worst:.4} um at \
         eps = 1 um, vs a {practical_floor:.1} um alias floor at the finest \
         cell anyone would run (0.05 mm / 45 deg). Ratio {:.0}x. THE MESH IS \
         NOT THE BINDING LIMIT; THE INSTRUMENT IS.",
        practical_floor / worst.max(1e-9)
    );
    assert!(
        worst * 5.0 < practical_floor,
        "tessellation error ({worst:.4} um) is no longer comfortably below the \
         practical alias floor ({practical_floor:.1} um) — refining the fixture \
         would start to buy something again, and §3 of the repeatability study \
         must be re-read before any bin is discussed"
    );
}

// ===========================================================================
// B-2 / B-4 — NOT RUN
// ===========================================================================

/// **B-2 (V2, regeneration). NOT RUN.**
///
/// *"Regenerate the toolpath 5× from the same project and simulate each.
/// Report the per-column p50/p95/p99/max of `|z_run_i − z_run_1|`. **The
/// minimum reportable bin is the p99 of this distribution, rounded UP to the
/// next value in {5, 10, 20, 50, 100, 200} µm.** This number is the
/// deliverable; it is not permitted to be chosen to fit a desired dial.
/// Vacuity guard: if V2's p99 is `0.000`, the generator is fully deterministic
/// and the bin is set by V4 alone — that is a legitimate outcome and must be
/// stated as such, not treated as a failed measurement."*
///
/// **This is the arm that sets the bin, and it is the one still not run.**
/// It needs the full generate→simulate pipeline over ARP-1 five times with
/// `column_deviations` collected, which is a T3-tier cost (minutes+) and needs
/// the machine's single Cargo slot uncontended.
///
/// **Owner:** a scheduled execution pass holding the Cargo slot.
/// **Re-open condition:** none needed — it is pre-registered above and may be
/// run unchanged at any time. Nothing about it is blocked on a ruling.
///
/// **It does not gate anything today.** Checkpoint E ruled B1/B2 stay closed;
/// the bin this would produce is for a future programme. The
/// [`b5_non_reportable_region_map`] floor holds without it, because
/// `max(V2, V4) ≥ V4`.
#[test]
#[ignore = "T3 tier: 5x full generate+simulate over ARP-1; needs an uncontended Cargo slot"]
fn v2_regeneration_variance() {
    panic!(
        "NOT RUN by design — see this test's doc comment. Running it is a \
         scheduled T3-tier pass, not a CI action, and it must implement B-2's \
         bar exactly as written: 5 regenerations, per-column |z_i - z_1|, p99 \
         rounded UP to the next of {{5,10,20,50,100,200}} um, and a p99 of \
         0.000 reported as 'the generator is deterministic and V4 sets the \
         bin' rather than as a failed measurement."
    );
}

/// **B-4 (V3, cell sensitivity, characterization only). NOT RUN.**
///
/// *"Sweep cell ∈ {0.25, 0.10, 0.05} mm. **No bar** — this arm exists to
/// document the shape and to make the 'never compare across cells' rule
/// concrete with numbers. It must **not** be used to choose a bin."*
///
/// V3 is **not variance at all**: it is deterministic and reproducible. Never
/// pool it with V1/V2. The standing rule it makes concrete — *never clear
/// collisions across mismatched resolutions* — was learned the hard way when a
/// live GUI auto-sim at 0.1 mm reported 20 collisions that a headless run at
/// 0.5 mm reported as 0.
///
/// **Owner:** the same scheduled pass as [`v2_regeneration_variance`].
#[test]
#[ignore = "T3 tier: characterization sweep; no bar, must never be used to choose a bin"]
fn v3_cell_sensitivity() {
    panic!(
        "NOT RUN by design — see this test's doc comment. B-4 carries NO BAR \
         and its output must never be used to choose a bin."
    );
}
