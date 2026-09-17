//! F-024 and F-028 — the world stock bbox and the heights an identity
//! setup resolves in.

use super::*;

/// F-024 (third-site fix, 2026-05-25): end-to-end regression that the viz
/// worker simulation, when fed the controller-built world `stock_bbox`,
/// reports per-sample `axial_engagement_mm` matching the commanded DOC
/// rather than the full stock height.
///
/// This is the controller-path equivalent of
/// `compute::worker::tests::as001_viz_path_first_pass_axial_engagement_within_commanded_doc_f024`
/// — that test built the world `stock_bbox` by hand. This test builds the
/// bbox via `build_world_stock_bbox(&session)`, so it fails (per-sample
/// peak axial = ~12 mm) if the controller-side helper ever regresses to the
/// pre-fix zero-rooted construction even if the worker-side fix is intact.
#[test]
fn controller_built_stock_bbox_drives_axial_engagement_within_commanded_doc_f024() {
    use rs_cam_core::compute::stock_config::StockConfig;
    use rs_cam_core::material::{Material, WoodSpecies};
    use rs_cam_core::stock::simulation_cut::CutKinematics;
    use std::sync::atomic::AtomicBool;

    use rs_cam_core::compute::cutter::build_cutter;
    use rs_cam_core::compute::simulate::{SimGroupEntry, SimToolpathEntry};

    use crate::compute::SimulationRequest as VizSimulationRequest;

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
    let session = ProjectSessionBuilder::new().stock(stock).build();

    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    tool.shank_diameter = 6.35;
    tool.shank_length = 20.0;
    tool.stickout = 45.0;
    tool.flute_count = 2;
    tool.name = "End Mill 6mm".to_owned();

    // First-pass-of-pocket-style linear cutting at world Z=-2. Same shape as
    // the worker-tests F-024 regression so the assertion threshold stays
    // comparable; the only difference is that we use the controller helper
    // to build the world `stock_bbox`.
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(10.0, 30.0, 5.0));
    tp.feed_to(P3::new(10.0, 30.0, -2.0), 385.0);
    for i in 0..40 {
        let x = 10.0 + (i as f64) * 1.5;
        tp.feed_to(P3::new(x, 30.0, -2.0), 770.0);
    }
    tp.rapid_to(P3::new(70.0, 30.0, 5.0));

    // World bbox via the controller helper. With the fix this respects the
    // stock origin (Z=[-12, 0]); pre-fix it was Z=[0, 12].
    let world_stock_bbox = crate::controller::events::simulation::build_world_stock_bbox(&session);

    let request = VizSimulationRequest {
        core: rs_cam_core::compute::simulate::SimulationRequest {
            groups: vec![SimGroupEntry {
                toolpaths: vec![SimToolpathEntry {
                    id: ToolpathId(1),
                    name: "AS001 Pocket Pass 1".to_owned(),
                    annotated: Arc::new(
                        rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(tp),
                    ),
                    tool: Arc::new(build_cutter(&tool)),
                    flute_count: tool.flute_count,
                    tool_summary: tool.summary(),
                    semantic_trace: None,
                    spindle_rpm: Some(18_000),
                    metrics_not_applicable: false,
                    drill_op: None,
                    operation_config_hash: 0,
                }],
                direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
                // Identity-setup shape from `controller::events::simulation`:
                // no local bbox, so the simulator grids `stock_bbox`.
                local_stock_bbox: None,
                local_to_global: None,
                phantom_prior_stock: None,
            }],
            stock_bbox: world_stock_bbox,
            stock_top_z: world_stock_bbox.max.z,
            resolution: 1.0,
            metric_options: rs_cam_core::stock::simulation_cut::SimulationMetricOptions {
                enabled: true,
                capture_arc_engagement: true,
            },
            spindle_rpm: 18_000,
            rapid_feed_mm_min: 5_000.0,
            model_mesh: None,
            kinematics: None,
        },
        memoize_prefix: false,
    };

    let cancel = AtomicBool::new(false);
    let result = crate::compute::worker::execute::run_simulation_with_phase(
        &request,
        &cancel,
        |_phase| {},
        None,
    )
    .expect("viz simulation completes");

    let cut_trace = result.core.cut_trace.as_ref().expect("metric cut trace");

    let mut first_pass_axials: Vec<f64> = cut_trace
        .samples
        .iter()
        .filter(|s| s.is_cutting && s.cut_kinematics == CutKinematics::Linear)
        .filter(|s| (s.position[2] - (-2.0)).abs() < 0.5)
        .map(|s| s.axial_engagement_mm)
        .collect();
    first_pass_axials.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    assert!(
        !first_pass_axials.is_empty(),
        "expected at least one linear cutting sample near Z=-2; got 0"
    );

    let peak = *first_pass_axials.last().unwrap_or(&0.0);
    assert!(
        peak <= 3.0,
        "F-024 (third site): first-pass axial engagement with controller-built \
         world bbox should be <= 3.0 mm (commanded 2.0 + grid discretisation \
         margin); got peak = {peak:.4} mm across {} samples. If this is ~12 mm, \
         the controller is dropping `stock.origin_z` when constructing the \
         world bbox passed to the worker — see \
         `controller::events::simulation::build_world_stock_bbox` and the \
         F-024 finding notes.",
        first_pass_axials.len()
    );
}

