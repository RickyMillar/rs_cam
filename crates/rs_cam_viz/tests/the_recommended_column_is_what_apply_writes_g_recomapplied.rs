//! **G-RECOMAPPLIED — the card's recommended column is what `⚡ Apply all`
//! writes.**
//!
//! # The defect (operator screenshot, 2026-09-24 12:01)
//!
//! The Feeds card's `current → recommended · Δ vs current` rail printed the
//! RAW calculator DOC and WOC (`explain.recommended.axial_depth_mm`,
//! `radial_width_mm`). `⚡ Apply all` writes the values after
//! `enforce_invariants`, and since the aggressiveness dial (Suggest pass 6b)
//! that funnel scales them by one common factor. On a Ø6 hardwood pocket the
//! column read "DOC 0.81 mm → 4.20 mm ↑ 5.17×", and Apply wrote 0.81 mm
//! again. The ⚡ pills had the same defect once and read the funnel's dry run
//! since G-PILLCLAMP; the card now reads the same dry run.
//!
//! # What this pins
//!
//! 1. The painted DOC and WOC recommended values equal the depth per pass and
//!    the stepover that `⚡ Apply all` then writes.
//! 2. The cell separates the two: the raw calculator value differs from the
//!    applied value, so arm 1 is not vacuous.
//! 3. The DOC hover keeps the calculator value and names the dial.
//! 4. After the apply the Δ tag is "≈": the operation already carries the
//!    recommendation.
//! 5. The power row reads the same cut: its painted percent equals
//!    `feeds::power_at_operating_point` on the operation that `⚡ Apply all`
//!    writes, and its hover quotes the calculator's cut beside it.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_viz::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest,
    GenerationControl, LaneSnapshot, OptimizeRequest, SimulationRequest, ToolpathSubmitOutcome,
};
use rs_cam_viz::controller::AppController;
use rs_cam_viz::state::runtime::ToolpathRuntime;
use rs_cam_viz::state::selection::Selection;
use rs_cam_viz::ui::{AppEvent, properties, tokens};

/// The Simulation workspace's right rail, the narrowest the Feeds tab holds.
const PANEL_WIDTH: f32 = 240.0;

/// The operator's dial setting on the screenshot.
const AGGRESSIVENESS: f64 = 0.85;

/// The row prints two decimals, so a value within half a step prints the
/// same. The small margin absorbs the binary representation of 0.005.
const TOLERANCE_MM: f64 = 0.005 + 1e-9;

// ── harness ────────────────────────────────────────────────────────────────

/// A compute backend that accepts every submission and returns nothing. The
/// apply handler only marks the toolpath stale; it never waits on a lane.
struct SilentBackend;

impl ComputeBackend for SilentBackend {
    fn submit_toolpath(&mut self, _request: ComputeRequest) -> ToolpathSubmitOutcome {
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

/// A Ø6 2-flute flat end mill on its default stickout, a default pocket,
/// generic hardwood, and the generic router with the dial at 0.85.
///
/// The default stickout is 45 mm, L/D 7.5, so the long-tool share is 0.75
/// and the dial's load target is 0.85 × 0.75 = 64 % of the base engagement.
fn controller() -> AppController<SilentBackend> {
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.flute_count = 2;

    let mut machine = MachineProfile::generic_wood_router();
    machine.aggressiveness = AGGRESSIVENESS;

    let stock = StockConfig {
        material: Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        ..Default::default()
    };

    let config = ToolpathConfig {
        id: rs_cam_core::ToolpathId(0),
        name: "Recommended column fixture".to_owned(),
        enabled: true,
        operation: OperationConfig::Pocket(Default::default()),
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: 1,
        model_id: 1,
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

    let model = LoadedModel {
        id: 1,
        path: PathBuf::from("recommended_column_fixture.svg"),
        name: "Recommended column fixture".to_owned(),
        kind: Some(ModelKind::Svg),
        mesh: None,
        polygons: Some(Arc::new(vec![Polygon2::rectangle(0.0, 0.0, 40.0, 40.0)])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
    };

    let mut builder = ProjectSessionBuilder::new()
        .stock(stock)
        .tool(tool)
        .model(model)
        .machine(machine);
    builder
        .add_toolpath(0, config)
        .expect("add the recommended column fixture");
    let mut controller = AppController::with_backend(SilentBackend);
    controller.state.session = builder.build();
    let id = controller.state.session.toolpath_configs()[0].id;
    controller
        .state
        .gui
        .toolpath_rt
        .insert(id, ToolpathRuntime::new(true));
    controller.state.selection = Selection::Toolpath(id);
    controller
}

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    // Paints tooltips too, which is where the calculator value lives.
    ctx.memory_mut(|memory| memory.set_everything_is_visible(true));
    ctx
}

/// Every text run the Feeds tab paints, in paint order. The production tab
/// override is one-shot, so the second frame is the steady state.
fn painted_text(controller: &mut AppController<SilentBackend>) -> Vec<String> {
    let id = controller.state.session.toolpath_configs()[0].id;
    controller.state.gui.pending_toolpath_tab =
        Some((id, rs_cam_viz::ui::properties::ToolpathTab::FeedsSpeeds));
    let ctx = ctx();
    let mut texts = Vec::new();
    for pass in 0..2 {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::Panel::right("recomapplied_properties")
                .default_size(PANEL_WIDTH)
                .max_size(PANEL_WIDTH)
                .resizable(true)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        let mut events = Vec::new();
                        properties::draw(ui, &mut controller.state, &mut events);
                    });
                });
        });
        if pass == 1 {
            for clipped in &out.shapes {
                if let egui::epaint::Shape::Text(text) = &clipped.shape {
                    texts.push(text.galley.job.text.clone());
                }
            }
        }
        out.textures_delta.clear();
    }
    texts
}

