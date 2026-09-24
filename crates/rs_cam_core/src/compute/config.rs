use serde::{Deserialize, Serialize};

/// Where the toolpath's stock material comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StockSource {
    /// Start from raw stock (default).
    #[default]
    Fresh,
    /// Simulate all prior enabled toolpaths to determine starting material.
    FromRemainingStock,
}

// Canonical definition now lives in `crate::ids` (R3 — id/index split);
// re-exported here so the long-standing `compute::config::ToolpathId`
// path keeps working.
pub use crate::ids::ToolpathId;

/// Why a toolpath cannot generate *yet* — and which operation it is waiting
/// for. A/M11.
///
/// This is a **sequencing** state, not a failure. An op with
/// `StockSource::FromRemainingStock` needs the simulated stock of the ops
/// before it, and that snapshot only exists after a *simulation*. So a rest op
/// can never see stock produced by an op generated earlier in the same
/// `generate_all` pass — measured on wanaka 2026-07-30, where `Rivers`
/// generated fine and its own successor `Lakes` failed in the same pass.
/// Reporting that as `Error` made "cannot yet" indistinguishable from "cannot
/// ever", and left the operator with no way to tell a one-round wait from a
/// four-round one.
/// W5 item (a): the struct SERIALISES, so every surface that reports a block
/// renders one shape. Six sites used to hand-build the object from a
/// `serde_json::json!` literal, and one of them dropped `message`. Nothing
/// reads the shape back, so `Deserialize` is not derived.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AwaitingPriorStock {
    /// The upstream operation whose simulated stock is missing. `None` only
    /// when this op is first in its setup and there is genuinely nothing
    /// upstream to name — a real configuration error, spelled out in
    /// `message`.
    pub blocking_toolpath_id: Option<ToolpathId>,
    /// That operation's 0-based index at the time the block was recorded.
    /// Advisory: indices shift when toolpaths are added or removed, so
    /// resolve by `blocking_toolpath_id` when it matters.
    pub blocking_toolpath_index: Option<usize>,
    /// The operator-facing sentence. Always names the blocker when there is
    /// one. Shape pinned by a sentry so it cannot silently regress to the
    /// pre-A/M11 text, which named no operation and did not say the cycle may
    /// need repeating.
    pub message: String,
}

/// Where a toolpath stands. Consumed identically by the GUI badge, the MCP
/// `list_toolpaths` / diagnostics surface, and the generate_all reporter —
/// there is deliberately no second taxonomy.
#[derive(Debug, Clone)]
pub enum ComputeStatus {
    Pending,
    Computing,
    Done,
    /// A/M11 — blocked on sequencing, not failed. See [`AwaitingPriorStock`].
    AwaitingPriorStock(AwaitingPriorStock),
    /// The operation is switched off (`enabled: false`).
    ///
    /// **Never stored.** It is produced only by [`ComputeStatus::effective`],
    /// which lets `enabled: false` win over whatever the op last recorded.
    /// Storing it would mean deciding what to restore on re-enable; deriving
    /// it means a disabled op can never carry a live-looking error — the
    /// A/M11 defect where `3D Finish 6` read as broken when it was merely off.
    Disabled,
    /// A genuine generation failure. Something is wrong with the operation,
    /// its inputs, or its geometry; no amount of re-running will fix it.
    Error(String),
}

impl ComputeStatus {
    /// The status a reader should show, given whether the op is enabled.
    /// Every surface goes through here so GUI, MCP and CLI cannot drift.
    pub fn effective(enabled: bool, raw: &Self) -> &Self {
        static DISABLED: ComputeStatus = ComputeStatus::Disabled;
        if enabled { raw } else { &DISABLED }
    }

    /// Stable machine-readable label. Used by the MCP `status` field and the
    /// GUI tooltips.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::Computing => "Computing",
            Self::Done => "Done",
            Self::AwaitingPriorStock(_) => "AwaitingPriorStock",
            Self::Disabled => "Disabled",
            Self::Error(_) => "Error",
        }
    }

    /// The failure text — and ONLY for genuine failures. A blocked op is not
    /// an error and must not inflate an error list; read it through
    /// [`Self::blocked_on`].
    pub fn error_text(&self) -> Option<&str> {
        match self {
            Self::Error(e) => Some(e.as_str()),
            Self::Pending
            | Self::Computing
            | Self::Done
            | Self::AwaitingPriorStock(_)
            | Self::Disabled => None,
        }
    }

    /// The sequencing block, when there is one.
    pub fn blocked_on(&self) -> Option<&AwaitingPriorStock> {
        match self {
            Self::AwaitingPriorStock(b) => Some(b),
            Self::Pending | Self::Computing | Self::Done | Self::Disabled | Self::Error(_) => None,
        }
    }

    /// Any message worth showing the operator, failure or block.
    pub fn detail(&self) -> Option<&str> {
        match self {
            Self::Error(e) => Some(e.as_str()),
            Self::AwaitingPriorStock(b) => Some(b.message.as_str()),
            Self::Pending | Self::Computing | Self::Done | Self::Disabled => None,
        }
    }

    /// `true` when this op still needs a generate to reach `Done`. A blocked
    /// op counts — it will generate once its upstream stock exists.
    pub fn needs_generation(&self) -> bool {
        match self {
            Self::Pending | Self::AwaitingPriorStock(_) => true,
            Self::Computing | Self::Done | Self::Disabled | Self::Error(_) => false,
        }
    }
}

/// Minimum clearance (mm) between `safe_z` and the top of the stock.
///
/// Rapids at `safe_z` must clear the uncut stock surface. The user-configured
/// `post.safe_z` is a legacy 2D-friendly default (often 10mm above work Z=0),
/// which sits *inside* the stock for any 3D job with stock extending above
/// that value. This clearance forces `safe_z` to at least `stock_top + 5mm`.
pub const SAFE_Z_CLEARANCE_MM: f64 = 5.0;

/// Apply the stock-clearance floor to a raw `post.safe_z` value.
///
/// Returns `max(raw_safe_z, stock_top + SAFE_Z_CLEARANCE_MM)`. Use this at the
/// single point where a [`HeightContext`] is built so that every operation and
/// downstream helper sees the same effective safe_z.
pub fn effective_safe_z(raw_safe_z: f64, stock_top: f64) -> f64 {
    raw_safe_z.max(stock_top + SAFE_Z_CLEARANCE_MM)
}

/// Named reference point for expressing heights relative to geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeightReference {
    StockTop,
    StockBottom,
    ModelTop,
    ModelBottom,
}

impl HeightReference {
    pub const ALL: &[HeightReference] = &[
        HeightReference::StockTop,
        HeightReference::StockBottom,
        HeightReference::ModelTop,
        HeightReference::ModelBottom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            HeightReference::StockTop => "Stock Top",
            HeightReference::StockBottom => "Stock Bottom",
            HeightReference::ModelTop => "Model Top",
            HeightReference::ModelBottom => "Model Bottom",
        }
    }

    /// Resolve the reference Z value from context. Model refs fall back to stock.
    pub fn resolve_z(self, ctx: &HeightContext) -> f64 {
        match self {
            HeightReference::StockTop => ctx.stock_top_z,
            HeightReference::StockBottom => ctx.stock_bottom_z,
            HeightReference::ModelTop => ctx.model_top_z.unwrap_or(ctx.stock_top_z),
            HeightReference::ModelBottom => ctx.model_bottom_z.unwrap_or(ctx.stock_bottom_z),
        }
    }
}

/// An offset from a named reference point.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ReferenceOffset {
    pub reference: HeightReference,
    pub offset: f64,
}

/// Context needed to resolve heights -- stock/model geometry and post config.
#[derive(Debug, Clone, Copy)]
pub struct HeightContext {
    pub safe_z: f64,
    pub op_depth: f64,
    pub stock_top_z: f64,
    pub stock_bottom_z: f64,
    pub model_top_z: Option<f64>,
    pub model_bottom_z: Option<f64>,
}

