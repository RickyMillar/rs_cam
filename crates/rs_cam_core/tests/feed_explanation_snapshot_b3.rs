//! **B3 — the feed-explanation snapshot assembler.**
//!
//! `planning/review_2026-08-04/TECH_DEBT_2_RESEARCH_AND_FIX_PLAN.md` §H1
//! authorises a test-only "feed explanation snapshot" assembler for the
//! feeds census, with one constraint: *it must label stages rather than
//! choose a winner in prose*. This file is that assembler. It asserts
//! **relationships between stages**, never that a stage is right.
//!
//! ## What the operator saw (live session 2026-07-30)
//!
//! Four chipload numbers for one operation, no two agreeing
//! (`planning/review_2026-07-29/ORCHESTRATION_LOG.md:1284-1290`):
//!
//! | stage | mm/tooth |
//! |---|---|
//! | narration nominal | 0.0714 |
//! | `feeds.chipload_clamped_to_floor` pre → post | 0.0044 → 0.0250 |
//! | gate observed | 0.000737 |
//! | gate band | 0.00458 – 0.00916 |
//!
//! `planning/review_2026-08-04/FEEDS_CENSUS.md` §4.3 reconciled the gate
//! observation **by hand**, to +0.24 %:
//!
//! ```text
//! gate_observed = commanded_fpt
//!               × mean_chip_factor(LUT nominal arc)   ← 0.080335 on that row
//!               × predicted_feed / commanded_feed     ← 0.1282 (−87.18 %)
//! ```
//!
//! ## THE FLIP — re-pinned 2026-08-06
//!
//! `planning/review_2026-08-04/CHIPLOAD_LITERATURE_VERDICT.md` answered
//! Checkpoint B item T4.1 from primary sources: the vendor chipload
//! column is a linear **advance per tooth** in every source family in
//! the shipped LUT, and the `ae` window the middle term normalises to is
//! a repo-authored application window, not a vendor measurement
//! condition (§2.3). The middle term was therefore **deleted**, and this
//! fixture's operation moves:
//!
//! | | before | after |
//! |---|---:|---:|
//! | gate observed | 0.000737 mm | **0.009153 mm/tooth** |
//! | band on this fixture | 0.003605 – 0.007211 | unchanged |
//! | position | **20 % of the MINIMUM** | **127 % of the MAXIMUM** |
//! | verdict | `Within` + **burn** advisory | **`Exceeds(High)`** — breakage |
//! | commanded (unchanged) | 0.0714 mm/tooth | = 9.90× the band maximum |
//!
//! The gate reported an operation **27 % past its breakage ceiling** as
//! sitting five times below its burn floor — and issued a *burn*
//! advisory, i.e. advice to speed up, for an operation already over the
//! top of its band. Both the magnitude and the SIDE were wrong.
//!
//! ## Correction to B-lit's own prediction, measured here
//!
//! `CHIPLOAD_LITERATURE_VERDICT.md` §3.2 predicted this operation would
//! land at *"99.9 % of band maximum"* against a band of
//! `0.004579 – 0.009159`. That band is the **live 2026-07-30 session's**,
//! whose evidence string reads `diameter scale x0.42`. This committed
//! fixture resolves its lookup diameter from the sample's axial DOC and
//! lands at `d-scale 0.3005`, giving the narrower band above. Same row
//! (`amana-tapered-hardwood-scallop-3175-2f`), same arc factor
//! (0.080482, matching B-lit's `f_lut` to six figures), different
//! diameter scale.
//!
//! So B-lit's direction is confirmed and its severity is **understated**:
//! it wrote *"one percent more effective feed and the same op is
//! `Exceeds(High)`"*. On the committed fixture the operation is already
//! 27 % past. Recorded rather than smoothed over — the fixture is not
//! adjusted to reproduce the predicted verdict, because the prediction
//! was the estimate and this is the measurement.
//!
//! The tests below are restated, not deleted, so the inversion is
//! visible in the diff.
//!
//! ## …AND THE FLIP WAS UNDONE THE SAME DAY, BY THE DIAMETER LAW
//!
//! `vendor_lookup::CHIPLOAD_DIAMETER_EXPONENT` moved `1.0 → 0.61`
//! (`CHIPLOAD_LITERATURE_VERDICT.md` §4.1; magnitudes in
//! `LAW_MAGNITUDE_TABLES.md` §2, operator-ruled 2026-08-06). This
//! fixture is the single most exposed cell in the repo to that change:
//! it queries a **Ø0.954 mm** lookup diameter against a **Ø3.175 mm**
//! row, the largest down-transfer any committed fixture makes.
//!
//! **Ruling A1 (2026-09-24) moved the key.** A tapered-ball row is now
//! looked up at the tip, so the gate queries **Ø1.0 mm**, not the Ø0.954 mm
//! engaged diameter at the 0.35 mm sample depth. The tables here are the
//! record of the Ø0.954 key and are not re-derived.
//!
//! | | pre-conversion | post-conversion | post-law |
//! |---|---:|---:|---:|
//! | gate observed | 0.000737 | **0.009153** | 0.009153 *(unchanged)* |
//! | d-scale applied | 0.3005 | 0.3005 | **0.4802** (×1.598) |
//! | band | 0.003605–0.007211 | same | **0.005763–0.011525** |
//! | position | 20 % of MIN | **127 % of MAX** | **79 % of MAX** |
//! | verdict | `Within` + burn advisory | **`Exceeds(High)`** | **`Within`**, no advisory |
//! | commanded 0.0714 vs band max | 9.90× | 9.90× | **6.20×** |
//!
//! **Read the two events separately — they are not a revert.** The unit
//! deletion moved the *observation* by 12.43× and that stands; the
//! diameter law moved the *band* by 1.598× and that is what un-trips the
//! verdict. The conversion wave's finding — that the gate reported an
//! operation as five times under its burn floor when it was not, and
//! issued advice to speed up — is unaffected: the pre-conversion reading
//! is still below the floor, still asserted, still the defect. What no
//! longer holds is the *secondary* claim that the corrected reading put
//! this operation past its breakage ceiling. On a Ø1 tool the `^1.0`
//! band was too narrow, and the vendors' own charts say so.
//!
//! Every assertion the law moved is restated in place with the old value
//! and the reason quoted, never deleted.
//!
//! ## RE-PREMISED 2026-10-01: Parallel finishing on the printed Ø1 row
//!
//! The Scallop premise above lost its band before 2026-10-01 (the ruling
//! of 2026-10-01, below, gives it a printed band again). Two commits
//! removed it:
//!
//! 1. **42192d6e** (2026-09-23) loaded the printed Onsrud 77-100 tapered
//!    rows. From that commit the Scallop query matched
//!    `onsrud-hardwood-77-100-1_4-scallop` (Ø6.35, no `ae` window), not
//!    the recorded row `amana-tapered-hardwood-scallop-3175-2f`. The gate
//!    still gave `Within`, but `explain` panicked at "row publishes
//!    ae_min" in five tests. Measured: at 331e6fa6 (the parent) all seven
//!    tests pass; at 42192d6e five fail.
//! 2. **87027060** (2026-09-24) added the size rule "a tool under 1.5 mm
//!    refuses a row more than 2x its size" (`feeds::support`,
//!    `micro_extrapolation_refusal`), on the Suggest side only. The gate
//!    took the rule in **90aaf54b** (the G1 size claim: a `Refused` size
//!    basis has no band, and the gate abstains). Measured: the gate gives
//!    `Within` at 87027060 and at 04f11dd6, and `Unmodeled(NoVendorData)`
//!    at 90aaf54b. The recorded row is Ø3.175, 3.2x the Ø1 tip, so it
//!    refuses too. Reading it by its id does not bring the band back.
//!
//! Until the operator ruling of 2026-10-01 a test
//! (`the_old_scallop_premise_is_refused_by_the_size_rule`) pinned that
//! refusal, so it was a recorded behaviour, not a silent loss.
//!
//! **The ruling of 2026-10-01 ("yes") gives the Scallop cell a band
//! again.** The Amana ZrN v8 chart names no operation; its rows are filed
//! under parallel / finish as an assignment. A third `FAMILY_RULES` entry
//! (`feeds/extrapolation/family.rs`, home (Parallel, Finish), serves
//! Contour, Scallop and Trace) carries the printed Ø1 row to Scallop. So
//! the old Scallop query matches [`PREMISE_ROW`], `Exact`, with the band
//! printed x 25.4 (`amana_zrn_3d_v8.txt:9`), and the gate gives it the
//! Parallel verdict. [`the_old_scallop_premise_ships_the_printed_tip_row`]
//! pins that, and keeps a refusal pin for the same Ø1 tip in plywood,
//! where the only row (Onsrud 1/4 in, Ø6.35) is more than 2x the tip. The
//! tests below keep the Parallel premise: it is the row's home family.
//!
//! **The new premise.** The same tool, material, feed, RPM, sample depth
//! and achieved-feed fraction, on **Parallel finishing** (`DropCutter`,
//! `(Parallel, Finish)`). That query matches
//! `amana-tapered-hardwood-parallel-1000-2f-zrn-v8`, a printed row at
//! the tip (`size_basis` `Exact`):
//!
//! - band 0.00075"–0.002" per tooth = **0.01905–0.0508 mm**
//!   (`data/vendor_lut/sources/amana_zrn_3d_v8.txt:9`, column "1mm",
//!   row "Wood, MDF, Sign-Foam"; × 25.4; transcribed at
//!   `data/vendor_lut/observations/amana_zrn_tapered_v8.json:78-79`);
//! - the chart RPM is 18 000 and its depth rule is 1 x D
//!   (`amana_zrn_3d_v8.txt:3`), the fixture's RPM;
//! - d-scale 1.0 (row Ø1.0, `amana_zrn_tapered_v8.json:75`, at the tip
//!   key of ruling A1); h-scale 1.0 (row Janka 1450,
//!   `amana_zrn_tapered_v8.json:74`, = hard maple); depth de-rate 1.0
//!   (DOC 0.35 / engaged Ø0.954 = 0.37 ≤ 1, `feeds/geometry.rs:156-157`);
//! - the row has no `ae` window, so the source is `VendorLutMissingAe`
//!   (`tool_load/chipload.rs:650-651`) and the low side is advisory
//!   (`tool_load/verdict.rs:1301-1307`).
//!
//! No printed tapered-ball or ball row in the LUT has an `ae` window. Every
//! wood row with one is grade c and repo-authored (`chipload.rs:31-37`).
//! The deleted stage is therefore read off the recorded row by its id
//! ([`RECORDED_ROW`]). It is the factor the census measured on that row,
//! kept as a record; the gate does not compute it.
//!
//! The fixture's feed numbers are the live session's and do not change,
//! as the rule above says: the fixture is not adjusted to reproduce a
//! verdict. The band moves, so the band positions move:
//!
//! | | Scallop, recorded row (to 42192d6e) | Parallel, printed Ø1 row |
//! |---|---:|---:|
//! | band | 0.005763–0.011525 | **0.01905–0.0508** |
//! | extrapolated | yes (3.175 → 0.954) | **no** (printed at the tip) |
//! | achieved 0.009153 | 79 % of MAX | **48.0 % of MIN** |
//! | verdict | `Within`, no advisory | **`Within` + burn advisory** |
//! | commanded 0.0714 vs band max | 6.20x | **1.41x** |
//! | pre-conversion 0.000737 | 12.8 % of MIN | **3.9 % of MIN** |
//!
//! Read the new row as it is: on the vendor's printed band the live
//! operation's achieved feed is **under the floor**. The pre-conversion
//! gate put it 12.4x further under. The conversion moved the reading to
//! the correct unit; it did not move it to the correct side, because on
//! this row both readings are on the low side.
//!
//! A hand-checked arithmetic identity is not evidence a gate change can be
//! re-measured against. This file re-derives the same identity **through
//! the shipped code**, with no generator, arc fitter, lead-in, dressup or
//! depth planner participating — the trace is hand-built, so a reproduction
//! cannot be attributed to any of them.
//!
//! ## The five labelled stages
//!
//! [`FeedExplanation`] carries them. Each is sourced from exactly one
//! production symbol, named in its field doc. No field is computed from
//! another except where the census claims a relationship, and those claims
//! are the assertions.
//!
//! ## What each test establishes
//!
//! 1. [`stage_3_lut_arc_factor_matches_shipped_chip_geometry`] — the census
//!    read `tool_load::chipload::mean_chip_factor` (a private mirror) rather
//!    than the production chip model. This drives
//!    `MillingCutter::chip_geometry` across an arc sweep and shows the mirror
//!    is exact. Without this the whole reconciliation rests on a copy.
//! 2. [`the_gate_observation_reconciles_to_the_two_labelled_stages`] — the
//!    census's identity minus the deleted term, live, and the verdict
//!    flip it produces.
//! 3. [`the_sample_engagement_arc_cancels_out_of_the_gate_observation`] —
//!    the census's strongest structural claim: after D9 the sample's own arc
//!    algebraically cancels, so "observed chipload" carries no information
//!    about sample engagement. Two fixtures differing **only** in sample arc
//!    must produce the same observation.
//! 4. [`the_predicted_feed_factor_is_live`] — non-vacuity. With
//!    `predicted_feeds` empty the observation must move by exactly the feed
//!    ratio, proving stage 4 is not inert in this fixture.
//! 5. [`the_gate_observation_and_the_band_are_now_the_same_quantity`] —
//!    **inverted 2026-08-06.** It measured census F-1 as a ratio and
//!    deliberately took no position, because T4.1 was open. T4.1 is
//!    answered, so the test now asserts the units agree and records what
//!    the old ratio was.
//! 6. [`the_commanded_feed_per_tooth_is_never_compared_to_the_band`] —
//!    census P-10. The only same-unit comparison available is not made by
//!    any verdict, and here it is far above the band.
//! 7. [`the_two_doc_ratio_diameters_only_diverge_for_v_bit_geometry`] —
//!    census §9 item 5 / P-5. The census asserted, statically, that
//!    Suggest's post-mutation band re-derivation and the gate's derating
//!    can diverge "on ball tools at shallow DOC". Measured here through
//!    `feeds::calculate`, that is **false**; the reachable case is
//!    narrower and is a different geometry.
//! 8. [`the_old_scallop_premise_ships_the_printed_tip_row`] — the
//!    premise this file left on 2026-10-01, under the operator ruling of
//!    the same day: the Ø1 Scallop cell reads the printed v8 row, `Exact`,
//!    with the Parallel verdict; the same tip in plywood still refuses on
//!    the size rule. Until the ruling this test pinned the refusal in hard
//!    maple (`the_old_scallop_premise_is_refused_by_the_size_rule`).
//!
//! **Nothing here proposes a fix.** Every number this file prints is
//! reported under the stage that produced it.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::dexel_stock::effective_chip_thickness_mm;
use rs_cam_core::feeds::vendor_lookup::{LookupQuery, LookupResult, find_best_chip_envelope_row};
use rs_cam_core::feeds::vendor_lut::{
    HardnessKind, LutOperationFamily, LutPassRole, MaterialFamily, ToolFamily,
};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::kinematics::PredictedFeedMap;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::stock::simulation_cut::{
    CutKinematics, Engagement, SimulationCutSample, SimulationCutTrace,
};
use rs_cam_core::tool::{EngagementMode, MillingCutter, TaperedBallEndmill, ToolDefinition};
use rs_cam_core::tool_load::verdict::{ChipBoundsSource, ChiploadVerdict};
use rs_cam_core::tool_load::{GateEnv, ToleranceBands, ToolpathLoadContext, chipload};

