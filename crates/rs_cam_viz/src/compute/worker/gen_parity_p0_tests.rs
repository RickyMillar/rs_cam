//! Phase 0 characterization — "Generation agrees across session and GUI
//! worker entry points" (`planning/arch_consolidation_2026-09-09/PLAN.md`,
//! the Phase 0 characterization list).
//!
//! # One door
//!
//! The worker mirrors no core module. It runs core's `Job` steps: the
//! controller submits a request, and the worker's `run_compute` calls
//! `rs_cam_core::session::execute_job`, which runs the generation, the
//! dressups and the boundary clip. `ProjectSession::generate_toolpath` runs
//! the same steps. One input assembly, one executor.
//!
//! WP11b made that true. Before it the worker held its own
//! `generate_via_core` and its own `apply_dressups`, and everything before
//! the executor was written twice. WP12 then made the loose executor
//! `pub(crate)`, so no viz source can reach it at all.
//!
//! These tests drive both entry points from ONE `ProjectSession` and compare
//! the emitted motion. The module is in-crate because `run_compute` is
//! `pub(super)`.
//!
//! # What this fixture reaches
//!
//! Tracker row N12 (`planning/arch_consolidation_2026-09-09/STATUS.md`)
//! lists seven generation-input divergences. A fresh-stock 2D Pocket over an
//! SVG rectangle reaches exactly ONE of them:
//!
//! * **N12 item 3 — the feed-optimisation stock.** The viz door BUILT one
//!   and the core door passed `None`, so one configuration emitted two sets
//!   of feed rates. `DressupConfig::default()` carries
//!   `feed_optimization: true` (`core/compute/config.rs:2007`) and
//!   `feed_optimization_unavailable_reason` (`core/compute/catalog.rs:2749`)
//!   refuses only remaining-stock, Rest and 3D operations, so the pass is
//!   LIVE on this fixture. WP11b closed the item: the GUI door runs core's
//!   `start` / `execute_job` steps and the core door reads the stock.
//!
//! The other six stay unpinned, and the next phase must reach them with
//! other fixtures:
//!
//! * **Items 1 and 2 — `face_selection` and a `BoundarySource::FaceSelection`
//!   boundary.** Both need a STEP model with picked BREP faces. This crate
//!   has no in-memory STEP fixture, so no test here can build one.
//! * **Item 4 — the dressup stock-top frame** (core `emission_stock_bbox.max.z`
//!   against viz `req.heights.top_z`). It needs a PINNED Top Z. With the
//!   default Auto top the two are equal by construction:
//!   `HeightsConfig::resolve` returns `ctx.stock_top_z`, and both doors take
//!   that from `SetupEvalContext::heights_stock_bbox`.
//! * **Item 5 — the entry-probe index.** It needs a mesh and an entry
//!   dressup. `DressupConfig::default()` sets `entry_style` to
//!   `DressupEntryStyle::None`, and a prism operation declares no
//!   `entry_probe_leave`, so neither door builds a probe here.
//! * **Item 6 — the `HeightContext` builders.** Both generation builders read
//!   the model top from the MESH alone, so a polygon-only model gives `None`
//!   on both sides and the difference cannot show.
//! * **Item 7 — the open-coded single-polygon clip.** It needs an enabled
//!   boundary. `BoundaryConfig::default()` has `enabled: false`.
//!
//! # Why the second test now asserts identity
//!
//! Phase 3 owned the fix: one resolver feeds both doors. The second test
//! pinned the divergence until WP11b, and inverted into full identity when
//! the core door gained the feed-optimisation stock. The first test absorbs
//! the claim and stays green, which is the evidence the flip is real. The
//! behavioural half of the item — that the core door modulates at all — is
//! measured in core, at `tests/gen_inputs_one_assembly_n12.rs`, because an
//! identity between two routes that call one function is true by
//! construction.

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::fingerprint::{ToolpathFingerprint, diff_fingerprints};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_core::toolpath::Toolpath;

use crate::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest,
    GenerationControl, LaneSnapshot, OptimizeRequest, SimulationRequest, ToolpathSubmitOutcome,
};
use crate::controller::AppController;
use crate::state::job::{ToolConfig, ToolId, ToolType};
use crate::state::toolpath::{ComputeStatus, DressupConfig, OperationConfig, ToolpathId};
use crate::ui::AppEvent;

