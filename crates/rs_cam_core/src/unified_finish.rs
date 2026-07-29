//! P2.c + P2.d of the unified finishing pass
//! (`planning/unified_finish_planner_design.md`): the per-region generation
//! orchestrator and the region ROUTER.
//!
//! [`unified_finish_toolpath_with_cancel`] composes the P2.b decomposition
//! ([`crate::finish_planner::decompose`]) with three EXISTING region-scoped
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

use std::ops::Range;

use crate::crease_paths::centerline_cut_paths;
use crate::debug_trace::ToolpathDebugContext;
use crate::dropcutter::{DropCutterGrid, batch_drop_cutter_with_cancel};
use crate::finish_planner::{FinishBand, FinishPlannerParams, decompose};
use crate::finish_setup::{
    FinishSurface, SLOPE_FILTER_MAX_DEG, SLOPE_FILTER_MIN_DEG,
    build_classification_surface_with_cancel,
};
use crate::geo::{P2, P3};
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::machine_kinematics::{LinkKinematics, retract_link_time, surface_link_time};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::pencil::{PencilParams, emit_paths};
use crate::polygon::Polygon2;
use crate::region_set::RegionSet;
#[cfg(test)]
use crate::rest_field::RestGrid;
use crate::rest_field::{RestFieldParams, RestReference, detect_rest_valleys};
use crate::scallop::{
    ScallopDirection, ScallopParams, ScallopRuntimeAnnotation,
    scallop_toolpath_structured_annotated_with_cancel,
};
use crate::surface_link::build_surface_link;
use crate::tool::MillingCutter;
use crate::toolpath::{MoveIntent, Toolpath, raster_toolpath_from_grid};
use crate::waterline::{WaterlineParams, waterline_toolpath_with_cancel};

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
    /// Stock to leave on the surface (mm) — scallop path only (raster and
    /// waterline don't take one today; parity with the standalone ops).
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
            intra_region_hookup_mm: 0.0,
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
/// [`crate::scallop::ScallopDirection`]'s pattern (a core enum re-exported
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
    /// Detector params. `pencil_radius` is overwritten with the op's own
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
    /// [`crate::pencil::emit_paths`] → `PencilParams::hookup_distance`).
    ///
    /// A dial rather than a constant because `emit_paths` links with NO
    /// territory boundary: `build_surface_link` only checks that the
    /// cutter keeps mesh contact, so a crease link is free to leave the
    /// rest island it belongs to and feed across ground the op was
    /// confined away from. That is the same gouge class
    /// [`crate::surface_link::RelinkParams::boundary`] exists to prevent,
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

    /// Label carried on the STRUCTURAL `SpanKind::Region` span. Pinned
    /// verbatim to the pre-A/M8 text — spans feed the TSP's span remap and
    /// the GUI span list, so this string is a compatibility surface.
    pub fn span_label(self) -> String {
        match self {
            Self::Band(_) => format!("{} band", self.band_label()),
            Self::Crease => "Pencil claims".to_owned(),
        }
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
}

/// Orchestration report: decomposition stats + what each band generated +
/// the P2.d route.
#[derive(Debug, Clone, Default)]
pub struct UnifiedFinishReport {
    pub decompose: crate::finish_planner::DecomposeStats,
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
    pub rest_grid: Option<std::sync::Arc<crate::rest_field::RestGrid>>,
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
    /// See [`crate::scallop::ScallopReport::uncut_core_mm2`].
    pub uncut_core_mm2: f64,
}

/// Summed [`crate::surface_link::RelinkReport`] counters across regions.
#[derive(Debug, Clone, Copy, Default)]
pub struct RelinkTotals {
    pub fragments: usize,
    pub surface_links: usize,
    pub retract_links: usize,
    pub too_far: usize,
    pub off_surface: usize,
    pub slower_than_retract: usize,
    pub outside_boundary: usize,
}

