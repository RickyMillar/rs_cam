//! One declarative row per operation: its editor, its shape diagram and its
//! extra validation.
//!
//! # What the finding was (UI-05)
//!
//! `OperationConfig` was re-matched in five UI places beside the core
//! `for_each_op!` list: the editor dispatch and the diagram fallback in
//! `toolpath_panel.rs`, two identical arms in `validate.rs`,
//! `shape_diagrams.rs` and `pills.rs`. Four of those are exhaustive, so the
//! compiler catches a new operation. The diagram fallback was not: it ended
//! `_ => {}`, so a new operation drew NO diagram and nothing said so.
//!
//! # What this module is
//!
//! Core solved the same problem once. `for_each_op!` generates
//! `OperationType`, `ALL`, `category()`, `name()` and the rest from one
//! 24-row list, and `registry_entry()` will not compile without a row. That
//! macro is not exported, so the viz side states its own table — and
//! `operations_registry` holds it against `OperationType::ALL`, the way
//! `overlays_registry` holds the overlay list.
//!
//! An operation that shows no diagram now SAYS SO, with the reason, in
//! [`OpDiagram::None`]. That is the silent arm made visible.
//!
//! # The row is selected by the operation's own type
//!
//! [`row`] looks the row up by `OperationConfig::op_type()`, so the
//! single-variant match inside each adapter below always takes its arm. The
//! adapters exist because the editors take the CONFIG, not the enum; they
//! are the same shape as core's `GenerateFn` adapters.

use rs_cam_core::compute::toolpath_stats::ClaimsReferenceFinding;

use super::validate::{DepthBeyondStock, ThroughCut};
use super::{
    StepoverPattern, draw_adaptive_params, draw_adaptive3d_params, draw_alignment_pin_drill_params,
    draw_chamfer_params, draw_drill_params, draw_dropcutter_params, draw_face_params,
    draw_horizontal_finish_params, draw_inlay_diagram, draw_inlay_params, draw_outline_diagram,
    draw_pencil_diagram, draw_pencil_params, draw_pocket_params, draw_point_set_diagram,
    draw_profile_params, draw_project_curve_params, draw_radial_diagram, draw_radial_finish_params,
    draw_ramp_finish_diagram, draw_ramp_finish_params, draw_rest_params, draw_scallop_params,
    draw_spiral_diagram, draw_spiral_finish_params, draw_steep_shallow_diagram,
    draw_steep_shallow_params, draw_stepover_diagram, draw_trace_params,
    draw_unified_finish_params, draw_vcarve_params, draw_waterline_params, draw_zigzag_params,
};
use crate::state::job::{ModelId, ToolId};
use crate::state::toolpath::{OperationConfig, OperationType, ProfileSide, StockSource};
use crate::ui::properties::pills::PillSuggestions;

/// Everything a per-operation editor may read or write, in one place.
///
/// UI-05: the editors took between 3 and 6 arguments each, and Pencil and
/// UnifiedFinish took arguments no other editor did, so the dispatch could
/// not be a table. One context makes the signature uniform; a field only
/// that operation reads is still only read by that operation.
pub(in crate::ui::properties) struct OpDrawCtx<'a> {
    pub tools: &'a [(ToolId, String, f64)],
    pub models: &'a [(ModelId, String)],
    pub drill_layers: &'a [String],
    pub drill_targets: &'a [rs_cam_core::io::dxf_input::DrillTarget],
    pub pills: Option<&'a PillSuggestions<'a>>,
    pub depth_caution: Option<&'a DepthBeyondStock>,
    pub through_cut: Option<&'a ThroughCut>,
    /// The active tool's radius. Adaptive3d maps engagement to stepover
    /// with it.
    pub tool_radius: f64,
    /// What the LAST generation resolved `claims_reference` to.
    pub resolved_claims_reference: Option<ClaimsReferenceFinding>,
    /// READ ONLY. The Geometry tab's one "Start from" row writes this
    /// field (W2, G-STARTFROM); an editor only reads it. Pencil gates its
    /// analytic reference picker on it, Unified Finish gates its claims
    /// block.
    pub stock_source: StockSource,
}

