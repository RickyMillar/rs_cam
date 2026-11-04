//! Research test support of Geodesic Adaptive (`adaptive::geodesic`), the
//! adaptive research 2026-10-23, candidate (a). Behind the `research`
//! feature; no product path calls it.
//!
//! Integration step 7.3d moved the plan into the product module
//! (`adaptive::geodesic`, the one preset C35 plan A) and deleted the research
//! options and presets
//! (`planning/adaptive_research_2026-10-23/integration/step7_design.md`).
//! What stays here reads a plan; it plans nothing of its own:
//! - the pass-load readout of a plan and its step classes
//!   ([`PassLoadReadout`], [`StepPart`], [`Producer`]);
//! - the cascade diagnostic ([`CascadeReadout`]);
//! - the turn feeds and their records ([`turn_feed_factor`],
//!   [`TurnRecord`]) and the entry split ([`EntrySplit`]);
//! - the 2D harness door ([`run_2d`], [`run_2d_with`]): a shipped strategy
//!   through the product pipeline, or the product plan through
//!   `adaptive::geodesic` (`research_plan_piece`), then the shared cleanup
//!   and emission, with every readout.

use crate::geo::{P2, point_to_segment_distance};
use crate::interrupt::{CancelCheck, Cancelled};
use crate::polygon::Polygon2;
use crate::toolpath::{MoveIntent, Toolpath};

use super::geodesic::{
    PieceSetup, ResearchPlan, planner_cell, research_params_with as params_with,
    research_plan_piece,
};
use super::path::{
    AdaptiveSegment, apply_contour_parallel_residue_cleanup_turns, apply_residue_mop_cleanup_turns,
    pass_load_for, pass_step_len, segments_to_toolpath, subdivide, subdivide_each,
};
use super::search::{PassLoad, PassLoadMeasure, ToolCentreRegion, measure_step};
use super::{
    AdaptiveParams, AdaptiveRuntimeEvent, CleanupStrategy, KeepDownLinks, MaterialGrid,
    PathStrategy2d,
};

/// What the product plan gave for one piece (the research door
/// `research_plan_piece`), for the harness summaries.
#[derive(Debug, Clone, Default)]
pub struct FieldReport {
    /// The field applied: a starter pocket fit and the seed was set.
    pub applied: bool,
    /// The seed (the starter pocket's centre).
    pub seed: Option<P2>,
    /// The seed disc's radius: the tool radius.
    pub seed_radius: f64,
    /// The levels of the field.
    pub levels: usize,
    /// The wraps planned.
    pub chains: usize,
    /// `K`: the levels the core spiral cut.
    pub spiral_levels: usize,
    /// The planned clearing turns.
    pub turns: usize,
}

/// The segment of `poly` nearest `p`: the distance and the segment's two
/// ends. `None` for fewer than two points.
fn nearest_segment(poly: &[P2], p: P2) -> Option<(f64, P2, P2)> {
    poly.windows(2)
        .filter_map(|w| match w {
            &[u, v] => Some((point_to_segment_distance(&p, &u, &v), u, v)),
            _ => None,
        })
        .min_by(|x, y| x.0.total_cmp(&y.0))
}

// ── The pass-load readout ──────────────────────────────────────────────

/// Which producer planned a segment: the kind of the last marker before it
/// in the cleaned plan (`StarterPocket`, `PassEntry`, `ResidueContour`,
/// `BoundaryCleanup`, `ResidueMop`, `SlotClearing`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Producer {
    /// Before any marker.
    #[default]
    Unmarked,
    /// The helical starter pocket.
    Starter,
    /// The clearing passes: candidate (a)'s wraps, the agent's passes, the
    /// contour spiral.
    Clearing,
    /// A contour-parallel residue loop.
    ResidueContour,
    /// The final wall pass (`finish_walls_within_pass_load`).
    WallPass,
    /// A residue-mop patch.
    ResidueMop,
    /// The opt-in slot-clearing lines.
    SlotLines,
}

impl Producer {
    /// The producer that a segment after `event` belongs to. A pass summary
    /// and a forced clear do not start a new producer.
    fn after(self, event: &AdaptiveRuntimeEvent) -> Self {
        match event {
            AdaptiveRuntimeEvent::StarterPocket { .. } => Self::Starter,
            AdaptiveRuntimeEvent::PassEntry { .. } => Self::Clearing,
            AdaptiveRuntimeEvent::ResidueContour { .. } => Self::ResidueContour,
            AdaptiveRuntimeEvent::BoundaryCleanup { .. } => Self::WallPass,
            AdaptiveRuntimeEvent::ResidueMop { .. } => Self::ResidueMop,
            AdaptiveRuntimeEvent::SlotClearing { .. } => Self::SlotLines,
            AdaptiveRuntimeEvent::PassSummary { .. } | AdaptiveRuntimeEvent::ForcedClear { .. } => {
                self
            }
        }
    }

    /// A short name for a summary line.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Unmarked => "unmarked",
            Self::Starter => "starter",
            Self::Clearing => "clearing",
            Self::ResidueContour => "residue contour",
            Self::WallPass => "wall pass",
            Self::ResidueMop => "residue mop",
            Self::SlotLines => "slot lines",
        }
    }
}

/// One step that the readout reads over the ceiling.
#[derive(Debug, Clone, Copy)]
pub struct OverStep {
    /// Where the step starts and ends (mm).
    pub from: P2,
    pub to: P2,
    /// The reading (`measure_step`, a_e / D) of the step.
    pub reading: f64,
    /// The producer of the segment that holds the step.
    pub producer: Producer,
    /// True for a keep-down `Link` step, false for a `Cut` step.
    pub link: bool,
    /// The index of the segment in the plan of its piece.
    pub segment: usize,
    /// Where on candidate (a)'s plan the step lies ([`StepPart`]); only the
    /// raw-plan readout classifies it.
    pub part: StepPart,
}

/// Where a step of candidate (a)'s plan lies (C3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StepPart {
    /// Not classified (a cleaned plan, a shipped strategy).
    #[default]
    Unclassified,
    /// On the core spiral (its end within one cell of the spiral polyline).
    Spiral,
    /// On a planned clearing turn (C14): both ends on the
    /// turn's segment, the test the replay admits it by
    /// (`LinkLoad::admits_turn`).
    Turn,
    /// On a planned wrap (its end within one cell of the wrap polyline).
    Wrap,
    /// Off every planned piece: a seam step or an arc-to-arc step.
    Join,
}

impl StepPart {
    /// A short name for a summary line.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Unclassified => "",
            Self::Spiral => "spiral",
            Self::Turn => "clearing turn",
            Self::Wrap => "on a wrap",
            Self::Join => "join (seam or arc-to-arc step)",
        }
    }

    /// A one-word name for a tally line.
    #[must_use]
    pub fn short(self) -> &'static str {
        match self {
            Self::Unclassified => "",
            Self::Spiral => "spiral",
            Self::Turn => "turn",
            Self::Wrap => "wrap",
            Self::Join => "join",
        }
    }
}

/// The planned polylines of candidate (a) by [`StepPart`], to classify
/// each step of the raw plan: a step whose end AND midpoint lie within one
/// cell of a polyline is on that part (the first match in the order
/// spiral, morph leg, return, transition, wrap); else it is a join. (C5: the midpoint
/// test is new; at C4 a radial join step that ended on the next wrap was
/// classed "wrap".) One cell is the lattice the
/// pieces were traced on. A return runs on its leg's polyline the other
/// way, so for a morph leg and a return the step must also run the way of
/// the nearest segment (a positive dot product).
#[derive(Debug, Clone, Default)]
struct PartIndex {
    cell: f64,
    /// Each polyline with its bounding box (low, high corner) and part.
    polys: Vec<(P2, P2, Vec<P2>, StepPart)>,
    /// C14: the planned clearing turns, tested first and exactly
    /// ([`Self::on_turn`]).
    turns: Vec<(P2, P2)>,
    /// C14: true for the index of the emitted plan, which classes only the
    /// turns: every other step is [`StepPart::Unclassified`], as before.
    only_turns: bool,
}

impl PartIndex {
    fn new(cell: f64) -> Self {
        Self {
            cell,
            polys: Vec::new(),
            turns: Vec::new(),
            only_turns: false,
        }
    }

    /// C14: the index of the planned turns `turns` alone, for the readout
    /// of the emitted plan.
    fn of_turns(turns: &[(P2, P2)], cell: f64) -> Self {
        Self {
            turns: turns.to_vec(),
            only_turns: true,
            ..Self::new(cell)
        }
    }

    /// C14: the index of the planned turn that holds the step from `a` to
    /// `b` (both ends on its segment within the walker's same-point
    /// tolerance, as `LinkLoad::admits_turn` reads it).
    fn on_turn(&self, a: P2, b: P2) -> Option<usize> {
        self.turns.iter().position(|&(p, q)| {
            point_to_segment_distance(&a, &p, &q) < 1e-6
                && point_to_segment_distance(&b, &p, &q) < 1e-6
        })
    }

    fn add(&mut self, pts: &[P2], part: StepPart) {
        if pts.is_empty() {
            return;
        }
        let mut lo = P2::new(f64::INFINITY, f64::INFINITY);
        let mut hi = P2::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
        for p in pts {
            lo = P2::new(lo.x.min(p.x), lo.y.min(p.y));
            hi = P2::new(hi.x.max(p.x), hi.y.max(p.y));
        }
        self.polys.push((lo, hi, pts.to_vec(), part));
    }

    /// The part of the step from `a` to `p`.
    fn part(&self, a: P2, p: P2) -> StepPart {
        if self.on_turn(a, p).is_some() {
            return StepPart::Turn;
        }
        if self.only_turns {
            return StepPart::Unclassified;
        }
        let mid = P2::new(0.5 * (a.x + p.x), 0.5 * (a.y + p.y));
        let near = |part: StepPart| {
            self.polys.iter().any(|(lo, hi, pts, q)| {
                *q == part
                    && p.x >= lo.x - self.cell
                    && p.x <= hi.x + self.cell
                    && p.y >= lo.y - self.cell
                    && p.y <= hi.y + self.cell
                    && nearest_segment(pts, mid).is_some_and(|(d, _, _)| d <= self.cell)
                    && nearest_segment(pts, p).is_some_and(|(d, _, _)| d <= self.cell)
            })
        };
        [StepPart::Spiral, StepPart::Wrap]
            .into_iter()
            .find(|&part| near(part))
            .unwrap_or(StepPart::Join)
    }
}