impl HeightContext {
    /// Minimal context for tests / simple cases where only safe_z and op_depth matter.
    /// Stock spans 0 -> safe_z, no model.
    pub fn simple(safe_z: f64, op_depth: f64) -> Self {
        Self {
            safe_z,
            op_depth,
            stock_top_z: 0.0,
            stock_bottom_z: -op_depth,
            model_top_z: None,
            model_bottom_z: None,
        }
    }
}

/// Controls whether a height value is auto-computed, manually set, or relative to a reference.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", content = "value", rename_all = "snake_case")]
pub enum HeightMode {
    /// Auto-compute from stock/operation context.
    Auto,
    /// User-specified absolute Z value.
    Manual(f64),
    /// Offset from a named reference point (e.g. "5 mm from Stock Top").
    FromReference(ReferenceOffset),
}

impl HeightMode {
    /// Resolve to a concrete Z value given auto default and context.
    pub fn resolve_value(&self, auto_value: f64, ctx: &HeightContext) -> f64 {
        match self {
            HeightMode::Auto => auto_value,
            HeightMode::Manual(value) => *value,
            HeightMode::FromReference(r) => r.reference.resolve_z(ctx) + r.offset,
        }
    }

    pub fn is_auto(&self) -> bool {
        matches!(self, HeightMode::Auto)
    }
}

/// Five-level height system controlling vertical tool motion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeightsConfig {
    pub clearance_z: HeightMode,
    pub retract_z: HeightMode,
    pub feed_z: HeightMode,
    pub top_z: HeightMode,
    pub bottom_z: HeightMode,
}

impl Default for HeightsConfig {
    fn default() -> Self {
        Self {
            clearance_z: HeightMode::Auto,
            retract_z: HeightMode::Auto,
            feed_z: HeightMode::Auto,
            top_z: HeightMode::Auto,
            bottom_z: HeightMode::Auto,
        }
    }
}

impl HeightsConfig {
    /// Resolve all heights given stock/model/post context.
    ///
    /// F-028 (2026-05-25): `top_z` Auto now resolves to `ctx.stock_top_z`
    /// (the stock top in the operation's emission frame) instead of `0.0`.
    /// Pre-fix any project where stock top sat at world Z != 0 emitted 2.5D
    /// cuts at world Z=[-depth, 0] (i.e. below or above the actual stock),
    /// because face / pocket / profile / etc. fed `heights.top_z = 0` into
    /// their depth stepping. With the fix the depth stepping anchors to the
    /// actual stock top in the right frame (`world` for identity setups,
    /// `local` for non-identity setups — the caller passes the right
    /// `stock_top_z` per setup; see `session/compute.rs::compute` for the
    /// identity-vs-non-identity dispatch).
    ///
    /// `bottom_z` Auto remains `-op_depth.abs()` so depth-from-top semantics
    /// keep working for callers that haven't migrated; combining with the
    /// new `top_z` Auto yields `bottom_z = -op_depth` (unchanged when
    /// `stock_top_z == 0`, which is the common test/fixture case).
    ///
    /// R3 (Corne case, 2026-09-18): on a context with NO depth
    /// (`op_depth == 0`, the `DepthSemantics::None` family) the Auto bottom
    /// is the MODEL bottom, floored at the stock bottom. Before this the
    /// zero-depth arithmetic gave `bottom_z = top_z - 0 = stock top`, so an
    /// Auto waterline laddered ONE level at the stock top, above the mesh,
    /// and an Auto `UnifiedFinish` pinned its very-steep band to the rim
    /// (`planning/corne_case_analysis_2026-09-18/ANALYSIS.md` §4.1).
    /// [`Self::auto_bottom_z`] is the rule; the Heights tab displays the
    /// same resolver, so display and generation agree.
    ///
    /// The Auto TOP stays at the stock top for every depth. The session's
    /// entry-descent pass reads `heights.top_z` as the fresh-stock ceiling
    /// (`session/compute.rs`, `optimize_entry_descents_annotated`), so an
    /// Auto top at the model top would rapid a fresh-stock rough into the
    /// overhead above the model.
    pub fn resolve(&self, ctx: &HeightContext) -> ResolvedHeights {
        let retract = self.retract_z.resolve_value(ctx.safe_z, ctx);
        let top_z = self.top_z.resolve_value(ctx.stock_top_z, ctx);
        ResolvedHeights {
            clearance_z: self.clearance_z.resolve_value(retract + 10.0, ctx),
            retract_z: retract,
            feed_z: self.feed_z.resolve_value(retract - 2.0, ctx),
            top_z,
            bottom_z: self
                .bottom_z
                .resolve_value(Self::auto_bottom_z(top_z, ctx), ctx),
            top_pinned: !self.top_z.is_auto(),
            bottom_pinned: !self.bottom_z.is_auto(),
        }
    }

    /// The Auto bottom for a resolved `top_z` (R3).
    ///
    /// * A context WITH a depth keeps the F-028 arithmetic exactly:
    ///   `top_z - op_depth`. The 2.5D family and `cutting_levels` anchor on
    ///   it, and a pinned bottom on those ops reaches no motion
    ///   (`pinned_bottom_z_reaches_motion_g_bottompin`).
    /// * A ZERO-depth context has no dial to subtract, so the floor is the
    ///   model bottom when the context carries a model, else the stock
    ///   bottom; either way never below the stock bottom. `top_z` is not
    ///   read on this arm: a pinned top below the model bottom yields an
    ///   inverted range, which the waterline ladder reports as empty.
    pub fn auto_bottom_z(top_z: f64, ctx: &HeightContext) -> f64 {
        if ctx.op_depth.abs() < ZERO_DEPTH_EPSILON {
            ctx.model_bottom_z
                .map_or(ctx.stock_bottom_z, |model_bottom| {
                    model_bottom.max(ctx.stock_bottom_z)
                })
        } else {
            top_z - ctx.op_depth.abs()
        }
    }
}

/// Below this an `op_depth` counts as "no depth dial" for
/// [`HeightsConfig::auto_bottom_z`].
pub const ZERO_DEPTH_EPSILON: f64 = 1e-9;

/// Fully resolved (concrete) heights for a single operation.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedHeights {
    pub clearance_z: f64,
    pub retract_z: f64,
    pub feed_z: f64,
    pub top_z: f64,
    pub bottom_z: f64,
    /// True when `top_z` came from a user choice (Manual / FromReference)
    /// rather than Auto. Carried for ops that distinguish user intent from
    /// the Auto default; note adaptive3d deliberately ignores a pinned top
    /// (see `generate_adaptive3d` — roughing must start from the real
    /// material top or the simulator carries unplanned overhead).
    pub top_pinned: bool,
    /// True when `bottom_z` came from a user choice. adaptive3d honors
    /// this as a floor on its Z-level plan (audit 2026-06-12, finding 2).
    pub bottom_pinned: bool,
}

impl ResolvedHeights {
    /// The depth range: distance from top_z to bottom_z (positive value).
    pub fn depth(&self) -> f64 {
        (self.top_z - self.bottom_z).abs()
    }
}

/// Tool containment mode for machining boundary (re-export from core).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryContainment {
    #[default]
    Center,
    Inside,
    Outside,
}