/// One `label current → recommended Δ` row of the card.
struct Row {
    current: String,
    recommended: String,
    delta: String,
}

/// The row that `rail_row` paints under `label`: the label run, then four
/// single-line runs in paint order. Multi-line runs are hovers.
fn row(texts: &[String], label: &str) -> Row {
    let marked = format!("{label} {}", tokens::GLYPH_DETAIL);
    let at = texts
        .iter()
        .position(|t| *t == marked)
        .unwrap_or_else(|| panic!("no `{marked}` row painted; runs were {texts:#?}"));
    let runs: Vec<&String> = texts[at + 1..]
        .iter()
        .filter(|t| !t.contains('\n'))
        .take(4)
        .collect();
    assert_eq!(runs.len(), 4, "the `{marked}` row painted too few runs");
    assert_eq!(runs[1], "\u{2192}", "the `{marked}` row lost its arrow");
    Row {
        current: runs[0].clone(),
        recommended: runs[2].clone(),
        delta: runs[3].clone(),
    }
}

/// The millimetre value of a painted `0.81 mm` run.
fn mm(run: &str) -> f64 {
    run.strip_suffix(" mm")
        .unwrap_or_else(|| panic!("`{run}` is not a millimetre value"))
        .parse()
        .unwrap_or_else(|_| panic!("`{run}` does not parse"))
}

/// The raw calculator recommendation for the fixture's operation.
fn calculator(controller: &AppController<SilentBackend>) -> rs_cam_core::feeds::FeedsResult {
    let session = &controller.state.session;
    let tc = &session.toolpath_configs()[0];
    let tool = session.tools()[0].clone();
    rs_cam_core::feeds::suggest::feeds_preview_for_operation(
        &tc.operation,
        &tool,
        &session.stock_config().material,
        session.machine(),
        rs_cam_core::feeds::embedded_vendor_lut(),
        session.post_config().spindle_strategy,
    )
    .recommended()
    .clone()
}

// ── the arms ───────────────────────────────────────────────────────────────

#[test]
fn the_recommended_column_is_what_apply_writes_g_recomapplied() {
    let mut controller = controller();
    let raw = calculator(&controller);
    let texts = painted_text(&mut controller);
    let doc = row(&texts, "DOC");
    let woc = row(&texts, "WOC");

    let id = controller.state.session.toolpath_configs()[0].id;
    controller.handle_internal_event(AppEvent::ApplyFeedsAll(id));
    let op = controller.state.session.toolpath_configs()[0]
        .operation
        .clone();
    let applied_doc = op.depth_per_pass().expect("a pocket carries DOC");
    let applied_woc = op.stepover().expect("a pocket carries WOC");

    // Arm 2 first: a cell where the funnel does not move the values would
    // pass arm 1 on the raw column too.
    assert!(
        (raw.axial_depth_mm - applied_doc).abs() > 0.1,
        "the fixture does not separate the calculator DOC ({:.3} mm) from the \
         applied DOC ({applied_doc:.3} mm), so this sentry proves nothing",
        raw.axial_depth_mm
    );

    // Arm 1: the painted value is the applied value.
    for (name, painted, applied) in [
        ("DOC", &doc.recommended, applied_doc),
        ("WOC", &woc.recommended, applied_woc),
    ] {
        assert_eq!(
            *painted,
            format!("{applied:.2} mm"),
            "the {name} row recommends {painted}, and `⚡ Apply all` writes \
             {applied:.3} mm"
        );
        assert!(
            (mm(painted) - applied).abs() <= TOLERANCE_MM,
            "the {name} row recommends {painted}, and `⚡ Apply all` writes \
             {applied:.3} mm"
        );
    }

    // Arm 3: the hover keeps the calculator value and names the dial.
    let calculator_clause = format!("calculator {:.2} mm", raw.axial_depth_mm);
    let apply_clause = format!("so Apply writes {applied_doc:.2} mm");
    let hover = texts
        .iter()
        .find(|t| t.contains('\n') && t.contains(&calculator_clause))
        .unwrap_or_else(|| {
            panic!("no DOC hover carries `{calculator_clause}`; runs were {texts:#?}")
        });
    assert!(
        hover.contains("the dial holds the load at") && hover.contains(&apply_clause),
        "the DOC hover does not say that the dial moved the value and what \
         Apply writes: {hover}"
    );
}