/// The readout of one class of steps (C4): a producer of the cleaned plan,
/// or a producer and a [`StepPart`] of the raw plan.
#[derive(Debug, Clone, Default)]
pub struct LoadTally {
    /// The class: the producer's label, then the part or "link".
    pub label: String,
    /// The largest reading of a step in the class.
    pub max_reading: f64,
    /// Steps read.
    pub steps: usize,
    /// Steps over the ceiling: in the raw plan, the steps the replay
    /// refuses.
    pub steps_over: usize,
    /// C7: the largest swept-width reading and the steps over the ceiling
    /// in it ...
    pub max_swept: f64,
    pub over_swept: usize,
    /// ... and the same in area per mm over D ([`ReplayMeasure::AreaPerMm`]),
    /// for the same steps.
    pub max_area: f64,
    pub over_area: usize,
}

/// The planner's own pass-load reading of a plan, replayed on a fresh grid.
///
/// Every `Cut` step (split to the planner's pass step) and every `Link` step
/// is read with `measure_step` and held with `step_within_pass_load` on the
/// grid as cut so far, and then stamped, the way `replay_kept_segments` and
/// `feed_link_checked` (`path.rs`) read them. A `Rapid` stamps the entry hole
/// (`LinkLoad::stamp_entry`). The steps out of a straight plunge are exempt
/// (`PassLoad::departing`) and counted apart. Slot-clearing lines (opt-in)
/// are stamped, not read. This is the planner predicate on the planner grid,
/// not the simulation oracle of `a_clearing_cut_holds_the_pass_load`.
///
/// Each step over the ceiling is kept in [`Self::over`] with its position,
/// its producer and its reading (verifier D, 2026-10-23: the readout gave
/// no place, so an over-step could not be told from shipped code).
#[derive(Debug, Clone, Default)]
pub struct PassLoadReadout {
    /// The ceiling (radial fraction a_e / D).
    pub ceiling: f64,
    /// The largest reading of a step that is not exempt.
    pub max_reading: f64,
    /// Steps read.
    pub steps: usize,
    /// Steps over the ceiling (`step_within_pass_load` false).
    pub steps_over: usize,
    /// Steps out of a straight plunge, exempt from the ceiling.
    pub exempt_steps: usize,
    /// Every step over the ceiling, in plan order.
    pub over: Vec<OverStep>,
    /// The readout per class of steps ([`LoadTally`]), in the order the
    /// classes first appear.
    pub tallies: Vec<LoadTally>,
    /// C7: the measure `max_reading`, `steps_over` and `over` are read in
    /// (the measure the column's replay held the plan to).
    pub measure: ReplayMeasure,
    /// C7: both readings of every step read, side by side: the largest
    /// swept width and its steps over the ceiling ...
    pub max_swept: f64,
    pub over_swept: usize,
    /// ... and the largest area per mm over D and its steps over.
    pub max_area: f64,
    pub over_area: usize,
    /// C14: every step on a planned clearing turn ([`StepPart::Turn`]),
    /// with its measured engagement. A turn step over the ceiling is the
    /// counted exemption: it is not in [`Self::steps_over`] or
    /// [`Self::over`] (the replay admits it), and it is counted in
    /// [`Self::turn_over`].
    pub turn_steps: Vec<TurnStep>,
    pub turn_over: usize,
    /// C14: the largest reading and the steps over the ceiling among the
    /// steps on no turn (the rule every other step holds).
    pub non_turn_max: f64,
    pub non_turn_over: usize,
    /// C38 §15.6: every step out of a straight plunge (exempt from the
    /// ceiling, in [`Self::exempt_steps`]), with the plan window that holds
    /// it and its measured engagement, for its feed.
    pub departure_steps: Vec<DepartureStep>,
}

/// C38 §15.6: one step out of a straight plunge, read on the readout's
/// grid. It is exempt from the ceiling, not from the feed rule: the 3D arm
/// feeds its window at the slot factor of its reading.
#[derive(Debug, Clone, Copy)]
pub struct DepartureStep {
    /// The plan window (one step of the `Cut` path) that holds the step.
    pub window_from: P2,
    pub window_to: P2,
    /// The swept width and the area per mm, fractions of D.
    pub swept: f64,
    pub area: f64,
}

/// C14: one step on a planned clearing turn, read on the readout's grid.
#[derive(Debug, Clone, Copy)]
pub struct TurnStep {
    /// The index of the turn in its piece's turn list.
    pub turn: usize,
    pub from: P2,
    pub to: P2,
    /// The swept width and the area per mm, fractions of D.
    pub swept: f64,
    pub area: f64,
}

impl PassLoadReadout {
    fn merge(&mut self, other: &Self) {
        self.ceiling = self.ceiling.max(other.ceiling);
        self.max_reading = self.max_reading.max(other.max_reading);
        self.steps += other.steps;
        self.steps_over += other.steps_over;
        self.exempt_steps += other.exempt_steps;
        self.over.extend_from_slice(&other.over);
        self.measure = other.measure;
        self.max_swept = self.max_swept.max(other.max_swept);
        self.over_swept += other.over_swept;
        self.max_area = self.max_area.max(other.max_area);
        self.over_area += other.over_area;
        self.turn_steps.extend_from_slice(&other.turn_steps);
        self.turn_over += other.turn_over;
        self.non_turn_max = self.non_turn_max.max(other.non_turn_max);
        self.non_turn_over += other.non_turn_over;
        self.departure_steps
            .extend_from_slice(&other.departure_steps);
        for t in &other.tallies {
            self.tally(t);
        }
    }

    /// Add the counts of `sample` to the tally of its label.
    fn tally(&mut self, sample: &LoadTally) {
        if let Some(t) = self.tallies.iter_mut().find(|t| t.label == sample.label) {
            t.max_reading = t.max_reading.max(sample.max_reading);
            t.steps += sample.steps;
            t.steps_over += sample.steps_over;
            t.max_swept = t.max_swept.max(sample.max_swept);
            t.over_swept += sample.over_swept;
            t.max_area = t.max_area.max(sample.max_area);
            t.over_area += sample.over_area;
        } else {
            self.tallies.push(sample.clone());
        }
    }
}

/// The measure a research replay holds the plan to (C7). Public twin of
/// the crate's `PassLoadMeasure` for the harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReplayMeasure {
    /// The shipped rule: swept width against `c·D`.
    #[default]
    SweptWidth,
    /// FreeCAD's quantity (R1 §3): the area of standing stock a step clears
    /// per mm of travel ([`super::search::compute_area_per_mm`]), against
    /// the same `c·D`. On a straight side cut of width `w` the area per mm
    /// is `w`, the swept width, so the ceiling carries over with no new
    /// constant.
    AreaPerMm,
}

impl ReplayMeasure {
    fn crate_measure(self) -> PassLoadMeasure {
        match self {
            Self::SweptWidth => PassLoadMeasure::SweptWidth,
            Self::AreaPerMm => PassLoadMeasure::AreaPerMm,
        }
    }

    /// The pass load of `params` on a lattice of `cell` in this measure.
    fn pass(self, params: &AdaptiveParams, cell: f64) -> PassLoad {
        match self {
            Self::SweptWidth => pass_load_for(params, cell),
            Self::AreaPerMm => PassLoad::area_per_mm(params.stepover, params.tool_radius, cell),
        }
    }
}

/// Where a step sits in the plan, for an [`OverStep`].
#[derive(Clone, Copy)]
struct StepTag {
    producer: Producer,
    link: bool,
    segment: usize,
}

