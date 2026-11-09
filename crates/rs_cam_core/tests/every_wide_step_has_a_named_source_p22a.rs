//! P22a (feeds contract, `planning/feeds_contract_2026-11-03/phases/P22_DESIGN.md`
//! section 6): a read-only readout. Every fed sample of a Geodesic Adaptive
//! 3D Rough whose measured width is over the pass ceiling `c D` gets a
//! source, read through public doors only.
//!
//! - The width is the power gate's width: `Engagement::from_sample`
//!   (`r (1 - cos psi)` at the measured arc).
//! - The ceiling is the planner's own: `c = radial_woc_fraction_from_leading_arc(
//!   pass_engagement_limit(s, R))` (the 3D stock-read split reads the same
//!   value, `adaptive3d/clearing.rs`). `pass_engagement_limit` holds
//!   `PASS_ENGAGEMENT_TOLERANCE`.
//! - The source is read from the emitted moves (the entry intents), the
//!   span ancestry of the sample, and (P22a-2) the `FeedCap` items of the
//!   semantic trace: each names the planner rule of a capped run
//!   (`adaptive3d::FeedCapSource`). The inferred cap classes of P22a stay as
//!   a cross-check of the exact ones (the agreement table). The perimeter
//!   sweep has no marker and stays inferred.
//!
//! Arm 1 (runs by default, debug, about 10 s): the shelf pocket of the D65
//! sentry through `ProjectSession`. It prints the histogram
//! (`--nocapture`). It asserts that every sample over `c D` plus one
//! planner cell plus one simulation cell (the read precision of both
//! lattices) has a named source, except one KNOWN RED case: the floor pass
//! along the island foot (see the test body and P22a.md).
//!
//! Arm 2 (`--ignored`, heavy): wanaka 21, toolpath id 1 "Back bulk rough
//! 18 mm", generated with its ancestors and simulated through the session.
//! It writes the table to `$CARGO_TARGET_TMPDIR/p22a_wide_sources.md`.
//!
//! `cargo test -p rs_cam_core -q --test every_wide_step_has_a_named_source_p22a -- --nocapture`

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::print_stderr,
    reason = "test code: a panic is how a test reports a failure; the test prints its readout for the reader"
)]

mod common;
#[path = "common/source_scan.rs"]
mod source_scan;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::atomic::AtomicBool;

use common::meshes::height_field;
use common::session::{mesh_model, single_op_session_with, stock_over};
use common::tools::endmill_tool_config;

use rs_cam_core::adaptive3d::FeedCapSource;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{
    Adaptive3dConfig, Adaptive3dEntryStyle, ClearingStrategy,
};
use rs_cam_core::feeds::engagement::Engagement;
use rs_cam_core::geo::P3;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::session::generation_plan::{self, Scope, Step};
use rs_cam_core::session::{
    Command, ProjectSession, SetDressupConfigArgs, SimulationOptions, SimulationResolution,
};
use rs_cam_core::stock::simulation_cut::SimulationCutTrace;
use rs_cam_core::tool::ToolDefinition;
use rs_cam_core::tool_load::locality::{SpanLookup, is_phantom_transit};
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};
use rs_cam_core::trace::semantic_trace::{
    SemanticKey, ToolpathSemanticKind, ToolpathSemanticTrace,
};
use rs_cam_core::trace::toolpath_spans::Span;

// ── The source of a fed sample ──────────────────────────────────────────

/// The source of a fed sample. The `Cap*` classes are exact (the `FeedCap`
/// items, P22a-2); the `Inferred*` cap classes rebuild a planner rule from
/// the emitted moves and serve only the cross-check; the perimeter sweep is
/// inferred (no marker names it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Source {
    /// A sample whose span ancestry the gates drop as phantom transit
    /// (`is_phantom_transit`: waterline cleanup, link bridge, lead-out,
    /// dressup artifact). No gate grades it.
    PhantomTransit,
    /// An entry move (`EntryRamp`, `EntryHelix`, `EntryPlunge`).
    RampEntry,
    HelixEntry,
    PlungeEntry,
    /// A fed keep-down link (`MoveIntent::Linking`).
    KeepDownLink,
    /// EXACT: a planned clearing turn (`FeedCapSource::PlannedTurn`).
    CapTurn,
    /// EXACT: D65 in the 2D replay (`FeedCapSource::Capped2d`).
    CapD65Plan,
    /// EXACT: a departure window (`FeedCapSource::DepartureWindow`).
    CapDeparture,
    /// EXACT: D65 in the 3D lift (`FeedCapSource::CappedSplit3d`).
    CapSplit3d,
    /// EXACT: a C4 contour run over the ceiling (`FeedCapSource::LevelContour`).
    CapLevelContour,
    /// A capped move that no `FeedCap` item covers: a defect of the door.
    CapUnnamed,
    /// INFERRED departure window (`geodesic::slice::fed_runs`): a capped
    /// run in the first cut after an entry whose start lies within R of
    /// the entry's end (the 2D rule: a step out of the plunge starts within
    /// R of the plunge point).
    InferredDeparture,
    /// INFERRED D65 3D stock-read split (`adaptive3d/clearing.rs`,
    /// `admits_capped_split`): a capped run that is not a departure and
    /// whose net step meets the shape-split rule (`|dz| >
    /// max(k dpp, Z_DROP_FLOOR_MM)` or `|dz| / xy > PLUNGE_SLOPE_LIMIT`).
    /// Only a shape-split step can be a capped split step.
    InferredSplit3d,
    /// INFERRED 2D cap, run no longer than the turn bound
    /// (`adaptive::path::turn_bound`): a planned turn, or a short D65 2D
    /// window. The two are not told apart.
    InferredCap2dShort,
    /// INFERRED D65 2D window (`LinkLoad::admits_capped`): a capped run
    /// longer than the turn bound, so not one turn.
    InferredCap2dLong,
    /// INFERRED perimeter sweep (`adaptive3d/clearing.rs`, `clear_one_region`
    /// "Perimeter sweep"): the inset region boundary cut as one uncapped
    /// loop between an entry and a retract, a full slot, emitted where the
    /// C31 grown outline is `None`. Rule: every clearing move between the
    /// entry and the next non-clearing move is uncapped, the run has at
    /// least three moves, and its end lies within its longest move of its
    /// start (the emitter drops the closing edge of the loop).
    InferredPerimeter,
    /// A sample of an uncapped flat clearing move that lies at the joint
    /// with a capped run or a draped step (within the read precision of the
    /// move end, where the next move is capped or draped, or of the move
    /// start, where the previous move is): the sim reads the material of
    /// that step at the shared point.
    CapJoint,
    /// An uncapped clearing move with a Z change: a draped 3D step that the
    /// stock-read split read at or under `c` on the planner stock
    /// (`step_load`), so it went out uncapped and unsplit.
    UncappedDrape,
    /// A flat clearing cut with no planner cap. No planner rule admits it
    /// over the ceiling: the "other" class of the design.
    Uncapped,
    /// Any other intent (lead-in or lead-out, finishing, unknown).
    OtherIntent,
}

