//! **A-8 (F-OPT) — the first retarget/reconciliation fixture that measures
//! a real retarget outcome.**
//!
//! # Why this file exists
//!
//! TD2 close-out ledger row **F-OPT**:
//!
//! > The optimizer's retarget multiplier (`retarget/chipload.rs`,
//! > `target / gate-observed`) now divides by a different denominator on
//! > every `ae`-bearing row. Its own tests build synthetic verdicts and
//! > stayed green, so **no evidence measures a real retarget outcome
//! > moving.**
//!
//! Every existing chipload-retarget test hand-builds a `ChiploadVerdict`
//! and a `MatchedRow` (see `retarget::chipload`'s own `mod tests`). Those
//! tests assert the *arithmetic*, and the arithmetic is right. What no test
//! did was let a **simulation** produce the verdict and then ask whether the
//! feed the retargeter picks actually reconciles when the sim is re-run.
//!
//! # Why it lives in `src/` and not `tests/`
//!
//! The retarget stage is assembled from `pub(crate)` parts —
//! `context::find_matched_lut_row`, `search_policy()`, `SearchPolicy` — so
//! an integration test cannot build the retargeter the way
//! `run_retarget_stage` builds it. Reproducing the construction in a
//! `tests/` file would measure the reproduction, not the code. This module
//! calls the same functions the production stage calls.
//!
//! # Instrument integrity
//!
//! * The verdict is produced by `ProjectSession::tool_load_report()` — the
//!   same entry point the GUI diagnostics panel, the CLI report and MCP
//!   read.
//! * The retargeter is constructed from `find_matched_lut_row` +
//!   `search_policy()`, field-for-field as `run_retarget_stage` does.
//! * The candidate sim options come from `candidate::candidate_sim_options`,
//!   so the arms run at the operating point the optimizer actually scores
//!   candidates at — not a hand-picked one.
//! * Both arms differ **only in commanded feed**. Feed moves no geometry, so
//!   the arms are comparable by construction; the fixture additionally
//!   asserts the `StockSnapshotStamp` (S-4) is equal across arms, and states
//!   in the assertion message that on a fresh-stock op that equality is
//!   `None == None` and therefore carries no information on its own.
//! * The simulation cell is a named constant and is printed beside every
//!   measurement.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::compute::StockConfig;
use crate::compute::catalog::{OperationConfig, OptimizationSurface};
use crate::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use crate::compute::operation_configs::PocketConfig;
use crate::compute::stock_config::{ModelKind, ModelUnits};
use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
use crate::debug_trace::ToolpathDebugOptions;
use crate::gcode::CoolantMode;
use crate::geo::P2;
use crate::ids::ToolpathId;
use crate::machine::MachineProfile;
use crate::material::{Material, WoodSpecies};
use crate::polygon::Polygon2;
use crate::session::{LoadedModel, ProjectSession, ToolpathConfig};
use crate::tool_load::verdict::{ChipSide, ChiploadVerdict};

use super::axes::{AxisContext, SearchAxis};
use super::candidate::candidate_sim_options;
use super::context::{EvaluationContext, find_matched_lut_row};
use super::patches::PatchSource;
use super::retarget::Retargeter;
use super::retarget::chipload::ChiploadFeedRetargeter;
use super::search_policy;
use super::space::SearchSpace;

/// Dexel cell for every arm. Named so it can be quoted beside the numbers.
/// Deliberately the optimizer's own refined cell — the one every *reported*
/// candidate verdict is measured at.
const CELL_MM: f64 = 0.5;

