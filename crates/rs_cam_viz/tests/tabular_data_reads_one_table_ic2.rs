//! IC2: tabular data reads one table; derivation goes in Details.
//!
//! # The defect this exists to catch
//!
//! The operator, 2026-10-13, after the IC1 clean-up: "if there is no
//! recommendation, can we just not recommend or show that line? … there is
//! very little consistency in how we display data like this. recommendation
//! and this show completely different ui. its all just tabular data."
//!
//! # What it pins
//!
//! 1. A recommendation row whose recommended value prints the same as the
//!    current value is not drawn in the default view. It is in the Feeds
//!    card's `Details`.
//! 2. The derivation rows (the Suggest stages, the long-tool share, the
//!    force line) are inside a `Details` disclosure that is closed by
//!    default. They paint only when it is open.
//! 3. Every surface that shows derived data renders it through the one
//!    table component, and the deleted per-section renderers stay deleted.
//!
//! The behavioural arms render the real inspector (`properties::draw`) twice:
//! once as the operator first sees it, and once with every disclosure open
//! (`set_everything_is_visible`), so each absence has a presence beside it.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    Command, LoadedModel, ProjectSessionBuilder, ReplaceToolpathConfigArgs, ToolpathConfig,
};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::runtime::ToolpathRuntime;
use rs_cam_viz::state::selection::Selection;
use rs_cam_viz::ui::components::data_table::DETAILS_LABEL;
use rs_cam_viz::ui::{properties, tokens};

/// The default inspector width (`app.rs`).
const PANEL_WIDTH: f32 = 280.0;

/// A Ø6 2-flute flat end mill on its default stickout (45 mm, so the
/// long-tool share is 0.75), a default pocket, generic hardwood (it has a
/// force line), and the generic router with the dial at 0.85. This is the
/// `the_recommended_column_is_what_apply_writes_g_recomapplied` cell.
fn fixture() -> AppState {
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.flute_count = 2;
    let mut machine = MachineProfile::generic_wood_router();
    machine.aggressiveness = 0.85;
    let stock = StockConfig {
        material: Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        ..Default::default()
    };
    let config = ToolpathConfig {
        shoe: Default::default(),
        id: rs_cam_core::ToolpathId(0),
        name: "IC2 fixture".to_owned(),
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
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        rest_analysis: Default::default(),
        planner_origin: None,
    };
    let model = LoadedModel {
        id: 1,
        path: PathBuf::from("ic2_fixture.svg"),
        name: "IC2 fixture".to_owned(),
        kind: Some(ModelKind::Svg),
        mesh: None,
        items: Arc::new(rs_cam_core::selection::ItemSet::from_polygons(vec![
            Polygon2::rectangle(0.0, 0.0, 40.0, 40.0),
        ])),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
    };
    let mut builder = ProjectSessionBuilder::new()
        .stock(stock)
        .tool(tool)
        .model(model)
        .machine(machine);
    builder
        .add_toolpath(0, config)
        .expect("add the IC2 fixture");
    let mut state = AppState::new();
    state.session = builder.build();
    let id = state.session.toolpath_configs()[0].id;
    state.gui.toolpath_rt.insert(id, ToolpathRuntime::new(true));
    state.selection = Selection::Toolpath(id);
    state
}