impl Source {
    const fn label(self) -> &'static str {
        match self {
            Self::PhantomTransit => "phantom transit (gates drop)",
            Self::RampEntry => "ramp entry",
            Self::HelixEntry => "helix entry",
            Self::PlungeEntry => "plunge entry",
            Self::KeepDownLink => "keep-down link",
            Self::CapTurn => "planned turn (exact)",
            Self::CapD65Plan => "D65 2D (exact)",
            Self::CapDeparture => "departure window (exact)",
            Self::CapSplit3d => "D65 3D split (exact)",
            Self::CapLevelContour => "C4 level contour (exact)",
            Self::CapUnnamed => "OTHER: capped move with no FeedCap item",
            Self::InferredDeparture => "departure window (inferred)",
            Self::InferredSplit3d => "D65 3D split (inferred)",
            Self::InferredCap2dShort => "2D cap <= turn bound: turn or D65 2D (inferred)",
            Self::InferredCap2dLong => "D65 2D > turn bound (inferred)",
            Self::InferredPerimeter => "perimeter sweep (inferred)",
            Self::CapJoint => "joint with a capped run or a draped step",
            Self::UncappedDrape => "uncapped draped 3D step (planner read <= c)",
            Self::Uncapped => "OTHER: uncapped flat clearing cut",
            Self::OtherIntent => "OTHER: other intent",
        }
    }

    /// The exact class of a cap source.
    const fn of_cap(source: FeedCapSource) -> Self {
        match source {
            FeedCapSource::PlannedTurn => Self::CapTurn,
            FeedCapSource::Capped2d => Self::CapD65Plan,
            FeedCapSource::DepartureWindow => Self::CapDeparture,
            FeedCapSource::CappedSplit3d => Self::CapSplit3d,
            FeedCapSource::LevelContour => Self::CapLevelContour,
        }
    }

    const fn is_named(self) -> bool {
        match self {
            Self::PhantomTransit
            | Self::RampEntry
            | Self::HelixEntry
            | Self::PlungeEntry
            | Self::KeepDownLink
            | Self::CapTurn
            | Self::CapD65Plan
            | Self::CapDeparture
            | Self::CapSplit3d
            | Self::CapLevelContour
            | Self::InferredDeparture
            | Self::InferredSplit3d
            | Self::InferredCap2dShort
            | Self::InferredCap2dLong
            | Self::InferredPerimeter
            | Self::CapJoint
            | Self::UncappedDrape => true,
            Self::Uncapped | Self::OtherIntent | Self::CapUnnamed => false,
        }
    }
}

// ── Planner constants, read from the repo source ────────────────────────

/// The number in `src` between `prefix` and `suffix` (the first match).
fn number_in(src: &str, prefix: &str, suffix: &str) -> f64 {
    let start = src
        .find(prefix)
        .unwrap_or_else(|| panic_missing(prefix))
        .saturating_add(prefix.len());
    let rest = &src[start..];
    let end = rest.find(suffix).unwrap_or_else(|| panic_missing(suffix));
    rest[..end]
        .trim()
        .parse()
        .unwrap_or_else(|_| panic_missing(prefix))
}

#[expect(
    clippy::panic,
    reason = "test code: a missing source line fails the test"
)]
fn panic_missing(what: &str) -> ! {
    panic!("the planner source no longer holds `{what}`: update the P22a readout")
}

fn core_source(rel: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    source_scan::strip_comments(&std::fs::read_to_string(&path).expect("the planner source reads"))
}

/// The planner rules the inference rebuilds, with each constant read from
/// the source line that holds it.
#[derive(Debug, Clone, Copy)]
struct PlannerRules {
    /// `PLUNGE_SLOPE_LIMIT` (`adaptive3d/clearing.rs`).
    slope_limit: f64,
    /// `Z_DROP_FLOOR_MM` (`adaptive3d/clearing.rs`).
    z_drop_floor: f64,
    /// The factor of `ctx.depth_per_pass * k` (`adaptive3d/clearing.rs`).
    z_drop_factor: f64,
    /// `PLANNER_CELLS_PER_TOOL_RADIUS` (`adaptive/path.rs`).
    cells_per_radius: f64,
}

