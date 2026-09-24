//! F-036b — Adaptive feed modulation pipeline: flag plumbing +
//! simulator integration.
//!
//! F-036 landed the modulation algorithm (`crates/rs_cam_core/src/
//! feed_modulation.rs`). F-036a added the regression-net for per-move
//! F-word emission. F-036b plumbs the algorithm into the production
//! sim pipeline:
//!
//!  1. `SimulationOptions::adaptive_feed_modulation: bool` (default
//!     `false`) gates the post-sim modulation pass.
//!  2. After the simulator produces its sample stream,
//!     `ProjectSession::apply_adaptive_feed_modulation` aggregates
//!     per-(toolpath_id, move_index) `PerMoveEngagement` from the
//!     trace, looks up the vendor LUT chipload band, and calls
//!     `adaptive_feed_modulate` on a clone of each toolpath. When the
//!     modulator changes any feed, the swapped `Arc<AnnotatedToolpath>`
//!     in `session.results` carries the modulated IR into G-code
//!     emission.
//!  3. The emitter from F-036a already handles per-move F-word
//!     variation correctly (Statement::Linear { feed } per move,
//!     modal F-elision collapses constant runs).
//!
//! These tests pin the seven acceptance bars from the finding file:
//!
//!  - `flag_off_emits_identical_gcode_to_pre_f036` — load-bearing
//!    regression check. AS001 pocket with flag OFF must emit
//!    byte-identical G-code to the same session without modulation.
//!  - `modulated_path_has_per_segment_feed_variation` — flag ON,
//!    distinct per-move feeds appear in `Toolpath::moves[i]`.
//!  - `modulation_caps_cutting_chipload_at_the_point` — AS001's row is
//!    a printed point (A2); the modulator caps each cutting move at it
//!    with no floor.
//!  - `modulated_cycle_time_lower_than_unmodulated` — F-034 cycle-time
//!    integrator reports a shorter (or equal) cycle for the modulated
//!    path vs the commanded path.
//!  - `modulated_path_never_emits_below_min_chipload` — defensive
//!    floor; rubbing protection.
//!  - `modulated_path_preserves_f024_axial_engagement_invariant` —
//!    bridge to F-024.
//!  - `modulated_path_preserves_zero_rapid_collision_invariant` —
//!    bridge to F-017's rapid-collision floor.
//!
//! See `planning/acceptance_loop/findings/F-036b-feed-modulation-flag-plumbing.md`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use rs_cam_core::ids::ToolpathId;
use std::f64::consts::TAU;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::{PocketConfig, PocketPattern};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P2;
use rs_cam_core::machine::kinematics::MachineKinematics;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    Command, LoadedModel, ProjectSession, ProjectSessionBuilder, SetMachineArgs, SimulationOptions,
    ToolpathConfig,
};
use rs_cam_core::stock::simulation_cut::CutKinematics;
use rs_cam_core::trace::debug_trace::ToolpathDebugOptions;

// ----- AS001 pocket fixture (matches F-024 / F-035) -----------------

fn make_endmill_6mm() -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    tool.shank_diameter = 6.35;
    tool.shank_length = 20.0;
    tool.stickout = 45.0;
    tool.flute_count = 2;
    tool.name = "End Mill 6mm (F-036b test)".to_owned();
    tool
}

fn rounded_rect_with_island() -> Polygon2 {
    let exterior = vec![
        P2::new(5.0, 5.0),
        P2::new(75.0, 5.0),
        P2::new(75.0, 55.0),
        P2::new(5.0, 55.0),
    ];
    let mut hole = Vec::with_capacity(64);
    let cx = 40.0;
    let cy = 30.0;
    let r = 10.0;
    let n = 64;
    for i in 0..n {
        let t = (i as f64) * TAU / (n as f64);
        hole.push(P2::new(cx + r * (-t).cos(), cy + r * (-t).sin()));
    }
    Polygon2::with_holes(exterior, vec![hole])
}

/// Build the same AS001 pocket session F-024/F-035 use. Attaches
/// kinematics when `attach_kinematics` is true so the F-036b post-pass
/// fires; otherwise modulation is a no-op even with the flag on.
fn build_as001_pocket_session(attach_kinematics: bool) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new();
    let stock = StockConfig {
        x: 100.0,
        y: 100.0,
        z: 12.0,
        origin_x: -10.0,
        origin_y: -10.0,
        origin_z: -12.0,
        auto_from_model: false,
        material: Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        ..StockConfig::default()
    };
    builder = builder.stock(stock);

    let tool_idx = builder.add_tool(make_endmill_6mm());
    let tool_id = builder.tools()[tool_idx].id.0;

    let polygon = rounded_rect_with_island();
    let model = LoadedModel {
        id: 0,
        name: "as001_pocket".to_owned(),
        mesh: None,
        polygons: Some(Arc::new(vec![polygon])),
        drill_targets: std::sync::Arc::new(Vec::new()),
        layers: std::sync::Arc::new(Vec::new()),
        path: PathBuf::from("synthetic://as001_pocket.svg"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    };
    let model_id = builder.add_model(model);

    let pocket = PocketConfig {
        stepover: 2.0,
        depth: 6.0,
        depth_per_pass: 2.0,
        feed_rate: 770.0,
        plunge_rate: 385.0,
        climb: true,
        pattern: PocketPattern::Contour,
        angle: 0.0,
        finishing_passes: 0,
        spindle_rpm: Some(18_000),
    };
    let tc = ToolpathConfig {
        id: ToolpathId(0),
        name: "Pocket".to_owned(),
        enabled: true,
        operation: OperationConfig::Pocket(pocket),
        dressups: DressupConfig::for_op(OperationType::Pocket),
        heights: HeightsConfig::default(),
        tool_id,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: StockSource::default(),
        coolant: CoolantMode::Off,
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
        planner_origin: None,
    };
    let _ = builder.add_toolpath(0, tc).expect("add pocket toolpath");
    let mut session = builder.build();

    if attach_kinematics {
        let mut machine = session.machine().clone();
        machine.kinematics = Some(MachineKinematics::shapeoko_xxl_stock());
        let _ = session
            .apply(Command::SetMachine(SetMachineArgs {
                machine: Box::new(machine),
            }))
            .expect("the machine row refuses nothing");
    }
    session
}

