//! The 3D door of Geodesic Adaptive (integration step 7.4): what the 3D
//! slice layer (`adaptive3d/clearing.rs`, `plan_one_outline`) calls for one
//! slice piece. The plan ([`slice_segments`]), its cleanup and its replay
//! after the F-038 group filter ([`slice_cleanup`], [`slice_replay`]), the
//! keep-down links held at the level ([`HeldLinks`]), the chord tolerance
//! margin ([`slice_load_margin`]), and the feed caps of the planned turns
//! and the departure windows ([`feed_caps`]).

use crate::adaptive::path::{
    AdaptiveSegment, pass_load_for, pass_step_len, planner_cell_size, replay_after_group_filter,
    subdivide, subdivide_each,
};
use crate::adaptive::search::{PassLoad, PassLoadMeasure, measure_step};
use crate::adaptive::{AdaptiveParams, AdaptiveRuntimeEvent, MaterialGrid};
use crate::geo::{P2, point_to_segment_distance};
use crate::interrupt::{CancelCheck, Cancelled};
use crate::polygon::Polygon2;

use super::plan::{PieceSetup, cleaned, plan_piece, planner_cell};

/// The plan of one slice piece before the cleanup, and its planned turns.
pub(crate) struct SlicePlan {
    /// The starter pocket, the core spiral and the wraps.
    pub(crate) segments: Vec<AdaptiveSegment>,
    /// The planned clearing turns, each no longer than `turn_bound`.
    pub(crate) turns: Vec<(P2, P2)>,
}

/// The plan of one slice piece (`plan_piece`). `None` where the field does
/// not apply (no starter pocket fits, or no node is seeded): the arm then
/// leaves the piece to the rest pass.
///
/// # Errors
///
/// [`Cancelled`] when `cancel` fires.
pub(crate) fn slice_segments(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    cancel: &dyn CancelCheck,
) -> Result<Option<SlicePlan>, Cancelled> {
    Ok(plan_piece(polygon, params, cancel)?.map(|p| SlicePlan {
        segments: p.segments,
        turns: p.turns,
    }))
}

/// The 2D cleanup of the plan, whose replay admits the planned turns and,
/// D65, a step over the ceiling that reads at most a slot ([`feed_caps`]
/// caps its feed).
pub(crate) fn slice_cleanup(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    segments: Vec<AdaptiveSegment>,
    turns: &[(P2, P2)],
) -> Vec<AdaptiveSegment> {
    cleaned(polygon, params, segments, turns, true)
}

/// The replay of the kept plan after the F-038 group filter
/// (`replay_after_group_filter`, research C22a): a link or a cut that the
/// cleanup admitted over a group the filter then dropped is decided again
/// on the ground the kept plan cuts.
pub(crate) fn slice_replay(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    segments: Vec<AdaptiveSegment>,
    turns: &[(P2, P2)],
) -> Vec<AdaptiveSegment> {
    replay_after_group_filter(
        polygon,
        params,
        segments,
        PassLoadMeasure::SweptWidth,
        turns,
        true,
    )
}

/// The margin (mm) between the pass ceiling `c·D` (`PassLoad::swept_width`,
/// the replay's own ceiling) and the swept width `min(s, D)` that a straight
/// pass at the stepover reads.
///
/// A chord of the emitted cut that lies a sagitta `h` inside its walked arc
/// leaves `h` of stock that the 2D plan held cut, so the next pass meets
/// `s + h` at mid-chord. A chord tolerance at or under this margin keeps
/// that pass at or under `c·D`. The floor is the shortest emitted move
/// (`MIN_EMITTED_SEGMENT_MM`), so the RDP always has a positive tolerance.
pub(crate) fn slice_load_margin(stepover: f64, tool_radius: f64, tolerance: f64) -> f64 {
    let diameter = 2.0 * tool_radius;
    let cell = planner_cell_size(tool_radius, tolerance);
    let ceiling = PassLoad::swept_width(stepover, tool_radius, cell).ceiling;
    (ceiling * diameter - stepover.min(diameter)).max(crate::toolpath::MIN_EMITTED_SEGMENT_MM)
}

/// The rule that admitted a capped run of [`feed_caps`] over the ceiling
/// (P22a-2). The 3D slice names it on its cap marker
/// (`adaptive3d::FeedCapSource`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SliceCapKind {
    /// A planned clearing turn (`LinkLoad::admits_turn`).
    Turn,
    /// D65: a step over the ceiling that reads at most a slot
    /// (`LinkLoad::admits_capped`).
    Capped,
    /// A plan window out of a straight plunge (`PassLoad::departing`).
    Departure,
}

