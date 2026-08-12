//! **A-5 (F-T35) — the `arc_fit_ratio_for_op` evidence package.**
//!
//! # What is being measured and why
//!
//! `feeds::predict::arc_fit_ratio_for_op` carries a per-operation-family
//! table (Adaptive3d 0.25, DropCutter 0.15, `Calibrated`; everything else a
//! `Default`) that was fitted against the post-sim chipload gate's **arc-mean
//! chip thickness** observation. That observation was **deleted on
//! 2026-08-06**: the gate now reports `effective_feed / (rpm · flutes)`, a
//! linear advance per tooth (`feeds::explanation`'s module header, and
//! `planning/review_2026-08-04/CHIPLOAD_LITERATURE_VERDICT.md` for the
//! primary sources).
//!
//! The table did not follow. `suggest::recalibrate_feed_for_chipload` still
//! solves
//!
//! ```text
//! feed = target / arc_fit_ratio × rpm × flutes
//! ```
//!
//! and is gated on `ArcFitRatioSource::Calibrated`, so exactly two families
//! — Adaptive3d and DropCutter — get an automatic feed lift, sized by
//! `1 / ratio`, aimed at a quantity nothing reports any more.
//!
//! This harness measures what that costs, on the real Suggest path, and what
//! each of the three dispositions the operator review put on the table would
//! do to the shipped recommendation. It is the evidence for **Checkpoint J**.
//!
//! # The three dispositions, as arms
//!
//! | arm | disposition | feed applied to the measured op |
//! |---|---|---|
//! | **A — shipped** | today's behaviour | Suggest's own recommended feed, lift included |
//! | **B — retire** | (a) retire the automatic feed-up entirely | `FeedRaisedForChipload::requested_mm_per_min`, i.e. the feed Suggest had *before* pass 8 |
//! | **C — re-key to 1.0** | the reference point the 4×/6.7× claim is stated against | `min(target × rpm × flutes, cutting ceiling)` — the lift the *current* gate observation would justify |
//!
//! Disposition (b) — demote to a clearly-marked estimated mode — moves no
//! number and therefore has no arm: it is arm A with a label, which is why
//! the report-tier wording change ships separately and before the ruling.
//! Disposition (c) — move the action into the simulation-backed optimizer —
//! has no arm either, for a stronger reason: Suggest would emit arm B and
//! the optimizer would retarget from the *measured* gate observation, which
//! is arm B's measured column in this file. Both are read off the arms that
//! exist rather than being simulated as fourth and fifth arms.
//!
//! # Instrument integrity
//!
//! * Suggest-side numbers come from `ProjectSession::cutter_op_profile` — the
//!   same assembly the GUI Suggest button, the MCP rationale endpoint and the
//!   CLI `--apply-suggest` path use — never from a hand-computed formula.
//! * Sim-side numbers come from `ToolpathLoadVerdict::feed_explanation`,
//!   the stage-labelled record: stage 1 commanded advance/tooth, stage 2
//!   vendor band, stage 3 achieved/commanded feed ratio, stage 4 the gate's
//!   own observation. All four in `ADVANCE_PER_TOOTH`, so every ratio in the
//!   report is a like-for-like comparison.
//! * Every arm reads `ToolpathStats::stock_snapshot` (S-4's `StockSnapshotStamp`)
//!   and the A/B/C comparison **asserts the three arms consumed the same
//!   machined stock**. The arms differ in feed only; feed does not move
//!   geometry, so an unequal stamp would mean the comparison was unfair
//!   before any conclusion about feed could be drawn.
//! * The simulation cell is a named constant and is reported beside every
//!   measurement.
//!
//! # Fixture shape
//!
//! Every fixture is a two-op cascade on the same synthetic surface: op 0 is a
//! fresh-stock rough, op 1 is the **measured** op reading the stock op 0 left
//! (`StockSource::FromRemainingStock`). The cascade is what makes the
//! snapshot stamp `Some` — a fresh-stock generation consumes no machined
//! stock and would stamp `None`, making the equality assertion vacuous.
//!
//! Two fixtures per affected family, differing in tool, species and machine
//! feed ceiling, because whether the machine ceiling binds decides whether
//! the lift is realised at all.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
// The measured numbers ARE the record.
#![allow(clippy::print_stdout, clippy::print_stderr)]

