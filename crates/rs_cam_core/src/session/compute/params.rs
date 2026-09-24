//! Parameter mutation on [`crate::session::ProjectSession`].
//!
//! Holds the toolpath- and tool-parameter setters, the shared range and
//! unknown-field refusals, the operation schema read, and the two advisor
//! capture entries. Split out of `session/compute.rs` (P4).
//!
//! `set_param_refuses_absent_field_n5` reads this file by exact path: its
//! assertion 5 slices the named `match` arms of `set_toolpath_param_impl` and
//! requires each one to call `check_param_range`.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tracing::instrument;

use crate::compute::tool_config::ToolId;
use crate::session::{Command, Effects, ProjectSession, SessionError, SetToolpathParamArgs};

use super::{
    AdvisorContext, OptimizeToolpathHandle, RecommendClearingStrategyHandle,
    execute_recommend_clearing_strategy,
};

/// Strip a single layer of surrounding ASCII double-quotes from a string
/// if present. Used by the MCP coercion path to tolerate clients that
/// double-encode scalar values (e.g. `"7"` arriving as the literal
/// 3-char string `"7"`). Returns the input unchanged when there are no
/// surrounding quotes or when the string isn't long enough to have any.
pub(crate) fn strip_outer_quotes(s: &str) -> &str {
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

/// The one refusal a caller gets when a parameter name reaches no field
/// on this operation.
///
/// The generic serde arm and the two optional named arms (`stepover`,
/// `depth_per_pass`) all build the message here, so the two routes
/// cannot report the same condition in different words (N5).
fn unknown_param_error(
    operation: &crate::compute::catalog::OperationConfig,
    param: &str,
) -> SessionError {
    // CMP-08: `param_names` now carries the three aliases this module's
    // named arms write, and the toolpath-level names follow it. Before
    // that the list was wrong in both directions for Waterline, RampFinish
    // and Pencil — it omitted a name this setter accepts, so the message
    // told the caller a name was invalid while the setter took it.
    SessionError::InvalidParam(format!(
        "unknown parameter '{param}' for {} operation. Valid parameters: {}. \
         Toolpath parameters: {}",
        operation.label(),
        operation.param_names().join(", "),
        crate::compute::catalog::OperationConfig::toolpath_param_names().join(", ")
    ))
}

/// DR-LIVE (2026-08-14): refuse a value outside the domain the registry
/// declares for this param, BEFORE it reaches the config. A refusal, not
/// a clamp — the caller finds out its number was rejected instead of
/// quietly becoming another number. A param with no declared range is
/// unchanged (see `ParamRange`'s doc: absent means *not stated*, and
/// stating them is a per-param decision, not a sweep).
///
/// Every numeric route into `set_toolpath_param` calls this one helper,
/// so a named arm and the generic serde arm cannot drift apart (N5).
fn check_param_range(
    operation: &crate::compute::catalog::OperationConfig,
    param: &str,
    value: f64,
) -> Result<(), SessionError> {
    let Some(range) = operation.param_range(param) else {
        return Ok(());
    };
    if range.accepts(value) {
        return Ok(());
    }
    Err(SessionError::InvalidParam(format!(
        "'{param}' = {value} is outside the accepted range for {} \
         ({}); the value was NOT applied",
        operation.label(),
        range.describe(),
    )))
}

impl ProjectSession {
    // ── Mutation ──────────────────────────────────────────────────

    /// Set a parameter on a toolpath's operation config.
    ///
    /// Common parameters (`feed_rate`, `plunge_rate`, `stepover`, `depth_per_pass`)
    /// are applied via the [`OperationParams`] trait. Config-specific parameters
    /// (e.g. `angle`, `min_z`, `passes`) are applied via serde round-trip so that
    /// all 23 operation variants are handled generically.
    ///
    /// `stepover` and `depth_per_pass` are OPTIONAL on the
    /// [`OperationParams`](crate::compute::catalog::OperationParams)
    /// trait. An operation whose config has no such field refuses the
    /// write with the same `unknown parameter` message the serde arm
    /// uses (N5). Before that refusal existed the arm discarded the
    /// value, stamped manual provenance on the absent field, staled the
    /// result chain and reported success.
    ///
    /// Invalidates the cached compute result for this toolpath.
    ///
    /// Since WP1 this is a thin wrapper over [`ProjectSession::apply`].
    /// WP3 gave it the door's own answer: it reports the same
    /// [`Effects`] the command door reports, so the two routes cannot
    /// carry two staleness models.
    ///
    /// # Why the `dead_code` allow
    ///
    /// WP15b made this `pub(crate)`, and no PRODUCTION caller is left in
    /// the crate: every surface takes `Command::SetToolpathParam`
    /// instead. The in-crate `#[cfg(test)]` modules still call it, and
    /// the compiler does not read those in the non-test build, so it
    /// reports the method as never used. The method stays for two
    /// reasons. It is the declared wrapper exemption that
    /// `setters_have_rows_wp15a::every_wrapper_exemption_calls_the_door`
    /// reads — that arm panics when no `impl ProjectSession` block
    /// declares it. And it is the one setter whose own body proves the
    /// two routes are one.
    #[allow(dead_code)]
    pub(crate) fn set_toolpath_param(
        &mut self,
        index: usize,
        param: &str,
        value: serde_json::Value,
    ) -> Result<Effects, SessionError> {
        let command = Command::SetToolpathParam(SetToolpathParamArgs {
            index,
            param: param.to_owned(),
            value,
        });
        self.apply(command)
    }

    /// The body of the `set_toolpath_param` command.
    ///
    /// [`ProjectSession::apply`] is the only caller. It reads the
    /// revision map around this call and builds
    /// [`Effects`](crate::session::Effects) from it.
    ///
    /// The `#[instrument]` span keeps the name `set_toolpath_param`, so a
    /// trace consumer reads the name it read before WP1.
    #[instrument(name = "set_toolpath_param", skip(self, value))]
    pub(crate) fn set_toolpath_param_impl(
        &mut self,
        index: usize,
        param: &str,
        value: serde_json::Value,
    ) -> Result<(), SessionError> {
        let tc = self
            .toolpath_configs
            .get_mut(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;

        // Numeric fields accept either JSON numbers or numeric strings —
        // some MCP / JSON-RPC clients stringify scalar values when the
        // schema type is permissive (`serde_json::Value`), so this
        // fallback keeps the API resilient. `strip_outer_quotes` also
        // tolerates the doubly-encoded `"\"7\""` form.
        let as_number = |v: &serde_json::Value| -> Option<f64> {
            v.as_f64()
                .or_else(|| v.as_str().and_then(|s| strip_outer_quotes(s).parse().ok()))
        };
        let is_integer_param_type = |ty: Option<&str>| -> bool {
            matches!(ty, Some("usize" | "u32" | "option<usize>" | "option<u32>"))
        };
        let number_from_integral_float = |n: &serde_json::Number| -> Option<serde_json::Value> {
            let f = n.as_f64()?;
            if !f.is_finite() || f.fract() != 0.0 {
                return None;
            }
            if f < i64::MIN as f64 || f > i64::MAX as f64 {
                return None;
            }
            // SAFETY: finite + integer-valued + i64 range checked above.
            #[allow(clippy::cast_possible_truncation)]
            Some(serde_json::Value::Number(serde_json::Number::from(
                f as i64,
            )))
        };

        match param {
            "feed_rate" => {
                let v = as_number(&value).ok_or_else(|| {
                    SessionError::InvalidParam("feed_rate must be a number".to_owned())
                })?;
                check_param_range(&tc.operation, param, v)?;
                tc.operation.set_feed_rate(v);
                tc.feeds_provenance.set(
                    crate::feeds::FeedsField::FeedRate,
                    crate::feeds::ValueProvenance::manual(),
                );
            }
            "plunge_rate" => {
                let v = as_number(&value).ok_or_else(|| {
                    SessionError::InvalidParam("plunge_rate must be a number".to_owned())
                })?;
                check_param_range(&tc.operation, param, v)?;
                tc.operation.set_plunge_rate(v);
                tc.feeds_provenance.set(
                    crate::feeds::FeedsField::PlungeRate,
                    crate::feeds::ValueProvenance::manual(),
                );
            }
            "stepover" => {
                let v = as_number(&value).ok_or_else(|| {
                    SessionError::InvalidParam("stepover must be a number".to_owned())
                })?;
                check_param_range(&tc.operation, param, v)?;
                // N5: `set_stepover` reports whether this config carries
                // the field. On `false` nothing was written, so the
                // caller gets the serde arm's own refusal. The
                // provenance stamp below and the result invalidation at
                // the end of this function are never reached.
                if !tc.operation.set_stepover(v) {
                    return Err(unknown_param_error(&tc.operation, param));
                }
                tc.feeds_provenance.set(
                    crate::feeds::FeedsField::Stepover,
                    crate::feeds::ValueProvenance::manual(),
                );
            }
            "depth_per_pass" => {
                let v = as_number(&value).ok_or_else(|| {
                    SessionError::InvalidParam("depth_per_pass must be a number".to_owned())
                })?;
                check_param_range(&tc.operation, param, v)?;
                // N5, as for `stepover` above. Note the three ALIAS
                // setters this arm is the only route to: Waterline maps
                // `depth_per_pass` onto `z_step`, RampFinish onto
                // `max_stepdown`, and Pencil maps `stepover` onto
                // `offset_stepover`. Since CMP-08 the registry PUBLISHES
                // those three names, as `ParamDef::aliases` on the field
                // each one writes, so the schema and the refusal message
                // agree with this arm.
                if !tc.operation.set_depth_per_pass(v) {
                    return Err(unknown_param_error(&tc.operation, param));
                }
                tc.feeds_provenance.set(
                    crate::feeds::FeedsField::DepthPerPass,
                    crate::feeds::ValueProvenance::manual(),
                );
            }
            "spindle_rpm" => {
                // Accept Null, integer, integer-valued float, or numeric string.
                // MCP / JSON-RPC clients vary in how they encode numerics —
                // some always serialize as f64 (so 13500 arrives as 13500.0),
                // others stringify when the schema's `value` is permissive.
                // The router is the right place to absorb the friction; we
                // reject only on actual loss of precision or out-of-range.
                let rpm = match &value {
                    serde_json::Value::Null => None,
                    other => {
                        let f = as_number(other).ok_or_else(|| {
                            SessionError::InvalidParam(
                                "spindle_rpm must be a non-negative integer or null".to_owned(),
                            )
                        })?;
                        if !f.is_finite() || f < 0.0 || f > f64::from(u32::MAX) {
                            return Err(SessionError::InvalidParam(format!(
                                "spindle_rpm out of range: {f}"
                            )));
                        }
                        if f.fract() != 0.0 {
                            return Err(SessionError::InvalidParam(format!(
                                "spindle_rpm must be a whole number: {f}"
                            )));
                        }
                        // SAFETY: bounds + finite + integer-valued checked above.
                        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                        Some(f as u32)
                    }
                };
                if let Some(r) = rpm {
                    check_param_range(&tc.operation, param, f64::from(r))?;
                }
                tc.operation.set_spindle_rpm(rpm);
                tc.feeds_provenance.set(
                    crate::feeds::FeedsField::SpindleRpm,
                    crate::feeds::ValueProvenance::manual(),
                );
            }
            // Not an operation parameter: it writes `ToolpathConfig`
            // state, so no `param_defs` array holds it. The published
            // schema lists it under `toolpath_params` (CMP-08).
            "debug_enabled" => {
                let v = match &value {
                    serde_json::Value::Bool(b) => *b,
                    serde_json::Value::Number(n) => n.as_i64().is_some_and(|i| i != 0),
                    _ => {
                        return Err(SessionError::InvalidParam(
                            "debug_enabled must be a bool or 0/1".to_owned(),
                        ));
                    }
                };
                tc.debug_options.enabled = v;
            }
            _ => {
                // Config-specific param: serialize -> merge -> deserialize
                let target_type = tc.operation.param_type_name(param);
                let mut json = serde_json::to_value(&tc.operation).map_err(|e| {
                    SessionError::InvalidParam(format!("failed to serialize config: {e}"))
                })?;
                // The OperationConfig uses tagged representation: { "kind": "...", "params": { ... } }
                // Merge the param into the "params" object.
                let params_obj = json
                    .get_mut("params")
                    .and_then(|v| v.as_object_mut())
                    .ok_or_else(|| {
                        SessionError::InvalidParam(
                            "unexpected config structure during serde round-trip".to_owned(),
                        )
                    })?;
                // Insert the value — even if the key doesn't exist yet. This
                // handles Optional fields that serde skips when None (e.g.
                // surface_model_id on ProjectCurve).
                let existed = params_obj.contains_key(param);
                // Coerce 0/1 -> bool when the existing field is a boolean.
                // Lets callers that can only produce JSON numbers (e.g. MCP
                // clients that treat every value as numeric) drive boolean
                // params like z_blend, detect_flat_areas, slot_clearing.
                //
                // Also coerce numeric strings ("7", "12.5") into JSON
                // numbers when the existing field is numeric (or absent).
                // The four explicitly-handled fields above (`feed_rate`,
                // `plunge_rate`, `stepover`, `depth_per_pass`) get this
                // for free via `as_number`; this wildcard fallback
                // extends the same tolerance to op-specific fields like
                // `depth`, `cut_depth`, `min_z`, etc., which previously
                // round-tripped through serde and rejected strings.
                // (Roadmap E.6.a)
                let value = match (params_obj.get(param), &value) {
                    (Some(existing), serde_json::Value::Number(n)) if existing.is_boolean() => {
                        match n.as_i64() {
                            Some(0) => serde_json::Value::Bool(false),
                            Some(1) => serde_json::Value::Bool(true),
                            _ => value,
                        }
                    }
                    // Some MCP wrappers double-encode strings ("7" arrives
                    // as the literal 3-char string `"7"`). Strip surrounding
                    // quotes before parsing so both `"7"` and `"\"7\""`
                    // arrive as a number.
                    (Some(existing), serde_json::Value::String(s)) if existing.is_boolean() => {
                        match strip_outer_quotes(s).to_ascii_lowercase().as_str() {
                            "0" | "false" => serde_json::Value::Bool(false),
                            "1" | "true" => serde_json::Value::Bool(true),
                            _ => value,
                        }
                    }
                    (existing_opt, serde_json::Value::Number(n))
                        if is_integer_param_type(target_type)
                            || existing_opt.and_then(|v| v.as_i64()).is_some() =>
                    {
                        number_from_integral_float(n).unwrap_or(value)
                    }
                    // Some MCP wrappers double-encode strings ("7" arrives
                    // as the literal 3-char string `"7"`). Strip surrounding
                    // quotes before parsing so both `"7"` and `"\"7\""`
                    // arrive as a number. Integer-backed fields must become
                    // JSON integer numbers (not 1.0), otherwise serde rejects
                    // them for usize/u32/newtype Option wrappers.
                    (existing_opt, serde_json::Value::String(s))
                        if is_integer_param_type(target_type)
                            || existing_opt.and_then(|v| v.as_i64()).is_some() =>
                    {
                        match strip_outer_quotes(s).parse::<i64>() {
                            Ok(n) => serde_json::Value::Number(serde_json::Number::from(n)),
                            Err(_) => match strip_outer_quotes(s).parse::<f64>() {
                                Ok(n) => serde_json::Number::from_f64(n)
                                    .map(serde_json::Value::Number)
                                    .unwrap_or(value),
                                Err(_) => value,
                            },
                        }
                    }
                    (existing_opt, serde_json::Value::String(s))
                        if existing_opt.is_none_or(|v| v.is_number()) =>
                    {
                        match strip_outer_quotes(s).parse::<f64>() {
                            Ok(n) => serde_json::Number::from_f64(n)
                                .map(serde_json::Value::Number)
                                .unwrap_or(value),
                            Err(_) => value,
                        }
                    }
                    // A list field clears with `null` as well as with `[]`,
                    // as `spindle_rpm` and `prev_tool_id` clear with `null`
                    // (step-ladder Phase 4, `coarse_steps`).
                    (_, serde_json::Value::Null)
                        if target_type.is_some_and(|ty| ty.starts_with("vec<")) =>
                    {
                        serde_json::Value::Array(Vec::new())
                    }
                    _ => value,
                };
                // DR-LIVE, now through the helper the named arms share
                // (N5). The range gate runs BEFORE the value reaches
                // serde.
                if let Some(n) = value.as_f64() {
                    check_param_range(&tc.operation, param, n)?;
                }
                params_obj.insert(param.to_owned(), value);
                let new_op: crate::compute::catalog::OperationConfig = serde_json::from_value(json)
                    .map_err(|e| {
                        if !existed && target_type.is_none() {
                            unknown_param_error(&tc.operation, param)
                        } else {
                            SessionError::InvalidParam(format!("invalid value for '{param}': {e}"))
                        }
                    })?;
                // Verify the param was actually consumed: re-serialize and check.
                // Serde ignores unknown fields by default, so a truly unknown param
                // would deserialize successfully but be silently dropped. A
                // registry param needs no check: CMP-10 holds each def to a
                // real field, and an empty list that serde skips on write
                // (`coarse_steps = []`) is not in the re-serialized params.
                if !existed && target_type.is_none() {
                    let check = serde_json::to_value(&new_op).map_err(|e| {
                        tracing::error!(%e, "failed to re-serialize operation config for param verification");
                        SessionError::InvalidParam(format!(
                            "failed to verify param '{param}': {e}"
                        ))
                    })?;
                    let found = check
                        .get("params")
                        .and_then(|v| v.as_object())
                        .is_some_and(|obj| obj.contains_key(param));
                    if !found {
                        return Err(unknown_param_error(&tc.operation, param));
                    }
                }
                tc.operation = new_op;
                // G6 ramp: a hand-set entry feed is an override, as
                // `feed_rate` is above.
                if param == "ramp_feed_rate" {
                    tc.feeds_provenance.set(
                        crate::feeds::FeedsField::RampFeedRate,
                        crate::feeds::ValueProvenance::manual(),
                    );
                }
            }
        }

        // Invalidate cached result for this toolpath — and, when it
        // participates in the setup's material-removal chain, everything
        // downstream that was generated against the stock it leaves
        // (2026-07-09 staleness collision class; see
        // `invalidate_result_chain`).
        let enabled = self
            .toolpath_configs
            .get(index)
            .is_some_and(|tc| tc.enabled);
        self.invalidate_result_chain(index, enabled);

        Ok(())
    }

    /// Return the full parameter schema for an operation kind without
    /// requiring an existing toolpath.
    pub fn operation_schema(
        operation_type: &str,
    ) -> Result<crate::compute::catalog::OperationSchema, SessionError> {
        let op_type: crate::compute::catalog::OperationType = serde_json::from_value(
            serde_json::Value::String(operation_type.to_owned()),
        )
        .map_err(|e| {
            let valid = crate::compute::catalog::OperationType::ALL
                .iter()
                .map(|op| op.kind_str())
                .collect::<Vec<_>>()
                .join(", ");
            SessionError::InvalidParam(format!(
                "unknown operation type '{operation_type}': {e}. Valid operation_type values: {valid}"
            ))
        })?;
        Ok(crate::compute::catalog::OperationConfig::schema_for_type(
            op_type,
        ))
    }

    /// Set a parameter on a tool definition.
    ///
    /// Supported parameters: `diameter`, `flute_count`, `stickout`, `corner_radius`,
    /// `cutting_length`, `included_angle`, `taper_half_angle`, `shaft_diameter`,
    /// `shank_diameter`, `shank_length`, `holder_diameter`, and `size_units`
    /// (`"metric"` or `"imperial"`).
    ///
    /// Invalidates cached results for all toolpaths that reference this tool.
    /// `size_units` is the exception: it changes how the size is SHOWN, no
    /// stored length, so it invalidates nothing.
    ///
    /// The door of the `SetToolParam` command row. WP3 missed this
    /// method because it calls no invalidator directly; WP4 converts it
    /// (§15 ruling 7), so the MCP reply reads the set the setter dropped
    /// instead of re-deriving a second answer.
    #[instrument(skip(self, value))]
    pub(crate) fn set_tool_param(
        &mut self,
        index: usize,
        param: &str,
        value: &serde_json::Value,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            session.set_tool_param_impl(index, param, value)
        })
    }

    /// Write one tool parameter and drop what depends on the tool.
    ///
    /// The raw half of [`Self::set_tool_param`].
    fn set_tool_param_impl(
        &mut self,
        index: usize,
        param: &str,
        value: &serde_json::Value,
    ) -> Result<(), SessionError> {
        let tool = self.tools.get_mut(index).ok_or_else(|| {
            SessionError::InvalidParam(format!("tool index {index} out of bounds"))
        })?;

        match param {
            "diameter" => {
                tool.diameter = value.as_f64().ok_or_else(|| {
                    SessionError::InvalidParam("diameter must be a number".to_owned())
                })?;
            }
            "flute_count" => {
                let v = value.as_u64().ok_or_else(|| {
                    SessionError::InvalidParam("flute_count must be an integer".to_owned())
                })?;
                tool.flute_count = v as u32;
            }
            "stickout" => {
                tool.stickout = value.as_f64().ok_or_else(|| {
                    SessionError::InvalidParam("stickout must be a number".to_owned())
                })?;
            }
            "corner_radius" => {
                tool.corner_radius = value.as_f64().ok_or_else(|| {
                    SessionError::InvalidParam("corner_radius must be a number".to_owned())
                })?;
            }
            "cutting_length" => {
                tool.cutting_length = value.as_f64().ok_or_else(|| {
                    SessionError::InvalidParam("cutting_length must be a number".to_owned())
                })?;
            }
            "included_angle" => {
                tool.included_angle = value.as_f64().ok_or_else(|| {
                    SessionError::InvalidParam("included_angle must be a number".to_owned())
                })?;
            }
            "taper_half_angle" => {
                tool.taper_half_angle = value.as_f64().ok_or_else(|| {
                    SessionError::InvalidParam("taper_half_angle must be a number".to_owned())
                })?;
            }
            "shaft_diameter" => {
                tool.shaft_diameter = value.as_f64().ok_or_else(|| {
                    SessionError::InvalidParam("shaft_diameter must be a number".to_owned())
                })?;
            }
            "shank_diameter" => {
                tool.shank_diameter = value.as_f64().ok_or_else(|| {
                    SessionError::InvalidParam("shank_diameter must be a number".to_owned())
                })?;
            }
            "shank_length" => {
                tool.shank_length = value.as_f64().ok_or_else(|| {
                    SessionError::InvalidParam("shank_length must be a number".to_owned())
                })?;
            }
            "holder_diameter" => {
                tool.holder_diameter = value.as_f64().ok_or_else(|| {
                    SessionError::InvalidParam("holder_diameter must be a number".to_owned())
                })?;
            }
            "size_units" => {
                let units = value
                    .as_str()
                    .and_then(crate::compute::tool_config::SizeUnits::parse_lenient)
                    .ok_or_else(|| {
                        SessionError::InvalidParam(format!(
                            "size_units must be \"metric\" or \"imperial\" (got {value})"
                        ))
                    })?;
                tool.size_units = Some(units);
                // Display only: no stored length changed, so no result is
                // dropped.
                return Ok(());
            }
            _ => {
                return Err(SessionError::InvalidParam(format!(
                    "unknown tool parameter '{param}'"
                )));
            }
        }

        // Invalidate cached results for all toolpaths that use this tool
        let tool_raw_id = tool.id.0;
        self.drop_tool_results(tool_raw_id);

        Ok(())
    }

    // ── Compute ────────────────────────────────────────────────────

    /// Compare clearing strategies for an `Adaptive3d` toolpath and recommend
    /// the one that minimises wall-clock at the load limit on this machine
    /// (`planning/STRATEGY_ADVISOR_2026-06-17.md`).
    ///
    /// For each candidate [`ClearingStrategy`] it (1) runs Suggest to
    /// back the params off to the deflection / power limits, (2) plans the
    /// clearing toolpath off a *single shared* [`resolve_generation_inputs`]
    /// resolution (only the strategy varies — geometry / heights / stock
    /// frame are resolved once), and (3) ranks them via
    /// [`crate::machine::strategy_advisor::recommend_strategy`], which times each path
    /// through the accel-aware integrator at this machine's
    /// [`effective_kinematics`](crate::machine::MachineProfile::effective_kinematics).
    ///
    /// Raw clearing paths (no dressups / recorders / persistence) are the
    /// wall-clock comparison unit. Returns `Ok(None)` when the op is not an
    /// `Adaptive3d` op or no candidate plans a usable path. The candidate set
    /// is intentionally the two endpoints of the speed/load trade-off
    /// (conventional vs constant-engagement); it extends by adding to
    /// [`ADVISOR_CANDIDATE_STRATEGIES`].
    ///
    /// [`resolve_generation_inputs`]: Self::resolve_generation_inputs
    pub fn recommend_clearing_strategy(
        &self,
        index: usize,
        cancel: &AtomicBool,
    ) -> Result<Option<crate::machine::strategy_advisor::StrategyRecommendation>, SessionError>
    {
        let handle = self.capture_recommend_clearing_strategy(index, cancel)?;
        execute_recommend_clearing_strategy(&handle, cancel)
    }

    /// Capture the `recommend_clearing_strategy` job — step (i).
    ///
    /// It resolves the generation inputs once and copies every other
    /// session read the ranking makes: the machine, the stock, the post
    /// dials, the setup frame and the toolpath's own identity, name,
    /// enabled flag, stored operation and bound cutter. The last two are
    /// the STORED pair, not the load-limited clone the advisor plans —
    /// the chipload envelope resolver reads the stored operation, and
    /// capturing the clone instead would change which vendor row the
    /// modulator's band comes from.
    ///
    /// It takes `&self` and writes nothing.
    ///
    /// # Errors
    ///
    /// Everything [`Self::resolve_generation_inputs`] refuses, plus
    /// [`SessionError::ToolpathNotFound`] when `index` names no toolpath.
    pub(crate) fn capture_recommend_clearing_strategy(
        &self,
        index: usize,
        cancel: &AtomicBool,
    ) -> Result<RecommendClearingStrategyHandle, SessionError> {
        let inputs = self.resolve_generation_inputs(index, cancel)?;
        let tc = self
            .toolpath_configs
            .get(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        let setup = self.find_setup_for_toolpath_index(index);
        let context = AdvisorContext {
            machine: self.machine.clone(),
            post: self.post.clone(),
            stock: self.stock.clone(),
            toolpath_id: tc.id,
            toolpath_name: tc.name.clone(),
            toolpath_enabled: tc.enabled,
            stored_operation: tc.operation.clone(),
            stored_tool: self.get_tool(ToolId(tc.tool_id)).cloned(),
            stock_bbox: self.stock_bbox(),
            model_bbox: self.model_bbox(tc.model_id),
            setup_ctx: super::SetupEvalContext::build_for_setup(self, setup),
        };
        Ok(RecommendClearingStrategyHandle {
            index,
            inputs,
            context,
        })
    }

    /// Capture the `optimize_toolpath` job — step (i).
    ///
    /// It CLONES the session and takes the baseline cut trace off the
    /// session beside it. The candidate loop writes the toolpath's
    /// parameters, regenerates it and re-simulates the project once per
    /// candidate, so this row cannot reduce its inputs to a capture list
    /// the way the two read rows do (§24 ruling 2).
    ///
    /// It takes `&self` and writes nothing. The clone is the job's own,
    /// so the live session stays usable while the search runs.
    ///
    /// **The trace comes from the SESSION, not from a caller** (§28
    /// ruling 5). The GUI adopts every simulation into the session, so
    /// the two slots hold one run until a mutation clears the session's.
    ///
    /// # Errors
    ///
    /// [`SessionError::ToolpathNotFound`] when `index` names no
    /// toolpath, and [`SessionError::SimulationRequired`] when the
    /// session holds no cut trace to score against.
    pub(crate) fn capture_optimize_toolpath(
        &self,
        index: usize,
    ) -> Result<OptimizeToolpathHandle, SessionError> {
        if self.toolpath_configs.get(index).is_none() {
            return Err(SessionError::ToolpathNotFound(index));
        }
        let trace = self
            .simulation_result()
            .and_then(|result| result.cut_trace.as_ref())
            .map(Arc::clone)
            .ok_or_else(|| {
                SessionError::SimulationRequired(
                    "optimize_toolpath scores every candidate against a baseline cut trace"
                        .to_owned(),
                )
            })?;
        Ok(OptimizeToolpathHandle {
            index,
            // G-RESTRES: a what-if copy (`ProjectSession::what_if_copy`).
            session: self.what_if_copy(),
            trace,
            // WP29 — a SILENT sink at the construction site. A caller that
            // wants to read the search's progress attaches its own with
            // `OptimizeToolpathHandle::with_progress`; the MCP route
            // attaches none and writes nowhere.
            progress: Arc::new(crate::tool_load::optimize::OptimizeProgress::default()),
        })
    }
}
