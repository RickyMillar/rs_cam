//! P2.c + P2.d of the unified finishing pass
//! (`planning/unified_finish_planner_design.md`): the per-region generation
//! orchestrator and the region ROUTER.
//!
//! [`unified_finish_toolpath_with_cancel`] composes the P2.b decomposition
//! ([`crate::finish::finish_planner::decompose`]) with three EXISTING region-scoped
//! strategy generators — since P2.d, one call PER REGION (single-polygon
//! [`RegionSet`]) rather than per band, so regions can be ordered freely.
//! With a [`LinkKinematics`] envelope in scope the regions are then routed
//! greedily: each junction's edge cost is
//! `min(retract_link_time, surface_link_time)` integrated with the F-034
//! model (P0 lesson: NEVER distance/feed — it misjudges segmented paths by
//! up to 10×), the route is seeded at the steepest band present
//! (steep-first tool-freshness, `steep_shallow`'s established ordering),
//! and the WINNING candidate is what actually gets emitted between the two
//! region toolpaths: a gouge-checked surface link (Linking feeds, follower's
//! rapid+plunge preamble stripped) or the native retract+rapid+replunge.
//! Without `LinkKinematics` the order falls back to steep-first band-major
//! concatenation (P2.c behavior) with native links only — never a
//! distance-costed guess.
//!
//! Creases fold into the claims pipeline (v3 S1,
//! `planning/unified_v3_design.md` §2.1): when [`ClaimsConfig`] is `Some`,
//! [`detect_rest_valleys`] runs against THIS op's own tip cutter before
//! [`decompose`], its centerlines feed `decompose`'s `creases` param (every
//! claimed corridor is carved out of the band label grid there), and every
//! crease that actually claimed a corridor gets its cut paths emitted as
//! ONE additional node appended after the routed bands (native link, not
//! threaded through `route_greedy` — S3 is where the fused router picks it
//! up). `claims: None` reproduces the pre-v3 op byte-for-byte: `decompose`
//! still gets an empty crease slice, exactly as before.
//!
//! A region-level territory filter ("S2", `planning/unified_v3_design.md`
//! §2.1 step 4 / §0.a) was built on top of S1's per-cell rest MEASUREMENT
//! and then REMOVED (2026-07-27, §7/§8 of that doc): with a
//! `ClaimsConfig::territory_stock` in scope, every covered classification
//! cell got a per-cell rest verdict (stock top − pencil drop vs
//! `min_rest_depth_mm`), and after `decompose` produced its conditioned
//! band islands, any WHOLE island whose measured rest share fell below a
//! dial was dropped before Step 4 generated it — never a cell hole (the
//! fragmentation lesson: cell masking shredded the same decomposition
//! 4 → 35 regions on 0.2% of cells). It measured dead on both sides: with
//! `territory_clip` (S4, below) OFF, decompose's conditioned islands are
//! whole-band-sized on real terrain, so every island contains SOME
//! above-dial rest and the filter never drops anything (+47% vs the
//! all-over-tip baseline); with `territory_clip` ON, coverage is already
//! masked to rest territory BEFORE `decompose` runs, so every conditioned
//! island IS a rest island and the filter's share reads ~1.0 everywhere —
//! it can never fire. Its own input (a 5-point footprint max-top/min-drop
//! sample across two grids) was separately measured to saturate on sloped
//! terrain (the neighbourhood z-span alone dwarfs an mm-class dial), so
//! the measurement was both unused and misleading. The fragmentation
//! lesson survives below as S4's reason for masking BEFORE `decompose`
//! rather than punching cell holes after.
//!
//! Process-proof build-list item 3 (`planning/v3_process_proof_prompt.md`,
//! design doc §0.a / §2.1 step 2 amendment) adds [`CreaseReference`]: the
//! S1 "claims are GEOMETRIC, territory is MATERIAL" rule was proven on a
//! ROUGH→finish chain, where a stock-referenced rest field reads roughing
//! TERRACES as a phantom dendritic crease network. That lesson does NOT
//! generalize to every stock reference — on a CASCADE where this op's
//! `territory_stock` is itself a FINISH-QUALITY prior pass (Op A's own
//! ball all-over scallop), the machined stock is the R2-validated honest
//! crease reference instead. `ClaimsConfig::crease_reference` selects the
//! arm; the default (`CreaseReference::SelfProbe`) is byte-identical to
//! S1/S2. Emission stays additive either way — no corridor carving.
//!
//! S4 (`planning/unified_v3_design.md` §0.a / §2.1 step 4, process-proof
//! campaign) adds rest-territory CONFINEMENT: measured on the wanaka ×2
//! cascade A/B, Op B (this op as a rest-clearer after a ball all-over
//! pass) ran +47% over the all-over-tip baseline under the region-level
//! drop filter described above, because that filter can only DROP an
//! island whole — when decompose's conditioned islands are whole-band-
//! sized, every giant island contains above-dial rest somewhere and gets
//! kept WHOLE, generated at full tip dials over territory that's already
//! finished. `ClaimsConfig::territory_clip` ANDs a rest keep-mask
//! (untrusted keeps, measured-skippable drops, built from the detector's
//! own stock-referenced rest FIELD — NOT a per-cell footprint verdict,
//! which saturates on sloped terrain, see the field doc) into `covered`
//! BEFORE `decompose`, so the conditioning pipeline itself normalizes the
//! rest islands and every emitted band region IS a conditioned rest
//! island — design doc §2.1 step 4's prescribed implementation, with the
//! R1 mitigations. See the `territory_clip` field doc for why this is not
//! the S1 sparse-pinprick fragmentation pathology, and for the measured
//! dead ends it replaced (region-level keep-or-drop; post-decompose
//! polygon clipping, which dies on `MAX_REST_REGIONS`-class caps over
//! dendritic rest masks).
//!
//! No dressups, no boundary clipping here: the stitched toolpath this
//! module returns flows through the NORMAL session post-passes (boundary
//! clip, `optimize_entry_descents`, feed modulation, F-034 accounting)
//! exactly like every other op's raw generator output.

mod region_routing;

use std::ops::Range;

use crate::finish::classify_probe::ClassificationSampler;
use crate::finish::crease_paths::centerline_cut_paths;
use crate::finish::finish_planner::{FinishBand, FinishPlannerParams, decompose};
use crate::finish::finish_setup::{
    FinishResolutionPolicy, SLOPE_FILTER_MAX_DEG, SLOPE_FILTER_MIN_DEG,
    build_classification_surface_with_sampler_and_cancel,
};
use crate::finish::pencil::PencilParams;
use crate::finish::scallop::{
    ScallopDirection, ScallopParams, ScallopRuntimeAnnotation,
    scallop_toolpath_structured_annotated_with_cancel,
};
use crate::geo::P2;
use crate::geometry::region_set::RegionSet;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::machine::kinematics::LinkKinematics;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::ops::waterline::{WaterlineParams, waterline_toolpath_with_cancel, waterline_z_levels};
use crate::polygon::Polygon2;
use crate::surface::dropcutter::DropCutterGrid;
use crate::surface::rest_field::{RestFieldParams, RestReference, detect_rest_valleys};
use crate::tool::MillingCutter;
use crate::toolpath::{MoveIntent, Toolpath, raster_toolpath_from_grid};
use crate::trace::debug_trace::ToolpathDebugContext;

use region_routing::{
    RegionPath, SHALLOW_DERATE_MIN_SLOPE_DEG, band_rank, band_z_range, build_shallow_raster_grid,
    cutting_length_mm, region_sampling_window, route_greedy, shallow_region_max_slope_deg,
    strippable_preamble, trailing_retracts,
};

// ── Types ────────────────────────────────────────────────────────────────

/// Inherited-dial bundle for the unified finish op (one-new-dial rule: the
/// genuinely new dials are the two thresholds + overlap, which live on
/// [`FinishPlannerParams`]; everything here is inherited from the existing
/// per-strategy params).
#[derive(Debug, Clone)]
pub struct UnifiedFinishParams {
    /// Scallop height for mid-steep rings (mm) — scallop's primary dial.
    pub scallop_height: f64,
    /// Path tolerance (also drives classification/generation cell size).
    pub tolerance: f64,
    /// Raster stepover for shallow regions (mm).
    pub raster_stepover: f64,
    /// Waterline Z step for very-steep regions (mm).
    pub z_step: f64,
    /// Waterline contour sampling (mm).
    pub sampling: f64,
    /// Stock to leave (mm) — honoured by **all three bands**.
    ///
    /// Applied as a `+Z` shift on the drop-cutter contact point: mid-steep
    /// scallop lifts each ring vertex, shallow raster lifts each grid point
    /// (with the off-mesh sentinel and its filter threshold moving together),
    /// very-steep waterline lifts each contour point. Surface links and the
    /// crease/claims pencil ride at the same offset, so no band seam steps.
    ///
    /// **F3 / D-16.2 (2026-08-06).** This doc used to read "scallop path only
    /// (raster and waterline don't take one today; parity with the standalone
    /// ops)". Two of the three bands silently dropped the operator's dial, and
    /// the "parity" claim was false against `SteepShallow`, whose shallow
    /// raster has always applied it. Both bands were fixed together — fixing
    /// shallow alone would have introduced a `stock_to_leave`-sized step at
    /// the shallow↔waterline seam that did not exist before.
    ///
    /// **The approximation, stated.** A vertical lift leaves
    /// `stock_to_leave · cos θ` measured normal to the surface — 0.71× at 45°
    /// down to 0.26× at 75°. Every finish op in the repo shares this
    /// convention and two pin it in unit tests; whether it should become a
    /// surface-normal offset is a separate repo-wide question, deliberately
    /// NOT coupled to this field.
    pub stock_to_leave: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub safe_z: f64,
    /// INTRA-region stay-down linking (design doc §9): max XY gap (mm) a
    /// surface-following link may span between two consecutive cut
    /// fragments INSIDE one region. `0.0` disables the pass.
    ///
    /// This is the lever §9 identified. The router already links BETWEEN
    /// regions via `surface_link::build_surface_link`; within a region each
    /// fragment junction still costs a full retract-to-safe-Z round trip,
    /// and on wanaka ×2 there are 12 780 of them — 17 077 s of rapids whose
    /// cost is the two ~30 mm Z legs, not the XY hop. Reordering shortens
    /// the hop; only linking removes the legs.
    pub intra_region_hookup_mm: f64,
    /// Which sampler builds this op's classification grid (M3 wave 7b).
    ///
    /// Defaults to [`ClassificationSampler::PRODUCTION`]. It is a field
    /// rather than a global so the COLUMNS A/B can run both classifiers
    /// through the identical production pipeline in one process, and so a
    /// sentry can pin which one an op ran without inspecting global state.
    pub classification_sampler: ClassificationSampler,
    /// C2 (`planning/thin_organic_2026-08-27/PROGRAMME.md` Track C): split
    /// each SHALLOW region into monotone cells on the region's own raster
    /// lattice, and rotate that lattice to the region's PCA-minor axis when
    /// the region clears [`crate::geometry::monotone_cells::ELONGATION_GATE`].
    ///
    /// `true` (**the default since 2026-09-01**, the C4 operator surface
    /// review — `planning/thin_organic_2026-08-27/FINDINGS.md` §7, "C4
    /// ruling"). `false` is byte-identical to the pre-C2 band: one
    /// `raster_toolpath_from_grid` call per region on the shared 0° grid.
    ///
    /// The decomposition and the lattice always share ONE frame — see
    /// [`crate::geometry::monotone_cells`] for why decomposing at 0° and re-sweeping
    /// the cells at an angle is a different, and measured-worse, candidate.
    ///
    /// Measured, ceiling arm (`FINDINGS.md` §0i): **1.155×** on the wanaka
    /// top-three shallow regions, **1.215×** on the one region that clears
    /// the elongation gate. Those are RIG figures on the operator's mesh —
    /// approach them, do not promise them. C4 (rendered-surface review)
    /// bound adoption because cell seams change the cusp pattern; the
    /// operator passed it 2026-09-01.
    pub monotone_cell_decomposition: bool,
}

impl Default for UnifiedFinishParams {
    /// Mirrors the standalone ops' own defaults (`ScallopConfig` /
    /// `WaterlineConfig` / `DropCutterConfig` in
    /// `compute::operation_configs`) so switching between the standalone
    /// three-op stack and this orchestrator doesn't silently change
    /// feeds/quality at the same tool. `safe_z` has no config-struct
    /// equivalent (it's the runtime `ctx.heights.retract_z` at the op-adapter
    /// layer, wave 2) — 30.0 mirrors `ScallopParams::default()`'s
    /// stand-in value.
    fn default() -> Self {
        Self {
            scallop_height: 0.1,
            tolerance: 0.05,
            raster_stepover: 1.0,
            z_step: 1.0,
            sampling: 0.5,
            stock_to_leave: 0.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 30.0,
            // Kept in lockstep with
            // `operation_configs::default_unified_finish_intra_region_hookup_mm`
            // (ON at 6.0 since 2026-08-03, by operator ruling on wave 12's
            // evidence). Production always overrides this from config, so
            // only direct library callers and unit tests read it — which is
            // exactly why it matters that it agrees: a core default and a
            // serde default that disagree is the divergence class this
            // programme has already found twice.
            intra_region_hookup_mm: 6.0,
            classification_sampler: ClassificationSampler::PRODUCTION,
            // ON since 2026-09-01: the C4 operator surface review passed
            // (`planning/thin_organic_2026-08-27/FINDINGS.md` §7, "C4
            // ruling"). Kept in lockstep with
            // `operation_configs::default_unified_finish_monotone_cell_
            // decomposition`, for the reason the line above states.
            monotone_cell_decomposition: true,
        }
    }
}

/// Per-band generation telemetry for the report/debug surface.
#[derive(Debug, Clone, Copy, Default)]
pub struct BandGenStats {
    pub region_count: usize,
    pub move_count: usize,
}

/// One routed junction between two consecutive regions in the cut order
/// (P2.d telemetry — lets the A/B harness see which link kind won where).
///
/// Stays `pub`: it is the element type of the `pub` field
/// `UnifiedFinishReport::links`, so a crate-private form raises
/// `private_interfaces` (S29, 2026-09-16).
#[derive(Debug, Clone, Copy)]
pub struct RoutedLink {
    /// Index into the decomposition's `planned.regions` we linked FROM.
    pub from_region: usize,
    /// Index into `planned.regions` we linked TO.
    pub to_region: usize,
    /// True when the gouge-checked surface link won the integrator
    /// comparison and was emitted as Linking feeds; false when the native
    /// retract+rapid+replunge was kept.
    pub surface: bool,
    /// Integrated time (s) of the winning candidate.
    pub cost_s: f64,
    /// Integrated time (s) of the losing candidate — `None` when the
    /// surface candidate was unavailable (off-mesh, outside the machining
    /// boundary, or the follower has no strippable entry preamble), so no
    /// comparison happened.
    pub alt_cost_s: Option<f64>,
}

// ── Claims pipeline (v3 S1) ─────────────────────────────────────────────

