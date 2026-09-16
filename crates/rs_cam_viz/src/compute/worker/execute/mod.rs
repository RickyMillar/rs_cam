use super::helpers::{
    build_simulation_cut_artifact, build_trace_artifact, debug_artifact_dir,
    simulation_metric_artifact_dir,
};
#[cfg(test)]
use super::test_fixture::{RequestSpec, board, no_dressups, request, retract_z, stock_bbox};
use super::{
    Arc, AtomicBool, ComputeError, ComputeRequest, SimulationRequest, SimulationResult,
    ToolpathPhaseTracker, ToolpathResult,
};
#[cfg(test)]
use crate::state::job::{ToolConfig, ToolId, ToolType};
#[cfg(test)]
use crate::state::toolpath::{
    FaceConfig, FaceDirection, InlayConfig, OperationConfig, ProfileConfig, RestConfig,
    ZigzagConfig,
};
use rs_cam_core::compute::build_cutter;
#[cfg(test)]
use rs_cam_core::polygon::Polygon2;
#[cfg(test)]
use rs_cam_core::toolpath::MoveType;

/// How many simulation cut artifacts to retain in `target/simulation_metrics`
/// (G-SIMDUMP: each dump can be multiple GB at fine resolutions).
const SIM_CUT_ARTIFACT_RETAIN: usize = 5;

pub(super) struct ComputeExecutionOutcome {
    pub result: Result<ToolpathResult, ComputeError>,
    pub debug_trace: Option<Arc<rs_cam_core::debug_trace::ToolpathDebugTrace>>,
    pub semantic_trace: Option<Arc<rs_cam_core::semantic_trace::ToolpathSemanticTrace>>,
    pub debug_trace_path: Option<std::path::PathBuf>,
}

/// Convert viz `SimulationRequest` into a core `SimulationRequest` so the
/// actual simulation can be delegated to `rs_cam_core::compute::simulate`.
fn build_core_simulation_request(
    req: &SimulationRequest,
) -> rs_cam_core::compute::simulate::SimulationRequest {
    use rs_cam_core::compute::simulate::{SimGroupEntry, SimToolpathEntry};

    let groups = req
        .groups
        .iter()
        .map(|group| {
            // F-024 follow-up (2026-05-25): mirror the
            // `session::compute::compute_simulation_groups` decision shape so
            // the viz worker's production simulation path matches the core
            // `ProjectSession::run_simulation` path. For identity setups
            // (`face_up=Top`, `z_rotation=Deg0`) the viz controller leaves
            // `local_to_global = None` and the toolpath emits cut moves in
            // world frame (Z=[0, -depth], because `HeightsConfig::resolve`
            // auto-defaults `top_z = 0.0`). The per-setup dexel grid must
            // therefore also be in world frame. Forwarding the local
            // zero-rooted `local_stock_bbox` (Z=[0, stock_z]) here placed the
            // cutter at world Z=-2 below every dexel ray and inflated
            // `axial_engagement_mm` to the full stock height — which fed the
            // deflection gate 374-573 µm tip-deflection readings on AS001-
            // shape pockets that should land well under 50 µm.
            //
            // Fix: when `local_to_global` is `None` (identity setup), pass
            // `local_stock_bbox = None` too. `run_simulation` then falls back
            // to `request.stock_bbox` (world frame) for the per-setup grid,
            // matching the toolpath frame.
            //
            // Non-identity setups continue to forward the viz-side
            // zero-rooted `local_stock_bbox` paired with `local_to_global` —
            // that path is outside F-024's scope (see core finding).
            let (local_stock_bbox, local_to_global) =
                if let Some(info) = group.local_to_global.as_ref() {
                    (Some(group.local_stock_bbox), Some(info.clone()))
                } else {
                    (None, None)
                };

            SimGroupEntry {
                toolpaths: group
                    .toolpaths
                    .iter()
                    .map(|tp| {
                        let cutter = build_cutter(&tp.tool);
                        SimToolpathEntry {
                            id: tp.id,
                            name: tp.name.clone(),
                            annotated: Arc::clone(&tp.annotated),
                            tool: cutter,
                            flute_count: tp.tool.flute_count,
                            tool_summary: tp.tool.summary(),
                            semantic_trace: tp.semantic_trace.clone(),
                            spindle_rpm: tp.spindle_rpm,
                            metrics_not_applicable: tp.metrics_not_applicable,
                            drill_op: tp.drill_op.clone(),
                            operation_config_hash: tp.operation_config_hash,
                        }
                    })
                    .collect(),
                direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
                local_stock_bbox,
                local_to_global,
                // F.4 — forward the phantom-prior-stock candidate computed
                // by the controller (`build_simulation_groups`) verbatim;
                // the core simulator inserts the phantom snapshot at the
                // recorded group position.
                phantom_prior_stock: group.phantom_prior_stock,
            }
        })
        .collect();

    rs_cam_core::compute::simulate::SimulationRequest {
        groups,
        stock_bbox: req.stock_bbox,
        stock_top_z: req.stock_top_z,
        resolution: req.resolution,
        metric_options: req.metric_options,
        spindle_rpm: req.spindle_rpm,
        rapid_feed_mm_min: req.rapid_feed_mm_min,
        model_mesh: req.model_mesh.clone(),
        // F-034/F-035 — the viz request carries core's own
        // `KinematicsContext`, so this is a forward, not a rebuild.
        // `None` (the default for every shipped preset) keeps the run
        // byte-identical to pre-F-034 / pre-F-035.
        kinematics: req.kinematics,
    }
}

