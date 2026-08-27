//! Multi-tool island finishing — the **planner action** and the boundary
//! resolution that feeds the ops it emits.
//!
//! Phase O of `planning/multitool_2026-08-23/ORCHESTRATION_PLAN.md`. The
//! layers below it already exist and are not re-implemented here:
//! [`crate::tier_map`] walks the *n*-tool residual, [`crate::tier_map_cache`]
//! memoises the walk, and [`crate::tier_islands`] conditions the labels into
//! per-tier [`RegionSet`]s. This module is the seam between those and the
//! session's op chain.
//!
//! # Why an op CHAIN and not one op
//!
//! T3 §1, decisively: gates, G-code (`M6` per phase boundary), feeds
//! provenance and stock chaining all key on **one op, one tool**. The
//! one-op precedent this repo already measured (`pencil_claims`) cost +22.5%
//! time for zero quality gain. So a plan is `k` ordinary `unified_finish`
//! ops, coarse → fine, each carrying its own tool — and per-tier diagnostics
//! come free as per-op rows.
//!
//! # Planning is CHEAP; the tier map is not computed here
//!
//! [`ProjectSession::plan_multitool_finishing`] computes **no tier map**. It
//! validates the ladder, allocates a plan id, replaces any prior plan, and
//! writes `k` configs. The boundary each fine tier carries is a *recipe*
//! ([`BoundarySource::PlannedTierRegions`]), resolved lazily at generation
//! time through [`crate::tier_map_cache::cached_tier_map`] — so the `k`
//! sibling ops share ONE grid walk, and re-planning at a different coarseness
//! costs nothing until something is generated.
//!
//! That is deliberate and it is the same call [`BoundarySource::DerivedRestRegions`]
//! makes: a project file stores what to compute, never a snapshot of a
//! computation. A per-tool full-grid drop-cutter map costs ≈ 8 s at 0.6 mm and
//! ≈ 31 s at 0.3 mm on the reference board — not something to pay at dialog
//! time, and not something to freeze into a `.toml` where it would go stale
//! against the mesh it describes.
//!
//! [`ProjectSession::preview_multitool_plan`] is the one entry point that DOES
//! pay for the walk, on purpose: it is the operator's veto surface, and a veto
//! has to be shown the real territory. It takes `&self` and a cancel token —
//! it emits nothing, replaces nothing, and can be abandoned mid-walk.
//!
//! # Equal cusp across tiers
//!
//! Every tier is dialled to the SAME cusp height, so the seam between two
//! tiers blends two cusp patterns of equal amplitude instead of printing a
//! step. For a ball of tip radius `R` leaving cusp height `h` between
//! adjacent passes, the stepover that produces it is
//!
//! ```text
//! s = 2·√(2·R·h − h²)
//! ```
//!
//! ([`equal_cusp_stepover_mm`]). At `h = 30 µm` that is **0.597 mm** for an
//! R1.5 ball and **0.690 mm** for an R2.0 — the pair the T4 probe derived by
//! hand (`ORCHESTRATION_PLAN.md` §0) and the pair
//! `tests/multitool_plan_emission_o1.rs` pins.
//!
//! `stock_to_leave` is held EQUAL across tiers for the same reason (T2 §7): a
//! seam across a slope change prints a `stock_to_leave · Δcos θ` step, and
//! with one value everywhere the term vanishes.
//!
//! # Re-plan is REPLACE, not accumulate
//!
//! One plan per setup. Re-planning removes **every** op in the target setup
//! carrying a [`PlannerOrigin`], whatever its `plan_id`, before emitting the
//! new ladder — through [`ProjectSession::remove_toolpath`], so the setups'
//! index lists and the results cache stay consistent. The removed ids come
//! back on [`MultitoolPlanOutcome::replaced`] so a caller can tell the
//! operator what went.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tracing::warn;

use crate::compute::catalog::{OperationConfig, OperationType};
use crate::compute::config::{
    BoundaryConfig, BoundaryContainment, BoundarySource, DressupConfig, HeightsConfig, StockSource,
};
use crate::compute::cutter::build_cutter;
use crate::compute::operation_configs::UnifiedFinishConfig;
use crate::compute::tool_config::ToolConfig;
use crate::ids::ToolpathId;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::polygon::Polygon2;
use crate::tier_islands::{TierIslandParams, TierIslands, extract_tier_islands};
use crate::tier_map::{ResidualTreatment, TierLadder, TierMap, TierMapParams};
use crate::tool::{MillingCutter, ToolDefinition};

use super::{PlannerOrigin, ProjectSession, SessionError, SetupEvalContext, ToolpathConfig};

/// Default tier-map planning resolution (mm).
///
/// **Never 0.15**: a full-grid drop-cutter map at that cell costs ≈ 125 s per
/// ladder tool on the 200 mm reference board (`T1_FINDINGS.md` §1.3), and the
/// board OOMs simulation at 0.1 mm. 0.4 sits in the plan's 0.3–0.6 band.
pub const DEFAULT_PLAN_CELL_MM: f64 = 0.4;