fn opts(adaptive_feed_modulation: bool) -> SimulationOptions {
    SimulationOptions {
        resolution: 1.0,
        skip_ids: Vec::new(),
        metrics_enabled: true,
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation,
        modulation_strategy:
            rs_cam_core::dressup::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_feed_scale: 1.0,
    }
}

fn run_session(attach_kinematics: bool, modulate: bool) -> ProjectSession {
    let mut session = build_as001_pocket_session(attach_kinematics);
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generate pocket toolpath");
    session
        .run_simulation(&opts(modulate), &cancel)
        .expect("simulation completes");
    session
}

// (toolpath-IR test helper was dropped; the F-036b acceptance tests
// inspect emitted G-code via `export_session_gcode` below — the public
// surface that GUI / MCP exits through — which is the right level to
// pin per-move feed variation against.)

/// Read the toolpath IR out of the session after sim. Uses the
/// internal `tool_load_report` accessor's data path: the session keeps
/// `Arc<AnnotatedToolpath>` per toolpath in `results`. We export
/// G-code via the public emitter and inspect the resulting F-words.
fn export_session_gcode(session: &ProjectSession) -> String {
    // `export_gcode_checked` runs through `project_load_report` + the
    // gate enforcement. For the byte-identical test we want only the
    // emitted text — pass the most permissive policy so the gate
    // doesn't refuse a hardwood-pocket export.
    let policy = rs_cam_core::gcode::ToolLoadExportPolicy {
        accept_unmodeled: true,
        accept_exceeded: true,
    };
    rs_cam_core::gcode::export_gcode_checked(
        session,
        session
            .simulation_result()
            .and_then(|s| s.cut_trace.as_deref()),
        policy,
    )
    .expect("export_gcode_checked succeeds on AS001 pocket")
}

/// Count standalone F-words emitted in `gcode`, returning the list of
/// feed values seen on cutting lines (excludes comments).
fn collect_f_words(gcode: &str) -> Vec<f64> {
    let mut feeds = Vec::new();
    for line in gcode.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('(') || trimmed.starts_with(';') {
            continue;
        }
        for token in line.split_whitespace() {
            if let Some(rest) = token.strip_prefix('F')
                && let Ok(n) = rest.parse::<f64>()
            {
                feeds.push(n);
            }
        }
    }
    feeds
}

/// The emitted F words that belong to the CUTTING population, with the
/// histograms that explain the split.
struct CuttingFeedWords {
    /// Every F word the program carries, `rendered feed -> count`.
    all: std::collections::BTreeMap<String, usize>,
    /// The F words a cutting move's feed produces.
    kept: Vec<f64>,
    /// The same population as `kept`, rendered the way `all` is rendered
    /// so one failure line does not carry two spellings of one feed.
    kept_histogram: std::collections::BTreeMap<String, usize>,
    /// Feed words that a cutting move and a non-cutting move both carry.
    /// The filter keeps such a word, so it records the overlap.
    shared: Vec<String>,
}

impl CuttingFeedWords {
    /// The median of the cutting population.
    fn median(&self) -> f64 {
        let mut kept = self.kept.clone();
        assert!(
            !kept.is_empty(),
            "F-036b AB3: no cutting F word in the emitted program. A median over an \
             empty population proves nothing."
        );
        kept.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        kept[kept.len() / 2]
    }

    /// One line for a failure message: the two histograms and the overlap.
    fn report(&self) -> String {
        format!(
            "all F words n={} {:?}; cutting F words n={} {:?}; feeds both populations \
             carry {:?}",
            self.all.values().sum::<usize>(),
            self.all,
            self.kept.len(),
            self.kept_histogram,
            self.shared
        )
    }
}

