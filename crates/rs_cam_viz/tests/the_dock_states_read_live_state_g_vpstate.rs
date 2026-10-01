//! G-VPSTATE: the viewport dock reads live state (viewport redesign, phases
//! 3 and 4).
//!
//! - (a) each of the seven `RowState` arms is reachable from a state
//!   fixture: Computing from a running job, Stale from an epoch bump,
//!   Failed from a recorded error;
//! - (b) a `Compute & show` result tagged with an old target or an old mode
//!   does not take over the rendering;
//! - (c) `Compute` and `Compute & show` have two distinct outcomes;
//! - (d) every active colour encoding has a real legend line, over at least
//!   seven distinct encodings;
//! - (e) at 320 × 600 pt with six legends, the dock stack stays inside its
//!   height budget and every popover stays inside the viewport;
//! - (f) Tab reaches the four section buttons in order and then the open
//!   popover; a click outside closes the popover; `Escape` closes one
//!   surface at a time;
//! - (g) the compact legend (operator ruling 2026-10-02): it never covers
//!   the dock or the orientation gizmo at any common viewport size, its
//!   caveat lines paint only while the pointer is on a chip, and folded it
//!   is one `Legend (n)` chip.
//!
//! Two mirrored render colours and the rest ramp top are pinned against
//! their render sources, so the legend cannot drift from the picture.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rs_cam_core::ToolpathId;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::geo::P3;
use rs_cam_core::maps::grid::GridSpec;
use rs_cam_core::maps::rest_heatmap_mesh::{rest_grid_to_heatmap_mesh, rest_ramp_color};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    AddToolpathArgs, AdoptResultArgs, Command, InvalidateStockArgs, LoadedModel,
    ProjectSessionBuilder, ToolpathConfig,
};
use rs_cam_core::stock::collision::{
    AssemblySegment, CollisionEvent, CollisionKind, CollisionReport,
};
use rs_cam_core::surface::rest_field::RestGrid;
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;
use rs_cam_viz::compute::{ComputeLane, LaneSnapshot};
use rs_cam_viz::render::camera::ProjectionMode;
use rs_cam_viz::state::job::{ModelKind, ModelUnits};
use rs_cam_viz::state::overlays::{DockSection, OverlayPanelState};
use rs_cam_viz::state::runtime::{ComputeStatus, ReachStatus, ToolpathRuntime};
use rs_cam_viz::state::selection::Selection;
use rs_cam_viz::state::toolpath::ToolpathResult;
use rs_cam_viz::state::viewport::ToolpathColorMode;
use rs_cam_viz::state::{AppState, Workspace};
use rs_cam_viz::ui::overlays::legend_rail::{self, RailLine, mirrored};
use rs_cam_viz::ui::overlays::live::{JobLane, Recovery};
use rs_cam_viz::ui::overlays::panel::{self, ComputeOutcome, RowState, ShowSettle};
use rs_cam_viz::ui::overlays::registry::{self, OverlaySurface};
use rs_cam_viz::ui::viewport_overlay::{self, DOCK_AREA_ID, POPOVER_AREA_ID};
use rs_cam_viz::ui::{AppEvent, tokens};

// ── fixtures ─────────────────────────────────────────────────────────

const TOOL: usize = 1;
const MODEL: usize = 4;

fn source(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    assert!(
        text.len() > 500,
        "{} is too short to be the file this scan means",
        path.display()
    );
    text
}

fn row(id: &str) -> &'static registry::OverlayRow {
    registry::row(id).unwrap_or_else(|| panic!("registry row `{id}` is gone"))
}

fn state_of(state: &AppState, id: &str) -> RowState {
    RowState::of(state, row(id))
}

/// A Toolpaths state with one tool and one 2D model, and no toolpath.
fn fresh_state() -> AppState {
    let mut state = AppState::new();
    let mut tool = ToolConfig::new_default(ToolId(TOOL), ToolType::EndMill);
    tool.diameter = 6.0;
    state.session = ProjectSessionBuilder::new()
        .tool(tool)
        .model(LoadedModel {
            id: MODEL,
            path: PathBuf::from("model.svg"),
            name: "2D".to_owned(),
            kind: Some(ModelKind::Svg),
            mesh: None,
            polygons: Some(Arc::new(vec![Polygon2::rectangle(
                -10.0, -10.0, 10.0, 10.0,
            )])),
            drill_targets: Arc::new(Vec::new()),
            layers: Arc::new(Vec::new()),
            enriched_mesh: None,
            units: Some(ModelUnits::Millimeters),
            winding_report: None,
            load_error: None,
        })
        .build();
    state.workspace = Workspace::Toolpaths;
    state
}

fn config(name: &str) -> ToolpathConfig {
    ToolpathConfig {
        id: ToolpathId(0),
        name: name.to_owned(),
        enabled: true,
        operation: OperationConfig::Pocket(Default::default()),
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: TOOL,
        model_id: MODEL,
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
    }
}

fn sample_path() -> Toolpath {
    let mut path = Toolpath::new();
    path.rapid_to(P3::new(0.0, 0.0, 5.0));
    path.feed_to(P3::new(10.0, 0.0, -1.0), 600.0);
    path.feed_to(P3::new(10.0, 10.0, -1.0), 600.0);
    path.rapid_to(P3::new(10.0, 10.0, 5.0));
    path
}

/// A 5 × 5 rest grid with one outlier, so its 95th percentile is well
/// under its maximum.
fn rest_grid() -> RestGrid {
    let spec = GridSpec {
        nx: 5,
        ny: 5,
        origin_x: 0.0,
        origin_y: 0.0,
        cell_mm: 1.0,
    };
    let mut rest: Vec<f32> = (0..25).map(|i| 0.2 + 0.05 * i as f32).collect();
    rest[0] = f32::NAN;
    rest[24] = 10.0;
    let mut surface_z = vec![0.0_f32; 25];
    surface_z[7] = f32::NAN;
    RestGrid {
        grid: spec,
        rest,
        surface_z,
        threshold: 0.3,
    }
}

fn annotated(with_grid: bool) -> AnnotatedToolpath {
    let mut annotated = AnnotatedToolpath::new(sample_path());
    if with_grid {
        annotated.rest_grid = Some(Arc::new(rest_grid()));
    }
    annotated
}