// ── The fixture, pinned to the live operation's shape ───────────────────
//
// Synthesised in-test. `planning/airrun_2026-06-01/wanaka.toml` is a
// read-only play-file (plan §2 rule 9) and is NOT an input here.

/// Ball-tip diameter of the tapered cutter (mm).
const TIP_DIAMETER_MM: f64 = 1.0;
/// Taper half-angle (degrees).
const TAPER_HALF_ANGLE_DEG: f64 = 5.26;
/// Shank diameter (mm) — caps the tapered cone's growth.
const SHANK_DIAMETER_MM: f64 = 6.0;

const RPM: u32 = 18_000;
const FLUTES: u32 = 2;
/// Commanded feed chosen so `feed / (rpm · flutes)` lands on the live
/// narration nominal of 0.0714 mm/tooth.
const COMMANDED_FEED_MM_MIN: f64 = 0.0714 * RPM as f64 * FLUTES as f64;
/// Live `modulation_summary.median_feed_delta_pct = −87.18`.
const PREDICTED_FEED_FRACTION: f64 = 0.128_2;

/// Axial engagement of every sample (mm). Sits in the tapered ball's
/// spherical region and clear of the ±0.05 mm ball/cone transition zone
/// that `TaperedBallEndmill::chip_geometry` refuses.
const SAMPLE_AXIAL_DOC_MM: f64 = 0.35;

/// A sample engagement arc deliberately far from the matched row's nominal
/// arc, so the D9 normalisation is doing real work in this fixture.
const SAMPLE_ARC_A_RAD: f64 = 1.5;
/// A second, very different arc. Test 3 requires the observation to be
/// identical under both.
const SAMPLE_ARC_B_RAD: f64 = 0.4;

const SAMPLE_COUNT: usize = 9;
const TOOLPATH: ToolpathId = ToolpathId(0);

// ── The premise (re-premised 2026-10-01, see the module header) ─────────

/// Parallel finishing: the `DropCutter` raster, queried as
/// `(Parallel, Finish)` (`vendor_normalize::lut_query_for` keeps it).
const OPERATION_KIND: OperationType = OperationType::DropCutter;
const OPERATION_FAMILY: LutOperationFamily = LutOperationFamily::Parallel;
const PASS_ROLE: LutPassRole = LutPassRole::Finish;

/// The row the new premise matches.
const PREMISE_ROW: &str = "amana-tapered-hardwood-parallel-1000-2f-zrn-v8";

/// Printed chip load per tooth, inches: `0.00075" - 0.002"`,
/// `crates/rs_cam_core/data/vendor_lut/sources/amana_zrn_3d_v8.txt:9`
/// (2 Flute Ball Nose, column 1mm, row "Wood, MDF, Sign-Foam").
const PRINTED_MIN_IN: f64 = 0.000_75;
const PRINTED_MAX_IN: f64 = 0.002;
/// Inches to millimetres, by definition.
const MM_PER_IN: f64 = 25.4;

