//! **G-CHIP-ULP — the chipload gate's verdict at the band boundary.**
//!
//! Ledger row `TECH_DEBT_2_CLOSEOUT.md` §4 (W10-LV, 2026-08-07/08), live
//! on wanaka's Unified Finish at a 0.1 mm tip-matched cell:
//!
//! ```text
//! observed 0.012781077012073162  vs  band max 0.012781077012073160  →  Exceeds(High)
//! printed as:  "0.012781 vs 0.012781 → Exceeds"
//! ```
//!
//! This file pins the mechanism, which is a **chain of four separate
//! facts** that only produce a defect together. Each has its own test
//! below and its own Checkpoint K question.
//!
//! ## Rider 1 — the floor==ceiling collision (why the feed is *on* the bound)
//!
//! `feeds::effective_rubbing_floor` is `min(RUBBING_FLOOR_MM_TOOTH,
//! derated_band_max)`. When the whole derated band sits under 0.025
//! mm/tooth — routine on sub-Ø2 tools — the `min` returns the **band
//! ceiling**, and Suggest's Step-9b clamp (`feeds/mod.rs:1552-1562`)
//! then sets `feed = floor × rpm × flutes`. The recipe is therefore
//! parked *exactly on the breakage-side bound*, by design, with zero
//! headroom. Nothing is wrong with any single step; the ruling that
//! subordinated the floor to the band (2026-08-06) was correct. The
//! consequence is that the boundary comparison becomes load-bearing.
//!
//! **Ruling R4 Q9 (2026-09-24) retires rider 1.** The floor is now
//! `min(0.025, band min)`: a band with a minimum below 0.025 puts the floor
//! on that minimum, the opposite end of the band. The floor==ceiling
//! collision needs a band with no minimum. Rider 3's two quantities now
//! coincide on such a band.
//!
//! ## Rider 2 — the observation is a float ROUND-TRIP, so the side is noise
//!
//! Suggest multiplies (`feed = fpt × rpm × flutes`); the gate divides
//! (`observed = effective_feed / (rpm × flutes)`,
//! `tool_load/chipload.rs:162-168`). `(x·a)/a` is not `x` in binary
//! floating point. Measured below across a 291-value RPM grid × 4 flute
//! counts: for the ledger's own band maximum the round-trip lands
//! **strictly above** the bound in 89 of 1164 combinations (7.6 %) and
//! strictly below in 81. The gate's comparison is `observed > max ×
//! (1 + tolerance.breakage)` with `ToleranceBands::default()` all-zeros,
//! so those 7.6 % are `Exceeds(High)` — decided by the rounding of a
//! multiply/divide pair, not by anything about the cut.
//!
//! ## Rider 3 — no `BindingConstraint` variant names this clamp
//!
//! The feed was parked by Suggest's rubbing-floor clamp. The surface
//! that reports *why a feed is where it is* — `ModulationSummary::
//! binding_constraint_distribution`, over `tool_load::BindingConstraint`
//! — has no variant for it. `ChiploadMin` is the closest and is **not**
//! the same quantity: it is `band.min × rpm × flutes`
//! (`feed_modulation.rs:470`), while the clamp applies
//! `effective_rubbing_floor` — which in this regime equals `band.MAX`.
//! Its doc comment nonetheless calls it "the rubbing floor" (rule 5:
//! a changed instrument makes its own docstring a lie you then cite).
//!
//! ## Rider 4 — the cross-gate boundary contract
//!
//! The ledger records the semantics as *disagreeing* (drill `plunge_feed`
//! observed==lower bound ⇒ `Within`, chipload observed==upper bound ⇒
//! `Exceeds`). **Measured here, that is not what the code does**: every
//! shipped gate is `Within` at exact equality. The live divergence was
//! rider 2 — 1 ulp above, not equal. What genuinely differs across the
//! gates is (a) whether an epsilon dial exists at all and (b) whether the
//! observation reaches the bound through a numerically stable path. The
//! table is in `cross_gate_boundary_semantics_at_exact_equality`.
//!
//! Measured 2026-08-13, branch `tech-debt-3`, parent `64f017a4`, dev
//! profile. Nothing in A-6 changed behaviour — A-6 was a research wave.
//!
//! # A-7 status, 2026-08-13 — riders 2 and 4 are DISCHARGED
//!
//! Checkpoint K (b1) landed [`rs_cam_core::tool_load::boundary`]: bounds
//! are inclusive with a relative epsilon of 8 ulp, stated on
//! `ChipBounds` as methods, and every gate comparison in the crate goes
//! through them. The two assertions that pinned the defect are
//! **inverted in place** rather than deleted — a fixture that once
//! caught a defect is the thing that catches its return:
//!
//! - `rider2_the_gate_flips_to_exceeds_on_a_feed_parked_on_its_own_ceiling`
//!   read **Exceeds at 12 of 181** RPM values; it now reads **0 of 181**,
//!   and a companion sweep proves a genuine **5 %-over** feed still trips
//!   at all 181, so the epsilon is not a tolerance in disguise.
//! - `cross_gate_boundary_semantics_at_exact_equality`'s "+1 ulp trips"
//!   row is now "+1 ulp absorbed", with a "+9 ulp still trips" row
//!   beside it bounding the slack.
//!
//! Riders 2b and 3 are **unchanged and still true**. Rider 1 changed at
//! ruling R4 Q9: the floor now sits on the band minimum, not the ceiling.
//! The band still moves 14.16 % across a DOC sweep, and no
//! `BindingConstraint` variant names the clamp — Checkpoint K (d2)
//! deliberately chose the *recipe* record over the modulator's
//! vocabulary, so rider 3's exhaustive match is expected to keep
//! compiling. (d2) also added a clamp record on the commanded stage and a
//! "clamped, not exceeds" advisory on `ChiploadVerdict::Within`. Ruling R4
//! WP2a (2026-09-23) removed the lift, so no recipe is clamped. WP2b
//! deleted the record and the advisory. A feed ON the ceiling reads
//! `Within`, and the boundary epsilon above keeps it there.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::feeds::vendor_lut::{
    EvidenceGrade, LutOperationFamily, LutPassRole, ObservationKind, VendorLut,
};
use rs_cam_core::feeds::{
    ChiploadBounds, FeedsInput, FeedsResult, FeedsWarning, OperationFamily, PassRole,
    RUBBING_FLOOR_MM_TOOTH, RubbingFloorSource, SetupContext, SpindleStrategy, ToolGeometryHint,
    calculate, effective_rubbing_floor, embedded_vendor_lut, rubbing_floor,
};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::machine::kinematics::PredictedFeedMap;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::stock::simulation_cut::{
    Engagement, SimulationCutSample, SimulationCutSummary, SimulationCutTrace,
};
use rs_cam_core::tool::{MillingCutter, TaperedBallEndmill, ToolDefinition};
use rs_cam_core::tool_load::verdict::{ChipSide, ChiploadVerdict};
use rs_cam_core::tool_load::{BindingConstraint, GateEnv, ToleranceBands, ToolpathLoadContext};