#[test]
fn the_delta_is_level_after_the_apply_g_recomapplied() {
    let mut controller = controller();
    let id = controller.state.session.toolpath_configs()[0].id;
    controller.handle_internal_event(AppEvent::ApplyFeedsAll(id));
    let op = controller.state.session.toolpath_configs()[0]
        .operation
        .clone();
    let applied_doc = op.depth_per_pass().expect("a pocket carries DOC");
    let applied_woc = op.stepover().expect("a pocket carries WOC");

    let texts = painted_text(&mut controller);
    for (name, applied) in [("DOC", applied_doc), ("WOC", applied_woc)] {
        let painted = row(&texts, name);
        assert_eq!(
            painted.current,
            format!("{applied:.2} mm"),
            "the {name} row's current value is not the applied value"
        );
        assert!(
            (mm(&painted.recommended) - applied).abs() <= TOLERANCE_MM,
            "after `⚡ Apply all` the {name} row still recommends {} against \
             the applied {applied:.3} mm",
            painted.recommended
        );
        assert_eq!(
            painted.delta, "\u{2248}",
            "after `⚡ Apply all` the {name} row's Δ tag reads `{}`; the \
             operation already carries the recommendation",
            painted.delta
        );
    }
}

/// The percent the power row paints, parsed back off the face.
fn painted_percent(face: &str) -> f64 {
    let at = face
        .find('%')
        .unwrap_or_else(|| panic!("the power face paints no percent: {face:?}"));
    let digits: String = face[..at]
        .trim_end()
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    digits
        .chars()
        .rev()
        .collect::<String>()
        .parse()
        .unwrap_or_else(|_| panic!("the power face paints no parsable percent: {face:?}"))
}

#[test]
fn the_power_row_reads_the_applied_cut_g_recomapplied() {
    let mut controller = controller();
    let texts = painted_text(&mut controller);
    let label = format!("Power {}", tokens::GLYPH_DETAIL);
    let at = texts
        .iter()
        .position(|t| *t == label)
        .unwrap_or_else(|| panic!("no power row painted; runs were {texts:#?}"));
    let face: Vec<&String> = texts[at + 1..]
        .iter()
        .filter(|t| !t.contains('\n'))
        .take(2)
        .collect();
    let painted = painted_percent(face[0]);

    let id = controller.state.session.toolpath_configs()[0].id;
    controller.handle_internal_event(AppEvent::ApplyFeedsAll(id));
    let session = &controller.state.session;
    let op = &session.toolpath_configs()[0].operation;
    let figure = rs_cam_core::feeds::power_at_operating_point(
        op,
        &session.tools()[0],
        &session.stock_config().material,
        session.machine(),
        None,
    )
    .unwrap_or_else(|reason| panic!("the applied pocket refuses the power door: {reason:?}"));
    let expected = figure.required_kw / figure.available_kw * 100.0;
    assert!(
        (painted - expected).abs() <= 0.05,
        "the power row painted {painted} % before the apply, and \
         feeds::power_at_operating_point reads {expected:.2} % on the \
         operation `⚡ Apply all` writes ({:.4} kW of {:.4} kW at {:.3} mm \
         by {:.3} mm)",
        figure.required_kw,
        figure.available_kw,
        figure.ap_mm,
        figure.ae_mm,
    );

    let hover = texts
        .iter()
        .filter(|t| t.contains('\n') && t.starts_with("Power"))
        .max_by_key(|t| t.len())
        .unwrap_or_else(|| panic!("the power row painted no hover; runs were {texts:#?}"));
    assert!(
        hover.contains("Power: calculator")
            && hover.contains("the dial holds the load at")
            && hover.contains(&format!("so the cut Apply writes draws {expected:.1} %")),
        "the power hover does not quote the calculator's cut beside the \
         applied one: {hover}"
    );
}