/// Build viz playback data from the viz request (global-frame toolpaths + tool
/// config + cut direction + optional global-frame drill_op). Core does not
/// produce this because it is a viz-only concern (incremental playback in the
/// 3D viewport). The `drill_op`, when present, lets `update_live_sim` apply
/// analytical removal during forward scrub rather than relying on
/// `simulate_toolpath_range`'s degenerate-Z dexel stamping for plunge moves.
fn build_playback_data(req: &SimulationRequest) -> Vec<super::PlaybackToolpath> {
    use super::PlaybackToolpath;
    use rs_cam_core::compute::simulate::{group_drill_op_to_global, group_toolpath_to_global};

    use rs_cam_core::dexel_stock::StockCutDirection;

    // The zero-rooted stock-relative frame the global playback stock lives in.
    let global_bbox = rs_cam_core::geo::BoundingBox3 {
        min: rs_cam_core::geo::P3::new(0.0, 0.0, 0.0),
        max: rs_cam_core::geo::P3::new(
            req.stock_bbox.max.x - req.stock_bbox.min.x,
            req.stock_bbox.max.y - req.stock_bbox.min.y,
            req.stock_bbox.max.z - req.stock_bbox.min.z,
        ),
    };

    let mut playback = Vec::new();
    for (group_ordinal, group) in req.groups.iter().enumerate() {
        let playback_direction = group
            .local_to_global
            .as_ref()
            .map_or(StockCutDirection::FromTop, |info| info.cut_direction());
        // G-LATERALSCRUB. A group whose tool axis is a global X or Y dexel
        // axis is replayed in its OWN frame instead: the global stock turns
        // only its Z grid into a closed solid, so a lateral stamp there
        // removes nothing an operator can see. The core simulator makes the
        // matching call — same predicate, same consequence — and publishes
        // that group's checkpoints with the local stock.
        let lateral = !matches!(
            playback_direction,
            StockCutDirection::FromTop | StockCutDirection::FromBottom
        );

        for tp in &group.toolpaths {
            let entry = if lateral {
                PlaybackToolpath {
                    // Setup-local coordinates verbatim — which is what the
                    // group's toolpaths already are — stamped down local Z,
                    // exactly as `compute/simulate.rs` stamps `group_stock`.
                    toolpath: Arc::new(tp.annotated.toolpath.clone()),
                    tool: tp.tool.clone(),
                    direction: StockCutDirection::FromTop,
                    drill_op: tp.drill_op.clone(),
                    group: group_ordinal,
                    frame: group.local_to_global.clone(),
                    stock_bbox: group.local_stock_bbox,
                }
            } else {
                // Frame-map through the SAME core helpers the global stock is
                // stamped with, so live playback and the checkpoint stocks
                // can't drift apart. Identity groups emit in world frame and
                // are shifted by `-stock_bbox.min`; carrying a private copy
                // of this mapping is how G-SIM-IDENTITY-FRAME got a second
                // home.
                PlaybackToolpath {
                    toolpath: Arc::new(group_toolpath_to_global(
                        &tp.annotated.toolpath,
                        &group.local_to_global,
                        req.stock_bbox.min,
                    )),
                    tool: tp.tool.clone(),
                    direction: playback_direction,
                    drill_op: tp.drill_op.as_ref().map(|drill_op_arc| {
                        Arc::new(group_drill_op_to_global(
                            drill_op_arc,
                            &group.local_to_global,
                            req.stock_bbox.min,
                        ))
                    }),
                    group: group_ordinal,
                    frame: None,
                    stock_bbox: global_bbox,
                }
            };
            playback.push(entry);
        }
    }
    playback
}