mod common;

use std::sync::atomic::AtomicBool;

use common::meshes::height_field;
use common::session::{mesh_model, pinned_heights, stock_over, toolpath_config};
use common::tools::{ball_tool_config, endmill_tool_config, tapered_ball_tool_config};

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{StockSnapshotStamp, StockSource};
use rs_cam_core::compute::operation_configs::{
    Adaptive3dConfig, Adaptive3dEntryStyle, DropCutterConfig,
};
use rs_cam_core::compute::tool_config::ToolConfig;
use rs_cam_core::feeds::suggest::{FeedRecalibrationCap, SuggestWarning};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::session::{ProjectSession, SimulationOptions};

// ── Fixture constants ───────────────────────────────────────────────────

/// Half-extent (mm) of the fixture surface.
const HALF: f64 = 15.0;

/// Stock height (mm).
const STOCK_Z: f64 = 8.0;

/// Peak-to-trough of the fixture surface (mm).
const RELIEF: f64 = 3.0;

/// The simulation cell (mm). Reported beside every measurement, per §0
/// rule 10 — a collision count or an engagement reading without its cell
/// is not a measurement.
const SIM_CELL_MM: f64 = 0.4;

/// A smooth double bump occupying the top `RELIEF` mm of the stock, so the
/// rough leaves real material for the measured op to cut.
fn bumpy_surface() -> rs_cam_core::mesh::TriangleMesh {
    height_field(HALF, 0.5, |x, y| {
        let sx = (x / HALF * std::f64::consts::PI).cos();
        let sy = (y / HALF * std::f64::consts::PI).cos();
        STOCK_Z - RELIEF * 0.5 * (1.0 - sx * sy * 0.9)
    })
}

// ── Fixtures ────────────────────────────────────────────────────────────

struct Fixture {
    label: &'static str,
    family: &'static str,
    material: Material,
    /// Tool for op 0 (the upstream rough). Always a flat endmill.
    rough_tool: ToolConfig,
    /// Tool for op 1 (the measured op).
    tool: ToolConfig,
    /// The measured operation, with `spindle_rpm` pinned.
    op: OperationConfig,
    /// RPM pinned on the measured op. Every arm applies this same RPM, so
    /// the A/B/C comparison is attributable to the feed alone.
    rpm: u32,
    /// `max_feed_mm_min` for the fixture's machine.
    max_feed_mm_min: f64,
    /// Explicit `max_cutting_feed_mm_min`. `None` leaves the conservative
    /// derived ceiling `min(max_feed, DEFAULT_CUTTING_FEED_CAP_MM_MIN = 6000)`.
    ///
    /// This dial is load-bearing for the evidence, not fixture decoration:
    /// whether the ceiling binds decides whether the arc-fit lift is
    /// *realised* or merely *solved for*, and the two differ by a lot.
    max_cutting_feed_mm_min: Option<f64>,
}

fn rough_tool() -> ToolConfig {
    let mut t = endmill_tool_config(6.0);
    t.stickout = 20.0;
    t.flute_count = 2;
    t
}

fn adaptive3d_op(feed: f64, rpm: u32, stepover: f64, dpp: f64) -> OperationConfig {
    OperationConfig::Adaptive3d(Adaptive3dConfig {
        stepover,
        depth_per_pass: dpp,
        feed_rate: feed,
        plunge_rate: 300.0,
        spindle_rpm: Some(rpm),
        // Helix entry so the fixture does not also trip
        // `PlungeEntryUnstableAtDpp` and confound the warning list.
        entry_style: Adaptive3dEntryStyle::Helix,
        ..Adaptive3dConfig::default()
    })
}

