//! W3.0: the Layered shape. One runner steps an op down its Z ladder.
//!
//! A Layered op cuts its input at a ladder of Z levels. The input is a 2D
//! outline (the 2.5D ops and Chamfer) or a surface (Waterline, W6.1); the
//! op names it as [`LayeredOp::Input`]. The op writes two halves:
//!
//! - [`LayeredOp::prepare`] makes the Z-independent geometry of one region;
//! - [`LayeredOp::emit_level`] cuts that geometry at one level. It can
//!   fail, it polls the cancel, and it reads the runner's rest rule at the
//!   level ([`LevelRest`]) when the op's rest rule cuts per level
//!   ([`RestUse::ContourFilter`]).
//!
//! [`run_layered`] owns everything else, for every Layered op:
//!
//! - the ladder ([`LayeredOp::ladder`]);
//! - the rest step per level, as the op's [`RestUse`] states it
//!   ([`RestUse::LevelClip`] reads `standing_material::regions_per_level`,
//!   [`RestUse::LevelSkip`] reads `standing_material::levels_in_reach`,
//!   [`RestUse::ContourFilter`] resolves
//!   `standing_material::ContourFilterRule` once and gives the op the rule
//!   at each level, then records its level tally);
//! - one `prepare` per distinct region (the whole outline and each clipped
//!   region) of an outline, never one per level. Region-first prepares one
//!   outline at a time and drops its geometry after its last level, so peak
//!   memory does not grow with the outline count; level-first prepares
//!   every outline first;
//! - the region order (ruling R4, [`RegionOrdering`]);
//! - a cancel poll at each level, and the skip of an empty level;
//! - the findings: the measured zero of the offset channel (for an op that
//!   offsets, [`LayeredOp::MEASURES_OFFSETS`]), the rest-level finding and
//!   each prepare report, through the W2.3 record door;
//! - the link stage. When the op declares one ([`LayeredOp::after_plan_link`],
//!   default: none), the runner runs the one after-plan link
//!   (`shared::link_after_plan`) on the whole toolpath after the last level
//!   and before the spans, and records its totals;
//! - the spans. The op's [`LayeredOp::spans`] hook builds them (the
//!   default: the depth runs, snapped to the ladder), and its
//!   [`LayeredOp::annotate`] hook reads them into the semantic trace (the
//!   default reads each run as a region).
//!
//! # The region order (R4)
//!
//! A region is one input outline. [`RegionOrdering::ByArea`] is
//! region-first: each outline goes to full depth before the next. It is
//! the default for a Layered op, and it is the order of every 2.5D op
//! before W3. [`RegionOrdering::Global`] is level-first: each level visits
//! every outline before the ladder descends. Both orders give the same
//! moves per (outline, level); only the sequence changes.
//!
//! The depth-run spans then follow the sequence. Region-first gives one
//! depth pass per (outline, level); level-first gives one per level, and
//! each holds the runs of every outline at that level. Each depth pass is
//! a rapid-order barrier, so the rapid-order stage reorders runs inside one
//! level and never moves a run across a level.
//!
//! # A ladder per area (geometry selection P3.5)
//!
//! Under the floor "each picked face" the prologue gives each closed area
//! its own ladder (`Prepared::region_ladders`): the catalogue ladder from
//! the one Top Z to the floor of that area. [`run_layered`] then steps each
//! outline down its own ladder. Region-first takes each outline to its own
//! floor. Level-first walks the union of the ladders, top first: each entry
//! is a pair (Z, repeat index), so a spring pass stays a distinct entry, and
//! an outline cuts at an entry only when its own ladder holds the same pair
//! (the same Z bits, never a float tolerance). [`LevelInfo`] gives the
//! position and the final Z in the outline's own ladder, so Profile stands
//! its tabs on the floor of its own area. With one ladder (every other
//! floor) the runner is the runner before P3.5, bit for bit.
//!
//! # The retract rule
//!
//! Region-first keeps the rule of the old hand-written ladder (D3 deleted
//! it): a retract to `safe_z`
//! comes before each non-empty level of an outline except its first
//! ladder level, and no retract comes between outlines. Level-first puts a
//! retract before each non-empty piece after the first. Each retract is a
//! no-op when the tool already stands at `safe_z`, which is where every
//! Layered emitter ends.

use std::borrow::Cow;
use std::cell::Cell;

use crate::compute::catalog::RestUse;
use crate::compute::operation_configs::RegionOrdering;
use crate::compute::toolpath_stats::RestLevelsFinding;
use crate::dexel_stock::TriDexelStock;
use crate::geo::P2;
use crate::interrupt::{CancelCheck, check_cancel};
use crate::machine::kinematics::LinkKinematics;
use crate::polygon::Polygon2;
use crate::tool::MillingCutter;
use crate::toolpath::Toolpath;
use crate::trace::semantic_trace::ToolpathSemanticContext;

