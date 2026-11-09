//! The plan of one piece ([`plan_piece`]): the piece set-up on the planner
//! lattice, the starter pocket, the field, the core spiral, the wraps and
//! the planned turns; and the 2D adaptive cleanup it goes through
//! ([`cleaned`]).

use std::f64::consts::SQRT_2;

use crate::geo::P2;
use crate::geometry::marching_squares::CHAIN_EPS;
use crate::interrupt::{CancelCheck, Cancelled};
use crate::polygon::{Polygon2, offset_polygon};

use crate::adaptive::path::{
    AdaptiveSegment, LinkLoad, apply_contour_parallel_residue_cleanup_turns,
    apply_residue_mop_cleanup_turns, emit_helical_starter_pocket, pass_load_for, planner_cell_size,
    turn_bound,
};
use crate::adaptive::search::{PassLoadMeasure, ToolCentreRegion};

use super::field::{GeodesicField, Lattice, Walls};
use super::wraps::{SpiralCore, held_core_spiral, ordered_wraps_beside_core};
use crate::adaptive::{
    AdaptiveParams, AdaptiveRuntimeEvent, CleanupStrategy, KeepDownLinks, MaterialGrid,
    PathStrategy2d,
};

// ── The producer ───────────────────────────────────────────────────────

/// The planner input every producer of one piece shares.
pub(crate) struct PieceSetup {
    pub(crate) grid: MaterialGrid,
    pub(crate) mask: Vec<bool>,
    pub(crate) link_load: LinkLoad,
}

impl PieceSetup {
    /// The set-up of `adaptive_segments_with_debug` (`path.rs`) for
    /// `polygon`: the material grid, its fringe, the machinable mask and the
    /// link load. `None` when the inset by the tool radius is empty.
    pub(crate) fn new(polygon: &Polygon2, params: &AdaptiveParams, cell: f64) -> Option<Self> {
        let machinable = offset_polygon(polygon, params.tool_radius)
            .into_iter()
            .next()?;
        let mut grid = MaterialGrid::from_polygon(polygon, cell);
        if params.keep_down_links == KeepDownLinks::WithinPassLoad {
            grid.keep_fringe(polygon);
        }
        if let Some(mask) = &params.standing_material {
            grid.seed_from_material_mask(mask);
        }
        let mask = MaterialGrid::build_machinable_mask(
            &machinable,
            grid.origin_x,
            grid.origin_y,
            grid.rows,
            grid.cols,
            grid.cell_size,
        );
        let link_load = LinkLoad::new(params, &grid, polygon);
        Some(Self {
            grid,
            mask,
            link_load,
        })
    }
}

/// The planner cell of `params` (`path.rs`, [`planner_cell_size`], the one
/// source of the planner lattice).
pub(crate) fn planner_cell(params: &AdaptiveParams) -> f64 {
    planner_cell_size(params.tool_radius, params.tolerance)
}

/// The plan of one piece.
pub(super) struct Produced {
    /// The starter pocket, then the core spiral and the wraps, before the
    /// cleanup.
    pub(super) segments: Vec<AdaptiveSegment>,
    /// The planned clearing turns, each a straight step no longer than
    /// [`turn_bound`].
    pub(super) turns: Vec<(P2, P2)>,
    /// Research test support ([`research_plan_piece`]): the wraps in
    /// cutting order, the core spiral, the seed, `K` and the level count.
    #[cfg(feature = "research")]
    pub(super) research: ResearchPlan,
}

/// Research test support (`research_geodesic.rs`, `research` feature): the
/// raw plan of one piece that the research readouts and the 2D harness read
/// (before the cleanup). No product path reads it.
#[cfg(feature = "research")]
#[derive(Clone)]
pub(crate) struct ResearchPlan {
    /// The starter pocket, then the core spiral and the wraps.
    pub(crate) segments: Vec<AdaptiveSegment>,
    /// The planned clearing turns.
    pub(crate) turns: Vec<(P2, P2)>,
    /// The wraps in cutting order, oriented.
    pub(crate) wraps: Vec<Vec<P2>>,
    /// The core spiral (empty without core levels).
    pub(crate) spiral: Vec<P2>,
    /// The seed (the starter pocket's centre).
    pub(crate) seed: P2,
    /// `K`: the levels the core spiral cut.
    pub(crate) spiral_levels: usize,
    /// The levels of the field.
    pub(crate) levels: usize,
}

