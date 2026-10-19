//! Adapter: op-precondition static checks → [`Diagnostic`] list.
//!
//! Generate-time errors lifted into static validation so the user sees
//! the problem on the Params tab *before* clicking Generate. Before
//! this adapter (F-015), these op families had preconditions that
//! only fired at generation time:
//!
//! - **Drill / AlignmentPinDrill** — needs at least one hole position
//!   (for Drill a point or circle of its selection, G-DRILLCENTROID; for
//!   AlignmentPinDrill a pin on the live stock or a hole of its
//!   selection — the two sources the generator drills). The selection
//!   verdict comes from the session (`ProjectSession::hole_selection`,
//!   the resolution generation reads); this adapter only converts it.
//! - **ProjectCurve** — needs both a source curve (the toolpath's
//!   `model_id` must point at a polygon-bearing model) AND a target
//!   surface mesh (either the same model or any other loaded model).
//!
//! Each precondition emits a `Severity::Blocking` diagnostic so the
//! params banner surfaces it as actionable and the export gate refuses
//! to proceed. Confidence is `Static` (config-only check, no sim
//! required) and `state` is `Current`.

use crate::compute::catalog::OperationConfig;
use crate::diagnostics::{
    Category, Confidence, Diagnostic, DiagnosticId, DiagnosticState, Scope, Severity, Source, ids,
};
use crate::ids::ToolpathId;

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
    /// Geometry summary of the model this toolpath targets (the
    /// toolpath's `model_id`). `None` if the model can't be resolved
    /// (e.g. broken reference).
    pub target_model: Option<TargetModelGeometry>,
    /// True when at least one loaded model in the session carries a
    /// triangle mesh. Used by the ProjectCurve check to decide whether a
    /// projection surface is available somewhere in the scene.
    pub any_loaded_model_has_mesh: bool,
    /// The diameter of each alignment pin of the live stock. The
    /// `AlignmentPinDrill` check reads this, never the op's `holes`
    /// snapshot: the generator replaces `holes` with the live stock pins
    /// of the op's diameter before it drills (R1), so the snapshot can be
    /// empty or stale.
    pub stock_alignment_pin_diameters: Vec<f64>,
    /// The count of the holes that a drill op drills from its own
    /// selection (geometry selection P2.3c). Zero for every other op.
    pub selection_holes: usize,
    /// The refusal that generation gives for the selection of a drill op,
    /// with the op name (`selection::view`, through
    /// `ProjectSession::hole_selection`). `None` when the selection gives
    /// the op its holes, and for every other op.
    pub selection_refusal: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TargetModelGeometry {
    pub has_polygons: bool,
    pub has_mesh: bool,
}