/// One straight run that the replay admits over the ceiling: a planned
/// turn, or a plan window out of a straight plunge (a departure; its steps
/// are exempt from the ceiling, not from the feed rule).
#[derive(Debug, Clone, Copy)]
struct FedRun {
    from: P2,
    to: P2,
    /// The largest [`step_reading`] of its steps.
    reading: f64,
}

/// The load reading of one fed step from `from` to `to` on `grid`, as a
/// fraction of the diameter: the larger of two terms (lead ruling
/// 2026-10-28, step 7.4a).
///
/// - The swept width `a_e / D`: the engagement across the path, the
///   planner's width read (`measure_step` with `swept`, the
///   [`PassLoad::swept_width`] rule the replay admits a step by).
/// - The area per mm over the diameter: the removal per mm of travel
///   ([`PassLoad::area_per_mm`] with `area`). This is the quantity of the
///   one load `L` (chip area per strip over the full-depth slot,
///   `tool_load::chip_load_fraction`). The swept width under-reads it on a
///   curve: the path turns inside the material, and the cutter removes more
///   per mm than the width across its end.
///
/// On a straight full-depth cut the two agree (on the lattice, to one
/// cell over the diameter): a side cut of width `w` clears `w` mm² per mm,
/// and the full-pass load is the extent over the diameter (test
/// `a_full_pass_reads_the_extent_over_the_diameter`). The maximum only
/// raises the reading: the safe reading wins. Test
/// `the_area_term_binds_on_a_tight_turn` plants a turn where it binds.
pub(super) fn step_reading(
    grid: &MaterialGrid,
    from: P2,
    to: P2,
    tool_radius: f64,
    swept: &PassLoad,
    area: &PassLoad,
) -> f64 {
    let angle = (to.y - from.y).atan2(to.x - from.x);
    let width = measure_step(grid, from, to.x, to.y, tool_radius, angle, swept);
    let removal = measure_step(grid, from, to.x, to.y, tool_radius, angle, area);
    width.max(removal)
}

/// The feed factor of a run whose steps read `reading` ([`step_reading`])
/// against the pass ceiling `c`: `c / reading` over the ceiling, else 1.
///
/// The removal rate of a step is `a_p × reading·D × F`; the steady cut holds
/// the reading at `c`, so `F' = F · c / reading` holds the steady removal
/// rate (the shear term of the power model, `tool_load/power.rs`). No
/// constant enters.
pub(super) fn feed_factor(ceiling: f64, reading: f64) -> f64 {
    if reading > ceiling && reading > 0.0 {
        ceiling / reading
    } else {
        1.0
    }
}

