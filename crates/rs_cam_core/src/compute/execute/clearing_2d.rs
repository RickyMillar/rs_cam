//! The 2.5D clearing family: zigzag, trace, profile, pocket, face and
//! adaptive.
//!
//! Every adapter here steps a 2D region down a ladder of Z levels. Pocket
//! (W3.1), Zigzag, Trace, Profile (W3.2) and Face (W3.3) run on the Layered
//! runner (`layered.rs`): each writes `prepare` (the Z-independent geometry
//! of a region) and `emit_level` (that geometry at one Z). Each reads its
//! rest rule from its capabilities (`Capabilities::rest_use`, W3.4). 2D
//! Adaptive keeps its own ladder: its planner is fallible, polls the
//! cancel, plans each level from that level's stock and returns
//! annotations, which `LayeredOp::emit_level` cannot carry. It reads the
//! stock through the shared step all the same (`RestUse::LevelMask`,
//! `standing_material::level_masks`). Split out of `compute/execute.rs` (P4).

use crate::compute::catalog::{OperationConfig, RestUse};
use crate::compute::operation_configs::{
    CompensationType, PocketConfig, PocketPattern, ProfileConfig, RegionOrdering, TraceConfig,
};
use crate::geo::P2;
use crate::interrupt::CancelCheck;
use crate::polygon::Polygon2;
use crate::tool::MillingCutter;
use crate::toolpath::Toolpath;
use crate::trace::semantic_trace::ToolpathSemanticContext;

use super::OffsetFailures;
use super::layered::{
    LayerCtx, LayeredOp, LevelInfo, LevelRest, ReachBand, effective_levels, run_layered,
};
use super::shared::{generated_with_depth_run_spans, with_depth_run_annotation};
use super::standing_material::{LevelSeed, OffGrid, level_masks};
use super::{ExecutionContext, GeneratedToolpath, OperationError, Prepared};

/// The feeds and heights every Layered adapter here stamps into its moves.
#[derive(Clone, Copy)]
struct LayerFeeds {
    tool_radius: f64,
    safe_z: f64,
    feed_rate: f64,
    plunge_rate: f64,
}

impl LayerFeeds {
    fn new(ctx: &ExecutionContext<'_>, op: &OperationConfig) -> Self {
        Self {
            tool_radius: ctx.tool_def.radius(),
            safe_z: ctx.heights.retract_z,
            feed_rate: op.feed_rate(),
            plunge_rate: op.plunge_rate(),
        }
    }
}

/// Zigzag family adapter (W3.2): [`ZigzagRaster`] on the Layered runner,
/// the kernel of the Zigzag Pocket. The runner polls the cancel per level
/// (pinned by `every_generator_polls_a_preset_cancel_flag_past_the_prologue`).
pub(crate) fn generate_zigzag(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Zigzag, "generate_zigzag");
    let raster = ZigzagRaster {
        stepover: cfg.stepover,
        angle: cfg.angle,
        depth_per_pass: cfg.depth_per_pass,
        region_ordering: cfg.region_ordering,
        rest_use: op.capabilities().rest_use,
        feeds: LayerFeeds::new(ctx, op),
    };
    run_layered(&raster, ctx, prepared)
}

/// Trace family adapter (W3.2): [`TracePass`] on the Layered runner. Trace
/// keeps its own semantic annotation, `annotate_trace_spans`, through the
/// [`LayeredOp::annotate`] hook. The runner polls the cancel per level
/// (pinned by `every_generator_polls_a_preset_cancel_flag_past_the_prologue`).
pub(crate) fn generate_trace(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Trace, "generate_trace");
    let feeds = LayerFeeds::new(ctx, op);
    let pass = TracePass {
        cfg,
        rest_use: op.capabilities().rest_use,
        params: crate::ops::trace_path::TraceParams {
            tool_radius: feeds.tool_radius,
            depth: cfg.depth,
            depth_per_pass: cfg.depth_per_pass,
            feed_rate: feeds.feed_rate,
            plunge_rate: feeds.plunge_rate,
            safe_z: feeds.safe_z,
            compensation: cfg.compensation,
            top_z: ctx.heights.top_z,
        },
    };
    run_layered(&pass, ctx, prepared)
}