/// Add one toolpath and select it. `generated` puts a result in the core
/// cache; the viz store always holds a drawable copy, so without the core
/// result the toolpath reads `EditedSince`.
fn add_selected(state: &mut AppState, with_grid: bool, generated: bool) -> ToolpathId {
    let index = state
        .session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(config("Finish")),
        }))
        .unwrap()
        .created
        .expect("the AddToolpath row reports the new index");
    let id = state.session.toolpath_configs()[index].id;
    let mut rt = ToolpathRuntime::new(true);
    rt.status = ComputeStatus::Done;
    rt.result = Some(ToolpathResult {
        annotated: Arc::new(annotated(with_grid)),
        stats: Default::default(),
        debug_trace: None,
        semantic_trace: None,
        debug_trace_path: None,
        drill_op: None,
    });
    state.gui.toolpath_rt.insert(id, rt);
    if generated {
        let revision = state.session.toolpath_revision(index);
        let _effects = state
            .session
            .apply(Command::AdoptResult(AdoptResultArgs {
                index,
                revision,
                result: Box::new(rs_cam_core::session::ToolpathComputeResult {
                    op_data: rs_cam_core::ops::drill_op::OpData::Toolpath(Arc::new(annotated(
                        with_grid,
                    ))),
                    stats: Default::default(),
                    debug_trace: None,
                    semantic_trace: None,
                }),
            }))
            .expect("the index is in range");
    }
    state.selection = Selection::Toolpath(id);
    id
}

fn report(collisions: usize) -> CollisionReport {
    CollisionReport {
        collisions: (0..collisions)
            .map(|i| CollisionEvent {
                move_index: i,
                position: P3::new(i as f64, 0.0, 0.0),
                penetration_depth: 0.5,
                segment: AssemblySegment::Holder,
                kind: CollisionKind::Workpiece,
            })
            .collect(),
        min_safe_stickout: 30.0,
    }
}

/// A collision check that landed at the current epoch.
fn land_collision_check(state: &mut AppState, collisions: usize) {
    state.simulation.submitted_collision_epoch = None;
    state.simulation.checks.collision_report = Some(report(collisions));
    state.simulation.checks.checked_at_epoch = Some(state.session.simulation_epoch());
}

// ── (a) the seven arms ───────────────────────────────────────────────

#[test]
fn every_row_state_arm_is_reachable_from_live_state_g_vpstate() {
    let mut seen: BTreeMap<&'static str, String> = BTreeMap::new();
    let mut record = |state: RowState, case: &str| {
        seen.insert(state.name(), case.to_owned());
        state
    };

    // Showing and Blocked: the plain registry answers.
    let state = fresh_state();
    let grid = record(state_of(&state, "grid"), "grid, default on");
    assert_eq!(grid, RowState::Showing);
    let islands = record(state_of(&state, "planner_islands"), "no renderer");
    assert!(matches!(islands, RowState::Blocked { .. }), "{islands:?}");

    // Needs compute: no collision check has run.
    let needs = record(state_of(&state, "collisions"), "no check");
    assert!(
        matches!(needs, RowState::NeedsCompute { action, .. } if action == registry::OverlayAction::RunCollisionCheck),
        "{needs:?}"
    );

    // Computing from a running job: the submit stamp of the check.
    let mut state = fresh_state();
    state.simulation.submitted_collision_epoch = Some(state.session.simulation_epoch());
    let computing = record(state_of(&state, "collisions"), "check submitted");
    assert!(
        matches!(
            computing,
            RowState::Computing {
                lane: JobLane::Analysis,
                ..
            }
        ),
        "{computing:?}"
    );

    // Off: the check landed, current, flag off.
    let mut state = fresh_state();
    land_collision_check(&mut state, 0);
    state.viewport.show_collisions = false;
    let off = record(state_of(&state, "collisions"), "check landed");
    assert_eq!(off, RowState::Off);

    // Stale from an epoch bump: any stock edit drops the simulation.
    let before = state.session.simulation_epoch();
    let _effects = state
        .session
        .apply(Command::InvalidateStock(InvalidateStockArgs))
        .unwrap();
    assert!(
        state.session.simulation_epoch() > before,
        "non-vacuity: InvalidateStock no longer moves the simulation epoch"
    );
    let stale = record(state_of(&state, "collisions"), "epoch bumped");
    assert!(
        matches!(
            stale,
            RowState::Stale {
                recovery: Some(Recovery::RunCollisionCheck),
                ..
            }
        ),
        "{stale:?}"
    );

    // Reach: Computing and Failed for the selection.
    let mut state = fresh_state();
    let id = ToolpathId(7);
    state.selection = Selection::Toolpath(id);
    state.gui.reach_overlay.toolpath = Some(id);
    state.gui.reach_overlay.status = ReachStatus::Computing;
    let reach_computing = state_of(&state, "reach_map");
    assert!(
        matches!(
            reach_computing,
            RowState::Computing {
                lane: JobLane::Reach,
                switchable: true
            }
        ),
        "inventory §2.3 item 1: a computing walk must not read as Ready: {reach_computing:?}"
    );

    let message = "the drop-cutter walk found no surface";
    state.gui.reach_overlay.status = ReachStatus::Failed(message.to_owned());
    let failed = record(state_of(&state, "reach_map"), "reach walk failed");
    match &failed {
        RowState::Failed {
            message: shown,
            recovery: Recovery::RetryReach,
        } => assert_eq!(shown, message, "the failure text must be verbatim"),
        other => panic!("inventory §2.3 item 2: a failed walk reads {other:?}"),
    }
    // The MCP refusal carries the same failure, not "select a finishing
    // operation".
    let reason = (row("reach_map").precondition)(&state)
        .reason()
        .map(str::to_owned)
        .unwrap_or_default();
    assert!(
        reason.contains(message),
        "the reach precondition reason does not carry the failure: {reason}"
    );
    // Retry forgets the key; the scheduler asks again.
    let mut events = Vec::new();
    panel::run_recovery(&mut state, &mut events, Recovery::RetryReach);
    assert_eq!(state.gui.reach_overlay.toolpath, None);
    assert!(
        events.is_empty(),
        "Retry must start no compute from the draw loop"
    );

    assert_eq!(
        seen.keys().copied().collect::<Vec<_>>(),
        vec![
            "blocked",
            "computing",
            "failed",
            "needs_compute",
            "off",
            "showing",
            "stale"
        ],
        "every RowState arm must be reachable: {seen:?}"
    );
}