impl PlannerRules {
    fn read() -> Self {
        let clearing = core_source("src/adaptive3d/clearing.rs");
        let path = core_source("src/adaptive/path.rs");
        Self {
            slope_limit: number_in(&clearing, "const PLUNGE_SLOPE_LIMIT: f64 =", ";"),
            z_drop_floor: number_in(&clearing, "const Z_DROP_FLOOR_MM: f64 =", ";"),
            z_drop_factor: number_in(
                &clearing,
                "let z_drop_threshold = (ctx.depth_per_pass *",
                ")",
            ),
            cells_per_radius: number_in(&path, "const PLANNER_CELLS_PER_TOOL_RADIUS: f64 =", ";"),
        }
    }

    /// The planner cell `max(R / cells_per_radius, tolerance)`
    /// (`adaptive::path::planner_cell_size`).
    fn planner_cell(self, tool_radius: f64, tolerance: f64) -> f64 {
        (tool_radius / self.cells_per_radius).max(tolerance)
    }

    /// `adaptive::path::turn_bound`: `s + (1 + sqrt 2) cell`.
    fn turn_bound(self, stepover: f64, tool_radius: f64, tolerance: f64) -> f64 {
        stepover + (1.0 + std::f64::consts::SQRT_2) * self.planner_cell(tool_radius, tolerance)
    }

    /// The shape-split rule of the 3D lift, on the net step `a` to `b`.
    fn shape_split(self, a: P3, b: P3, depth_per_pass: f64) -> bool {
        let dz = (b.z - a.z).abs();
        let xy = (b.x - a.x).hypot(b.y - a.y);
        let slope = if xy > 1e-6 {
            dz / xy
        } else if dz > 1e-6 {
            f64::INFINITY
        } else {
            0.0
        };
        dz > (depth_per_pass * self.z_drop_factor).max(self.z_drop_floor)
            || slope > self.slope_limit
    }
}

// ── The classifier ──────────────────────────────────────────────────────

/// What the classifier needs of the op.
#[derive(Debug, Clone, Copy)]
struct OpFacts {
    tool_radius: f64,
    stepover: f64,
    depth_per_pass: f64,
    tolerance: f64,
}

impl OpFacts {
    /// The pass ceiling `c` as a radial fraction of D (the planner's own
    /// read; `pass_engagement_limit` holds `PASS_ENGAGEMENT_TOLERANCE`).
    fn ceiling_fraction(self) -> f64 {
        rs_cam_core::ops::adaptive_shared::radial_woc_fraction_from_leading_arc(
            rs_cam_core::adaptive::pass_engagement_limit(self.stepover, self.tool_radius),
        )
    }

    fn ceiling_mm(self) -> f64 {
        self.ceiling_fraction() * 2.0 * self.tool_radius
    }
}

/// One capped run: consecutive clearing moves with one `feed_cap`.
#[derive(Debug, Clone, Copy)]
struct CapRun {
    first: usize,
    last: usize,
    start: P3,
    end: P3,
    xy_len: f64,
    /// The run lies in the first cut after an entry, and this is the
    /// entry's end.
    after_entry: Option<P3>,
}