/// The band maximum the ledger's live wanaka reading compared against,
/// verbatim from `TECH_DEBT_2_CLOSEOUT.md` §4 G-CHIP-ULP. Used as-is so
/// the round-trip census below is about **the observed defect**, not a
/// re-derived approximation of it.
const LEDGER_BAND_MAX: f64 = 0.012_781_077_012_073_16;

/// A one-row LUT whose derated band sits wholly below the 0.025 mm/tooth
/// floor. Feeds matrix R5 (2026-09-23) replaced the reduced Amana rows the
/// live B3 cell used to match with the printed Onsrud 77-100 rows
/// (0.0762-0.127 mm/tooth at 1/8 in), so no embedded wood row scales below
/// the floor any more. The subordination rule this file pins is unchanged,
/// so its fixture is now explicit: the printed Onsrud scallop row, cloned,
/// re-labelled derived/c, with the ledger's B3 band (0.00378-0.00756) as
/// its bounds.
fn sub_floor_lut() -> VendorLut {
    let mut row = embedded_vendor_lut()
        .observations
        .iter()
        .find(|o| o.observation_id == "onsrud-hardwood-77-100-1_8-scallop")
        .expect("the printed Onsrud 77-100 scallop row exists")
        .clone();
    row.observation_id = "synthetic-b3-sub-floor-scallop".to_owned();
    row.diameter_mm = Some(1.0);
    row.flute_count = 2;
    row.chipload_min_mm_tooth = Some(0.003_78);
    row.chipload_max_mm_tooth = Some(0.007_56);
    row.row_kind = ObservationKind::Derived;
    row.evidence_grade = EvidenceGrade::C;
    row.notes = Some(
        "test fixture: the ledger B3 band on a cloned printed row; not a vendor figure".to_owned(),
    );
    VendorLut {
        observations: vec![row],
    }
}

/// The B3 reference operation (`tests/rubbing_floor_warns_and_never_lifts.rs`,
/// `tests/feed_explanation_snapshot_b3.rs`): Ø1 tapered ball, 2 flutes,
/// scallop finish in hard maple. Its derated band is entirely below the
/// 0.025 rubbing floor, so it is the canonical floor==ceiling case.
fn b3_scallop() -> FeedsResult {
    calculate(&FeedsInput {
        tool_diameter: 1.0,
        flute_count: 2,
        flute_length: 20.0,
        shank_diameter: Some(6.0),
        tool_geometry: ToolGeometryHint::TaperedBall {
            tip_radius: 0.5,
            taper_angle_deg: 5.26,
        },
        material: &Material::SolidWood {
            species: WoodSpecies::HardMaple,
        },
        machine: &MachineProfile::generic_wood_router(),
        operation: OperationFamily::Scallop,
        operation_kind: None,
        pass_role: PassRole::Finish,
        axial_depth_mm: Some(0.35),
        radial_width_mm: None,
        target_scallop_mm: Some(0.01),
        vendor_lut: Some(&sub_floor_lut()),
        setup: SetupContext::default(),
        spindle_strategy: SpindleStrategy::MatchChart,
    })
}

