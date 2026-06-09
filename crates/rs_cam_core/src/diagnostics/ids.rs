//! Stable diagnostic-rule IDs.
//!
//! Adapters use these constants when constructing
//! [`crate::diagnostics::Diagnostic`] values. Downstream code (the
//! [`crate::diagnostics::apply_supersession`] reducer, MCP consumers
//! that branch on rule kind, snapshot tests) reaches for the same
//! constants — no string literals scattered through the codebase.
//!
//! Naming: dotted lowercase with category-first prefixes. Don't
//! repurpose an existing ID for a different rule even if the message
//! is similar — IDs are wire-stable across MCP and snapshot tests.

// ── Load gates ───────────────────────────────────────────────────────
pub const LOAD_CHIPLOAD_HIGH: &str = "load.chipload.high";
pub const LOAD_CHIPLOAD_LOW: &str = "load.chipload.low";
pub const LOAD_CHIPLOAD_WITHIN: &str = "load.chipload.within";
pub const LOAD_POWER_EXCEEDS: &str = "load.power.exceeds";
pub const LOAD_POWER_WITHIN: &str = "load.power.within";
pub const LOAD_DEFLECTION_EXCEEDS: &str = "load.deflection.exceeds";
pub const LOAD_DEFLECTION_WITHIN: &str = "load.deflection.within";

// ── Drill gates ──────────────────────────────────────────────────────
pub const DRILL_CHIP_WELDING: &str = "drill.chip_welding";
pub const DRILL_PECK_ADEQUACY: &str = "drill.peck_adequacy";
pub const DRILL_PLUNGE_FEED: &str = "drill.plunge_feed";

// ── Project verdicts ─────────────────────────────────────────────────
pub const PROJECT_HOLDER_COLLISION: &str = "project.holder_collision";
pub const PROJECT_RAPID_COLLISION: &str = "project.rapid_collision";
pub const PROJECT_PLUNGE_STRESS: &str = "project.plunge_stress";
pub const PROJECT_AIR_CUT_HIGH: &str = "project.air_cut_high";
pub const PROJECT_GENERATED_EMPTY: &str = "project.generated_empty";

// ── Feeds calculator warnings ───────────────────────────────────────
pub const FEEDS_FEED_CLAMPED: &str = "feeds.feed_clamped";
pub const FEEDS_POWER_LIMITED: &str = "feeds.power_limited";
pub const FEEDS_SHANK_TOO_LARGE: &str = "feeds.shank_too_large";
pub const FEEDS_DOC_EXCEEDS_FLUTE: &str = "feeds.doc_exceeds_flute";
pub const FEEDS_SLOTTING_DETECTED: &str = "feeds.slotting_detected";
pub const FEEDS_SCALLOP_INVALID: &str = "feeds.scallop_invalid";
pub const FEEDS_CHIPLOAD_CLAMPED_TO_FLOOR: &str = "feeds.chipload_clamped_to_floor";
pub const FEEDS_DRILL_FEED_CLAMPED_TO_ENVELOPE: &str = "feeds.drill_feed_clamped_to_envelope";

// ── Pre-sim heuristic hints (superseded by load gates) ───────────────
pub const FEEDS_FEED_VS_LUT_HIGH: &str = "feeds.feed_vs_lut.high";
pub const FEEDS_FEED_VS_LUT_LOW: &str = "feeds.feed_vs_lut.low";
pub const FEEDS_STEPOVER_VS_LUT: &str = "feeds.stepover_vs_lut";
pub const FEEDS_DPP_VS_LUT: &str = "feeds.dpp_vs_lut";

// ── Static geometric / tool-op checks ────────────────────────────────
pub const GEOM_STEPOVER_EXCEEDS_DIAMETER: &str = "geom.stepover_exceeds_diameter";
pub const GEOM_DPP_EXCEEDS_CUTTING_LENGTH: &str = "geom.dpp_exceeds_cutting_length";
pub const GEOM_DPP_OVER_1_5X_DIAMETER: &str = "geom.dpp_over_1_5x_diameter";
pub const GEOM_BOTTOM_ABOVE_TOP_Z: &str = "geom.bottom_above_top_z";
pub const GEOM_FEED_Z_BELOW_TOP_Z: &str = "geom.feed_z_below_top_z";
pub const GEOM_RETRACT_Z_BELOW_FEED_Z: &str = "geom.retract_z_below_feed_z";
pub const GEOM_CLEARANCE_Z_BELOW_RETRACT_Z: &str = "geom.clearance_z_below_retract_z";
pub const GEOM_PLUNGE_EXCEEDS_FEED: &str = "geom.plunge_exceeds_feed";

// ── Tool / operation compatibility ───────────────────────────────────
pub const COMPAT_END_MILL_SCALLOP_PENCIL: &str = "compat.end_mill_on_scallop_pencil";
pub const COMPAT_BALL_NOSE_FLAT_CLEARING: &str = "compat.ball_nose_on_flat_clearing";

// ── Surface quality / cycle-time hints ───────────────────────────────
pub const QUALITY_STEPOVER_OVER_80_PCT: &str = "quality.stepover_over_80_pct_diameter";
pub const QUALITY_FINISH_STEPOVER_OVER_50_PCT: &str = "quality.finish_stepover_over_50_pct";
pub const QUALITY_BALL_SCALLOP_HEIGHT: &str = "quality.ball_scallop_height";
pub const EFFICIENCY_VERY_FINE_STEPOVER: &str = "efficiency.very_fine_stepover";