pub(crate) fn run_simulation_with_phase<F>(
    req: &SimulationRequest,
    cancel: &AtomicBool,
    set_phase: F,
    memo: Option<rs_cam_core::compute::sim_prefix::SimMemo<'_>>,
) -> Result<SimulationResult, ComputeError>
where
    F: FnMut(&str),
{
    use rs_cam_core::compute::simulate;

    // Convert viz request to core request and delegate. `memo` carries the
    // analysis lane's S5 prefix cache; `None` is the pre-S5 behaviour.
    let core_req = build_core_simulation_request(req);
    let core_result = simulate::run_simulation_memoized(&core_req, cancel, set_phase, memo)
        .map_err(|_cancelled| ComputeError::Cancelled)?;

    // Build viz-only playback data (global-frame toolpaths for viewport replay).
    let playback_data = build_playback_data(req);

    // Write cut-trace artifact to disk (viz-only filesystem concern).
    let cut_trace_path = if let Some(trace) = core_result.cut_trace.as_ref() {
        let artifact = build_simulation_cut_artifact(req, (**trace).clone());
        match rs_cam_core::simulation_cut::write_simulation_cut_artifact(
            &simulation_metric_artifact_dir(),
            "simulation_metrics",
            &artifact,
        ) {
            Ok(p) => {
                // G-SIMDUMP: unbounded dumps filled the disk (96 GB observed);
                // keep only the newest few — each can be multiple GB.
                let pruned = rs_cam_core::simulation_cut::prune_simulation_cut_artifacts(
                    &simulation_metric_artifact_dir(),
                    SIM_CUT_ARTIFACT_RETAIN,
                );
                if pruned > 0 {
                    tracing::info!("Pruned {pruned} old simulation cut artifact(s)");
                }
                Some(p)
            }
            Err(error) => {
                tracing::warn!("Failed to write simulation cut artifact: {error}");
                None
            }
        }
    } else {
        None
    };

    Ok(SimulationResult {
        core: core_result,
        playback_data,
        cut_trace_path,
    })
}

/// Run one generation job — the worker thread's whole generation step.
///
/// WP11b: this used to be the second assembly of every generation input
/// (tracker row N12). It is now a thin adapter: the request carries the
/// handle `ProjectSession::start` produced, and this hands that to
/// `rs_cam_core::session::execute_job`. Everything between — the dressups,
/// the boundary clip, the entry-descent split, the empty-generation gate,
/// the stats join and the drill-op view — lives in core and is shared with
/// the CLI.
///
/// What stays viz's: the phase string the lane shows, the trace `Arc`s the
/// drain parks on `toolpath_rt`, and the debug artifact file.
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn run_compute(req: &ComputeRequest) -> ComputeExecutionOutcome {
    run_compute_with_phase_tracker(req, None)
}

