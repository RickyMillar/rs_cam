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
//! S2 (`planning/unified_v3_design.md` §2.1 step 4 / §0.a) adds a
//! REGION-level territory filter on top of S1's per-cell rest
//! MEASUREMENT: with a `ClaimsConfig::territory_stock` in scope, every
//! covered classification cell gets a per-cell rest verdict (stock top −
//! pencil drop vs `min_rest_depth_mm`), and after `decompose` produces its
//! conditioned band islands, any WHOLE island whose measured rest share
//! falls below `ClaimsConfig::min_region_rest_share` is dropped before
//! Step 4 generates it — never a cell hole (the fragmentation lesson: cell
//! masking shredded the same decomposition 4 → 35 regions on 0.2% of
//! cells). `min_region_rest_share: 0.0` (default) is the S1 no-op.
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
use crate::rest_field::{RestFieldParams, RestGrid, RestReference, detect_rest_valleys};
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

/// In-op pencil-claims pipeline inputs (v3 S1, `planning/unified_v3_design.md`
/// §2.1). `claims: None` on [`unified_finish_toolpath_with_cancel`]
/// reproduces the pre-v3 op exactly: no detector run, no crease claims, no
/// crease node — the Wave 3 A/B harness pins this as the baseline.
pub struct ClaimsConfig<'a> {
    /// Machined prior stock for the TERRITORY mask ONLY (rest islands,
    /// step 4 below) — already XY-frame-guarded by the caller; `None` →
    /// territory stays full.
    ///
    /// Deliberately NOT a crease-detection reference: the S1 A/B on the
    /// wanaka rough→finish chain proved that a stock-referenced rest field
    /// reads the ROUGHING TERRACE pattern as a dendritic phantom "crease"
    /// network — universal claims then carve corridors the emitter never
    /// cuts (10k+ new uncut mid-steep columns). Claims are GEOMETRIC
    /// (design-surface valleys via the analytic self-probe, below);
    /// territory is MATERIAL (this stock).
    pub territory_stock: Option<&'a crate::dexel_stock::TriDexelStock>,
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
    /// S2 region-level territory filter (design doc §2.1 step 4 / §0.a):
    /// after `decompose`, a conditioned band island is DROPPED WHOLE when
    /// the fraction of its own covered classification cells that still
    /// carry measured rest (the per-cell verdict from `min_rest_depth_mm`
    /// above — untrusted samples count AS rest, never as skippable) falls
    /// below this dial. `0.0` (default) turns the filter off — S1
    /// behavior, byte-identical: no region is ever dropped. Requires a
    /// `territory_stock`; without one the per-cell verdict grid never
    /// exists and this dial is inert. Cell holes are NEVER punched — the
    /// Step 2.5 fragmentation lesson (naive cell-level masking shredded
    /// this same conditioned decomposition 4 → 35 regions from 0.2% of
    /// cells) is exactly why this filter only ever drops WHOLE regions,
    /// after `decompose` has already done its conditioning.
    pub min_region_rest_share: f64,
}

/// Which territory the claims pipeline banded (design doc §2.1 step 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClaimTerritoryMode {
    /// No stock reference in play: banding covers the whole classified
    /// surface, same as the pre-v3 op. Creases still claim corridors.
    #[default]
    Full,
    /// Stock reference in play: skippable territory is MEASURED
    /// (`ClaimsReport::rest_excluded_cells`) at the cell level, but that
    /// per-cell measurement is never applied as cell holes (fragments the
    /// conditioned decomposition — see `ClaimsConfig::min_region_rest_share`
    /// doc). S2 lands region-LEVEL application instead: whole conditioned
    /// islands whose measured rest share is below
    /// `ClaimsConfig::min_region_rest_share` are dropped after `decompose`;
    /// `0.0` (default) keeps this identical to the S1 telemetry-only
    /// behavior.
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