/// Positional agreement bar, copied from `worker::tests::assert_toolpaths_match`.
const POSITION_EPSILON_MM: f64 = 1e-9;

/// How many mismatches one panic message prints before it summarises.
const MAX_REPORTED_MISMATCHES: usize = 8;

// ── A backend that records the request and computes nothing ───────────

/// The controller keeps its backend private, so the record lives behind an
/// `Arc` the test still holds after the backend moves into the controller.
/// `ComputeBackend` requires `Send`, so this is a `Mutex`, not a `RefCell`.
struct RecordingBackend {
    submitted: Arc<Mutex<Vec<ComputeRequest>>>,
}

impl ComputeBackend for RecordingBackend {
    fn submit_toolpath(&mut self, request: ComputeRequest) -> ToolpathSubmitOutcome {
        if let Ok(mut log) = self.submitted.lock() {
            log.push(request);
        }
        ToolpathSubmitOutcome::Queued
    }
    fn submit_simulation(&mut self, _request: SimulationRequest) {}
    fn submit_collision(&mut self, _request: CollisionRequest) {}
    fn submit_optimize(&mut self, _request: OptimizeRequest) {}
    fn cancel_lane(&mut self, _lane: ComputeLane) {}
    fn drain_results(&mut self) -> Vec<ComputeMessage> {
        Vec::new()
    }
    fn lane_snapshot(&self, lane: ComputeLane) -> LaneSnapshot {
        LaneSnapshot::idle(lane)
    }
    fn generation_control(&self) -> GenerationControl {
        GenerationControl::detached()
    }
}

// ── The fixture ───────────────────────────────────────────────────────

/// One controller, the record its backend writes, and the toolpath both
/// doors generate.
struct Fixture {
    controller: AppController<RecordingBackend>,
    submitted: Arc<Mutex<Vec<ComputeRequest>>>,
    tp_id: ToolpathId,
    tp_index: usize,
}

