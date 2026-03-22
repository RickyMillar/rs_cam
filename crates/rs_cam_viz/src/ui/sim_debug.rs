use crate::state::simulation::ToolpathTraceAvailability;

pub fn trace_availability_badge(
    availability: ToolpathTraceAvailability,
) -> Option<(&'static str, egui::Color32)> {
    match availability {
        ToolpathTraceAvailability::None => None,
        ToolpathTraceAvailability::Semantic => {
            Some(("SEM", egui::Color32::from_rgb(110, 170, 220)))
        }
        ToolpathTraceAvailability::Performance => {
            Some(("PERF", egui::Color32::from_rgb(210, 170, 90)))
        }
        ToolpathTraceAvailability::PerformanceAndSemantic => {
            Some(("TRACE", egui::Color32::from_rgb(120, 210, 150)))
        }
        ToolpathTraceAvailability::Partial => Some(("PART", egui::Color32::from_rgb(220, 140, 90))),
    }
}

pub fn draw_trace_badge(ui: &mut egui::Ui, availability: ToolpathTraceAvailability) {
    if let Some((label, color)) = trace_availability_badge(availability) {
        egui::Frame::default()
            .fill(color.linear_multiply(0.12))
            .stroke(egui::Stroke::new(1.0, color.linear_multiply(0.75)))
            .inner_margin(egui::Margin::symmetric(4.0, 1.0))
            .rounding(3.0)
            .show(ui, |ui| {
                ui.label(egui::RichText::new(label).small().strong().color(color));
            });
    }
}

pub fn semantic_kind_label(
    kind: &rs_cam_core::semantic_trace::ToolpathSemanticKind,
) -> &'static str {
    use rs_cam_core::semantic_trace::ToolpathSemanticKind;

    match kind {
        ToolpathSemanticKind::Operation => "Operation",
        ToolpathSemanticKind::DepthLevel => "Depth",
        ToolpathSemanticKind::Region => "Region",
        ToolpathSemanticKind::Pass => "Pass",
        ToolpathSemanticKind::Entry => "Entry",
        ToolpathSemanticKind::SlotClearing => "Slot clearing",
        ToolpathSemanticKind::Cleanup => "Cleanup",
        ToolpathSemanticKind::ForcedClear => "Forced clear",
        ToolpathSemanticKind::Contour => "Contour",
        ToolpathSemanticKind::Raster => "Raster",
        ToolpathSemanticKind::Row => "Row",
        ToolpathSemanticKind::Slice => "Slice",
        ToolpathSemanticKind::Hole => "Hole",
        ToolpathSemanticKind::Cycle => "Cycle",
        ToolpathSemanticKind::Chain => "Chain",
        ToolpathSemanticKind::Band => "Band",
        ToolpathSemanticKind::Ramp => "Ramp",
        ToolpathSemanticKind::Ring => "Ring",
        ToolpathSemanticKind::Ray => "Ray",
        ToolpathSemanticKind::Curve => "Curve",
        ToolpathSemanticKind::Dressup => "Dressup",
        ToolpathSemanticKind::FinishPass => "Finish pass",
        ToolpathSemanticKind::OffsetPass => "Offset pass",
        ToolpathSemanticKind::Centerline => "Centerline",
        ToolpathSemanticKind::BoundaryClip => "Boundary clip",
        ToolpathSemanticKind::Optimization => "Optimization",
    }
}

pub fn semantic_kind_color(
    kind: &rs_cam_core::semantic_trace::ToolpathSemanticKind,
) -> egui::Color32 {
    use rs_cam_core::semantic_trace::ToolpathSemanticKind;

    match kind {
        ToolpathSemanticKind::Operation => egui::Color32::from_rgb(160, 170, 210),
        ToolpathSemanticKind::DepthLevel => egui::Color32::from_rgb(120, 150, 230),
        ToolpathSemanticKind::Region => egui::Color32::from_rgb(150, 120, 220),
        ToolpathSemanticKind::Pass => egui::Color32::from_rgb(110, 210, 140),
        ToolpathSemanticKind::Entry => egui::Color32::from_rgb(230, 180, 90),
        ToolpathSemanticKind::SlotClearing => egui::Color32::from_rgb(210, 120, 120),
        ToolpathSemanticKind::Cleanup => egui::Color32::from_rgb(110, 200, 210),
        ToolpathSemanticKind::ForcedClear => egui::Color32::from_rgb(240, 130, 110),
        ToolpathSemanticKind::Contour => egui::Color32::from_rgb(130, 200, 220),
        ToolpathSemanticKind::Raster => egui::Color32::from_rgb(120, 200, 130),
        ToolpathSemanticKind::Row => egui::Color32::from_rgb(110, 190, 140),
        ToolpathSemanticKind::Slice => egui::Color32::from_rgb(140, 200, 240),
        ToolpathSemanticKind::Hole => egui::Color32::from_rgb(220, 170, 100),
        ToolpathSemanticKind::Cycle => egui::Color32::from_rgb(220, 140, 110),
        ToolpathSemanticKind::Chain => egui::Color32::from_rgb(150, 190, 230),
        ToolpathSemanticKind::Band => egui::Color32::from_rgb(100, 180, 180),
        ToolpathSemanticKind::Ramp => egui::Color32::from_rgb(220, 150, 110),
        ToolpathSemanticKind::Ring => egui::Color32::from_rgb(180, 150, 230),
        ToolpathSemanticKind::Ray => egui::Color32::from_rgb(240, 190, 100),
        ToolpathSemanticKind::Curve => egui::Color32::from_rgb(180, 210, 110),
        ToolpathSemanticKind::Dressup => egui::Color32::from_rgb(220, 120, 200),
        ToolpathSemanticKind::FinishPass => egui::Color32::from_rgb(110, 230, 170),
        ToolpathSemanticKind::OffsetPass => egui::Color32::from_rgb(90, 200, 180),
        ToolpathSemanticKind::Centerline => egui::Color32::from_rgb(200, 210, 120),
        ToolpathSemanticKind::BoundaryClip => egui::Color32::from_rgb(230, 130, 180),
        ToolpathSemanticKind::Optimization => egui::Color32::from_rgb(250, 200, 120),
    }
}

pub fn format_json_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(v) => v.to_string(),
        serde_json::Value::Number(v) => v.to_string(),
        serde_json::Value::String(v) => v.clone(),
        serde_json::Value::Array(values) => {
            let joined = values
                .iter()
                .map(format_json_value)
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{joined}]")
        }
        serde_json::Value::Object(map) => {
            let joined = map
                .iter()
                .map(|(key, value)| format!("{key}: {}", format_json_value(value)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{{joined}}}")
        }
    }
}
