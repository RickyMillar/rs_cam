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
use crate::toolpath::{SidedToolpath, Toolpath};
use crate::trace::semantic_trace::ToolpathSemanticContext;

use super::OffsetFailures;
use super::layered::{
    Ladders, LayerCtx, LayeredOp, LevelInfo, LevelRest, ReachBand, UnionLadder,
    area_pieces_less_obstacles, effective_levels, run_layered,
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
            // P6.2: a Bottom setup mirrors the drawing; the side of an
            // open path holds in the drawing.
            drawing_mirrored: ctx.z_flipped(),
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
///
/// P5: the hand tabs (`tab_points`) resolve here, once, against every
/// region of the op, because the nearest loop is a question over all of
/// them. A point farther than [`hand_tab_reach`] from every loop refuses
/// the op: a dropped hand tab is a part with less hold than the user set.
///
/// [`hand_tab_reach`]: crate::dressup::tabs::hand_tab_reach
///
/// P6.3: side `On` reads its items as lines (`ItemUse::Curves`): closed
/// rings and open paths, cut on the line. Outside and Inside read the
/// closed areas (`ItemUse::Regions`). The capability decides, not the side.
pub(crate) fn generate_profile(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Profile, "generate_profile");
    let feeds = LayerFeeds::new(ctx, op);
    let capabilities = op.capabilities();
    let regions = profile_lines(prepared, capabilities.item_use)?;
    let hand_tabs = crate::dressup::tabs::resolve_hand_tabs(
        &cfg.tab_points,
        ctx.setup_transform,
        ctx.tool_def.radius(),
        regions,
    )
    .map_err(OperationError::InvalidParam)?;
    let pass = ProfilePass {
        cfg,
        item_use: capabilities.item_use,
        regions,
        hand_tabs,
        rest_use: capabilities.rest_use,
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

/// The lines a Profile cuts (P6.3): the closed areas of a `Regions` op
/// (Outside, Inside), or the closed rings and open paths of a `Curves` op
/// (side `On`).
///
/// # Errors
///
/// The prologue's geometry mismatch, or a use that a Profile never states.
fn profile_lines<'p>(
    prepared: &'p Prepared<'p>,
    item_use: crate::compute::catalog::ItemUse,
) -> Result<&'p [Polygon2], OperationError> {
    use crate::compute::catalog::ItemUse;
    match item_use {
        ItemUse::Regions => Ok(prepared.regions()?.as_slice()),
        ItemUse::Curves => Ok(prepared.curves()?.as_slice()),
        ItemUse::None | ItemUse::RegionsExtendOpen { .. } | ItemUse::Holes { .. } => {
            Err(OperationError::Internal(format!(
                "Profile states item use {item_use:?}; it reads Regions or Curves"
            )))
        }
    }
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
    type Geometry = Vec<crate::toolpath::MaterialRing>;
    /// One report per piece. Each cascade adds its offset failures and,
    /// when a bound stopped it (Checkpoint C, Q3, F-10), the area it
    /// leaves standing. The runner records it once per region, never once
    /// per level.
    type Report = Vec<crate::ops::pocket::PocketCascadeReport>;

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
    ) -> Result<SidedToolpath, OperationError> {
        let feeds = &self.feeds;
        Ok(crate::ops::pocket::pocket_contours_to_sided_toolpath(
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
    region_ordering: RegionOrdering,
    rest_use: RestUse,
    feeds: LayerFeeds,
}

impl LayeredOp for ZigzagRaster {
    type Input = Polygon2;
    type Geometry = Vec<[P2; 2]>;
    type Report = OffsetFailures;

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
    ) -> Result<SidedToolpath, OperationError> {
        let feeds = &self.feeds;
        // P6.0c: rows are open passes; no closed pass states a side.
        Ok(SidedToolpath::without_closed_passes(
            crate::ops::zigzag::lines_to_toolpath(
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
            ),
        ))
    }
}

/// What one Trace `prepare` reports (P6.2): the offset failures, and each
/// open path whose offset to one side is not one piece.
struct TraceReport {
    offsets: OffsetFailures,
    splits: Vec<crate::compute::toolpath_stats::OpenOffsetSplit>,
}

impl super::findings::RecordFindings for TraceReport {
    fn record_into(&self, findings: &mut super::GenerationFindings) {
        super::findings::RecordFindings::record_into(&self.offsets, findings);
        // P6.2: `GenerationFindings` has no field for the split yet. The
        // field, the stats copy and the diagnostic adapter are outside
        // this item (`compute/execute.rs`, `compute/stats.rs`,
        // `diagnostics/adapters/from_generation.rs`). Until they land, the
        // split goes to the log.
        for split in &self.splits {
            tracing::warn!(pieces = split.pieces, "Trace: {}", split.message());
        }
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
    type Report = TraceReport;

    fn region_ordering(&self) -> RegionOrdering {
        self.cfg.region_ordering
    }

    fn rest_use(&self) -> RestUse {
        self.rest_use
    }

    /// The lines of the selection (P2.3a): closed loops and open paths
    /// (`ItemUse::Curves`), for every compensation (P6.2).
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
    /// radius from them. P6.3: an open path (or an open offset piece,
    /// P6.2) goes into `open`, so its band has no closing chord.
    fn reach_band(&self, geometry: &Self::Geometry, tool_radius: f64) -> ReachBand {
        let (closed, open): (Vec<&Polygon2>, Vec<&Polygon2>) =
            geometry.iter().partition(|poly| poly.closed);
        ReachBand {
            rings: closed
                .iter()
                .flat_map(|poly| std::iter::once(&poly.exterior).chain(&poly.holes))
                .cloned()
                .collect(),
            open: open.iter().map(|poly| poly.exterior.clone()).collect(),
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
        let mut splits = Vec::new();
        for piece in region {
            let compensated =
                crate::ops::trace_path::trace_compensated_polygons_reported(piece, &self.params);
            failures += compensated.failures;
            splits.extend(compensated.split);
            rings.extend(compensated.paths);
        }
        Ok((
            rings,
            TraceReport {
                offsets: OffsetFailures(failures),
                splits,
            },
        ))
    }

    fn emit_level(
        &self,
        geometry: &Self::Geometry,
        level: LevelInfo,
        _rest: &LevelRest<'_>,
        _cancel: &dyn CancelCheck,
    ) -> Result<SidedToolpath, OperationError> {
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
/// level, with tabs on each level below the tab top (P5b).
struct ProfilePass<'a> {
    cfg: &'a ProfileConfig,
    /// How the op reads its items (P6.3): `Regions` (Outside, Inside) or
    /// `Curves` (side `On`).
    item_use: crate::compute::catalog::ItemUse,
    /// The lines of the op ([`profile_lines`]), to find the region of an
    /// outline in `prepare` and its hand tabs.
    regions: &'a [Polygon2],
    hand_tabs: Vec<crate::dressup::tabs::HandTab>,
    rest_use: RestUse,
    /// The params of every level; `emit_level` sets `cut_depth`.
    base: crate::ops::profile::ProfileParams,
}

/// One contour of a Profile region: the ring the pass cuts (in cut order,
/// its start in a gap between tabs) and its tabs. The tabs do not depend
/// on Z: every level reuses them. P6.3: `closed == false` is an open path
/// of side `On`: the pass cuts it from its first point to its last point,
/// with no tab.
struct ProfileContour {
    ring: Vec<P2>,
    closed: bool,
    tabs: Vec<crate::dressup::Tab>,
}

/// What one Profile `prepare` reports: its offset failures and its tab
/// notes.
struct ProfileReport {
    offsets: OffsetFailures,
    tabs: Option<crate::compute::toolpath_stats::TabPlacementFinding>,
}

impl super::findings::RecordFindings for ProfileReport {
    fn record_into(&self, findings: &mut super::GenerationFindings) {
        findings.record(&self.offsets);
        findings.record(&self.tabs);
    }
}

impl LayeredOp for ProfilePass<'_> {
    type Input = Polygon2;
    type Geometry = Vec<ProfileContour>;
    type Report = ProfileReport;

    fn region_ordering(&self) -> RegionOrdering {
        self.cfg.region_ordering
    }

    fn rest_use(&self) -> RestUse {
        self.rest_use
    }

    /// The lines of the op ([`profile_lines`]): the regions for Outside
    /// and Inside, the closed rings and open paths for side `On` (P6.3).
    fn outlines<'p>(
        &'p self,
        prepared: &'p Prepared<'p>,
    ) -> Result<std::borrow::Cow<'p, [Polygon2]>, OperationError>
    where
        Self::Input: 'p,
    {
        profile_lines(prepared, self.item_use).map(std::borrow::Cow::Borrowed)
    }

    /// A computed contour is the tool centre: the tool reaches one radius
    /// from it. With the controller compensation the contour is the
    /// outline itself, and the controller puts the tool centre one radius
    /// to one side, so the band is two radii wide on each side. P6.3: an
    /// open contour goes into `open`, so its band has no closing chord.
    fn reach_band(&self, geometry: &Self::Geometry, tool_radius: f64) -> ReachBand {
        let ring_of = |closed: bool| {
            geometry
                .iter()
                .filter(move |c| c.closed == closed)
                .map(|c| c.ring.clone())
                .collect()
        };
        ReachBand {
            rings: ring_of(true),
            open: ring_of(false),
            half_width: if self.base.compensate_in_controller {
                2.0 * tool_radius
            } else {
                tool_radius
            },
        }
    }

    /// The side offset (or the outline itself when the controller
    /// compensates): one contour per piece. A piece whose offset collapses
    /// gives no contour, so it cuts nothing and needs no tab.
    ///
    /// P5: the tabs are Z-free, so they are placed here, once per region.
    /// P4.6: the placement is THE one of `dressup::tabs::plan_profile_tabs`,
    /// which the `tab_layout` query reads too, so the editor marks are the
    /// tabs this pass cuts.
    fn prepare(
        &self,
        region: &[Polygon2],
        _layer: &LayerCtx<'_>,
    ) -> Result<(Self::Geometry, Self::Report), OperationError> {
        let plan = crate::dressup::tabs::plan_profile_tabs(
            region,
            &crate::dressup::tabs::ProfileTabRule {
                regions: self.regions,
                hand: &self.hand_tabs,
                count: self.cfg.tab_count,
                width: self.cfg.tab_width,
                profile: &self.base,
            },
        );
        let contours = plan
            .contours
            .into_iter()
            .map(|c| ProfileContour {
                tabs: c.tabbed.tabs(self.cfg.tab_height),
                ring: c.tabbed.ring,
                closed: c.closed,
            })
            .collect();
        let tabs =
            (!plan.notes.is_empty()).then(|| crate::compute::toolpath_stats::TabPlacementFinding {
                op: crate::compute::catalog::OperationType::Profile.label(),
                notes: plan.notes,
            });
        Ok((
            contours,
            ProfileReport {
                offsets: OffsetFailures(plan.offset_failures),
                tabs,
            },
        ))
    }

    /// Each contour at `level.z`, climb or conventional, with its own tabs
    /// (placed in `prepare`), cut by the one lift emitter,
    /// [`crate::dressup::apply_tabs`]. The tab top is `level.final_z +
    /// tab_height`; a level below it lifts over each tab to the tab top, a
    /// level at or above it cuts through (P5b). So a tab keeps its full
    /// height when `tab_height` is more than `depth_per_pass`.
    fn emit_level(
        &self,
        geometry: &Self::Geometry,
        level: LevelInfo,
        _rest: &LevelRest<'_>,
        _cancel: &dyn CancelCheck,
    ) -> Result<SidedToolpath, OperationError> {
        let params = crate::ops::profile::ProfileParams {
            cut_depth: level.z,
            ..self.base
        };
        let mut tp = SidedToolpath::new();
        for contour in geometry {
            if !contour.closed {
                // P6.3: an open path of side `On`, in its drawn direction,
                // with no chord, no tab and no closed side (no lead).
                tp.append(crate::ops::profile::profile_path_open_to_toolpath(
                    &contour.ring,
                    &params,
                ));
                continue;
            }
            let pass = crate::ops::profile::profile_ring_to_toolpath(
                &contour.ring,
                &params,
                params.side.ring_material(),
            );
            let tabbed =
                crate::dressup::apply_tabs(pass.toolpath, &contour.tabs, level.z, level.final_z);
            tp.append(SidedToolpath {
                toolpath: tabbed,
                closed_passes: pass.closed_passes,
            });
        }
        Ok(tp)
    }
}

/// Face family adapter (W3.3): [`FacePass`] on the Layered runner. The
/// outlines are the regions of the op selection (geometry selection P4.2),
/// and the ladder is the one Face ladder (`ops::face::face_levels`), so the
/// depth-run spans snap to real levels (D7). The runner polls the cancel
/// per level (pinned by
/// `every_generator_polls_a_preset_cancel_flag_past_the_prologue`).
///
/// The default selection is the stock outline (decision D5). The stock
/// region becomes the facing rectangle: the stock XY bounds grown by
/// `stock_offset`, as before P4.2. Any other region is faced inside its
/// outline: the tool stays inside it, as in a pocket (ruling O-3).
///
/// F-028: the ladder anchors at `heights.top_z`, which under Auto follows
/// `ctx.stock_top_z`. An identity setup lands at the world stock top and a
/// non-identity setup at the local stock top, both in the frame the
/// toolpath is stamped into. A picked face moves an Auto top to the face
/// (the 2.5D rule of the session).
pub(crate) fn generate_face(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Face, "generate_face");
    let regions = prepared.regions()?;
    let tool_radius = ctx.tool_def.radius();
    let outlines: Vec<Polygon2> = regions
        .as_slice()
        .iter()
        .enumerate()
        .map(|(index, region)| {
            if regions.is_stock(index) {
                // The stock region is the facing rectangle, as before
                // P4.2. It ignores the holes of the region (drawing rings
                // inside the stock): Face levels the whole stock top.
                crate::ops::face::face_rectangle(ctx.stock_bbox, cfg.stock_offset)
            } else {
                region.clone()
            }
        })
        .collect();
    let lattices = outlines
        .iter()
        .map(|outline| crate::ops::face::FaceLattice::of_rect(outline, tool_radius, cfg.stepover))
        .collect();
    let pass = FacePass {
        cfg,
        rest_use: op.capabilities().rest_use,
        outlines,
        lattices,
        feeds: LayerFeeds::new(ctx, op),
    };
    run_layered(&pass, ctx, prepared)
}

/// The Face op: the scan rows of each facing outline (or of its rest
/// pieces), cut at each level of the Face ladder.
struct FacePass<'a> {
    cfg: &'a crate::compute::operation_configs::FaceConfig,
    rest_use: RestUse,
    /// The facing outlines: the stock rectangle grown by `stock_offset`,
    /// and each other region of the selection as it is.
    outlines: Vec<Polygon2>,
    /// The rows of each whole outline, by index. A rest piece or an
    /// obstacle piece cuts only on the rows of its outline (B8), so a rest
    /// row never reaches past a fresh row.
    lattices: Vec<Option<crate::ops::face::FaceLattice>>,
    feeds: LayerFeeds,
}