/// The row the census and the 2026-08-06 flip were measured on. The
/// deleted stage reads its `ae` window by this id.
const RECORDED_ROW: &str = "amana-tapered-hardwood-scallop-3175-2f";

fn tool() -> ToolDefinition {
    ToolDefinition::new(
        Box::new(TaperedBallEndmill::new(
            TIP_DIAMETER_MM,
            TAPER_HALF_ANGLE_DEG,
            SHANK_DIAMETER_MM,
            20.0,
        )),
        SHANK_DIAMETER_MM,
        30.0,
        20.0,
        40.0,
        FLUTES,
        ToolMaterial::Carbide,
    )
}

/// Hard maple: Janka 1450, the exact hardness the matched row publishes,
/// so the hardness scale is 1.00 and the diameter scale is the only one
/// moving — mirroring the live evidence string
/// `"diameter scale x0.42, hardness scale x1.00"`. The premise row of
/// 2026-10-01 also publishes 1450 (`amana_zrn_tapered_v8.json:74`), so
/// both scales are 1.00 on it.
fn material() -> Material {
    Material::SolidWood {
        species: WoodSpecies::HardMaple,
    }
}

// ── Stage record ────────────────────────────────────────────────────────

/// One labelled explanation of "what chipload is this operation running
/// at", assembled from every stage that answers the question.
///
/// The stages are deliberately **not** reduced to a single number. The
/// census's finding is that they answer different questions in different
/// units; collapsing them here would reproduce the defect.
#[derive(Debug, Clone)]
struct FeedExplanation {
    /// **Stage 1 — commanded.** `feed / (rpm · flutes)`, the quantity
    /// `narrate.rs:426` prints (as "commanded feed-per-tooth" since T1.2,
    /// "nominal chipload" before it). Unit: mm of linear
    /// *advance* per tooth.
    commanded_fpt_mm: f64,
    /// **Stage 2 — LUT band.** `chip_load_min/max_mm` off the matched row
    /// (`vendor_lookup::find_best_chip_envelope_row`), already scaled for
    /// diameter and hardness. Unit: vendor advance per tooth.
    band_min_mm: Option<f64>,
    band_max_mm: f64,
    /// **Stage 2 provenance.** Row id, the two scales, and the ±40 %
    /// extrapolation flag, as the gate classifies them.
    row_id: String,
    diameter_scale: f64,
    hardness_scale: f64,
    extrapolated: bool,
    bounds_source: ChipBoundsSource,
    /// **The deleted stage.** `mean_chip / feed_per_tooth` at the
    /// engagement arc the recorded row's repo-authored `ae` window
    /// implies ([`RECORDED_ROW`], read by its id since 2026-10-01),
    /// measured through `MillingCutter::chip_geometry`. Dimensionless.
    ///
    /// Kept in this record as the **exhibit of what the gate used to
    /// multiply by** — it is what turns 0.009153 into 0.000737 — and is
    /// no longer part of any identity. Nothing in production computes it.
    lut_arc_rad: f64,
    lut_arc_factor: f64,
    /// **Stage 4 — achieved-feed factor.** `predicted / commanded`, the
    /// substitution `tool_load::effective_feed_for_sample` performs.
    /// Dimensionless. `1.0` when the trace carries no predicted feeds.
    predicted_feed_factor: f64,
    /// **Stage 5 — gate observation.** What `tool_load::chipload::evaluate`
    /// reports. Since 2026-08-06: mm of linear *advance* per tooth at the
    /// achieved feed, taken as the median over the steady-state set —
    /// the same unit as stage 1 and stage 2.
    gate_observed_mm: f64,
    /// Which arm the verdict landed on, and whether the low side was
    /// demoted to an advisory (F3.3).
    verdict_label: &'static str,
    burn_advisory: bool,
}

impl FeedExplanation {
    /// The identity, evaluated from the labelled stages: stage 1 × stage
    /// 4. A *prediction of stage 5*, not itself an observation.
    ///
    /// The census's §4.3 form carried a third factor,
    /// [`Self::lut_arc_factor`]. B-lit deleted it. The residual assertion
    /// against this prediction is the same one the census's form carried
    /// (≤ 1 %), which is Checkpoint B's evidence item 4 for the
    /// conversion.
    fn predicted_gate_observation(&self) -> f64 {
        self.commanded_fpt_mm * self.predicted_feed_factor
    }

    /// What the pre-conversion gate would have reported on this fixture:
    /// the same prediction times the deleted chip-geometry factor. Kept
    /// so the flip is a measured number in this file rather than a claim
    /// in a comment.
    fn pre_conversion_gate_observation(&self) -> f64 {
        self.predicted_gate_observation() * self.lut_arc_factor
    }

    fn report(&self, title: &str) {
        eprintln!("\n── feed explanation: {title} ─────────────────────────");
        eprintln!(
            "  stage 1  commanded feed-per-tooth   {:.6} mm/tooth   (advance)",
            self.commanded_fpt_mm
        );
        eprintln!(
            "  stage 2  LUT band                   {:.6} .. {:.6} mm/tooth (vendor advance)",
            self.band_min_mm.unwrap_or(f64::NAN),
            self.band_max_mm
        );
        eprintln!(
            "           row {} | d-scale x{:.4} | h-scale x{:.4} | extrapolated {} | source {:?}",
            self.row_id,
            self.diameter_scale,
            self.hardness_scale,
            self.extrapolated,
            self.bounds_source
        );
        eprintln!(
            "  DELETED  LUT arc {:.6} rad -> factor {:.6}          (no longer applied)",
            self.lut_arc_rad, self.lut_arc_factor
        );
        eprintln!(
            "  stage 4  achieved/commanded feed    {:.6}            (dimensionless)",
            self.predicted_feed_factor
        );
        eprintln!(
            "  stage 5  gate observation           {:.9} mm/tooth  (advance, at achieved feed)",
            self.gate_observed_mm
        );
        eprintln!(
            "           verdict {} | burn_advisory {}",
            self.verdict_label, self.burn_advisory
        );
        eprintln!(
            "  identity 1x4 = {:.9}   observed = {:.9}   residual {:+.4} %",
            self.predicted_gate_observation(),
            self.gate_observed_mm,
            100.0 * (self.gate_observed_mm / self.predicted_gate_observation() - 1.0)
        );
    }
}

// ── Assembly ────────────────────────────────────────────────────────────

/// Stage 2 — resolve the row the gate will match, through the same public
/// entry point the gate's private `matched_chip_envelope` uses.
fn matched_row(tool: &ToolDefinition, lookup_diameter_mm: f64) -> LookupResult {
    matched_row_for(tool, lookup_diameter_mm, OPERATION_FAMILY, PASS_ROLE)
        .expect("the embedded LUT publishes a tapered-ball parallel finish row in hardwood")
}

/// [`matched_row`] for any `(family, role)`. The old Scallop premise reads
/// it too (test 8).
fn matched_row_for(
    tool: &ToolDefinition,
    lookup_diameter_mm: f64,
    operation_family: LutOperationFamily,
    pass_role: LutPassRole,
) -> Option<LookupResult> {
    let query = LookupQuery {
        tool_family: ToolFamily::TaperedBallNose,
        tool_subfamily: None,
        diameter_mm: lookup_diameter_mm,
        flute_count: FLUTES,
        material_family: MaterialFamily::Hardwood,
        hardness_kind: Some(HardnessKind::Janka),
        hardness_value: Some(1450.0),
        operation_family,
        pass_role,
    };
    find_best_chip_envelope_row(
        rs_cam_core::feeds::embedded_vendor_lut(),
        &query,
        &tool.to_geometry_hint(),
    )
}

/// Stage 3 — the engagement arc the recorded row was authored against,
/// from its `ae` window. Mirrors the deleted `chipload::lut_nominal_arc_rad`;
/// the *factor* below is then measured through production code rather
/// than mirrored.
///
/// The arc is read off [`RECORDED_ROW`], the row the census measured, by
/// its id. The premise row of 2026-10-01 publishes no `ae` window, and no
/// printed tapered-ball row does, so the matched row implies no arc. The
/// deleted stage is a record of what the gate multiplied by on the
/// recorded row; it is not a property of the row that matches today.
fn lut_nominal_arc_rad() -> f64 {
    let row = rs_cam_core::feeds::embedded_vendor_lut()
        .observations
        .iter()
        .find(|o| o.observation_id == RECORDED_ROW)
        .expect("the recorded row still ships in the embedded LUT");
    let ae_mid = (row.ae_min_mm.expect("the recorded row publishes ae_min")
        + row.ae_max_mm.expect("the recorded row publishes ae_max"))
        * 0.5;
    let diameter = row
        .diameter_mm
        .expect("the recorded row publishes a diameter");
    (1.0 - 2.0 * ae_mid / diameter).clamp(-1.0, 1.0).acos()
}

