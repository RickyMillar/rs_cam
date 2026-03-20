pub mod stock;
pub mod tool;
pub mod post;

use crate::state::AppState;
use crate::state::selection::Selection;
use crate::state::toolpath::*;
use crate::ui::AppEvent;

pub fn draw(ui: &mut egui::Ui, state: &mut AppState, events: &mut Vec<AppEvent>) {
    match state.selection.clone() {
        Selection::None => {
            ui.label(
                egui::RichText::new("Select an item in the project tree")
                    .italics()
                    .color(egui::Color32::from_rgb(120, 120, 130)),
            );
        }
        Selection::Stock => {
            // Capture snapshot for undo before editing
            if state.history.stock_snapshot.is_none() {
                state.history.stock_snapshot = Some(state.job.stock.clone());
            }
            stock::draw(ui, &mut state.job.stock, events);
            // If an edit just finished (DragValue released), push undo
            if events.iter().any(|e| matches!(e, AppEvent::StockChanged)) {
                if let Some(old) = state.history.stock_snapshot.take() {
                    if old.x != state.job.stock.x || old.y != state.job.stock.y || old.z != state.job.stock.z
                        || old.origin_x != state.job.stock.origin_x || old.origin_y != state.job.stock.origin_y
                        || old.origin_z != state.job.stock.origin_z || old.padding != state.job.stock.padding
                    {
                        state.history.push(crate::state::history::UndoAction::StockChange {
                            old,
                            new: state.job.stock.clone(),
                        });
                    }
                }
            }
        }
        Selection::PostProcessor => {
            post::draw(ui, &mut state.job.post);
        }
        Selection::Model(id) => {
            if let Some(model) = state.job.models.iter().find(|m| m.id == id) {
                ui.heading(&model.name);
                ui.separator();
                ui.label(format!("Type: {:?}", model.kind));
                ui.label(format!("Path: {}", model.path.display()));
                if let Some(mesh) = &model.mesh {
                    ui.add_space(4.0);
                    ui.label(format!("Vertices: {}", mesh.vertices.len()));
                    ui.label(format!("Triangles: {}", mesh.triangles.len()));
                    let bb = &mesh.bbox;
                    ui.label(format!(
                        "Bounds: {:.1} x {:.1} x {:.1} mm",
                        bb.max.x - bb.min.x,
                        bb.max.y - bb.min.y,
                        bb.max.z - bb.min.z
                    ));
                }
                if let Some(polys) = &model.polygons {
                    ui.add_space(4.0);
                    ui.label(format!("Polygons: {}", polys.len()));
                }
            }
        }
        Selection::Tool(id) => {
            if let Some(t) = state.job.tools.iter_mut().find(|t| t.id == id) {
                tool::draw(ui, t);
            }
        }
        Selection::Toolpath(id) => {
            // Snapshot tool/model lists to avoid borrow conflict with toolpaths
            let tools: Vec<_> = state.job.tools.iter().map(|t| (t.id, t.summary(), t.diameter)).collect();
            let models: Vec<_> = state.job.models.iter().map(|m| (m.id, m.name.clone())).collect();

            if let Some(entry) = state.job.toolpaths.iter_mut().find(|t| t.id == id) {
                draw_toolpath_panel(ui, entry, &tools, &models, events);
            }
        }
    }
}