/// Default residual tolerance (mm) for the tier map.
pub const DEFAULT_PLAN_TOLERANCE_MM: f64 = 0.05;

/// Default grid padding (mm) beyond the finest tool's envelope.
pub const DEFAULT_PLAN_MARGIN_MM: f64 = 0.5;

/// Default target cusp height (mm) every tier is dialled to. 30 µm is the
/// number the C2 keeper and the T4 two-tier probe were both measured at.
pub const DEFAULT_PLAN_CUSP_HEIGHT_MM: f64 = 0.03;

/// The stepover that leaves cusp height `cusp_height_mm` between adjacent
/// passes of a ball of tip radius `cusp_radius_mm`:
/// `s = 2·√(2·R·h − h²)`.
///
/// ONE site, because the whole point of the dial is that two tiers agree: a
/// second spelling of this law in a UI pre-fill or a test would be free to
/// drift, and the seam it produced would be a height step nobody could
/// attribute.
///
/// Returns `0.0` for a non-positive radius or cusp, and for a cusp deeper
/// than the ball's own radius (`h ≥ 2R` makes the radicand negative — that is
/// not a stepover, it is a mis-typed number).
#[must_use]
pub fn equal_cusp_stepover_mm(cusp_radius_mm: f64, cusp_height_mm: f64) -> f64 {
    let (r, h) = (cusp_radius_mm, cusp_height_mm);
    if !(r.is_finite() && h.is_finite()) || r <= 0.0 || h <= 0.0 {
        return 0.0;
    }
    let radicand = 2.0 * r * h - h * h;
    if radicand <= 0.0 {
        return 0.0;
    }
    2.0 * radicand.sqrt()
}

/// What to plan. Every field is an operator dial; nothing here is derived
/// from the session.
#[derive(Debug, Clone)]
pub struct MultitoolPlanSpec {
    /// Setup the chain is emitted into.
    pub setup_index: usize,
    /// Model every emitted op targets. Must carry a 3D mesh.
    pub model_id: usize,
    /// Session tool ids taking part. Order is irrelevant — the ladder sorts
    /// coarse → fine on [`MillingCutter::cusp_radius_mm`] itself, which is
    /// the tip-sphere feature scale, never `radius()` (on a tapered ball
    /// that is the shank, and it would call a Ø1-tip finisher the coarsest
    /// tool on the ladder).
    pub tool_ids: Vec<usize>,
    /// Tier-map planning resolution (mm). See [`DEFAULT_PLAN_CELL_MM`].
    pub cell_mm: f64,
    /// Residual tolerance (mm).
    pub tolerance_mm: f64,
    /// Grid padding (mm) beyond the finest tool's envelope.
    pub margin_mm: f64,
    /// How the raw residual is treated. `SlopeCompensated` is the B1
    /// decision (`ORCHESTRATION_PLAN.md` §6): on the wanaka A/B the raw
    /// treatment gave the fine tier 71.6% of the board and the compensated
    /// one 22.0%, within 3% of the stock-referenced truth.
    pub treatment: ResidualTreatment,
    /// Island close / min-area / overlap / cap dials.
    pub islands: TierIslandParams,
    /// Target cusp height (mm) every tier is dialled to. See the module doc
    /// and [`equal_cusp_stepover_mm`].
    pub cusp_height_mm: f64,
}

impl Default for MultitoolPlanSpec {
    fn default() -> Self {
        Self {
            setup_index: 0,
            model_id: 0,
            tool_ids: Vec::new(),
            cell_mm: DEFAULT_PLAN_CELL_MM,
            tolerance_mm: DEFAULT_PLAN_TOLERANCE_MM,
            margin_mm: DEFAULT_PLAN_MARGIN_MM,
            // B1 decision — see the field doc.
            treatment: ResidualTreatment::SlopeCompensated,
            islands: TierIslandParams::default(),
            cusp_height_mm: DEFAULT_PLAN_CUSP_HEIGHT_MM,
        }
    }
}

/// What one plan run did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultitoolPlanOutcome {
    /// The id this run stamped on every op it emitted.
    pub plan_id: u64,
    /// Emitted ops, coarse → fine.
    pub toolpath_ids: Vec<ToolpathId>,
    /// Prior planner-origin ops this run removed. Empty on a first plan.
    pub replaced: Vec<ToolpathId>,
}