/// Profile family adapter (W3.2): [`ProfilePass`] on the Layered runner.
/// The side and the climb direction act in `prepare` and `emit_level`; the
/// tabs go on each level at the final Z ([`LevelInfo::is_final`]). The
/// runner polls the cancel per level (pinned by
/// `every_generator_polls_a_preset_cancel_flag_past_the_prologue`).
///
/// Each Profile pass ends with its own retract to `safe_z`
/// (`Toolpath::emit_closed_contour_with_intent`), so the runner's retract
/// between levels is a no-op here, as it was in the hand-written ladder.
pub(crate) fn generate_profile(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Profile, "generate_profile");
    let feeds = LayerFeeds::new(ctx, op);
    let pass = ProfilePass {
        cfg,
        rest_use: op.capabilities().rest_use,
        base: crate::ops::profile::ProfileParams {
            tool_radius: feeds.tool_radius,
            side: cfg.side,
            cut_depth: 0.0,
            feed_rate: feeds.feed_rate,
            plunge_rate: feeds.plunge_rate,
            safe_z: feeds.safe_z,
            climb: cfg.climb,
            compensate_in_controller: cfg.compensation == CompensationType::InControl,
        },
    };
    run_layered(&pass, ctx, prepared)
}

/// Pocket family adapter (W3.1): the Contour or the Zigzag pattern on the
/// Layered runner (`layered.rs`). The runner owns the ladder, the rest clip
/// per level (G-RESTLEVELS, `RestUse::LevelClip`), one prepare per distinct
/// region, the region order, the cancel poll per level and the spans. The
/// Contour cascade also polls per offset ring (S.5: pocket's own unbounded
/// `loop {}`).
pub(crate) fn generate_pocket(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Pocket, "generate_pocket");
    let feeds = LayerFeeds::new(ctx, op);
    let rest_use = op.capabilities().rest_use;
    match cfg.pattern {
        PocketPattern::Contour => run_layered(
            &PocketContour {
                cfg,
                rest_use,
                feeds,
            },
            ctx,
            prepared,
        ),
        PocketPattern::Zigzag => run_layered(
            &ZigzagRaster {
                stepover: cfg.stepover,
                angle: cfg.angle,
                depth_per_pass: cfg.depth_per_pass,
                region_ordering: cfg.region_ordering,
                rest_use,
                feeds,
            },
            ctx,
            prepared,
        ),
    }
}

/// The Contour pattern of Pocket: the `OffsetRingSet` cascade of each
/// region.
struct PocketContour<'a> {
    cfg: &'a PocketConfig,
    rest_use: RestUse,
    feeds: LayerFeeds,
}

impl LayeredOp for PocketContour<'_> {
    type Input = Polygon2;
    type Geometry = Vec<Vec<P2>>;
    /// One report per piece. Each cascade adds its offset failures and,
    /// when a bound stopped it (Checkpoint C, Q3, F-10), the area it
    /// leaves standing. The runner records it once per region, never once
    /// per level.
    type Report = Vec<crate::ops::pocket::PocketCascadeReport>;

    fn depth_per_pass(&self) -> f64 {
        self.cfg.depth_per_pass
    }

    fn region_ordering(&self) -> RegionOrdering {
        self.cfg.region_ordering
    }

    fn rest_use(&self) -> RestUse {
        self.rest_use
    }

    fn prepare(
        &self,
        region: &[Polygon2],
        layer: &LayerCtx<'_>,
    ) -> Result<(Self::Geometry, Self::Report), OperationError> {
        let mut contours = Vec::new();
        let mut reports = Vec::with_capacity(region.len());
        for piece in region {
            let (rings, report) = crate::ops::pocket::pocket_contours_reported_with_cancel(
                piece,
                layer.tool_radius,
                self.cfg.stepover,
                layer.cancel,
            )?;
            reports.push(report);
            contours.extend(rings);
        }
        Ok((contours, reports))
    }

    fn emit_level(
        &self,
        geometry: &Self::Geometry,
        level: LevelInfo,
        _rest: &LevelRest<'_>,
        _cancel: &dyn CancelCheck,
    ) -> Result<Toolpath, OperationError> {
        let feeds = &self.feeds;
        Ok(crate::ops::pocket::pocket_contours_to_toolpath(
            geometry,
            &crate::ops::pocket::PocketParams {
                tool_radius: feeds.tool_radius,
                stepover: self.cfg.stepover,
                cut_depth: level.z,
                feed_rate: feeds.feed_rate,
                plunge_rate: feeds.plunge_rate,
                safe_z: feeds.safe_z,
                climb: self.cfg.climb,
            },
        ))
    }
}

