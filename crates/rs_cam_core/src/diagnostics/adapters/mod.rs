//! Adapters from upstream diagnostic sources into the unified
//! [`super::Diagnostic`] schema.
//!
//! Each submodule handles one source. Adapters are pure functions —
//! they take borrowed inputs and return a `Vec<Diagnostic>`. The
//! orchestrator at [`super::diagnose_toolpath`] (PR-3 onwards)
//! composes them and runs the supersession reducer.
//!
//! Design rules adapters follow:
//!
//! - Set [`super::DiagnosticState::Current`] only when the underlying
//!   evidence is fresh. Sim-required gates use `NeedsSimulation`;
//!   not-applicable kinds (drill milling gates) use `NotApplicable`.
//! - Use [`super::ids`] constants for `id`; never inline string IDs.
//! - Set `supersedes` only when the diagnostic carries authoritative
//!   evidence over a known heuristic family (e.g. `load.chipload`
//!   supersedes the four `feeds.*_vs_lut.*` heuristics).
//! - Keep `message` terse and operator-facing. Details go into
//!   `evidence`.

pub mod from_feeds;
pub mod from_generation;
pub mod from_model_refs;
pub mod from_preconditions;
pub mod from_project_diagnostics;
pub mod from_stale_default;
pub mod from_static_checks;
pub mod from_tool_load;