/// How a machining boundary is derived.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BoundarySource {
    /// Stock bounding rectangle (current default).
    #[default]
    Stock,
    /// 2D silhouette of the 3D model projected along the tool axis (Z-down).
    ModelSilhouette,
    /// Imported 2D geometry (DXF/SVG closed chains) — indices into the
    /// toolpath's polygon list.
    Geometry { polygon_indices: Vec<usize> },
    /// Selected STEP/CAD faces projected to XY.
    FaceSelection,
    /// Rest regions derived from another toolpath's rest-depth analysis
    /// (`AnnotatedToolpath::rest_regions`, populated by the pencil
    /// RestDepth detector). `source_toolpath_id` is the *stable id*
    /// (`ToolpathConfig.id`, not an index) of the toolpath whose cached
    /// generation result supplies the polygons — the regions are
    /// re-resolved from that result at generation time, never copied in
    /// here, so they always reflect the source's latest generation.
    DerivedRestRegions {
        source_toolpath_id: crate::ids::ToolpathId,
    },
    /// One tier's islands from a multi-tool **tier map**
    /// (`planning/multitool_2026-08-23/ORCHESTRATION_PLAN.md` Phase O item 2).
    ///
    /// This variant carries the **recipe**, never the geometry — the same
    /// call [`Self::DerivedRestRegions`] makes, and for the same two reasons:
    /// a project file must store what to compute rather than a snapshot of a
    /// computation, and the regions must reflect the mesh and the ladder as
    /// they are at generation time. The islands are re-derived through
    /// [`crate::maps::tier_map_cache::cached_tier_map`], so the `k` sibling ops one
    /// plan emits share a single grid walk rather than paying `k` of them.
    ///
    /// Unlike `DerivedRestRegions` this has **no source-toolpath
    /// dependency**: it derives from the mesh and the ladder alone, so an op
    /// carrying it never waits on a prior generation and never enters the
    /// `AwaitingPriorStock` arms. (Such an op is normally *also*
    /// `StockSource::FromRemainingStock`, which is a separate dependency on
    /// simulated stock and is enforced separately.)
    ///
    /// `tier` is always ≥ 1. Tier 0 is the complement — the coarse tool
    /// sweeps its territory as one pass and needs no boundary — so
    /// [`crate::maps::tier_islands::TierIslands`] publishes no set for it and this
    /// variant is never emitted with `tier: 0`.
    PlannedTierRegions {
        /// The FULL ladder, coarse → fine, as session tool ids. The whole
        /// ladder is needed even to resolve one tier: a tier label is "the
        /// coarsest tool on THIS ladder that holds this cell", which is not
        /// a statement any subset can reproduce.
        tool_ids: Vec<usize>,
        /// Which tier's islands bound this op (≥ 1).
        tier: u8,
        /// Tier-map planning resolution (mm). Plan at 0.3–0.6; a full-grid
        /// drop-cutter map at 0.15 costs ~125 s per tool on a 200 mm board
        /// (`crate::maps::tier_map`'s module doc).
        cell_mm: f64,
        /// Residual tolerance (mm) — see [`crate::maps::tier_map::TierMapParams`].
        tolerance_mm: f64,
        /// Grid padding (mm) beyond the finest tool's envelope.
        margin_mm: f64,
        /// How the raw residual is treated before the tolerance comparison.
        treatment: crate::maps::tier_map::ResidualTreatment,
        /// Island close / min-area / overlap / cap dials.
        islands: crate::maps::tier_islands::TierIslandParams,
    },
}

impl BoundarySource {
    pub fn label(&self) -> &'static str {
        match self {
            BoundarySource::Stock => "Stock",
            BoundarySource::ModelSilhouette => "Model Silhouette",
            BoundarySource::Geometry { .. } => "Imported Geometry",
            BoundarySource::FaceSelection => "Face Selection",
            BoundarySource::DerivedRestRegions { .. } => "Rest Regions",
            BoundarySource::PlannedTierRegions { .. } => "Planned Tier Regions",
        }
    }
}

/// Full machining boundary configuration.
///
/// Can live on `StockConfig` (global default) or on individual `ToolpathEntry`
/// (per-toolpath override).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoundaryConfig {
    pub enabled: bool,
    pub source: BoundarySource,
    pub containment: BoundaryContainment,
    /// Additional offset in mm (positive = expand, negative = shrink).
    /// Applied after source resolution, before tool-radius containment.
    pub offset: f64,
}

impl Default for BoundaryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            source: BoundarySource::Stock,
            containment: BoundaryContainment::Center,
            offset: 0.0,
        }
    }
}

impl BoundaryConfig {
    /// The creation-time boundary of a 3D operation on a mesh model: the
    /// model silhouette, `Center` containment, expanded by one tool
    /// diameter (R2, `planning/corne_case_analysis_2026-09-18/ANALYSIS.md`
    /// §5). Roadmap B.7 enabled the silhouette so the cutter does not sweep
    /// the whole stock on a small part in oversized stock. Under `Center`
    /// containment the cutter centre stays inside the polygon. A silhouette
    /// with no offset therefore keeps the cutter one radius short of every
    /// outer face. The diameter offset lets the cutter pass the outer face
    /// with one radius of clearance.
    ///
    /// `offset` stores a NUMBER, a copy of `tool_diameter_mm` at creation.
    /// A later tool change does not update it. Both creation doors (the
    /// GUI controller and the MCP `add_toolpath` door) call this with the
    /// diameter of the tool they bind.
    #[must_use]
    pub fn for_3d_op(tool_diameter_mm: f64) -> Self {
        Self {
            enabled: true,
            source: BoundarySource::ModelSilhouette,
            containment: BoundaryContainment::Center,
            offset: tool_diameter_mm,
        }
    }
}

/// Op-agnostic rest analysis: any toolpath can turn this on to run the
/// rest-depth detector (`rest_field::detect_rest_valleys`) against ITS OWN
/// tool as the fine cutter, using `reference_tool_id` (or the machined stock,
/// or a nominal ball — same resolution order pencil's rest-depth detector
/// already uses) as the reference. Before this config existed, only pencil's
/// `RestDepth` detector arm populated `rest_grid` / `rest_regions` on the
/// generated toolpath; this makes that analysis available to every family
/// (scallop, waterline, adaptive3d, ...) without generating a pencil
/// centerline toolpath at all — see `compute::execute`'s post-generation
/// rest-analysis pass for the generic wiring, and `pencil.rs::rest_depth_arm`
/// for the original detector this reuses.
///
/// Field defaults are READ FROM `rest_field::RestFieldParams::default()`,
/// not restated here — the two structs describe the same underlying
/// algorithm from two call sites (config vs. detector internals), so the
/// detector owns the numbers and this config copies them.
///
/// CMP-16: both files used to write `cell_mm` = 0.5,
/// `min_valley_depth` = 0.05 and `region_margin_mm` = 0.5 as separate
/// literals, under a doc line that said they "should stay numerically in
/// sync", and no test compared them. The consumer proves the coupling is
/// real: `execute/dressup_apply.rs` builds one `RestFieldParams` from
/// this config's three fields and takes `num_offset_passes_cap` and
/// `min_cut_length` from `RestFieldParams::default()` in the same
/// literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RestAnalysisConfig {
    pub enabled: bool,
    /// Real library tool whose geometry defines the rest reference. `None`
    /// falls back to the machined stock (when available) or a nominal ball,
    /// same resolution order as `PencilConfig::reference_tool_id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_tool_id: Option<crate::compute::tool_config::ToolId>,
    /// XY grid cell size (mm) for the rest field. Smaller = finer regions.
    pub cell_mm: f64,
    /// Rest-depth threshold (mm): a cell counts as REST material once the
    /// reference floats more than this above the true surface.
    pub min_valley_depth: f64,
    /// Extra clearance (mm) added around detected rest regions beyond the
    /// generating toolpath's own **CUSP (tip-sphere) radius**, when dilating
    /// the mask into region polygons.
    ///
    /// F1 (2026-08-23): the base was the ENVELOPE radius until then, which
    /// on a tapered ball is the shank — 3.5 mm of dilation on the shipped
    /// Ø1-tip / Ø6-shank taper, enough to weld dendritic islands into one
    /// region. See [`crate::surface::rest_field::RestFieldParams::region_margin_mm`].
    pub region_margin_mm: f64,
    /// PR-7 (H2.5): offset stepover (mm) the ROUTING criterion assumes a
    /// downstream pencil fan would emit — `pencil ⟺ X_reach ≤ cap ×
    /// stepover` ([`crate::surface::reach`]).
    ///
    /// `None` (the default, and what every project written before PR-7
    /// deserializes to) means **size it from the canonical reach policy**
    /// for THIS toolpath's own cutter, at [`Self::min_valley_depth`] — the
    /// same [`crate::surface::reach::suggested_offset_stepover_mm`] call
    /// `UnifiedFinish`'s claims pipeline makes. Before PR-7 this pass took
    /// the detector's literal 0.5 mm default, which on a tapered tool
    /// describes no fan anything would emit.
    ///
    /// Set it explicitly only to model a SPECIFIC downstream fan — e.g. a
    /// pencil operation whose `offset_stepover` the operator has pinned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset_stepover_mm: Option<f64>,
    /// PR-7 (H2.5): offset passes per side that fan is permitted, the `cap`
    /// in the same criterion. `None` = the detector's own default (0 —
    /// centreline only, which the coverage cap FLOOR still widens to the
    /// band one pass actually works, see
    /// [`crate::surface::reach::coverage_cap_passes`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_offset_passes: Option<usize>,
}