fn draw_toolpath_panel(
    ui: &mut egui::Ui,
    entry: &mut ToolpathEntry,
    tools: &[(crate::state::job::ToolId, String, f64)],
    models: &[(crate::state::job::ModelId, String)],
    events: &mut Vec<AppEvent>,
) {
    ui.heading(&entry.name);
    ui.separator();

    // Name
    ui.horizontal(|ui| {
        ui.label("Name:");
        ui.text_edit_singleline(&mut entry.name);
    });

    // Tool selector
    ui.horizontal(|ui| {
        ui.label("Tool:");
        let tool_label = tools.iter().find(|(id, _, _)| *id == entry.tool_id)
            .map(|(_, s, _)| s.as_str()).unwrap_or("(none)");
        egui::ComboBox::from_id_salt("tp_tool")
            .selected_text(tool_label)
            .show_ui(ui, |ui| {
                for (id, name, _) in tools {
                    ui.selectable_value(&mut entry.tool_id, *id, name.as_str());
                }
            });
    });

    // Model selector
    ui.horizontal(|ui| {
        ui.label("Input:");
        let model_label = models.iter().find(|(id, _)| *id == entry.model_id)
            .map(|(_, s)| s.as_str()).unwrap_or("(none)");
        egui::ComboBox::from_id_salt("tp_model")
            .selected_text(model_label)
            .show_ui(ui, |ui| {
                for (id, name) in models {
                    ui.selectable_value(&mut entry.model_id, *id, name.as_str());
                }
            });
    });

    ui.add_space(8.0);
    ui.label(
        egui::RichText::new("Cutting Parameters")
            .strong()
            .color(egui::Color32::from_rgb(180, 180, 195)),
    );

    // Operation-specific parameters
    match &mut entry.operation {
        OperationConfig::Pocket(cfg) => draw_pocket_params(ui, cfg),
        OperationConfig::Profile(cfg) => draw_profile_params(ui, cfg),
        OperationConfig::Adaptive(cfg) => draw_adaptive_params(ui, cfg),
        OperationConfig::VCarve(cfg) => draw_vcarve_params(ui, cfg),
        OperationConfig::Rest(cfg) => draw_rest_params(ui, cfg, tools),
        OperationConfig::Inlay(cfg) => draw_inlay_params(ui, cfg),
        OperationConfig::Zigzag(cfg) => draw_zigzag_params(ui, cfg),
        OperationConfig::DropCutter(cfg) => draw_dropcutter_params(ui, cfg),
        OperationConfig::Adaptive3d(cfg) => draw_adaptive3d_params(ui, cfg),
        OperationConfig::Waterline(cfg) => draw_waterline_params(ui, cfg),
        OperationConfig::Pencil(cfg) => draw_pencil_params(ui, cfg),
        OperationConfig::Scallop(cfg) => draw_scallop_params(ui, cfg),
        OperationConfig::SteepShallow(cfg) => draw_steep_shallow_params(ui, cfg),
        OperationConfig::RampFinish(cfg) => draw_ramp_finish_params(ui, cfg),
    }

    // Dressup modifications
    ui.add_space(8.0);
    ui.collapsing("Modifications", |ui| {
        draw_dressup_params(ui, &mut entry.dressups);
    });

    ui.add_space(12.0);

    // Generate button + status
    ui.horizontal(|ui| {
        if ui.add_enabled(!tools.is_empty(), egui::Button::new("Generate")).clicked() {
            events.push(AppEvent::GenerateToolpath(entry.id));
        }
        match &entry.status {
            ComputeStatus::Pending => { ui.label("Ready"); }
            ComputeStatus::Computing(p) => { ui.label(format!("Computing... {:.0}%", p * 100.0)); }
            ComputeStatus::Done => {
                ui.label(egui::RichText::new("Done").color(egui::Color32::from_rgb(100, 180, 100)));
            }
            ComputeStatus::Error(e) => {
                ui.label(egui::RichText::new(format!("Error: {e}")).color(egui::Color32::from_rgb(220, 80, 80)));
            }
        }
    });

    if let Some(result) = &entry.result {
        ui.add_space(4.0);
        ui.label(format!("Moves: {}", result.stats.move_count));
        ui.label(format!("Cutting: {:.0} mm", result.stats.cutting_distance));
        ui.label(format!("Rapid: {:.0} mm", result.stats.rapid_distance));
    }
}

// --- Parameter grid helpers ---