use super::findings::RecordFindings;
use super::shared::{
    AfterPlanLink, generated_with_depth_run_spans, link_after_plan, with_depth_run_annotation,
};
use super::standing_material::{
    ContourFilterRule, LevelPlan, LevelRegion, OffGrid, StandingNear, levels_in_reach,
    regions_per_level,
};
use super::{
    ExecutionContext, GeneratedToolpath, MeshInput, OffsetFailures, OperationError, Prepared,
};

/// One level of the ladder, as [`LayeredOp::emit_level`] gets it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LevelInfo {
    /// The Z of the level.
    pub z: f64,
    /// The position of the level in the ladder, from 0 at the top.
    pub index: usize,
    /// The Z of the final level of the ladder: the deepest level, which
    /// each spring pass (`finishing_passes`) repeats. Profile stands its
    /// tabs on it: every level below the tab top lifts over a tab (P5b).
    pub final_z: f64,
}

/// The tool-centre rings of one prepared region and the half-width (mm)
/// of the band the tool reaches around them. [`RestUse::LevelSkip`] reads
/// it ([`LayeredOp::reach_band`]).
pub(crate) struct ReachBand {
    /// The closed tool-centre rings.
    pub rings: Vec<Vec<P2>>,
    /// The distance (mm) from a ring to the edge of the cut: the tool
    /// radius for a tool-centre ring, twice the radius when the controller
    /// offsets the ring.
    pub half_width: f64,
}

/// What [`LayeredOp::prepare`] gets besides the region.
pub(crate) struct LayerCtx<'c, I = Polygon2> {
    /// The tool radius (mm).
    pub tool_radius: f64,
    /// The cooperative cancel. A kernel that loops polls it.
    pub cancel: &'c dyn CancelCheck,
    /// The whole outline the region comes from. A rest clip region is a
    /// part of it. An op that puts its rows on a lattice reads the lattice
    /// from it, so a clipped region keeps the rows of the whole (B9).
    pub outline: &'c I,
}

/// The input a Layered op steps down its ladder ([`LayeredOp::Input`]).
///
/// A 2D op steps a [`Polygon2`]: the `Level*` rest rules read it, and a
/// [`RestUse::LevelClip`] clip gives pieces of it back to `prepare`. A
/// surface op (Waterline) steps an input with no outline: the runner then
/// cuts it whole at every level, and the op reads the runner's
/// `ContourFilter` rule per level ([`LevelRest`]).
pub(crate) trait LayerInput: Clone {
    /// The 2D outline the `Level*` rest rules read, or `None` for an input
    /// with no outline. The runner cuts an input with no outline whole at
    /// every level: it cuts more, never less.
    fn outline(&self) -> Option<&Polygon2>;

    /// The inputs of 2D polygons: the checked polygons of the op (the
    /// default [`LayeredOp::outlines`]) and the pieces of a rest clip. An
    /// input with no outline holds none, and an op on it states its own
    /// [`LayeredOp::outlines`].
    fn from_polygons<'p>(polygons: Cow<'p, [Polygon2]>) -> Cow<'p, [Self]>
    where
        Self: 'p;
}

impl LayerInput for Polygon2 {
    fn outline(&self) -> Option<&Polygon2> {
        Some(self)
    }

    fn from_polygons<'p>(polygons: Cow<'p, [Polygon2]>) -> Cow<'p, [Self]>
    where
        Self: 'p,
    {
        polygons
    }
}

/// A surface (Waterline, W6.1): the mesh and its index, the query handle
/// the push cutter reads at each level. It has no 2D outline, so the
/// runner cuts it whole at each level.
impl LayerInput for MeshInput<'_> {
    fn outline(&self) -> Option<&Polygon2> {
        None
    }

    fn from_polygons<'p>(_polygons: Cow<'p, [Polygon2]>) -> Cow<'p, [Self]>
    where
        Self: 'p,
    {
        Cow::Owned(Vec::new())
    }
}

/// The runner's rest rule, as [`LayeredOp::emit_level`] gets it at one
/// level ([`RestUse::ContourFilter`]).
pub(crate) struct LevelRest<'s> {
    /// The rule at the level's Z (`ContourFilterRule::at`). `None`: a
    /// fresh op, or a rest rule that is not `ContourFilter`. The op keeps
    /// every point then.
    pub standing: Option<StandingNear<'s>>,
    /// The time model of the rule (`ContourFilterRule::split_timing`).
    pub split_timing: Option<&'s LinkKinematics>,
    /// The runner's level tally (G-RESTLEVELS).
    tally: &'s Cell<Option<RestLevelsFinding>>,
}