/// What a [`MultitoolPlanSpec`] would plan, computed without emitting
/// anything — the payload behind the operator's tier-map veto
/// (`ORCHESTRATION_PLAN.md` §3.1) and behind the agent-visible preview.
///
/// Every vector is in **ladder order, coarse → fine**, and the three are
/// index-aligned with each other and with [`TierMap`]'s labels: entry `k` is
/// tier `k`. That is the same ordering [`TierIslands::per_tier`] publishes
/// (minus its absent tier 0), and the same ordering the emitted op chain runs
/// in.
#[derive(Debug, Clone)]
pub struct MultitoolPreview {
    /// Per-cell tier labels over the planning grid.
    pub map: TierMap,
    /// The labels conditioned into per-tier island sets. Read
    /// [`TierIslands::tiers_where_the_cap_acted`] before showing an operator a
    /// count: a tier whose cap fired has merged or dropped islands, and the
    /// preview is where that has to be said.
    pub islands: TierIslands,
    /// Tip-sphere radii (mm), coarse → fine. `len() == map.tier_count`.
    pub cusp_radii_mm: Vec<f64>,
    /// Tool names in the same order, for labelling.
    pub tool_names: Vec<String>,
    /// Session tool ids in the same order — the ladder
    /// [`ProjectSession::plan_multitool_finishing`] will emit with, already
    /// sorted, so a caller that applies this preview gets the tiers it saw.
    pub tool_ids: Vec<usize>,
}

impl ProjectSession {
    /// Emit a multi-tool finishing chain into `spec.setup_index`, replacing
    /// any chain already there.
    ///
    /// **Cheap**: no tier map is computed. See the module doc.
    ///
    /// # Errors
    ///
    /// [`SessionError::SetupNotFound`] for an unknown setup;
    /// [`SessionError::MissingGeometry`] when the model is absent or carries
    /// no 3D mesh; [`SessionError::InvalidParam`] for a ladder under two
    /// tools, an unknown tool id, or a ladder
    /// [`TierLadder::new`] refuses.
    pub fn plan_multitool_finishing(
        &mut self,
        spec: &MultitoolPlanSpec,
    ) -> Result<MultitoolPlanOutcome, SessionError> {
        self.validate_multitool_spec(spec)?;

        // Resolve, then order coarse → fine. Validation only: the ladder is
        // rebuilt at generation time from the ids stored on the boundary,
        // because the tools may have been re-dialled in between.
        let ladder_tools = self.resolve_ladder_tools(&spec.tool_ids)?;
        let cutters: Vec<ToolDefinition> = ladder_tools.iter().map(build_cutter).collect();
        let refs: Vec<&dyn MillingCutter> =
            cutters.iter().map(|c| c as &dyn MillingCutter).collect();
        TierLadder::new(&refs).map_err(|e| {
            SessionError::InvalidParam(format!("multi-tool finishing: invalid ladder — {e}"))
        })?;
        let tier_count = u8::try_from(ladder_tools.len()).map_err(|_overflow| {
            SessionError::InvalidParam(format!(
                "multi-tool finishing: {} tiers exceeds the one-byte tier label",
                ladder_tools.len()
            ))
        })?;
        let ordered_ids: Vec<usize> = ladder_tools.iter().map(|t| t.id.0).collect();
        let cusp_radii: Vec<f64> = cutters.iter().map(MillingCutter::cusp_radius_mm).collect();

        let plan_id = self.next_plan_id();
        let replaced = self.remove_planned_toolpaths(spec.setup_index);

        let mut toolpath_ids = Vec::with_capacity(ladder_tools.len());
        for (tier_usize, tool) in ladder_tools.iter().enumerate() {
            // Unreachable: `tier_count` converted above, so every index
            // below it fits a `u8`. Written as a `let ... else` rather than
            // an unwrap because a lint-clean impossibility costs one line.
            let Ok(tier) = u8::try_from(tier_usize) else {
                continue;
            };
            let cusp = cusp_radii.get(tier_usize).copied().unwrap_or(0.0);
            let mut operation = plan_tier_operation(cusp, spec);
            let feeds_provenance = self.suggest_feeds_for(&mut operation, tool);
            let cfg = ToolpathConfig {
                id: ToolpathId(0),
                name: format!("Finish tier {tier} (R{cusp:.1})"),
                enabled: true,
                dressups: DressupConfig::for_op(OperationType::UnifiedFinish),
                heights: HeightsConfig::default(),
                tool_id: tool.id.0,
                model_id: spec.model_id,
                pre_gcode: None,
                post_gcode: None,
                boundary: tier_boundary(tier, &ordered_ids, spec),
                // A planner boundary is the whole point of the op; letting
                // the stock default overwrite it would silently un-confine
                // the fine tier back onto the whole board.
                boundary_inherit: false,
                rest_analysis: crate::compute::config::RestAnalysisConfig::default(),
                // EVERY tier, tier 0 included: they chain after roughing and
                // after each other, so each one cuts what its predecessor
                // left. Tier 0's own predecessor is the roughing pass.
                stock_source: StockSource::FromRemainingStock,
                coolant: crate::gcode::CoolantMode::Off,
                face_selection: None,
                debug_options: crate::debug_trace::ToolpathDebugOptions::default(),
                operation,
                feeds_provenance,
                planner_origin: Some(PlannerOrigin {
                    plan_id,
                    tier,
                    tier_count,
                }),
            };
            let index = self.add_toolpath(spec.setup_index, cfg)?;
            let Some(added) = self.toolpath_configs.get(index) else {
                return Err(SessionError::ToolpathNotFound(index));
            };
            toolpath_ids.push(added.id);
        }

        Ok(MultitoolPlanOutcome {
            plan_id,
            toolpath_ids,
            replaced,
        })
    }