/// Split the emitted F words of `session` into the cutting population and
/// the rest.
///
/// **Why the filter exists (RT-5, 2026-09-17).** AB3 claims a CUTTING
/// chipload. [`collect_f_words`] takes every F word in the program, the
/// entry plunge and the entry ramp feeds included, so a median over it
/// measures a different population. `fd135a04` (G-RAMPCONTAIN) folded the
/// ramp along the following cut and multiplied the entry words; the median
/// then crossed out of the cutting moves and the arm went red on an
/// instrument defect, not on a modulation regression. The diagnosis is on
/// record in `planning/ui_fix_2026-09-09/reports/J2.md` §4c — the structure
/// purge deleted that path, so read it with
/// `git show ea4d5bfb^:planning/ui_fix_2026-09-09/reports/J2.md` — and this
/// is the remedy §4c prescribes. The band, the fixture and the three
/// assertions are unchanged.
///
/// **The classifier is the gate's own partition, read on the IR.** The gate
/// drops a sample whose span ancestry is an entry or a transit span
/// (`rs_cam_core::tool_load::locality::is_steady_state_for_gate`, the
/// canonical gate-side predicate). The toolpath IR carries the same fact per
/// move as [`MoveIntent`], so `ClearingCut | FinishingCut` is the cutting
/// population and every other intent is an entry, a link or a lead. AB5
/// (`modulated_path_never_emits_below_min_chipload`) partitions the same way.
/// A feed threshold would silently drop a cutting move the modulator
/// legitimately LOWERED, so the filter reads the intent, never a feed value.
///
/// **The export stays the measured artefact.** The function renders each
/// cutting move's feed the way the post renders an F word — the emitter
/// clamps to `max_feed`, then rounds to `decimals.feed` — and keeps the F
/// words that match. Residual, measured and reported, not hidden: a folded
/// ramp move can carry a cutting feed, and such an F word stays in the
/// population. `shared` names every feed both populations carry.
fn cutting_feed_words(session: &ProjectSession) -> CuttingFeedWords {
    use rs_cam_core::toolpath::MoveIntent;
    use std::collections::{BTreeMap, BTreeSet};

    let post = session.post_config().format.definition();
    let dp = post.decimals.feed;
    let max_feed = post.limits.max_feed.map(|f| f.get());
    // Render a feed the way the emitter writes it: clamp first
    // (`gcode/emitter.rs` `clamp_feed`), then round to the post's feed
    // decimals. Re-rendering an already emitted word is the identity.
    let word = |feed: f64| -> String {
        let clamped = max_feed.map_or(feed, |max| feed.min(max));
        format!("{clamped:.dp$}")
    };

    let result = session
        .get_result(0)
        .expect("session result for toolpath 0");
    let mut cutting: BTreeSet<String> = BTreeSet::new();
    let mut linking: BTreeSet<String> = BTreeSet::new();
    for mv in &result.annotated().toolpath.moves {
        let Some(feed) = mv.move_type.feed_rate() else {
            continue;
        };
        if matches!(
            mv.intent,
            MoveIntent::ClearingCut | MoveIntent::FinishingCut
        ) {
            cutting.insert(word(feed));
        } else {
            linking.insert(word(feed));
        }
    }

    let feeds = collect_f_words(&export_session_gcode(session));
    assert!(!feeds.is_empty(), "expected F-words in emitted G-code");
    let mut all: BTreeMap<String, usize> = BTreeMap::new();
    let mut kept = Vec::new();
    let mut kept_histogram: BTreeMap<String, usize> = BTreeMap::new();
    let mut unattributed: BTreeMap<String, usize> = BTreeMap::new();
    for feed in &feeds {
        let key = word(*feed);
        *all.entry(key.clone()).or_default() += 1;
        if cutting.contains(&key) {
            kept.push(*feed);
            *kept_histogram.entry(key).or_default() += 1;
        } else if !linking.contains(&key) {
            *unattributed.entry(key).or_default() += 1;
        }
    }
    // Instrument integrity: every F word must come from a move the IR
    // carries. An unattributed word means the emitter wrote a feed the
    // split cannot place, and the median then has no defined population.
    assert!(
        unattributed.is_empty(),
        "F-036b AB3: the F words {unattributed:?} match no move in the toolpath IR. \
         The cutting / non-cutting split cannot attribute them."
    );

    let shared = cutting.intersection(&linking).cloned().collect();
    CuttingFeedWords {
        all,
        kept,
        kept_histogram,
        shared,
    }
}

// ============== AB1: load-bearing flag-OFF byte-identical ===========

/// AB1 (load-bearing).
///
/// With `adaptive_feed_modulation = false`, the G-code emitted from
/// AS001 must be byte-identical to a control run that never touched
/// modulation. This pins the flag-OFF branch — the regression-net rule
/// requires the smoke baseline at
/// `planning/toolpath_acceptance/baselines/2026-05-26.csv` to continue
/// passing byte-identical, and that baseline runs with the flag OFF.
///
/// The control here is the same session with `kinematics = None`. The
/// flag is OFF on every path below, and `modulate_simulation_trace`
/// returns before it calls `apply_adaptive_feed_modulation`, so no path
/// modulates. Note what the kinematics block does and does not gate
/// inside `apply_adaptive_feed_modulation`. The modulation of feeds has
/// no kinematics guard: it reads `effective_kinematics()`, so a machine
/// with no kinematics block still modulates when the flag is ON. Since
/// N7 (2026-09-11) the pass's cycle-time re-integration is guarded on
/// `self.machine.kinematics.is_some()`, so that machine's published
/// runtimes stay byte-identical on both sides of the flag
/// (`tests/retime_respects_no_kinematics_n7.rs`).
#[test]
fn flag_off_emits_identical_gcode_to_pre_f036() {
    // Path A: kinematics attached + flag OFF.
    let session_a = run_session(true, false);
    let gcode_a = export_session_gcode(&session_a);

    // Path B: no kinematics. The flag is OFF, so the post-pass never
    // runs. The flag is the gate here, not the kinematics block.
    let session_b = run_session(false, false);
    let gcode_b = export_session_gcode(&session_b);

    // Path C: belt-and-suspenders — kinematics attached but no
    // modulation flag. Should match A.
    let session_c = run_session(true, false);
    let gcode_c = export_session_gcode(&session_c);

    assert_eq!(
        gcode_a, gcode_b,
        "F-036b AB1: flag-OFF emission must be byte-identical between \
         kinematics=Some+flag-OFF and kinematics=None"
    );
    assert_eq!(
        gcode_a, gcode_c,
        "F-036b AB1: flag-OFF emission must be deterministic across runs"
    );
}