pub(super) fn run_compute_with_phase(
    req: &ComputeRequest,
    phase_tracker: &ToolpathPhaseTracker,
) -> ComputeExecutionOutcome {
    run_compute_with_phase_tracker(req, Some(phase_tracker))
}

/// `pub(super)` so the worker's tests can drive it with no phase tracker.
pub(super) fn run_compute_with_phase_tracker(
    req: &ComputeRequest,
    phase_tracker: Option<&ToolpathPhaseTracker>,
) -> ComputeExecutionOutcome {
    let debug_options = req.handle.debug_options();
    let mut observer = rs_cam_core::session::GenObserver::none().with_debug_options(debug_options);
    if let Some(tracker) = phase_tracker {
        observer = observer.with_phase_sink(Arc::new(tracker.clone()));
    }

    let cancel: &AtomicBool = &req.viz.cancel;
    let outcome = rs_cam_core::session::execute_job(&req.handle, &observer, cancel);

    match outcome {
        Ok(computed) => {
            let rs_cam_core::session::ToolpathComputeResult {
                op_data,
                stats,
                debug_trace,
                semantic_trace,
            } = computed;
            let debug_trace = debug_trace.map(Arc::new);
            let semantic_trace = semantic_trace.map(Arc::new);
            // The artifact file, written only when the operator asked for a
            // trace. Core returns both traces on every generation, so the
            // gate is here and not on the recorders.
            let debug_trace_path = debug_options
                .enabled
                .then(|| {
                    write_trace_artifact(req, debug_trace.as_deref(), semantic_trace.as_deref())
                })
                .flatten();
            let annotated = Arc::clone(op_data.annotated());
            let drill_op = op_data.drill_op().map(Arc::clone);
            let result = ToolpathResult {
                annotated,
                stats,
                debug_trace: debug_trace.clone(),
                semantic_trace: semantic_trace.clone(),
                debug_trace_path: debug_trace_path.clone(),
                drill_op,
            };
            ComputeExecutionOutcome {
                result: Ok(result),
                debug_trace,
                semantic_trace,
                debug_trace_path,
            }
        }
        Err(error) => ComputeExecutionOutcome {
            result: Err(map_session_error(&error, cancel)),
            debug_trace: None,
            semantic_trace: None,
            debug_trace_path: None,
        },
    }
}

/// Turn a core refusal into the lane's error.
///
/// `SessionError` carries no cancel variant, so the FLAG is the evidence
/// that a generation stopped rather than failed. A caller that reads the
/// message instead would report "Operation was cancelled" as a failure and
/// the drain would mark the toolpath `Error` — which is what G-REGEN-RACE
/// exists to prevent.
fn map_session_error(
    error: &rs_cam_core::session::SessionError,
    cancel: &AtomicBool,
) -> ComputeError {
    if cancel.load(std::sync::atomic::Ordering::SeqCst) {
        ComputeError::Cancelled
    } else {
        ComputeError::Message(error.to_string())
    }
}

/// Write this generation's trace artifact and answer its path.
///
/// `None` means NOT WRITTEN — the write failed, and the warning says why.
fn write_trace_artifact(
    req: &ComputeRequest,
    debug_trace: Option<&rs_cam_core::debug_trace::ToolpathDebugTrace>,
    semantic_trace: Option<&rs_cam_core::semantic_trace::ToolpathSemanticTrace>,
) -> Option<std::path::PathBuf> {
    let artifact = build_trace_artifact(req, debug_trace.cloned(), semantic_trace.cloned());
    let file_stem = format!("{}-{}", req.viz.toolpath_id.0, req.handle.toolpath_name());
    match rs_cam_core::semantic_trace::write_toolpath_trace_artifact(
        &debug_artifact_dir(),
        &file_stem,
        &artifact,
    ) {
        Ok(path) => Some(path),
        Err(error) => {
            tracing::warn!(
                "Failed to write toolpath debug artifact for {}: {error}",
                req.viz.toolpath_id.0
            );
            None
        }
    }
}