    /// The setup / model / ladder-length preconditions both
    /// [`Self::plan_multitool_finishing`] and [`Self::preview_multitool_plan`]
    /// hold to.
    ///
    /// ONE site, because the preview is the operator's veto surface: a
    /// preview that accepted a spec the plan then refused would put the
    /// refusal after the decision instead of before it.
    ///
    /// # Errors
    ///
    /// See [`Self::plan_multitool_finishing`].
    fn validate_multitool_spec(&self, spec: &MultitoolPlanSpec) -> Result<(), SessionError> {
        if self.setups.get(spec.setup_index).is_none() {
            return Err(SessionError::SetupNotFound(spec.setup_index));
        }
        let model_has_mesh = self
            .models
            .iter()
            .find(|m| m.id == spec.model_id)
            .map(|m| m.mesh.is_some());
        match model_has_mesh {
            None => {
                return Err(SessionError::MissingGeometry(format!(
                    "multi-tool finishing: no model with id {} in this project",
                    spec.model_id
                )));
            }
            Some(false) => {
                return Err(SessionError::MissingGeometry(format!(
                    "multi-tool finishing: model {} carries no 3D mesh. The tier map is a \
                     drop-cutter residual over a surface; a 2D drawing has none.",
                    spec.model_id
                )));
            }
            Some(true) => {}
        }
        if spec.tool_ids.len() < 2 {
            return Err(SessionError::InvalidParam(format!(
                "multi-tool finishing needs at least two tools; {} given. A one-tool ladder \
                 is an ordinary finish pass — use one directly.",
                spec.tool_ids.len()
            )));
        }
        Ok(())
    }

    /// The mesh and spatial index `spec`'s setup would GENERATE against.
    ///
    /// The same two lines `session::compute` runs per toolpath — the
    /// setup-transformed mesh from [`crate::geom_cache::cached_transform`] on a
    /// non-identity setup, the model's own mesh otherwise, and
    /// [`crate::geom_cache::cached_auto_index`] over whichever it is. Sharing
    /// the memo is what makes the preview's map and the emitted ops' maps the
    /// SAME cached object rather than two walks that happen to agree: the
    /// tier-map memo keys on mesh identity, so a preview built off a private
    /// copy of the mesh would miss on every generate.
    fn plan_geometry(
        &self,
        spec: &MultitoolPlanSpec,
    ) -> (Option<Arc<TriangleMesh>>, Option<Arc<SpatialIndex>>) {
        let Some(mut mesh) = self
            .models
            .iter()
            .find(|m| m.id == spec.model_id)
            .and_then(|m| m.mesh.clone())
        else {
            return (None, None);
        };
        let ctx = SetupEvalContext::build_for_setup(self, self.setups.get(spec.setup_index));
        if ctx.needs_transform() {
            mesh = crate::geom_cache::cached_transform(
                &mesh,
                &self.setup_transform_info(ctx.face_up, ctx.z_rotation),
            );
        }
        let index = crate::geom_cache::cached_auto_index(&mesh);
        (Some(mesh), Some(index))
    }