/// One 6.35 mm end mill, one 40 x 40 SVG rectangle, one default Pocket.
///
/// The recipe follows `controller::results_parity_tests::parity_controller`.
/// `add_model` runs `StockConfig::update_from_bbox`, and the 2D arm of that
/// function puts the stock TOP at the polygon plane (`origin_z` becomes
/// `-25.0`), so the default Pocket's cuts at Z = -1.5 and Z = -3.0 sit
/// inside material. A stock the cuts never reach would make the
/// feed-optimisation pass read air everywhere and the divergence would hide.
fn fixture(feed_optimization: bool) -> Fixture {
    let submitted = Arc::new(Mutex::new(Vec::new()));
    let mut controller = AppController::with_backend(RecordingBackend {
        submitted: Arc::clone(&submitted),
    });
    let tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    let mut builder = ProjectSessionBuilder::new().tool(tool);
    let _ = builder.add_model(LoadedModel {
        id: 0,
        path: std::path::PathBuf::from("plate.svg"),
        name: "Plate".to_owned(),
        kind: Some(ModelKind::Svg),
        mesh: None,
        polygons: Some(Arc::new(vec![Polygon2::rectangle(
            -20.0, -20.0, 20.0, 20.0,
        )])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
    });
    let tp_config = ToolpathConfig {
        id: ToolpathId(0),
        name: "Pocket".to_owned(),
        enabled: true,
        operation: OperationConfig::Pocket(Default::default()),
        dressups: DressupConfig {
            feed_optimization,
            ..Default::default()
        },
        heights: Default::default(),
        tool_id: 1,
        model_id: 0,
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        stock_source: Default::default(),
        coolant: Default::default(),
        face_selection: None,
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        rest_analysis: Default::default(),
        planner_origin: None,
    };
    let tp_index = builder
        .add_toolpath(0, tp_config)
        .expect("the default setup accepts a toolpath");
    controller.state.session = builder.build();
    let tp_id = controller.state.session.toolpath_configs()[tp_index].id;
    Fixture {
        controller,
        submitted,
        tp_id,
        tp_index,
    }
}

/// Drive both doors over one session and return (GUI door, session door).
///
/// The GUI request is the one `submit_toolpath_compute` built. The test sets
/// no `ComputeRequest` field by hand — that is the point: the fields under
/// test are the ones the controller resolves.
fn generate_through_both_doors(feed_optimization: bool) -> (Toolpath, Toolpath) {
    let Fixture {
        mut controller,
        submitted,
        tp_id,
        tp_index,
    } = fixture(feed_optimization);

    // Arm A, first half: the toolpath panel's own Generate event. The
    // recording backend keeps the request instead of running it.
    controller.handle_internal_event(AppEvent::GenerateToolpath(tp_id));
    let request = {
        let mut log = submitted.lock().expect("the record is not poisoned");
        assert_eq!(
            log.len(),
            1,
            "the GUI door must submit exactly one request; the toolpath reads {}",
            submit_state(&controller, tp_id)
        );
        log.pop().expect("one request is recorded")
    };

    let cancel = AtomicBool::new(false);

    // Arm A, second half: the worker runs that request.
    let gui = super::execute::run_compute(&request)
        .result
        .expect("the GUI worker door generates a toolpath")
        .annotated
        .toolpath
        .clone();

    // Arm B: the session door, over the same session and the same config.
    let session = controller
        .state
        .session
        .generate_toolpath(tp_index, &cancel)
        .expect("the session door generates a toolpath")
        .annotated()
        .toolpath
        .clone();

    (gui, session)
}

/// What the GUI recorded about a submit, for a failure message.
fn submit_state(controller: &AppController<RecordingBackend>, tp_id: ToolpathId) -> String {
    match controller.state.gui.toolpath_rt.get(&tp_id) {
        None => "no runtime entry".to_owned(),
        Some(rt) => match &rt.status {
            ComputeStatus::Error(message) => format!("Error: {message}"),
            other => other.label().to_owned(),
        },
    }
}

// ── Comparators ───────────────────────────────────────────────────────

/// Whether the feed rate is part of the move comparison.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Feeds {
    Compare,
    /// The item-3 flip left no caller: both doors now compare the feed rates.
    /// The variant stays because the comparator below documents it.
    #[allow(dead_code)]
    Ignore,
}

fn cutting_move_count(toolpath: &Toolpath) -> usize {
    toolpath
        .moves
        .iter()
        .filter(|mv| mv.move_type.is_cutting())
        .count()
}

/// Both doors produced real motion.
///
/// A gate handed an empty population passes and looks healthy, so the
/// identity claim below is worth nothing until this runs first.
fn assert_non_vacuous(gui: &Toolpath, session: &Toolpath) {
    assert!(
        !gui.moves.is_empty(),
        "the GUI worker door produced no moves"
    );
    assert!(
        !session.moves.is_empty(),
        "the session door produced no moves"
    );
    assert!(
        cutting_move_count(gui) > 0,
        "the GUI worker door produced no cutting moves"
    );
    assert!(
        cutting_move_count(session) > 0,
        "the session door produced no cutting moves"
    );
}