/// The rest row reads the one freshness answer of the selected toolpath.
#[test]
fn the_rest_row_reads_toolpath_freshness_g_vpstate() {
    let mut state = fresh_state();
    let id = add_selected(&mut state, true, true);
    assert_eq!(state_of(&state, "rest_heatmap"), RowState::Off);

    state.gui.toolpath_rt.get_mut(&id).unwrap().status = ComputeStatus::Computing;
    assert!(matches!(
        state_of(&state, "rest_heatmap"),
        RowState::Computing {
            lane: JobLane::Toolpath,
            ..
        }
    ));

    state.gui.toolpath_rt.get_mut(&id).unwrap().status =
        ComputeStatus::Error("pocket found no region".to_owned());
    assert!(matches!(
        state_of(&state, "rest_heatmap"),
        RowState::Failed {
            recovery: Recovery::Generate(failed),
            ..
        } if failed == id
    ));

    // EditedSince: the viz store holds the old grid, the core holds none.
    let mut state = fresh_state();
    let id = add_selected(&mut state, true, false);
    match state_of(&state, "rest_heatmap") {
        RowState::Stale { note, recovery } => {
            assert!(
                note.contains("#1"),
                "the stale line names the toolpath: {note}"
            );
            assert_eq!(recovery, Some(Recovery::Generate(id)));
        }
        other => panic!("inventory §2.3 item 7: an edited rest grid reads {other:?}"),
    }
}

// ── (b) a late result does not take over ─────────────────────────────

#[test]
fn a_result_for_an_old_target_does_not_take_over_g_vpstate() {
    let mut state = fresh_state();
    state.selection = Selection::Toolpath(ToolpathId(1));
    assert!(!state.viewport.show_collisions, "fixture precondition");
    let mut events = Vec::new();
    assert!(panel::request_compute(
        &mut state,
        &mut events,
        row("collisions"),
        ComputeOutcome::ComputeAndShow
    ));
    let request = state
        .overlays
        .pending_show
        .clone()
        .expect("the request is tagged");
    assert_eq!(request.target, Some(ToolpathId(1)));
    assert_eq!(request.preferred, None);

    // The operator moves on while the check runs.
    state.selection = Selection::Toolpath(ToolpathId(2));
    state.simulation.submitted_collision_epoch = Some(state.session.simulation_epoch());
    land_collision_check(&mut state, 3);
    assert_eq!(panel::settle_pending_show(&mut state), ShowSettle::Dropped);
    assert!(
        !state.viewport.show_collisions,
        "a result tagged with the old target switched the row on"
    );
    assert!(state.overlays.pending_show.is_none());

    // The mode tag: the operator chose Off for the model while the rest
    // grid computed.
    let mut state = fresh_state();
    let id = ToolpathId(9);
    state.selection = Selection::Toolpath(id);
    assert!(
        state.viewport.show_reach_map,
        "fixture: Reach is the default"
    );
    let mut events = Vec::new();
    // The rest button opens the confirm and starts nothing.
    assert!(panel::request_compute(
        &mut state,
        &mut events,
        row("rest_heatmap"),
        ComputeOutcome::ComputeAndShow
    ));
    assert_eq!(state.overlays.rest_confirm, Some(id));
    assert!(events.is_empty(), "the confirm must come before the work");
    assert!(state.overlays.pending_show.is_none());

    panel::compute_rest(&mut state, &mut events, id, ComputeOutcome::ComputeAndShow);
    assert!(matches!(events.as_slice(), [AppEvent::GenerateToolpath(t)] if *t == id));
    assert_eq!(state.overlays.rest_confirm, None);
    assert_eq!(
        state
            .overlays
            .pending_show
            .as_ref()
            .and_then(|p| p.preferred),
        Some("reach_map")
    );
    registry::clear_surface(&mut state, OverlaySurface::Model);
    assert_eq!(panel::settle_pending_show(&mut state), ShowSettle::Dropped);
    assert!(!state.viewport.show_rest_heatmap && !state.viewport.show_reach_map);
}

// ── (c) two outcomes ─────────────────────────────────────────────────

#[test]
fn compute_and_compute_and_show_have_distinct_outcomes_g_vpstate() {
    // Compute: the work runs, the flag stays off.
    let mut state = fresh_state();
    let mut events = Vec::new();
    assert!(panel::request_compute(
        &mut state,
        &mut events,
        row("collisions"),
        ComputeOutcome::ComputeOnly
    ));
    assert!(matches!(events.as_slice(), [AppEvent::RunCollisionCheck]));
    assert!(state.overlays.pending_show.is_none());
    land_collision_check(&mut state, 2);
    assert_eq!(
        panel::settle_pending_show(&mut state),
        ShowSettle::NoRequest
    );
    assert!(
        !state.viewport.show_collisions,
        "Compute switched the row on"
    );

    // Compute & show: the same work, and the row goes on when it lands.
    let mut state = fresh_state();
    let mut events = Vec::new();
    assert!(panel::request_compute(
        &mut state,
        &mut events,
        row("collisions"),
        ComputeOutcome::ComputeAndShow
    ));
    assert!(matches!(events.as_slice(), [AppEvent::RunCollisionCheck]));
    assert!(
        !state.viewport.show_collisions,
        "the row must not go on at the click"
    );
    state.simulation.submitted_collision_epoch = Some(state.session.simulation_epoch());
    assert_eq!(panel::settle_pending_show(&mut state), ShowSettle::Waiting);
    assert!(state.overlays.pending_show.as_ref().unwrap().seen_computing);
    land_collision_check(&mut state, 2);
    assert_eq!(panel::settle_pending_show(&mut state), ShowSettle::Applied);
    assert!(state.viewport.show_collisions);

    // A run that ends with no data ends the request.
    let mut state = fresh_state();
    let mut events = Vec::new();
    panel::request_compute(
        &mut state,
        &mut events,
        row("collisions"),
        ComputeOutcome::ComputeAndShow,
    );
    state.simulation.submitted_collision_epoch = Some(0);
    panel::settle_pending_show(&mut state);
    state.simulation.submitted_collision_epoch = None;
    assert_eq!(panel::settle_pending_show(&mut state), ShowSettle::Dropped);
    assert!(!state.viewport.show_collisions);
}