/// The readout of `segments` on `polygon`. With `parts` (candidate (a)'s
/// raw plan) each clearing step is classed by [`PartIndex::part`] and each
/// over-step records its part; the tallies are per producer and part.
/// Without it the tallies are per producer.
fn pass_load_readout(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    segments: &[AdaptiveSegment],
    parts: Option<&PartIndex>,
    cell: f64,
    measure: ReplayMeasure,
) -> PassLoadReadout {
    let r = params.tool_radius;
    let pass = measure.pass(params, cell);
    let swept_pass = ReplayMeasure::SweptWidth.pass(params, cell);
    let area_pass = ReplayMeasure::AreaPerMm.pass(params, cell);
    let mut out = PassLoadReadout {
        ceiling: pass.ceiling,
        measure,
        ..PassLoadReadout::default()
    };
    let Some(mut setup) = PieceSetup::new(polygon, params, cell) else {
        return out;
    };
    let step_len = pass_step_len(cell);
    let mut here: Option<P2> = None;
    let mut plunged_at: Option<P2> = None;
    let mut producer = Producer::Unmarked;
    // The replay's own rule (`replay_kept_segments`): a segment after a
    // `SlotClearing` marker, up to the next marker of any kind.
    let mut in_slot_lines = false;
    let read = |grid: &MaterialGrid,
                a: P2,
                b: P2,
                departing: bool,
                tag: StepTag,
                out: &mut PassLoadReadout| {
        if departing {
            out.exempt_steps += 1;
            return;
        }
        let angle = (b.y - a.y).atan2(b.x - a.x);
        let reading = measure_step(grid, a, b.x, b.y, r, angle, &pass);
        let swept = measure_step(grid, a, b.x, b.y, r, angle, &swept_pass);
        let area = measure_step(grid, a, b.x, b.y, r, angle, &area_pass);
        out.steps += 1;
        out.max_reading = out.max_reading.max(reading);
        let part = match parts {
            Some(index) if tag.producer == Producer::Clearing && !tag.link => index.part(a, b),
            _ => StepPart::Unclassified,
        };
        // C14: a step on a planned turn is the counted exemption.
        let turn = match parts {
            Some(index) if part == StepPart::Turn => index.on_turn(a, b),
            _ => None,
        };
        let within = reading <= pass.ceiling;
        match turn {
            Some(turn) => {
                out.turn_steps.push(TurnStep {
                    turn,
                    from: a,
                    to: b,
                    swept,
                    area,
                });
                out.turn_over += usize::from(!within);
            }
            None => {
                out.non_turn_max = out.non_turn_max.max(reading);
                out.non_turn_over += usize::from(!within);
            }
        }
        let held = within || turn.is_some();
        let (swept_over, area_over) = (swept > swept_pass.ceiling, area > area_pass.ceiling);
        out.max_swept = out.max_swept.max(swept);
        out.over_swept += usize::from(swept_over);
        out.max_area = out.max_area.max(area);
        out.over_area += usize::from(area_over);
        out.tally(&LoadTally {
            label: step_label(tag, part),
            max_reading: reading,
            steps: 1,
            steps_over: usize::from(!held),
            max_swept: swept,
            over_swept: usize::from(swept_over),
            max_area: area,
            over_area: usize::from(area_over),
        });
        if !held {
            out.steps_over += 1;
            out.over.push(OverStep {
                from: a,
                to: b,
                reading,
                producer: tag.producer,
                link: tag.link,
                segment: tag.segment,
                part,
            });
        }
    };
    for (index, seg) in segments.iter().enumerate() {
        match seg {
            AdaptiveSegment::Marker(event) => {
                producer = producer.after(event);
                in_slot_lines = matches!(event, AdaptiveRuntimeEvent::SlotClearing { .. });
            }
            AdaptiveSegment::Rapid(p) => {
                setup.link_load.stamp_entry(&mut setup.grid, *p);
                plunged_at = setup.link_load.plunge_departure_exempt().then_some(*p);
                here = Some(*p);
            }
            AdaptiveSegment::Link(p) => {
                if let Some(from) = here {
                    let len = (p.x - from.x).hypot(p.y - from.y);
                    let n = ((len / step_len).ceil() as usize).max(1);
                    setup.grid.clear_circle(from.x, from.y, r);
                    let tag = StepTag {
                        producer,
                        link: true,
                        segment: index,
                    };
                    let mut at = from;
                    for k in 1..=n {
                        let t = k as f64 / n as f64;
                        let next =
                            P2::new(from.x + t * (p.x - from.x), from.y + t * (p.y - from.y));
                        read(&setup.grid, at, next, false, tag, &mut out);
                        setup.grid.clear_segment(at, next, r);
                        at = next;
                    }
                }
                plunged_at = None;
                here = Some(*p);
            }
            AdaptiveSegment::Cut(path) => {
                // The points of `subdivide`, each with its plan window, so
                // that a departing step knows its window (§15.6).
                let mut pts: Vec<(P2, usize)> = Vec::with_capacity(path.len());
                subdivide_each(path, step_len, |p, window| pts.push((p, window)));
                if let Some(&(p0, _)) = pts.first() {
                    setup.grid.clear_circle(p0.x, p0.y, r);
                }
                let departure = plunged_at.take();
                let tag = StepTag {
                    producer,
                    link: false,
                    segment: index,
                };
                for w in pts.windows(2) {
                    let &[(a, _), (b, window)] = w else {
                        continue;
                    };
                    if !in_slot_lines {
                        let departing = departure.is_some_and(|d| (a.x - d.x).hypot(a.y - d.y) < r);
                        if departing
                            && let (Some(&wa), Some(&wb)) = (path.get(window), path.get(window + 1))
                        {
                            let angle = (b.y - a.y).atan2(b.x - a.x);
                            out.departure_steps.push(DepartureStep {
                                window_from: wa,
                                window_to: wb,
                                swept: measure_step(
                                    &setup.grid,
                                    a,
                                    b.x,
                                    b.y,
                                    r,
                                    angle,
                                    &swept_pass,
                                ),
                                area: measure_step(&setup.grid, a, b.x, b.y, r, angle, &area_pass),
                            });
                        }
                        read(&setup.grid, a, b, departing, tag, &mut out);
                    }
                    setup.grid.clear_segment(a, b, r);
                }
                if let Some(&(end, _)) = pts.last() {
                    here = Some(end);
                }
            }
        }
    }
    out
}

/// The tally label of a step: the producer, then "link" or the part.
fn step_label(tag: StepTag, part: StepPart) -> String {
    if tag.link {
        format!("{} link", tag.producer.label())
    } else if part == StepPart::Unclassified {
        tag.producer.label().to_owned()
    } else {
        format!("{} {}", tag.producer.label(), part.short())
    }
}

/// One step the replay model refuses (C5, [`CascadeReadout`]).
#[derive(Debug, Clone, Copy)]
pub struct Refusal {
    pub from: P2,
    pub to: P2,
    pub producer: Producer,
    pub part: StepPart,
    /// True for a step of a keep-down `Link`.
    pub link: bool,
    /// The index of the segment in the raw plan of its piece.
    pub segment: usize,
    /// The reading on the replay grid (only accepted steps stamped).
    pub replay_reading: f64,
    /// The reading on the planned grid (every step before it stamped).
    pub planned_reading: f64,
    /// True when the step holds on the planned grid: the stock it met is
    /// stock the plan cleared but the replay did not stamp, because a step
    /// before it was refused. False: the step meets too much stock even
    /// when every step before it is cut (an intrinsic refusal).
    pub cascade: bool,
    /// The distance (mm) of the step end from the seed.
    pub rho: Option<f64>,
}

impl Refusal {
    /// The class of the step (as [`LoadTally::label`]).
    #[must_use]
    pub fn label(&self) -> String {
        step_label(
            StepTag {
                producer: self.producer,
                link: self.link,
                segment: self.segment,
            },
            self.part,
        )
    }
}

/// The cascade counts of one class of steps (C5).
#[derive(Debug, Clone, Default)]
pub struct CascadeTally {
    /// The class (as [`LoadTally::label`]).
    pub label: String,
    /// Steps the replay model refuses.
    pub refused: usize,
    /// Of those, the cascades (they hold on the planned grid).
    pub cascade: usize,
    /// Of those, the intrinsic refusals.
    pub intrinsic: usize,
    /// The largest planned-grid reading of an intrinsic refusal.
    pub max_intrinsic: f64,
}

/// The cascade diagnostic of a raw plan (C5).
///
/// The replay (`replay_kept_segments`, `emit_capped_walk_with` in
/// `path.rs`) stamps only the steps it accepts. A refused step leaves its
/// stock, and a later step that the plan meant to run on cleared stock may
/// then be refused too: a cascade. The diagnostic walks the raw plan on two
/// grids at once:
///
/// - the planned grid stamps every step (the raw readout's grid, so its
///   over-steps are those of [`Summary2d::raw_pass_load`]);
/// - the replay grid models the replay: a step that holds there is
///   stamped; a refused step is not, and the cutter lifts; while lifted, a
///   step that holds WITHOUT the disc at its start cleared re-starts the
///   cut there, with the entry hole stamped (`LinkLoad::stamp_entry`; the
///   replay may reach it by a keep-down link instead, which stamps the
///   link's corridor, not a hole). A `Link` holds when
///   every step of it holds (else it is a re-entry at its end); a `Rapid`
///   stamps its entry on both grids.
///
/// A refusal on the replay grid is a cascade when the same step holds on
/// the planned grid, and intrinsic when it does not.
#[derive(Debug, Clone, Default)]
pub struct CascadeReadout {
    /// Per class of steps, in the order the classes first appear.
    pub tallies: Vec<CascadeTally>,
    /// The first refusal of each producer piece (a run of steps of one raw
    /// segment and one part), in plan order.
    pub first_of_piece: Vec<Refusal>,
    /// Every intrinsic refusal, in plan order.
    pub intrinsic: Vec<Refusal>,
    /// C6: the intrinsic refusals whose planned reading lies in the first
    /// lattice quantum over the ceiling, `(c, c + cell/D]`, with `cell` the
    /// lattice the walk read on ([`Self::read_cell`]). The lattice reads an
    /// edge up to one cell off (`PassLoad::swept_width`), so a refusal in
    /// that band may be the measure's quantum, not the stock.
    pub near_ceiling: usize,
    /// The lattice cell (mm) the walk read on.
    pub read_cell: f64,
}

impl CascadeReadout {
    fn merge(&mut self, other: &Self) {
        for t in &other.tallies {
            self.add(t);
        }
        self.first_of_piece.extend_from_slice(&other.first_of_piece);
        self.intrinsic.extend_from_slice(&other.intrinsic);
        self.near_ceiling += other.near_ceiling;
        self.read_cell = self.read_cell.max(other.read_cell);
    }

    /// Add the counts of `t` to the tally of its label.
    fn add(&mut self, t: &CascadeTally) {
        if let Some(mine) = self.tallies.iter_mut().find(|m| m.label == t.label) {
            mine.refused += t.refused;
            mine.cascade += t.cascade;
            mine.intrinsic += t.intrinsic;
            mine.max_intrinsic = mine.max_intrinsic.max(t.max_intrinsic);
        } else {
            self.tallies.push(t.clone());
        }
    }

    /// The refusals, the cascades among them, and the intrinsic ones, over
    /// every class.
    #[must_use]
    pub fn totals(&self) -> (usize, usize, usize) {
        self.tallies.iter().fold((0, 0, 0), |(r, c, i), t| {
            (r + t.refused, c + t.cascade, i + t.intrinsic)
        })
    }
}

/// The two grids of the cascade walk and where the replay cutter stands.
struct CascadeWalk {
    plan: PieceSetup,
    replay: PieceSetup,
    pass: PassLoad,
    tool_radius: f64,
    /// True while the replay cutter is down (cutting).
    down: bool,
    piece: Option<(usize, StepPart)>,
    piece_refused: bool,
    seed: Option<P2>,
    out: CascadeReadout,
}

impl CascadeWalk {
    /// One cutting step from `a` to `b` on both grids.
    fn step(&mut self, a: P2, b: P2, tag: StepTag, part: StepPart, departing: bool) {
        let key = (tag.segment, part);
        if self.piece != Some(key) {
            self.piece = Some(key);
            self.piece_refused = false;
        }
        let load = if departing {
            self.pass.departing()
        } else {
            self.pass
        };
        let r = self.tool_radius;
        let angle = (b.y - a.y).atan2(b.x - a.x);
        let planned = measure_step(&self.plan.grid, a, b.x, b.y, r, angle, &load);
        self.plan.grid.clear_segment(a, b, r);
        let replayed = measure_step(&self.replay.grid, a, b.x, b.y, r, angle, &load);
        // C14: the replay admits a planned turn over the ceiling.
        if replayed <= load.ceiling || part == StepPart::Turn {
            if !self.down {
                self.replay.link_load.stamp_entry(&mut self.replay.grid, a);
                self.down = true;
            }
            self.replay.grid.clear_segment(a, b, r);
            return;
        }
        self.down = false;
        self.refuse(a, b, tag, part, replayed, planned);
    }