/// Stage 3 — `mean_chip / feed_per_tooth` at `arc`, measured by driving
/// the **production** chip model at unit feed per tooth. Independent of
/// `tool_load::chipload::mean_chip_factor`.
fn arc_factor_via_chip_geometry(cutter: &dyn MillingCutter, arc_rad: f64) -> f64 {
    cutter
        .chip_geometry(
            SAMPLE_AXIAL_DOC_MM,
            arc_rad,
            1.0, // unit feed per tooth ⇒ mean_chip IS the factor
            FLUTES,
            EngagementMode::Climb,
        )
        .expect("chip geometry supported at this DOC / arc")
        .mean_chip_thickness_mm
}

/// Build the trace. Every sample's `effective_chip_thickness_mm` comes
/// from the shipped `dexel_stock::effective_chip_thickness_mm` driven by
/// the real cutter — no chip value is hand-written into this fixture.
fn trace(sample_arc_rad: f64, with_predicted_feeds: bool) -> SimulationCutTrace {
    let tool = tool();
    let commanded_fpt = COMMANDED_FEED_MM_MIN / (f64::from(RPM) * f64::from(FLUTES));
    let chip = effective_chip_thickness_mm(
        &tool,
        SAMPLE_AXIAL_DOC_MM,
        Some(sample_arc_rad),
        commanded_fpt,
        FLUTES,
    )
    .expect("the production chip model resolves at this DOC / arc");

    let samples: Vec<SimulationCutSample> = (0..SAMPLE_COUNT)
        .map(|i| SimulationCutSample {
            toolpath_id: TOOLPATH,
            move_index: i + 1,
            sample_index: i,
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: CutKinematics::Linear,
            feed_rate_mm_min: COMMANDED_FEED_MM_MIN,
            spindle_rpm: RPM,
            flute_count: FLUTES,
            axial_doc_mm: SAMPLE_AXIAL_DOC_MM,
            axial_engagement_mm: SAMPLE_AXIAL_DOC_MM,
            arc_engagement_radians: Some(sample_arc_rad),
            chipload_mm_per_tooth: commanded_fpt,
            effective_chip_thickness_mm: Some(chip),
            engagement: Engagement::with_radial_woc(0.4),
            removed_volume_est_mm3: 0.1,
            mrr_mm3_s: 1.0,
            ..SimulationCutSample::test_fixture()
        })
        .collect();

    let mut predicted_feeds = PredictedFeedMap::new();
    if with_predicted_feeds {
        for s in &samples {
            predicted_feeds.insert(
                (s.toolpath_id, s.move_index),
                COMMANDED_FEED_MM_MIN * PREDICTED_FEED_FRACTION,
            );
        }
    }

    SimulationCutTrace {
        sample_step_mm: 1.0,
        samples,
        predicted_feeds,
        ..SimulationCutTrace::test_fixture()
    }
}

/// Run the shipped gate over the fixture trace for one premise.
fn gate_verdict(
    sample_arc_rad: f64,
    with_predicted_feeds: bool,
    operation_kind: OperationType,
    operation_family: LutOperationFamily,
    pass_role: LutPassRole,
) -> ChiploadVerdict {
    gate_verdict_in(
        &material(),
        sample_arc_rad,
        with_predicted_feeds,
        operation_kind,
        operation_family,
        pass_role,
    )
}

