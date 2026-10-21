//! W4 — every operation conforms to the contract its registry row states.
//!
//! # Why this exists
//!
//! The operation-architecture plan (`planning/op_architecture_2026-10-14/`
//! PLAN.md, wave W4) asks for one conformance suite generated over
//! `OperationType::ALL`. Each check reads the op's registry row and its
//! `capabilities()`, never a hand list of ops, so a new op gets every check
//! with no edit here. Each failure names the op and the check
//! (`<op>/<check>`).
//!
//! # The path and the fixtures
//!
//! Every case builds a `ProjectSession` from `common::op_fixtures` (the
//! G-OPGOLDEN fixtures: one per `GeometryRequirement`, the registry tool)
//! and calls `generate_toolpath`, the door the GUI worker, MCP and the CLI
//! share.
//!
//! # The checks
//!
//! 1. **Cancel first.** The claim "a pre-set cancel flag reports Cancelled
//!    before any refusal" lives at the dispatch, which is `pub(crate)`. Its
//!    one source stays in `compute/execute/tests.rs`:
//!    `cancellable_families_honour_a_preset_cancel_flag` (the prologue
//!    order), `every_generator_polls_a_preset_cancel_flag_past_the_prologue`
//!    (each generator polls the flag) and
//!    `every_tool_refusal_is_the_registry_rows_through_the_dispatch`. They
//!    cannot move here without a public door into the dispatch. This file
//!    adds the session-door row only.
//! 2. **Rest**, per `Capabilities::rest_use`: see
//!    [`check2_rest_rule_per_op`] and
//!    [`check2b_a_rest_op_removes_no_more_material_than_a_fresh_op`].
//! 3. **Empty.** Every generation in this file is classified; a
//!    nothing-to-cut fixture gives each op a reason to be empty. See
//!    [`classify`].
//! 4. **Flipped frame.** Every op on a `FaceUp::Bottom` setup over mirrored
//!    inputs (`common::op_fixtures`, "The flipped frame"). See
//!    [`check4_a_flipped_setup_cuts_inside_the_stock_and_mirrors_the_top_up_cut`].
//! 5. **Determinism.** Two runs on the default pool and one run on a
//!    one-thread pool give the same G-OPGOLDEN fingerprint.
//! 6. **Schema round trip.** See
//!    [`check6_every_schema_default_round_trips_and_every_driver_field_is_refused`]
//!    and [`check6b_every_null_schema_default_round_trips`].
//!
//! W4 found two product defects (check 2b: Waterline, G-RAMPCHORD; check
//! 6b: Unified Finish). Both are fixed; the claims stand green.
//! 7. **No Unknown intent** (B11, the W6 census rule). G-OPGOLDEN owns the
//!    fresh and rest arms. This file runs the same counter
//!    (`common::op_fixtures::unknown_intents`) on the arms only it
//!    generates: flipped, nothing-to-cut, and the two rest-pair arms.
//! 8. **Obstacles** (K1). A box strip across the stock, its top above the
//!    stock top, on every op, top-up AND flipped. See
//!    [`check8_no_op_puts_the_tool_inside_an_obstacle`].
//! 9. **Selection** (geometry selection P2.3b, DESIGN 3.1). An op holds a
//!    selection exactly when its `item_use` reads items; the selection is
//!    a driver-set field with the op's own default, and a default
//!    selection writes no key. See
//!    [`check9_an_op_holds_a_selection_exactly_when_it_cuts_items`].
//!
//! # The simulation cell
//!
//! The rest check simulates at `SIM_CELL_MM` = 0.5 mm. The narrowest
//! feature of the fixtures is the smallest registry tool (Ø3.175 mm) and the
//! 2 mm stepover of the prior pocket, so the cell is at most a quarter of
//! each. The simulator blends a cell the tool covers in part, so each
//! cellwise claim skips the one-cell band at the edge of the cut
//! ([`EDGE_CELLS`]).
//!
//! ```text
//! cargo test -p rs_cam_core -q --test every_op_conforms_w4
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::panic::AssertUnwindSafe;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, OnceLock};

use common::op_fixtures::{
    CaseSpec, Fixture, Frame, Golden, Prior, case_session, finest_decimals, flat_stock, mesh_stock,
    on_mesh, quantize, run_prior, simulate, tool_for, unknown_intents,
};
use common::session::toolpath_config;
use rayon::prelude::*;
use rs_cam_core::compute::catalog::{ItemUse, Kinematics, OperationConfig, OperationType, RestUse};
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId};
use rs_cam_core::compute::{OperationError, StockConfig};
use rs_cam_core::dexel_stock::{FRESH_MATERIAL_THRESHOLD_MM, TriDexelStock};
use rs_cam_core::feeds::CutterKind;
use rs_cam_core::geo::P3;
use rs_cam_core::geometry::arc_util::linearize_arc;
use rs_cam_core::machine::kinematics::compute_cycle_time;
use rs_cam_core::session::{
    AddObstacleArgs, Command, Obstacle, ObstacleGeometry, ProjectSession, ProjectSessionBuilder,
    SessionError, SetToolpathParamArgs, SetupEvalContext,
};
use rs_cam_core::toolpath::{Move, MoveIntent, MoveType, Toolpath};
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;

/// The band (cells) at the edge of a cut that a cellwise claim skips: the
/// simulator blends a cell the tool covers in part.
const EDGE_CELLS: usize = 1;

/// The float noise (mm) of two cut-length sums over the same moves.
const CUT_LENGTH_EPS_MM: f64 = 1e-6;

/// The material height (mm) the simulator reads as standing material
/// (`FRESH_MATERIAL_THRESHOLD_MM`), as the `f32` of the stock grid.
fn threshold() -> f32 {
    FRESH_MATERIAL_THRESHOLD_MM as f32
}

// ── One generation ──────────────────────────────────────────────────────

/// What one generation gave.
enum Outcome {
    /// The adopted toolpath and its spans.
    Done(AnnotatedToolpath),
    /// The session refused. Kept whole, so a check can read the variant.
    Refused(SessionError),
    /// The generation panicked. The text is the panic payload.
    Panicked(String),
}

impl Outcome {
    fn toolpath(&self) -> Option<&Toolpath> {
        match self {
            Self::Done(at) => Some(&at.toolpath),
            Self::Refused(_) | Self::Panicked(_) => None,
        }
    }

    fn golden(&self) -> Golden {
        match self {
            Self::Done(at) => Golden::of(at),
            Self::Refused(e) => Golden::refused(&e.to_string()),
            Self::Panicked(p) => Golden::refused(&format!("panic: {p}")),
        }
    }

    fn describe(&self) -> String {
        match self {
            Self::Done(at) => format!("done, {} moves", at.toolpath.moves.len()),
            Self::Refused(e) => format!("refused: {e:?}"),
            Self::Panicked(p) => format!("panicked: {p}"),
        }
    }
}

fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a non-text panic payload".to_owned())
}

/// Generate toolpath `index` of `session`. A panic becomes an outcome, so
/// the check names the op instead of stopping at the first panic.
fn generate(session: &mut ProjectSession, index: usize) -> Outcome {
    let cancel = AtomicBool::new(false);
    match std::panic::catch_unwind(AssertUnwindSafe(|| {
        session
            .generate_toolpath(index, &cancel)
            .map(|r| AnnotatedToolpath::clone(r.annotated()))
    })) {
        Ok(Ok(at)) => Outcome::Done(at),
        Ok(Err(e)) => Outcome::Refused(e),
        Err(payload) => Outcome::Panicked(panic_text(payload.as_ref())),
    }
}

/// The default case of `op`: fresh stock, top-up, the standard fixture and
/// the registry tool.
fn spec(name: &str, op: OperationType) -> CaseSpec<'_> {
    CaseSpec {
        name,
        op: OperationConfig::new_default(op),
        prior: Prior::None,
        stock_source: StockSource::Fresh,
        frame: Frame::TopUp,
        fixture: Fixture::Standard,
        tool: None,
    }
}