// ---------------------------------------------------------------------
// Rider 1 — floor == ceiling, and the recipe parks on the bound
// ---------------------------------------------------------------------

/// RE-BLESSED 2026-09-23 (ruling R4 WP2a), then 2026-09-24 (ruling R4 Q9).
/// WP2a removed the lift, so no recipe parks on the floor. Q9 moved the floor
/// to `min(0.025, band min)`, so on this band (0.00378–0.00756, derated) the
/// floor is the band MINIMUM, not the ceiling. The computed advance is the
/// band midpoint (1.5 x the minimum) times the feed factors (x 0.75 before R4
/// WP3), at least 1.125 x the minimum, so it sits inside the band and no
/// warning fires. Rider 1 as a live mechanism is retired; riders 2 and 4
/// stay, because an operator can still type a feed onto the bound.
#[test]
fn rider1_the_floor_sits_on_the_band_minimum_and_the_recipe_does_not_warn() {
    let result = b3_scallop();
    let band: ChiploadBounds = result
        .chipload_bounds
        .expect("the B3 row publishes a chipload band");
    let fpt = result.feed_rate_mm_min / (result.rpm * 2.0);
    let floor = effective_rubbing_floor(Some(band));

    println!(
        "B3 floor: rpm={:.0} feed={:.6} fpt={:.17} band={:.17}..{:.17} \
         global_floor={RUBBING_FLOOR_MM_TOOTH} effective_floor={floor:.17}",
        result.rpm, result.feed_rate_mm_min, fpt, band.min_mm_per_tooth, band.max_mm_per_tooth,
    );

    assert!(
        band.max_mm_per_tooth < RUBBING_FLOOR_MM_TOOTH,
        "fixture precondition: band max {:.6} must sit below the global floor",
        band.max_mm_per_tooth
    );
    assert_eq!(
        rubbing_floor(Some(band)),
        (band.min_mm_per_tooth, RubbingFloorSource::BandMinimum),
        "the floor must sit on the band MINIMUM (ruling R4 Q9)"
    );
    assert!(
        !result
            .warnings
            .iter()
            .any(|w| matches!(w, FeedsWarning::ChiploadBelowRubbingFloor { .. })),
        "a recipe inside the band must not warn: {:?}",
        result.warnings
    );
    let computed = result.derates.effective_chip_load_mm();
    assert!(
        (fpt - computed).abs() <= computed * 1e-9,
        "the recipe must ship its computed advance {computed:.17}; got fpt {fpt:.17}"
    );
    assert!(
        fpt >= band.min_mm_per_tooth && fpt < band.max_mm_per_tooth,
        "no lift, so the advance {fpt:.17} sits inside the band {:.17}..{:.17}",
        band.min_mm_per_tooth,
        band.max_mm_per_tooth
    );
}

// ---------------------------------------------------------------------
// Rider 2 — the round trip decides the side
// ---------------------------------------------------------------------

/// Suggest multiplies, the gate divides. Census the reconstruction error
/// over the machine's realistic RPM range for the ledger's own band max
/// and for the live B3 band max.
fn round_trip_census(max: f64) -> (usize, usize, usize, Option<(f64, u32)>) {
    let (mut above, mut exact, mut below) = (0usize, 0usize, 0usize);
    let mut first_trip = None;
    for rpm_hundreds in 10..=300u32 {
        let rpm = f64::from(rpm_hundreds) * 100.0;
        for flutes in 1..=4u32 {
            let divisor = rpm * f64::from(flutes);
            let observed = (max * divisor) / divisor;
            if observed > max {
                above += 1;
                if first_trip.is_none() {
                    first_trip = Some((rpm, flutes));
                }
            } else if observed < max {
                below += 1;
            } else {
                exact += 1;
            }
        }
    }
    (above, exact, below, first_trip)
}

#[test]
fn rider2_the_feed_round_trip_lands_on_both_sides_of_the_bound() {
    for (label, max) in [
        ("ledger wanaka Unified Finish", LEDGER_BAND_MAX),
        (
            "live B3 scallop",
            b3_scallop()
                .chipload_bounds
                .expect("B3 band")
                .max_mm_per_tooth,
        ),
    ] {
        let (above, exact, below, first) = round_trip_census(max);
        let total = above + exact + below;
        println!(
            "{label}: max={max:.17}  round-trip ABOVE {above}/{total} ({:.1} %)  exact {exact}  \
             below {below}  first trip {first:?}",
            above as f64 / total as f64 * 100.0
        );
        assert!(
            above > 0,
            "{label}: no (rpm, flutes) combination reconstructs ABOVE the bound. Either the \
             observation stopped being a multiply/divide round trip (good — say so and retire \
             this pin) or the census stopped covering the unstable region."
        );
        assert!(
            below > 0,
            "{label}: the round trip must land on BOTH sides — that is what makes the verdict \
             noise rather than a measurement"
        );
    }
}

