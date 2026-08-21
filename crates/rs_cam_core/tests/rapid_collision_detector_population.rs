//! The rapid-collision detector's POPULATION, asserted rather than assumed.
//!
//! Oracle: `planning/perf_review_2026-08-19/RESEARCH_corpus_instruments.md`
//! §2 (b) — "211 baseline rapid collisions are 0 today: is the detector
//! blind?".
//!
//! ## Why this file exists
//!
//! The `2026-06-04` smoke baseline recorded 211 rapid collisions across four
//! cases (AS007 6, AS009 1, AS010 104, AS017 100). Every one of them reads
//! `0` today. There are two readings of that, and they call for opposite
//! responses:
//!
//! 1. the diving rapids stopped being emitted — a real improvement; or
//! 2. the DETECTOR stopped seeing them — a silent loss of a safety
//!    instrument, wearing the costume of a fix.
//!
//! A count of zero cannot distinguish them on its own. `CLAUDE.md` states
//! the general form: *"a gate handed an empty population passes and looks
//! healthy"* — so before a zero is read as good news, the instrument that
//! produced it has to be shown to have a non-empty population.
//!
//! ## The experiment
//!
//! A control/forced pair, identical in **every** respect except one height:
//!
//! * control — all five heights `Auto` (the shipped resolution);
//! * forced — `retract_z = Manual(-3.0)`, which parks every link/retract
//!   rapid 3 mm BELOW the stock top (stock top is world Z = 0 here, the
//!   `origin_z = -12` frame the 2D fixtures use).
//!
//! The control must read exactly `0` and the forced arm must read a
//! substantial positive count. Both assertions carry weight: without the
//! control the positive one could come from a detector that flags
//! everything, and without the forced arm the corpus's zeros mean nothing.
//!
//! ## Why not a corpus row
//!
//! The smoke runner gives every toolpath it adds a `HeightsConfig::default()`
//! (`crates/rs_cam_cli/src/smoke.rs`), i.e. all five heights `Auto`. The
//! corpus therefore *cannot express* a pinned retract, and a corpus row could
//! not carry this fixture without also changing `SmokeCase`/`BaselineRow`.
//! It belongs here.
//!
//! ## What is read
//!
//! `session.diagnostics().per_toolpath[].rapid_collision_count` — the exact
//! field the smoke runner writes to its baseline CSV and the CLI `project`
//! report publishes — cross-checked against the raw
//! `SimulationResult::rapid_collisions` the detector produces. If those two
//! ever disagree, the corpus is reading a different number from the one the
//! simulator measured, which is its own defect.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

mod common;
use common::make_endmill_6mm;

use std::f64::consts::TAU;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{
    BoundaryConfig, DressupConfig, HeightMode, HeightsConfig, StockSource,
};
use rs_cam_core::compute::operation_configs::{PocketConfig, PocketPattern};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P2;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSession, SimulationOptions, ToolpathConfig};

/// Stock top in world Z for this fixture. `origin_z = -12` with `z = 12`,
/// so the top of the material sits at 0 — the frame the shipped 2D
/// templates use.
const STOCK_TOP_Z: f64 = 0.0;

/// How far below the stock top the forced arm parks its retract plane.
const FORCED_RETRACT_BELOW_TOP_MM: f64 = 3.0;

/// The `demo_pocket.svg` shape: a 70×50 rectangle with a Ø20 island.
fn rect_with_island() -> Polygon2 {
    let exterior = vec![
        P2::new(5.0, 5.0),
        P2::new(75.0, 5.0),
        P2::new(75.0, 55.0),
        P2::new(5.0, 55.0),
    ];
    let mut hole = Vec::with_capacity(64);
    let (cx, cy, r, n) = (40.0, 30.0, 10.0, 64);
    for i in 0..n {
        let t = (i as f64) * TAU / (n as f64);
        // CW winding for holes (negative area).
        hole.push(P2::new(cx + r * (-t).cos(), cy + r * (-t).sin()));
    }
    Polygon2::with_holes(exterior, vec![hole])
}