/// Which reference the claims crease detector runs against (process-proof
/// build-list item 3, `planning/unified_v3_design.md` §0.a / §2.1 step 2
/// amendment, 2026-07-13).
///
/// The S1 lesson — **claims are GEOMETRIC, territory is MATERIAL** — was
/// proven on a ROUGH→finish chain: there, a stock-referenced rest field
/// reads the roughing TERRACE pattern as a dendritic phantom crease
/// network (measured: 10k+ new uncut mid-steep columns once those
/// phantoms claimed corridors). That lesson holds wherever the prior
/// stock is UNFINISHED. It does NOT generalize to a cascade where this
/// op's prior stock is itself a FINISH-QUALITY pass (Op A's own ball
/// all-over scallop) — there the machined stock is the R2-validated
/// honest reference (it beat the analytic reference ~14× on real cusps
/// and unreached valleys, not roughing artefacts). Emission stays
/// additive under either variant — this enum only changes what feeds the
/// detector, never whether corridors get carved.
///
/// Derives `Serialize`/`Deserialize` directly — mirrors
/// [`crate::finish::scallop::ScallopDirection`]'s pattern (a core enum re-exported
/// as-is into `compute::operation_configs`, rather than a parallel
/// config-side enum) — so `UnifiedFinishConfig::claims_reference` can use
/// this type verbatim.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreaseReference {
    /// Analytic self-probe: geometric valleys of the design surface — the
    /// thing pencil corridors are FOR. The only honest signal on
    /// rough-reference chains (terraces read as phantom creases there),
    /// and self-defeating for single-tool ops BY CONSTRUCTION (measured:
    /// +22.5% finish time, zero quality gain — the float field marks
    /// exactly what the tool CANNOT reach).
    #[default]
    SelfProbe,
    /// The machined prior stock (`RestReference::Stock`, the R2-validated
    /// arm): sanctioned ONLY when that stock is FINISH-QUALITY (a
    /// cascade's Op B following Op A's own ball pass) — rest depth there
    /// is real uncut cusps and unreached valleys, not roughing terraces.
    /// Falls back to `SelfProbe` (with a `tracing::warn!`) when no
    /// `ClaimsConfig::territory_stock` is in scope — there is no stock to
    /// reference.
    MachinedStock,
}

/// The `claims_reference` DIAL as an operator sets it — one variant wider
/// than [`CreaseReference`], which is what the dial RESOLVES to (A/M6).
///
/// The two types are deliberately distinct. `CreaseReference` answers *which
/// field did the detector actually run against*, and every consumer inside
/// the pipeline needs an answer with no third option. `ClaimsReference`
/// answers *what did the operator ask for*, and the whole point of
/// [`Auto`](Self::Auto) is that it asks for a DERIVATION rather than for a
/// field. Collapsing them would put "decide later" into the type a detector
/// has to switch on.
///
/// **Serde compatibility is load-bearing.** The two pre-A/M6 names are
/// unchanged, so a project file that says `self_probe` or `machined_stock`
/// keeps its explicit meaning to the letter. Only the ABSENT field changes
/// meaning: it used to default to `SelfProbe` and now defaults to `Auto`
/// (`compute::operation_configs::default_unified_finish_claims_reference`),
/// and every resolution — derived or explicit — is recorded in
/// [`crate::compute::config::ClaimsReferenceFinding`] so the change is
/// visible rather than silent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimsReference {
    /// DERIVE it from what is in scope. A machined prior stock — this op
    /// cuts `StockSource::FromRemainingStock` and the simulated snapshot
    /// exists, i.e. `ClaimsConfig::territory_stock` is `Some` — resolves to
    /// [`CreaseReference::MachinedStock`]. Nothing in scope resolves to
    /// [`CreaseReference::SelfProbe`], which is the honest answer for a
    /// FIRST finish op: there is no machined stock to reference.
    ///
    /// A/M6 measured the cost of getting this wrong on wanaka, single
    /// variable, 0.1 mm sim, after a full same-tool finish: 46 366 mm of
    /// cutting under `self_probe` against 5 259 mm under `machined_stock`
    /// (−88.7%). The analytic field names exactly what the tool cannot
    /// reach, so a same-tool rest op referenced against it re-cuts the whole
    /// part.
    ///
    /// **Known limitation, stated rather than hidden:** this derivation
    /// keys on the PRESENCE of a prior stock, not on its QUALITY. On a
    /// rough→finish chain the S1 lesson still applies (roughing terraces
    /// read as a phantom dendritic crease network), and the operator should
    /// pin [`SelfProbe`](Self::SelfProbe) explicitly. The recorded finding
    /// says so.
    #[default]
    Auto,
    /// Pin the analytic self-probe, whatever is in scope. Honoured verbatim
    /// — and reported when a machined prior exists, because that
    /// combination is the A/M6 footgun and is almost never deliberate.
    SelfProbe,
    /// Pin the machined prior stock. Honoured when one is in scope; when
    /// none is, the detector has nothing to reference and degrades to the
    /// analytic self-probe, which is reported rather than warned into a
    /// log nobody is subscribed to.
    MachinedStock,
}

/// What [`ClaimsReference`] resolved to, and WHY — the provenance half of
/// A/M6.
///
/// Fieldless on purpose: the three facts a reader needs (what was asked for,
/// what was used, whether a machined prior was in scope) are exactly one of
/// six combinations, so the variant IS the record. This mirrors
/// [`crate::finish::finish_setup::CellSource`], which carries the same kind of
/// "which rule produced this number" provenance for the finish grid.
///
/// [`Self::resolve`] is the ONE place the mapping lives, so a new
/// [`ClaimsReference`] variant cannot be added without deciding what it
/// claims.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimsReferenceResolution {
    /// `Auto`, machined prior stock in scope → [`CreaseReference::MachinedStock`].
    /// The A/M6 fix, doing its job.
    DerivedMachinedStock,
    /// `Auto`, nothing in scope → [`CreaseReference::SelfProbe`]. A first
    /// finish op with no prior; the analytic field is the only one there is.
    DerivedSelfProbeNoPrior,
    /// Operator pinned `self_probe` and no machined prior was in scope.
    /// Nothing was available to use instead — the pin cost nothing.
    ExplicitSelfProbeNoPrior,
    /// Operator pinned `self_probe` **while a machined prior stock was in
    /// scope**. Honoured because it was explicit; reported because it is the
    /// A/M6 footgun in its exact shape.
    ExplicitSelfProbeOverridingPrior,
    /// Operator pinned `machined_stock` and one was in scope.
    ExplicitMachinedStock,
    /// Operator pinned `machined_stock` and NONE was in scope. Degraded to
    /// the analytic self-probe: the op is not referencing what its dial
    /// says it is.
    ExplicitMachinedStockWithoutPrior,
}

impl ClaimsReferenceResolution {
    /// Resolve the dial against what is in scope. The only mapping site.
    ///
    /// `prior_stock_in_scope` is `ClaimsConfig::territory_stock.is_some()`
    /// AFTER the caller's XY-frame guard — a stock that does not overlap
    /// this model in XY is not a reference, it is a different part.
    #[must_use]
    pub const fn resolve(setting: ClaimsReference, prior_stock_in_scope: bool) -> Self {
        match (setting, prior_stock_in_scope) {
            (ClaimsReference::Auto, true) => Self::DerivedMachinedStock,
            (ClaimsReference::Auto, false) => Self::DerivedSelfProbeNoPrior,
            (ClaimsReference::SelfProbe, true) => Self::ExplicitSelfProbeOverridingPrior,
            (ClaimsReference::SelfProbe, false) => Self::ExplicitSelfProbeNoPrior,
            (ClaimsReference::MachinedStock, true) => Self::ExplicitMachinedStock,
            (ClaimsReference::MachinedStock, false) => Self::ExplicitMachinedStockWithoutPrior,
        }
    }

    /// The field the detector actually runs against.
    #[must_use]
    pub const fn reference(self) -> CreaseReference {
        match self {
            Self::DerivedMachinedStock | Self::ExplicitMachinedStock => {
                CreaseReference::MachinedStock
            }
            Self::DerivedSelfProbeNoPrior
            | Self::ExplicitSelfProbeNoPrior
            | Self::ExplicitSelfProbeOverridingPrior
            | Self::ExplicitMachinedStockWithoutPrior => CreaseReference::SelfProbe,
        }
    }

    /// What the operator's dial said.
    #[must_use]
    pub const fn setting(self) -> ClaimsReference {
        match self {
            Self::DerivedMachinedStock | Self::DerivedSelfProbeNoPrior => ClaimsReference::Auto,
            Self::ExplicitSelfProbeNoPrior | Self::ExplicitSelfProbeOverridingPrior => {
                ClaimsReference::SelfProbe
            }
            Self::ExplicitMachinedStock | Self::ExplicitMachinedStockWithoutPrior => {
                ClaimsReference::MachinedStock
            }
        }
    }

    /// Whether a machined prior stock was in scope at resolution time.
    #[must_use]
    pub const fn prior_stock_in_scope(self) -> bool {
        match self {
            Self::DerivedMachinedStock
            | Self::ExplicitSelfProbeOverridingPrior
            | Self::ExplicitMachinedStock => true,
            Self::DerivedSelfProbeNoPrior
            | Self::ExplicitSelfProbeNoPrior
            | Self::ExplicitMachinedStockWithoutPrior => false,
        }
    }

    /// `true` when the reference was DERIVED rather than pinned.
    #[must_use]
    pub const fn is_derived(self) -> bool {
        matches!(
            self,
            Self::DerivedMachinedStock | Self::DerivedSelfProbeNoPrior
        )
    }

    /// `true` when the operator should be told loudly: either the A/M6
    /// footgun (explicit `self_probe` over a real machined prior) or a
    /// `machined_stock` dial that could not be honoured.
    #[must_use]
    pub const fn needs_attention(self) -> bool {
        matches!(
            self,
            Self::ExplicitSelfProbeOverridingPrior | Self::ExplicitMachinedStockWithoutPrior
        )
    }

    /// Stable machine-readable token — MCP field values, findings, tests.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::DerivedMachinedStock => "derived_machined_stock",
            Self::DerivedSelfProbeNoPrior => "derived_self_probe_no_prior",
            Self::ExplicitSelfProbeNoPrior => "explicit_self_probe_no_prior",
            Self::ExplicitSelfProbeOverridingPrior => "explicit_self_probe_overriding_prior",
            Self::ExplicitMachinedStock => "explicit_machined_stock",
            Self::ExplicitMachinedStockWithoutPrior => "explicit_machined_stock_without_prior",
        }
    }

    /// One operator-facing sentence saying why this reference was used, and
    /// what to do when it is the wrong one.
    #[must_use]
    pub const fn why(self) -> &'static str {
        match self {
            Self::DerivedMachinedStock => {
                "Auto: this operation cuts remaining stock and the simulated prior \
                 snapshot is in scope, so rest is measured against the material a \
                 previous pass actually left. If the previous pass was a ROUGHING \
                 pass, pin claims_reference = self_probe instead — a stock-referenced \
                 detector reads roughing terraces as phantom creases."
            }
            Self::DerivedSelfProbeNoPrior => {
                "Auto: no machined prior stock is in scope (this operation cuts fresh \
                 stock, or its prior snapshot does not overlap the model), so rest is \
                 derived analytically from the design surface. That is the honest \
                 reference for a FIRST finish pass. For a rest pass after another \
                 finish with the same tool, set the stock source to remaining stock \
                 so Auto can use the machined reference."
            }
            Self::ExplicitSelfProbeNoPrior => {
                "Pinned to the analytic self-probe; no machined prior stock was in \
                 scope, so nothing else was available anyway."
            }
            Self::ExplicitSelfProbeOverridingPrior => {
                "Pinned to the analytic self-probe WHILE a machined prior stock is in \
                 scope. The analytic field marks where this cutter cannot reach the \
                 model — for a same-tool cascade that is precisely what the pass \
                 cannot fix, so the operation re-cuts ground the previous pass already \
                 finished (measured −88.7% cutting when corrected). Set \
                 claims_reference to auto or machined_stock unless the prior pass was \
                 a ROUGHING pass."
            }
            Self::ExplicitMachinedStock => {
                "Pinned to the machined prior stock, which is in scope. Rest is measured \
                 against the material the previous pass actually left."
            }
            Self::ExplicitMachinedStockWithoutPrior => {
                "Pinned to the machined prior stock, but NONE is in scope — the \
                 detector degraded to the analytic self-probe. Set this operation's \
                 stock source to remaining stock, then generate and simulate the \
                 upstream operation so the snapshot exists (an operation waiting on \
                 that snapshot reports AwaitingPriorStock and resolves itself once the \
                 ladder is climbed)."
            }
        }
    }
}