    /// Record a refusal on the replay grid. A refused step is never a
    /// departure from a plunge (that load has no ceiling), so the ceiling is
    /// the pass ceiling.
    fn refuse(&mut self, a: P2, b: P2, tag: StepTag, part: StepPart, replayed: f64, planned: f64) {
        let cascade = planned <= self.pass.ceiling;
        let refusal = Refusal {
            from: a,
            to: b,
            producer: tag.producer,
            part,
            link: tag.link,
            segment: tag.segment,
            replay_reading: replayed,
            planned_reading: planned,
            cascade,
            rho: self.seed.map(|c| (b.x - c.x).hypot(b.y - c.y)),
        };
        self.out.add(&CascadeTally {
            label: step_label(tag, part),
            refused: 1,
            cascade: usize::from(cascade),
            intrinsic: usize::from(!cascade),
            max_intrinsic: if cascade { 0.0 } else { planned },
        });
        if !cascade {
            self.out.intrinsic.push(refusal);
            let quantum = self.out.read_cell / (2.0 * self.tool_radius);
            if planned <= self.pass.ceiling + quantum {
                self.out.near_ceiling += 1;
            }
        }
        if !self.piece_refused {
            self.piece_refused = true;
            self.out.first_of_piece.push(refusal);
        }
    }

    /// A keep-down link from `from` to `to` in steps of at most `step_len`.
    fn link(&mut self, from: P2, to: P2, step_len: f64, tag: StepTag) {
        let r = self.tool_radius;
        let len = (to.x - from.x).hypot(to.y - from.y);
        let n = ((len / step_len).ceil() as usize).max(1);
        self.plan.grid.clear_circle(from.x, from.y, r);
        let mut trial = self.replay.grid.clone();
        if self.down {
            trial.clear_circle(from.x, from.y, r);
        }
        let mut held = true;
        let mut at = from;
        for k in 1..=n {
            let t = k as f64 / n as f64;
            let next = P2::new(from.x + t * (to.x - from.x), from.y + t * (to.y - from.y));
            let angle = (next.y - at.y).atan2(next.x - at.x);
            let planned = measure_step(&self.plan.grid, at, next.x, next.y, r, angle, &self.pass);
            self.plan.grid.clear_segment(at, next, r);
            if held {
                let replayed = measure_step(&trial, at, next.x, next.y, r, angle, &self.pass);
                if replayed <= self.pass.ceiling {
                    trial.clear_segment(at, next, r);
                } else {
                    held = false;
                    self.piece = Some((tag.segment, StepPart::Unclassified));
                    self.piece_refused = false;
                    self.refuse(at, next, tag, StepPart::Unclassified, replayed, planned);
                }
            }
            at = next;
        }
        if held {
            self.replay.grid = trial;
        } else {
            // The refused link is a re-entry at its end.
            self.replay.link_load.stamp_entry(&mut self.replay.grid, to);
        }
        self.down = true;
    }
}

/// The cascade diagnostic ([`CascadeReadout`]) of the raw plan `segments`
/// of one piece, read on a lattice of `cell`. `parts` classes the clearing
/// steps as in [`pass_load_readout`]; `seed` gives each refusal its
/// distance.
fn cascade_readout(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    segments: &[AdaptiveSegment],
    parts: Option<&PartIndex>,
    seed: Option<P2>,
    cell: f64,
    measure: ReplayMeasure,
) -> CascadeReadout {
    let (Some(plan), Some(replay)) = (
        PieceSetup::new(polygon, params, cell),
        PieceSetup::new(polygon, params, cell),
    ) else {
        return CascadeReadout::default();
    };
    let mut walk = CascadeWalk {
        plan,
        replay,
        pass: measure.pass(params, cell),
        tool_radius: params.tool_radius,
        down: true,
        piece: None,
        piece_refused: false,
        seed,
        out: CascadeReadout {
            read_cell: cell,
            ..CascadeReadout::default()
        },
    };
    let r = params.tool_radius;
    let step_len = pass_step_len(cell);
    let mut here: Option<P2> = None;
    let mut plunged_at: Option<P2> = None;
    let mut producer = Producer::Unmarked;
    let mut in_slot_lines = false;
    for (index, seg) in segments.iter().enumerate() {
        match seg {
            AdaptiveSegment::Marker(event) => {
                producer = producer.after(event);
                in_slot_lines = matches!(event, AdaptiveRuntimeEvent::SlotClearing { .. });
            }
            AdaptiveSegment::Rapid(p) => {
                walk.plan.link_load.stamp_entry(&mut walk.plan.grid, *p);
                walk.replay.link_load.stamp_entry(&mut walk.replay.grid, *p);
                walk.down = true;
                plunged_at = walk.plan.link_load.plunge_departure_exempt().then_some(*p);
                here = Some(*p);
            }
            AdaptiveSegment::Link(p) => {
                if let Some(from) = here {
                    let tag = StepTag {
                        producer,
                        link: true,
                        segment: index,
                    };
                    walk.link(from, *p, step_len, tag);
                }
                plunged_at = None;
                here = Some(*p);
            }
            AdaptiveSegment::Cut(path) => {
                let pts = subdivide(path, step_len);
                if let Some(&p0) = pts.first() {
                    walk.plan.grid.clear_circle(p0.x, p0.y, r);
                    if walk.down {
                        walk.replay.grid.clear_circle(p0.x, p0.y, r);
                    }
                }
                let departure = plunged_at.take();
                let tag = StepTag {
                    producer,
                    link: false,
                    segment: index,
                };
                for w in pts.windows(2) {
                    let &[a, b] = w else {
                        continue;
                    };
                    if in_slot_lines {
                        walk.plan.grid.clear_segment(a, b, r);
                        walk.replay.grid.clear_segment(a, b, r);
                        continue;
                    }
                    let departing = departure.is_some_and(|d| (a.x - d.x).hypot(a.y - d.y) < r);
                    let part = match parts {
                        Some(ix) if producer == Producer::Clearing => ix.part(a, b),
                        _ => StepPart::Unclassified,
                    };
                    walk.step(a, b, tag, part, departing);
                }
                if let Some(&end) = pts.last() {
                    here = Some(end);
                }
            }
        }
    }
    walk.out
}

/// What a drawn span of the plan is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanKind {
    /// A `Cut` polyline.
    Cut,
    /// A keep-down `Link`: a straight move at cut depth.
    Link,
    /// A `Rapid` re-entry: the travel at safe Z to the entry, then the entry.
    Rapid,
}

/// One span of the cleaned plan, for the render: what it is, who planned
/// it, and its cutter-centre points. A `Link` or `Rapid` span holds its
/// start and its end.
#[derive(Debug, Clone)]
pub struct Span2d {
    pub kind: SpanKind,
    pub producer: Producer,
    pub pts: Vec<P2>,
}

fn spans_of(segments: &[AdaptiveSegment]) -> Vec<Span2d> {
    let mut out = Vec::new();
    let mut producer = Producer::Unmarked;
    let mut here: Option<P2> = None;
    for seg in segments {
        match seg {
            AdaptiveSegment::Marker(event) => producer = producer.after(event),
            AdaptiveSegment::Cut(path) => {
                out.push(Span2d {
                    kind: SpanKind::Cut,
                    producer,
                    pts: path.clone(),
                });
                here = path.last().copied().or(here);
            }
            AdaptiveSegment::Link(p) | AdaptiveSegment::Rapid(p) => {
                let kind = if matches!(seg, AdaptiveSegment::Link(_)) {
                    SpanKind::Link
                } else {
                    SpanKind::Rapid
                };
                out.push(Span2d {
                    kind,
                    producer,
                    pts: here.into_iter().chain(std::iter::once(*p)).collect(),
                });
                here = Some(*p);
            }
        }
    }
    out
}

// ── The run (harness door) ─────────────────────────────────────────────

/// Which 2D clearing a research run plans.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Candidate2d {
    /// A shipped strategy, through the product pipeline.
    Shipped(PathStrategy2d),
    /// The product plan of Geodesic Adaptive (`adaptive::geodesic`, C35
    /// plan A).
    Geodesic,
}

/// Counts of one run, from the planner segments (before the toolpath
/// simplification and the corner blend) and the emitted toolpath.
#[derive(Debug, Clone, Default)]
pub struct Summary2d {
    /// Inset pieces planned.
    pub pieces: usize,
    /// Length (mm) of every `Cut` polyline.
    pub cut_length_mm: f64,
    /// Length (mm) of every keep-down `Link`.
    pub link_length_mm: f64,
    /// Keep-down links (`Link` segments).
    pub keep_down_links: usize,
    /// Entries: plunges (`Rapid` segments), the first one of each piece
    /// included.
    pub entries: usize,
    /// Lifts to the safe Z in the emitted toolpath (`MoveIntent::Retract`),
    /// the last one of each piece included.
    pub retracts: usize,
    /// The planner's pass-load reading of the cleaned plan (what is
    /// emitted).
    pub pass_load: PassLoadReadout,
    /// The same reading of the producer's raw plan, before the cleanup
    /// replays it (C3): its over-steps are the steps the replay refuses,
    /// in plan order. Exact up to the first one; after it the replay has
    /// lifted and re-entered, so its grid differs from this readout's.
    pub raw_pass_load: PassLoadReadout,
    /// Where the entries of the cleaned plan come from ([`EntrySplit`]).
    pub entry_split: EntrySplit,
    /// The cascade diagnostic of the raw plan (C5, [`CascadeReadout`]).
    pub cascade: CascadeReadout,
    /// C14: the clearing turns of the emitted plan (the zigzag arc links).
    pub turns: TurnSummary,
}