/// The zigzag raster: the wall inset and the scan lines of each region.
/// ONE kernel for the Zigzag op and the Zigzag pattern of Pocket; each
/// adapter fills the dials from its own config. The rows of each region
/// lie on the scan lattice of its whole outline (B9), as the rows of a
/// rest Face lie on the rows of the facing rectangle (B8).
struct ZigzagRaster {
    stepover: f64,
    angle: f64,
    depth_per_pass: f64,
    region_ordering: RegionOrdering,
    rest_use: RestUse,
    feeds: LayerFeeds,
}

impl LayeredOp for ZigzagRaster {
    type Input = Polygon2;
    type Geometry = Vec<[P2; 2]>;
    type Report = OffsetFailures;

    fn depth_per_pass(&self) -> f64 {
        self.depth_per_pass
    }

    fn region_ordering(&self) -> RegionOrdering {
        self.region_ordering
    }

    fn rest_use(&self) -> RestUse {
        self.rest_use
    }

    fn prepare(
        &self,
        region: &[Polygon2],
        layer: &LayerCtx<'_>,
    ) -> Result<(Self::Geometry, Self::Report), OperationError> {
        // B9: the rows of a rest clip region lie on the rows of the whole
        // outline, with the direction of each fresh row. Without the
        // lattice, row 0 sat on the region's own lowest point, so every
        // rest row moved off the fresh rows and flipped its direction.
        let lattice =
            crate::ops::zigzag::scan_lattice_origin(layer.outline, layer.tool_radius, self.angle);
        let mut lines = Vec::new();
        let mut failures = 0;
        for piece in region {
            let (piece_lines, piece_failures) =
                crate::ops::zigzag::zigzag_lines_on_lattice_reported(
                    piece,
                    layer.tool_radius,
                    self.stepover,
                    self.angle,
                    lattice,
                );
            failures += piece_failures;
            lines.extend(piece_lines);
        }
        Ok((lines, OffsetFailures(failures)))
    }

    fn emit_level(
        &self,
        geometry: &Self::Geometry,
        level: LevelInfo,
        _rest: &LevelRest<'_>,
        _cancel: &dyn CancelCheck,
    ) -> Result<Toolpath, OperationError> {
        let feeds = &self.feeds;
        Ok(crate::ops::zigzag::lines_to_toolpath(
            geometry,
            &crate::ops::zigzag::ZigzagParams {
                tool_radius: feeds.tool_radius,
                stepover: self.stepover,
                cut_depth: level.z,
                feed_rate: feeds.feed_rate,
                plunge_rate: feeds.plunge_rate,
                safe_z: feeds.safe_z,
                angle: self.angle,
            },
        ))
    }
}

/// The Trace op: the compensated rings of each region, traced at each
/// level.
struct TracePass<'a> {
    cfg: &'a TraceConfig,
    rest_use: RestUse,
    params: crate::ops::trace_path::TraceParams,
}

impl LayeredOp for TracePass<'_> {
    type Input = Polygon2;
    type Geometry = Vec<Polygon2>;
    type Report = OffsetFailures;

    fn depth_per_pass(&self) -> f64 {
        self.cfg.depth_per_pass
    }

    fn region_ordering(&self) -> RegionOrdering {
        self.cfg.region_ordering
    }

    fn rest_use(&self) -> RestUse {
        self.rest_use
    }

    /// The lines of the selection (P2.3a): closed loops and open paths on
    /// the line (`ItemUse::Curves`), or the closed areas as lines with a
    /// compensation (`ItemUse::Regions`, until the open offset of P6).
    fn outlines<'p>(
        &'p self,
        prepared: &'p Prepared<'p>,
    ) -> Result<std::borrow::Cow<'p, [Polygon2]>, OperationError>
    where
        Self::Input: 'p,
    {
        prepared
            .curves()
            .map(|curves| std::borrow::Cow::Borrowed(curves.as_slice()))
    }

    /// The compensated rings are the tool centre, so the tool reaches one
    /// radius from them.
    // P1: an open path reads as closed here; the chord only widens the band (safe). P6: exact.
    fn reach_band(&self, geometry: &Self::Geometry, tool_radius: f64) -> ReachBand {
        ReachBand {
            rings: geometry
                .iter()
                .flat_map(|poly| std::iter::once(&poly.exterior).chain(&poly.holes))
                .cloned()
                .collect(),
            half_width: tool_radius,
        }
    }

    /// The cutter compensation: one offset per piece, never one per level.
    fn prepare(
        &self,
        region: &[Polygon2],
        _layer: &LayerCtx<'_>,
    ) -> Result<(Self::Geometry, Self::Report), OperationError> {
        let mut rings = Vec::new();
        let mut failures = 0;
        for piece in region {
            let (piece_rings, piece_failures) =
                crate::ops::trace_path::trace_compensated_polygons_reported(piece, &self.params);
            failures += piece_failures;
            rings.extend(piece_rings);
        }
        Ok((rings, OffsetFailures(failures)))
    }

    fn emit_level(
        &self,
        geometry: &Self::Geometry,
        level: LevelInfo,
        _rest: &LevelRest<'_>,
        _cancel: &dyn CancelCheck,
    ) -> Result<Toolpath, OperationError> {
        Ok(crate::ops::trace_path::trace_polygons_at_z(
            geometry,
            level.z,
            &self.params,
        ))
    }

    /// One region, and the depth runs inside it as chains (C8).
    fn annotate(
        &self,
        generated: GeneratedToolpath,
        semantic_ctx: Option<&ToolpathSemanticContext>,
    ) -> GeneratedToolpath {
        if let Some(sem) = semantic_ctx {
            crate::compute::annotate::annotate_trace_spans(
                &generated.spans,
                &generated.toolpath,
                sem,
            );
        }
        generated
    }
}

