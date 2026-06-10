//! Auto-fix payloads for diagnostics.
//!
//! When a diagnostic has a canonical resolution the system can apply
//! programmatically, the adapter attaches a [`DiagnosticFix`] so the
//! UI can render a "Fix" button and so MCP / batch tools can apply
//! the fix without re-discovering it.
//!
//! Fixes are descriptive (what to change and to what value), not
//! imperative (what code to run). The consumer chooses how to apply
//! them — typically via [`crate::session::ProjectSession::set_toolpath_param`]
//! or the stale-default applier.

use crate::ids::ToolpathId;
use serde::{Deserialize, Serialize};

/// One auto-applyable fix. Tagged so consumers can branch on the kind
/// without parsing the description.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum DiagnosticFix {
    /// Set a parameter on a toolpath operation config. Mirrors the
    /// `set_toolpath_param` MCP contract — same `param` names.
    SetToolpathParam {
        toolpath_id: ToolpathId,
        param: String,
        new_value: serde_json::Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        old_value: Option<serde_json::Value>,
    },
    /// Apply a known stale-default rule's fix. The
    /// [`crate::compute::validate::StaleDefault::rule_id`] is carried
    /// so consumers can call the existing `apply_stale_default_fix`
    /// path.
    ApplyStaleDefault {
        rule_id: String,
        toolpath_id: ToolpathId,
        new_value: f64,
    },
}