/// C14: the clearing turns of a run: each turn of the emitted plan with
/// its measured engagement and the reduced feed it carries.
#[derive(Debug, Clone, Default)]
pub struct TurnSummary {
    /// Turns the producer planned.
    pub planned: usize,
    /// One record per planned turn that the emitted plan cuts (at least one
    /// step read on it), in plan order.
    pub turns: Vec<TurnRecord>,
    /// Turns whose emitted toolpath moves carry the reduced feed, and the
    /// emitted turns with no move found to carry it (must be 0).
    pub fed: usize,
    pub unfed: usize,
    /// C20: emitted turns with no move found on them whose feed factor is
    /// 1 (their measured engagement is within the ceiling). Such a turn
    /// needs no reduced feed: its feed is the full feed, which every
    /// clearing move carries. It is not in [`Self::unfed`].
    pub full_feed_unmatched: usize,
    /// §14.6: turns the 2D plan cuts that have no emitted move and neither
    /// end inside the boundary clip. The 3D stage's clip removed them; they
    /// are not in [`Self::turns`] and not in [`Self::unfed`].
    pub clipped: usize,
}

/// C14: one clearing turn of the emitted plan.
#[derive(Debug, Clone, Copy)]
pub struct TurnRecord {
    pub from: P2,
    pub to: P2,
    pub length_mm: f64,
    /// Its steps read, and those over the ceiling (the exemption used).
    pub steps: usize,
    pub steps_over: usize,
    /// The largest swept width and area per mm of its steps (fractions of
    /// D): its measured engagement.
    pub max_swept: f64,
    pub max_area: f64,
    /// The feed factor ([`turn_feed_factor`]) and the feed (mm/min) its
    /// moves carry.
    pub feed_factor: f64,
    pub feed_mm_min: f64,
    /// C16: false when no emitted move was found to carry the feed (the
    /// turn counted in [`TurnSummary::unfed`], or in
    /// [`TurnSummary::full_feed_unmatched`] when its factor is 1).
    pub fed: bool,
    /// C20: for a turn with no move on it, the emitted move that comes
    /// nearest to holding the whole turn ([`HoldingMove`]).
    pub holding: Option<HoldingMove>,
    /// C38 §15.4: the Z level of the 3D slice that planned it
    /// ([`SliceTurns::add`]); `None` in 2D.
    pub level_z: Option<f64>,
}

/// C20: the emitted `Linear` move that holds a turn whose own vertex the
/// emitter dropped. The emitter drops a walked point that lies within
/// `MIN_EMITTED_SEGMENT_MM` of the chord of its neighbours
/// (`simplify_path_keeping_reversals`), so a turn that runs on the line of
/// the arc it joins can lose its end: no move then starts and ends on the
/// turn. The move here is the one whose segment is nearest to both turn
/// ends (the smaller of the two largest distances); both distances under
/// `MIN_EMITTED_SEGMENT_MM` show the drop.
#[derive(Debug, Clone, Copy)]
pub struct HoldingMove {
    pub from: P2,
    pub to: P2,
    /// The distances (mm) of the turn's start and end from the move's
    /// segment.
    pub off_from: f64,
    pub off_to: f64,
}

/// C20: the [`HoldingMove`] of the turn `from` → `to` in `tp`, or `None`
/// when `tp` has no `Linear` move with a start.
fn holding_move(tp: &Toolpath, from: P2, to: P2) -> Option<HoldingMove> {
    let mut best: Option<HoldingMove> = None;
    let mut start: Option<P2> = None;
    for m in &tp.moves {
        let target = P2::new(m.target.x, m.target.y);
        if let (Some(a), crate::toolpath::MoveType::Linear { .. }) = (start, &m.move_type)
            && (target.x - a.x).hypot(target.y - a.y) > 1e-6
        {
            let held = HoldingMove {
                from: a,
                to: target,
                off_from: point_to_segment_distance(&from, &a, &target),
                off_to: point_to_segment_distance(&to, &a, &target),
            };
            if best.is_none_or(|b| held.off_from.max(held.off_to) < b.off_from.max(b.off_to)) {
                best = Some(held);
            }
        }
        start = Some(target);
    }
    best
}

/// C14: the feed factor of a clearing turn whose measured engagement is
/// `swept` (swept width over D) and `area` (area per mm over D), against
/// the pass ceiling `ceiling` (a_e / D).
///
/// The relation: hold the material removal rate of the steady cut at the
/// ceiling. The removal rate of a move is `a_p × (area per mm) × F`, and on
/// the steady side cut the area per mm is the swept width (C7, Cavalieri),
/// at most `c·D`. So a move that clears `a` mm² per mm holds the steady
/// removal rate at `F' = F · c·D / a`. This is the shear term of the power
/// model (`tool_load/power.rs`, `P = A·(Ks·MRR + F_edge·a_p·V_c·z·ψ/2π)`;
/// `dressup/feed_modulation.rs`: "only the shear term moves with feed").
/// The edge term does not move with feed, so no feed holds it; it is
/// reported, not hidden. The larger of the two readings is taken (the
/// swept width reads the span of a head-on cap, the area its removal; the
/// lattice under-reads a thin cap, C7).
///
/// The same feed also holds the peak lateral force (`feeds/force.rs`,
/// `F = a_p · (Ks · f_z · sin θ_peak + F_edge)`, `θ_peak = min(ψ, π/2)`,
/// `cos ψ = 1 − 2·a_e/D`) at or under the steady cut's: that force needs only
/// `F' = F · sin θ_peak(c) / sin θ_peak(w)`, and `c/w` is the smaller factor
/// for every `w > c` (`c/w < 1 < (1 − c)/(1 − w)`, so
/// `(c/w)² < c(1 − c)/(w(1 − w))`; past `w = 1/2` the force ratio is
/// `sin θ_peak(c)`, still above `c/w`). Chip thinning is not used: the
/// operator ruled it out (G-CHIPTHIN-HALFFIX, `dressup/feed_modulation.rs`).
/// No constant enters.
#[must_use]
pub fn turn_feed_factor(ceiling: f64, swept: f64, area: f64) -> f64 {
    let engaged = swept.max(area);
    if engaged > ceiling && engaged > 0.0 {
        ceiling / engaged
    } else {
        1.0
    }
}

/// C14: the turn records of one piece from its emitted readout `load`
/// (whose turn indices point into `turns`), and the reduced feed written
/// on the moves of `tp` that lie on each turn (a `Linear` move whose start
/// and target are on the turn's segment within 1e-6 mm, the walker's
/// same-point tolerance). Adds them to `summary`. With a corner radius
/// (`min_cutting_radius > 0`) the blend arcs at a turn's two corners keep
/// the full feed; the 2D default is 0 (`AdaptiveConfig::default`), the value
/// the harness uses.
///
/// C20: a turn with no move on it is unfed only when it needs a reduced
/// feed (factor under 1). A turn whose measured engagement is within the
/// ceiling has the factor 1 ([`turn_feed_factor`]), so its feed is the full
/// feed `F`. Every clearing move is emitted at `F` and this function only
/// lowers feeds, so every move that holds the turn carries at most `F`:
/// the turn holds its feed by construction. It is counted in
/// [`TurnSummary::full_feed_unmatched`]. No constant enters. The move that
/// holds an unmatched turn is kept as evidence ([`HoldingMove`]).
fn apply_turn_feeds(
    tp: &mut Toolpath,
    load: &PassLoadReadout,
    turns: &[(P2, P2)],
    feed_rate: f64,
    summary: &mut TurnSummary,
) {
    summary.planned += turns.len();
    let on = |p: P2, q: P2, x: P2| point_to_segment_distance(&x, &p, &q) < 1e-6;
    for (k, &(from, to)) in turns.iter().enumerate() {
        let steps: Vec<&TurnStep> = load.turn_steps.iter().filter(|t| t.turn == k).collect();
        if steps.is_empty() {
            continue;
        }
        let max_swept = steps.iter().map(|t| t.swept).fold(0.0_f64, f64::max);
        let max_area = steps.iter().map(|t| t.area).fold(0.0_f64, f64::max);
        let factor = turn_feed_factor(load.ceiling, max_swept, max_area);
        let feed = feed_rate * factor;
        let mut fed = false;
        let mut start: Option<P2> = None;
        for m in &mut tp.moves {
            let target = P2::new(m.target.x, m.target.y);
            if let (Some(a), crate::toolpath::MoveType::Linear { feed_rate: f }) =
                (start, &mut m.move_type)
                && (target.x - a.x).hypot(target.y - a.y) > 1e-6
                && on(from, to, a)
                && on(from, to, target)
            {
                *f = f.min(feed);
                fed = true;
            }
            start = Some(target);
        }
        if fed {
            summary.fed += 1;
        } else if factor < 1.0 {
            summary.unfed += 1;
        } else {
            summary.full_feed_unmatched += 1;
        }
        summary.turns.push(TurnRecord {
            from,
            to,
            length_mm: (to.x - from.x).hypot(to.y - from.y),
            steps: steps.len(),
            steps_over: steps
                .iter()
                .filter(|t| t.swept > load.ceiling || t.area > load.ceiling)
                .count(),
            max_swept,
            max_area,
            feed_factor: factor,
            feed_mm_min: feed,
            fed,
            holding: if fed {
                None
            } else {
                holding_move(tp, from, to)
            },
            level_z: None,
        });
    }
}

/// Where the entries (`Rapid`s) of a cleaned plan come from (C3). The
/// producer is the kind of the last marker before the entry.
#[derive(Debug, Clone, Default)]
pub struct EntrySplit {
    /// The starter pocket's plunge.
    pub starter: usize,
    /// A clearing re-entry within one pass step of a planned wrap end (the
    /// replay re-enters at a walk point, and walk points are at most one
    /// pass step apart): a join the replay refused (a seam step, an
    /// arc-to-arc step, a link, a ramp start).
    pub clearing_join: usize,
    /// Any other clearing re-entry: the replay split a wrap mid-way. For a
    /// shipped strategy (no wraps) every clearing re-entry counts here,
    /// the agent's frontier hops included.
    pub clearing_split: usize,
    /// A residue contour's re-entry.
    pub residue_contour: usize,
    /// A wall pass's re-entry.
    pub wall_pass: usize,
    /// A residue mop's re-entry.
    pub residue_mop: usize,
    /// Slot lines or no marker.
    pub other: usize,
}

impl EntrySplit {
    fn merge(&mut self, other: &Self) {
        self.starter += other.starter;
        self.clearing_join += other.clearing_join;
        self.clearing_split += other.clearing_split;
        self.residue_contour += other.residue_contour;
        self.wall_pass += other.wall_pass;
        self.residue_mop += other.residue_mop;
        self.other += other.other;
    }
}