/// `⚡ Apply all`, through the same core doors the controller's funnel
/// uses: the preview, `suggest::apply` on a draft, and the command door.
fn apply_all(state: &mut AppState) {
    use rs_cam_core::feeds::suggest::{
        ApplyContext, ApplyScope, SuggestContext, apply, feeds_preview_for_operation,
    };
    let session = &state.session;
    let mut draft = session.toolpath_configs()[0].clone();
    let tool = session.tools()[0].clone();
    let machine = session.machine().clone();
    let material = session.stock_config().material.clone();
    let preview = feeds_preview_for_operation(
        &draft.operation,
        &tool,
        &material,
        &machine,
        rs_cam_core::feeds::embedded_vendor_lut(),
        session.post_config().spindle_strategy,
    );
    let rec = preview
        .applicable()
        .expect("the pocket pairing is applicable");
    let model_bbox = session.model_bbox(draft.model_id);
    let dressups = draft.dressups.clone();
    let pass_role = draft.operation.feeds_style().1;
    let _warnings = apply(
        &rec,
        ApplyScope::Both,
        &mut draft.operation,
        &mut draft.feeds_provenance,
        ApplyContext {
            tool: &tool,
            machine: &machine,
            material: &material,
            pass_role,
            suggest: SuggestContext {
                model_bbox: model_bbox.as_ref(),
                dressups: Some(&dressups),
                ..SuggestContext::default()
            },
        },
    );
    let _effects = state
        .session
        .apply(Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
            index: 0,
            config: Box::new(draft),
        }))
        .expect("the apply writes through the command door");
}

/// Every text run the Feeds tab paints on its steady-state frame. `open`
/// opens every disclosure; it also paints the hovers, which always hold a
/// newline, so the arms read single-line runs only.
fn painted(state: &mut AppState, open: bool) -> Vec<String> {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    if open {
        ctx.memory_mut(|memory| memory.set_everything_is_visible(true));
    }
    let id = state.session.toolpath_configs()[0].id;
    state.gui.pending_toolpath_tab = Some((id, properties::ToolpathTab::FeedsSpeeds));
    let mut texts = Vec::new();
    for pass in 0..2 {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::Panel::right("ic2_properties")
                .default_size(PANEL_WIDTH)
                .max_size(PANEL_WIDTH)
                .resizable(false)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        let mut events = Vec::new();
                        properties::draw(ui, state, &mut events);
                    });
                });
        });
        if pass == 1 {
            for clipped in &out.shapes {
                if let egui::epaint::Shape::Text(text) = &clipped.shape
                    && !text.galley.job.text.contains('\n')
                {
                    texts.push(text.galley.job.text.clone());
                }
            }
        }
        out.textures_delta.clear();
    }
    texts
}

/// The label run of a comparison row: its name and the detail mark.
fn row_label(name: &str) -> String {
    format!("{name} {}", tokens::GLYPH_DETAIL)
}

// ── arm 1 — no recommendation, no row ───────────────────────────────────

#[test]
fn a_level_recommendation_is_not_drawn_ic2() {
    let mut state = fixture();
    // Non-vacuity: before the apply the DOC and WOC rows differ, so the
    // default view draws them.
    let before = painted(&mut state, false);
    for name in ["DOC", "WOC"] {
        assert!(
            before.contains(&row_label(name)),
            "before the apply the {name} row must be on the face; runs {before:#?}"
        );
    }

    apply_all(&mut state);
    let face = painted(&mut state, false);
    for name in ["DOC", "WOC"] {
        assert!(
            !face.contains(&row_label(name)),
            "after `⚡ Apply all` the {name} row recommends the value the operation \
             already holds, and the default view still draws it; runs {face:#?}"
        );
    }
    // The level rows live in Details: open, each is there once, level.
    let open = painted(&mut state, true);
    for name in ["DOC", "WOC"] {
        let label = row_label(name);
        let count = open.iter().filter(|t| **t == label).count();
        assert_eq!(
            count, 1,
            "with Details open the {name} row must be drawn once; runs {open:#?}"
        );
    }
    assert!(
        face.iter().any(|t| t.contains("at the recommendation")),
        "a card with no difference says so in one status line; runs {face:#?}"
    );
}

// ── arm 2 — derivation is behind a closed Details ───────────────────────

/// Runs that only derivation rows paint, on this fixture.
const DERIVATION_PREFIXES: [&str; 4] = [
    // The aggressiveness dial record (the Suggest stage).
    "Aggressiveness 0.85",
    // The long-tool share (a repo rule).
    "Long tool",
    // The force line of generic hardwood (ruling B6).
    "Force line:",
    // The context row of the vendor source.
    "Vendor row",
];