/// The emitted plan `segments` of one slice piece, each `Cut` split where a
/// planned turn or a departure window starts or ends, and the feed factor
/// of each output segment (`None`: no cap). The 3D lift wraps a capped
/// cut in `FeedCapStart` and `FeedCapEnd` markers, and the emitter writes
/// `Move::feed_cap` on its moves. No move is matched by position.
///
/// The runs (research C22, C38 §15.6): one replay of `segments` on the
/// piece's planner lattice, with the replay's own entry stamps and cutter
/// stamps, reads each step of the clearing producer that lies on a planned
/// turn (both ends on its segment within 1e-6 mm, as
/// `LinkLoad::admits_turn` reads it), and each step out of a straight
/// plunge (its start within `R` of the plunge point) with the plan window
/// that holds it. The runs are the turns with at least one step, in turn
/// order, then the departure windows in replay order.
///
/// The split (research `split_at_turns`, D14, C38 §15.8): each turn is a
/// `Cut` of its own; all departure windows share one run key, so a run of
/// them stays one `Cut`, capped at the smallest factor of the windows it
/// crosses. A
/// step of zero length stays with the run it is in. The points and their
/// order do not change.
pub(crate) fn feed_caps(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    segments: Vec<AdaptiveSegment>,
    turns: &[(P2, P2)],
) -> (Vec<AdaptiveSegment>, Vec<Option<(f64, SliceCapKind)>>) {
    let (runs, first_capped, first_departure, ceiling, _grid) =
        fed_runs(polygon, params, &segments, turns);
    if runs.is_empty() {
        let n = segments.len();
        return (segments, vec![None; n]);
    }
    // The run a step lies on (its index in `runs`), and the run key the
    // split reads: one turn, or every departure window together.
    let run_of = |a: P2, b: P2| {
        runs.iter().position(|t| {
            point_to_segment_distance(&a, &t.from, &t.to) < 1e-6
                && point_to_segment_distance(&b, &t.from, &t.to) < 1e-6
        })
    };
    let key_of = |run: Option<usize>| run.map(|i| i.min(first_departure));
    let factor_of = |run: usize| {
        runs.get(run)
            .map_or(1.0, |t| feed_factor(ceiling, t.reading))
    };
    let mut out = Vec::with_capacity(segments.len());
    let mut caps = Vec::with_capacity(segments.len());
    for seg in segments {
        let path = match seg {
            AdaptiveSegment::Cut(path) if path.len() >= 2 => path,
            other @ (AdaptiveSegment::Cut(_)
            | AdaptiveSegment::Link(_)
            | AdaptiveSegment::Rapid(_)
            | AdaptiveSegment::Marker(_)) => {
                out.push(other);
                caps.push(None);
                continue;
            }
        };
        let mut piece: Vec<P2> = Vec::new();
        // The run key the piece holds: `None` before its first step; then
        // the key of its steps, or `None` for steps on no run. The factor
        // of the piece: the smallest factor of the runs its steps lie on
        // (one turn, or the departure windows it crosses).
        let mut run: Option<Option<usize>> = None;
        let mut factor = 1.0_f64;
        // The kind of a run key: its band in the order of `fed_runs`.
        let kind_of = |key: usize| {
            if key < first_capped {
                SliceCapKind::Turn
            } else if key < first_departure {
                SliceCapKind::Capped
            } else {
                SliceCapKind::Departure
            }
        };
        let cap = |key: Option<usize>, factor: f64| {
            key.filter(|_| factor < 1.0).map(|k| (factor, kind_of(k)))
        };
        for w in path.windows(2) {
            let &[a, b] = w else {
                continue;
            };
            let (kind, step_factor) = if (b.x - a.x).hypot(b.y - a.y) > 1e-6 {
                let on = run_of(a, b);
                (key_of(on), on.map_or(1.0, factor_of))
            } else {
                (run.flatten(), 1.0)
            };
            if let Some(held) = run
                && held != kind
                && piece.len() >= 2
            {
                out.push(AdaptiveSegment::Cut(std::mem::take(&mut piece)));
                caps.push(cap(held, factor));
                factor = 1.0;
            }
            if piece.is_empty() {
                piece.push(a);
            }
            piece.push(b);
            run = Some(kind);
            factor = factor.min(step_factor);
        }
        if piece.len() >= 2 {
            out.push(AdaptiveSegment::Cut(piece));
            caps.push(cap(run.flatten(), factor));
        }
    }
    (out, caps)
}