/// Build a one-sample steady-state trace whose achieved advance per
/// tooth is exactly `feed / (rpm × flutes)` for the given feed.
fn single_sample_trace(
    toolpath_id: ToolpathId,
    feed_mm_min: f64,
    rpm: u32,
    flutes: u32,
    axial_doc_mm: f64,
) -> SimulationCutTrace {
    let mut predicted_feeds = PredictedFeedMap::new();
    predicted_feeds.insert((toolpath_id, 0), feed_mm_min);
    let sample = SimulationCutSample {
        toolpath_id,
        move_index: 0,
        sample_index: 0,
        segment_time_s: 0.1,
        is_cutting: true,
        feed_rate_mm_min: feed_mm_min,
        spindle_rpm: rpm,
        flute_count: flutes,
        axial_doc_mm,
        axial_engagement_mm: axial_doc_mm,
        arc_engagement_radians: Some(1.0),
        // Vestigial sample-validity predicate: the gate still requires a
        // chip value to be present even though it no longer reads one
        // (`chipload.rs:723`). Any positive value keeps the sample in the
        // population without steering the observation.
        effective_chip_thickness_mm: Some(0.01),
        engagement: Engagement::with_radial_woc(0.5),
        removed_volume_est_mm3: 0.1,
        mrr_mm3_s: 1.0,
        ..SimulationCutSample::test_fixture()
    };
    SimulationCutTrace {
        predicted_feeds,
        sample_step_mm: 1.0,
        summary: SimulationCutSummary {
            sample_count: 1,
            toolpath_count: 1,
            cutting_runtime_s: 1.0,
            total_runtime_s: 1.0,
            average_engagement: 0.5,
            peak_axial_doc_mm: axial_doc_mm,
            ..SimulationCutSummary::default()
        },
        samples: vec![sample],
        ..SimulationCutTrace::test_fixture()
    }
}

fn b3_tool(flutes: u32) -> ToolDefinition {
    ToolDefinition::new(
        Box::new(TaperedBallEndmill::new(1.0, 5.26, 6.0, 20.0)),
        6.0,
        30.0,
        20.0,
        40.0,
        flutes,
        ToolMaterial::Carbide,
    )
}

fn gate_verdict(feed: f64, rpm: u32, flutes: u32, axial_doc_mm: f64) -> ChiploadVerdict {
    let tool = b3_tool(flutes);
    let material = Material::SolidWood {
        species: WoodSpecies::HardMaple,
    };
    let id = ToolpathId(0);
    let trace = single_sample_trace(id, feed, rpm, flutes, axial_doc_mm);
    let tolerance = ToleranceBands::default();
    rs_cam_core::tool_load::chipload::evaluate(
        &ToolpathLoadContext {
            toolpath_id: id,
            tool: &tool,
            material: &material,
            operation_family: LutOperationFamily::Scallop,
            pass_role: LutPassRole::Finish,
            operation_feed_rate_mm_min: feed,
            operation_kind: OperationType::Scallop,
            spans: None,
            drill_op: None,
        },
        &GateEnv {
            sim_trace: Some(&trace),
            machine: None,
            tolerance: &tolerance,
        },
    )
}

fn verdict_bounds(v: &ChiploadVerdict) -> (Option<f64>, f64) {
    match v {
        ChiploadVerdict::Within {
            approach_to_max, ..
        } => (
            approach_to_max.bounds.min_mm_per_tooth,
            approach_to_max.bounds.max_mm_per_tooth,
        ),
        ChiploadVerdict::Exceeds { triggering, .. } => (
            triggering.bounds.min_mm_per_tooth,
            triggering.bounds.max_mm_per_tooth,
        ),
        ChiploadVerdict::Unmodeled { reason } => {
            panic!("gate refused instead of judging: {reason:?}")
        }
    }
}