/// Q2 of MOCKUPS §12. `AutoEnableRestAnalysis` drops the toolpath result
/// only, so it never clears the simulation; the plan that the generation
/// starts may run it again. The confirm says "may".
#[test]
fn the_rest_confirm_says_may_g_vpstate() {
    assert!(panel::REST_CONFIRM_CONSEQUENCE.contains(" may "));
    assert!(!panel::REST_CONFIRM_CONSEQUENCE.contains(" will "));
    let config = source("../rs_cam_core/src/session/mutation/config.rs");
    let start = config
        .find("fn auto_enable_rest_analysis_for_source(")
        .expect("the rest setter moved");
    let body: String = config[start..]
        .lines()
        .take(26)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        body.contains("session.drop_result(idx)") && !body.contains("drop_simulation"),
        "the rest setter now clears the simulation; the confirm must say `will`:\n{body}"
    );
}

// ── (d) every active encoding has a real legend ──────────────────────

fn legend_fixture() -> AppState {
    let mut state = fresh_state();
    add_selected(&mut state, true, true);
    registry::set_overlay(&mut state, row("rest_heatmap"), true);
    land_collision_check(&mut state, 3);
    registry::set_overlay(&mut state, row("collisions"), true);
    state
}

#[test]
fn every_active_encoding_has_a_real_legend_g_vpstate() {
    let mut keys = std::collections::BTreeSet::new();
    let palette = legend_fixture();
    let mut engagement = legend_fixture();
    registry::set_overlay(&mut engagement, row("move_colour_engagement"), true);
    assert_eq!(
        engagement.viewport.toolpath_color_mode,
        ToolpathColorMode::Engagement
    );

    for state in [&palette, &engagement] {
        let lines = legend_rail::active_lines(state);
        assert!(
            lines.len() >= 5,
            "the fixture draws only {} rail lines: {lines:?}",
            lines.len()
        );
        for line in &lines {
            assert!(
                line.is_encoding(),
                "a status line in a fixture that draws: {line:?}"
            );
            let name = legend_rail::line_name(state, line);
            assert!(
                name.contains('\u{00B7}'),
                "the legend `{name}` names no target"
            );
            if let RailLine::Categories(kind) = line {
                let entries = legend_rail::category_entries(state, *kind);
                assert!(
                    !entries.is_empty() && entries.iter().all(|(label, _)| !label.is_empty()),
                    "the {name} legend has no categories"
                );
            }
            keys.insert(line.key());
        }
    }
    assert!(
        keys.len() >= 7,
        "non-vacuity: only {} distinct encodings across the two fixtures: {keys:?}",
        keys.len()
    );
    for key in [
        "Toolpaths",
        "Moves",
        "Entry markers",
        "Height planes",
        "Collisions",
    ] {
        assert!(keys.contains(key), "the {key} legend is missing: {keys:?}");
    }
    let rail = source("src/ui/overlays/legend_rail.rs");
    assert!(
        !rail.contains("NameOnly"),
        "the name-only legend kind is back"
    );
}

/// A data-less mode that paints a substitute colour gets a status line,
/// never a scale legend (inventory §2.3, items 3 and 4).
#[test]
fn a_substitute_colour_is_named_on_the_rail_g_vpstate() {
    let mut state = legend_fixture();
    registry::set_overlay(&mut state, row("move_colour_advance_per_tooth"), true);
    // The precondition refuses the switch without a simulation; the mode
    // can still stay after a simulation clears.
    state.viewport.toolpath_color_mode = ToolpathColorMode::AdvancePerTooth;
    let lines = legend_rail::active_lines(&state);
    assert!(
        lines
            .iter()
            .any(|line| matches!(line, RailLine::Status(s) if s.text.contains("grey"))),
        "advance per tooth without a simulation must say the moves draw grey: {lines:?}"
    );
    assert!(
        !lines
            .iter()
            .any(|line| line.key() == "move_colour_advance_per_tooth"),
        "a scale legend names colours that are not on screen"
    );
}

/// The legend's red end is the value at which the mesh reaches red.
#[test]
fn the_rest_legend_top_is_the_mesh_top_g_vpstate() {
    let grid = rest_grid();
    let top = registry::rest_ramp_top(&grid);
    assert!(
        top > grid.threshold as f32 && top < 10.0,
        "non-vacuity: the top {top} must sit under the outlier"
    );
    let mesh = rest_grid_to_heatmap_mesh(&grid).expect("the grid forms a mesh");
    let threshold = grid.threshold as f32;
    for r in 0..grid.grid.ny {
        for c in 0..grid.grid.nx {
            let i = grid.grid.index_of(r, c);
            let valid = grid.rest[i].is_finite() && grid.surface_z[i].is_finite();
            let rest = if valid { grid.rest[i] } else { f32::NAN };
            let want = rest_ramp_color(rest, threshold, top);
            let k = 3 * (r * grid.grid.nx + c);
            assert_eq!(
                &mesh.colors[k..k + 3],
                &want[..],
                "cell ({r}, {c}): the legend top {top} does not reproduce the mesh colour"
            );
        }
    }
}