    /// Compute the tier map and per-tier islands `spec` would plan with,
    /// **without touching a single toolpath**.
    ///
    /// `&self` is the veto guarantee, and it is load-bearing rather than
    /// stylistic: the operator decision point this serves
    /// (`ORCHESTRATION_PLAN.md` §3.1) is "see the territory before anything is
    /// generated, and reject without cost". A preview that could mutate would
    /// make rejecting cost something.
    ///
    /// It is NOT cheap the way [`Self::plan_multitool_finishing`] is — this is
    /// the call that pays for the grid walk (≈ 8 s at 0.6 mm, ≈ 31 s at 0.3 mm
    /// per ladder tool on the reference board). It is memoised
    /// ([`crate::tier_map_cache`]), so a second preview that changes only the
    /// island dials re-runs the morphology alone, and the ops the plan later
    /// emits resolve their boundaries off the very same cached map. Run it off
    /// the UI thread and hand it a `cancel` the caller can fire.
    ///
    /// # Errors
    ///
    /// Everything [`Self::plan_multitool_finishing`] refuses, plus
    /// [`SessionError::MissingGeometry`] when the model's mesh cannot be
    /// resolved in the setup's frame and [`SessionError::OperationFailed`]
    /// when the walk is cancelled or the island extraction refuses.
    pub fn preview_multitool_plan(
        &self,
        spec: &MultitoolPlanSpec,
        cancel: &AtomicBool,
    ) -> Result<MultitoolPreview, SessionError> {
        self.validate_multitool_spec(spec)?;
        let (mesh, index) = self.plan_geometry(spec);
        let recipe = PlannedTierRecipe {
            tool_ids: &spec.tool_ids,
            // Ignored: `require_fine_tier` is false, so no tier is selected.
            tier: 0,
            cell_mm: spec.cell_mm,
            tolerance_mm: spec.tolerance_mm,
            margin_mm: spec.margin_mm,
            treatment: spec.treatment,
            islands: spec.islands,
        };
        let resolved = self.resolve_tier_plan(
            "multi-tool preview",
            TierPlanGeometry {
                mesh: mesh.as_ref(),
                index: index.as_ref(),
            },
            &recipe,
            false,
            cancel,
        )?;
        Ok(MultitoolPreview {
            map: resolved.map.as_ref().clone(),
            islands: resolved.islands,
            cusp_radii_mm: resolved.cusp_radii_mm,
            tool_names: resolved.tools.iter().map(|t| t.name.clone()).collect(),
            tool_ids: resolved.tools.iter().map(|t| t.id.0).collect(),
        })
    }

    /// Resolve `tool_ids` to real tools, ordered coarse → fine on cusp
    /// radius with a total tie-break on the id so two equal-radius tools
    /// order the same way on every run.
    fn resolve_ladder_tools(&self, tool_ids: &[usize]) -> Result<Vec<ToolConfig>, SessionError> {
        let mut tools = Vec::with_capacity(tool_ids.len());
        for &id in tool_ids {
            let Some(tool) = self.tools.iter().find(|t| t.id.0 == id) else {
                return Err(SessionError::InvalidParam(format!(
                    "multi-tool finishing: no tool with id {id} in this project"
                )));
            };
            tools.push(tool.clone());
        }
        tools.sort_by(|a, b| {
            let (ra, rb) = (
                build_cutter(a).cusp_radius_mm(),
                build_cutter(b).cusp_radius_mm(),
            );
            rb.total_cmp(&ra).then_with(|| a.id.0.cmp(&b.id.0))
        });
        Ok(tools)
    }

    /// One past the largest `plan_id` any toolpath in the project carries.
    /// Monotonic, and it never reuses an id even when the plan that owned it
    /// was removed — a stamp that could recur would let two different
    /// ladders read as one.
    fn next_plan_id(&self) -> u64 {
        self.toolpath_configs
            .iter()
            .filter_map(|tc| tc.planner_origin.as_ref().map(|o| o.plan_id))
            .max()
            .map_or(0, |m| m.saturating_add(1))
    }

    /// Remove every planner-origin op in `setup_index`, returning their ids
    /// in emission order.
    ///
    /// Removal goes through [`Self::remove_toolpath`] — never a raw `Vec`
    /// removal — because that is what re-keys the results cache and shifts
    /// every setup's `toolpath_indices`. Indices are taken descending so
    /// each removal cannot invalidate the next one.
    fn remove_planned_toolpaths(&mut self, setup_index: usize) -> Vec<ToolpathId> {
        let Some(setup) = self.setups.get(setup_index) else {
            return Vec::new();
        };
        let mut victims: Vec<usize> = setup
            .toolpath_indices
            .iter()
            .copied()
            .filter(|&i| {
                self.toolpath_configs
                    .get(i)
                    .is_some_and(|tc| tc.planner_origin.is_some())
            })
            .collect();
        victims.sort_unstable();
        let ids: Vec<ToolpathId> = victims
            .iter()
            .filter_map(|&i| self.toolpath_configs.get(i).map(|tc| tc.id))
            .collect();
        for &index in victims.iter().rev() {
            if let Err(e) = self.remove_toolpath(index) {
                warn!(
                    index,
                    error = %e,
                    "multi-tool re-plan: could not remove a prior planner op"
                );
            }
        }
        ids
    }