/// Claims-pipeline telemetry (design doc §2.1, R2 "pencil over-claiming").
/// `None` on [`UnifiedFinishReport::claims`] when the claims pipeline never
/// ran (`claims: None` at the call site).
#[derive(Debug, Clone, Copy, Default)]
pub struct ClaimsReport {
    pub territory_mode: ClaimTerritoryMode,
    /// Classification cells the rest-island measurement found skippable
    /// (stock top − pencil drop below the dial). TELEMETRY ONLY — coverage
    /// itself is NEVER mutated by this scalar: cell-level masking fragments
    /// the conditioned decomposition at dendritic necks (live 2026-07-13,
    /// 4 → 35 regions from 0.2 % of cells). S2's region-level filter below
    /// consumes the underlying PER-CELL verdict grid directly (not this
    /// count) when deciding whether to drop a whole region. Always 0 under
    /// `ClaimTerritoryMode::Full`.
    pub rest_excluded_cells: usize,
    /// Claimed cut paths emitted (centerline + width-capped offset passes,
    /// summed across every crease with `corridor.is_some()`).
    pub crease_path_count: usize,
    /// Total cutting length (mm) of the crease node's `FinishingCut` moves.
    pub crease_path_length_mm: f64,
    /// `RestFieldReport::coverage()` — traced skeleton fraction that
    /// survived the length gate. Independent of banding territory.
    pub detector_coverage: f64,
    /// S2 region-level territory filter results (design doc §2.1 step 4 /
    /// §0.a): count of conditioned band islands dropped WHOLE because
    /// their measured rest share fell below
    /// `ClaimsConfig::min_region_rest_share`. Always 0 when the dial is
    /// `0.0` or no `territory_stock` was supplied — the filter never ran.
    /// `UnifiedFinishReport::decompose`'s `region_count` is deliberately
    /// left as the decompose-time truth (what conditioning produced,
    /// before any S2 drop) rather than corrected down — these two S2
    /// counters are where the drop shows up instead.
    pub regions_dropped: usize,
    /// Summed polygon area (mm²) of the dropped regions. Always 0.0 under
    /// the same conditions as `regions_dropped`.
    pub dropped_area_mm2: f64,
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
    let covered: Vec<bool> = surface
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
    // Per-cell S2 verdict grid (row-major, aligned with the classification
    // grid): `Some(true)` at index `i` means covered cell `i` still carries
    // rest OR is untrusted (i.e. NOT measured-skippable). Built alongside
    // `rest_excluded_cells` below; `None` when there's no territory stock
    // (or claims didn't run at all) — the S2 filter (Step 3.4) is then
    // structurally inert regardless of `ClaimsConfig::min_region_rest_share`.
    let mut rest_ok: Option<Vec<bool>> = None;
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
        // Crease detection ALWAYS runs against the analytic self-probe —
        // geometric valleys of the design surface, the thing pencil
        // corridors are FOR. See `ClaimsConfig::territory_stock` for why
        // the machined stock must not be the crease reference.
        let probe = crate::tool::BallEndmill::new(
            crate::pencil::SURFACE_PROBE_BALL_DIAMETER_MM,
            crate::pencil::SURFACE_PROBE_BALL_LENGTH_MM,
        );
        let rf = detect_rest_valleys(
            mesh,
            index,
            cutter,
            RestReference::Cutter {
                tool: &probe,
                is_surface_probe: true,
            },
            &rf_params,
        );
        check_cancel(cancel)?;