/// F-028 viz-path follow-up regression test.
///
/// AS001-shape session (stock origin_z=-12, identity setup, auto_from_model
/// =false, world stock top at Z=0) with a Pocket toolpath on a 2D polygon
/// model. `submit_toolpath_compute` must build a `HeightContext` from the
/// **world** stock bbox, so `heights.top_z = 0.0` (world stock top) and the
/// downstream toolpath generator emits cuts at Z=-2, -4, -6 to match the
/// downstream sim path's world-frame dexel grid.
///
/// Pre-fix readings (commit `f00c733`): `heights.top_z = 12.0`,
/// `stock_bbox.max.z = 12.0`, cuts emitted at Z=10, 8, 6 → simulation peak
/// axial = 0, total removed = 0, air cut = 96 %.
///
/// Post-fix: both are 0.0 (world stock top for AS001), cuts emit at
/// Z=-2, -4, -6, simulation engages stock.
#[test]
fn as001_pocket_heights_resolve_in_world_frame_for_identity_setup_f028() {
    use rs_cam_core::compute::operation_configs::PocketConfig;
    use rs_cam_core::compute::operation_configs::PocketPattern;
    use rs_cam_core::compute::stock_config::StockConfig;
    use rs_cam_core::material::{Material, WoodSpecies};
    use rs_cam_core::polygon::Polygon2;

    let mut controller = AppController::with_backend(CapturingBackend::default());

    // 6 mm endmill matching the AS001 fixture.
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    tool.shank_diameter = 6.35;
    tool.shank_length = 20.0;
    tool.stickout = 45.0;
    tool.flute_count = 2;
    tool.name = "End Mill 6mm".to_owned();
    controller.state.session = ProjectSessionBuilder::new().tool(tool).build();

    // AS001 stock: origin_z=-12, z=12 → world stock top at Z=0.
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
    let _ = controller
        .state
        .session
        .apply(Command::SetStockConfig(SetStockConfigArgs {
            stock: Box::new(stock),
        }))
        .expect("the stock edit applies");

    // 2D polygon model (matches the SVG-driven AS001 pocket case).
    let model_id = controller
        .state
        .session
        .apply(Command::AddModel(AddModelArgs {
            model: Box::new(LoadedModel {
                id: 0,
                path: std::path::PathBuf::from("demo_pocket.svg"),
                name: "demo_pocket".to_owned(),
                kind: Some(ModelKind::Svg),
                mesh: None,
                polygons: Some(Arc::new(vec![Polygon2::rectangle(20.0, 20.0, 80.0, 80.0)])),
                drill_targets: std::sync::Arc::new(Vec::new()),
                layers: std::sync::Arc::new(Vec::new()),
                enriched_mesh: None,
                units: Some(ModelUnits::Millimeters),
                winding_report: None,
                load_error: None,
            }),
        }))
        .expect("the session takes the model")
        .created
        .expect("the AddModel row reports the new model id");

    // AS001 pocket params: depth=6, dpp=2, stepover=2.4, feed=900, plunge=350.
    let pocket = PocketConfig {
        stepover: 2.4,
        depth: 6.0,
        depth_per_pass: 2.0,
        feed_rate: 900.0,
        plunge_rate: 350.0,
        climb: true,
        pattern: PocketPattern::Contour,
        angle: 0.0,
        finishing_passes: 0,
        spindle_rpm: Some(18_000),
    };

    let tp_config = ToolpathConfig {
        id: ToolpathId(0),
        name: "Pocket (AS001)".to_owned(),
        enabled: true,
        operation: OperationConfig::Pocket(pocket),
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: 1,
        model_id,
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
    let tp_idx = controller
        .state
        .session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(tp_config),
        }))
        .expect("add pocket to default setup")
        .created
        .expect("the AddToolpath row reports the new toolpath index");
    let tp_id = controller
        .state
        .session
        .toolpath_configs()
        .get(tp_idx)
        .expect("toolpath_configs slot present after add_toolpath")
        .id;

    // Drive the production code path. The default setup is identity
    // (face_up=Top, z_rotation=Deg0).
    controller.submit_toolpath_compute(tp_id);

    let request = controller
        .compute
        .captured
        .as_ref()
        .expect("submit_toolpath_compute should have submitted a ComputeRequest");

    // WP11b: the request carries the handle, whose fields are private. The
    // handle publishes the same facts as a snapshot, which is what the
    // debug artifact has always recorded — so the claim is unchanged and
    // the reader is the one door.
    let snapshot = request.handle.request_snapshot();
    let number = |path: [&str; 2]| -> f64 {
        snapshot[path[0]][path[1]]
            .as_f64()
            .unwrap_or_else(|| panic!("the snapshot must carry {path:?}: {snapshot}"))
    };
    let bbox_number = |corner: &str, axis: &str| -> f64 {
        snapshot["stock_bbox"][corner][axis]
            .as_f64()
            .unwrap_or_else(|| panic!("the snapshot must carry stock_bbox: {snapshot}"))
    };

    // The world stock top for AS001 sits at Z=0 (origin_z=-12 + stock.z=12).
    let top_z = number(["heights", "top_z"]);
    assert!(
        top_z.abs() < 1e-9,
        "F-028 viz-path: heights.top_z must resolve to the world stock top \
         (Z=0 for AS001 identity setup); got {top_z:.6}. Pre-fix this read \
         12.0 (the local zero-rooted stock_top), and the toolpath generator \
         emitted cuts at Z=10, 8, 6 in setup-local frame. The downstream \
         viz simulation drops `local_to_global = None` for identity setups \
         (F-024 viz-worker follow-up `1dd1aa7`) so the dexel grid is rebuilt \
         in world frame — and the generator's local-frame cuts at Z=10 sat \
         10 mm above the world stock top at Z=0. Round-07 MCP smoke: \
         peak_axial=0, total_removed=0, air_cut=96 %."
    );

    // Bottom of first pass: top_z - depth_per_pass*N or full depth.
    // With heights.top_z = 0 and depth = 6 (full pocket depth), bottom = -6.
    let bottom_z = number(["heights", "bottom_z"]);
    assert!(
        (bottom_z - -6.0).abs() < 1e-9,
        "F-028 viz-path: heights.bottom_z must resolve to top_z - depth = -6 \
         for the AS001 pocket; got {bottom_z:.6}. Pre-fix this read 6.0 \
         (12 - 6 in local frame)."
    );

    let max_z = bbox_number("max", "z");
    assert!(
        max_z.abs() < 1e-9,
        "F-028 viz-path: the emission stock bbox max.z must equal the world \
         stock top (Z=0 for AS001 identity setup); got {max_z:.6}. Pre-fix \
         this read 12.0 — the controller built a zero-rooted local bbox \
         even for identity setups, and the worker then built boundary \
         rectangles in that frame."
    );

    let min_z = bbox_number("min", "z");
    assert!(
        (min_z - -12.0).abs() < 1e-9,
        "F-028 viz-path: the emission stock bbox min.z must equal \
         stock.origin_z (-12 for AS001 identity setup); got {min_z:.6}. \
         Pre-fix this read 0.0."
    );

    // XY frame: for identity setups the bbox must respect stock.origin_x/y too.
    let min_x = bbox_number("min", "x");
    let min_y = bbox_number("min", "y");
    assert!(
        (min_x - -10.0).abs() < 1e-9 && (min_y - -10.0).abs() < 1e-9,
        "F-028 viz-path: the emission stock bbox min.{{x,y}} must equal \
         stock.origin_{{x,y}} (-10, -10 for AS001 identity setup); got \
         ({min_x:.6}, {min_y:.6})."
    );
}