/// One op on its own, on fresh stock.
fn generate_alone(op: OperationType, frame: Frame, fixture: Fixture) -> Outcome {
    let (mut session, index) = case_session(CaseSpec {
        frame,
        fixture,
        ..spec(op.kind_str(), op)
    });
    generate(&mut session, index)
}

fn fixture_stock(op: OperationType) -> StockConfig {
    if on_mesh(op) {
        mesh_stock()
    } else {
        flat_stock()
    }
}

/// True for a move the planner marks as a cut (a drill feed included).
/// Entry, lead, link and retract moves are a dressup's business.
fn is_cut(m: &Move) -> bool {
    m.move_type != MoveType::Rapid
        && matches!(
            m.intent,
            MoveIntent::ClearingCut | MoveIntent::FinishingCut | MoveIntent::Drilling
        )
}

/// The cutting moves of `tp` ([`is_cut`]).
fn cutting_moves(tp: &Toolpath) -> impl Iterator<Item = &Move> {
    tp.moves.iter().filter(|m| is_cut(m))
}

/// A move target at the finest post precision.
type QPoint = [i64; 3];

fn q(p: P3) -> QPoint {
    let d = finest_decimals().xyz;
    [quantize(p.x, d), quantize(p.y, d), quantize(p.z, d)]
}

/// The inverse of [`q`].
fn unq(p: QPoint) -> P3 {
    let scale = 10f64.powi(i32::try_from(finest_decimals().xyz).unwrap());
    P3::new(
        p[0] as f64 / scale,
        p[1] as f64 / scale,
        p[2] as f64 / scale,
    )
}

/// A multiset of move targets.
type Targets = BTreeMap<QPoint, usize>;

fn targets<'a>(moves: impl Iterator<Item = &'a Move>) -> Targets {
    let mut set = BTreeMap::new();
    for m in moves {
        *set.entry(q(m.target)).or_insert(0) += 1;
    }
    set
}

/// The targets of the cutting moves.
fn cut_targets(tp: &Toolpath) -> Targets {
    targets(cutting_moves(tp))
}

/// The targets of every feed move (any intent).
fn feed_targets(tp: &Toolpath) -> Targets {
    targets(tp.moves.iter().filter(|m| m.move_type != MoveType::Rapid))
}

/// The summed length (mm) of the cutting moves ([`is_cut`]). An arc counts
/// as its chord; both sides of every comparison count arcs the same way.
/// Entry descents are left out on purpose: a rest pass descends through
/// less air (the entry split reads the snapshot), so a feed total would
/// shrink even when the cut does not.
fn cut_length(tp: &Toolpath) -> f64 {
    let mut prev: Option<P3> = None;
    let mut total = 0.0;
    for m in &tp.moves {
        if is_cut(m)
            && let Some(p) = prev
        {
            total += (m.target - p).norm();
        }
        prev = Some(m.target);
    }
    total
}

/// True when `a` holds every element of `b` at least as often.
fn contains_all(a: &Targets, b: &Targets) -> bool {
    b.iter().all(|(k, n)| a.get(k).is_some_and(|m| m >= n))
}

// ── The cached arms ─────────────────────────────────────────────────────

/// The per-op arms that several checks read, generated once.
struct Arms {
    fresh: Outcome,
    flipped: Outcome,
    nothing: Outcome,
}

fn arms() -> &'static BTreeMap<&'static str, Arms> {
    static ARMS: OnceLock<BTreeMap<&'static str, Arms>> = OnceLock::new();
    ARMS.get_or_init(|| {
        OperationType::ALL
            .par_iter()
            .map(|&op| {
                (
                    op.kind_str(),
                    Arms {
                        fresh: generate_alone(op, Frame::TopUp, Fixture::Standard),
                        flipped: generate_alone(op, Frame::Flipped, Fixture::Standard),
                        nothing: generate_alone(op, Frame::TopUp, Fixture::NothingToCut),
                    },
                )
            })
            .collect()
    })
}

// ── 1. Cancel first (the session-door row) ──────────────────────────────

/// With the flag pre-set and a tool the row refuses (when it refuses any),
/// `generate_toolpath` reports the dispatch's cancel text and adopts no
/// result. The dispatch-level claims stay in `compute/execute/tests.rs`
/// (module doc, check 1).
#[test]
fn check1_a_preset_cancel_reaches_the_session_door_before_a_tool_refusal() {
    let cancelled = OperationError::Cancelled.to_string();
    let mut with_refused_tool = 0;
    for &op in OperationType::ALL {
        let key = op.kind_str();
        let refused = CutterKind::ALL
            .iter()
            .copied()
            .find(|&k| op.tool_refusal(k).is_some());
        if refused.is_some() {
            with_refused_tool += 1;
        }
        let tool = refused.map(|k| ToolConfig::new_default(ToolId(0), k.tool_type()));
        let (mut session, index) = case_session(CaseSpec {
            tool,
            ..spec(key, op)
        });
        let cancel = AtomicBool::new(true);
        match session.generate_toolpath(index, &cancel) {
            Err(SessionError::Operation(OperationError::Cancelled)) => {}
            Ok(_) => panic!("{key}/cancel: a pre-set cancel was ignored"),
            Err(other) => panic!(
                "{key}/cancel: expected the cancel ({cancelled:?}) ahead of every refusal, \
                 got {other:?}"
            ),
        }
        assert!(
            session.get_result(index).is_none(),
            "{key}/cancel: a cancelled generation adopted a result"
        );

        // A rest op with no snapshot: the cancel comes before the session
        // door's own rest precondition too.
        let (mut session, index) = case_session(CaseSpec {
            stock_source: StockSource::FromRemainingStock,
            ..spec(key, op)
        });
        match session.generate_toolpath(index, &cancel) {
            Err(SessionError::Operation(OperationError::Cancelled)) => {}
            other => panic!(
                "{key}/cancel_rest: expected the cancel ({cancelled:?}) ahead of the \
                 snapshot refusal, got {:?}",
                other.map(|_| ())
            ),
        }
    }
    assert!(
        with_refused_tool > 0,
        "no row refuses a tool; the tool half of this check proves nothing"
    );
}

// ── 2. Rest ─────────────────────────────────────────────────────────────

/// One op after the rest-arm prior op, as Fresh and as rest.
struct RestPair {
    fresh: Outcome,
    rest: Outcome,
    /// The stock after the prior op.
    prior_stock: Arc<TriDexelStock>,
    /// The stock after the prior op and the op, for each source. `None`
    /// for an op whose rule is `RestUse::None` (no claim reads it), or when
    /// the op gave no toolpath.
    fresh_final: Option<Arc<TriDexelStock>>,
    rest_final: Option<Arc<TriDexelStock>>,
}

fn last_stock(session: &ProjectSession) -> Arc<TriDexelStock> {
    session
        .simulation_result()
        .and_then(|sim| sim.checkpoints.last().map(|c| Arc::clone(&c.mesh_stock)))
        .expect("the simulation holds a checkpoint")
}

/// Generate the prior op, simulate, generate the op with `source`, and
/// simulate again when `carve` is set and the op gave a toolpath.
fn after_prior(
    op: OperationType,
    source: StockSource,
    carve: bool,
) -> (Outcome, Arc<TriDexelStock>, Option<Arc<TriDexelStock>>) {
    let (mut session, index) = case_session(CaseSpec {
        prior: Prior::RestPrior,
        stock_source: source,
        ..spec(op.kind_str(), op)
    });
    run_prior(&mut session);
    let prior_stock = last_stock(&session);
    let outcome = generate(&mut session, index);
    let final_stock = (carve && outcome.toolpath().is_some()).then(|| {
        simulate(&mut session);
        last_stock(&session)
    });
    (outcome, prior_stock, final_stock)
}