// ============== AB2: flag-ON produces per-segment feed variation ====

/// AB2.
///
/// With the flag ON + kinematics attached, the modulator should
/// rewrite per-move feeds so the emitted G-code carries more than one
/// distinct F-word. Pre-F-036b the AS001 pocket emits a single F770
/// (`feed_rate: 770.0` in `PocketConfig`) for the whole cutting run;
/// post-F-036b the modulator should land each move at a chipload-
/// targeted feed that depends on the move's radial WOC.
#[test]
fn modulated_path_has_per_segment_feed_variation() {
    let session_off = run_session(true, false);
    let gcode_off = export_session_gcode(&session_off);
    let feeds_off = collect_f_words(&gcode_off);
    let distinct_off: std::collections::BTreeSet<u64> = feeds_off
        .iter()
        .map(|f| (*f * 100.0).round() as u64)
        .collect();

    let session_on = run_session(true, true);
    let gcode_on = export_session_gcode(&session_on);
    let feeds_on = collect_f_words(&gcode_on);
    let distinct_on: std::collections::BTreeSet<u64> = feeds_on
        .iter()
        .map(|f| (*f * 100.0).round() as u64)
        .collect();

    assert!(
        distinct_on.len() > distinct_off.len(),
        "F-036b AB2: flag-ON should produce MORE distinct F-words than flag-OFF. \
         Got flag-OFF distinct = {} ({:?}), flag-ON distinct = {} ({:?}). \
         If equal, the modulator either didn't fire (LUT band missing? kinematics off?) \
         or rewrote every move to the same value (band collapsed).",
        distinct_off.len(),
        distinct_off,
        distinct_on.len(),
        distinct_on,
    );
}

// ===== AB2b: modulation runs + stays fresh WITHOUT explicit kinematics =====

/// AB2b (unified load model, step 6 / option-b).
///
/// The modulator no longer requires an explicit machine `kinematics` block:
/// it falls back to `effective_kinematics` (the generic wood-router profile),
/// matching the strategy advisor. This is what lets the GUI/MCP sim path —
/// which applies modulation on machines that carry no kinematics block — show
/// the per-path operating point.
///
/// It also pins the freshness fix: the post-pass rewrites per-move feeds, so
/// each modulated toolpath hashes differently than the pre-modulation value
/// captured in the trace provenance. The pass refreshes those hashes against
/// the modulated IR; without that, `sim_trace_is_fresh` reads `false` and the
/// load report degrades every gate to `StaleSimulation`, dropping the
/// `modulation_summary` the card/report rely on.
#[test]
fn modulation_runs_and_stays_fresh_without_kinematics() {
    // No kinematics block, modulation flag ON.
    let session = run_session(false, true);
    let trace = session
        .simulation_result()
        .and_then(|s| s.cut_trace.as_ref())
        .expect("simulation produced a cut trace");

    assert!(
        !trace.modulation_summaries.is_empty(),
        "modulation must run without an explicit kinematics block \
         (effective_kinematics fallback) — got no per-toolpath summaries"
    );
    assert!(
        rs_cam_core::gcode::sim_trace_is_fresh(&session, trace),
        "the post-modulation trace must stay FRESH: the provenance toolpath \
         hashes are refreshed to the modulated IR so the load report keeps \
         its gates (and the modulation summary) instead of reading stale"
    );
}

// ===== AB3: modulation holds the cutting chipload at the printed point =====