// ---------------------------------------------------------------------------
// Submit-time fail-hard must resolve a pending MCP `generate_toolpath`
// waiter, not strand it. Confirmed live: an MCP `generate_toolpath` call
// hung ~9 hours because `submit_toolpath_compute` returned early on a
// precondition rejection (e.g. the `DerivedRestRegions` self-reference
// check, or the `FromRemainingStock` no-prior-sim check) without ever
// invoking `notify_mcp_toolpath_complete` — that notify only fired from
// `drain_compute_results`, which never runs for a request that never
// reached the compute worker.
// ---------------------------------------------------------------------------

#[cfg(feature = "mcp")]
#[test]
fn submit_toolpath_compute_self_referential_boundary_resolves_mcp_waiter() {
    let mut controller = sample_controller();
    let source_id = ToolpathId(0);
    let dependent_id = add_derived_rest_dependent(&mut controller, source_id);

    // Rewrite the dependent's boundary to reference itself — the
    // self-referential fail-hard precondition in `submit_toolpath_compute`.
    //
    // WP7: the whole-config row, not `SetBoundaryConfig`. The boundary
    // row runs `auto_enable_rest_analysis_for_source`, which would flip
    // the dependent's own `rest_analysis.enabled` — a field this test
    // does not intend to write.
    let (index, stored) = controller
        .state
        .session
        .find_toolpath_config_by_id(dependent_id)
        .expect("dependent toolpath config must exist");
    let mut config = stored.clone();
    config.boundary.source = crate::state::toolpath::BoundarySource::DerivedRestRegions {
        source_toolpath_id: dependent_id,
    };
    let _ = controller
        .state
        .session
        .apply(Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
            index,
            config: Box::new(config),
        }))
        .expect("the dependent sits at a live index");

    // Register a pending MCP `generate_toolpath` waiter for this toolpath,
    // mirroring what `app/mcp.rs::mcp_generate_toolpath` does before pushing
    // the `GenerateToolpath` event.
    controller.pending_mcp = Some(crate::mcp_bridge::PendingMcpCompute::new());
    let (tx, mut rx) = tokio::sync::oneshot::channel();
    controller
        .pending_mcp
        .as_mut()
        .expect("pending_mcp was just set")
        .toolpath
        .insert(dependent_id, tx);

    // Drive the production submit path directly (mirrors how
    // `AppEvent::GenerateToolpath` is dispatched in `controller/events/mod.rs`).
    controller.submit_toolpath_compute(dependent_id);

    // The MCP oneshot must already be resolved — no drain step should be
    // required, because this request never reached the compute worker.
    let response = rx.try_recv().expect(
        "submit-time fail-hard must resolve the pending MCP oneshot immediately, \
         not leave the caller waiting on a compute result that will never arrive",
    );
    let payload = response
        .result
        .expect("mcp response should carry an Ok(json) payload describing the error");
    assert!(
        payload.contains("own rest regions"),
        "mcp error payload should describe the self-referential boundary rejection, got: {payload}"
    );

    let rt = controller
        .state
        .gui
        .toolpath_rt
        .get(&dependent_id)
        .expect("dependent runtime should exist after fail-hard");
    assert!(
        matches!(
            &rt.status,
            crate::state::toolpath::ComputeStatus::Error(e) if e.contains("own rest regions")
        ),
        "toolpath runtime status should be Error mentioning the self-reference, got {:?}",
        rt.status
    );

    // The pending_mcp map must no longer hold this toolpath's sender —
    // `notify_mcp_toolpath_complete` removes it on resolution.
    assert!(
        !controller
            .pending_mcp
            .as_ref()
            .expect("pending_mcp still set")
            .toolpath
            .contains_key(&dependent_id),
        "resolved MCP waiter should be removed from the pending map"
    );
}