fn rest_pairs() -> &'static BTreeMap<&'static str, RestPair> {
    static PAIRS: OnceLock<BTreeMap<&'static str, RestPair>> = OnceLock::new();
    PAIRS.get_or_init(|| {
        OperationType::ALL
            .par_iter()
            .map(|&op| {
                let carve = op.capabilities().rest_use != RestUse::None;
                let (fresh, prior_stock, fresh_final) = after_prior(op, StockSource::Fresh, carve);
                let (rest, _, rest_final) = after_prior(op, StockSource::FromRemainingStock, carve);
                (
                    op.kind_str(),
                    RestPair {
                        fresh,
                        rest,
                        prior_stock,
                        fresh_final,
                        rest_final,
                    },
                )
            })
            .collect()
    })
}

/// The three stocks of one rest pair on one grid, with the floor (mm) that
/// an empty column reads as: the stock bottom.
struct Stocks<'a> {
    prior: &'a TriDexelStock,
    fresh: &'a TriDexelStock,
    rest: &'a TriDexelStock,
    floor: f32,
}

impl Stocks<'_> {
    fn top(&self, s: &TriDexelStock, r: usize, c: usize) -> f32 {
        s.z_grid.top_z_at(r, c).unwrap_or(self.floor)
    }

    /// True when `s` lowered the cell below the prior stock.
    fn lowered(&self, s: &TriDexelStock, r: usize, c: usize) -> bool {
        self.top(s, r, c) < self.top(self.prior, r, c) - threshold()
    }

    /// True when `s` lowered the cell and every cell within
    /// [`EDGE_CELLS`] of it: the cell lies inside the cut, off its edge.
    fn inside_cut(&self, s: &TriDexelStock, r: usize, c: usize) -> bool {
        let g = &self.fresh.z_grid;
        let k = EDGE_CELLS as isize;
        (-k..=k).all(|dr| {
            (-k..=k).all(|dc| {
                let (rr, cc) = (r as isize + dr, c as isize + dc);
                rr >= 0
                    && cc >= 0
                    && (rr as usize) < g.rows
                    && (cc as usize) < g.cols
                    && self.lowered(s, rr as usize, cc as usize)
            })
        })
    }

    fn cells(&self) -> impl Iterator<Item = (usize, usize)> {
        let g = &self.fresh.z_grid;
        let cols = g.cols;
        (0..g.rows).flat_map(move |r| (0..cols).map(move |c| (r, c)))
    }

    /// The volume (mm³) `s` removed from the prior stock.
    fn removed(&self, s: &TriDexelStock) -> f64 {
        let g = &self.fresh.z_grid;
        let v: f64 = self
            .cells()
            .map(|(r, c)| f64::from((self.top(self.prior, r, c) - self.top(s, r, c)).max(0.0)))
            .sum();
        v * g.cell_size * g.cell_size
    }

    /// The count of cells the fresh op lowered.
    fn fresh_reach_cells(&self) -> usize {
        self.cells()
            .filter(|&(r, c)| self.lowered(self.fresh, r, c))
            .count()
    }

    /// The geometric stock claim of `ContourFilter` (W6.1). A cell counts
    /// where the Fresh tool covers it completely: its centre lies within
    /// `R - cell` of a Fresh cutting move, sampled at half a cell (arcs
    /// too). The lowest such move is the depth the Fresh cut takes the
    /// cell to. Gives the most material (mm) the rest stock holds above
    /// `max(fresh top, that depth)`, and the count of cells judged.
    fn worst_left_under_cover(&self, fresh: &Toolpath, tool_radius: f64) -> (f32, usize) {
        let g = &self.fresh.z_grid;
        let cell = g.cell_size;
        let reach = tool_radius - cell;
        let span = (reach / cell).ceil() as isize + 1;
        let mut cover = vec![f64::INFINITY; g.rows * g.cols];
        let mut mark = |p: P3| {
            let col = ((p.x - g.origin_u) / cell).round() as isize;
            let row = ((p.y - g.origin_v) / cell).round() as isize;
            for rr in row - span..=row + span {
                for cc in col - span..=col + span {
                    if rr < 0 || cc < 0 || rr as usize >= g.rows || cc as usize >= g.cols {
                        continue;
                    }
                    let (x, y) = g.cell_to_world(rr as usize, cc as usize);
                    if (x - p.x).hypot(y - p.y) <= reach {
                        let i = rr as usize * g.cols + cc as usize;
                        cover[i] = cover[i].min(p.z);
                    }
                }
            }
        };
        let step = cell / 2.0;
        let mut prev: Option<P3> = None;
        for m in &fresh.moves {
            if is_cut(m)
                && let Some(a) = prev
            {
                let points = match m.move_type {
                    MoveType::ArcCW { i, j, .. } => linearize_arc(a, m.target, i, j, true, step),
                    MoveType::ArcCCW { i, j, .. } => linearize_arc(a, m.target, i, j, false, step),
                    MoveType::Linear { .. } | MoveType::Rapid => {
                        let n = ((m.target - a).norm() / step).ceil().max(1.0) as usize;
                        (0..=n)
                            .map(|k| a + (m.target - a) * (k as f64 / n as f64))
                            .collect()
                    }
                };
                for p in points {
                    mark(p);
                }
            }
            prev = Some(m.target);
        }
        let mut worst: f32 = 0.0;
        let mut judged = 0;
        for (r, c) in self.cells() {
            let z = cover[r * g.cols + c];
            if !z.is_finite() {
                continue;
            }
            judged += 1;
            let floor = self.top(self.fresh, r, c).max(z as f32);
            worst = worst.max(self.top(self.rest, r, c) - floor);
        }
        (worst, judged)
    }

    /// Inside the cut of `reach` (off its edge band): the most material
    /// (mm) the rest stock holds above the fresh stock, and the count of
    /// cells the claim judged.
    fn worst_left_inside(&self, reach: &TriDexelStock) -> (f32, usize) {
        let mut worst: f32 = 0.0;
        let mut judged = 0;
        for (r, c) in self.cells() {
            if self.inside_cut(reach, r, c) {
                judged += 1;
                worst = worst.max(self.top(self.rest, r, c) - self.top(self.fresh, r, c));
            }
        }
        (worst, judged)
    }
}

fn stocks(op: OperationType, pair: &RestPair) -> Option<Stocks<'_>> {
    Some(Stocks {
        prior: &pair.prior_stock,
        fresh: pair.fresh_final.as_deref()?,
        rest: pair.rest_final.as_deref()?,
        floor: fixture_stock(op).origin_z as f32,
    })
}