    /// Run the Suggest funnel for one emitted tier and return its
    /// provenance, having written the suggested feeds into `operation`.
    ///
    /// On a refusal the op keeps its config defaults and an all-`None`
    /// provenance, with a `warn` — never a skipped emission. Same policy as
    /// the alignment-pin-drill auto-create, and for the same reason: a
    /// missing op is a silent hole in the chain, a default feed is a visible
    /// number the operator can correct.
    ///
    /// The GEOMETRY dials set by [`plan_tier_operation`] are re-applied
    /// afterwards. Suggest owns feeds; the planner owns the equal-cusp
    /// stepover, and letting the funnel's own stepover heuristic overwrite it
    /// would silently break the property the whole plan rests on.
    fn suggest_feeds_for(
        &self,
        operation: &mut OperationConfig,
        tool: &ToolConfig,
    ) -> crate::feeds::FeedsProvenance {
        let planned = operation.clone();
        let suggested = crate::feeds::suggest::suggest_for_operation(
            crate::feeds::suggest::SuggestForOperationInput {
                operation: &planned,
                tool,
                machine: &self.machine,
                material: &self.stock.material,
                workholding: self.stock.workholding_rigidity,
                lut: crate::feeds::embedded_vendor_lut(),
                spindle_strategy: crate::feeds::SpindleStrategy::default(),
                context: crate::feeds::suggest::SuggestContext::default(),
            },
        );
        match suggested {
            Ok(s) => {
                *operation = s.operation;
                restore_planned_geometry(operation, &planned);
                s.provenance
            }
            Err(e) => {
                warn!(
                    tool = %tool.name,
                    error = %e,
                    "multi-tool plan: Suggest refused for this tier; emitting with config \
                     defaults"
                );
                crate::feeds::FeedsProvenance::default()
            }
        }
    }
}

/// The `UnifiedFinishConfig` for one tier: the type's defaults, then the
/// three dials the planner owns.
///
/// A free function rather than a method because it reads nothing from the
/// session — the tier's cusp radius and the spec are the whole input, which
/// is what lets a UI pre-fill show the same numbers the plan will emit.
fn plan_tier_operation(cusp_radius_mm: f64, spec: &MultitoolPlanSpec) -> OperationConfig {
    let stepover = equal_cusp_stepover_mm(cusp_radius_mm, spec.cusp_height_mm);
    let defaults = UnifiedFinishConfig::default();
    OperationConfig::UnifiedFinish(UnifiedFinishConfig {
        // Equal cusp across tiers — the seam blends two patterns of the same
        // amplitude. `scallop_height` IS the cusp height by definition;
        // `raster_stepover` is the stepover that produces it on the shallow
        // band.
        scallop_height: spec.cusp_height_mm,
        raster_stepover: if stepover > 0.0 {
            stepover
        } else {
            defaults.raster_stepover
        },
        // Held EQUAL across tiers (T2 §7): any non-zero value prints a
        // `stock_to_leave · Δcos θ` step at a tier seam that crosses a slope
        // change.
        stock_to_leave: 0.0,
        ..defaults
    })
}

/// Re-apply the planner-owned geometry dials after the Suggest funnel has
/// rewritten the operation. See [`ProjectSession::suggest_feeds_for`].
fn restore_planned_geometry(operation: &mut OperationConfig, planned: &OperationConfig) {
    if let (OperationConfig::UnifiedFinish(out), OperationConfig::UnifiedFinish(want)) =
        (&mut *operation, planned)
    {
        out.scallop_height = want.scallop_height;
        out.raster_stepover = want.raster_stepover;
        out.stock_to_leave = want.stock_to_leave;
    }
}

/// The boundary one tier carries.
///
/// Tier 0 gets the type default (disabled): its cusp target holds everywhere
/// by construction, so the coarse tool sweeps the whole board as one pass and
/// needs no confinement. Only fine tiers carry islands.
fn tier_boundary(tier: u8, ordered_ids: &[usize], spec: &MultitoolPlanSpec) -> BoundaryConfig {
    if tier == 0 {
        return BoundaryConfig::default();
    }
    BoundaryConfig {
        enabled: true,
        source: BoundarySource::PlannedTierRegions {
            tool_ids: ordered_ids.to_vec(),
            tier,
            cell_mm: spec.cell_mm,
            tolerance_mm: spec.tolerance_mm,
            margin_mm: spec.margin_mm,
            treatment: spec.treatment,
            islands: spec.islands,
        },
        // CENTRE containment, matching every shipped rest-driven consumer.
        // `Inside` would inset each island by a further tool radius on top
        // of the `overlap_mm` blend band the islands already carry, which is
        // the opposite of what the band is for.
        containment: BoundaryContainment::Center,
        offset: 0.0,
    }
}

// ── Boundary resolution ─────────────────────────────────────────────────

/// The recipe half of [`BoundarySource::PlannedTierRegions`], borrowed.
///
/// A struct rather than eight positional arguments because both call sites in
/// `session::compute` destructure the same variant, and a positional
/// signature is how `cell_mm` and `tolerance_mm` end up swapped.
pub(crate) struct PlannedTierRecipe<'a> {
    pub tool_ids: &'a [usize],
    pub tier: u8,
    pub cell_mm: f64,
    pub tolerance_mm: f64,
    pub margin_mm: f64,
    pub treatment: ResidualTreatment,
    pub islands: TierIslandParams,
}