/// AB3 (rescoped 2026-06-20; re-premised 2026-09-24 for A2, point mode).
///
/// **The fixture's row is a point.** Since R5 (2026-09-23) AS001's Ø6 2F
/// flat end mill in hardwood, pocket, roughing, resolves
/// `amana-flat-hardwood-pocket-6000-2f-spektra`. That row prints one value
/// (max 0.127 mm/tooth, no minimum). The two-limit row for the same cell,
/// `amana-flat-hardwood-pocket-6000-2f` (0.032–0.055, grade c, repo
/// authored), is still in the LUT but loses to the grade-b print. No
/// two-limit row wins for this tool, material and operation, and a move to
/// aluminium or another tool would change the AS001 identity that AB1, AB4,
/// AB6 and AB7 pin. So the fixture stays, and this arm pins the point
/// behaviour.
///
/// **What changed.** Before A2 the modulator floored every move it wrote at
/// `band.min × rpm × flutes` (1152 mm/min, the old median). A point has no
/// minimum, so the modulator applies no floor. It caps each move at
/// `v' × rpm × flutes` (v' is the DOC-derated point) and the other caps
/// (deflection, power, machine, reach, plunge) can take a move below the
/// commanded feed. The old claim "the flag-ON median lands inside the band"
/// has no anchor now, and "the median rises" is not safe: the moves that the
/// old floor lifted fall to their real caps.
///
/// The arm pins the honest point contract:
///   1. flag OFF, the gate reads `Within` with point bounds (no minimum,
///      source `VendorLutPointPreset`, max v'). The under-fed default is a
///      burn ADVISORY (decision Q1 (a)), never `Exceeds(Low)`;
///   2. the flag-OFF median cutting chipload is below v';
///   3. every cutting move the modulator wrote is at or below the point cap
///      `v' × rpm × flutes`; and
///   4. at least one cutting move the modulator wrote is above the
///      commanded 770 mm/min (the cap sits above the default, so the
///      modulator raised some feed toward the point).
///
/// **RT-5 (2026-09-17) — the population.** The medians read the CUTTING F
/// words only, through [`cutting_feed_words`]. Items 3 and 4 read the
/// modulated IR and diff it against the pre-modulation IR by move index, as
/// AB5 does, so a move the feed-optimisation dressup wrote does not count.
/// The test prints both medians and the flag-ON histogram; no median value
/// is pinned.
#[test]
fn modulation_caps_cutting_chipload_at_the_point() {
    use rs_cam_core::tool_load::ChiploadVerdict;
    use rs_cam_core::tool_load::verdict::ChipBoundsSource;
    use rs_cam_core::toolpath::MoveIntent;

    // RPM × flutes for AS001's 6 mm 2-flute end mill (`build_as001_pocket_session`).
    const RPM_X_FLUTES: f64 = 18_000.0 * 2.0;
    let chip = |feed: f64| feed / RPM_X_FLUTES;

    // 1. Flag OFF: the gate reads the point. Derive v' from the gate
    //    rather than hard-coding the LUT row.
    let session_off = run_session(true, false);
    let report_off = session_off.tool_load_report();
    let v_off = report_off
        .per_toolpath
        .iter()
        .find(|v| v.toolpath_id == ToolpathId(0))
        .expect("tool-load verdict for pocket toolpath");
    let ChiploadVerdict::Within {
        approach_to_min,
        approach_to_max,
        burn_advisory,
        ..
    } = &v_off.chipload
    else {
        panic!(
            "A2: AS001's default-fed pocket resolves a point row, so the gate must read \
             Within with a burn advisory (never Exceeds(Low)); got {:?}",
            v_off.chipload
        );
    };
    assert!(
        approach_to_min.is_none(),
        "A2: a point has no minimum, so the gate carries no approach-to-min metric; \
         got {approach_to_min:?}"
    );
    let bounds = &approach_to_max.bounds;
    assert_eq!(
        bounds.source,
        ChipBoundsSource::VendorLutPointPreset,
        "A2: AS001's row prints one value, so the gate bounds are a point preset"
    );
    assert_eq!(
        bounds.min_mm_per_tooth, None,
        "A2: the point bounds carry no minimum"
    );
    let point = bounds.max_mm_per_tooth;
    assert!(
        burn_advisory.is_some(),
        "A2 Q1 (a): the under-fed default (770 mm/min = {:.4} mm/tooth, below the point \
         {point:.4}) must give a burn ADVISORY",
        chip(770.0)
    );

    // The modulator reads the same point (one resolver).
    let words_off = cutting_feed_words(&session_off);
    let median_off = words_off.median();

    let mut session_on = build_as001_pocket_session(true);
    let cancel = AtomicBool::new(false);
    session_on
        .generate_toolpath(0, &cancel)
        .expect("generate pocket toolpath");
    let pre_feeds: Vec<Option<f64>> = session_on
        .get_result(0)
        .expect("session result before the simulation")
        .annotated()
        .toolpath
        .moves
        .iter()
        .map(|mv| mv.move_type.feed_rate())
        .collect();
    session_on
        .run_simulation(&opts(true), &cancel)
        .expect("simulation completes");
    let trace_on = session_on
        .simulation_result()
        .and_then(|s| s.cut_trace.as_ref())
        .expect("cut trace flag-on");
    let bands_on =
        rs_cam_core::tool_load::modulation_bands_for_session(&session_on, Some(trace_on));
    let band_on = bands_on
        .get(&ToolpathId(0))
        .expect("A2: the modulator has a target for AS001's pocket");
    assert!(
        band_on.is_point(),
        "A2: the modulator reads AS001's row as a point; got {band_on:?}"
    );
    let cap_feed = band_on.max_mm_per_tooth * RPM_X_FLUTES;

    let words_on = cutting_feed_words(&session_on);
    let median_on = words_on.median();
    eprintln!(
        "F-036b AB3 (A2): point v' = {point:.5} mm/tooth (gate), {:.5} (modulator); \
         cap {cap_feed:.1} mm/min; median off {median_off:.1} ({:.5} mm/tooth), \
         median on {median_on:.1} ({:.5} mm/tooth). Flag-ON population: {}",
        band_on.max_mm_per_tooth,
        chip(median_off),
        chip(median_on),
        words_on.report()
    );

    // 2. flag-OFF median sits below the point (the under-fed default).
    assert!(
        chip(median_off) < point,
        "flag-OFF median chipload {:.4} should be below the point {point:.4} (under-fed \
         default). Flag-OFF population: {}",
        chip(median_off),
        words_off.report()
    );

    // 3 and 4: read the moves the modulator wrote.
    let toolpath = &session_on
        .get_result(0)
        .expect("session result for toolpath 0")
        .annotated()
        .toolpath;
    assert_eq!(
        pre_feeds.len(),
        toolpath.moves.len(),
        "F-036b AB3: the per-index diff reads one move list, not two"
    );
    let commanded = 770.0;
    let mut written = 0usize;
    let mut above_cap: Vec<(usize, f64)> = Vec::new();
    let mut raised = 0usize;
    for (i, mv) in toolpath.moves.iter().enumerate() {
        if !matches!(
            mv.intent,
            MoveIntent::ClearingCut | MoveIntent::FinishingCut
        ) {
            continue;
        }
        let (Some(feed), Some(pre_feed)) = (mv.move_type.feed_rate(), pre_feeds[i]) else {
            continue;
        };
        if (feed - pre_feed).abs() == 0.0 {
            continue;
        }
        written += 1;
        // 0.5 mm/min absorbs the f64 product; the cap is exact otherwise.
        if feed > cap_feed + 0.5 {
            above_cap.push((i, feed));
        }
        if feed > commanded + 0.5 {
            raised += 1;
        }
    }
    assert!(
        above_cap.is_empty(),
        "A2: the modulator must cap every cutting move at the point \
         ({cap_feed:.1} mm/min); {} of {written} written moves are above it: {:?}",
        above_cap.len(),
        above_cap
    );
    assert!(
        raised > 0,
        "A2: the modulator wrote {written} cutting moves and raised none above the \
         commanded {commanded} mm/min. The point cap ({cap_feed:.1} mm/min) sits above \
         the default, so modulation must raise some feed toward it."
    );
}