impl LevelRest<'_> {
    /// Add this level to the runner's tally: `kept` when the rule kept a
    /// point inside the boundary, `dropped` when it dropped one (both after
    /// the time rule). Only the kernel knows them, so the op reports them
    /// here; the runner records the tally after the last level. No effect
    /// when [`Self::standing`] is `None`.
    pub(crate) fn tally(&self, kept: bool, dropped: bool) {
        if self.standing.is_none() {
            return;
        }
        let mut tally = self.tally.get().unwrap_or(RestLevelsFinding {
            planned_levels: 0,
            skipped_levels: 0,
            clipped_levels: 0,
            fallback: None,
        });
        tally.planned_levels += 1;
        match (kept, dropped) {
            (false, true) => tally.skipped_levels += 1,
            (true, true) => tally.clipped_levels += 1,
            (_, false) => {}
        }
        self.tally.set(Some(tally));
    }
}

/// An op in the Layered shape. [`run_layered`] runs it.
pub(crate) trait LayeredOp {
    /// What the op steps down its ladder: a 2D outline, or a surface.
    type Input: LayerInput;
    /// The Z-independent geometry of one region.
    type Geometry;
    /// What one `prepare` reports. The runner records it.
    type Report: RecordFindings;

    /// True when the op offsets its input through a `_reported` offset
    /// call. The runner then records the measured zero of the offset
    /// channel before the first prepare (Checkpoint C). An op that offsets
    /// nothing (Waterline) must not claim a zero.
    const MEASURES_OFFSETS: bool = true;

    /// The order in which the runner visits outlines and levels.
    fn region_ordering(&self) -> RegionOrdering;

    /// How the op uses the remaining stock. The adapter reads it from the
    /// op's capabilities (`OperationConfig::capabilities().rest_use`), so
    /// the GUI, MCP and the runner read one answer. The runner acts on
    /// `LevelClip`, `LevelSkip` and `ContourFilter` (it gives the op the
    /// rule at each level, [`LevelRest`]). It cuts every other kind whole
    /// at each level.
    fn rest_use(&self) -> RestUse;

    /// What a cell off the stock grid holds for [`RestUse::LevelClip`].
    /// The default counts it as material. Face overrides it: its outline
    /// grows the stock box on purpose.
    fn off_grid(&self) -> OffGrid {
        OffGrid::Material
    }

    /// The reach band of `geometry` for [`RestUse::LevelSkip`]. The default
    /// holds no ring, and the runner then cuts every level. A `LevelSkip`
    /// op overrides it.
    fn reach_band(&self, _geometry: &Self::Geometry, tool_radius: f64) -> ReachBand {
        ReachBand {
            rings: Vec::new(),
            half_width: tool_radius,
        }
    }

    /// The Z-independent geometry of `region`. `region` is the whole outline
    /// or the pieces of one clipped region; the op treats each piece alike.
    fn prepare(
        &self,
        region: &[Self::Input],
        layer: &LayerCtx<'_, Self::Input>,
    ) -> Result<(Self::Geometry, Self::Report), OperationError>;

    /// The cut of `geometry` at one level. An empty toolpath is a level
    /// with nothing to cut; the runner skips it. A 2.5D op reads neither
    /// `rest` nor `cancel`: the runner polls the cancel before each level.
    /// A kernel that runs long inside one level polls `cancel` itself.
    ///
    /// # Errors
    ///
    /// [`OperationError::Cancelled`] when `cancel` fires inside the level.
    fn emit_level(
        &self,
        geometry: &Self::Geometry,
        level: LevelInfo,
        rest: &LevelRest<'_>,
        cancel: &dyn CancelCheck,
    ) -> Result<Toolpath, OperationError>;

    /// The outlines the runner steps, one region each. The default is the
    /// closed areas of the op's selection (`Prepared::regions`, P2.3a).
    /// Face overrides it: its one outline is the facing rectangle of the
    /// stock (W3.3). Trace overrides it: it reads the lines
    /// (`Prepared::curves`). Waterline overrides it: its one input is the
    /// surface (W6.1).
    ///
    /// # Errors
    ///
    /// The prologue's geometry mismatch when the op has no regions.
    fn outlines<'p>(
        &'p self,
        prepared: &'p Prepared<'p>,
    ) -> Result<Cow<'p, [Self::Input]>, OperationError>
    where
        Self::Input: 'p,
    {
        prepared
            .regions()
            .map(|regions| Self::Input::from_polygons(Cow::Borrowed(regions.as_slice())))
    }

    /// The Z ladder, top first. The default is the catalogue's
    /// `cutting_levels`, which already hold the spring passes of an op that
    /// has them (Pocket, Profile).
    ///
    /// # Errors
    ///
    /// The default refuses an empty ladder ([`effective_levels`], L-16).
    fn ladder(&self, ctx: &ExecutionContext<'_>) -> Result<Vec<f64>, OperationError> {
        effective_levels(ctx.cutting_levels)
    }

    /// The spans of the finished toolpath. The default builds one depth
    /// run per (outline, level) and snaps it to the ladder `levels`.
    /// Chamfer overrides it: its one level holds one cut run per ring, and
    /// its spans keep the "Chamfer run" label they had before W3.5.
    fn spans(&self, toolpath: Toolpath, levels: &[f64]) -> GeneratedToolpath {
        generated_with_depth_run_spans(toolpath, levels)
    }

    /// The finishing link stage of the whole toolpath, when the op has
    /// one. The runner runs it (`shared::link_after_plan`) after the last
    /// level and before [`Self::spans`]. The default links nothing.
    /// Waterline declares one (G-LINKSTAGE, with the one link ceiling).
    fn after_plan_link(&self) -> Option<AfterPlanLink<'_>> {
        None
    }

    /// The semantic annotation of the finished toolpath. The default reads
    /// the depth-run spans as regions. Trace overrides it: its runs are
    /// chains inside one region.
    fn annotate(
        &self,
        generated: GeneratedToolpath,
        semantic_ctx: Option<&ToolpathSemanticContext>,
    ) -> GeneratedToolpath {
        with_depth_run_annotation(generated, semantic_ctx)
    }
}