/// Run every precondition check against a single toolpath, returning a
/// flat diagnostic list.
pub fn diagnostics_from_preconditions(
    toolpath_id: ToolpathId,
    op: &OperationConfig,
    ctx: &PreconditionContext,
) -> Vec<Diagnostic> {
    let scope = Scope::Toolpath { id: toolpath_id };
    let mut out = Vec::new();
    match op {
        OperationConfig::Drill(_) => {
            out.extend(drill_checks(&scope, ctx));
        }
        OperationConfig::AlignmentPinDrill(cfg) => {
            // The generator drills the live stock pins plus the holes of
            // the selection, so this check reads the same two inputs.
            let has_pins_of_size = ctx
                .stock_alignment_pin_diameters
                .iter()
                .any(|d| crate::compute::alignment_pins::same_pin_diameter(*d, cfg.diameter));
            let message = match &ctx.selection_refusal {
                Some(refusal) => Some(format!("{refusal} — generation will fail.")),
                None if !has_pins_of_size && ctx.selection_holes == 0 => Some(format!(
                    "Alignment pin drill has no hole positions: no stock pin is \
                     {d:.2} mm. Add alignment pins of that size to the stock.",
                    d = cfg.diameter
                )),
                None => None,
            };
            if let Some(message) = message {
                out.push(Diagnostic {
                    id: DiagnosticId::from(ids::PRECOND_ALIGNMENT_PIN_DRILL_NO_HOLES),
                    scope,
                    category: Category::Geometry,
                    severity: Severity::Blocking,
                    confidence: Confidence::Static,
                    state: DiagnosticState::Current,
                    source: Source::StaticValidation,
                    message,
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

fn drill_checks(scope: &Scope, ctx: &PreconditionContext) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    // Drill drills the points and circles of its selection. A polygon
    // outline is not a hole source (G-DRILLCENTROID). The session gives
    // the generator's own refusal sentence for the selection.
    if let Some(message) = &ctx.selection_refusal {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::PRECOND_DRILL_NO_HOLES),
            scope: scope.clone(),
            category: Category::Geometry,
            severity: Severity::Blocking,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            message: format!("{message} — generation will fail."),
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
        AlignmentPinDrillConfig, DrillConfig, ProjectCurveConfig,
    };

    fn drill_op() -> OperationConfig {
        OperationConfig::Drill(DrillConfig::default())
    }

    fn project_curve_op() -> OperationConfig {
        OperationConfig::ProjectCurve(ProjectCurveConfig::default())
    }

    fn alignment_pin_drill_op() -> OperationConfig {
        OperationConfig::AlignmentPinDrill(AlignmentPinDrillConfig::default())
    }

    /// The ribbon prints the refusal sentence the session gives for the
    /// selection, the same one the generator gives.
    #[test]
    fn drill_no_holes_fires_on_a_selection_refusal() {
        let ctx = PreconditionContext {
            selection_refusal: Some("Drill: nothing to cut".to_owned()),
            ..Default::default()
        };
        let diags = diagnostics_from_preconditions(ToolpathId(0), &drill_op(), &ctx);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].id.as_str(), ids::PRECOND_DRILL_NO_HOLES);
        assert_eq!(diags[0].severity, Severity::Blocking);
        assert!(
            diags[0].message.starts_with("Drill: nothing to cut"),
            "got {:?}",
            diags[0].message
        );
    }

    #[test]
    fn drill_silent_when_the_selection_gives_holes() {
        let ctx = PreconditionContext {
            selection_holes: 2,
            ..Default::default()
        };
        let diags = diagnostics_from_preconditions(ToolpathId(0), &drill_op(), &ctx);
        assert!(diags.is_empty());
    }

    #[test]
    fn alignment_pin_drill_no_holes_fires_without_stock_pins_or_picks() {
        let ctx = PreconditionContext::default();
        let diags = diagnostics_from_preconditions(ToolpathId(0), &alignment_pin_drill_op(), &ctx);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].id.as_str(),
            ids::PRECOND_ALIGNMENT_PIN_DRILL_NO_HOLES
        );
        assert_eq!(diags[0].severity, Severity::Blocking);
    }

    /// The live case: two pins on the stock. The generator drills the two
    /// stock pins, so the check stays silent.
    #[test]
    fn alignment_pin_drill_silent_with_stock_pins() {
        let ctx = PreconditionContext {
            stock_alignment_pin_diameters: vec![6.0, 6.0],
            ..Default::default()
        };
        let diags = diagnostics_from_preconditions(ToolpathId(0), &alignment_pin_drill_op(), &ctx);
        assert!(diags.is_empty(), "got {diags:?}");
    }

    #[test]
    fn alignment_pin_drill_silent_with_selected_holes_and_no_stock_pins() {
        let ctx = PreconditionContext {
            selection_holes: 1,
            ..Default::default()
        };
        let diags = diagnostics_from_preconditions(ToolpathId(0), &alignment_pin_drill_op(), &ctx);
        assert!(diags.is_empty(), "got {diags:?}");
    }

    /// A stale extra pick blocks a Pin Drill too, even with stock pins.
    #[test]
    fn alignment_pin_drill_fires_on_a_selection_refusal() {
        let ctx = PreconditionContext {
            stock_alignment_pin_diameters: vec![6.0],
            selection_refusal: Some("Alignment Pin Drill: a pick is lost".to_owned()),
            ..Default::default()
        };
        let diags = diagnostics_from_preconditions(ToolpathId(0), &alignment_pin_drill_op(), &ctx);
        assert_eq!(diags.len(), 1, "got {diags:?}");
        assert!(diags[0].message.contains("a pick is lost"));
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
        let diags = diagnostics_from_preconditions(ToolpathId(0), &project_curve_op(), &ctx);
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
        let diags = diagnostics_from_preconditions(ToolpathId(0), &project_curve_op(), &ctx);
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
        let diags = diagnostics_from_preconditions(ToolpathId(0), &project_curve_op(), &ctx);
        assert!(diags.is_empty(), "got {:?}", diags);
    }

    #[test]
    fn non_precondition_ops_emit_nothing() {
        let ctx = PreconditionContext::default();
        let pocket = OperationConfig::new_default(crate::compute::catalog::OperationType::Pocket);
        let diags = diagnostics_from_preconditions(ToolpathId(0), &pocket, &ctx);
        assert!(diags.is_empty());
    }
}