// ============== AB4: modulated cycle time <= unmodulated ===========

/// AB4.
///
/// The modulator targets the geometric midpoint of the LUT chipload
/// band — for AS001's 6 mm flat in hardwood-pocket-roughing that
/// midpoint is HIGHER than the commanded 770 mm/min default. Net
/// effect: most cutting moves get a higher feed and cycle time falls
/// (or stays equal when capped by the machine's accel-limited
/// achievable feed).
///
/// Reads F-034's kinematics integrator via `apply_kinematics_cycle_time`'s
/// public surface on the session: with modulation, the per-toolpath
/// cycle time from the simulator's metric summary should be <=
/// the unmodulated cycle time, within a 1 % tolerance for integration
/// noise.
#[test]
fn modulated_cycle_time_lower_than_unmodulated() {
    let session_off = run_session(true, false);
    let trace_off = session_off
        .simulation_result()
        .and_then(|s| s.cut_trace.as_ref())
        .expect("cut trace flag-off");
    let cycle_off = trace_off.summary.total_runtime_s;

    let session_on = run_session(true, true);
    let trace_on = session_on
        .simulation_result()
        .and_then(|s| s.cut_trace.as_ref())
        .expect("cut trace flag-on");
    let cycle_on = trace_on.summary.total_runtime_s;

    // Allow a ~1 % slack for sim-integration noise.  The strong claim
    // is "modulation should not REGRESS cycle time"; depending on the
    // LUT band geometry it may also accelerate.
    assert!(
        cycle_on <= cycle_off * 1.01,
        "F-036b AB4: modulated cycle time {cycle_on:.3}s exceeds unmodulated \
         {cycle_off:.3}s by more than 1%. Modulation should not regress cycle time \
         (the algorithm targets the LUT band midpoint which is typically >= commanded feed)."
    );
}

// ============== AB5: modulated chipload >= band floor ==============