/// In-op pencil-claims pipeline inputs (v3 S1, `planning/unified_v3_design.md`
/// §2.1). `claims: None` on [`unified_finish_toolpath_with_cancel`]
/// reproduces the pre-v3 op exactly: no detector run, no crease claims, no
/// crease node — the Wave 3 A/B harness pins this as the baseline.
pub struct ClaimsConfig<'a> {
    /// Machined prior stock for the TERRITORY mask (rest islands, step 4
    /// below) — already XY-frame-guarded by the caller; `None` → territory
    /// stays full.
    ///
    /// Also feeds crease DETECTION when `crease_reference ==
    /// CreaseReference::MachinedStock` — see that enum's doc for when that
    /// is sanctioned (finish-quality references only) versus the S1 A/B on
    /// the wanaka rough→finish chain, which proved a stock-referenced rest
    /// field reads the ROUGHING TERRACE pattern as a dendritic phantom
    /// "crease" network on an UNFINISHED prior stock — universal claims
    /// there then carve corridors the emitter never cuts (10k+ new uncut
    /// mid-steep columns). With `crease_reference` left at its default
    /// (`CreaseReference::SelfProbe`), this field is territory-only:
    /// claims are GEOMETRIC (design-surface valleys via the analytic
    /// self-probe); territory is MATERIAL (this stock).
    pub territory_stock: Option<&'a crate::dexel_stock::TriDexelStock>,
    /// Crease-detector reference (see [`CreaseReference`]). Defaults to
    /// `CreaseReference::SelfProbe` — the S1/S2 behavior, byte-identical.
    pub crease_reference: CreaseReference,
    /// Detector params. `routing_radius_mm` is overwritten with the op's own
    /// `cutter.radius()` before use — the finishing tool IS the pencil in
    /// this op (UnifiedFinish is single-tool, ball-tip-only), unlike the
    /// standalone pencil op's separate reference/pencil tool pair.
    pub rest_field_params: RestFieldParams,
    /// Rest-depth territory gate (mm, step 4 below): with a
    /// `territory_stock`, a classification cell stays `covered` unless the
    /// MEASURED rest there (stock top − pencil drop) is finite and below
    /// this. Untrusted samples (`NaN` drop / outside the stock grid) KEEP
    /// their coverage — only measured-thin territory drops out, so the
    /// detector's boundary-erosion rim can never amputate band area.
    /// Independent of `rest_field_params.min_valley_depth`, which gates
    /// the crease detector itself, not banding territory.
    pub min_rest_depth_mm: f64,
    /// S4 rest-territory confinement (design doc §2.1 step 4's prescribed
    /// implementation, `§0.a` item 3): AND a per-cell rest keep-mask (the
    /// SAME `min_rest_depth_mm` measurement, thresholded directly against
    /// the detector's rest field — untrusted samples keep coverage) into
    /// `covered` BEFORE `decompose`, so the
    /// conditioning pipeline (hysteresis flood, morph close, min-area
    /// absorption, overlap-dilated extraction) does its normal job on the
    /// intersected mask and the emitted band regions ARE conditioned rest
    /// islands.
    ///
    /// Why this is not the S1 cell-masking pathology: that measurement
    /// (4 → 35 regions, ring-cascading fragment edges) came from punching
    /// SPARSE pinprick holes (~0.2% of cells, at dendritic necks) into an
    /// otherwise-full mask AFTER territory had effectively been decided —
    /// artificial cuts through continuous band territory whose fragment
    /// boundaries sat mid-band, in uncut material. Territory INTERSECTION
    /// in rest-clearer mode is the opposite regime (design doc §5 R1,
    /// which pre-sanctions it with these exact mitigations): most of the
    /// mask drops because a finish-quality prior pass already cut it, the
    /// kept islands are the real work, their boundaries sit in
    /// ALREADY-FINISHED stock, and decompose's own `overlap_mm` dilation
    /// at extraction reaches back over the boundary so band passes blend
    /// outward into finished territory instead of leaving edge rings.
    ///
    /// Two dead ends this replaced (both measured on the wanaka ×2
    /// cascade, 2026-07-13): a region-level whole-island drop filter
    /// (measured and since removed, `unified_v3_design.md` §7/§8) alone
    /// cannot shrink whole-band-sized conditioned islands (+47% over the
    /// all-over baseline), and post-decompose POLYGON clipping (detector
    /// region-polygons, then a rest-field keep-mask) dies on
    /// `MAX_REST_REGIONS`-class caps — dendritic rest masks fragment into
    /// hundreds of islands, extraction keeps the largest 64, and the
    /// silently-dropped area surfaces as a 2-3× '>+.5' leftover tail.
    /// The mask-AND has no polygonization step to cap.
    ///
    /// The keep-mask is the detector's own stock-referenced rest FIELD
    /// (NaN keeps, `rest ≥ min_rest_depth_mm` keeps, dilated by pencil
    /// radius + region margin) — NOT a per-cell footprint verdict grid
    /// (5-point max/min sampling across two grids, measured and since
    /// removed): that measurement is keep-biased by design and
    /// saturates to "keep everything" on sloped or textured terrain
    /// (measured: Op B ran all-over again, 54 k s). Consequently this
    /// requires `crease_reference == MachinedStock` AND a
    /// `territory_stock` in scope; otherwise the orchestrator warns and
    /// skips confinement. Default `false`: byte-identical to the pre-S4
    /// op.
    pub territory_clip: bool,
    /// XY gap (mm) the crease node's emitter may bridge with a stay-down
    /// SURFACE FEED instead of retracting (Step 3.5 →
    /// [`crate::finish::pencil::emit_paths`] → `PencilParams::hookup_distance`).
    ///
    /// A dial rather than a constant because `emit_paths` links with NO
    /// territory boundary: `build_surface_link` only checks that the
    /// cutter keeps mesh contact, so a crease link is free to leave the
    /// rest island it belongs to and feed across ground the op was
    /// confined away from. That is the same gouge class
    /// [`crate::finish::surface_link::RelinkParams::boundary`] exists to prevent,
    /// and until the boundary is threaded through `emit_paths` this is the
    /// only lever on it. `0.0` disables crease linking entirely (every
    /// crease run gets its own retract + entry).
    ///
    /// Default is `PencilParams::default()`'s 5.0 — what every
    /// measurement before 2026-07-27 ran with.
    pub crease_hookup_mm: f64,
}

/// Which territory the claims pipeline banded (design doc §2.1 step 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClaimTerritoryMode {
    /// No stock reference in play: banding covers the whole classified
    /// surface, same as the pre-v3 op. Creases still claim corridors.
    #[default]
    Full,
    /// Stock reference in play: territory is MATERIAL, not just
    /// geometric — a rest reference (`ClaimsConfig::territory_stock`) is
    /// in scope. `ClaimsConfig::territory_clip` (S4) uses the detector's
    /// own rest field to confine banding to rest islands BEFORE
    /// `decompose` runs; without `territory_clip`, this mode is
    /// telemetry only (`ClaimsReport::detector_coverage` still reports
    /// what the detector found).
    RestIslands,
}

/// Which kind of routed node a [`RegionTableEntry`] describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionKind {
    /// A generated band region — see [`FinishBand`].
    Band(FinishBand),
    /// The single trailing pencil-claims node: every claimed crease's cut
    /// paths, concatenated and appended after the routed bands (native
    /// link — S1 does not thread this through `route_greedy`, see the
    /// module doc).
    Crease,
}

/// One routed node's identity and final move range in the stitched
/// toolpath — the Region-span attribution table (design doc §2.4: "Region
/// spans are a MUST"). `region_id` on the emitted `SpanPayload::Region` is
/// the index into [`UnifiedFinishReport::region_table`].
#[derive(Debug, Clone)]
pub struct RegionTableEntry {
    pub kind: RegionKind,
    pub move_range: Range<usize>,
    /// Polygon area (mm²): the band's own polygon area, or the summed
    /// claimed-corridor area for the crease node. Always `Some` today —
    /// both are cheap to compute from data already in hand.
    pub area_mm2: Option<f64>,
}

/// Which existing strategy generator produced a routed node's moves.
///
/// This is the STRATEGY half of the region label (plan A/M8): the whole
/// premise of the unified op is mixing strategies, so a diagnostic that
/// names only the band cannot say which generator earned its time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionStrategy {
    /// Shallow band — parallel raster rows.
    Raster,
    /// Mid-steep band — scallop-continuous rings.
    Scallop,
    /// Very-steep band — waterline Z-level contours.
    Waterline,
    /// Crease node — claimed pencil corridors.
    Pencil,
}

impl RegionStrategy {
    /// Stable lowercase token used in labels, semantic params, and gates.
    pub fn label(self) -> &'static str {
        match self {
            Self::Raster => "raster",
            Self::Scallop => "scallop",
            Self::Waterline => "waterline",
            Self::Pencil => "pencil",
        }
    }
}

impl RegionKind {
    /// The band this node belongs to, or `None` for the crease node.
    pub fn band(self) -> Option<FinishBand> {
        match self {
            Self::Band(band) => Some(band),
            Self::Crease => None,
        }
    }

    /// Stable band token. Matches `FinishBand`'s `Debug` spelling for the
    /// three bands so existing labels and log lines read the same;
    /// `"Crease"` for the trailing claims node, which has no band.
    pub fn band_label(self) -> &'static str {
        match self {
            Self::Band(FinishBand::Shallow) => "Shallow",
            Self::Band(FinishBand::MidSteep) => "MidSteep",
            Self::Band(FinishBand::VerySteep) => "VerySteep",
            Self::Crease => "Crease",
        }
    }

    /// Which generator emitted this node's moves.
    pub fn strategy(self) -> RegionStrategy {
        match self {
            Self::Band(FinishBand::Shallow) => RegionStrategy::Raster,
            Self::Band(FinishBand::MidSteep) => RegionStrategy::Scallop,
            Self::Band(FinishBand::VerySteep) => RegionStrategy::Waterline,
            Self::Crease => RegionStrategy::Pencil,
        }
    }

    /// Every variant, in declaration order — the list [`Self::from_span_label`]
    /// is derived from, so the label vocabulary cannot drift from the enum.
    pub const ALL: [Self; 4] = [
        Self::Band(FinishBand::Shallow),
        Self::Band(FinishBand::MidSteep),
        Self::Band(FinishBand::VerySteep),
        Self::Crease,
    ];

    /// Label carried on the STRUCTURAL `SpanKind::Region` span. Pinned
    /// verbatim to the pre-A/M8 text — spans feed the TSP's span remap and
    /// the GUI span list, so this string is a compatibility surface.
    pub fn span_label(self) -> String {
        match self {
            Self::Band(_) => format!("{} band", self.band_label()),
            Self::Crease => "Pencil claims".to_owned(),
        }
    }

    /// Inverse of [`Self::span_label`]. `None` for any label this enum did
    /// not produce — including the `Ring N` and `Z level` annotation spans
    /// that interleave with the region nodes as SIBLINGS rather than nesting
    /// inside them.
    ///
    /// # Why this exists rather than a payload field
    ///
    /// C4 (2026-08-02). Two harnesses reconstructed a node's KIND by matching
    /// its label against string literals they carried themselves —
    /// `p2c_headless_ab_wanaka.rs`'s `label == "Pencil claims"` crease probe
    /// and `v3_cascade_ab.rs`'s four-arm `strategy_of_span`. `SpanPayload::
    /// Region` carries a [`crate::trace::toolpath_spans::RegionSpanRole`], which
    /// answers node-vs-pass, but not WHICH node: putting `RegionKind` on the
    /// payload would drag `FinishBand` and this module into `toolpath_spans`,
    /// a deliberately low-level module with no finishing dependencies.
    ///
    /// So the string stays, and this is the one place that knows it.
    /// `span_label`/`from_span_label` round-trip over [`Self::ALL`] in
    /// `tests::region_kind_span_labels_round_trip`, which means a consumer
    /// asking `from_span_label(..) == Some(RegionKind::Crease)` breaks
    /// LOUDLY (here, at the round-trip gate) if the label ever changes,
    /// instead of silently matching nothing.
    ///
    /// **H4's mix tables must be built on this, on `RegionSpanRole`, or on
    /// [`crate::trace::semantic_trace::SemanticKey`] — never on a label literal
    /// spelled out at the consumer.**
    #[must_use]
    pub fn from_span_label(label: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.span_label() == label)
    }
}

/// One routed region node's identity — the SINGLE source both the
/// structural `SpanKind::Region` spans ([`unified_finish_spans`]) and the
/// SEMANTIC `Region` trace items (`compute::annotate::
/// annotate_unified_finish_regions`) are built from.
///
/// Plan A/M8: those two systems were independent and only the structural
/// one was implemented, so `narrate_toolpath` reported `regions 0` for the
/// one operation whose entire premise is mixing strategies. Both now read
/// this table, and `tests/unified_finish_semantic_regions.rs` asserts they
/// agree on count and move range so they cannot drift apart again.
#[derive(Debug, Clone)]
pub struct RegionAnnotation {
    /// Index into [`UnifiedFinishReport::region_table`] — the same value
    /// carried on `SpanPayload::Region`.
    pub region_id: u32,
    pub kind: RegionKind,
    pub move_range: Range<usize>,
    pub area_mm2: Option<f64>,
}

impl RegionAnnotation {
    /// See [`RegionKind::span_label`].
    pub fn span_label(&self) -> String {
        self.kind.span_label()
    }

    /// Label carried on the SEMANTIC trace item: band AND strategy, which
    /// is what the A/M8 acceptance gate and H4's mix table need.
    pub fn semantic_label(&self) -> String {
        match self.kind {
            RegionKind::Band(_) => format!(
                "{} band ({})",
                self.kind.band_label(),
                self.kind.strategy().label()
            ),
            RegionKind::Crease => format!("Crease claims ({})", self.kind.strategy().label()),
        }
    }
}

/// Project [`UnifiedFinishReport::region_table`] into the shared region
/// annotations both the structural spans and the semantic trace consume.
pub fn unified_finish_region_annotations(report: &UnifiedFinishReport) -> Vec<RegionAnnotation> {
    report
        .region_table
        .iter()
        .enumerate()
        .map(|(region_id, entry)| RegionAnnotation {
            region_id: region_id as u32,
            kind: entry.kind,
            move_range: entry.move_range.clone(),
            area_mm2: entry.area_mm2,
        })
        .collect()
}

/// Offset passes per side the claims pipeline is permitted to emit
/// (`planning/unified_v3_design.md` §2.1 item 5).
///
/// Named because the coverage routing criterion (`X_reach ≤ cap × stepover`)
/// and the emission fan must use the SAME cap — a literal repeated at two
/// call sites is exactly how PR-5 found the detector routing against a fan
/// nobody emitted.
pub const CLAIMS_OFFSET_PASS_CAP: usize = 4;

/// One phrase naming why [`ClaimsReport::offset_stepover_reference_depth_mm`]
/// is the depth the policy was sized at. Shipped in the operator-facing
/// diagnostic, so it lives next to the value.
pub const CLAIMS_STEPOVER_DEPTH_BASIS: &str = "the detector's min_valley_depth — the shallowest rest it will report, \
     where the cutter's engaged width is narrowest";

/// Claims-pipeline telemetry (design doc §2.1, R2 "pencil over-claiming").
/// `None` on [`UnifiedFinishReport::claims`] when the claims pipeline never
/// ran (`claims: None` at the call site).
#[derive(Debug, Clone, Copy, Default)]
pub struct ClaimsReport {
    pub territory_mode: ClaimTerritoryMode,
    /// Claimed cut paths emitted (centerline + width-capped offset passes,
    /// summed across every crease with `corridor.is_some()`).
    pub crease_path_count: usize,
    /// Total cutting length (mm) of the crease node's `FinishingCut` moves.
    pub crease_path_length_mm: f64,
    /// `RestFieldReport::coverage()` — traced skeleton fraction that
    /// survived the length gate. Independent of banding territory.
    pub detector_coverage: f64,
    /// S4 rest-territory confinement telemetry (`ClaimsConfig::
    /// territory_clip` doc): covered classification cells removed from
    /// `covered` by the pre-decompose mask-AND (measured-skippable cells;
    /// untrusted cells always stay). Always 0 when `territory_clip` is
    /// `false` or no `territory_stock` was supplied.
    pub territory_masked_cells: usize,
    /// `territory_masked_cells × cell²` (mm²) — the coverage area the
    /// mask-AND handed back to the prior pass. Always 0.0 under the same
    /// conditions.
    pub territory_masked_area_mm2: f64,
    /// Region count that actually reaches Step 4's per-region generation
    /// loop — `planned.regions.len()` after decompose (over the possibly
    /// mask-ANDed coverage).
    pub post_territory_region_count: usize,
    /// PR-6a (H2.3): the offset stepover (mm) the claims pipeline DERIVED
    /// from [`crate::surface::reach::suggested_offset_stepover_mm`] and used for both
    /// the routing criterion and the emitted fan. Not a dial — this is the
    /// only place the number is visible.
    pub offset_stepover_mm: f64,
    /// Depth (mm) the reach policy was evaluated at to produce
    /// [`Self::offset_stepover_mm`]. See [`CLAIMS_STEPOVER_DEPTH_BASIS`].
    pub offset_stepover_reference_depth_mm: f64,
    /// What the RETIRED envelope rule (`envelope_radius_mm() * 0.5`) would
    /// have produced on this cutter. Equal to [`Self::offset_stepover_mm`]
    /// on any plain ball — the migration moves tapered tools only.
    pub envelope_rule_stepover_mm: f64,
}