/// The prepared geometry of one outline: its level plan and one geometry
/// per distinct region of that plan.
struct OutlineLayers<G> {
    plan: LevelPlan,
    /// The geometry of the whole outline. `None` when no level cuts it.
    whole: Option<G>,
    /// The geometry of `plan.regions[i]`.
    clipped: Vec<G>,
}

impl<G> OutlineLayers<G> {
    /// The geometry level `index` cuts, or `None` for an empty level.
    fn geometry_at(&self, index: usize) -> Option<&G> {
        match self.plan.levels.get(index) {
            None | Some(LevelRegion::Whole) => self.whole.as_ref(),
            Some(LevelRegion::Empty) => None,
            Some(LevelRegion::Clipped(i)) => self.clipped.get(*i).or(self.whole.as_ref()),
        }
    }
}

/// The ladders of the outlines of one run (geometry selection P3.5): one
/// shared ladder, or one ladder per closed area under the floor "each
/// picked face".
struct Ladders {
    /// Each distinct ladder, top first.
    ladders: Vec<Vec<f64>>,
    /// For each outline: the index of its ladder in `ladders`.
    of_outline: Vec<usize>,
}

impl Ladders {
    /// The ladder at `index`; empty for an index that names none.
    fn ladder(&self, index: usize) -> &[f64] {
        self.ladders.get(index).map_or(&[], Vec::as_slice)
    }

    /// The ladders of `outline_count` outlines: the per-area ladders of the
    /// prologue when it gives them, else `shared` for every outline. Each
    /// per-area ladder passes the L-16 guard ([`effective_levels`]).
    fn new(
        prepared: &Prepared<'_>,
        shared: Vec<f64>,
        outline_count: usize,
    ) -> Result<Self, OperationError> {
        let Some(per_area) = prepared.region_ladders() else {
            return Ok(Self {
                ladders: vec![shared],
                of_outline: vec![0; outline_count],
            });
        };
        if per_area.len() != outline_count {
            return Err(OperationError::Other(format!(
                "registry adapter mismatch: {} has {} area ladders for {outline_count}                  outlines; only an op that steps the closed areas of its selection takes                  the floor \"each picked face\"",
                prepared.op_type().name(),
                per_area.len()
            )));
        }
        Ok(Self {
            ladders: per_area
                .iter()
                .map(|ladder| effective_levels(ladder))
                .collect::<Result<_, _>>()?,
            of_outline: (0..outline_count).collect(),
        })
    }
}

/// The union of the ladders for level-first order (P3.5): its entries top
/// first, and where each ladder holds each entry.
struct UnionLadder {
    /// The Z of each entry, top first. A spring pass is its own entry.
    z: Vec<f64>,
    /// For each ladder: at each entry, the position of that entry in the
    /// ladder, or `None` when the ladder does not hold it.
    at: Vec<Vec<Option<usize>>>,
}

impl UnionLadder {
    /// The position of union entry `entry` in ladder `ladder`, or `None`
    /// when that ladder does not hold it.
    fn position(&self, ladder: usize, entry: usize) -> Option<usize> {
        self.at
            .get(ladder)
            .and_then(|at| at.get(entry))
            .copied()
            .flatten()
    }

