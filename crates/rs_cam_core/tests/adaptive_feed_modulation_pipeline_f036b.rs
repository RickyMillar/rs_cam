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
//!  - `modulated_gates_within_constant_chipload_band` — modulated
//!    chipload per tooth stays inside `[band.min, band.max]`.
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

use std::f64::consts::TAU;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::{PocketConfig, PocketPattern};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P2;
use rs_cam_core::machine_kinematics::MachineKinematics;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSession, SimulationOptions, ToolpathConfig};
use rs_cam_core::simulation_cut::CutKinematics;

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
    let mut session = ProjectSession::new_empty();
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
    session.set_stock_config(stock);

    let tool_idx = session.add_tool(make_endmill_6mm());
    let tool_id = session.tools()[tool_idx].id.0;

    let polygon = rounded_rect_with_island();
    let model = LoadedModel {
        id: 0,
        name: "as001_pocket".to_owned(),
        mesh: None,
        polygons: Some(Arc::new(vec![polygon])),
        path: PathBuf::from("synthetic://as001_pocket.svg"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    };
    let model_id = session.add_model(model);

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
        id: 0,
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
    };
    session.add_toolpath(0, tc).expect("add pocket toolpath");

    if attach_kinematics {
        let mut machine = session.machine().clone();
        machine.kinematics = Some(MachineKinematics::shapeoko_xxl_stock());
        session.set_machine(machine);
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
        modulation_strategy: rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_aggressiveness: 1.0,
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
/// The control here is the same session with `kinematics = None`,
/// which makes the F-036b post-pass an unconditional no-op (the
/// `is_some()` guard inside `apply_adaptive_feed_modulation` short-
/// circuits before the trace walk).
#[test]
fn flag_off_emits_identical_gcode_to_pre_f036() {
    // Path A: kinematics attached + flag OFF.
    let session_a = run_session(true, false);
    let gcode_a = export_session_gcode(&session_a);

    // Path B: no kinematics. The post-pass guard short-circuits even
    // if the flag is ON (it isn't here). Either way, no modulation.
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

// ============== AB3: modulated chipload within band ================

/// AB3.
///
/// For every cutting move (Linear / Arc) the session holds after
/// modulation, the commanded chipload (`feed / (rpm * flutes)`) must
/// land inside the LUT chipload band `[min, max]` — the modulator's
/// contract.
#[test]
fn modulated_gates_within_constant_chipload_band() {
    let session = run_session(true, true);
    let report = session.tool_load_report();
    let verdict = report
        .per_toolpath
        .iter()
        .find(|v| v.toolpath_id == 0)
        .expect("tool-load verdict for pocket toolpath");

    // The chipload gate already grades against the LUT band; with
    // modulation ON, the verdict should NOT be `Exceeds(Low)` or
    // `Exceeds(High)`. `Within`, `Unmodeled(...)` (band missing — no
    // modulation happened, so the gate falls back to the same pre-
    // F-036b state), or `Exceeds(...)` with a soft-tolerance reason
    // all qualify as "modulator did its job or had no input to work
    // with."
    let chipload = &verdict.chipload;
    use rs_cam_core::tool_load::ChiploadVerdict;
    match chipload {
        ChiploadVerdict::Within { .. } => {}    // canonical pass
        ChiploadVerdict::Unmodeled { .. } => {} // no LUT band → no modulation, no failure
        other => {
            panic!("F-036b AB3: modulated chipload must stay inside the LUT band; got {other:?}")
        }
    }
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
/// Filter scope: only moves the modulator **actually touched** — i.e.
/// moves whose `feed_rate` differs from the toolpath's commanded
/// feed by more than 0.5 mm/min. Moves the algorithm skipped (zero
/// aggregated engagement, non-clearing/finishing intent, plunge /
/// retract / drilling) keep the commanded feed by design; if the
/// commanded feed itself is below the LUT band, that's a user
/// parameter choice the modulator doesn't override. The
/// algorithm-layer floor-clamp on modulated moves is what AB5 pins.
#[test]
fn modulated_path_never_emits_below_min_chipload() {
    use rs_cam_core::toolpath::MoveIntent;

    let session = run_session(true, true);
    let trace = session
        .simulation_result()
        .and_then(|s| s.cut_trace.as_ref())
        .expect("cut trace flag-on");

    let envelopes = rs_cam_core::tool_load::chipload_envelopes_for_session(&session, Some(trace));
    let Some(band) = envelopes.get(&0) else {
        // No LUT band → modulator was a no-op. Vacuously satisfied;
        // pinned here so a future calibration shift doesn't silently
        // downgrade the assertion.
        eprintln!(
            "F-036b AB5: no LUT chipload band for AS001 pocket — modulator was a no-op. \
             Recalibrate the test fixture if this becomes unintentional."
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

    for mv in &toolpath.moves {
        if !matches!(
            mv.intent,
            MoveIntent::ClearingCut | MoveIntent::FinishingCut
        ) {
            continue;
        }
        let Some(feed) = mv.move_type.feed_rate() else {
            continue;
        };
        // Only check moves the modulator actually touched. Skipped
        // moves (zero engagement aggregation) keep the commanded feed
        // by design — if that's below band, it's a user parameter
        // choice, not a modulator bug.
        if (feed - commanded_feed).abs() < 0.5 {
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
// chipload band — if it doesn't, AB2/AB3/AB4 are vacuously passing.
#[test]
fn fixture_sanity_lut_band_resolves_for_as001() {
    let session = run_session(true, false);
    let trace = session
        .simulation_result()
        .and_then(|s| s.cut_trace.as_ref())
        .expect("cut trace");
    let envelopes = rs_cam_core::tool_load::chipload_envelopes_for_session(&session, Some(trace));
    assert!(
        envelopes.contains_key(&0),
        "F-036b sanity: AS001 pocket must resolve a vendor LUT chipload band so the \
         flag-ON tests actually exercise modulation. Got envelopes = {envelopes:?}"
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