/// The Profile op: the tool-centre contour of each region, cut at each
/// level, with tabs at the final Z.
struct ProfilePass<'a> {
    cfg: &'a ProfileConfig,
    rest_use: RestUse,
    /// The params of every level; `emit_level` sets `cut_depth`.
    base: crate::ops::profile::ProfileParams,
}

impl LayeredOp for ProfilePass<'_> {
    type Input = Polygon2;
    type Geometry = Vec<Vec<P2>>;
    type Report = OffsetFailures;

    fn depth_per_pass(&self) -> f64 {
        self.cfg.depth_per_pass
    }

    fn region_ordering(&self) -> RegionOrdering {
        self.cfg.region_ordering
    }

    fn rest_use(&self) -> RestUse {
        self.rest_use
    }

    /// A computed contour is the tool centre: the tool reaches one radius
    /// from it. With the controller compensation the contour is the
    /// outline itself, and the controller puts the tool centre one radius
    /// to one side, so the band is two radii wide on each side.
    fn reach_band(&self, geometry: &Self::Geometry, tool_radius: f64) -> ReachBand {
        ReachBand {
            rings: geometry.clone(),
            half_width: if self.base.compensate_in_controller {
                2.0 * tool_radius
            } else {
                tool_radius
            },
        }
    }

    /// The side offset (or the outline itself when the controller
    /// compensates): one contour per piece. A piece whose offset collapses
    /// gives no contour.
    fn prepare(
        &self,
        region: &[Polygon2],
        _layer: &LayerCtx<'_>,
    ) -> Result<(Self::Geometry, Self::Report), OperationError> {
        let mut contours = Vec::new();
        let mut failures = 0;
        for piece in region {
            let (contour, piece_failures) =
                crate::ops::profile::profile_path_reported(piece, &self.base);
            failures += piece_failures;
            contours.extend(contour);
        }
        Ok((contours, OffsetFailures(failures)))
    }

    /// Each contour at `level.z`, climb or conventional. On a level at the
    /// final Z each contour gets its own evenly spaced tabs.
    fn emit_level(
        &self,
        geometry: &Self::Geometry,
        level: LevelInfo,
        _rest: &LevelRest<'_>,
        _cancel: &dyn CancelCheck,
    ) -> Result<Toolpath, OperationError> {
        let params = crate::ops::profile::ProfileParams {
            cut_depth: level.z,
            ..self.base
        };
        let tabs = (self.cfg.tab_count > 0 && level.is_final).then(|| {
            crate::dressup::even_tabs(self.cfg.tab_count, self.cfg.tab_width, self.cfg.tab_height)
        });
        let mut tp = Toolpath::new();
        for contour in geometry {
            let pass = crate::ops::profile::profile_path_to_toolpath(contour, &params);
            let pass = match &tabs {
                Some(tabs) => crate::dressup::apply_tabs(pass, tabs, level.z),
                None => pass,
            };
            tp.moves.extend(pass.moves);
        }
        Ok(tp)
    }
}