/// Compare two toolpaths move by move.
///
/// `MoveType::with_feed_rate(0.0)` normalises the feed out and keeps the
/// variant and the arc I/J offsets, so `Feeds::Ignore` still compares the
/// geometry an arc carries in its move type.
fn assert_same_moves(gui: &Toolpath, session: &Toolpath, feeds: Feeds) {
    let mut mismatches: Vec<String> = Vec::new();

    if gui.moves.len() != session.moves.len() {
        mismatches.push(format!(
            "move count: gui {} vs session {}",
            gui.moves.len(),
            session.moves.len()
        ));
    }

    for (index, (lhs, rhs)) in gui.moves.iter().zip(&session.moves).enumerate() {
        let left = lhs.move_type.with_feed_rate(0.0);
        let right = rhs.move_type.with_feed_rate(0.0);
        if left != right {
            mismatches.push(format!("move {index}: type {left:?} vs {right:?}"));
        }
        if (lhs.target.x - rhs.target.x).abs() >= POSITION_EPSILON_MM
            || (lhs.target.y - rhs.target.y).abs() >= POSITION_EPSILON_MM
            || (lhs.target.z - rhs.target.z).abs() >= POSITION_EPSILON_MM
        {
            mismatches.push(format!(
                "move {index}: target ({}, {}, {}) vs ({}, {}, {})",
                lhs.target.x, lhs.target.y, lhs.target.z, rhs.target.x, rhs.target.y, rhs.target.z
            ));
        }
        if feeds == Feeds::Compare && lhs.move_type.feed_rate() != rhs.move_type.feed_rate() {
            mismatches.push(format!(
                "move {index}: feed {:?} vs {:?}",
                lhs.move_type.feed_rate(),
                rhs.move_type.feed_rate()
            ));
        }
    }

    if !mismatches.is_empty() {
        let shown: Vec<&str> = mismatches
            .iter()
            .take(MAX_REPORTED_MISMATCHES)
            .map(String::as_str)
            .collect();
        panic!(
            "the two generation doors disagree ({} mismatches)\n{}\n{}",
            mismatches.len(),
            shown.join("\n"),
            fingerprint_report(gui, session)
        );
    }
}

/// A readable shape summary of the disagreement.
fn fingerprint_report(gui: &Toolpath, session: &Toolpath) -> String {
    let diff = diff_fingerprints(
        &ToolpathFingerprint::from_toolpath(gui),
        &ToolpathFingerprint::from_toolpath(session),
    );
    if !diff.has_changes() {
        return "fingerprints agree; the difference is below their resolution".to_owned();
    }
    let rows: Vec<String> = diff
        .changed_fields
        .iter()
        .map(|change| {
            format!(
                "  {}: gui {} vs session {}",
                change.field, change.before, change.after
            )
        })
        .collect();
    format!("fingerprint fields that differ:\n{}", rows.join("\n"))
}

/// How many fed moves carry a different feed rate on the two doors.
///
/// A rapid carries no feed rate, so it can never enter this count.
fn feed_disagreement_count(gui: &Toolpath, session: &Toolpath) -> usize {
    gui.moves
        .iter()
        .zip(&session.moves)
        .filter(|(lhs, rhs)| {
            lhs.move_type.is_cutting() && lhs.move_type.feed_rate() != rhs.move_type.feed_rate()
        })
        .count()
}

// ── The tests ─────────────────────────────────────────────────────────

/// With feed optimisation off the two doors emit ONE toolpath.
///
/// This is the invariant Phase 3 must keep: two entry points, one answer,
/// feed rates included.
#[test]
fn the_two_doors_generate_one_geometry_with_feed_optimization_off() {
    let (gui, session) = generate_through_both_doors(false);
    assert_non_vacuous(&gui, &session);
    assert_same_moves(&gui, &session, Feeds::Compare);
}

/// With the shipped default the two doors emit ONE set of feed rates —
/// N12 item 3, closed.
///
/// The geometry always agreed: `apply_dressups` runs feed optimisation LAST
/// (step 8), after the rapid reorder, and `optimize_feed_rates` rewrites the
/// feed rate in place — move count, order, targets, arc offsets and intents
/// all pass through (`core/src/feedopt.rs:179-190`). So the divergence was
/// confined to the feed rate.
///
/// This test PINNED the divergence until WP11b. The GUI door now runs core's
/// `start` / `execute_job` steps, so one resolver answers both doors and the
/// core door reads the feed-optimisation stock it used to pass as `None`.
/// The assertion is inverted, per the flip note this comment replaces: the
/// feeds agree, and the test above absorbs the claim.
#[test]
fn with_the_shipped_default_the_two_doors_emit_one_set_of_feed_rates() {
    let (gui, session) = generate_through_both_doors(true);
    assert_non_vacuous(&gui, &session);

    // The geometry is one answer, and so are the feeds.
    assert_same_moves(&gui, &session, Feeds::Compare);

    let differing = feed_disagreement_count(&gui, &session);
    assert_eq!(
        differing,
        0,
        "N12 item 3 is closed: one resolver feeds both doors, so no fed \
         move may carry a different feed rate; {differing} of the {} fed \
         moves differ",
        cutting_move_count(&gui)
    );
}