fn dv(ui: &mut egui::Ui, label: &str, val: &mut f64, suffix: &str, speed: f64, range: std::ops::RangeInclusive<f64>) {
    ui.label(label);
    ui.add(egui::DragValue::new(val).suffix(suffix).speed(speed).range(range));
    ui.end_row();
}

fn draw_pocket_params(ui: &mut egui::Ui, cfg: &mut PocketConfig) {
    egui::Grid::new("pocket_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        ui.label("Pattern:");
        egui::ComboBox::from_id_salt("pocket_pat").selected_text(match cfg.pattern {
            PocketPattern::Contour => "Contour", PocketPattern::Zigzag => "Zigzag",
        }).show_ui(ui, |ui| {
            ui.selectable_value(&mut cfg.pattern, PocketPattern::Contour, "Contour");
            ui.selectable_value(&mut cfg.pattern, PocketPattern::Zigzag, "Zigzag");
        });
        ui.end_row();
        dv(ui, "Stepover:", &mut cfg.stepover, " mm", 0.1, 0.05..=50.0);
        dv(ui, "Depth:", &mut cfg.depth, " mm", 0.1, 0.1..=100.0);
        dv(ui, "Depth/Pass:", &mut cfg.depth_per_pass, " mm", 0.1, 0.1..=50.0);
        dv(ui, "Feed Rate:", &mut cfg.feed_rate, " mm/min", 10.0, 1.0..=50000.0);
        dv(ui, "Plunge Rate:", &mut cfg.plunge_rate, " mm/min", 10.0, 1.0..=10000.0);
        ui.label("Climb:"); ui.checkbox(&mut cfg.climb, ""); ui.end_row();
        if cfg.pattern == PocketPattern::Zigzag {
            dv(ui, "Angle:", &mut cfg.angle, " deg", 1.0, 0.0..=360.0);
        }
    });
}

fn draw_profile_params(ui: &mut egui::Ui, cfg: &mut ProfileConfig) {
    egui::Grid::new("profile_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        ui.label("Side:");
        egui::ComboBox::from_id_salt("prof_side").selected_text(match cfg.side {
            ProfileSide::Outside => "Outside", ProfileSide::Inside => "Inside",
        }).show_ui(ui, |ui| {
            ui.selectable_value(&mut cfg.side, ProfileSide::Outside, "Outside");
            ui.selectable_value(&mut cfg.side, ProfileSide::Inside, "Inside");
        });
        ui.end_row();
        dv(ui, "Depth:", &mut cfg.depth, " mm", 0.1, 0.1..=100.0);
        dv(ui, "Depth/Pass:", &mut cfg.depth_per_pass, " mm", 0.1, 0.1..=50.0);
        dv(ui, "Feed Rate:", &mut cfg.feed_rate, " mm/min", 10.0, 1.0..=50000.0);
        dv(ui, "Plunge Rate:", &mut cfg.plunge_rate, " mm/min", 10.0, 1.0..=10000.0);
        ui.label("Climb:"); ui.checkbox(&mut cfg.climb, ""); ui.end_row();
    });
    ui.add_space(8.0);
    ui.collapsing("Tabs", |ui| {
        egui::Grid::new("tab_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            ui.label("Count:");
            let mut count = cfg.tab_count as i32;
            if ui.add(egui::DragValue::new(&mut count).range(0..=20)).changed() {
                cfg.tab_count = count.max(0) as usize;
            }
            ui.end_row();
            if cfg.tab_count > 0 {
                dv(ui, "Width:", &mut cfg.tab_width, " mm", 0.5, 1.0..=50.0);
                dv(ui, "Height:", &mut cfg.tab_height, " mm", 0.5, 0.5..=20.0);
            }
        });
    });
}