/// Which resolved height clipped a band's Z range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeightClip {
    /// `ResolvedHeights::bottom_z` raised the ladder's floor.
    BottomZ,
    /// `ResolvedHeights::top_z` lowered the ladder's ceiling.
    TopZ,
}

impl HeightClip {
    /// Stable token used in findings, narration and diagnostics.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::BottomZ => "bottom_z",
            Self::TopZ => "top_z",
        }
    }
}

/// One planned band region whose cutting was entirely erased by height
/// resolution (Wave D1, ledger task #15).
///
/// Recorded ONLY when the region emitted no cutting at all AND the resolved
/// heights are what took the levels away — a partially clipped band that
/// still cuts is not reported here, and a band that plans nothing on its own
/// (no covered cells) is a decomposition outcome, not a height one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DroppedBand {
    pub band: FinishBand,
    /// Index into the decomposition's `planned.regions`.
    pub region_index: usize,
    /// XY-projected area (mm²) of the planned band polygon —
    /// [`UnifiedFinishReport::provenance`] describes it.
    pub area_mm2: f64,
    /// Z levels the band's OWN surface span would have laddered.
    pub planned_levels: usize,
    /// Z levels that survived height resolution.
    pub resolved_levels: usize,
    /// The resolved height (mm) that clipped it.
    pub clip_z_mm: f64,
    /// Which height that was.
    pub clip: HeightClip,
}

/// What the resolved heights did to ONE band's Z ladder — the raw
/// measurement both [`DroppedBand`] and [`ClippedBand`] are cut from.
///
/// C8 widened this from the Wave-D1 four-tuple to carry the Z BOUNDS as
/// well as the level counts, because "requested vs delivered heights" is
/// what an operator can act on: level counts say how much was lost, the
/// bounds say where.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BandHeightClip {
    /// Which resolved height bit.
    pub clip: HeightClip,
    /// Its value (mm) — the number in the operator's heights config.
    pub clip_z_mm: f64,
    /// Z levels the band's OWN surface span would have laddered.
    pub planned_levels: usize,
    /// Z levels that survived height resolution.
    pub resolved_levels: usize,
    /// Ceiling the band's own surface span asked for (mm).
    pub requested_top_z_mm: f64,
    /// Floor the band's own surface span asked for (mm).
    pub requested_bottom_z_mm: f64,
    /// Ceiling the resolved heights allowed (mm).
    pub delivered_top_z_mm: f64,
    /// Floor the resolved heights allowed (mm).
    pub delivered_bottom_z_mm: f64,
}

impl BandHeightClip {
    /// Vertical extent (mm) the clamps removed from this band's ladder.
    #[must_use]
    pub fn lost_height_mm(&self) -> f64 {
        let requested = (self.requested_top_z_mm - self.requested_bottom_z_mm).max(0.0);
        let delivered = (self.delivered_top_z_mm - self.delivered_bottom_z_mm).max(0.0);
        (requested - delivered).max(0.0)
    }
}

/// One planned band region whose Z ladder was SHORTENED by height
/// resolution but which still emitted cutting (C8).
///
/// The complement of [`DroppedBand`], and the gap `ANTIPATTERNS_BACKLOG.md`
/// P8 logged: "Partial height clipping is unreported (only total band
/// collapse produces the D1 finding)." Wave D1 measured the clip on every
/// band and then threw the measurement away unless the region emitted
/// nothing at all — reasoning, correctly, that a total collapse must not be
/// buried under partial ones. That is an argument about SEVERITY, not about
/// whether to report: a band that machined the top 2 mm of a 12 mm wall and
/// stopped is an unfinished feature, and it was silent.
///
/// So the two travel in separate collections with separate severities: a
/// dropped band is a `Caution` ("this feature will be UNMACHINED"), a
/// clipped band is `Info` ("this feature is PARTLY machined, here is what
/// was left").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClippedBand {
    pub band: FinishBand,
    /// Index into the decomposition's `planned.regions`.
    pub region_index: usize,
    /// XY-projected area (mm²) of the planned band polygon —
    /// [`UnifiedFinishReport::provenance`] describes it.
    pub area_mm2: f64,
    /// The clip itself: which height, its value, the level counts and the
    /// requested-vs-delivered Z bounds.
    pub clip: BandHeightClip,
}

/// Fold [`UnifiedFinishReport::clipped_bands`] into the single Copy finding
/// [`crate::compute::config::ToolpathStats`] carries. `None` when no band
/// was partially clipped.
///
/// Aggregates the same way [`dropped_band_finding`] does — area and count
/// over every clipped region, band token and clipping height from the
/// largest-area one — so the two messages read alike and can be compared.
#[must_use]
pub fn clipped_band_finding(
    report: &UnifiedFinishReport,
) -> Option<crate::compute::config::ClippedBandFinding> {
    let worst = report
        .clipped_bands
        .iter()
        .copied()
        .reduce(|a, b| if b.area_mm2 > a.area_mm2 { b } else { a })?;
    Some(crate::compute::config::ClippedBandFinding {
        band_label: RegionKind::Band(worst.band).band_label(),
        region_count: report.clipped_bands.len(),
        area_mm2: report.clipped_bands.iter().map(|c| c.area_mm2).sum(),
        clip_z_mm: worst.clip.clip_z_mm,
        clip_label: worst.clip.clip.label(),
        bands_measured: report.height_clip_measured,
        requested_top_z_mm: worst.clip.requested_top_z_mm,
        requested_bottom_z_mm: worst.clip.requested_bottom_z_mm,
        delivered_top_z_mm: worst.clip.delivered_top_z_mm,
        delivered_bottom_z_mm: worst.clip.delivered_bottom_z_mm,
        planned_levels: worst.clip.planned_levels,
        resolved_levels: worst.clip.resolved_levels,
        max_lost_height_mm: report
            .clipped_bands
            .iter()
            .map(|c| c.clip.lost_height_mm())
            .fold(0.0_f64, f64::max),
        provenance: report.provenance,
    })
}

/// Fold [`UnifiedFinishReport::dropped_bands`] into the single Copy finding
/// [`crate::compute::config::ToolpathStats`] carries. `None` when nothing
/// was dropped.
///
/// Area and region count aggregate over EVERY dropped region; the band token
/// and the clipping height name the largest-area one, so the message points
/// at the feature an operator will actually go looking for.
#[must_use]
pub fn dropped_band_finding(
    report: &UnifiedFinishReport,
) -> Option<crate::compute::config::DroppedBandFinding> {
    let worst = report
        .dropped_bands
        .iter()
        .copied()
        .reduce(|a, b| if b.area_mm2 > a.area_mm2 { b } else { a })?;
    Some(crate::compute::config::DroppedBandFinding {
        band_label: RegionKind::Band(worst.band).band_label(),
        region_count: report.dropped_bands.len(),
        area_mm2: report.dropped_bands.iter().map(|d| d.area_mm2).sum(),
        clip_z_mm: worst.clip_z_mm,
        clip_label: worst.clip.label(),
        bands_measured: report.height_clip_measured,
        provenance: report.provenance,
    })
}

/// Orchestration report: decomposition stats + what each band generated +
/// the P2.d route.
#[derive(Debug, Clone, Default)]
pub struct UnifiedFinishReport {
    pub decompose: crate::finish::finish_planner::DecomposeStats,
    pub very_steep: BandGenStats,
    pub mid_steep: BandGenStats,
    pub shallow: BandGenStats,
    /// Claims-pipeline telemetry (v3 S1) — `None` when `claims` was not
    /// supplied to the call.
    pub claims: Option<ClaimsReport>,
    /// One entry per routed node (band regions in stitch order, then the
    /// trailing crease node if any) — the Region-span source table (design
    /// doc §2.4).
    pub region_table: Vec<RegionTableEntry>,
    /// Region cut order (indices into the decomposition's
    /// `planned.regions`). Steep-first band-major when no
    /// [`LinkKinematics`] was supplied; greedy link-costed otherwise.
    pub route: Vec<usize>,
    /// The junction decisions, `route.len().saturating_sub(1)` entries —
    /// empty when routing ran without kinematics (no costing happened).
    pub links: Vec<RoutedLink>,
    /// The claims detector's continuous rest field (design doc §2.4:
    /// carried through so the GUI heatmap and probes can see the op's OWN
    /// territory evidence). `None` when claims didn't run.
    pub rest_grid: Option<std::sync::Arc<crate::surface::rest_field::RestGrid>>,
    /// The claims detector's rest-region polygons (the
    /// `DerivedRestRegions` source shape). `None` when claims didn't run.
    pub rest_regions: Option<std::sync::Arc<Vec<Polygon2>>>,
    /// Intra-region stay-down linking totals, summed over every region
    /// (`UnifiedFinishParams::intra_region_hookup_mm`). All zero when the
    /// pass is disabled.
    pub relink: RelinkTotals,
    /// Region-interior area (mm²) left UNCUT by a mid-steep scallop ring
    /// cascade that hit `max_rings` before collapsing, summed over every
    /// mid-steep region. `0.0` when every cascade collapsed normally.
    /// See [`crate::finish::scallop::ScallopReport::uncut_core_mm2`].
    ///
    /// **Different provenance from [`Self::provenance`]**: this one is
    /// [`crate::finish::scallop::ScallopReport::PROVENANCE`] — a ring-cascade
    /// residual, not a grid-quantised band area. The two must never be
    /// summed or ratio'd against each other.
    pub uncut_core_mm2: f64,
    /// M4 §5b: the hole-aware sibling of [`Self::uncut_core_mm2`], summed
    /// over the same mid-steep regions. `0.0` when every cascade collapsed
    /// normally. See [`crate::finish::scallop::ScallopReport::untouched_mm2`] and
    /// [`crate::finish::scallop::ScallopReport::UNTOUCHED_PROVENANCE`].
    pub untouched_mm2: f64,
    /// M4 §5b: the ESTIMATED reached-but-dropped sibling of
    /// [`Self::uncut_core_mm2`], summed over the same mid-steep regions. See
    /// [`crate::finish::scallop::ScallopReport::standing_mm2`] and
    /// [`crate::finish::scallop::ScallopReport::STANDING_PROVENANCE`] for the
    /// estimator's formula and stated limitations.
    pub standing_mm2: f64,
    /// Wave D1: planned band regions whose cutting was entirely erased by
    /// height resolution (see [`DroppedBand`]). Empty on a healthy run.
    /// Folded into the per-toolpath finding by [`dropped_band_finding`].
    pub dropped_bands: Vec<DroppedBand>,
    /// C8: planned band regions whose Z ladder was SHORTENED by height
    /// resolution but which still cut (see [`ClippedBand`]). Empty on a
    /// healthy run. Folded into the per-toolpath finding by
    /// [`clipped_band_finding`]. Disjoint from [`Self::dropped_bands`] by
    /// construction — a region appears in exactly one of them, or neither.
    pub clipped_bands: Vec<ClippedBand>,
    /// FIN-14: which bands compared their own Z span against the resolved
    /// heights on this run.
    ///
    /// Empty means no band did, and it never means "no band was clipped":
    /// [`Self::dropped_bands`] and [`Self::clipped_bands`] can only speak
    /// for the bands in this set. Only the `VerySteep` arm reads `top_z` and
    /// `bottom_z`, so only that arm can enter the set today — see
    /// [`crate::compute::config::MeasuredBands`].
    pub height_clip_measured: crate::compute::config::MeasuredBands,
    /// Wave D1: tip float measured on the crease node's centrelines.
    /// `None` when the claims pipeline never ran, so no centrelines existed
    /// to measure — never read as "nothing floated".
    pub tip_float: Option<crate::compute::config::TipFloatFinding>,
    /// What the AREA fields of [`Self::region_table`] mean (M1) — copied from
    /// the decomposition that produced them, so a consumer never has to guess
    /// which grid or which conditioning stage an `area_mm2` came from.
    ///
    /// Scope: `region_table[..].area_mm2` and anything derived from the band
    /// polygons. It does NOT describe [`Self::uncut_core_mm2`] (see there) or
    /// the length/count fields, which carry their own units.
    pub provenance: crate::measurement::MeasurementProvenance,
    /// C2 monotone-cell decomposition telemetry.
    ///
    /// **`None` = the pass did not run** — `monotone_cell_decomposition` was
    /// off, or this op emitted no Shallow region at all. `Some` with
    /// `membership_fallbacks == 0` is measured-clean. Never coerce the
    /// absent value to zero (X6 / the `ToolpathStats` contract).
    pub monotone_cells: Option<MonotoneCellTotals>,
    /// Honest-raster derates (Track B, 2026-09-01): one entry per Shallow
    /// region whose raster stepover was tightened by `cos(theta_max)`.
    /// Empty = no Shallow region carried slope above
    /// [`SHALLOW_DERATE_MIN_SLOPE_DEG`], so every region used the
    /// configured `raster_stepover` unchanged. Folded into
    /// `ToolpathStats::derived_stepovers` by the op adapter.
    pub shallow_slope_derates: Vec<ShallowSlopeDerate>,
}

/// One Shallow region's honest-raster stepover derate (Track B fix).
///
/// The shipped Shallow raster spaces its passes in XY projection, so on a
/// slope `theta` the achieved surface spacing is `s_XY / cos(theta)` —
/// wider than the configured value. The fix derates the effective XY
/// stepover by `cos(theta_max)` of the region, which lands the achieved
/// surface spacing at the configured value on the region's worst slope
/// (`planning/honest_raster_2026-09-01/FINDINGS.md`). The operator's
/// `raster_stepover` dial is not rewritten; this record is the audit trail
/// for the derived value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShallowSlopeDerate {
    /// Index into the decomposition's `planned.regions`.
    pub region_index: usize,
    /// The maximum slope (deg) the region's covered cells carry, read from
    /// the classification slope map and clamped to the planner's steep
    /// threshold (see `shallow_region_max_slope_deg`).
    pub slope_max_deg: f64,
    /// The operator's `raster_stepover` (mm), unchanged.
    pub configured_stepover_mm: f64,
    /// The stepover (mm) the region's lattice was actually built at:
    /// `configured_stepover_mm * cos(slope_max_deg)`.
    pub derated_stepover_mm: f64,
}

/// What the C2 shallow-band decomposition did, summed over every Shallow
/// region of one operation.
///
/// Report-only: no gate consumes it, and recording it changes no geometry.
///
/// `Serialize` because this whole struct travels on the diagnostic wires
/// (CLI `tp_*.json`, MCP `get_diagnostics`) as ONE object rather than being
/// flattened into loose counters — the five numbers are only interpretable
/// together, and `regions` is the denominator of the other four.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct MonotoneCellTotals {
    /// Shallow regions the decomposition was attempted on.
    pub regions: usize,
    /// Of those, how many cleared
    /// [`crate::geometry::monotone_cells::ELONGATION_GATE`] and were decomposed AND
    /// rastered in their own PCA-minor frame.
    pub regions_rotated: usize,
    /// Cells whose raster was emitted, summed across regions.
    pub cells_emitted: usize,
    /// Regions where the reconstructed cells did **not** select the same
    /// lattice population as the undivided region, so the band fell back to
    /// the undivided raster for that region.
    ///
    /// The fallback is the safe arm: a cell set that loses a lattice point
    /// leaves material uncut. A non-zero count here is a defect report about
    /// the decomposition, not about the part — the emitted path is the
    /// pre-C2 one.
    pub membership_fallbacks: usize,
    /// Regions where no cell polygon could be reconstructed at all (an
    /// empty region, or a marching-squares extraction that produced no
    /// loop), so the band fell back to the undivided raster.
    pub empty_fallbacks: usize,
}

