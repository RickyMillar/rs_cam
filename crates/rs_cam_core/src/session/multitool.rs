//! Multi-tool island finishing — the **planner action** and the boundary
//! resolution that feeds the ops it emits.
//!
//! Phase O of `planning/multitool_2026-08-23/ORCHESTRATION_PLAN.md`. The
//! layers below it already exist and are not re-implemented here:
//! [`crate::maps::tier_map`] walks the *n*-tool residual, [`crate::maps::tier_map_cache`]
//! memoises the walk, and [`crate::maps::tier_islands`] conditions the labels into
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
//! time through [`crate::maps::tier_map_cache::cached_tier_map`] — so the `k`
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
use crate::maps::tier_islands::{
    TierBandAdvisory, TierIslandParams, TierIslands, extract_tier_islands,
};
use crate::maps::tier_map::{ResidualTreatment, TierLadder, TierMap, TierMapParams};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::polygon::Polygon2;
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
    /// When true, tier 0 does NOT sweep the whole board: it carries an
    /// INVERTED planned-tier boundary — everything except the fine tiers'
    /// `owned` islands — so the coarse tool skips ground a finer tool will
    /// re-finish anyway (operator-requested 2026-08-27). The seam still
    /// blends: fine tiers machine `owned + overlap_mm`, so the blend band
    /// lands on coarse-finished ground either way. The trade, stated: the
    /// fine tools then meet the ROUGHING pass's terraces inside their
    /// islands instead of a coarse-finished surface — higher load on small
    /// cutters, measurable by the load gates, which is why this is a dial
    /// and not the default.
    pub coarse_skips_fine_islands: bool,
    /// C2 (`planning/thin_organic_2026-08-27/PROGRAMME.md` Track C): the
    /// value every emitted tier's
    /// [`UnifiedFinishConfig::monotone_cell_decomposition`] carries.
    ///
    /// The planner is where this dial matters most — §0i's 1.155× / 1.215×
    /// were measured on exactly these tier ops, on dendritic tier islands —
    /// so it is a plan-level choice rather than something the operator has
    /// to set on each emitted operation afterwards. Default `true` since
    /// 2026-09-01 (C4 operator surface review passed — `FINDINGS.md` §7,
    /// "C4 ruling"), the same arm the op type itself ships; the tier config
    /// still carries the value explicitly, and
    /// [`restore_planned_geometry`] re-applies it after the Suggest funnel
    /// rewrites the operation.
    pub monotone_cell_decomposition: bool,
    /// Which OPERATION cuts each tier — the operator's separation of
    /// "region generation" from "toolpath choice" (2026-09-03). Indexed by
    /// LADDER TIER (coarse → fine, i.e. the sorted order, not `tool_ids`
    /// order); a missing entry means [`TierStrategy::UnifiedFinish`], so an
    /// empty vec reproduces the pre-dial planner byte-identically. The
    /// tier's TERRITORY is unchanged by this choice — it rides the
    /// boundary (`PlannedTierRegions`), which is op-agnostic.
    pub tier_strategies: Vec<TierStrategy>,
}

/// The operation family that cuts one planner tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TierStrategy {
    #[default]
    /// The band mix (raster / scallop / waterline per slope band) — the
    /// planner's historical only choice, and still the default.
    UnifiedFinish,
    /// The shipped offset-cascade scallop, whole territory as rings.
    Scallop,
    /// The iso-field scallop (M8: per-point spacing, cosine slope law,
    /// completion by construction). Measured 0.875× the unified op's time
    /// at 2.5 pp better envelope coverage on the wanaka board — on ONE
    /// large organic territory. On confetti territory (many small
    /// islands) per-island ring cascades pay plunges (the S1 arm-R
    /// lesson); the GUI shows the island count next to the dial.
    IsoScallop,
}

impl Default for MultitoolPlanSpec {
    fn default() -> Self {
        Self {
            tier_strategies: Vec::new(),
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
            coarse_skips_fine_islands: false,
            // ON since 2026-09-01 (C4 ruling) — the planner follows the op
            // type's own default.
            monotone_cell_decomposition: true,
        }
    }
}

/// What one plan run did.
#[derive(Debug, Clone, PartialEq)]
pub struct MultitoolPlanOutcome {
    /// The id this run stamped on every op it emitted.
    pub plan_id: u64,
    /// Emitted ops, coarse → fine.
    pub toolpath_ids: Vec<ToolpathId>,
    /// Prior planner-origin ops this run removed. Empty on a first plan.
    pub replaced: Vec<ToolpathId>,
    /// G-OVERLAPFILL: what the overlap band cost each fine tier in
    /// territory — see [`crate::maps::tier_islands::TierBandAdvisory`].
    ///
    /// **Three-valued.** `None` means **not measured**: planning is cheap by
    /// contract (no tier map is built here), and no map for this ladder and
    /// these dials was already in
    /// [`crate::maps::tier_map_cache`]. Preview first — either
    /// [`ProjectSession::preview_multitool_plan`] or a generate — and the
    /// next plan reads it. `Some(vec![])` means measured and healthy: every
    /// tier's band is under
    /// [`crate::maps::tier_islands::BAND_RATIO_ADVISORY_BOUND`].
    pub band_advisories: Option<Vec<TierBandAdvisory>>,
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

