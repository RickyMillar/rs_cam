//! Adapter: static config-only checks → [`Diagnostic`] list.
//!
//! These checks need *only* the toolpath config + tool + (optionally)
//! the resolved heights. They run before simulation and stay valid
//! independent of any sim trace. Geometric impossibilities (e.g.
//! stepover > diameter) live here, as do tool/op compatibility rules
//! and Z-frame ordering.
//!
//! Heuristic surface-quality / cycle-time hints are also here but
//! are emitted at [`Severity::Hint`] / [`Severity::Info`] so the
//! headline ribbon doesn't show them by default.
//!
//! Mirrors the PR-1 categorization currently living in
//! `crates/rs_cam_viz/src/ui/properties/operations/mod.rs::collect_warnings`.
//! The intent is for PR-3 to delete the GUI-side function and
//! consume this list instead.

use crate::compute::catalog::{OperationConfig, UiProcessRole};
use crate::compute::config::HeightContext;
use crate::compute::tool_config::{ToolConfig, ToolType};
use crate::diagnostics::{
    Category, Confidence, Diagnostic, DiagnosticEvidence, DiagnosticId, DiagnosticState, Scope,
    Severity, Source, ids,
};
use crate::ids::ToolpathId;

/// Run every static check against a single toolpath, returning a flat
/// diagnostic list (no supersession applied yet).
pub fn diagnostics_from_static_checks(
    toolpath_id: ToolpathId,
    op: &OperationConfig,
    tool: &ToolConfig,
    heights_resolved: Option<&ResolvedHeights>,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let scope = Scope::Toolpath { id: toolpath_id };

    let feed = op.feed_rate();
    let plunge = op.plunge_rate();
    let spec = op.op_type().spec();

    out.extend(tool_op_compat_checks(&scope, op, tool));
    out.extend(stepover_checks(&scope, op, tool, spec.ui_process_role));
    out.extend(depth_checks(&scope, op, tool));
    out.extend(feed_checks(&scope, feed, plunge));
    if let Some(h) = heights_resolved {
        out.extend(heights_checks(&scope, h));
    }

    out
}

/// Resolved heights snapshot. Matches the four-Z layout the GUI
/// passes through `HeightContext`. Adapter callers resolve heights
/// once and pass the result here so this module doesn't need to
/// reach into the HeightMode resolution code.
#[derive(Debug, Clone)]
pub struct ResolvedHeights {
    pub top_z: f64,
    pub bottom_z: f64,
    pub feed_z: f64,
    pub retract_z: f64,
    pub clearance_z: f64,
}

impl ResolvedHeights {
    /// Build from the GUI's [`HeightContext`] using stock-top as a
    /// fallback when only the context is in hand. Used by the
    /// adapter caller path that already has the context.
    pub fn from_context(ctx: &HeightContext) -> Self {
        // Without a HeightsConfig in scope we just project the
        // stock-top + safe_z numbers; consumers that have the
        // resolved heights should call the struct constructor
        // directly.
        Self {
            top_z: ctx.stock_top_z,
            bottom_z: ctx.stock_bottom_z,
            feed_z: ctx.stock_top_z,
            retract_z: ctx.safe_z,
            clearance_z: ctx.safe_z,
        }
    }
}

// ── tool/operation compatibility ────────────────────────────────────

