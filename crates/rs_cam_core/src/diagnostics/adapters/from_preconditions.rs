//! Adapter: op-precondition static checks → [`Diagnostic`] list.
//!
//! Generate-time errors lifted into static validation so the user sees
//! the problem on the Params tab *before* clicking Generate. Before
//! this adapter (F-015), three op families had preconditions that
//! only fired at generation time:
//!
//! - **Rest machining** — needs an earlier enabled toolpath in the same
//!   setup using a larger previous tool on the same model. The viz-side
//!   `validate_toolpath` produced an inline error string, but
//!   `add_toolpath` / `set_toolpath_param` returned `Ok` with no
//!   `diagnostic_delta` entry, so MCP agents (and the GUI banner)
//!   only learned about the problem after the runtime
//!   `"Error: Rest machining requires …"` fired from
//!   `compute::execute`.
//! - **Drill / AlignmentPinDrill** — needs at least one hole position
//!   (either from the model's polygon centroids for Drill, or from the
//!   snapshotted `holes` array for AlignmentPinDrill).
//! - **ProjectCurve** — needs both a source curve (the toolpath's
//!   `model_id` must point at a polygon-bearing model) AND a target
//!   surface mesh (either the same model or any other loaded model).
//!
//! Each precondition emits a `Severity::Blocking` diagnostic so the
//! params banner surfaces it as actionable and the export gate refuses
//! to proceed. Confidence is `Static` (config-only check, no sim
//! required) and `state` is `Current`.

use crate::compute::catalog::OperationConfig;
use crate::compute::tool_config::ToolId;
use crate::diagnostics::{
    Category, Confidence, Diagnostic, DiagnosticId, DiagnosticState, Scope, Severity, Source, ids,
};

/// Per-toolpath, per-session context the precondition adapter needs.
///
/// Built by the session layer (see
/// [`crate::session::ProjectSession::diagnose_toolpath_with_trace`]) so
/// the adapter stays a pure function. The context describes the
/// neighbouring toolpaths and model geometry the current toolpath
/// depends on — anything the runtime would have looked at when it
/// raised a generate-time error.
#[derive(Debug, Clone, Default)]
pub struct PreconditionContext {
    /// Toolpaths that come *before* this one inside the same setup, in
    /// setup-display order. Used by the rest-machining check to verify a
    /// prior enabled toolpath exists for the chosen `prev_tool_id` on
    /// the right model.
    pub prior_toolpaths_in_setup: Vec<PriorToolpathSummary>,
    /// Geometry summary of the model this toolpath targets (the
    /// toolpath's `model_id`). `None` if the model can't be resolved
    /// (e.g. broken reference).
    pub target_model: Option<TargetModelGeometry>,
    /// True when at least one loaded model in the session carries a
    /// triangle mesh. Used by the ProjectCurve check to decide whether a
    /// projection surface is available somewhere in the scene.
    pub any_loaded_model_has_mesh: bool,
    /// Diameter of each tool referenced by `tool_id` in the session.
    /// Used to validate the rest-machining "prev tool must be larger"
    /// precondition without re-passing the full tool list.
    pub tool_diameters: Vec<ToolDiameterEntry>,
}