fn draw_adaptive_params(ui: &mut egui::Ui, cfg: &mut AdaptiveConfig) {
    egui::Grid::new("adapt_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        dv(ui, "Stepover:", &mut cfg.stepover, " mm", 0.1, 0.05..=50.0);
        dv(ui, "Depth:", &mut cfg.depth, " mm", 0.1, 0.1..=100.0);
        dv(ui, "Depth/Pass:", &mut cfg.depth_per_pass, " mm", 0.1, 0.1..=50.0);
        dv(ui, "Feed Rate:", &mut cfg.feed_rate, " mm/min", 10.0, 1.0..=50000.0);
        dv(ui, "Plunge Rate:", &mut cfg.plunge_rate, " mm/min", 10.0, 1.0..=10000.0);
        dv(ui, "Tolerance:", &mut cfg.tolerance, " mm", 0.01, 0.01..=1.0);
        ui.label("Slot Clearing:"); ui.checkbox(&mut cfg.slot_clearing, ""); ui.end_row();
        dv(ui, "Min Cut Radius:", &mut cfg.min_cutting_radius, " mm", 0.1, 0.0..=50.0);
    });
}

fn draw_vcarve_params(ui: &mut egui::Ui, cfg: &mut VCarveConfig) {
    ui.label(egui::RichText::new("Requires V-Bit tool").italics().color(egui::Color32::from_rgb(150, 140, 110)));
    egui::Grid::new("vcarve_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        dv(ui, "Max Depth:", &mut cfg.max_depth, " mm", 0.1, 0.1..=50.0);
        dv(ui, "Stepover:", &mut cfg.stepover, " mm", 0.05, 0.01..=10.0);
        dv(ui, "Feed Rate:", &mut cfg.feed_rate, " mm/min", 10.0, 1.0..=50000.0);
        dv(ui, "Plunge Rate:", &mut cfg.plunge_rate, " mm/min", 10.0, 1.0..=10000.0);
        dv(ui, "Tolerance:", &mut cfg.tolerance, " mm", 0.01, 0.01..=1.0);
    });
}

fn draw_rest_params(
    ui: &mut egui::Ui,
    cfg: &mut RestConfig,
    tools: &[(crate::state::job::ToolId, String, f64)],
) {
    egui::Grid::new("rest_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        ui.label("Previous Tool:");
        let prev_label = cfg.prev_tool_id
            .and_then(|pid| tools.iter().find(|(id, _, _)| *id == pid))
            .map(|(_, s, _)| s.as_str())
            .unwrap_or("(select)");
        egui::ComboBox::from_id_salt("rest_prev_tool").selected_text(prev_label).show_ui(ui, |ui| {
            for (id, name, _) in tools {
                let selected = cfg.prev_tool_id == Some(*id);
                if ui.selectable_label(selected, name.as_str()).clicked() {
                    cfg.prev_tool_id = Some(*id);
                }
            }
        });
        ui.end_row();
        dv(ui, "Stepover:", &mut cfg.stepover, " mm", 0.1, 0.05..=50.0);
        dv(ui, "Depth:", &mut cfg.depth, " mm", 0.1, 0.1..=100.0);
        dv(ui, "Depth/Pass:", &mut cfg.depth_per_pass, " mm", 0.1, 0.1..=50.0);
        dv(ui, "Feed Rate:", &mut cfg.feed_rate, " mm/min", 10.0, 1.0..=50000.0);
        dv(ui, "Plunge Rate:", &mut cfg.plunge_rate, " mm/min", 10.0, 1.0..=10000.0);
        dv(ui, "Angle:", &mut cfg.angle, " deg", 1.0, 0.0..=360.0);
    });
}