/// The runs of [`feed_caps`]: the turns with a step, then the D65 capped
/// windows, then the departure windows; the index of the first capped
/// window; the index of the first departure window; the pass ceiling; and
/// the replay grid after the last segment (`None` when the piece has no
/// set-up).
fn fed_runs(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    segments: &[AdaptiveSegment],
    turns: &[(P2, P2)],
) -> (Vec<FedRun>, usize, usize, f64, Option<MaterialGrid>) {
    let r = params.tool_radius;
    let cell = planner_cell(params);
    let pass = pass_load_for(params, cell);
    let area = PassLoad::area_per_mm(params.stepover, r, cell);
    let Some(mut setup) = PieceSetup::new(polygon, params, cell) else {
        return (Vec::new(), 0, 0, pass.ceiling, None);
    };
    let step_len = pass_step_len(cell);
    let on_turn = |a: P2, b: P2| {
        turns.iter().position(|&(p, q)| {
            point_to_segment_distance(&a, &p, &q) < 1e-6
                && point_to_segment_distance(&b, &p, &q) < 1e-6
        })
    };
    let mut turn_reads: Vec<Option<f64>> = vec![None; turns.len()];
    let mut departures: Vec<FedRun> = Vec::new();
    // D65: the plan windows that hold a step the replay admitted over the
    // ceiling under the capped rule (`LinkLoad::admits_capped`): a step off
    // every turn and every departure whose swept width reads over c.
    let mut capped: Vec<FedRun> = Vec::new();
    let mut here: Option<P2> = None;
    let mut plunged_at: Option<P2> = None;
    let mut clearing = false;
    // The replay's own rule (`replay_kept_segments`): a segment after a
    // `SlotClearing` marker, up to the next marker of any kind.
    let mut in_slot_lines = false;
    for seg in segments {
        match seg {
            AdaptiveSegment::Marker(event) => {
                clearing = match event {
                    AdaptiveRuntimeEvent::PassEntry { .. } => true,
                    AdaptiveRuntimeEvent::PassSummary { .. }
                    | AdaptiveRuntimeEvent::ForcedClear { .. } => clearing,
                    AdaptiveRuntimeEvent::StarterPocket { .. }
                    | AdaptiveRuntimeEvent::ResidueContour { .. }
                    | AdaptiveRuntimeEvent::BoundaryCleanup { .. }
                    | AdaptiveRuntimeEvent::ResidueMop { .. }
                    | AdaptiveRuntimeEvent::SlotClearing { .. } => false,
                };
                in_slot_lines = matches!(event, AdaptiveRuntimeEvent::SlotClearing { .. });
            }
            AdaptiveSegment::Rapid(p) => {
                setup.link_load.stamp_entry(&mut setup.grid, *p);
                plunged_at = setup.link_load.plunge_departure_exempt().then_some(*p);
                here = Some(*p);
            }
            AdaptiveSegment::Link(p) => {
                if let Some(from) = here {
                    setup.grid.clear_circle(from.x, from.y, r);
                    let mut at = from;
                    for next in subdivide(&[from, *p], step_len).into_iter().skip(1) {
                        setup.grid.clear_segment(at, next, r);
                        at = next;
                    }
                }
                plunged_at = None;
                here = Some(*p);
            }
            AdaptiveSegment::Cut(path) => {
                let mut pts: Vec<(P2, usize)> = Vec::with_capacity(path.len());
                subdivide_each(path, step_len, |p, window| pts.push((p, window)));
                if let Some(&(p0, _)) = pts.first() {
                    setup.grid.clear_circle(p0.x, p0.y, r);
                }
                let departure = plunged_at.take();
                for w in pts.windows(2) {
                    let &[(a, _), (b, window)] = w else {
                        continue;
                    };
                    if !in_slot_lines {
                        let departing = departure.is_some_and(|d| (a.x - d.x).hypot(a.y - d.y) < r);
                        if departing {
                            if let (Some(&wa), Some(&wb)) = (path.get(window), path.get(window + 1))
                            {
                                let reading = step_reading(&setup.grid, a, b, r, &pass, &area);
                                match departures.last_mut() {
                                    Some(d) if d.from == wa && d.to == wb => {
                                        d.reading = d.reading.max(reading);
                                    }
                                    _ => departures.push(FedRun {
                                        from: wa,
                                        to: wb,
                                        reading,
                                    }),
                                }
                            }
                        } else if clearing && let Some(k) = on_turn(a, b) {
                            let reading = step_reading(&setup.grid, a, b, r, &pass, &area);
                            if let Some(slot) = turn_reads.get_mut(k) {
                                *slot = Some(slot.map_or(reading, |m: f64| m.max(reading)));
                            }
                        } else if on_turn(a, b).is_none()
                            && measure_step(
                                &setup.grid,
                                a,
                                b.x,
                                b.y,
                                r,
                                (b.y - a.y).atan2(b.x - a.x),
                                &pass,
                            ) > pass.ceiling
                            && let (Some(&wa), Some(&wb)) = (path.get(window), path.get(window + 1))
                        {
                            let reading = step_reading(&setup.grid, a, b, r, &pass, &area);
                            match capped.last_mut() {
                                Some(c) if c.from == wa && c.to == wb => {
                                    c.reading = c.reading.max(reading);
                                }
                                _ => capped.push(FedRun {
                                    from: wa,
                                    to: wb,
                                    reading,
                                }),
                            }
                        }
                    }
                    setup.grid.clear_segment(a, b, r);
                }
                if let Some(&(end, _)) = pts.last() {
                    here = Some(end);
                }
            }
        }
    }
    let mut runs: Vec<FedRun> = turns
        .iter()
        .zip(&turn_reads)
        .filter_map(|(&(from, to), read)| read.map(|reading| FedRun { from, to, reading }))
        .collect();
    // D65: each capped window is a run of its own (a key of its own, as a
    // turn), before the departure windows, which share one key.
    let first_capped = runs.len();
    runs.extend(capped);
    let first_departure = runs.len();
    runs.extend(departures);
    (
        runs,
        first_capped,
        first_departure,
        pass.ceiling,
        Some(setup.grid),
    )
}