/// AB5.
///
/// The modulator must never emit below the LUT band's `min` — that's
/// the rubbing / burn protection.
///
/// **F-036b1 implementation**: walk the modulated `Toolpath` IR
/// directly rather than reading post-sim sample chipload. The
/// simulator doesn't re-run after modulation, so
/// `sample.chipload_mm_per_tooth` still reflects the pre-modulation
/// commanded feed — that's the wrong field to read. The IR's
/// `move_type.feed_rate()` carries the post-modulation feed; chipload
/// at the move is then `feed / (rpm * flute_count)`.
///
/// Filter scope: only moves the modulator **actually touched**. The arm
/// reads that from EVIDENCE, not from a proxy. It keeps the
/// pre-modulation per-move feeds after `generate_toolpath`, runs the
/// simulation with the flag ON, and diffs the two move lists by index. A
/// move whose feed moved is a move the modulator wrote. Moves the
/// algorithm skipped (zero aggregated engagement, non-clearing /
/// finishing intent, plunge / retract / drilling) keep their
/// pre-modulation feed by design; if that feed is below the LUT band,
/// that is a generator or user parameter choice the modulator does not
/// override. The algorithm-layer floor-clamp on modulated moves is what
/// AB5 pins.
///
/// **WP26 (2026-09-13) — why the old proxy broke.** The arm used to keep
/// a move whose feed differs from the operation's own `feed_rate()` by
/// 0.5 mm/min or more. That proxy holds only while the modulator is the
/// one pass that writes a per-move feed. Since WP11b (`80a9cf4d`,
/// 2026-09-11) the feed-optimisation dressup writes per-move feeds on
/// the same generation door, so the proxy counted 21 of 126 moves the
/// modulator never touched and read a feed-optimisation value as a
/// modulator value. The solver's own floor
/// (`feed_modulation.rs:518-529`) puts every move it writes at
/// `band.min × rpm × flutes`, or at the machine cutting-feed ceiling
/// when that ceiling is lower. A below-floor move is therefore always a
/// pass-through at its generated feed: the zero-engagement arm returns
/// the MOVE's own feed (`:663-670`), and the writer rewrites a move only
/// when the new feed differs from it (`:706-710`). The diff cannot make
/// the proxy's mistake, and it also keeps a move the modulator LOWERED,
/// which any feed-value threshold drops.
///
/// The arm skips a move whose stamped `BindingConstraint` is
/// `PlungeRate`. The Phase 3 geometric plunge guard caps a
/// vertical-dominant move at the operation's own `plunge_rate`, which
/// sits below the band floor by construction, so AB5 does not pin it.
#[test]
fn modulated_path_never_emits_below_min_chipload() {
    use rs_cam_core::tool_load::BindingConstraint;
    use rs_cam_core::toolpath::MoveIntent;

    // One session. `generate_toolpath` adopts the pre-modulation IR into
    // `session.results`, and `run_simulation` reads that result without
    // regenerating it, so these feeds are the feeds the modulator was
    // handed. The post-pass swaps a NEW `Arc` into the result slot and
    // never mutates through the old one
    // (`session/compute.rs:4253-4258`, `:4382-4405`).
    let mut session = build_as001_pocket_session(true);
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generate pocket toolpath");
    let pre_feeds: Vec<Option<f64>> = session
        .get_result(0)
        .expect("session result before the simulation")
        .annotated()
        .toolpath
        .moves
        .iter()
        .map(|mv| mv.move_type.feed_rate())
        .collect();
    session
        .run_simulation(&opts(true), &cancel)
        .expect("simulation completes");

    let trace = session
        .simulation_result()
        .and_then(|s| s.cut_trace.as_ref())
        .expect("cut trace flag-on");

    let envelopes = rs_cam_core::tool_load::chipload_envelopes_for_session(&session, Some(trace));
    let Some(band) = envelopes.get(&ToolpathId(0)) else {
        // No two-limit band → no floor to check. Vacuously satisfied.
        // Since R5 AS001 resolves a printed point (A2): the modulator
        // runs, capped at the point with no floor, and AB3 pins that cap.
        eprintln!(
            "F-036b AB5: no two-limit LUT chipload band for AS001 pocket (a printed point \
             has no floor, A2), so this arm checks nothing. Recalibrate the test fixture \
             if this becomes unintentional."
        );
        return;
    };

    let result = session
        .get_result(0)
        .expect("session result for toolpath 0");
    let toolpath = &result.annotated().toolpath;
    let tc = session.get_toolpath_config(0).expect("toolpath config 0");
    let tool = session
        .get_tool(rs_cam_core::compute::tool_config::ToolId(tc.tool_id))
        .expect("tool referenced by toolpath");
    let rpm = tc
        .operation
        .spindle_rpm()
        .map(f64::from)
        .unwrap_or(18_000.0);
    let flutes = f64::from(tool.flute_count.max(1));
    let commanded_feed = tc.operation.feed_rate();

    let mut modulated_moves = 0usize;
    let mut below_floor = 0usize;
    let mut worst_below = f64::INFINITY;
    let floor = band.start * 0.95;

    // The diff needs one move list. A length change means the
    // simulation regenerated the toolpath, and a silent `.get(i)` miss
    // would then drop moves and make the arm vacuous.
    assert_eq!(
        pre_feeds.len(),
        toolpath.moves.len(),
        "F-036b AB5: the modulated IR carries {post} moves against {pre} before the \
         simulation. The per-index diff reads one move list, not two.",
        post = toolpath.moves.len(),
        pre = pre_feeds.len()
    );

    for (i, mv) in toolpath.moves.iter().enumerate() {
        if !matches!(
            mv.intent,
            MoveIntent::ClearingCut | MoveIntent::FinishingCut
        ) {
            continue;
        }
        let Some(feed) = mv.move_type.feed_rate() else {
            continue;
        };
        // Only check moves the modulator actually touched, and read that
        // off the pre-modulation IR. Skipped moves (zero engagement
        // aggregation) keep their generated feed by design — if that is
        // below band, it is a user or generator choice, not a modulator
        // bug. The comparison is exact: a tolerance is the old proxy
        // coming back.
        let Some(pre_feed) = pre_feeds.get(i).copied().flatten() else {
            continue;
        };
        let modulator_wrote_it = (feed - pre_feed).abs() > 0.0;
        if !modulator_wrote_it {
            continue;
        }
        // The Phase 3 geometric plunge guard writes the operation's own
        // plunge rate, which sits below the band floor by construction.
        let binding = trace
            .modulated_feeds
            .get(&(ToolpathId(0), i))
            .map(|(_, constraint)| *constraint);
        if matches!(binding, Some(BindingConstraint::PlungeRate)) {
            continue;
        }
        modulated_moves += 1;
        let denom = rpm * flutes;
        if denom <= 0.0 {
            continue;
        }
        let chipload = feed / denom;
        if chipload < floor {
            below_floor += 1;
            if chipload < worst_below {
                worst_below = chipload;
            }
            // Name every counted move, so a red reports which moves it
            // counted and which constraint bound each one.
            eprintln!(
                "F-036b AB5 below floor: move {i}, intent {intent:?}, pre-modulation feed \
                 {pre_feed:.1}, modulated feed {feed:.1}, chipload {chipload:.4} mm/tooth, \
                 binding {binding:?}",
                intent = mv.intent
            );
        }
    }

    assert_eq!(
        below_floor,
        0,
        "F-036b AB5: modulated IR has {below_floor} of {modulated_moves} modulated moves \
         below the LUT band floor (worst = {worst:.4} mm/tooth, band floor = \
         {floor:.4} mm/tooth, band.start = {start:.4}, commanded feed = {cmd:.0} mm/min). \
         The modulator's band-floor clamp must hold on every move it touches.",
        worst = if worst_below.is_finite() {
            worst_below
        } else {
            0.0
        },
        floor = floor,
        start = band.start,
        cmd = commanded_feed
    );
    assert!(
        modulated_moves > 0,
        "F-036b AB5: modulator made no changes on AS001 pocket. The test fixture must \
         exercise the band-floor clamp; recalibrate if the commanded feed is now inside \
         the LUT band (band.start = {:.4} mm/tooth, commanded chipload = {:.4} mm/tooth).",
        band.start,
        commanded_feed / (rpm * flutes)
    );
}

// ============== AB6: F-024 axial-engagement invariant preserved =====