/// The geometry a tier-map walk runs over, borrowed. A struct because both
/// halves are `Option` and two adjacent optional references are exactly the
/// pair a positional signature lets a caller swap.
#[derive(Clone, Copy)]
struct TierPlanGeometry<'a> {
    mesh: Option<&'a Arc<TriangleMesh>>,
    index: Option<&'a Arc<SpatialIndex>>,
}

/// One resolved tier map and its islands, plus the ladder they were built
/// from. The single answer both the preview and the boundary resolution read.
struct TierPlanResolution {
    /// Shared with the memo — the point of the `Arc` is that the second
    /// resolution of the same recipe is the SAME object, not an equal one.
    map: Arc<TierMap>,
    islands: TierIslands,
    /// Tip-sphere radii (mm), coarse → fine.
    cusp_radii_mm: Vec<f64>,
    /// The ladder's tools, coarse → fine.
    tools: Vec<ToolConfig>,
}

impl ProjectSession {
    /// Ladder → memoised tier map → per-tier islands.
    ///
    /// The single spelling of that pipeline. Both callers
    /// ([`Self::preview_multitool_plan`] and
    /// [`Self::resolve_planned_tier_region_polys`]) must produce the same
    /// islands for the same recipe or the operator's veto would be a veto on
    /// something other than what gets machined; one function is how that stops
    /// being a property two code paths have to keep.
    ///
    /// `require_fine_tier` validates `recipe.tier` as a real fine tier
    /// **before** the walk, so a mis-addressed boundary refuses without paying
    /// for a full-grid drop-cutter map. Pass `false` when no single tier is
    /// being selected (the preview wants every tier).
    fn resolve_tier_plan(
        &self,
        op_name: &str,
        geometry: TierPlanGeometry<'_>,
        recipe: &PlannedTierRecipe<'_>,
        require_fine_tier: bool,
        cancel: &AtomicBool,
    ) -> Result<TierPlanResolution, SessionError> {
        let (Some(mesh), Some(index)) = (geometry.mesh, geometry.index) else {
            return Err(SessionError::MissingGeometry(format!(
                "'{op_name}' needs a planned tier's islands, but its model carries no 3D \
                 mesh. The tier map is a drop-cutter residual over a surface."
            )));
        };

        let tools = self
            .resolve_ladder_tools(recipe.tool_ids)
            .map_err(|e| annotate(op_name, &e))?;
        let cutters: Vec<ToolDefinition> = tools.iter().map(build_cutter).collect();
        let refs: Vec<&dyn MillingCutter> =
            cutters.iter().map(|c| c as &dyn MillingCutter).collect();
        let ladder = TierLadder::new(&refs).map_err(|e| {
            SessionError::InvalidParam(format!(
                "'{op_name}': the ladder stored on its boundary is not usable — {e}"
            ))
        })?;
        if require_fine_tier && (usize::from(recipe.tier) >= ladder.len() || recipe.tier == 0) {
            return Err(SessionError::InvalidParam(format!(
                "'{op_name}' is bounded to tier {} of a {}-tier ladder. Tier 0 is the coarse \
                 tool's complement and carries no islands; anything at or above the ladder \
                 length does not exist.",
                recipe.tier,
                ladder.len()
            )));
        }

        let params = TierMapParams {
            cell_mm: recipe.cell_mm,
            tolerance_mm: recipe.tolerance_mm,
            margin_mm: recipe.margin_mm,
            treatment: recipe.treatment,
        };
        let map = crate::tier_map_cache::cached_tier_map(
            mesh,
            index.as_ref(),
            &ladder,
            &params,
            &(|| cancel.load(std::sync::atomic::Ordering::SeqCst)),
        )
        .map_err(|e| {
            SessionError::OperationFailed(format!("'{op_name}': tier map could not be built — {e}"))
        })?;

        let cusp_radii_mm: Vec<f64> = cutters.iter().map(MillingCutter::cusp_radius_mm).collect();
        let islands = extract_tier_islands(&map, &recipe.islands, &cusp_radii_mm).map_err(|e| {
            SessionError::OperationFailed(format!(
                "'{op_name}': tier islands could not be extracted — {e}"
            ))
        })?;

        Ok(TierPlanResolution {
            map,
            islands,
            cusp_radii_mm,
            tools,
        })
    }

