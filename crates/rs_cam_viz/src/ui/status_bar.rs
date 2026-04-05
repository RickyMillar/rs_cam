use crate::compute::{ComputeLane, LaneSnapshot, LaneState};
use crate::state::AppState;
use crate::ui::automation;
use crate::ui::theme;

pub fn draw(
    ui: &mut egui::Ui,
    state: &AppState,
    collision_count: usize,
    lanes: &[LaneSnapshot; 2],
) {
    ui.horizontal(|ui| {
        let model_count = state.job.models.len();
        let tri_count: usize = state
            .job
            .models
            .iter()
            .filter_map(|model| model.mesh.as_ref().map(|mesh| mesh.triangles.len()))
            .sum();

        if model_count > 0 {
            ui.label(format!(
                "Models: {}  |  Triangles: {}",
                model_count, tri_count
            ));
        } else {
            ui.label("Ready");
        }

        let tp_done = state
            .job
            .all_toolpaths()
            .filter(|toolpath| {
                matches!(toolpath.status, crate::state::toolpath::ComputeStatus::Done)
            })
            .count();
        if tp_done > 0 {
            ui.separator();
            ui.label(format!(
                "Toolpaths: {}/{}",
                tp_done,
                state.job.toolpath_count()
            ));
        }

        for lane in lanes {
            if matches!(lane.state, LaneState::Idle) && lane.queue_depth == 0 {
                continue;
            }
            ui.separator();
            let label = lane_chip_label(lane);
            let tooltip = match lane.lane {
                ComputeLane::Toolpath => "Toolpath compute lane",
                ComputeLane::Analysis => "Analysis compute lane",
            };
            let response = ui
                .label(egui::RichText::new(&label).color(match lane.state {
                    LaneState::Idle => theme::LANE_IDLE,
                    LaneState::Queued => theme::LANE_QUEUED,
                    LaneState::Running => theme::LANE_RUNNING,
                    LaneState::Cancelling => theme::LANE_CANCELLING,
                }))
                .on_hover_text(tooltip);
            automation::record(
                ui,
                match lane.lane {
                    ComputeLane::Toolpath => "status_lane_toolpath",
                    ComputeLane::Analysis => "status_lane_analysis",
                },
                &response,
                &label,
            );
        }

        if state.simulation.has_results() {
            ui.separator();
            ui.label(egui::RichText::new("SIM").color(theme::SUCCESS))
                .on_hover_text("Simulation results available");
        }

        if collision_count > 0 {
            ui.separator();
            ui.label(
                egui::RichText::new(format!("{} collisions", collision_count)).color(theme::ERROR),
            );
        }

        if state.job.dirty {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new("Modified")
                        .italics()
                        .color(egui::Color32::from_rgb(140, 140, 100)),
                );
            });
        }
    });
}

fn lane_chip_label(lane: &LaneSnapshot) -> String {
    let prefix = match lane.lane {
        ComputeLane::Toolpath => "TP",
        ComputeLane::Analysis => "AN",
    };
    let state = match lane.state {
        LaneState::Idle => "idle",
        LaneState::Queued => "queued",
        LaneState::Running => "running",
        LaneState::Cancelling => "cancelling",
    };
    let mut label = format!("{prefix} {state}");
    if lane.queue_depth > 0 {
        label.push_str(&format!(" · q{}", lane.queue_depth));
    }
    if let Some(job) = &lane.current_job {
        label.push_str(&format!(" · {job}"));
    }
    if let Some(elapsed) = lane.elapsed() {
        label.push_str(&format!(" · {:.1}s", elapsed.as_secs_f32()));
    }
    label
}