impl FacePass<'_> {
    /// The lattice of the whole outline that `outline` comes from. The
    /// runner can give a piece of an outline (the obstacle clip), so with
    /// more than one outline the piece takes the smallest outline whose
    /// box holds the box of the piece. With one outline, every piece takes
    /// its lattice (the facing rectangle before P4.2).
    fn lattice_for(
        &self,
        outline: &Polygon2,
        tool_radius: f64,
    ) -> Option<crate::ops::face::FaceLattice> {
        if let [only] = self.lattices.as_slice() {
            return *only;
        }
        const BOX_EPS_MM: f64 = 1e-9;
        let [x0, y0, x1, y1] = outline.bbox();
        self.outlines
            .iter()
            .zip(&self.lattices)
            .filter(|(whole, _)| {
                let [wx0, wy0, wx1, wy1] = whole.bbox();
                wx0 <= x0 + BOX_EPS_MM
                    && wy0 <= y0 + BOX_EPS_MM
                    && wx1 >= x1 - BOX_EPS_MM
                    && wy1 >= y1 - BOX_EPS_MM
            })
            .min_by(|(a, _), (b, _)| a.area().total_cmp(&b.area()))
            .map_or_else(
                || crate::ops::face::FaceLattice::of_rect(outline, tool_radius, self.cfg.stepover),
                |(_, lattice)| *lattice,
            )
    }
}