impl RelinkTotals {
    /// Fraction of junctions kept on the surface. `None` when the pass
    /// never ran or the op had a single fragment per region.
    pub fn link_rate(&self) -> Option<f64> {
        let junctions = self.surface_links + self.retract_links;
        (junctions > 0).then(|| self.surface_links as f64 / junctions as f64)
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
///    — consumers disambiguate by nesting depth, not by assuming one table.
/// 3. `RapidOrderBarrier`s at each node start, plus per-Z barriers inside
///    waterline (VerySteep) nodes. See
///    [`crate::compute::spans::region_node_barriers`].
pub fn unified_finish_spans(
    toolpath: &Toolpath,
    annotations: &[ScallopRuntimeAnnotation],
    report: &UnifiedFinishReport,
) -> Vec<crate::toolpath_spans::Span> {
    use crate::compute::spans::{RegionNode, region_node_barriers, spans_from_labeled_events};
    use crate::toolpath_spans::{Span, SpanKind, SpanPayload};

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

// ── Orchestrator ─────────────────────────────────────────────────────────

/// Decompose the surface into bands, generate each REGION's toolpath with
/// the appropriate EXISTING strategy generator, route the regions (greedy
/// nearest-by-integrated-link-time when `link_kinematics` is `Some`;
/// steep-first band-major otherwise), and stitch them with the winning
/// link candidate emitted at each junction.
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
    check_cancel(cancel)?;

    // ── Step 1: classify on the TRUE surface ────────────────────────────
    // P2.b decision (`finish_setup::build_classification_surface_with_cancel`
    // doc): a ball tool's offset (generation) surface geometrically hides
    // steepness at feature scales at/below the ball radius. Classification
    // MUST read the true surface or band assignment collapses to
    // all-shallow on relief at that scale — never swap this for
    // `build_finish_surface_with_cancel`.
    let surface =
        build_classification_surface_with_cancel(mesh, index, cutter, params.tolerance, cancel)?;
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
        .covered
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
    let mut claims_paths: Vec<crate::pencil::PencilPath> = Vec::new();
    let mut claims_report: Option<ClaimsReport> = None;
    let mut claims_rest_grid: Option<std::sync::Arc<crate::rest_field::RestGrid>> = None;
    let mut claims_rest_regions: Option<std::sync::Arc<Vec<Polygon2>>> = None;
    if let Some(cfg) = claims {
        check_cancel(cancel)?;
        let rf_params = RestFieldParams {
            cell_mm: cfg.rest_field_params.cell_mm,
            min_valley_depth: cfg.rest_field_params.min_valley_depth,
            route_width_factor: cfg.rest_field_params.route_width_factor,
            // The tip cutter IS the pencil in this op — see `ClaimsConfig`
            // doc — regardless of what the caller set here.
            pencil_radius: cutter.radius(),
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
            crate::pencil::SURFACE_PROBE_BALL_DIAMETER_MM,
            crate::pencil::SURFACE_PROBE_BALL_LENGTH_MM,
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
        let mut emitted_paths: Vec<crate::pencil::PencilPath> = Vec::new();
        for centerline in rf.centerlines {
            check_cancel(cancel)?;
            let paths = centerline_cut_paths(
                std::slice::from_ref(&centerline),
                mesh,
                index,
                cutter,
                cutter.radius(),
                params.sampling,
                // Offset stepover mirrors `PencilParams`'s own default
                // (`tool_radius * 0.5`) — no dedicated dial in S1.
                cutter.radius() * 0.5,
                // Offset-pass cap (design doc §2.1 item 5).
                4,
                cfg.rest_field_params.min_cut_length,
                params.stock_to_leave,
                cancel,
            )?;
            if !paths.is_empty() {
                emitted_paths.extend(paths);
                claimed_creases += 1;
            }
        }
        claims_paths = emitted_paths;
        tracing::debug!(
            detected,
            claimed = claimed_creases,
            paths = claims_paths.len(),
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
            let dist = crate::grid_field::distance_transform_2d(&keep, grid.ny, grid.nx);
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
    let planned = decompose(&surface.slope_map, &covered, &[], cutter.radius(), planner);
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
        let (tp, _anns) = emit_paths(&claims_paths, mesh, index, cutter, &pencil_params);
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
        rest_grid: claims_rest_grid,
        rest_regions: claims_rest_regions,
        ..UnifiedFinishReport::default()
    };
    let mut region_paths: Vec<RegionPath> = Vec::new();
    let mut shallow_grid: Option<DropCutterGrid> = None;

    let mut uncut_core_mm2 = 0.0_f64;
    for (region_index, region) in planned.regions.iter().enumerate() {
        check_cancel(cancel)?;
        let region_set = RegionSet::new(vec![region.polygon.clone()]);
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
                let wp = WaterlineParams {
                    sampling: params.sampling,
                    feed_rate: params.feed_rate,
                    plunge_rate: params.plunge_rate,
                    safe_z: params.safe_z,
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
                };
                // Generation intentionally uses the ball-center OFFSET
                // surface here (`scallop_toolpath_structured_annotated_
                // with_cancel` builds its own via `finish_setup::
                // build_finish_surface_with_cancel` internally) — only
                // classification (step 1, above) reads the true surface.
                // This is the P2.b "classify true, generate offset" split
                // from the design doc, not an inconsistency.
                let (tp, anns, scallop_report) =
                    scallop_toolpath_structured_annotated_with_cancel(
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
                (tp, anns)
            }
            FinishBand::Shallow => {
                let grid = match shallow_grid.as_ref() {
                    Some(grid) => grid,
                    None => {
                        // Mesh-bottom floor, mirroring `generate_drop_cutter`'s
                        // `effective_min_z` (compute::execute.rs). The
                        // stock-bbox floor (`ctx.stock_bbox.min.z - 1.0`
                        // there) is the op-adapter's job — this pure-core
                        // function only sees a mesh, not a stock model, so
                        // callers needing that extra floor should pre-max it
                        // into `bottom_z` before calling in.
                        let effective_min_z = mesh.bbox.min.z - 0.1;
                        // `batch_drop_cutter_with_cancel` requires
                        // `&(dyn CancelCheck + Sync)` for its rayon closures;
                        // this function only receives a plain
                        // `&dyn CancelCheck`, matching every finish-op call
                        // site up this chain (see the identical constraint
                        // documented on `slope::SurfaceHeightmap::
                        // from_mesh_with_cancel`). Widening our own signature
                        // to `+ Sync` would ripple through every future
                        // caller for the sake of one internal call, so this
                        // band checks cancellation immediately before and
                        // after the batch call instead of threading `cancel`
                        // through it.
                        let never_cancel = || false;
                        let mut grid = batch_drop_cutter_with_cancel(
                            mesh,
                            index,
                            cutter,
                            params.raster_stepover,
                            0.0,
                            effective_min_z,
                            &never_cancel,
                        )?;
                        // Drop grid points whose vertical ray misses every
                        // triangle in the mesh. Replicated from
                        // `compute::execute::generate_drop_cutter`'s
                        // identical guard: `point_drop_cutter` marks a point
                        // contacted whenever the cutter (which has radius)
                        // touches ANY nearby triangle — including the rim of
                        // a mesh that doesn't cover that XY. Without this
                        // check the tool rides the edge and carves a trench
                        // around the part.
                        for pt in &mut grid.points {
                            let mut over = false;
                            for &tri_idx in &index.query(pt.x, pt.y, 0.0) {
                                // SAFETY: tri_idx comes from `index.query`,
                                // which only ever returns indices into
                                // `mesh.faces` (mirrors `generate_drop_cutter`'s
                                // identical loop in compute::execute.rs).
                                #[allow(clippy::indexing_slicing)]
                                let tri = &mesh.faces[tri_idx];
                                if tri.contains_point_xy(pt.x, pt.y) {
                                    over = true;
                                    break;
                                }
                            }
                            if !over {
                                pt.z = effective_min_z;
                                pt.contacted = false;
                            }
                        }
                        check_cancel(cancel)?;
                        shallow_grid.insert(grid)
                    }
                };
                let effective_min_z = mesh.bbox.min.z - 0.1;
                let tp = raster_toolpath_from_grid(
                    grid,
                    params.feed_rate,
                    params.plunge_rate,
                    params.safe_z,
                    Some(effective_min_z),
                    Some(&region_set),
                );
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
            let rp = crate::surface_link::RelinkParams {
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
            };
            let (linked, rep) =
                crate::surface_link::relink_fragments(&tp, mesh, index, cutter, &rp);
            report.relink.fragments += rep.fragments;
            report.relink.surface_links += rep.surface_links;
            report.relink.retract_links += rep.retract_links;
            report.relink.too_far += rep.too_far;
            report.relink.off_surface += rep.off_surface;
            report.relink.slower_than_retract += rep.slower_than_retract;
            report.relink.outside_boundary += rep.outside_boundary;
            let anns = anns
                .into_iter()
                .map(|a| ScallopRuntimeAnnotation {
                    move_index: rep.move_remap.get(a.move_index).copied().unwrap_or(0),
                    event: a.event,
                })
                .collect();
            (linked, anns)
        } else {
            (tp, anns)
        };

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
            link_rate = report.relink.link_rate().unwrap_or(0.0),
            "unified_finish: intra-region stay-down linking"
        );
    }
    report.claims = claims_report;
    report.region_table = region_table;
    report.uncut_core_mm2 = uncut_core_mm2;

    Ok((stitched, annotations, report))
}

// ── P2.d router internals ─────────────────────────────────────────────────

/// One region's generated toolpath plus the stitch metadata the router
/// needs: where the tool ENTERS the surface (its plunge target), where it
/// LEAVES it (last cut before the trailing retracts), and how many moves
/// each end contributes so a surface link can strip them.
struct RegionPath {
    /// Index into the decomposition's `planned.regions`.
    region_index: usize,
    band: FinishBand,
    tp: Toolpath,
    anns: Vec<ScallopRuntimeAnnotation>,
    /// Leading moves (Linking rapid + EntryPlunge) removable when a surface
    /// link lands the tool at `entry` directly. 0 = not strippable.
    head_strip: usize,
    /// Surface entry point — the first run's EntryPlunge target.
    entry: Option<P3>,
    /// Trailing `MoveIntent::Retract` moves removable when the NEXT
    /// junction is a surface link (the tool stays on the surface).
    tail_strip: usize,
    /// Surface exit point — the last move before the trailing retracts.
    exit: Option<P3>,
}

/// A costed junction between two consecutive regions in the route.
struct JunctionChoice {
    surface: bool,
    /// Interior surface-link points (empty for a retract junction, and for
    /// a zero-length surface junction).
    pts: Vec<P3>,
    cost_s: f64,
    alt_cost_s: Option<f64>,
}

fn band_rank(band: FinishBand) -> u8 {
    match band {
        FinishBand::VerySteep => 2,
        FinishBand::MidSteep => 1,
        FinishBand::Shallow => 0,
    }
}

/// Leading strippable preamble: the index `k` of the first
/// `FinishingCut` move plus the surface entry point (the move `k-1`
/// EntryPlunge target). `None` when the toolpath doesn't open with the
/// canonical `Linking rapid(s) → EntryPlunge → FinishingCut` shape every
/// strategy generator in this module emits — an unrecognized preamble is
/// kept verbatim rather than guessed at.
fn strippable_preamble(tp: &Toolpath) -> Option<(usize, P3)> {
    let k = tp
        .moves
        .iter()
        .position(|m| m.intent == MoveIntent::FinishingCut)?;
    if k == 0 {
        return None;
    }
    let head_ok = tp
        .moves
        .iter()
        .take(k)
        .all(|m| matches!(m.intent, MoveIntent::Linking | MoveIntent::EntryPlunge));
    if !head_ok {
        return None;
    }
    let plunge = tp.moves.get(k - 1)?;
    (plunge.intent == MoveIntent::EntryPlunge).then_some((k, plunge.target))
}

/// Trailing retract count + the surface exit point right before them.
fn trailing_retracts(tp: &Toolpath) -> (usize, Option<P3>) {
    let n = tp
        .moves
        .iter()
        .rev()
        .take_while(|m| m.intent == MoveIntent::Retract)
        .count();
    let exit = tp
        .moves
        .len()
        .checked_sub(n + 1)
        .and_then(|i| tp.moves.get(i))
        .map(|m| m.target);
    (n, exit)
}

/// Greedy nearest-by-integrated-link-time route over the generated
/// regions, seeded at the steepest band present (first planned region of
/// that band — deterministic). Returns the order (indices into `paths`)
/// and the winning junction choice for each consecutive pair. 2-opt is
/// deliberately absent: the design doc's P0 discipline is measure first —
/// it only gets built if the A/B says greedy leaves >5% on the table.
#[allow(clippy::too_many_arguments)] // link geometry + machine envelope are irreducible inputs
fn route_greedy(
    paths: &[RegionPath],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &UnifiedFinishParams,
    machining_boundary: Option<&RegionSet<'_>>,
    lk: &LinkKinematics,
    cancel: &dyn CancelCheck,
) -> Result<(Vec<usize>, Vec<Option<JunctionChoice>>), Cancelled> {
    let seed = (0..paths.len()).max_by_key(|&i| {
        (
            paths.get(i).map_or(0, |rp| band_rank(rp.band)),
            std::cmp::Reverse(i),
        )
    });
    let Some(seed) = seed else {
        return Ok((Vec::new(), Vec::new()));
    };

    let mut visited = vec![false; paths.len()];
    if let Some(v) = visited.get_mut(seed) {
        *v = true;
    }
    let mut order = vec![seed];
    let mut junctions: Vec<Option<JunctionChoice>> = Vec::new();
    let mut current = seed;

    for _ in 1..paths.len() {
        check_cancel(cancel)?;
        let from = paths
            .get(current)
            .and_then(|rp| rp.exit.or_else(|| rp.tp.moves.last().map(|m| m.target)));
        let Some(from) = from else { break };

        let mut best: Option<(usize, JunctionChoice)> = None;
        for (j, candidate) in paths.iter().enumerate() {
            if visited.get(j).copied().unwrap_or(true) {
                continue;
            }
            let choice = choose_link(
                from,
                candidate,
                mesh,
                index,
                cutter,
                params,
                machining_boundary,
                lk,
            );
            // Strict `<` keeps the earliest candidate on exact ties —
            // deterministic (`paths` preserves the planned-region order).
            if best.as_ref().is_none_or(|(_, b)| choice.cost_s < b.cost_s) {
                best = Some((j, choice));
            }
        }
        let Some((next, choice)) = best else { break };
        if let Some(v) = visited.get_mut(next) {
            *v = true;
        }
        order.push(next);
        junctions.push(Some(choice));
        current = next;
    }

    Ok((order, junctions))
}

/// Cost both link candidates from `from` (the previous region's surface
/// exit) into `to` and return the winner. The surface candidate mirrors
/// pencil's emit decision exactly: gouge-checked via [`build_surface_link`]
/// (real cutter → it rides the OFFSET surface), additionally required to
/// stay inside the machining boundary (the selective-scallop gouge rule:
/// never feed across excluded islands), and only available when the
/// follower has a strippable entry. Ties go to the surface link, matching
/// pencil's `surface_t <= retract_t`.
#[allow(clippy::too_many_arguments)] // link geometry + machine envelope are irreducible inputs
fn choose_link(
    from: P3,
    to: &RegionPath,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &UnifiedFinishParams,
    machining_boundary: Option<&RegionSet<'_>>,
    lk: &LinkKinematics,
) -> JunctionChoice {
    let entry_surface = (to.head_strip > 0).then_some(to.entry).flatten();
    let retract_to = entry_surface
        .or(to.entry)
        .or_else(|| to.tp.moves.first().map(|m| m.target))
        .unwrap_or(from);
    let retract_t = retract_link_time(
        from,
        retract_to,
        params.safe_z,
        // No rapid-down-to-clearance: the cost model can't verify the input
        // stock's ceiling from here (that check lives in the post-generation
        // `optimize_entry_descents` pass), so it must not assume a descent
        // it can't guarantee is safe — same reasoning as pencil's emit path.
        None,
        params.plunge_rate,
        &lk.kinematics,
        lk.max_feed_mm_min,
        lk.rapid_feed_mm_min,
    );

    let surface_candidate = entry_surface.and_then(|entry| {
        let pts = build_surface_link(
            from,
            entry,
            mesh,
            index,
            cutter,
            params.stock_to_leave,
            params.sampling,
        )?;
        // Inside the machining boundary only — a surface feed across an
        // excluded island is exactly the gouge class the boundary exists
        // to prevent.
        if let Some(boundary) = machining_boundary
            && !pts.iter().all(|p| boundary.contains(&P2::new(p.x, p.y)))
        {
            return None;
        }
        let mut costed_path = pts.clone();
        costed_path.push(entry);
        let surface_t = surface_link_time(
            from,
            &costed_path,
            params.feed_rate,
            &lk.kinematics,
            lk.max_feed_mm_min,
            lk.rapid_feed_mm_min,
        );
        Some((pts, surface_t))
    });

    match surface_candidate {
        Some((pts, surface_t)) if surface_t <= retract_t => JunctionChoice {
            surface: true,
            pts,
            cost_s: surface_t,
            alt_cost_s: Some(retract_t),
        },
        Some((_, surface_t)) => JunctionChoice {
            surface: false,
            pts: Vec::new(),
            cost_s: retract_t,
            alt_cost_s: Some(surface_t),
        },
        None => JunctionChoice {
            surface: false,
            pts: Vec::new(),
            cost_s: retract_t,
            alt_cost_s: None,
        },
    }
}

/// Z range (min, max) of the classification surface's covered cells whose
/// centers fall inside `regions`. `None` when nothing survives both
/// filters — a genuinely degenerate band (its own polygon shrank to
/// nothing after conditioning, or every covered cell sits just outside it
/// at the mask boundary).
fn band_z_range(
    surface: &FinishSurface,
    covered: &[bool],
    regions: &RegionSet<'_>,
) -> Option<(f64, f64)> {
    let cols = surface.cols();
    if cols == 0 {
        return None;
    }
    let cell = surface.cell_size();
    let origin_x = surface.slope_map.origin_x;
    let origin_y = surface.slope_map.origin_y;

    let mut min_z = f64::INFINITY;
    let mut max_z = f64::NEG_INFINITY;
    for (i, &z) in surface.heightmap.z_values.iter().enumerate() {
        if !covered.get(i).copied().unwrap_or(false) {
            continue;
        }
        let row = i / cols;
        let col = i % cols;
        let x = origin_x + col as f64 * cell;
        let y = origin_y + row as f64 * cell;
        if regions.contains(&P2::new(x, y)) {
            min_z = min_z.min(z);
            max_z = max_z.max(z);
        }
    }
    (min_z.is_finite() && max_z.is_finite()).then_some((min_z, max_z))
}

// ── Claims pipeline internals (v3 S1/S4) ────────────────────────────────

/// Nearest-cell index into a [`RestGrid`] at world `(x, y)`. The rest
/// grid's own origin/cell size (from [`detect_rest_valleys`]) is
/// independent of the classification surface's grid, so this is
/// deliberately a round-to-nearest lookup rather than an index computed
/// from the caller's grid geometry — mirroring [`RestReference::Stock`]'s
/// nearest-cell-only contract (never interpolate a dexel top across a
/// steep wall). `None` outside the grid.
///
/// Test-only since the S2 removal (2026-08-03): the S4 mask walks the
/// rest grid in its own index space and needs no world-XY lookup, but the
/// claims sentries probe specific world points to prove which reference
/// arm fed the detector.
#[cfg(test)]
fn rest_grid_index(grid: &RestGrid, x: f64, y: f64) -> Option<usize> {
    if grid.nx == 0 || grid.ny == 0 || grid.cell_mm <= 0.0 {
        return None;
    }
    let col = ((x - grid.origin_x) / grid.cell_mm).round();
    let row = ((y - grid.origin_y) / grid.cell_mm).round();
    if col < 0.0 || row < 0.0 {
        return None;
    }
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let (col, row) = (col as usize, row as usize);
    if col >= grid.nx || row >= grid.ny {
        return None;
    }
    Some(row * grid.nx + col)
}

/// Total length (mm) of `tp`'s `FinishingCut` moves — each move's
/// contribution is the distance from the PRECEDING move's target (rapid,
/// plunge, or another cut) to its own, so a cut immediately following a
/// plunge/link still counts the segment actually cut. Used for the crease
/// node's `crease_path_length_mm` telemetry instead of reaching into
/// `pencil::PencilPath`'s private fields.
fn cutting_length_mm(tp: &Toolpath) -> f64 {
    let mut total = 0.0;
    let mut prev: Option<P3> = None;
    for mv in &tp.moves {
        if mv.intent == MoveIntent::FinishingCut
            && let Some(p) = prev
        {
            total += ((mv.target.x - p.x).powi(2)
                + (mv.target.y - p.y).powi(2)
                + (mv.target.z - p.z).powi(2))
            .sqrt();
        }
        prev = Some(mv.target);
    }
    total
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::mesh::{make_test_flat, make_test_hemisphere};
    use crate::polygon::Polygon2;
    use crate::tool::BallEndmill;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn ball(diameter: f64) -> BallEndmill {
        BallEndmill::new(diameter, diameter * 5.0)
    }

    // ── fixture 1: flat plate (Shallow only) ────────────────────────────

    #[test]
    fn flat_plate_generates_raster_only() {
        let mesh = make_test_flat(60.0);
        let index = SpatialIndex::build(&mesh, 10.0);
        let cutter = ball(6.0); // radius 3.0
        let params = UnifiedFinishParams::default();
        let planner = FinishPlannerParams::for_tool(cutter.cusp_radius());
        let never_cancel = || false;

        let (tp, anns, report) = unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            5.0,
            -5.0,
            &params,
            &planner,
            None,
            None,
            None,
            None,
            &never_cancel,
        )
        .unwrap();

        assert!(
            report.shallow.move_count > 0,
            "flat plate should produce shallow raster moves"
        );
        assert_eq!(report.mid_steep.move_count, 0);
        assert_eq!(report.very_steep.move_count, 0);
        assert!(!tp.moves.is_empty());
        assert!(
            anns.is_empty(),
            "raster-only run should carry no scallop annotations"
        );
    }

    // ── fixture 2: hemisphere (spans all three bands) ───────────────────
    //
    // Radius/tool-radius/cell-size mirror `finish_planner`'s own
    // `dome_decomposes_into_four_regions` test (radius 30, cell 1.0,
    // `FinishPlannerParams::for_tool(3.0)`) as closely as possible — that
    // test proves the decomposition dials produce a clean
    // shallow/mid/very-steep split at this exact scale. The only new
    // variable here is going through a real triangulated mesh + the
    // classification-surface probe pipeline instead of a synthetic z-grid;
    // R1's hysteresis/close/min-area conditioning is specifically built to
    // absorb the resulting facet noise.
    fn steep_cone_fixture() -> (TriangleMesh, SpatialIndex, BallEndmill, FinishPlannerParams) {
        let mesh = make_test_hemisphere(30.0, 24);
        let index = SpatialIndex::build(&mesh, 10.0);
        let cutter = ball(6.0); // radius 3.0
        let planner = FinishPlannerParams::for_tool(cutter.cusp_radius());
        (mesh, index, cutter, planner)
    }

    fn steep_cone_params() -> UnifiedFinishParams {
        UnifiedFinishParams {
            // Forces classification cell_size to exactly 1.0mm
            // (`(tool_radius / 4).max(tolerance)` with tool_radius = 3.0),
            // matching the mirrored `finish_planner` test's grid.
            tolerance: 1.0,
            ..UnifiedFinishParams::default()
        }
    }

    #[test]
    fn steep_cone_generates_multiple_bands() {
        let (mesh, index, cutter, planner) = steep_cone_fixture();
        let params = steep_cone_params();
        let never_cancel = || false;

        let (tp, _anns, report) = unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            35.0,
            0.0,
            &params,
            &planner,
            None,
            None,
            None,
            None,
            &never_cancel,
        )
        .unwrap();

        let populated = [report.very_steep, report.mid_steep, report.shallow]
            .iter()
            .filter(|b| b.move_count > 0)
            .count();
        assert!(
            populated >= 2,
            "hemisphere should span at least two bands, got very_steep={} mid_steep={} shallow={}",
            report.very_steep.move_count,
            report.mid_steep.move_count,
            report.shallow.move_count
        );
        let expected_total =
            report.very_steep.move_count + report.mid_steep.move_count + report.shallow.move_count;
        assert_eq!(tp.moves.len(), expected_total);
    }

    #[test]
    fn annotations_shifted_by_concat_offset() {
        let (mesh, index, cutter, planner) = steep_cone_fixture();
        let params = steep_cone_params();
        let never_cancel = || false;

        let (tp, anns, report) = unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            35.0,
            0.0,
            &params,
            &planner,
            None,
            None,
            None,
            None,
            &never_cancel,
        )
        .unwrap();

        assert!(
            !anns.is_empty(),
            "mid-steep band should carry scallop ring annotations"
        );
        for ann in &anns {
            assert!(
                ann.move_index < tp.moves.len(),
                "annotation move_index {} escaped the stitched toolpath ({} moves)",
                ann.move_index,
                tp.moves.len()
            );
            assert!(
                ann.move_index >= report.very_steep.move_count,
                "annotation move_index {} lands before the mid-steep band's concat offset ({})",
                ann.move_index,
                report.very_steep.move_count
            );
        }
    }