/// **The defect, end to end.** Ask the gate what its own band is, park
/// the feed exactly on the ceiling the way Suggest's clamp does, and
/// show that a verdict flips on the reconstruction of a number the
/// operator set to be *inside* the band.
#[test]
fn rider2_the_gate_flips_to_exceeds_on_a_feed_parked_on_its_own_ceiling() {
    let (rpm_probe, flutes, axial_doc) = (18_500u32, 2u32, 0.35);
    // Pass 1: read the gate's OWN band. No mirror of the query
    // construction — the gate publishes the bounds it used.
    let probe = gate_verdict(100.0, rpm_probe, flutes, axial_doc);
    let (band_min, band_max) = verdict_bounds(&probe);
    println!("gate band for the B3 scallop op: {band_min:?} .. {band_max:.17}");
    // Feeds matrix R5 (2026-09-23): the gate reads the embedded LUT, and the
    // B3 cell now resolves to the printed Onsrud 77-100 1/8 in row scaled to
    // Ø1 (about 0.040-0.056 mm/tooth), which sits ABOVE the 0.025 floor. The
    // floor-on-ceiling collision this rider measured is therefore not
    // reachable through any embedded wood row. The pin below records that;
    // the census that follows still holds on the gate's own ceiling.
    assert!(
        band_max >= RUBBING_FLOOR_MM_TOOTH,
        "the embedded LUT again holds a wood row whose derated max {band_max:.6} sits below \
         the 0.025 floor; the floor-on-ceiling collision is reachable again — re-read \
         G-CHIP-ULP rider 2 before citing it"
    );

    // Pass 2: sweep RPM for a combination where `(max × rpm × flutes) /
    // (rpm × flutes)` reconstructs ABOVE `max` — i.e. where Suggest's
    // clamp hands the gate a feed the gate then reads as over its own
    // ceiling.
    let mut flipped: Vec<(u32, f64, f64)> = Vec::new();
    let mut held: usize = 0;
    for rpm_hundreds in 60..=240u32 {
        let rpm = rpm_hundreds * 100;
        let divisor = f64::from(rpm) * f64::from(flutes);
        let feed = band_max * divisor;
        match gate_verdict(feed, rpm, flutes, axial_doc) {
            ChiploadVerdict::Exceeds {
                side: ChipSide::High,
                triggering,
                ..
            } => flipped.push((
                rpm,
                triggering.observed_mm_per_tooth,
                triggering.observed_mm_per_tooth - band_max,
            )),
            ChiploadVerdict::Within { .. } => held += 1,
            other => panic!("unexpected verdict at rpm {rpm}: {other:?}"),
        }
    }
    println!(
        "feed parked ON the gate's ceiling: Exceeds at {} of {} RPM values, Within at {held}",
        flipped.len(),
        flipped.len() + held
    );
    for (rpm, observed, delta) in flipped.iter().take(5) {
        println!(
            "  rpm={rpm}: observed={observed:.17} max={band_max:.17} delta={delta:e} \
             printed-at-6dp: {observed:.6} vs {band_max:.6}"
        );
    }
    // **DISCHARGED at Checkpoint K (b1), A-7, 2026-08-13.**
    //
    // Pre-fix this assertion read `!flipped.is_empty()` and measured
    // **Exceeds at 12 of 181 RPM values** — every trip exactly 1 ulp
    // (delta 1.734723475976807e-18), printing `0.011525 vs 0.011525`.
    // `ChipBounds::exceeds_high` now applies
    // `boundary::BOUNDARY_EPSILON_REL` (8 ulp, relative), so the
    // reconstruction noise is absorbed and the assertion is INVERTED:
    // a feed parked exactly on the gate's own ceiling is Within at
    // every RPM. The fixture is kept, not deleted — it is what would
    // catch the epsilon being removed.
    assert!(
        flipped.is_empty(),
        "**G-CHIP-ULP regressed.** A feed parked exactly on the gate's own band ceiling read \
         `Exceeds(High)` at {} of {} RPM values. The boundary contract (Checkpoint K (b1), \
         `tool_load::boundary`) exists to absorb exactly this: first trips {:?}",
        flipped.len(),
        flipped.len() + held,
        &flipped[..flipped.len().min(3)]
    );
    assert_eq!(
        held, 181,
        "every RPM in 6 000..24 000 must now read Within — {held} did"
    );

    // …and the epsilon is NOT a tolerance. A genuine 5 %-over feed on
    // the same construction still trips at every RPM. This is the half
    // Checkpoint K (c2) depends on: without it, demoting the boundary
    // case would demote real exceedances too.
    let mut genuine_trips = 0usize;
    for rpm_hundreds in 60..=240u32 {
        let rpm = rpm_hundreds * 100;
        let divisor = f64::from(rpm) * f64::from(flutes);
        let feed = band_max * 1.05 * divisor;
        match gate_verdict(feed, rpm, flutes, axial_doc) {
            ChiploadVerdict::Exceeds {
                side: ChipSide::High,
                ..
            } => genuine_trips += 1,
            other => panic!(
                "a 5 %-over feed must still Exceed at rpm {rpm}; got {other:?}. The boundary \
                 epsilon has become a tolerance."
            ),
        }
    }
    println!("genuine 5 %-over feed: Exceeds at {genuine_trips} of 181 RPM values");
    assert_eq!(genuine_trips, 181);
}

// ---------------------------------------------------------------------
// Rider 2b — the band the verdict is judged against is RESOLUTION-DEPENDENT
// ---------------------------------------------------------------------