/// AB6 (bridge to F-024).
///
/// Modulation rewrites per-move feed but never touches XYZ targets,
/// so the dexel grid + axial engagement readout must be unchanged.
/// First-pass axial engagement must remain `<= 3.0 mm` (the F-024
/// invariant — commanded 2.0 mm + grid discretisation margin).
#[test]
fn modulated_path_preserves_f024_axial_engagement_invariant() {
    let session = run_session(true, true);
    let trace = session
        .simulation_result()
        .and_then(|s| s.cut_trace.as_ref())
        .expect("cut trace flag-on");

    let mut first_pass_axials: Vec<f64> = trace
        .samples
        .iter()
        .filter(|s| s.is_cutting && s.cut_kinematics != CutKinematics::Plunge)
        .filter(|s| (s.position[2] - (-2.0)).abs() < 0.5)
        .map(|s| s.axial_engagement_mm)
        .collect();
    first_pass_axials.sort_by(|a, b| a.partial_cmp(b).unwrap());

    assert!(
        !first_pass_axials.is_empty(),
        "F-036b AB6: expected at least one cutting sample near Z=-2 with modulation on"
    );
    let peak = *first_pass_axials.last().unwrap();
    assert!(
        peak <= 3.0,
        "F-036b AB6 (bridge to F-024): modulated path's first-pass axial engagement \
         should remain <= 3.0 mm; got peak = {peak:.4} mm. Modulation must not contaminate \
         dexel-grid frame correctness — it only rewrites per-move feed."
    );
}

// ============== AB7: F-017 zero rapid-collision invariant ==========

/// AB7 (bridge to F-017).
///
/// AS001 is a known-clean toolpath: zero rapid-collisions baseline.
/// Modulation must not introduce new collisions (it touches only
/// feed values, never positions), so the post-sim rapid-collision
/// count must remain zero.
#[test]
fn modulated_path_preserves_zero_rapid_collision_invariant() {
    let session = run_session(true, true);
    let sim = session.simulation_result().expect("simulation result");
    let n = sim.rapid_collisions.len();
    assert_eq!(
        n, 0,
        "F-036b AB7 (bridge to F-017): modulation must not change the rapid-collision \
         count. AS001 baseline = 0; with modulation = {n}. Modulator must not touch XYZ \
         targets."
    );
}

// ============== Sanity: modulation actually fires ==================
//
// Defensive check that the test fixture's LUT path actually returns a
// chipload target for the modulator — if it doesn't, AB2/AB3/AB4 are
// vacuously passing.
//
// A2 (point mode, 2026-09-24): AS001's row prints one value
// (`amana-flat-hardwood-pocket-6000-2f-spektra`, max 0.127, no minimum), so
// the target is a point. The modulator reads it through
// `modulation_bands_for_session`. The band-only envelope map (the viewport
// colouring, decision Q3) does not carry it.
#[test]
fn fixture_sanity_lut_band_resolves_for_as001() {
    let session = run_session(true, false);
    let trace = session
        .simulation_result()
        .and_then(|s| s.cut_trace.as_ref())
        .expect("cut trace");
    let bands = rs_cam_core::tool_load::modulation_bands_for_session(&session, Some(trace));
    let band = bands.get(&ToolpathId(0)).unwrap_or_else(|| {
        panic!(
            "F-036b sanity: AS001 pocket must resolve a vendor LUT chipload target so the \
             flag-ON tests actually exercise modulation. Got modulation bands = {bands:?}"
        )
    });
    assert!(
        band.is_point(),
        "A2: AS001's row prints one value, so the modulator target is a point; got {band:?}"
    );

    let tc = session.get_toolpath_config(0).expect("toolpath config 0");
    let tool = session
        .get_tool(ToolId(tc.tool_id))
        .expect("tool referenced by toolpath");
    let target = rs_cam_core::tool_load::chip_target_for_toolpath(
        &session.stock_config().material,
        tool,
        &tc.operation,
        tc.id,
        Some(trace),
    );
    assert_eq!(
        target,
        Some(rs_cam_core::tool_load::ChipTarget::Point(
            band.max_mm_per_tooth
        )),
        "A2: chip_target_for_toolpath gives the point the modulator reads"
    );

    let envelopes = rs_cam_core::tool_load::chipload_envelopes_for_session(&session, Some(trace));
    assert!(
        !envelopes.contains_key(&ToolpathId(0)),
        "A2 Q3: the band-only envelope map must not carry a point; got {envelopes:?}"
    );
}

// ============== Defensive: feed values match emitter contract =======
//
// Per F-036a, the emitter collapses equal feeds into a single F-word
// (modal F-elision). Confirm that flag-ON for AS001 produces at least
// two distinct F-words in the emitted G-code — same surface AB2
// inspects but on the load-bearing F-036a emitter rather than via the
// distinct-set comparison.
#[test]
fn modulated_gcode_carries_at_least_two_distinct_f_words() {
    let session = run_session(true, true);
    let gcode = export_session_gcode(&session);
    let feeds = collect_f_words(&gcode);
    let distinct: std::collections::BTreeSet<u64> =
        feeds.iter().map(|f| (*f * 100.0).round() as u64).collect();
    assert!(
        distinct.len() >= 2,
        "F-036b: modulated AS001 G-code must emit >= 2 distinct F-words; got \
         {} distinct from {} F-words ({:?}). F-036a's modal F-elision shouldn't \
         collapse modulated feeds since their values differ by move.",
        distinct.len(),
        feeds.len(),
        distinct,
    );
}