fn drop_cutter_op(feed: f64, rpm: u32, stepover: f64) -> OperationConfig {
    OperationConfig::DropCutter(DropCutterConfig {
        stepover,
        feed_rate: feed,
        plunge_rate: 300.0,
        spindle_rpm: Some(rpm),
        min_z: STOCK_Z - RELIEF - 1.0,
        ..DropCutterConfig::default()
    })
}

fn fixtures() -> Vec<Fixture> {
    let mut a3d_2_tool = endmill_tool_config(8.0);
    a3d_2_tool.stickout = 25.0;
    a3d_2_tool.flute_count = 3;

    let mut a3d_1_tool = endmill_tool_config(6.0);
    a3d_1_tool.stickout = 18.0;
    a3d_1_tool.flute_count = 2;

    let mut dc_1_tool = ball_tool_config(3.0);
    dc_1_tool.stickout = 20.0;
    dc_1_tool.flute_count = 2;

    let mut dc_2_tool = tapered_ball_tool_config(2.0, 7.0, 6.0);
    dc_2_tool.stickout = 30.0;
    dc_2_tool.flute_count = 2;

    vec![
        // Uncapped: isolates the closed form, so the 1/ratio claim is
        // measured rather than inferred through a ceiling.
        Fixture {
            label: "A3D-1 Ø6 2F endmill / HardMaple / ceiling lifted",
            family: "Adaptive3d",
            material: Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            rough_tool: rough_tool(),
            tool: a3d_1_tool,
            op: adaptive3d_op(900.0, 16_000, 1.2, 2.0),
            rpm: 16_000,
            max_feed_mm_min: 15_000.0,
            max_cutting_feed_mm_min: Some(15_000.0),
        },
        // Stock ceiling: what an operator on a normal machine actually gets.
        Fixture {
            label: "A3D-2 Ø8 3F endmill / WhiteOak / stock ceiling",
            family: "Adaptive3d",
            material: Material::SolidWood {
                species: WoodSpecies::WhiteOak,
            },
            rough_tool: rough_tool(),
            tool: a3d_2_tool,
            op: adaptive3d_op(1100.0, 14_000, 1.6, 2.0),
            rpm: 14_000,
            max_feed_mm_min: 8_000.0,
            max_cutting_feed_mm_min: None,
        },
        Fixture {
            label: "DC-1 Ø3 ball / HardMaple / stock ceiling",
            family: "DropCutter",
            material: Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            rough_tool: rough_tool(),
            tool: dc_1_tool,
            op: drop_cutter_op(1200.0, 18_000, 0.35),
            rpm: 18_000,
            max_feed_mm_min: 12_000.0,
            max_cutting_feed_mm_min: None,
        },
        // A small router: the ceiling binds on a DropCutter lift too.
        Fixture {
            label: "DC-2 Ø2-tip tapered ball / WhiteOak / small router",
            family: "DropCutter",
            material: Material::SolidWood {
                species: WoodSpecies::WhiteOak,
            },
            rough_tool: rough_tool(),
            tool: dc_2_tool,
            op: drop_cutter_op(1000.0, 20_000, 0.25),
            rpm: 20_000,
            max_feed_mm_min: 2_500.0,
            max_cutting_feed_mm_min: None,
        },
    ]
}