    /// The union of `ladders`. One ladder is its own union, in its own
    /// order, so the runner is the runner before P3.5. Several ladders
    /// merge by (Z, repeat index): Z top first by `f64::total_cmp`, then the
    /// repeat index. Two entries are one entry only when the Z bits and the
    /// repeat index are equal: no float tolerance, so a spring pass never
    /// merges with its level and never doubles.
    fn of(ladders: &[Vec<f64>]) -> Self {
        if let [only] = ladders {
            return Self {
                z: only.clone(),
                at: vec![(0..only.len()).map(Some).collect()],
            };
        }
        let pairs_of = |ladder: &[f64]| -> Vec<(f64, usize)> {
            ladder
                .iter()
                .enumerate()
                .map(|(i, &z)| {
                    let repeat = ladder
                        .iter()
                        .take(i)
                        .filter(|earlier| earlier.total_cmp(&z).is_eq())
                        .count();
                    (z, repeat)
                })
                .collect()
        };
        let order = |a: &(f64, usize), b: &(f64, usize)| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1));
        let pairs: Vec<Vec<(f64, usize)>> = ladders.iter().map(|l| pairs_of(l)).collect();
        let mut entries: Vec<(f64, usize)> = pairs.iter().flatten().copied().collect();
        entries.sort_by(order);
        entries.dedup_by(|a, b| order(a, b).is_eq());
        let at = pairs
            .iter()
            .map(|ladder| {
                let mut at = vec![None; entries.len()];
                for (position, pair) in ladder.iter().enumerate() {
                    if let Ok(entry) = entries.binary_search_by(|e| order(e, pair))
                        && let Some(slot) = at.get_mut(entry)
                    {
                        *slot = Some(position);
                    }
                }
                at
            })
            .collect();
        Self {
            z: entries.into_iter().map(|(z, _)| z).collect(),
            at,
        }
    }
}

/// THE runner of a Layered op. See the module doc for what it owns.
pub(crate) fn run_layered<O: LayeredOp>(
    op: &O,
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
) -> Result<GeneratedToolpath, OperationError> {
    let inputs = op.outlines(prepared)?;
    let ladders = Ladders::new(prepared, op.ladder(ctx)?, inputs.len())?;
    let (outlines, ladder_of) = area_outlines_less_obstacles(op, prepared, inputs, &ladders);
    let union = UnionLadder::of(&ladders.ladders);
    let safe_z = ctx.heights.retract_z;
    let cancel_fn = ctx.cancel_fn();
    let tool_radius = ctx.tool_def.radius();
    // C3: the territory resolved both stocks. `ContourFilter` compares the
    // stock with the surface, so its rule reads the guarded one; `LevelClip`
    // and `LevelSkip` have no mesh to guard against.
    let territory = prepared.territory();
    let unguarded = territory.unguarded_stock();
    let contour = ContourFilterRule::resolve(
        op.rest_use(),
        territory,
        tool_radius,
        ctx.link_kinematics.as_ref(),
    );
    let tally = Cell::new(None);
    let rules = LevelRules {
        contour: contour.as_ref(),
        tally: &tally,
    };
    // Checkpoint C: an op that measures its offsets holds a measured zero
    // before the first prepare. Each prepare report adds its own count
    // (W2.3).
    if O::MEASURES_OFFSETS {
        ctx.record(&OffsetFailures(0));
    }

    let mut combined = Toolpath::new();
    match op.region_ordering() {
        // Region-first: one outline is prepared at a time, and its geometry
        // is dropped after its last level. Peak memory does not grow with
        // the outline count.
        RegionOrdering::ByArea => {
            for (outline, &l) in outlines.iter().zip(&ladder_of) {
                let levels = ladders.ladder(l);
                let layers =
                    prepare_outline(op, ctx, unguarded, tool_radius, &cancel_fn, outline, levels)?;
                let mut emitted = false;
                for i in 0..levels.len() {
                    check_cancel(&cancel_fn)?;
                    let Some(piece) = emit_at(op, &layers, levels, i, &rules, &cancel_fn)? else {
                        continue;
                    };
                    if i > 0 && emitted {
                        combined.final_retract(safe_z);
                    }
                    combined.moves.extend(piece.moves);
                    emitted = true;
                }
            }
        }
        // Level-first: each level visits every outline, so every outline
        // is prepared before the first level.
        RegionOrdering::Global => {
            let mut layers = Vec::with_capacity(outlines.len());
            for (outline, &l) in outlines.iter().zip(&ladder_of) {
                layers.push(prepare_outline(
                    op,
                    ctx,
                    unguarded,
                    tool_radius,
                    &cancel_fn,
                    outline,
                    ladders.ladder(l),
                )?);
            }
            for entry in 0..union.z.len() {
                for (outline_layers, &l) in layers.iter().zip(&ladder_of) {
                    // P3.5: the outline cuts at this entry only when its own
                    // ladder holds it, at its own position.
                    let Some(i) = union.position(l, entry) else {
                        continue;
                    };
                    check_cancel(&cancel_fn)?;
                    let levels = ladders.ladder(l);
                    let Some(piece) = emit_at(op, outline_layers, levels, i, &rules, &cancel_fn)?
                    else {
                        continue;
                    };
                    if !combined.moves.is_empty() {
                        combined.final_retract(safe_z);
                    }
                    combined.moves.extend(piece.moves);
                }
            }
        }
    }

    // G-RESTLEVELS: the level tally of the contour filter. `None` (no
    // record) when the rule did not run.
    ctx.record(&tally.get());
    let linked = match op.after_plan_link() {
        Some(link) => link_after_plan(&link, ctx, territory, combined),
        None => combined,
    };
    // P3.5: the spans snap to the union of the ladders (one ladder: the
    // ladder itself).
    Ok(op.annotate(op.spans(linked, &union.z), ctx.semantic_ctx))
}