/// Ø6.35 flat end mill, 2 flutes — `ToolConfig::new_default`'s end mill.
const TOOL_DIAMETER_MM: f64 = 6.35;
/// Single-pass axial depth. Chosen so `doc / diameter ≈ 3.1`, i.e. past the
/// knee where `geometry::doc_derating_scale` bottoms out at 0.50 — the
/// regime where the gate's ceiling and the retargeter's target are furthest
/// apart. Any value above ~1.67 × diameter reproduces the finding; this one
/// makes the size of it unmistakable.
const DEEP_DEPTH_MM: f64 = 20.0;
/// Control depth. `3.0 / 6.35 = 0.47 < 1`, so `doc_derating_scale` returns
/// 1.0 and the gate's ceiling IS the matched row's maximum. The retarget
/// reconciles here, which is what makes the deep arm's failure attributable
/// to the derate and nothing else.
const SHALLOW_DEPTH_MM: f64 = 3.0;
/// Commanded feed. With the fixture's 18 000 rpm × 2 flutes this is an
/// advance of 0.3333 mm/tooth, comfortably above any Ø6 hardwood pocket
/// band, so the HIGH side trips genuinely rather than by contrivance.
///
/// The high side matters: **F-MISSAE** demotes the low (burn) side to an
/// advisory on 176 of 252 shipped LUT rows via
/// `ChipBoundsSource::low_side_is_advisory`, so a burn-side fixture would
/// usually produce `Within` + advisory and the retargeter — which fires only
/// on `Exceeds` — would never run.
const BASELINE_FEED_MM_MIN: f64 = 12_000.0;