/// Two-op cascade: op 0 fresh-stock rough, op 1 the measured op reading what
/// op 0 left.
fn build_session(fx: &Fixture) -> ProjectSession {
    let mut session = ProjectSession::new_empty();

    let mut stock = stock_over(HALF, STOCK_Z);
    stock.material = fx.material.clone();
    session.set_stock_config(stock);

    session.set_machine(MachineProfile {
        max_feed_mm_min: fx.max_feed_mm_min,
        max_cutting_feed_mm_min: fx.max_cutting_feed_mm_min,
        ..MachineProfile::default()
    });

    let rt = session.add_tool(fx.rough_tool.clone());
    let rough_id = session.tools()[rt].id.0;
    let mt = session.add_tool(fx.tool.clone());
    let meas_id = session.tools()[mt].id.0;
    let model_id = session.add_model(mesh_model(bumpy_surface(), "a5_bumps"));

    let heights = pinned_heights(STOCK_Z, STOCK_Z - RELIEF);

    let mut rough = toolpath_config(
        "Rough (upstream, fresh stock)",
        adaptive3d_op(1500.0, 16_000, 2.5, 3.0),
        rough_id,
        model_id,
    );
    rough.heights = heights.clone();
    session.add_toolpath(0, rough).expect("add rough op");

    let mut measured = toolpath_config(fx.label, fx.op.clone(), meas_id, model_id);
    measured.heights = heights;
    measured.stock_source = StockSource::FromRemainingStock;
    session.add_toolpath(0, measured).expect("add measured op");

    session
}

// ── Suggest-side read ───────────────────────────────────────────────────

/// Everything the real Suggest path says about the measured op, before any
/// simulation. Field names use A-2's canonical vocabulary.
#[derive(Debug, Clone)]
struct SuggestRead {
    /// The arc-fit ratio the predictor applied, and its provenance tag.
    arc_fit_ratio: f64,
    arc_fit_source: String,
    /// `FeedRaisedForChipload::requested_mm_per_min` — Suggest's feed with
    /// pass 8 (the arc-fit recalibration) removed. This IS disposition (a).
    requested_feed: f64,
    /// `FeedRaisedForChipload::raised_mm_per_min` — Suggest's shipped feed.
    raised_feed: f64,
    /// Which cap, if any, bound the solve.
    cap_hit: Option<FeedRecalibrationCap>,
    /// `lut_target_mm_per_tooth` — the band target the solve aimed at.
    lut_target: f64,
    /// The derated vendor band on the matched row.
    band: Option<(f64, f64)>,
    /// `feeds_result.rpm` — what the calculator recommended, for the record.
    feeds_rpm: f64,
    /// The RPM on the operation the fixture was *authored* with, kept only
    /// so the report can show that Suggest moved it.
    authored_rpm: u32,
    /// The feed on `suggested_operation` — the number the GUI writes.
    final_feed: f64,
    /// The machine's cutting-feed ceiling.
    cutting_ceiling: f64,
    flutes: u32,
    /// **The RPM the recalibration actually solved against**, read off
    /// `suggested_operation` (falling back to `feeds_result.rpm`).
    ///
    /// This is not the RPM the fixture was authored with. An earlier Suggest
    /// pass rewrites `spindle_rpm` on the operation before pass 8 runs, so
    /// the closed form `feed = target / ratio × rpm × flutes` uses the
    /// *rewritten* RPM. Deriving arm C from the authored RPM instead put
    /// DC-1's ratio at 7.04× against a closed-form 6.67× — the harness's own
    /// first red, and the reason this field exists.
    rpm: u32,
}

impl SuggestRead {
    /// Commanded advance per tooth (mm) at the pre-recalibration feed.
    fn commanded_fpt_before(&self) -> f64 {
        self.requested_feed / (f64::from(self.rpm) * f64::from(self.flutes))
    }
    /// Commanded advance per tooth (mm) at the shipped feed.
    fn commanded_fpt_after(&self) -> f64 {
        self.raised_feed / (f64::from(self.rpm) * f64::from(self.flutes))
    }
    /// Arm C's feed: what the solve would produce with the table retired to
    /// 1.0, i.e. aimed at the band target in the unit the gate now reports.
    fn arm_c_feed(&self) -> f64 {
        (self.lut_target * f64::from(self.rpm) * f64::from(self.flutes)).min(self.cutting_ceiling)
    }
}