/// The [`EntrySplit`] of a cleaned plan whose producer planned `wraps`; a
/// re-entry within `step` of a wrap end is a join.
fn entry_split(segments: &[AdaptiveSegment], wraps: &[Vec<P2>], step: f64) -> EntrySplit {
    let ends: Vec<P2> = wraps
        .iter()
        .flat_map(|w| w.first().into_iter().chain(w.last()))
        .copied()
        .collect();
    let mut out = EntrySplit::default();
    let mut producer = Producer::Unmarked;
    for seg in segments {
        match seg {
            AdaptiveSegment::Marker(event) => producer = producer.after(event),
            AdaptiveSegment::Rapid(p) => match producer {
                Producer::Starter => out.starter += 1,
                Producer::Clearing => {
                    let near = |q: &P2| (q.x - p.x).hypot(q.y - p.y) <= step;
                    if ends.iter().any(near) {
                        out.clearing_join += 1;
                    } else {
                        out.clearing_split += 1;
                    }
                }
                Producer::ResidueContour => out.residue_contour += 1,
                Producer::WallPass => out.wall_pass += 1,
                Producer::ResidueMop => out.residue_mop += 1,
                Producer::Unmarked | Producer::SlotLines => out.other += 1,
            },
            AdaptiveSegment::Cut(_) | AdaptiveSegment::Link(_) => {}
        }
    }
    out
}

/// One research run over every inset piece of an outline.
pub struct Run2d {
    /// The emitted toolpath of every piece, in order.
    pub toolpath: Toolpath,
    pub summary: Summary2d,
    /// Candidate (a) only: one report per piece.
    pub fields: Vec<FieldReport>,
    /// Candidate (a) only: the wraps in cutting order, oriented.
    pub wraps: Vec<Vec<P2>>,
    /// The cleaned plan of every piece as drawn spans (planner output,
    /// before the toolpath emission), in order.
    pub spans: Vec<Span2d>,
}

/// The cleanup of `adaptive_toolpath_structured_annotated_traced_with_cancel`
/// (`mod.rs`): the same dispatch on the cleanup strategy.
/// C14: the replay of the kept segments admits the planned clearing turns
/// `turns` (empty for every column before C14).
fn cleaned(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    segments: Vec<AdaptiveSegment>,
    measure: ReplayMeasure,
    turns: &[(P2, P2)],
) -> Vec<AdaptiveSegment> {
    let m = measure.crate_measure();
    match params.cleanup_strategy {
        CleanupStrategy::Legacy => segments,
        CleanupStrategy::ResidueMop | CleanupStrategy::ContourParallelNarrow => {
            apply_residue_mop_cleanup_turns(polygon, params, &segments, m, turns, false)
        }
        CleanupStrategy::ContourParallelHybrid => apply_contour_parallel_residue_cleanup_turns(
            polygon, params, &segments, m, turns, false,
        ),
    }
}

/// Plan `outline` with `candidate` and the shared `params`, piece by piece
/// as the 2D adapter does (`inset_piece_outlines`). A shipped strategy runs
/// the product pipeline (`adaptive_segments_with_debug`, the cleanup, the
/// toolpath emission). Candidate (a) runs its producer and then the same
/// cleanup and emission, with `PathStrategy2d::Agent` in the params so the
/// contour spiral's default corner blend does not apply. A piece where the
/// field does not apply is not planned; its [`FieldReport`] says so.
#[must_use]
pub fn run_2d(outline: &Polygon2, params: &AdaptiveParams, candidate: Candidate2d) -> Run2d {
    run_2d_with(
        outline,
        params,
        candidate,
        ReadResolution::Planner,
        ReplayMeasure::SweptWidth,
    )
}

/// The lattice the research readouts of the RAW plan read on (C6, harness
/// only). The producer, the shipped cleanup and its replay, and the
/// readout of the emitted plan stay on the planner cell
/// (`max(R/6, tolerance)`, `path.rs` `planner_cell_size`): the shipped
/// cleanup derives its own cell, so a finer replay would need a seam in
/// `path.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadResolution {
    /// The planner cell.
    Planner,
    /// Half the planner cell: the same plan read at twice the lattice
    /// resolution, to tell the measure's one-cell quantum from stock.
    HalfCell,
}

/// [`run_2d`] with the raw-plan readouts (`Summary2d::raw_pass_load`,
/// `Summary2d::cascade`) read at `read` ([`ReadResolution`]), and the
/// cleanup's replay held to `measure` ([`ReplayMeasure`], C7). The
/// readouts read both measures; their over-steps and the cascade model are
/// in `measure`. The producer is the same whatever `measure`, so a column
/// under [`ReplayMeasure::AreaPerMm`] judges the same plan by area.
#[must_use]
pub fn run_2d_with(
    outline: &Polygon2,
    params: &AdaptiveParams,
    candidate: Candidate2d,
    read: ReadResolution,
    measure: ReplayMeasure,
) -> Run2d {
    let never: &dyn CancelCheck = &|| false;
    let strategy = match candidate {
        Candidate2d::Shipped(s) => s,
        Candidate2d::Geodesic => PathStrategy2d::Agent,
    };
    let params = params_with(params, strategy);
    let pieces = super::inset_piece_outlines(outline, params.tool_radius);
    let pieces = if pieces.is_empty() {
        vec![outline.clone()]
    } else {
        pieces
    };
    let mut run = Run2d {
        toolpath: Toolpath::new(),
        summary: Summary2d {
            pieces: pieces.len(),
            ..Summary2d::default()
        },
        fields: Vec::new(),
        wraps: Vec::new(),
        spans: Vec::new(),
    };
    for piece in &pieces {
        let mut piece_wraps: Vec<Vec<P2>> = Vec::new();
        let mut piece_parts: Option<PartIndex> = None;
        let mut piece_seed: Option<P2> = None;
        let mut piece_turns: Vec<(P2, P2)> = Vec::new();
        let raw = match candidate {
            Candidate2d::Shipped(_) => {
                super::adaptive_segments_with_debug(piece, &params, never, None, None).ok()
            }
            Candidate2d::Geodesic => match research_plan_piece(piece, &params, never) {
                Ok(Some(plan)) => {
                    let ResearchPlan {
                        segments,
                        turns,
                        wraps,
                        spiral,
                        seed,
                        spiral_levels,
                        levels,
                    } = plan;
                    let mut parts = PartIndex::new(planner_cell(&params));
                    parts.add(&spiral, StepPart::Spiral);
                    for w in &wraps {
                        parts.add(w, StepPart::Wrap);
                    }
                    parts.turns.clone_from(&turns);
                    run.fields.push(FieldReport {
                        applied: true,
                        seed: Some(seed),
                        seed_radius: params.tool_radius,
                        levels,
                        chains: wraps.len(),
                        spiral_levels,
                        turns: turns.len(),
                    });
                    piece_parts = Some(parts);
                    piece_turns = turns;
                    piece_seed = Some(seed);
                    piece_wraps.clone_from(&wraps);
                    run.wraps.extend(wraps);
                    Some(segments)
                }
                Ok(None) | Err(Cancelled) => {
                    run.fields.push(FieldReport::default());
                    None
                }
            },
        };
        let Some(raw) = raw else {
            continue;
        };
        let cell = planner_cell(&params);
        let read_cell = match read {
            ReadResolution::Planner => cell,
            ReadResolution::HalfCell => cell / 2.0,
        };
        let raw_load = pass_load_readout(
            piece,
            &params,
            &raw,
            piece_parts.as_ref(),
            read_cell,
            measure,
        );
        run.summary.raw_pass_load.merge(&raw_load);
        run.summary.cascade.merge(&cascade_readout(
            piece,
            &params,
            &raw,
            piece_parts.as_ref(),
            piece_seed,
            read_cell,
            measure,
        ));
        let segments = cleaned(piece, &params, raw, measure, &piece_turns);
        run.summary
            .entry_split
            .merge(&entry_split(&segments, &piece_wraps, pass_step_len(cell)));
        let region = (params.keep_down_links == KeepDownLinks::WithinPassLoad)
            .then(|| ToolCentreRegion::new(piece, params.tool_radius));
        let (mut tp, _) = segments_to_toolpath(&segments, &params, region.as_ref());
        add_counts(&mut run.summary, &segments);
        run.spans.extend(spans_of(&segments));
        // C14: the emitted readout classes the planned turns only, and each
        // emitted turn's moves get the feed its measured engagement needs.
        let turn_index = (!piece_turns.is_empty()).then(|| PartIndex::of_turns(&piece_turns, cell));
        let emitted = pass_load_readout(
            piece,
            &params,
            &segments,
            turn_index.as_ref(),
            cell,
            measure,
        );
        if !piece_turns.is_empty() {
            apply_turn_feeds(
                &mut tp,
                &emitted,
                &piece_turns,
                params.feed_rate,
                &mut run.summary.turns,
            );
        }
        run.summary.pass_load.merge(&emitted);
        run.summary.retracts += tp
            .moves
            .iter()
            .filter(|m| m.intent == MoveIntent::Retract)
            .count();
        run.toolpath.moves.extend(tp.moves);
    }
    run
}

fn add_counts(summary: &mut Summary2d, segments: &[AdaptiveSegment]) {
    let mut here: Option<P2> = None;
    for seg in segments {
        match seg {
            AdaptiveSegment::Cut(path) => {
                summary.cut_length_mm += path
                    .windows(2)
                    .filter_map(|w| match w {
                        &[a, b] => Some((b.x - a.x).hypot(b.y - a.y)),
                        _ => None,
                    })
                    .sum::<f64>();
                if let Some(&end) = path.last() {
                    here = Some(end);
                }
            }
            AdaptiveSegment::Link(p) => {
                summary.keep_down_links += 1;
                if let Some(from) = here {
                    summary.link_length_mm += (p.x - from.x).hypot(p.y - from.y);
                }
                here = Some(*p);
            }
            AdaptiveSegment::Rapid(p) => {
                summary.entries += 1;
                here = Some(*p);
            }
            AdaptiveSegment::Marker(_) => {}
        }
    }
}