/// The mirrored colours are the render literals.
#[test]
fn the_mirrored_colours_match_the_render_literals_g_vpstate() {
    let render = source("src/render/toolpath_render.rs");
    for colour in [
        mirrored::SPAN_ENTRY_RGB,
        mirrored::SPAN_LEAD_OUT_RGB,
        mirrored::SPAN_LINK_BRIDGE_RGB,
    ] {
        let literal = format!(
            "brighten([{:.2}, {:.2}, {:.2}])",
            colour[0], colour[1], colour[2]
        );
        assert!(
            render.contains(&literal),
            "toolpath_render.rs no longer holds {literal}; update legend_rail::mirrored"
        );
    }
    let rapid = format!("base[0] * {:.2}", mirrored::RAPID_FACTOR);
    assert!(render.contains(&rapid), "the rapid factor moved: {rapid}");
    assert!(
        render.contains("0.5 * (base[0] + 0.5)"),
        "the dressup tint moved"
    );

    let upload = source("src/app/gpu_upload.rs");
    assert!(
        upload.contains("[0.95, 0.8 * (1.0 - t) + 0.1, 0.1 * (1.0 - t)]"),
        "the collision density ramp moved; update legend_rail::mirrored"
    );
    let isolated = mirrored::collision_density_rgb(0.0);
    let clustered = mirrored::collision_density_rgb(1.0);
    assert!(
        (isolated[1] - 0.9).abs() < 1e-5 && (clustered[1] - 0.1).abs() < 1e-5,
        "the ramp ends moved: {isolated:?} … {clustered:?}"
    );
    assert!(
        legend_rail::entry_preview_rgb().is_some(),
        "the entry preview builder drew nothing for a plain ramp"
    );
}

// ── (e) the 320 pt budget with six legends ───────────────────────────

fn ctx(width: f32, height: f32) -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(raw_input(width, height, Vec::new()), |_ui| {});
    warmup.textures_delta.clear();
    ctx
}

fn raw_input(width: f32, height: f32, events: Vec<egui::Event>) -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(width, height),
        )),
        events,
        ..Default::default()
    }
}

fn idle_lanes() -> [LaneSnapshot; 5] {
    [
        LaneSnapshot::idle(ComputeLane::Toolpath),
        LaneSnapshot::idle(ComputeLane::Analysis),
        LaneSnapshot::idle(ComputeLane::Optimize),
        LaneSnapshot::idle(ComputeLane::Job),
        LaneSnapshot::idle(ComputeLane::Reach),
    ]
}

/// One frame of the dock with `events` as input. Returns the dock outcome.
fn frame(
    ctx: &egui::Context,
    state: &mut AppState,
    viewport: egui::Rect,
    events: Vec<egui::Event>,
) -> viewport_overlay::DockOutcome {
    let lanes = idle_lanes();
    let mut app_events: Vec<AppEvent> = Vec::new();
    let mut outcome = viewport_overlay::DockOutcome::default();
    let mut output = ctx.run_ui(
        raw_input(viewport.width(), viewport.height(), events),
        |ui| {
            outcome = viewport_overlay::draw(
                ui,
                state,
                ProjectionMode::Perspective,
                &lanes,
                &mut app_events,
                viewport,
            );
        },
    );
    output.textures_delta.clear();
    outcome
}

fn area(ctx: &egui::Context, id: &str) -> Option<egui::Rect> {
    ctx.memory(|m| m.area_rect(egui::Id::new(id)))
}

#[test]
fn the_dock_keeps_its_height_budget_at_320_by_600_g_vpstate() {
    let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(320.0, 600.0));
    for section in [None]
        .into_iter()
        .chain(DockSection::ALL.into_iter().map(Some))
    {
        let mut state = legend_fixture();
        state.overlays.open_section = section;
        let ctx = ctx(viewport.width(), viewport.height());
        for _ in 0..3 {
            frame(&ctx, &mut state, viewport, Vec::new());
        }
        let dock = area(&ctx, DOCK_AREA_ID).expect("the dock never drew");
        let budget = viewport_overlay::DOCK_STACK_MAX_HEIGHT_FRACTION * viewport.height();
        assert!(
            dock.height() <= budget,
            "measured: the target and dock stack is {:.1} pt high at 320 × 600 with six legends on; the budget is {budget:.1} pt",
            dock.height()
        );
        assert!(
            dock.width() <= viewport.width() - 2.0 * viewport_overlay::DOCK_GUTTER + 0.5,
            "measured: the dock is {:.1} pt wide; 288 pt is available at 320 pt",
            dock.width()
        );
        if let Some(section) = section {
            let popover = area(&ctx, POPOVER_AREA_ID)
                .unwrap_or_else(|| panic!("the {section:?} popover never drew"));
            assert!(
                viewport.contains_rect(popover.shrink(0.5)),
                "measured: the {section:?} popover {popover:?} overflows the 320 × 600 viewport"
            );
        }
    }
}

/// The short-viewport rule: a popover that does not fit scrolls inside.
#[test]
fn a_popover_scrolls_inside_a_short_viewport_g_vpstate() {
    let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(320.0, 300.0));
    for section in DockSection::ALL {
        let mut state = legend_fixture();
        state.overlays.open_section = Some(section);
        let ctx = ctx(viewport.width(), viewport.height());
        for _ in 0..3 {
            frame(&ctx, &mut state, viewport, Vec::new());
        }
        let popover = area(&ctx, POPOVER_AREA_ID)
            .unwrap_or_else(|| panic!("the {section:?} popover never drew"));
        assert!(
            popover.top() >= viewport.top() - 0.5 && popover.bottom() <= viewport.bottom() + 0.5,
            "measured: the {section:?} popover {popover:?} leaves a 320 × 300 viewport"
        );
    }
}

// ── (f) interaction ──────────────────────────────────────────────────

fn tab() -> egui::Event {
    egui::Event::Key {
        key: egui::Key::Tab,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn tab_walks_the_sections_in_order_then_the_popover_g_vpstate() {
    let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 700.0));
    let mut state = fresh_state();
    state.overlays.open_section = Some(DockSection::Inspect);
    let ctx = ctx(viewport.width(), viewport.height());
    for _ in 0..3 {
        frame(&ctx, &mut state, viewport, Vec::new());
    }
    let dock = area(&ctx, DOCK_AREA_ID).expect("the dock never drew");
    let popover = area(&ctx, POPOVER_AREA_ID).expect("the popover never drew");

    let mut dock_lefts = Vec::new();
    let mut reached_popover = false;
    for _ in 0..12 {
        frame(&ctx, &mut state, viewport, vec![tab()]);
        let Some(id) = ctx.memory(|m| m.focused()) else {
            continue;
        };
        let Some(response) = ctx.read_response(id) else {
            continue;
        };
        let centre = response.rect.center();
        if popover.contains(centre) {
            reached_popover = true;
            break;
        }
        if dock.contains(centre) {
            dock_lefts.push(response.rect.left());
        }
    }
    assert!(
        dock_lefts.len() >= 4,
        "Tab reached only {} dock buttons before the popover",
        dock_lefts.len()
    );
    assert!(
        dock_lefts.windows(2).all(|w| w[0] < w[1]),
        "Tab must walk the sections left to right: {dock_lefts:?}"
    );
    assert!(reached_popover, "Tab never moved into the open popover");
}