fn tool_op_compat_checks(
    scope: &Scope,
    op: &OperationConfig,
    tool: &ToolConfig,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let tool_type = tool.tool_type;

    // Ball-nose on 2D clearing → demoted quality hint (PR-1).
    if matches!(tool_type, ToolType::BallNose | ToolType::TaperedBallNose)
        && matches!(
            op,
            OperationConfig::Pocket(_) | OperationConfig::Face(_) | OperationConfig::Zigzag(_)
        )
    {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::COMPAT_BALL_NOSE_FLAT_CLEARING),
            scope: scope.clone(),
            category: Category::Quality,
            severity: Severity::Hint,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            message:
                "Ball nose tools leave scallops on flat surfaces. Consider a flat end mill for clearing."
                    .to_owned(),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }

    // End mill on Scallop/UnifiedFinish/Pencil → required ball geometry.
    if matches!(tool_type, ToolType::EndMill)
        && matches!(
            op,
            OperationConfig::Scallop(_)
                | OperationConfig::UnifiedFinish(_)
                | OperationConfig::Pencil(_)
        )
    {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::COMPAT_END_MILL_SCALLOP_PENCIL),
            scope: scope.clone(),
            category: Category::ToolLoad,
            severity: Severity::Critical,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            message:
                "Scallop, Unified Finish, and Pencil operations require a ball nose tool for correct surface contact."
                    .to_owned(),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }

    out
}

// ── stepover ────────────────────────────────────────────────────────

fn stepover_checks(
    scope: &Scope,
    op: &OperationConfig,
    tool: &ToolConfig,
    role: UiProcessRole,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let Some(stepover) = op.stepover() else {
        return out;
    };
    let diameter = tool.diameter;
    if diameter <= 0.0 {
        return out;
    }

    if stepover > diameter {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::GEOM_STEPOVER_EXCEEDS_DIAMETER),
            scope: scope.clone(),
            category: Category::Geometry,
            severity: Severity::Critical,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            message: format!(
                "Stepover ({stepover:.2} mm) exceeds tool diameter ({diameter:.1} mm) \
                 — will leave uncut strips."
            ),
            evidence: Some(DiagnosticEvidence::GeometryCompare {
                lhs_label: "stepover".to_owned(),
                lhs_value: stepover,
                rhs_label: "tool_diameter".to_owned(),
                rhs_value: diameter,
                unit: "mm".to_owned(),
            }),
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
        return out;
    }

    if stepover > diameter * 0.8 {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::QUALITY_STEPOVER_OVER_80_PCT),
            scope: scope.clone(),
            category: Category::Quality,
            severity: Severity::Hint,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            message: format!(
                "Stepover is {:.0}% of tool diameter — may leave visible scallops.",
                (stepover / diameter) * 100.0
            ),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }

    if stepover < diameter * 0.05 {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::EFFICIENCY_VERY_FINE_STEPOVER),
            scope: scope.clone(),
            category: Category::Efficiency,
            severity: Severity::Info,
            confidence: Confidence::Heuristic,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            message: "Very fine stepover — cycle time will be significantly longer.".to_owned(),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }

    if matches!(role, UiProcessRole::Finish) && stepover > diameter * 0.5 {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::QUALITY_FINISH_STEPOVER_OVER_50_PCT),
            scope: scope.clone(),
            category: Category::Quality,
            severity: Severity::Hint,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            message: format!(
                "Stepover ({stepover:.2} mm) is >{:.0}% of tool diameter. Finish quality may suffer.",
                (stepover / diameter) * 100.0
            ),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }

    if matches!(
        tool.tool_type,
        ToolType::BallNose | ToolType::TaperedBallNose
    ) && matches!(role, UiProcessRole::Finish)
    {
        let ball_r = diameter / 2.0;
        if ball_r > 0.0 && stepover < diameter {
            let half_so = stepover / 2.0;
            let inner = ball_r * ball_r - half_so * half_so;
            if inner > 0.0 {
                let scallop_h = ball_r - inner.sqrt();
                if scallop_h > 0.1 {
                    out.push(Diagnostic {
                        id: DiagnosticId::from(ids::QUALITY_BALL_SCALLOP_HEIGHT),
                        scope: scope.clone(),
                        category: Category::Quality,
                        severity: Severity::Hint,
                        confidence: Confidence::Static,
                        state: DiagnosticState::Current,
                        source: Source::StaticValidation,
                        message: format!(
                            "Scallop height {scallop_h:.3} mm at this stepover. \
                             Reduce stepover for a smoother finish."
                        ),
                        evidence: None,
                        fix: None,
                        supersedes: vec![],
                        suppressed_diagnostics: vec![],
                    });
                }
            }
        }
    }

    out
}

