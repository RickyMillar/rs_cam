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
            stock::draw(ui, &mut state.job.stock, events);
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
    }

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