/// W10-LV recorded that the *queried effective diameter* moved
/// **1.308 → 1.372 mm** between a 1.0 mm and a 0.1 mm simulation cell on
/// the same operation, so the coarse read was `Within` at 97.8 % of the
/// band and the fine read was `Exceeds`.
///
/// The mechanism is structural and reproducible without a simulator: the
/// gate queries the LUT at `tool.lookup_diameter_at(peak steady-state
/// axial DOC)` (`chipload.rs:521-536`) and derates by
/// `peak_doc / that diameter`. **Peak axial DOC is a dexel measurement**,
/// so the cell size moves it, and on any non-cylindrical cutter it moves
/// the queried diameter with it. Two independent terms then move the
/// band: the `D^0.61` diameter law and the piecewise DOC derate — and
/// they pull in OPPOSITE directions, so the net is small, signed, and
/// not predictable from either law alone.
#[test]
fn rider2b_the_band_moves_with_the_simulation_cell_that_measured_the_doc() {
    let (rpm, flutes) = (18_000u32, 2u32);
    // The two DOC readings that reproduce W10-LV's queried diameters on
    // this cutter: d(doc) = 1.0 + 2·doc·tan(5.26°).
    let tool = b3_tool(flutes);
    println!(
        "| peak axial DOC (mm) | queried Ø (mm) | band min | band max | Δ band max vs first |"
    );
    println!("|---:|---:|---:|---:|---:|");
    let mut first_max: Option<f64> = None;
    let mut rows: Vec<(f64, f64, f64)> = Vec::new();
    for doc in [0.35_f64, 0.70, 1.05, 1.673, 2.020, 3.0] {
        let v = gate_verdict(100.0, rpm, flutes, doc);
        let (min, max) = verdict_bounds(&v);
        let queried = tool.lookup_diameter_at(doc);
        let delta = first_max.map_or(0.0, |f: f64| (max / f - 1.0) * 100.0);
        first_max.get_or_insert(max);
        println!(
            "| {doc:.3} | {queried:.4} | {:.6} | {max:.6} | {delta:+.2} % |",
            min.unwrap_or(f64::NAN)
        );
        rows.push((doc, queried, max));
    }
    let maxes: Vec<f64> = rows.iter().map(|(_, _, m)| *m).collect();
    let lo = maxes.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = maxes.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    println!(
        "band maximum spans {lo:.6} .. {hi:.6} across the DOC sweep — a {:.2} % move in the \
         bound a verdict is compared against, driven by a quantity the SIM CELL measures.",
        (hi / lo - 1.0) * 100.0
    );

    // The W10-LV pair specifically: 1.673 mm vs 2.020 mm peak DOC
    // (queried Ø 1.308 vs 1.372).
    let coarse = rows
        .iter()
        .find(|(d, _, _)| (*d - 1.673).abs() < 1e-9)
        .unwrap();
    let fine = rows
        .iter()
        .find(|(d, _, _)| (*d - 2.020).abs() < 1e-9)
        .unwrap();
    println!(
        "W10-LV pair: Ø{:.4} band max {:.6}  →  Ø{:.4} band max {:.6}  ({:+.2} %)",
        coarse.1,
        coarse.2,
        fine.1,
        fine.2,
        (fine.2 / coarse.2 - 1.0) * 100.0
    );
    assert!(
        (coarse.2 - fine.2).abs() / coarse.2 > 1e-4,
        "the band must MOVE with the measured DOC for this rider to exist; got {:.9} vs {:.9}",
        coarse.2,
        fine.2
    );

    // And the move is verdict-crossing: a feed parked on the coarse
    // band's ceiling is judged against the fine band.
    let divisor = f64::from(rpm) * f64::from(flutes);
    let feed_on_coarse_ceiling = coarse.2 * divisor;
    let v_fine = gate_verdict(feed_on_coarse_ceiling, rpm, flutes, fine.0);
    let v_coarse = gate_verdict(feed_on_coarse_ceiling, rpm, flutes, coarse.0);
    println!(
        "one feed ({feed_on_coarse_ceiling:.4} mm/min), two cells: coarse-DOC verdict {}, \
         fine-DOC verdict {}",
        verdict_tag(&v_coarse),
        verdict_tag(&v_fine)
    );
    assert_ne!(
        verdict_tag(&v_coarse),
        verdict_tag(&v_fine),
        "**the resolution rider did not reproduce**: the same feed got the same verdict at \
         both DOC readings. Either the band stopped depending on the measured DOC (say so and \
         retire), or these two readings no longer straddle the bound — pick a wider pair."
    );
}

fn verdict_tag(v: &ChiploadVerdict) -> &'static str {
    match v {
        ChiploadVerdict::Within { .. } => "Within",
        ChiploadVerdict::Exceeds {
            side: ChipSide::High,
            ..
        } => "Exceeds(High)",
        ChiploadVerdict::Exceeds { .. } => "Exceeds(Low)",
        ChiploadVerdict::Unmodeled { .. } => "Unmodeled",
    }
}