impl Default for RestAnalysisConfig {
    fn default() -> Self {
        // CMP-16: the detector's own defaults, read rather than restated.
        let detector = crate::surface::rest_field::RestFieldParams::default();
        Self {
            enabled: false,
            reference_tool_id: None,
            cell_mm: detector.cell_mm,
            min_valley_depth: detector.min_valley_depth,
            region_margin_mm: detector.region_margin_mm,
            // Not a value: an instruction to ask the reach policy. See the
            // field docs.
            offset_stepover_mm: None,
            num_offset_passes: None,
        }
    }
}

/// Entry style for plunge replacement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DressupEntryStyle {
    None,
    Ramp,
    Helix,
}

impl DressupEntryStyle {
    /// Convert to core `EntryStyle` using parameters from the dressup config.
    /// Returns `None` for `DressupEntryStyle::None` (no entry transformation).
    pub fn to_core(self, cfg: &DressupConfig) -> Option<crate::dressup::EntryStyle> {
        use crate::dressup::EntryStyle;
        match self {
            Self::None => None,
            Self::Ramp => Some(EntryStyle::Ramp {
                max_angle_deg: cfg.ramp_angle,
            }),
            Self::Helix => Some(EntryStyle::Helix {
                radius: cfg.helix_radius,
                pitch: cfg.helix_pitch,
            }),
        }
    }
}

/// Default deviation budget (mm) for [`DressupConfig::segment_merge`].
fn default_segment_merge_tolerance() -> f64 {
    0.3
}

/// The dogbone dressup's parameters. `None` on [`DressupConfig`] means the
/// dressup is off (CUT-13).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DogboneParams {
    /// Corner angle (degrees) at or below which a dogbone is cut.
    pub angle: f64,
}

impl Default for DogboneParams {
    fn default() -> Self {
        Self { angle: 90.0 }
    }
}

/// The lead-in / lead-out dressup's parameters.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LeadParams {
    /// Lead arc radius (mm).
    pub radius: f64,
    /// F-040: Lead-in feed rate (mm/min). When `Some`, lead-in arc moves
    /// emitted by `apply_lead_in_out` use this rate (typically slower than
    /// cutting feed for a softer entry / cleaner dwell mark). When `None`,
    /// lead-in inherits the operation's primary `feed_rate` (pre-F-040
    /// behaviour). Default `None`.
    pub in_feed_rate: Option<f64>,
    /// F-040: Lead-out feed rate (mm/min). Same fallback semantics —
    /// typically faster than cutting feed (chip-clear on exit). Default `None`.
    pub out_feed_rate: Option<f64>,
}

impl Default for LeadParams {
    fn default() -> Self {
        Self {
            radius: 2.0,
            in_feed_rate: None,
            out_feed_rate: None,
        }
    }
}

/// The link-moves dressup's parameters.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LinkDressupParams {
    /// Longest gap (mm) the tool may stay down across.
    pub max_distance: f64,
    /// Feed rate (mm/min) of a link move.
    pub feed_rate: f64,
}

impl Default for LinkDressupParams {
    fn default() -> Self {
        Self {
            max_distance: 10.0,
            feed_rate: 500.0,
        }
    }
}

/// The arc-fitting dressup's parameters.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ArcFitParams {
    /// Deviation budget (mm) between the fitted arc and the linear run.
    pub tolerance: f64,
}

impl Default for ArcFitParams {
    fn default() -> Self {
        Self { tolerance: 0.05 }
    }
}

/// The segment-merge dressup's parameters.
///
/// Phase 1 (accel-friendly toolpaths): merge dense runs of consecutive
/// same-feed linear cut moves whose interior points lie within `tolerance`
/// of the retained chord, so a low-acceleration controller can ramp to the
/// commanded feed instead of stalling on sub-millimetre segments. Runs after
/// arc-fitting.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SegmentMergeParams {
    /// Deviation budget (mm). Sits between the generation tolerance and
    /// stock-to-leave (roughing leaves >= 0.5 mm). Default 0.3.
    pub tolerance: f64,
}

impl Default for SegmentMergeParams {
    fn default() -> Self {
        Self {
            tolerance: default_segment_merge_tolerance(),
        }
    }
}

/// Configurable dressups applied after toolpath generation.
///
/// CUT-13: five dressups carry their parameters inside an `Option` instead
/// of pairing an enable `bool` with a value field that means nothing while
/// the bool is false. "arc_fitting = false with arc_tolerance = 0.05" is no
/// longer representable.
///
/// The wire form is unchanged. [`DressupConfigWire`] below carries the flat
/// bool-plus-value key layout every project file, every MCP
/// `set_dressup_field` key and the wire snapshot already use, and
/// `#[serde(from, into)]` converts between the two. Nothing outside this
/// file sees the wire struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(from = "DressupConfigWire", into = "DressupConfigWire")]
pub struct DressupConfig {
    pub entry_style: DressupEntryStyle,
    pub ramp_angle: f64,
    pub helix_radius: f64,
    pub helix_pitch: f64,
    /// `Some` when the dogbone dressup runs.
    pub dogbone: Option<DogboneParams>,
    /// `Some` when the lead-in / lead-out dressup runs.
    pub lead_in_out: Option<LeadParams>,
    /// `Some` when the link-moves dressup runs.
    pub link_moves: Option<LinkDressupParams>,
    /// `Some` when the arc-fitting dressup runs.
    pub arc_fitting: Option<ArcFitParams>,
    /// `Some` when the segment-merge dressup runs.
    pub segment_merge: Option<SegmentMergeParams>,
    /// Feed-rate optimisation. This one keeps the flat bool-plus-value
    /// shape: its only reader outside `compute/` is a fixture in
    /// `tool_load/`, which another session owns. See the commit body.
    pub feed_optimization: bool,
    pub feed_max_rate: f64,
    pub feed_ramp_rate: f64,
    /// Rapid-order optimisation. A bool with no value field of its own, so
    /// there is no pair to fold.
    pub optimize_rapid_order: bool,
    /// When the air-cut filter may replace a run of in-air cutting with a
    /// retract bridge (`planning/unified_v3_design.md` §10).
    ///
    /// `Always` (default) is the historical behaviour: bridge every air
    /// run however short. That is a net loss whenever the retract round
    /// trip is longer than the air it skips, which on a rest-clearer is
    /// most of them — the unified rest-clearer's 1 634 emitted fragments
    /// became 15 373 after filtering, and the added bridges account for
    /// almost all of its 539 m of rapid travel.
    pub air_bridge_policy: crate::dressup::AirBridgePolicy,
}

/// The serialized shape of [`DressupConfig`]: one enable `bool` beside each
/// value field, exactly as every project file and every MCP dressup key
/// already spell it.
///
/// This struct exists so CUT-13 can fix the in-memory type without moving a
/// single wire key. Read `DressupConfig`; nothing else should name this.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DressupConfigWire {
    entry_style: DressupEntryStyle,
    ramp_angle: f64,
    helix_radius: f64,
    helix_pitch: f64,
    dogbone: bool,
    dogbone_angle: f64,
    lead_in_out: bool,
    lead_radius: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    lead_in_feed_rate: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    lead_out_feed_rate: Option<f64>,
    link_moves: bool,
    link_max_distance: f64,
    link_feed_rate: f64,
    arc_fitting: bool,
    arc_tolerance: f64,
    #[serde(default)]
    segment_merge: bool,
    #[serde(default = "default_segment_merge_tolerance")]
    segment_merge_tolerance: f64,
    feed_optimization: bool,
    feed_max_rate: f64,
    feed_ramp_rate: f64,
    optimize_rapid_order: bool,
    #[serde(default)]
    air_bridge_policy: crate::dressup::AirBridgePolicy,
}