/// K3: the outlines of an area op ([`RestUse::LevelClip`]: Pocket,
/// Zigzag, Face) less the obstacles, through the one territory rule
/// ([`super::Territory::area_outlines`]) at the lowest level of the
/// ladder. The op then plans around a clamp, and a row links to the next
/// row on its own side instead of a lift over the clamp. A contour op and
/// an input with no outline keep their outlines.
///
/// The answer holds, for each outline, the index of its ladder in
/// `ladders`. P3.5: with one ladder this is ONE call, as before P3.5 (the
/// K3 golden). With a ladder per area, each run of neighbour outlines on
/// one ladder is one call at the lowest level of that ladder, and each
/// clipped piece keeps the ladder of its run. The runs keep the order of
/// the areas (ruling L-7).
fn area_outlines_less_obstacles<'p, O: LayeredOp>(
    op: &O,
    prepared: &Prepared<'_>,
    outlines: Cow<'p, [O::Input]>,
    ladders: &Ladders,
) -> (Cow<'p, [O::Input]>, Vec<usize>) {
    let lowest = |levels: &[f64]| levels.iter().copied().fold(f64::INFINITY, f64::min);
    let as_is = |outlines: Cow<'p, [O::Input]>| (outlines, ladders.of_outline.clone());
    if op.rest_use() != RestUse::LevelClip {
        return as_is(outlines);
    }
    let Some(polygons) = outlines
        .iter()
        .map(|input| input.outline().cloned())
        .collect::<Option<Vec<Polygon2>>>()
    else {
        return as_is(outlines);
    };
    let territory = prepared.territory();
    if let [only] = ladders.ladders.as_slice() {
        return match territory.area_outlines(Cow::Borrowed(polygons.as_slice()), lowest(only)) {
            Cow::Borrowed(_) => as_is(outlines),
            Cow::Owned(clipped) => {
                let pieces = O::Input::from_polygons(Cow::Owned(clipped)).into_owned();
                let count = pieces.len();
                (Cow::Owned(pieces), vec![0; count])
            }
        };
    }
    // The runs of neighbour outlines on one ladder: (ladder, polygons,
    // inputs).
    let mut runs: Vec<(usize, Vec<Polygon2>, Vec<O::Input>)> = Vec::new();
    for ((polygon, input), &ladder) in polygons
        .into_iter()
        .zip(outlines.iter())
        .zip(&ladders.of_outline)
    {
        match runs.last_mut() {
            Some((run_ladder, run_polygons, run_inputs)) if *run_ladder == ladder => {
                run_polygons.push(polygon);
                run_inputs.push(input.clone());
            }
            Some(_) | None => runs.push((ladder, vec![polygon], vec![input.clone()])),
        }
    }
    let mut pieces: Vec<O::Input> = Vec::with_capacity(outlines.len());
    let mut ladder_of = Vec::with_capacity(outlines.len());
    for (ladder, run_polygons, run_inputs) in runs {
        let run_pieces = match territory.area_outlines(
            Cow::Borrowed(run_polygons.as_slice()),
            lowest(ladders.ladder(ladder)),
        ) {
            Cow::Borrowed(_) => run_inputs,
            Cow::Owned(clipped) => O::Input::from_polygons(Cow::Owned(clipped)).into_owned(),
        };
        ladder_of.extend(std::iter::repeat_n(ladder, run_pieces.len()));
        pieces.extend(run_pieces);
    }
    (Cow::Owned(pieces), ladder_of)
}

/// The runner's per-level rest inputs: the `ContourFilter` rule and its
/// tally.
struct LevelRules<'r> {
    contour: Option<&'r ContourFilterRule<'r>>,
    tally: &'r Cell<Option<RestLevelsFinding>>,
}

/// The cut of one outline at level `i`, or `None` for an empty level.
fn emit_at<O: LayeredOp>(
    op: &O,
    layers: &OutlineLayers<O::Geometry>,
    levels: &[f64],
    i: usize,
    rules: &LevelRules<'_>,
    cancel: &dyn CancelCheck,
) -> Result<Option<Toolpath>, OperationError> {
    let (Some(&z), Some(geometry), Some(&final_z)) =
        (levels.get(i), layers.geometry_at(i), levels.last())
    else {
        return Ok(None);
    };
    let piece = op.emit_level(
        geometry,
        LevelInfo {
            z,
            index: i,
            final_z,
        },
        &LevelRest {
            standing: rules.contour.map(|rule| rule.at(z)),
            split_timing: rules.contour.and_then(|rule| rule.split_timing()),
            tally: rules.tally,
        },
        cancel,
    )?;
    Ok((!piece.moves.is_empty()).then_some(piece))
}