/// Summed [`crate::finish::surface_link::RelinkReport`] counters across regions.
///
/// Every DECLINE reason the relinker distinguishes is carried, because the
/// question this record has to answer is not "did linking happen" but "why
/// didn't it": T4 measured **19,132 intra-node retract round trips** on an op
/// whose relink was ON at 6.0 mm hookup
/// (`planning/multitool_2026-08-23/ORCHESTRATION_PLAN.md` §0), and nothing
/// on any surface said which of the five refusals produced them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RelinkTotals {
    pub fragments: usize,
    pub surface_links: usize,
    /// TIER (a) — links that arrive at cutting depth, so the next fragment
    /// needs no entry at all. The ACCEPTANCE measure for G-LINKSTAGE: on an
    /// entry-bound pass this is the only counter that moves the wall clock.
    /// See [`crate::finish::surface_link::RelinkReport::at_depth_links`].
    pub at_depth_links: usize,
    /// TIER (b) — links that lifted to the local stock ceiling and descended
    /// again. They remove a RETRACT, not an entry.
    pub clearance_hops: usize,
    /// Closed-loop fragments the stage rotated to start near the previous
    /// exit. `0` for a family that declares no fragment kinds.
    pub rotated_loops: usize,
    pub retract_links: usize,
    pub too_far: usize,
    pub off_surface: usize,
    pub slower_than_retract: usize,
    pub outside_boundary: usize,
    /// Junctions where a [`crate::finish::surface_link::LinkCeiling`] put the
    /// clearance at or above `safe_z`, so the retract was kept. Structurally
    /// `0` for every op generated without a ceiling — see
    /// [`crate::finish::surface_link::RelinkReport::ceiling_above_safe_z`].
    pub ceiling_above_safe_z: usize,
}

impl RelinkTotals {
    /// Fraction of junctions kept on the surface. `None` when the pass
    /// never ran or the op had a single fragment per region.
    pub(crate) fn link_rate(&self) -> Option<f64> {
        let junctions = self.surface_links + self.retract_links;
        (junctions > 0).then(|| self.surface_links as f64 / junctions as f64)
    }

    /// Junctions the relinker considered and declined, by any reason. Equal
    /// to [`Self::retract_links`] on a pass whose every retract was a
    /// considered junction; smaller when some retracts were structural (the
    /// first fragment's approach and the closing retract are not junctions).
    #[must_use]
    pub const fn declined(&self) -> usize {
        self.too_far
            + self.off_surface
            + self.slower_than_retract
            + self.outside_boundary
            + self.ceiling_above_safe_z
    }

    /// Fold one region's report into the running totals. One site, so a new
    /// counter on [`crate::finish::surface_link::RelinkReport`] is added here rather
    /// than in however many hand-written `+=` blocks exist.
    pub fn add(&mut self, rep: &crate::finish::surface_link::RelinkReport) {
        self.fragments += rep.fragments;
        self.surface_links += rep.surface_links;
        self.at_depth_links += rep.at_depth_links;
        self.clearance_hops += rep.clearance_hops;
        self.rotated_loops += rep.rotated_loops;
        self.retract_links += rep.retract_links;
        self.too_far += rep.too_far;
        self.off_surface += rep.off_surface;
        self.slower_than_retract += rep.slower_than_retract;
        self.outside_boundary += rep.outside_boundary;
        self.ceiling_above_safe_z += rep.ceiling_above_safe_z;
    }
}

/// Build the span vector for a `unified_finish_toolpath_with_cancel`
/// result. Extracted so the op adapter (`compute::execute::
/// generate_unified_finish`) and the capability sentries share ONE
/// definition — the spans carry this op's rapid-order barriers, so a test
/// that rebuilt them by hand would silently stop testing production the
/// moment either side drifted.
///
/// Layers, outermost first:
/// 1. `Operation` + the per-event `Region` spans (scallop rings).
/// 2. Node-level `Region` spans, one per [`RegionTableEntry`], with
///    `region_id` = index into `report.region_table`. Spliced in right
///    after `Operation` so `span_path_at` lists coarse ancestors first.
///    These are a SEPARATE `region_id` space from the scallop-event spans
///    — consumers disambiguate with
///    [`crate::trace::toolpath_spans::RegionSpanRole`] (`Node` here,
///    `GeneratorPass` for the ring spans), never by parsing the label.
/// 3. `RapidOrderBarrier`s at each node start, plus per-Z barriers inside
///    waterline (VerySteep) nodes. See
///    [`crate::compute::spans::region_node_barriers`].
pub fn unified_finish_spans(
    toolpath: &Toolpath,
    annotations: &[ScallopRuntimeAnnotation],
    report: &UnifiedFinishReport,
) -> Vec<crate::trace::toolpath_spans::Span> {
    use crate::compute::spans::{RegionNode, region_node_barriers, spans_from_labeled_events};
    use crate::trace::toolpath_spans::{RegionSpanRole, Span, SpanKind, SpanPayload};

    let mut spans = spans_from_labeled_events(
        toolpath.moves.len(),
        annotations
            .iter()
            .map(|ann| (ann.move_index, ann.event.label())),
    );

    // A/M8: labels and ranges come from the shared region-annotation table
    // the SEMANTIC trace also reads, so the structural and semantic region
    // systems cannot drift apart.
    let node_spans: Vec<Span> = unified_finish_region_annotations(report)
        .into_iter()
        .map(|region| {
            let label = region.span_label();
            Span::new(
                region.move_range.start,
                region.move_range.end,
                SpanKind::Region,
            )
            .with_label(label)
            .with_payload(SpanPayload::Region {
                region_id: region.region_id,
                // Wave D3: the discriminator. `region_id` here indexes
                // `report.region_table`, NOT the scallop-ring id space the
                // event spans above use.
                role: RegionSpanRole::Node,
            })
        })
        .collect();
    let insert_at = 1.min(spans.len());
    spans.splice(insert_at..insert_at, node_spans);

    let nodes: Vec<RegionNode> = report
        .region_table
        .iter()
        .map(|entry| RegionNode {
            move_range: entry.move_range.clone(),
            // Only the VerySteep band ladders in Z (it routes to
            // `waterline_toolpath_with_cancel`). MidSteep scallop rings and
            // Shallow raster rows are single-pass over a height field, so
            // their runs are materially independent of visiting order.
            depth_ordered: matches!(entry.kind, RegionKind::Band(FinishBand::VerySteep)),
        })
        .collect();
    spans.extend(region_node_barriers(toolpath, &nodes));

    spans
}

// ── Resolution policy selection (H3 step 2) ──────────────────────────────

/// The resolution policy UnifiedFinish CLASSIFIES on.
///
/// UnifiedFinish selects `FinishResolutionMode::CuspQuarter` —
/// `(cusp_radius/4).max(tolerance)`, the tip scale — which is what
/// `finish_setup::build_classification_surface_with_cancel` computed
/// internally before H3 step 1. Do not coarsen this to the envelope: §14q
/// measured the tapered case, where a shank-derived classification cell made
/// wanaka's steep ribbons unrepresentable and the decomposition emitted ZERO
/// VerySteep regions.
#[must_use]
pub fn unified_finish_classification_resolution(
    cutter: &dyn MillingCutter,
    tolerance: f64,
) -> FinishResolutionPolicy {
    FinishResolutionPolicy::cusp_quarter(cutter, tolerance)
}

/// The resolution policy UnifiedFinish's MidSteep band GENERATES on.
///
/// UnifiedFinish does not build a generation grid itself: each band delegates
/// to an existing strategy generator, and only the MidSteep band's scallop
/// generator builds one. So UnifiedFinish's generation resolution IS
/// [`crate::finish::scallop::scallop_generation_resolution`] — this function names
/// that inheritance rather than re-deriving it, so moving scallop's policy
/// (H3 step 4) moves UnifiedFinish's MidSteep band with it, by construction.
///
/// The other bands have no finish grid: VerySteep runs waterline (contours
/// straight off the mesh), Shallow runs a drop-cutter grid at its own raster
/// stepover, and pencil/crease paths come from the rest field. H3 step 5
/// ("re-run UnifiedFinish only after standalone strategy behavior is
/// understood") therefore reduces to: understand scallop.
#[must_use]
pub fn unified_finish_mid_steep_generation_resolution(
    cutter: &dyn MillingCutter,
    tolerance: f64,
) -> FinishResolutionPolicy {
    crate::finish::scallop::scallop_generation_resolution(cutter, tolerance)
}

// ── Orchestrator ─────────────────────────────────────────────────────────

/// Decompose the surface into bands, generate each REGION's toolpath with
/// the appropriate EXISTING strategy generator, route the regions (greedy
/// nearest-by-integrated-link-time when `link_kinematics` is `Some`;
/// steep-first band-major otherwise), and stitch them with the winning
/// link candidate emitted at each junction.
///
/// The intra-region relink rides the mesh surface. That is only correct when
/// the mesh IS the material; a `FromRemainingStock` op has standing stock
/// above it wherever nothing has cut yet. Callers that hold that op's input
/// stock snapshot must use
/// [`unified_finish_toolpath_with_cancel_and_ceiling`] — this entry point is
/// the fresh-stock arm, and is defined as that one with no ceiling so the two
/// cannot drift.
#[allow(clippy::too_many_arguments)] // op-generator adapter surface, mirrors the strategy fns it composes
pub fn unified_finish_toolpath_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    top_z: f64,
    bottom_z: f64,
    params: &UnifiedFinishParams,
    planner: &FinishPlannerParams,
    machining_boundary: Option<&RegionSet<'_>>,
    link_kinematics: Option<&LinkKinematics>,
    // v3 S1 claims pipeline (module doc). `None` is a byte-identical no-op.
    claims: Option<&ClaimsConfig<'_>>,
    debug: Option<&ToolpathDebugContext>,
    cancel: &dyn CancelCheck,
) -> Result<(Toolpath, Vec<ScallopRuntimeAnnotation>, UnifiedFinishReport), Cancelled> {
    unified_finish_toolpath_with_cancel_and_ceiling(
        mesh,
        index,
        cutter,
        top_z,
        bottom_z,
        params,
        planner,
        machining_boundary,
        link_kinematics,
        claims,
        debug,
        None,
        cancel,
    )
}