#[test]
fn derivation_rows_are_in_a_closed_details_ic2() {
    let mut state = fixture();
    let face = painted(&mut state, false);
    assert!(
        face.iter().any(|t| t.starts_with(DETAILS_LABEL)),
        "the Feeds card draws no `{DETAILS_LABEL}` disclosure; runs {face:#?}"
    );
    for prefix in DERIVATION_PREFIXES {
        assert!(
            !face.iter().any(|t| t.starts_with(prefix)),
            "`{prefix}` is derivation and paints in the default view; it belongs \
             in the closed `{DETAILS_LABEL}`. Runs {face:#?}"
        );
    }
    // Non-vacuity: each one is there when Details is open.
    let open = painted(&mut state, true);
    for prefix in DERIVATION_PREFIXES {
        assert!(
            open.iter().any(|t| t.starts_with(prefix)),
            "`{prefix}` is not painted even with every disclosure open, so the \
             arm above proves nothing. Runs {open:#?}"
        );
    }
}

// ── arm 3 — one table component ─────────────────────────────────────────

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// The source with every `//` comment removed, so a name in a comment is
/// not read as code.
fn code_only(rel: &str) -> String {
    std::fs::read_to_string(src_root().join(rel))
        .unwrap_or_else(|e| panic!("read {rel}: {e}"))
        .lines()
        .map(|l| l.find("//").map_or(l, |i| &l[..i]))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The files that show derived data, and the fewest table sites each holds.
const TABLE_SURFACES: [(&str, usize); 4] = [
    ("ui/feeds/compare.rs", 3),
    ("ui/feeds/why.rs", 1),
    ("ui/properties/feeds_speeds.rs", 2),
    ("ui/sim_diagnostics.rs", 3),
];

/// The per-section renderers the table replaced. Each is a function name
/// or a grid id that must not come back.
const DELETED_RENDERERS: [&str; 15] = [
    "fn rail_row(",
    "fn rail_line(",
    "fn rail_cell(",
    "fn rail_efficiency_row(",
    "fn rail_power_row(",
    "fn rail_force_line_rows(",
    "fn rail_mrr_row(",
    "fn verdict_row_label(",
    "fn draw_context_chip(",
    "fn detail_line(",
    "fn plain_line(",
    "fn draw_suggest_lines(",
    "fn draw_row_basis_lines(",
    "fn draw_advance_per_tooth_card(",
    "fn wrapped_cell(",
];

#[test]
fn derived_data_renders_through_the_one_table_ic2() {
    let mut total = 0;
    for (rel, min) in TABLE_SURFACES {
        let code = code_only(rel);
        let sites = code.matches("DataTable::new(").count() + code.matches("DataRow::").count();
        assert!(
            sites >= min,
            "{rel} holds {sites} DataTable / DataRow sites, fewer than {min}. A \
             derived-data surface renders through `components::DataTable`."
        );
        total += sites;
        for name in DELETED_RENDERERS {
            assert!(
                !code.contains(name),
                "{rel} defines `{name}` again. The per-section renderer was \
                 replaced by `components::DataTable`."
            );
        }
    }
    // Non-vacuity: the scan read real code.
    assert!(total >= 20, "the scan found only {total} table sites");
    // One disclosure for derivation: the Feeds card and the operating point
    // open `Details`, and nothing draws the old breakdown header.
    let feeds = code_only("ui/feeds/compare.rs") + &code_only("ui/properties/feeds_speeds.rs");
    assert!(
        feeds.matches("Details::new(").count() >= 2,
        "the Feeds card and the operating point each open one `Details`"
    );
    assert!(
        !feeds.contains("\"Constraint breakdown\""),
        "the constraint breakdown is rows in `Details`, not a second disclosure"
    );
}