/// Face family adapter (W3.3): [`FacePass`] on the Layered runner. The one
/// outline is the facing rectangle of the stock, and the ladder is the one
/// Face ladder (`ops::face::face_levels`), so the depth-run spans now snap
/// to real levels (D7). The runner polls the cancel per level (pinned by
/// `every_generator_polls_a_preset_cancel_flag_past_the_prologue`).
///
/// F-028: the ladder anchors at `heights.top_z`, which under Auto follows
/// `ctx.stock_top_z`. An identity setup lands at the world stock top and a
/// non-identity setup at the local stock top, both in the frame the
/// toolpath is stamped into.
pub(crate) fn generate_face(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Face, "generate_face");
    let rect = crate::ops::face::face_rectangle(ctx.stock_bbox, cfg.stock_offset);
    let lattice =
        crate::ops::face::FaceLattice::of_rect(&rect, ctx.tool_def.radius(), cfg.stepover);
    let pass = FacePass {
        cfg,
        rest_use: op.capabilities().rest_use,
        rect,
        lattice,
        feeds: LayerFeeds::new(ctx, op),
    };
    run_layered(&pass, ctx, prepared)
}

/// The Face op: the scan rows of the facing rectangle (or of its rest
/// pieces), cut at each level of the Face ladder.
struct FacePass<'a> {
    cfg: &'a crate::compute::operation_configs::FaceConfig,
    rest_use: RestUse,
    /// The facing rectangle: the stock XY bounds grown by `stock_offset`.
    rect: Polygon2,
    /// The rows of the whole rectangle. A rest piece cuts only on them
    /// (B8), so a rest row never reaches past a fresh row.
    lattice: Option<crate::ops::face::FaceLattice>,
    feeds: LayerFeeds,
}

impl LayeredOp for FacePass<'_> {
    type Input = Polygon2;
    type Geometry = Vec<[P2; 2]>;
    type Report = OffsetFailures;

    fn depth_per_pass(&self) -> f64 {
        self.cfg.depth_per_pass
    }

    /// One outline, so the order changes nothing.
    fn region_ordering(&self) -> RegionOrdering {
        RegionOrdering::ByArea
    }

    fn rest_use(&self) -> RestUse {
        self.rest_use
    }

    /// The facing rectangle grows the stock box by `stock_offset`; the
    /// grid covers the stock box, so a cell off it is air.
    fn off_grid(&self) -> OffGrid {
        OffGrid::Air
    }

    fn outlines<'p>(
        &'p self,
        _prepared: &'p Prepared<'p>,
    ) -> Result<std::borrow::Cow<'p, [Polygon2]>, OperationError>
    where
        Polygon2: 'p,
    {
        Ok(std::borrow::Cow::Borrowed(std::slice::from_ref(&self.rect)))
    }

    /// The Face ladder of the config, from `heights.top_z`. Face does not
    /// read `cutting_levels`: its ladder was always its own depth dial
    /// from the stock top.
    fn ladder(&self, ctx: &ExecutionContext<'_>) -> Vec<f64> {
        crate::ops::face::face_levels(ctx.heights.top_z, self.cfg.depth, self.cfg.depth_per_pass)
    }

    fn prepare(
        &self,
        region: &[Polygon2],
        layer: &LayerCtx<'_>,
    ) -> Result<(Self::Geometry, Self::Report), OperationError> {
        let (lines, failures) = crate::ops::face::face_scan_lines_reported(
            region,
            layer.tool_radius,
            self.cfg.stepover,
            self.cfg.direction,
            self.lattice,
        );
        Ok((lines, OffsetFailures(failures)))
    }

    fn emit_level(
        &self,
        geometry: &Self::Geometry,
        level: LevelInfo,
        _rest: &LevelRest<'_>,
        _cancel: &dyn CancelCheck,
    ) -> Result<Toolpath, OperationError> {
        let feeds = &self.feeds;
        Ok(crate::ops::zigzag::lines_to_toolpath(
            geometry,
            &crate::ops::zigzag::ZigzagParams {
                tool_radius: feeds.tool_radius,
                stepover: self.cfg.stepover,
                cut_depth: level.z,
                feed_rate: feeds.feed_rate,
                plunge_rate: feeds.plunge_rate,
                safe_z: feeds.safe_z,
                angle: crate::ops::face::FACE_ANGLE,
            },
        ))
    }
}