    /// Resolve one tier's machining polygons for
    /// [`BoundarySource::PlannedTierRegions`].
    ///
    /// Called from BOTH the pre-generation boundary resolution and the
    /// post-generation clip, which must agree about what "this tier's
    /// islands" are. They do because the walk itself is memoised on (mesh
    /// identity, ladder, params) — the second call is a cache hit on the
    /// exact map the first one built, so agreement is structural rather than
    /// a property of two code paths staying in step (the P2.3 lesson).
    /// [`Self::preview_multitool_plan`] joins that guarantee from the other
    /// end: all three go through [`Self::resolve_tier_plan`], so what the
    /// operator vetoed and what the fine tier is confined to are one
    /// computation, not two that agree.
    ///
    /// Returns the tier's `machining` set — the islands grown by
    /// `overlap_mm` — not `owned`. The blend band is the point: the fine
    /// tool's first pass must land on ground the coarse tool already cut.
    ///
    /// An **empty** result is not an error: it means the coarse tool holds
    /// the whole board at this tolerance, which is a planning outcome, not a
    /// misconfiguration. The op then generates nothing, and because every
    /// planner-emitted op is `StockSource::FromRemainingStock` the
    /// zero-emission gate reads it as
    /// [`crate::compute::generated_empty::LegitimateEmptyReason::RestMachining`]
    /// rather than refusing. (This is where the variant deliberately
    /// DIVERGES from `DerivedRestRegions`, whose resolver returns a hard
    /// error on an empty region list — there, empty means the operator
    /// pointed at a source that produced no rest, which is something to
    /// fix.)
    ///
    /// # Errors
    ///
    /// [`SessionError::MissingGeometry`] with no mesh or spatial index in
    /// scope; [`SessionError::InvalidParam`] for an unknown tool id, a
    /// ladder [`TierLadder::new`] refuses, or a tier index off the ladder;
    /// [`SessionError::OperationFailed`] if the walk is cancelled or the
    /// island extraction refuses. Every message names `op_name`.
    pub(crate) fn resolve_planned_tier_region_polys(
        &self,
        op_name: &str,
        mesh: Option<&Arc<TriangleMesh>>,
        index: Option<&Arc<SpatialIndex>>,
        recipe: &PlannedTierRecipe<'_>,
        cancel: &AtomicBool,
    ) -> Result<Vec<Polygon2>, SessionError> {
        let resolved = self.resolve_tier_plan(
            op_name,
            TierPlanGeometry { mesh, index },
            recipe,
            true,
            cancel,
        )?;
        Ok(resolved
            .islands
            .set_for_tier(recipe.tier)
            .map(|set| set.machining.as_slice().to_vec())
            .unwrap_or_default())
    }
}

/// Prefix a resolver error with the op it belongs to. A boundary failure that
/// does not name its operation is unactionable in a `generate_all` log.
fn annotate(op_name: &str, err: &SessionError) -> SessionError {
    SessionError::InvalidParam(format!("'{op_name}': {err}"))
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

    #[test]
    fn the_equal_cusp_law_reproduces_the_t4_pair() {
        // `ORCHESTRATION_PLAN.md` §0: tier-A was retooled R1.5 -> R2.0 at
        // equal 30 µm cusp, raster_stepover 0.6 -> 0.69. Both by hand.
        let h = 0.03;
        assert!((equal_cusp_stepover_mm(1.5, h) - 0.596_992_462).abs() < 1e-9);
        assert!((equal_cusp_stepover_mm(2.0, h) - 0.690_217_357).abs() < 1e-9);
    }

    #[test]
    fn the_equal_cusp_law_refuses_nonsense_rather_than_returning_nan() {
        assert_eq!(equal_cusp_stepover_mm(0.0, 0.03), 0.0);
        assert_eq!(equal_cusp_stepover_mm(-1.0, 0.03), 0.0);
        assert_eq!(equal_cusp_stepover_mm(1.5, 0.0), 0.0);
        // h >= 2R: the radicand goes negative, which is not a stepover.
        assert_eq!(equal_cusp_stepover_mm(0.5, 1.0), 0.0);
        assert_eq!(equal_cusp_stepover_mm(f64::NAN, 0.03), 0.0);
    }

    #[test]
    fn a_coarser_tool_gets_a_wider_stepover_at_the_same_cusp() {
        let h = 0.03;
        assert!(equal_cusp_stepover_mm(2.0, h) > equal_cusp_stepover_mm(1.5, h));
        assert!(equal_cusp_stepover_mm(1.5, h) > equal_cusp_stepover_mm(0.5, h));
    }

    #[test]
    fn tier_zero_carries_no_boundary_and_fine_tiers_do() {
        let spec = MultitoolPlanSpec {
            tool_ids: vec![0, 1],
            ..MultitoolPlanSpec::default()
        };
        let coarse = tier_boundary(0, &[0, 1], &spec);
        assert!(!coarse.enabled);
        assert_eq!(coarse.source, BoundarySource::Stock);

        let fine = tier_boundary(1, &[0, 1], &spec);
        assert!(fine.enabled);
        assert_eq!(fine.containment, BoundaryContainment::Center);
        match fine.source {
            BoundarySource::PlannedTierRegions {
                ref tool_ids, tier, ..
            } => {
                assert_eq!(tool_ids, &vec![0, 1], "the FULL ladder, not just this tier");
                assert_eq!(tier, 1);
            }
            ref other => panic!("expected PlannedTierRegions, got {other:?}"),
        }
    }
}