/// What shape diagram an operation shows under its parameter grid.
pub enum OpDiagram {
    /// The stepover-pattern diagram, from `StepoverPattern::from_operation`.
    Stepover,
    /// A drawing of this operation's own.
    Draw(fn(&OperationConfig, &mut egui::Ui)),
    /// This operation shows no diagram, and says why.
    ///
    /// The reason is the point: the old `_ => {}` fallback could not tell a
    /// deliberate blank from a forgotten arm.
    None(&'static str),
}

impl OpDiagram {
    /// The stated reason this operation shows no diagram, or `None` when it
    /// shows one. `operations_registry` asserts the reason is never empty.
    pub fn none_reason(&self) -> Option<&'static str> {
        match self {
            Self::None(reason) => Some(reason),
            Self::Stepover | Self::Draw(_) => None,
        }
    }
}

/// The extra validation one operation adds beyond the shared geometry and
/// tool checks, or `None` when it adds none.
type ValidateFn = fn(&OperationConfig, &OpValidateCtx<'_>, &mut Vec<String>);

/// What a per-operation validation arm reads.
pub(in crate::ui::properties) struct OpValidateCtx<'a> {
    /// The chosen tool, as the validation context models it.
    pub tool: &'a super::validate::ValidationTool,
    pub ctx: &'a super::validate::ToolpathValidationContext,
    pub tp_id: rs_cam_core::ToolpathId,
    pub model_id: ModelId,
}

/// One operation's UI row.
pub struct OpUiRow {
    pub op: OperationType,
    /// The editor. Its context type is crate-internal, so the field is too;
    /// the struct literal still cannot be written without it, which is what
    /// the completeness sentry relies on.
    pub(in crate::ui::properties) draw: fn(&mut egui::Ui, &mut OperationConfig, &mut OpDrawCtx<'_>),
    pub diagram: OpDiagram,
    pub(in crate::ui::properties) validate: Option<ValidateFn>,
}

/// The row for one operation.
///
/// `OP_UI_ROWS` covers `OperationType::ALL` exactly once —
/// `operations_registry` is the sentry — so this cannot return `None` for a
/// live operation.
pub fn row(op: OperationType) -> &'static OpUiRow {
    OP_UI_ROWS
        .iter()
        .find(|r| r.op == op)
        .unwrap_or_else(|| unreachable!("every OperationType has a UI row (operations_registry)"))
}

/// Draw the operation's editor, then its diagram.
pub(in crate::ui::properties) fn draw_editor_and_diagram(
    ui: &mut egui::Ui,
    op: &mut OperationConfig,
    cx: &mut OpDrawCtx<'_>,
) {
    let entry = row(op.op_type());
    (entry.draw)(ui, op, cx);
    ui.add_space(6.0);
    match &entry.diagram {
        OpDiagram::Stepover => {
            if let Some(pattern) = StepoverPattern::from_operation(op) {
                draw_stepover_diagram(ui, &pattern);
            }
        }
        OpDiagram::Draw(f) => f(op, ui),
        OpDiagram::None(_) => {}
    }
}

/// The per-operation errors both validation entry points collect.
pub(in crate::ui::properties) fn op_errors(
    op: &OperationConfig,
    cx: &OpValidateCtx<'_>,
    errs: &mut Vec<String>,
) {
    if let Some(check) = row(op.op_type()).validate {
        check(op, cx, errs);
    }
}

// ── The editor adapters ─────────────────────────────────────────────────
//
// Each one binds the config out of the enum and calls the editor with the
// arguments that editor reads. `row` selects by `op_type()`, so the `if
// let` always takes its arm.