/// The rest rule of every op, from `capabilities().rest_use`. A prior op
/// clears part of the fixture (the left half of the outline to 3 mm on the
/// flat fixture, a default 3D Rough on the mesh fixture). The op then runs
/// once as Fresh and once as rest after it, in two sessions.
///
/// - `LevelClip`, `LevelMask`, `StockColumn`: inside the Fresh cut (off its
///   edge band) the rest stock stands no higher than the Fresh stock plus
///   the simulator's material threshold; and the rest op cuts less (the
///   length of its cutting moves) than the Fresh op, because the prior op
///   cleared part of its region.
/// - `LevelSkip`: the same stock claim; and the rest cutting moves are a
///   subset of the Fresh cutting moves (a skip never makes a new path).
/// - `ContourFilter` (W6.1): a geometric stock claim
///   ([`Stocks::worst_left_under_cover`]): in each cell the Fresh tool
///   covers completely, the rest stock stands no higher than the Fresh
///   stock (or the depth of that cover) plus the material threshold. The
///   lowered-cell claim does not fit this rule: a split run starts where
///   the Fresh loop does not, and the simulator blends a part-covered cell
///   by its stamp count, so a Fresh plunge and a rest feed through one
///   point leave that cell at different heights (measured 2026-10-15:
///   0.3 to 0.55 mm, with the kept points bit for bit equal). And the rest
///   op cuts no more (the length of its cutting moves) than the Fresh op:
///   a filter only drops runs. The arc fit fits a split run again, so the
///   moves themselves are not compared.
/// - `Planner`: the stock claim inside the REST cut (the planner plans its
///   own paths, so the Fresh cut is no reference); and it cuts less.
/// - `LinkSafetyOnly`: the cutting moves are the Fresh cutting moves. Only
///   links and entries may differ (that is the rule's contract). No stock
///   claim: a Fresh link that feeds along the surface cleans a cusp that a
///   rest link, lifted over the stock, leaves (Scallop: 0.31 mm in 2 cells,
///   measured 2026-10-15).
/// - `Reference`: no claim beyond check 2b. The op reads the stock as its
///   rest reference, so its cut differs from the Fresh cut by design.
///   Measured 2026-10-15 inside the rest cut: Pencil leaves 0.09 mm and
///   Unified Finish 0.098 mm above the Fresh stock.
/// - `None`: the op does not read the remaining stock. Every rest cutting
///   move is a Fresh cutting move, and each Fresh cutting move that the rest
///   output lacks stands in air in the prior stock (the session's air-cut
///   filter dropped it). The whole toolpath is not compared: the dressups
///   read the snapshot for every op.
///
/// Non-vacuity: each rule the registry uses (other than `None` and
/// `Reference`) has an op whose Fresh cut lowers some cells.
#[test]
// SAFETY: the W6.1 `ContourFilter` measurement (leftover, time, rapids) is
// printed as the record of the arm.
#[allow(clippy::print_stderr)]
fn check2_rest_rule_per_op() {
    let mut reached: BTreeMap<String, usize> = BTreeMap::new();
    let mut defects = Vec::new();
    for &op in OperationType::ALL {
        let key = op.kind_str();
        let pair = &rest_pairs()[key];
        let rule = op.capabilities().rest_use;
        let (Some(fresh), Some(rest)) = (pair.fresh.toolpath(), pair.rest.toolpath()) else {
            defects.push(format!(
                "{key}/rest: the pair must generate; Fresh {}, rest {}",
                pair.fresh.describe(),
                pair.rest.describe()
            ));
            continue;
        };
        let (fl, rl) = (cut_length(fresh), cut_length(rest));
        let cuts_less = |defects: &mut Vec<String>| {
            if rl >= fl {
                defects.push(format!(
                    "{key}/rest ({rule:?}): the rest op cuts {rl:.2} mm, the Fresh op \
                     {fl:.2} mm. The prior op cleared part of the region, so the rule must \
                     save cutting motion."
                ));
            }
        };
        let mut stock_left = |defects: &mut Vec<String>, reach_rest: bool| {
            let Some(s) = stocks(op, pair) else {
                defects.push(format!("{key}/rest ({rule:?}): no final stock"));
                return;
            };
            let reach = if reach_rest { s.rest } else { s.fresh };
            let (worst, judged) = s.worst_left_inside(reach);
            if worst > threshold() {
                defects.push(format!(
                    "{key}/rest ({rule:?}): inside the {} cut ({judged} cells) the rest \
                     stock stands {worst:.3} mm over the Fresh stock; the bound is {:.3} mm",
                    if reach_rest { "rest" } else { "Fresh" },
                    threshold()
                ));
            }
            *reached.entry(format!("{rule:?}")).or_insert(0) += s.fresh_reach_cells();
        };
        match rule {
            RestUse::LevelClip | RestUse::LevelMask | RestUse::StockColumn => {
                stock_left(&mut defects, false);
                cuts_less(&mut defects);
            }
            RestUse::LevelSkip => {
                stock_left(&mut defects, false);
                if !contains_all(&cut_targets(fresh), &cut_targets(rest)) {
                    defects.push(format!(
                        "{key}/rest (LevelSkip): the rest op cuts a move the Fresh op does not"
                    ));
                }
            }
            RestUse::ContourFilter => {
                match stocks(op, pair) {
                    None => defects.push(format!("{key}/rest (ContourFilter): no final stock")),
                    Some(s) => {
                        let r = tool_for(op).diameter / 2.0;
                        // The record of W6.1: the lowered-cell leftover,
                        // and the time and rapids of each arm on the one
                        // cycle-time model (`MachineProfile::link_kinematics`).
                        let (lowered_worst, lowered_judged) = s.worst_left_inside(s.fresh);
                        let lk = rs_cam_core::machine::MachineProfile::default().link_kinematics();
                        let time = |tp: &Toolpath| {
                            compute_cycle_time(
                                tp,
                                &lk.kinematics,
                                lk.max_feed_mm_min,
                                lk.rapid_feed_mm_min,
                            )
                        };
                        let rapids = |tp: &Toolpath| {
                            tp.moves
                                .iter()
                                .filter(|m| m.move_type == MoveType::Rapid)
                                .count()
                        };
                        eprintln!(
                            "{key}/rest (ContourFilter): lowered-cell leftover {lowered_worst:.3} mm \
                             over {lowered_judged} cells; time fresh {:.2} s, rest {:.2} s; \
                             rapids fresh {} ({:.1} mm), rest {} ({:.1} mm)",
                            time(fresh),
                            time(rest),
                            rapids(fresh),
                            fresh.total_rapid_distance(),
                            rapids(rest),
                            rest.total_rapid_distance()
                        );
                        let (worst, judged) = s.worst_left_under_cover(fresh, r);
                        if judged == 0 || worst > threshold() {
                            defects.push(format!(
                                "{key}/rest (ContourFilter): under the Fresh tool's full cover \
                                 ({judged} cells) the rest stock stands {worst:.3} mm over the \
                                 Fresh cut; the bound is {:.3} mm",
                                threshold()
                            ));
                        }
                        *reached.entry(format!("{rule:?}")).or_insert(0) += s.fresh_reach_cells();
                    }
                }
                if rl > fl + CUT_LENGTH_EPS_MM {
                    defects.push(format!(
                        "{key}/rest (ContourFilter): the rest op cuts {rl:.2} mm, the Fresh \
                         op {fl:.2} mm. A contour filter only drops runs."
                    ));
                }
            }
            RestUse::Planner => {
                stock_left(&mut defects, true);
                cuts_less(&mut defects);
            }
            RestUse::LinkSafetyOnly => {
                if cut_targets(fresh) != cut_targets(rest) {
                    defects.push(format!(
                        "{key}/rest (LinkSafetyOnly): the rest cutting moves differ from \
                         the Fresh cutting moves"
                    ));
                }
                *reached.entry(format!("{rule:?}")).or_insert(0) += 1;
            }
            RestUse::Reference => {}
            RestUse::None => {
                let (f, r) = (cut_targets(fresh), cut_targets(rest));
                if !contains_all(&f, &r) {
                    defects.push(format!(
                        "{key}/rest (None): the rest op cuts a move the Fresh op does not"
                    ));
                }
                let prior = &pair.prior_stock;
                for (point, n) in &f {
                    if r.get(point).copied().unwrap_or(0) >= *n {
                        continue;
                    }
                    let p = unq(*point);
                    let top = prior
                        .z_grid
                        .world_to_cell(p.x, p.y)
                        .and_then(|(row, col)| prior.z_grid.top_z_at(row, col));
                    if top.is_some_and(|t| f64::from(t) > p.z + FRESH_MATERIAL_THRESHOLD_MM) {
                        defects.push(format!(
                            "{key}/rest (None): the rest op dropped the cut at {p:?}, where \
                             the prior stock stands at {top:?}. An op whose rule is None must \
                             not read the remaining stock."
                        ));
                    }
                }
            }
        }
    }
    assert!(defects.is_empty(), "{}", defects.join("\n"));
    for rule in OperationType::ALL
        .iter()
        .map(|op| op.capabilities().rest_use)
        .filter(|r| !matches!(r, RestUse::None | RestUse::Reference))
    {
        assert!(
            reached.get(&format!("{rule:?}")).copied().unwrap_or(0) > 0,
            "no op with the rule {rule:?} cut anything on this fixture; its claims prove \
             nothing: {reached:?}"
        );
    }
}