/// The source of each move of `tp` (by index; a rapid or a retract gets
/// `None`), with the capped moves read from `exact` (the `FeedCap` items),
/// and the inferred class of each capped move (the cross-check).
fn move_sources(
    tp: &Toolpath,
    op: OpFacts,
    rules: PlannerRules,
    exact: &[Option<FeedCapSource>],
) -> (Vec<Option<Source>>, Vec<Option<Source>>) {
    let moves = &tp.moves;
    let mut out: Vec<Option<Source>> = vec![None; moves.len()];
    let mut inferred: Vec<Option<Source>> = vec![None; moves.len()];
    let turn_bound = rules.turn_bound(op.stepover, op.tool_radius, op.tolerance);
    // The end of the last entry while the first cut after it runs.
    let mut entry_end: Option<P3> = None;
    let mut runs: Vec<CapRun> = Vec::new();
    let mut open: Option<(CapRun, u64)> = None;
    let close = |open: &mut Option<(CapRun, u64)>, runs: &mut Vec<CapRun>| {
        if let Some((run, _)) = open.take() {
            runs.push(run);
        }
    };
    for (i, m) in moves.iter().enumerate() {
        let prev = if i == 0 {
            m.target
        } else {
            moves[i - 1].target
        };
        let fed = !matches!(m.move_type, MoveType::Rapid);
        let source = match m.intent {
            MoveIntent::EntryRamp => Some(Source::RampEntry),
            MoveIntent::EntryHelix => Some(Source::HelixEntry),
            MoveIntent::EntryPlunge => Some(Source::PlungeEntry),
            MoveIntent::Linking => Some(Source::KeepDownLink),
            MoveIntent::ClearingCut => {
                if m.feed_cap.is_some() {
                    // Resolved per run below.
                    None
                } else if (m.target.z - prev.z).abs() > 1e-6 {
                    Some(Source::UncappedDrape)
                } else {
                    Some(Source::Uncapped)
                }
            }
            MoveIntent::Drilling
            | MoveIntent::FinishingCut
            | MoveIntent::LeadIn
            | MoveIntent::LeadOut
            | MoveIntent::Unknown => Some(Source::OtherIntent),
            MoveIntent::EntryApproach | MoveIntent::Retract | MoveIntent::ObstacleClearance => None,
        };
        let entry = matches!(
            m.intent,
            MoveIntent::EntryRamp | MoveIntent::EntryHelix | MoveIntent::EntryPlunge
        );
        // The capped run this move continues, opens or closes.
        match (m.intent, m.feed_cap) {
            (MoveIntent::ClearingCut, Some(cap)) if fed => {
                let key = cap.to_bits();
                let xy = (m.target.x - prev.x).hypot(m.target.y - prev.y);
                match open.as_mut() {
                    Some((run, k)) if *k == key && run.last + 1 == i => {
                        run.last = i;
                        run.end = m.target;
                        run.xy_len += xy;
                    }
                    Some(_) | None => {
                        close(&mut open, &mut runs);
                        open = Some((
                            CapRun {
                                first: i,
                                last: i,
                                start: prev,
                                end: m.target,
                                xy_len: xy,
                                after_entry: entry_end,
                            },
                            key,
                        ));
                    }
                }
            }
            _ => close(&mut open, &mut runs),
        }
        if entry {
            entry_end = Some(m.target);
        } else if m.intent != MoveIntent::ClearingCut {
            // A link, a retract or a rapid ends the first cut.
            entry_end = None;
        }
        out[i] = if fed { source } else { None };
    }
    close(&mut open, &mut runs);
    mark_perimeter_sweeps(tp, &mut out);
    for run in &runs {
        let departure = run
            .after_entry
            .is_some_and(|e| (run.start.x - e.x).hypot(run.start.y - e.y) < op.tool_radius);
        let source = if departure {
            Source::InferredDeparture
        } else if rules.shape_split(run.start, run.end, op.depth_per_pass) {
            Source::InferredSplit3d
        } else if run.xy_len <= turn_bound {
            Source::InferredCap2dShort
        } else {
            Source::InferredCap2dLong
        };
        for slot in &mut inferred[run.first..=run.last] {
            *slot = Some(source);
        }
    }
    for (i, slot) in out.iter_mut().enumerate() {
        if tp.moves[i].feed_cap.is_some() && tp.moves[i].intent == MoveIntent::ClearingCut {
            *slot = Some(
                exact
                    .get(i)
                    .copied()
                    .flatten()
                    .map_or(Source::CapUnnamed, Source::of_cap),
            );
        }
    }
    (out, inferred)
}

/// The exact cap source of each move: the `FeedCap` items of the semantic
/// trace (P22a-2), on the capped moves they cover. Also the pin counts:
/// capped clearing moves in no item, in two or more items, and uncapped
/// clearing moves inside an item.
fn exact_sources(
    tp: &Toolpath,
    trace: Option<&ToolpathSemanticTrace>,
) -> (Vec<Option<FeedCapSource>>, CapPin) {
    let n = tp.moves.len();
    let mut source: Vec<Option<FeedCapSource>> = vec![None; n];
    let mut cover = vec![0usize; n];
    let mut pin = CapPin::default();
    for item in trace.map_or(&[][..], |t| t.items.as_slice()) {
        if item.kind != ToolpathSemanticKind::FeedCap {
            continue;
        }
        pin.items += 1;
        let key = item
            .params
            .get(SemanticKey::CapSource)
            .and_then(|v| v.as_str())
            .and_then(FeedCapSource::from_key)
            .expect("a FeedCap item names its source");
        let (Some(a), Some(b)) = (item.move_start, item.move_end) else {
            pin.unlinked += 1;
            continue;
        };
        for k in a..=b.min(n.saturating_sub(1)) {
            let m = &tp.moves[k];
            if m.intent != MoveIntent::ClearingCut {
                continue;
            }
            if m.feed_cap.is_some() {
                cover[k] += 1;
                source[k] = Some(key);
            } else {
                pin.uncapped_inside += 1;
            }
        }
    }
    for (k, m) in tp.moves.iter().enumerate() {
        if m.intent == MoveIntent::ClearingCut && m.feed_cap.is_some() {
            pin.capped += 1;
            match cover[k] {
                0 => pin.capped_outside += 1,
                1 => {}
                _ => pin.capped_twice += 1,
            }
        }
    }
    (source, pin)
}

/// The 1:1 pin between the `FeedCap` items and `Move::feed_cap`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct CapPin {
    items: usize,
    unlinked: usize,
    capped: usize,
    capped_outside: usize,
    capped_twice: usize,
    uncapped_inside: usize,
}

/// [`Source::InferredPerimeter`]: mark each uncapped loop that fills a
/// whole entry-to-retract cut.
fn mark_perimeter_sweeps(tp: &Toolpath, out: &mut [Option<Source>]) {
    let moves = &tp.moves;
    let entry = |i: usize| {
        moves.get(i).is_some_and(|m| {
            matches!(
                m.intent,
                MoveIntent::EntryRamp | MoveIntent::EntryHelix | MoveIntent::EntryPlunge
            )
        })
    };
    let clearing = |i: usize| {
        moves
            .get(i)
            .is_some_and(|m| m.intent == MoveIntent::ClearingCut)
    };
    for s in 1..moves.len() {
        if !entry(s - 1) || !clearing(s) {
            continue;
        }
        let mut e = s;
        while clearing(e + 1) {
            e += 1;
        }
        if moves[s..=e].iter().any(|m| m.feed_cap.is_some()) || e - s < 2 {
            continue;
        }
        let start = moves[s - 1].target;
        let xy = |a: P3, b: P3| (b.x - a.x).hypot(b.y - a.y);
        let longest = (s..=e)
            .map(|k| xy(moves[k - 1].target, moves[k].target))
            .fold(0.0, f64::max);
        if xy(start, moves[e].target) <= longest {
            for slot in &mut out[s..=e] {
                *slot = Some(Source::InferredPerimeter);
            }
        }
    }
}