macro_rules! editor {
    ($name:ident, $variant:ident, |$ui:ident, $cfg:ident, $cx:ident| $body:expr) => {
        fn $name($ui: &mut egui::Ui, op: &mut OperationConfig, $cx: &mut OpDrawCtx<'_>) {
            if let OperationConfig::$variant($cfg) = op {
                $body;
            }
        }
    };
}

editor!(ed_face, Face, |ui, cfg, cx| draw_face_params(
    ui,
    cfg,
    cx.pills,
    cx.depth_caution
));
editor!(ed_pocket, Pocket, |ui, cfg, cx| draw_pocket_params(
    ui,
    cfg,
    cx.pills,
    cx.depth_caution
));
editor!(ed_profile, Profile, |ui, cfg, cx| draw_profile_params(
    ui,
    cfg,
    cx.pills,
    cx.depth_caution,
    cx.through_cut
));
editor!(ed_adaptive, Adaptive, |ui, cfg, cx| draw_adaptive_params(
    ui,
    cfg,
    cx.pills,
    cx.depth_caution
));
editor!(ed_vcarve, VCarve, |ui, cfg, cx| draw_vcarve_params(
    ui,
    cfg,
    cx.pills,
    cx.depth_caution
));
editor!(ed_rest, Rest, |ui, cfg, cx| draw_rest_params(
    ui,
    cfg,
    cx.tools,
    cx.pills,
    cx.depth_caution
));
editor!(ed_inlay, Inlay, |ui, cfg, cx| draw_inlay_params(
    ui,
    cfg,
    cx.pills,
    cx.depth_caution
));
editor!(ed_zigzag, Zigzag, |ui, cfg, cx| draw_zigzag_params(
    ui,
    cfg,
    cx.pills,
    cx.depth_caution
));
editor!(ed_trace, Trace, |ui, cfg, cx| draw_trace_params(
    ui,
    cfg,
    cx.pills,
    cx.depth_caution
));
editor!(ed_drill, Drill, |ui, cfg, cx| draw_drill_params(
    ui,
    cfg,
    cx.drill_layers,
    cx.drill_targets,
    cx.pills,
    cx.depth_caution
));
editor!(ed_chamfer, Chamfer, |ui, cfg, cx| draw_chamfer_params(
    ui,
    cfg,
    cx.pills,
    cx.depth_caution
));
editor!(ed_dropcutter, DropCutter, |ui, cfg, cx| {
    draw_dropcutter_params(ui, cfg, cx.pills);
});
editor!(ed_adaptive3d, Adaptive3d, |ui, cfg, cx| {
    draw_adaptive3d_params(ui, cfg, cx.tool_radius, cx.pills);
});
editor!(
    ed_waterline,
    Waterline,
    |ui, cfg, cx| draw_waterline_params(ui, cfg, cx.pills)
);
editor!(ed_pencil, Pencil, |ui, cfg, cx| draw_pencil_params(
    ui,
    cfg,
    cx.tools,
    cx.pills,
    cx.stock_source
));
editor!(ed_scallop, Scallop, |ui, cfg, cx| draw_scallop_params(
    ui, cfg, cx.pills
));
editor!(ed_unified_finish, UnifiedFinish, |ui, cfg, cx| {
    draw_unified_finish_params(
        ui,
        cfg,
        cx.pills,
        cx.resolved_claims_reference,
        cx.stock_source,
    );
});
editor!(ed_steep_shallow, SteepShallow, |ui, cfg, cx| {
    draw_steep_shallow_params(ui, cfg, cx.pills);
});
editor!(ed_ramp_finish, RampFinish, |ui, cfg, cx| {
    draw_ramp_finish_params(ui, cfg, cx.pills);
});
editor!(ed_spiral_finish, SpiralFinish, |ui, cfg, cx| {
    draw_spiral_finish_params(ui, cfg, cx.pills);
});
editor!(ed_radial_finish, RadialFinish, |ui, cfg, cx| {
    draw_radial_finish_params(ui, cfg, cx.pills);
});
editor!(ed_horizontal_finish, HorizontalFinish, |ui, cfg, cx| {
    draw_horizontal_finish_params(ui, cfg, cx.pills);
});
editor!(ed_project_curve, ProjectCurve, |ui, cfg, cx| {
    draw_project_curve_params(ui, cfg, cx.models, cx.pills);
});
editor!(ed_alignment_pin_drill, AlignmentPinDrill, |ui, cfg, cx| {
    draw_alignment_pin_drill_params(ui, cfg, cx.drill_layers, cx.drill_targets, cx.pills);
});