/// The volume claim of every op whose rule is not `None`: after the same
/// prior op, the rest op removes no more material than the Fresh op. The
/// slack is the simulator's material threshold over each cell the Fresh op
/// lowered.
///
/// W4 finding, 2026-10-15 (G-RAMPCHORD, fixed): the rest Waterline removed
/// 356.6 mm³ against 156.5 mm³ and cut up to 3.1 mm under the cap surface
/// in 493 cells. Its stock-aware entry ramp lapped along the CHORD of an arc
/// move, because `upcoming_run` (`dressup/entry_descent.rs`) built the run
/// from the move targets alone. The run now samples each arc; the sentry is
/// `an_entry_ramp_follows_the_arc_g_rampchord`.
#[test]
fn check2b_a_rest_op_removes_no_more_material_than_a_fresh_op() {
    let mut judged = 0;
    let mut red = Vec::new();
    for &op in OperationType::ALL {
        let key = op.kind_str();
        let pair = &rest_pairs()[key];
        let Some(s) = stocks(op, pair) else {
            continue;
        };
        judged += 1;
        let (vf, vr) = (s.removed(s.fresh), s.removed(s.rest));
        let g = &s.fresh.z_grid;
        let slack =
            s.fresh_reach_cells() as f64 * g.cell_size * g.cell_size * FRESH_MATERIAL_THRESHOLD_MM;
        if vr > vf + slack {
            red.push(format!(
                "{key}/rest_volume ({:?}): rest {vr:.1} mm³ > Fresh {vf:.1} mm³ + {slack:.1}",
                op.capabilities().rest_use
            ));
        }
    }
    assert!(
        judged > 0,
        "no op has a rest rule; the claim proves nothing"
    );
    assert!(red.is_empty(), "{}", red.join("\n"));
}

// ── 3. Empty ────────────────────────────────────────────────────────────

/// The class of one generation, or the defect it shows.
///
/// - Done with a feed move: `done`.
/// - Done with no feed move: legal only when the row exempts an empty
///   result (`empty_is_feature_selective`), the op is a drill cycle, or the
///   arm reads the remaining stock (`compute/generated_empty.rs`: a rest
///   pass may find nothing left). The session's G-ENTRYEMPTY gate refuses
///   every other empty result.
/// - `GeneratedEmpty`: legal only when none of those exemptions holds.
/// - Any other refusal must be a NAMED one: a typed `SessionError`
///   variant, or `SessionError::Operation` with a named
///   `OperationError` (missing geometry, invalid tool, invalid parameter),
///   or a selection refusal (`SessionError::Selection`). Free text
///   (`OperationFailed`) and a wiring defect
///   (`OperationError::Internal`) are unnamed errors. F054: the test
///   matches the variant, not the message.
/// - A panic is always a defect.
fn classify(op: OperationType, rest: bool, outcome: &Outcome) -> Result<&'static str, String> {
    let policy = op.registry_entry().policy;
    let exempt = policy.empty_is_feature_selective;
    match outcome {
        Outcome::Done(at) => {
            if at
                .toolpath
                .moves
                .iter()
                .any(|m| m.move_type != MoveType::Rapid)
            {
                Ok("done")
            } else if exempt || rest || policy.kinematics == Kinematics::DrillZOnly {
                Ok("done-empty")
            } else {
                Err(
                    "done with no feed move, and the row does not exempt an empty result"
                        .to_owned(),
                )
            }
        }
        Outcome::Refused(SessionError::GeneratedEmpty(text)) => {
            if exempt || rest {
                Err(format!(
                    "refused as empty, and the row exempts an empty result: {text}"
                ))
            } else {
                Ok("refused-empty")
            }
        }
        // A selection refusal is `SessionError::Selection` (geometry
        // selection P2.4), a typed refusal: the last refusal arm.
        Outcome::Refused(SessionError::Operation(error)) => match error {
            OperationError::MissingGeometry(_)
            | OperationError::InvalidTool(_)
            | OperationError::InvalidParam(_) => Ok("named-refusal"),
            other => Err(format!("an unnamed error: {other}")),
        },
        Outcome::Refused(SessionError::OperationFailed(text)) => {
            Err(format!("an unnamed error: {text}"))
        }
        Outcome::Refused(_) => Ok("named-refusal"),
        Outcome::Panicked(p) => Err(format!("panicked: {p}")),
    }
}

#[test]
fn check3_an_empty_generation_is_done_empty_or_a_named_refusal() {
    let mut classes: BTreeMap<&str, usize> = BTreeMap::new();
    let mut defects = Vec::new();
    for &op in OperationType::ALL {
        let key = op.kind_str();
        let a = &arms()[key];
        let p = &rest_pairs()[key];
        for (arm, rest, outcome) in [
            ("fresh", false, &a.fresh),
            ("flipped", false, &a.flipped),
            ("nothing", false, &a.nothing),
            ("after_prior_fresh", false, &p.fresh),
            ("after_prior_rest", true, &p.rest),
        ] {
            match classify(op, rest, outcome) {
                Ok(class) => *classes.entry(class).or_insert(0) += 1,
                Err(why) => defects.push(format!("{key}/{arm}/empty: {why}")),
            }
        }
    }
    assert!(defects.is_empty(), "{}", defects.join("\n"));
    // Non-vacuity: the nothing-to-cut fixture made at least one gated op
    // refuse, and at least one exempt op succeed empty.
    assert!(
        classes.get("refused-empty").copied().unwrap_or(0) > 0,
        "no gated op refused an empty result: {classes:?}"
    );
    assert!(
        classes.get("done-empty").copied().unwrap_or(0) > 0,
        "no exempt op gave an empty result: {classes:?}"
    );
}

// ── 4. Flipped frame ────────────────────────────────────────────────────

/// The top-up toolpath moved by `-origin`: what a frame-correct op emits on
/// the flipped setup (`common::op_fixtures`, "The flipped frame").
fn moved_by_minus_origin(tp: &Toolpath, stock: &StockConfig) -> Toolpath {
    let mut moved = tp.clone();
    for m in &mut moved.moves {
        m.target = P3::new(
            m.target.x - stock.origin_x,
            m.target.y - stock.origin_y,
            m.target.z - stock.origin_z,
        );
    }
    moved
}

/// `[min, max]` of the feed-move targets, per axis.
fn feed_bbox(tp: &Toolpath) -> [[f64; 3]; 2] {
    let mut b = [[f64::INFINITY; 3], [f64::NEG_INFINITY; 3]];
    for m in tp.moves.iter().filter(|m| m.move_type != MoveType::Rapid) {
        for (axis, v) in [m.target.x, m.target.y, m.target.z].into_iter().enumerate() {
            b[0][axis] = b[0][axis].min(v);
            b[1][axis] = b[1][axis].max(v);
        }
    }
    b
}