impl LayeredOp for FacePass<'_> {
    type Input = Polygon2;
    type Geometry = Vec<[P2; 2]>;
    type Report = OffsetFailures;

    /// Region-first: each outline goes to full depth before the next. With
    /// the stock default there is one outline, so the order changes
    /// nothing.
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
        Ok(std::borrow::Cow::Borrowed(self.outlines.as_slice()))
    }

    /// The Face ladder of the config, from `heights.top_z`. Face does not
    /// read `cutting_levels`: its ladder was always its own depth dial
    /// from the stock top.
    fn ladder(&self, ctx: &ExecutionContext<'_>) -> Result<Vec<f64>, OperationError> {
        Ok(crate::ops::face::face_levels(
            ctx.heights.top_z,
            self.cfg.depth,
            self.cfg.depth_per_pass,
        ))
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
            self.lattice_for(layer.outline, layer.tool_radius),
        );
        Ok((lines, OffsetFailures(failures)))
    }

    fn emit_level(
        &self,
        geometry: &Self::Geometry,
        level: LevelInfo,
        _rest: &LevelRest<'_>,
        _cancel: &dyn CancelCheck,
    ) -> Result<SidedToolpath, OperationError> {
        let feeds = &self.feeds;
        // P6.0c: rows are open passes; no closed pass states a side.
        Ok(SidedToolpath::without_closed_passes(
            crate::ops::zigzag::lines_to_toolpath(
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
            ),
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
    // Geometry selection P3.6 (ruling L-5): under the floor "each picked
    // face" each area steps its own ladder (`Prepared::region_ladders`), in
    // this loop, not in `run_layered`. One ladder (every other floor) is
    // the loop before P3.6, bit for bit.
    let shared = effective_levels(ctx.cutting_levels)?;
    let regions = prepared.regions()?;
    let ladders = Ladders::new(prepared, shared, regions.as_slice().len())?;
    // K3: an area op plans around the obstacles (the territory rule), each
    // run of areas at the lowest level of its own ladder (P3.5).
    let (polys, ladder_of) = area_pieces_less_obstacles(
        prepared,
        std::borrow::Cow::Borrowed(regions.as_slice()),
        &ladders,
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
    // P3.6: each piece keeps the ladder of its area. The areas keep the
    // selection order, and each piece goes to its own floor before the
    // next (ruling L-7).
    let outlines =
        polys
            .iter()
            .zip(&piece_outlines)
            .zip(&ladder_of)
            .flat_map(|((poly, pieces), &ladder)| {
                let pieces = if pieces.is_empty() {
                    std::slice::from_ref(poly)
                } else {
                    pieces.as_slice()
                };
                pieces.iter().map(move |piece| (piece, ladder))
            });
    let rest_use = op.capabilities().rest_use;
    for (poly, ladder) in outlines {
        let levels = ladders.ladder(ladder);
        // G-RESTLEVELS (`RestUse::LevelMask`): one stock read per level of
        // this piece. It skips a level with no material in reach and seeds
        // each other level's planner grid.
        let seeds = if rest_use == RestUse::LevelMask {
            let plan = level_masks(
                prepared.territory().unguarded_stock(),
                poly,
                levels,
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
    // P3.6: the spans snap to the union of the ladders (one ladder: the
    // ladder itself).
    Ok(with_depth_run_annotation(
        generated_with_depth_run_spans(combined, &UnionLadder::of(&ladders.ladders).z),
        ctx.semantic_ctx,
    ))
}