// ── The bespoke diagrams ────────────────────────────────────────────────

fn dia_profile(op: &OperationConfig, ui: &mut egui::Ui) {
    if let OperationConfig::Profile(cfg) = op {
        let side = match cfg.side {
            ProfileSide::Outside => "Outside",
            ProfileSide::Inside => "Inside",
        };
        draw_outline_diagram(ui, &format!("Profile ({side})"), Some(side));
    }
}

fn dia_chamfer(_op: &OperationConfig, ui: &mut egui::Ui) {
    draw_outline_diagram(ui, "Chamfer (edge contour)", None);
}

fn dia_trace(op: &OperationConfig, ui: &mut egui::Ui) {
    if let OperationConfig::Trace(cfg) = op {
        let comp = match cfg.compensation {
            crate::state::toolpath::TraceCompensation::None => None,
            crate::state::toolpath::TraceCompensation::Left => Some("Inside"),
            crate::state::toolpath::TraceCompensation::Right => Some("Outside"),
        };
        draw_outline_diagram(ui, "Trace", comp);
    }
}

fn dia_project_curve(_op: &OperationConfig, ui: &mut egui::Ui) {
    draw_outline_diagram(ui, "Project Curve", None);
}

fn dia_adaptive(op: &OperationConfig, ui: &mut egui::Ui) {
    if let OperationConfig::Adaptive(cfg) = op {
        draw_spiral_diagram(ui, cfg.stepover, true);
    }
}

fn dia_adaptive3d(op: &OperationConfig, ui: &mut egui::Ui) {
    if let OperationConfig::Adaptive3d(cfg) = op {
        draw_spiral_diagram(ui, cfg.stepover, true);
    }
}

fn dia_spiral_finish(op: &OperationConfig, ui: &mut egui::Ui) {
    if let OperationConfig::SpiralFinish(cfg) = op {
        let outward = cfg.direction == crate::state::toolpath::SpiralDirection::InsideOut;
        draw_spiral_diagram(ui, cfg.stepover, outward);
    }
}

fn dia_radial_finish(op: &OperationConfig, ui: &mut egui::Ui) {
    if let OperationConfig::RadialFinish(cfg) = op {
        draw_radial_diagram(ui, cfg.angular_step);
    }
}

fn dia_drill(_op: &OperationConfig, ui: &mut egui::Ui) {
    draw_point_set_diagram(ui, "Drill Points");
}

fn dia_pin_drill(_op: &OperationConfig, ui: &mut egui::Ui) {
    draw_point_set_diagram(ui, "Pin Drill Holes");
}

fn dia_pencil(op: &OperationConfig, ui: &mut egui::Ui) {
    if let OperationConfig::Pencil(cfg) = op {
        draw_pencil_diagram(ui, cfg.num_offset_passes, cfg.offset_stepover);
    }
}

fn dia_steep_shallow(op: &OperationConfig, ui: &mut egui::Ui) {
    if let OperationConfig::SteepShallow(cfg) = op {
        draw_steep_shallow_diagram(ui, cfg.threshold_angle);
    }
}

fn dia_ramp_finish(op: &OperationConfig, ui: &mut egui::Ui) {
    if let OperationConfig::RampFinish(cfg) = op {
        draw_ramp_finish_diagram(ui, cfg.max_stepdown);
    }
}