        // Territory = rest islands (design doc §2.1 step 4), measured
        // directly as stock top − pencil drop per classification cell.
        // The analytic run's `surface_z` field IS the pencil drop, so no
        // second detector pass is needed. Untrusted samples keep their
        // coverage (`ClaimsConfig::min_rest_depth_mm` doc).
        let territory_mode = if cfg.territory_stock.is_some() {
            ClaimTerritoryMode::RestIslands
        } else {
            ClaimTerritoryMode::Full
        };
        // Territory measurement is TELEMETRY-ONLY in S1. Two live
        // validations (2026-07-13) killed cell-level masking outright:
        // the naive nearest-cell mask skipped ~340 mm² via cross-grid
        // misregistration, and even the conservative footprint-max /
        // neighbour-agreeing variant — down to ~0.2 % of cells,
        // PHYSICALLY-TRUE "tool can't improve this" readings — punched
        // holes at the steep NECKS of the dendritic band mask and
        // fragmented the conditioned decomposition 4 → 35 regions, each
        // fragment ring-cascading from its own boundary (+2.4 k columns
        // of >0.5 mm leftover at fragment edges). Any sub-region hole is
        // poison to `decompose`'s conditioning. Region-LEVEL territory
        // (drop whole conditioned islands that measure ≥X% skippable —
        // decompose-then-filter) is S2 scope; the counter below is its
        // sizing telemetry.
        let mut rest_excluded_cells = 0usize;
        if let Some(stock) = cfg.territory_stock {
            // `ok[i]` starts `false` for every cell (covers both the
            // "not covered" don't-care value the module doc promises, and
            // the "measured skippable" verdict below); it flips `true`
            // only when a covered cell is proven to still carry rest.
            let mut ok = vec![false; covered.len()];
            let half = cell * 0.5;
            let offsets = [
                (0.0, 0.0),
                (-half, -half),
                (-half, half),
                (half, -half),
                (half, half),
            ];
            for (i, cov) in covered.iter().enumerate() {
                if !*cov {
                    continue;
                }
                let row = i / cols;
                let col = i % cols;
                let x = origin_x + col as f64 * cell;
                let y = origin_y + row as f64 * cell;
                let mut drop_min = f64::INFINITY;
                let mut top_max = f64::NEG_INFINITY;
                for (dx, dy) in offsets {
                    if let Some(d) = rest_grid_surface_z(&rf.rest_grid, x + dx, y + dy) {
                        drop_min = drop_min.min(d);
                    }
                    if let Some(t) = stock
                        .z_grid
                        .world_to_cell(x + dx, y + dy)
                        .and_then(|(r, c)| stock.z_grid.top_z_at(r, c))
                    {
                        top_max = top_max.max(f64::from(t));
                    }
                }
                let measured_skippable = drop_min.is_finite()
                    && top_max.is_finite()
                    && top_max - drop_min < cfg.min_rest_depth_mm;
                if measured_skippable {
                    rest_excluded_cells += 1;
                } else if let Some(cell_ok) = ok.get_mut(i) {
                    // Rest survives, OR the sample is untrusted (NaN drop /
                    // outside the stock grid) — `min_rest_depth_mm`'s doc:
                    // untrusted samples keep coverage, so they read as
                    // "not measured-skippable" here too.
                    *cell_ok = true;
                }
            }
            rest_ok = Some(ok);
        }

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
            rest_excluded_cells,
            crease_path_count: 0,
            crease_path_length_mm: 0.0,
            detector_coverage: rf.report.coverage(),
            // Filled in by Step 3.4 once the S2 region filter has run.
            regions_dropped: 0,
            dropped_area_mm2: 0.0,
        });
        // §2.4 carry-through: the detector's field + region polygons ride
        // the report so the adapter can attach them to the generated
        // toolpath (GUI heatmap, DerivedRestRegions, probes).
        claims_rest_grid = Some(std::sync::Arc::new(rf.rest_grid));
        claims_rest_regions = Some(std::sync::Arc::new(rf.region_polygons));
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
    let mut planned = decompose(&surface.slope_map, &covered, &[], cutter.radius(), planner);
    check_cancel(cancel)?;

    // ── Step 3.4: S2 region-level territory filter (design doc §2.1 step 4
    // / §0.a) ────────────────────────────────────────────────────────────
    // Drop whole conditioned band islands whose measured rest share falls
    // below `ClaimsConfig::min_region_rest_share` — NEVER cell holes (the
    // Step 2.5 fragmentation lesson: naive cell-level masking shredded this
    // same decomposition 4 → 35 regions from 0.2% of cells is exactly why
    // this filter only ever drops WHOLE regions). Filtering happens here,
    // before Step 4's per-region generation loop, so a dropped island
    // generates nothing, never enters `region_table`, and the router
    // (Step 5) never sees it. `DecomposeStats::region_count` is
    // deliberately left as the decompose-time truth (what conditioning
    // produced, before any S2 drop) — see `ClaimsReport::regions_dropped`
    // doc for why the drop counters live there instead.
    let mut regions_dropped = 0usize;
    let mut dropped_area_mm2 = 0.0f64;
    if let (Some(cfg), Some(ok)) = (claims, rest_ok.as_ref())
        && cfg.min_region_rest_share > 0.0
    {
        let rows = surface.rows();
        planned.regions.retain(|region| {
            let Some(share) = region_rest_share(
                &region.polygon,
                &covered,
                ok,
                rows,
                cols,
                origin_x,
                origin_y,
                cell,
            ) else {
                // No covered cells inside this region at all — no evidence
                // either way. Never drop on an empty denominator (safety:
                // §2.1 step 4's "keep on no evidence" rule).
                return true;
            };
            if share < cfg.min_region_rest_share {
                let area = region.polygon.area();
                regions_dropped += 1;
                dropped_area_mm2 += area;
                tracing::debug!(
                    band = ?region.band,
                    share,
                    area_mm2 = area,
                    "unified_finish S2: dropping region below min_region_rest_share"
                );
                false
            } else {
                true
            }
        });
    }
    if let Some(report) = claims_report.as_mut() {
        report.regions_dropped = regions_dropped;
        report.dropped_area_mm2 = dropped_area_mm2;
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
                scallop_toolpath_structured_annotated_with_cancel(
                    mesh,
                    index,
                    cutter,
                    &sp,
                    debug,
                    Some(&region_set),
                    cancel,
                )?
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

        if let (Some(link), Some(entry)) = (incoming_surface, rp.entry) {
            for p in &link.pts {
                stitched.feed_to_with_intent(*p, params.feed_rate, MoveIntent::Linking);
            }
            stitched.feed_to_with_intent(entry, params.feed_rate, MoveIntent::Linking);
        }

        let offset = stitched.moves.len();
        let kept = rp.tp.moves.len().saturating_sub(head + tail);
        stitched
            .moves
            .extend(rp.tp.moves.iter().skip(head).take(kept).cloned());
        let end = stitched.moves.len();
        region_table.push(RegionTableEntry {
            kind: RegionKind::Band(rp.band),
            move_range: offset..end,
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

    report.claims = claims_report;
    report.region_table = region_table;

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

// ── Claims pipeline internals (v3 S1/S2) ────────────────────────────────

/// Nearest-cell sample of a [`RestGrid`] at world `(x, y)`. The rest grid's
/// own origin/cell size (from [`detect_rest_valleys`]) is independent of
/// the classification surface's grid, so this is deliberately a
/// round-to-nearest lookup rather than an index computed from the caller's
/// grid geometry — mirrors [`RestReference::Stock`]'s own nearest-cell-only
/// contract (never interpolate a dexel top across a steep wall). `None`
/// outside the grid or on an untrusted (`NaN`) cell.
fn rest_grid_index(grid: &RestGrid, x: f64, y: f64) -> Option<usize> {
    if grid.nx == 0 || grid.ny == 0 || grid.cell_mm <= 0.0 {
        return None;
    }
    let col = ((x - grid.origin_x) / grid.cell_mm).round();
    let row = ((y - grid.origin_y) / grid.cell_mm).round();
    if col < 0.0 || row < 0.0 {
        return None;
    }
    let (col, row) = (col as usize, row as usize);
    if col >= grid.nx || row >= grid.ny {
        return None;
    }
    Some(row * grid.nx + col)
}

/// Nearest-cell pencil-drop height from the claims detector's grid —
/// `None` outside the grid or on untrusted (`NaN`) cells. Feeds the
/// territory mask's direct `stock top − drop` rest measurement.
fn rest_grid_surface_z(grid: &RestGrid, x: f64, y: f64) -> Option<f64> {
    rest_grid_index(grid, x, y)
        .and_then(|i| grid.surface_z.get(i))
        .copied()
        .filter(|v| !v.is_nan())
        .map(f64::from)
}

/// Bounding box (min_x, min_y, max_x, max_y) of `polygon`'s exterior.
/// `Polygon2` has no bbox helper of its own (checked `polygon.rs`), so this
/// scans the exterior vertices directly — a cheap one-off pre-clip for
/// [`region_rest_share`]'s cell scan, not a general-purpose utility.
/// `None` for a degenerate (empty) exterior.
fn polygon_bbox(polygon: &Polygon2) -> Option<(f64, f64, f64, f64)> {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for p in &polygon.exterior {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }
    (min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite())
        .then_some((min_x, min_y, max_x, max_y))
}

/// Fraction of `polygon`'s own COVERED classification cells that still
/// carry measured rest (S2 region-level territory filter, design doc
/// §2.1 step 4 / §0.a): `n_rest / n_cov` over cells whose CENTER lands
/// inside `polygon` (the same `P2::new(x, y)` center convention Step 2's
/// boundary loop and [`band_z_range`] both use). Only cells inside
/// `polygon`'s own bounding box are scanned — cheap since `Polygon2` has no
/// dedicated spatial index, and correctness still comes from the exact
/// `contains_point` test below, not the clip itself. Containment uses
/// `Polygon2::contains_point` directly (holes-aware) rather than routing a
/// single polygon through a `RegionSet` — `RegionSet::contains` is just a
/// `.any()` wrapper over exactly this call for a one-element slice, so the
/// direct call is strictly cheaper and equally correct.
///
/// `None` when the region contains no covered cells at all (`n_cov == 0`,
/// including a polygon whose bounding box doesn't overlap the grid) — the
/// caller's contract (§2.1 step 4's safety rule) is to KEEP the region on
/// `None` rather than divide by zero or guess.
#[allow(clippy::too_many_arguments)] // grid geometry is 5 irreducible scalars, not groupable without a new type
fn region_rest_share(
    polygon: &Polygon2,
    covered: &[bool],
    rest_ok: &[bool],
    rows: usize,
    cols: usize,
    origin_x: f64,
    origin_y: f64,
    cell: f64,
) -> Option<f64> {
    if rows == 0 || cols == 0 || cell <= 0.0 {
        return None;
    }
    let (min_x, min_y, max_x, max_y) = polygon_bbox(polygon)?;
    let grid_max_x = origin_x + (cols - 1) as f64 * cell;
    let grid_max_y = origin_y + (rows - 1) as f64 * cell;
    if max_x < origin_x || min_x > grid_max_x || max_y < origin_y || min_y > grid_max_y {
        return None;
    }
    let col_lo = ((min_x - origin_x) / cell)
        .floor()
        .clamp(0.0, (cols - 1) as f64) as usize;
    let col_hi = ((max_x - origin_x) / cell)
        .ceil()
        .clamp(0.0, (cols - 1) as f64) as usize;
    let row_lo = ((min_y - origin_y) / cell)
        .floor()
        .clamp(0.0, (rows - 1) as f64) as usize;
    let row_hi = ((max_y - origin_y) / cell)
        .ceil()
        .clamp(0.0, (rows - 1) as f64) as usize;

    let mut n_cov = 0usize;
    let mut n_rest = 0usize;
    for row in row_lo..=row_hi {
        for col in col_lo..=col_hi {
            let i = row * cols + col;
            if !covered.get(i).copied().unwrap_or(false) {
                continue;
            }
            let x = origin_x + col as f64 * cell;
            let y = origin_y + row as f64 * cell;
            if !polygon.contains_point(&P2::new(x, y)) {
                continue;
            }
            n_cov += 1;
            if rest_ok.get(i).copied().unwrap_or(true) {
                n_rest += 1;
            }
        }
    }
    if n_cov == 0 {
        return None;
    }
    Some(n_rest as f64 / n_cov as f64)
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
        let planner = FinishPlannerParams::for_tool(cutter.radius());
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
        let planner = FinishPlannerParams::for_tool(cutter.radius());
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
        let planner = FinishPlannerParams::for_tool(cutter.radius());

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
        let planner = FinishPlannerParams::for_tool(cutter.radius());
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
        let planner = FinishPlannerParams::for_tool(cutter.radius());
        (mesh, index, cutter, params, planner)
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
            // S2 dial off: byte-identical S1 behavior, no region is ever
            // dropped (asserted below).
            min_region_rest_share: 0.0,
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
        // carved out of the bands (width-honest carving is S2 scope), so
        // decompose sees no creases.
        assert_eq!(report.decompose.claimed_creases, 0);
        // No territory stock: territory stays full (design doc §2.1
        // step 4), so no cells are measured skippable.
        assert_eq!(claims.territory_mode, ClaimTerritoryMode::Full);
        assert_eq!(claims.rest_excluded_cells, 0);
        // S2 dial off (and no territory stock either): the region-level
        // filter never engages.
        assert_eq!(claims.regions_dropped, 0);
        assert_eq!(claims.dropped_area_mm2, 0.0);

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
            // Not under test here (region-table range invariants); off is
            // the safe/default choice.
            min_region_rest_share: 0.0,
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

    // ── S2 region-level territory filter ────────────────────────────────

    /// Direct unit test of the share math [`region_rest_share`] factors
    /// out of the orchestrator (preferred over only exercising it through
    /// a full generation run — see the module's S2 doc). A 4×4 grid,
    /// cell 1.0mm, origin (0,0): every cell covered; the first two rows
    /// (row-major indices 0..8) carry rest, the last two (8..16) don't.
    #[test]
    fn region_rest_share_computes_fraction_and_keeps_on_no_evidence() {
        let rows = 4;
        let cols = 4;
        let covered = vec![true; rows * cols];
        let mut rest_ok = vec![false; rows * cols];
        for i in 0..8 {
            if let Some(v) = rest_ok.get_mut(i) {
                *v = true;
            }
        }

        // Polygon margin kept off every grid line (cell centers sit at
        // integer coordinates here) so no cell center lands exactly on a
        // boundary edge, where point-in-polygon is ambiguous.
        let whole = Polygon2::rectangle(-1.0, -1.0, 5.0, 5.0);
        let share = region_rest_share(&whole, &covered, &rest_ok, rows, cols, 0.0, 0.0, 1.0)
            .expect("polygon covers the whole grid");
        assert!(
            (share - 0.5).abs() < 1e-9,
            "expected 50% rest share over the whole grid, got {share}"
        );

        // Confined to the rest-carrying rows (y = 0, 1): full share.
        let top_half = Polygon2::rectangle(-1.0, -1.0, 5.0, 1.5);
        let share_top = region_rest_share(&top_half, &covered, &rest_ok, rows, cols, 0.0, 0.0, 1.0)
            .expect("polygon covers the rest-carrying rows");
        assert!(
            (share_top - 1.0).abs() < 1e-9,
            "expected 100% rest share confined to the rest rows, got {share_top}"
        );

        // Confined to the skippable rows (y = 2, 3): zero share.
        let bottom_half = Polygon2::rectangle(-1.0, 1.5, 5.0, 5.0);
        let share_bottom =
            region_rest_share(&bottom_half, &covered, &rest_ok, rows, cols, 0.0, 0.0, 1.0)
                .expect("polygon covers the skippable rows");
        assert!(
            share_bottom.abs() < 1e-9,
            "expected 0% rest share confined to the skippable rows, got {share_bottom}"
        );

        // Entirely off-grid: no covered cells found at all — the caller's
        // contract is "no evidence, never drop", not a spurious 0/0 share.
        let off_grid = Polygon2::rectangle(100.0, 100.0, 104.0, 104.0);
        assert!(
            region_rest_share(&off_grid, &covered, &rest_ok, rows, cols, 0.0, 0.0, 1.0).is_none(),
            "off-grid polygon must report no evidence"
        );
    }

    /// Orchestrator-level S2 test: with the dial at `0.0`, engaging a
    /// territory stock changes nothing (`regions_dropped` stays 0); with
    /// the SAME territory stock and the dial raised, regions actually get
    /// dropped and the emitted toolpath shrinks accordingly.
    ///
    /// No prior test anywhere in this crate exercises `RestReference::Stock`
    /// (checked before writing this), so there's no "stamp a stock to match
    /// the mesh exactly" fixture to reuse, and building one by hand would
    /// require replicating the claims detector's own probe-drop math to
    /// avoid a flaky near-miss on `top_max - drop_min`. Sidestepping that:
    /// this test uses a fresh, UNCUT solid-brick stock
    /// (`TriDexelStock::from_bounds`, top = the mesh's own bbox ceiling)
    /// together with a deliberately saturating `min_rest_depth_mm` — since
    /// `top_max` and `drop_min` are both always finite over this mesh,
    /// ANY finite gap between them reads as "measured skippable" under a
    /// threshold this large, regardless of the stock's exact height. That
    /// gives the same observable effect as a fully-finished stock (rest_ok
    /// false at every covered cell) without needing byte-exact geometric
    /// agreement between the stock's dexel grid and the detector's probe
    /// grid.
    #[test]
    fn s2_dial_drops_regions_once_territory_reads_uniformly_skippable() {
        let (mesh, index, cutter, params, planner) = trench_claims_fixture();
        let stock = crate::dexel_stock::TriDexelStock::from_bounds(&mesh.bbox, 1.0);
        let never_cancel = || false;

        let off_cfg = ClaimsConfig {
            territory_stock: Some(&stock),
            rest_field_params: RestFieldParams {
                cell_mm: 0.5,
                min_valley_depth: 0.05,
                route_width_factor: 10.0,
                ..RestFieldParams::default()
            },
            min_rest_depth_mm: 1.0e6,
            min_region_rest_share: 0.0,
        };
        let (tp_off, _anns_off, report_off) = unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            5.0,
            -5.0,
            &params,
            &planner,
            None,
            None,
            Some(&off_cfg),
            None,
            &never_cancel,
        )
        .unwrap();
        let claims_off = report_off.claims.expect("claims pipeline must have run");
        assert_eq!(
            claims_off.regions_dropped, 0,
            "0.0 dial must never drop a region, even with territory measured"
        );
        assert_eq!(claims_off.dropped_area_mm2, 0.0);

        let on_cfg = ClaimsConfig {
            min_region_rest_share: 0.9,
            ..off_cfg
        };
        let (tp_on, _anns_on, report_on) = unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            5.0,
            -5.0,
            &params,
            &planner,
            None,
            None,
            Some(&on_cfg),
            None,
            &never_cancel,
        )
        .unwrap();
        let claims_on = report_on.claims.expect("claims pipeline must have run");

        assert!(
            claims_on.regions_dropped > 0,
            "expected at least one region dropped under a saturating rest-depth \
             threshold, got {claims_on:?}"
        );
        assert!(claims_on.dropped_area_mm2 > 0.0);
        assert!(
            tp_on.moves.len() <= tp_off.moves.len(),
            "dropping regions must not increase move count: on={} off={}",
            tp_on.moves.len(),
            tp_off.moves.len()
        );
    }
}