#[test]
fn an_outside_click_closes_the_popover_and_selects_nothing_g_vpstate() {
    let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
    let mut state = fresh_state();
    state.overlays.open_section = Some(DockSection::Scene);
    let ctx = ctx(viewport.width(), viewport.height());
    for _ in 0..3 {
        frame(&ctx, &mut state, viewport, Vec::new());
    }
    let outside = egui::pos2(20.0, 20.0);
    let dock = area(&ctx, DOCK_AREA_ID).unwrap();
    let popover = area(&ctx, POPOVER_AREA_ID).unwrap();
    assert!(!dock.contains(outside) && !popover.contains(outside));
    // The viewport receives input outside the dock: no dock layer is there.
    let layer = ctx.layer_id_at(outside).map(|layer| layer.id);
    assert!(
        layer != Some(egui::Id::new(DOCK_AREA_ID)) && layer != Some(egui::Id::new(POPOVER_AREA_ID)),
        "a dock layer covers the viewport at {outside:?}"
    );

    let press = |pressed| egui::Event::PointerButton {
        pos: outside,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(
        &ctx,
        &mut state,
        viewport,
        vec![egui::Event::PointerMoved(outside), press(true)],
    );
    let outcome = frame(&ctx, &mut state, viewport, vec![press(false)]);
    assert_eq!(state.overlays.open_section, None, "the popover stayed open");
    assert!(
        outcome.closed_popover_on_click,
        "the closing click must not also select in the viewport (MOCKUPS Q1)"
    );
}

#[test]
fn escape_closes_one_surface_at_a_time_g_vpstate() {
    let mut overlays = OverlayPanelState::new();
    overlays.open = true;
    overlays.open_section = Some(DockSection::Inspect);
    overlays.rest_confirm = Some(ToolpathId(3));
    assert!(overlays.takes_escape_first());
    assert!(overlays.close_one_for_escape());
    assert_eq!(overlays.rest_confirm, None, "the confirm closes first");
    assert!(overlays.open_section.is_some());
    assert!(overlays.close_one_for_escape());
    assert_eq!(overlays.open_section, None, "then the popover");
    assert!(!overlays.takes_escape_first());
    assert!(overlays.close_one_for_escape());
    assert!(!overlays.open, "then the catalogue");
    assert!(
        !overlays.close_one_for_escape(),
        "with nothing open, Escape keeps its workspace meaning (Simulation: back to Toolpaths)"
    );
}

// ── (g) the compact legend ───────────────────────────────────────────

/// One frame of the dock and the legend. Returns every text the frame
/// painted, with its rect.
fn painted_frame(
    ctx: &egui::Context,
    state: &mut AppState,
    viewport: egui::Rect,
    events: Vec<egui::Event>,
) -> Vec<(String, egui::Rect)> {
    let lanes = idle_lanes();
    let mut app_events: Vec<AppEvent> = Vec::new();
    let mut output = ctx.run_ui(
        raw_input(viewport.width(), viewport.height(), events),
        |ui| {
            viewport_overlay::draw(
                ui,
                state,
                ProjectionMode::Perspective,
                &lanes,
                &mut app_events,
                viewport,
            );
        },
    );
    output.textures_delta.clear();
    fn walk(shape: &egui::Shape, out: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Text(text) => {
                let rect = text.galley.rect.translate(text.pos.to_vec2());
                out.push((text.galley.text().to_owned(), rect));
            }
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for clipped in &output.shapes {
        walk(&clipped.shape, &mut out);
    }
    out
}

fn painted_contains(texts: &[(String, egui::Rect)], needle: &str) -> bool {
    texts.iter().any(|(text, _)| text.contains(needle))
}

fn click_at(pos: egui::Pos2) -> [Vec<egui::Event>; 2] {
    let press = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    [
        vec![egui::Event::PointerMoved(pos), press(true)],
        vec![press(false)],
    ]
}

/// The orientation gizmo of `app/viewport.rs`: a 50 pt disc 10 pt in from
/// the top-right corner.
fn gizmo_rect(viewport: egui::Rect) -> egui::Rect {
    egui::Rect::from_min_size(
        egui::pos2(viewport.right() - 60.0, viewport.top()),
        egui::vec2(60.0, 60.0),
    )
}

/// The legend never intersects the dock bar or the gizmo. The sizes cover
/// the operator's windows (1400 × 900, 1920 × 1165) as an upper bound, the
/// 3D view inside them once the side panels take their width, a 1100 pt
/// window, and the 320 pt floor of the dock sentry.
#[test]
fn the_legend_never_covers_the_dock_or_the_gizmo_g_legend() {
    let sizes = [
        (1920.0, 1165.0),
        (1400.0, 900.0),
        (1100.0, 800.0),
        (1340.0, 1000.0),
        (860.0, 760.0),
        (600.0, 700.0),
        (480.0, 600.0),
        (320.0, 600.0),
        (320.0, 300.0),
    ];
    let mut beside = 0;
    for (width, height) in sizes {
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
        for collapsed in [false, true] {
            let mut state = legend_fixture();
            state.overlays.legend_collapsed = collapsed;
            let ctx = ctx(width, height);
            for _ in 0..3 {
                frame(&ctx, &mut state, viewport, Vec::new());
            }
            let dock = area(&ctx, DOCK_AREA_ID).expect("the dock never drew");
            let legend = area(&ctx, legend_rail::LEGEND_AREA_ID).expect("the legend never drew");
            let case = format!("{width} × {height}, collapsed {collapsed}");
            assert!(
                !legend.intersects(dock),
                "measured at {case}: the legend {legend:?} covers the dock {dock:?}"
            );
            assert!(
                !legend.intersects(gizmo_rect(viewport)),
                "measured at {case}: the legend {legend:?} covers the gizmo"
            );
            assert!(
                viewport.contains_rect(legend.shrink(0.5)),
                "measured at {case}: the legend {legend:?} leaves the viewport"
            );
            assert!(
                legend.right() >= viewport.right() - viewport_overlay::DOCK_GUTTER - 0.5,
                "measured at {case}: the legend {legend:?} is not on the right edge"
            );
            assert!(
                legend.height() <= legend_rail::LEGEND_MAX_HEIGHT_FRACTION * height + 1.0
                    || legend.height() <= tokens::ROW_ACTION + 4.0 * tokens::SPACE_2,
                "measured at {case}: the legend is {:.1} pt high, over a quarter of the viewport",
                legend.height()
            );
            if legend.bottom() > dock.top() {
                beside += 1;
            }
            if width >= 860.0 && !collapsed {
                assert!(
                    legend.bottom() > dock.top(),
                    "at {case} the legend must sit beside the dock, on the viewport bottom"
                );
            }
        }
    }
    assert!(
        beside >= 8,
        "non-vacuity: the legend sat beside the dock only {beside} times"
    );
}

/// The chip text is short and the caveats paint only on hover.
#[test]
fn hover_reveals_the_long_text_and_only_hover_g_legend() {
    let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0));
    let mut state = legend_fixture();
    let lines = legend_rail::active_lines(&state);
    let mut caveat_count = 0;
    for line in &lines {
        let chip = legend_rail::chip_text(&state, line);
        for (caveat, _) in legend_rail::caveat_lines(&state, line) {
            caveat_count += 1;
            assert!(
                !chip.contains(&caveat),
                "the chip `{chip}` carries its caveat `{caveat}`"
            );
        }
    }
    assert!(
        caveat_count >= 4,
        "non-vacuity: only {caveat_count} caveat lines"
    );

    let heights = lines
        .iter()
        .find(|line| {
            matches!(
                line,
                RailLine::Categories(legend_rail::Categories::HeightPlanes)
            )
        })
        .expect("the fixture draws height planes");
    let name = legend_rail::line_name(&state, heights);
    let caveat = "the Heights tab of the inspector gives each Z in mm";

    let ctx = ctx(viewport.width(), viewport.height());
    let mut texts = Vec::new();
    for _ in 0..3 {
        texts = painted_frame(&ctx, &mut state, viewport, Vec::new());
    }
    assert!(
        painted_contains(&texts, &name),
        "the chip `{name}` never painted"
    );
    assert!(
        !painted_contains(&texts, caveat),
        "a caveat painted without a hover"
    );
    assert!(area(&ctx, legend_rail::LEGEND_DETAIL_AREA_ID).is_none());

    let chip = ctx
        .read_response(legend_rail::chip_id(&name))
        .expect("the chip has a response")
        .rect;
    for _ in 0..2 {
        texts = painted_frame(
            &ctx,
            &mut state,
            viewport,
            vec![egui::Event::PointerMoved(chip.center())],
        );
    }
    assert!(
        painted_contains(&texts, caveat),
        "the hover on `{name}` did not paint its caveat"
    );
    let detail = area(&ctx, legend_rail::LEGEND_DETAIL_AREA_ID).expect("no detail area");
    assert!(
        viewport.contains_rect(detail.shrink(0.5)),
        "the detail {detail:?} leaves the viewport"
    );
    assert!(
        !detail.contains(chip.center()),
        "the detail {detail:?} lies under the pointer"
    );
    // The hover must hold from frame to frame: the detail takes no input.
    texts = painted_frame(&ctx, &mut state, viewport, Vec::new());
    assert!(painted_contains(&texts, caveat), "the detail flickered off");
}