fn draw_inlay_params(ui: &mut egui::Ui, cfg: &mut InlayConfig) {
    ui.label(egui::RichText::new("Requires V-Bit tool").italics().color(egui::Color32::from_rgb(150, 140, 110)));
    egui::Grid::new("inlay_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        dv(ui, "Pocket Depth:", &mut cfg.pocket_depth, " mm", 0.1, 0.1..=50.0);
        dv(ui, "Glue Gap:", &mut cfg.glue_gap, " mm", 0.01, 0.0..=2.0);
        dv(ui, "Flat Depth:", &mut cfg.flat_depth, " mm", 0.1, 0.0..=20.0);
        dv(ui, "Boundary Offset:", &mut cfg.boundary_offset, " mm", 0.05, 0.0..=10.0);
        dv(ui, "Stepover:", &mut cfg.stepover, " mm", 0.1, 0.05..=50.0);
        dv(ui, "Flat Tool Radius:", &mut cfg.flat_tool_radius, " mm", 0.1, 0.1..=50.0);
        dv(ui, "Feed Rate:", &mut cfg.feed_rate, " mm/min", 10.0, 1.0..=50000.0);
        dv(ui, "Plunge Rate:", &mut cfg.plunge_rate, " mm/min", 10.0, 1.0..=10000.0);
        dv(ui, "Tolerance:", &mut cfg.tolerance, " mm", 0.01, 0.01..=1.0);
    });
}

fn draw_zigzag_params(ui: &mut egui::Ui, cfg: &mut ZigzagConfig) {
    egui::Grid::new("zigzag_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        dv(ui, "Stepover:", &mut cfg.stepover, " mm", 0.1, 0.05..=50.0);
        dv(ui, "Depth:", &mut cfg.depth, " mm", 0.1, 0.1..=100.0);
        dv(ui, "Depth/Pass:", &mut cfg.depth_per_pass, " mm", 0.1, 0.1..=50.0);
        dv(ui, "Feed Rate:", &mut cfg.feed_rate, " mm/min", 10.0, 1.0..=50000.0);
        dv(ui, "Plunge Rate:", &mut cfg.plunge_rate, " mm/min", 10.0, 1.0..=10000.0);
        dv(ui, "Angle:", &mut cfg.angle, " deg", 1.0, 0.0..=360.0);
    });
}

// ── 3D operation parameters ──────────────────────────────────────────────

fn draw_dropcutter_params(ui: &mut egui::Ui, cfg: &mut DropCutterConfig) {
    egui::Grid::new("dc_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        dv(ui, "Stepover:", &mut cfg.stepover, " mm", 0.1, 0.05..=50.0);
        dv(ui, "Feed Rate:", &mut cfg.feed_rate, " mm/min", 10.0, 1.0..=50000.0);
        dv(ui, "Plunge Rate:", &mut cfg.plunge_rate, " mm/min", 10.0, 1.0..=10000.0);
        dv(ui, "Min Z:", &mut cfg.min_z, " mm", 0.5, -500.0..=0.0);
    });
}

fn draw_adaptive3d_params(ui: &mut egui::Ui, cfg: &mut Adaptive3dConfig) {
    egui::Grid::new("a3d_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        dv(ui, "Stepover:", &mut cfg.stepover, " mm", 0.1, 0.05..=50.0);
        dv(ui, "Depth/Pass:", &mut cfg.depth_per_pass, " mm", 0.1, 0.1..=50.0);
        dv(ui, "Stock to Leave:", &mut cfg.stock_to_leave, " mm", 0.05, 0.0..=10.0);
        dv(ui, "Stock Top Z:", &mut cfg.stock_top_z, " mm", 0.5, -100.0..=200.0);
        dv(ui, "Feed Rate:", &mut cfg.feed_rate, " mm/min", 10.0, 1.0..=50000.0);
        dv(ui, "Plunge Rate:", &mut cfg.plunge_rate, " mm/min", 10.0, 1.0..=10000.0);
        dv(ui, "Tolerance:", &mut cfg.tolerance, " mm", 0.01, 0.01..=1.0);
        dv(ui, "Min Cut Radius:", &mut cfg.min_cutting_radius, " mm", 0.1, 0.0..=50.0);
        ui.label("Entry Style:");
        egui::ComboBox::from_id_salt("a3d_entry").selected_text(match cfg.entry_style {
            EntryStyle::Plunge => "Plunge", EntryStyle::Helix => "Helix", EntryStyle::Ramp => "Ramp",
        }).show_ui(ui, |ui| {
            ui.selectable_value(&mut cfg.entry_style, EntryStyle::Plunge, "Plunge");
            ui.selectable_value(&mut cfg.entry_style, EntryStyle::Helix, "Helix");
            ui.selectable_value(&mut cfg.entry_style, EntryStyle::Ramp, "Ramp");
        });
        ui.end_row();
        dv(ui, "Fine Stepdown:", &mut cfg.fine_stepdown, " mm", 0.1, 0.0..=10.0);
        ui.label("Detect Flat:"); ui.checkbox(&mut cfg.detect_flat_areas, ""); ui.end_row();
        ui.label("Ordering:");
        egui::ComboBox::from_id_salt("a3d_ord").selected_text(match cfg.region_ordering {
            RegionOrdering::Global => "Global", RegionOrdering::ByArea => "By Area",
        }).show_ui(ui, |ui| {
            ui.selectable_value(&mut cfg.region_ordering, RegionOrdering::Global, "Global");
            ui.selectable_value(&mut cfg.region_ordering, RegionOrdering::ByArea, "By Area");
        });
        ui.end_row();
    });
}