/// The level plan of one outline and the geometry of each distinct region.
fn prepare_outline<O: LayeredOp>(
    op: &O,
    ctx: &ExecutionContext<'_>,
    unguarded: Option<&TriDexelStock>,
    tool_radius: f64,
    cancel: &dyn CancelCheck,
    outline: &O::Input,
    levels: &[f64],
) -> Result<OutlineLayers<O::Geometry>, OperationError> {
    if ctx.is_cancelled() {
        return Err(OperationError::Cancelled);
    }
    let layer = &LayerCtx {
        tool_radius,
        cancel,
        outline,
    };
    let prepare = |region: &[O::Input]| {
        let (geometry, report) = op.prepare(region, layer)?;
        ctx.record(&report);
        Ok::<_, OperationError>(geometry)
    };
    // Every rule decides here, so a new `RestUse` does not compile until
    // the Layered runner states what it does with it.
    let whole_plan = || LevelPlan::whole(levels.len(), None);
    let mut plan = match (op.rest_use(), outline.outline()) {
        (RestUse::LevelClip, Some(polygon)) => regions_per_level(
            unguarded,
            polygon,
            levels,
            layer.tool_radius,
            op.off_grid(),
            layer.cancel,
        )?,
        // W3.4: the skip reads the prepared tool-centre rings, so the whole
        // outline is prepared first. The plan holds `Whole` or `Empty` only.
        (RestUse::LevelSkip, Some(_)) => {
            let whole = prepare(std::slice::from_ref(outline))?;
            let band = op.reach_band(&whole, layer.tool_radius);
            let plan = levels_in_reach(
                unguarded,
                &band.rings,
                levels,
                band.half_width,
                layer.cancel,
            )?;
            ctx.record(&plan.finding);
            return Ok(OutlineLayers {
                plan,
                whole: Some(whole),
                clipped: Vec::new(),
            });
        }
        // An input with no outline has no region to clip or band to read:
        // it cuts the whole input at each level.
        (RestUse::LevelClip | RestUse::LevelSkip, None) => whole_plan(),
        // A `ContourFilter` op cuts the whole input at each level and gets
        // the rule per level ([`LevelRest`]).
        (RestUse::ContourFilter, _) => whole_plan(),
        // A Layered op that ignores the remaining stock cuts every level.
        (RestUse::None, _) => whole_plan(),
        // No Layered op has these rules: 2D Adaptive (`LevelMask`) and
        // Drill (`StockColumn`) keep their own loops (ruling C9), 3D Rough
        // (`Planner`) is Volumetric, and `Reference` and `LinkSafetyOnly`
        // are finish rules. The runner cuts every level for them.
        (
            RestUse::LevelMask
            | RestUse::StockColumn
            | RestUse::Planner
            | RestUse::Reference
            | RestUse::LinkSafetyOnly,
            _,
        ) => whole_plan(),
    };
    ctx.record(&plan.finding);
    let whole = if plan.needs_whole() {
        Some(prepare(std::slice::from_ref(outline))?)
    } else {
        None
    };
    // The pieces of each clip go to `prepare` once; `plan.levels` keeps
    // the index of each, so the plan does not need the pieces again.
    let clipped = std::mem::take(&mut plan.regions)
        .into_iter()
        .map(|region| prepare(&O::Input::from_polygons(Cow::Owned(region))))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(OutlineLayers {
        plan,
        whole,
        clipped,
    })
}