fn fixture_session(depth_mm: f64) -> ProjectSession {
    let mut session = ProjectSession::new_empty();

    let mut stock = StockConfig {
        x: 44.0,
        y: 44.0,
        z: 25.0,
        origin_x: -22.0,
        origin_y: -22.0,
        origin_z: -25.0,
        auto_from_model: false,
        padding: 0.0,
        ..StockConfig::default()
    };
    // Hard maple: a real hardwood row, not the softwood default.
    stock.material = Material::SolidWood {
        species: WoodSpecies::HardMaple,
    };
    session.set_stock_config(stock);

    // A machine whose feed ceiling does not bind the fixture — the point is
    // the chipload band, and a machine clamp would silently become the
    // binding constraint and make the measurement about something else.
    session.set_machine(MachineProfile {
        max_feed_mm_min: 24_000.0,
        max_cutting_feed_mm_min: Some(24_000.0),
        ..MachineProfile::generic_wood_router()
    });

    let tool_idx = session.add_tool(ToolConfig {
        diameter: TOOL_DIAMETER_MM,
        cutting_length: 30.0,
        ..ToolConfig::new_default(ToolId(0), ToolType::EndMill)
    });
    let tool_id = session.tools()[tool_idx].id.0;

    let square = Polygon2::new(vec![
        P2::new(-15.0, -15.0),
        P2::new(15.0, -15.0),
        P2::new(15.0, 15.0),
        P2::new(-15.0, 15.0),
    ]);
    let model_id = session.add_model(LoadedModel {
        id: 0,
        name: "a8 retarget square".to_owned(),
        mesh: None,
        polygons: Some(Arc::new(vec![square])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: std::path::PathBuf::from("synthetic://a8-retarget"),
        kind: Some(ModelKind::Svg),
        units: Some(ModelUnits::Millimeters),
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    });

    let op = OperationConfig::Pocket(PocketConfig {
        depth: depth_mm,
        depth_per_pass: depth_mm,
        feed_rate: BASELINE_FEED_MM_MIN,
        spindle_rpm: Some(18_000),
        ..Default::default()
    });
    let op_type = op.op_type();
    session
        .add_toolpath(
            0,
            ToolpathConfig {
                id: ToolpathId(0),
                name: "a8 deep pocket".to_owned(),
                enabled: true,
                operation: op,
                dressups: DressupConfig::for_op(op_type),
                heights: HeightsConfig::default(),
                tool_id,
                model_id,
                pre_gcode: None,
                post_gcode: None,
                boundary: BoundaryConfig::default(),
                boundary_inherit: true,
                stock_source: StockSource::Fresh,
                coolant: CoolantMode::Off,
                face_selection: None,
                debug_options: ToolpathDebugOptions::default(),
                feeds_provenance: crate::feeds::FeedsProvenance::default(),
                rest_analysis: crate::compute::config::RestAnalysisConfig::default(),
            },
        )
        .expect("add toolpath");
    session
}

/// One measured arm: generate at `feed`, simulate at [`CELL_MM`] under the
/// optimizer's own candidate options, and read the gate.
struct Arm {
    feed: f64,
    verdict: ChiploadVerdict,
    stock_snapshot: Option<crate::compute::config::StockSnapshotStamp>,
    /// The gate's own steady-state **population**, from
    /// `FeedExplanation::gate.sample_count`. X-VAC: a gate handed an empty
    /// population passes and looks healthy, so every arm reports this and
    /// every assertion below is guarded on it.
    ///
    /// Deliberately NOT `triggering.evidence.sample_range.len()` — that is
    /// the index range of the one triggering sample and is 1 by
    /// construction, which would make the guard vacuous in exactly the way
    /// X-VAC warns about.
    gate_population: usize,
}

fn run_arm(session: &mut ProjectSession, feed: f64) -> Arm {
    let cancel = AtomicBool::new(false);
    let mut op = session
        .get_toolpath_config(0)
        .expect("toolpath 0")
        .operation
        .clone();
    op.set_feed_rate(feed);
    let dressups = session
        .get_toolpath_config(0)
        .expect("toolpath 0")
        .dressups
        .clone();
    session
        .apply_toolpath_param_snapshot(0, op, dressups, None)
        .expect("apply feed");
    session
        .generate_toolpath(0, &cancel)
        .expect("generate the fixture");
    let stock_snapshot = session
        .get_result(0)
        .expect("generated result")
        .stats
        .stock_snapshot;
    session
        .run_simulation(&candidate_sim_options(CELL_MM), &cancel)
        .expect("simulate the fixture");

    let tp_id = session.toolpath_configs()[0].id;
    let report = session.tool_load_report();
    let v = report
        .per_toolpath
        .iter()
        .find(|v| v.toolpath_id == tp_id)
        .expect("the fixture toolpath has a load verdict");
    Arm {
        feed,
        verdict: v.chipload.clone(),
        stock_snapshot,
        gate_population: v
            .feed_explanation
            .as_deref()
            .map_or(0, |e| e.gate.sample_count),
    }
}

/// `(observed advance per tooth, the ceiling the GATE compared it against)`.
///
/// The second value is the load-bearing one: it is
/// `ChiploadMetric::bounds.max_mm_per_tooth`, i.e. the **DOC-derated** band
/// the gate judged by. Since Checkpoint P (1a) it is also the band the
/// retargeter aims at — the point of the ruling was to make those one number
/// rather than two.
fn observed_and_ceiling(v: &ChiploadVerdict) -> (f64, f64) {
    match v {
        ChiploadVerdict::Exceeds { triggering, .. } => (
            triggering.observed_mm_per_tooth,
            triggering.bounds.max_mm_per_tooth,
        ),
        ChiploadVerdict::Within {
            approach_to_max, ..
        } => (
            approach_to_max.observed_mm_per_tooth,
            approach_to_max.bounds.max_mm_per_tooth,
        ),
        ChiploadVerdict::Unmodeled { reason } => {
            panic!("fixture produced no chipload measurement at all: {reason:?}")
        }
    }
}

/// One measured retarget round-trip at a given single-pass axial depth.
struct Round {
    doc_over_diameter: f64,
    /// The chipload band the gate judged the BASELINE by (DOC-derated).
    gate_ceiling: f64,
    /// The matched vendor row's own maximum — diameter/hardness-scaled,
    /// **not** DOC-derated. Until Checkpoint P (1a) this is what the
    /// retargeter was handed; it is kept so the inversion can be quoted
    /// against it rather than reconstructed.
    row_max: f64,
    /// `gate_ceiling / high_headroom` — what the retargeter aims the feed at
    /// since P-(1a): the DOC-derated band off the verdict, with headroom.
    retarget_target: f64,
    /// `row_max / high_headroom` — what it aimed at BEFORE P-(1a). Equal to
    /// [`Self::retarget_target`] exactly while the derate is inactive, which
    /// is why the control arm did not move.
    pre_p1a_target: f64,
    baseline_feed: f64,
    retargeted_feed: f64,
    /// Advance per tooth the re-simulation actually measured at the
    /// retargeted feed.
    reconciled_observed: f64,
    /// The gate's verdict after the re-simulation.
    reconciled_is_exceeds: bool,
    row_id: String,
    diameter_scale: f64,
    hardness_scale: f64,
    extrapolated: bool,
}

/// Suggest-side recipe → sim → the REAL retargeter → re-sim, with every
/// number read off the production path. Returns the round for a caller to
/// assert on.
fn measure_retarget_round(depth_mm: f64) -> Round {
    let mut session = fixture_session(depth_mm);

    // ── Arm A — baseline ────────────────────────────────────────────
    let baseline = run_arm(&mut session, BASELINE_FEED_MM_MIN);
    let (obs_a, ceiling_a) = observed_and_ceiling(&baseline.verdict);

    println!("\n=== A-8 retarget reconciliation, cell {CELL_MM} mm, DOC {depth_mm} mm ===");
    println!(
        "arm A (baseline)   feed {:>9.1}  observed {obs_a:.5}  gate ceiling {ceiling_a:.5}  \
         gate population n={}  verdict {:?}",
        baseline.feed,
        baseline.gate_population,
        baseline.verdict.state()
    );

    // X-VAC: a gate handed an empty population passes and looks healthy.
    assert!(
        baseline.gate_population > 0,
        "the baseline gate ran on an EMPTY steady-state population — every \
         verdict below would be vacuous"
    );
    let ChiploadVerdict::Exceeds {
        side: ChipSide::High,
        ..
    } = &baseline.verdict
    else {
        panic!(
            "the fixture must trip the HIGH side for the retargeter to fire at \
             all (F-MISSAE suppresses the low side on 176/252 rows); got {:?}",
            baseline.verdict
        );
    };

    // ── The retargeter, assembled exactly as `run_retarget_stage` does ──
    let ctx = EvaluationContext::from_session(&session, 0).expect("evaluation context");
    let baseline_op = session
        .get_toolpath_config(0)
        .expect("toolpath 0")
        .operation
        .clone();
    let machine = session.machine().clone();
    let policy = search_policy();
    let row = find_matched_lut_row(&ctx.tool, &ctx.material, &ctx, baseline_op.depth_per_pass())
        .expect("the fixture must match a chipload-bearing LUT row");
    let view = match baseline_op.optimization_surface() {
        OptimizationSurface::Optimizable(v) => v,
        OptimizationSurface::NotOptimizable { reason } => {
            panic!("pocket must be optimizable: {reason:?}")
        }
    };
    let axis_ctx = AxisContext {
        project_default_rpm: 18_000,
        machine: &machine,
        tool: &ctx.tool,
        material: &ctx.material,
    };
    let space = SearchSpace::build(&view, &axis_ctx, Some(&row), policy);
    let retargeter = ChiploadFeedRetargeter {
        low_headroom: policy.retarget.chipload_low_headroom.value,
        high_headroom: policy.retarget.chipload_high_headroom.value,
        plunge_tracking_threshold: policy.feed.plunge_tracking_threshold_fraction.value,
    };

    let solution = retargeter
        .target(&baseline.verdict, &space, &view, &axis_ctx)
        .expect("the retargeter must fire on an Exceeds(High) verdict");
    let feed_patch = solution
        .patches
        .iter()
        .find(|p| p.axis == SearchAxis::FeedRate && p.source == PatchSource::Primary)
        .expect("a chipload retarget emits a primary feed patch");
    let retargeted_feed = feed_patch.value;

    let row_max = row.chip_load_max_mm.expect("matched row publishes a max");
    let headroom = policy.retarget.chipload_high_headroom.value;
    // Checkpoint P (1a): the retargeter aims at the band the GATE used.
    let retarget_target = ceiling_a / headroom;
    // What it aimed at before P-(1a), retained so the inversion is quotable.
    let pre_p1a_target = row_max / headroom;

    println!(
        "row {} (d={:.3} mm, Ø×{:.3}, Janka×{:.3}, extrapolated {})",
        row.observation_id,
        row.row_diameter_mm,
        row.chipload_diameter_scale,
        row.chipload_hardness_scale,
        row.is_extrapolated
    );
    println!(
        "row max (Ø/Janka-scaled) {row_max:.5}   gate's DOC-derated ceiling \
         {ceiling_a:.5}   derate ×{:.4}",
        ceiling_a / row_max
    );
    println!(
        "retarget target {retarget_target:.5} = gate ceiling / headroom \
         {headroom:.2}    target / gate ceiling = {:.4}   \
         (pre-P-(1a) target was {pre_p1a_target:.5} = {:.4}x the ceiling)",
        retarget_target / ceiling_a,
        pre_p1a_target / ceiling_a
    );
    println!(
        "retarget feed   {retargeted_feed:>9.1}  ({:.4}x baseline)   rationale: {}",
        retargeted_feed / baseline.feed,
        solution.rationale
    );

    // ── Arm B — the retargeted feed, re-simulated ───────────────────
    let retargeted = run_arm(&mut session, retargeted_feed);
    let (obs_b, ceiling_b) = observed_and_ceiling(&retargeted.verdict);
    println!(
        "arm B (retargeted) feed {:>9.1}  observed {obs_b:.5}  gate ceiling {ceiling_b:.5}  \
         gate population n={}  verdict {:?}",
        retargeted.feed,
        retargeted.gate_population,
        retargeted.verdict.state()
    );
    assert!(
        retargeted.gate_population > 0,
        "the re-simulated gate ran on an EMPTY steady-state population"
    );

    // Comparability. Feed moves no geometry, so both arms machine the same
    // stock; S-4's stamp is asserted equal to prove it rather than assume
    // it. STATED so the assertion is not over-read: on a fresh-stock op
    // both stamps are `None`, and `None == None` carries no information —
    // the load-bearing comparability argument here is that the only field
    // differing between the arms is `feed_rate`.
    assert_eq!(
        baseline.stock_snapshot, retargeted.stock_snapshot,
        "arms must consume the same machined stock to be comparable"
    );

    // The gate's ceiling is a property of the geometry, not the feed, so it
    // must not have moved between arms. If it did, the comparison below is
    // between two different bars.
    assert!(
        (ceiling_a - ceiling_b).abs() < 1e-12,
        "the gate ceiling moved between arms ({ceiling_a:.6} -> {ceiling_b:.6}); \
         feed is not supposed to move the band"
    );

    println!(
        "reconciliation: observed {obs_b:.5} vs gate ceiling {ceiling_b:.5} -> {:.4}x ({})\n",
        obs_b / ceiling_b,
        if obs_b > ceiling_b {
            "STILL OVER"
        } else {
            "inside"
        }
    );

    // The retargeter is linear in feed and lands where it aims. Asserted
    // here, once, so both callers can reason about the TARGET rather than
    // re-deriving the landing.
    assert!(
        (obs_b / retarget_target - 1.0).abs() < 0.05,
        "the retarget is linear in feed and must land within 5 % of its own \
         target: observed {obs_b:.5} vs target {retarget_target:.5}"
    );

    Round {
        doc_over_diameter: depth_mm / TOOL_DIAMETER_MM,
        gate_ceiling: ceiling_a,
        row_max,
        retarget_target,
        pre_p1a_target,
        baseline_feed: baseline.feed,
        retargeted_feed,
        reconciled_observed: obs_b,
        reconciled_is_exceeds: matches!(retargeted.verdict, ChiploadVerdict::Exceeds { .. }),
        row_id: row.observation_id.clone(),
        diameter_scale: row.chipload_diameter_scale,
        hardness_scale: row.chipload_hardness_scale,
        extrapolated: row.is_extrapolated,
    }
}

/// **Control arm — the fixture contains its mechanism.**
///
/// At a shallow single pass (`DOC/Ø < 1`) `geometry::doc_derating_scale`
/// returns 1.0, so the gate's ceiling and the retargeter's row *are* the
/// same number. The retarget then reconciles: the re-simulated verdict is
/// `Within`.
///
/// This is what makes the deep case below attributable. Without it, "the
/// retarget did not reconcile" could be any of a dozen things — a clamp, a
/// stale trace, a population artifact. With it, the only difference between
/// the two rounds is the DOC derate.
///
/// **Checkpoint P (1a) did not move this arm**, and that is load-bearing: at
/// `DOC/Ø < 1` the raw row and the derated band are the same number, so the
/// old target and the new one are bit-identical. Asserted below rather than
/// argued, so the deep arm's inversion is attributable to the derate alone.
#[test]
fn a_retarget_reconciles_while_the_doc_derate_is_inactive() {
    let shallow = measure_retarget_round(SHALLOW_DEPTH_MM);

    // P-(1a) control: with the derate inactive the two bands coincide, so the
    // change of band source is a no-op here. Feed measured unchanged at
    // 1708.1 mm/min across the ruling.
    assert!(
        (shallow.retarget_target - shallow.pre_p1a_target).abs() < 1e-12,
        "CONTROL: P-(1a) must be a no-op below the knee; new target {:.6} vs \
         pre-P-(1a) {:.6}",
        shallow.retarget_target,
        shallow.pre_p1a_target
    );
    assert!(
        (shallow.retargeted_feed - 1708.1).abs() < 1.0,
        "CONTROL: the control arm's feed is unchanged across P-(1a) \
         (A-8 measured 1708.1 mm/min); got {:.1}",
        shallow.retargeted_feed
    );

    assert!(
        shallow.doc_over_diameter < 1.0,
        "control arm must sit below the derate knee, got {:.3}",
        shallow.doc_over_diameter
    );
    assert!(
        (shallow.gate_ceiling / shallow.row_max - 1.0).abs() < 1e-9,
        "with no derate the gate ceiling IS the row max: {:.6} vs {:.6}",
        shallow.gate_ceiling,
        shallow.row_max
    );
    assert!(
        shallow.retarget_target < shallow.gate_ceiling,
        "the retarget target must sit below the bar it will be judged by"
    );
    assert!(
        !shallow.reconciled_is_exceeds,
        "CONTROL: with the derate inactive the retarget must reconcile to \
         Within; observed {:.5} vs ceiling {:.5}",
        shallow.reconciled_observed, shallow.gate_ceiling
    );
}

/// **The finding arm (F-OPT), INVERTED by Checkpoint P (1a) on 2026-08-14.**
///
/// # What this test asserted before, and what it asserts now
///
/// A-8 wrote this arm to pin the *defect*, and said in its own words that
/// when the ruling landed it must be inverted deliberately with the old and
/// new numbers recorded. This is that inversion. Measured on this fixture
/// (Ø6.35 flat 2F, hard maple, 20 mm single pass, `DOC/Ø = 3.15`, cell
/// 0.5 mm, row `amana-flat-hardwood-pocket-6000-2f`):
///
/// | quantity | before P-(1a) | after P-(1a) |
/// |---|---|---|
/// | retarget target | 0.04745 (= row max / 1.2) | 0.02372 (= gate ceiling / 1.2) |
/// | target ÷ gate ceiling | **1.6667** (above the bar) | **0.8333** (below the bar) |
/// | retargeted feed | 1708.1 mm/min | 854.1 mm/min (0.5×) |
/// | re-simulated verdict | `Exceeds(High)` | `Within` |
///
/// The old assertion was `deep.reconciled_is_exceeds`; it is now
/// `!deep.reconciled_is_exceeds`. Nothing else about the fixture moved — same
/// tool, same stock, same cell, same matched row, same gate population.
///
/// # Mechanism, for the record
///
/// `run_retarget_stage` used to hand `ChiploadFeedRetargeter` the **raw**
/// matched row (diameter- and hardness-scaled, *not* DOC-derated) while the
/// chipload gate compares against `geometry::derate_chipload_bounds(...)` of
/// that same row at the measured peak axial DOC. Past `DOC/Ø ≈ 1.67` the
/// target `row_max / 1.2` sat above the gate's own ceiling, and the derated
/// bar bottoms out at `0.50 × row_max` from `DOC/Ø ≥ 3` — so a retarget that
/// landed perfectly on its own target still read `Exceeds(High)`, burning a
/// full generate + simulate on a guaranteed-rejected candidate.
///
/// The retargeter now reads `ChiploadMetric::bounds` off the verdict it is
/// handed, so there is no second band to diverge from.
///
/// **The control arm is what makes this attributable**: it did not move (the
/// derate is inactive there, so the two bands were already the same number),
/// which rules out a clamp, a stale trace or a population artifact.
#[test]
fn a_retarget_reconciles_once_it_reads_the_derated_band() {
    let deep = measure_retarget_round(DEEP_DEPTH_MM);

    assert!(
        deep.doc_over_diameter >= 3.0,
        "the finding arm must sit past the derate knee, got {:.3}",
        deep.doc_over_diameter
    );

    // The derate is still there — P-(1a) did not touch it. What changed is
    // which of the two bands the retargeter aims at.
    let derate = deep.gate_ceiling / deep.row_max;
    assert!(
        (derate - 0.5).abs() < 1e-9,
        "past DOC/D = 3 the derate is 0.50; measured {derate:.6}"
    );

    // OLD (pre-P-(1a)): this ratio was 1.6667 — the target sat ABOVE the bar.
    let pre_over_ceiling = deep.pre_p1a_target / deep.gate_ceiling;
    assert!(
        (pre_over_ceiling - 5.0 / 3.0).abs() < 1e-9,
        "the pre-P-(1a) target must still reproduce at 1.6667x the bar, or \
         this inversion is not comparable to what A-8 measured; got \
         {pre_over_ceiling:.6}"
    );

    // NEW: the target is the gate's own ceiling with headroom, so it sits
    // BELOW the bar by exactly 1/1.2 whatever the derate does.
    let target_over_ceiling = deep.retarget_target / deep.gate_ceiling;
    assert!(
        (target_over_ceiling - 1.0 / 1.2).abs() < 1e-9,
        "P-(1a): the retargeter aims at gate_ceiling/1.2, i.e. 0.8333x the \
         bar, at every depth. Measured {target_over_ceiling:.6} (target \
         {:.6}, ceiling {:.6}, row {} Ox{:.3} Jankax{:.3} extrapolated {})",
        deep.retarget_target,
        deep.gate_ceiling,
        deep.row_id,
        deep.diameter_scale,
        deep.hardness_scale,
        deep.extrapolated
    );

    // The feed drops by exactly the derate. The retarget multiplier is
    // `target / observed_peak` and only the numerator moved, so the feed this
    // arm emits is `derate` times the one A-8 measured: 1708.1 -> 854.1.
    // This is the "retargeted feeds drop up to 2x" the ruling anticipated,
    // measured rather than asserted.
    let pre_p1a_feed = deep.retargeted_feed / derate;
    assert!(
        (pre_p1a_feed - 1708.1).abs() < 1.0,
        "the reconstructed pre-P-(1a) feed must match A-8's measured 1708.1 \
         mm/min or the arms are not comparable; got {pre_p1a_feed:.1} from \
         retargeted {:.1} / derate {derate:.4}",
        deep.retargeted_feed
    );

    assert!(
        !deep.reconciled_is_exceeds,
        "P-(1a) INVERSION: the retarget must now reconcile to Within. \
         baseline feed {:.1} -> retargeted {:.1}, observed {:.5}, ceiling \
         {:.5} (was Exceeds at 1708.1 mm/min before P-(1a))",
        deep.baseline_feed, deep.retargeted_feed, deep.reconciled_observed, deep.gate_ceiling
    );
}