#[cfg(test)]
#[expect(
    clippy::panic,
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;
    use crate::adaptive::path::{planner_cell_size, turn_bound};
    use std::f64::consts::{PI, TAU};

    /// The 6 mm end mill of the 2D fixtures (`test_data/ux_2d_*.toml`).
    const R: f64 = 3.0;

    /// The 2D Adaptive defaults (`AdaptiveConfig::default`), with the
    /// stepover the fixture states (P10: `fixture_values`).
    fn op_defaults() -> (f64, f64) {
        let cfg = crate::compute::operation_configs::AdaptiveConfig::default();
        let stepover = crate::compute::fixture_values::fixture_geometry(
            crate::compute::catalog::OperationType::Adaptive,
        )
        .iter()
        .find_map(|&(field, v)| (field == "stepover").then_some(v))
        .unwrap_or(f64::NAN);
        (stepover, cfg.tolerance)
    }

    /// A circle of radius `radius` about `c`, with enough sides that a chord
    /// lies within `sag` of the arc: `n = ⌈π / acos(1 - sag / radius)⌉`.
    fn circle(c: P2, radius: f64, sag: f64) -> Vec<P2> {
        let n = (PI / (1.0 - sag / radius).acos()).ceil() as usize;
        (0..n)
            .map(|i| {
                let a = TAU * i as f64 / n as f64;
                P2::new(c.x + radius * a.cos(), c.y + radius * a.sin())
            })
            .collect()
    }

    /// C6: the transition join on a disc large enough to pass `ρ*`. The rim
    /// and every closed wrap drift out to the next level (one rim
    /// transition, the rest closed to closed, all traced inside the
    /// region); inside `ρ*` the fraction is 1 (the whole wrap, the spiral
    /// form); it is at most 1 everywhere and under 1 somewhere past `ρ*`
    /// (the margin branch runs); and the sectors nest: each sector angle is
    /// at most the one before, to the angle of one cell at that radius (the
    /// contour gives a wrap's length to about one cell).
    /// C7: a material grid over the square `[0, side]²` on the planner
    /// lattice, with the stock standing only where `stands(x, y)`.
    fn stock_grid(side: f64, cell: f64, stands: impl Fn(f64, f64) -> bool) -> MaterialGrid {
        let square = Polygon2::new(vec![
            P2::new(0.0, 0.0),
            P2::new(side, 0.0),
            P2::new(side, side),
            P2::new(0.0, side),
        ]);
        let mut grid = MaterialGrid::from_polygon(&square, cell);
        let (ox, oy, cols) = (grid.origin_x, grid.origin_y, grid.cols);
        for (i, v) in grid.cells.iter_mut().enumerate() {
            let x = ox + (i % cols) as f64 * cell;
            let y = oy + (i / cols) as f64 * cell;
            if *v == super::super::material_grid::CELL_MATERIAL && !stands(x, y) {
                *v = super::super::material_grid::CELL_CLEARED;
            }
        }
        grid
    }

    /// The two readings (mm) of the step `a -> b` on `grid`: the swept
    /// width at `b` and the area per mm.
    fn readings(grid: &MaterialGrid, a: P2, b: P2) -> (f64, f64) {
        let angle = (b.y - a.y).atan2(b.x - a.x);
        let swept = super::super::search::compute_swept_width(grid, b.x, b.y, R, angle) * 2.0 * R;
        let area = super::super::search::compute_area_per_mm(grid, a, b.x, b.y, R);
        (swept, area)
    }

    /// C7: on a straight side cut of width `s` the two measures agree within
    /// one cell, and each is within one cell of `s` (the area is the band's
    /// rows times the step, Cavalieri; the swept width reads the band up to
    /// one cell narrow). This is why `c·D` carries over as the area ceiling.
    #[test]
    fn a_straight_side_cut_reads_alike_in_both_measures() {
        let (s, tol) = op_defaults();
        let cell = planner_cell_size(R, tol);
        // The pass before at y = 10 cleared to y = 10 + R; the pass now runs
        // at y = 10 + s, along +x, one pass step at a time.
        let mut grid = stock_grid(45.0, cell, |_, y| y > 10.0 + R);
        let y = 10.0 + s;
        grid.clear_segment(P2::new(5.0, y), P2::new(20.0, y), R);
        let step = pass_step_len(cell);
        let (swept, area) = readings(&grid, P2::new(20.0, y), P2::new(20.0 + step, y));
        assert!(
            (swept - area).abs() <= cell + 1e-9,
            "the measures agree within one cell: swept {swept:.3}, area {area:.3}"
        );
        for (name, v) in [("swept", swept), ("area", area)] {
            assert!(
                (v - s).abs() <= cell + 1e-9,
                "{name} {v:.3} reads the side cut {s} within one cell {cell}"
            );
        }
    }

    /// C7: the wall-end turn. A pass at level k reaches the wall (the
    /// tool-centre wall at x = 37 of a 40 mm wide pocket) and turns to step
    /// up the wall by `s` to the next level, in two pass steps, head-on
    /// into the stock above `y_k + R`. The swept width of the second step
    /// reads at least `2s`, AND its area per mm reads over `c·D` too, as
    /// does the mean area per mm of the whole advance.
    ///
    /// This is the opposite of the claim the C7 brief asked to pin (area per
    /// mm ≤ c·D at the turn). The exact geometry agrees with the lattice: a
    /// step of `h` head-on into stock clears the cap of height `h` of the
    /// cutter's disc, `R² acos((R - h)/R) - (R - h)√(2Rh - h²)`: 3.10 mm² for
    /// the first millimetre, 5.15 mm² for the second (R 3), against
    /// `c·D` = 2.18 mm² per mm. (The lattice reads the first step's thin cap
    /// at 2.0 mm²; that step is not asserted.)
    #[test]
    fn a_head_on_step_up_a_wall_is_over_the_ceiling_in_both_measures() {
        let params = test_params();
        let (s, tol) = op_defaults();
        let cell = planner_cell_size(R, tol);
        let cd = pass_load_for(&params, cell).ceiling * 2.0 * R;
        let y_k = 15.0;
        let wall_x = 40.0 - R;
        let mut grid = stock_grid(40.0, cell, |_, y| y > y_k + R);
        let half = s / 2.0;
        let p0 = P2::new(wall_x, y_k);
        let p1 = P2::new(wall_x, y_k + half);
        let p2 = P2::new(wall_x, y_k + s);
        let (_, area_1) = readings(&grid, p0, p1);
        grid.clear_segment(p0, p1, R);
        let (swept_2, area_2) = readings(&grid, p1, p2);
        assert!(
            swept_2 >= 2.0 * s,
            "the swept width {swept_2:.3} reads at least 2s"
        );
        assert!(
            area_2 > cd,
            "the second step's area per mm {area_2:.3} is over c·D {cd:.4}"
        );
        let mean = (area_1 * half + area_2 * half) / s;
        assert!(
            mean > cd,
            "the advance's mean area per mm {mean:.3} is over c·D {cd:.4}"
        );
    }

    /// C7: where the two measures part. The stock is two slivers either side
    /// of the cutter's path (one lattice column each, 2.5 mm off the path):
    /// the swept width reads their whole span (at least `2s`), the area per
    /// mm reads only what the step removes (under `c·D`).
    #[test]
    fn two_slivers_read_wide_in_swept_width_and_narrow_in_area() {
        let params = test_params();
        let (s, tol) = op_defaults();
        let cell = planner_cell_size(R, tol);
        let cd = pass_load_for(&params, cell).ceiling * 2.0 * R;
        let x0 = 20.0;
        let grid = stock_grid(45.0, cell, |x, _| {
            let d = (x - x0).abs();
            d >= 2.5 - cell / 2.0 && d <= 2.5 + cell / 2.0
        });
        let step = pass_step_len(cell);
        let (swept, area) = readings(&grid, P2::new(x0, 20.0), P2::new(x0, 20.0 + step));
        assert!(
            swept >= 2.0 * s,
            "the swept width {swept:.3} spans both slivers"
        );
        assert!(area <= cd, "the area per mm {area:.3} is under c·D {cd:.4}");
    }

    /// The 2D Adaptive params of the unit tests: the op defaults, the 6 mm
    /// end mill, the default helix, the 2D pass-load rule.
    fn test_params() -> AdaptiveParams {
        let (s, tol) = op_defaults();
        let helix = crate::compute::config::HELIX_RADIUS_OVER_D * 2.0 * R;
        AdaptiveParams {
            tool_radius: R,
            stepover: s,
            cut_depth: -1.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 5.0,
            tolerance: tol,
            slot_clearing: false,
            min_cutting_radius: 0.0,
            standing_material: None,
            cleanup_strategy: CleanupStrategy::ContourParallelHybrid,
            engagement_measure: super::super::EngagementMeasure::DiskArea,
            path_strategy: PathStrategy2d::Agent,
            trochoid_cap_mult: 1.2,
            keep_down_links: KeepDownLinks::WithinPassLoad,
            entry_helix_radius: helix,
        }
    }

    /// The 60 x 40 mm pocket with a 6 mm round island of the unit tests.
    fn island_pocket() -> Polygon2 {
        let (_, tol) = op_defaults();
        let cell = planner_cell_size(R, tol);
        let mut hole = circle(P2::new(25.0, 20.0), 6.0, cell);
        hole.reverse();
        Polygon2::with_holes(
            vec![
                P2::new(0.0, 0.0),
                P2::new(60.0, 0.0),
                P2::new(60.0, 40.0),
                P2::new(0.0, 40.0),
            ],
            vec![hole],
        )
    }

    /// The outline of `fixtures/demo_star.svg` (the G1 star), as drawn: it is
    /// mirror-symmetric about x = 50 and NOT a regular star (its tips lie
    /// 45.5, 47.6, 49.8, 49.8 and 47.6 mm from (50, 50.5)).
    fn demo_star() -> Polygon2 {
        let pts = [
            (50.0, 5.0),
            (61.0, 35.0),
            (95.0, 35.0),
            (68.0, 57.0),
            (79.0, 91.0),
            (50.0, 70.0),
            (21.0, 91.0),
            (32.0, 57.0),
            (5.0, 35.0),
            (39.0, 35.0),
        ];
        let mut ring: Vec<P2> = pts.iter().map(|&(x, y)| P2::new(x, y)).collect();
        // Counter-clockwise exterior, the polygon convention.
        let twice_area: f64 = ring
            .iter()
            .zip(ring.iter().cycle().skip(1))
            .map(|(a, b)| a.x * b.y - b.x * a.y)
            .sum();
        if twice_area < 0.0 {
            ring.reverse();
        }
        Polygon2::new(ring)
    }

    /// Every step candidate (a) emits, after the shared cleanup, holds the
    /// pass load: the readout replays the plan and asks
    /// `step_within_pass_load` of each step.
    #[test]
    #[ignore = "G-MOPLOAD: one shipped ResidueMop step reads 0.4524 over the 0.3626 ceiling \
                (D11, 2026-10-24; 0.3669 at D2, the drift is between D2 and ea468814); the \
                candidate's own steps hold. Un-ignore when the mop holds the pass load \
                (DEFECTS.md)"]
    fn every_emitted_step_holds_the_pass_load() {
        let params = test_params();
        let run = run_2d(&island_pocket(), &params, Candidate2d::Geodesic);
        assert!(
            run.fields.iter().any(|f| f.applied && f.chains > 0),
            "the field must apply and cut wraps: {:?}",
            run.fields
        );
        let load = &run.summary.pass_load;
        assert!(load.steps > 100, "the readout must read the plan: {load:?}");
        assert_eq!(
            load.steps_over, 0,
            "every step holds the pass load: {load:?}"
        );
        assert!(load.max_reading <= load.ceiling, "{load:?}");
    }

    /// C20: a turn on the line of the arc it joins, whose end vertex the
    /// emitter dropped (D12: the 0.048 mm turn on the pocket). The move
    /// that holds it runs from the turn's start past its end.
    /// - Within the ceiling (factor 1): not unfed; it carries the full feed.
    /// - Over the ceiling (factor under 1): unfed, as before.
    /// - Either way the readout names the holding move, with both turn ends
    ///   on its segment.
    /// - With the vertex kept, the turn's move carries the reduced feed.
    #[test]
    fn a_turn_whose_vertex_the_emitter_drops_is_unfed_only_when_it_needs_a_cut_feed() {
        let params = test_params();
        let (_, tol) = op_defaults();
        let cell = planner_cell_size(R, tol);
        let c = pass_load_for(&params, cell).ceiling;
        let f = params.feed_rate;
        let z = params.cut_depth;
        let (a, b, end) = (P2::new(1.0, 0.0), P2::new(1.048, 0.0), P2::new(2.0, 0.0));
        let path = |pts: &[P2]| {
            let mut tp = Toolpath::new();
            for p in pts {
                tp.feed_to(crate::geo::P3::new(p.x, p.y, z), f);
            }
            tp
        };
        let load = |swept: f64, area: f64| PassLoadReadout {
            ceiling: c,
            turn_steps: vec![TurnStep {
                turn: 0,
                from: a,
                to: b,
                swept,
                area,
            }],
            ..PassLoadReadout::default()
        };
        // Swept 0 and one cell of area, as D12 read it.
        let quiet = load(0.0, cell / (2.0 * R));
        let engaged = load(2.0 * c, 2.0 * c);

        let mut ts = TurnSummary::default();
        let mut tp = path(&[P2::new(0.0, 0.0), a, end]);
        apply_turn_feeds(&mut tp, &quiet, &[(a, b)], f, &mut ts);
        assert_eq!(
            (ts.fed, ts.unfed, ts.full_feed_unmatched),
            (0, 0, 1),
            "{ts:?}"
        );
        let r = ts.turns.first().expect("one turn record");
        assert!(
            !r.fed && (r.feed_factor - 1.0).abs() < f64::EPSILON,
            "{r:?}"
        );
        let h = r.holding.expect("a move holds the turn");
        assert!(
            (h.from.x - a.x).abs() < 1e-12 && (h.to.x - end.x).abs() < 1e-12,
            "{h:?}"
        );
        assert!(
            h.off_from.max(h.off_to) < crate::toolpath::MIN_EMITTED_SEGMENT_MM,
            "{h:?}"
        );

        let mut ts = TurnSummary::default();
        let mut tp = path(&[P2::new(0.0, 0.0), a, end]);
        apply_turn_feeds(&mut tp, &engaged, &[(a, b)], f, &mut ts);
        assert_eq!(
            (ts.fed, ts.unfed, ts.full_feed_unmatched),
            (0, 1, 0),
            "{ts:?}"
        );
        assert!(ts.turns.first().and_then(|r| r.holding).is_some());

        let mut ts = TurnSummary::default();
        let mut tp = path(&[P2::new(0.0, 0.0), a, b, end]);
        apply_turn_feeds(&mut tp, &engaged, &[(a, b)], f, &mut ts);
        assert_eq!(
            (ts.fed, ts.unfed, ts.full_feed_unmatched),
            (1, 0, 0),
            "{ts:?}"
        );
        let r = ts.turns.first().expect("one turn record");
        assert!(r.holding.is_none() && r.feed_factor < 1.0, "{r:?}");
        let fed: Vec<f64> = tp
            .moves
            .iter()
            .filter_map(|m| match m.move_type {
                crate::toolpath::MoveType::Linear { feed_rate } => Some(feed_rate),
                crate::toolpath::MoveType::Rapid
                | crate::toolpath::MoveType::ArcCW { .. }
                | crate::toolpath::MoveType::ArcCCW { .. } => None,
            })
            .collect();
        // The moves: to the origin, to the turn's start, the turn, on.
        let &[_, _, turn, after] = fed.as_slice() else {
            panic!("four moves: {fed:?}")
        };
        assert!((turn - f * r.feed_factor).abs() < 1e-9, "{fed:?}");
        assert!((after - f).abs() < f64::EPSILON, "{fed:?}");
    }

    /// C14: the reduced feed of a clearing turn ([`turn_feed_factor`]), for
    /// engagements from the ceiling to a full slot:
    /// - the removal rate is the steady cut's: `factor · w = c`;
    /// - the peak lateral force of `feeds/force.rs` is at or under the
    ///   steady cut's: `factor · sin θ_peak(w) ≤ sin θ_peak(c)`;
    /// - the larger of the two readings decides, and a turn within the
    ///   ceiling keeps the full feed.
    #[test]
    fn the_turn_feed_holds_the_steady_removal_and_the_lateral_force() {
        let params = test_params();
        let (s, tol) = op_defaults();
        let c = pass_load_for(&params, planner_cell_size(R, tol)).ceiling;
        assert!(c > 0.0 && c < 0.5, "the ceiling {c} at s {s}");
        let sin_peak = |w: f64| {
            crate::feeds::force::immersion_angle(w * 2.0 * R, R)
                .min(std::f64::consts::FRAC_PI_2)
                .sin()
        };
        for j in 1..=20 {
            let w = c + f64::from(j) * (1.0 - c) / 20.0;
            let factor = turn_feed_factor(c, w, 0.0);
            assert!(factor > 0.0 && factor < 1.0, "w {w}: factor {factor}");
            assert!(
                (factor * w - c).abs() < 1e-12,
                "w {w}: removal {}",
                factor * w
            );
            assert!(
                factor * sin_peak(w) <= sin_peak(c) + 1e-12,
                "w {w}: force ratio {}",
                factor * sin_peak(w) / sin_peak(c)
            );
            assert!((turn_feed_factor(c, 0.0, w) - factor).abs() < 1e-12);
            assert!((turn_feed_factor(c, w, 0.5 * (c + w)) - factor).abs() < 1e-12);
        }
        assert!((turn_feed_factor(c, 0.9 * c, 0.5 * c) - 1.0).abs() < f64::EPSILON);
    }

    /// C14 on the product plan (the zigzag arc links) of the island pocket.
    /// - Turns are planned, and every planned turn is no longer than the
    ///   bound (`turn_bound`).
    /// - Every emitted turn carries the reduced feed of its measured
    ///   engagement on its toolpath moves.
    /// - No clearing step that is not a turn passes the ceiling (the cuts
    ///   and the keep-down links).
    #[test]
    fn a_clearing_turn_is_bounded_and_fed_down_and_no_other_step_passes() {
        let params = test_params();
        let (s, tol) = op_defaults();
        let bound = turn_bound(s, planner_cell_size(R, tol));
        let run = run_2d(&island_pocket(), &params, Candidate2d::Geodesic);
        assert!(
            run.fields.iter().map(|f| f.turns).sum::<usize>() > 0,
            "the pocket's arcs end at walls, so turns are planned: {:?}",
            run.fields
        );
        let ts = &run.summary.turns;
        assert!(!ts.turns.is_empty(), "{ts:?}");
        assert_eq!(ts.unfed, 0, "every turn carries its feed");
        let load = &run.summary.pass_load;
        for r in &ts.turns {
            assert!(r.length_mm <= bound, "{r:?}");
            assert!(r.feed_factor > 0.0 && r.feed_factor <= 1.0, "{r:?}");
            assert!((r.feed_mm_min - params.feed_rate * r.feed_factor).abs() < 1e-9);
            if r.feed_factor < 1.0 {
                let engaged = r.max_swept.max(r.max_area);
                assert!(
                    (r.feed_factor * engaged - load.ceiling).abs() < 1e-9,
                    "{r:?}"
                );
            }
        }
        for t in load
            .tallies
            .iter()
            .filter(|t| t.label == "clearing" || t.label == "clearing link")
        {
            assert_eq!(t.steps_over, 0, "{t:?}");
            assert!(t.max_reading <= load.ceiling, "{t:?}");
        }
    }

    /// G1-1: every wrap vertex, and every segment between two vertices of a
    /// wrap, lies inside the tool-centre region (`ToolCentreRegion`, the
    /// planner's exact test). The contour can cross a concave stretch of the
    /// region edge inside one cell; `inside_pieces` cuts the wrap there. The
    /// test holds no tolerance: the planner holds none either.
    #[test]
    fn every_wrap_lies_inside_the_tool_centre_region() {
        let params = test_params();
        for (name, outline) in [
            ("island pocket", island_pocket()),
            ("demo star", demo_star()),
        ] {
            let run = run_2d(&outline, &params, Candidate2d::Geodesic);
            let region = ToolCentreRegion::new(&outline, params.tool_radius);
            assert!(!run.wraps.is_empty(), "{name}: candidate (a) cuts wraps");
            for wrap in &run.wraps {
                for p in wrap {
                    assert!(region.contains(p), "{name}: wrap vertex {p:?} is outside");
                }
                for pair in wrap.windows(2) {
                    let &[a, b] = pair else {
                        continue;
                    };
                    assert!(
                        region.contains_segment(a, b),
                        "{name}: wrap segment {a:?} -> {b:?} leaves the region"
                    );
                }
            }
        }
    }
}
