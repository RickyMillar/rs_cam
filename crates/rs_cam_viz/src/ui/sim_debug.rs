use crate::state::simulation::ToolpathTraceAvailability;

/// The word a trace-availability badge carries, if it carries one.
///
/// UP4: this used to hand back a COLOUR as well, and the four it chose were
/// `(120, 210, 150)` green, `(210, 170, 90)` amber, `(220, 140, 90)` orange
/// and `(110, 170, 220)` blue — the verdict palette, spent on a CATEGORY.
/// `DESIGN_SPEC.md` §2.6 principle 1 forbids exactly that: "TRACE" in the
/// same green as a pass is the same defect as a vendor band in the same
/// green as a pass. Trace availability is generator-debug provenance and
/// carries no verdict at all, so the four are separated by their WORD.
pub fn trace_availability_badge(availability: ToolpathTraceAvailability) -> Option<&'static str> {
    match availability {
        ToolpathTraceAvailability::None => None,
        ToolpathTraceAvailability::Semantic => Some("SEM"),
        ToolpathTraceAvailability::Performance => Some("PERF"),
        ToolpathTraceAvailability::PerformanceAndSemantic => Some("TRACE"),
        ToolpathTraceAvailability::Partial => Some("PART"),
    }
}

pub fn draw_trace_badge(ui: &mut egui::Ui, availability: ToolpathTraceAvailability) {
    if let Some(label) = trace_availability_badge(availability) {
        // An OBSERVATION, per ruling R15: neutral ground, muted text, no
        // glyph. It cannot borrow a verdict colour because it has none.
        egui::Frame::default()
            .fill(crate::ui::tokens::SURFACE_RAISED)
            .stroke(egui::Stroke::new(1.0_f32, crate::ui::tokens::BORDER))
            .inner_margin(egui::Margin::symmetric(
                crate::ui::tokens::SPACE_2 as i8,
                crate::ui::tokens::SPACE_1 as i8,
            ))
            .corner_radius(crate::ui::tokens::RADIUS_SM)
            .show(ui, |ui| {
                ui.label(
                    crate::ui::components::text::micro(label).color(crate::ui::tokens::TEXT_MUTED),
                );
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

/// The colour of one semantic block in the generator-trace ribbon.
///
/// UP4: these 26 kinds are a CATEGORY and they carried 26 hand-picked hues —
/// `(110,210,140)` green on `Pass`, `Raster`, `Row`, `FinishPass` and
/// `OffsetPass`, `(210,120,120)` and `(240,130,110)` red on `SlotClearing`
/// and `ForcedClear`, `(230,180,90)` amber on `Entry`, `Hole` and `Ray`.
/// `DESIGN_SPEC.md` §2.6 principle 1 forbids exactly that: a green `Pass`
/// block beside a red `ForcedClear` block reads as "passed, failed" when
/// neither block is a judgement at all — both are just structure the
/// generator emitted.
///
/// They now walk [`SPAN_SCALE`], the six-step cool ramp that exists for a
/// category, in declaration order, so no two neighbouring kinds share a step.
#[allow(clippy::indexing_slicing)]
// SAFETY: every index below is a literal in `0..6` and `SPAN_SCALE` has six
// entries, so each one is in bounds at compile time.
pub fn semantic_kind_color(
    kind: &rs_cam_core::semantic_trace::ToolpathSemanticKind,
) -> egui::Color32 {
    use crate::ui::tokens::SPAN_SCALE;
    use rs_cam_core::semantic_trace::ToolpathSemanticKind;

    match kind {
        ToolpathSemanticKind::Operation => SPAN_SCALE[0],
        ToolpathSemanticKind::DepthLevel => SPAN_SCALE[1],
        ToolpathSemanticKind::Region => SPAN_SCALE[2],
        ToolpathSemanticKind::Pass => SPAN_SCALE[3],
        ToolpathSemanticKind::Entry => SPAN_SCALE[4],
        ToolpathSemanticKind::SlotClearing => SPAN_SCALE[5],
        ToolpathSemanticKind::Cleanup => SPAN_SCALE[0],
        ToolpathSemanticKind::ForcedClear => SPAN_SCALE[1],
        ToolpathSemanticKind::Contour => SPAN_SCALE[2],
        ToolpathSemanticKind::Raster => SPAN_SCALE[3],
        ToolpathSemanticKind::Row => SPAN_SCALE[4],
        ToolpathSemanticKind::Slice => SPAN_SCALE[5],
        ToolpathSemanticKind::Hole => SPAN_SCALE[0],
        ToolpathSemanticKind::Cycle => SPAN_SCALE[1],
        ToolpathSemanticKind::Chain => SPAN_SCALE[2],
        ToolpathSemanticKind::Band => SPAN_SCALE[3],
        ToolpathSemanticKind::Ramp => SPAN_SCALE[4],
        ToolpathSemanticKind::Ring => SPAN_SCALE[5],
        ToolpathSemanticKind::Ray => SPAN_SCALE[0],
        ToolpathSemanticKind::Curve => SPAN_SCALE[1],
        ToolpathSemanticKind::Dressup => SPAN_SCALE[2],
        ToolpathSemanticKind::FinishPass => SPAN_SCALE[3],
        ToolpathSemanticKind::OffsetPass => SPAN_SCALE[4],
        ToolpathSemanticKind::Centerline => SPAN_SCALE[5],
        ToolpathSemanticKind::BoundaryClip => SPAN_SCALE[0],
        ToolpathSemanticKind::Optimization => SPAN_SCALE[1],
    }
}

pub fn format_json_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_owned(),
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

pub fn json_f64(value: Option<&serde_json::Value>) -> Option<f64> {
    value.and_then(|value| match value {
        serde_json::Value::Number(number) => number.as_f64(),
        _ => None,
    })
}

pub fn debug_span_math_summary(kind: &str) -> Option<&'static str> {
    match kind {
        "surface_heightmap" => {
            Some("Drop-cutter sampling over a grid to approximate the model surface.")
        }
        "z_level_plan" => Some(
            "Build Z levels from stepdown, optional shelf detection, and optional fine-stepdown expansion.",
        ),
        "region_detect" => {
            Some("Flood-fill connected remaining-material regions on the heightmap.")
        }
        "z_level" => Some("Per-level orchestration and remaining-material accounting."),
        "pre_stamp" => Some("Pre-stamp thin steep-wall bands to avoid low-yield recuts."),
        "adaptive_pass" => Some(
            "Constant-engagement stepping: direction search, local material checks, and tool stamping.",
        ),
        "entry_search" => {
            Some("Search the remaining-material grid for the next viable entry point.")
        }
        "preflight" => Some("Cheap direction-search preflight from the candidate entry point."),
        "widen_band" => {
            Some("Stamp offset bands around productive passes to widen the cleared channel.")
        }
        "waterline_cleanup" => {
            Some("Contour steep walls to clean boundary material missed by the adaptive spiral.")
        }
        "prepare_input" => {
            Some("Precompute and transform operation inputs before toolpath generation.")
        }
        "core_generate" => Some("The main toolpath kernel for the selected operation."),
        "dressups" => {
            Some("Apply entries, links, leads, clips, and other post-generation modifiers.")
        }
        "arc_fit" => Some("Fit linear segments into XY arcs where possible."),
        "feed_optimization" => {
            Some("Estimate cutter engagement and retime feed rates against stock.")
        }
        "final_stats" => Some("Compute final toolpath statistics and summarize the result."),
        _ => None,
    }
}