/// The 2.5D ladder: the pre-computed `cutting_levels` of the catalogue.
///
/// L-16 (geometry selection P3.2): an empty ladder refuses. Before P3.2 an
/// empty ladder fell back to `top - heights.depth()`, and `depth()` is
/// `(top - bottom).abs()`: a floor 5 mm ABOVE the top cut 5 mm BELOW it.
/// The session refuses a floor at or above the top first
/// (`FloorRefusal::AboveTop`); this refusal is the last guard.
///
/// # Errors
///
/// [`OperationError::Other`] when the ladder is empty.
pub(super) fn effective_levels(cutting_levels: &[f64]) -> Result<Vec<f64>, OperationError> {
    if cutting_levels.is_empty() {
        return Err(OperationError::Other(
            "the Z ladder of this op is empty: its floor is at or above its top, or its depth \
             or depth per pass is zero. Set a floor below the top and a depth per pass above \
             zero"
                .to_owned(),
        ));
    }
    Ok(cutting_levels.to_vec())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    //! Claim 3 of `tests/layered_framework_owns_the_ladder_w3.rs`: a cancel
    //! set mid-ladder stops the runner at the next level. The hook sits
    //! inside `emit_level`, so this test lives beside the runner.

    use std::cell::RefCell;
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;
    use crate::compute::catalog::{OperationConfig, OperationType};
    use crate::compute::config::ResolvedHeights;
    use crate::compute::cutter::build_cutter;
    use crate::compute::execute::GenerationFindings;
    use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
    use crate::geo::{BoundingBox3, P2, P3};

    /// The level at which the test op sets the cancel flag.
    const CANCEL_AT: usize = 1;

    /// A Layered op that emits one feed move per level and sets the cancel
    /// flag while it emits level [`CANCEL_AT`].
    struct CancelMidLadder<'a> {
        cancel: &'a AtomicBool,
        emitted: RefCell<Vec<usize>>,
        ordering: RegionOrdering,
    }

    impl LayeredOp for CancelMidLadder<'_> {
        type Input = Polygon2;
        type Geometry = ();
        type Report = OffsetFailures;

        fn region_ordering(&self) -> RegionOrdering {
            self.ordering
        }

        fn rest_use(&self) -> RestUse {
            RestUse::None
        }

        fn prepare(
            &self,
            _region: &[Polygon2],
            _layer: &LayerCtx<'_>,
        ) -> Result<((), OffsetFailures), OperationError> {
            Ok(((), OffsetFailures(0)))
        }

        fn emit_level(
            &self,
            _geometry: &(),
            level: LevelInfo,
            _rest: &LevelRest<'_>,
            _cancel: &dyn CancelCheck,
        ) -> Result<Toolpath, OperationError> {
            self.emitted.borrow_mut().push(level.index);
            if level.index == CANCEL_AT {
                self.cancel.store(true, Ordering::SeqCst);
            }
            let mut tp = Toolpath::new();
            tp.feed_to(P3::new(0.0, 0.0, level.z), 100.0);
            Ok(tp)
        }
    }

    fn square(cx: f64) -> Polygon2 {
        Polygon2::new(vec![
            P2::new(cx - 5.0, -5.0),
            P2::new(cx + 5.0, -5.0),
            P2::new(cx + 5.0, 5.0),
            P2::new(cx - 5.0, 5.0),
        ])
    }

    /// L-16 (geometry selection P3.2): an empty ladder refuses. It never
    /// falls back to a ladder from `ResolvedHeights::depth()`, which cut an
    /// inverted floor below the top. Part of G-BOTTOMPIN arm 5.
    #[test]
    fn an_empty_ladder_refuses_l16() {
        let refused = effective_levels(&[]);
        assert!(
            matches!(&refused, Err(OperationError::Other(text)) if text.contains("ladder")),
            "an empty ladder must refuse, got {refused:?}"
        );
        assert_eq!(effective_levels(&[-1.0, -2.0]).unwrap(), vec![-1.0, -2.0]);
    }

    #[test]
    fn a_cancel_set_mid_ladder_stops_at_the_next_level() {
        let heights = ResolvedHeights {
            clearance_z: 15.0,
            retract_z: 10.0,
            feed_z: 8.0,
            top_z: 0.0,
            bottom_z: -5.0,
            top_pinned: false,
            bottom_pinned: false,
        };
        let bbox = BoundingBox3 {
            min: P3::new(-20.0, -10.0, -10.0),
            max: P3::new(20.0, 10.0, 0.0),
        };
        let tool_cfg = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        let tool_def = build_cutter(&tool_cfg).unwrap();
        let polys = vec![square(-10.0), square(10.0)];
        // L-16: the runner reads the catalogue ladder; an empty one refuses.
        let levels = crate::ops::depth::floor_ladder(heights.top_z, heights.bottom_z, 1.0, 0);
        for ordering in [RegionOrdering::ByArea, RegionOrdering::Global] {
            let cancel = AtomicBool::new(false);
            let findings = RefCell::new(GenerationFindings::default());
            let ctx = ExecutionContext {
                selection: crate::compute::execute::SelectionInputs::from_polygons(&polys),
                ..ExecutionContext::new(
                    &findings, &tool_def, &tool_cfg, &heights, &levels, &bbox, &cancel,
                )
            };
            let prepared = crate::compute::execute::prologue::prepare(
                &ctx,
                &OperationConfig::new_default(OperationType::Pocket),
            )
            .unwrap();
            let op = CancelMidLadder {
                cancel: &cancel,
                emitted: RefCell::new(Vec::new()),
                ordering,
            };
            let ladder = op.ladder(&ctx).unwrap();
            assert!(
                ladder.len() > CANCEL_AT + 1,
                "{ordering:?}: the ladder must reach past the cancel level: {ladder:?}"
            );
            let result = run_layered(&op, &ctx, &prepared);
            assert!(
                matches!(result, Err(OperationError::Cancelled)),
                "{ordering:?}: a cancel set mid-ladder must end the run as Cancelled"
            );
            // The flag goes up while the first outline emits level 1. The
            // runner polls before the next (outline, level) and stops there.
            let expected: Vec<usize> = match ordering {
                RegionOrdering::ByArea => vec![0, 1],
                RegionOrdering::Global => vec![0, 0, 1],
            };
            assert_eq!(
                *op.emitted.borrow(),
                expected,
                "{ordering:?}: no level after the cancel may emit"
            );
        }
    }
}