fn dia_inlay(op: &OperationConfig, ui: &mut egui::Ui) {
    if let OperationConfig::Inlay(cfg) = op {
        draw_inlay_diagram(ui, cfg.pocket_depth, cfg.glue_gap, cfg.flat_depth);
    }
}

// ── The validation arms ─────────────────────────────────────────────────
//
// `validate_toolpath` and `validate_toolpath_config` each carried a copy of
// this match — byte for byte, including the messages. One table, two
// callers.

fn val_stepover_under_diameter(
    op: &OperationConfig,
    cx: &OpValidateCtx<'_>,
    errs: &mut Vec<String>,
) {
    let stepover = match op {
        OperationConfig::Pocket(c) => c.stepover,
        OperationConfig::Adaptive(c) => c.stepover,
        _ => return,
    };
    if stepover >= cx.tool.diameter {
        errs.push("Stepover must be less than tool diameter".into());
    }
}

/// Step-ladder Phase 4: a bad `coarse_steps` ladder, or a ladder on a
/// strategy other than Contour Parallel, disables Generate with the
/// adapter's own sentence. The text comes from core, so the panel and the
/// generator cannot say two things.
fn val_adaptive3d_step_ladder(
    op: &OperationConfig,
    _cx: &OpValidateCtx<'_>,
    errs: &mut Vec<String>,
) {
    if let OperationConfig::Adaptive3d(cfg) = op
        && let Some(refusal) = rs_cam_core::compute::execute::adaptive3d_step_ladder_refusal(cfg)
    {
        errs.push(refusal);
    }
}

fn val_needs_v_bit(op: &OperationConfig, cx: &OpValidateCtx<'_>, errs: &mut Vec<String>) {
    if cx.tool.tool_type != crate::state::job::ToolType::VBit {
        errs.push(format!("{} requires a V-Bit tool", op.op_type().name()));
    }
}

fn val_rest(op: &OperationConfig, cx: &OpValidateCtx<'_>, errs: &mut Vec<String>) {
    let OperationConfig::Rest(c) = op else {
        return;
    };
    let Some(prev) = c.prev_tool_id else {
        errs.push("Previous tool not selected".into());
        return;
    };
    let prev_d = cx.ctx.tool_diameter(prev);
    if let Some(pd) = prev_d
        && pd <= cx.tool.diameter
    {
        errs.push("Previous tool must be larger than current tool".into());
    }
    if !super::validate::has_prior_rest_source(cx.ctx, cx.tp_id, cx.model_id, prev) {
        errs.push(
            "Rest machining requires an earlier enabled operation in the same setup using the previous tool on the same model"
                .into(),
        );
    }
}

fn val_drill(op: &OperationConfig, cx: &OpValidateCtx<'_>, errs: &mut Vec<String>) {
    let OperationConfig::Drill(c) = op else {
        return;
    };
    if let Some(msg) = super::validate::drill_targets_refusal(cx.ctx, cx.model_id, c) {
        errs.push(msg.to_owned());
    }
}

// ── The table ───────────────────────────────────────────────────────────