/// One histogram row.
#[derive(Debug, Clone, Default)]
struct Row {
    /// Samples over `c D` plus one cell, by the innermost span of their
    /// move (kind and label).
    by_span: BTreeMap<String, usize>,
    /// Samples over `c D`.
    wide: usize,
    /// Samples over `c D` plus one simulation cell.
    wide_cell: usize,
    /// Samples over `c D` plus one planner cell plus one simulation cell:
    /// past the read precision of both lattices.
    wide_precise: usize,
    /// The positions of the `wide_precise` samples.
    precise_at: Vec<[f64; 3]>,
    /// The removal check of the `wide_precise` samples: each sample's
    /// measured removal rate (`mrr_mm3_s`) over the rate of a full
    /// rectangle at its width (`ap ae F / 60`), and the count. Near 1: the
    /// width is real removal. Near 0: the arc overstates the width (a thin
    /// ring or a post inside the cutter, `adaptive/CLAUDE.md` "Do not").
    removal_sum: f64,
    removal_n: usize,
    /// The distinct moves of the `wide_precise` samples.
    precise_moves: std::collections::BTreeSet<usize>,
    max_ae: f64,
    /// The move, the sample position and the feed of the widest sample.
    max_move: usize,
    max_at: [f64; 3],
    max_feed: f64,
    max_ap: f64,
}

/// The readout of one toolpath.
#[derive(Debug, Clone, Default)]
struct Readout {
    rows: BTreeMap<Source, Row>,
    graded: usize,
    ceiling_mm: f64,
    ceiling_fraction: f64,
    diameter: f64,
    cell: f64,
    planner_cell: f64,
    turn_bound: f64,
    capped_moves: usize,
    moves: usize,
    pin: CapPin,
    /// Capped moves by (exact class, inferred class): the cross-check.
    agreement: BTreeMap<(Source, Source), usize>,
}

/// The public inputs of one readout.
struct Inputs<'a> {
    tp: &'a Toolpath,
    semantic: Option<&'a ToolpathSemanticTrace>,
    spans: Option<&'a [Span]>,
    trace: &'a SimulationCutTrace,
    id: ToolpathId,
    tool: &'a ToolDefinition,
    op: OpFacts,
    rules: PlannerRules,
    cell: f64,
}

fn readout(inputs: &Inputs<'_>) -> Readout {
    let &Inputs {
        tp,
        semantic,
        spans,
        trace,
        id,
        tool,
        op,
        rules,
        cell,
    } = inputs;
    let (exact, pin) = exact_sources(tp, semantic);
    let (sources, inferred) = move_sources(tp, op, rules, &exact);
    let mut agreement: BTreeMap<(Source, Source), usize> = BTreeMap::new();
    for (s, i) in sources.iter().zip(&inferred) {
        if let (Some(s), Some(i)) = (s, i) {
            *agreement.entry((*s, *i)).or_default() += 1;
        }
    }
    let lookup = spans.map(SpanLookup::new);
    let ceiling_mm = op.ceiling_mm();
    let mut out = Readout {
        ceiling_mm,
        ceiling_fraction: op.ceiling_fraction(),
        diameter: 2.0 * op.tool_radius,
        cell,
        planner_cell: rules.planner_cell(op.tool_radius, op.tolerance),
        turn_bound: rules.turn_bound(op.stepover, op.tool_radius, op.tolerance),
        capped_moves: tp.moves.iter().filter(|m| m.feed_cap.is_some()).count(),
        moves: tp.moves.len(),
        pin,
        agreement,
        ..Readout::default()
    };
    for sample in &trace.samples {
        if sample.toolpath_id != id || !sample.is_cutting {
            continue;
        }
        let Some(engagement) = Engagement::from_sample(sample, tool) else {
            continue;
        };
        out.graded += 1;
        let ae = engagement.ae_mm();
        if ae <= ceiling_mm {
            continue;
        }
        let precision = out.planner_cell + cell;
        let source = if is_phantom_transit(sample, lookup.as_ref()) {
            Source::PhantomTransit
        } else {
            let source = sources
                .get(sample.move_index)
                .copied()
                .flatten()
                .unwrap_or(Source::OtherIntent);
            if source == Source::Uncapped
                && at_cap_joint(tp, sample.move_index, sample.position, precision)
            {
                Source::CapJoint
            } else {
                source
            }
        };
        let row = out.rows.entry(source).or_default();
        row.wide += 1;
        if ae > ceiling_mm + precision {
            row.wide_precise += 1;
            row.precise_at.push(sample.position);
            let _ = row.precise_moves.insert(sample.move_index);
            let full = engagement.ap_mm() * ae * sample.feed_rate_mm_min / 60.0;
            if full > 0.0 {
                row.removal_sum += sample.mrr_mm3_s / full;
                row.removal_n += 1;
            }
        }
        if ae > ceiling_mm + cell {
            row.wide_cell += 1;
            *row.by_span
                .entry(innermost_span(spans, sample.move_index))
                .or_default() += 1;
        }
        if ae > row.max_ae {
            row.max_ae = ae;
            row.max_move = sample.move_index;
            row.max_at = sample.position;
            row.max_feed = sample.feed_rate_mm_min;
            row.max_ap = engagement.ap_mm();
        }
    }
    out
}