/// Folded, the legend is one chip; the chip and the fold glyph switch it.
#[test]
fn a_folded_legend_is_one_chip_g_legend() {
    let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0));
    let mut state = legend_fixture();
    let lines = legend_rail::active_lines(&state);
    let names: Vec<String> = lines
        .iter()
        .map(|line| legend_rail::line_name(&state, line))
        .collect();
    let folded = format!("{} ({})", legend_rail::COLLAPSED_WORD, lines.len());
    state.overlays.legend_collapsed = true;

    let ctx = ctx(viewport.width(), viewport.height());
    let mut texts = Vec::new();
    for _ in 0..3 {
        texts = painted_frame(&ctx, &mut state, viewport, Vec::new());
    }
    assert!(
        painted_contains(&texts, &folded),
        "the `{folded}` chip never painted"
    );
    for name in &names {
        assert!(
            !painted_contains(&texts, name),
            "the folded legend painted the chip `{name}`"
        );
    }
    let legend = area(&ctx, legend_rail::LEGEND_AREA_ID).expect("the legend never drew");
    assert!(
        legend.height() <= tokens::ROW_ACTION + 2.0 * tokens::SPACE_2 + 2.0,
        "the folded legend is {:.1} pt high: more than one chip",
        legend.height()
    );

    for events in click_at(legend.center()) {
        painted_frame(&ctx, &mut state, viewport, events);
    }
    assert!(
        !state.overlays.legend_collapsed,
        "a click on the chip did not unfold it"
    );
    // The area places itself from the size of the frame before, so the
    // open legend settles over a few frames.
    for _ in 0..4 {
        texts = painted_frame(&ctx, &mut state, viewport, Vec::new());
    }
    for name in &names {
        assert!(
            painted_contains(&texts, name),
            "the open legend lost `{name}`"
        );
    }
    let fold = texts
        .iter()
        .find(|(text, _)| text == legend_rail::FOLD_GLYPH)
        .map(|(_, rect)| *rect)
        .expect("the fold glyph never painted");
    for events in click_at(fold.center()) {
        painted_frame(&ctx, &mut state, viewport, events);
    }
    assert!(
        state.overlays.legend_collapsed,
        "the fold glyph did not fold the legend"
    );
}