/// Adaptive (2D) family adapter. Cancellable; per-level annotation
/// move_index offsetting reproduced verbatim (annotate_adaptive2d
/// consumes the combined-toolpath indices).
pub(crate) fn generate_adaptive(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Adaptive, "generate_adaptive");
    let levels = effective_levels(ctx.cutting_levels, ctx.heights, cfg.depth_per_pass);
    // K3: an area op plans around the obstacles (the territory rule).
    let polys = prepared.territory().area_outlines(
        std::borrow::Cow::Borrowed(prepared.regions()?.as_slice()),
        levels.iter().copied().fold(f64::INFINITY, f64::min),
    );
    let cancel_fn = ctx.cancel_fn();
    let safe_z = ctx.heights.retract_z;
    let mut combined = Toolpath::new();
    let mut all_annotations = Vec::new();
    // G-ADAPTPIECES (D2): the planner plans one piece of the machinable
    // inset, so an outline whose inset splits is planned piece by piece.
    // Each piece goes to full depth before the next, as each outline does.
    let piece_outlines: Vec<Vec<Polygon2>> = polys
        .iter()
        .map(|poly| crate::adaptive::inset_piece_outlines(poly, ctx.tool_def.radius()))
        .collect();
    let outlines = polys
        .iter()
        .zip(&piece_outlines)
        .flat_map(|(poly, pieces)| {
            if pieces.is_empty() {
                std::slice::from_ref(poly)
            } else {
                pieces.as_slice()
            }
        });
    let rest_use = op.capabilities().rest_use;
    for poly in outlines {
        // G-RESTLEVELS (`RestUse::LevelMask`): one stock read per level of
        // this piece. It skips a level with no material in reach and seeds
        // each other level's planner grid.
        let seeds = if rest_use == RestUse::LevelMask {
            let plan = level_masks(
                prepared.territory().unguarded_stock(),
                poly,
                &levels,
                ctx.tool_def.radius(),
                &cancel_fn,
            )?;
            ctx.record(&plan.finding);
            plan.levels
        } else {
            vec![LevelSeed::Fresh; levels.len()]
        };
        for ((level_i, &z), seed) in levels.iter().enumerate().zip(seeds) {
            let standing_material = match seed {
                LevelSeed::Empty => continue,
                LevelSeed::Fresh => None,
                LevelSeed::Material(mask) => Some(mask),
            };
            let params = crate::adaptive::AdaptiveParams {
                tool_radius: ctx.tool_def.radius(),
                stepover: cfg.stepover,
                cut_depth: z,
                feed_rate: op.feed_rate(),
                plunge_rate: op.plunge_rate(),
                safe_z,
                tolerance: cfg.tolerance,
                slot_clearing: cfg.slot_clearing,
                min_cutting_radius: cfg.min_cutting_radius,
                standing_material,
                cleanup_strategy: cfg.cleanup_strategy,
                engagement_measure: cfg.engagement_measure,
                path_strategy: cfg.path_strategy,
                trochoid_cap_mult: 1.2,
                keep_down_links: crate::adaptive::KeepDownLinks::WithinPassLoad,
                // G-ADAPTPASSLOAD: the hole the entry dressup's helix cuts.
                entry_helix_radius: ctx.entry_helix_radius.unwrap_or(0.0),
            };
            let (level_tp, mut annotations) =
                crate::adaptive::adaptive_toolpath_structured_annotated_traced_with_cancel(
                    poly,
                    &params,
                    &cancel_fn,
                    ctx.debug_ctx,
                )?;
            if level_tp.moves.is_empty() {
                continue;
            }
            // Inter-level retract: before each non-empty level but the first
            if level_i > 0 {
                let offset_before = combined.moves.len();
                combined.final_retract(safe_z);
                // Account for any retract moves added
                let retract_added = combined.moves.len() - offset_before;
                // Offset annotation move_index values
                let offset = offset_before + retract_added;
                for ann in &mut annotations {
                    ann.move_index += offset;
                }
            } else {
                let offset = combined.moves.len();
                for ann in &mut annotations {
                    ann.move_index += offset;
                }
            }
            all_annotations.extend(annotations);
            combined.moves.extend(level_tp.moves);
        }
    }
    if let Some(sem) = ctx.semantic_ctx {
        crate::compute::annotate::annotate_adaptive2d(&all_annotations, &combined, sem);
    }
    Ok(with_depth_run_annotation(
        generated_with_depth_run_spans(combined, &levels),
        ctx.semantic_ctx,
    ))
}