/// Whether a sample at `at` on move `i` lies within `reach` (XY) of the
/// end of move `i` where move `i + 1` is capped or draped, or of its start
/// where move `i - 1` is.
fn at_cap_joint(tp: &Toolpath, i: usize, at: [f64; 3], reach: f64) -> bool {
    let near = |p: P3| (p.x - at[0]).hypot(p.y - at[1]) <= reach;
    // Move `k` is capped, or is a clearing move with a Z change.
    let marked = |k: usize| {
        k >= 1
            && tp.moves.get(k).is_some_and(|m| {
                m.feed_cap.is_some()
                    || (m.intent == MoveIntent::ClearingCut
                        && (m.target.z - tp.moves[k - 1].target.z).abs() > 1e-6)
            })
    };
    (marked(i + 1) && near(tp.moves[i].target))
        || (i >= 1 && marked(i - 1) && near(tp.moves[i - 1].target))
}

/// The kind and label of the smallest span that holds move `i`.
fn innermost_span(spans: Option<&[Span]>, i: usize) -> String {
    spans
        .unwrap_or_default()
        .iter()
        .filter(|s| s.start_move <= i && i < s.end_move)
        .min_by_key(|s| s.end_move - s.start_move)
        .map_or_else(
            || "no span".to_owned(),
            |s| format!("{:?} '{}'", s.kind, s.label),
        )
}

fn table(name: &str, r: &Readout) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "## {name}\n\nD {:.3} mm; c {:.4}; c D {:.3} mm; sim cell {:.3} mm; planner cell {:.3} mm; \
         turn bound {:.3} mm; {} moves, {} capped; {} graded cutting samples.\n",
        r.diameter,
        r.ceiling_fraction,
        r.ceiling_mm,
        r.cell,
        r.planner_cell,
        r.turn_bound,
        r.moves,
        r.capped_moves,
        r.graded
    );
    let _ = writeln!(
        s,
        "| Source | Samples > c D | > c D + sim cell | > c D + both cells | Mean removal / (ap ae F) of those | Max ae (mm) | Max ae / D | Widest: move, ap (mm), feed, XYZ |"
    );
    let _ = writeln!(s, "|---|---|---|---|---|---|---|---|");
    for (source, row) in &r.rows {
        let _ = writeln!(
            s,
            "| {} | {} | {} | {} | {:.2} | {:.2} | {:.3} | {}, {:.2}, {:.0}, ({:.1}, {:.1}, {:.2}) |",
            source.label(),
            row.wide,
            row.wide_cell,
            row.wide_precise,
            if row.removal_n > 0 {
                row.removal_sum / row.removal_n as f64
            } else {
                f64::NAN
            },
            row.max_ae,
            row.max_ae / r.diameter,
            row.max_move,
            row.max_ap,
            row.max_feed,
            row.max_at[0],
            row.max_at[1],
            row.max_at[2],
        );
    }
    let _ = writeln!(
        s,
        "\nWhere the samples over c D + both cells lie (distinct moves; XY extent; Z range):\n"
    );
    for (source, row) in &r.rows {
        let ext = row.precise_at.iter().fold(
            [
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
            ],
            |e, p| {
                [
                    e[0].min(p[0]),
                    e[1].max(p[0]),
                    e[2].min(p[1]),
                    e[3].max(p[1]),
                    e[4].min(p[2]),
                    e[5].max(p[2]),
                ]
            },
        );
        let _ = writeln!(
            s,
            "- {}: {} moves (first: {:?}); X {:.1}..{:.1}, Y {:.1}..{:.1}, Z {:.2}..{:.2}",
            source.label(),
            row.precise_moves.len(),
            row.precise_moves.iter().take(12).collect::<Vec<_>>(),
            ext[0],
            ext[1],
            ext[2],
            ext[3],
            ext[4],
            ext[5]
        );
    }
    let _ = writeln!(
        s,
        "\nFeedCap pin: {} items ({} unlinked); {} capped clearing moves, {} in no item, {} in two \
         or more; {} uncapped clearing moves inside an item.\n",
        r.pin.items,
        r.pin.unlinked,
        r.pin.capped,
        r.pin.capped_outside,
        r.pin.capped_twice,
        r.pin.uncapped_inside
    );
    let _ = writeln!(s, "Cross-check, capped moves by (exact, inferred):\n");
    for ((exact, inferred), n) in &r.agreement {
        let _ = writeln!(s, "- {} / {}: {n}", exact.label(), inferred.label());
    }
    let _ = writeln!(
        s,
        "\nSamples over c D + cell by the innermost span of the move:\n"
    );
    for (source, row) in &r.rows {
        for (span, n) in &row.by_span {
            let _ = writeln!(s, "- {}: {span}: {n}", source.label());
        }
    }
    s
}

fn options(resolution_mm: f64) -> SimulationOptions {
    SimulationOptions {
        resolution: SimulationResolution::Fixed(resolution_mm),
        skip_ids: Vec::new(),
        // The widths do not read the feed; the planner feeds stay as
        // generated, so `feed_cap` is the planner's cap.
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: false,
        modulation_strategy:
            rs_cam_core::dressup::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_feed_scale: 1.0,
    }
}