// ── depth ───────────────────────────────────────────────────────────

fn depth_checks(scope: &Scope, op: &OperationConfig, tool: &ToolConfig) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let Some(dpp) = op.depth_per_pass() else {
        return out;
    };

    if tool.cutting_length > 0.0 && dpp > tool.cutting_length {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::GEOM_DPP_EXCEEDS_CUTTING_LENGTH),
            scope: scope.clone(),
            category: Category::Safety,
            severity: Severity::Critical,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            message: format!(
                "Depth per pass ({dpp:.1} mm) exceeds cutting length ({:.1} mm). \
                 Tool shank will contact material.",
                tool.cutting_length
            ),
            evidence: Some(DiagnosticEvidence::GeometryCompare {
                lhs_label: "depth_per_pass".to_owned(),
                lhs_value: dpp,
                rhs_label: "cutting_length".to_owned(),
                rhs_value: tool.cutting_length,
                unit: "mm".to_owned(),
            }),
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }

    // Preflight hint: deep cut relative to diameter. Superseded by
    // the rigorous deflection gate when sim evidence is current.
    if tool.diameter > 0.0 && dpp > tool.diameter * 1.5 {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::GEOM_DPP_OVER_1_5X_DIAMETER),
            scope: scope.clone(),
            category: Category::ToolLoad,
            severity: Severity::Hint,
            confidence: Confidence::Heuristic,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            message: format!(
                "Preflight hint: depth ({dpp:.1} mm) exceeds 1.5× tool diameter \
                 ({:.1} mm) — verify deflection with simulation.",
                tool.diameter
            ),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }

    out
}

// ── feed / plunge ───────────────────────────────────────────────────

fn feed_checks(scope: &Scope, feed: f64, plunge: f64) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if plunge > feed && feed > 0.0 {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::GEOM_PLUNGE_EXCEEDS_FEED),
            scope: scope.clone(),
            category: Category::Safety,
            severity: Severity::Caution,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            message: format!(
                "Plunge rate ({plunge:.0}) exceeds feed rate ({feed:.0}). \
                 Unusual — plunge is typically 30–50% of feed."
            ),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }
    out
}

// ── heights cross-validation ────────────────────────────────────────

fn heights_checks(scope: &Scope, h: &ResolvedHeights) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if h.bottom_z > h.top_z {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::GEOM_BOTTOM_ABOVE_TOP_Z),
            scope: scope.clone(),
            category: Category::Geometry,
            severity: Severity::Critical,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            message: format!(
                "Bottom Z ({:.1}) is above Top Z ({:.1}). No material will be cut.",
                h.bottom_z, h.top_z
            ),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }
    if h.feed_z < h.top_z {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::GEOM_FEED_Z_BELOW_TOP_Z),
            scope: scope.clone(),
            category: Category::Geometry,
            severity: Severity::Caution,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            message: format!(
                "Feed Z ({:.1}) is below Top Z ({:.1}). Tool will plunge into material at feed rate.",
                h.feed_z, h.top_z
            ),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }
    if h.retract_z < h.feed_z {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::GEOM_RETRACT_Z_BELOW_FEED_Z),
            scope: scope.clone(),
            category: Category::Geometry,
            severity: Severity::Caution,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            message: "Retract Z is below Feed Z. Retract moves won't clear the approach height."
                .to_owned(),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }
    if h.clearance_z < h.retract_z {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::GEOM_CLEARANCE_Z_BELOW_RETRACT_Z),
            scope: scope.clone(),
            category: Category::Geometry,
            severity: Severity::Caution,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            message: "Clearance Z is below Retract Z. Rapid moves between operations may collide."
                .to_owned(),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }
    out
}