/// Every op on a `FaceUp::Bottom` setup over mirrored inputs.
///
/// 1. Inside the stock of that frame: every feed-move target lies within
///    the stock footprint grown by the tool radius and no lower than the
///    stock bottom less the tool radius; every cutting move lies no higher
///    than the stock top. (A link or a drill approach feeds through air
///    above the top.)
/// 2. The mirror: the feed-move targets are the top-up feed-move targets
///    moved by `-origin`, exactly at the post precision. Rapids are not
///    compared: the default retract is an absolute Z, so its clearance over
///    the stock top differs between the frames. A planner whose plan moves
///    under a float-level change of its input (2D Adaptive, 3D Rough,
///    Pencil) cannot give the exact set. For such an op the claim falls back
///    to the feed bounding box: each face of the box moves by no more than
///    the tool radius. The smallest frame defect on record (G-PROFILE-FLIP,
///    an offset to the wrong side) moves a face by twice the radius.
#[test]
fn check4_a_flipped_setup_cuts_inside_the_stock_and_mirrors_the_top_up_cut() {
    let mut exact = 0;
    let mut fallback = Vec::new();
    let mut defects = Vec::new();
    let tol = 10f64.powi(-i32::try_from(finest_decimals().xyz).unwrap());
    for &op in OperationType::ALL {
        let key = op.kind_str();
        let a = &arms()[key];
        let (Some(top), Some(flip)) = (a.fresh.toolpath(), a.flipped.toolpath()) else {
            defects.push(format!(
                "{key}/flipped: both frames must generate; top-up {}, flipped {}",
                a.fresh.describe(),
                a.flipped.describe()
            ));
            continue;
        };
        let stock = fixture_stock(op);
        let r = tool_for(op).diameter / 2.0;
        if let Some(m) = flip.moves.iter().find(|m| {
            let p = m.target;
            let cut = matches!(m.intent, MoveIntent::ClearingCut | MoveIntent::FinishingCut);
            m.move_type != MoveType::Rapid
                && !(p.x >= -r - tol
                    && p.x <= stock.x + r + tol
                    && p.y >= -r - tol
                    && p.y <= stock.y + r + tol
                    && (!cut || p.z <= stock.z + tol)
                    && p.z >= -r - tol)
        }) {
            defects.push(format!(
                "{key}/flipped: a feed move to {:?} lies outside the stock [0, {}] x [0, {}] \
                 x [0, {}] grown by the tool radius {r}",
                m.target, stock.x, stock.y, stock.z
            ));
        }
        let moved = moved_by_minus_origin(top, &stock);
        if feed_targets(&moved) == feed_targets(flip) {
            exact += 1;
            continue;
        }
        let (bm, bf) = (feed_bbox(&moved), feed_bbox(flip));
        let worst = (0..2)
            .flat_map(|side| (0..3).map(move |axis| (side, axis)))
            .map(|(side, axis)| (bm[side][axis] - bf[side][axis]).abs())
            .fold(0.0_f64, f64::max);
        fallback.push(format!("{key} ({worst:.3} mm)"));
        if worst > r {
            defects.push(format!(
                "{key}/flipped: the feed box moves {worst:.3} mm against the top-up box \
                 moved by -origin; the bound is the tool radius {r} mm. Top-up {bm:?}, \
                 flipped {bf:?}"
            ));
        }
    }
    assert!(defects.is_empty(), "{}", defects.join("\n"));
    assert!(
        exact > fallback.len(),
        "most ops must mirror exactly; the bounding-box fallback took {fallback:?}"
    );
}

// ── 5. Determinism ──────────────────────────────────────────────────────

/// Two runs on the default pool, and one run of every op in turn on a
/// one-thread pool, give the same G-OPGOLDEN fingerprint.
#[test]
fn check5_two_runs_and_one_thread_give_the_same_toolpath() {
    let single = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .expect("a one-thread pool");
    let one: BTreeMap<&str, Golden> = single.install(|| {
        OperationType::ALL
            .iter()
            .map(|&op| {
                (
                    op.kind_str(),
                    generate_alone(op, Frame::TopUp, Fixture::Standard).golden(),
                )
            })
            .collect()
    });
    let again: BTreeMap<&str, Golden> = OperationType::ALL
        .par_iter()
        .map(|&op| {
            (
                op.kind_str(),
                generate_alone(op, Frame::TopUp, Fixture::Standard).golden(),
            )
        })
        .collect();
    let mut defects = Vec::new();
    for &op in OperationType::ALL {
        let key = op.kind_str();
        let first = arms()[key].fresh.golden();
        if first != again[key] {
            defects.push(format!(
                "{key}/determinism: two runs differ:\n  {first:?}\n  {:?}",
                again[key]
            ));
        }
        if first != one[key] {
            defects.push(format!(
                "{key}/determinism: one thread differs from the default pool:\n  {first:?}\n  \
                 {:?}",
                one[key]
            ));
        }
    }
    assert!(defects.is_empty(), "{}", defects.join("\n"));
}

// ── 6. Schema round trip ────────────────────────────────────────────────

/// A session that holds one default config of `op` and nothing else: the
/// setter needs no geometry.
fn default_op_session(op: OperationType) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new();
    let tool_idx = builder.add_tool(tool_for(op));
    let tool_id = builder.tools()[tool_idx].id.0;
    let _ = builder
        .add_toolpath(
            0,
            toolpath_config(op.kind_str(), OperationConfig::new_default(op), tool_id, 0),
        )
        .expect("add the op");
    builder.build()
}

fn set_param(
    session: &mut ProjectSession,
    name: &str,
    value: serde_json::Value,
) -> Result<(), SessionError> {
    session
        .apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index: 0,
            param: name.to_owned(),
            value,
        }))
        .map(|_| ())
}

/// For every op, on a default config:
///
/// - each required (not optional) published param names a field the
///   serialized config holds, so a write to it writes something;
/// - each published param and each alias whose published default is not
///   null, set through `Command::SetToolpathParam` to that default, is
///   accepted and leaves the operation config unchanged (a null default is
///   check 6b);
/// - each toolpath-level param (`debug_enabled`), set to the value the
///   toolpath holds, is accepted and changes nothing. The schema publishes no
///   default for it (null), so the config's own value is the round trip;
/// - each driver-set field of the op (`ParamTable::driver_set`), set to the value the config
///   holds, is refused with the row's reason and changes nothing.
#[test]
fn check6_every_schema_default_round_trips_and_every_driver_field_is_refused() {
    let mut defects = Vec::new();
    let mut set_count = 0;
    let mut driver_count = 0;
    // The operation config only: a feed setter also stamps the toolpath's
    // feeds provenance (`Manual`), which is the setter's job.
    let config = |s: &ProjectSession| format!("{:?}", s.toolpath_configs()[0].operation);
    for &op in OperationType::ALL {
        let key = op.kind_str();
        let schema = OperationConfig::schema_for_type(op);
        let mut session = default_op_session(op);
        let before = config(&session);
        let held = serde_json::to_value(&session.toolpath_configs()[0].operation)
            .expect("an op serializes");
        for p in schema.params.iter().filter(|p| !p.optional) {
            if held.pointer(&format!("/params/{}", p.name)).is_none() {
                defects.push(format!(
                    "{key}/schema: the required param `{}` names no field the config holds; \
                     a write to it writes nothing",
                    p.name
                ));
            }
        }
        for p in schema.params.iter().filter(|p| !p.default.is_null()) {
            let name = &p.name;
            set_count += 1;
            match set_param(&mut session, name, p.default.clone()) {
                Err(e) => defects.push(format!(
                    "{key}/schema: `{name}` = {} (the published default) is refused: {e}",
                    p.default
                )),
                Ok(()) if config(&session) != before => {
                    defects.push(format!(
                        "{key}/schema: `{name}` = {} (the published default) changed \
                         the config",
                        p.default
                    ));
                    session = default_op_session(op);
                }
                Ok(()) => {}
            }
        }
        let held_debug = session.toolpath_configs()[0].debug_options.enabled;
        let debug = |s: &ProjectSession| s.toolpath_configs()[0].debug_options.enabled;
        for p in &schema.toolpath_params {
            set_count += 1;
            let value = match p.name.as_str() {
                "debug_enabled" => serde_json::Value::Bool(held_debug),
                other => {
                    defects.push(format!(
                        "{key}/schema: the toolpath param `{other}` has no round-trip value \
                         in this check; add one"
                    ));
                    continue;
                }
            };
            if let Err(e) = set_param(&mut session, &p.name, value.clone()) {
                defects.push(format!(
                    "{key}/schema: `{}` = {value} is refused: {e}",
                    p.name
                ));
            } else if config(&session) != before || debug(&session) != held_debug {
                defects.push(format!(
                    "{key}/schema: `{}` = {value} changed the config",
                    p.name
                ));
            }
        }
        let op_value =
            serde_json::to_value(&session.toolpath_configs()[0].operation).expect("an op");
        for field in op.registry_entry().params.driver_set {
            driver_count += 1;
            let value = op_value
                .pointer(&format!("/{}", field.name))
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            match set_param(&mut session, field.name, value.clone()) {
                Err(SessionError::InvalidParam(text)) if text.contains(field.reason) => {}
                other => defects.push(format!(
                    "{key}/schema: the driver-set field `{}` = {value} must be refused with \
                     its reason, got {other:?}",
                    field.name
                )),
            }
            if config(&session) != before {
                defects.push(format!(
                    "{key}/schema: the refused driver-set field `{}` changed the config",
                    field.name
                ));
            }
        }
    }
    assert!(set_count > 150, "only {set_count} params were set");
    assert!(driver_count > 0, "no driver-set field was tried");
    assert!(defects.is_empty(), "{}", defects.join("\n"));
}