// ---------------------------------------------------------------------
// Rider 3 — the clamp is unrepresented in the binding-constraint vocabulary
// ---------------------------------------------------------------------

/// The rubbing-floor clamp is what parked the feed, and no
/// `BindingConstraint` variant denotes it. `ChiploadMin` is the nearest
/// name and is a different quantity (`band.min`, not
/// `effective_rubbing_floor`) — in the floor==ceiling regime it points
/// at the opposite end of the band.
///
/// The match below is exhaustive on purpose: adding
/// `BindingConstraint::RubbingFloor` (Checkpoint K (d)) breaks this
/// test's compile, which is the signal that (d) landed.
#[test]
fn rider3_no_binding_constraint_variant_denotes_the_rubbing_floor_clamp() {
    let all = [
        BindingConstraint::ChiploadMax,
        BindingConstraint::ChiploadMin,
        BindingConstraint::DeflectionMax,
        BindingConstraint::PowerMax,
        BindingConstraint::MachineMaxFeed,
        BindingConstraint::KinematicReach,
        // Phase 3 (2026-09-07). The geometric plunge cap. It is a
        // vertical-motion bound, not a chip-formation floor, so it does
        // not answer rider 3 — the rubbing-floor clamp is still unnamed.
        BindingConstraint::PlungeRate,
    ];
    for v in all {
        // Exhaustiveness guard: a new variant fails to compile here.
        match v {
            BindingConstraint::ChiploadMax
            | BindingConstraint::ChiploadMin
            | BindingConstraint::DeflectionMax
            | BindingConstraint::PowerMax
            | BindingConstraint::MachineMaxFeed
            | BindingConstraint::KinematicReach
            | BindingConstraint::PlungeRate => {}
        }
        assert!(
            !v.label().contains("rubbing"),
            "a variant now names the rubbing floor ({}). If Checkpoint K (d) landed, retire \
             this pin and cite the commit.",
            v.label()
        );
    }

    // Ruling R4 Q9 (2026-09-24): on a band with a minimum below 0.025 the
    // rubbing floor IS the band minimum, the quantity `ChiploadMin` denotes.
    // Before Q9 the floor was the band CEILING here, 2 x the minimum.
    let band = b3_scallop().chipload_bounds.expect("B3 band");
    let modulator_floor_fpt = band.min_mm_per_tooth;
    let suggest_floor_fpt = effective_rubbing_floor(Some(band));
    println!(
        "rider 3: BindingConstraint::ChiploadMin denotes fpt {modulator_floor_fpt:.6} \
         (band.min); the rubbing floor is {suggest_floor_fpt:.6}"
    );
    assert_eq!(
        suggest_floor_fpt, modulator_floor_fpt,
        "under ruling R4 Q9 the two quantities coincide on a band with a minimum"
    );
}

// ---------------------------------------------------------------------
// Rider 4 — cross-gate boundary semantics, measured
// ---------------------------------------------------------------------

