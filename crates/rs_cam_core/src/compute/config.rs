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

#[derive(Debug, Clone)]
pub enum ComputeStatus {
    Pending,
    Computing,
    Done,
    Error(String),
}

#[derive(Debug, Clone, Default)]
pub struct ToolpathStats {
    pub move_count: usize,
    pub cutting_distance: f64,
    pub rapid_distance: f64,
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
    pub fn resolve(&self, ctx: &HeightContext) -> ResolvedHeights {
        let retract = self.retract_z.resolve_value(ctx.safe_z, ctx);
        let top_z = self.top_z.resolve_value(ctx.stock_top_z, ctx);
        ResolvedHeights {
            clearance_z: self.clearance_z.resolve_value(retract + 10.0, ctx),
            retract_z: retract,
            feed_z: self.feed_z.resolve_value(retract - 2.0, ctx),
            top_z,
            bottom_z: self.bottom_z.resolve_value(top_z - ctx.op_depth.abs(), ctx),
            top_pinned: !self.top_z.is_auto(),
            bottom_pinned: !self.bottom_z.is_auto(),
        }
    }
}

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
}

impl BoundarySource {
    /// Sources with no extra configuration beyond picking them — safe for a
    /// simple combo box. `Geometry` needs a polygon-index picker,
    /// `FaceSelection` a face picker, and `DerivedRestRegions` a
    /// source-toolpath picker, so none of those three are listed here.
    pub const ALL_SIMPLE: &[BoundarySource] =
        &[BoundarySource::Stock, BoundarySource::ModelSilhouette];

    pub fn label(&self) -> &'static str {
        match self {
            BoundarySource::Stock => "Stock",
            BoundarySource::ModelSilhouette => "Model Silhouette",
            BoundarySource::Geometry { .. } => "Imported Geometry",
            BoundarySource::FaceSelection => "Face Selection",
            BoundarySource::DerivedRestRegions { .. } => "Rest Regions",
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
/// Field defaults mirror `rest_field::RestFieldParams`'s own defaults
/// (`cell_mm` = 0.5, `min_valley_depth` = 0.05, `region_margin_mm` = 0.5) —
/// the two structs describe the same underlying algorithm from two call
/// sites (config vs. detector internals) and should stay numerically in sync.
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
    /// generating toolpath's own tool radius, when dilating the mask into
    /// region polygons.
    pub region_margin_mm: f64,
}

impl Default for RestAnalysisConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            reference_tool_id: None,
            cell_mm: 0.5,
            min_valley_depth: 0.05,
            region_margin_mm: 0.5,
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

/// How the tool retracts between cutting passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetractStrategy {
    /// Always retract to retract_z (safest, default).
    Full,
    /// Retract just above the highest Z on nearby path + 2mm (faster).
    Minimum,
}

/// Default deviation budget (mm) for [`DressupConfig::segment_merge`].
fn default_segment_merge_tolerance() -> f64 {
    0.3
}

/// Configurable dressups applied after toolpath generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DressupConfig {
    pub entry_style: DressupEntryStyle,
    pub ramp_angle: f64,
    pub helix_radius: f64,
    pub helix_pitch: f64,
    pub dogbone: bool,
    pub dogbone_angle: f64,
    pub lead_in_out: bool,
    pub lead_radius: f64,
    /// F-040: Lead-in feed rate (mm/min). When `Some`, lead-in arc moves
    /// emitted by `apply_lead_in_out` use this rate (typically slower than
    /// cutting feed for a softer entry / cleaner dwell mark). When `None`,
    /// lead-in inherits the operation's primary `feed_rate` (pre-F-040
    /// behaviour). Default `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lead_in_feed_rate: Option<f64>,
    /// F-040: Lead-out feed rate (mm/min). Same fallback semantics —
    /// typically faster than cutting feed (chip-clear on exit). Default `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lead_out_feed_rate: Option<f64>,
    pub link_moves: bool,
    pub link_max_distance: f64,
    pub link_feed_rate: f64,
    pub arc_fitting: bool,
    pub arc_tolerance: f64,
    /// Phase 1 (accel-friendly toolpaths): merge dense runs of consecutive
    /// same-feed linear cut moves whose interior points lie within
    /// `segment_merge_tolerance` of the retained chord, so a low-acceleration
    /// controller can ramp to the commanded feed instead of stalling on
    /// sub-millimetre segments. Runs after arc-fitting. Default on for
    /// roughing; `#[serde(default)]` keeps older project files loadable.
    #[serde(default)]
    pub segment_merge: bool,
    /// Deviation budget (mm) for `segment_merge`. Sits between the generation
    /// tolerance and stock-to-leave (roughing leaves ≥0.5 mm). Default 0.3.
    #[serde(default = "default_segment_merge_tolerance")]
    pub segment_merge_tolerance: f64,
    pub feed_optimization: bool,
    pub feed_max_rate: f64,
    pub feed_ramp_rate: f64,
    pub optimize_rapid_order: bool,
    pub retract_strategy: RetractStrategy,
    /// When the air-cut filter may replace a run of in-air cutting with a
    /// retract bridge (`planning/unified_v3_design.md` §10).
    ///
    /// `Always` (default) is the historical behaviour: bridge every air
    /// run however short. That is a net loss whenever the retract round
    /// trip is longer than the air it skips, which on a rest-clearer is
    /// most of them — the unified rest-clearer's 1 634 emitted fragments
    /// became 15 373 after filtering, and the added bridges account for
    /// almost all of its 539 m of rapid travel.
    #[serde(default)]
    pub air_bridge_policy: crate::dressup::AirBridgePolicy,
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
            dogbone: false,
            dogbone_angle: 90.0,
            lead_in_out: false,
            lead_radius: 2.0,
            lead_in_feed_rate: None,
            lead_out_feed_rate: None,
            link_moves: true,
            link_max_distance: 10.0,
            link_feed_rate: 500.0,
            arc_fitting: false,
            arc_tolerance: 0.05,
            segment_merge: false,
            segment_merge_tolerance: 0.3,
            feed_optimization: true,
            feed_max_rate: 3000.0,
            feed_ramp_rate: 200.0,
            optimize_rapid_order: true,
            retract_strategy: RetractStrategy::Full,
            air_bridge_policy: crate::dressup::AirBridgePolicy::default(),
        }
    }
}