/// The legend reaches its final rect within three frames of a growth,
/// holds it, and clips no chip. An area gives its content the rect of the frame before; a scroll
/// area inside it that reads that rect grows by a few points per frame, and
/// the old rail crept up the viewport that way.
#[test]
fn the_legend_settles_without_a_slide_g_legend() {
    for (width, height) in [(1400.0, 900.0), (480.0, 600.0)] {
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
        // The legend GROWS here, from one folded chip to every chip: the
        // creep shows only on growth.
        let mut state = legend_fixture();
        state.overlays.legend_collapsed = true;
        let ctx = ctx(width, height);
        for _ in 0..3 {
            frame(&ctx, &mut state, viewport, Vec::new());
        }
        state.overlays.legend_collapsed = false;
        for _ in 0..3 {
            frame(&ctx, &mut state, viewport, Vec::new());
        }
        let early = area(&ctx, legend_rail::LEGEND_AREA_ID).expect("the legend never drew");
        for _ in 0..20 {
            frame(&ctx, &mut state, viewport, Vec::new());
        }
        let late = area(&ctx, legend_rail::LEGEND_AREA_ID).expect("the legend never drew");
        assert!(
            (early.min - late.min).length() < 0.5 && (early.max - late.max).length() < 0.5,
            "measured at {width} × {height}: the legend moved from {early:?} after three frames to {late:?} after 23"
        );
        // Stable is not enough: a scroll area held to the old rect is
        // stable and clips. Every chip must lie inside the legend.
        for line in legend_rail::active_lines(&state) {
            let name = legend_rail::line_name(&state, &line);
            let chip = ctx
                .read_response(legend_rail::chip_id(&name))
                .unwrap_or_else(|| panic!("the chip `{name}` has no response"))
                .rect;
            assert!(
                late.contains_rect(chip),
                "measured at {width} × {height}: the chip `{name}` {chip:?} lies outside the legend {late:?}"
            );
        }
        assert!(
            early.height() > tokens::ROW_DENSE * 2.0,
            "non-vacuity: the open legend is only {:.1} pt high",
            early.height()
        );
    }
}

/// The reach key shows the WHOLE reach colour key (operator, 2026-10-02:
/// "it does not have green on the scale which is the 'can reach' state"):
/// the reached green, the miss ramp, the unresolved grey and the
/// not-measured colour, each from `reach_color`, in the hover key and in
/// the chip swatch.
#[test]
fn the_reach_key_shows_reached_miss_and_unresolved_g_legend() {
    use rs_cam_core::maps::reach_map::{ReachRamp, reach_color};
    let state = legend_fixture();
    let ramp = ReachRamp {
        tolerance_mm: 0.1,
        floor_mm: 0.567,
        max_gap_mm: 5.05,
    };
    let line = RailLine::Scale(registry::Legend::Reach(ramp));
    let key = legend_rail::key_colours(&state, &line);
    let swatch = legend_rail::swatch_colours(&state, &line);

    let reached = reach_color(0.05, f32::NAN, ramp);
    assert!(
        reached[1] > reached[0] && reached[1] > reached[2],
        "non-vacuity: the reached colour {reached:?} is not green"
    );
    let unresolved = reach_color(0.3, 0.3, ramp);
    let not_measured = reach_color(f32::NAN, f32::NAN, ramp);
    let shallow_miss = reach_color(0.6, f32::NAN, ramp);
    let deep_miss = reach_color(5.05, f32::NAN, ramp);
    assert_ne!(unresolved, not_measured, "non-vacuity: two greys");

    let green = |c: &[f32; 3]| c[1] > c[0] && c[1] > c[2];
    assert!(key.iter().any(green), "the reach key has no green: {key:?}");
    assert!(
        key.contains(&reached),
        "the reach key lacks the reached colour {reached:?}"
    );
    for (what, colour) in [("unresolved", unresolved), ("not measured", not_measured)] {
        assert!(
            key.contains(&colour),
            "the reach key lacks the {what} colour {colour:?}: {key:?}"
        );
        assert!(
            swatch.contains(&tokens::from_linear_rgb(colour)),
            "the reach chip swatch lacks the {what} colour"
        );
    }
    assert!(
        swatch.contains(&tokens::from_linear_rgb(reached)),
        "the reach chip swatch lacks the reached green"
    );
    // The miss ramp: yellow at the bar end, dark red at the deep end.
    let near = |a: &[f32; 3], b: &[f32; 3]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 0.08);
    assert!(
        key.iter().any(|c| near(c, &shallow_miss)) && key.iter().any(|c| near(c, &deep_miss)),
        "the reach key lacks its miss ramp ends {shallow_miss:?} … {deep_miss:?}"
    );
    // Order: green first, the greys last, so the swatch reads left to right
    // from reached to deeper misses.
    assert_eq!(key.first(), Some(&reached));
    assert_eq!(key.last(), Some(&not_measured));
}

/// Every chip swatch paints every colour its overlay draws.
#[test]
fn every_chip_swatch_shows_every_colour_of_its_key_g_legend() {
    let state = legend_fixture();
    let lines = legend_rail::active_lines(&state);
    let mut checked = 0;
    for line in &lines {
        let name = legend_rail::line_name(&state, line);
        let swatch = legend_rail::swatch_colours(&state, line);
        for colour in legend_rail::key_colours(&state, line) {
            assert!(
                swatch.contains(&tokens::from_linear_rgb(colour)),
                "the `{name}` chip swatch lacks {colour:?}"
            );
        }
        if let RailLine::Categories(kind) = line {
            let entries = legend_rail::category_entries(&state, *kind);
            for (label, colour) in entries {
                if let Some(colour) = colour {
                    assert!(
                        swatch.contains(&tokens::from_linear_rgb(colour)),
                        "the `{name}` chip swatch lacks the `{label}` colour"
                    );
                }
            }
        }
        checked += 1;
    }
    assert!(checked >= 5, "non-vacuity: only {checked} lines");
    let heights = lines
        .iter()
        .find(|line| {
            matches!(
                line,
                RailLine::Categories(legend_rail::Categories::HeightPlanes)
            )
        })
        .expect("the fixture draws height planes");
    assert_eq!(
        legend_rail::swatch_colours(&state, heights).len(),
        5,
        "the height-plane chip must show all five planes"
    );
}