/// The readout of toolpath `index` of a simulated session.
fn session_readout(session: &ProjectSession, index: usize, op: OpFacts, cell: f64) -> Readout {
    let tc = session.toolpath_config_at(index).expect("the toolpath");
    let tool_cfg = session
        .tool_by_id(tc.tool_id.expect("a tool"))
        .expect("the tool");
    let tool = rs_cam_core::compute::cutter::build_cutter(tool_cfg).expect("the cutter builds");
    let result = session.result_at(index).expect("a result");
    let annotated = result.annotated();
    let trace = session
        .simulation_result()
        .and_then(|s| s.cut_trace.as_deref())
        .expect("a cut trace");
    readout(&Inputs {
        tp: &annotated.toolpath,
        semantic: result.semantic_trace.as_deref(),
        spans: annotated.spans_valid.then_some(annotated.spans.as_slice()),
        trace,
        id: tc.id,
        tool: &tool,
        op,
        rules: PlannerRules::read(),
        cell,
    })
}

/// The op facts of a 3D Rough config.
fn op_facts(op: &OperationConfig, tool_radius: f64) -> OpFacts {
    let OperationConfig::Adaptive3d(cfg) = op else {
        panic_missing("an Adaptive3d op")
    };
    OpFacts {
        tool_radius,
        stepover: cfg.stepover.expect("a stated stepover"),
        depth_per_pass: cfg.depth_per_pass.expect("a stated depth per pass"),
        tolerance: cfg.tolerance,
    }
}

// ── Arm 1: the shelf pocket ─────────────────────────────────────────────

// The shelf pocket of the D65 sentry
// (`a_geodesic_step_over_the_ceiling_is_cut_capped_d65`).
const TOP_Z: f64 = 10.0;
const FLOOR_Z: f64 = 4.0;
const MESH_HALF: f64 = 40.0;
const MESH_STEP: f64 = 0.5;
const POCKET_HALF_X: f64 = 34.0;
const POCKET_HALF_Y: f64 = 24.0;
const ISLAND_RADIUS: f64 = 5.0;
const SHELF_Z: f64 = 6.5;
const TOOL_DIAMETER: f64 = 6.0;
const STEPOVER: f64 = 2.0;
/// The simulation cell of arm 1.
const SIM_CELL: f64 = 0.25;

fn shelf_session() -> ProjectSession {
    let mesh = height_field(MESH_HALF, MESH_STEP, |x, y| {
        if x.hypot(y) <= ISLAND_RADIUS {
            TOP_Z
        } else if (12.0..=30.0).contains(&x) && y.abs() <= 12.0 {
            SHELF_Z
        } else if x.abs() <= POCKET_HALF_X && y.abs() <= POCKET_HALF_Y {
            FLOOR_Z
        } else {
            TOP_Z
        }
    });
    let op = OperationConfig::Adaptive3d(Adaptive3dConfig {
        stepover: Some(STEPOVER),
        depth_per_pass: Some(TOP_Z - FLOOR_Z),
        clearing_strategy: ClearingStrategy::GeodesicAdaptive,
        entry_style: Adaptive3dEntryStyle::Ramp,
        ..Adaptive3dConfig::default()
    });
    single_op_session_with(
        stock_over(MESH_HALF, TOP_Z),
        endmill_tool_config(TOOL_DIAMETER),
        mesh_model(mesh, "shelf"),
        "shelf rough",
        op,
        |_| {},
    )
}

#[test]
fn every_wide_step_on_the_shelf_has_a_named_source() {
    let mut session = shelf_session();
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("the rough generates");
    session
        .run_simulation(&options(SIM_CELL), &cancel)
        .expect("the simulation runs");
    let op = op_facts(
        &session.toolpath_config_at(0).expect("the op").operation,
        TOOL_DIAMETER / 2.0,
    );
    let r = session_readout(&session, 0, op, SIM_CELL);
    eprintln!("{}", table("shelf pocket (D65 fixture), 6 mm, s 2", &r));
    // P22a-2: every capped clearing move lies in exactly one FeedCap item,
    // and no item covers an uncapped clearing move.
    assert!(
        r.pin.items > 0,
        "non-vacuity: the shelf has no FeedCap item"
    );
    assert_eq!(
        (
            r.pin.unlinked,
            r.pin.capped_outside,
            r.pin.capped_twice,
            r.pin.uncapped_inside
        ),
        (0, 0, 0, 0),
        "the FeedCap items and Move::feed_cap disagree: {:?}",
        r.pin
    );
    let wide: usize = r.rows.values().map(|row| row.wide).sum();
    assert!(wide > 0, "non-vacuity: the shelf has no sample over c D");
    // KNOWN RED (P22a finding, HEAD 90e03440b): the floor pass that hugs
    // the island wall (moves near the island foot) reads past the read
    // precision with no planner cap and no Z change; no public door names
    // it. Every other wide sample has a named source. The pin: each
    // unnamed sample lies in the island-foot ring, and the ring holds at
    // least one. The ring: the tool centre within R + one mesh step (the
    // height field's wall width) + both cells of the island wall. When a
    // change names or removes the case, this fails: then assert that
    // `unnamed` is empty.
    let precision = r.planner_cell + r.cell;
    let ring = ISLAND_RADIUS + MESH_STEP + TOOL_DIAMETER / 2.0 + precision;
    let unnamed: Vec<[f64; 3]> = r
        .rows
        .iter()
        .filter(|(s, _)| !s.is_named())
        .flat_map(|(_, row)| row.precise_at.iter().copied())
        .collect();
    let off_ring: Vec<[f64; 3]> = unnamed
        .iter()
        .copied()
        .filter(|p| p[0].hypot(p[1]) > ring)
        .collect();
    assert!(
        off_ring.is_empty(),
        "samples over c D + both cells with no named source, off the island-foot ring \
         (radius {ring:.2} mm): {off_ring:?}"
    );
    assert!(
        !unnamed.is_empty(),
        "the island-foot case is gone: the known red flips; assert that no sample is unnamed"
    );
}