impl DressupConfig {
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
                arc_fitting: true,
                // Phase 1: roughing leaves ≥0.5 mm stock, so a 0.3 mm merge
                // deviation never touches the finish surface — pure win for
                // controller tracking. Finish/SemiFinish stay off (surface
                // fidelity).
                segment_merge: true,
                link_moves: true,
                optimize_rapid_order: true,
                ..base
            },
            UiProcessRole::SemiFinish => Self {
                entry_style: DressupEntryStyle::Ramp,
                arc_fitting: true,
                optimize_rapid_order: true,
                ..base
            },
            UiProcessRole::Finish => Self {
                entry_style: DressupEntryStyle::Ramp,
                lead_in_out: true,
                arc_fitting: true,
                optimize_rapid_order: true,
                ..base
            },
        }
    }

    /// Smart defaults based on the operation type. Uses `for_role` as a base
    /// and overrides per-op where the role-level defaults don't fit.
    pub fn for_op(op: super::catalog::OperationType) -> Self {
        use super::catalog::OperationType;
        let mut cfg = Self::for_role(op.spec().ui_process_role);
        // ProjectCurve traces 2D rings projected onto a surface. Ramp entries
        // would cut a straight diagonal across the surface next to each ring
        // start — and there can be hundreds of tiny rings. Use a direct plunge.
        if op == OperationType::ProjectCurve {
            cfg.entry_style = DressupEntryStyle::None;
            cfg.lead_in_out = false;
            // Link moves bridge separate path fragments at cutting depth.
            // For project_curve, fragments can be distant (different rings,
            // or gaps where the path leaves the mesh footprint), and bridging
            // them at depth carves phantom lines across the stock. Always
            // rapid-retract between fragments.
            cfg.link_moves = false;
        }
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
            if self.lead_in_out {
                self.lead_in_out = false;
                changed = true;
            }
            if self.link_moves {
                self.link_moves = false;
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
                OperationType::Drill | OperationType::Trace | OperationType::Adaptive3d => {
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

    /// Phase 1 T5: normalize_for_op applies the registry policy with the
    /// pre-registry semantics — strip-all clears entry+lead+link;
    /// force-no-entry clears entry ONLY; prefer-helix upgrades Ramp and
    /// leaves Helix/None alone; unrestricted ops pass through untouched.
    #[test]
    fn normalize_for_op_applies_registry_policy() {
        use super::super::catalog::OperationType;

        let dirty = || DressupConfig {
            entry_style: DressupEntryStyle::Ramp,
            lead_in_out: true,
            link_moves: true,
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
            assert!(!cfg.lead_in_out, "{op:?}");
            assert!(!cfg.link_moves, "{op:?}");
            // Idempotent.
            assert!(!cfg.normalize_for_op(op));
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
            assert!(cfg.lead_in_out, "{op:?}: lead-in/out must survive");
            assert!(cfg.link_moves, "{op:?}: link moves must survive");
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
        assert!(cfg.lead_in_out && cfg.link_moves);
    }
}
