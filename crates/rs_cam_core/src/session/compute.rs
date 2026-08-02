//! Compute and mutation methods on [`ProjectSession`].

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tracing::instrument;

use crate::compute::collision_check::{
    CollisionCheckRequest, CollisionCheckResult, run_collision_check,
};
use crate::compute::config::{HeightContext, ToolpathStats};
use crate::compute::cutter::build_cutter;
use crate::compute::operation_configs::ClearingStrategy;
use crate::compute::simulate::{
    SimGroupEntry, SimToolpathEntry, SimulationRequest, run_simulation,
};
use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
use crate::compute::transform::FaceUp;
use crate::debug_trace::ToolpathDebugRecorder;
use crate::dexel_stock::StockCutDirection;
use crate::geo::{BoundingBox3, P3};
use crate::ids::ToolpathId;
use crate::mesh::TriangleMesh;
use crate::semantic_trace::{
    SemanticKey, ToolpathSemanticKind, ToolpathSemanticRecorder, enrich_traces,
};
use crate::simulation_cut::SimulationMetricOptions;
use crate::tool::MillingCutter;

use serde::{Deserialize, Serialize};

use super::{
    ProjectDiagnostics, ProjectEvidence, ProjectSession, SessionError, SimulationOptions,
    ToolpathComputeResult, ToolpathDiagnostic, Verdict, VerdictEvidence, VerdictKind,
    VerdictSeverity,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaleSet {
    pub toolpath_indices: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationKind {
    ToolpathParamChanged { toolpath_index: usize },
    ToolParamChanged { tool_index: usize },
    SetupChanged { setup_id: usize },
    StockChanged,
    AllToolpaths,
}

pub fn compute_stale_set(session: &ProjectSession, mutation: MutationKind) -> StaleSet {
    let mut toolpath_indices: Vec<usize> = match mutation {
        MutationKind::ToolpathParamChanged { toolpath_index } => (toolpath_index
            < session.toolpath_count())
        .then_some(toolpath_index)
        .into_iter()
        .collect(),
        MutationKind::ToolParamChanged { tool_index } => session
            .tools()
            .get(tool_index)
            .map(|tool| {
                session
                    .toolpath_configs()
                    .iter()
                    .enumerate()
                    .filter_map(|(index, tc)| (tc.tool_id == tool.id.0).then_some(index))
                    .collect()
            })
            .unwrap_or_default(),
        MutationKind::SetupChanged { setup_id } => session
            .find_setup_by_id(setup_id)
            .map(|(_, setup)| setup.toolpath_indices.clone())
            .unwrap_or_default(),
        MutationKind::StockChanged | MutationKind::AllToolpaths => {
            (0..session.toolpath_count()).collect()
        }
    };
    toolpath_indices.sort_unstable();
    toolpath_indices.dedup();
    StaleSet { toolpath_indices }
}

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

/// Transform an axis-aligned bbox from world frame into a setup-local
/// frame defined by `info`. Result remains axis-aligned because all
/// setup transforms are 90° increments + translation.
fn transform_bbox_world_to_local(
    bbox: &crate::geo::BoundingBox3,
    info: &crate::compute::transform::SetupTransformInfo,
) -> crate::geo::BoundingBox3 {
    use crate::geo::P3;
    let corners = [
        P3::new(bbox.min.x, bbox.min.y, bbox.min.z),
        P3::new(bbox.max.x, bbox.min.y, bbox.min.z),
        P3::new(bbox.min.x, bbox.max.y, bbox.min.z),
        P3::new(bbox.max.x, bbox.max.y, bbox.min.z),
        P3::new(bbox.min.x, bbox.min.y, bbox.max.z),
        P3::new(bbox.max.x, bbox.min.y, bbox.max.z),
        P3::new(bbox.min.x, bbox.max.y, bbox.max.z),
        P3::new(bbox.max.x, bbox.max.y, bbox.max.z),
    ];
    let mut min = P3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut max = P3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for c in corners {
        let p = info.world_to_local(c);
        min.x = min.x.min(p.x);
        min.y = min.y.min(p.y);
        min.z = min.z.min(p.z);
        max.x = max.x.max(p.x);
        max.y = max.y.max(p.y);
        max.z = max.z.max(p.z);
    }
    crate::geo::BoundingBox3 { min, max }
}

/// Translate a triangle mesh by (dx, dy, dz) and rebuild bbox/faces.
fn translate_mesh(mesh: &TriangleMesh, dx: f64, dy: f64, dz: f64) -> TriangleMesh {
    let verts: Vec<P3> = mesh
        .vertices
        .iter()
        .map(|v| P3::new(v.x + dx, v.y + dy, v.z + dz))
        .collect();
    TriangleMesh::from_raw(verts, mesh.triangles.clone())
}

/// Fully-owned, per-generation inputs resolved from session state by
/// [`ProjectSession::resolve_generation_inputs`]. Owning everything (mesh /
/// polygons via `Arc`, an owned [`SpatialIndex`](crate::mesh::SpatialIndex))
/// lets a caller generate one *or many* toolpaths off a single resolution
/// without re-deriving any frame-sensitive value: `generate_toolpath`
/// consumes it once; the strategy advisor reuses it across candidate
/// strategies (only the `operation`'s clearing strategy varies per candidate).
struct ResolvedGenInputs {
    tool: ToolConfig,
    mesh: Option<Arc<TriangleMesh>>,
    polygons: Option<Arc<Vec<crate::polygon::Polygon2>>>,
    keep_out_footprints: Vec<crate::polygon::Polygon2>,
    boundary_config: crate::compute::config::BoundaryConfig,
    emission_stock_bbox: BoundingBox3,
    heights: crate::compute::config::ResolvedHeights,
    tool_def: crate::tool::ToolDefinition,
    spatial_index: Option<crate::mesh::SpatialIndex>,
    cutting_levels: Vec<f64>,
    prev_tool_radius: Option<f64>,
    /// R1 (pencil): the resolved real reference tool config when the Pencil op's
    /// `reference_tool_id` names a library tool. Resolved here (the context has
    /// no tool list) exactly like `prev_tool_radius`.
    reference_tool_cfg: Option<ToolConfig>,
    operation: crate::compute::OperationConfig,
    pre_boundary: Option<crate::polygon::Polygon2>,
    /// P2.3: the per-region processed polygon set for a `DerivedRestRegions`
    /// boundary — the same set [`ProjectSession::apply_boundary_clip_multi`]
    /// re-derives for its post-generation clip (keep-outs subtracted, user
    /// offset applied, but NOT yet tool-radius inset). Resolved once here
    /// alongside `pre_boundary`'s union attempt and shared with the
    /// mesh-finish family's pre-clip via `ExecutionContext::boundary_regions`
    /// — never re-derived just for this field. `None` for every other
    /// boundary source (or when the boundary is disabled).
    pre_boundary_regions: Option<Vec<crate::polygon::Polygon2>>,
}

/// Clearing strategies the advisor compares for a 3D roughing op — the two
/// endpoints of the speed/load trade-off: conventional offset clearing
/// ([`ContourParallel`](ClearingStrategy::ContourParallel)) vs
/// constant-engagement trochoidal ([`ContourSpiral`](ClearingStrategy::ContourSpiral)).
/// `recommend_clearing_strategy` times both at their load-limited params and
/// lets machine acceleration decide. Extend by adding variants here.
const ADVISOR_CANDIDATE_STRATEGIES: [ClearingStrategy; 2] = [
    ClearingStrategy::ContourParallel,
    ClearingStrategy::ContourSpiral,
];

/// Map a Suggest pass's warnings to the binding [`LoadRegime`] for the
/// advisor's *why* string. Deflection-binding warnings mean the tool is the
/// limit (tool-limited); everything else reads as unconstrained here.
///
/// Machine-limited (power-binding) detection is deliberately not inferred
/// from Suggest warnings — Suggest does not emit a power-cap warning, and the
/// regime label only colours the explanation (the *choice* is always the
/// measured wall-clock minimum), so a conservative "unconstrained" default is
/// honest until a power-gate signal is threaded in.
///
/// [`LoadRegime`]: crate::strategy_advisor::LoadRegime
fn regime_from_suggest_warnings(
    warnings: &[crate::feeds::suggest::SuggestWarning],
) -> crate::strategy_advisor::LoadRegime {
    use crate::feeds::suggest::SuggestWarning;
    let deflection_bound = warnings.iter().any(|w| match w {
        SuggestWarning::DppCappedByDeflection { .. } => true,
        SuggestWarning::AxialDocClampedByEnvelope { binding, .. } => *binding == "deflection",
        _ => false,
    });
    if deflection_bound {
        crate::strategy_advisor::LoadRegime::ToolLimited
    } else {
        crate::strategy_advisor::LoadRegime::Unconstrained
    }
}

/// Map a modulated path's per-move binding-constraint distribution to the
/// advisor's [`LoadRegime`]. The dominant (most-frequent) binding constraint
/// decides: deflection → tool-limited; power / machine-max-feed /
/// kinematic-reach → machine-limited; the chipload band (max or min) →
/// unconstrained (the comfortable regime, neither the tool nor the machine
/// stressed). This is the unified-load-model upgrade over
/// [`regime_from_suggest_warnings`]: the label now comes from the actual
/// per-move binding signal of the *optimized* path, not a Suggest-warning
/// heuristic (`planning/UNIFIED_LOAD_MODEL_2026-06-18.md` §5.4).
fn regime_from_binding(
    summary: &crate::tool_load::ModulationSummary,
) -> crate::strategy_advisor::LoadRegime {
    use crate::strategy_advisor::LoadRegime;
    use crate::tool_load::BindingConstraint;
    let dominant = summary
        .binding_constraint_distribution
        .iter()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(binding, _)| *binding);
    match dominant {
        Some(BindingConstraint::DeflectionMax) => LoadRegime::ToolLimited,
        Some(
            BindingConstraint::PowerMax
            | BindingConstraint::MachineMaxFeed
            | BindingConstraint::KinematicReach,
        ) => LoadRegime::MachineLimited,
        Some(BindingConstraint::ChiploadMax | BindingConstraint::ChiploadMin) | None => {
            LoadRegime::Unconstrained
        }
    }
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
    /// Invalidates the cached compute result for this toolpath.
    #[instrument(skip(self, value))]
    pub fn set_toolpath_param(
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
                tc.operation.set_stepover(v);
                tc.feeds_provenance.set(
                    crate::feeds::FeedsField::Stepover,
                    crate::feeds::ValueProvenance::manual(),
                );
            }
            "depth_per_pass" => {
                let v = as_number(&value).ok_or_else(|| {
                    SessionError::InvalidParam("depth_per_pass must be a number".to_owned())
                })?;
                tc.operation.set_depth_per_pass(v);
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
                tc.operation.set_spindle_rpm(rpm);
                tc.feeds_provenance.set(
                    crate::feeds::FeedsField::SpindleRpm,
                    crate::feeds::ValueProvenance::manual(),
                );
            }
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
                    _ => value,
                };
                params_obj.insert(param.to_owned(), value);
                let valid_params = tc.operation.param_names();
                let new_op: crate::compute::catalog::OperationConfig = serde_json::from_value(json)
                    .map_err(|e| {
                        if !existed && target_type.is_none() {
                            SessionError::InvalidParam(format!(
                                "unknown parameter '{param}' for {} operation. Valid parameters: {}",
                                tc.operation.label(),
                                valid_params.join(", ")
                            ))
                        } else {
                            SessionError::InvalidParam(format!("invalid value for '{param}': {e}"))
                        }
                    })?;
                // Verify the param was actually consumed: re-serialize and check.
                // Serde ignores unknown fields by default, so a truly unknown param
                // would deserialize successfully but be silently dropped.
                if !existed {
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
                        return Err(SessionError::InvalidParam(format!(
                            "unknown parameter '{param}' for {} operation. Valid parameters: {}",
                            tc.operation.label(),
                            valid_params.join(", ")
                        )));
                    }
                }
                tc.operation = new_op;
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
    /// `shank_diameter`, `shank_length`, `holder_diameter`.
    ///
    /// Invalidates cached results for all toolpaths that reference this tool.
    #[instrument(skip(self, value))]
    pub fn set_tool_param(
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
            _ => {
                return Err(SessionError::InvalidParam(format!(
                    "unknown tool parameter '{param}'"
                )));
            }
        }

        // Invalidate cached results for all toolpaths that use this tool
        let tool_raw_id = tool.id.0;
        self.invalidate_tool(tool_raw_id);

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
    /// [`crate::strategy_advisor::recommend_strategy`], which times each path
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
    ) -> Result<Option<crate::strategy_advisor::StrategyRecommendation>, SessionError> {
        use crate::strategy_advisor::{StrategyCandidate, recommend_strategy};

        let resolved = self.resolve_generation_inputs(index)?;
        // Only Adaptive3d carries a clearing strategy.
        if !matches!(
            resolved.operation,
            crate::compute::OperationConfig::Adaptive3d(_)
        ) {
            return Ok(None);
        }

        let machine = self.machine();
        let material = &self.stock_config().material;
        let workholding = self.stock_config().workholding_rigidity;

        // Plan each candidate at its load-limited params. Collect OWNED
        // toolpaths so the `StrategyCandidate` borrows outlive the ranking.
        let mut planned: Vec<(
            ClearingStrategy,
            crate::toolpath::Toolpath,
            crate::strategy_advisor::LoadRegime,
        )> = Vec::new();
        for &strategy in ADVISOR_CANDIDATE_STRATEGIES.iter() {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            // Override the clearing strategy on a clone of the resolved op.
            let mut op = resolved.operation.clone();
            if let crate::compute::OperationConfig::Adaptive3d(ref mut cfg) = op {
                cfg.clearing_strategy = strategy;
            }
            // Back the params off to the load limit via Suggest.
            let suggested = crate::feeds::suggest::suggest_for_operation(
                crate::feeds::suggest::SuggestForOperationInput {
                    operation: &op,
                    tool: &resolved.tool,
                    machine,
                    material,
                    workholding,
                    lut: crate::feeds::embedded_vendor_lut(),
                    spindle_strategy: crate::feeds::SpindleStrategy::default(),
                    context: crate::feeds::suggest::SuggestContext::default(),
                },
            );
            let (op_loadlimited, regime) = match suggested {
                Ok(s) => {
                    let regime = regime_from_suggest_warnings(&s.warnings);
                    (s.operation, regime)
                }
                // Suggest refused (e.g. a material without primary-source Kc).
                // Still worth timing at the raw params; regime is unknown.
                Err(_) => (op, crate::strategy_advisor::LoadRegime::Unconstrained),
            };
            // Plan the clearing toolpath — no recorders / dressups / persist;
            // the raw path is what we time.
            let result = crate::compute::execute::execute_operation_annotated(
                &op_loadlimited,
                resolved.mesh.as_deref(),
                resolved.spatial_index.as_ref(),
                resolved.polygons.as_deref().map(|v| v.as_slice()),
                &resolved.tool_def,
                &resolved.tool,
                &resolved.heights,
                &resolved.cutting_levels,
                &resolved.emission_stock_bbox,
                resolved.prev_tool_radius,
                // Strategy-timing path plans clearing ops only, never pencil.
                None,
                None,
                cancel,
                None,
                None,
                resolved.pre_boundary.as_ref(),
            );
            if let Ok(annotated) = result {
                let annotated_arc = Arc::new(annotated);
                // Compare OPTIMIZED candidates: simulate the path, run F-039
                // modulation, and time the MODULATED toolpath so the spiral's
                // flatter, lighter engagement (which modulation can exploit
                // harder than the parallel path's corner spikes) shows up in
                // wall-clock. The regime label falls out of the per-move
                // binding constraint of the optimized path. Falls back to the
                // raw path + Suggest-warning regime when the machine carries
                // no kinematics or the candidate can't be simulated/modulated.
                let (toolpath, regime) = match self.optimized_candidate(
                    index,
                    &annotated_arc,
                    &resolved.tool,
                    &op_loadlimited,
                    cancel,
                ) {
                    Some(opt) => opt,
                    None => (annotated_arc.toolpath.clone(), regime),
                };
                planned.push((strategy, toolpath, regime));
            }
        }

        let candidates: Vec<StrategyCandidate<'_>> = planned
            .iter()
            .map(|(strategy, toolpath, regime)| StrategyCandidate {
                strategy: *strategy,
                toolpath,
                regime: *regime,
                geometry_forced: false,
            })
            .collect();

        Ok(recommend_strategy(&candidates, machine))
    }

    /// Strategy-advisor companion to
    /// [`recommend_clearing_strategy`](Self::recommend_clearing_strategy):
    /// turn a raw candidate path into the *optimized* path the user would
    /// actually run, plus its binding [`LoadRegime`]. Simulates the candidate
    /// in isolation to capture per-move engagement, then routes it through the
    /// shared F-039 core
    /// ([`modulate_annotated_against_trace`](Self::modulate_annotated_against_trace))
    /// so the timed path carries modulated feeds. Returns `None` (caller times
    /// the raw path with the Suggest-warning regime) when the machine has no
    /// kinematics block, the candidate can't be simulated, no chipload band is
    /// available, or modulation refuses.
    fn optimized_candidate(
        &self,
        index: usize,
        annotated: &Arc<crate::toolpath_spans::AnnotatedToolpath>,
        tool_cfg: &ToolConfig,
        operation: &crate::compute::OperationConfig,
        cancel: &AtomicBool,
    ) -> Option<(
        crate::toolpath::Toolpath,
        crate::strategy_advisor::LoadRegime,
    )> {
        // Modulate against the SAME kinematics + feed envelope
        // [`recommend_strategy`](crate::strategy_advisor::recommend_strategy)
        // times the candidate with, so the optimized feeds are clamped to the
        // exact ceilings they're then timed against. `effective_kinematics`
        // (never `None` — falls back to the generic-wood-router profile) is
        // also why the advisor can optimize machines that carry no explicit
        // kinematics block, unlike the production post-sim pass.
        let kinematics = self.machine.effective_kinematics();
        let max_feed = self.machine.max_feed_mm_min.max(1.0);
        let rapid_feed = max_feed;

        let cut_trace = self.simulate_candidate_isolated(
            index,
            Arc::clone(annotated),
            tool_cfg,
            operation,
            cancel,
        )?;
        let toolpath_id = self.toolpath_configs.get(index)?.id;
        let band_range = crate::tool_load::chipload_envelopes_for_session(self, Some(&cut_trace))
            .get(&toolpath_id)
            .cloned()?;
        let band = crate::feed_modulation::ChiploadBand::new(band_range.start, band_range.end)?;
        // ConstrainedMax @ aggressiveness 1.0 — the "bomber feeds" operating
        // point and the `SimulationOptions` default, so the advisor times the
        // same path the user gets after a default sim.
        let strategy = crate::feed_modulation::ModulationStrategy::ConstrainedMax;
        let aggressiveness = 1.0;

        let (modulated, outcome) = self.modulate_annotated_against_trace(
            annotated.as_ref(),
            operation,
            tool_cfg,
            toolpath_id,
            &cut_trace,
            band,
            kinematics,
            max_feed,
            rapid_feed,
            strategy,
            aggressiveness,
        )?;
        let regime = outcome
            .build_summary(operation.feed_rate(), aggressiveness, strategy)
            .map(|s| regime_from_binding(&s))
            .unwrap_or(crate::strategy_advisor::LoadRegime::Unconstrained);
        Some((modulated, regime))
    }

    /// Shared `SimulationRequest` assembly for [`run_simulation`](Self::run_simulation)
    /// and [`simulate_candidate_isolated`](Self::simulate_candidate_isolated) (S.12
    /// dedup). Both build the request off an already-assembled `groups` +
    /// `resolution` pair through the identical stock-frame / rapid-feed-ternary /
    /// kinematics-map shape; only these knobs differ between the two callers:
    ///
    /// - `metrics_enabled` / `capture_arc_engagement`: `run_simulation` mirrors
    ///   `SimulationOptions::metrics_enabled` into both fields (a single toggle
    ///   the production path exposes). `simulate_candidate_isolated` force-enables
    ///   both unconditionally — the strategy advisor's modulator needs per-move
    ///   engagement on every candidate regardless of the session's default sim
    ///   options.
    /// - `model_mesh`: `run_simulation` supplies the translated model mesh so the
    ///   simulator can compute sim-vs-model deviation; `simulate_candidate_isolated`
    ///   passes `None` — a throwaway candidate path is scored on engagement/feed,
    ///   not surface deviation.
    /// - `use_predicted_feed_in_gates`: `run_simulation` mirrors
    ///   `SimulationOptions::use_predicted_feed_in_gates`; the isolated path
    ///   force-disables it, since it evaluates candidates *before* any
    ///   feed-modulation pass exists to populate a predicted-feed map.
    fn build_sim_request(
        &self,
        groups: Vec<SimGroupEntry>,
        stock_bbox: BoundingBox3,
        resolution: f64,
        metric_options: SimulationMetricOptions,
        model_mesh: Option<Arc<TriangleMesh>>,
        use_predicted_feed_in_gates: bool,
    ) -> SimulationRequest {
        SimulationRequest {
            groups,
            stock_bbox,
            stock_top_z: stock_bbox.max.z,
            resolution,
            metric_options,
            spindle_rpm: self.post.spindle_speed,
            rapid_feed_mm_min: if self.post.high_feedrate_mode {
                self.post.high_feedrate
            } else {
                self.machine.max_feed_mm_min.max(1.0)
            },
            model_mesh,
            kinematics: self.machine.kinematics.map(|kin| {
                crate::compute::simulate::KinematicsContext {
                    kinematics: kin,
                    max_feed_mm_min: self.machine.max_feed_mm_min.max(1.0),
                    use_predicted_feed_in_gates,
                }
            }),
        }
    }

    /// Simulate a single throwaway toolpath in isolation (one setup group,
    /// one entry) and return its cut trace, with arc-engagement capture on so
    /// the per-move engagement the modulator needs is present. Used by the
    /// strategy advisor to evaluate candidate strategies that are not (yet)
    /// persisted in `self.results`; shares its `SimulationRequest` assembly
    /// with [`run_simulation`](Self::run_simulation) via
    /// [`build_sim_request`](Self::build_sim_request) for the single-path case.
    fn simulate_candidate_isolated(
        &self,
        index: usize,
        annotated: Arc<crate::toolpath_spans::AnnotatedToolpath>,
        tool_cfg: &ToolConfig,
        operation: &crate::compute::OperationConfig,
        cancel: &AtomicBool,
    ) -> Option<Arc<crate::simulation_cut::SimulationCutTrace>> {
        let tc = self.toolpath_configs.get(index)?;
        if annotated.toolpath.moves.len() < 2 {
            return None;
        }
        let stock_bbox = self.stock_bbox();
        let setup = self.find_setup_for_toolpath_index(index);
        let setup_ctx = super::SetupEvalContext::build_for_setup(self, setup);
        let direction = match setup_ctx.face_up {
            FaceUp::Bottom => StockCutDirection::FromBottom,
            _ => StockCutDirection::FromTop,
        };

        let entry = SimToolpathEntry {
            id: tc.id,
            name: tc.name.clone(),
            annotated,
            tool: build_cutter(tool_cfg),
            flute_count: tool_cfg.flute_count,
            tool_summary: tool_cfg.summary(),
            semantic_trace: None,
            spindle_rpm: operation.spindle_rpm(),
            metrics_not_applicable: false,
            drill_op: None,
            operation_config_hash: crate::compute::simulate::hash_operation_config(operation),
        };
        let local_stock_bbox = setup_ctx.sim_local_stock_bbox();
        let groups = vec![SimGroupEntry {
            toolpaths: vec![entry],
            direction,
            local_stock_bbox,
            local_to_global: setup_ctx.local_to_global,
            phantom_prior_stock: None,
        }];
        let resolution = auto_resolution_for_groups(&groups, &stock_bbox);
        // Deviation (model_mesh) is not needed for engagement capture; both
        // metrics flags force-on (modulator needs arc engagement regardless
        // of session defaults); predicted-feed gates force-off (no modulation
        // pass has run yet to populate a predicted-feed map). See
        // `build_sim_request`'s doc comment for the full rationale.
        let request = self.build_sim_request(
            groups,
            stock_bbox,
            resolution,
            SimulationMetricOptions {
                enabled: true,
                capture_arc_engagement: true,
            },
            None,
            false,
        );
        run_simulation(&request, cancel).ok()?.cut_trace
    }

    /// Resolve every per-generation input from session state for toolpath
    /// `index`: tool + cutter, geometry (mesh / polygons, setup-transformed),
    /// spatial index, resolved heights, cutting levels, emission-frame stock
    /// bbox, keep-outs, boundary + pre-clip polygon, rest-machining prev-tool
    /// radius, and the compute-time-patched operation. A pure read of `self`
    /// returning a fully-owned [`ResolvedGenInputs`] so callers can generate
    /// one or many toolpaths off a single resolution (the strategy advisor's
    /// per-strategy candidates) without re-deriving frame-sensitive values.
    ///
    /// [`generate_toolpath`](Self::generate_toolpath) destructures this and
    /// then owns the per-generation recorders + dressup/persist tail; the
    /// extraction keeps that pipeline byte-identical (the recorders simply
    /// move after the resolution, which never depended on them).
    fn resolve_generation_inputs(&self, index: usize) -> Result<ResolvedGenInputs, SessionError> {
        let tc = self
            .toolpath_configs
            .get(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;

        let tool = self
            .find_tool_by_raw_id(tc.tool_id)
            .ok_or(SessionError::ToolNotFound(ToolId(tc.tool_id)))?
            .clone();

        let model = self.find_model_by_raw_id(tc.model_id);

        let mut mesh = model.and_then(|m| m.mesh.clone());
        let mut polygons = model.and_then(|m| m.polygons.clone());

        // ProjectCurve optionally references a separate surface model for
        // the mesh (so polygons can come from a DXF while the projection
        // target is a terrain STL). Match the GUI compute path.
        if let crate::compute::OperationConfig::ProjectCurve(ref cfg) = tc.operation
            && let Some(surface_id) = cfg.surface_model_id
            && let Some(surface) = self.find_model_by_raw_id(surface_id.0)
            && surface.mesh.is_some()
        {
            mesh = surface.mesh.clone();
        }

        // Validate geometry requirements
        if tc.operation.is_3d() && mesh.is_none() {
            return Err(SessionError::MissingGeometry(
                "Operation requires a 3D mesh (STL/STEP)".to_owned(),
            ));
        }
        if !tc.operation.is_3d() && !tc.operation.is_stock_based() && polygons.is_none() {
            return Err(SessionError::MissingGeometry(
                "Operation requires 2D geometry (SVG/DXF)".to_owned(),
            ));
        }

        // Find the setup for orientation and keep-out info.
        // F-030: route every frame-derived value (stock bbox, transform,
        // safe_z, heights bbox) through a single `SetupEvalContext`. The
        // 5 historical sites that re-derived these ad hoc are now thin
        // wrappers over the same builder.
        let setup = self.find_setup_for_toolpath_index(index);
        let ctx = super::SetupEvalContext::build_for_setup(self, setup);
        let face_up = ctx.face_up;
        let z_rotation = ctx.z_rotation;

        // Collect keep-out footprints from setup fixtures and keep-out zones
        let mut keep_out_footprints: Vec<crate::polygon::Polygon2> = Vec::new();
        if let Some(s) = setup {
            for fixture in &s.fixtures {
                if fixture.enabled {
                    keep_out_footprints.push(fixture.footprint());
                }
            }
            for keep_out in &s.keep_out_zones {
                if keep_out.enabled {
                    keep_out_footprints.push(keep_out.footprint());
                }
            }
        }

        // Clone boundary config before we lose the borrow on tc
        let boundary_config = tc.boundary.clone();

        // ── Setup transforms ──────────────────────────────────────────
        // When a setup has non-identity face_up or z_rotation, transform
        // mesh and polygons into setup-local coordinates (matching the GUI
        // compute path).
        if ctx.needs_transform() {
            if let Some(raw_mesh) = mesh.as_ref() {
                mesh = Some(Arc::new(
                    self.transform_mesh_to_setup(raw_mesh, face_up, z_rotation),
                ));
            }
            if let Some(raw_polygons) = polygons.as_ref() {
                polygons = Some(Arc::new(self.transform_polygons_to_setup(
                    raw_polygons,
                    face_up,
                    z_rotation,
                )));
            }
            // Transform keep-out footprints into setup-local frame
            if !keep_out_footprints.is_empty() {
                keep_out_footprints =
                    self.transform_polygons_to_setup(&keep_out_footprints, face_up, z_rotation);
            }
        }

        // Stock bbox in the frame the toolpath is emitted in (world for
        // identity setups, zero-rooted local for non-identity). Ops read
        // `OpContext::stock_bbox` for depth anchoring (adaptive3d's
        // `stock_top_z`, face/drill tops), boundary resolution, and dressup
        // stock-top clamps — all of which must live in the emission frame.
        // The GUI controller already forwards this frame
        // (`controller/events/compute.rs` passes `ctx.heights_stock_bbox`
        // as the request bbox); pre-fix this path passed the zero-rooted
        // `local_stock_bbox` even for identity setups, so CLI/session
        // generation diverged from the GUI by `-origin` whenever
        // `stock.origin != 0` (heights/setup-frame audit 2026-06-12,
        // finding 4).
        let emission_stock_bbox = ctx.heights_stock_bbox;

        // F-028 (2026-05-25) established this frame for the height context
        // (heights.top_z anchors 2.5D depth stepping and must match where
        // the toolpath actually emits); the 2026-06-12 audit extended it to
        // the op/boundary/dressup bbox above, which had been left on the
        // zero-rooted local bbox.

        // Resolve heights. `effective_safe_z` floors the user-configured
        // `post.safe_z` at `stock_top + clearance` so rapids clear the stock.
        // F-024 invariant: floor reads from the local zero-rooted bbox even
        // for identity setups — a conservatively-higher floor (never lower
        // than the world stock top for identity setups, never lower than the
        // local stock top for non-identity setups) is always safe. Provided
        // by `SetupEvalContext::safe_z`.
        let safe_z = ctx.safe_z;

        let model_bbox = mesh.as_ref().map(|m| &m.bbox);
        let height_ctx = HeightContext {
            safe_z,
            op_depth: tc.operation.default_depth_for_heights(),
            stock_top_z: emission_stock_bbox.max.z,
            stock_bottom_z: emission_stock_bbox.min.z,
            model_top_z: model_bbox.map(|b| b.max.z),
            model_bottom_z: model_bbox.map(|b| b.min.z),
        };
        let heights = tc.heights.resolve(&height_ctx);

        // Build tool definition
        let tool_def = build_cutter(&tool);

        // Build spatial index for 3D ops
        let spatial_index = mesh
            .as_ref()
            .map(|m| crate::mesh::SpatialIndex::build_auto(m));

        // Compute cutting levels from the operation config (empty for 3D ops,
        // actual depth levels for 2D ops like Profile, Pocket, Adaptive, etc.)
        let cutting_levels = tc.operation.cutting_levels(heights.top_z);

        // For Rest machining, resolve prev_tool_radius from the RestConfig's
        // prev_tool_id, matching the GUI compute path.
        let prev_tool_radius = if let crate::compute::OperationConfig::Rest(ref cfg) = tc.operation
        {
            cfg.prev_tool_id.and_then(|prev_id| {
                self.tools
                    .iter()
                    .find(|t| t.id == prev_id)
                    .map(|t| t.diameter / 2.0)
            })
        } else {
            None
        };

        // R1 (pencil): resolve the real reference tool config from the Pencil
        // op's `reference_tool_id`, mirroring the prev_tool_radius resolution
        // above. `None` (unset id, or id not found) falls back to the nominal
        // `reference_tool_diameter` ball downstream — never an error.
        //
        // P2.5: non-Pencil ops with `rest_analysis` enabled resolve their
        // reference tool the same way, from `RestAnalysisConfig::reference_tool_id`
        // — same slot, same fallback semantics (`None` = self-referenced probe
        // downstream in `attach_generic_rest_analysis`, never an error).
        let reference_tool_cfg =
            if let crate::compute::OperationConfig::Pencil(ref cfg) = tc.operation {
                cfg.reference_tool_id.and_then(|ref_id| {
                    let found = self.tools.iter().find(|t| t.id == ref_id).cloned();
                    if found.is_none() {
                        tracing::warn!(
                            ?ref_id,
                            "Pencil reference_tool_id not found in tool list; \
                             falling back to nominal reference diameter"
                        );
                    }
                    found
                })
            } else if tc.rest_analysis.enabled {
                tc.rest_analysis.reference_tool_id.and_then(|ref_id| {
                    let found = self.tools.iter().find(|t| t.id == ref_id).cloned();
                    if found.is_none() {
                        tracing::warn!(
                            ?ref_id,
                            "RestAnalysis reference_tool_id not found in tool list; \
                             falling back to self-referenced probe"
                        );
                    }
                    found
                })
            } else {
                None
            };

        // Clone operation so we can patch `setup_z_flipped` on ProjectCurve.
        // This flag is #[serde(skip)] and set at compute time — single source of
        // truth is the setup transform's `is_z_flipped()`.
        let mut operation = tc.operation.clone();
        if let crate::compute::OperationConfig::ProjectCurve(ref mut cfg) = operation {
            // F-030: `SetupEvalContext::is_z_flipped()` returns false for
            // identity setups and `xform.is_z_flipped()` otherwise — same
            // combined predicate as the previous `needs_transform && xform.is_z_flipped()`.
            cfg.setup_z_flipped = ctx.is_z_flipped();
        }

        // Pre-resolve the effective boundary polygon so adaptive3d can
        // pre-clip its internal stock. `apply_boundary_clip` (below) resolves
        // its source polygon through this same `resolve_containment_polygon`
        // call (S.9 dedup — the two used to carry independent copies of this
        // computation, which is why they could drift). Doing this before
        // generation rather than after avoids the "cut moves outside
        // boundary become rapids" failure mode that left dexel cells
        // unstamped in deep passes.
        // `DerivedRestRegions` resolves to a *set* of disjoint polygons, but
        // adaptive3d's internal-stock pre-clip wants a single containment
        // polygon. v1 limitation: union the per-region polygons (keep-outs +
        // offset already applied) and use the result only if it collapses to
        // exactly one polygon; otherwise skip the pre-clip entirely and rely
        // on `apply_boundary_clip_multi`'s post-generation clip to enforce
        // the real boundary (this only costs adaptive3d some discarded
        // pre-clearing, not correctness).
        let mut pre_boundary_regions: Option<Vec<crate::polygon::Polygon2>> = None;
        let pre_boundary: Option<crate::polygon::Polygon2> = if boundary_config.enabled {
            if let crate::compute::config::BoundarySource::DerivedRestRegions {
                source_toolpath_id,
            } = &boundary_config.source
            {
                match self.resolve_derived_rest_region_polys(index, *source_toolpath_id) {
                    Ok(regions) => {
                        let processed_set = crate::region_set::RegionSet::from_slice(&regions)
                            .processed(&keep_out_footprints, boundary_config.offset);
                        let single = processed_set.single_union();
                        let region_count = processed_set.len();
                        // P2.3: share this exact `processed` set with the
                        // mesh-finish family's pre-clip — it's the same set
                        // `apply_boundary_clip_multi` re-derives for the
                        // post-generation clip, resolved here once rather
                        // than a third time just for this field.
                        pre_boundary_regions = Some(processed_set.as_slice().to_vec());
                        if single.is_some() {
                            single
                        } else {
                            tracing::debug!(
                                region_count = region_count,
                                "DerivedRestRegions pre-boundary union did not collapse to a \
                                 single polygon; skipping adaptive3d pre-clip (the \
                                 post-generation boundary clip still enforces the real \
                                 boundary)"
                            );
                            None
                        }
                    }
                    Err(e) => {
                        tracing::debug!(
                            error = %e,
                            "DerivedRestRegions source unavailable while resolving \
                             pre-boundary; skipping adaptive3d pre-clip"
                        );
                        None
                    }
                }
            } else {
                Self::resolve_containment_polygon(
                    &boundary_config,
                    &emission_stock_bbox,
                    mesh.as_deref(),
                    &keep_out_footprints,
                )
            }
        } else {
            None
        };

        Ok(ResolvedGenInputs {
            tool,
            mesh,
            polygons,
            keep_out_footprints,
            boundary_config,
            emission_stock_bbox,
            heights,
            tool_def,
            spatial_index,
            cutting_levels,
            prev_tool_radius,
            reference_tool_cfg,
            operation,
            pre_boundary,
            pre_boundary_regions,
        })
    }

    /// Generate a single toolpath by index.
    #[instrument(skip(self, cancel))]
    pub fn generate_toolpath(
        &mut self,
        index: usize,
        cancel: &AtomicBool,
    ) -> Result<&ToolpathComputeResult, SessionError> {
        // Rest-machining precondition, checked BEFORE any geometry work so we fail
        // fast and NEVER fall back to fresh stock: a `FromRemainingStock` op must
        // have a simulated remaining-stock snapshot. Absent it (no prior simulation,
        // or the predecessor changed since the last sim), error out — a fine rest
        // tool seeded with fresh stock clears the whole part instead of the leftover
        // (unbounded compute + wrong result; the 6mm→1mm runaway that motivated this).
        {
            let tc = self
                .toolpath_configs
                .get(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            if tc.stock_source == crate::session::StockSource::FromRemainingStock
                && self
                    .simulation
                    .as_ref()
                    .and_then(|sim| sim.prior_stocks.get(&tc.id))
                    .is_none()
            {
                return Err(SessionError::OperationFailed(format!(
                    "'{}' is set to use remaining stock (rest machining) but no simulated \
                     remaining-stock snapshot is available. Run a simulation of the preceding \
                     operations first, then regenerate — or set the stock source to Fresh if \
                     this is the first operation. (Refusing to fall back to fresh stock: a \
                     fine tool would clear the whole part instead of the leftover.)",
                    tc.name
                )));
            }
        }

        // DerivedRestRegions boundary precondition (P2.2), same shape and
        // same reasoning as the FromRemainingStock check above: checked
        // BEFORE any geometry work so we fail fast with a message naming
        // exactly what's missing, rather than silently clipping against
        // stale/absent regions (or against nothing at all).
        {
            let tc = self
                .toolpath_configs
                .get(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            if tc.boundary.enabled
                && let crate::compute::config::BoundarySource::DerivedRestRegions {
                    source_toolpath_id,
                } = &tc.boundary.source
            {
                self.resolve_derived_rest_region_polys(index, *source_toolpath_id)?;
            }
        }

        let ResolvedGenInputs {
            tool,
            mesh,
            polygons,
            keep_out_footprints,
            boundary_config,
            emission_stock_bbox,
            heights,
            tool_def,
            spatial_index,
            cutting_levels,
            prev_tool_radius,
            reference_tool_cfg,
            operation,
            pre_boundary,
            pre_boundary_regions,
        } = self.resolve_generation_inputs(index)?;

        // Re-borrow the config for the per-generation recorder labels and the
        // dressup/persist tail below. The resolved bundle owns everything
        // else; this borrow touches only `self.toolpath_configs`, leaving the
        // `self.results` write that ends the method field-disjoint — the same
        // borrow shape as before `resolve_generation_inputs` was extracted.
        let tc = self
            .toolpath_configs
            .get(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;

        // Create recorders
        let debug_recorder = ToolpathDebugRecorder::new(tc.name.clone(), tc.operation.label());
        let semantic_recorder =
            ToolpathSemanticRecorder::new(tc.name.clone(), tc.operation.label());
        let debug_root = debug_recorder.root_context();
        let semantic_root = semantic_recorder.root_context();

        let core_scope = debug_root.start_span("core_generate", tc.operation.label());
        let core_ctx = core_scope.context();

        let op_label = tc.operation.label().to_owned();

        // Execute the operation via the shared compute::execute module (annotated variant)
        // Rest machining: when this toolpath cuts the stock previous ops left
        // (`StockSource::FromRemainingStock`), seed generation with the per-op
        // simulated snapshot so adaptive3d clears only the leftover. The same
        // snapshot is reused for dressup air-cut filtering below. The
        // "snapshot present" precondition was enforced at function entry (a
        // FromRemainingStock op with no snapshot already returned an error), so
        // here `as_deref()` is guaranteed `Some` — never a fresh-stock fallback.
        let prior_stock_arc = self
            .simulation
            .as_ref()
            .and_then(|sim| sim.prior_stocks.get(&tc.id).cloned());
        let gen_initial_stock = match tc.stock_source {
            crate::session::StockSource::FromRemainingStock => prior_stock_arc.as_deref(),
            crate::session::StockSource::Fresh => None,
        };

        let op_scope = semantic_root.start_item(ToolpathSemanticKind::Operation, &op_label);
        let child_ctx = op_scope.context();
        // P1 W4a: the pencil family's emit-time surface-link-vs-retract
        // decision costs candidates against the real machine envelope —
        // same accessor pattern `apply_adaptive_feed_modulation` uses
        // (`effective_kinematics` never `None`; `cutting_feed_ceiling_mm_min`
        // for the cutting-feed cap, `max_feed_mm_min` for the travel rate).
        let link_kinematics = Some(crate::machine_kinematics::LinkKinematics {
            kinematics: self.machine.effective_kinematics(),
            max_feed_mm_min: self.machine.cutting_feed_ceiling_mm_min().max(1.0),
            rapid_feed_mm_min: self.machine.max_feed_mm_min.max(1.0),
        });
        // P2.3: `_with_regions` threads `pre_boundary_regions` (resolved once,
        // above, alongside `pre_boundary`) into the mesh-finish family's
        // pre-clip via `ExecutionContext::boundary_regions`. Every other
        // caller of the plain `execute_operation_annotated` still gets `None`
        // through its unchanged signature.
        let tp_result = crate::compute::execute::execute_operation_annotated_with_regions(
            &operation,
            mesh.as_deref(),
            spatial_index.as_ref(),
            polygons.as_deref().map(|v| v.as_slice()),
            &tool_def,
            &tool,
            &heights,
            &cutting_levels,
            &emission_stock_bbox,
            prev_tool_radius,
            reference_tool_cfg,
            Some(&core_ctx),
            cancel,
            gen_initial_stock,
            Some(&child_ctx),
            pre_boundary.as_ref(),
            pre_boundary_regions.as_deref(),
            Some(&tc.rest_analysis),
            link_kinematics,
        );

        match tp_result {
            Ok((annotated, findings)) => {
                let mut annotated = annotated;

                if !annotated.toolpath.moves.is_empty() {
                    core_scope.set_move_range(0, annotated.toolpath.moves.len().saturating_sub(1));
                    op_scope.bind_to_toolpath(
                        &annotated.toolpath,
                        0,
                        annotated.toolpath.moves.len(),
                    );
                }
                drop(core_scope);

                // Apply dressups. Reuse the `prior_stock` snapshot hoisted above
                // (enables air-cut filter + rest-machining-aware dressups).
                let prior_stock_ref = prior_stock_arc.as_deref();
                // C1: the index-carrying channels this call site owns. The
                // session produces a semantic trace, so its recorder is
                // registered here once and every transform below reconciles
                // against it — dressups, the boundary clip and the
                // entry-descent split alike.
                let mut channels =
                    crate::transform_provenance::ReconcileSet::new(Some(&semantic_recorder), None);
                let dressed = crate::compute::execute::apply_dressups(
                    annotated,
                    &tc.dressups,
                    tc.operation.feed_rate(),
                    tool_def.diameter(),
                    heights.retract_z,
                    // Stock top in the simulator's frame (= the bbox passed to
                    // dexel construction) — used by `apply_entry` to keep ramp /
                    // helix descent rapids above stock. The dexel collision
                    // check operates on this same Z; using a different frame
                    // (e.g. `heights.top_z`) here re-introduces the
                    // false-positive rapids the fix targets.
                    emission_stock_bbox.max.z,
                    prior_stock_ref,
                    None,
                    None,
                    tc.operation.transform_capabilities(),
                    None,
                    None,
                    // No per-dressup ITEMS on this path (that is the GUI
                    // worker's trace), but the items recorded at generation
                    // time must still follow the moves through every step.
                    &mut channels,
                );
                annotated = dressed;

                // ── Boundary clipping ─────────────────────────────────
                // After dressups, clip the toolpath to the machining boundary
                // (matching the GUI compute path). Spans are precisely
                // remapped through the clip via the input→output provenance
                // map (S83) so spans_valid stays true.
                if boundary_config.enabled {
                    annotated =
                        if let crate::compute::config::BoundarySource::DerivedRestRegions {
                            source_toolpath_id,
                        } = &boundary_config.source
                        {
                            // Precondition already validated at function entry —
                            // this can only fail here if the source toolpath's
                            // result was invalidated mid-generation, which can't
                            // happen under `&mut self`. Propagate defensively
                            // rather than `#[allow(clippy::unwrap_used)]`.
                            let regions =
                                self.resolve_derived_rest_region_polys(index, *source_toolpath_id)?;
                            Self::apply_boundary_clip_multi(
                                annotated,
                                &boundary_config,
                                &regions,
                                &keep_out_footprints,
                                tool_def.diameter(),
                                heights.retract_z,
                                &semantic_root,
                                &mut channels,
                            )
                        } else {
                            Self::apply_boundary_clip(
                                annotated,
                                &boundary_config,
                                &emission_stock_bbox,
                                mesh.as_deref(),
                                &keep_out_footprints,
                                tool_def.diameter(),
                                heights.retract_z,
                                &semantic_root,
                                &mut channels,
                            )
                        };
                }

                // ── Entry-descent optimization ────────────────────────
                // P1 W2 (reworked): split long safe_z-to-cut-depth plunges
                // by rapiding down to just above the INPUT STOCK's material
                // ceiling first — using the actual stock (the same snapshot
                // generation was seeded with for FromRemainingStock ops, or
                // the fresh-stock top otherwise), never a mesh height. This
                // runs on every generation (not just finish passes) since
                // the stock-derived ceiling is safe by construction — unlike
                // the mesh-derived height it replaces, which understates
                // remaining stock on rest-machining ops (the 151-collision
                // Rivers lesson — see `optimize_entry_descents`'s doc).
                //
                // Inserts moves after span construction, so the spans are
                // remapped through the same provenance-map contract the
                // boundary clip uses (`Span::remap`), rather than
                // invalidating them.
                {
                    let (transformed, _split_count) =
                        crate::dressup::optimize_entry_descents_annotated(
                            annotated,
                            gen_initial_stock,
                            heights.top_z,
                            tool_def.radius(),
                        );
                    annotated = transformed.reconcile(&mut channels).into_inner();
                }

                let stats = ToolpathStats {
                    move_count: annotated.toolpath.moves.len(),
                    cutting_distance: annotated.toolpath.total_cutting_distance(),
                    rapid_distance: annotated.toolpath.total_rapid_distance(),
                    standing_material_mm2: findings.standing_material_mm2,
                    // Wave D1: same channel, same rule — `None` is "not
                    // measured", never "nothing wrong".
                    dropped_band: findings.dropped_band.map(Box::new),
                    tip_float: findings.tip_float,
                    // PR-5: a retired dial still set in the loaded project.
                    deprecated_dial: findings.deprecated_dial.map(Box::new),
                    // PR-6a: the reach-policy stepover this op derived.
                    derived_stepovers: findings.derived_stepovers.clone(),
                    clipped_band: findings.clipped_band.map(Box::new),
                    // PR-8b: what the ramp-finish reach clamp did.
                    ramp_reach_clamp: findings.ramp_reach_clamp.map(Box::new),
                    // A/M6: which rest reference the claims pipeline resolved to.
                    claims_reference: findings.claims_reference,
                    // A/M7 gate 1: retract round-trip count, split by
                    // in-node vs between-nodes when spans are trustworthy.
                    retract_trips: Some(crate::compute::stats::compute_retract_trips(
                        &annotated.toolpath,
                        annotated.spans_valid.then_some(annotated.spans.as_slice()),
                    )),
                };

                let mut debug_trace = debug_recorder.finish();
                let mut semantic_trace = semantic_recorder.finish();
                enrich_traces(&mut debug_trace, &mut semantic_trace);

                // Build the drill-op view atomically with the annotated
                // toolpath when this is a drill cycle (§6.E dual-rep
                // invariant). Material comes from the live stock config so
                // the chip-welding / peck-adequacy / plunge-feed gates
                // (F-016) see the workpiece's actual hardness.
                let drill_op = crate::compute::execute::build_drill_op_for_config(
                    &operation,
                    polygons.as_deref().map(|v| v.as_slice()),
                    &tool_def,
                    &tool,
                    &emission_stock_bbox,
                    self.stock.material.clone(),
                );
                let annotated_arc = Arc::new(annotated);
                let op_data = match drill_op {
                    Some(d) => crate::drill_op::OpData::DrillOp(Arc::new(d), annotated_arc),
                    None => crate::drill_op::OpData::Toolpath(annotated_arc),
                };
                self.results.insert(
                    index,
                    ToolpathComputeResult {
                        op_data,
                        stats,
                        debug_trace: Some(debug_trace),
                        semantic_trace: Some(semantic_trace),
                    },
                );
                // SAFETY: we just inserted at this key
                #[allow(clippy::indexing_slicing)]
                Ok(&self.results[&index])
            }
            Err(e) => {
                drop(core_scope);
                let _ = debug_recorder.finish();
                let _ = semantic_recorder.finish();
                Err(SessionError::OperationFailed(e.to_string()))
            }
        }
    }

    /// Resolve the polygon set for `BoundarySource::DerivedRestRegions`,
    /// or a `SessionError::OperationFailed` naming exactly what's missing.
    ///
    /// Shared by the fail-hard precondition in [`Self::generate_toolpath`]
    /// (checked before any geometry work) and the boundary resolution in
    /// [`Self::resolve_generation_inputs`] / the post-dressup clip — all
    /// three call sites must agree on what "the derived regions" are, so
    /// this is the only place that reads `self.results` for it.
    ///
    /// `this_index` is the index of the toolpath *being generated* (whose
    /// boundary references `source_toolpath_id`); it is only used to reject
    /// a toolpath referencing its own regions as its boundary.
    pub(crate) fn resolve_derived_rest_region_polys(
        &self,
        this_index: usize,
        source_toolpath_id: crate::ids::ToolpathId,
    ) -> Result<Arc<Vec<crate::polygon::Polygon2>>, SessionError> {
        let Some(source_index) = self
            .toolpath_configs
            .iter()
            .position(|tc| tc.id == source_toolpath_id)
        else {
            return Err(SessionError::OperationFailed(format!(
                "Boundary references toolpath id {source_toolpath_id} for its rest regions, \
                 but no toolpath with that id exists anymore. Pick a different source toolpath \
                 for the boundary, or disable the boundary.",
            )));
        };

        if source_index == this_index {
            return Err(SessionError::OperationFailed(
                "Boundary references this toolpath's own rest regions — a toolpath cannot use \
                 itself as the source for a derived-rest-regions boundary. Pick a different \
                 source toolpath."
                    .to_owned(),
            ));
        }

        let Some(source_tc) = self.toolpath_configs.get(source_index) else {
            // Unreachable in practice: `source_index` came from `position()`
            // on this same Vec a few lines above.
            return Err(SessionError::ToolpathNotFound(source_index));
        };
        let source_name = &source_tc.name;

        let Some(result) = self.results.get(&source_index) else {
            return Err(SessionError::OperationFailed(format!(
                "'{source_name}' has no generated result yet — generate '{source_name}' first; \
                 its rest analysis produces the regions this boundary needs.",
            )));
        };

        match result.annotated().rest_regions.as_ref() {
            Some(regions) if !regions.is_empty() => Ok(Arc::clone(regions)),
            _ => Err(SessionError::OperationFailed(format!(
                "'{source_name}' produced no rest regions — it must be a pencil operation with \
                 the rest-depth detector enabled, and its rest analysis must have found \
                 material above the threshold. Check the pencil rest-depth settings on \
                 '{source_name}' and regenerate it.",
            ))),
        }
    }

    /// Resolve the boundary "containment polygon" — the polygon the cutter's
    /// footprint must stay inside (Containment=Inside) or outside (Outside).
    /// For ModelSilhouette source this returns the silhouette itself
    /// (after keep-outs and user offset). The downstream toolpath clip
    /// (`clip_toolpath_to_boundary`) does its own tool-radius inset to gate
    /// CUTTER CENTER positions — but for adaptive3d's internal-stock
    /// pre-clip we want the silhouette itself, since the cutter footprint
    /// (when its center is at silhouette - tool_radius) reaches the
    /// silhouette boundary and validly stamps cells in that band.
    ///
    /// Not used for `BoundarySource::DerivedRestRegions` — that source can
    /// resolve to multiple disjoint polygons, which this single-polygon
    /// signature can't represent. See
    /// [`Self::resolve_derived_rest_region_polys`] +
    /// [`crate::region_set::RegionSet::processed`] for that source's path,
    /// wired in by the two call sites below (`resolve_generation_inputs`'s
    /// `pre_boundary` and `generate_toolpath`'s post-dressup clip).
    pub(crate) fn resolve_containment_polygon(
        boundary_config: &crate::compute::config::BoundaryConfig,
        stock_bbox: &BoundingBox3,
        mesh: Option<&crate::mesh::TriangleMesh>,
        keep_out_footprints: &[crate::polygon::Polygon2],
    ) -> Option<crate::polygon::Polygon2> {
        use crate::boundary::subtract_keepouts;
        use crate::compute::config::BoundarySource;

        let mut stock_poly = match &boundary_config.source {
            BoundarySource::ModelSilhouette if mesh.is_some() => {
                #[allow(clippy::unwrap_used)]
                let m = mesh.unwrap();
                let silhouettes = crate::boundary::model_silhouette(m, None);
                crate::polygon::largest_by_area(&silhouettes)
                    .cloned()
                    .unwrap_or_else(|| {
                        crate::polygon::Polygon2::rectangle(
                            stock_bbox.min.x,
                            stock_bbox.min.y,
                            stock_bbox.max.x,
                            stock_bbox.max.y,
                        )
                    })
            }
            _ => crate::polygon::Polygon2::rectangle(
                stock_bbox.min.x,
                stock_bbox.min.y,
                stock_bbox.max.x,
                stock_bbox.max.y,
            ),
        };
        if !keep_out_footprints.is_empty() {
            stock_poly = subtract_keepouts(&stock_poly, keep_out_footprints);
        }
        if boundary_config.offset.abs() > 1e-9 {
            let offset_polys = crate::polygon::offset_polygon(&stock_poly, -boundary_config.offset);
            if let Some(largest) = crate::polygon::largest_by_area(&offset_polys) {
                stock_poly = largest.clone();
            }
        }
        Some(stock_poly)
    }

    /// Apply boundary clipping to a toolpath, subtracting keep-out footprints.
    ///
    /// Takes/returns an [`AnnotatedToolpath`]. Spans are precisely remapped
    /// through the clip via the provenance map returned from
    /// [`crate::boundary::clip_toolpath_to_boundary_with_provenance`]; the
    /// clipper never drops input moves, only inserts retract/rapid pairs
    /// between them, so a Region span that originally covered "the moves
    /// doing the cut for region X" still covers them post-clip plus any
    /// retracts inserted into the middle. `spans_valid` stays `true`.
    #[allow(clippy::too_many_arguments)]
    pub fn apply_boundary_clip(
        annotated: crate::toolpath_spans::AnnotatedToolpath,
        boundary_config: &crate::compute::config::BoundaryConfig,
        stock_bbox: &BoundingBox3,
        mesh: Option<&crate::mesh::TriangleMesh>,
        keep_out_footprints: &[crate::polygon::Polygon2],
        tool_diameter: f64,
        safe_z: f64,
        semantic_ctx: &crate::semantic_trace::ToolpathSemanticContext,
        channels: &mut crate::transform_provenance::ReconcileSet<'_>,
    ) -> crate::toolpath_spans::AnnotatedToolpath {
        use crate::boundary::{
            ToolContainment, clip_annotated_to_boundary_set, effective_boundary,
        };

        // Resolve the source polygon for the boundary (ModelSilhouette /
        // FaceSelection fall back to the stock rectangle when the required
        // geometry isn't available), subtract keep-outs, and apply the
        // user-configured offset. Shared with the adaptive3d pre-clip path
        // in `resolve_generation_inputs` — see that function's doc comment
        // for why the two must agree on the source polygon. `stock_bbox` is
        // always provided here, so the rectangle fallback inside
        // `resolve_containment_polygon` is unreachable in practice; kept for
        // parity with that function's `Option` signature.
        let stock_poly = Self::resolve_containment_polygon(
            boundary_config,
            stock_bbox,
            mesh,
            keep_out_footprints,
        )
        .unwrap_or_else(|| {
            crate::polygon::Polygon2::rectangle(
                stock_bbox.min.x,
                stock_bbox.min.y,
                stock_bbox.max.x,
                stock_bbox.max.y,
            )
        });

        // Map BoundaryContainment -> ToolContainment.
        let containment = match boundary_config.containment {
            crate::compute::config::BoundaryContainment::Center => ToolContainment::Center,
            crate::compute::config::BoundaryContainment::Inside => ToolContainment::Inside,
            crate::compute::config::BoundaryContainment::Outside => ToolContainment::Outside,
        };

        let tool_radius = tool_diameter / 2.0;
        let boundaries = effective_boundary(&stock_poly, containment, tool_radius);
        // An empty `boundaries` means the boundary collapsed (e.g. the tool
        // is larger than the stock): the set clipper passes the toolpath
        // through with an identity mapping, so the collapsed and the clipped
        // path leave the channels in provably the same state.
        let clipped = clip_annotated_to_boundary_set(
            annotated,
            boundaries.first().map(std::slice::from_ref).unwrap_or(&[]),
            safe_z,
        )
        .reconcile(channels)
        .into_inner();

        // Recorded AFTER the reconcile so this item's own link is bound to
        // post-clip indices and is not then remapped a second time.
        if !boundaries.is_empty() {
            let clip_scope =
                semantic_ctx.start_item(ToolpathSemanticKind::BoundaryClip, "Boundary clip");
            clip_scope.set_param(
                SemanticKey::Containment,
                match boundary_config.containment {
                    crate::compute::config::BoundaryContainment::Center => "center",
                    crate::compute::config::BoundaryContainment::Inside => "inside",
                    crate::compute::config::BoundaryContainment::Outside => "outside",
                },
            );
            clip_scope.set_param(SemanticKey::KeepOutCount, keep_out_footprints.len());
            if !clipped.toolpath.moves.is_empty() {
                clip_scope.bind_to_toolpath(&clipped.toolpath, 0, clipped.toolpath.moves.len());
            }
        }

        clipped
    }

    /// Multi-region variant of [`Self::apply_boundary_clip`] for
    /// `BoundarySource::DerivedRestRegions`, whose source resolves to a *set*
    /// of disjoint polygons rather than one containment polygon.
    ///
    /// `regions` are the raw rest regions from
    /// [`Self::resolve_derived_rest_region_polys`]; keep-out subtraction and
    /// the user offset are applied per-region here (via
    /// [`crate::region_set::RegionSet::processed`]), then each region runs
    /// through `effective_boundary` independently for the containment /
    /// tool-radius handling — a region that collapses under the inset is
    /// dropped from the set. If EVERY region collapses the boundary is
    /// treated as collapsed, same as the single-polygon path's empty
    /// `effective_boundary` case: the original toolpath is returned
    /// unchanged (identity span mapping) with a `tracing::warn!`.
    ///
    /// Span remapping contract is identical to [`Self::apply_boundary_clip`]
    /// — the set clipper never drops input moves, so `spans_valid` stays
    /// `true`.
    #[allow(clippy::too_many_arguments)]
    pub fn apply_boundary_clip_multi(
        annotated: crate::toolpath_spans::AnnotatedToolpath,
        boundary_config: &crate::compute::config::BoundaryConfig,
        regions: &[crate::polygon::Polygon2],
        keep_out_footprints: &[crate::polygon::Polygon2],
        tool_diameter: f64,
        safe_z: f64,
        semantic_ctx: &crate::semantic_trace::ToolpathSemanticContext,
        channels: &mut crate::transform_provenance::ReconcileSet<'_>,
    ) -> crate::toolpath_spans::AnnotatedToolpath {
        use crate::boundary::{
            ToolContainment, clip_annotated_to_boundary_set, effective_boundary,
        };

        // Per-region keep-out subtraction + user offset (regions that
        // collapse under the offset are dropped), mirroring what
        // `resolve_containment_polygon` does to its single polygon.
        let processed = crate::region_set::RegionSet::from_slice(regions)
            .processed(keep_out_footprints, boundary_config.offset);

        // Map BoundaryContainment -> ToolContainment.
        let containment = match boundary_config.containment {
            crate::compute::config::BoundaryContainment::Center => ToolContainment::Center,
            crate::compute::config::BoundaryContainment::Inside => ToolContainment::Inside,
            crate::compute::config::BoundaryContainment::Outside => ToolContainment::Outside,
        };

        // Containment / tool-radius handling per region. `effective_boundary`
        // may split one region into several (or collapse it to none) — flatten
        // everything into one set; membership downstream is "inside ANY".
        let tool_radius = tool_diameter / 2.0;
        let boundaries: Vec<crate::polygon::Polygon2> = processed
            .as_slice()
            .iter()
            .flat_map(|region| effective_boundary(region, containment, tool_radius))
            .collect();

        if boundaries.is_empty() {
            // Every region collapsed (offset/inset ate them all) — same
            // "boundary collapsed" semantics as the single-polygon path:
            // the toolpath passes through with an identity mapping, which is
            // still reconciled so the collapsed and clipped paths leave the
            // channels in provably the same state.
            tracing::warn!(
                region_count = regions.len(),
                "DerivedRestRegions boundary collapsed (all regions vanished under \
                 offset/containment inset) — leaving toolpath unclipped"
            );
        }

        let clipped = clip_annotated_to_boundary_set(annotated, &boundaries, safe_z)
            .reconcile(channels)
            .into_inner();

        // Recorded AFTER the reconcile so this item's own link is bound to
        // post-clip indices and is not then remapped a second time.
        if !boundaries.is_empty() {
            let clip_scope =
                semantic_ctx.start_item(ToolpathSemanticKind::BoundaryClip, "Boundary clip");
            clip_scope.set_param(
                SemanticKey::Containment,
                match boundary_config.containment {
                    crate::compute::config::BoundaryContainment::Center => "center",
                    crate::compute::config::BoundaryContainment::Inside => "inside",
                    crate::compute::config::BoundaryContainment::Outside => "outside",
                },
            );
            clip_scope.set_param(SemanticKey::KeepOutCount, keep_out_footprints.len());
            clip_scope.set_param(SemanticKey::RegionCount, boundaries.len());
            if !clipped.toolpath.moves.is_empty() {
                clip_scope.bind_to_toolpath(&clipped.toolpath, 0, clipped.toolpath.moves.len());
            }
        }

        clipped
    }

    /// Generate all enabled toolpaths, skipping those whose IDs are in `skip`.
    #[instrument(skip(self, skip_ids, cancel))]
    pub fn generate_all(
        &mut self,
        skip_ids: &[ToolpathId],
        cancel: &AtomicBool,
    ) -> Result<(), SessionError> {
        // Collect info needed for skip/logging before mutable borrow
        let tp_info: Vec<(usize, ToolpathId, String, bool)> = self
            .toolpath_configs
            .iter()
            .enumerate()
            .map(|(idx, tc)| (idx, tc.id, tc.name.clone(), tc.enabled))
            .collect();

        for (idx, tp_id, tp_name, enabled) in &tp_info {
            if !enabled {
                continue;
            }
            if skip_ids.contains(tp_id) {
                tracing::info!(id = tp_id.0, name = %tp_name, "Skipping toolpath (skip list)");
                continue;
            }
            match self.generate_toolpath(*idx, cancel) {
                Ok(_) => {}
                Err(SessionError::MissingGeometry(msg)) => {
                    tracing::warn!(id = tp_id.0, name = %tp_name, reason = %msg, "Skipping toolpath");
                }
                Err(e) => {
                    tracing::error!(id = tp_id.0, name = %tp_name, error = %e, "Toolpath failed");
                }
            }
        }
        Ok(())
    }

    // ── Analysis ───────────────────────────────────────────────────

    /// Run tri-dexel stock simulation over all computed toolpaths.
    #[instrument(skip(self, opts))]
    pub fn run_simulation(
        &mut self,
        opts: &SimulationOptions,
        cancel: &AtomicBool,
    ) -> Result<&super::SimulationResult, SessionError> {
        let stock_bbox = self.stock_bbox();

        // Build simulation groups from setups
        let mut groups = Vec::new();
        for setup in &self.setups {
            let direction = match setup.face_up {
                FaceUp::Bottom => StockCutDirection::FromBottom,
                _ => StockCutDirection::FromTop,
            };

            let mut entries = Vec::new();
            // F.4: track whether this group's first pending (enabled,
            // ungenerated) `FromRemainingStock` toolpath needs a phantom
            // `prior_stocks` snapshot. Visited for every toolpath config in
            // plan order — not just the ones that make it into `entries` —
            // so the scan sees the true "generated yet?" state regardless
            // of `skip_ids` / short-toolpath filtering below.
            let mut phantom_scan = crate::compute::simulate::PhantomPriorStockScan::default();
            for &tp_idx in &setup.toolpath_indices {
                let Some(tc) = self.toolpath_configs.get(tp_idx) else {
                    continue;
                };
                let result = self.results.get(&tp_idx);
                phantom_scan.visit(
                    entries.len(),
                    tc.enabled,
                    result.is_some(),
                    tc.id,
                    tc.stock_source,
                );
                if let Some(result) = result {
                    if opts.skip_ids.contains(&tc.id) {
                        continue;
                    }
                    if result.annotated().toolpath.moves.len() < 2 {
                        continue;
                    }

                    let tool_config = self.find_tool_by_raw_id(tc.tool_id);
                    let flute_count = tool_config.map(|t| t.flute_count).unwrap_or(2);
                    let tool_summary = tool_config
                        .map(|t| t.summary())
                        .unwrap_or_else(|| "Unknown".to_owned());
                    let tool_def = tool_config.map(build_cutter).unwrap_or_else(|| {
                        build_cutter(&ToolConfig::new_default(ToolId(0), ToolType::EndMill))
                    });

                    // §6.C / §6.I revision: prefer the move-intent signal
                    // (a toolpath containing any `MoveIntent::Drilling` move)
                    // over the op-kind heuristic. The op-kind matches stay
                    // as the fallback so an in-flight Drill/AlignmentPinDrill
                    // whose generator hasn't been migrated still gets flagged.
                    let op_type = tc.operation.op_type();
                    let has_drilling_intent = result
                        .annotated()
                        .toolpath
                        .moves
                        .iter()
                        .any(|m| matches!(m.intent, crate::toolpath::MoveIntent::Drilling));
                    // §6.E DrillOp variant is the new primary signal;
                    // intent + op-kind remain as fallbacks for the
                    // dual-representation invariant.
                    let metrics_not_applicable = result.is_drill_op()
                        || has_drilling_intent
                        || matches!(
                            op_type,
                            crate::compute::catalog::OperationType::Drill
                                | crate::compute::catalog::OperationType::AlignmentPinDrill
                        );
                    entries.push(SimToolpathEntry {
                        id: tc.id,
                        name: tc.name.clone(),
                        annotated: Arc::clone(result.annotated()),
                        tool: tool_def,
                        flute_count,
                        tool_summary,
                        semantic_trace: result.semantic_trace.as_ref().map(|t| Arc::new(t.clone())),
                        spindle_rpm: tc.operation.spindle_rpm(),
                        metrics_not_applicable,
                        drill_op: result.drill_op().cloned(),
                        operation_config_hash: crate::compute::simulate::hash_operation_config(
                            &tc.operation,
                        ),
                    });
                }
            }

            let phantom_prior_stock = phantom_scan.finish();
            // F.4: a setup whose every toolpath is still ungenerated builds
            // an empty `entries` vec — but if the FIRST enabled config in
            // plan order is a pending `FromRemainingStock` op, the "stock
            // before it" is simply the setup's untouched initial stock
            // (there are zero predecessors to distrust), so the phantom is
            // still valid and the group must still be emitted (with an
            // empty `toolpaths` vec) to carry it.
            if !entries.is_empty() || phantom_prior_stock.is_some() {
                // Per-setup local stock bbox and transform info derived from
                // the shared SetupTransformInfo helper (Phase E/D dedup).
                //
                // F-024 (2026-05-25): the per-setup dexel grid MUST span the
                // same Z range as the toolpath the simulator will stamp into
                // it. The toolpath generator's auto-default `top_z` is `0.0`
                // (see `compute/config.rs` `HeightsConfig::resolve`), so the
                // toolpath emits cut moves at Z=[0, -depth] regardless of
                // setup orientation. For identity setups (`face_up=Top`,
                // `z_rotation=Deg0`) no transform is applied to the toolpath
                // before stamping, so the dexel grid must also be in world
                // frame (`stock.origin_z..stock.origin_z + stock.z`). Using a
                // zero-rooted local bbox `(0..stock_z)` here placed the
                // cutter at world Z=-2 below every dexel ray, which clears
                // the full ray length and inflates `axial_engagement_mm` to
                // the full stock height. Falling through to `None` makes
                // `run_simulation` fall back to `request.stock_bbox` (world
                // frame) for the per-setup grid, matching the toolpath frame.
                //
                // Non-identity setups continue to use the zero-origin
                // effective bbox — that path's frame consistency (toolpath
                // emission, `local_to_global` transform shape) is outside
                // F-024's scope.
                //
                // F-030: drive these decisions from the shared
                // `SetupEvalContext`. `sim_local_stock_bbox()` returns
                // `None` for identity setups (F-024 invariant) and
                // `Some(local_stock_bbox)` paired with `local_to_global`
                // for non-identity setups.
                let setup_ctx = super::SetupEvalContext::build_for_setup(self, Some(setup));
                let local_stock_bbox = setup_ctx.sim_local_stock_bbox();
                let local_to_global = setup_ctx.local_to_global.clone();

                groups.push(SimGroupEntry {
                    toolpaths: entries,
                    direction,
                    local_stock_bbox,
                    local_to_global,
                    phantom_prior_stock,
                });
            }
        }

        // Compute effective resolution: auto-resolution matches the GUI's
        // heuristic (5 cells across the smallest tool radius, clamped to
        // [0.02, 0.5] mm, further capped so the grid stays under ~8M cells).
        let resolution = if opts.auto_resolution {
            auto_resolution_for_groups(&groups, &stock_bbox)
        } else {
            opts.resolution
        };

        // Deviation comparison happens in the simulation's stock-relative
        // global frame (0..stock_size); `SimulationRequest::model_mesh`'s
        // contract is that frame, so translate the world-space model by
        // -stock_origin here. NON-identity groups' `local_to_global`
        // outputs land in that frame directly (face/rotation transforms
        // cancel and origin is never re-added). IDENTITY groups' grids
        // are WORLD-framed (F-024) — the deviation passes frame-map their
        // query points by -stock_bbox.min themselves (see
        // `collect_column_deviations`; the first scaled-wanaka cascade
        // A/B mis-read a uniform ~−4 mm "overcut" when this half of the
        // contract was missing, 2026-07-13).
        let model_mesh = self.models.iter().find_map(|m| m.mesh.clone()).map(|m| {
            Arc::new(translate_mesh(
                &m,
                -self.stock.origin_x,
                -self.stock.origin_y,
                -self.stock.origin_z,
            ))
        });
        // F-034: opt-in kinematics-aware cycle time / F-035 predicted-feed
        // gates are threaded through as `opts.use_predicted_feed_in_gates`;
        // see `build_sim_request`'s doc comment for how this path's knobs
        // differ from `simulate_candidate_isolated`'s.
        let request = self.build_sim_request(
            groups,
            stock_bbox,
            resolution,
            SimulationMetricOptions {
                enabled: opts.metrics_enabled,
                capture_arc_engagement: opts.metrics_enabled,
            },
            model_mesh,
            opts.use_predicted_feed_in_gates,
        );

        let mut result = run_simulation(&request, cancel)?;

        // F-036b — adaptive feed modulation post-pass.
        //
        // After the simulator produces the per-sample engagement
        // record, walk it once per cutting toolpath, aggregate samples
        // into a `Vec<PerMoveEngagement>` keyed by `move_index`, look
        // up the vendor LUT's chipload band, and call
        // [`crate::feed_modulation::adaptive_feed_modulate`] on a
        // mutable clone of the toolpath. The modulated toolpath replaces
        // the cached `Arc<AnnotatedToolpath>` in `self.results` so the
        // downstream G-code emitter
        // (`crate::gcode::emit_gcode` via `export_gcode_checked`) picks
        // up the per-move modulated feeds and emits per-move F-words —
        // F-036a's emitter contract.
        //
        // Inert when:
        //  - `opts.adaptive_feed_modulation == false` (the default; the
        //    smoke baseline and every legacy test pass with this
        //    branch skipped, byte-identical).
        //  - The vendor LUT has no `chip_load_min_mm` /
        //    `chip_load_max_mm` row for the active
        //    `(tool family, material, op family, pass role, diameter)`
        //    tuple — modulator gets no `ChiploadBand`, the per-toolpath
        //    call is skipped, the IR is untouched.
        //
        // Modulation no longer requires an explicit machine `kinematics`
        // block — it falls back to `effective_kinematics` (the generic
        // wood-router profile), matching the strategy advisor. The GUI/MCP
        // sim path applies the same pass via [`modulate_simulation_trace`].
        self.modulate_simulation_trace(&mut result.cut_trace, opts);

        self.simulation = Some(result);
        // SAFETY: we just assigned Some
        #[allow(clippy::unwrap_used)]
        Ok(self.simulation.as_ref().unwrap())
    }

    /// Modulate ONE toolpath's per-move feeds against a simulation cut
    /// trace, returning the modulated [`Toolpath`] and the raw
    /// [`ModulationOutcome`] (per-move binding map + summary inputs).
    ///
    /// This is the shared F-039 core consumed by two callers:
    /// [`apply_adaptive_feed_modulation`](Self::apply_adaptive_feed_modulation)
    /// (the production post-sim pass, which stamps the result back onto
    /// `self.results`) and
    /// [`recommend_clearing_strategy`](Self::recommend_clearing_strategy)
    /// (the strategy advisor, which times the *modulated* path so it
    /// compares optimized candidates rather than raw Suggest-feed ones).
    /// Keeping the engagement aggregation + `ModulationContext` build in
    /// one place is the anti-drift discipline of the unified load model
    /// (`planning/UNIFIED_LOAD_MODEL_2026-06-18.md` §5) — the deflection
    /// cap, power cap, and chipload band are derived here once.
    ///
    /// Returns `None` when the op carries no usable RPM, has no moves, or
    /// the modulator refuses (e.g. an empty engagement vector).
    #[allow(clippy::too_many_arguments)]
    fn modulate_annotated_against_trace(
        &self,
        annotated: &crate::toolpath_spans::AnnotatedToolpath,
        operation: &crate::compute::OperationConfig,
        tool_cfg: &ToolConfig,
        toolpath_id: ToolpathId,
        cut_trace: &crate::simulation_cut::SimulationCutTrace,
        band: crate::feed_modulation::ChiploadBand,
        kinematics: crate::machine_kinematics::MachineKinematics,
        max_feed: f64,
        rapid_feed: f64,
        strategy: crate::feed_modulation::ModulationStrategy,
        aggressiveness: f64,
    ) -> Option<(
        crate::toolpath::Toolpath,
        crate::feed_modulation::ModulationOutcome,
    )> {
        use crate::feed_modulation::{
            DeflectionLimitInputs, ModulationContext, PerMoveEngagement, PowerLimitInputs,
            adaptive_feed_modulate,
        };

        let flute_count = tool_cfg.flute_count.max(1);
        let spindle_rpm = operation.spindle_rpm().unwrap_or(self.post.spindle_speed);
        if spindle_rpm == 0 {
            return None;
        }
        let move_count = annotated.toolpath.moves.len();
        if move_count == 0 {
            return None;
        }

        // Stage 4 — planner-predicted engagement for the constructive
        // contour-spiral, in two layers:
        //
        //  (a) Per-move: the spiral's own leading-arc engagement (α/2π)
        //      computed on its clean 2D material grid, carried
        //      positionally on the AnnotatedToolpath and looked up by
        //      cut-move target. RDP simplification keeps a subset of the
        //      emitted points verbatim, so kept cut moves hit exactly.
        //  (b) Uniform fallback: the op's target engagement
        //      (stepover/diameter via the F1 leading-arc → radial-WOC
        //      bridge), used for cut moves whose position isn't in the
        //      sampler (arc-fit / lead-in points) and for the 2D
        //      Adaptive spiral op, which carries no 3D sampler.
        //
        // The dexel simulator's cylinder-side `radial_woc_fraction`
        // reads ~10× low for adaptive ops (CLAUDE.md), so modulation on
        // the sim scalar alone never lets the flat-load spiral run
        // faster. Per move we take `max(sim, planner)` so any genuine
        // spike the simulator *does* resolve still wins — never feeding
        // above the higher of the two estimates. Gated strictly to the
        // ContourSpiral strategy: the Agent / AgentSearch path has real
        // ~2.5× target engagement spikes that a planner floor would
        // dangerously over-feed. See
        // planning/ADAPTIVE_CLEARING_ALGO_REVIEW_2026-06-12.md §"Stage 4".
        let is_contour_spiral = matches!(
            operation,
            crate::compute::OperationConfig::Adaptive3d(c)
                if matches!(
                    c.clearing_strategy,
                    crate::compute::operation_configs::ClearingStrategy::ContourSpiral
                )
        ) || matches!(
            operation,
            crate::compute::OperationConfig::Adaptive(c)
                if matches!(c.path_strategy, crate::adaptive::PathStrategy2d::ContourSpiral)
        );
        let planner_uniform_woc: Option<f64> = if is_contour_spiral {
            let stepover = match operation {
                crate::compute::OperationConfig::Adaptive3d(c) => Some(c.stepover),
                crate::compute::OperationConfig::Adaptive(c) => Some(c.stepover),
                _ => None,
            };
            stepover.and_then(|s| {
                let r = tool_cfg.diameter * 0.5;
                (r > 0.0 && s > 0.0).then(|| {
                    let f = crate::adaptive_shared::target_engagement_fraction(s, r);
                    crate::adaptive_shared::radial_woc_fraction_from_leading_arc(f)
                })
            })
        } else {
            None
        };
        // Position key for the per-move planner-engagement lookup
        // (0.001 mm grid — far finer than the cut-point spacing).
        let pos_key = |p: &crate::geo::P3| -> (i64, i64, i64) {
            (
                (p.x * 1000.0).round() as i64,
                (p.y * 1000.0).round() as i64,
                (p.z * 1000.0).round() as i64,
            )
        };
        let planner_map: std::collections::HashMap<(i64, i64, i64), f64> =
            if is_contour_spiral && !annotated.planner_engagement.is_empty() {
                annotated
                    .planner_engagement
                    .iter()
                    .map(|(p, f)| (pos_key(p), *f))
                    .collect()
            } else {
                std::collections::HashMap::new()
            };

        // Aggregate per-move engagement (time-weighted mean over the
        // move's samples). Samples filter on `is_cutting` so air-cut
        // and rapid moves stay at default `(0.0, 0.0)` engagement —
        // the modulator skips them via its own `should_skip` /
        // zero-engagement short-circuits.
        let mut radial_num = vec![0.0_f64; move_count];
        let mut axial_num = vec![0.0_f64; move_count];
        let mut weight_sum = vec![0.0_f64; move_count];
        for sample in &cut_trace.samples {
            if sample.toolpath_id != toolpath_id {
                continue;
            }
            if !sample.is_cutting {
                continue;
            }
            if sample.move_index >= move_count {
                continue;
            }
            let w = sample.segment_time_s.max(0.0);
            if w <= 0.0 {
                continue;
            }
            #[allow(clippy::indexing_slicing)]
            // SAFETY: move_index < move_count checked above.
            {
                radial_num[sample.move_index] += sample.engagement.radial_woc_fraction.max(0.0) * w;
                // C2: an unmeasured axial fraction contributes nothing but
                // still carries its time weight — byte-identical to the
                // pre-C2 `0.0` sentinel, and now visibly a choice. The
                // modulator's own `PerMoveEngagement` keeps a plain f64:
                // there, `0.0` legitimately means "air" (see its doc).
                axial_num[sample.move_index] +=
                    sample.engagement.axial_doc_fraction.unwrap_or(0.0).max(0.0) * w;
                weight_sum[sample.move_index] += w;
            }
        }
        let engagements: Vec<PerMoveEngagement> = (0..move_count)
            .map(|i| {
                #[allow(clippy::indexing_slicing)]
                // SAFETY: i < move_count by construction.
                let w = weight_sum[i];
                if w <= 0.0 {
                    return PerMoveEngagement::default();
                }
                #[allow(clippy::indexing_slicing)]
                // SAFETY: i < move_count by construction.
                let sim_radial = radial_num[i] / w;
                #[allow(clippy::indexing_slicing)]
                // SAFETY: i < move_count by construction.
                let axial = axial_num[i] / w;
                // Apply the planner engagement on lateral clearing /
                // finishing cuts only — entry helix, ramp, and linking
                // moves are not the spiral's flat-load wraps, so they
                // keep the sim-measured reading. Per-move sampler first,
                // uniform target floor as fallback; `max` with sim keeps
                // any genuine spike the simulator resolves.
                let m = annotated.toolpath.moves.get(i);
                let radial = if matches!(
                    m.map(|m| m.intent),
                    Some(crate::toolpath::MoveIntent::ClearingCut)
                        | Some(crate::toolpath::MoveIntent::FinishingCut)
                ) {
                    let planner_woc = m
                        .and_then(|m| planner_map.get(&pos_key(&m.target)).copied())
                        .map(crate::adaptive_shared::radial_woc_fraction_from_leading_arc)
                        .or(planner_uniform_woc);
                    match planner_woc {
                        Some(pw) => sim_radial.max(pw),
                        None => sim_radial,
                    }
                } else {
                    sim_radial
                };
                PerMoveEngagement {
                    radial_woc_fraction: radial,
                    axial_doc_fraction: axial,
                }
            })
            .collect();

        // F-039 — wire optional deflection + power constraint
        // inputs. Material + tool data is enough to recover Kc,
        // stickout, engagement diameter, and Young's modulus; the
        // machine's `power_at_rpm × safety_factor` gives the
        // available power.
        let material = &self.stock.material;
        // Materials without a primary-source Kc disable both the
        // deflection and power constraints in the constrained-max
        // solver; the solver falls through to chipload + machine +
        // kinematics caps. See `Material::kc_n_per_mm2`.
        let kc_opt = material.kc_n_per_mm2();
        let tool_def = crate::compute::cutter::build_cutter(tool_cfg);
        // Use the per-toolpath max axial DOC from the cut trace
        // as the deflection / power reference; falls back to
        // diameter when unavailable (no cutting samples → no
        // constraint active).
        let max_axial = cut_trace
            .samples
            .iter()
            .filter(|s| s.toolpath_id == toolpath_id && s.is_cutting)
            .map(|s| s.axial_engagement_mm.max(0.0))
            .fold(0.0_f64, f64::max);
        let nominal_axial = if max_axial > 0.0 { max_axial } else { 0.0 };
        let engagement_dia = tool_def.lookup_diameter_at(max_axial.max(0.0));
        let stickout = tool_def.stickout.max(0.0);
        let youngs = tool_def.tool_material.youngs_modulus_n_per_mm2();
        // Feed-aware deflection cap: the optimizer solves its feed cap
        // from the SAME affine force model (Ks/F_edge) and integrated
        // beam compliance the post-sim deflection gate uses, so the two
        // agree on a cut. Compliance is δ-per-newton at the toolpath's
        // peak axial DOC; deflection is linear in force so one scalar
        // suffices.
        let deflection_inputs = match crate::feeds::force::affine_coefficients(material) {
            Some((ks, f_edge)) if stickout > 0.0 && youngs > 0.0 => {
                let compliance = tool_def.tip_deflection_mm(1.0, max_axial.max(0.0), youngs);
                if compliance.is_finite() && compliance > 0.0 {
                    Some(DeflectionLimitInputs {
                        ks_n_per_mm2: ks,
                        f_edge_n_per_mm: f_edge,
                        compliance_mm_per_n: compliance,
                        max_tip_deflection_mm: crate::tool_load::deflection::EXCEEDS_BOUND_MM,
                    })
                } else {
                    None
                }
            }
            _ => None,
        };
        let machine_profile = &self.machine;
        let available_kw =
            machine_profile.power_at_rpm(spindle_rpm as f64) * machine_profile.safety_factor;
        let power_inputs = match kc_opt {
            Some(kc) if available_kw > 0.0 => Some(PowerLimitInputs {
                // S2-9 (2026-05-31): pass raw Kc; the solver applies
                // GRAIN_ANISOTROPY_FACTOR internally so this site
                // doesn't re-encode the multiplier literal.
                kc_n_per_mm2: kc,
                engagement_diameter_mm: engagement_dia,
                available_kw,
            }),
            _ => None,
        };

        let ctx = ModulationContext {
            spindle_rpm: spindle_rpm as f64,
            flute_count,
            max_feed_mm_min: max_feed,
            rapid_feed_mm_min: rapid_feed,
            chipload_band: band,
            kinematics: &kinematics,
            strategy,
            aggressiveness,
            deflection_inputs,
            power_inputs,
            nominal_axial_doc_mm: nominal_axial,
        };

        let mut modulated_toolpath = annotated.toolpath.clone();
        let outcome = adaptive_feed_modulate(&mut modulated_toolpath, &engagements, &ctx).ok()?;
        Some((modulated_toolpath, outcome))
    }

    /// F-039 — apply the adaptive feed-modulation post-pass to an
    /// already-computed simulation cut trace. The public entry point for the
    /// GUI/MCP sim path (`planning/UNIFIED_LOAD_MODEL_2026-06-18.md` §10.6):
    /// the in-process worker produces the trace, this applies modulation on
    /// the main thread where the session lives. Mutates the modulated
    /// toolpaths into `self.results` (so G-code export carries the optimized
    /// per-move feeds) and stamps `modulation_summaries` + re-timed runtimes
    /// onto the trace in place. Gated on `opts.adaptive_feed_modulation`;
    /// otherwise a no-op (the byte-identical baseline).
    pub fn modulate_simulation_trace(
        &mut self,
        cut_trace: &mut Option<Arc<crate::simulation_cut::SimulationCutTrace>>,
        opts: &super::SimulationOptions,
    ) {
        if !opts.adaptive_feed_modulation {
            return;
        }
        self.apply_adaptive_feed_modulation(cut_trace, opts);
    }

    /// F-036b — apply the per-move adaptive feed modulator to every
    /// computed toolpath after `run_simulation` produces its trace.
    ///
    /// Walks each toolpath's `SimulationCutSample`s, aggregates them
    /// time-weighted into a `Vec<PerMoveEngagement>` keyed by
    /// `move_index`, looks up the vendor LUT chipload band, builds the
    /// `ModulationContext`, and calls
    /// [`crate::feed_modulation::adaptive_feed_modulate`] on a `clone`
    /// of the cached `Arc<AnnotatedToolpath>::toolpath`. When the
    /// modulator reports any feed change, the `Arc<AnnotatedToolpath>`
    /// in `self.results` is swapped for a new one wrapping the modulated
    /// toolpath — `Arc::make_mut` is not used because the trace
    /// `samples` still hold the legacy `Arc` and we want them to remain
    /// pinned to the pre-modulation IR for diagnostic continuity.
    ///
    /// No-op (silently) when:
    ///  - The simulation produced no `cut_trace` (`metrics_enabled =
    ///    false`).
    ///  - A toolpath has no cut samples (drill-only / all-rapid / etc.).
    ///  - The vendor LUT has no chipload band for the toolpath.
    ///  - The modulator returns
    ///    `ModulationError::EngagementLengthMismatch` (defensive — only
    ///    fires when the toolpath has been re-generated between sim and
    ///    modulation; impossible inside `run_simulation`'s single
    ///    transaction).
    ///
    /// The chipload band source is
    /// [`crate::tool_load::chipload_envelopes_for_session`] — the same
    /// helper the chipload viewport coloring + timeline envelope readout
    /// already use, so band semantics match the rest of the load-gates
    /// surface.
    fn apply_adaptive_feed_modulation(
        &mut self,
        cut_trace: &mut Option<Arc<crate::simulation_cut::SimulationCutTrace>>,
        opts: &super::SimulationOptions,
    ) {
        // Engagement aggregation + `ModulationContext` build now live in the
        // shared `modulate_annotated_against_trace`; this pass only needs the
        // chipload band to gate which toolpaths are eligible.
        use crate::feed_modulation::ChiploadBand;

        let Some(cut_trace_ref) = cut_trace.as_deref() else {
            return;
        };
        // Modulation runs against the machine's effective kinematics — the
        // generic-wood-router fallback when no explicit block is set — so it
        // applies on every machine, matching the strategy advisor (step 5).
        let kinematics = self.machine.effective_kinematics();
        let envelopes = crate::tool_load::chipload_envelopes_for_session(self, Some(cut_trace_ref));
        if envelopes.is_empty() {
            return;
        }

        // F-039 — accumulator for the per-(toolpath_id, move_index)
        // `(feed, binding)` map and the per-toolpath
        // `ModulationSummary`. Stamped onto the cut trace below.
        let mut modulated_feeds: std::collections::BTreeMap<
            (ToolpathId, usize),
            (f64, crate::tool_load::BindingConstraint),
        > = std::collections::BTreeMap::new();
        let mut modulation_summaries: std::collections::BTreeMap<
            ToolpathId,
            crate::tool_load::ModulationSummary,
        > = std::collections::BTreeMap::new();

        // F4: modulation raises/lowers CUTTING feed — its ceiling is
        // the cutting ceiling. Rapids keep the travel rate.
        let max_feed = self.machine.cutting_feed_ceiling_mm_min().max(1.0);
        let rapid_feed = if self.post.high_feedrate_mode {
            self.post.high_feedrate.max(1.0)
        } else {
            self.machine.max_feed_mm_min.max(1.0)
        };

        // Toolpath indices to walk: enabled, with a result, with at least
        // one cut sample in the trace.
        let candidate_indices: Vec<(usize, ToolpathId)> = self
            .toolpath_configs
            .iter()
            .enumerate()
            .filter(|(idx, tc)| tc.enabled && self.results.contains_key(idx))
            .map(|(idx, tc)| (idx, tc.id))
            .collect();

        for (idx, toolpath_id) in candidate_indices {
            let Some(band_range) = envelopes.get(&toolpath_id) else {
                continue;
            };
            let Some(band) = ChiploadBand::new(band_range.start, band_range.end) else {
                continue;
            };
            let Some(tc) = self.toolpath_configs.get(idx) else {
                continue;
            };
            let Some(tool_cfg) = self.find_tool_by_raw_id(tc.tool_id) else {
                continue;
            };
            let Some(result) = self.results.get(&idx) else {
                continue;
            };
            let annotated_arc = result.annotated();
            let commanded_feed_for_summary = tc.operation.feed_rate();
            // F-039 core (shared with the strategy advisor): aggregate
            // engagement + build the deflection / power / chipload context
            // and modulate this toolpath's per-move feeds in one place.
            let Some((modulated_toolpath, outcome)) = self.modulate_annotated_against_trace(
                annotated_arc.as_ref(),
                &tc.operation,
                tool_cfg,
                toolpath_id,
                cut_trace_ref,
                band,
                kinematics,
                max_feed,
                rapid_feed,
                opts.modulation_strategy,
                opts.modulation_aggressiveness,
            ) else {
                continue;
            };
            // Stamp per-move map onto the trace-wide accumulator
            // (always — even if no feed actually changed, the
            // diagnostic surface uses this for the "all moves
            // visited" coverage signal).
            for (move_idx, value) in &outcome.per_move {
                modulated_feeds.insert((toolpath_id, *move_idx), *value);
            }
            if let Some(summary) = outcome.build_summary(
                commanded_feed_for_summary,
                opts.modulation_aggressiveness,
                opts.modulation_strategy,
            ) {
                modulation_summaries.insert(toolpath_id, summary);
            }
            if outcome.changed == 0 {
                continue;
            }
            let new_annotated = crate::toolpath_spans::AnnotatedToolpath {
                toolpath: modulated_toolpath,
                spans: annotated_arc.spans.clone(),
                spans_valid: annotated_arc.spans_valid,
                // Modulation rewrites feeds, not geometry — the planner
                // engagement samples stay valid by position.
                planner_engagement: annotated_arc.planner_engagement.clone(),
                // Rest-field overlay grid is toolpath-wide metadata, unaffected
                // by feed modulation — carry it through unchanged.
                rest_grid: annotated_arc.rest_grid.clone(),
                // Same for the derived machining-region polygons.
                rest_regions: annotated_arc.rest_regions.clone(),
            };
            let new_arc = Arc::new(new_annotated);
            // Rebuild the op_data variant with the swapped Arc.
            let new_op_data = match &result.op_data {
                crate::drill_op::OpData::Toolpath(_) => crate::drill_op::OpData::Toolpath(new_arc),
                crate::drill_op::OpData::DrillOp(drill, _) => {
                    crate::drill_op::OpData::DrillOp(Arc::clone(drill), new_arc)
                }
            };
            if let Some(slot) = self.results.get_mut(&idx) {
                slot.op_data = new_op_data;
            }
        }

        // F-036b1 sequencing fix: F-034's `apply_kinematics_cycle_time`
        // runs INSIDE `run_simulation` (before modulation), so the
        // trace's `total_runtime_s` reflects the pre-modulation
        // commanded feeds. After modulation rewrites per-move feeds,
        // re-walk every toolpath through `compute_cycle_time` and update
        // the trace's per-toolpath + project-total runtime so callers
        // (F-036c regression test, GUI panel, diagnostics summary) see
        // the modulated cycle time.
        let Some(trace_arc) = cut_trace.as_mut() else {
            return;
        };
        let trace = Arc::make_mut(trace_arc);
        let mut project_total = 0.0;
        let mut project_breakdown = crate::machine_kinematics::CycleTimeBreakdown::default();
        for tp_summary in &mut trace.toolpath_summaries {
            let Some((idx, _)) = self
                .toolpath_configs
                .iter()
                .enumerate()
                .find(|(_, tc)| tc.id == tp_summary.toolpath_id)
            else {
                project_total += tp_summary.total_runtime_s;
                continue;
            };
            let Some(result_slot) = self.results.get(&idx) else {
                project_total += tp_summary.total_runtime_s;
                continue;
            };
            let toolpath = &result_slot.annotated().toolpath;
            // Recompute the MoveIntent breakdown alongside the total —
            // leaving F-034's pre-modulation breakdown in place would
            // desynchronize `runtime_by_intent.total_s` from the
            // modulated `total_runtime_s` written below.
            let b = crate::machine_kinematics::compute_cycle_time_breakdown(
                toolpath,
                &kinematics,
                max_feed,
                rapid_feed,
            );
            tp_summary.total_runtime_s = b.total_s;
            tp_summary.runtime_by_intent = Some(b);
            project_total += b.total_s;
            project_breakdown += b;
        }
        trace.summary.total_runtime_s = project_total;
        trace.summary.runtime_by_intent = Some(project_breakdown);
        // F-039 — stamp the per-move binding map + per-toolpath
        // modulation summaries onto the trace. Both fields are
        // `#[serde(skip)]` so artifact round-tripping is unaffected;
        // the maps are re-derivable when modulation re-runs.
        trace.modulated_feeds = modulated_feeds;
        trace.modulation_summaries = modulation_summaries;
        // Make the load gates grade the MODULATED feed, not the pre-modulation
        // sample feed. The cut-trace samples carry the feed the sim ran at
        // (modulation is a post-pass), so without this the chipload / power /
        // deflection gates — which read `effective_feed_for_sample`, backed by
        // `predicted_feeds` — would report the *un-modulated* load: an under-fed
        // path reads chipload-low even though modulation raised it into band.
        // Stamping the per-move modulated feed into `predicted_feeds` closes
        // that gap, the gate↔modulation agreement the unified load model targets
        // (`planning/UNIFIED_LOAD_MODEL_2026-06-18.md` §7).
        for (&key, &(feed, _binding)) in &trace.modulated_feeds {
            trace.predicted_feeds.insert(key, feed);
        }
        // Modulation rewrote per-move feeds, so each modulated toolpath now
        // hashes differently than the pre-modulation value captured in the
        // trace's provenance — which would make `sim_trace_is_fresh` (and the
        // load report) read `StaleSimulation`, dropping the very
        // `modulation_summary` this pass just stamped. Refresh the provenance
        // toolpath hashes against the modulated IR so the trace stays FRESH
        // relative to the toolpaths it now describes. Only toolpaths the
        // trace already covers are touched (others aren't part of this sim).
        if let Some(provenance) = trace.provenance.as_mut() {
            for (idx, tc) in self.toolpath_configs.iter().enumerate() {
                if !provenance.toolpath_hashes.contains_key(&tc.id) {
                    continue;
                }
                if let Some(slot) = self.results.get(&idx) {
                    provenance.toolpath_hashes.insert(
                        tc.id,
                        crate::compute::simulate::hash_toolpath(&slot.annotated().toolpath),
                    );
                }
            }
        }
    }

    /// Run a collision check for a specific toolpath by index.
    #[instrument(skip(self))]
    pub fn collision_check(
        &self,
        index: usize,
        cancel: &AtomicBool,
    ) -> Result<CollisionCheckResult, SessionError> {
        let tc = self
            .toolpath_configs
            .get(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        let result = self
            .results
            .get(&index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        let model = self
            .find_model_by_raw_id(tc.model_id)
            .and_then(|m| m.mesh.as_ref())
            .ok_or_else(|| {
                SessionError::MissingGeometry("Collision check requires a 3D mesh".to_owned())
            })?;

        let tool = self
            .find_tool_by_raw_id(tc.tool_id)
            .ok_or(SessionError::ToolNotFound(ToolId(tc.tool_id)))?;
        let tool_def = build_cutter(tool);

        let request = CollisionCheckRequest {
            toolpath: result.toolpath(),
            tool: tool_def,
            mesh: model,
            obstacles: self.collision_obstacles_for_toolpath(index),
        };
        let check_result = run_collision_check(&request, cancel)?;
        Ok(check_result)
    }

    /// Build the fixture obstacle set for a toolpath's setup — every
    /// enabled fixture as a clearance-expanded axis-aligned box. The
    /// single source of truth shared by the core collision check above
    /// and the GUI worker path, so both flag holder-vs-fixture crashes
    /// identically (W0.1 / P6-003). Returns empty when the toolpath has
    /// no setup or no enabled fixtures.
    pub fn collision_obstacles_for_toolpath(
        &self,
        index: usize,
    ) -> Vec<crate::collision::CollisionObstacle> {
        let Some(setup) = self.find_setup_for_toolpath_index(index) else {
            return Vec::new();
        };
        setup
            .fixtures
            .iter()
            .filter(|f| f.enabled)
            .map(|f| {
                let c = f.clearance;
                crate::collision::CollisionObstacle {
                    id: f.id.0,
                    aabb: crate::geo::BoundingBox3 {
                        min: crate::geo::P3::new(f.origin_x - c, f.origin_y - c, f.origin_z - c),
                        max: crate::geo::P3::new(
                            f.origin_x + f.size_x + c,
                            f.origin_y + f.size_y + c,
                            f.origin_z + f.size_z + c,
                        ),
                    },
                }
            })
            .collect()
    }

    /// Narrate one generated toolpath in prose for agent-oriented debugging.
    #[instrument(skip(self))]
    pub fn narrate_toolpath(&self, index: usize) -> Result<String, SessionError> {
        let tc = self
            .toolpath_configs
            .get(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        let result = self
            .results
            .get(&index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        let tool = self
            .find_tool_by_raw_id(tc.tool_id)
            .ok_or(SessionError::ToolNotFound(ToolId(tc.tool_id)))?;
        let tool_def = build_cutter(tool);
        let cut_trace = self
            .simulation
            .as_ref()
            .and_then(|sim| sim.cut_trace.as_deref());
        let context = crate::narrate::ToolpathNarrationContext {
            toolpath_id: Some(tc.id),
            toolpath_name: Some(tc.name.as_str()),
            operation_label: Some(tc.operation.label()),
            operation_kind: Some(tc.operation.op_type()),
            depth_per_pass_mm: tc.operation.depth_per_pass(),
            stepover_mm: tc.operation.stepover(),
            tool_diameter_mm: Some(tool.diameter),
            feed_rate_mm_min: Some(tc.operation.feed_rate()),
            spindle_rpm: Some(
                tc.operation
                    .spindle_rpm()
                    .unwrap_or(self.post.spindle_speed),
            ),
            flute_count: Some(tool.flute_count),
            is_drill_cycle: tc.operation.op_type().is_drill_kinematics(),
            material: Some(&self.stock.material),
            // A/M9: the generation-time finding rides on this toolpath's own
            // stats. `None` here is "not measured", which narration says out
            // loud rather than rendering as a zero.
            standing_material_mm2: result.stats.standing_material_mm2,
            // Wave D1: same rule. `None` reads as "not measured" and
            // narration says so rather than staying silent.
            dropped_band: result.stats.dropped_band.as_deref().copied(),
            clipped_band: result.stats.clipped_band.as_deref().copied(),
            ramp_reach_clamp: result.stats.ramp_reach_clamp.as_deref().copied(),
            tip_float: result.stats.tip_float,
            // A/M7 gate 1: same rule — the finding rides on this
            // toolpath's own stats, and `None` means "never computed",
            // never "zero trips".
            retract_trips: result.stats.retract_trips,
        };

        Ok(crate::narrate::narrate_toolpath_with_context(
            result.annotated(),
            result.semantic_trace.as_ref(),
            cut_trace,
            result.debug_trace.as_ref(),
            &tool_def,
            &context,
        ))
    }

    /// Compute project diagnostics from current results and the
    /// session's cached simulation. Consumers that hold sim evidence
    /// outside the core session (the GUI keeps it on viz-side state)
    /// should call [`Self::diagnostics_with_evidence`] instead.
    ///
    /// This batch entry point runs the holder/shank collision sweep
    /// (one `collision_check` per computed toolpath) to build full
    /// evidence — appropriate for CLI/export, NOT for per-frame UI.
    #[instrument(skip(self))]
    pub fn diagnostics(&self) -> ProjectDiagnostics {
        let no_cancel = AtomicBool::new(false);
        let holder_collisions = self.holder_collision_counts(&no_cancel);
        let evidence = self
            .simulation
            .as_ref()
            .map(|sim| {
                ProjectEvidence::from_simulation_with_holder_collisions(
                    sim,
                    holder_collisions.clone(),
                )
            })
            .unwrap_or_else(|| ProjectEvidence {
                holder_collisions: holder_collisions.clone(),
                ..ProjectEvidence::default()
            });
        self.diagnostics_with_evidence(&evidence)
    }

    /// Run the holder/shank collision check for every computed
    /// toolpath and return `(toolpath_id, collision_count)` pairs.
    /// Toolpaths whose check fails (missing mesh, etc.) count as 0,
    /// matching the legacy in-diagnostics behavior.
    ///
    /// This is the expensive sweep (spatial-index build + interpolated
    /// toolpath walk per toolpath). Interactive surfaces should reuse
    /// their last dedicated check instead of calling this per frame.
    pub fn holder_collision_counts(&self, cancel: &AtomicBool) -> Vec<(ToolpathId, usize)> {
        self.toolpath_configs
            .iter()
            .enumerate()
            .filter(|(idx, _)| self.results.contains_key(idx))
            .map(|(idx, tc)| {
                let count = self
                    .collision_check(idx, cancel)
                    .map(|r| r.collision_report.collisions.len())
                    .unwrap_or(0);
                (tc.id, count)
            })
            .collect()
    }

    /// Same as [`Self::diagnostics`] but takes a borrow view over
    /// simulation evidence. Used by the GUI MCP handler where the
    /// active sim lives on viz-side state, not on the core session.
    ///
    /// Rapid collision counts come from the supplied evidence (which checks
    /// against the actual remaining stock surface). If no evidence is
    /// supplied, rapid collision counts are 0 — we don't fall back to the
    /// inaccurate original-bbox check.
    #[instrument(skip_all)]
    pub fn diagnostics_with_evidence(&self, evidence: &ProjectEvidence<'_>) -> ProjectDiagnostics {
        let mut per_toolpath = Vec::new();
        let mut total_collision_count: usize = 0;
        let mut total_rapid_collision_count: usize = 0;

        // Per-TP context collected in the toolpath loop for later verdict
        // emission. We index by `toolpath_id` (which equals `tc.id`).
        struct RapidWorst {
            move_index: usize,
            z: f64,
        }
        let mut holder_collisions_by_tp: Vec<(ToolpathId, String, usize)> = Vec::new();
        let mut rapid_collisions_by_tp: Vec<(ToolpathId, String, usize, RapidWorst)> = Vec::new();
        let mut empty_results_by_tp: Vec<(ToolpathId, String, &str)> = Vec::new();

        // Build per-boundary maps from the simulation result. Each boundary
        // maps to one toolpath via its `id`.
        type RapidCountsByBoundary = Vec<(ToolpathId, usize)>;
        type RapidWorstByBoundary = Vec<(ToolpathId, RapidWorst)>;
        let (rapid_counts_by_boundary, rapid_worst_by_boundary): (
            RapidCountsByBoundary,
            RapidWorstByBoundary,
        ) = {
            let counts = evidence
                .boundaries
                .iter()
                .map(|&(id, start, end)| {
                    let count = evidence
                        .rapid_collision_move_indices
                        .iter()
                        .filter(|&&mi| mi >= start && mi < end)
                        .count();
                    (id, count)
                })
                .collect();

            // Pick the worst (lowest end.z = deepest descent) rapid collision
            // per boundary so the verdict layer can cite a representative
            // move for the fix hint.
            let worst = evidence
                .boundaries
                .iter()
                .filter_map(|&(id, start, end)| {
                    evidence
                        .rapid_collisions
                        .iter()
                        .filter(|rc| rc.move_index >= start && rc.move_index < end)
                        .min_by(|a, c| {
                            a.end
                                .z
                                .partial_cmp(&c.end.z)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .map(|rc| {
                            (
                                id,
                                RapidWorst {
                                    move_index: rc.move_index,
                                    z: rc.end.z,
                                },
                            )
                        })
                })
                .collect();

            (counts, worst)
        };

        for (idx, tc) in self.toolpath_configs.iter().enumerate() {
            if let Some(result) = self.results.get(&idx) {
                // Look up rapid collision count + worst move from simulation
                // boundaries.
                let rapid_count = rapid_counts_by_boundary
                    .iter()
                    .filter(|(id, _)| *id == tc.id)
                    .map(|(_, count)| *count)
                    .sum::<usize>();
                total_rapid_collision_count += rapid_count;

                if rapid_count > 0
                    && let Some((_, worst)) =
                        rapid_worst_by_boundary.iter().find(|(id, _)| *id == tc.id)
                {
                    rapid_collisions_by_tp.push((
                        tc.id,
                        tc.name.clone(),
                        rapid_count,
                        RapidWorst {
                            move_index: worst.move_index,
                            z: worst.z,
                        },
                    ));
                }

                // Holder/shank collisions come from the supplied evidence
                // (the caller's most recent dedicated check, or
                // `holder_collision_counts` for batch paths). This used
                // to run `collision_check` inline — a spatial-index
                // build + full toolpath sweep PER TOOLPATH — which the
                // GUI setup panel then executed every frame (2026-06-11
                // setup-tab lag). Diagnostics consume evidence; they do
                // not compute it.
                let holder_collision_count = evidence
                    .holder_collisions
                    .iter()
                    .find(|(id, _)| *id == tc.id)
                    .map(|(_, count)| *count)
                    .unwrap_or(0);
                total_collision_count += holder_collision_count;

                if holder_collision_count > 0 {
                    holder_collisions_by_tp.push((tc.id, tc.name.clone(), holder_collision_count));
                }

                // C7: detect toolpaths that generated successfully but laid
                // down zero in-material cut. Drill kinematics are exempt —
                // the dexel-side cutting metric doesn't apply to Z-only ops.
                let op_type = tc.operation.op_type();
                if result.stats.cutting_distance <= 0.0 && !op_type.is_drill_kinematics() {
                    empty_results_by_tp.push((tc.id, tc.name.clone(), op_type.label()));
                }

                let tool_name = self
                    .find_tool_by_raw_id(tc.tool_id)
                    .map(|t| t.name.clone())
                    .unwrap_or_default();

                per_toolpath.push(ToolpathDiagnostic {
                    toolpath_id: tc.id,
                    name: tc.name.clone(),
                    operation_type: op_type.label().to_owned(),
                    op_kind: op_type.kind_str().to_owned(),
                    tool_name,
                    move_count: result.stats.move_count,
                    cutting_distance_mm: result.stats.cutting_distance,
                    rapid_distance_mm: result.stats.rapid_distance,
                    collision_count: holder_collision_count,
                    rapid_collision_count: rapid_count,
                    standing_material_mm2: result.stats.standing_material_mm2,
                    // Wave D1 — the MCP wire. `None` serialises null.
                    unmachined_band_area_mm2: result
                        .stats
                        .dropped_band
                        .as_deref()
                        .map(|f| f.area_mm2),
                    tip_float_points: result.stats.tip_float.map(|f| f.floating_points),
                    max_tip_float_mm: result.stats.tip_float.map(|f| f.max_float_mm),
                });
            }
        }

        // Extract simulation metrics if available.
        //
        // LH-1: air cut is published under BOTH denominators, each named.
        // The legacy `air_cut_percentage` keeps its total-runtime value —
        // every threshold in this file and in the GUI was tuned against it —
        // and the cutting-time reading (what the MCP narration reports)
        // travels beside it instead of contradicting it under the same name.
        let (
            total_runtime_s,
            air_cut_pct_of_total_runtime,
            air_cut_pct_of_cutting_time,
            average_engagement,
        ) = if let Some(trace) = evidence.cut_trace {
            use crate::simulation_cut::AirCutRatios;
            let summary = &trace.summary;
            (
                summary.total_runtime_s,
                summary.air_cut_pct_of_total_runtime(),
                summary.air_cut_pct_of_cutting_time(),
                summary.average_engagement,
            )
        } else {
            (0.0, 0.0, 0.0, 0.0)
        };

        // Per-TP op-kind-aware air-cut warnings. A blanket project-wide
        // threshold (the old `>40%`) fires falsely on projects that contain
        // even one sparse-pattern op like ProjectCurve, where 80–95% air-cut
        // is intrinsic. Per-TP thresholds live on `OperationType`
        // (see `OperationType::air_cut_high_threshold_pct`).
        // P1 — see planning/P1_AIR_CUT_THRESHOLDS_RCA.md.
        let air_cut_offenders: Vec<(String, f64)> = evidence
            .cut_trace
            .map(|trace| {
                air_cut_offenders_for_toolpaths(&trace.toolpath_summaries, &self.toolpath_configs)
            })
            .unwrap_or_default();

        // P2: plunge-stress warnings for small ball / tapered-ball tools.
        // Fix 2 caps fresh LUT recommendations, but pre-Fix-2 projects carry
        // static-default plunge rates that bypass the cap. See
        // `planning/P2_PLUNGE_STRESS_GATE_RCA.md`.
        let plunge_stress_offenders = plunge_stress_offenders_for_session(self);

        // ── Build the structured verdict list (severity-ranked) ───────
        let mut verdicts: Vec<Verdict> = Vec::new();

        // Critical: holder/shank collision per TP.
        for (id, name, count) in &holder_collisions_by_tp {
            verdicts.push(Verdict {
                severity: VerdictSeverity::Critical,
                kind: VerdictKind::HolderCollision,
                headline: format!(
                    "ERROR: holder/shank collisions on TP{id} '{name}' ({count} collisions)"
                ),
                offender_toolpath_ids: vec![*id],
                fix_hint: "Increase tool stickout, switch to a tool with a smaller holder, \
                           or raise the operation's safe-Z. Re-run collision check after \
                           the change."
                    .to_owned(),
                evidence: VerdictEvidence {
                    move_index: None,
                    z_value: None,
                    count: Some(*count),
                },
            });
        }

        // Critical: rapid-through-stock collisions per TP, with worst-move
        // context for the fix hint.
        for (id, name, count, worst) in &rapid_collisions_by_tp {
            verdicts.push(Verdict {
                severity: VerdictSeverity::Critical,
                kind: VerdictKind::RapidCollision,
                headline: format!(
                    "WARNING: rapid collisions on TP{id} '{name}' ({count} collisions, \
                     worst at move {move_index}, z={z:.3})",
                    move_index = worst.move_index,
                    z = worst.z,
                ),
                offender_toolpath_ids: vec![*id],
                fix_hint: "Likely cause: inter-region rapid moves not lifting to safe-Z. \
                           Fix: increase retract_z in operation params, raise safe-Z on \
                           the post-config, or set a tighter boundary so the lift path \
                           clears already-cut regions."
                    .to_owned(),
                evidence: VerdictEvidence {
                    move_index: Some(worst.move_index),
                    z_value: Some(worst.z),
                    count: Some(*count),
                },
            });
        }

        // Important: unsafe plunge feed on small ball / tapered-ball tools.
        for (name, rate, cap) in &plunge_stress_offenders {
            verdicts.push(Verdict {
                severity: VerdictSeverity::Important,
                kind: VerdictKind::PlungeStress,
                headline: format!(
                    "WARNING: unsafe plunge rate on '{name}' ({rate:.0} > cap {cap:.0} mm/min)"
                ),
                offender_toolpath_ids: self
                    .toolpath_configs
                    .iter()
                    .filter(|tc| tc.name == *name)
                    .map(|tc| tc.id)
                    .collect(),
                fix_hint: format!(
                    "Cap plunge feed at {cap:.0} mm/min for this tool geometry. Use \
                     the Feeds tab Suggest button to repopulate with safe values."
                ),
                evidence: VerdictEvidence {
                    move_index: None,
                    z_value: None,
                    count: None,
                },
            });
        }

        // Important: toolpath generated but produced zero in-material cut.
        for (id, name, op_label) in &empty_results_by_tp {
            verdicts.push(Verdict {
                severity: VerdictSeverity::Important,
                kind: VerdictKind::GeneratedEmpty,
                headline: format!(
                    "WARNING: TP{id} '{name}' ({op_label}) generated but produced zero \
                     in-material cut"
                ),
                offender_toolpath_ids: vec![*id],
                fix_hint: "Check that the setup orientation, stock alignment, and target \
                           model geometry overlap. For project_curve specifically, \
                           `depth` follows the \"positive = into material\" convention — \
                           a negative depth lifts the cutter into air. The \
                           project_curve_negative_depth validator rule flags this \
                           pre-generation; the project_summary.stale_defaults entry has \
                           a one-click flip-sign fix."
                    .to_owned(),
                evidence: VerdictEvidence {
                    move_index: None,
                    z_value: None,
                    count: None,
                },
            });
        }

        // Polish: air-cut high (per-TP, op-kind-aware threshold).
        if !air_cut_offenders.is_empty() {
            let names: Vec<String> = air_cut_offenders
                .iter()
                // LH-1: the threshold is on the TOTAL-RUNTIME measure; the
                // headline says so rather than leaving "air-cut" ambiguous.
                .map(|(n, pct)| format!("'{n}' is {pct:.0}% air-cut of total runtime"))
                .collect();
            let offender_ids: Vec<ToolpathId> = air_cut_offenders
                .iter()
                .filter_map(|(n, _)| {
                    self.toolpath_configs
                        .iter()
                        .find(|tc| tc.name == *n)
                        .map(|tc| tc.id)
                })
                .collect();
            verdicts.push(Verdict {
                severity: VerdictSeverity::Polish,
                kind: VerdictKind::AirCut,
                headline: format!("WARNING: high air cutting on {}", names.join(", ")),
                offender_toolpath_ids: offender_ids,
                fix_hint: "Tighten the boundary, lower the stock-top, or pre-rough with \
                           a faster op so the finishing pass doesn't traverse uncut \
                           material. Negligible for sparse projection ops."
                    .to_owned(),
                evidence: VerdictEvidence {
                    move_index: None,
                    z_value: None,
                    count: None,
                },
            });
        }

        // Sort by severity (Critical → Important → Polish). Within a
        // severity the insertion order above is the intended display order.
        verdicts.sort_by_key(|v| v.severity);

        // Legacy single-line verdict for backward-compat callers: take the
        // highest-severity entry's headline; "OK" when empty.
        let verdict = verdicts
            .first()
            .map(|v| v.headline.clone())
            .unwrap_or_else(|| "OK".to_owned());

        ProjectDiagnostics {
            total_runtime_s,
            air_cut_percentage: air_cut_pct_of_total_runtime,
            air_cut_pct_of_total_runtime,
            air_cut_pct_of_cutting_time,
            average_engagement,
            collision_count: total_collision_count,
            rapid_collision_count: total_rapid_collision_count,
            per_toolpath,
            verdict,
            verdicts,
        }
    }

    // ── Export ──────────────────────────────────────────────────────

    /// Export G-code for all computed toolpaths under the default tool-load
    /// policy (refuse on Exceeds or Unmodeled). For an override-capable
    /// variant see [`export_gcode_with_policy`].
    #[instrument(skip(self))]
    pub fn export_gcode(&self, path: &Path) -> Result<(), SessionError> {
        self.export_gcode_with_policy(path, crate::gcode::ToolLoadExportPolicy::default())
    }

    /// Export G-code with an explicit tool-load policy. Used by callers that
    /// want to override `Exceeds` or `Unmodeled` verdicts (UI checkbox,
    /// `--accept-…` CLI flag, MCP parameter).
    #[instrument(skip(self))]
    pub fn export_gcode_with_policy(
        &self,
        path: &Path,
        policy: crate::gcode::ToolLoadExportPolicy,
    ) -> Result<(), SessionError> {
        let gcode = crate::gcode::export_gcode_checked(
            self,
            self.simulation
                .as_ref()
                .and_then(|simulation| simulation.cut_trace.as_deref()),
            policy,
        )
        .map_err(|e| SessionError::Export(e.to_string()))?;
        std::fs::write(path, gcode).map_err(|e| {
            SessionError::Export(format!("Failed to write G-code to {}: {e}", path.display()))
        })
    }

    /// Compute the per-toolpath tool-load report against current state. Used
    /// by `get_tool_load_report` (MCP) and the export gate. Returns deflection
    /// + chipload populated; power is `Unmodeled(NotImplemented)` until
    ///
    /// Phase 1b lands the arc-engagement-driven power criterion.
    pub fn tool_load_report(&self) -> crate::tool_load::ToolLoadReport {
        let sim_trace = self
            .simulation
            .as_ref()
            .and_then(|simulation| simulation.cut_trace.as_deref());
        crate::gcode::project_load_report(self, sim_trace)
    }

    /// Compute the unified diagnostic list for a single toolpath
    /// using the session's own cached simulation. Consumers that
    /// have a sim trace held outside the core session (the GUI
    /// keeps its trace on the viz-side state) should call
    /// [`Self::diagnose_toolpath_with_trace`] instead — otherwise
    /// the load gates will read as `NeedsSimulation` even when a
    /// fresh trace exists elsewhere.
    ///
    /// Returns the list with [`crate::diagnostics::apply_supersession`]
    /// already applied — heuristic pre-sim hints vanish when sim
    /// evidence is current.
    pub fn diagnose_toolpath(
        &self,
        index: usize,
    ) -> Result<Vec<crate::diagnostics::Diagnostic>, SessionError> {
        let sim_trace = self
            .simulation
            .as_ref()
            .and_then(|sim| sim.cut_trace.as_deref());
        self.diagnose_toolpath_with_trace(index, sim_trace)
    }

    /// Same as [`Self::diagnose_toolpath`] but takes an explicit
    /// sim trace. Used by the GUI MCP handler where the active sim
    /// trace lives on the viz-side state, not on the core session.
    ///
    /// PR-5 polish: computes heights via [`Self::height_context_for_toolpath`]
    /// and the feeds-calculator result via [`Self::feeds_result_for_toolpath`],
    /// so MCP consumers see the same heights / pre-sim hint diagnostics the
    /// GUI panel renders.
    pub fn diagnose_toolpath_with_trace(
        &self,
        index: usize,
        sim_trace: Option<&crate::simulation_cut::SimulationCutTrace>,
    ) -> Result<Vec<crate::diagnostics::Diagnostic>, SessionError> {
        let tc = self
            .toolpath_configs
            .get(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        let tool = self
            .find_tool_by_raw_id(tc.tool_id)
            .ok_or(SessionError::ToolNotFound(ToolId(tc.tool_id)))?;
        let stale_defaults = crate::compute::validate::validate_one_toolpath(
            tc,
            Some(tool),
            &self.stock.material,
            self.stock_bbox().min.z,
        );
        // Build the load report from the *supplied* trace so callers
        // can plug in viz-side traces. `gcode::project_load_report`
        // applies its own staleness check via `sim_trace_is_fresh`.
        let report = crate::gcode::project_load_report(self, sim_trace);
        let load_verdict = report.per_toolpath.iter().find(|v| v.toolpath_id == tc.id);

        let height_ctx = self.height_context_for_toolpath(tc);
        let heights =
            crate::diagnostics::adapters::from_static_checks::ResolvedHeights::from_context(
                &height_ctx,
            );
        let feeds_result = self.feeds_result_for_toolpath(tc, tool);
        let preconditions = self.precondition_context_for_toolpath(tc);
        let model_refs = self.model_ref_context_for_toolpath(tc);
        // Generation-time findings ride on the stats of this toolpath's own
        // generated result. Absent until it has been generated, which is
        // exactly when there is nothing to report.
        let stats = self
            .toolpath_configs
            .iter()
            .position(|t| t.id == tc.id)
            .and_then(|idx| self.results.get(&idx))
            .map(|r| &r.stats);

        let inputs = crate::diagnostics::ToolpathDiagnoseInputs {
            toolpath_id: tc.id,
            operation: &tc.operation,
            tool,
            heights: Some(&heights),
            feeds_result: feeds_result.as_ref(),
            load_verdict,
            stale_defaults: &stale_defaults,
            preconditions: Some(&preconditions),
            model_refs: Some(&model_refs),
            stats,
        };
        Ok(crate::diagnostics::diagnose_toolpath_inputs(&inputs))
    }

    /// Build a [`ModelRefContext`] for a single toolpath (F-023).
    /// Captures whether the toolpath's `model_id` resolves against the
    /// project's loaded models so the unified diagnostic stream emits
    /// the same "Selected model missing" signal the GUI banner has
    /// always shown.
    fn model_ref_context_for_toolpath(
        &self,
        tc: &super::ToolpathConfig,
    ) -> crate::diagnostics::diagnose::ModelRefContext {
        crate::diagnostics::diagnose::ModelRefContext {
            model_id: tc.model_id,
            model_resolved: self.models.iter().any(|m| m.id == tc.model_id),
        }
    }

    /// Build a [`PreconditionContext`] for a single toolpath. Captures
    /// the prior toolpaths in the same setup (so the rest-machining
    /// precondition can verify a larger upstream tool exists), the
    /// target model's geometry kind (so the drill / project-curve
    /// preconditions can verify a curve / mesh source is in scope),
    /// and the per-tool diameters used by the rest "prev tool must be
    /// larger" check.
    ///
    /// Pure helper — no I/O, no mutation. Safe to call repeatedly per
    /// diagnose round.
    fn precondition_context_for_toolpath(
        &self,
        tc: &super::ToolpathConfig,
    ) -> crate::diagnostics::diagnose::PreconditionContext {
        use crate::diagnostics::diagnose::{
            PreconditionContext, PriorToolpathSummary, TargetModelGeometry, ToolDiameterEntry,
        };

        // Find the setup that owns this toolpath and collect prior
        // toolpaths in that setup (lower display index than `tc`).
        let mut prior_toolpaths_in_setup = Vec::new();
        if let Some(setup) = self.find_setup_for_toolpath_id(tc.id) {
            for &idx in &setup.toolpath_indices {
                if let Some(other) = self.toolpath_configs.get(idx) {
                    if other.id == tc.id {
                        break;
                    }
                    prior_toolpaths_in_setup.push(PriorToolpathSummary {
                        enabled: other.enabled,
                        tool_id: other.tool_id,
                        model_id: other.model_id,
                    });
                }
            }
        }

        let target_model =
            self.models
                .iter()
                .find(|m| m.id == tc.model_id)
                .map(|m| TargetModelGeometry {
                    has_polygons: m.polygons.as_ref().is_some_and(|p| !p.is_empty()),
                    has_mesh: m.mesh.is_some(),
                });

        let any_loaded_model_has_mesh = self.models.iter().any(|m| m.mesh.is_some());

        let tool_diameters = self
            .tools
            .iter()
            .map(|t| ToolDiameterEntry {
                id: t.id,
                diameter: t.diameter,
            })
            .collect();

        PreconditionContext {
            prior_toolpaths_in_setup,
            target_model,
            any_loaded_model_has_mesh,
            tool_diameters,
        }
    }

    /// Build a [`HeightContext`] for a given toolpath. Mirrors the GUI's
    /// `height_context_from_session` so MCP consumers see the same heights
    /// diagnostics the params panel renders.
    ///
    /// F-030: stock-frame + transform decisions delegate to
    /// [`super::SetupEvalContext`] so this path and the generation path
    /// (`generate_toolpath`) share a single source of truth for
    /// `stock_top_z` / `safe_z` / model-bbox-in-frame.
    pub fn height_context_for_toolpath(
        &self,
        tc: &super::ToolpathConfig,
    ) -> crate::compute::config::HeightContext {
        let setup = self.find_setup_for_toolpath_id(tc.id);
        let ctx = super::SetupEvalContext::build_for_setup(self, setup);
        let raw_mb = self
            .models
            .iter()
            .find(|m| m.id == tc.model_id)
            .and_then(|m| {
                m.mesh.as_ref().map(|mesh| mesh.bbox).or_else(|| {
                    m.polygons
                        .as_deref()
                        .and_then(|v| super::mutation::polygons_bbox(v))
                })
            });
        // Apply the setup transform that owns this toolpath so
        // model_top/bottom_z are in the setup-local frame for non-identity
        // setups. Identity setups leave the world bbox untouched.
        let mb = match (raw_mb, ctx.local_to_global.as_ref()) {
            (Some(b), Some(info)) => Some(transform_bbox_world_to_local(&b, info)),
            (Some(b), None) => Some(b),
            _ => None,
        };
        let heights_bbox = ctx.heights_stock_bbox;
        crate::compute::config::HeightContext {
            safe_z: ctx.safe_z,
            op_depth: tc.operation.default_depth_for_heights(),
            stock_top_z: heights_bbox.max.z,
            stock_bottom_z: heights_bbox.min.z,
            model_top_z: mb.map(|b| b.max.z),
            model_bottom_z: mb.map(|b| b.min.z),
        }
    }

    /// Run the feeds calculator for a single toolpath against the session's
    /// material/machine/post — used by [`Self::diagnose_toolpath_with_trace`]
    /// to surface the same feeds warnings + pre-sim heuristic hints the GUI
    /// params panel emits.
    ///
    /// Returns `None` for op kinds whose feeds_style doesn't model cutting
    /// (e.g. tool-change-only ops), in which case the heuristic hint
    /// adapter is skipped.
    pub fn feeds_result_for_toolpath(
        &self,
        tc: &super::ToolpathConfig,
        tool: &ToolConfig,
    ) -> Option<crate::feeds::FeedsResult> {
        // A refused tool × operation pairing (e.g. flat endmill on a
        // Scallop op) maps to `None` here — the diagnostics layer
        // treats "no recipe available" the same as "feeds_style
        // doesn't model cutting".
        crate::feeds::suggest::feeds_result_for_operation(
            &tc.operation,
            tool,
            &self.stock.material,
            &self.machine,
            self.stock.workholding_rigidity,
            crate::feeds::embedded_vendor_lut(),
            self.post.spindle_strategy,
        )
        .ok()
    }

    /// Project-wide diagnostics derived from the
    /// [`ProjectDiagnostics`] snapshot using the session's cached
    /// simulation. Consumers with sim evidence outside the session
    /// (the GUI) should call [`Self::diagnose_project_with_evidence`].
    pub fn diagnose_project(&self) -> Vec<crate::diagnostics::Diagnostic> {
        // Batch entry point — same full-evidence semantics as
        // `diagnostics()`, including the holder-collision sweep.
        let diag = self.diagnostics();
        crate::diagnostics::diagnose_project_diagnostics(&diag)
    }

    /// Same as [`Self::diagnose_project`] but takes a borrow view
    /// over sim evidence. Used by the GUI MCP handler.
    pub fn diagnose_project_with_evidence(
        &self,
        evidence: &ProjectEvidence<'_>,
    ) -> Vec<crate::diagnostics::Diagnostic> {
        let diag = self.diagnostics_with_evidence(evidence);
        crate::diagnostics::diagnose_project_diagnostics(&diag)
    }

    /// Export diagnostics as JSON files to an output directory.
    #[instrument(skip(self))]
    pub fn export_diagnostics_json(&self, output_dir: &Path) -> Result<(), SessionError> {
        std::fs::create_dir_all(output_dir)?;
        let diag = self.diagnostics();
        let json = serde_json::to_string_pretty(&diag)
            .map_err(|e| SessionError::Export(format!("Failed to serialize diagnostics: {e}")))?;
        let path = output_dir.join("summary.json");
        std::fs::write(&path, json)
            .map_err(|e| SessionError::Export(format!("Failed to write {}: {e}", path.display())))
    }
}

/// Identify toolpaths whose air-cut percentage exceeds their op-kind's
/// high-water threshold. Returns `(toolpath_name, air_cut_pct)` pairs.
///
/// Pure helper; takes only the data it needs so it can be unit-tested
/// without constructing a full `SimulationResult`. See
/// `planning/P1_AIR_CUT_THRESHOLDS_RCA.md` for the threshold rationale.
fn air_cut_offenders_for_toolpaths(
    toolpath_summaries: &[crate::simulation_cut::SimulationToolpathCutSummary],
    toolpath_configs: &[super::ToolpathConfig],
) -> Vec<(String, f64)> {
    use crate::simulation_cut::AirCutRatios;

    let mut offenders = Vec::new();
    for tp_summary in toolpath_summaries {
        if tp_summary.total_runtime_s <= 0.0 {
            continue;
        }
        let Some(tc) = toolpath_configs
            .iter()
            .find(|t| t.id == tp_summary.toolpath_id)
        else {
            continue;
        };
        let Some(threshold) = tc.operation.op_type().air_cut_high_threshold_pct() else {
            continue;
        };
        // LH-1: `air_cut_high_threshold_pct` is defined against TOTAL runtime
        // (cutting + rapids). Named accessor, not a bare division, so the
        // choice is visible here and cannot silently drift to the
        // cutting-time reading the MCP narration prints.
        let air_pct = tp_summary.air_cut_pct_of_total_runtime();
        if air_pct > threshold {
            offenders.push((tc.name.clone(), air_pct));
        }
    }
    offenders
}

/// Identify toolpaths whose configured plunge rate exceeds the safe cap
/// for the tool's geometry (ball / tapered-ball). Returns `(name,
/// plunge_rate, cap)` triples in toolpath order.
///
/// See `planning/P2_PLUNGE_STRESS_GATE_RCA.md`.
fn plunge_stress_offenders_for_session(session: &ProjectSession) -> Vec<(String, f64, f64)> {
    use crate::compute::tool_config::ToolId;
    use crate::tool::MillingCutter;
    use crate::tool_load::plunge_stress::check_plunge_stress;

    let mut offenders = Vec::new();
    for tc in &session.toolpath_configs {
        if !tc.enabled {
            continue;
        }
        let Some(tool_cfg) = session.get_tool(ToolId(tc.tool_id)) else {
            continue;
        };
        let tool_def = crate::compute::cutter::build_cutter(tool_cfg);
        let geometry = tool_def.to_geometry_hint();
        let plunge_rate = tc.operation.plunge_rate();
        if plunge_rate <= 0.0 {
            continue;
        }
        if let Some(w) = check_plunge_stress(geometry, tool_def.diameter(), plunge_rate) {
            offenders.push((tc.name.clone(), w.plunge_rate_mm_min, w.safe_cap_mm_min));
        }
    }
    offenders
}

/// Compute auto-resolution from simulation groups and stock bbox.
///
/// Mirrors the GUI's `auto_resolution_for_tools` heuristic:
/// - 5 cells across the smallest tool radius for decent curve resolution
/// - Clamped to [0.02, 0.5] mm
/// - Further limited so the grid stays under ~8M cells
fn auto_resolution_for_groups(groups: &[SimGroupEntry], stock_bbox: &BoundingBox3) -> f64 {
    use crate::tool::MillingCutter as _;

    let min_radius = groups
        .iter()
        .flat_map(|g| g.toolpaths.iter())
        .map(|entry| entry.tool.radius())
        .fold(f64::INFINITY, f64::min);

    // 5 cells across the radius gives decent curve resolution
    let from_tool = (min_radius / 5.0).clamp(0.02, 0.5);

    // Cap so grid stays under ~8M cells (reasonable memory / mesh size)
    let max_cells: f64 = 8_000_000.0;
    let sx = stock_bbox.max.x - stock_bbox.min.x;
    let sy = stock_bbox.max.y - stock_bbox.min.y;
    let from_grid = ((sx * sy) / max_cells).sqrt().max(0.02);

    from_tool.max(from_grid)
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
    use crate::compute::catalog::OperationConfig;
    use crate::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig};
    use crate::compute::operation_configs::{DrillConfig, PocketConfig, RestConfig};
    use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
    use crate::debug_trace::ToolpathDebugOptions;
    use crate::gcode::CoolantMode;
    use crate::session::ToolpathConfig;
    use serde_json::json;

    fn make_session() -> ProjectSession {
        let mut s = ProjectSession::new_empty();
        let tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        s.add_tool(tool);
        s
    }

    fn make_tc(tool_id: usize) -> ToolpathConfig {
        ToolpathConfig {
            id: ToolpathId(0),
            name: "test".to_owned(),
            enabled: true,
            operation: OperationConfig::Pocket(PocketConfig::default()),
            dressups: DressupConfig::default(),
            heights: HeightsConfig::default(),
            tool_id,
            model_id: 0,
            pre_gcode: None,
            post_gcode: None,
            boundary: BoundaryConfig::default(),
            boundary_inherit: true,
            stock_source: crate::session::StockSource::Fresh,
            coolant: CoolantMode::Off,
            face_selection: None,
            debug_options: ToolpathDebugOptions::default(),
            feeds_provenance: crate::feeds::FeedsProvenance::default(),
            rest_analysis: crate::compute::config::RestAnalysisConfig::default(),
        }
    }

    // ── set_toolpath_param ───────────────────────────────────────

    #[test]
    fn set_toolpath_param_feed_rate() {
        let mut s = make_session();
        s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
        s.set_toolpath_param(0, "feed_rate", json!(2000.0)).unwrap();
        // Verify via OperationParams trait
        match &s.toolpath_configs()[0].operation {
            OperationConfig::Pocket(cfg) => assert!((cfg.feed_rate - 2000.0).abs() < 1e-9),
            _ => panic!("expected Pocket"),
        }
    }

    #[test]
    fn set_toolpath_param_plunge_rate() {
        let mut s = make_session();
        s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
        s.set_toolpath_param(0, "plunge_rate", json!(500.0))
            .unwrap();
        match &s.toolpath_configs()[0].operation {
            OperationConfig::Pocket(cfg) => assert!((cfg.plunge_rate - 500.0).abs() < 1e-9),
            _ => panic!("expected Pocket"),
        }
    }

    #[test]
    fn set_toolpath_param_stepover() {
        let mut s = make_session();
        s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
        s.set_toolpath_param(0, "stepover", json!(0.5)).unwrap();
        match &s.toolpath_configs()[0].operation {
            OperationConfig::Pocket(cfg) => assert!((cfg.stepover - 0.5).abs() < 1e-9),
            _ => panic!("expected Pocket"),
        }
    }

    #[test]
    fn set_toolpath_param_depth_per_pass() {
        let mut s = make_session();
        s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
        s.set_toolpath_param(0, "depth_per_pass", json!(1.5))
            .unwrap();
        match &s.toolpath_configs()[0].operation {
            OperationConfig::Pocket(cfg) => assert!((cfg.depth_per_pass - 1.5).abs() < 1e-9),
            _ => panic!("expected Pocket"),
        }
    }

    #[test]
    fn set_toolpath_param_coerces_integer_to_bool() {
        // MCP clients that can only produce JSON numbers should still be
        // able to set boolean params like `climb`, `z_blend`, etc.
        let mut s = make_session();
        s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
        // Pocket default has climb=true; flip it via integer 0.
        s.set_toolpath_param(0, "climb", json!(0)).unwrap();
        match &s.toolpath_configs()[0].operation {
            OperationConfig::Pocket(cfg) => assert!(!cfg.climb),
            _ => panic!("expected Pocket"),
        }
        // Flip back with integer 1.
        s.set_toolpath_param(0, "climb", json!(1)).unwrap();
        match &s.toolpath_configs()[0].operation {
            OperationConfig::Pocket(cfg) => assert!(cfg.climb),
            _ => panic!("expected Pocket"),
        }
        // Non-0/1 integers fall through to serde, which will reject them.
        let result = s.set_toolpath_param(0, "climb", json!(42));
        assert!(matches!(result, Err(SessionError::InvalidParam(_))));
        // Actual booleans still work.
        s.set_toolpath_param(0, "climb", json!(false)).unwrap();
        match &s.toolpath_configs()[0].operation {
            OperationConfig::Pocket(cfg) => assert!(!cfg.climb),
            _ => panic!("expected Pocket"),
        }
    }

    fn make_rest_tc(tool_id: usize) -> ToolpathConfig {
        let mut tc = make_tc(tool_id);
        tc.operation = OperationConfig::Rest(RestConfig::default());
        tc
    }

    fn make_drill_tc(tool_id: usize) -> ToolpathConfig {
        let mut tc = make_tc(tool_id);
        tc.operation = OperationConfig::Drill(DrillConfig::default());
        tc
    }

    #[test]
    fn set_toolpath_param_drill_plunge_rate_updates_feed_rate() {
        let mut s = make_session();
        s.add_toolpath(0, make_drill_tc(s.tools()[0].id.0)).unwrap();
        s.set_toolpath_param(0, "plunge_rate", json!(250.0))
            .unwrap();
        match &s.toolpath_configs()[0].operation {
            OperationConfig::Drill(cfg) => assert_eq!(cfg.feed_rate, 250.0),
            _ => panic!("expected Drill"),
        }
    }

    #[test]
    fn set_toolpath_param_prev_tool_id_accepts_int() {
        let mut s = make_session();
        s.add_toolpath(0, make_rest_tc(s.tools()[0].id.0)).unwrap();
        s.set_toolpath_param(0, "prev_tool_id", json!(1)).unwrap();
        match &s.toolpath_configs()[0].operation {
            OperationConfig::Rest(cfg) => assert_eq!(cfg.prev_tool_id, Some(ToolId(1))),
            _ => panic!("expected Rest"),
        }
    }

    #[test]
    fn set_toolpath_param_prev_tool_id_accepts_string() {
        let mut s = make_session();
        s.add_toolpath(0, make_rest_tc(s.tools()[0].id.0)).unwrap();
        s.set_toolpath_param(0, "prev_tool_id", json!("1")).unwrap();
        match &s.toolpath_configs()[0].operation {
            OperationConfig::Rest(cfg) => assert_eq!(cfg.prev_tool_id, Some(ToolId(1))),
            _ => panic!("expected Rest"),
        }
    }

    #[test]
    fn set_toolpath_param_prev_tool_id_accepts_float_wire_number() {
        let mut s = make_session();
        s.add_toolpath(0, make_rest_tc(s.tools()[0].id.0)).unwrap();
        s.set_toolpath_param(0, "prev_tool_id", json!(1.0)).unwrap();
        match &s.toolpath_configs()[0].operation {
            OperationConfig::Rest(cfg) => assert_eq!(cfg.prev_tool_id, Some(ToolId(1))),
            _ => panic!("expected Rest"),
        }
    }

    #[test]
    fn set_toolpath_param_prev_tool_id_accepts_null_to_clear() {
        let mut s = make_session();
        s.add_toolpath(0, make_rest_tc(s.tools()[0].id.0)).unwrap();
        s.set_toolpath_param(0, "prev_tool_id", json!(1)).unwrap();
        s.set_toolpath_param(0, "prev_tool_id", serde_json::Value::Null)
            .unwrap();
        match &s.toolpath_configs()[0].operation {
            OperationConfig::Rest(cfg) => assert_eq!(cfg.prev_tool_id, None),
            _ => panic!("expected Rest"),
        }
    }

    #[test]
    fn get_operation_schema_rest_lists_prev_tool_id() {
        let schema = ProjectSession::operation_schema("rest").unwrap();
        let prev = schema
            .params
            .iter()
            .find(|param| param.name == "prev_tool_id")
            .unwrap();
        assert_eq!(prev.type_name, "option<usize>");
        assert!(prev.optional);
        assert_eq!(prev.default, serde_json::Value::Null);
    }

    #[test]
    fn get_operation_schema_unknown_op_returns_error() {
        let err = ProjectSession::operation_schema("not_real")
            .unwrap_err()
            .to_string();
        assert!(err.contains("Valid operation_type values"));
        assert!(err.contains("rest"));
    }

    #[test]
    fn get_operation_schema_drill_has_drill_specific_fields() {
        let schema = ProjectSession::operation_schema("drill").unwrap();
        let names: std::collections::HashSet<_> = schema
            .params
            .iter()
            .map(|param| param.name.as_str())
            .collect();
        assert!(names.contains("cycle"));
        assert!(names.contains("peck_depth"));
        assert!(names.contains("retract_z"));
    }

    #[test]
    fn operation_schema_params_match_params_with_nulls_for_every_op() {
        for &op_type in crate::compute::catalog::OperationType::ALL {
            let op = OperationConfig::new_default(op_type);
            let params = op.params_value_including_nulls();
            let param_obj = params.as_object().unwrap();
            let schema = OperationConfig::schema_for_type(op_type);
            let schema_names: std::collections::HashSet<_> = schema
                .params
                .iter()
                .map(|param| param.name.as_str())
                .collect();
            let param_names: std::collections::HashSet<_> =
                param_obj.keys().map(String::as_str).collect();
            assert_eq!(schema_names, param_names, "schema mismatch for {op_type:?}");
        }
    }

    #[test]
    fn set_toolpath_param_wrong_type_errors() {
        let mut s = make_session();
        s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
        let result = s.set_toolpath_param(0, "feed_rate", json!("not a number"));
        assert!(matches!(result, Err(SessionError::InvalidParam(_))));
    }

    #[test]
    fn set_toolpath_param_unknown_param() {
        let mut s = make_session();
        s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
        let result = s.set_toolpath_param(0, "totally_fake_param", json!(42.0));
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unknown parameter 'totally_fake_param'"));
        assert!(err.contains("Valid parameters"));
        assert!(err.contains("stepover"));
    }

    #[test]
    fn set_toolpath_param_invalid_index() {
        let mut s = make_session();
        let result = s.set_toolpath_param(99, "feed_rate", json!(100.0));
        assert!(matches!(result, Err(SessionError::ToolpathNotFound(99))));
    }

    #[test]
    fn set_toolpath_param_spindle_rpm() {
        let mut s = make_session();
        s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
        // Default is None.
        assert_eq!(s.toolpath_configs()[0].operation.spindle_rpm(), None);
        s.set_toolpath_param(0, "spindle_rpm", json!(15000))
            .unwrap();
        assert_eq!(s.toolpath_configs()[0].operation.spindle_rpm(), Some(15000));
    }

    #[test]
    fn set_toolpath_param_spindle_rpm_null_clears() {
        let mut s = make_session();
        s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
        s.set_toolpath_param(0, "spindle_rpm", json!(20_000))
            .unwrap();
        assert_eq!(
            s.toolpath_configs()[0].operation.spindle_rpm(),
            Some(20_000)
        );
        s.set_toolpath_param(0, "spindle_rpm", serde_json::Value::Null)
            .unwrap();
        assert_eq!(s.toolpath_configs()[0].operation.spindle_rpm(), None);
    }

    #[test]
    fn set_toolpath_param_spindle_rpm_invalid_type() {
        let mut s = make_session();
        s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
        let result = s.set_toolpath_param(0, "spindle_rpm", json!("not a number"));
        assert!(matches!(result, Err(SessionError::InvalidParam(_))));
        // Negative numbers fail u64 conversion.
        let result = s.set_toolpath_param(0, "spindle_rpm", json!(-1));
        assert!(matches!(result, Err(SessionError::InvalidParam(_))));
    }

    /// F2 — MCP / JSON-RPC clients sometimes serialize integer literals as
    /// f64 (so 13500 arrives as 13500.0). The router accepts integer-valued
    /// floats and parseable numeric strings as well as plain integers.
    #[test]
    fn set_toolpath_param_spindle_rpm_accepts_f64_and_string() {
        let mut s = make_session();
        s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
        // Integer-valued f64.
        s.set_toolpath_param(0, "spindle_rpm", json!(13500.0))
            .unwrap();
        assert_eq!(s.toolpath_configs()[0].operation.spindle_rpm(), Some(13500));
        // Numeric string.
        s.set_toolpath_param(0, "spindle_rpm", json!("18000"))
            .unwrap();
        assert_eq!(s.toolpath_configs()[0].operation.spindle_rpm(), Some(18000));
        // Non-integer float is rejected (would lose precision).
        let result = s.set_toolpath_param(0, "spindle_rpm", json!(13500.5));
        assert!(matches!(result, Err(SessionError::InvalidParam(_))));
    }

    #[test]
    fn set_toolpath_param_invalidates_result() {
        let mut s = make_session();
        s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
        s.results.insert(
            0,
            ToolpathComputeResult {
                op_data: crate::drill_op::OpData::Toolpath(Arc::new(
                    crate::toolpath_spans::AnnotatedToolpath::new(crate::toolpath::Toolpath::new()),
                )),
                stats: ToolpathStats::default(),
                debug_trace: None,
                semantic_trace: None,
            },
        );
        s.set_toolpath_param(0, "feed_rate", json!(1000.0)).unwrap();
        assert!(!s.results.contains_key(&0));
    }

    // ── set_tool_param ───────────────────────────────────────────

    fn feed_vs_lut_high_recommended_value(s: &ProjectSession) -> f64 {
        let diagnostics = s.diagnose_toolpath(0).unwrap();
        let diag = diagnostics
            .iter()
            .find(|d| d.id.0 == crate::diagnostics::ids::FEEDS_FEED_VS_LUT_HIGH)
            .expect("feeds.feed_vs_lut.high diagnostic");
        match diag.evidence.as_ref().expect("diagnostic evidence") {
            crate::diagnostics::DiagnosticEvidence::GeometryCompare {
                rhs_label,
                rhs_value,
                ..
            } => {
                assert_eq!(rhs_label, "recommended");
                *rhs_value
            }
            other => panic!("unexpected evidence: {other:?}"),
        }
    }

    #[test]
    fn suggest_output_matches_feed_vs_lut_high_diagnostic_recommendation() {
        let mut s = make_session();
        let tool = s.tools()[0].clone();
        let mut tc = make_tc(tool.id.0);
        let suggested = crate::feeds::suggest::suggest_for_operation(
            crate::feeds::suggest::SuggestForOperationInput {
                operation: &tc.operation,
                tool: &tool,
                machine: s.machine(),
                material: &s.stock_config().material,
                workholding: s.stock_config().workholding_rigidity,
                lut: crate::feeds::embedded_vendor_lut(),
                spindle_strategy: crate::feeds::SpindleStrategy::default(),
                context: crate::feeds::suggest::SuggestContext::default(),
            },
        )
        .expect("test fixture pairs a flat endmill with a Pocket op — not a refused combination");
        tc.operation
            .set_feed_rate(suggested.feeds_result.feed_rate_mm_min * 3.0);
        s.add_toolpath(0, tc).unwrap();

        let diagnostic_rec = feed_vs_lut_high_recommended_value(&s);
        assert!((diagnostic_rec - suggested.feeds_result.feed_rate_mm_min).abs() < 1e-6);
    }

    #[test]
    fn workholding_changes_suggest_output_and_diagnostic_baseline_consistently() {
        fn session_for_workholding(
            workholding: crate::feeds::WorkholdingRigidity,
        ) -> (ProjectSession, f64) {
            let mut s = make_session();
            let mut stock = s.stock_config().clone();
            stock.workholding_rigidity = workholding;
            // Use a Custom material so the suggest path takes the
            // hardness/Kc fallback model rather than a vendor-LUT match.
            // The 2026-05-31 Phase 4 promotion added Onsrud-grade 6.35 mm
            // softwood pocket rows whose chipload max saturates the
            // suggested feed at both rigidity levels — that's correct
            // behavior for the suggest pipeline but defeats this test's
            // *intent*, which is to verify rigidity flows consistently
            // through both `suggest_for_operation` and the diagnostic
            // baseline. Custom material isolates the rigidity scaler.
            stock.material = crate::material::Material::Custom {
                name: "test_workholding_fixture".to_owned(),
                feed_scale_factor: 1.5,
                kc: 25.0,
            };
            s.set_stock_config(stock);
            let tool = s.tools()[0].clone();
            let mut tc = make_tc(tool.id.0);
            let suggested = crate::feeds::suggest::suggest_for_operation(
                crate::feeds::suggest::SuggestForOperationInput {
                    operation: &tc.operation,
                    tool: &tool,
                    machine: s.machine(),
                    material: &s.stock_config().material,
                    workholding,
                    lut: crate::feeds::embedded_vendor_lut(),
                    spindle_strategy: crate::feeds::SpindleStrategy::default(),
                    context: crate::feeds::suggest::SuggestContext::default(),
                },
            )
            .expect(
                "test fixture pairs a flat endmill with a Pocket op — not a refused combination",
            );
            tc.operation
                .set_feed_rate(suggested.feeds_result.feed_rate_mm_min * 3.0);
            s.add_toolpath(0, tc).unwrap();
            (s, suggested.feeds_result.feed_rate_mm_min)
        }

        let (medium, medium_suggest) =
            session_for_workholding(crate::feeds::WorkholdingRigidity::Medium);
        let (high, high_suggest) = session_for_workholding(crate::feeds::WorkholdingRigidity::High);

        let medium_diag = feed_vs_lut_high_recommended_value(&medium);
        let high_diag = feed_vs_lut_high_recommended_value(&high);
        assert!(high_suggest > medium_suggest);
        assert!((medium_diag - medium_suggest).abs() < 1e-6);
        assert!((high_diag - high_suggest).abs() < 1e-6);
    }

    #[test]
    fn compute_stale_set_for_tool_param_returns_referencing_toolpaths() {
        let mut s = make_session();
        let tool_id = s.tools()[0].id.0;
        s.add_toolpath(0, make_tc(tool_id)).unwrap();
        s.add_toolpath(0, make_drill_tc(tool_id)).unwrap();
        let stale = compute_stale_set(&s, MutationKind::ToolParamChanged { tool_index: 0 });
        assert_eq!(stale.toolpath_indices, vec![0, 1]);
    }

    #[test]
    fn compute_stale_set_for_toolpath_param_returns_single_toolpath() {
        let mut s = make_session();
        s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
        let stale = compute_stale_set(&s, MutationKind::ToolpathParamChanged { toolpath_index: 0 });
        assert_eq!(stale.toolpath_indices, vec![0]);
    }

    #[test]
    fn compute_stale_set_for_setup_change_returns_setup_toolpaths() {
        let mut s = make_session();
        let tool_id = s.tools()[0].id.0;
        s.add_toolpath(0, make_tc(tool_id)).unwrap();
        s.add_toolpath(0, make_tc(tool_id)).unwrap();
        let setup_id = s.list_setups()[0].id;
        let stale = compute_stale_set(&s, MutationKind::SetupChanged { setup_id });
        assert_eq!(stale.toolpath_indices, vec![0, 1]);
    }

    #[test]
    fn compute_stale_set_for_stock_change_returns_all_toolpaths() {
        let mut s = make_session();
        let tool_id = s.tools()[0].id.0;
        s.add_toolpath(0, make_tc(tool_id)).unwrap();
        s.add_toolpath(0, make_drill_tc(tool_id)).unwrap();
        let stale = compute_stale_set(&s, MutationKind::StockChanged);
        assert_eq!(stale.toolpath_indices, vec![0, 1]);
    }

    #[test]
    fn set_tool_param_diameter() {
        let mut s = make_session();
        s.set_tool_param(0, "diameter", &json!(6.0)).unwrap();
        assert!((s.tools()[0].diameter - 6.0).abs() < 1e-9);
    }

    #[test]
    fn set_tool_param_flute_count() {
        let mut s = make_session();
        s.set_tool_param(0, "flute_count", &json!(4)).unwrap();
        assert_eq!(s.tools()[0].flute_count, 4);
    }

    #[test]
    fn set_tool_param_stickout() {
        let mut s = make_session();
        s.set_tool_param(0, "stickout", &json!(25.0)).unwrap();
        assert!((s.tools()[0].stickout - 25.0).abs() < 1e-9);
    }

    #[test]
    fn set_tool_param_corner_radius() {
        let mut s = make_session();
        s.set_tool_param(0, "corner_radius", &json!(0.5)).unwrap();
        assert!((s.tools()[0].corner_radius - 0.5).abs() < 1e-9);
    }

    #[test]
    fn set_tool_param_cutting_length() {
        let mut s = make_session();
        s.set_tool_param(0, "cutting_length", &json!(20.0)).unwrap();
        assert!((s.tools()[0].cutting_length - 20.0).abs() < 1e-9);
    }

    #[test]
    fn set_tool_param_invalid_index() {
        let mut s = make_session();
        let result = s.set_tool_param(99, "diameter", &json!(6.0));
        assert!(matches!(result, Err(SessionError::InvalidParam(_))));
    }

    #[test]
    fn set_tool_param_wrong_type() {
        let mut s = make_session();
        let result = s.set_tool_param(0, "diameter", &json!("not a number"));
        assert!(matches!(result, Err(SessionError::InvalidParam(_))));
    }

    #[test]
    fn set_tool_param_invalidates_toolpath_results() {
        let mut s = make_session();
        s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
        s.results.insert(
            0,
            ToolpathComputeResult {
                op_data: crate::drill_op::OpData::Toolpath(Arc::new(
                    crate::toolpath_spans::AnnotatedToolpath::new(crate::toolpath::Toolpath::new()),
                )),
                stats: ToolpathStats::default(),
                debug_trace: None,
                semantic_trace: None,
            },
        );

        s.set_tool_param(0, "diameter", &json!(8.0)).unwrap();
        assert!(!s.results.contains_key(&0));
    }

    // ── generate_toolpath error paths ────────────────────────────

    #[test]
    fn generate_toolpath_not_found() {
        let mut s = make_session();
        let cancel = AtomicBool::new(false);
        let result = s.generate_toolpath(99, &cancel);
        assert!(matches!(result, Err(SessionError::ToolpathNotFound(99))));
    }

    /// Rest machining FAILS HARD instead of silently clearing fresh stock.
    /// A `FromRemainingStock` op with no simulated remaining-stock snapshot must
    /// error at generate time — regression net for the fresh-fallback runaway
    /// where a fine rest tool, seeded with fresh stock, cleared the whole part
    /// (unbounded compute). The precondition is checked at `generate_toolpath`
    /// entry, before any geometry work.
    #[test]
    fn generate_from_remaining_stock_without_sim_errors_hard() {
        let mut s = make_session();
        let mut tc = make_tc(s.tools()[0].id.0);
        tc.stock_source = crate::session::StockSource::FromRemainingStock;
        s.add_toolpath(0, tc).unwrap();
        let cancel = AtomicBool::new(false);
        match s.generate_toolpath(0, &cancel) {
            Err(SessionError::OperationFailed(msg)) => assert!(
                msg.contains("remaining stock"),
                "error should name the missing rest-stock snapshot: {msg}"
            ),
            Err(other) => panic!("expected OperationFailed, got: {other}"),
            Ok(_) => panic!("rest op without a prior sim must error, not clear fresh stock"),
        }
    }

    /// Fixture for the F.4 phantom-prior-stock tests below: one tool plus a
    /// small square-polygon model at `model_id == 0` (matching `make_tc`'s
    /// default), so a `Pocket` op generates a real, multi-move toolpath.
    /// `run_simulation`'s request builder skips any toolpath with fewer
    /// than 2 moves, so an empty/geometry-less fixture would never
    /// populate `prior_stocks` at all.
    fn make_session_with_pocket_model() -> ProjectSession {
        let mut s = make_session();
        let polygon = crate::polygon::Polygon2 {
            exterior: vec![
                crate::geo::P2::new(0.0, 0.0),
                crate::geo::P2::new(30.0, 0.0),
                crate::geo::P2::new(30.0, 30.0),
                crate::geo::P2::new(0.0, 30.0),
            ],
            holes: vec![],
            closed: true,
        };
        let model = crate::session::LoadedModel {
            id: 0,
            name: "phantom_prior_stock_fixture".to_owned(),
            mesh: None,
            polygons: Some(Arc::new(vec![polygon])),
            drill_targets: Arc::new(Vec::new()),
            layers: Arc::new(Vec::new()),
            path: std::path::PathBuf::from("synthetic://phantom_prior_stock_fixture.svg"),
            kind: None,
            units: None,
            enriched_mesh: None,
            winding_report: None,
            load_error: None,
        };
        s.add_model(model);
        s
    }

    /// F.4 — the core half of the regression net for the
    /// `FromRemainingStock` regeneration catch-22. TP1 is preceded by a
    /// generated TP0 in the same setup: before any simulation, TP1 must
    /// still fail hard (unchanged precondition); after `run_simulation`
    /// populates the phantom `prior_stocks` snapshot for TP1 (the first
    /// pending toolpath in its group), TP1 must regenerate successfully —
    /// closing the catch-22 where an ungenerated toolpath, never present
    /// in a `SimGroupEntry`, could never receive a snapshot at all.
    #[test]
    fn phantom_prior_stock_unlocks_regeneration_after_sim() {
        let mut s = make_session_with_pocket_model();
        let tool_id = s.tools()[0].id.0;
        s.add_toolpath(0, make_tc(tool_id)).unwrap();
        let mut rest_tc = make_tc(tool_id);
        rest_tc.stock_source = crate::session::StockSource::FromRemainingStock;
        s.add_toolpath(0, rest_tc).unwrap();

        let cancel = AtomicBool::new(false);
        s.generate_toolpath(0, &cancel)
            .expect("TP0 (Fresh) should generate against real polygon geometry");

        // Before any simulation: TP1 still fails hard (unchanged
        // precondition — generating never falls back to fresh stock).
        match s.generate_toolpath(1, &cancel) {
            Err(SessionError::OperationFailed(_)) => {}
            Err(other) => panic!("expected OperationFailed before any sim, got error: {other:?}"),
            Ok(_) => panic!("expected OperationFailed before any sim, got a generated toolpath"),
        }

        s.run_simulation(&SimulationOptions::default(), &cancel)
            .expect("simulation over TP0 should succeed and populate prior_stocks");

        // F.4: TP1 is the first (and only) pending toolpath in its group,
        // so `run_simulation` recorded a phantom snapshot for it — it must
        // now regenerate.
        s.generate_toolpath(1, &cancel)
            .expect("TP1 should regenerate once the phantom prior-stock snapshot exists");
    }

    /// F.4 ladder rule: with TWO consecutive pending `FromRemainingStock`
    /// toolpaths after a generated TP0, one simulation run unlocks only
    /// the FIRST pending toolpath (TP1). TP2 stays gated — its snapshot
    /// would be missing TP1's cuts (TP1 hasn't itself been generated and
    /// re-simulated yet), which for a rest-machining op means real
    /// overcut risk, not just a stale preview.
    #[test]
    fn phantom_prior_stock_ladder_unlocks_only_first_pending_op() {
        let mut s = make_session_with_pocket_model();
        let tool_id = s.tools()[0].id.0;
        s.add_toolpath(0, make_tc(tool_id)).unwrap();
        let mut rest_tc_1 = make_tc(tool_id);
        rest_tc_1.stock_source = crate::session::StockSource::FromRemainingStock;
        s.add_toolpath(0, rest_tc_1).unwrap();
        let mut rest_tc_2 = make_tc(tool_id);
        rest_tc_2.stock_source = crate::session::StockSource::FromRemainingStock;
        s.add_toolpath(0, rest_tc_2).unwrap();

        let cancel = AtomicBool::new(false);
        s.generate_toolpath(0, &cancel)
            .expect("TP0 (Fresh) should generate against real polygon geometry");

        s.run_simulation(&SimulationOptions::default(), &cancel)
            .expect("simulation over TP0 should succeed");

        // TP1 is the first pending op in the group — unlocked.
        s.generate_toolpath(1, &cancel)
            .expect("TP1 should regenerate: first pending op in its group");

        // TP2 is still pending behind TP1, which hasn't itself been
        // generated + re-simulated — the ladder rule keeps it gated.
        match s.generate_toolpath(2, &cancel) {
            Err(SessionError::OperationFailed(_)) => {}
            Err(other) => panic!(
                "TP2 must stay gated until TP1 is regenerated and re-simulated, got error: \
                 {other:?}"
            ),
            Ok(_) => panic!(
                "TP2 must stay gated until TP1 is regenerated and re-simulated, but it generated"
            ),
        }
    }

    // ── diagnostics ──────────────────────────────────────────────

    #[test]
    fn diagnostics_empty() {
        let s = ProjectSession::new_empty();
        let diag = s.diagnostics();
        assert_eq!(diag.verdict, "OK");
        assert!(diag.per_toolpath.is_empty());
        assert!(diag.verdicts.is_empty());
    }

    // ── Verdict layer (PR-1: A4 + B8 + B1-verdict + A12 + C7) ──────

    fn make_session_with_two_tps() -> ProjectSession {
        let mut s = make_session();
        // TP0: Pocket — exercising A4 (TP-named verdict) and C7 (empty cut).
        s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
        // TP1: Drill — exercises A12 (op_kind tag).
        let mut drill_tc = make_tc(s.tools()[0].id.0);
        drill_tc.operation =
            OperationConfig::Drill(crate::compute::operation_configs::DrillConfig::default());
        drill_tc.name = "Pin holes".to_owned();
        s.add_toolpath(0, drill_tc).unwrap();
        s
    }

    fn empty_result() -> ToolpathComputeResult {
        ToolpathComputeResult {
            op_data: crate::drill_op::OpData::Toolpath(Arc::new(
                crate::toolpath_spans::AnnotatedToolpath::new(crate::toolpath::Toolpath::new()),
            )),
            stats: ToolpathStats::default(),
            debug_trace: None,
            semantic_trace: None,
        }
    }

    /// Setup-tab lag fix (2026-06-11): holder/shank collisions are
    /// EVIDENCE consumed by `diagnostics_with_evidence`, not something
    /// it computes. Empty evidence → no holder verdict even though
    /// results exist; supplied counts → verdict with exactly those
    /// counts. (The old behavior ran a full `collision_check` sweep per
    /// toolpath inside diagnostics, which the GUI setup panel then
    /// executed every frame.)
    #[test]
    fn diagnostics_with_evidence_consumes_holder_counts_instead_of_computing() {
        let mut s = make_session_with_two_tps();
        let mut r = empty_result();
        r.stats.cutting_distance = 100.0;
        s.results.insert(0, r);

        // No holder evidence → no holder verdict, zero count.
        let diag = s.diagnostics_with_evidence(&ProjectEvidence::default());
        assert_eq!(diag.collision_count, 0);
        assert!(
            !diag
                .verdicts
                .iter()
                .any(|v| matches!(v.kind, crate::session::VerdictKind::HolderCollision)),
            "no holder verdict without holder evidence"
        );

        // Supplied counts surface verbatim.
        let tp0_id = s.toolpath_configs()[0].id;
        let evidence = ProjectEvidence {
            holder_collisions: vec![(tp0_id, 3)],
            ..ProjectEvidence::default()
        };
        let diag = s.diagnostics_with_evidence(&evidence);
        assert_eq!(diag.collision_count, 3);
        let verdict = diag
            .verdicts
            .iter()
            .find(|v| matches!(v.kind, crate::session::VerdictKind::HolderCollision))
            .expect("holder verdict from supplied evidence");
        assert_eq!(verdict.evidence.count, Some(3));
        assert_eq!(verdict.offender_toolpath_ids, vec![tp0_id]);
    }

    /// A12: per-toolpath diagnostics carry an `op_kind` snake_case tag so
    /// downstream consumers can suppress rapid:cut-ratio signals on
    /// drill / pin-drill ops.
    #[test]
    fn diagnostics_tags_per_tp_with_op_kind() {
        let mut s = make_session_with_two_tps();
        // Both TPs need a result to appear in per_toolpath; cutting_distance
        // is non-zero so the C7 GeneratedEmpty verdict doesn't fire for TP0.
        let mut r = empty_result();
        r.stats.cutting_distance = 100.0;
        s.results.insert(0, r);
        let mut r1 = empty_result();
        r1.stats.cutting_distance = 50.0;
        s.results.insert(1, r1);

        let diag = s.diagnostics();
        assert_eq!(diag.per_toolpath.len(), 2);
        let pocket = diag
            .per_toolpath
            .iter()
            .find(|d| d.name == "test")
            .expect("pocket TP present");
        assert_eq!(pocket.op_kind, "pocket");
        let drill = diag
            .per_toolpath
            .iter()
            .find(|d| d.name == "Pin holes")
            .expect("drill TP present");
        assert_eq!(drill.op_kind, "drill");
    }

    /// C7: a non-drill toolpath that generated successfully but laid down
    /// zero in-material cut emits a GeneratedEmpty verdict naming the TP.
    /// Drill toolpaths with zero cutting distance are intentionally
    /// exempt — the dexel cutting metric doesn't apply to Z-only ops.
    #[test]
    fn diagnostics_emits_generated_empty_for_zero_cut_non_drill() {
        let mut s = make_session_with_two_tps();
        s.results.insert(0, empty_result()); // pocket, cutting_distance == 0
        // Drill also has zero cutting_distance but should NOT trigger C7.
        s.results.insert(1, empty_result());

        let diag = s.diagnostics();
        let empty_verdicts: Vec<_> = diag
            .verdicts
            .iter()
            .filter(|v| v.kind == VerdictKind::GeneratedEmpty)
            .collect();
        assert_eq!(
            empty_verdicts.len(),
            1,
            "expected one GeneratedEmpty verdict (pocket); drill must be exempt: {:?}",
            diag.verdicts
        );
        let v = empty_verdicts[0];
        assert_eq!(v.severity, VerdictSeverity::Important);
        assert!(
            v.headline.contains("'test'"),
            "headline should name the offending TP: {}",
            v.headline
        );
        assert_eq!(v.offender_toolpath_ids, vec![s.toolpath_configs[0].id]);
        assert!(!v.fix_hint.is_empty(), "fix_hint must be populated");
    }

    /// B8: verdicts are severity-ranked (Critical → Important → Polish).
    /// When several conditions are present the list returns all of them
    /// in priority order instead of picking only one (the old early-exit).
    #[test]
    fn diagnostics_ranks_verdicts_by_severity() {
        use crate::compute::simulate::{SimBoundary, SimulationResult};
        use crate::dexel_stock::StockCutDirection;
        use crate::stock_mesh::StockMesh;

        let mut s = make_session_with_two_tps();
        // TP0 (pocket): zero cut → C7 GeneratedEmpty (Important).
        s.results.insert(0, empty_result());
        // TP1 (drill): nonzero cut so it stays out of GeneratedEmpty.
        let mut r1 = empty_result();
        r1.stats.cutting_distance = 50.0;
        s.results.insert(1, r1);

        // Inject a simulation result that carries a rapid-through-stock
        // collision on TP0 → triggers a Critical RapidCollision verdict.
        let pocket_tp_id = s.toolpath_configs[0].id;
        s.simulation = Some(SimulationResult {
            mesh: StockMesh {
                vertices: Vec::new(),
                indices: Vec::new(),
                colors: Vec::new(),
            },
            total_moves: 5,
            deviations: None,
            column_deviations: None,
            boundaries: vec![SimBoundary {
                id: pocket_tp_id,
                name: "test".to_owned(),
                tool_name: "EM".to_owned(),
                start_move: 0,
                end_move: 5,
                direction: StockCutDirection::FromTop,
            }],
            checkpoints: Vec::new(),
            rapid_collisions: vec![crate::collision::RapidCollision {
                move_index: 2,
                start: P3::new(0.0, 0.0, 5.0),
                end: P3::new(0.0, 0.0, -2.5),
            }],
            rapid_collision_move_indices: vec![2],
            cut_trace: None,
            resolution_clamped: false,
            column_grid_cell_mm: 0.5,
            prior_stocks: std::collections::HashMap::new(),
        });

        let diag = s.diagnostics();
        assert!(
            diag.verdicts.len() >= 2,
            "expected both Critical + Important verdicts; got {:?}",
            diag.verdicts
        );
        // Critical comes before Important.
        assert_eq!(diag.verdicts[0].severity, VerdictSeverity::Critical);
        assert_eq!(diag.verdicts[0].kind, VerdictKind::RapidCollision);
        let importants: Vec<_> = diag
            .verdicts
            .iter()
            .filter(|v| v.severity == VerdictSeverity::Important)
            .collect();
        assert!(
            importants
                .iter()
                .any(|v| v.kind == VerdictKind::GeneratedEmpty),
            "GeneratedEmpty must still surface alongside RapidCollision"
        );
        // Legacy single-line verdict mirrors the top-severity headline.
        assert_eq!(diag.verdict, diag.verdicts[0].headline);
    }

    /// B1 (verdict half) + A4: rapid-collision verdict names the offending
    /// TP, quotes the collision count, cites the worst move's z, and offers
    /// a fix hint pointing at retract_z / safe-Z / boundary config.
    #[test]
    fn diagnostics_rapid_collision_verdict_carries_evidence() {
        use crate::compute::simulate::{SimBoundary, SimulationResult};
        use crate::dexel_stock::StockCutDirection;
        use crate::stock_mesh::StockMesh;

        let mut s = make_session();
        s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
        let mut r = empty_result();
        r.stats.cutting_distance = 100.0;
        s.results.insert(0, r);

        let tp_id = s.toolpath_configs[0].id;
        s.simulation = Some(SimulationResult {
            mesh: StockMesh {
                vertices: Vec::new(),
                indices: Vec::new(),
                colors: Vec::new(),
            },
            total_moves: 10,
            deviations: None,
            column_deviations: None,
            boundaries: vec![SimBoundary {
                id: tp_id,
                name: "test".to_owned(),
                tool_name: "EM".to_owned(),
                start_move: 0,
                end_move: 10,
                direction: StockCutDirection::FromTop,
            }],
            checkpoints: Vec::new(),
            rapid_collisions: vec![
                crate::collision::RapidCollision {
                    move_index: 1,
                    start: P3::new(0.0, 0.0, 5.0),
                    end: P3::new(0.0, 0.0, 1.0),
                },
                // Deepest rapid — this should be cited as the worst move.
                crate::collision::RapidCollision {
                    move_index: 7,
                    start: P3::new(1.0, 1.0, 5.0),
                    end: P3::new(1.0, 1.0, -3.25),
                },
            ],
            rapid_collision_move_indices: vec![1, 7],
            cut_trace: None,
            resolution_clamped: false,
            column_grid_cell_mm: 0.5,
            prior_stocks: std::collections::HashMap::new(),
        });

        let diag = s.diagnostics();
        let v = diag
            .verdicts
            .iter()
            .find(|v| v.kind == VerdictKind::RapidCollision)
            .expect("rapid collision verdict must fire");
        assert_eq!(v.severity, VerdictSeverity::Critical);
        // A4: names the TP.
        assert!(
            v.headline.contains("'test'"),
            "headline names offending TP: {}",
            v.headline
        );
        // Count is quoted.
        assert!(
            v.headline.contains("2 collisions"),
            "headline: {}",
            v.headline
        );
        // Worst-move evidence cites move_index=7 and z=-3.250.
        assert_eq!(v.evidence.move_index, Some(7));
        assert_eq!(v.evidence.count, Some(2));
        assert!(
            v.evidence.z_value.is_some()
                && (v.evidence.z_value.unwrap_or(0.0) - (-3.25)).abs() < 1e-6,
            "evidence.z_value: {:?}",
            v.evidence.z_value
        );
        // Fix hint mentions retract_z (the operator's lever).
        let hint_lc = v.fix_hint.to_lowercase();
        assert!(
            hint_lc.contains("retract_z")
                || hint_lc.contains("safe-z")
                || hint_lc.contains("boundary"),
            "fix_hint must point at retract_z / safe-Z / boundary: {}",
            v.fix_hint
        );
        assert_eq!(v.offender_toolpath_ids, vec![tp_id]);
    }

    // ── P1: op-kind-aware air-cut thresholds ──────────────────────

    use crate::compute::operation_configs::{
        Adaptive3dConfig, AlignmentPinDrillConfig, DropCutterConfig, ProjectCurveConfig,
    };
    use crate::simulation_cut::SimulationToolpathCutSummary;

    fn make_tp(id: usize, name: &str, op: OperationConfig) -> ToolpathConfig {
        ToolpathConfig {
            id: ToolpathId(id),
            name: name.to_owned(),
            enabled: true,
            operation: op,
            dressups: DressupConfig::default(),
            heights: HeightsConfig::default(),
            tool_id: 0,
            model_id: 0,
            pre_gcode: None,
            post_gcode: None,
            boundary: BoundaryConfig::default(),
            boundary_inherit: true,
            stock_source: crate::session::StockSource::Fresh,
            coolant: CoolantMode::Off,
            face_selection: None,
            debug_options: ToolpathDebugOptions::default(),
            feeds_provenance: crate::feeds::FeedsProvenance::default(),
            rest_analysis: crate::compute::config::RestAnalysisConfig::default(),
        }
    }

    fn summary(id: usize, air_pct: f64) -> SimulationToolpathCutSummary {
        let total = 100.0;
        SimulationToolpathCutSummary {
            toolpath_id: ToolpathId(id),
            sample_count: 0,
            total_runtime_s: total,
            cutting_runtime_s: total * (1.0 - air_pct / 100.0),
            rapid_runtime_s: 0.0,
            air_cut_time_s: total * air_pct / 100.0,
            low_engagement_time_s: 0.0,
            average_engagement: 0.0,
            peak_chipload_mm_per_tooth: 0.0,
            peak_axial_doc_mm: 0.0,
            peak_plunge_descent_mm: 0.0,
            total_removed_volume_est_mm3: 0.0,
            average_mrr_mm3_s: 0.0,
            metrics_not_applicable: false,
            per_kinematics: std::collections::BTreeMap::new(),
            runtime_by_intent: None,
        }
    }

    #[test]
    fn air_cut_offenders_silent_on_sparse_project_curve() {
        // Wanaka TP3: 92% air-cut on ProjectCurve is intrinsic, not a defect.
        let tps = vec![make_tp(
            0,
            "Rivers (back)",
            OperationConfig::ProjectCurve(ProjectCurveConfig::default()),
        )];
        let sums = vec![summary(0, 92.1)];
        let offenders = air_cut_offenders_for_toolpaths(&sums, &tps);
        assert!(
            offenders.is_empty(),
            "ProjectCurve at baseline air-cut should not warn; got {offenders:?}"
        );
    }

    #[test]
    fn air_cut_offenders_silent_on_adaptive3d_below_threshold() {
        // Wanaka TP1: 28.5% air-cut on Adaptive3d should be silent.
        let tps = vec![make_tp(
            0,
            "Back Rough",
            OperationConfig::Adaptive3d(Adaptive3dConfig::default()),
        )];
        let sums = vec![summary(0, 28.5)];
        let offenders = air_cut_offenders_for_toolpaths(&sums, &tps);
        assert!(
            offenders.is_empty(),
            "Adaptive3d below 40% threshold should be silent; got {offenders:?}"
        );
    }

    #[test]
    fn air_cut_offenders_silent_on_drop_cutter_finish() {
        // Wanaka TP7: 11.5% air-cut on DropCutter is healthy.
        let tps = vec![make_tp(
            0,
            "3D Finish 6",
            OperationConfig::DropCutter(DropCutterConfig::default()),
        )];
        let sums = vec![summary(0, 11.5)];
        let offenders = air_cut_offenders_for_toolpaths(&sums, &tps);
        assert!(offenders.is_empty(), "DropCutter at 11.5% should be silent");
    }

    #[test]
    fn air_cut_offenders_warns_on_adaptive3d_above_threshold() {
        // 60% air-cut on Adaptive3d is well above the 40% high-water mark.
        let tps = vec![make_tp(
            0,
            "Bad Rough",
            OperationConfig::Adaptive3d(Adaptive3dConfig::default()),
        )];
        let sums = vec![summary(0, 60.0)];
        let offenders = air_cut_offenders_for_toolpaths(&sums, &tps);
        assert_eq!(offenders.len(), 1, "Adaptive3d at 60% should warn");
        assert_eq!(offenders[0].0, "Bad Rough");
        assert!((offenders[0].1 - 60.0).abs() < 1e-6);
    }

    #[test]
    fn air_cut_offenders_warns_on_drop_cutter_above_threshold() {
        // 40% air-cut on DropCutter exceeds the 30% finish threshold.
        let tps = vec![make_tp(
            0,
            "Sloppy Finish",
            OperationConfig::DropCutter(DropCutterConfig::default()),
        )];
        let sums = vec![summary(0, 40.0)];
        let offenders = air_cut_offenders_for_toolpaths(&sums, &tps);
        assert_eq!(offenders.len(), 1, "DropCutter at 40% should warn");
    }

    #[test]
    fn air_cut_offenders_warns_on_project_curve_near_total_air() {
        // 99% air-cut on ProjectCurve indicates a misconfigured TP — flag it.
        let tps = vec![make_tp(
            0,
            "Empty Rivers",
            OperationConfig::ProjectCurve(ProjectCurveConfig::default()),
        )];
        let sums = vec![summary(0, 99.0)];
        let offenders = air_cut_offenders_for_toolpaths(&sums, &tps);
        assert_eq!(offenders.len(), 1, "ProjectCurve at 99% should warn");
    }

    #[test]
    fn air_cut_offenders_suppresses_drill_ops_entirely() {
        // Drill kinematics: air-cut metric is unusable. Never warn.
        let tps = vec![make_tp(
            0,
            "Pin Drill",
            OperationConfig::AlignmentPinDrill(AlignmentPinDrillConfig::default()),
        )];
        let sums = vec![summary(0, 100.0)]; // dexel reports 100% always
        let offenders = air_cut_offenders_for_toolpaths(&sums, &tps);
        assert!(
            offenders.is_empty(),
            "AlignmentPinDrill must never trigger air-cut warning (P4 suppression)"
        );
    }

    // ── P2: plunge-stress gate at session level ──────────────────

    fn make_tapered_ball_tool(diameter: f64) -> ToolConfig {
        let mut t = ToolConfig::new_default(ToolId(0), ToolType::TaperedBallNose);
        t.diameter = diameter; // tip diameter
        t.shaft_diameter = (diameter + 2.0).max(3.0);
        t.taper_half_angle = 7.0;
        t
    }

    fn make_tp_with_plunge(
        id: usize,
        name: &str,
        op: OperationConfig,
        plunge_rate: f64,
    ) -> ToolpathConfig {
        let mut tc = make_tp(id, name, op);
        tc.operation.as_params_mut().set_plunge_rate(plunge_rate);
        tc
    }

    #[test]
    fn plunge_stress_warns_on_wanaka_tp7_pattern() {
        // 1 mm tapered ball at 750 mm/min plunge — the TP7 finding.
        let mut s = ProjectSession::new_empty();
        s.add_tool(make_tapered_ball_tool(1.0));
        let tp = make_tp_with_plunge(
            0,
            "3D Finish 6",
            OperationConfig::DropCutter(DropCutterConfig::default()),
            750.0,
        );
        s.add_toolpath(0, tp).unwrap();
        let offenders = plunge_stress_offenders_for_session(&s);
        assert_eq!(offenders.len(), 1);
        assert_eq!(offenders[0].0, "3D Finish 6");
        assert!((offenders[0].1 - 750.0).abs() < 1e-6);
        assert!((offenders[0].2 - 150.0).abs() < 1e-6);
    }

    #[test]
    fn plunge_stress_silent_for_flat_em_at_750() {
        // 6 mm flat end-mill at 750 mm/min — no cap applies.
        let mut s = ProjectSession::new_empty();
        let tp = make_tp_with_plunge(
            0,
            "Back Rough",
            OperationConfig::Adaptive3d(Adaptive3dConfig::default()),
            750.0,
        );
        s.add_toolpath(0, tp).unwrap();
        let offenders = plunge_stress_offenders_for_session(&s);
        assert!(
            offenders.is_empty(),
            "flat EM should be silent on plunge stress; got {offenders:?}"
        );
    }

    #[test]
    fn plunge_stress_silent_when_at_or_below_cap() {
        // 1 mm tapered ball at 150 mm/min — exactly at cap.
        let mut s = ProjectSession::new_empty();
        s.add_tool(make_tapered_ball_tool(1.0));
        let tp = make_tp_with_plunge(
            0,
            "Engrave",
            OperationConfig::ProjectCurve(ProjectCurveConfig::default()),
            150.0,
        );
        s.add_toolpath(0, tp).unwrap();
        let offenders = plunge_stress_offenders_for_session(&s);
        assert!(offenders.is_empty(), "150 mm/min on 1 mm TB is at cap");
    }

    #[test]
    fn plunge_stress_ignores_disabled_toolpaths() {
        let mut s = ProjectSession::new_empty();
        s.add_tool(make_tapered_ball_tool(1.0));
        let mut tp = make_tp_with_plunge(
            0,
            "Disabled",
            OperationConfig::DropCutter(DropCutterConfig::default()),
            750.0,
        );
        tp.enabled = false;
        s.add_toolpath(0, tp).unwrap();
        let offenders = plunge_stress_offenders_for_session(&s);
        assert!(offenders.is_empty(), "disabled TPs should be ignored");
    }

    #[test]
    fn air_cut_offenders_isolates_bad_tp_in_mixed_project() {
        // Wanaka-like mix: ProjectCurve at 92% (noise) + Adaptive3d at 60% (signal).
        // Only the Adaptive3d should be flagged.
        let tps = vec![
            make_tp(
                0,
                "Rivers",
                OperationConfig::ProjectCurve(ProjectCurveConfig::default()),
            ),
            make_tp(
                1,
                "Bad Rough",
                OperationConfig::Adaptive3d(Adaptive3dConfig::default()),
            ),
        ];
        let sums = vec![summary(0, 92.0), summary(1, 60.0)];
        let offenders = air_cut_offenders_for_toolpaths(&sums, &tps);
        assert_eq!(offenders.len(), 1);
        assert_eq!(offenders[0].0, "Bad Rough");
    }

    // ── strategy advisor: optimized-candidate modulation (step 5) ────

    /// Load `ux_3d_terrain.toml` and add an AS013-shape adaptive3d op with the
    /// given clearing strategy — mirrors the `strategy_advisor_smoke` fixture
    /// so the advisor's per-candidate optimization can be exercised in-crate
    /// (the private `optimized_candidate` is not reachable from the integration
    /// test).
    fn terrain_adaptive3d_session(strategy: ClearingStrategy) -> ProjectSession {
        use crate::compute::operation_configs::{
            Adaptive3dConfig, Adaptive3dEntryStyle, RegionOrdering,
        };
        let toml_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../test_data/ux_3d_terrain.toml");
        let mut session = ProjectSession::load(&toml_path).expect("load ux_3d_terrain");
        let tool_id = session
            .tools()
            .iter()
            .find(|t| (t.diameter - 6.0).abs() < 1e-6)
            .map(|t| t.id.0)
            .expect("ux_3d_terrain.toml defines a 6 mm end mill");
        let model_id = session
            .models()
            .iter()
            .find(|m| m.mesh.is_some())
            .map(|m| m.id)
            .expect("ux_3d_terrain.toml loads terrain_small.stl");
        let adaptive3d = Adaptive3dConfig {
            trochoid_cap_mult: 1.6,
            engagement_measure: crate::adaptive::EngagementMeasure::DiskArea,
            stepover: 1.2,
            depth_per_pass: 3.0,
            stock_to_leave_axial: 0.5,
            stock_to_leave_radial: 0.5,
            feed_rate: 2500.0,
            plunge_rate: 500.0,
            tolerance: 0.25,
            min_cutting_radius: 0.0,
            entry_style: Adaptive3dEntryStyle::Plunge,
            ramp_angle_deg: 3.0,
            helix_radius_factor: 0.4,
            helix_pitch: 1.0,
            fine_stepdown: 0.0,
            detect_flat_areas: false,
            region_ordering: RegionOrdering::Global,
            clearing_strategy: strategy,
            z_blend: false,
            mill_shallow_areas: false,
            shallow_angle_deg: None,
            shallow_stepdown: None,
            spindle_rpm: Some(18_000),
            min_region_cut_length_mm: 0.0,
            max_stay_down_distance_mm: Some(0.0),
            stay_down_clearance_mm: 0.5,
        };
        let tc = ToolpathConfig {
            id: ToolpathId(0),
            name: "AS013 adaptive3d".to_owned(),
            enabled: true,
            operation: OperationConfig::Adaptive3d(adaptive3d),
            dressups: DressupConfig::for_op(crate::compute::catalog::OperationType::Adaptive3d),
            heights: HeightsConfig::default(),
            tool_id,
            model_id,
            pre_gcode: None,
            post_gcode: None,
            boundary: BoundaryConfig::default(),
            boundary_inherit: true,
            stock_source: crate::compute::config::StockSource::default(),
            coolant: CoolantMode::Off,
            face_selection: None,
            debug_options: ToolpathDebugOptions::default(),
            feeds_provenance: crate::feeds::FeedsProvenance::default(),
            rest_analysis: crate::compute::config::RestAnalysisConfig::default(),
        };
        session
            .add_toolpath(0, tc)
            .expect("add adaptive3d toolpath");
        session
    }

    fn cut_move_feed(m: &crate::toolpath::Move) -> Option<f64> {
        match m.move_type {
            crate::toolpath::MoveType::Linear { feed_rate }
            | crate::toolpath::MoveType::ArcCW { feed_rate, .. }
            | crate::toolpath::MoveType::ArcCCW { feed_rate, .. } => Some(feed_rate),
            crate::toolpath::MoveType::Rapid => None,
        }
    }

    /// Step-5 sentry: the advisor times the *modulated* path, not the raw
    /// Suggest-feed path. Proves `optimized_candidate` rewrites at least one
    /// cut-move feed (so the wall-clock the advisor compares reflects F-039
    /// optimization) and returns a modelled binding regime. If step 5 were
    /// reverted to timing raw paths this test fails: feeds would be untouched.
    #[test]
    fn advisor_modulates_candidate_feeds_before_timing() {
        let session = terrain_adaptive3d_session(ClearingStrategy::ContourSpiral);
        let cancel = AtomicBool::new(false);
        let resolved = session
            .resolve_generation_inputs(0)
            .expect("resolve generation inputs for the adaptive3d op");

        // Build the raw candidate path exactly as `recommend_clearing_strategy`
        // does (minus the Suggest load-limit — modulation rewrites whatever
        // feeds the planned path carries, so the commanded 2500 mm/min is a
        // fair starting point for the "did feeds change?" check).
        let annotated = crate::compute::execute::execute_operation_annotated(
            &resolved.operation,
            resolved.mesh.as_deref(),
            resolved.spatial_index.as_ref(),
            resolved.polygons.as_deref().map(|v| v.as_slice()),
            &resolved.tool_def,
            &resolved.tool,
            &resolved.heights,
            &resolved.cutting_levels,
            &resolved.emission_stock_bbox,
            resolved.prev_tool_radius,
            None,
            None,
            &cancel,
            None,
            None,
            resolved.pre_boundary.as_ref(),
        )
        .expect("plan the spiral candidate");
        let annotated_arc = Arc::new(annotated);
        let raw_feeds: Vec<Option<f64>> = annotated_arc
            .toolpath
            .moves
            .iter()
            .map(cut_move_feed)
            .collect();

        let (modulated, regime) = session
            .optimized_candidate(
                0,
                &annotated_arc,
                &resolved.tool,
                &resolved.operation,
                &cancel,
            )
            .expect("advisor optimizes the candidate (effective_kinematics is always Some)");

        // Geometry is untouched; only feeds change.
        assert_eq!(
            modulated.moves.len(),
            annotated_arc.toolpath.moves.len(),
            "modulation rewrites feeds, not geometry"
        );
        let changed = modulated
            .moves
            .iter()
            .zip(&raw_feeds)
            .filter(|(m, raw)| match (cut_move_feed(m), raw) {
                (Some(a), Some(b)) => (a - b).abs() > 0.5,
                _ => false,
            })
            .count();
        assert!(
            changed > 0,
            "ConstrainedMax modulation must rewrite at least one cut-move feed \
             before the advisor times the path (else it's timing the raw path)"
        );
        assert!(
            matches!(
                regime,
                crate::strategy_advisor::LoadRegime::ToolLimited
                    | crate::strategy_advisor::LoadRegime::MachineLimited
                    | crate::strategy_advisor::LoadRegime::Unconstrained
            ),
            "regime must be a modelled binding value derived from the optimized path"
        );
    }

    // ── DerivedRestRegions boundary (P2.2) ───────────────────────

    fn derived_boundary(source_id: usize) -> BoundaryConfig {
        BoundaryConfig {
            enabled: true,
            source: crate::compute::config::BoundarySource::DerivedRestRegions {
                source_toolpath_id: ToolpathId(source_id),
            },
            ..BoundaryConfig::default()
        }
    }

    /// A minimal cached generation result whose annotated toolpath carries
    /// (or lacks) `rest_regions`, for staleness-precondition tests.
    fn fake_result_with_regions(
        regions: Option<Vec<crate::polygon::Polygon2>>,
    ) -> ToolpathComputeResult {
        let mut at =
            crate::toolpath_spans::AnnotatedToolpath::new(crate::toolpath::Toolpath::new());
        at.rest_regions = regions.map(Arc::new);
        ToolpathComputeResult {
            op_data: crate::drill_op::OpData::Toolpath(Arc::new(at)),
            stats: ToolpathStats {
                move_count: 0,
                cutting_distance: 0.0,
                rapid_distance: 0.0,
                // Not measured: this fake never ran a cascade, planned no
                // bands and emitted no centrelines.
                standing_material_mm2: None,
                dropped_band: None,
                tip_float: None,
                deprecated_dial: None,
                derived_stepovers: Vec::new(),
                clipped_band: None,
                ramp_reach_clamp: None,
                claims_reference: None,
                retract_trips: None,
            },
            debug_trace: None,
            semantic_trace: None,
        }
    }

    fn expect_operation_failed(result: Result<&ToolpathComputeResult, SessionError>) -> String {
        match result {
            Err(SessionError::OperationFailed(msg)) => msg,
            Err(other) => panic!("expected OperationFailed, got {other:?}"),
            Ok(_) => panic!("expected OperationFailed, got Ok"),
        }
    }

    #[test]
    fn derived_rest_regions_boundary_missing_source_errors() {
        let mut s = make_session();
        let mut tc = make_tc(s.tools()[0].id.0);
        tc.boundary = derived_boundary(999);
        s.add_toolpath(0, tc).unwrap();

        let cancel = AtomicBool::new(false);
        let msg = expect_operation_failed(s.generate_toolpath(0, &cancel));
        assert!(
            msg.contains("999"),
            "error should name the missing source id: {msg}"
        );
        assert!(
            msg.contains("no longer") || msg.contains("no toolpath with that id"),
            "error should say the referenced toolpath doesn't exist: {msg}"
        );
    }

    #[test]
    fn derived_rest_regions_boundary_self_reference_errors() {
        let mut s = make_session();
        let mut tc = make_tc(s.tools()[0].id.0);
        // First add_toolpath assigns id 0, so referencing id 0 is a
        // self-reference.
        tc.boundary = derived_boundary(0);
        s.add_toolpath(0, tc).unwrap();

        let cancel = AtomicBool::new(false);
        let msg = expect_operation_failed(s.generate_toolpath(0, &cancel));
        assert!(
            msg.contains("itself") || msg.contains("own rest regions"),
            "error should reject the self-reference: {msg}"
        );
    }

    #[test]
    fn derived_rest_regions_boundary_ungenerated_source_errors() {
        let mut s = make_session();
        let mut source_tc = make_tc(s.tools()[0].id.0);
        source_tc.name = "Pencil Rest".to_owned();
        s.add_toolpath(0, source_tc).unwrap(); // gets id 0

        let mut tc = make_tc(s.tools()[0].id.0);
        tc.boundary = derived_boundary(0);
        s.add_toolpath(0, tc).unwrap(); // gets id 1, index 1

        let cancel = AtomicBool::new(false);
        let msg = expect_operation_failed(s.generate_toolpath(1, &cancel));
        assert!(
            msg.contains("Pencil Rest"),
            "error should name the source toolpath: {msg}"
        );
        assert!(
            msg.contains("generate"),
            "error should tell the user to generate the source first: {msg}"
        );
    }

    #[test]
    fn derived_rest_regions_boundary_source_without_regions_errors() {
        let mut s = make_session();
        let mut source_tc = make_tc(s.tools()[0].id.0);
        source_tc.name = "Pencil Rest".to_owned();
        s.add_toolpath(0, source_tc).unwrap(); // id 0, index 0

        let mut tc = make_tc(s.tools()[0].id.0);
        tc.boundary = derived_boundary(0);
        s.add_toolpath(0, tc).unwrap(); // id 1, index 1

        // Source has a cached result, but its rest_regions is None (e.g. a
        // pencil op without the rest-depth detector, or any other op kind).
        s.results.insert(0, fake_result_with_regions(None));

        let cancel = AtomicBool::new(false);
        let msg = expect_operation_failed(s.generate_toolpath(1, &cancel));
        assert!(
            msg.contains("Pencil Rest"),
            "error should name the source toolpath: {msg}"
        );
        assert!(
            msg.contains("no rest regions"),
            "error should explain the source produced no regions: {msg}"
        );

        // Empty (rather than absent) regions fail the same way.
        s.results
            .insert(0, fake_result_with_regions(Some(Vec::new())));
        let msg = expect_operation_failed(s.generate_toolpath(1, &cancel));
        assert!(msg.contains("no rest regions"), "empty regions: {msg}");
    }

    #[test]
    fn derived_rest_regions_resolve_happy_path_returns_regions() {
        let mut s = make_session();
        let mut source_tc = make_tc(s.tools()[0].id.0);
        source_tc.name = "Pencil Rest".to_owned();
        s.add_toolpath(0, source_tc).unwrap(); // id 0, index 0

        let mut tc = make_tc(s.tools()[0].id.0);
        tc.boundary = derived_boundary(0);
        s.add_toolpath(0, tc).unwrap(); // id 1, index 1

        let regions = vec![
            crate::polygon::Polygon2::rectangle(0.0, 0.0, 10.0, 10.0),
            crate::polygon::Polygon2::rectangle(30.0, 30.0, 40.0, 40.0),
        ];
        s.results.insert(0, fake_result_with_regions(Some(regions)));

        let resolved = s
            .resolve_derived_rest_region_polys(1, ToolpathId(0))
            .expect("regions present on the source result");
        assert_eq!(resolved.len(), 2, "both disjoint regions come through");
    }

    #[test]
    fn apply_boundary_clip_multi_clips_to_disjoint_regions() {
        use crate::toolpath_spans::{AnnotatedToolpath, Span, SpanKind};

        // Two disjoint regions; a 3-move path visiting region A, the gap,
        // then region B. The gap move must become a rapid at safe_z, the two
        // region moves must survive, and spans must stay valid.
        let regions = vec![
            crate::polygon::Polygon2::rectangle(0.0, 0.0, 10.0, 10.0),
            crate::polygon::Polygon2::rectangle(30.0, 30.0, 40.0, 40.0),
        ];

        let mut tp = crate::toolpath::Toolpath::new();
        tp.feed_to(P3::new(5.0, 5.0, -1.0), 1000.0); // region A
        tp.feed_to(P3::new(20.0, 20.0, -1.0), 1000.0); // gap
        tp.feed_to(P3::new(35.0, 35.0, -1.0), 1000.0); // region B
        let n_moves = tp.moves.len();
        let annotated =
            AnnotatedToolpath::with_spans(tp, vec![Span::new(0, n_moves, SpanKind::Operation)]);

        let boundary = derived_boundary(0);
        let safe_z = 20.0;
        let recorder = ToolpathSemanticRecorder::new("test-tp", "Pocket");
        let semantic_ctx = recorder.root_context();

        let clipped = ProjectSession::apply_boundary_clip_multi(
            annotated,
            &boundary,
            &regions,
            &[],
            2.0,
            safe_z,
            &semantic_ctx,
            &mut crate::transform_provenance::ReconcileSet::new(Some(&recorder), None),
        );

        assert!(clipped.spans_valid, "spans stay valid through the set clip");
        assert_eq!(clipped.spans.len(), 1);
        assert_eq!(
            clipped.spans[0].end_move,
            clipped.toolpath.moves.len(),
            "operation span covers the whole clipped path"
        );

        // Gap move became a rapid at safe_z.
        let gap = clipped
            .toolpath
            .moves
            .iter()
            .find(|m| (m.target.x - 20.0).abs() < 1e-10)
            .expect("gap move present");
        assert_eq!(gap.move_type, crate::toolpath::MoveType::Rapid);
        assert!((gap.target.z - safe_z).abs() < 1e-10);

        // Both region moves survive as cuts.
        for (x, y) in [(5.0, 5.0), (35.0, 35.0)] {
            assert!(
                clipped.toolpath.moves.iter().any(|m| {
                    m.move_type != crate::toolpath::MoveType::Rapid
                        && (m.target.x - x).abs() < 1e-10
                        && (m.target.y - y).abs() < 1e-10
                }),
                "cut at ({x}, {y}) should survive the set clip"
            );
        }
    }

    #[test]
    fn apply_boundary_clip_multi_all_regions_collapsed_returns_original() {
        use crate::toolpath_spans::AnnotatedToolpath;

        // A tiny region with a large negative user offset collapses; with
        // every region gone the toolpath must pass through unchanged (the
        // single-polygon path's "boundary collapsed" semantics).
        let regions = vec![crate::polygon::Polygon2::rectangle(0.0, 0.0, 2.0, 2.0)];

        let mut tp = crate::toolpath::Toolpath::new();
        tp.feed_to(P3::new(50.0, 50.0, -1.0), 1000.0);
        tp.feed_to(P3::new(60.0, 50.0, -1.0), 1000.0);
        let move_count = tp.moves.len();
        let annotated = AnnotatedToolpath::new(tp);

        let mut boundary = derived_boundary(0);
        boundary.offset = -10.0; // shrink by 10mm — eats the 2mm square

        let recorder = ToolpathSemanticRecorder::new("test-tp", "Pocket");
        let semantic_ctx = recorder.root_context();

        let clipped = ProjectSession::apply_boundary_clip_multi(
            annotated,
            &boundary,
            &regions,
            &[],
            2.0,
            20.0,
            &semantic_ctx,
            &mut crate::transform_provenance::ReconcileSet::new(Some(&recorder), None),
        );

        assert_eq!(
            clipped.toolpath.moves.len(),
            move_count,
            "collapsed boundary set must leave the toolpath unchanged"
        );
        assert!(
            clipped
                .toolpath
                .moves
                .iter()
                .all(|m| m.move_type != crate::toolpath::MoveType::Rapid),
            "no retracts inserted when the boundary collapses"
        );
    }
}