        // Pinned bottom_z, derived once for the whole ladder. Unified's
        // `DepthSemantics::None` makes an Auto `bottom_z` resolve to the
        // STOCK TOP (the Wave-D1 mechanism, `DroppedBandFinding`), which
        // clips the very-steep band's entire Z range away — measured live on
        // wanaka: 702 mm² of steep walls left at full stock across two
        // regions. Hand-built wanaka tiers pin bottom_z manually just below
        // the mesh floor; the planner does the same, from the same
        // setup-frame mesh the tier map walks. `validate_multitool_spec`
        // already guaranteed the mesh exists.
        let plan_mesh = self.plan_mesh(spec.model_id, spec.setup_index);
        let heights = match plan_mesh.as_ref() {
            Some(mesh) => HeightsConfig {
                bottom_z: crate::compute::config::HeightMode::Manual(mesh.bbox.min.z - 0.2),
                ..HeightsConfig::default()
            },
            None => HeightsConfig::default(),
        };

        // Read off a map somebody already paid for, or say nothing. Building
        // one here would break this call's cheapness contract.
        let band_advisories = plan_mesh
            .as_ref()
            .and_then(|mesh| Self::peek_band_advisories(mesh, &refs, &cusp_radii, spec));

        // Q1: the bbox the Suggest funnel reads for the runtime-sanity
        // stepover back-off. One read for the whole ladder — every tier
        // machines `spec.model_id`.
        let model_bbox = self.model_bbox(spec.model_id);

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
            let strategy = spec
                .tier_strategies
                .get(tier_usize)
                .copied()
                .unwrap_or_default();
            let mut operation = plan_tier_operation(tier, cusp, spec, strategy);
            let feeds_provenance =
                self.suggest_feeds_for(&mut operation, tool, model_bbox.as_ref());
            let strategy_tag = match strategy {
                TierStrategy::UnifiedFinish => "",
                TierStrategy::Scallop => " scallop",
                TierStrategy::IsoScallop => " iso",
            };
            let dressup_op = match strategy {
                TierStrategy::UnifiedFinish => OperationType::UnifiedFinish,
                TierStrategy::Scallop | TierStrategy::IsoScallop => OperationType::Scallop,
            };
            let cfg = ToolpathConfig {
                id: ToolpathId(0),
                name: format!("Finish tier {tier}{strategy_tag} (R{cusp:.1})"),
                enabled: true,
                dressups: DressupConfig::for_op(dressup_op),
                heights: heights.clone(),
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
                debug_options: crate::trace::debug_trace::ToolpathDebugOptions::default(),
                operation,
                feeds_provenance,
                planner_origin: Some(PlannerOrigin {
                    plan_id,
                    tier,
                    tier_count,
                }),
            };
            // The `pub(crate)` half. The public `add_toolpath` reports
            // `Effects`, and this planner answers with its own outcome
            // record; the two halves append the same toolpath.
            let index = self.add_toolpath_impl(spec.setup_index, cfg)?;
            let Some(added) = self.toolpath_configs.get(index) else {
                return Err(SessionError::ToolpathNotFound(index));
            };
            toolpath_ids.push(added.id);
        }

        Ok(MultitoolPlanOutcome {
            plan_id,
            toolpath_ids,
            replaced,
            band_advisories,
        })
    }

    /// G-OVERLAPFILL advisories off an ALREADY CACHED tier map, or `None`.
    ///
    /// Never walks a map: [`crate::maps::tier_map_cache::peek_tier_map`] is a
    /// lookup. On a hit it does re-run the island morphology (an O(cells)
    /// pass, the same one a dial-only re-preview pays), which is what buys
    /// the areas; on a miss it costs a hash and reports "not measured".
    fn peek_band_advisories(
        mesh: &Arc<TriangleMesh>,
        refs: &[&dyn MillingCutter],
        cusp_radii: &[f64],
        spec: &MultitoolPlanSpec,
    ) -> Option<Vec<TierBandAdvisory>> {
        let ladder = TierLadder::new(refs).ok()?;
        let params = TierMapParams {
            cell_mm: spec.cell_mm,
            tolerance_mm: spec.tolerance_mm,
            margin_mm: spec.margin_mm,
            treatment: spec.treatment,
        };
        let map = crate::maps::tier_map_cache::peek_tier_map(mesh, &ladder, &params)?;
        let islands = extract_tier_islands(&map, &spec.islands, cusp_radii).ok()?;
        Some(islands.band_advisories().collect())
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
    /// setup-transformed mesh from [`crate::maps::geom_cache::cached_transform`] on a
    /// non-identity setup, the model's own mesh otherwise, and
    /// [`crate::maps::geom_cache::cached_auto_index`] over whichever it is. Sharing
    /// the memo is what makes the preview's map and the emitted ops' maps the
    /// SAME cached object rather than two walks that happen to agree: the
    /// tier-map memo keys on mesh identity, so a preview built off a private
    /// copy of the mesh would miss on every generate.
    /// The model's mesh in `setup_index`'s emission frame — the SAME cached
    /// object generation resolves against. No spatial index: emission-time
    /// callers (pinned heights) need only the bbox, and a cold index build
    /// on a large mesh would make the "planning is CHEAP" contract a lie.
    fn plan_mesh(&self, model_id: usize, setup_index: usize) -> Option<Arc<TriangleMesh>> {
        let mut mesh = self
            .models
            .iter()
            .find(|m| m.id == model_id)
            .and_then(|m| m.mesh.clone())?;
        let ctx = SetupEvalContext::build_for_setup(self, self.setups.get(setup_index));
        if ctx.needs_transform() {
            mesh = crate::maps::geom_cache::cached_transform(
                &mesh,
                &self.setup_transform_info(ctx.face_up, ctx.z_rotation),
            );
        }
        Some(mesh)
    }

    fn plan_geometry(
        &self,
        model_id: usize,
        setup_index: usize,
    ) -> (Option<Arc<TriangleMesh>>, Option<Arc<SpatialIndex>>) {
        let Some(mesh) = self.plan_mesh(model_id, setup_index) else {
            return (None, None);
        };
        let index = crate::maps::geom_cache::cached_auto_index(&mesh);
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
    /// ([`crate::maps::tier_map_cache`]), so a second preview that changes only the
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
        let handle = self.capture_preview_tier_map(spec)?;
        execute_preview_tier_map(&handle, cancel)
    }

    /// Capture the `preview_tier_map` job — step (i).
    ///
    /// It validates the spec, resolves the setup-frame mesh and its index,
    /// and resolves the ladder's tools. Every refusal a ladder can raise
    /// therefore appears at SUBMIT time, before the grid walk is paid for.
    ///
    /// It takes `&self` and writes nothing. The preview is the operator's
    /// veto surface, and a veto that cost something would not be a veto.
    ///
    /// # Errors
    ///
    /// Everything `validate_multitool_spec` refuses, plus
    /// [`SessionError::InvalidParam`] when a ladder tool id names no tool
    /// in the project.
    pub(crate) fn capture_preview_tier_map(
        &self,
        spec: &MultitoolPlanSpec,
    ) -> Result<PreviewTierMapHandle, SessionError> {
        self.validate_multitool_spec(spec)?;
        let (mesh, index) = self.plan_geometry(spec.model_id, spec.setup_index);
        let tools = self
            .resolve_ladder_tools(&spec.tool_ids)
            .map_err(|e| annotate(PREVIEW_OP_NAME, &e))?;
        Ok(PreviewTierMapHandle {
            mesh,
            index,
            tools,
            spec: spec.clone(),
        })
    }

    /// Resolve the RAW machining polygons for a toolpath whose enabled
    /// boundary is [`BoundarySource::PlannedTierRegions`] — the GUI worker
    /// path's twin of the arm `resolve_generation_inputs` runs in
    /// `session/compute.rs`. `Ok(None)` when the toolpath's boundary is
    /// disabled or any other source; `Ok(Some(vec![]))` when the tier is
    /// legitimately EMPTY (the coarse tool holds the whole board — the op
    /// must then generate nothing, **never** fall back to an unconfined
    /// board; G-TIERWORKER, operator-observed 2026-08-27: the worker path
    /// lacked this resolution entirely and a fine tier re-finished the
    /// flats full-board).
    ///
    /// RAW means keep-outs and the user boundary offset are NOT applied —
    /// the caller's clip pipeline applies them, exactly as it does for the
    /// polygons a `DerivedRestRegions` source supplies.
    ///
    /// One pipeline: this goes through the same [`Self::resolve_tier_plan`]
    /// the core generation path and the preview use, so the GUI worker, the
    /// CLI and the operator's veto all describe the same islands.
    ///
    /// **Test door.** The harnesses under `crates/rs_cam_core/tests` are the
    /// only callers. No production path reads it.
    pub fn planned_tier_boundary_polys(
        &self,
        toolpath_id: ToolpathId,
        cancel: &AtomicBool,
    ) -> Result<Option<Vec<Polygon2>>, SessionError> {
        let Some((_, tc)) = self.find_toolpath_config_by_id(toolpath_id) else {
            return Err(SessionError::OperationFailed(format!(
                "no toolpath with id {} while resolving its planned-tier boundary",
                toolpath_id.0
            )));
        };
        if !tc.boundary.enabled {
            return Ok(None);
        }
        let crate::compute::config::BoundarySource::PlannedTierRegions {
            ref tool_ids,
            tier,
            cell_mm,
            tolerance_mm,
            margin_mm,
            treatment,
            islands,
        } = tc.boundary.source
        else {
            return Ok(None);
        };
        let op_name = tc.name.clone();
        let Some(setup_index) = self.setup_of_toolpath_id(toolpath_id) else {
            return Err(SessionError::OperationFailed(format!(
                "'{op_name}': toolpath belongs to no setup"
            )));
        };
        let (mesh, index) = self.plan_geometry(tc.model_id, setup_index);
        let recipe = PlannedTierRecipe {
            tool_ids,
            tier,
            cell_mm,
            tolerance_mm,
            margin_mm,
            treatment,
            islands,
        };
        self.resolve_planned_tier_region_polys(
            &op_name,
            mesh.as_ref(),
            index.as_ref(),
            &recipe,
            cancel,
        )
        .map(Some)
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
        model_bbox: Option<&crate::geo::BoundingBox3>,
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
                // Q1: the bbox the runtime-sanity back-off reads.
                // `restore_planned_geometry` below re-applies the
                // planner's equal-cusp stepover, so the back-off cannot
                // move the tier's stepover; it stays populated so the
                // funnel sees the same context every other surface does.
                // `upstream_leftover_stock_mm` stays `None`: no lookup
                // here gives it, and v1 does not read it.
                context: crate::feeds::suggest::SuggestContext {
                    model_bbox,
                    ..crate::feeds::suggest::SuggestContext::default()
                },
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
/// dials the planner owns (equal-cusp `scallop_height` / `raster_stepover` /
/// `z_step`, a zeroed `stock_to_leave`, the tier link cap, and C2's
/// `monotone_cell_decomposition`).
///
/// A free function rather than a method because it reads nothing from the
/// session — the tier's index, cusp radius and the spec are the whole input,
/// which is what lets a UI pre-fill show the same numbers the plan will emit.
///
/// `tier` is the ladder index. It selects the spiral mode: see
/// [`tier_is_per_island`] and the `continuous` comment below.
fn plan_tier_operation(
    tier: u8,
    cusp_radius_mm: f64,
    spec: &MultitoolPlanSpec,
    strategy: TierStrategy,
) -> OperationConfig {
    let stepover = equal_cusp_stepover_mm(cusp_radius_mm, spec.cusp_height_mm);
    match strategy {
        TierStrategy::Scallop | TierStrategy::IsoScallop => {
            let defaults = crate::compute::operation_configs::ScallopConfig::default();
            return OperationConfig::Scallop(crate::compute::operation_configs::ScallopConfig {
                // Equal cusp across tiers, same as the unified arm below.
                scallop_height: spec.cusp_height_mm,
                // G-TIERCONTINUOUS (2026-09-09). One continuous spiral is the
                // measured win on ONE region (S1: one entry plunge, metres of
                // rapids instead of kilometres). On a PER-ISLAND tier it is
                // the opposite: `scallop.rs` makes a ring-to-ring connector a
                // cutting feed only when the hop is inside the widest ring
                // spacing, and a dendritic island breaks that bound on most
                // junctions, so the connector falls back to
                // retract/rapid/replunge; the intra-pass hookup relink that
                // would convert those junctions back into surface links is
                // SKIPPED under `continuous`. Measured live on the wanaka
                // board (`planning/island_clip_2026-09-09/SPEC.md` §5, T3 vs
                // T3b): 929 → 613 retracts over 484 rings (2.04 → 1.02 per
                // ring), pair time −15 %, entry_load CRITICAL → caution.
                //
                // `intra_pass_hookup_mm` is deliberately NOT set here: the
                // type default is already 3.0 mm, the value the T3b run
                // measured, and `..defaults` carries it. Raising it to 6.0
                // (T3c) joined only 46 more ring pairs.
                continuous: !tier_is_per_island(tier, spec),
                slope_from: 0.0,
                slope_to: 90.0,
                stock_to_leave: 0.0,
                iso_field: matches!(strategy, TierStrategy::IsoScallop),
                ..defaults
            });
        }
        TierStrategy::UnifiedFinish => {}
    }
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
        // The very-steep band's contour spacing IS its cusp spacing: on a
        // near-vertical wall the ball's cusps stack vertically, so the same
        // equal-cusp law sizes `z_step`. The type default (1.0 mm) is a
        // generic dial — against a 10–30 µm cusp everywhere else it reads
        // as the steep walls being abandoned (operator-observed 2026-08-27).
        z_step: if stepover > 0.0 {
            stepover
        } else {
            defaults.z_step
        },
        // Held EQUAL across tiers (T2 §7): any non-zero value prints a
        // `stock_to_leave · Δcos θ` step at a tier seam that crosses a slope
        // change.
        stock_to_leave: 0.0,
        // A generous link-permission cap for planner tiers. The 6 mm type
        // default was tuned for compact regions; on dendritic islands the
        // finger-to-finger hops routinely exceed it, and every candidate is
        // still gouge-checked against standing stock AND integrator-costed
        // against the retract it replaces, so a longer candidate only ever
        // wins when it is actually faster — the cap is permission, not
        // safety. (Measured 2026-08-27: the confined wanaka tier 1 kept
        // 1,263 intra-node retracts under the 6 mm cap even after
        // G-LINKVETO.)
        intra_region_hookup_mm: 25.0,
        // C2: a plan-level choice, carried onto every tier. Default `false`,
        // which is `defaults`' own value — set explicitly rather than left
        // to `..defaults` so the planner's answer is visible here and so
        // `restore_planned_geometry` has something to restore.
        monotone_cell_decomposition: spec.monotone_cell_decomposition,
        ..defaults
    })
}