/// [`gate_verdict`] in another material. Test 8 runs it in plywood.
fn gate_verdict_in(
    material: &Material,
    sample_arc_rad: f64,
    with_predicted_feeds: bool,
    operation_kind: OperationType,
    operation_family: LutOperationFamily,
    pass_role: LutPassRole,
) -> ChiploadVerdict {
    let tool = tool();
    let trace = trace(sample_arc_rad, with_predicted_feeds);
    let tolerance = ToleranceBands::default();
    chipload::evaluate(
        &ToolpathLoadContext {
            toolpath_id: TOOLPATH,
            tool: &tool,
            material,
            operation_family,
            pass_role,
            operation_feed_rate_mm_min: COMMANDED_FEED_MM_MIN,
            operation_kind,
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

/// Run the shipped gate over the fixture and assemble the five stages.
fn explain(sample_arc_rad: f64, with_predicted_feeds: bool) -> FeedExplanation {
    let tool = tool();
    let verdict = gate_verdict(
        sample_arc_rad,
        with_predicted_feeds,
        OPERATION_KIND,
        OPERATION_FAMILY,
        PASS_ROLE,
    );

    // The gate's own lookup key at the peak steady-state axial DOC. Since
    // ruling A1 (2026-09-24) that is the 1.0 mm tip of the tapered ball
    // (`lut_key_diameter_for_cutter`), not the 0.954 mm engaged diameter
    // that the tables in the module header record.
    let lookup_diameter =
        rs_cam_core::feeds::geometry::lut_key_diameter_for_cutter(&tool, SAMPLE_AXIAL_DOC_MM);
    let row = matched_row(&tool, lookup_diameter);
    let lut_arc = lut_nominal_arc_rad();

    let (gate_observed_mm, band_min, band_max, source, verdict_label, burn_advisory) =
        match &verdict {
            ChiploadVerdict::Within {
                approach_to_min,
                approach_to_max,
                burn_advisory,
                ..
            } => {
                // The low-side observation is the median statistic — the same
                // one the live session read. Prefer the advisory (which is the
                // demoted trip), else the approach metric.
                let m = burn_advisory
                    .as_deref()
                    .or(approach_to_min.as_ref())
                    .unwrap_or(approach_to_max);
                (
                    m.observed_mm_per_tooth,
                    m.bounds.min_mm_per_tooth,
                    m.bounds.max_mm_per_tooth,
                    m.bounds.source,
                    "Within",
                    burn_advisory.is_some(),
                )
            }
            ChiploadVerdict::Exceeds { triggering, .. } => (
                triggering.observed_mm_per_tooth,
                triggering.bounds.min_mm_per_tooth,
                triggering.bounds.max_mm_per_tooth,
                triggering.bounds.source,
                "Exceeds",
                false,
            ),
            ChiploadVerdict::Unmodeled { reason } => {
                panic!(
                    "the fixture must reach a modelled verdict — the assembler cannot explain a \
                 refusal. Got Unmodeled({reason:?})"
                )
            }
        };

    FeedExplanation {
        commanded_fpt_mm: COMMANDED_FEED_MM_MIN / (f64::from(RPM) * f64::from(FLUTES)),
        band_min_mm: band_min,
        band_max_mm: band_max,
        row_id: row.observation_id.clone(),
        diameter_scale: row.chipload_diameter_scale,
        hardness_scale: row.chipload_hardness_scale,
        extrapolated: row.is_extrapolated,
        bounds_source: source,
        lut_arc_rad: lut_arc,
        lut_arc_factor: arc_factor_via_chip_geometry(&tool, lut_arc),
        predicted_feed_factor: if with_predicted_feeds {
            PREDICTED_FEED_FRACTION
        } else {
            1.0
        },
        gate_observed_mm,
        verdict_label,
        burn_advisory,
    }
}

// ── 1. The mirror ───────────────────────────────────────────────────────

/// FEEDS_CENSUS.md §9 item 2. The census read the arc factor off
/// `tool_load::chipload::mean_chip_factor` — a private mirror of the
/// production chip model. If the mirror had drifted, the entire §4.3
/// reconciliation would have been arithmetic about the wrong function.
///
/// **The production mirror was deleted on 2026-08-06 with D9.** This
/// test is kept and is still not vacuous: `MillingCutter::chip_geometry`
/// still ships and is still read by `is_bipolar_engagement`, the MCP
/// per-sample peak and the narration histogram, and the closed form
/// below is still the right one for it. What the test no longer does is
/// underwrite a gate identity.
///
/// Drives `MillingCutter::chip_geometry` at unit feed per tooth across an
/// arc sweep and compares against the mirror's closed form. Also pins the
/// two anchor values the mirror's own docstring advertises.
#[test]
fn stage_3_lut_arc_factor_matches_shipped_chip_geometry() {
    /// The mirror, verbatim from `tool_load::chipload::mean_chip_factor`.
    fn mirror(arc_rad: f64) -> f64 {
        let arc = arc_rad.clamp(1e-9, std::f64::consts::PI);
        let h_max = if arc >= std::f64::consts::PI {
            1.0
        } else {
            arc.sin().abs()
        };
        (2.0 * h_max / arc) * (1.0 - (arc * 0.5).cos())
    }

    let tool = tool();
    for &arc in &[
        0.2_f64,
        0.4,
        0.58,
        SAMPLE_ARC_B_RAD,
        1.0,
        1.0843860798928202,
        SAMPLE_ARC_A_RAD,
        2.0,
        3.0,
        std::f64::consts::PI,
    ] {
        let shipped = arc_factor_via_chip_geometry(&tool, arc);
        let mirrored = mirror(arc);
        assert!(
            (shipped - mirrored).abs() < 1e-12,
            "arc {arc}: production chip_geometry gives {shipped}, the chipload gate's private \
             the closed-form mirror gives {mirrored}. A drift here invalidates the census's \
             §4.3 arithmetic and every statement in this file about what the pre-conversion \
             gate observed."
        );
    }

    // Anchors the mirror's docstring advertises, as closed forms rather
    // than decimals — at these two arcs the factor has an exact value:
    //   arc = π   ⇒ (2·1/π)·(1 − cos(π/2))   = 2/π          ≈ 0.6366
    //   arc = π/2 ⇒ (2·1/(π/2))·(1 − cos(π/4)) = (4/π)(1 − √2/2) ≈ 0.3729
    assert!(
        (mirror(std::f64::consts::PI) - std::f64::consts::FRAC_2_PI).abs() < 1e-12,
        "slot anchor moved: the arc-mean factor at π must be exactly 2/π"
    );
    let half_immersion = (4.0 / std::f64::consts::PI) * (1.0 - std::f64::consts::FRAC_1_SQRT_2);
    assert!(
        (mirror(std::f64::consts::FRAC_PI_2) - half_immersion).abs() < 1e-12,
        "half-immersion anchor moved: the arc-mean factor at π/2 must be exactly (4/π)(1 − √2/2)"
    );
    eprintln!(
        "CONFIRMED: the arc-mean closed form is exact against MillingCutter::chip_geometry across \
         10 arcs (max |Δ| < 1e-12)."
    );
}

// ── 2. The identity ─────────────────────────────────────────────────────

/// FEEDS_CENSUS.md §4.3, live, **minus the term the literature verdict
/// deleted**: the gate's observation is the commanded feed-per-tooth
/// times the achieved/commanded feed ratio. Two labelled stages, no
/// third term.
///
/// This is Checkpoint B evidence item 4 for the conversion — "the B3
/// fixture re-run with its residual assertion intact; the identity must
/// still close at ≤ 1 %". It does, against a different identity.
///
/// It is also **the flip**, measured: the same fixture, the same row,
/// the same feed, and a verdict that used to carry a *burn* advisory is
/// now a hard *breakage* trip. See the module header for the correction
/// this measurement makes to B-lit's own §3.2 prediction.
#[test]
fn the_gate_observation_reconciles_to_the_two_labelled_stages() {
    let e = explain(SAMPLE_ARC_A_RAD, true);
    e.report("live-shaped fixture (arc A, predicted feeds on)");

    let residual = e.gate_observed_mm / e.predicted_gate_observation() - 1.0;
    assert!(
        residual.abs() < 0.01,
        "gate_observed must equal commanded_fpt x feed_factor. \
         Predicted {:.9}, observed {:.9}, residual {:+.4} %. A residual above 1 % means a term \
         nothing named is participating.",
        e.predicted_gate_observation(),
        e.gate_observed_mm,
        100.0 * residual
    );

    // RE-PREMISED 2026-10-01. This read
    //     assert!(e.extrapolated,
    //             "the fixture is meant to reproduce an extrapolated row");
    // and it was true of the Scallop premise: a Ø0.954 key on the Ø3.175
    // recorded row. That premise has no band now (42192d6e, then the size
    // rule of 87027060 in the gate since 90aaf54b; test 8 pins it). On
    // Parallel finishing the Ø1 tip key (ruling A1) meets a row printed at
    // Ø1.0 (`amana_zrn_tapered_v8.json:75`) and Janka 1450 (`:74`), so the
    // raw transfer ratio is 1.0 and the row is not extrapolated. The band
    // is the printed band, to the last digit.
    assert_eq!(e.row_id, PREMISE_ROW, "the premise row must match");
    assert!(
        !e.extrapolated,
        "a row printed at the tip is not extrapolated (d-scale {}, h-scale {})",
        e.diameter_scale, e.hardness_scale
    );
    assert!(
        (e.diameter_scale - 1.0).abs() < 1e-12 && (e.hardness_scale - 1.0).abs() < 1e-12,
        "both scales must be 1.0 at the printed size and hardness; got d {} h {}",
        e.diameter_scale,
        e.hardness_scale
    );
    let printed_min = PRINTED_MIN_IN * MM_PER_IN;
    let printed_max = PRINTED_MAX_IN * MM_PER_IN;
    assert!(
        (e.band_min_mm.expect("the printed row has a floor") / printed_min - 1.0).abs() < 1e-12
            && (e.band_max_mm / printed_max - 1.0).abs() < 1e-12,
        "the gate band must be the printed 0.00075\"-0.002\" = {printed_min}-{printed_max} mm \
         (amana_zrn_3d_v8.txt:9; no scale, no depth de-rate at DOC/D 0.37); got {:?}-{}",
        e.band_min_mm,
        e.band_max_mm
    );
    assert_eq!(
        e.bounds_source,
        ChipBoundsSource::VendorLutMissingAe,
        "the printed row has no ae window (chipload.rs:650-651), so its low side is advisory"
    );

    // ── THE FLIP ────────────────────────────────────────────────────
    //
    // INVERTED 2026-08-06. These two assertions used to read
    //     assert_eq!(e.verdict_label, "Within");
    //     assert!(e.burn_advisory, "an observation this far below the
    //             floor must surface as a burn advisory (F3.3 + H4)");
    // and both were true of the DEFECT: the observation was 0.000737,
    // five times under a floor of 0.003605, because the gate multiplied
    // the commanded advance by a chip-geometry factor derived from a
    // repo-authored stepover window. Measured on the axis the vendor
    // publishes, the operation was 27 % PAST its ceiling.
    //
    // RE-PINNED AGAIN 2026-08-06, SAME DAY, by the diameter law.
    // `vendor_lookup::CHIPLOAD_DIAMETER_EXPONENT` moved 1.0 → 0.61, so a
    // Ø0.954 lookup against a Ø3.175 row carries d-scale 0.3005 → 0.4802
    // (×1.598) and this fixture's band widens 0.003605–0.007211 →
    // 0.005763–0.011525. The observation does NOT move — it is
    // `commanded × achieved-feed ratio` and touches no band — so the
    // operation lands at 79 % of its (wider) maximum and the verdict
    // returns to `Within`.
    //
    // Both re-pins are honest and they do not cancel:
    //   * the unit deletion moved the observation 0.000737 → 0.009153
    //     (12.43×) and that is unaffected;
    //   * the diameter law moved the BAND and that is what un-trips it.
    // What is no longer true is "this operation is over its breakage
    // ceiling". What was still true then, and was the finding the file
    // exists for, is that the pre-conversion reading sat five times under
    // the floor while the corrected one sat comfortably inside the band.
    //
    // RE-PREMISED 2026-10-01 (the module header has the causes). These
    // three assertions read
    //     assert_eq!(e.verdict_label, "Within", ...);
    //     assert!(!e.burn_advisory, "no burn advisory: ...");
    //     assert!(of_max < 1.0 && e.gate_observed_mm > band_min, ...);
    // on the 0.005763-0.011525 band of the Scallop premise. On the printed
    // Ø1 band, 0.01905-0.0508 (amana_zrn_3d_v8.txt:9), the SAME
    // observation 0.009153 is under the floor, at
    //     0.0714 x 0.1282 / 0.01905 = 48.0 % of MIN.
    // The verdict stays `Within`, because the low side of a row with no
    // ae window is advisory (verdict.rs:1301-1307), and the burn advisory
    // fires. The finding moves with the band: the conversion put the
    // reading on the vendor's axis; on this row that axis says "too slow".
    // The side the pre-conversion reading took is kept below.
    let band_min = e.band_min_mm.expect("row publishes a floor");
    let of_max = e.gate_observed_mm / e.band_max_mm;
    let of_min = e.gate_observed_mm / band_min;
    assert_eq!(
        e.verdict_label,
        "Within",
        "the operation is at {:.1} % of its band MINIMUM ({band_min:.6}); the low side of \
         a row with no ae window is advisory, so the verdict must stay Within",
        100.0 * of_min,
    );
    assert!(
        e.burn_advisory,
        "the observation {:.9} is under the printed floor {band_min:.9}, so the demoted \
         low-side trip must surface as a burn advisory (F3.3)",
        e.gate_observed_mm
    );
    let expected_of_min = COMMANDED_FEED_MM_MIN / (f64::from(RPM) * f64::from(FLUTES))
        * PREDICTED_FEED_FRACTION
        / (PRINTED_MIN_IN * MM_PER_IN);
    assert!(
        (of_min / expected_of_min - 1.0).abs() < 1e-9 && of_min < 1.0 && of_max < 1.0,
        "observed {:.9} must sit at 0.0714 x 0.1282 / 0.01905 = {:.4} of the printed \
         floor, under the band {band_min:.6}..{:.6}; got {of_min:.6} of min, {of_max:.6} of max",
        e.gate_observed_mm,
        expected_of_min,
        e.band_max_mm,
    );

    // And the size of what was deleted, on the recorded row, as a number.
    let pre = e.pre_conversion_gate_observation();
    eprintln!(
        "  FLIP: pre-conversion observed {pre:.9} = {:.1} % of band MIN {band_min:.6}; \
         post-conversion observed {:.9} = {:.1} % of band MIN (Within + BURN advisory). \
         Deleted factor {:.6}x, read off the recorded row {RECORDED_ROW}. On the Scallop \
         premise (to 42192d6e) the post-conversion reading sat at 79 % of a \
         0.005763-0.011525 band; on the printed Ø1 band both readings are under the floor.",
        100.0 * pre / band_min,
        e.gate_observed_mm,
        100.0 * of_min,
        e.lut_arc_factor,
    );
    assert!(
        pre < band_min,
        "the exhibit only holds if the pre-conversion reading really was below the floor"
    );
}

// ── 3. The structural claim ─────────────────────────────────────────────

/// FEEDS_CENSUS.md §4.3's strongest claim, and the one that made the
/// deletion possible rather than merely desirable: the sample's own
/// engagement arc cancels algebraically —
/// `cl_norm = fz · f(arc_sample) · f_lut / f(arc_sample) = fz · f_lut` —
/// so the gate's "observed chipload" carried **no information about the
/// sample's engagement**, which is the thing the metric was named for.
///
/// Still true after the conversion, and now true by construction rather
/// than by cancellation: the observation is `effective_feed / (rpm ·
/// flutes)` and there is no arc term to cancel. Kept because a reader
/// should be able to see that the gate is not engagement-aware — and
/// because that is precisely what `is_bipolar_engagement` and the
/// chipload entry-spike advisory now cannot get from it (recorded in
/// `tool_load::chipload`).
///
/// Two fixtures differing only in sample arc (1.5 rad vs 0.4 rad — a
/// factor of 3.4 in the raw chip thickness) must report the same
/// observation.
#[test]
fn the_sample_engagement_arc_cancels_out_of_the_gate_observation() {
    let a = explain(SAMPLE_ARC_A_RAD, true);
    let b = explain(SAMPLE_ARC_B_RAD, true);
    a.report("arc A = 1.5 rad");
    b.report("arc B = 0.4 rad");

    // Non-vacuity: the two fixtures really do present different chips.
    let tool = tool();
    let raw_a = arc_factor_via_chip_geometry(&tool, SAMPLE_ARC_A_RAD);
    let raw_b = arc_factor_via_chip_geometry(&tool, SAMPLE_ARC_B_RAD);
    assert!(
        raw_a / raw_b > 2.0,
        "the two arcs must produce materially different raw chips or this test proves nothing \
         (got {raw_a} vs {raw_b})"
    );

    let delta = (a.gate_observed_mm - b.gate_observed_mm).abs()
        / a.gate_observed_mm.max(b.gate_observed_mm);
    assert!(
        delta < 1e-9,
        "the sample arc must cancel: arc A gives {:.12}, arc B gives {:.12} (relative Δ {delta:.3e}) \
         from raw chips differing by {:.2}x",
        a.gate_observed_mm,
        b.gate_observed_mm,
        raw_a / raw_b
    );
    eprintln!(
        "CONFIRMED: raw chip differs {:.2}x between the two arcs; the gate's observation is \
         identical to {delta:.1e}. The reported 'chipload' is fz x f(LUT row) x feed ratio.",
        raw_a / raw_b
    );
}

// ── 4. Non-vacuity of stage 4 ───────────────────────────────────────────

/// The predicted-feed factor must be live in this fixture, not inert.
/// With `predicted_feeds` empty the gate falls back to commanded feed
/// (`effective_feed_for_sample`), so the observation must rise by exactly
/// `1 / PREDICTED_FEED_FRACTION`.
#[test]
fn the_predicted_feed_factor_is_live() {
    let on = explain(SAMPLE_ARC_A_RAD, true);
    let off = explain(SAMPLE_ARC_A_RAD, false);
    off.report("predicted feeds OFF (commanded feed)");

    assert!(
        (off.predicted_feed_factor - 1.0).abs() < 1e-12,
        "the OFF arm must report a unit feed factor"
    );
    let ratio = on.gate_observed_mm / off.gate_observed_mm;
    assert!(
        (ratio - PREDICTED_FEED_FRACTION).abs() < 1e-9,
        "turning predicted feeds off must scale the observation by exactly \
         1 / {PREDICTED_FEED_FRACTION}; got {ratio}"
    );
    // Both arms still reconcile to their own stages.
    assert!((off.gate_observed_mm / off.predicted_gate_observation() - 1.0).abs() < 0.01);
}

// ── 5. The unit finding, measured — no verdict on which side is right ────

/// **INVERTED 2026-08-06.** This test was named
/// `the_gate_observation_and_the_band_are_not_the_same_quantity` and
/// measured census F-1 as a ratio while deliberately taking no position
/// on which side was right, because Checkpoint B item T4.1 was open.
///
/// T4.1 is answered: `CHIPLOAD_LITERATURE_VERDICT.md` §2 verifies, per
/// source family and with verbatim quotations, that the vendor column is
/// a linear advance per tooth — and on the Amana chart behind *this*
/// fixture's row the identity is numerically self-verifying against the
/// chart's own IPM column. The gate's observation was converted to
/// match. The test now asserts they agree, and keeps the old ratio as
/// the record of how far apart they were.
#[test]
fn the_gate_observation_and_the_band_are_now_the_same_quantity() {
    let e = explain(SAMPLE_ARC_A_RAD, true);

    // The identity that makes them comparable: observation = commanded
    // advance × achieved-feed ratio, with no change of quantity.
    let expected = e.commanded_fpt_mm * e.predicted_feed_factor;
    assert!(
        (e.gate_observed_mm / expected - 1.0).abs() < 0.01,
        "the gate must report a linear advance per tooth: expected {expected:.9}, \
         got {:.9}",
        e.gate_observed_mm
    );

    // The size of the old mismatch, kept as evidence rather than a claim.
    let convention_ratio = 1.0 / e.lut_arc_factor;
    assert!(
        convention_ratio > 2.0,
        "the exhibit only means something if the two conventions really were far \
         apart on this row; got {convention_ratio}"
    );
    eprintln!(
        "MEASURED: on the recorded row {RECORDED_ROW} the pre-conversion gate reported mm of \
         CHIP against a band of ADVANCE, differing by {convention_ratio:.2}x (arc {:.5} rad \
         from a repo-authored ae window, factor {:.6}). T4.1 answered: the column is an \
         advance per tooth. The premise row today is {}.",
        e.lut_arc_rad, e.lut_arc_factor, e.row_id
    );

    // And the consequence, restated.
    //
    // RE-PREMISED 2026-10-01. This read
    //     assert!(e.gate_observed_mm > min,
    //             "observed ... must now sit ABOVE the floor — the live
    //              session's below-floor reading was the unit defect");
    // and it was true of the 0.005763-0.011525 band of the Scallop
    // premise. That premise has no band now (module header; test 8). On
    // the printed Ø1 band, 0.01905-0.0508 (amana_zrn_3d_v8.txt:9), the
    // SAME observation sits under the floor. The unit finding (the
    // assertions above) does not depend on the band; the side does. The
    // assertion below states the side the printed band gives, at its
    // exact position.
    let min = e.band_min_mm.expect("row publishes a floor");
    let expected = e.commanded_fpt_mm * e.predicted_feed_factor;
    assert!(
        e.gate_observed_mm < min
            && (e.gate_observed_mm / min - expected / (PRINTED_MIN_IN * MM_PER_IN)).abs() < 1e-9,
        "on the printed Ø1 band the observation {:.9} must sit UNDER the floor {min:.9}, at \
         0.0714 x 0.1282 / 0.01905 = {:.4} of it",
        e.gate_observed_mm,
        expected / (PRINTED_MIN_IN * MM_PER_IN)
    );
    // RE-PINNED 2026-08-06 (laws). This read
    //     assert!(e.gate_observed_mm > e.band_max_mm,
    //             "and above the ceiling ...: the defect inverted the SIDE");
    // and it was true of the band as the `^1.0` diameter law scaled it.
    // Adopting `D^0.61` widens a Ø1 query against a Ø3.175 row by
    // d-scale 0.3005 → 0.4802 (×1.598), band 0.003605–0.007211 →
    // 0.005763–0.011525. The SAME observation, 0.009153, is now inside.
    // The unit deletion's finding survives — the pre-conversion reading
    // was below the floor and the corrected one is not, which is the
    // inversion — but the corrected reading is no longer past the
    // ceiling on this fixture.
    //
    // RE-PREMISED 2026-10-01: the ceiling is now the printed 0.0508
    // (amana_zrn_3d_v8.txt:9); the observation is 0.180x of it. The
    // assertion is unchanged.
    assert!(
        e.gate_observed_mm < e.band_max_mm,
        "the observation {:.9} sits BELOW the ceiling {:.9}. If it is above, the band \
         moved and the re-pins above are stale.",
        e.gate_observed_mm,
        e.band_max_mm
    );
    assert!(
        e.pre_conversion_gate_observation() < min,
        "...and the pre-conversion reading must reproduce below it, or the flip is not real"
    );
}

// ── 6. The comparison nobody makes ──────────────────────────────────────

/// FEEDS_CENSUS.md P-10. Commanded feed-per-tooth and the vendor band are
/// the same unit at compatible stages. No verdict makes the comparison —
/// it is a diagnostic (`load.chipload.commanded_above_band`, T1.5). Here
/// the commanded value is far above the band and the gate still says
/// `Within`, because the machine will not reach the commanded feed.
///
/// Unchanged by the 2026-08-06 conversion, and now readable: the gate's
/// own number is the SAME quantity at the achieved feed, so the pair
/// "7.80× commanded / 1.00× achieved" is a single statement about one
/// operation. B-lit §3.2 calls their ratio — the −87 % kinematic
/// throttle — the operator-actionable fact, not either figure alone.
#[test]
fn the_commanded_feed_per_tooth_is_never_compared_to_the_band() {
    let e = explain(SAMPLE_ARC_A_RAD, true);
    let overshoot = e.commanded_fpt_mm / e.band_max_mm;
    // RE-PREMISED 2026-10-01. This read
    //     assert!(overshoot > 2.0, "... well above the band max ...");
    // on the 0.011525 ceiling of the Scallop premise (6.20x). That premise
    // has no band now (module header; test 8). The printed Ø1 ceiling is
    // 0.002" = 0.0508 mm (amana_zrn_3d_v8.txt:9), so the same commanded
    // 0.0714 is 0.0714 / 0.0508 = 1.4055x over it. The claim keeps its
    // property (the commanded advance is over the band maximum) and is now
    // pinned at the exact printed ratio, so it cannot drift either way.
    let expected_overshoot =
        COMMANDED_FEED_MM_MIN / (f64::from(RPM) * f64::from(FLUTES)) / (PRINTED_MAX_IN * MM_PER_IN);
    assert!(
        overshoot > 1.0 && (overshoot / expected_overshoot - 1.0).abs() < 1e-9,
        "the fixture must reproduce a commanded feed-per-tooth above the band max, at \
         0.0714 / 0.0508 = {expected_overshoot:.4}x, or the missing comparison has nothing to \
         surface; got {overshoot}x"
    );
    // This assertion has now been written three ways, and the third is
    // the first one's value with a different reason behind it — which is
    // worth saying out loud rather than letting the diff read as a
    // revert.
    //
    //  1. Pre-conversion: `assert_eq!(verdict_label, "Within")`, noted as
    //     "the gate must nonetheless report Within — that IS the
    //     finding". True because the gate compared a CHIP thickness to
    //     an ADVANCE band, 12.43× apart.
    //  2. Post-conversion (`7571d9d`): `"Exceeds"` — same axis at last,
    //     and the achieved feed genuinely sat 1.27× over a
    //     0.003605–0.007211 band.
    //  3. Post-law (2026-08-06): `"Within"` again — the observation is
    //     unchanged at 0.009153, but `D^0.61` widened the band to
    //     0.005763–0.011525 on this Ø1 query, so the achieved feed is
    //     inside it at 0.79×.
    //
    //  4. Re-premised (2026-10-01): `"Within"`, now with a burn
    //     advisory — the printed Ø1 band 0.01905–0.0508 puts the achieved
    //     feed under the floor at 0.48x of it, and the low side of a row
    //     with no ae window is advisory.
    //
    // The P-10 finding the test carries is untouched by all four: no
    // verdict compares the COMMANDED feed-per-tooth to the band, and the
    // commanded value here is 1.41× over the printed maximum (6.20× over
    // the Scallop premise's) while the verdict says nothing about it.
    // That comparison is a diagnostic (`load.chipload.commanded_above_band`,
    // T1.5), and the assertion above is the part that must never soften.
    assert_eq!(
        e.verdict_label, "Within",
        "the gate reads the ACHIEVED feed, which is under the printed ceiling, \
         while the COMMANDED value sits {overshoot:.2}x over the same maximum and no \
         verdict says so — that gap is P-10 and it survives every re-pin"
    );
    // On the printed band the gap is wider than "says nothing": the only
    // advice the gate gives is the burn advisory (feed too LOW), on an
    // operation whose commanded feed is over the band MAXIMUM.
    assert!(
        e.burn_advisory,
        "on the printed Ø1 band the gate's only advice must be the burn advisory, while \
         the commanded value sits {overshoot:.2}x over the maximum"
    );
    let achieved_over_band = e.gate_observed_mm / e.band_max_mm;
    eprintln!(
        "MEASURED: commanded {:.6} mm/tooth vs band max {:.6} = {overshoot:.2}x over; \
         ACHIEVED {:.6} vs the same max = {achieved_over_band:.2}x; throttle {:.4}. \
         One statement at two feeds — the pair is the finding, not either figure.",
        e.commanded_fpt_mm,
        e.band_max_mm,
        e.gate_observed_mm,
        achieved_over_band / overshoot,
    );
    assert!(
        (achieved_over_band / overshoot - PREDICTED_FEED_FRACTION).abs() < 1e-6,
        "the two ratios must differ by exactly the achieved-feed fraction"
    );
}

// ── 7. P-5 reachability, measured ───────────────────────────────────────

/// FEEDS_CENSUS.md §9 item 5 and finding P-5.
///
/// `feeds::calculate` binds the identifier `effective_d` twice: once at
/// LUT semantics (`ToolGeometryHint::engaged_diameter_at_doc`, the
/// denominator of the band derating it performs itself) and once, shadowing
/// it, at chip-thinning semantics (`feeds::effective_diameter`, which is
/// what gets published as `FeedsResult::effective_diameter_mm`). The
/// published one is the denominator
/// `suggest::recompute_chipload_bounds_for_dpp` divides by after the axial
/// envelope mutates DPP, while `tool_load::chipload` divides by the LUT one.
///
/// The census inferred from that shadowing that the two could produce
/// different DOC-derate scales "on a ball nose at shallow DOC, up to 2x".
/// This test measures the derate both ways through shipped code — the
/// published `effective_diameter_mm` against `lookup_diameter_at` at the
/// same resolved DOC — over a DOC sweep on every cutter shape.
///
/// **Result: ball, bull, flat and tapered-ball never diverge.** Only V-bit
/// geometry can, because `engaged_diameter_at_doc` omits the tip flat that
/// `vbit_width_at_depth` includes. The non-vacuity assertion at the end
/// requires the V-bit case to actually appear, so this test cannot pass by
/// finding nothing anywhere.
#[test]
fn the_two_doc_ratio_diameters_only_diverge_for_v_bit_geometry() {
    use rs_cam_core::feeds::geometry::doc_derating_scale;
    use rs_cam_core::feeds::{
        FeedsInput, OperationFamily, PassRole, SetupContext, SpindleStrategy, ToolGeometryHint,
        calculate,
    };
    use rs_cam_core::machine::MachineProfile;
    use rs_cam_core::tool::{BallEndmill, BullNoseEndmill, FlatEndmill, VBitEndmill};

    let machine = MachineProfile::shapeoko_vfd();
    let mat = material();

    let cases: Vec<(&str, ToolGeometryHint, Box<dyn MillingCutter>, f64)> = vec![
        (
            "flat Ø6",
            ToolGeometryHint::Flat,
            Box::new(FlatEndmill::new(6.0, 40.0)),
            6.0,
        ),
        (
            "ball Ø6",
            ToolGeometryHint::Ball,
            Box::new(BallEndmill::new(6.0, 40.0)),
            6.0,
        ),
        (
            "bull Ø6 r1",
            ToolGeometryHint::Bull { corner_radius: 1.0 },
            Box::new(BullNoseEndmill::new(6.0, 1.0, 40.0)),
            6.0,
        ),
        (
            "tapered ball Ø1 tip",
            ToolGeometryHint::TaperedBall {
                tip_radius: TIP_DIAMETER_MM * 0.5,
                taper_angle_deg: TAPER_HALF_ANGLE_DEG,
            },
            Box::new(TaperedBallEndmill::new(
                TIP_DIAMETER_MM,
                TAPER_HALF_ANGLE_DEG,
                SHANK_DIAMETER_MM,
                20.0,
            )),
            TIP_DIAMETER_MM,
        ),
        (
            "v-bit 20° Ø6, 1.0 mm tip flat",
            ToolGeometryHint::VBit {
                included_angle: 20.0,
                tip_diameter: 1.0,
            },
            {
                // `VBitEndmill::new` defaults the flat to 0.0; the field is
                // public and set after construction (its own doc says so),
                // so cutter and hint describe the *same* truncated bit.
                let mut v = VBitEndmill::new(6.0, 20.0, 40.0);
                v.tip_diameter = 1.0;
                Box::new(v)
            },
            6.0,
        ),
    ];

    let mut divergent_shapes: Vec<&str> = Vec::new();
    for (label, hint, cutter, nominal_d) in &cases {
        let mut worst = 0.0_f64;
        let mut worst_at = 0.0_f64;
        for step in 1..=120 {
            let commanded_ap = f64::from(step) * 0.1;
            let result = calculate(&FeedsInput {
                tool_diameter: *nominal_d,
                flute_count: FLUTES,
                flute_length: 40.0,
                shank_diameter: Some(SHANK_DIAMETER_MM),
                tool_geometry: *hint,
                material: &mat,
                machine: &machine,
                operation: OperationFamily::Adaptive,
                operation_kind: None,
                pass_role: PassRole::Roughing,
                axial_depth_mm: Some(commanded_ap),
                // Well below the slotting threshold so Step 4b never
                // re-caps DOC behind our back.
                radial_width_mm: Some(nominal_d * 0.3),
                target_scallop_mm: None,
                vendor_lut: None,
                setup: SetupContext::default(),
                spindle_strategy: SpindleStrategy::default(),
            });
            // The DOC that actually survived every clamp inside calculate.
            let ap = result.axial_depth_mm;
            // Denominator A: what Suggest's post-mutation re-derivation uses.
            let scale_suggest = doc_derating_scale(ap / result.effective_diameter_mm.max(1e-9));
            // Denominator B: what the post-sim chipload gate uses.
            let scale_gate = doc_derating_scale(ap / cutter.lookup_diameter_at(ap).max(1e-9));
            let delta = (scale_suggest - scale_gate).abs();
            if delta > worst {
                worst = delta;
                worst_at = ap;
            }
        }
        if worst > 1e-12 {
            divergent_shapes.push(label);
            eprintln!("  {label:32} DIVERGES — worst |Δscale| {worst:.4} at DOC {worst_at:.2} mm");
        } else {
            eprintln!("  {label:32} identical derate across the whole DOC sweep");
        }
    }

    assert_eq!(
        divergent_shapes,
        vec!["v-bit 20° Ø6, 1.0 mm tip flat"],
        "the census predicted a ball-nose divergence and did not predict a V-bit one; measured \
         divergent shapes are {divergent_shapes:?}"
    );
    eprintln!(
        "MEASURED: P-5's numeric divergence is NOT reachable on ball / bull / flat / tapered-ball \
         geometry. Only V-bit diverges, because engaged_diameter_at_doc omits the tip flat that \
         vbit_width_at_depth includes. Combined with pick_axial_envelope setting dpp_mutated on \
         Adaptive3d ONLY (suggest.rs:1348-1356; VCarve declines it, the finish-3D family is \
         warning-only), the reachable case is Adaptive3d + V-bit."
    );
}

// ── 8. The old Scallop premise, under the ruling of 2026-10-01 ───────────

/// The Scallop premise of tests 2-6 until 2026-10-01: the same Ø1 tapered
/// tip, the same hard maple, the same trace, on `(Scallop, Finish)`.
///
/// Until the operator ruling of 2026-10-01 the gate abstained on it: the
/// query matched `onsrud-hardwood-77-100-1_4-pocket` (Ø6.35, served to
/// Scallop by the Onsrud 77-100 family rule), and its G1 size basis was
/// `Refused` (a key under 1.5 mm reads only a row inside 0.5x-2x of it,
/// `feeds::support::micro_extrapolation_refusal`, 87027060; 6.35 / 1.0 =
/// 6.35x; since 90aaf54b a refused basis has no band). That test was
/// `the_old_scallop_premise_is_refused_by_the_size_rule`.
///
/// The ruling ("yes", 2026-10-01): the Amana ZrN v8 tapered rows serve
/// every 3D finishing operation, because the chart names no operation and
/// "parallel / finish is an assignment" (the row notes,
/// `amana_zrn_tapered_v8.json`). The v8 family rule
/// (`feeds::extrapolation::FAMILY_RULES`, home (Parallel, Finish), serves
/// Contour, Scallop and Trace) carries [`PREMISE_ROW`] to Scallop. So the
/// old premise now reads the printed tip row:
///
/// - row [`PREMISE_ROW`], size basis `Exact` (row Ø1.0 = the tip key of
///   ruling A1), family basis `Transferred` from (Parallel, Finish);
/// - band = printed x 25.4 = 0.01905-0.0508 mm
///   (`data/vendor_lut/sources/amana_zrn_3d_v8.txt:9`), the band of the
///   Parallel premise, so the gate gives the Parallel verdict.
///
/// The size rule did not move. A Ø1 tip whose only row is more than 2x its
/// size still refuses: in Baltic birch plywood the v8 chart prints no row
/// ("Wood, MDF, Sign-Foam"), the query reads the Onsrud 1/4 in plywood
/// pocket row (Ø6.35), and the gate abstains. The recorded row (Ø3.175) is
/// 3.175x the tip, so it is still outside the micro window.
#[test]
fn the_old_scallop_premise_ships_the_printed_tip_row() {
    use rs_cam_core::feeds::extrapolation::{Gap, SizeBasis};
    use rs_cam_core::feeds::support::micro_extrapolation_refusal;
    use rs_cam_core::material::PlywoodGrade;
    use rs_cam_core::tool_load::UnmodeledReason;

    // The gate judges the Scallop cell, on the Parallel premise's band.
    let scallop = gate_verdict(
        SAMPLE_ARC_A_RAD,
        true,
        OperationType::Scallop,
        LutOperationFamily::Scallop,
        LutPassRole::Finish,
    );
    assert!(
        !matches!(scallop, ChiploadVerdict::Unmodeled { .. }),
        "the Ø1 tapered Scallop cell in hard maple is judged since the 2026-10-01 ruling; \
         got {scallop:?}"
    );
    let parallel = gate_verdict(
        SAMPLE_ARC_A_RAD,
        true,
        OPERATION_KIND,
        OPERATION_FAMILY,
        PASS_ROLE,
    );
    assert_eq!(
        scallop, parallel,
        "one printed row and one band: the Scallop verdict must be the Parallel verdict"
    );

    let tool = tool();
    let key = rs_cam_core::feeds::geometry::lut_key_diameter_for_cutter(&tool, SAMPLE_AXIAL_DOC_MM);
    assert!(
        (key - TIP_DIAMETER_MM).abs() < 1e-12,
        "ruling A1: a tapered ball is keyed at its tip; got {key}"
    );
    let row = matched_row_for(&tool, key, LutOperationFamily::Scallop, LutPassRole::Finish)
        .expect("the v8 family rule serves the printed tip row to Scallop");
    assert_eq!(row.observation_id, PREMISE_ROW);
    assert_eq!(
        row.size_basis,
        SizeBasis::Exact,
        "the row prints the Ø1 tip"
    );
    let claim = row
        .family_basis
        .claim()
        .expect("the row is filed under parallel / finish, so Scallop reads it transferred");
    assert_eq!(
        claim.home,
        (LutOperationFamily::Parallel, LutPassRole::Finish)
    );
    assert_eq!(
        claim.query,
        (LutOperationFamily::Scallop, LutPassRole::Finish)
    );
    assert_eq!(claim.source_rows, vec![PREMISE_ROW.to_owned()]);
    let band_min = row.chip_load_min_mm.expect("the row prints a minimum");
    let band_max = row.chip_load_max_mm.expect("the row prints a maximum");
    assert!(
        (band_min - PRINTED_MIN_IN * MM_PER_IN).abs() < 1e-12,
        "band min {band_min} must be the printed 0.00075 in x 25.4"
    );
    assert!(
        (band_max - PRINTED_MAX_IN * MM_PER_IN).abs() < 1e-12,
        "band max {band_max} must be the printed 0.002 in x 25.4"
    );

    // The size rule still refuses a Ø1 tip whose only row is more than 2x
    // its size: Baltic birch plywood, where the v8 chart prints no row.
    let plywood = Material::Plywood {
        grade: PlywoodGrade::BalticBirch,
    };
    let verdict = gate_verdict_in(
        &plywood,
        SAMPLE_ARC_A_RAD,
        true,
        OperationType::Scallop,
        LutOperationFamily::Scallop,
        LutPassRole::Finish,
    );
    assert!(
        matches!(
            verdict,
            ChiploadVerdict::Unmodeled {
                reason: UnmodeledReason::NoVendorData
            }
        ),
        "the Ø1 tapered Scallop cell in plywood must stay unjudged under the size rule; \
         got {verdict:?}"
    );
    let query = LookupQuery {
        tool_family: ToolFamily::TaperedBallNose,
        tool_subfamily: None,
        diameter_mm: key,
        flute_count: FLUTES,
        material_family: MaterialFamily::PlywoodHardwood,
        hardness_kind: None,
        hardness_value: None,
        operation_family: LutOperationFamily::Scallop,
        pass_role: LutPassRole::Finish,
    };
    let refused = find_best_chip_envelope_row(
        rs_cam_core::feeds::embedded_vendor_lut(),
        &query,
        &tool.to_geometry_hint(),
    )
    .expect("the envelope resolver still names a row; the size rule refuses it");
    assert_eq!(
        refused.observation_id,
        "onsrud-plywood-hardwood-77-100-1_4-pocket"
    );
    let expected_reason = "no published figure for a 1.00 mm tapered ball nose; the nearest \
                           chart row is 6.35 mm, 6.3x the tool, outside the 0.5x to 2x window \
                           a tool under 1.5 mm needs (ruling R1 applied to size)";
    match &refused.size_basis {
        SizeBasis::Refused { gap, reason } => {
            assert!(
                matches!(gap, Gap::Size),
                "the refusal must be a size gap; got {gap:?}"
            );
            assert_eq!(reason, expected_reason);
        }
        other => panic!("the Ø1 plywood Scallop row must be size-refused; got {other:?}"),
    }
    assert!(
        refused.chip_load_min_mm.is_none() && refused.chip_load_max_mm.is_none(),
        "a refused row publishes no band"
    );

    // The recorded row would refuse too: Ø3.175 is 3.175x the Ø1 tip.
    let recorded_diameter = rs_cam_core::feeds::embedded_vendor_lut()
        .observations
        .iter()
        .find(|o| o.observation_id == RECORDED_ROW)
        .and_then(|o| o.diameter_mm)
        .expect("the recorded row ships with a diameter");
    assert!(
        micro_extrapolation_refusal(
            ToolFamily::TaperedBallNose,
            TIP_DIAMETER_MM,
            key,
            recorded_diameter
        )
        .is_some(),
        "the recorded row (Ø{recorded_diameter}) must be outside the micro window of a Ø1 tip"
    );
}