fn draw_waterline_params(ui: &mut egui::Ui, cfg: &mut WaterlineConfig) {
    egui::Grid::new("wl_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        dv(ui, "Z Step:", &mut cfg.z_step, " mm", 0.1, 0.05..=20.0);
        dv(ui, "Sampling:", &mut cfg.sampling, " mm", 0.1, 0.1..=5.0);
        dv(ui, "Start Z:", &mut cfg.start_z, " mm", 0.5, -200.0..=200.0);
        dv(ui, "Final Z:", &mut cfg.final_z, " mm", 0.5, -200.0..=200.0);
        dv(ui, "Feed Rate:", &mut cfg.feed_rate, " mm/min", 10.0, 1.0..=50000.0);
        dv(ui, "Plunge Rate:", &mut cfg.plunge_rate, " mm/min", 10.0, 1.0..=10000.0);
    });
}

fn draw_pencil_params(ui: &mut egui::Ui, cfg: &mut PencilConfig) {
    egui::Grid::new("pen_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        dv(ui, "Bitangency Angle:", &mut cfg.bitangency_angle, " deg", 1.0, 90.0..=180.0);
        dv(ui, "Min Cut Length:", &mut cfg.min_cut_length, " mm", 0.5, 0.5..=50.0);
        dv(ui, "Hookup Distance:", &mut cfg.hookup_distance, " mm", 0.5, 0.5..=50.0);
        ui.label("Offset Passes:");
        let mut n = cfg.num_offset_passes as i32;
        if ui.add(egui::DragValue::new(&mut n).range(0..=10)).changed() { cfg.num_offset_passes = n.max(0) as usize; }
        ui.end_row();
        dv(ui, "Offset Stepover:", &mut cfg.offset_stepover, " mm", 0.1, 0.05..=10.0);
        dv(ui, "Sampling:", &mut cfg.sampling, " mm", 0.1, 0.1..=5.0);
        dv(ui, "Feed Rate:", &mut cfg.feed_rate, " mm/min", 10.0, 1.0..=50000.0);
        dv(ui, "Plunge Rate:", &mut cfg.plunge_rate, " mm/min", 10.0, 1.0..=10000.0);
        dv(ui, "Stock to Leave:", &mut cfg.stock_to_leave, " mm", 0.05, 0.0..=10.0);
    });
}