fn suggest_read(session: &ProjectSession, fx: &Fixture) -> SuggestRead {
    let tc = &session.toolpath_configs()[1];
    let profile = session
        .cutter_op_profile(tc)
        .expect("measured op resolves a tool");
    assert!(
        profile.feasibility.is_ok(),
        "{}: Suggest refused the tool × operation pairing: {:?}",
        fx.label,
        profile.feasibility
    );

    let pred = &profile.predictions.observed_chipload;
    let feeds = profile
        .feeds
        .as_ref()
        .expect("feasibility Ok ⟹ feeds present");
    let suggested = profile
        .suggested_operation
        .as_ref()
        .expect("feasibility Ok ⟹ suggested operation present");

    let lift = profile.warnings.iter().find_map(|w| match w {
        SuggestWarning::FeedRaisedForChipload {
            requested_mm_per_min,
            raised_mm_per_min,
            lut_target_mm_per_tooth,
            cap_hit,
            ..
        } => Some((
            *requested_mm_per_min,
            *raised_mm_per_min,
            *lut_target_mm_per_tooth,
            *cap_hit,
        )),
        _ => None,
    });

    let (requested_feed, raised_feed, lut_target, cap_hit) = lift.unwrap_or_else(|| {
        panic!(
            "{}: FeedRaisedForChipload must fire — this fixture exists to measure the lift. \
             warnings: {:?}",
            fx.label, profile.warnings
        )
    });

    SuggestRead {
        arc_fit_ratio: pred.arc_fit_ratio,
        arc_fit_source: format!("{:?}", pred.source),
        requested_feed,
        raised_feed,
        cap_hit,
        lut_target,
        band: feeds
            .chipload_bounds
            .map(|b| (b.min_mm_per_tooth, b.max_mm_per_tooth)),
        feeds_rpm: feeds.rpm,
        authored_rpm: fx.rpm,
        final_feed: suggested.feed_rate(),
        cutting_ceiling: session.machine().cutting_feed_ceiling_mm_min(),
        flutes: fx.tool.flute_count,
        rpm: suggested
            .spindle_rpm()
            .unwrap_or_else(|| feeds.rpm.round() as u32),
    }
}

// ── Step 1–3: the Suggest-side before/after table ───────────────────────

/// **The number Checkpoint J is about, re-measured.**
///
/// Prints, per fixture: the arc-fit ratio and its provenance, the feed
/// Suggest ships, the feed each disposition would ship, and the ratio
/// between them. The review quotes 4× (Adaptive3d) and 6.7× (DropCutter);
/// this test measures it rather than quoting it, and reports separately
/// whether the machine ceiling turns the nominal ratio into a smaller
/// realised one.
#[test]
fn arc_fit_lift_before_after_by_family() {
    println!("\n=== A-5 step 2/3 — Suggest-side before/after (no simulation) ===");
    println!(
        "{:<46} {:>7} {:>11} {:>10} {:>10} {:>10} {:>9} {:>9}",
        "fixture", "ratio", "source", "feed_B(a)", "feed_A", "feed_C", "A/B", "A/C"
    );

    for fx in fixtures() {
        let session = build_session(&fx);
        let r = suggest_read(&session, &fx);

        let arm_c = r.arm_c_feed();
        let a_over_b = r.raised_feed / r.requested_feed;
        let a_over_c = r.raised_feed / arm_c;

        println!(
            "{:<46} {:>7.2} {:>11} {:>10.1} {:>10.1} {:>10.1} {:>8.2}× {:>8.2}×",
            fx.label,
            r.arc_fit_ratio,
            r.arc_fit_source,
            r.requested_feed,
            r.raised_feed,
            arm_c,
            a_over_b,
            a_over_c,
        );
        println!(
            "      band {:?}  target {:.5}  cap {:?}  ceiling {:.0}  \
             rpm solved-against {} (authored {}, feeds.rpm {:.0})  flutes {}",
            r.band,
            r.lut_target,
            r.cap_hit,
            r.cutting_ceiling,
            r.rpm,
            r.authored_rpm,
            r.feeds_rpm,
            r.flutes
        );
        println!(
            "      commanded advance/tooth  before {:.5}  after {:.5} mm  (band max {:.5})",
            r.commanded_fpt_before(),
            r.commanded_fpt_after(),
            r.band.map_or(f64::NAN, |b| b.1),
        );

        // ── Structural assertions (what must not silently change) ──────
        assert_eq!(
            r.arc_fit_source, "Calibrated",
            "{}: the lift only fires on Calibrated rows; this fixture must be one",
            fx.label
        );
        assert!(
            r.raised_feed > r.requested_feed,
            "{}: the lift must raise feed ({} → {})",
            fx.label,
            r.requested_feed,
            r.raised_feed
        );
        // The closed form the disposition question turns on: when no cap
        // binds, arm A is exactly 1/ratio times arm C.
        if r.cap_hit.is_none() {
            let nominal = 1.0 / r.arc_fit_ratio;
            assert!(
                (a_over_c - nominal).abs() < 0.02,
                "{}: uncapped, arm A / arm C must equal 1/ratio = {:.3}, measured {:.3}",
                fx.label,
                nominal,
                a_over_c
            );
        }
        // The shipped feed is what the GUI writes.
        assert!(
            (r.final_feed - r.raised_feed).abs() < 0.5,
            "{}: suggested_operation feed {} must be the raised feed {}",
            fx.label,
            r.final_feed,
            r.raised_feed
        );
    }
}