// ── Arm 2: wanaka 21, toolpath id 1 ─────────────────────────────────────

const WANAKA: &str = "../../planning/rivmap_block_job/nz_wanaka_21_operator.toml";
const WANAKA_ID: usize = 1;

#[test]
#[ignore = "evidence: P22a readout on wanaka 21 idx 1 (generates the face and the rough, simulates; about 3 min in debug)"]
fn the_wide_sources_of_wanaka_idx_1() {
    wanaka_readout(true, "p22a_wide_sources.md");
}

/// The counterfactual: the same op with the rapid-order dressup off, so
/// the moves run in the planner's order.
#[test]
#[ignore = "evidence: P22a readout on wanaka 21 idx 1 with no rapid reorder (about 3 min in debug)"]
fn the_wide_sources_of_wanaka_idx_1_in_plan_order() {
    wanaka_readout(false, "p22a_wide_sources_plan_order.md");
}

fn wanaka_readout(rapid_order: bool, out_name: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(WANAKA);
    let mut session = ProjectSession::load(&path).expect("the wanaka project loads");
    let index = (0..session.toolpath_count())
        .find(|&i| {
            session
                .toolpath_config_at(i)
                .is_some_and(|tc| tc.id.raw() == WANAKA_ID)
        })
        .expect("toolpath id 1");
    if !rapid_order {
        let mut dressups = session
            .toolpath_config_at(index)
            .expect("the op")
            .dressups
            .clone();
        dressups.optimize_rapid_order = false;
        let _ = session
            .apply(Command::SetDressupConfig(SetDressupConfigArgs {
                index,
                dressups: Box::new(dressups),
            }))
            .expect("the dressup edit applies");
    }
    let id = session.toolpath_config_at(index).expect("the op").id;
    let cancel = AtomicBool::new(false);
    let cell = session.simulation_resolution_mm();
    let started = std::time::Instant::now();
    for step in generation_plan::plan(&session, Scope::Ancestors(id)) {
        match step {
            Step::Simulate { .. } => {
                session
                    .run_simulation(&options(cell), &cancel)
                    .expect("the plan simulation runs");
            }
            Step::Generate { index, .. } => {
                session
                    .generate_toolpath(index, &cancel)
                    .expect("an ancestor generates");
            }
        }
    }
    let generated_s = started.elapsed().as_secs_f64();
    session
        .run_simulation(&options(cell), &cancel)
        .expect("the closing simulation runs");
    let tc = session.toolpath_config_at(index).expect("the op");
    let tool_radius = session
        .tool_by_id(tc.tool_id.expect("a tool"))
        .expect("the tool")
        .diameter
        / 2.0;
    let op = op_facts(&tc.operation, tool_radius);
    let r = session_readout(&session, index, op, cell);
    let mut text = format!(
        "# P22a wide sources\n\nProject `{WANAKA}`, toolpath id {WANAKA_ID} '{}'. Plan and \
         generation {generated_s:.0} s, total {:.0} s (debug).\n\n",
        tc.name,
        started.elapsed().as_secs_f64()
    );
    // Each unnamed wide move: its geometry and its neighbours.
    let moves = &session.result_at(index).expect("a result").toolpath().moves;
    let _ = writeln!(
        text,
        "Unnamed wide moves (index: from -> to, XY length, feed, cap; previous and next intent):\n"
    );
    for (source, row) in &r.rows {
        if source.is_named() {
            continue;
        }
        for &i in &row.precise_moves {
            let (a, m) = (moves[i.saturating_sub(1)].target, &moves[i]);
            let neighbour = |k: Option<usize>| {
                k.and_then(|k| moves.get(k))
                    .map_or_else(|| "none".to_owned(), |n| format!("{:?}", n.intent))
            };
            let _ = writeln!(
                text,
                "- {i}: ({:.1}, {:.1}, {:.2}) -> ({:.1}, {:.1}, {:.2}), {:.1} mm, F {:?}, cap {:?}; {} / {}",
                a.x,
                a.y,
                a.z,
                m.target.x,
                m.target.y,
                m.target.z,
                (m.target.x - a.x).hypot(m.target.y - a.y),
                m.move_type.feed_rate(),
                m.feed_cap,
                neighbour(i.checked_sub(1)),
                neighbour(Some(i + 1)),
            );
        }
    }
    text.push('\n');
    let stock = session.stock_config();
    let _ = writeln!(
        text,
        "Stock X {:.1}..{:.1}, Y {:.1}..{:.1}, Z {:.1}..{:.1}; op stock source {:?}.\n",
        stock.origin_x,
        stock.origin_x + stock.x,
        stock.origin_y,
        stock.origin_y + stock.y,
        stock.origin_z,
        stock.origin_z + stock.z.unwrap_or_default(),
        tc.stock_source
    );
    let _ = writeln!(text, "Rapid-order dressup: {rapid_order}.\n");
    text.push_str(&table("wanaka 21 idx 1", &r));
    eprintln!("{text}");
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(out_name);
    std::fs::write(&out, text).expect("the table writes");
}