// Annotation helpers (annotate_operation_scope, annotate_adaptive3d_runtime_semantics,
// etc.) were removed along with their only callers — the SemanticToolpathOp impls.
// Dispatch now goes through rs_cam_core::session::execute_job.

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    //! These tests used to drive `generate_via_core`, a viz-side assembly
    //! of the generation inputs (tracker row N12). WP11b deleted it, so
    //! each one now describes a fixture and runs the production door. Two
    //! consequences to know when reading an assertion:
    //!
    //! * the DRESSUPS run. `RequestSpec::new` turns every one of them off,
    //!   which is the configuration `generate_via_core` gave these tests;
    //!   `DressupConfig::default()` would modulate the feeds and reorder
    //!   the rapids under them.
    //! * the board's TOP face is at `z = 0`, matching the
    //!   `HeightContext::simple` these fixtures used to pass.

    use super::*;
    use crate::compute::worker::Toolpath;

    /// One 40 x 40 mm rectangle, one tool, no dressups.
    fn polygon_spec(operation: OperationConfig, tool_type: ToolType) -> RequestSpec {
        RequestSpec::new(
            1,
            "Test",
            operation,
            ToolConfig::new_default(ToolId(1), tool_type),
        )
    }

    /// Run one fixture through the production door and answer its motion.
    fn generated(spec: RequestSpec) -> (Toolpath, f64) {
        let req = request(spec);
        let retract = retract_z(&req);
        let toolpath = run_compute(&req)
            .result
            .expect("the fixture generates a toolpath")
            .annotated
            .toolpath
            .clone();
        (toolpath, retract)
    }

    fn dominant_cut_axis(tp: &Toolpath, feed_rate: f64) -> Option<char> {
        let mut x_travel = 0.0;
        let mut y_travel = 0.0;

        for idx in 1..tp.moves.len() {
            if let MoveType::Linear {
                feed_rate: move_feed,
            } = tp.moves[idx].move_type
                && (move_feed - feed_rate).abs() < 1e-6
            {
                let dx = tp.moves[idx].target.x - tp.moves[idx - 1].target.x;
                let dy = tp.moves[idx].target.y - tp.moves[idx - 1].target.y;
                if dx.abs().max(dy.abs()) > 1e-3 {
                    x_travel += dx.abs();
                    y_travel += dy.abs();
                }
            }
        }

        if x_travel <= 1e-6 && y_travel <= 1e-6 {
            None
        } else if y_travel > x_travel {
            Some('y')
        } else {
            Some('x')
        }
    }

    // --- Task A3: Tabs only on final depth pass ---

    #[test]
    fn profile_multi_pass_tabs_only_on_final_depth() {
        let cfg = ProfileConfig {
            depth: 6.0,
            depth_per_pass: 2.0,
            tab_count: 4,
            tab_width: 6.0,
            tab_height: 2.0,
            finishing_passes: 0,
            ..ProfileConfig::default()
        };
        let (tp, safe_z) = generated(polygon_spec(
            OperationConfig::Profile(cfg.clone()),
            ToolType::EndMill,
        ));

        let final_z = -cfg.depth;
        let tab_z = final_z + cfg.tab_height;

        // Tab height moves should exist (tabs applied to final pass)
        let tab_moves: Vec<_> = tp
            .moves
            .iter()
            .filter(|m| (m.target.z - tab_z).abs() < 0.01)
            .collect();
        assert!(
            !tab_moves.is_empty(),
            "Final pass should have tab height moves at z={tab_z}"
        );

        // Intermediate passes (at Z != final_z) should have NO tab-height lifts.
        // Roughing levels are at -2, -4; final is at -6. Tab height is -4.
        // Check that the Z=-4 moves are actual cutting moves, not tab lifts:
        // Tab lifts have z = final_z + tab_height = -6 + 2 = -4 which
        // coincides with a roughing level. Instead, verify that there are
        // no tab-height moves between roughing passes (moves with z > final_z
        // that aren't at a legitimate roughing level or safe_z).
        let depth_stepping = rs_cam_core::depth::DepthStepping {
            start_z: 0.0,
            final_z: -cfg.depth.abs(),
            max_step_down: cfg.depth_per_pass,
            distribution: rs_cam_core::depth::DepthDistribution::Even,
            finish_allowance: 0.0,
            finishing_passes: cfg.finishing_passes,
        };
        let roughing_levels = depth_stepping.all_levels();
        for m in &tp.moves {
            if let MoveType::Linear { .. } = m.move_type {
                let z = m.target.z;
                // Any linear move should be at a legitimate level, tab_z, or
                // a plunge between levels.
                let at_known_level = roughing_levels.iter().any(|&lv| (z - lv).abs() < 0.01);
                let at_tab_z = (z - tab_z).abs() < 0.01;
                let is_plunge_or_retract = z > final_z + 0.01 && z < safe_z - 0.01;
                assert!(
                    at_known_level || at_tab_z || is_plunge_or_retract,
                    "unexpected linear move z={z}, expected one of levels {roughing_levels:?}, tab_z={tab_z}, or plunge"
                );
            }
        }
    }

    // --- Task A4: Face OneWay produces unidirectional cuts ---

    #[test]
    fn face_oneway_produces_nonempty_toolpath() {
        // OneWay face direction correctness is validated by core's own tests
        // (face::tests::face_oneway_all_passes_same_x_direction).
        // This test verifies that the viz -> core dispatch produces a non-empty result.
        let cfg = FaceConfig {
            direction: FaceDirection::OneWay,
            stepover: 10.0,
            ..FaceConfig::default()
        };
        let (tp, _) = generated(polygon_spec(OperationConfig::Face(cfg), ToolType::EndMill));

        // Verify the operation produces cutting moves
        let cutting_moves: Vec<_> = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
            .collect();
        assert!(
            !cutting_moves.is_empty(),
            "Face OneWay operation should produce cutting moves"
        );
    }

    #[test]
    fn face_zigzag_alternates_direction() {
        let cfg = FaceConfig {
            direction: FaceDirection::Zigzag,
            stepover: 10.0,
            ..FaceConfig::default()
        };
        let (tp, _) = generated(polygon_spec(
            OperationConfig::Face(cfg.clone()),
            ToolType::EndMill,
        ));

        let mut cut_directions = Vec::new();
        for i in 1..tp.moves.len() {
            if let MoveType::Linear { feed_rate } = tp.moves[i].move_type
                && (feed_rate - cfg.feed_rate).abs() < 1e-6
            {
                let dx = tp.moves[i].target.x - tp.moves[i - 1].target.x;
                if dx.abs() > 1.0 {
                    cut_directions.push(dx > 0.0);
                }
            }
        }

        assert!(
            cut_directions.len() >= 2,
            "Zigzag face should have multiple cutting rows"
        );
        // Should have at least one direction change
        let has_alternation = cut_directions.windows(2).any(|w| w[0] != w[1]);
        assert!(
            has_alternation,
            "Zigzag face should alternate cut directions"
        );
    }

    #[test]
    fn zigzag_angle_uses_degrees_for_runtime_path() {
        let cfg = ZigzagConfig {
            angle: 90.0,
            stepover: 6.0,
            depth: 2.0,
            depth_per_pass: 2.0,
            ..ZigzagConfig::default()
        };
        let (tp, _) = generated(polygon_spec(
            OperationConfig::Zigzag(cfg.clone()),
            ToolType::EndMill,
        ));

        assert_eq!(
            dominant_cut_axis(&tp, cfg.feed_rate),
            Some('y'),
            "90 degree zigzag should cut along Y, not a radian-converted skew"
        );
    }

    #[test]
    fn rest_semantic_angle_uses_degrees() {
        // The Ø16 reference tool replaces the request's old
        // `prev_tool_radius = Some(8.0)`: the resolver reads the radius off
        // the tool the config names, so the fixture has to hold that tool.
        let previous = ToolConfig::new_default(ToolId(2), ToolType::EndMill);
        let mut previous = previous;
        previous.diameter = 16.0;
        let cfg = RestConfig {
            angle: 90.0,
            stepover: 4.0,
            depth: 2.0,
            depth_per_pass: 2.0,
            prev_tool_id: Some(previous.id),
            ..RestConfig::default()
        };
        let mut spec = polygon_spec(OperationConfig::Rest(cfg.clone()), ToolType::EndMill);
        spec.extra_tools = vec![previous];
        let (tp, _) = generated(spec);

        assert_eq!(
            dominant_cut_axis(&tp, cfg.feed_rate),
            Some('y'),
            "semantic rest reconstruction should preserve the degree-based raster angle"
        );
    }

    // --- Task A10: Inlay female and male are separated ---

    #[test]
    fn inlay_output_contains_female_and_male_sections() {
        let cfg = InlayConfig::default();
        let mut spec = polygon_spec(OperationConfig::Inlay(cfg), ToolType::VBit);
        spec.polygons = Some(vec![Polygon2::rectangle(-10.0, -10.0, 10.0, 10.0)]);
        let (tp, safe_z) = generated(spec);

        assert!(!tp.moves.is_empty(), "Inlay should produce moves");

        // Find retract-to-safe-z moves that separate sections. The female
        // and male toolpaths should be separated by a retract to safe_z.
        let retract_indices: Vec<usize> = tp
            .moves
            .iter()
            .enumerate()
            .filter(|(_, m)| m.move_type == MoveType::Rapid && (m.target.z - safe_z).abs() < 0.01)
            .map(|(i, _)| i)
            .collect();

        // There should be multiple retract moves (at least the separator
        // between female and male plus retracts within each section)
        assert!(
            retract_indices.len() >= 2,
            "Should have retract separators between female and male sections"
        );

        // Find cutting moves below Z=0 (both female and male cut below the surface)
        let cutting_moves: Vec<_> = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { .. }) && m.target.z < -0.01)
            .collect();
        assert!(
            !cutting_moves.is_empty(),
            "Should have cutting moves below stock surface"
        );
    }

    // --- Task C14: Scallop requires BallNose tool ---

    #[test]
    fn scallop_rejects_non_ballnose_tool() {
        let OperationConfig::Scallop(cfg) =
            OperationConfig::new_default(crate::state::toolpath::OperationType::Scallop)
        else {
            unreachable!();
        };
        let spec = polygon_spec(OperationConfig::Scallop(cfg), ToolType::EndMill)
            .with_mesh(rs_cam_core::mesh::make_test_flat(40.0));
        let req = request(spec);
        let result = run_compute(&req).result;
        let Err(error) = result else {
            panic!("a flat end mill must not generate a scallop pass");
        };
        assert!(
            error.to_string().contains("Ball Nose"),
            "Error should mention Ball Nose requirement; got {error}"
        );
    }

    /// The fixture builder is used, and the dressup-free default is what
    /// these tests read.
    #[test]
    fn the_fixture_turns_every_dressup_off() {
        let dressups = no_dressups();
        assert!(!dressups.feed_optimization);
        assert!(!dressups.link_moves);
        assert!(!dressups.optimize_rapid_order);
    }

    /// The board the fixtures cut is topped at `z = 0`.
    #[test]
    fn the_fixture_board_is_topped_at_zero() {
        let bbox = stock_bbox(&board(25.0, 15.0));
        assert!((bbox.max.z).abs() < 1e-9, "stock top is {}", bbox.max.z);
    }
}