// ── Sim-side arms ───────────────────────────────────────────────────────

/// One arm's post-simulation reading of the measured op.
#[derive(Debug)]
struct ArmRead {
    applied_feed: f64,
    /// Provenance of the machined stock this generation consumed.
    stamp: Option<StockSnapshotStamp>,
    verdict: String,
    /// Stage 1 — commanded advance per tooth (mm).
    commanded_fpt: Option<f64>,
    /// Stage 2 — the vendor band.
    band: Option<(Option<f64>, f64)>,
    /// Stage 3 — achieved / commanded feed, and whether it was measured.
    achieved_ratio: Option<f64>,
    predicted_feeds_present: bool,
    /// Stage 4 — the gate's own observation and its population.
    gate_observed: Option<f64>,
    gate_samples: usize,
    /// Post-sim modulation, when it ran.
    modulation: Option<(usize, usize, f64)>,
}

fn simulate(session: &mut ProjectSession, modulation: bool) {
    let cancel = AtomicBool::new(false);
    session
        .run_simulation(
            &SimulationOptions {
                resolution: SIM_CELL_MM,
                auto_resolution: false,
                metrics_enabled: true,
                adaptive_feed_modulation: modulation,
                ..SimulationOptions::default()
            },
            &cancel,
        )
        .expect("cascade simulation");
}

/// Run one arm: write `feed` onto the measured op, regenerate it against the
/// stock currently in `prior_stocks`, simulate, and read every stage.
fn run_arm(session: &mut ProjectSession, feed: f64, rpm: u32, modulation: bool) -> ArmRead {
    {
        let tc = session
            .toolpath_configs_mut()
            .get_mut(1)
            .expect("measured op present");
        tc.operation.set_feed_rate(feed);
        tc.operation.set_spindle_rpm(Some(rpm));
    }
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(1, &cancel)
        .expect("measured op generates");

    let stamp = session
        .get_result(1)
        .expect("measured op result")
        .stats
        .stock_snapshot;

    simulate(session, modulation);

    let tp_id = session.toolpath_configs()[1].id;
    let report = session.tool_load_report();
    let v = report
        .per_toolpath
        .iter()
        .find(|v| v.toolpath_id == tp_id)
        .expect("measured op has a load verdict");

    let fe = v.feed_explanation.as_deref();
    ArmRead {
        applied_feed: feed,
        stamp,
        verdict: format!("{:?}", v.chipload.state()),
        commanded_fpt: fe.map(|e| e.commanded.feed_per_tooth_mm),
        band: fe.map(|e| (e.band.min_mm_per_tooth, e.band.max_mm_per_tooth)),
        achieved_ratio: fe.and_then(|e| e.achieved_feed.median_ratio),
        predicted_feeds_present: fe.is_some_and(|e| e.achieved_feed.predicted_feeds_present),
        gate_observed: fe.map(|e| e.gate.value_mm),
        gate_samples: fe.map_or(0, |e| e.gate.sample_count),
        modulation: v
            .modulation_summary
            .as_ref()
            .map(|m| (m.moves_touched, m.moves_total, m.median_feed_delta_pct)),
    }
}