#[derive(Debug, Clone)]
pub struct PriorToolpathSummary {
    pub enabled: bool,
    pub tool_id: usize,
    pub model_id: usize,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TargetModelGeometry {
    pub has_polygons: bool,
    pub has_mesh: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct ToolDiameterEntry {
    pub id: ToolId,
    pub diameter: f64,
}

/// Run every precondition check against a single toolpath, returning a
/// flat diagnostic list.
pub fn diagnostics_from_preconditions(
    toolpath_id: usize,
    op: &OperationConfig,
    current_tool_id: usize,
    ctx: &PreconditionContext,
) -> Vec<Diagnostic> {
    let scope = Scope::Toolpath { id: toolpath_id };
    let mut out = Vec::new();
    match op {
        OperationConfig::Rest(cfg) => {
            out.extend(rest_checks(&scope, cfg, current_tool_id, ctx));
        }
        OperationConfig::Drill(_) => {
            out.extend(drill_checks(&scope, ctx));
        }
        OperationConfig::AlignmentPinDrill(cfg) => {
            if cfg.holes.is_empty() {
                out.push(Diagnostic {
                    id: DiagnosticId::from(ids::PRECOND_ALIGNMENT_PIN_DRILL_NO_HOLES),
                    scope,
                    category: Category::Geometry,
                    severity: Severity::Blocking,
                    confidence: Confidence::Static,
                    state: DiagnosticState::Current,
                    source: Source::StaticValidation,
                    message: "Alignment pin drill has no hole positions. \
                              Add alignment pins to the stock before adding this op."
                        .to_owned(),
                    evidence: None,
                    fix: None,
                    supersedes: vec![],
                    suppressed_diagnostics: vec![],
                });
            }
        }
        OperationConfig::ProjectCurve(_) => {
            out.extend(project_curve_checks(&scope, ctx));
        }
        _ => {}
    }
    out
}

fn rest_checks(
    scope: &Scope,
    cfg: &crate::compute::operation_configs::RestConfig,
    current_tool_id: usize,
    ctx: &PreconditionContext,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let Some(prev_tool_id) = cfg.prev_tool_id else {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::PRECOND_REST_PREV_TOOL_MISSING),
            scope: scope.clone(),
            category: Category::Geometry,
            severity: Severity::Blocking,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            message: "Rest machining requires a previous tool — none is selected.".to_owned(),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
        return out;
    };

    // Prev-tool-larger check: rest machining cleans up material the
    // previous (larger) tool couldn't reach. If the prev tool is the
    // same size or smaller, the op has nothing to do.
    let current_diam = ctx
        .tool_diameters
        .iter()
        .find(|t| t.id == ToolId(current_tool_id))
        .map(|t| t.diameter);
    let prev_diam = ctx
        .tool_diameters
        .iter()
        .find(|t| t.id == prev_tool_id)
        .map(|t| t.diameter);
    if let (Some(curr), Some(prev)) = (current_diam, prev_diam)
        && prev <= curr
    {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::PRECOND_REST_PREV_TOOL_NOT_LARGER),
            scope: scope.clone(),
            category: Category::Geometry,
            severity: Severity::Blocking,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            message: format!(
                "Rest machining previous tool ({prev:.1} mm) must be larger than \
                 current tool ({curr:.1} mm) — there is no leftover material to clean."
            ),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }

    // Prior-source check: at least one earlier enabled toolpath in the
    // same setup must use prev_tool_id. (Setup-ordering / same-model
    // are extra constraints — out of scope per F-015 "Out of scope:
    // validating preconditions across setup ordering". We *do* check
    // ordering inside this setup because that's the cheapest signal;
    // cross-setup ordering is the explicitly-deferred case.)
    let has_prior = ctx
        .prior_toolpaths_in_setup
        .iter()
        .any(|t| t.enabled && t.tool_id == prev_tool_id.0);
    if !has_prior {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::PRECOND_REST_NO_PRIOR),
            scope: scope.clone(),
            category: Category::Geometry,
            severity: Severity::Blocking,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            message: "Rest machining requires an earlier enabled operation in the same \
                      setup using the previous tool. None found — generation will fail."
                .to_owned(),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }

    out
}

fn drill_checks(scope: &Scope, ctx: &PreconditionContext) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    // Drill resolves holes from the target model's polygon centroids.
    // If the target model has no polygons, generation will fail with
    // "No hole positions found (import SVG with circles)".
    let has_polygons = ctx.target_model.map(|m| m.has_polygons).unwrap_or(false);
    if !has_polygons {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::PRECOND_DRILL_NO_HOLES),
            scope: scope.clone(),
            category: Category::Geometry,
            severity: Severity::Blocking,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            message: "Drill cycle has no hole positions. Target model must contain \
                      2D polygons (e.g. an SVG with circles) — generation will fail."
                .to_owned(),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }
    out
}