/// The pair's shared session. `retract_z` is the ONLY thing either arm
/// changes — same stock, same tool, same geometry, same operation params,
/// same dressups, same simulation options.
fn build_session(retract_z: HeightMode) -> ProjectSession {
    let mut session = ProjectSession::new_empty();

    session.set_stock_config(StockConfig {
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
    });

    let tool_idx = session.add_tool(make_endmill_6mm());
    let tool_id = session.tools()[tool_idx].id.0;

    let model_id = session.add_model(LoadedModel {
        id: 0,
        name: "demo_pocket".to_owned(),
        mesh: None,
        polygons: Some(Arc::new(vec![rect_with_island()])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: PathBuf::from("synthetic://demo_pocket.svg"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    });

    let tc = ToolpathConfig {
        id: ToolpathId(0),
        name: "Collision population pocket".to_owned(),
        enabled: true,
        operation: OperationConfig::Pocket(PocketConfig {
            stepover: 2.4,
            depth: 4.0,
            depth_per_pass: 2.0,
            feed_rate: 2400.0,
            plunge_rate: 600.0,
            climb: true,
            pattern: PocketPattern::Contour,
            angle: 0.0,
            finishing_passes: 0,
            spindle_rpm: Some(18_000),
        }),
        // Shipped roughing dressups, identical on both arms.
        dressups: DressupConfig::for_op(OperationType::Pocket),
        heights: HeightsConfig {
            retract_z,
            ..HeightsConfig::default()
        },
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
    };
    session.add_toolpath(0, tc).expect("add pocket toolpath");
    session
}

/// Generate + simulate one arm and return
/// `(diagnostics count, raw detector count, move count)`.
fn measure(retract_z: HeightMode) -> (u32, usize, usize) {
    let mut session = build_session(retract_z);
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generate pocket toolpath");

    let move_count = session
        .get_result(0)
        .map(|r| r.toolpath().moves.len())
        .unwrap_or(0);
    // Non-vacuity: both arms must actually cut something. The forced arm
    // emits FEWER moves than the control (its retract plane is lower, so
    // link geometry shortens), which is why this floor is well below the
    // control's count rather than tuned to it.
    assert!(
        move_count > 100,
        "fixture must emit a substantive path or neither arm proves anything (got {move_count})"
    );

    let opts = SimulationOptions {
        resolution: 0.5,
        skip_ids: Vec::new(),
        metrics_enabled: true,
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: false,
        modulation_strategy: rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_aggressiveness: 1.0,
    };
    session
        .run_simulation(&opts, &cancel)
        .expect("simulation completes");

    let raw = session
        .simulation_result()
        .expect("simulation result")
        .rapid_collisions
        .len();

    let diag = session.diagnostics();
    let published = diag
        .per_toolpath
        .first()
        .map(|d| d.rapid_collision_count as u32)
        .expect("diagnostics carries a per-toolpath row");

    (published, raw, move_count)
}

/// The control arm: shipped `Auto` heights, no dive, no collisions.
///
/// This is the half that gives the corpus's `0`s their meaning. If it ever
/// goes non-zero, the forced arm below stops being evidence of anything.
#[test]
fn auto_heights_control_reads_zero_collisions() {
    let (published, raw, moves) = measure(HeightMode::Auto);
    eprintln!("control (retract Auto): {moves} moves, {published} collisions");
    assert_eq!(
        published, 0,
        "the control arm must be clean or the forced arm proves nothing"
    );
    assert_eq!(
        raw, 0,
        "the raw detector and the published count must agree on the control"
    );
}

/// The forced arm: `retract_z` pinned 3 mm below the stock top, so every
/// link/retract rapid travels through uncut material.
///
/// The floor is deliberately loose (research measured 35 through the CLI
/// `project` path). The claim under test is a POPULATION claim — the
/// detector fires substantially when a dive exists — not a claim about a
/// particular number, which would make the file a golden and would break on
/// any legitimate change to pocket link/retract emission.
#[test]
fn forced_dive_reads_a_substantial_collision_population() {
    let (published, raw, moves) = measure(HeightMode::Manual(
        STOCK_TOP_Z - FORCED_RETRACT_BELOW_TOP_MM,
    ));
    eprintln!("forced (retract -3.0): {moves} moves, {published} collisions");
    assert!(
        published >= 10,
        "a retract plane 3 mm inside the stock must produce a substantial \
         rapid-collision population; got {published}. If this reads 0 the \
         detector has gone blind and every 0 elsewhere — including the \
         corpus's 211 → 0 — is uninterpretable"
    );
    assert_eq!(
        published as usize, raw,
        "the count the corpus and the CLI report read \
         (diagnostics().per_toolpath[].rapid_collision_count) must equal the \
         detector's own output (SimulationResult::rapid_collisions); a gap \
         means the baseline is reading a different quantity from the one \
         the simulator measured"
    );
}

/// The pair, as one statement: the ONLY difference is the retract height,
/// and it separates the two readings. This is what discharges "is the
/// detector blind?" — a question a single arm cannot answer.
#[test]
fn the_pair_separates_and_only_the_retract_height_differs() {
    let (control, _, control_moves) = measure(HeightMode::Auto);
    let (forced, _, forced_moves) = measure(HeightMode::Manual(
        STOCK_TOP_Z - FORCED_RETRACT_BELOW_TOP_MM,
    ));
    eprintln!(
        "pair: control {control} collisions / {control_moves} moves, \
         forced {forced} collisions / {forced_moves} moves"
    );
    assert_eq!(control, 0);
    assert!(
        forced > control,
        "control {control} vs forced {forced} — the fixture stopped separating"
    );
}