/// One row per operation. `operations_registry` holds it against
/// `OperationType::ALL`, so a new operation cannot ship without a row.
pub const OP_UI_ROWS: &[OpUiRow] = &[
    OpUiRow {
        op: OperationType::Face,
        draw: ed_face,
        diagram: OpDiagram::Stepover,
        validate: None,
    },
    OpUiRow {
        op: OperationType::Pocket,
        draw: ed_pocket,
        diagram: OpDiagram::Stepover,
        validate: Some(val_stepover_under_diameter),
    },
    OpUiRow {
        op: OperationType::Profile,
        draw: ed_profile,
        diagram: OpDiagram::Draw(dia_profile),
        validate: None,
    },
    OpUiRow {
        op: OperationType::Adaptive,
        draw: ed_adaptive,
        diagram: OpDiagram::Draw(dia_adaptive),
        validate: Some(val_stepover_under_diameter),
    },
    OpUiRow {
        op: OperationType::VCarve,
        draw: ed_vcarve,
        diagram: OpDiagram::Stepover,
        validate: Some(val_needs_v_bit),
    },
    OpUiRow {
        op: OperationType::Rest,
        draw: ed_rest,
        diagram: OpDiagram::Stepover,
        validate: Some(val_rest),
    },
    OpUiRow {
        op: OperationType::Inlay,
        draw: ed_inlay,
        diagram: OpDiagram::Draw(dia_inlay),
        validate: Some(val_needs_v_bit),
    },
    OpUiRow {
        op: OperationType::Zigzag,
        draw: ed_zigzag,
        diagram: OpDiagram::Stepover,
        validate: None,
    },
    OpUiRow {
        op: OperationType::Trace,
        draw: ed_trace,
        diagram: OpDiagram::Draw(dia_trace),
        validate: None,
    },
    OpUiRow {
        op: OperationType::Drill,
        draw: ed_drill,
        diagram: OpDiagram::Draw(dia_drill),
        validate: Some(val_drill),
    },
    OpUiRow {
        op: OperationType::Chamfer,
        draw: ed_chamfer,
        diagram: OpDiagram::Draw(dia_chamfer),
        validate: Some(val_needs_v_bit),
    },
    OpUiRow {
        op: OperationType::DropCutter,
        draw: ed_dropcutter,
        diagram: OpDiagram::Stepover,
        validate: None,
    },
    OpUiRow {
        op: OperationType::Adaptive3d,
        draw: ed_adaptive3d,
        diagram: OpDiagram::Draw(dia_adaptive3d),
        validate: Some(val_adaptive3d_step_ladder),
    },
    OpUiRow {
        op: OperationType::Waterline,
        draw: ed_waterline,
        diagram: OpDiagram::Stepover,
        validate: None,
    },
    OpUiRow {
        op: OperationType::Pencil,
        draw: ed_pencil,
        diagram: OpDiagram::Draw(dia_pencil),
        validate: None,
    },
    OpUiRow {
        op: OperationType::Scallop,
        draw: ed_scallop,
        diagram: OpDiagram::Stepover,
        validate: None,
    },
    OpUiRow {
        op: OperationType::UnifiedFinish,
        draw: ed_unified_finish,
        // UI-05: this operation drew NO diagram before the table, through
        // the `_ => {}` fallback. It plans three bands at once, so no one
        // stepover picture is honest about what it cuts.
        diagram: OpDiagram::None(
            "the unified planner runs three bands; one stepover picture would \
             misreport two of them",
        ),
        validate: None,
    },
    OpUiRow {
        op: OperationType::SteepShallow,
        draw: ed_steep_shallow,
        diagram: OpDiagram::Draw(dia_steep_shallow),
        validate: None,
    },
    OpUiRow {
        op: OperationType::RampFinish,
        draw: ed_ramp_finish,
        diagram: OpDiagram::Draw(dia_ramp_finish),
        validate: None,
    },
    OpUiRow {
        op: OperationType::SpiralFinish,
        draw: ed_spiral_finish,
        diagram: OpDiagram::Draw(dia_spiral_finish),
        validate: None,
    },
    OpUiRow {
        op: OperationType::RadialFinish,
        draw: ed_radial_finish,
        diagram: OpDiagram::Draw(dia_radial_finish),
        validate: None,
    },
    OpUiRow {
        op: OperationType::HorizontalFinish,
        draw: ed_horizontal_finish,
        diagram: OpDiagram::Stepover,
        validate: None,
    },
    OpUiRow {
        op: OperationType::ProjectCurve,
        draw: ed_project_curve,
        diagram: OpDiagram::Draw(dia_project_curve),
        validate: None,
    },
    OpUiRow {
        op: OperationType::AlignmentPinDrill,
        draw: ed_alignment_pin_drill,
        diagram: OpDiagram::Draw(dia_pin_drill),
        validate: None,
    },
];