/// Research test support: [`plan_piece`] with its raw plan for the research
/// readouts ([`ResearchPlan`]). `None` where the field does not apply.
///
/// # Errors
///
/// [`Cancelled`] when `cancel` fires.
#[cfg(feature = "research")]
pub(crate) fn research_plan_piece(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    cancel: &dyn CancelCheck,
) -> Result<Option<ResearchPlan>, Cancelled> {
    Ok(plan_piece(polygon, params, cancel)?.map(|p| ResearchPlan {
        segments: p.segments,
        turns: p.turns,
        ..p.research
    }))
}

/// Append `path` to the cutting run `run` from the cutter position `cur`:
/// a gap no longer than `reach` is a cutting step inside the `Cut` (the
/// spiral's seam step); a longer one is a keep-down `Link`, which the
/// replay admits under the pass load or turns into a re-entry.
pub(super) fn append_path(
    path: &[P2],
    run: &mut Vec<P2>,
    cur: &mut P2,
    segments: &mut Vec<AdaptiveSegment>,
    reach: f64,
) {
    let Some(&start) = path.first() else {
        return;
    };
    let gap = (start.x - cur.x).hypot(start.y - cur.y);
    if gap <= reach {
        if gap > CHAIN_EPS {
            run.push(start);
        }
        run.extend(path.iter().skip(1).copied());
    } else {
        if run.len() >= 2 {
            segments.push(AdaptiveSegment::Cut(std::mem::take(run)));
        }
        segments.push(AdaptiveSegment::Link(start));
        *run = path.to_vec();
    }
    if let Some(&end) = run.last() {
        *cur = end;
    }
}

/// The plan of C35 plan A on one piece: the starter pocket, the core
/// spiral, then the wraps beside the core, each open arc joined to the next
/// by a planned clearing turn where the turn fits.
pub(super) fn plan_piece(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    cancel: &dyn CancelCheck,
) -> Result<Option<Produced>, Cancelled> {
    let r = params.tool_radius;
    let s = params.stepover;
    let cell = planner_cell(params);
    let Some(mut setup) = PieceSetup::new(polygon, params, cell) else {
        return Ok(None);
    };
    let boundary_distances = setup.grid.compute_boundary_distances();
    let holds = params.keep_down_links == KeepDownLinks::WithinPassLoad;
    let pass = pass_load_for(params, cell);
    let Some((mut segments, starter_end)) = emit_helical_starter_pocket(
        &mut setup.grid,
        &setup.mask,
        &boundary_distances,
        r,
        holds.then_some((&pass, s, &setup.link_load)),
    ) else {
        return Ok(None);
    };
    let Some(seed) = segments.iter().find_map(|seg| match seg {
        AdaptiveSegment::Marker(AdaptiveRuntimeEvent::StarterPocket { center_x, center_y }) => {
            Some(P2::new(*center_x, *center_y))
        }
        AdaptiveSegment::Cut(_)
        | AdaptiveSegment::Rapid(_)
        | AdaptiveSegment::Link(_)
        | AdaptiveSegment::Marker(_) => None,
    }) else {
        return Ok(None);
    };

    let region = ToolCentreRegion::new(polygon, r);
    let walls = Walls { region: &region };
    let lattice = Lattice::of_grid(&setup.grid);
    let inside: Vec<bool> = setup
        .mask
        .iter()
        .enumerate()
        .map(|(i, &m)| m && region.contains(&lattice.node(i)))
        .collect();
    let Some(mut field) = GeodesicField::on_lattice(lattice, inside, seed, r, s, &walls) else {
        return Ok(None);
    };
    field.use_level_spacing(r, pass.ceiling * 2.0 * r, params.tolerance, cancel)?;

    let reach = s + (1.0 + SQRT_2) * cell;
    let phi0 = (starter_end.y - seed.y).atan2(starter_end.x - seed.x);
    // The closed, star-shaped levels round the seed are one spiral; the
    // wraps start past them.
    let (spiral, spiral_levels) = held_core_spiral(
        &field,
        &region,
        seed,
        r,
        phi0,
        s,
        pass.ceiling * 2.0 * r,
        params.tolerance,
        cancel,
    )?;
    // §12.9: beside the core spiral, every piece of a core level that is not
    // the core's own loop stays a wrap.
    let core = (spiral_levels > 0).then_some(SpiralCore {
        seed,
        levels: spiral_levels,
    });
    let wraps = ordered_wraps_beside_core(
        &field,
        &region,
        spiral.last().copied().unwrap_or(starter_end),
        reach,
        s,
        spiral_levels + 1,
        core,
        cancel,
    )?;

    let first = if spiral.is_empty() {
        wraps.iter().find_map(|w| w.pts.first().copied())
    } else {
        spiral.first().copied()
    };
    if let Some(first) = first {
        segments.push(AdaptiveSegment::Marker(AdaptiveRuntimeEvent::PassEntry {
            pass_index: 1,
            entry_x: first.x,
            entry_y: first.y,
        }));
    }
    let mut run: Vec<P2> = vec![starter_end];
    let mut cur = starter_end;
    if !spiral.is_empty() {
        append_path(&spiral, &mut run, &mut cur, &mut segments, reach);
    }
    // The zigzag turn from the end of the open arc just cut onto the start
    // of the next one, where it is no longer than the turn bound and stays
    // inside the region.
    let bound = turn_bound(s, cell);
    let mut turns: Vec<(P2, P2)> = Vec::new();
    for (i, w) in wraps.iter().enumerate() {
        if !w.closed
            && let Some(prev) = i.checked_sub(1).and_then(|k| wraps.get(k))
            && !prev.closed
            && !prev.pts.is_empty()
            && let Some(&to) = w.pts.first()
        {
            let len = (to.x - cur.x).hypot(to.y - cur.y);
            if len <= bound && region.contains_segment(cur, to) && len > CHAIN_EPS {
                turns.push((cur, to));
            }
        }
        append_path(&w.pts, &mut run, &mut cur, &mut segments, reach);
    }
    if run.len() >= 2 {
        segments.push(AdaptiveSegment::Cut(run));
    }
    Ok(Some(Produced {
        #[cfg(feature = "research")]
        research: ResearchPlan {
            segments: Vec::new(),
            turns: Vec::new(),
            wraps: wraps.iter().map(|w| w.pts.clone()).collect(),
            spiral: spiral.clone(),
            seed,
            spiral_levels,
            levels: field.levels.len().saturating_sub(1),
        },
        segments,
        turns,
    }))
}