    #[test]
    fn deterministic() {
        let (mesh, index, cutter, planner) = steep_cone_fixture();
        let params = steep_cone_params();
        let never_cancel = || false;

        let (tp_a, _anns_a, report_a) = unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            35.0,
            0.0,
            &params,
            &planner,
            None,
            None,
            None,
            None,
            &never_cancel,
        )
        .unwrap();
        let (tp_b, _anns_b, report_b) = unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            35.0,
            0.0,
            &params,
            &planner,
            None,
            None,
            None,
            None,
            &never_cancel,
        )
        .unwrap();

        assert_eq!(tp_a.moves.len(), tp_b.moves.len());
        assert_eq!(
            report_a.very_steep.move_count,
            report_b.very_steep.move_count
        );
        assert_eq!(report_a.mid_steep.move_count, report_b.mid_steep.move_count);
        assert_eq!(report_a.shallow.move_count, report_b.shallow.move_count);

        let first_a = tp_a
            .moves
            .first()
            .map(|m| (m.target.x, m.target.y, m.target.z));
        let first_b = tp_b
            .moves
            .first()
            .map(|m| (m.target.x, m.target.y, m.target.z));
        assert_eq!(first_a, first_b);
        let last_a = tp_a
            .moves
            .last()
            .map(|m| (m.target.x, m.target.y, m.target.z));
        let last_b = tp_b
            .moves
            .last()
            .map(|m| (m.target.x, m.target.y, m.target.z));
        assert_eq!(last_a, last_b);
    }

    // ── cancellation ─────────────────────────────────────────────────────

    #[test]
    fn cancellation_propagates() {
        let mesh = make_test_flat(60.0);
        let index = SpatialIndex::build(&mesh, 10.0);
        let cutter = ball(6.0);
        let params = UnifiedFinishParams::default();
        let planner = FinishPlannerParams::for_tool(cutter.cusp_radius());

        // False on the very first check, true on every check after —
        // guarantees at least one check succeeds (so classification can
        // start) but the run cannot complete without observing cancel.
        let already_checked = AtomicBool::new(false);
        let cancel_after_first = || already_checked.swap(true, Ordering::SeqCst);

        let result = unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            5.0,
            -5.0,
            &params,
            &planner,
            None,
            None,
            None,
            None,
            &cancel_after_first,
        );
        assert!(
            result.is_err(),
            "expected cancellation to propagate as Err(Cancelled)"
        );
    }

    // ── P2.d router ──────────────────────────────────────────────────────

    #[test]
    fn preamble_and_retract_detection() {
        let mut tp = Toolpath::new();
        tp.rapid_to_with_intent(P3::new(1.0, 2.0, 30.0), MoveIntent::Linking);
        tp.feed_to_with_intent(P3::new(1.0, 2.0, 0.5), 500.0, MoveIntent::EntryPlunge);
        tp.feed_to_with_intent(P3::new(5.0, 2.0, 0.4), 1000.0, MoveIntent::FinishingCut);
        tp.feed_to_with_intent(P3::new(9.0, 2.0, 0.3), 1000.0, MoveIntent::FinishingCut);
        tp.rapid_to_with_intent(P3::new(9.0, 2.0, 30.0), MoveIntent::Retract);

        let (k, entry) = strippable_preamble(&tp).expect("canonical preamble");
        assert_eq!(k, 2);
        assert_eq!((entry.x, entry.y, entry.z), (1.0, 2.0, 0.5));

        let (n, exit) = trailing_retracts(&tp);
        assert_eq!(n, 1);
        let exit = exit.expect("exit point");
        assert_eq!((exit.x, exit.y, exit.z), (9.0, 2.0, 0.3));

        // A toolpath opening with a cut (no preamble) is not strippable.
        let mut bare = Toolpath::new();
        bare.feed_to_with_intent(P3::new(0.0, 0.0, 0.0), 1000.0, MoveIntent::FinishingCut);
        assert!(strippable_preamble(&bare).is_none());

        // An unrecognized preamble shape (e.g. a Retract before the first
        // cut) is kept verbatim rather than guessed at.
        let mut odd = Toolpath::new();
        odd.rapid_to_with_intent(P3::new(0.0, 0.0, 30.0), MoveIntent::Retract);
        odd.feed_to_with_intent(P3::new(0.0, 0.0, 0.0), 500.0, MoveIntent::EntryPlunge);
        odd.feed_to_with_intent(P3::new(1.0, 0.0, 0.0), 1000.0, MoveIntent::FinishingCut);
        assert!(strippable_preamble(&odd).is_none());
    }

    fn test_link_kinematics() -> crate::machine_kinematics::LinkKinematics {
        crate::machine_kinematics::LinkKinematics {
            kinematics: crate::machine_kinematics::MachineKinematics::default(),
            max_feed_mm_min: 3000.0,
            rapid_feed_mm_min: 5000.0,
        }
    }

    #[test]
    fn router_orders_regions_and_reports_links() {
        let (mesh, index, cutter, planner) = steep_cone_fixture();
        let params = steep_cone_params();
        let lk = test_link_kinematics();
        let never_cancel = || false;

        let (tp, anns, report) = unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            35.0,
            0.0,
            &params,
            &planner,
            None,
            Some(&lk),
            None,
            None,
            &never_cancel,
        )
        .unwrap();

        assert!(!tp.moves.is_empty());
        assert!(!report.route.is_empty(), "route must list every cut region");
        assert_eq!(
            report.links.len(),
            report.route.len() - 1,
            "one costed junction per consecutive route pair"
        );
        for link in &report.links {
            assert!(
                link.cost_s.is_finite() && link.cost_s >= 0.0,
                "junction cost must be a real integrated time, got {}",
                link.cost_s
            );
            if let Some(alt) = link.alt_cost_s {
                assert!(
                    link.cost_s <= alt,
                    "winning candidate ({:.3}s) must not cost more than the loser ({alt:.3}s)",
                    link.cost_s
                );
            }
        }
        for ann in &anns {
            assert!(
                ann.move_index < tp.moves.len(),
                "annotation move_index {} escaped the stitched toolpath ({} moves)",
                ann.move_index,
                tp.moves.len()
            );
        }
    }

    #[test]
    fn router_is_deterministic() {
        let (mesh, index, cutter, planner) = steep_cone_fixture();
        let params = steep_cone_params();
        let lk = test_link_kinematics();
        let never_cancel = || false;

        let run = || {
            unified_finish_toolpath_with_cancel(
                &mesh,
                &index,
                &cutter,
                35.0,
                0.0,
                &params,
                &planner,
                None,
                Some(&lk),
                None,
                None,
                &never_cancel,
            )
            .unwrap()
        };
        let (tp_a, _, report_a) = run();
        let (tp_b, _, report_b) = run();
        assert_eq!(tp_a.moves.len(), tp_b.moves.len());
        assert_eq!(report_a.route, report_b.route);
        assert_eq!(report_a.links.len(), report_b.links.len());
        for (a, b) in report_a.links.iter().zip(report_b.links.iter()) {
            assert_eq!(a.surface, b.surface);
            assert_eq!(a.from_region, b.from_region);
            assert_eq!(a.to_region, b.to_region);
        }
    }

    /// A synthetic region path at `x_center` on a flat surface (z = 0):
    /// the canonical `Linking rapid → EntryPlunge → cuts → Retract` shape
    /// every strategy in this module emits.
    fn synthetic_region(region_index: usize, band: FinishBand, x_center: f64) -> RegionPath {
        let mut tp = Toolpath::new();
        tp.rapid_to_with_intent(P3::new(x_center - 3.0, 0.0, 30.0), MoveIntent::Linking);
        tp.feed_to_with_intent(
            P3::new(x_center - 3.0, 0.0, 0.0),
            500.0,
            MoveIntent::EntryPlunge,
        );
        tp.feed_to_with_intent(
            P3::new(x_center + 3.0, 0.0, 0.0),
            1000.0,
            MoveIntent::FinishingCut,
        );
        tp.rapid_to_with_intent(P3::new(x_center + 3.0, 0.0, 30.0), MoveIntent::Retract);
        let (head_strip, entry) = strippable_preamble(&tp).map(|(k, p)| (k, Some(p))).unwrap();
        let (tail_strip, exit) = trailing_retracts(&tp);
        RegionPath {
            region_index,
            band,
            tp,
            anns: Vec::new(),
            head_strip,
            entry,
            tail_strip,
            exit,
        }
    }

    #[test]
    fn route_greedy_seeds_at_steepest_and_chains_nearest() {
        let mesh = make_test_flat(60.0);
        let index = SpatialIndex::build(&mesh, 10.0);
        let cutter = ball(6.0);
        let params = UnifiedFinishParams::default();
        let lk = test_link_kinematics();
        let never_cancel = || false;

        // Planned order (Shallow first, as `decompose` emits): a Shallow
        // region on each flank, the lone MidSteep in the middle.
        let paths = vec![
            synthetic_region(0, FinishBand::Shallow, -20.0),
            synthetic_region(1, FinishBand::Shallow, 20.0),
            synthetic_region(2, FinishBand::MidSteep, 0.0),
        ];

        let (order, junctions) = route_greedy(
            &paths,
            &mesh,
            &index,
            &cutter,
            &params,
            None,
            &lk,
            &never_cancel,
        )
        .unwrap();

        assert_eq!(
            order.first(),
            Some(&2),
            "route must seed at the steepest band present, got {order:?}"
        );
        // The MidSteep region exits at x = +3: the east flank's entry
        // (x = 17) is 14 mm away, the west flank's (x = -23) is 26 mm —
        // greedy must take the near one first.
        assert_eq!(order, vec![2, 1, 0]);
        assert_eq!(junctions.len(), 2);
        for j in junctions.iter().flatten() {
            assert!(j.cost_s.is_finite() && j.cost_s > 0.0);
        }
    }

    // ── machining boundary ───────────────────────────────────────────────

    #[test]
    fn boundary_restricts_output() {
        let mesh = make_test_flat(60.0);
        let index = SpatialIndex::build(&mesh, 10.0);
        let cutter = ball(6.0);
        let params = UnifiedFinishParams::default();
        let planner = FinishPlannerParams::for_tool(cutter.cusp_radius());
        let never_cancel = || false;

        let (unrestricted, _anns, _report) = unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            5.0,
            -5.0,
            &params,
            &planner,
            None,
            None,
            None,
            None,
            &never_cancel,
        )
        .unwrap();

        let square = Polygon2::rectangle(-10.0, -10.0, 10.0, 10.0);
        let regions = vec![square];
        let boundary = RegionSet::from_slice(&regions);

        let (restricted, _anns2, _report2) = unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            5.0,
            -5.0,
            &params,
            &planner,
            Some(&boundary),
            None,
            None,
            None,
            &never_cancel,
        )
        .unwrap();

        assert!(
            restricted.moves.len() < unrestricted.moves.len(),
            "boundary-restricted run should produce fewer moves ({} vs {})",
            restricted.moves.len(),
            unrestricted.moves.len()
        );

        // Stepover slack at the region edge: a raster row point can sit up
        // to one stepover past the boundary before the run-splitter drops
        // it.
        let margin = params.raster_stepover.max(cutter.radius());
        let mut saw_cut = false;
        for m in &restricted.moves {
            if let crate::toolpath::MoveType::Linear { .. } = m.move_type
                && m.intent == crate::toolpath::MoveIntent::FinishingCut
            {
                saw_cut = true;
                assert!(
                    m.target.x >= -10.0 - margin
                        && m.target.x <= 10.0 + margin
                        && m.target.y >= -10.0 - margin
                        && m.target.y <= 10.0 + margin,
                    "cutting move ({:.2},{:.2}) escaped the boundary square",
                    m.target.x,
                    m.target.y
                );
            }
        }
        assert!(
            saw_cut,
            "expected at least one cutting move inside the boundary"
        );
    }

    // ── claims pipeline (v3 S1) ──────────────────────────────────────────

    /// A Gaussian-profile trench running along X, centered at `y = 0`.
    /// Mirrors `rest_field::tests::make_trench` (duplicated here — small and
    /// test-only, each module's fixture stays independently readable): a
    /// hard-edged plane-wall V has a CONSTANT rest depth along its length
    /// (a plateau, no local maximum) and the RestDepth detector's
    /// NMS-based ridge extraction never latches onto one (see
    /// `rest_field::tests::v_valley_yields_one_centerline`'s doc comment) —
    /// only a curved profile like this one is genuinely detectable.
    fn make_trench_mesh(
        len_x: f64,
        half_y: f64,
        depth: f64,
        sigma: f64,
        nx: usize,
        ny: usize,
    ) -> TriangleMesh {
        let mut verts = Vec::new();
        for iy in 0..=ny {
            let y = -half_y + 2.0 * half_y * iy as f64 / ny as f64;
            let z = -depth * (-(y / sigma).powi(2)).exp();
            for ix in 0..=nx {
                let x = len_x * ix as f64 / nx as f64;
                verts.push(P3::new(x, y, z));
            }
        }
        let mut tris = Vec::new();
        let stride = nx + 1;
        for iy in 0..ny {
            for ix in 0..nx {
                let a = (iy * stride + ix) as u32;
                let b = a + 1;
                let c = a + stride as u32;
                let d = c + 1;
                tris.push([a, b, d]);
                tris.push([a, d, c]);
            }
        }
        TriangleMesh::from_raw(verts, tris)
    }

    /// Shared claims-on fixture: the trench mesh, a small pencil/finishing
    /// tool, and a bigger analytic reference tool that cannot reach the
    /// trench floor — same tool sizes as `rest_field`'s own
    /// `v_valley_yields_one_centerline` proof.
    fn trench_claims_fixture() -> (
        TriangleMesh,
        SpatialIndex,
        BallEndmill,
        UnifiedFinishParams,
        FinishPlannerParams,
    ) {
        let mesh = make_trench_mesh(30.0, 6.0, 1.5, 1.2, 30, 48);
        let index = SpatialIndex::build_auto(&mesh);
        // Crease detection is the analytic SELF-probe (rest = where the
        // op's own cutter floats above the bare surface), so the fixture
        // cutter must BRIDGE the 1.5 mm trench: radius 1.0 > half-width
        // 0.75 floats ~0.86 mm above the floor — a detectable rest ridge
        // along the trench axis.
        let cutter = ball(2.0);
        let params = UnifiedFinishParams {
            tolerance: 0.5,
            ..UnifiedFinishParams::default()
        };
        let planner = FinishPlannerParams::for_tool(cutter.cusp_radius());
        (mesh, index, cutter, params, planner)
    }

    /// Regression sentry: the region-node ranges must TILE the stitched
    /// toolpath, leaving no move between two nodes.
    ///
    /// A move outside every node range is not a cosmetic gap. Surface links
    /// are *feed* moves and `tsp::split_into_segments` only splits on
    /// `MoveType::Rapid`, so an orphaned link is glued into a cutting
    /// segment straddling the node boundary; the reorder relocates that
    /// segment into the region it just left, and
    /// `tsp::remap_spans`'s foreign-intrusion guard then DROPS the region
    /// span for containing a foreign move.
    ///
    /// Measured on wanaka ×2 before the fix: five region nodes dropped,
    /// each tripped by exactly one intruder — in every case the single
    /// `MoveIntent::Linking` move sitting at `span.end_move`. Those five
    /// carried 87% of the operation's cutting length, so the survivors
    /// described 12.6% of the op while `spans_valid` still read `true`,
    /// and `narrate_toolpath` reported `regions 0` on the live GUI.
    /// See `planning/unified_v3_design.md` §14c/§14d/§14g.
    #[test]
    fn region_node_ranges_tile_the_stitched_toolpath() {
        let (mesh, index, cutter, params, planner) = trench_claims_fixture();
        let never_cancel = || false;

        let (tp, _anns, report) = unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            5.0,
            -5.0,
            &params,
            &planner,
            None,
            None,
            None,
            None,
            &never_cancel,
        )
        .unwrap();

        assert!(
            !report.region_table.is_empty(),
            "fixture must route at least one region"
        );
        for pair in report.region_table.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            assert_eq!(
                a.move_range.end, b.move_range.start,
                "region nodes must be contiguous — {:?} ends at {} but {:?} \
                 starts at {}, orphaning {} move(s) that the rapid reorder \
                 can relocate into the previous node and so trip the \
                 foreign-intrusion guard",
                a.kind,
                a.move_range.end,
                b.kind,
                b.move_range.start,
                b.move_range.start.saturating_sub(a.move_range.end),
            );
        }
        if let Some(last) = report.region_table.last() {
            assert_eq!(
                last.move_range.end,
                tp.moves.len(),
                "the last region node must reach the end of the stitched \
                 toolpath"
            );
        }
    }

    #[test]
    fn claims_off_matches_legacy_band_only_output() {
        let (mesh, index, cutter, params, planner) = trench_claims_fixture();
        let never_cancel = || false;

        let (tp, _anns, report) = unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            5.0,
            -5.0,
            &params,
            &planner,
            None,
            None,
            None,
            None,
            &never_cancel,
        )
        .unwrap();

        assert!(
            report.claims.is_none(),
            "claims pipeline must not run when claims=None"
        );
        assert!(
            report
                .region_table
                .iter()
                .all(|e| matches!(e.kind, RegionKind::Band(_))),
            "no crease node should appear when claims=None, even on a \
             crease-bearing mesh: {:?}",
            report.region_table
        );
        let banded_total: usize = report
            .region_table
            .iter()
            .map(|e| e.move_range.end - e.move_range.start)
            .sum();
        assert_eq!(
            tp.moves.len(),
            banded_total,
            "toolpath must be exactly the band regions with claims off"
        );
    }

    #[test]
    fn claims_on_emits_crease_node_after_bands() {
        let (mesh, index, cutter, params, planner) = trench_claims_fixture();
        let claims_cfg = ClaimsConfig {
            territory_stock: None,
            // Build-list item 3's default arm — not under test here.
            crease_reference: CreaseReference::SelfProbe,
            rest_field_params: RestFieldParams {
                cell_mm: 0.5,
                min_valley_depth: 0.05,
                // Force pencil routing over clearing (mirrors
                // `rest_field::tests::v_valley_yields_one_centerline`) so
                // the detected ridge survives as a centerline to claim.
                route_width_factor: 10.0,
                ..RestFieldParams::default()
            },
            min_rest_depth_mm: 0.02,
            // S4 not under test here — off, the safe/default choice.
            territory_clip: false,
            crease_hookup_mm: 5.0,
        };
        let never_cancel = || false;

        let (tp, _anns, report) = unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            5.0,
            -5.0,
            &params,
            &planner,
            None,
            None,
            Some(&claims_cfg),
            None,
            &never_cancel,
        )
        .unwrap();

        let claims = report.claims.expect("claims pipeline must have run");
        assert!(
            claims.crease_path_count > 0,
            "expected claimed crease cut paths, got {claims:?}"
        );
        assert!(claims.crease_path_length_mm > 0.0);
        // S1 additive claims: the pencil node cuts, but corridors are NOT
        // carved out of the bands (width-honest carving is future work),
        // so decompose sees no creases.
        assert_eq!(report.decompose.claimed_creases, 0);
        // No territory stock: territory stays full (design doc §2.1
        // step 4), so no cells are measured skippable.
        assert_eq!(claims.territory_mode, ClaimTerritoryMode::Full);

        let crease_entry = report
            .region_table
            .last()
            .expect("region table must be non-empty");
        assert_eq!(crease_entry.kind, RegionKind::Crease);
        assert_eq!(
            crease_entry.move_range.end,
            tp.moves.len(),
            "crease node must be the tail of the stitched toolpath"
        );
        for entry in &report.region_table[..report.region_table.len() - 1] {
            assert!(
                matches!(entry.kind, RegionKind::Band(_)),
                "every non-trailing entry must be a band, got {entry:?}"
            );
            assert!(
                entry.move_range.end <= crease_entry.move_range.start,
                "crease node must come after every routed band"
            );
        }
    }

    #[test]
    fn region_table_ranges_are_within_bounds_and_non_overlapping() {
        let (mesh, index, cutter, params, planner) = trench_claims_fixture();
        let claims_cfg = ClaimsConfig {
            territory_stock: None,
            // Not under test here (region-table range invariants); the
            // default arm is the safe choice.
            crease_reference: CreaseReference::SelfProbe,
            rest_field_params: RestFieldParams {
                cell_mm: 0.5,
                min_valley_depth: 0.05,
                // Force pencil routing over clearing (mirrors
                // `rest_field::tests::v_valley_yields_one_centerline`) so
                // the detected ridge survives as a centerline to claim.
                route_width_factor: 10.0,
                ..RestFieldParams::default()
            },
            min_rest_depth_mm: 0.02,
            // S4 not under test here — off, the safe/default choice.
            territory_clip: false,
            crease_hookup_mm: 5.0,
        };
        let never_cancel = || false;

        let (tp, _anns, report) = unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            5.0,
            -5.0,
            &params,
            &planner,
            None,
            None,
            Some(&claims_cfg),
            None,
            &never_cancel,
        )
        .unwrap();

        assert!(
            !report.region_table.is_empty(),
            "expected at least the crease node in the region table"
        );
        let mut prev_end = 0usize;
        for entry in &report.region_table {
            assert!(
                entry.move_range.start <= entry.move_range.end,
                "malformed range {:?}",
                entry.move_range
            );
            assert!(
                entry.move_range.start >= prev_end,
                "region ranges must not overlap: {:?} starts before the \
                 previous entry ended at {prev_end}",
                entry.move_range
            );
            assert!(
                entry.move_range.end <= tp.moves.len(),
                "region range {:?} escaped the {}-move toolpath",
                entry.move_range,
                tp.moves.len()
            );
            prev_end = entry.move_range.end;
        }
    }

    // ── Build-list item 3: stock-referenced crease claims ───────────────

    /// [`CreaseReference::MachinedStock`] sentry (process-proof build-list
    /// item 3): the orchestrator must actually swap the crease detector's
    /// reference to the supplied stock (not silently keep using the
    /// self-probe), and must fall back to the self-probe cleanly — no
    /// panic, a `ClaimsReport` still comes back — when the caller opts
    /// into `MachinedStock` without a `territory_stock` in scope.
    ///
    /// Reuses `trench_claims_fixture` with a fresh, UNCUT solid-brick
    /// stock (`TriDexelStock::from_bounds`, top = the mesh's own bbox
    /// ceiling) rather than a hand-built stock matched to the mesh exactly
    /// — sidesteps needing byte-exact agreement with the detector's own
    /// probe-drop math. The brick's flat top sits at the SAME height the tip
    /// cutter itself contacts outside the trench, so this assigns a
    /// *smaller* rest magnitude at the trench than the self-probe (whose
    /// reference reaches the true floor) — assertions therefore stay off
    /// magnitude comparisons between arms (fixture-dependent, not a
    /// general property of `MachinedStock`) and instead check the
    /// structural contract: the swap actually ran, and the fallback
    /// actually fell back.
    #[test]
    fn crease_reference_machined_stock_runs_and_falls_back_without_stock() {
        let (mesh, index, cutter, params, planner) = trench_claims_fixture();
        let never_cancel = || false;
        let rf_params = || RestFieldParams {
            cell_mm: 0.5,
            min_valley_depth: 0.05,
            // Force pencil routing over clearing, same as the other claims
            // tests, so the detected ridge survives as a centerline.
            route_width_factor: 10.0,
            ..RestFieldParams::default()
        };

        // Self-probe baseline (S1/S2 default arm) — establishes what "the
        // detector ran at all" looks like on this fixture.
        let self_probe_cfg = ClaimsConfig {
            territory_stock: None,
            crease_reference: CreaseReference::SelfProbe,
            rest_field_params: rf_params(),
            min_rest_depth_mm: 0.02,
            // S4 not under test here — off, the safe/default choice.
            territory_clip: false,
            crease_hookup_mm: 5.0,
        };
        let (_tp, _anns, report_self) = unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            5.0,
            -5.0,
            &params,
            &planner,
            None,
            None,
            Some(&self_probe_cfg),
            None,
            &never_cancel,
        )
        .expect("self-probe crease detection must complete");
        let claims_self = report_self.claims.expect("claims pipeline must have run");

        // MachinedStock arm, stock present: must complete, must still read
        // `RestIslands` territory (driven by `territory_stock`, independent
        // of which reference fed the detector), and must actually have
        // claimed the trench crease under the STOCK reference (proves the
        // swap ran — a silently-ignored reference would still claim
        // nothing-changed output, which this mesh's single crease can't
        // distinguish from "swap didn't happen" on its own, so the real
        // proof is the fallback-vs-stock split below).
        let stock = crate::dexel_stock::TriDexelStock::from_bounds(&mesh.bbox, 1.0);
        let stock_cfg = ClaimsConfig {
            territory_stock: Some(&stock),
            crease_reference: CreaseReference::MachinedStock,
            rest_field_params: rf_params(),
            min_rest_depth_mm: 0.02,
            // S4 not under test here — off, the safe/default choice.
            territory_clip: false,
            crease_hookup_mm: 5.0,
        };
        let (_tp2, _anns2, report_stock) = unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            5.0,
            -5.0,
            &params,
            &planner,
            None,
            None,
            Some(&stock_cfg),
            None,
            &never_cancel,
        )
        .expect("machined-stock crease detection must complete");
        let claims_stock = report_stock.claims.expect("claims pipeline must have run");
        assert_eq!(
            claims_stock.territory_mode,
            ClaimTerritoryMode::RestIslands,
            "a territory_stock in scope must read RestIslands territory mode \
             regardless of which arm fed the crease detector"
        );
        // Direct evidence the swap actually fed `detect_rest_valleys`: at
        // the trench center, the solid brick's flat top sits well above
        // where the tip cutter can descend into the (too-narrow-to-bridge)
        // Gaussian dip, so `stock_top - pencil_drop` must read a
        // comfortably-over-threshold rest value there — independent of
        // whatever the ridge-tracing pipeline does with it downstream.
        let rest_grid_stock = report_stock
            .rest_grid
            .as_ref()
            .expect("rest grid must be carried through");
        let trench_center_rest = rest_grid_index(rest_grid_stock, 15.0, 0.0)
            .and_then(|i| rest_grid_stock.rest.get(i))
            .copied()
            .expect("trench center must be a trusted grid cell");
        assert!(
            trench_center_rest.is_finite() && f64::from(trench_center_rest) > 0.05,
            "expected the stock-referenced field to read material at the \
             trench center, got {trench_center_rest}"
        );
        assert!(
            claims_stock.crease_path_count > 0,
            "expected the trench crease to still claim cut paths under the \
             stock-referenced detector, got {claims_stock:?}"
        );
        assert!(claims_stock.crease_path_length_mm > 0.0);

        // No-stock fallback: `MachinedStock` with `territory_stock: None`
        // has no stock to reference, so it must fall back to the
        // self-probe rather than error or panic — and the fallback must
        // reproduce the self-probe run byte-for-byte (same code path),
        // proving the fallback actually engaged rather than, say, silently
        // detecting nothing.
        let fallback_cfg = ClaimsConfig {
            territory_stock: None,
            crease_reference: CreaseReference::MachinedStock,
            rest_field_params: rf_params(),
            min_rest_depth_mm: 0.02,
            // S4 not under test here — off, the safe/default choice.
            territory_clip: false,
            crease_hookup_mm: 5.0,
        };
        let (_tp3, _anns3, report_fallback) = unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            5.0,
            -5.0,
            &params,
            &planner,
            None,
            None,
            Some(&fallback_cfg),
            None,
            &never_cancel,
        )
        .expect("MachinedStock with no territory_stock must fall back cleanly, not error");
        let claims_fallback = report_fallback
            .claims
            .expect("claims pipeline must have run even on fallback");
        assert_eq!(
            claims_fallback.territory_mode,
            ClaimTerritoryMode::Full,
            "no territory_stock means territory stays Full even under MachinedStock"
        );
        assert_eq!(
            claims_fallback.crease_path_count, claims_self.crease_path_count,
            "the no-stock fallback must reproduce the self-probe run exactly"
        );
        assert!(
            (claims_fallback.crease_path_length_mm - claims_self.crease_path_length_mm).abs()
                < 1e-9,
            "fallback length {} must match self-probe length {}",
            claims_fallback.crease_path_length_mm,
            claims_self.crease_path_length_mm
        );
    }

    // ── S4 rest-territory confinement (mask-AND) ────────────────────────

    /// (a) `territory_clip: false` must be a total no-op: even with a
    /// territory stock in scope and a saturating `min_rest_depth_mm` (so
    /// every covered cell would read measured-skippable if the mask-AND
    /// ran), leaving `territory_clip` off must leave
    /// `territory_masked_cells`/`territory_masked_area_mm2` at zero and
    /// `post_territory_region_count` exactly the decompose count — the
    /// mask-AND never ran.
    #[test]
    fn territory_clip_off_is_a_total_no_op() {
        let (mesh, index, cutter, params, planner) = trench_claims_fixture();
        let stock = crate::dexel_stock::TriDexelStock::from_bounds(&mesh.bbox, 1.0);
        let never_cancel = || false;

        let cfg = ClaimsConfig {
            territory_stock: Some(&stock),
            crease_reference: CreaseReference::MachinedStock,
            rest_field_params: RestFieldParams {
                cell_mm: 0.5,
                min_valley_depth: 0.05,
                route_width_factor: 10.0,
                ..RestFieldParams::default()
            },
            min_rest_depth_mm: 1.0e6,
            territory_clip: false,
            crease_hookup_mm: 5.0,
        };
        let (_tp, _anns, report) = unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            5.0,
            -5.0,
            &params,
            &planner,
            None,
            None,
            Some(&cfg),
            None,
            &never_cancel,
        )
        .unwrap();
        let claims = report.claims.expect("claims pipeline must have run");
        assert_eq!(
            claims.territory_masked_cells, 0,
            "territory_clip: false must never mask coverage"
        );
        assert_eq!(claims.territory_masked_area_mm2, 0.0);
        assert_eq!(
            claims.post_territory_region_count, report.decompose.region_count,
            "with S4 off, post_territory_region_count is exactly the decompose count"
        );
    }

    /// (b) `territory_clip: true` WITHOUT a `territory_stock` is inert —
    /// the per-cell rest keep-mask never exists, so there is nothing
    /// to AND (`ClaimsConfig::territory_clip` doc: requires a
    /// territory_stock). Completes normally under either crease
    /// reference; `territory_masked_cells` stays 0.
    #[test]
    fn territory_clip_without_stock_is_inert() {
        let (mesh, index, cutter, params, planner) = trench_claims_fixture();
        let never_cancel = || false;

        let cfg = ClaimsConfig {
            territory_stock: None,
            crease_reference: CreaseReference::SelfProbe,
            rest_field_params: RestFieldParams {
                cell_mm: 0.5,
                min_valley_depth: 0.05,
                route_width_factor: 10.0,
                ..RestFieldParams::default()
            },
            min_rest_depth_mm: 0.02,
            territory_clip: true,
            crease_hookup_mm: 5.0,
        };
        let (_tp, _anns, report) = unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            5.0,
            -5.0,
            &params,
            &planner,
            None,
            None,
            Some(&cfg),
            None,
            &never_cancel,
        )
        .expect("territory_clip without a territory_stock must complete, not error");
        let claims = report.claims.expect("claims pipeline must have run");
        assert_eq!(
            claims.territory_masked_cells, 0,
            "no territory_stock -> no rest verdict grid -> nothing to AND"
        );
        assert_eq!(claims.territory_masked_area_mm2, 0.0);
    }

    /// (c) `territory_clip: true` under `CreaseReference::MachinedStock`
    /// with a real (not saturating) rest-depth threshold: the same uncut
    /// solid-brick stock as the build-list-3 test with `min_rest_depth_mm`
    /// at a real value (0.05) — the per-cell verdict keeps only cells
    /// where the brick top truly sits ≥0.05mm above the pencil drop (the
    /// trench, not the flat surface flanking it), so the mask-AND confines
    /// coverage to a genuinely smaller-than-the-band rest island rather
    /// than "the whole stock". Compares against the SAME config with
    /// `territory_clip: false` to prove the confinement actually shrank
    /// the emitted toolpath, not just changed telemetry.
    #[test]
    fn territory_clip_confines_bands_to_rest_islands_and_shrinks_the_toolpath() {
        let (mesh, index, cutter, params, planner) = trench_claims_fixture();
        let stock = crate::dexel_stock::TriDexelStock::from_bounds(&mesh.bbox, 1.0);
        let never_cancel = || false;
        let rf_params = || RestFieldParams {
            cell_mm: 0.5,
            min_valley_depth: 0.05,
            route_width_factor: 10.0,
            ..RestFieldParams::default()
        };

        let unclipped_cfg = ClaimsConfig {
            territory_stock: Some(&stock),
            crease_reference: CreaseReference::MachinedStock,
            rest_field_params: rf_params(),
            min_rest_depth_mm: 0.05,
            territory_clip: false,
            crease_hookup_mm: 5.0,
        };
        let (tp_unclipped, _anns, report_unclipped) = unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            5.0,
            -5.0,
            &params,
            &planner,
            None,
            None,
            Some(&unclipped_cfg),
            None,
            &never_cancel,
        )
        .unwrap();
        let claims_unclipped = report_unclipped
            .claims
            .expect("claims pipeline must have run");
        assert_eq!(
            claims_unclipped.territory_masked_cells, 0,
            "the unconfined baseline run must not itself mask anything"
        );
        let rest_regions = report_unclipped
            .rest_regions
            .as_ref()
            .expect("rest_regions must be carried through under MachinedStock");
        assert!(
            !rest_regions.is_empty(),
            "expected the detector to find a non-empty rest island on this \
             fixture — the fixture is meaningless for S4 otherwise"
        );

        let clipped_cfg = ClaimsConfig {
            territory_stock: Some(&stock),
            crease_reference: CreaseReference::MachinedStock,
            rest_field_params: rf_params(),
            min_rest_depth_mm: 0.05,
            territory_clip: true,
            crease_hookup_mm: 5.0,
        };
        let (tp_clipped, _anns2, report_clipped) = unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            5.0,
            -5.0,
            &params,
            &planner,
            None,
            None,
            Some(&clipped_cfg),
            None,
            &never_cancel,
        )
        .unwrap();
        let claims_clipped = report_clipped
            .claims
            .expect("claims pipeline must have run");

        assert!(
            claims_clipped.territory_masked_cells >= 1,
            "expected the S4 mask-AND to remove at least one \
             measured-skippable covered cell, got {claims_clipped:?}"
        );
        assert!(
            tp_clipped.moves.len() < tp_unclipped.moves.len(),
            "confining bands to rest islands must shrink the emitted \
             toolpath: confined={} unconfined={}",
            tp_clipped.moves.len(),
            tp_unclipped.moves.len()
        );
    }
}
