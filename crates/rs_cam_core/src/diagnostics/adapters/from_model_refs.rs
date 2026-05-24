//! Adapter: cross-reference checks for a toolpath's `model_id` →
//! [`Diagnostic`] list (F-023).
//!
//! The viz-side `validate_toolpath` already surfaces a static
//! "Selected model missing" banner when a toolpath references a
//! `model_id` that no loaded model carries. That banner is visible in
//! the GUI, but the MCP `add_toolpath` / `set_toolpath_param`
//! `diagnostic_delta` envelope — together with
//! `get_toolpath_diagnostics` and `get_project_diagnostics` — silently
//! dropped it on the floor, because the check lived on the wrong side
//! of the unified-diagnostic boundary.
//!
//! F-015 landed the static-validation adapter pattern under
//! [`super::from_preconditions`]. This adapter is a sibling — same
//! shape, same wire-up — covering the "ref → existence" class of
//! check. Today it fires one rule: the toolpath's `model_id` must
//! resolve to a loaded model unless the operation is stock-based
//! (face / drill-stock-area cases that don't need a model).
//!
//! The diagnostic is `Severity::Blocking` so the params banner
//! surfaces it as actionable, the export gate refuses to proceed, and
//! the MCP envelope's `gui_banners` / `warnings` carry it for any
//! agent driving the workflow.

use crate::compute::catalog::OperationConfig;
use crate::diagnostics::{
    Category, Confidence, Diagnostic, DiagnosticId, DiagnosticState, Scope, Severity, Source, ids,
};

/// Cross-reference context for the model-ref adapter.
///
/// Kept separate from [`super::from_preconditions::PreconditionContext`]
/// so the two adapters can evolve independently — the precondition
/// adapter cares about geometry kind on the *resolved* model, while
/// this adapter cares about *whether the reference resolves at all*.
#[derive(Debug, Clone, Default)]
pub struct ModelRefContext {
    /// The toolpath's `model_id` as stored on the [`crate::session::ToolpathConfig`].
    /// Surfaced in the diagnostic message so agents can match it
    /// against `inspect_model` output.
    pub model_id: usize,
    /// `true` when the session has a loaded model with `id == model_id`.
    /// `false` covers both "no model with that id" and "no models
    /// loaded at all" — the diagnostic message disambiguates if needed.
    pub model_resolved: bool,
}

/// Run every model-ref check against a single toolpath, returning a
/// flat diagnostic list. Stock-based ops (face, etc.) skip the check.
pub fn diagnostics_from_model_refs(
    toolpath_id: usize,
    op: &OperationConfig,
    ctx: &ModelRefContext,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    // Stock-based ops don't need a model — bail.
    if op.is_stock_based() {
        return out;
    }
    if !ctx.model_resolved {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::REF_MODEL_MISSING),
            scope: Scope::Toolpath { id: toolpath_id },
            category: Category::Geometry,
            severity: Severity::Blocking,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            message: format!(
                "Selected model is missing — toolpath references model_id {} but no loaded \
                 model has that id. Call `inspect_model` and set the toolpath's model to a \
                 valid `id` from the response.",
                ctx.model_id
            ),
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
    use crate::compute::catalog::OperationType;
    use crate::compute::operation_configs::PocketConfig;

    fn pocket_op() -> OperationConfig {
        OperationConfig::Pocket(PocketConfig::default())
    }

    #[test]
    fn missing_model_fires_blocking_with_id_in_message() {
        let ctx = ModelRefContext {
            model_id: 999,
            model_resolved: false,
        };
        let diags = diagnostics_from_model_refs(0, &pocket_op(), &ctx);
        assert_eq!(diags.len(), 1);
        let d = &diags[0];
        assert_eq!(d.id.as_str(), ids::REF_MODEL_MISSING);
        assert_eq!(d.severity, Severity::Blocking);
        assert_eq!(d.category, Category::Geometry);
        assert_eq!(d.source, Source::StaticValidation);
        assert!(
            d.message.contains("999"),
            "message should cite the invalid id: {}",
            d.message
        );
    }

    #[test]
    fn resolved_model_silences_adapter() {
        let ctx = ModelRefContext {
            model_id: 1,
            model_resolved: true,
        };
        let diags = diagnostics_from_model_refs(0, &pocket_op(), &ctx);
        assert!(diags.is_empty(), "got {:?}", diags);
    }

    #[test]
    fn stock_based_op_skips_check_even_when_unresolved() {
        // A face op against an undefined model_id is fine — face is
        // stock-driven and doesn't need a model.
        let ctx = ModelRefContext {
            model_id: 999,
            model_resolved: false,
        };
        let face = OperationConfig::new_default(OperationType::Face);
        let diags = diagnostics_from_model_refs(0, &face, &ctx);
        assert!(diags.is_empty(), "stock-based op should skip: {:?}", diags);
    }
}