/// [`unified_finish_toolpath_with_cancel`] plus a stock-aware ceiling for the
/// INTRA-REGION relink (Phase O item 3, `ORCHESTRATION_PLAN.md`).
///
/// `link_ceiling: None` is the fresh-stock arm and is byte-identical to the
/// entry point above — `tests/island_stay_down_links_o3.rs` asserts that
/// rather than leaving it to the delegation's shape.
///
/// `Some(ceiling)` makes every intra-region link leave the cut vertically,
/// traverse at `max(mesh surface, standing material) + PLUNGE_CLEARANCE_MM`,
/// and re-enter vertically — and refuses the link outright (keeping the
/// retract) once that clearance reaches `safe_z`, counted on
/// [`RelinkTotals::ceiling_above_safe_z`]. See [`crate::finish::surface_link::LinkCeiling`]
/// for why a surface-riding link on standing stock is a cutting feed through
/// material (the G-LINKLOAD class).
///
/// The ceiling is deliberately NOT threaded through [`ClaimsConfig`], which
/// also carries a stock: that one is `Some` only when `pencil_claims` is on,
/// and whether a link cuts through standing material has nothing to do with
/// whether the crease detector ran.
#[allow(clippy::too_many_arguments)] // op-generator adapter surface, mirrors the strategy fns it composes
pub fn unified_finish_toolpath_with_cancel_and_ceiling(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    top_z: f64,
    bottom_z: f64,
    params: &UnifiedFinishParams,
    planner: &FinishPlannerParams,
    machining_boundary: Option<&RegionSet<'_>>,
    link_kinematics: Option<&LinkKinematics>,
    // v3 S1 claims pipeline (module doc). `None` is a byte-identical no-op.
    claims: Option<&ClaimsConfig<'_>>,
    debug: Option<&ToolpathDebugContext>,
    link_ceiling: Option<crate::finish::surface_link::LinkCeiling<'_>>,
    cancel: &dyn CancelCheck,
) -> Result<(Toolpath, Vec<ScallopRuntimeAnnotation>, UnifiedFinishReport), Cancelled> {
    check_cancel(cancel)?;

    // ── Step 1: classify on the TRUE surface ────────────────────────────
    // P2.b decision (`finish_setup::build_classification_surface_with_cancel`
    // doc): a ball tool's offset (generation) surface geometrically hides
    // steepness at feature scales at/below the ball radius. Classification
    // MUST read the true surface or band assignment collapses to
    // all-shallow on relief at that scale — never swap this for
    // `build_finish_surface_with_cancel`.
    // The RESOLUTION is UnifiedFinish's own choice (H3 step 2) — see
    // `unified_finish_classification_resolution`.
    // The SAMPLER is `params.classification_sampler` (M3 wave 7b), defaulting
    // to `ClassificationSampler::PRODUCTION`. It is threaded rather than
    // pinned here so the COLUMNS A/B can drive both classifiers through this
    // exact code path.
    let surface = build_classification_surface_with_sampler_and_cancel(
        mesh,
        index,
        cutter,
        unified_finish_classification_resolution(cutter, params.tolerance),
        params.classification_sampler,
        cancel,
    )?;
    check_cancel(cancel)?;

    // ── Step 2: coverage ∧ machining boundary ───────────────────────────
    let cols = surface.cols();
    let cell = surface.cell_size();
    let origin_x = surface.slope_map.origin_x;
    let origin_y = surface.slope_map.origin_y;
    // Mutable: the S4 rest-territory confinement (below, after Step 2.5)
    // may AND the per-cell rest verdict into this before decompose —
    // `ClaimsConfig::territory_clip` doc.
    let mut covered: Vec<bool> = surface
        .heightmap
        .covered_flags()
        .iter()
        .enumerate()
        .map(|(i, &cov)| {
            if !cov {
                return false;
            }
            let Some(boundary) = machining_boundary else {
                return true;
            };
            let row = i / cols;
            let col = i % cols;
            let x = origin_x + col as f64 * cell;
            let y = origin_y + row as f64 * cell;
            boundary.contains(&P2::new(x, y))
        })
        .collect();
    check_cancel(cancel)?;

    // ── Step 2.5: in-op rest analysis + territory (v3 S1, design doc §2.1
    // steps 2+4) ──────────────────────────────────────────────────────────
    // `claims: None` leaves `creases` empty and `covered` untouched —
    // byte-identical to the pre-v3 op (module doc).
    let mut claimed_creases = 0usize;
    let mut claims_paths: Vec<crate::finish::pencil::PencilPath> = Vec::new();
    // Wave D1: `None` until the claims pipeline actually emits a centreline,
    // so "the claims pass never ran" stays distinguishable from "it ran and
    // nothing floated".
    let mut claims_tip_float: Option<crate::compute::config::TipFloatFinding> = None;
    let mut claims_report: Option<ClaimsReport> = None;
    let mut claims_rest_grid: Option<std::sync::Arc<crate::surface::rest_field::RestGrid>> = None;
    let mut claims_rest_regions: Option<std::sync::Arc<Vec<Polygon2>>> = None;
    if let Some(cfg) = claims {
        check_cancel(cancel)?;
        // PR-6a (H2.3): the crease/pencil fan's stepover comes from the
        // CANONICAL REACH POLICY, not from the cutter envelope. See
        // `crate::surface::reach::suggested_offset_stepover_mm` for the derivation and
        // why the shank-scaled predecessor (`envelope_radius_mm() * 0.5` —
        // 1.5 mm on the shipped Ø1-tip taper) could not describe passes the
        // tip cuts.
        //
        // REFERENCE DEPTH: `min_valley_depth`, the SHALLOWEST rest the
        // detector will report. `working_half_width_mm` is monotone
        // non-decreasing in depth, so this is the narrowest band any emitted
        // pass works — the conservative end, and the only depth in scope
        // before the detector has run: the ROUTING decision below
        // (`rf_params.offset_stepover_mm`) partitions cells before any
        // centreline exists, so it has no per-point depth to read yet and
        // stays keyed off this one scalar.
        //
        // EMISSION no longer shares that limitation (C9). A per-point
        // stepover — the same generalisation `reach`'s module doc records
        // for the cross-section — used to need the fan to stop being one
        // scalar; `centerline_cut_paths` now sizes
        // `suggested_offset_stepover_mm(cutter, depth_i)` at EVERY sampled
        // point of a measured centreline (every centreline `detect_rest_valleys`
        // returns carries real per-point samples), so `claims_offset_stepover_mm`
        // below now does only the two jobs described above — routing and the
        // unmeasured-centreline fallback — and no longer also determines how
        // far apart the emitted passes actually sit.
        let claims_offset_stepover_mm = crate::surface::reach::suggested_offset_stepover_mm(
            cutter,
            cfg.rest_field_params.min_valley_depth,
        );
        let rf_params = RestFieldParams {
            cell_mm: cfg.rest_field_params.cell_mm,
            min_valley_depth: cfg.rest_field_params.min_valley_depth,
            // Coverage routing (PR-5): the same fan the crease emission
            // below actually uses — the policy stepover derived above and the
            // 4-pass cap of design doc §2.1 item 5. Routing and emission must
            // see one fan, so both read these two bindings and nothing else.
            offset_stepover_mm: claims_offset_stepover_mm,
            num_offset_passes_cap: CLAIMS_OFFSET_PASS_CAP,
            min_cut_length: cfg.rest_field_params.min_cut_length,
            region_margin_mm: cfg.rest_field_params.region_margin_mm,
        };
        // Crease detection defaults to the analytic self-probe — geometric
        // valleys of the design surface, the thing pencil corridors are
        // FOR — but honors `ClaimsConfig::crease_reference` (build-list
        // item 3): opting into `MachinedStock` swaps in the machined prior
        // stock as the reference instead, sanctioned ONLY when that stock
        // is FINISH-QUALITY (a cascade's Op B following Op A's own ball
        // pass, the R2-validated arm) — never on a rough→finish chain,
        // where the S1 A/B proved the same swap reads roughing terraces as
        // a phantom dendritic crease network. See `CreaseReference` doc.
        let probe = crate::tool::BallEndmill::new(
            crate::finish::pencil::SURFACE_PROBE_BALL_DIAMETER_MM,
            crate::finish::pencil::SURFACE_PROBE_BALL_LENGTH_MM,
        );
        let self_probe_reference = || RestReference::Cutter {
            tool: &probe,
            is_surface_probe: true,
        };
        let crease_reference = match cfg.crease_reference {
            CreaseReference::SelfProbe => self_probe_reference(),
            CreaseReference::MachinedStock => match cfg.territory_stock {
                Some(stock) => RestReference::Stock(stock),
                None => {
                    tracing::warn!(
                        "unified_finish: ClaimsConfig::crease_reference is \
                         MachinedStock but no territory_stock is in scope; \
                         falling back to the analytic self-probe"
                    );
                    self_probe_reference()
                }
            },
        };
        let rf = detect_rest_valleys(mesh, index, cutter, crease_reference, &rf_params);
        check_cancel(cancel)?;

        // Territory = rest islands (design doc §2.1 step 4), measured
        // directly as stock top − pencil drop per classification cell.
        // `rf.rest_grid.surface_z` IS the pencil drop regardless of which
        // `crease_reference` arm fed `rf` — confirmed in
        // `rest_field::detect_rest_valleys`'s sample closure: `surface_z`
        // is populated from the pencil-only drop (`pc.z`) before the
        // `reference` match ever runs, so `RestReference::Stock` and
        // `RestReference::Cutter` populate it identically. No second
        // detector pass is needed under either arm. Untrusted samples keep
        // their coverage (`ClaimsConfig::min_rest_depth_mm` doc).
        let territory_mode = if cfg.territory_stock.is_some() {
            ClaimTerritoryMode::RestIslands
        } else {
            ClaimTerritoryMode::Full
        };
        // Claim ONLY what the pencil actually cut: EMIT FIRST, per
        // centerline, and let a centerline claim territory only when its
        // cut paths materialized. The first fix pre-applied the LENGTH
        // gate as a proxy and the second live validation (2026-07-13)
        // caught it: the analytic float field on wanaka yields a dendritic
        // ridge network whose centerlines pass the length gate, carve the
        // bands into 35 fragments — and then a deeper gate inside the
        // emission chain drops every path. Claim == emitted output is the
        // only carve-and-abandon-proof contract.
        let detected = rf.centerlines.len();
        let mut emitted_paths: Vec<crate::finish::pencil::PencilPath> = Vec::new();
        let mut float = crate::compute::config::TipFloatFinding::default();
        for centerline in rf.centerlines {
            check_cancel(cancel)?;
            let paths = centerline_cut_paths(
                std::slice::from_ref(&centerline),
                mesh,
                index,
                cutter,
                params.sampling,
                // PR-6a: the reach-policy stepover computed above — the SAME
                // binding the detector routed against. Every `centerline`
                // here came from `detect_rest_valleys`, so it carries real
                // per-point `samples` (C9): `centerline_cut_paths` sizes the
                // ACTUAL emitted fan per point from those, and only falls
                // back to this scalar argument for an unmeasured centreline,
                // which none of these are. It stays live as the ROUTING
                // binding above and as that fallback — see the comment
                // above this block.
                //
                // Historical note kept deliberately: the retired value here
                // was `cutter.envelope_radius_mm() * 0.5` (1.5 mm on the
                // wanaka taper), justified by a comment claiming parity with
                // `PencilParams`'s own default. PR-2 established that parity
                // was FALSE — the default is the literal 0.5 mm — so nothing
                // was owed to it (`TOOL_SCALE_SEMANTICS.md` §7.2).
                claims_offset_stepover_mm,
                // Offset-pass cap (design doc §2.1 item 5).
                CLAIMS_OFFSET_PASS_CAP,
                cfg.rest_field_params.min_cut_length,
                params.stock_to_leave,
                &mut float,
                cancel,
            )?;
            if !paths.is_empty() {
                emitted_paths.extend(paths);
                claimed_creases += 1;
            }
        }
        claims_paths = emitted_paths;
        claims_tip_float = Some(float);
        tracing::debug!(
            detected,
            claimed = claimed_creases,
            paths = claims_paths.len(),
            offset_stepover_mm = claims_offset_stepover_mm,
            envelope_rule_stepover_mm = cutter.envelope_radius_mm() * 0.5,
            "unified_finish claims: centerlines whose cut paths materialized"
        );

        claims_report = Some(ClaimsReport {
            territory_mode,
            crease_path_count: 0,
            crease_path_length_mm: 0.0,
            detector_coverage: rf.report.coverage(),
            // Filled in by the S4 mask-AND (below) once it has run.
            territory_masked_cells: 0,
            territory_masked_area_mm2: 0.0,
            post_territory_region_count: 0,
            offset_stepover_mm: claims_offset_stepover_mm,
            offset_stepover_reference_depth_mm: cfg.rest_field_params.min_valley_depth,
            envelope_rule_stepover_mm: cutter.envelope_radius_mm() * 0.5,
        });
        // §2.4 carry-through: the detector's field + region polygons ride
        // the report so the adapter can attach them to the generated
        // toolpath (GUI heatmap, DerivedRestRegions, probes).
        claims_rest_grid = Some(std::sync::Arc::new(rf.rest_grid));
        claims_rest_regions = Some(std::sync::Arc::new(rf.region_polygons));
    }

    // ── Step 2.6: S4 rest-territory confinement (mask-AND) ──────────────
    // `ClaimsConfig::territory_clip` doc: AND a rest keep-mask into
    // `covered` BEFORE decompose, so conditioning normalizes the rest
    // islands themselves (design doc §2.1 step 4's prescribed one-liner;
    // the sparse-pinprick fragmentation lesson and the capped-polygon-clip
    // dead end are documented on the config field).
    //
    // MASK SOURCE (wanaka ×2 run 4, 2026-07-13): NOT a per-cell footprint
    // verdict grid (5-point max-top vs min-drop across TWO grids,
    // measured and since removed — see `unified_v3_design.md` §7/§8).
    // That measurement is deliberately keep-biased for drop-safety, which
    // SATURATES on sloped or textured terrain when used as a mask (the
    // neighborhood z-span alone exceeds any mm-class dial: at 45° a
    // 0.5 mm sample offset reads
    // 0.5 mm of phantom "rest"), so the AND kept ~everything and Op B ran
    // all-over again (54 k s). The honest per-cell quantity is the
    // detector's own stock-referenced `rest` FIELD — a single consistent
    // differencing at the detector's own sample points, the same field
    // the tail probe validated against standing material — thresholded at
    // `min_rest_depth_mm` with NaN keeping (untrusted keeps coverage),
    // DILATED by pencil radius + region margin on the rest grid (EDT,
    // same dilation the detector's own region polygons get) and then
    // nearest-resampled onto the classification grid. Only meaningful
    // under `CreaseReference::MachinedStock` — the self-probe field is
    // geometric float, not material.
    let mut territory_masked_cells = 0usize;
    if let Some(cfg) = claims
        && cfg.territory_clip
    {
        let stock_backed = matches!(cfg.crease_reference, CreaseReference::MachinedStock)
            && cfg.territory_stock.is_some();
        if !stock_backed {
            tracing::warn!(
                "unified_finish S4: territory_clip needs crease_reference = \
                 MachinedStock with a territory_stock in scope (the mask \
                 source is the stock-referenced rest field); skipping \
                 confinement"
            );
        } else if let Some(grid) = claims_rest_grid.as_deref() {
            // keep = NaN ∪ (rest ≥ dial), dilated on the REST grid.
            let keep: Vec<bool> = grid
                .rest
                .iter()
                .map(|&r| r.is_nan() || f64::from(r) >= cfg.min_rest_depth_mm)
                .collect();
            // Dilation = one rest-grid cell (the sampling quantum,
            // bridges speckle) + the user's region-margin reach dial.
            // NOT `cutter.radius()`: for tapered tools that is the SHAFT
            // radius (`MillingCutter::radius` = diameter()/2 = widest
            // cutting point — 3 mm on the Ø1-tip ball), and a 3.5 mm
            // dilation welds a dendritic keep-mask into full coverage
            // (measured, run 5: Op B all-over again at 58.5 k s).
            // Reach-back over the mask edge is separately provided by
            // decompose's own `overlap_mm` dilation at extraction.
            let dilate_mm = grid.cell_mm + cfg.rest_field_params.region_margin_mm;
            let radius_cells = dilate_mm / grid.cell_mm.max(1e-9);
            let dist = crate::geometry::grid_field::distance_transform_2d(&keep, grid.ny, grid.nx);
            let keep_dilated: Vec<bool> = dist.iter().map(|&d| d <= radius_cells).collect();
            // Nearest-resample onto the classification grid and AND.
            for (i, cov) in covered.iter_mut().enumerate() {
                if !*cov {
                    continue;
                }
                let row = i / cols;
                let col = i % cols;
                let x = origin_x + col as f64 * cell;
                let y = origin_y + row as f64 * cell;
                let gc = ((x - grid.origin_x) / grid.cell_mm).round();
                let gr = ((y - grid.origin_y) / grid.cell_mm).round();
                let keep_here =
                    if gc < 0.0 || gr < 0.0 || gc >= grid.nx as f64 || gr >= grid.ny as f64 {
                        // Outside the detector's grid: no evidence — keep
                        // (untrusted keeps coverage).
                        true
                    } else {
                        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
                        let gi = gr as usize * grid.nx + gc as usize;
                        keep_dilated.get(gi).copied().unwrap_or(true)
                    };
                if !keep_here {
                    *cov = false;
                    territory_masked_cells += 1;
                }
            }
            tracing::debug!(
                territory_masked_cells,
                masked_area_mm2 = territory_masked_cells as f64 * cell * cell,
                dilate_mm,
                "unified_finish S4: rest-territory mask-AND applied before decompose"
            );
        }
    }
    if let Some(report) = claims_report.as_mut() {
        report.territory_masked_cells = territory_masked_cells;
        report.territory_masked_area_mm2 = territory_masked_cells as f64 * cell * cell;
    }

    // ── Step 3: decompose ────────────────────────────────────────────────
    // S1: crease claims are ADDITIVE — the pencil node cuts along the
    // detected valleys, but corridors are NOT carved out of the bands
    // (`&[]` below, always). The third live validation (2026-07-13)
    // closed the loop on carving: the pencil's emitted fan is only
    // ~tip-wide on this tool (offset passes don't fit under a Ø6 shank),
    // while corridors claim the full valley half-width — every carved
    // corridor leaves an uncut RING around its centerline, and the
    // fragments shred the conditioned decomposition. Width-honest carving
    // (claim exactly the emitted fan) + region-level conditioning is S2
    // scope; until then bands overlap the crease cut, which costs a
    // little double-cutting along centerlines and can never abandon
    // territory.
    let mut planned = decompose(&surface.slope_map, &covered, &[], planner);
    // `decompose` is handed a bare `SlopeMap` and stamps `Explicit`; this
    // caller knows the grid came from the CLASSIFICATION surface (M1).
    planned.stats.provenance.cell_source = surface.cell_source;
    check_cancel(cancel)?;

    // ── Step 3.4: S4 territory-clip bookkeeping (design doc §2.1 step 4 /
    // §0.a) ──────────────────────────────────────────────────────────────
    // S4's territory confinement happens BEFORE decompose — Step 2.6's
    // mask-AND — so by the time regions exist here they are already
    // conditioned rest islands; nothing is dropped at the region level in
    // this step. R1 guard: make over-fragmentation visible; no hard fail —
    // the A/B gates catch pathology.
    if planned.regions.len() > 256 {
        tracing::warn!(
            post_territory_region_count = planned.regions.len(),
            "unified_finish: post-territory region count exceeds 256; \
             check for over-fragmentation"
        );
    }

    if let Some(report) = claims_report.as_mut() {
        report.post_territory_region_count = planned.regions.len();
    }

    // ── Step 3.5: crease-claims emission (v3 S1, design doc §2.1 step 5) ──
    // The cut paths were already produced in Step 2.5 (claim == emitted
    // output, the carve-and-abandon-proof contract); this step only turns
    // them into the trailing crease node, appended AFTER the routed bands
    // in Step 6, natively linked — not threaded through `route_greedy`
    // (module doc, deferred to S3's fused router).
    let mut crease_tp = Toolpath::new();
    if !claims_paths.is_empty() {
        check_cancel(cancel)?;
        let pencil_params = PencilParams {
            feed_rate: params.feed_rate,
            plunge_rate: params.plunge_rate,
            safe_z: params.safe_z,
            stock_to_leave: params.stock_to_leave,
            sampling: params.sampling,
            link_kinematics: link_kinematics.cloned(),
            // Boundary-less: see `ClaimsConfig::crease_hookup_mm`.
            hookup_distance: claims.map_or(0.0, |c| c.crease_hookup_mm),
            ..PencilParams::default()
        };
        // G-ENTRYLOAD: give crease entries the same bite-budgeted ramp the
        // standalone pencil gets; the claims territory stock is the input
        // stock the entries descend through.
        let (tp, _anns) = crate::finish::pencil::emit_paths_with_entry_stock(
            &claims_paths,
            mesh,
            index,
            cutter,
            &pencil_params,
            claims.and_then(|c| c.territory_stock),
        );
        crease_tp = tp;
    }
    if let Some(report) = claims_report.as_mut() {
        report.crease_path_count = claims_paths.len();
        report.crease_path_length_mm = cutting_length_mm(&crease_tp);
    }

    // ── Step 4: per-region generation ───────────────────────────────────
    // P2.d: one strategy call PER REGION (single-polygon RegionSet) so the
    // router can order regions freely. The raster grid is mesh-global and
    // region-independent, so it is computed once (lazily) and shared by
    // every Shallow region; scallop and waterline rebuild their internal
    // surfaces per call — O(1) regions per band on wanaka today, so this
    // duplication is accepted and flagged as a P2.e datapoint if
    // conditioned region counts ever grow.
    let mut report = UnifiedFinishReport {
        decompose: planned.stats,
        provenance: planned.stats.provenance,
        rest_grid: claims_rest_grid,
        rest_regions: claims_rest_regions,
        tip_float: claims_tip_float,
        ..UnifiedFinishReport::default()
    };
    let mut region_paths: Vec<RegionPath> = Vec::new();
    let mut shallow_grid: Option<DropCutterGrid> = None;
    // C2: stays `None` unless the decomposition actually runs on at least
    // one Shallow region, so absence reads as "not measured" and never as
    // "measured zero" (X6 / the `ToolpathStats` finding contract).
    let mut monotone_cell_totals: Option<MonotoneCellTotals> = None;

    let mut uncut_core_mm2 = 0.0_f64;
    // M4 §5b: accumulated in lockstep with `uncut_core_mm2` below.
    let mut untouched_mm2 = 0.0_f64;
    let mut standing_mm2 = 0.0_f64;
    for (region_index, region) in planned.regions.iter().enumerate() {
        check_cancel(cancel)?;
        let region_set = RegionSet::new(vec![region.polygon.clone()]);
        // Wave D1: set by any band arm whose Z range the resolved heights
        // narrowed. Consulted AFTER generation, because a narrowed range is
        // only a FINDING when nothing came out of it.
        let mut height_clip: Option<BandHeightClip> = None;
        let (tp, anns) = match region.band {
            FinishBand::VerySteep => {
                let Some((band_min_z, band_max_z)) = band_z_range(&surface, &covered, &region_set)
                else {
                    tracing::warn!(
                        region_index,
                        "unified_finish: VerySteep region has no covered cells inside its own polygon; skipping"
                    );
                    continue;
                };
                let start_z = band_max_z.min(top_z);
                let final_z = band_min_z.max(bottom_z);
                // Wave D1 instrument (ledger task #15). `UnifiedFinishConfig`
                // declares `DepthSemantics::None`, so an Auto `bottom_z`
                // resolves to `top_z - 0.0` — the STOCK TOP — and the two
                // `.max`/`.min` clamps above then pin this ladder to the rim
                // of a groove that may be centimetres deep. Compare the
                // levels the band's own surface span asks for against the
                // ones the heights allow; if the difference turns out to
                // have cost the whole region its cutting, say so out loud
                // instead of leaving an unmachined feature with a
                // `region_count += 1` for a headstone.
                let planned_levels =
                    waterline_z_levels(band_max_z, band_min_z, params.z_step).len();
                let resolved_levels = waterline_z_levels(start_z, final_z, params.z_step).len();
                // FIN-14: the comparison happened, whatever it found. The
                // record of WHERE the instrument was taken travels with the
                // finding, so a `MidSteep` or `Shallow` region — whose arm
                // reads neither resolved height — reads as "not measured"
                // and never as a clean band.
                report.height_clip_measured.insert(FinishBand::VerySteep);
                if resolved_levels < planned_levels {
                    // Attribute to whichever clamp actually bit. When both
                    // did, the floor is the one an operator can act on
                    // (a raised `bottom_z` removes the DEEP levels).
                    let (clip, clip_z_mm) = if final_z > band_min_z {
                        (HeightClip::BottomZ, bottom_z)
                    } else {
                        (HeightClip::TopZ, top_z)
                    };
                    // C8: the BOUNDS travel with the level counts. Counts say
                    // how much of the ladder was lost; bounds say where, which
                    // is what an operator can act on.
                    height_clip = Some(BandHeightClip {
                        clip,
                        clip_z_mm,
                        planned_levels,
                        resolved_levels,
                        requested_top_z_mm: band_max_z,
                        requested_bottom_z_mm: band_min_z,
                        delivered_top_z_mm: start_z,
                        delivered_bottom_z_mm: final_z,
                    });
                }
                let wp = WaterlineParams {
                    sampling: params.sampling,
                    feed_rate: params.feed_rate,
                    plunge_rate: params.plunge_rate,
                    safe_z: params.safe_z,
                    // D-16.2 (F3): the VerySteep band honours the dial too.
                    // Before this the field did not exist on
                    // `WaterlineParams` at all, so this arm could not pass
                    // one — the twin defect of the Shallow band's.
                    stock_to_leave: params.stock_to_leave,
                };
                // `waterline_toolpath_with_cancel` ladders start_z..final_z
                // by `z_step` internally — no need to precompute levels.
                // Per-region z-range: each region only ladders the levels
                // its own surface patch spans.
                let tp = waterline_toolpath_with_cancel(
                    mesh,
                    index,
                    cutter,
                    start_z,
                    final_z,
                    params.z_step,
                    &wp,
                    Some(&region_set),
                    cancel,
                )?;
                (tp, Vec::new())
            }
            FinishBand::MidSteep => {
                let sp = ScallopParams {
                    scallop_height: params.scallop_height,
                    tolerance: params.tolerance,
                    direction: ScallopDirection::default(),
                    // Design decision #1: scallop-continuous rings for the
                    // mid-steep band.
                    continuous: true,
                    // Full slope window: the REGION is the confinement now
                    // (the band's polygon already IS the slope-selection),
                    // so scallop's own slope_from/slope_to filter is
                    // deliberately a no-op — see `finish_setup::
                    // {SLOPE_FILTER_MIN_DEG, SLOPE_FILTER_MAX_DEG}`.
                    slope_from: SLOPE_FILTER_MIN_DEG,
                    slope_to: SLOPE_FILTER_MAX_DEG,
                    feed_rate: params.feed_rate,
                    plunge_rate: params.plunge_rate,
                    safe_z: params.safe_z,
                    stock_to_leave: params.stock_to_leave,
                    // A/M7: deliberately OFF here, and not for lack of
                    // value. This band runs `continuous: true` (scallop's
                    // own spiral already chains the contours), and whatever
                    // junctions survive are relinked one level up by
                    // `intra_region_hookup_mm` against the REGION's polygon
                    // — the correct boundary for a band that owns only part
                    // of the surface. Relinking here as well would decide
                    // the same junctions twice, against a weaker boundary.
                    intra_pass_hookup_mm: 0.0,
                    link_kinematics: None,
                };
                // Generation intentionally uses the ball-center OFFSET
                // surface here (`scallop_toolpath_structured_annotated_
                // with_cancel` builds its own under
                // `scallop::scallop_generation_resolution`, mirrored by
                // `unified_finish_mid_steep_generation_resolution`) — only
                // classification (step 1, above) reads the true surface.
                // This is the P2.b "classify true, generate offset" split
                // from the design doc, not an inconsistency.
                let (tp, anns, scallop_report) = scallop_toolpath_structured_annotated_with_cancel(
                    mesh,
                    index,
                    cutter,
                    &sp,
                    debug,
                    Some(&region_set),
                    cancel,
                )?;
                // Summed across every mid-steep region, so a truncated
                // cascade in ANY of them reaches the op's report.
                uncut_core_mm2 += scallop_report.uncut_core_mm2;
                // M4 §5b: the hole-aware and estimator siblings, summed the
                // same way.
                untouched_mm2 += scallop_report.untouched_mm2;
                standing_mm2 += scallop_report.standing_mm2;
                (tp, anns)
            }
            FinishBand::Shallow => {
                // Honest raster (Track B fix, 2026-09-01, ALWAYS ON): the
                // raster spaces its passes in XY projection, so on a slope
                // theta the achieved SURFACE spacing is s_XY / cos(theta) —
                // up to 1.31x the configured scallop spec at 40°
                // (`planning/honest_raster_2026-09-01/FINDINGS.md`). Derate
                // the effective stepover by cos(theta_max) of THIS region,
                // BEFORE any lattice is built, so the derated value flows
                // identically into the undivided raster and the C2 cell
                // decomposition — one frame, one lattice (§0j).
                //
                // theta_max is the total slope, not the cross-feed slope, so
                // the derate is exact on the worst cross-feed slope and
                // conservative elsewhere. On convex ground contact focusing
                // refunds sec(theta) up to ~18°, so the derate is knowingly
                // conservative there too — accepted for v1 (synthesis §1
                // caveat); no curvature-aware refund is built.
                let theta_max_deg = shallow_region_max_slope_deg(
                    &surface,
                    &covered,
                    &region_set,
                    planner.steep_threshold_deg,
                );
                let derated = theta_max_deg > SHALLOW_DERATE_MIN_SLOPE_DEG;
                let step_over_mm = if derated {
                    params.raster_stepover * theta_max_deg.to_radians().cos()
                } else {
                    // cos(0) = 1: a flat region's lattice is bit-identical
                    // to the pre-fix one (it IS the shared memo below).
                    params.raster_stepover
                };
                if derated {
                    report.shallow_slope_derates.push(ShallowSlopeDerate {
                        region_index,
                        slope_max_deg: theta_max_deg,
                        configured_stepover_mm: params.raster_stepover,
                        derated_stepover_mm: step_over_mm,
                    });
                }
                // C2: choose this region's working FRAME before any lattice
                // is touched. Cheap — polygon-only — and it decides whether
                // the shared 0° memo can serve this region at all.
                //
                // `None` is the dial-off arm and reproduces the pre-C2 band
                // exactly: shared grid, one raster call, whole region.
                let frame = if params.monotone_cell_decomposition {
                    Some(crate::geometry::monotone_cells::region_frame(
                        &region.polygon,
                        step_over_mm,
                    ))
                } else {
                    None
                };
                // A gate-passing region gets its OWN whole-mesh lattice at
                // its PCA-minor axis, and it cannot share the memo: the
                // decomposition and the emission must sit on ONE frame
                // (§0j measured the mixed-frame candidate as a cost).
                //
                // The lattice deliberately keeps `batch_drop_cutter`'s
                // whole-mesh origin/phase/step rather than being trimmed to
                // the region bbox — trimming would move the lattice ORIGIN,
                // and §0j priced phase/origin alone at 0.954–0.962×, a cost
                // on its own.
                //
                // What IS clipped to the region is the set of lattice points
                // actually sampled (`region_sampling_window`). Same lattice,
                // same phase, fewer points computed — and fewer points then
                // swept by the decomposition, the membership check and the
                // per-cell rasters, all three of which are O(grid × polygon
                // containment). A whole-mesh lattice per gate-passing region
                // measured ~35 min of generation on the 192-region mt2
                // board against ~2 min dial-off (§7 follow-up 1).
                // A rotated OR derated region gets its OWN lattice: the
                // shared memo is built at the configured stepover and 0°,
                // and a derated region's lattice must carry the derated
                // step. Same whole-mesh origin/phase, region-windowed —
                // the construction the rotated C2 arm already uses.
                let direction_deg = frame.as_ref().map_or(0.0, |f| f.direction_deg);
                let private_grid = if frame.as_ref().is_some_and(|f| f.rotated) || derated {
                    Some(build_shallow_raster_grid(
                        mesh,
                        index,
                        cutter,
                        params,
                        step_over_mm,
                        direction_deg,
                        Some(region_sampling_window(&region.polygon, step_over_mm)),
                        cancel,
                    )?)
                } else {
                    None
                };
                let grid = if let Some(grid) = private_grid.as_ref() {
                    grid
                } else {
                    match shallow_grid.as_ref() {
                        Some(grid) => grid,
                        // NO window: this lattice is the memo SHARED by every
                        // non-rotated, non-derated Shallow region, so it must
                        // cover all of them. Windowing it to whichever region
                        // happened to build it first would blind the rest.
                        None => shallow_grid.insert(build_shallow_raster_grid(
                            mesh,
                            index,
                            cutter,
                            params,
                            params.raster_stepover,
                            0.0,
                            None,
                            cancel,
                        )?),
                    }
                };
                let effective_min_z = mesh.bbox.min.z - 0.1 + params.stock_to_leave;
                // The undivided arm, kept as a closure so every C2 fallback
                // path emits the identical pre-C2 geometry rather than a
                // second transcription of it.
                let undivided = |grid: &DropCutterGrid| {
                    raster_toolpath_from_grid(
                        grid,
                        params.feed_rate,
                        params.plunge_rate,
                        params.safe_z,
                        Some(effective_min_z),
                        Some(&region_set),
                    )
                };
                let tp = match frame {
                    None => undivided(grid),
                    Some(f) => {
                        let decomposed = crate::geometry::monotone_cells::lattice_monotone_cells(
                            grid,
                            &region.polygon,
                            effective_min_z,
                        );
                        check_cancel(cancel)?;
                        let totals = monotone_cell_totals.get_or_insert_default();
                        totals.regions = totals.regions.saturating_add(1);
                        if f.rotated {
                            totals.regions_rotated = totals.regions_rotated.saturating_add(1);
                        }
                        if decomposed.cells.is_empty() {
                            totals.empty_fallbacks = totals.empty_fallbacks.saturating_add(1);
                            tracing::warn!(
                                region_index,
                                topology_cells = decomposed.topology_cells,
                                "unified_finish: monotone decomposition produced no cell \
                                 polygons; emitting the undivided raster for this region"
                            );
                            undivided(grid)
                        } else {
                            // The REFUSE discipline the rig imposed on every
                            // cell arm it costed (`FINDINGS.md` §0g, Stage
                            // J). Both sides sit on this same grid, so it is
                            // an exact population test, not a proxy — and a
                            // cell set that lost a lattice point would be
                            // uncut material, so the disagreement arm falls
                            // back rather than emitting.
                            let mismatches =
                                crate::geometry::monotone_cells::cells_select_same_lattice(
                                    grid,
                                    &region.polygon,
                                    &decomposed.cells,
                                    effective_min_z,
                                );
                            if mismatches > 0 {
                                totals.membership_fallbacks =
                                    totals.membership_fallbacks.saturating_add(1);
                                tracing::warn!(
                                    region_index,
                                    mismatches,
                                    cells = decomposed.cells.len(),
                                    "unified_finish: monotone cells disagree with the \
                                     undivided region on emitted lattice points; emitting \
                                     the undivided raster for this region"
                                );
                                undivided(grid)
                            } else {
                                totals.cells_emitted =
                                    totals.cells_emitted.saturating_add(decomposed.cells.len());
                                // One raster call per cell, on the ONE
                                // shared lattice — §0i's construction. The
                                // relinker downstream then sees cell-shaped
                                // fragments; it keeps `reorder: true`, which
                                // is why no cell TSP is emitted here (§0j
                                // measured a greedy cell order as
                                // byte-identical).
                                let mut out = Toolpath::new();
                                for cell in &decomposed.cells {
                                    let cell_set = RegionSet::new(vec![cell.clone()]);
                                    let cell_tp = raster_toolpath_from_grid(
                                        grid,
                                        params.feed_rate,
                                        params.plunge_rate,
                                        params.safe_z,
                                        Some(effective_min_z),
                                        Some(&cell_set),
                                    );
                                    out.moves.extend(cell_tp.moves);
                                }
                                out
                            }
                        }
                    }
                };
                (tp, Vec::new())
            }
        };

        // §9 lever: keep the tool DOWN between this region's own fragments.
        // Runs before the router sees the region, so `strippable_preamble` /
        // `trailing_retracts` below read the relinked path — the router then
        // links region-to-region on top exactly as before. Fragment
        // INTERIORS are copied verbatim by `relink_fragments`; only the
        // (previously airborne) junctions change.
        let (tp, anns) = if params.intra_region_hookup_mm > 0.0 {
            let rp = crate::finish::surface_link::RelinkParams {
                hookup_distance: params.intra_region_hookup_mm,
                stock_to_leave: params.stock_to_leave,
                sampling: params.sampling,
                feed_rate: params.feed_rate,
                plunge_rate: params.plunge_rate,
                safe_z: params.safe_z,
                link_kinematics,
                // Nearest-first: a link is only possible when the next
                // fragment is CLOSE, so ordering and linking are the same
                // lever applied twice. The dressup-level TSP still runs
                // afterwards on whatever junctions stayed as retracts.
                reorder: true,
                // This region's OWN polygon — an intra-region link that
                // leaves it cuts territory the decomposition (and, under
                // `territory_clip`, the rest mask) deliberately excluded.
                boundary: Some(&region_set),
                // …but a LIFTED link may cross it: this boundary is the
                // decomposition's own region polygon, whose job is to confine
                // CUTTING, not to fence the tool out of a keep-out. Vetoing
                // airborne hops on a dendritic island cost 17,083 intra-node
                // retract trips on the confined wanaka tier 1 (363.8 m of
                // rapids for 86.8 m of cutting). A surface-riding link — the
                // fresh-stock arm below, and the `flush_ride` arm — still
                // answers the veto.
                airborne_links_may_leave_territory: true,
                // `None` on a fresh-stock pass, where the mesh IS the
                // material and the legacy surface-riding link is correct;
                // `Some` whenever the caller holds this op's INPUT stock,
                // because then material stands above the design surface
                // wherever nothing has cut yet and a surface-riding link
                // feeds straight through it (G-LINKLOAD).
                link_ceiling,
                // Finishing prior: flush ground under a ceiling is the
                // PRIOR pass's machined output, so riding it is a sub-cusp
                // skim — the anti-staple arm. Engraving's opposite prior
                // (raw face at surface height) keeps this false there.
                flush_ride: true,
            };
            let (linked, rep) = crate::finish::surface_link::relink_fragments(
                crate::trace::toolpath_spans::AnnotatedToolpath::new(tp),
                mesh,
                index,
                cutter,
                &rp,
            );
            report.relink.add(&rep);
            // C1: this band's ring annotations are the index-carrying
            // channel this site owns; declared, not hand-remapped.
            let mut anns = anns;
            let tp = {
                let mut channels =
                    crate::trace::transform_provenance::ReconcileSet::new(None, Some(&mut anns));
                linked.reconcile(&mut channels).into_inner().toolpath
            };
            (tp, anns)
        } else {
            (tp, anns)
        };

        // Wave D1 reported a narrowed Z range only once it had cost the
        // region every one of its cutting moves, on the reasoning that a
        // band which still cuts is "a different (and much quieter) problem"
        // and reporting it here would bury the one that leaves a feature
        // untouched.
        //
        // C8: that is an argument about SEVERITY, not about whether to
        // report. A band that machined the top 2 mm of a 12 mm wall and
        // stopped is an unfinished feature, and it was silent — the clip was
        // MEASURED on every band and then discarded unless the region
        // emitted nothing at all. The two now travel in separate
        // collections, so the loud one still cannot be buried under the
        // quiet ones.
        if let Some(clip) = height_clip {
            let area_mm2 = region.polygon.area();
            if tp.total_cutting_distance() <= 0.0 {
                tracing::warn!(
                    region_index,
                    band = ?region.band,
                    area_mm2,
                    planned_levels = clip.planned_levels,
                    resolved_levels = clip.resolved_levels,
                    clip = clip.clip.label(),
                    clip_z_mm = clip.clip_z_mm,
                    "unified_finish: band dropped entirely by height resolution — \
                     this feature will be UNMACHINED"
                );
                report.dropped_bands.push(DroppedBand {
                    band: region.band,
                    region_index,
                    area_mm2,
                    planned_levels: clip.planned_levels,
                    resolved_levels: clip.resolved_levels,
                    clip_z_mm: clip.clip_z_mm,
                    clip: clip.clip,
                });
            } else {
                tracing::info!(
                    region_index,
                    band = ?region.band,
                    area_mm2,
                    planned_levels = clip.planned_levels,
                    resolved_levels = clip.resolved_levels,
                    clip = clip.clip.label(),
                    clip_z_mm = clip.clip_z_mm,
                    requested_z = ?(clip.requested_bottom_z_mm, clip.requested_top_z_mm),
                    delivered_z = ?(clip.delivered_bottom_z_mm, clip.delivered_top_z_mm),
                    lost_height_mm = clip.lost_height_mm(),
                    "unified_finish: band Z ladder SHORTENED by height resolution — \
                     this feature is only partly machined"
                );
                report.clipped_bands.push(ClippedBand {
                    band: region.band,
                    region_index,
                    area_mm2,
                    clip,
                });
            }
        }

        let stats = match region.band {
            FinishBand::VerySteep => &mut report.very_steep,
            FinishBand::MidSteep => &mut report.mid_steep,
            FinishBand::Shallow => &mut report.shallow,
        };
        stats.region_count += 1;
        stats.move_count += tp.moves.len();
        if tp.moves.is_empty() {
            continue;
        }

        let (head_strip, entry) = strippable_preamble(&tp).map_or((0, None), |(k, p)| (k, Some(p)));
        let (tail_strip, exit) = trailing_retracts(&tp);
        region_paths.push(RegionPath {
            region_index,
            band: region.band,
            tp,
            anns,
            head_strip,
            entry,
            tail_strip,
            exit,
        });
    }
    // C2: `None` unless the pass ran on at least one Shallow region.
    report.monotone_cells = monotone_cell_totals;

    // No early return on `region_paths.is_empty()`: both branches below
    // already degrade to an empty `order`/`junctions` in that case
    // (`route_greedy`'s own empty-seed guard; the steep-first branch's
    // `order` is built from `region_paths.len()`), and a claims-only run
    // (bands empty, crease node non-empty — an unusual but legal territory
    // outcome) still needs Step 6 to run so the crease node gets emitted.

    // ── Step 5: route ─────────────────────────────────────────────────────
    let (order, junctions) = match link_kinematics {
        Some(lk) => route_greedy(
            &region_paths,
            mesh,
            index,
            cutter,
            params,
            machining_boundary,
            lk,
            cancel,
        )?,
        // No machine envelope in scope: steep-first band-major order
        // (P2.c's naive concat), native links only. NEVER a
        // distance-costed guess — the P0 probe measured distance/feed
        // misjudging segmented finishing paths by up to 10×.
        None => {
            let mut order: Vec<usize> = (0..region_paths.len()).collect();
            order.sort_by_key(|&i| {
                let band = region_paths
                    .get(i)
                    .map_or(FinishBand::Shallow, |rp| rp.band);
                (std::cmp::Reverse(band_rank(band)), i)
            });
            let junctions = (0..order.len().saturating_sub(1)).map(|_| None).collect();
            (order, junctions)
        }
    };

    report.route = order
        .iter()
        .filter_map(|&i| region_paths.get(i).map(|rp| rp.region_index))
        .collect();
    for (j, junction) in junctions.iter().enumerate() {
        let (Some(&from), Some(&to)) = (order.get(j), order.get(j + 1)) else {
            continue;
        };
        let (Some(from_rp), Some(to_rp)) = (region_paths.get(from), region_paths.get(to)) else {
            continue;
        };
        if let Some(link) = junction {
            report.links.push(RoutedLink {
                from_region: from_rp.region_index,
                to_region: to_rp.region_index,
                surface: link.surface,
                cost_s: link.cost_s,
                alt_cost_s: link.alt_cost_s,
            });
        }
    }

    // ── Step 6: stitch in route order, emitting the winning links ───────
    let mut stitched = Toolpath::new();
    let mut annotations: Vec<ScallopRuntimeAnnotation> = Vec::new();
    let mut region_table: Vec<RegionTableEntry> = Vec::new();
    for (pos, &pi) in order.iter().enumerate() {
        let incoming_surface = pos
            .checked_sub(1)
            .and_then(|j| junctions.get(j))
            .and_then(|l| l.as_ref())
            .filter(|l| l.surface);
        let outgoing_surface = junctions
            .get(pos)
            .and_then(|l| l.as_ref())
            .is_some_and(|l| l.surface);
        let Some(rp) = region_paths.get_mut(pi) else {
            continue;
        };

        // The incoming surface link replaces the follower's rapid+plunge
        // preamble; the outgoing one replaces this region's trailing
        // retract(s). Native junctions keep both untouched.
        let head = if incoming_surface.is_some() {
            rp.head_strip
        } else {
            0
        };
        let tail = if outgoing_surface { rp.tail_strip } else { 0 };

        // The node's range OPENS here, before its incoming surface link —
        // not after it. The link is this region's approach, so including it
        // is the truthful model, and it is also what keeps the region-node
        // ranges TILING.
        //
        // That tiling is load-bearing. A surface link is a *feed* move, and
        // `tsp::split_into_segments` only ever splits on `MoveType::Rapid`,
        // so a link left outside every node range still gets glued into a
        // cutting segment that straddles the node boundary. The reorder then
        // relocates that segment — carrying the link — into the middle of the
        // region it just left, and `tsp::remap_spans`'s foreign-intrusion
        // guard drops the region span for containing a move from outside
        // itself. Measured on wanaka ×2 before this fix: all five dropped
        // region nodes were tripped by exactly one intruder, in every case
        // the single `MoveIntent::Linking` move at `span.end_move` (design
        // doc §14d/§14g). The five carried 87% of the operation's cutting
        // length, so their loss left `narrate_toolpath` reporting
        // `regions 0`.
        let node_start = stitched.moves.len();
        if let (Some(link), Some(entry)) = (incoming_surface, rp.entry) {
            for p in &link.pts {
                stitched.feed_to_with_intent(*p, params.feed_rate, MoveIntent::Linking);
            }
            stitched.feed_to_with_intent(entry, params.feed_rate, MoveIntent::Linking);
        }

        // `offset` stays the region's OWN first move — annotation rebasing
        // below is expressed in the region toolpath's local frame and must
        // not be shifted by the link.
        let offset = stitched.moves.len();
        let kept = rp.tp.moves.len().saturating_sub(head + tail);
        stitched
            .moves
            .extend(rp.tp.moves.iter().skip(head).take(kept).cloned());
        let end = stitched.moves.len();
        region_table.push(RegionTableEntry {
            kind: RegionKind::Band(rp.band),
            move_range: node_start..end,
            area_mm2: planned
                .regions
                .get(rp.region_index)
                .map(|r| r.polygon.area()),
        });

        // Annotation `move_index` is local to this region's own toolpath;
        // shift into the stitched frame. An annotation pointing into a
        // stripped preamble clamps to the first kept move (the ring it
        // labels now begins right at the link's landing point).
        let anns = std::mem::take(&mut rp.anns);
        annotations.extend(anns.into_iter().map(|a| {
            let move_index = if a.move_index < head {
                offset
            } else if a.move_index >= head + kept {
                (offset + kept).saturating_sub(1)
            } else {
                offset + (a.move_index - head)
            };
            ScallopRuntimeAnnotation {
                move_index,
                event: a.event,
            }
        }));
    }

    // ── Crease node: appended AFTER every routed band, natively linked —
    // S1 does not thread it through `route_greedy` (module doc; S3's fused
    // router is where corridor endpoints become routable junctions).
    if !crease_tp.moves.is_empty() {
        let offset = stitched.moves.len();
        stitched.moves.extend(crease_tp.moves);
        let end = stitched.moves.len();
        let area_mm2: f64 = planned
            .creases
            .iter()
            .filter_map(|c| c.corridor.as_ref())
            .map(|p| p.area())
            .sum();
        region_table.push(RegionTableEntry {
            kind: RegionKind::Crease,
            move_range: offset..end,
            area_mm2: Some(area_mm2),
        });
    }

    if params.intra_region_hookup_mm > 0.0 {
        tracing::info!(
            fragments = report.relink.fragments,
            surface_links = report.relink.surface_links,
            retract_links = report.relink.retract_links,
            too_far = report.relink.too_far,
            off_surface = report.relink.off_surface,
            outside_boundary = report.relink.outside_boundary,
            slower_than_retract = report.relink.slower_than_retract,
            ceiling_above_safe_z = report.relink.ceiling_above_safe_z,
            link_rate = report.relink.link_rate().unwrap_or(0.0),
            "unified_finish: intra-region stay-down linking"
        );
    }
    report.claims = claims_report;
    report.region_table = region_table;
    report.uncut_core_mm2 = uncut_core_mm2;
    report.untouched_mm2 = untouched_mm2;
    report.standing_mm2 = standing_mm2;

    Ok((stitched, annotations, report))
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