fn draw_scallop_params(ui: &mut egui::Ui, cfg: &mut ScallopConfig) {
    egui::Grid::new("sc_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        dv(ui, "Scallop Height:", &mut cfg.scallop_height, " mm", 0.01, 0.01..=2.0);
        dv(ui, "Tolerance:", &mut cfg.tolerance, " mm", 0.01, 0.01..=1.0);
        ui.label("Direction:");
        egui::ComboBox::from_id_salt("sc_dir").selected_text(match cfg.direction {
            ScallopDirection::OutsideIn => "Outside In", ScallopDirection::InsideOut => "Inside Out",
        }).show_ui(ui, |ui| {
            ui.selectable_value(&mut cfg.direction, ScallopDirection::OutsideIn, "Outside In");
            ui.selectable_value(&mut cfg.direction, ScallopDirection::InsideOut, "Inside Out");
        });
        ui.end_row();
        ui.label("Continuous:"); ui.checkbox(&mut cfg.continuous, ""); ui.end_row();
        dv(ui, "Slope From:", &mut cfg.slope_from, " deg", 1.0, 0.0..=90.0);
        dv(ui, "Slope To:", &mut cfg.slope_to, " deg", 1.0, 0.0..=90.0);
        dv(ui, "Feed Rate:", &mut cfg.feed_rate, " mm/min", 10.0, 1.0..=50000.0);
        dv(ui, "Plunge Rate:", &mut cfg.plunge_rate, " mm/min", 10.0, 1.0..=10000.0);
        dv(ui, "Stock to Leave:", &mut cfg.stock_to_leave, " mm", 0.05, 0.0..=10.0);
    });
}

fn draw_steep_shallow_params(ui: &mut egui::Ui, cfg: &mut SteepShallowConfig) {
    egui::Grid::new("ss_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        dv(ui, "Threshold Angle:", &mut cfg.threshold_angle, " deg", 1.0, 10.0..=80.0);
        dv(ui, "Overlap:", &mut cfg.overlap_distance, " mm", 0.1, 0.0..=10.0);
        dv(ui, "Wall Clearance:", &mut cfg.wall_clearance, " mm", 0.1, 0.0..=10.0);
        ui.label("Steep First:"); ui.checkbox(&mut cfg.steep_first, ""); ui.end_row();
        dv(ui, "Stepover:", &mut cfg.stepover, " mm", 0.1, 0.05..=50.0);
        dv(ui, "Z Step:", &mut cfg.z_step, " mm", 0.1, 0.05..=20.0);
        dv(ui, "Feed Rate:", &mut cfg.feed_rate, " mm/min", 10.0, 1.0..=50000.0);
        dv(ui, "Plunge Rate:", &mut cfg.plunge_rate, " mm/min", 10.0, 1.0..=10000.0);
        dv(ui, "Sampling:", &mut cfg.sampling, " mm", 0.1, 0.1..=5.0);
        dv(ui, "Stock to Leave:", &mut cfg.stock_to_leave, " mm", 0.05, 0.0..=10.0);
        dv(ui, "Tolerance:", &mut cfg.tolerance, " mm", 0.01, 0.01..=1.0);
    });
}

fn draw_ramp_finish_params(ui: &mut egui::Ui, cfg: &mut RampFinishConfig) {
    egui::Grid::new("rf_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        dv(ui, "Max Stepdown:", &mut cfg.max_stepdown, " mm", 0.1, 0.05..=10.0);
        dv(ui, "Slope From:", &mut cfg.slope_from, " deg", 1.0, 0.0..=90.0);
        dv(ui, "Slope To:", &mut cfg.slope_to, " deg", 1.0, 0.0..=90.0);
        ui.label("Direction:");
        egui::ComboBox::from_id_salt("rf_dir").selected_text(match cfg.direction {
            CutDirection::Climb => "Climb", CutDirection::Conventional => "Conventional",
            CutDirection::BothWays => "Both Ways",
        }).show_ui(ui, |ui| {
            ui.selectable_value(&mut cfg.direction, CutDirection::Climb, "Climb");
            ui.selectable_value(&mut cfg.direction, CutDirection::Conventional, "Conventional");
            ui.selectable_value(&mut cfg.direction, CutDirection::BothWays, "Both Ways");
        });
        ui.end_row();
        ui.label("Bottom Up:"); ui.checkbox(&mut cfg.order_bottom_up, ""); ui.end_row();
        dv(ui, "Feed Rate:", &mut cfg.feed_rate, " mm/min", 10.0, 1.0..=50000.0);
        dv(ui, "Plunge Rate:", &mut cfg.plunge_rate, " mm/min", 10.0, 1.0..=10000.0);
        dv(ui, "Sampling:", &mut cfg.sampling, " mm", 0.1, 0.1..=5.0);
        dv(ui, "Stock to Leave:", &mut cfg.stock_to_leave, " mm", 0.05, 0.0..=10.0);
        dv(ui, "Tolerance:", &mut cfg.tolerance, " mm", 0.01, 0.01..=1.0);
    });
}