impl From<DressupConfigWire> for DressupConfig {
    fn from(w: DressupConfigWire) -> Self {
        Self {
            entry_style: w.entry_style,
            ramp_angle: w.ramp_angle,
            helix_radius: w.helix_radius,
            helix_pitch: w.helix_pitch,
            dogbone: w.dogbone.then_some(DogboneParams {
                angle: w.dogbone_angle,
            }),
            lead_in_out: w.lead_in_out.then_some(LeadParams {
                radius: w.lead_radius,
                in_feed_rate: w.lead_in_feed_rate,
                out_feed_rate: w.lead_out_feed_rate,
            }),
            link_moves: w.link_moves.then_some(LinkDressupParams {
                max_distance: w.link_max_distance,
                feed_rate: w.link_feed_rate,
            }),
            arc_fitting: w.arc_fitting.then_some(ArcFitParams {
                tolerance: w.arc_tolerance,
            }),
            segment_merge: w.segment_merge.then_some(SegmentMergeParams {
                tolerance: w.segment_merge_tolerance,
            }),
            feed_optimization: w.feed_optimization,
            feed_max_rate: w.feed_max_rate,
            feed_ramp_rate: w.feed_ramp_rate,
            optimize_rapid_order: w.optimize_rapid_order,
            air_bridge_policy: w.air_bridge_policy,
        }
    }
}

impl From<DressupConfig> for DressupConfigWire {
    fn from(c: DressupConfig) -> Self {
        let dogbone = c.dogbone.unwrap_or_default();
        let lead = c.lead_in_out.unwrap_or_default();
        let link = c.link_moves.unwrap_or_default();
        let arc = c.arc_fitting.unwrap_or_default();
        let merge = c.segment_merge.unwrap_or_default();
        Self {
            entry_style: c.entry_style,
            ramp_angle: c.ramp_angle,
            helix_radius: c.helix_radius,
            helix_pitch: c.helix_pitch,
            dogbone: c.dogbone.is_some(),
            dogbone_angle: dogbone.angle,
            lead_in_out: c.lead_in_out.is_some(),
            lead_radius: lead.radius,
            lead_in_feed_rate: lead.in_feed_rate,
            lead_out_feed_rate: lead.out_feed_rate,
            link_moves: c.link_moves.is_some(),
            link_max_distance: link.max_distance,
            link_feed_rate: link.feed_rate,
            arc_fitting: c.arc_fitting.is_some(),
            arc_tolerance: arc.tolerance,
            segment_merge: c.segment_merge.is_some(),
            segment_merge_tolerance: merge.tolerance,
            feed_optimization: c.feed_optimization,
            feed_max_rate: c.feed_max_rate,
            feed_ramp_rate: c.feed_ramp_rate,
            optimize_rapid_order: c.optimize_rapid_order,
            air_bridge_policy: c.air_bridge_policy,
        }
    }
}

/// One published dressup field: the wire name and what the field does.
///
/// CMP-17: the MCP `set_dressup_config` description used to be a hand-written
/// prose list. It named 17 fields of 23 and it advertised a dial nothing read.
/// This table is the one place the vocabulary is written down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DressupFieldDef {
    /// The wire name, identical to the serde name of the struct field.
    pub name: &'static str,
    /// What the field does, in one line.
    pub description: &'static str,
}

impl DressupConfig {
    /// Every settable dressup field.
    ///
    /// `dressup_field_names_are_published` (this file) holds the table against
    /// the struct, and the `rs_cam_viz` test of the same name holds the MCP
    /// `set_dressup_config` description against the table. A field added to
    /// `DressupConfig` without a row here fails the first test; a row that the
    /// description does not name fails the second.
    pub const FIELD_DEFS: &'static [DressupFieldDef] = &[
        DressupFieldDef {
            name: "entry_style",
            description: "how the tool enters the cut: none, ramp or helix",
        },
        DressupFieldDef {
            name: "ramp_angle",
            description: "ramp entry angle (degrees)",
        },
        DressupFieldDef {
            name: "helix_radius",
            description: "helix entry radius (mm)",
        },
        DressupFieldDef {
            name: "helix_pitch",
            description: "helix entry pitch per turn (mm)",
        },
        DressupFieldDef {
            name: "dogbone",
            description: "add dogbone overcuts at inside corners",
        },
        DressupFieldDef {
            name: "dogbone_angle",
            description: "corner angle at or below which a dogbone is cut (degrees)",
        },
        DressupFieldDef {
            name: "lead_in_out",
            description: "add tangential lead-in and lead-out arcs",
        },
        DressupFieldDef {
            name: "lead_radius",
            description: "lead-in and lead-out arc radius (mm)",
        },
        DressupFieldDef {
            name: "lead_in_feed_rate",
            description: "feed for the lead-in arc (mm/min); unset inherits the cutting feed",
        },
        DressupFieldDef {
            name: "lead_out_feed_rate",
            description: "feed for the lead-out arc (mm/min); unset inherits the cutting feed",
        },
        DressupFieldDef {
            name: "link_moves",
            description: "keep the tool down between near passes instead of retracting",
        },
        DressupFieldDef {
            name: "link_max_distance",
            description: "longest gap a link move may span (mm)",
        },
        DressupFieldDef {
            name: "link_feed_rate",
            description: "feed for a link move (mm/min)",
        },
        DressupFieldDef {
            name: "arc_fitting",
            description: "fit linear runs into G2 and G3 arcs",
        },
        DressupFieldDef {
            name: "arc_tolerance",
            description: "deviation budget for arc fitting (mm)",
        },
        DressupFieldDef {
            name: "segment_merge",
            description: "merge dense cut runs into longer moves; default on for roughing",
        },
        DressupFieldDef {
            name: "segment_merge_tolerance",
            description: "deviation budget for segment merge (mm)",
        },
        DressupFieldDef {
            name: "feed_optimization",
            description: "scale the feed with stock engagement",
        },
        DressupFieldDef {
            name: "feed_max_rate",
            description: "upper feed the optimiser may command (mm/min)",
        },
        DressupFieldDef {
            name: "feed_ramp_rate",
            description: "how fast the optimiser may change the feed (mm/min per mm)",
        },
        DressupFieldDef {
            name: "optimize_rapid_order",
            description: "reorder disconnected fragments to cut rapid travel",
        },
        DressupFieldDef {
            name: "air_bridge_policy",
            description: "when the air-cut filter may replace an air run with a retract bridge",
        },
    ];

    /// The published field names, in table order.
    #[must_use]
    pub fn published_field_names() -> Vec<&'static str> {
        Self::FIELD_DEFS.iter().map(|d| d.name).collect()
    }
}

impl Default for DressupConfig {
    fn default() -> Self {
        // Roadmap B.6 — three pure-flips: link_moves, feed_optimization,
        // optimize_rapid_order. All are wins for the typical case;
        // operations that can't tolerate them are stripped by
        // `normalize_for_op` (which `for_op` now also calls on construct).
        Self {
            entry_style: DressupEntryStyle::None,
            ramp_angle: 3.0,
            helix_radius: 2.0,
            helix_pitch: 1.0,
            dogbone: None,
            lead_in_out: None,
            link_moves: Some(LinkDressupParams::default()),
            arc_fitting: None,
            segment_merge: None,
            feed_optimization: true,
            feed_max_rate: 3000.0,
            feed_ramp_rate: 200.0,
            optimize_rapid_order: true,
            air_bridge_policy: crate::dressup::AirBridgePolicy::default(),
        }
    }
}