/// Re-apply the planner-owned geometry dials after the Suggest funnel has
/// rewritten the operation. See [`ProjectSession::suggest_feeds_for`].
fn restore_planned_geometry(operation: &mut OperationConfig, planned: &OperationConfig) {
    match (&mut *operation, planned) {
        (OperationConfig::UnifiedFinish(out), OperationConfig::UnifiedFinish(want)) => {
            out.scallop_height = want.scallop_height;
            out.raster_stepover = want.raster_stepover;
            out.stock_to_leave = want.stock_to_leave;
            // C2: the Suggest funnel rebuilds the whole operation from the
            // config type's defaults, so a planner-set `true` would be
            // silently clobbered back to `false` without this line.
            out.monotone_cell_decomposition = want.monotone_cell_decomposition;
        }
        (OperationConfig::Scallop(out), OperationConfig::Scallop(want)) => {
            // The same clobber class for a scallop tier: the funnel
            // rebuilds from type defaults, which would drop the planner's
            // cusp target, the continuous spiral and the iso-field choice.
            out.scallop_height = want.scallop_height;
            out.stock_to_leave = want.stock_to_leave;
            out.continuous = want.continuous;
            // The spiral mode and the relink cap are ONE decision
            // (G-TIERCONTINUOUS): `continuous: false` is only a win because
            // the hookup relink then runs. Restoring one without the other
            // is how the pair drifts.
            out.intra_pass_hookup_mm = want.intra_pass_hookup_mm;
            out.iso_field = want.iso_field;
            out.slope_from = want.slope_from;
            out.slope_to = want.slope_to;
        }
        _ => {}
    }
}