fn print_arm(tag: &str, a: &ArmRead) {
    println!(
        "  {tag:<22} feed {:>8.1}  verdict {:<10} cmd_fpt {:>9}  achieved× {:>8}  \
         gate {:>9} (n={})",
        a.applied_feed,
        a.verdict,
        a.commanded_fpt
            .map_or("—".to_owned(), |v| format!("{v:.5}")),
        a.achieved_ratio
            .map_or("—".to_owned(), |v| format!("{v:.4}")),
        a.gate_observed
            .map_or("—".to_owned(), |v| format!("{v:.5}")),
        a.gate_samples,
    );
    println!(
        "  {:<22} band {:?}  predicted_feeds {}  modulation {:?}  stamp {:?}",
        "",
        a.band,
        a.predicted_feeds_present,
        a.modulation,
        a.stamp.map(|s| (s.cell_size_mm(), s.digest)),
    );
}

/// **Step 1–2: Suggest → generate → simulate, three arms, same stock.**
///
/// The arms differ in commanded feed only. The stamp assertion is what makes
/// that claim checkable rather than assumed.
#[test]
fn arc_fit_arms_gate_observation() {
    println!("\n=== A-5 step 1/2 — simulation-backed arms (cell {SIM_CELL_MM} mm) ===");

    for (fx_index, fx) in fixtures().into_iter().enumerate() {
        let mut session = build_session(&fx);
        let r = suggest_read(&session, &fx);

        // Seed the cascade: generate the rough, then simulate. The rest op
        // REFUSES to generate before a snapshot exists (it will not fall
        // back to fresh stock), so the simulation has to come first.
        let cancel = AtomicBool::new(false);
        session
            .generate_toolpath(0, &cancel)
            .expect("rough generates");
        simulate(&mut session, false);

        println!("\n{} [{}]", fx.label, fx.family);

        // Every arm runs at the RPM Suggest resolved, so the A/B/C delta is
        // attributable to the commanded feed alone.
        let arm_a = run_arm(&mut session, r.raised_feed, r.rpm, false);
        let arm_b = run_arm(&mut session, r.requested_feed, r.rpm, false);
        let arm_c = run_arm(&mut session, r.arm_c_feed(), r.rpm, false);
        let arm_a_mod = run_arm(&mut session, r.raised_feed, r.rpm, true);
        let arm_b_mod = run_arm(&mut session, r.requested_feed, r.rpm, true);

        print_arm("A shipped", &arm_a);
        print_arm("B retire", &arm_b);
        print_arm("C re-key 1.0", &arm_c);
        print_arm("A + modulation", &arm_a_mod);
        print_arm("B + modulation", &arm_b_mod);

        // ── S-4 handoff: the arms must have consumed the same stock ────
        for (tag, arm) in [
            ("B", &arm_b),
            ("C", &arm_c),
            ("A+mod", &arm_a_mod),
            ("B+mod", &arm_b_mod),
        ] {
            assert_eq!(
                arm.stamp, arm_a.stamp,
                "{}: arm {tag} consumed a different machined-stock snapshot than arm A — \
                 the comparison is not fair (§6.1 rule 6). A={:?} {tag}={:?}",
                fx.label, arm_a.stamp, arm.stamp
            );
        }
        let stamp_a = arm_a.stamp.unwrap_or_else(|| {
            panic!(
                "{}: the cascade must give the measured op a machined-stock snapshot, \
                 otherwise the equality above is vacuous",
                fx.label
            )
        });
        // The stamp must be reading THIS run's grid, not a constant. Equal
        // digests across fixtures are expected here — every fixture's op 0 is
        // the same rough on the same stock — so the cell is what proves the
        // channel is live per-run.
        assert!(
            (stamp_a.cell_size_mm() - SIM_CELL_MM).abs() < 1e-12,
            "{}: stamp cell {} must be the cell this harness simulated at ({SIM_CELL_MM})",
            fx.label,
            stamp_a.cell_size_mm()
        );

        // ── Non-vacuity: the gate must have had a population ───────────
        assert!(
            arm_a.gate_samples > 0,
            "{}: arm A's chipload gate ran on an EMPTY population — a verdict from \
             an empty gate is vacuous (X-VAC)",
            fx.label
        );

        // ── The pre-fix reproduction (§0 rule 1 — stays permanently) ───
        //
        // Suggest's OWN recommendation, applied unmodified and simulated,
        // fails the chipload gate Suggest claims to target. Arm B (the lift
        // removed) and arm C (the lift re-aimed at the gate's actual unit)
        // do not. This is the finding; it must fail loudly if it ever
        // silently stops being true.
        assert_eq!(
            arm_a.verdict, "Exceeds",
            "{}: arm A is the pre-fix reproduction — Suggest's shipped feed must \
             trip the chipload gate. If this changed, the disposition evidence \
             is stale and Checkpoint J must be re-run.",
            fx.label
        );
        assert_eq!(
            arm_c.verdict, "Within",
            "{}: arm C aims the same solve at the unit the gate reports and must \
             land inside the band",
            fx.label
        );

        // ── Modulation erases the lift entirely ────────────────────────
        //
        // A and B are commanded 2.4×–5.7× apart, yet under adaptive feed
        // modulation both converge on the SAME gate observation. The lift's
        // only effect in a modulated workflow is how far the modulator has
        // to travel to undo it.
        let (Some(obs_a), Some(obs_b)) = (arm_a_mod.gate_observed, arm_b_mod.gate_observed) else {
            panic!(
                "{}: both modulated arms must produce a gate observation",
                fx.label
            )
        };
        assert!(
            (obs_a - obs_b).abs() <= 1e-6 * obs_a.abs().max(1.0),
            "{}: modulated arms A and B must converge on one operating point, \
             got {obs_a} vs {obs_b}",
            fx.label
        );

        // ── Instrument responsiveness, once ────────────────────────────
        //
        // Every stamp above compared EQUAL. A channel that can only ever
        // return equal has proven nothing, so on the first fixture only,
        // move the snapshot and require the stamp to notice (S-4 arm B2's
        // result, re-checked here because this file leans on it).
        if fx_index == 0 {
            let cancel = AtomicBool::new(false);
            session
                .run_simulation(
                    &SimulationOptions {
                        resolution: SIM_CELL_MM * 2.0,
                        auto_resolution: false,
                        metrics_enabled: false,
                        ..SimulationOptions::default()
                    },
                    &cancel,
                )
                .expect("re-simulate at a different cell");
            session
                .generate_toolpath(1, &cancel)
                .expect("regenerate against the moved snapshot");
            let moved = session
                .get_result(1)
                .expect("measured op result")
                .stats
                .stock_snapshot
                .expect("still a machined-stock generation");
            println!(
                "  {:<22} stamp {:?} (was {:?})",
                "stamp responsiveness",
                (moved.cell_size_mm(), moved.digest),
                (stamp_a.cell_size_mm(), stamp_a.digest),
            );
            assert_ne!(
                moved, stamp_a,
                "{}: re-simulating at a different cell must move the stamp — \
                 otherwise the equality assertions above are vacuous",
                fx.label
            );
        }
    }
}