// ── Dressup configuration ────────────────────────────────────────────────

fn draw_dressup_params(ui: &mut egui::Ui, cfg: &mut DressupConfig) {
    ui.horizontal(|ui| {
        ui.label("Entry Style:");
        egui::ComboBox::from_id_salt("dressup_entry")
            .selected_text(match cfg.entry_style {
                DressupEntryStyle::None => "None",
                DressupEntryStyle::Ramp => "Ramp",
                DressupEntryStyle::Helix => "Helix",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut cfg.entry_style, DressupEntryStyle::None, "None");
                ui.selectable_value(&mut cfg.entry_style, DressupEntryStyle::Ramp, "Ramp");
                ui.selectable_value(&mut cfg.entry_style, DressupEntryStyle::Helix, "Helix");
            });
    });
    match cfg.entry_style {
        DressupEntryStyle::Ramp => {
            egui::Grid::new("ramp_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                dv(ui, "  Max Angle:", &mut cfg.ramp_angle, " deg", 0.5, 0.5..=15.0);
            });
        }
        DressupEntryStyle::Helix => {
            egui::Grid::new("helix_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                dv(ui, "  Radius:", &mut cfg.helix_radius, " mm", 0.1, 0.5..=20.0);
                dv(ui, "  Pitch:", &mut cfg.helix_pitch, " mm", 0.1, 0.2..=10.0);
            });
        }
        DressupEntryStyle::None => {}
    }
    ui.add_space(4.0);
    ui.checkbox(&mut cfg.dogbone, "Dogbone overcuts");
    if cfg.dogbone {
        egui::Grid::new("dog_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            dv(ui, "  Max Angle:", &mut cfg.dogbone_angle, " deg", 1.0, 45.0..=135.0);
        });
    }
    ui.checkbox(&mut cfg.lead_in_out, "Lead-in / lead-out");
    if cfg.lead_in_out {
        egui::Grid::new("lead_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            dv(ui, "  Radius:", &mut cfg.lead_radius, " mm", 0.1, 0.5..=20.0);
        });
    }
    ui.checkbox(&mut cfg.link_moves, "Link moves (keep tool down)");
    if cfg.link_moves {
        egui::Grid::new("link_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            dv(ui, "  Max Distance:", &mut cfg.link_max_distance, " mm", 0.5, 1.0..=50.0);
            dv(ui, "  Feed Rate:", &mut cfg.link_feed_rate, " mm/min", 10.0, 50.0..=5000.0);
        });
    }
    ui.checkbox(&mut cfg.arc_fitting, "Arc fitting (G2/G3)");
    if cfg.arc_fitting {
        egui::Grid::new("arc_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            dv(ui, "  Tolerance:", &mut cfg.arc_tolerance, " mm", 0.01, 0.01..=0.5);
        });
    }
    ui.checkbox(&mut cfg.feed_optimization, "Feed rate optimization");
    if cfg.feed_optimization {
        egui::Grid::new("fopt_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            dv(ui, "  Max Rate:", &mut cfg.feed_max_rate, " mm/min", 50.0, 500.0..=20000.0);
            dv(ui, "  Ramp Rate:", &mut cfg.feed_ramp_rate, " mm/min/mm", 10.0, 10.0..=2000.0);
        });
    }
}