/// Every published param whose published default is null, set to null
/// through `Command::SetToolpathParam` on a default config, is accepted and
/// leaves the operation config unchanged. A null default says "absent";
/// the setter must take back what the schema hands out.
///
/// W4 finding, 2026-10-15 (fixed): Unified Finish published
/// `classification_sampler` with the default null, but the field was not an
/// `Option`, so the setter refused null. The field is now
/// `Option<ClassificationSampler>`: null is "not pinned" and runs
/// `ClassificationSampler::PRODUCTION`.
#[test]
fn check6b_every_null_schema_default_round_trips() {
    let mut defects = Vec::new();
    let mut set_count = 0;
    for &op in OperationType::ALL {
        let key = op.kind_str();
        let schema = OperationConfig::schema_for_type(op);
        for p in schema.params.iter().filter(|p| p.default.is_null()) {
            let name = &p.name;
            set_count += 1;
            let mut session = default_op_session(op);
            let before = format!("{:?}", session.toolpath_configs()[0].operation);
            match set_param(&mut session, name, serde_json::Value::Null) {
                Err(e) => defects.push(format!(
                    "{key}/schema_null: `{name}` = null (the published default) is \
                     refused: {e}"
                )),
                Ok(()) if format!("{:?}", session.toolpath_configs()[0].operation) != before => {
                    defects.push(format!(
                        "{key}/schema_null: `{name}` = null (the published default) \
                         changed the config"
                    ));
                }
                Ok(()) => {}
            }
        }
    }
    assert!(
        set_count > 0,
        "no published default is null; the claim proves nothing"
    );
    assert!(defects.is_empty(), "{}", defects.join("\n"));
}

// ── 7. No Unknown intent ────────────────────────────────────────────────

#[test]
fn check7_every_move_of_every_arm_here_has_a_known_intent() {
    let mut judged = 0;
    let mut defects = Vec::new();
    for &op in OperationType::ALL {
        let key = op.kind_str();
        let a = &arms()[key];
        let p = &rest_pairs()[key];
        for (arm, outcome) in [
            ("flipped", &a.flipped),
            ("nothing", &a.nothing),
            ("after_prior_fresh", &p.fresh),
            ("after_prior_rest", &p.rest),
        ] {
            if let Some(tp) = outcome.toolpath() {
                judged += tp.moves.len();
                let n = unknown_intents(tp);
                if n > 0 {
                    defects.push(format!("{key}/{arm}/intent: {n} moves with Unknown intent"));
                }
            }
        }
    }
    assert!(judged > 0, "no move was judged");
    assert!(defects.is_empty(), "{}", defects.join("\n"));
}

// ── 8. Obstacles (K1) ───────────────────────────────────────────────────

/// The strip (mm, world X) and its clearance. It crosses the cap of the mesh
/// fixture and the outline of the flat fixture between the two drill
/// targets, and stays clear of the pin (world X -25).
const STRIP_X: [f64; 2] = [2.0, 6.0];
const STRIP_CLEARANCE_MM: f64 = 1.0;
/// How far (mm) the strip reaches past the stock in Y, below its bottom and
/// above its top.
const STRIP_MARGIN_MM: f64 = 5.0;
/// The sample step (mm) of the analytic cutter test.
const STRIP_SAMPLE_MM: f64 = 0.25;

/// The strip of `stock` in the WORLD frame: `[min, max]`.
fn strip_world(stock: &StockConfig) -> [[f64; 3]; 2] {
    [
        [
            STRIP_X[0],
            stock.origin_y - STRIP_MARGIN_MM,
            stock.origin_z - STRIP_MARGIN_MM,
        ],
        [
            STRIP_X[1],
            stock.origin_y + stock.y + STRIP_MARGIN_MM,
            stock.origin_z + stock.z + STRIP_MARGIN_MM,
        ],
    ]
}

/// The distance (mm) in XY from `(x, y)` to the box `[min, max]`.
fn xy_distance_to_box(x: f64, y: f64, b: &[[f64; 3]; 2]) -> f64 {
    let dx = (b[0][0] - x).max(0.0).max(x - b[1][0]);
    let dy = (b[0][1] - y).max(0.0).max(y - b[1][1]);
    dx.hypot(dy)
}

/// The tip points of `tp` every [`STRIP_SAMPLE_MM`], with each move's index
/// and kind. An arc is linearised.
fn tip_samples(tp: &Toolpath) -> Vec<(usize, bool, P3)> {
    let mut out = Vec::new();
    let mut prev: Option<P3> = None;
    for (i, m) in tp.moves.iter().enumerate() {
        let a = prev.unwrap_or(m.target);
        prev = Some(m.target);
        let points = match m.move_type {
            MoveType::ArcCW { i: ci, j: cj, .. } => {
                linearize_arc(a, m.target, ci, cj, true, STRIP_SAMPLE_MM)
            }
            MoveType::ArcCCW { i: ci, j: cj, .. } => {
                linearize_arc(a, m.target, ci, cj, false, STRIP_SAMPLE_MM)
            }
            MoveType::Rapid | MoveType::Linear { .. } => vec![a, m.target],
        };
        let rapid = m.move_type == MoveType::Rapid;
        for w in points.windows(2) {
            let n = ((w[1] - w[0]).norm() / STRIP_SAMPLE_MM).ceil().max(1.0) as usize;
            for k in 0..=n {
                let t = k as f64 / n as f64;
                out.push((i, rapid, w[0] + (w[1] - w[0]) * t));
            }
        }
    }
    out
}

/// The case session of `op` in `frame` with the strip added to the case
/// op's setup, in that setup's frame. Returns the session, the op index and
/// the strip as the setup frame holds it.
fn strip_session(op: OperationType, frame: Frame) -> (ProjectSession, usize, [[f64; 3]; 2]) {
    let (mut session, index) = case_session(CaseSpec {
        frame,
        ..spec(op.kind_str(), op)
    });
    let setup_index = session
        .list_setups()
        .iter()
        .position(|s| s.toolpath_indices.contains(&index))
        .expect("the case op has a setup");
    let world = strip_world(session.stock_config());
    let setup = &session.list_setups()[setup_index];
    let ctx = SetupEvalContext::build_for_setup(&session, Some(setup));
    // The flipped frame is the zero-rooted setup frame: map the corners.
    let local = match ctx.local_to_global.as_ref() {
        None => world,
        Some(info) => {
            let mut lo = [f64::INFINITY; 3];
            let mut hi = [f64::NEG_INFINITY; 3];
            for x in [world[0][0], world[1][0]] {
                for y in [world[0][1], world[1][1]] {
                    for z in [world[0][2], world[1][2]] {
                        let p = info.world_to_local(P3::new(x, y, z));
                        for (axis, v) in [p.x, p.y, p.z].into_iter().enumerate() {
                            lo[axis] = lo[axis].min(v);
                            hi[axis] = hi[axis].max(v);
                        }
                    }
                }
            }
            [lo, hi]
        }
    };
    let _ = session
        .apply(Command::AddObstacle(AddObstacleArgs {
            setup_index,
            obstacle: Box::new(Obstacle {
                id: rs_cam_core::ids::ObstacleId(0),
                name: "W4 strip".to_owned(),
                enabled: true,
                clearance: STRIP_CLEARANCE_MM,
                geometry: ObstacleGeometry::Box {
                    min: local[0],
                    max: local[1],
                },
            }),
        }))
        .expect("the strip is a valid obstacle");
    (session, index, local)
}