impl DressupConfig {
    /// The enable key that owns a value key, for the five dressups whose
    /// parameters CUT-13 moved inside an `Option`.
    ///
    /// `set_dressup_field` uses this to refuse a value patch whose dressup is
    /// off: with `Option<Params>` that value has nowhere to live, so writing
    /// it would be a set the caller never gets back.
    #[must_use]
    pub fn owner_of_value_field(key: &str) -> Option<&'static str> {
        match key {
            "dogbone_angle" => Some("dogbone"),
            "lead_radius" | "lead_in_feed_rate" | "lead_out_feed_rate" => Some("lead_in_out"),
            "link_max_distance" | "link_feed_rate" => Some("link_moves"),
            "arc_tolerance" => Some("arc_fitting"),
            "segment_merge_tolerance" => Some("segment_merge"),
            _ => None,
        }
    }

    /// Whether the named dressup is on. Only the five keys
    /// [`Self::owner_of_value_field`] returns are answered; anything else
    /// reads `false`.
    #[must_use]
    pub fn is_dressup_enabled(&self, key: &str) -> bool {
        match key {
            "dogbone" => self.dogbone.is_some(),
            "lead_in_out" => self.lead_in_out.is_some(),
            "link_moves" => self.link_moves.is_some(),
            "arc_fitting" => self.arc_fitting.is_some(),
            "segment_merge" => self.segment_merge.is_some(),
            _ => false,
        }
    }

    /// Smart defaults based on operation process role.
    pub fn for_role(role: super::catalog::UiProcessRole) -> Self {
        use super::catalog::UiProcessRole;
        let base = Self::default();
        match role {
            // Roadmap B.5 — Roughing operations get a Ramp entry by
            // default (was None, which gave every Pocket / Profile /
            // Adaptive / Face / Adaptive3d a vertical plunge). The
            // op-type override below promotes Adaptive/Adaptive3d to
            // Helix and strips back to None for Drill/Trace.
            UiProcessRole::Roughing => Self {
                entry_style: DressupEntryStyle::Ramp,
                arc_fitting: Some(ArcFitParams::default()),
                // Phase 1: roughing leaves >= 0.5 mm stock, so a 0.3 mm merge
                // deviation never touches the finish surface — pure win for
                // controller tracking. Finish/SemiFinish stay off (surface
                // fidelity).
                segment_merge: Some(SegmentMergeParams::default()),
                link_moves: Some(LinkDressupParams::default()),
                optimize_rapid_order: true,
                ..base
            },
            // R10 (2026-09-18): the finishing families get NO entry ramp
            // by default (was Ramp). A finishing contour starts on a
            // 0.5 mm leave, so a plunge there is a 0.5 mm bite. A 3 degree
            // ramp is 19 mm of XY per mm of descent, and on a short run the
            // fold lays that XY as laps over the run: the Corne waterline
            // sawed a 3 mm wall to pins along a 2 mm run (case analysis
            // §4.5). The fold's lap cap refuses the saw; this default stops
            // the finishing families asking for it. Roughing keeps Ramp.
            UiProcessRole::SemiFinish => Self {
                entry_style: DressupEntryStyle::None,
                arc_fitting: Some(ArcFitParams::default()),
                optimize_rapid_order: true,
                ..base
            },
            UiProcessRole::Finish => Self {
                entry_style: DressupEntryStyle::None,
                lead_in_out: Some(LeadParams::default()),
                arc_fitting: Some(ArcFitParams::default()),
                optimize_rapid_order: true,
                ..base
            },
        }
    }

    /// Smart defaults based on the operation type: the role-level base,
    /// then the registry's own per-op policy. No op is named here.
    pub fn for_op(op: super::catalog::OperationType) -> Self {
        let mut cfg = Self::for_role(op.spec().ui_process_role);
        // CMP-26: the hand-written ProjectCurve strip that stood here was
        // a no-op. `normalize_for_op` below reads
        // `op.registry_entry().dressup_policy`, and ProjectCurve's row is
        // `DressupPolicy::strip_all("Incompatible with Project Curve: each
        // ring would get a phantom diagonal cut.")`, which sets exactly
        // those three fields — with the user-facing reason the GUI greys
        // the controls with. This was the last op-specific arm in a method
        // whose doc says the decisions moved to the registry in Phase 1 T5.
        //
        // Apply the same per-op constraints fresh creates would otherwise
        // only see on project-file load. Without this, an MCP/GUI fresh
        // toolpath would carry e.g. `link_moves = true` (from the new
        // Default B.6) on a DropCutter where it's known to break
        // (Roadmap B.5/B.6).
        cfg.normalize_for_op(op);
        cfg
    }

    /// Normalize an existing dressup config for the given operation type,
    /// disabling any dressup that's geometrically incompatible with how the
    /// operation emits toolpaths. Intended as a one-shot migration on
    /// project load so the UI state and compute behaviour stay in lockstep.
    /// Returns `true` if anything changed, `false` if the config was already
    /// consistent.
    pub fn normalize_for_op(&mut self, op: super::catalog::OperationType) -> bool {
        use super::catalog::EntryStylePolicy;
        // Phase 1 (architectural refactor T5): the per-op decisions live
        // in the registry's `dressup_policy` field — ONE source shared
        // with the viz dressup panel. The op-specific rationale (phantom
        // diagonals on ProjectCurve/DropCutter, Roadmap B.5 entry
        // overrides, F-031 Adaptive3d planner↔simulator stamp parity)
        // is documented on the registry entries in compute/catalog.rs.
        let policy = op.registry_entry().dressup_policy;
        let mut changed = false;
        if policy.strip_all_reason.is_some() {
            if self.entry_style != DressupEntryStyle::None {
                self.entry_style = DressupEntryStyle::None;
                changed = true;
            }
            if self.lead_in_out.is_some() {
                self.lead_in_out = None;
                changed = true;
            }
            if self.link_moves.is_some() {
                self.link_moves = None;
                changed = true;
            }
        }
        match policy.entry {
            EntryStylePolicy::ForceNone => {
                if self.entry_style != DressupEntryStyle::None {
                    self.entry_style = DressupEntryStyle::None;
                    changed = true;
                }
            }
            EntryStylePolicy::PreferHelix => {
                if self.entry_style == DressupEntryStyle::Ramp {
                    self.entry_style = DressupEntryStyle::Helix;
                    changed = true;
                }
            }
            EntryStylePolicy::AnyEntry => {}
        }
        changed
    }
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

    /// CUT-13 — the wire form carries every dressup value field.
    ///
    /// The in-memory type keeps each dressup's parameters inside an `Option`;
    /// the serialized form keeps the flat enable-bool-beside-value-field
    /// layout every project file and every MCP `set_dressup_field` key uses.
    /// `DressupConfigWire` is the only thing between them, and a value it
    /// forgets is a dial an operator sets and never gets back.
    ///
    /// This test sets a NON-DEFAULT value on all five folded dressups, so a
    /// conversion that substitutes the module default fails here. The
    /// round-trip sentries alone cannot see that: their fixtures carry the
    /// defaults.
    #[test]
    fn the_wire_form_carries_every_dressup_value_field() {
        let cfg = DressupConfig {
            dogbone: Some(DogboneParams { angle: 111.0 }),
            lead_in_out: Some(LeadParams {
                radius: 3.5,
                in_feed_rate: Some(321.0),
                out_feed_rate: Some(654.0),
            }),
            link_moves: Some(LinkDressupParams {
                max_distance: 17.0,
                feed_rate: 432.0,
            }),
            arc_fitting: Some(ArcFitParams { tolerance: 0.017 }),
            segment_merge: Some(SegmentMergeParams { tolerance: 0.19 }),
            ..DressupConfig::default()
        };
        let json = serde_json::to_value(&cfg).unwrap();
        // The flat keys are what a project file and an MCP client write.
        for (key, want) in [
            ("dogbone_angle", 111.0),
            ("lead_radius", 3.5),
            ("lead_in_feed_rate", 321.0),
            ("lead_out_feed_rate", 654.0),
            ("link_max_distance", 17.0),
            ("link_feed_rate", 432.0),
            ("arc_tolerance", 0.017),
            ("segment_merge_tolerance", 0.19),
        ] {
            let got = json[key].as_f64().unwrap_or_else(|| {
                panic!("the wire form must carry '{key}'");
            });
            assert!((got - want).abs() < 1e-12, "{key}: {got} is not {want}");
        }
        for key in [
            "dogbone",
            "lead_in_out",
            "link_moves",
            "arc_fitting",
            "segment_merge",
        ] {
            assert_eq!(json[key], serde_json::json!(true), "{key} must read on");
        }

        let back: DressupConfig = serde_json::from_value(json).unwrap();
        assert_eq!(back.dogbone, cfg.dogbone);
        assert_eq!(back.lead_in_out, cfg.lead_in_out);
        assert_eq!(back.link_moves, cfg.link_moves);
        assert_eq!(back.arc_fitting, cfg.arc_fitting);
        assert_eq!(back.segment_merge, cfg.segment_merge);
    }

    /// A dressup that is off still writes its flat keys, with the module
    /// default beside a `false`. That is what every project file written
    /// before CUT-13 holds, so the reader must keep producing it.
    #[test]
    fn a_dressup_that_is_off_still_writes_its_flat_keys() {
        let json = serde_json::to_value(DressupConfig::default()).unwrap();
        assert_eq!(json["arc_fitting"], serde_json::json!(false));
        assert!((json["arc_tolerance"].as_f64().unwrap() - 0.05).abs() < 1e-12);
        assert_eq!(json["dogbone"], serde_json::json!(false));
        assert!((json["dogbone_angle"].as_f64().unwrap() - 90.0).abs() < 1e-12);

        // And a file that carries a value beside a `false` loads with the
        // dressup off; the orphan value is what CUT-13 stopped representing.
        let mut obj = json;
        obj["arc_tolerance"] = serde_json::json!(0.3);
        let cfg: DressupConfig = serde_json::from_value(obj).unwrap();
        assert!(cfg.arc_fitting.is_none());
    }

    /// CMP-17 — the published dressup table names every `DressupConfig` field.
    ///
    /// The names are read from a serialized instance, not from a hand-written
    /// list, so a field added to the struct shows up here. Both `Option`
    /// fields carry `skip_serializing_if = "Option::is_none"`, so the instance
    /// sets them to `Some`; a default instance would hide them and the test
    /// would pass while the table was wrong in exactly the direction CMP-17
    /// reports.
    ///
    /// NOT MEASURED: the description text of a row, and whether any code reads
    /// the field. The `rs_cam_viz` test of the same name holds the MCP
    /// description against this table.
    #[test]
    fn dressup_field_names_are_published() {
        let full = DressupConfig {
            lead_in_out: Some(LeadParams {
                radius: 2.0,
                in_feed_rate: Some(300.0),
                out_feed_rate: Some(900.0),
            }),
            ..DressupConfig::default()
        };
        let value = serde_json::to_value(&full).unwrap();
        let serialized: std::collections::BTreeSet<String> = value
            .as_object()
            .expect("DressupConfig serializes to an object")
            .keys()
            .cloned()
            .collect();
        let published: std::collections::BTreeSet<String> = DressupConfig::published_field_names()
            .into_iter()
            .map(str::to_owned)
            .collect();

        let missing: Vec<&String> = serialized.difference(&published).collect();
        assert!(
            missing.is_empty(),
            "DressupConfig fields absent from DressupConfig::FIELD_DEFS: {missing:?}"
        );
        let extra: Vec<&String> = published.difference(&serialized).collect();
        assert!(
            extra.is_empty(),
            "DressupConfig::FIELD_DEFS rows that name no field: {extra:?}"
        );
    }

    fn test_ctx() -> HeightContext {
        HeightContext {
            safe_z: 10.0,
            op_depth: 5.0,
            stock_top_z: 25.0,
            stock_bottom_z: 0.0,
            model_top_z: Some(20.0),
            model_bottom_z: Some(2.0),
        }
    }

    #[test]
    fn auto_resolve_top_z_follows_ctx_stock_top_z() {
        // F-028: Auto top_z now anchors to ctx.stock_top_z (was hardcoded 0).
        let cfg = HeightsConfig::default();
        let ctx = test_ctx();
        let h = cfg.resolve(&ctx);
        // retract = safe_z = 10
        assert!((h.retract_z - 10.0).abs() < 1e-9);
        // clearance = retract + 10 = 20
        assert!((h.clearance_z - 20.0).abs() < 1e-9);
        // feed = retract - 2 = 8
        assert!((h.feed_z - 8.0).abs() < 1e-9);
        // F-028: top_z Auto = ctx.stock_top_z = 25 (was 0 pre-fix)
        assert!((h.top_z - 25.0).abs() < 1e-9);
        // F-028: bottom_z Auto = top_z - op_depth = 25 - 5 = 20 (was -5 pre-fix)
        assert!((h.bottom_z - 20.0).abs() < 1e-9);
    }

    #[test]
    fn auto_resolve_top_z_zero_when_ctx_stock_top_z_zero() {
        // Backwards-compat for callers that build `HeightContext::simple` /
        // pre-F-028 fixtures with `stock_top_z = 0.0`: Auto top_z still
        // resolves to 0 in that frame, and bottom_z auto = -op_depth.
        let cfg = HeightsConfig::default();
        let ctx = HeightContext::simple(10.0, 5.0);
        let h = cfg.resolve(&ctx);
        assert!((h.top_z - 0.0).abs() < 1e-9);
        assert!((h.bottom_z - (-5.0)).abs() < 1e-9);
    }

    #[test]
    fn manual_passthrough() {
        let cfg = HeightsConfig {
            clearance_z: HeightMode::Manual(50.0),
            retract_z: HeightMode::Manual(30.0),
            feed_z: HeightMode::Manual(28.0),
            top_z: HeightMode::Manual(1.0),
            bottom_z: HeightMode::Manual(-10.0),
        };
        let h = cfg.resolve(&test_ctx());
        assert!((h.clearance_z - 50.0).abs() < 1e-9);
        assert!((h.retract_z - 30.0).abs() < 1e-9);
        assert!((h.feed_z - 28.0).abs() < 1e-9);
        assert!((h.top_z - 1.0).abs() < 1e-9);
        assert!((h.bottom_z - (-10.0)).abs() < 1e-9);
    }

    #[test]
    fn from_reference_stock_top() {
        let cfg = HeightsConfig {
            top_z: HeightMode::FromReference(ReferenceOffset {
                reference: HeightReference::StockTop,
                offset: -2.0,
            }),
            ..HeightsConfig::default()
        };
        let h = cfg.resolve(&test_ctx());
        // stock_top = 25, offset = -2 => 23
        assert!((h.top_z - 23.0).abs() < 1e-9);
    }

    #[test]
    fn from_reference_stock_bottom() {
        let cfg = HeightsConfig {
            bottom_z: HeightMode::FromReference(ReferenceOffset {
                reference: HeightReference::StockBottom,
                offset: 1.0,
            }),
            ..HeightsConfig::default()
        };
        let h = cfg.resolve(&test_ctx());
        // stock_bottom = 0, offset = 1 => 1
        assert!((h.bottom_z - 1.0).abs() < 1e-9);
    }

    #[test]
    fn from_reference_model_top() {
        let cfg = HeightsConfig {
            top_z: HeightMode::FromReference(ReferenceOffset {
                reference: HeightReference::ModelTop,
                offset: -3.0,
            }),
            ..HeightsConfig::default()
        };
        let h = cfg.resolve(&test_ctx());
        // model_top = 20, offset = -3 => 17
        assert!((h.top_z - 17.0).abs() < 1e-9);
    }

    #[test]
    fn model_ref_fallback_to_stock() {
        let ctx = HeightContext {
            model_top_z: None,
            model_bottom_z: None,
            ..test_ctx()
        };
        let cfg = HeightsConfig {
            top_z: HeightMode::FromReference(ReferenceOffset {
                reference: HeightReference::ModelTop,
                offset: 0.0,
            }),
            bottom_z: HeightMode::FromReference(ReferenceOffset {
                reference: HeightReference::ModelBottom,
                offset: 0.0,
            }),
            ..HeightsConfig::default()
        };
        let h = cfg.resolve(&ctx);
        // falls back to stock_top=25 and stock_bottom=0
        assert!((h.top_z - 25.0).abs() < 1e-9);
        assert!((h.bottom_z - 0.0).abs() < 1e-9);
    }

    #[test]
    fn serde_roundtrip_all_variants() {
        let auto = HeightMode::Auto;
        let manual = HeightMode::Manual(5.0);
        let from_ref = HeightMode::FromReference(ReferenceOffset {
            reference: HeightReference::StockTop,
            offset: -2.5,
        });

        for mode in [auto, manual, from_ref] {
            let json = serde_json::to_string(&mode).unwrap();
            let restored: HeightMode = serde_json::from_str(&json).unwrap();
            assert_eq!(mode, restored);
        }
    }

    #[test]
    fn backward_compat_old_json() {
        // Old files produce these JSON forms
        let auto: HeightMode = serde_json::from_str(r#"{"mode":"auto"}"#).unwrap();
        assert!(auto.is_auto());

        let manual: HeightMode = serde_json::from_str(r#"{"mode":"manual","value":5.0}"#).unwrap();
        assert_eq!(manual, HeightMode::Manual(5.0));
    }

    #[test]
    fn height_reference_all_exhaustive() {
        assert_eq!(
            HeightReference::ALL.len(),
            4,
            "HeightReference::ALL out of sync with enum"
        );
        use std::collections::HashSet;
        let refs: HashSet<_> = HeightReference::ALL.iter().collect();
        assert_eq!(
            refs.len(),
            HeightReference::ALL.len(),
            "HeightReference::ALL has duplicates"
        );
    }

    /// Phase 1 T5: the per-op dressup policy table, pinned. The registry
    /// field forces every NEW op to decide its policy at compile time;
    /// this pin makes changing an EXISTING op's policy loud. Mirrors the
    /// pre-registry predicates exactly (no behavior change).
    #[test]
    fn dressup_policy_table_is_pinned() {
        use super::super::catalog::{EntryStylePolicy, OperationType};
        for &op in OperationType::ALL {
            let policy = op.registry_entry().dressup_policy;
            match op {
                // UnifiedFinish joined the strip-all set 2026-07-09 (P2.f
                // Task 2): role-default Ramp entries carved diagonal
                // trenches across the wanaka terrain on a fresh MCP-added
                // op — same failure mode as DropCutter's documented one.
                OperationType::ProjectCurve
                | OperationType::DropCutter
                | OperationType::UnifiedFinish => {
                    assert!(policy.strip_all_reason.is_some(), "{op:?}: strip-all");
                    assert_eq!(policy.entry, EntryStylePolicy::AnyEntry);
                }
                // AlignmentPinDrill joined the force-no-entry set 2026-08-21
                // (G-WANAKA-DRILL-RAMP): it had been ANY_DRESSUP while its
                // sibling Drill was ForceNone, so `DressupConfig::for_op`
                // shipped a role-default Ramp on the one op whose entire
                // purpose is flip registration. A ramped pin hole is an oval
                // slot — the emitted motion on a real job was a 19 mm lateral
                // move while descending. The generator hard-strips it, so the
                // policy row was the last place config and behaviour still
                // disagreed.
                OperationType::Drill
                | OperationType::AlignmentPinDrill
                | OperationType::Trace
                | OperationType::Adaptive3d => {
                    assert!(policy.strip_all_reason.is_none(), "{op:?}");
                    assert_eq!(policy.entry, EntryStylePolicy::ForceNone, "{op:?}");
                }
                OperationType::Adaptive => {
                    assert!(policy.strip_all_reason.is_none());
                    assert_eq!(policy.entry, EntryStylePolicy::PreferHelix);
                }
                _ => {
                    // Pre-registry these fell through the predicate
                    // matches untouched.
                    assert!(
                        policy.strip_all_reason.is_none(),
                        "{op:?}: expected ANY_DRESSUP"
                    );
                    assert_eq!(policy.entry, EntryStylePolicy::AnyEntry, "{op:?}");
                }
            }
        }
    }

    /// CMP-16: the config's rest defaults ARE the detector's, not a copy
    /// of them.
    ///
    /// The struct doc said the three numbers "should stay numerically in
    /// sync", both files wrote them as separate literals, and nothing
    /// compared the two. The defaults are derived now, so this test is a
    /// guard against someone re-inlining a literal — which is a real
    /// possibility, not a tautology: the derive is three field reads that
    /// an edit can replace one at a time.
    #[test]
    fn rest_analysis_config_defaults_match_the_detector() {
        let cfg = RestAnalysisConfig::default();
        let detector = crate::surface::rest_field::RestFieldParams::default();

        assert!((cfg.cell_mm - detector.cell_mm).abs() < f64::EPSILON);
        assert!((cfg.min_valley_depth - detector.min_valley_depth).abs() < f64::EPSILON);
        assert!((cfg.region_margin_mm - detector.region_margin_mm).abs() < f64::EPSILON);

        // Non-vacuity: a zeroed detector default would satisfy the three
        // comparisons above and mean nothing.
        assert!(detector.cell_mm > 0.0);
        assert!(detector.min_valley_depth > 0.0);
        assert!(detector.region_margin_mm > 0.0);

        // The two `None` fields are NOT the detector's numbers. Both are
        // instructions to ask a policy at use time, and reading a literal
        // from the detector would freeze the answer — see the field docs
        // (PR-7 / H2.5).
        assert_eq!(cfg.offset_stepover_mm, None);
        assert_eq!(cfg.num_offset_passes, None);
    }

    /// Phase 1 T5: normalize_for_op applies the registry policy with the
    /// pre-registry semantics — strip-all clears entry+lead+link;
    /// force-no-entry clears entry ONLY; prefer-helix upgrades Ramp and
    /// leaves Helix/None alone; unrestricted ops pass through untouched.
    #[test]
    fn normalize_for_op_applies_registry_policy() {
        use super::super::catalog::OperationType;

        let dirty = || DressupConfig {
            entry_style: DressupEntryStyle::Ramp,
            lead_in_out: Some(LeadParams::default()),
            link_moves: Some(LinkDressupParams::default()),
            ..DressupConfig::default()
        };

        // Strip-all: ProjectCurve/DropCutter/UnifiedFinish clear all three.
        for op in [
            OperationType::ProjectCurve,
            OperationType::DropCutter,
            OperationType::UnifiedFinish,
        ] {
            let mut cfg = dirty();
            assert!(cfg.normalize_for_op(op));
            assert_eq!(cfg.entry_style, DressupEntryStyle::None, "{op:?}");
            assert!(cfg.lead_in_out.is_none(), "{op:?}");
            assert!(cfg.link_moves.is_none(), "{op:?}");
            // Idempotent.
            assert!(!cfg.normalize_for_op(op));
        }

        // CMP-26: `for_op` — not just `normalize_for_op` — must produce
        // the strip from the REGISTRY alone. `for_op` used to hand-write
        // the ProjectCurve strip four lines before calling
        // `normalize_for_op`, with its own rationale comment beside the
        // registry's own. Added BEFORE that block was deleted, so it
        // pins the resulting config across the delete rather than after
        // it.
        for op in [
            OperationType::ProjectCurve,
            OperationType::DropCutter,
            OperationType::UnifiedFinish,
        ] {
            let cfg = DressupConfig::for_op(op);
            assert_eq!(cfg.entry_style, DressupEntryStyle::None, "{op:?}");
            assert!(cfg.lead_in_out.is_none(), "{op:?}");
            assert!(cfg.link_moves.is_none(), "{op:?}");
        }

        // Force-no-entry: entry cleared, lead/link untouched.
        for op in [
            OperationType::Drill,
            OperationType::Trace,
            OperationType::Adaptive3d,
        ] {
            let mut cfg = dirty();
            assert!(cfg.normalize_for_op(op));
            assert_eq!(cfg.entry_style, DressupEntryStyle::None, "{op:?}");
            assert!(
                cfg.lead_in_out.is_some(),
                "{op:?}: lead-in/out must survive"
            );
            assert!(cfg.link_moves.is_some(), "{op:?}: link moves must survive");
        }

        // Prefer-helix: Ramp upgrades, Helix and None pass through.
        let mut cfg = dirty();
        assert!(cfg.normalize_for_op(OperationType::Adaptive));
        assert_eq!(cfg.entry_style, DressupEntryStyle::Helix);
        assert!(!cfg.normalize_for_op(OperationType::Adaptive));
        cfg.entry_style = DressupEntryStyle::None;
        assert!(!cfg.normalize_for_op(OperationType::Adaptive));

        // Unrestricted op: nothing changes.
        let mut cfg = dirty();
        assert!(!cfg.normalize_for_op(OperationType::Pocket));
        assert_eq!(cfg.entry_style, DressupEntryStyle::Ramp);
        assert!(cfg.lead_in_out.is_some() && cfg.link_moves.is_some());
    }
}