fn project_curve_checks(scope: &Scope, ctx: &PreconditionContext) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let target_polys = ctx.target_model.map(|m| m.has_polygons).unwrap_or(false);
    let target_mesh = ctx.target_model.map(|m| m.has_mesh).unwrap_or(false);

    if !target_polys {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::PRECOND_PROJECT_CURVE_NO_CURVE),
            scope: scope.clone(),
            category: Category::Geometry,
            severity: Severity::Blocking,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            message: "Project Curve needs a source curve. The selected model has no 2D \
                      polygons — import an SVG/DXF with the curve and select it as the \
                      toolpath's model."
                .to_owned(),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }

    // A surface mesh must exist somewhere — either the same model
    // (rare: 2D + mesh combined) or another loaded model.
    if !target_mesh && !ctx.any_loaded_model_has_mesh {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::PRECOND_PROJECT_CURVE_NO_SURFACE),
            scope: scope.clone(),
            category: Category::Geometry,
            severity: Severity::Blocking,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            message: "Project Curve needs a target surface mesh. No mesh model is loaded \
                      in the project — import the surface (e.g. an STL) before adding \
                      this op."
                .to_owned(),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }

    out
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::compute::operation_configs::{
        AlignmentPinDrillConfig, DrillConfig, ProjectCurveConfig, RestConfig,
    };

    fn rest_op(prev: Option<ToolId>) -> OperationConfig {
        OperationConfig::Rest(RestConfig {
            prev_tool_id: prev,
            ..RestConfig::default()
        })
    }

    fn drill_op() -> OperationConfig {
        OperationConfig::Drill(DrillConfig::default())
    }

    fn project_curve_op() -> OperationConfig {
        OperationConfig::ProjectCurve(ProjectCurveConfig::default())
    }

    fn alignment_pin_drill_op(holes: Vec<[f64; 2]>) -> OperationConfig {
        OperationConfig::AlignmentPinDrill(AlignmentPinDrillConfig {
            holes,
            ..AlignmentPinDrillConfig::default()
        })
    }

    #[test]
    fn rest_missing_prev_tool_fires_blocking() {
        let ctx = PreconditionContext::default();
        let diags = diagnostics_from_preconditions(0, &rest_op(None), 0, &ctx);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].id.as_str(), ids::PRECOND_REST_PREV_TOOL_MISSING);
        assert_eq!(diags[0].severity, Severity::Blocking);
    }

    #[test]
    fn rest_no_prior_fires_when_setup_empty() {
        let ctx = PreconditionContext {
            tool_diameters: vec![
                ToolDiameterEntry {
                    id: ToolId(0),
                    diameter: 3.0,
                },
                ToolDiameterEntry {
                    id: ToolId(1),
                    diameter: 6.0,
                },
            ],
            ..Default::default()
        };
        let diags = diagnostics_from_preconditions(0, &rest_op(Some(ToolId(1))), 0, &ctx);
        assert!(
            diags
                .iter()
                .any(|d| d.id.as_str() == ids::PRECOND_REST_NO_PRIOR),
            "got {:?}",
            diags
        );
    }

    #[test]
    fn rest_silent_when_prior_enabled_toolpath_uses_prev_tool() {
        let ctx = PreconditionContext {
            prior_toolpaths_in_setup: vec![PriorToolpathSummary {
                enabled: true,
                tool_id: 1,
                model_id: 0,
            }],
            tool_diameters: vec![
                ToolDiameterEntry {
                    id: ToolId(0),
                    diameter: 3.0,
                },
                ToolDiameterEntry {
                    id: ToolId(1),
                    diameter: 6.0,
                },
            ],
            ..Default::default()
        };
        let diags = diagnostics_from_preconditions(0, &rest_op(Some(ToolId(1))), 0, &ctx);
        assert!(
            diags.is_empty(),
            "prior toolpath using larger tool 1 should clear all rest preconditions, got {:?}",
            diags
        );
    }

    #[test]
    fn rest_no_prior_fires_when_only_disabled_prior_uses_prev_tool() {
        let ctx = PreconditionContext {
            prior_toolpaths_in_setup: vec![PriorToolpathSummary {
                enabled: false,
                tool_id: 1,
                model_id: 0,
            }],
            tool_diameters: vec![
                ToolDiameterEntry {
                    id: ToolId(0),
                    diameter: 3.0,
                },
                ToolDiameterEntry {
                    id: ToolId(1),
                    diameter: 6.0,
                },
            ],
            ..Default::default()
        };
        let diags = diagnostics_from_preconditions(0, &rest_op(Some(ToolId(1))), 0, &ctx);
        assert!(
            diags
                .iter()
                .any(|d| d.id.as_str() == ids::PRECOND_REST_NO_PRIOR),
            "disabled prior should not satisfy the precondition"
        );
    }

    #[test]
    fn rest_prev_tool_not_larger_fires_when_prev_smaller_or_equal() {
        let ctx = PreconditionContext {
            prior_toolpaths_in_setup: vec![PriorToolpathSummary {
                enabled: true,
                tool_id: 1,
                model_id: 0,
            }],
            // prev (id 1) is smaller than current (id 0).
            tool_diameters: vec![
                ToolDiameterEntry {
                    id: ToolId(0),
                    diameter: 6.0,
                },
                ToolDiameterEntry {
                    id: ToolId(1),
                    diameter: 3.0,
                },
            ],
            ..Default::default()
        };
        let diags = diagnostics_from_preconditions(0, &rest_op(Some(ToolId(1))), 0, &ctx);
        assert!(
            diags
                .iter()
                .any(|d| d.id.as_str() == ids::PRECOND_REST_PREV_TOOL_NOT_LARGER),
            "got {:?}",
            diags
        );
    }

    #[test]
    fn drill_no_holes_fires_when_model_has_no_polygons() {
        let ctx = PreconditionContext {
            target_model: Some(TargetModelGeometry {
                has_polygons: false,
                has_mesh: true,
            }),
            any_loaded_model_has_mesh: true,
            ..Default::default()
        };
        let diags = diagnostics_from_preconditions(0, &drill_op(), 0, &ctx);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].id.as_str(), ids::PRECOND_DRILL_NO_HOLES);
        assert_eq!(diags[0].severity, Severity::Blocking);
    }

    #[test]
    fn drill_silent_when_model_has_polygons() {
        let ctx = PreconditionContext {
            target_model: Some(TargetModelGeometry {
                has_polygons: true,
                has_mesh: false,
            }),
            ..Default::default()
        };
        let diags = diagnostics_from_preconditions(0, &drill_op(), 0, &ctx);
        assert!(diags.is_empty());
    }

    #[test]
    fn alignment_pin_drill_no_holes_fires_when_holes_empty() {
        let ctx = PreconditionContext::default();
        let diags = diagnostics_from_preconditions(0, &alignment_pin_drill_op(vec![]), 0, &ctx);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].id.as_str(),
            ids::PRECOND_ALIGNMENT_PIN_DRILL_NO_HOLES
        );
    }

    #[test]
    fn alignment_pin_drill_silent_when_holes_present() {
        let ctx = PreconditionContext::default();
        let op = alignment_pin_drill_op(vec![[0.0, 0.0], [10.0, 10.0]]);
        let diags = diagnostics_from_preconditions(0, &op, 0, &ctx);
        assert!(diags.is_empty());
    }

    #[test]
    fn project_curve_missing_curve_fires_when_target_has_only_mesh() {
        // Project has only the surface STL (mesh, no polygons).
        let ctx = PreconditionContext {
            target_model: Some(TargetModelGeometry {
                has_polygons: false,
                has_mesh: true,
            }),
            any_loaded_model_has_mesh: true,
            ..Default::default()
        };
        let diags = diagnostics_from_preconditions(0, &project_curve_op(), 0, &ctx);
        assert!(
            diags
                .iter()
                .any(|d| d.id.as_str() == ids::PRECOND_PROJECT_CURVE_NO_CURVE),
            "got {:?}",
            diags
        );
    }

    #[test]
    fn project_curve_missing_surface_fires_when_only_curve_loaded() {
        // Project has only the SVG/DXF (polygons, no mesh anywhere).
        let ctx = PreconditionContext {
            target_model: Some(TargetModelGeometry {
                has_polygons: true,
                has_mesh: false,
            }),
            any_loaded_model_has_mesh: false,
            ..Default::default()
        };
        let diags = diagnostics_from_preconditions(0, &project_curve_op(), 0, &ctx);
        assert!(
            diags
                .iter()
                .any(|d| d.id.as_str() == ids::PRECOND_PROJECT_CURVE_NO_SURFACE),
            "got {:?}",
            diags
        );
    }

    #[test]
    fn project_curve_silent_when_curve_and_surface_present() {
        let ctx = PreconditionContext {
            target_model: Some(TargetModelGeometry {
                has_polygons: true,
                has_mesh: false,
            }),
            any_loaded_model_has_mesh: true,
            ..Default::default()
        };
        let diags = diagnostics_from_preconditions(0, &project_curve_op(), 0, &ctx);
        assert!(diags.is_empty(), "got {:?}", diags);
    }

    #[test]
    fn non_precondition_ops_emit_nothing() {
        let ctx = PreconditionContext::default();
        let pocket = OperationConfig::new_default(crate::compute::catalog::OperationType::Pocket);
        let diags = diagnostics_from_preconditions(0, &pocket, 0, &ctx);
        assert!(diags.is_empty());
    }
}