/// K1: for every op, top-up and flipped, with a box strip across the stock
/// whose top is above the stock top:
///
/// - the op generates, with at least one cut move (non-vacuity);
/// - the strip meets the op's FRESH cut (the arm without the strip), so the
///   claim tests a real crossing (non-vacuity), for every op whose fresh cut
///   reaches it;
/// - no feed move puts the cutter inside the grown strip below its grown
///   top, and every rapid over the grown strip is above its grown top. This
///   test is analytic and independent of the service: the box distance
///   against the cutter radius;
/// - the service's own check (cutter, shank and holder) finds no hit;
/// - the simulation reports 0 obstacle collisions.
#[test]
fn check8_no_op_puts_the_tool_inside_an_obstacle() {
    // The shared arms first, OUTSIDE the parallel loop: their `OnceLock`
    // init runs a rayon loop itself, and a pool worker that blocks on the
    // init from inside this loop can deadlock the pool.
    let arms = arms();
    let results: Vec<(String, Vec<String>, bool)> = OperationType::ALL
        .par_iter()
        .flat_map(|&op| [(op, Frame::TopUp), (op, Frame::Flipped)])
        .map(|(op, frame)| {
            let label = format!("{}/{frame:?}", op.kind_str());
            let mut defects = Vec::new();
            let (mut session, index, strip) = strip_session(op, frame);
            let tp = match generate(&mut session, index) {
                Outcome::Done(at) => at.toolpath,
                other => {
                    defects.push(format!("{label}/obstacle: {}", other.describe()));
                    return (label, defects, false);
                }
            };
            if cutting_moves(&tp).next().is_none() {
                defects.push(format!("{label}/obstacle: no cut move is left (vacuous)"));
            }
            let tc = session.get_toolpath_config(index).expect("the case op");
            let tool_cfg = session
                .tools()
                .iter()
                .find(|t| t.id.0 == tc.tool_id)
                .expect("the case tool")
                .clone();
            let tool = rs_cam_core::compute::cutter::build_cutter(&tool_cfg).unwrap();
            let r = tool.to_assembly(&rs_cam_core::machine::head::HeadFit::default()).cutter_radius;
            let grown = [
                strip[0].map(|v| v - STRIP_CLEARANCE_MM),
                strip[1].map(|v| v + STRIP_CLEARANCE_MM),
            ];
            let top = grown[1][2];
            for (i, rapid, p) in tip_samples(&tp) {
                let d = xy_distance_to_box(p.x, p.y, &grown);
                if !rapid && p.z < top - 1e-6 && d < r - 1e-6 {
                    defects.push(format!(
                        "{label}/obstacle: feed move {i} puts the cutter {:.3} mm into the strip \
                         at ({:.3}, {:.3}, {:.3})",
                        r - d,
                        p.x,
                        p.y,
                        p.z
                    ));
                    break;
                }
                if rapid && d < r - 1e-6 && p.z < top - 1e-6 {
                    defects.push(format!(
                        "{label}/obstacle: rapid {i} crosses the strip at z {:.3}, below its top {top:.3}",
                        p.z
                    ));
                    break;
                }
            }
            let set = session
                .obstacle_set_for_toolpath(index)
                .expect("the strip resolves");
            let view = set.for_tool(&tool.to_assembly(&rs_cam_core::machine::head::HeadFit::default()), None).expect("a tool view");
            let join = session.obstacle_join_for_toolpath(index);
            let hits = view.check_toolpath(&tp, join);
            if let Some(hit) = hits.first() {
                defects.push(format!(
                    "{label}/obstacle: the service finds {} hits, first at move {} ({})",
                    hits.len(),
                    hit.move_index,
                    hit.segment
                ));
            }
            // Non-vacuity: the strip meets the fresh cut of this op.
            let crossed = arms[op.kind_str()]
                .fresh
                .toolpath()
                .is_some_and(|fresh| {
                    let fresh_frame = match frame {
                        Frame::TopUp => fresh.clone(),
                        Frame::Flipped => return true,
                    };
                    !view.check_toolpath(&fresh_frame, join).is_empty()
                });
            simulate(&mut session);
            let sim_hits = session
                .simulation_result()
                .map_or(usize::MAX, |sim| sim.obstacle_collisions.len());
            if sim_hits != 0 {
                defects.push(format!(
                    "{label}/obstacle: the simulation reports {sim_hits} obstacle collisions"
                ));
            }
            (label, defects, crossed)
        })
        .collect();
    let defects: Vec<String> = results.iter().flat_map(|(_, d, _)| d.clone()).collect();
    let uncrossed: Vec<&str> = results
        .iter()
        .filter(|(_, _, crossed)| !crossed)
        .map(|(label, _, _)| label.as_str())
        .collect();
    assert!(defects.is_empty(), "{}", defects.join("\n"));
    // The pin drill drills the one pin, at world X -25: the strip leaves it
    // by design (it must not refuse the pin drill as empty). Every other op
    // crosses the strip, so the claim tests a real crossing for each.
    assert_eq!(
        uncrossed,
        [format!(
            "{}/TopUp",
            OperationType::AlignmentPinDrill.kind_str()
        )],
        "the strip must meet the fresh cut of every op but the pin drill"
    );
}

/// The registry is the case list: every op is in `ALL` once, so each check
/// above (which iterates `ALL`) covers every op.
/// Check 9 (module doc): the selection slot follows the item use.
///
/// For each op of the registry, on its default config:
///
/// - an op whose `item_use` is not `None` holds a selection
///   (`OperationConfig::selection`) and states a default
///   (`default_selection`); an op whose `item_use` is `None` has neither;
/// - the default config holds its default selection;
/// - `selection` is a driver-set field (check 6 refuses it), never a
///   published param;
/// - a default selection writes no `selection` key, and a file with no key
///   loads it back.
#[test]
fn check9_an_op_holds_a_selection_exactly_when_it_cuts_items() {
    let mut defects = Vec::new();
    let mut slots = 0;
    for &op in OperationType::ALL {
        let key = op.kind_str();
        let cfg = OperationConfig::new_default(op);
        let cuts_items = match cfg.capabilities().item_use {
            ItemUse::None => false,
            ItemUse::Regions | ItemUse::Curves | ItemUse::Holes { .. } => true,
        };
        let held = cfg.selection();
        let default = cfg.default_selection();
        if held.is_some() != cuts_items || default.is_some() != cuts_items {
            defects.push(format!(
                "{key}/check9: item use {:?}, selection slot {}, default {}",
                cfg.capabilities().item_use,
                held.is_some(),
                default.is_some()
            ));
            continue;
        }
        let in_driver_set = op
            .registry_entry()
            .params
            .driver_set
            .iter()
            .any(|f| f.name == "selection");
        let published = OperationConfig::param_names_for_type(op).contains(&"selection");
        if published || in_driver_set != cuts_items {
            defects.push(format!(
                "{key}/check9: `selection` published {published}, driver-set {in_driver_set}"
            ));
        }
        let Some(held) = held else { continue };
        slots += 1;
        if Some(held) != default.as_ref() {
            defects.push(format!(
                "{key}/check9: the default config holds {held:?}, not its default {default:?}"
            ));
        }
        let value = serde_json::to_value(&cfg).expect("an op serializes");
        if value.pointer("/params/selection").is_some() {
            defects.push(format!("{key}/check9: a default selection wrote a key"));
        }
        let back: OperationConfig = serde_json::from_value(value).expect("an op loads");
        if back.selection() != default.as_ref() {
            defects.push(format!(
                "{key}/check9: a file with no key loaded {:?}",
                back.selection()
            ));
        }
    }
    // P4.2: Face holds a selection (the stock default), so 13 ops do.
    assert!(slots >= 13, "only {slots} ops hold a selection");
    assert!(defects.is_empty(), "{}", defects.join("\n"));
}

#[test]
fn the_case_list_is_the_registry() {
    let kinds: BTreeSet<&str> = OperationType::ALL.iter().map(|op| op.kind_str()).collect();
    assert_eq!(kinds.len(), OperationType::ALL.len(), "a kind repeats");
    assert_eq!(arms().len(), OperationType::ALL.len());
    assert_eq!(rest_pairs().len(), OperationType::ALL.len());
}