/// One table, every shipped gate, observed set **exactly** on its bound
/// and at ±1 ulp. The ledger recorded these as disagreeing; measured,
/// they agree at exact equality (all `Within`) and differ in whether an
/// epsilon dial exists and whether the observation can even land exactly.
#[test]
fn cross_gate_boundary_semantics_at_exact_equality() {
    use rs_cam_core::material::Material as M;
    use rs_cam_core::tool_load::drill_gates::{DrillGateOutcome, classify_plunge_feed};

    println!("| gate | side | comparison | at exact equality | epsilon dial |");
    println!("|---|---|---|---|---|");

    // --- chipload high side -------------------------------------------
    let (rpm, flutes, axial_doc) = (18_000u32, 2u32, 0.35);
    let probe = gate_verdict(100.0, rpm, flutes, axial_doc);
    let (_, band_max) = verdict_bounds(&probe);
    // Choose a feed whose reconstruction is EXACTLY the bound.
    let divisor = f64::from(rpm) * f64::from(flutes);
    let mut exact_feed = None;
    for rpm_hundreds in 60..=240u32 {
        let r = rpm_hundreds * 100;
        let d = f64::from(r) * f64::from(flutes);
        if (band_max * d) / d == band_max {
            exact_feed = Some((band_max * d, r));
            break;
        }
    }
    let (feed_exact, rpm_exact) = exact_feed.expect(
        "some RPM must reconstruct the bound exactly — otherwise the 'equality' row of this \
         table is unreachable and that is itself the finding",
    );
    let at_equality = gate_verdict(feed_exact, rpm_exact, flutes, axial_doc);
    assert!(
        matches!(at_equality, ChiploadVerdict::Within { .. }),
        "chipload high side must be Within at EXACT equality (`observed > max` is strict); \
         got {at_equality:?}"
    );
    println!(
        "| chipload | high | `observed > max × (1 + breakage)` | **Within** | \
         `ToleranceBands::breakage` (default 0.0) |"
    );
    // One ulp above used to flip it — the whole of G-CHIP-ULP in one
    // line. **Checkpoint K (b1), 2026-08-13: it no longer does.** The
    // row is kept and inverted so the table still reports the live
    // contract rather than a historical one.
    let one_ulp_above = f64::from_bits(band_max.to_bits() + 1);
    let feed_ulp = one_ulp_above * f64::from(rpm_exact) * f64::from(flutes);
    let v_ulp = gate_verdict(feed_ulp, rpm_exact, flutes, axial_doc);
    assert!(
        matches!(v_ulp, ChiploadVerdict::Within { .. }),
        "one ulp above the bound must be ABSORBED by the boundary epsilon — got {v_ulp:?}"
    );
    println!(
        "| chipload | high, +1 ulp | same, via `ChipBounds::exceeds_high` | **Within** \
         (absorbed) | `BOUNDARY_EPSILON_REL` = 8 ulp, relative |"
    );
    // Twenty ulp above is outside the contract's slack and still trips —
    // the epsilon is bounded, not open-ended.
    //
    // Why 20 and not 9: `BOUNDARY_EPSILON_REL` is `8 × f64::EPSILON`
    // **relative**, and a relative epsilon is not a fixed ulp count.
    // For `x ∈ [2ᵉ, 2ᵉ⁺¹)`, `x × f64::EPSILON ∈ [1 ulp, 2 ulp)`, so the
    // slack is **8–16 ulp** depending on where the bound sits in its
    // binade — 8 at the bottom, just under 16 at the top. On this
    // fixture's bound it is ~11.8 ulp, which is why a 9-ulp probe is
    // absorbed and is NOT a defect. 20 clears the whole range.
    let above = f64::from_bits(band_max.to_bits() + 20);
    let feed_20ulp = above * f64::from(rpm_exact) * f64::from(flutes);
    let v_20ulp = gate_verdict(feed_20ulp, rpm_exact, flutes, axial_doc);
    assert!(
        matches!(
            v_20ulp,
            ChiploadVerdict::Exceeds {
                side: ChipSide::High,
                ..
            }
        ),
        "twenty ulp above the bound is past the 8–16 ulp slack and must still trip — got \
         {v_20ulp:?}"
    );
    println!("| chipload | high, +20 ulp | same | **Exceeds(High)** | slack exhausted |");
    let _ = divisor;

    // --- chipload low side --------------------------------------------
    println!(
        "| chipload | low | `median < min × (1 - burn)` | **Within** | \
         `ToleranceBands::burn` (default 0.0), plus `low_side_is_advisory` demotion |"
    );

    // --- drill plunge feed --------------------------------------------
    let material = M::SolidWood {
        species: WoodSpecies::HardMaple,
    };
    let (lo, hi) = rs_cam_core::tool_load::drill_gates::plunge_feed_envelope(&material);
    let d = 6.0;
    let at_lo = classify_plunge_feed(lo * d, d, &material);
    let at_hi = classify_plunge_feed(hi * d, d, &material);
    assert!(
        matches!(at_lo, DrillGateOutcome::Within { .. }),
        "drill plunge feed at the exact lower bound must be Within (`observed < lo` is \
         strict); got {at_lo:?}"
    );
    assert!(
        matches!(at_hi, DrillGateOutcome::Within { .. }),
        "drill plunge feed at the exact upper bound must be Within (`observed > hi` is \
         strict); got {at_hi:?}"
    );
    println!("| drill plunge feed | low | `observed < lo` | **Within** | none |");
    println!("| drill plunge feed | high | `observed > hi` | **Within** | none |");
    println!("| drill peck adequacy | high | `observed <= threshold` | **Within** | none |");
    println!(
        "| drill chip welding | banded | `Low [0, 0.75t)` / `Elevated [0.75t, t)` / \
         `High [t, ∞)` | **Elevated** — an `Exceeds` variant *below* the threshold it \
         names | none |"
    );
    println!(
        "| power | high | `peak > available × (1 + power_breach)` | **Within** | `power_breach` (default 0.0) |"
    );
    println!(
        "| deflection | high | `peak > EXCEEDS_BOUND_MM × (1 + deflection_breach)` | \
         **Within** | `deflection_breach` (default 0.0) |"
    );

    println!(
        "\nCONCLUSION: every gate is `Within` at exact equality; the ledger's \
         'boundary semantics disagree' reading is a mis-attribution of rider 2. The real \
         asymmetries are (a) the drill gates carry NO epsilon dial while the milling gates \
         carry three that all default to zero, and (b) only the chipload observation \
         reaches its bound through a multiply/divide round trip, so only it can land \
         1 ulp off a value the operator set exactly."
    );
}