/// Does this tier machine a set of ISLANDS rather than the whole board?
///
/// One predicate, two consumers: [`tier_boundary`] gives such a tier a
/// [`BoundarySource::PlannedTierRegions`] boundary, and
/// [`plan_tier_operation`] turns the continuous spiral OFF on it
/// (G-TIERCONTINUOUS). The two must not drift — a per-island boundary with a
/// whole-board spiral mode is exactly the defect the ledger row records.
///
/// Every fine tier (index ≥ 1) is per-island. Tier 0 is the coarse tool's
/// complement and sweeps the whole board, unless
/// [`MultitoolPlanSpec::coarse_skips_fine_islands`] hands it the complement
/// of the fine islands, which is an island set like any other.
const fn tier_is_per_island(tier: u8, spec: &MultitoolPlanSpec) -> bool {
    tier != 0 || spec.coarse_skips_fine_islands
}

/// The boundary one tier carries.
///
/// Tier 0 gets the type default (disabled) unless
/// [`MultitoolPlanSpec::coarse_skips_fine_islands`] is on: its cusp target
/// holds everywhere by construction, so by default the coarse tool sweeps
/// the whole board as one pass. With the skip dial on, tier 0 carries the
/// SAME variant with `tier: 0`, which the resolver reads as the COMPLEMENT
/// of every fine tier's owned islands (see
/// `resolve_planned_tier_region_polys`).
fn tier_boundary(tier: u8, ordered_ids: &[usize], spec: &MultitoolPlanSpec) -> BoundaryConfig {
    if !tier_is_per_island(tier, spec) {
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
        let tools = self
            .resolve_ladder_tools(recipe.tool_ids)
            .map_err(|e| annotate(op_name, &e))?;
        resolve_tier_plan_with_tools(op_name, geometry, tools, recipe, require_fine_tier, cancel)
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
    /// For a FINE tier (≥ 1), returns the tier's `machining` set — the
    /// islands grown by `overlap_mm` — not `owned`. The blend band is the
    /// point: the fine tool's first pass must land on ground the coarse
    /// tool already cut. For `tier: 0`, returns the COMPLEMENT of every
    /// fine tier's `owned` set (the coarse-skips-fine-islands arm — see the
    /// inline comment).
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
        // `tier: 0` is the COMPLEMENT arm (coarse-skips-fine-islands,
        // operator-requested 2026-08-27): everything except the fine tiers'
        // `owned` islands — including NO_TIER cells, whose steep walls only
        // tier 0's waterline band ever touches. Complement of `owned`, not
        // of `machining`: the fine tiers cut `owned + overlap`, so the
        // overlap band is machined by BOTH sides and the seam still blends.
        // Holes ride the polygons natively (`region_polygons_from_mask`
        // groups loops by even-odd depth), which is what makes "a board
        // with nine island-shaped holes" one honest region set.
        if recipe.tier == 0 {
            let resolved = self.resolve_tier_plan(
                op_name,
                TierPlanGeometry { mesh, index },
                recipe,
                false,
                cancel,
            )?;
            let map = resolved.map.as_ref();
            let total = map.nx.saturating_mul(map.ny);
            let mut complement = vec![true; total];
            for set in &resolved.islands.per_tier {
                for (i, &owned) in set.owned_mask.iter().enumerate() {
                    if owned && let Some(cell) = complement.get_mut(i) {
                        *cell = false;
                    }
                }
            }
            // Clear the border ring: marching squares treats cells as
            // CORNERS, so a mask that is true along the grid border has no
            // closable outer ring — the tracer then emits only the island
            // hole-loops, and even-odd grouping returns them as depth-0
            // polygons: the exact INVERSE of this arm's answer (measured on
            // the bowl fixture: one 5 mm² polygon containing the origin).
            // The border cells sit inside the `margin_mm` band past the
            // mesh, so the territory lost is off-part; at `cell_mm >
            // margin_mm` the bite reaches at most one cell of real edge,
            // below the coarse tool's own cusp scale.
            if map.nx > 0 && map.ny > 0 {
                for (i, cell) in complement.iter_mut().enumerate() {
                    let (r, c) = (i / map.nx, i % map.nx);
                    if r == 0 || r == map.ny - 1 || c == 0 || c == map.nx - 1 {
                        *cell = false;
                    }
                }
            }
            let grid = crate::geometry::grid2::Grid2::from_vec(map.nx, map.ny, complement)
                .map_err(|e| {
                    SessionError::OperationFailed(format!(
                        "'{op_name}': complement mask shape mismatch — {e}"
                    ))
                })?;
            return Ok(crate::geometry::region_mask::region_polygons_from_mask(
                &grid,
                map.origin_x,
                map.origin_y,
                map.cell_mm,
                0.0,
            ));
        }
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

/// Ladder → memoised tier map → per-tier islands, over an ALREADY resolved
/// ladder.
///
/// The session half of `ProjectSession::resolve_tier_plan` is the tool
/// lookup alone. Everything below it reads the ladder, the geometry and the
/// recipe, so it is a free function and the `preview_tier_map` job's step
/// (ii) calls it holding no session.
fn resolve_tier_plan_with_tools(
    op_name: &str,
    geometry: TierPlanGeometry<'_>,
    tools: Vec<ToolConfig>,
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

    let cutters: Vec<ToolDefinition> = tools.iter().map(build_cutter).collect();
    let refs: Vec<&dyn MillingCutter> = cutters.iter().map(|c| c as &dyn MillingCutter).collect();
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
    let map = crate::maps::tier_map_cache::cached_tier_map(
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

/// The name every tier-map preview refusal carries, on both doors.
const PREVIEW_OP_NAME: &str = "multi-tool preview";

/// What [`ProjectSession::start`] captured for one `preview_tier_map` job.
///
/// The handle is the whole input of step (ii). It owns the ladder, the
/// setup-frame mesh and its spatial index, so
/// [`execute_preview_tier_map`] reads no session and runs off the frame
/// loop.
///
/// The mesh and the index ride as `Arc`s taken from the geometry memo, not
/// as copies. The tier-map memo keys on mesh IDENTITY, so a handle that
/// carried a private copy would miss the cache on every later generate —
/// the exact property `plan_mesh` exists to hold.
///
/// Every field is private. A handle is the evidence of one submit, so no
/// caller outside this module builds one.
pub struct PreviewTierMapHandle {
    /// The model's mesh in the setup's emission frame. `None` when the
    /// model carries no 3D mesh; step (ii) then refuses.
    mesh: Option<Arc<TriangleMesh>>,
    /// The spatial index over that mesh.
    index: Option<Arc<SpatialIndex>>,
    /// The ladder's tools, coarse → fine, as `resolve_ladder_tools`
    /// ordered them at capture. Nothing below re-sorts them.
    tools: Vec<ToolConfig>,
    /// The dials the operator previewed.
    spec: MultitoolPlanSpec,
}

impl PreviewTierMapHandle {
    /// How many tools take part in the ladder.
    #[must_use]
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    /// The dials the operator previewed, as the submit step read them.
    ///
    /// A READ. It hands out no way to build a handle, which is what the
    /// private fields protect. WP14b added it because the GUI now submits
    /// this row rather than a request carrying the spec, and the dialog's
    /// own sentry reads the submitted dials
    /// (`the_dialogs_dials_reach_the_submitted_spec`).
    #[must_use]
    pub fn spec(&self) -> &MultitoolPlanSpec {
        &self.spec
    }
}

/// Walk the tier map from a handle — step (ii) of the `preview_tier_map`
/// job.
///
/// **This function holds no session.** It is a free function over
/// `&`[`PreviewTierMapHandle`], so it coerces to a plain `fn` pointer and
/// cannot capture a `&ProjectSession`. That is what lets a caller run it
/// off the frame loop while the session stays usable.
///
/// The walk is the expensive step: a full-grid drop-cutter residual, about
/// 8 s at 0.6 mm and 31 s at 0.3 mm per ladder tool on the reference board.
/// `cancel` reaches it at grid-row granularity.
///
/// The job writes nothing. The preview is a read, so there is no step
/// (iii): the answer goes to the caller and the session is untouched.
///
/// # Errors
///
/// [`SessionError::MissingGeometry`] when the model carried no mesh,
/// [`SessionError::InvalidParam`] when the ladder is not usable, and
/// [`SessionError::OperationFailed`] when the walk is cancelled or the
/// island extraction refuses.
pub fn execute_preview_tier_map(
    handle: &PreviewTierMapHandle,
    cancel: &AtomicBool,
) -> Result<MultitoolPreview, SessionError> {
    let spec = &handle.spec;
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
    let resolved = resolve_tier_plan_with_tools(
        PREVIEW_OP_NAME,
        TierPlanGeometry {
            mesh: handle.mesh.as_ref(),
            index: handle.index.as_ref(),
        },
        handle.tools.clone(),
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

    /// The skip dial flips ONLY tier 0's boundary — from the type default
    /// (sweep everything) to the same variant with `tier: 0`, which the
    /// resolver reads as the complement of the fine tiers' owned islands.
    #[test]
    fn the_skip_dial_gives_tier_zero_an_inverted_boundary() {
        let spec = MultitoolPlanSpec {
            tool_ids: vec![0, 1],
            coarse_skips_fine_islands: true,
            ..MultitoolPlanSpec::default()
        };
        let coarse = tier_boundary(0, &[0, 1], &spec);
        assert!(coarse.enabled);
        match coarse.source {
            BoundarySource::PlannedTierRegions {
                ref tool_ids, tier, ..
            } => {
                assert_eq!(tool_ids, &vec![0, 1]);
                assert_eq!(tier, 0, "tier 0 IS the complement arm's address");
            }
            ref other => panic!("expected PlannedTierRegions, got {other:?}"),
        }
        // The fine tier is unchanged by the dial.
        let fine = tier_boundary(1, &[0, 1], &spec);
        assert!(fine.enabled);
        assert!(matches!(
            fine.source,
            BoundarySource::PlannedTierRegions { tier: 1, .. }
        ));
    }

    /// The very-steep band's contour spacing derives from the same
    /// equal-cusp law as the stepover — the generic 1.0 mm z_step default
    /// against a 10–30 µm cusp elsewhere reads as the steep walls being
    /// abandoned (operator-observed 2026-08-27).
    #[test]
    fn the_planner_sizes_z_step_by_the_equal_cusp_law() {
        let spec = MultitoolPlanSpec::default();
        let op = plan_tier_operation(1, 2.0, &spec, TierStrategy::UnifiedFinish);
        let OperationConfig::UnifiedFinish(cfg) = op else {
            panic!("planner emits unified_finish");
        };
        let expected = equal_cusp_stepover_mm(2.0, spec.cusp_height_mm);
        assert!((cfg.z_step - expected).abs() < 1e-12);
        assert!((cfg.raster_stepover - expected).abs() < 1e-12);
    }

    /// The operator's separation (2026-09-03): a tier strategy changes the
    /// OPERATION, never the territory. A scallop tier carries the planner's
    /// cusp target, the spiral mode for its territory, and the iso choice; a
    /// missing strategy entry is the historical unified planner.
    #[test]
    fn tier_strategy_picks_the_operation_and_keeps_the_planner_dials() {
        let spec = MultitoolPlanSpec::default();
        let op = plan_tier_operation(1, 2.0, &spec, TierStrategy::IsoScallop);
        let OperationConfig::Scallop(cfg) = op else {
            panic!("iso strategy emits a scallop op");
        };
        assert!((cfg.scallop_height - spec.cusp_height_mm).abs() < 1e-12);
        assert!(cfg.iso_field, "iso strategy sets the iso_field dial");
        assert!((cfg.stock_to_leave).abs() < 1e-12);

        let op = plan_tier_operation(1, 2.0, &spec, TierStrategy::Scallop);
        let OperationConfig::Scallop(cfg) = op else {
            panic!("scallop strategy emits a scallop op");
        };
        assert!(!cfg.iso_field, "plain scallop keeps the cascade");

        // Absent entries default to unified — byte-identical legacy plans.
        assert_eq!(
            spec.tier_strategies.first().copied().unwrap_or_default(),
            TierStrategy::UnifiedFinish
        );
    }

    /// G-TIERCONTINUOUS sentry. The spiral mode follows the TERRITORY, and
    /// the predicate that decides the territory is the one that decides the
    /// mode. A per-island tier gets `continuous: false` so the intra-pass
    /// hookup relink runs; a whole-board tier keeps the S1 spiral.
    ///
    /// Measured on wanaka (`planning/island_clip_2026-09-09/SPEC.md` §5):
    /// 929 → 613 retracts, pair time −15 %.
    #[test]
    fn a_per_island_scallop_tier_is_not_a_continuous_spiral() {
        let spec = MultitoolPlanSpec::default();
        assert!(
            !spec.coarse_skips_fine_islands,
            "this fixture reads the default dial"
        );

        for strategy in [TierStrategy::Scallop, TierStrategy::IsoScallop] {
            // Tier 0 without the skip dial sweeps the whole board: the S1
            // spiral is the measured win there and is unchanged.
            let op = plan_tier_operation(0, 2.0, &spec, strategy);
            let OperationConfig::Scallop(cfg) = op else {
                panic!("a scallop strategy emits a scallop op");
            };
            assert!(
                cfg.continuous,
                "the whole-board tier keeps the S1 continuous spiral"
            );
            assert!(
                !tier_boundary(0, &[0, 1], &spec).enabled,
                "the same tier carries no island boundary"
            );

            // Every fine tier machines islands.
            let op = plan_tier_operation(1, 2.0, &spec, strategy);
            let OperationConfig::Scallop(cfg) = op else {
                panic!("a scallop strategy emits a scallop op");
            };
            assert!(
                !cfg.continuous,
                "a per-island tier must leave the relink enabled"
            );
            assert!(
                cfg.intra_pass_hookup_mm >= 3.0,
                "the relink cap must be at least the measured 3.0 mm, got {}",
                cfg.intra_pass_hookup_mm
            );
            assert!(
                tier_boundary(1, &[0, 1], &spec).enabled,
                "the same tier carries an island boundary"
            );
        }

        // With the skip dial on, tier 0 machines the complement of the fine
        // islands — an island set like any other, so the mode follows it.
        let skipping = MultitoolPlanSpec {
            coarse_skips_fine_islands: true,
            ..MultitoolPlanSpec::default()
        };
        let op = plan_tier_operation(0, 2.0, &skipping, TierStrategy::Scallop);
        let OperationConfig::Scallop(cfg) = op else {
            panic!("a scallop strategy emits a scallop op");
        };
        assert!(
            !cfg.continuous,
            "coarse_skips_fine_islands makes tier 0 per-island too"
        );
        assert!(tier_boundary(0, &[0, 1], &skipping).enabled);
    }

    /// Suggest rebuilds the operation from the config type's defaults, so
    /// every planner-owned dial has to be restored afterwards. `continuous`
    /// and `intra_pass_hookup_mm` are ONE decision; restoring one without the
    /// other is how the pair drifts.
    #[test]
    fn the_geometry_restore_carries_the_spiral_mode_and_its_relink_cap() {
        let planned =
            plan_tier_operation(1, 2.0, &MultitoolPlanSpec::default(), TierStrategy::Scallop);
        // What the funnel would hand back: the type's own defaults.
        let mut rebuilt =
            OperationConfig::Scallop(crate::compute::operation_configs::ScallopConfig {
                continuous: true,
                intra_pass_hookup_mm: 0.0,
                ..Default::default()
            });
        restore_planned_geometry(&mut rebuilt, &planned);
        let OperationConfig::Scallop(cfg) = rebuilt else {
            panic!("the restore does not change the variant");
        };
        assert!(!cfg.continuous);
        assert!(cfg.intra_pass_hookup_mm >= 3.0);
    }
}