// ── Workflow / state ─────────────────────────────────────────────────
// (Workflow-only diagnostic — emitted by the GUI consumer for the
// auto-regen notice and other workflow notices. Other "state"
// adapter outputs piggy-back on the existing load-gate ids with
// `state: NeedsSimulation`.)
//
// NB: the load adapter today wraps gate-specific ids with
// `DiagnosticState::NeedsSimulation` / `StaleEvidence`, so there's
// no separate `state.needs_simulation` rule. If we add one later,
// declare it here.

// ── Stale-default validator rules (one ID per rule_id) ──────────────
pub const STALE_DROP_CUTTER_MIN_Z: &str = "stale.drop_cutter_min_z";
pub const STALE_TAPERED_BALL_PLUNGE: &str = "stale.tapered_ball_plunge";
pub const STALE_WOOD_ADAPTIVE_STEPOVER: &str = "stale.wood_adaptive_stepover";
pub const STALE_PROJECT_CURVE_NEGATIVE_DEPTH: &str = "stale.project_curve_negative_depth";

// ── Op-precondition rules (F-015) ────────────────────────────────────
//
// Generate-time errors lifted into static validation so the user sees
// the problem on the Params tab before clicking Generate. Each rule
// mirrors a runtime check that previously bubbled up as a generation
// error.
pub const PRECOND_REST_NO_PRIOR: &str = "precondition.rest_no_prior";
pub const PRECOND_REST_PREV_TOOL_MISSING: &str = "precondition.rest_prev_tool_missing";
pub const PRECOND_REST_PREV_TOOL_NOT_LARGER: &str = "precondition.rest_prev_tool_not_larger";
pub const PRECOND_DRILL_NO_HOLES: &str = "precondition.drill_no_holes";
pub const PRECOND_ALIGNMENT_PIN_DRILL_NO_HOLES: &str = "precondition.alignment_pin_drill_no_holes";
pub const PRECOND_PROJECT_CURVE_NO_CURVE: &str = "precondition.project_curve_no_curve";
pub const PRECOND_PROJECT_CURVE_NO_SURFACE: &str = "precondition.project_curve_no_surface";

// ── Cross-reference rules (F-023) ────────────────────────────────────
//
// Static checks that a toolpath's references (model, eventually
// tool / setup) resolve against the current project. Fires before
// generation time so the GUI banner *and* the MCP `diagnostic_delta`
// envelope carry the same signal.
pub const REF_MODEL_MISSING: &str = "ref.model_missing";

/// All canonical diagnostic IDs. Used by tests to enforce uniqueness
/// and as a registry for downstream consumers that want to iterate
/// the supported rule set.
pub const ALL: &[&str] = &[
    LOAD_CHIPLOAD_HIGH,
    LOAD_CHIPLOAD_LOW,
    LOAD_CHIPLOAD_WITHIN,
    LOAD_POWER_EXCEEDS,
    LOAD_POWER_WITHIN,
    LOAD_DEFLECTION_EXCEEDS,
    LOAD_DEFLECTION_WITHIN,
    DRILL_CHIP_WELDING,
    DRILL_PECK_ADEQUACY,
    DRILL_PLUNGE_FEED,
    PROJECT_HOLDER_COLLISION,
    PROJECT_RAPID_COLLISION,
    PROJECT_PLUNGE_STRESS,
    PROJECT_AIR_CUT_HIGH,
    PROJECT_GENERATED_EMPTY,
    FEEDS_FEED_CLAMPED,
    FEEDS_POWER_LIMITED,
    FEEDS_SHANK_TOO_LARGE,
    FEEDS_DOC_EXCEEDS_FLUTE,
    FEEDS_SLOTTING_DETECTED,
    FEEDS_SCALLOP_INVALID,
    FEEDS_CHIPLOAD_CLAMPED_TO_FLOOR,
    FEEDS_DRILL_FEED_CLAMPED_TO_ENVELOPE,
    FEEDS_FEED_VS_LUT_HIGH,
    FEEDS_FEED_VS_LUT_LOW,
    FEEDS_STEPOVER_VS_LUT,
    FEEDS_DPP_VS_LUT,
    GEOM_STEPOVER_EXCEEDS_DIAMETER,
    GEOM_DPP_EXCEEDS_CUTTING_LENGTH,
    GEOM_DPP_OVER_1_5X_DIAMETER,
    GEOM_BOTTOM_ABOVE_TOP_Z,
    GEOM_FEED_Z_BELOW_TOP_Z,
    GEOM_RETRACT_Z_BELOW_FEED_Z,
    GEOM_CLEARANCE_Z_BELOW_RETRACT_Z,
    GEOM_PLUNGE_EXCEEDS_FEED,
    COMPAT_END_MILL_SCALLOP_PENCIL,
    COMPAT_BALL_NOSE_FLAT_CLEARING,
    QUALITY_STEPOVER_OVER_80_PCT,
    QUALITY_FINISH_STEPOVER_OVER_50_PCT,
    QUALITY_BALL_SCALLOP_HEIGHT,
    EFFICIENCY_VERY_FINE_STEPOVER,
    STALE_DROP_CUTTER_MIN_Z,
    STALE_TAPERED_BALL_PLUNGE,
    STALE_WOOD_ADAPTIVE_STEPOVER,
    STALE_PROJECT_CURVE_NEGATIVE_DEPTH,
    PRECOND_REST_NO_PRIOR,
    PRECOND_REST_PREV_TOOL_MISSING,
    PRECOND_REST_PREV_TOOL_NOT_LARGER,
    PRECOND_DRILL_NO_HOLES,
    PRECOND_ALIGNMENT_PIN_DRILL_NO_HOLES,
    PRECOND_PROJECT_CURVE_NO_CURVE,
    PRECOND_PROJECT_CURVE_NO_SURFACE,
    REF_MODEL_MISSING,
];
