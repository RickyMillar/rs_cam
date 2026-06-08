surface: optimize-modal
file: crates/rs_cam_viz/src/ui/optimize_modal.rs
kind: modal
job: Show the sim-based optimizer's ranked candidate parameter sets for one toolpath (current baseline, cycle delta, per-gate verdict, rationale) and let the user Apply a safe candidate.
opens-from: AppEvent::OpenOptimizeModal(toolpath_id) from sim_diagnostics.rs inline "Optimize" buttons (hotspot card :412, issue card :459, toolpath boundary :867). Outcome computed synchronously and cached in optimize_modal state.
controls:
- request-optimize-suggestion (sim-feedback, ranked candidate table)
- apply-optimize-candidate (per-row Apply/Apply ⭐ → ApplyOptimizeCandidate; gated by !verdict.any_exceeded)
- set-feed-rate / set-spindle-rpm / set-stepover / set-doc (these are the ParamDelta a candidate writes when applied)
- close-optimize-modal (Close/Cancel → CloseOptimizeModal)
reads-state: optimize_modal.status (Loading/Failed/Ready(OptimizeOutcome)); candidate params/verdict/cycle_time; narrative (headline, envelope, entry_advisories, suggestions)
writes-state: (via ApplyOptimizeCandidate) operation feed/rpm/stepover/depth_per_pass per the chosen ParamDelta
confusable-with: feeds-modal-toolpath + feeds-modal-nomogram-explore (both also recommend feed/rpm/stepover/doc for one toolpath and apply them — but from the LUT/derate model, not a sim search; verdicts vs vendor-band are different mental models)
recommendation-sources-touched: sim-feedback
health: yellow — single clear job, but it recommends and writes the SAME parameter fields as the feeds modal via a parallel engine, and the operator-suggestion "Try this" block is text-only advice with no affordance (P5/P7); user must know which of two recommenders to trust.