pub(crate) fn params_with(params: &AdaptiveParams, strategy: PathStrategy2d) -> AdaptiveParams {
    AdaptiveParams {
        tool_radius: params.tool_radius,
        stepover: params.stepover,
        cut_depth: params.cut_depth,
        feed_rate: params.feed_rate,
        plunge_rate: params.plunge_rate,
        safe_z: params.safe_z,
        tolerance: params.tolerance,
        slot_clearing: params.slot_clearing,
        min_cutting_radius: params.min_cutting_radius,
        standing_material: params.standing_material.as_ref().map(std::sync::Arc::clone),
        cleanup_strategy: params.cleanup_strategy,
        engagement_measure: params.engagement_measure,
        path_strategy: strategy,
        trochoid_cap_mult: params.trochoid_cap_mult,
        keep_down_links: params.keep_down_links,
        entry_helix_radius: params.entry_helix_radius,
        entry_helix_core_floor: params.entry_helix_core_floor,
    }
}

/// The cleanup of `adaptive_toolpath_structured_annotated_traced_with_cancel`
/// (`mod.rs`): the same dispatch on the cleanup strategy.
/// C14: the replay of the kept segments admits the planned clearing turns
/// `turns` (empty for every column before C14). D65: with `capped`, it also
/// admits a step over the ceiling that reads at most a slot
/// (`LinkLoad::admits_capped`); only the 3D slice door sets it, because only
/// that door caps the feed of such a step (`slice::feed_caps`).
pub(crate) fn cleaned(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    segments: Vec<AdaptiveSegment>,
    turns: &[(P2, P2)],
    capped: bool,
) -> Vec<AdaptiveSegment> {
    let m = PassLoadMeasure::SweptWidth;
    match params.cleanup_strategy {
        CleanupStrategy::Legacy => segments,
        CleanupStrategy::ResidueMop | CleanupStrategy::ContourParallelNarrow => {
            apply_residue_mop_cleanup_turns(polygon, params, &segments, m, turns, capped)
        }
        CleanupStrategy::ContourParallelHybrid => apply_contour_parallel_residue_cleanup_turns(
            polygon, params, &segments, m, turns, capped,
        ),
    }
}