/// P22f-2a probe (test-support only): the centres of the cells that the
/// replay of `segments` (the plan that goes out) holds cut, in the piece's
/// 2D frame (world XY). The replay is [`feed_caps`]'s own (`fed_runs`).
#[cfg(feature = "test-support")]
pub(crate) fn replay_cut_cells(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    segments: &[AdaptiveSegment],
    turns: &[(P2, P2)],
) -> Vec<P2> {
    let (_, _, _, _, grid) = fed_runs(polygon, params, segments, turns);
    let Some(grid) = grid else {
        return Vec::new();
    };
    let centre = |index: usize| (index as f64 + 0.5) * grid.cell_size;
    grid.cells
        .iter()
        .enumerate()
        .filter(|&(_, &cell)| cell == crate::adaptive::material_grid::CELL_CLEARED)
        .map(|(i, _)| {
            P2::new(
                grid.origin_x + centre(i % grid.cols),
                grid.origin_y + centre(i / grid.cols),
            )
        })
        .collect()
}

/// C22: the length (mm) of the `Link`s of `segments` from the cutter
/// position (a `Link` before any position adds 0). The 3D slice's F-038
/// group filter adds it to a group's cut length under
/// [`KeepDownLinks::WithinPassLoad`], because the arm cuts each such link
/// ([`HeldLinks`]); C12 counted the same length as cut length.
pub(crate) fn link_length(segments: &[AdaptiveSegment]) -> f64 {
    let mut here: Option<P2> = None;
    let mut length = 0.0;
    for seg in segments {
        match seg {
            AdaptiveSegment::Link(end) => {
                if let Some(from) = here {
                    length += (end.x - from.x).hypot(end.y - from.y);
                }
                here = Some(*end);
            }
            AdaptiveSegment::Cut(path) => {
                if let Some(&end) = path.last() {
                    here = Some(end);
                }
            }
            AdaptiveSegment::Rapid(entry) => here = Some(*entry),
            AdaptiveSegment::Marker(_) => {}
        }
    }
    length
}

/// The keep-down links of the 3D arm, held at the level as keep-down
/// links (C12, C22, C26, C26b).
///
/// Under [`KeepDownLinks::WithinPassLoad`] the replay admits a link only
/// within the pass load and stamps it on the planner grid as a feed at
/// depth. The returns of the one-way arcs and the links of the zigzag arcs
/// (C14) run through the band just cut. The shipped 3D lift turns a `Link`
/// into an entry. C12 emitted each link as a cut, but the lift then calls
/// a run of a cut through cleared stock "air", and a run longer than the
/// air crossover (`air_run_crossover_mm`) becomes an entry: a retract (or
/// a keep-down link that ends at the rapid floor) and the entry style down
/// to the level. The 2D plan has no entry there (C22).
///
/// C22 held each link as a `Cut`, so the toolpath tagged it a clearing cut
/// and the pages read it as cut length (C24: 1940 mm, 22 % of the one-way
/// cut at Corne Z 5.00). C26 emitted each held link as the 3D keep-down
/// link `Adaptive3dSegment::Link`, tagged `MoveIntent::Linking`. But a
/// 3D `Link` is two points, which the emitter drapes only at every tool
/// radius, and the lift's large-Z split does not see it. Where the 2D
/// link crosses terrain above the level (the level drapes), the draped
/// link climbed the wall in one straight move and cut the model (C26b,
/// Corne zigzag: X 55.935 Y -15.766, 1.45 mm below the leave).
///
/// So [`Self::next`] gives each `Link` after a known position as the
/// path from the cutter position to the link end, subdivided at the
/// planner cell (the lift reads each point, as it reads a cut). The 3D
/// slice emits it as one keep-down `Link` only when every lifted point is
/// at the level; else it lifts it as C22 did: a cut held at depth, which
/// the lift splits at a large Z step. A `Link` before any position stays
/// a 2D `Link` (an entry).
pub(crate) struct HeldLinks {
    here: Option<P2>,
    cell: f64,
}

impl HeldLinks {
    pub(crate) fn new(params: &AdaptiveParams) -> Self {
        Self {
            here: None,
            cell: planner_cell(params),
        }
    }

    /// `segment` unchanged, and for a `Link` after a known position the
    /// held path: the cutter position to the link end, subdivided at the
    /// planner cell.
    pub(crate) fn next(&mut self, segment: AdaptiveSegment) -> (AdaptiveSegment, Option<Vec<P2>>) {
        let held = match &segment {
            AdaptiveSegment::Link(end) => self
                .here
                .replace(*end)
                .map(|from| subdivide(&[from, *end], self.cell)),
            AdaptiveSegment::Cut(path) => {
                if let Some(&end) = path.last() {
                    self.here = Some(end);
                }
                None
            }
            AdaptiveSegment::Rapid(entry) => {
                self.here = Some(*entry);
                None
            }
            AdaptiveSegment::Marker(_) => None,
        };
        (segment, held)
    }
}
