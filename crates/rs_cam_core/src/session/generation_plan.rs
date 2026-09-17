//! The ordered work that makes a scope current: one plan walk for the GUI,
//! the MCP server and the CLI.
//!
//! `PLAN.md` §4.2 of `planning/gen_sim_rest_ux_2026-09-18/`, placed here by
//! `IMPLEMENTATION.md` §1.1. The walk reads the session and
//! [`crate::session::dependencies::edges`], and runs nothing. Three surfaces
//! used to hold their own round-based fixpoint, so they could disagree about
//! what "make this current" means. They now drive the same [`Vec<Step>`].
//!
//! # The order
//!
//! Setup order is plan order. Inside a setup the order is
//! `SetupData::toolpath_indices`, which is NOT the numeric index order. That
//! is the only order in which a Regions or a PrevTool source is guaranteed
//! earlier than its consumer, so those two edge kinds need no step of their
//! own: the walk already generates the source first.
//!
//! # One Simulate step per rest operation (the F.4 property)
//!
//! A [`Step::Simulate`] sits immediately before an operation that starts
//! from the remaining stock and whose snapshot is missing.
//! `PhantomPriorStockScan` locks on the FIRST enabled operation with no
//! generated result, so one simulation unlocks at most one pending
//! operation per setup. Setup 1 = `[rough (Fresh), rest1, rest2]`, all
//! cold, gives:
//!
//! ```text
//! Generate(rough), Simulate(upto rest1), Generate(rest1),
//! Simulate(upto rest2), Generate(rest2)
//! ```
//!
//! The first simulation runs when only `rough` has a result, so the scan
//! resolves at `rest1`. The second runs when `rough` and `rest1` both have
//! results, so it resolves at `rest2`. So k rest operations in one setup
//! give exactly k Simulate steps, and a project that already holds a
//! covering simulation plans zero of them.
//!
//! # A Simulate step is never narrowed to `upto`
//!
//! The consumer resolves `Step::Simulate::setup` to a POSITION with
//! `ProjectSession::find_setup_by_id`, and must simulate EVERY enabled
//! operation in setups `0..=position`, never a narrowed id list and never
//! `0..=id.0`. The group builder feeds the scan the count of the operations
//! its include filter ADMITTED. Drop a generated operation that sits before
//! the pending one and that count shifts down, so the snapshot is taken
//! before that operation's cuts. For a rest operation that is an over-cut,
//! not a stale preview.
//!
//! # No trailing full simulation
//!
//! The walk emits no `SimulateAll` step. The GUI appends its own final
//! full simulation so the Simulation workspace lands fresh, and an MCP plan
//! appends one only with an explicit resolution, or a no-rest `generate_all`
//! would simulate at a cell size the agent never chose. W1 owns both.

use std::collections::BTreeSet;

use crate::compute::config::StockSource;
use crate::ids::{SetupId, ToolpathId};
use crate::session::ProjectSession;
use crate::session::dependencies::edges;

/// What the caller wants made current.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    /// Every enabled operation in the project.
    Project,
    /// Every enabled operation in one setup.
    ///
    /// A Simulate step still covers the setups BEFORE this one, and the
    /// phantom scan locks on the first enabled operation with no result. So
    /// an earlier setup holding an ungenerated enabled operation stalls
    /// this scope's rest operations. A caller that cannot promise the
    /// earlier setups are current uses [`Scope::Project`].
    Setup(SetupId),
    /// One operation, and everything it depends on, directly or through
    /// another dependency.
    Ancestors(ToolpathId),
}

/// One piece of work, in plan order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Step {
    /// Simulate so the operation named by `upto` gets its prior-stock
    /// snapshot.
    ///
    /// `setup` is the setup's STABLE id, not its position. The consumer
    /// resolves it with `ProjectSession::find_setup_by_id` and simulates
    /// every enabled operation in setups `0..=position`. Read the module
    /// doc before narrowing that request.
    Simulate { setup: SetupId, upto: ToolpathId },
    /// Generate one operation.
    ///
    /// The step carries both the stable id and the index the generator
    /// takes, so the driver needs no second lookup. The plan emits a
    /// Generate step for every enabled operation in scope, whether or not
    /// it already holds a result: the F.4 property depends on the source
    /// step being present, and a driver that wants to skip a current
    /// operation decides that for itself.
    Generate { toolpath: ToolpathId, index: usize },
}

/// The ordered work that makes `scope` current.
///
/// Pure: it reads the session and the edges, and runs nothing.
pub fn plan(session: &ProjectSession, scope: Scope) -> Vec<Step> {
    let admitted: Option<BTreeSet<ToolpathId>> = match scope {
        Scope::Project => None,
        Scope::Setup(setup_id) => Some(setup_toolpath_ids(session, setup_id)),
        Scope::Ancestors(toolpath_id) => Some(ancestors(session, toolpath_id)),
    };

    let mut steps: Vec<Step> = Vec::new();
    for setup in session.list_setups() {
        for &index in &setup.toolpath_indices {
            let Some(tc) = session.toolpath_configs().get(index) else {
                continue;
            };
            if !tc.enabled {
                continue;
            }
            if admitted.as_ref().is_some_and(|set| !set.contains(&tc.id)) {
                continue;
            }
            if tc.stock_source == StockSource::FromRemainingStock && !has_snapshot(session, tc.id) {
                steps.push(Step::Simulate {
                    setup: SetupId(setup.id),
                    upto: tc.id,
                });
            }
            steps.push(Step::Generate {
                toolpath: tc.id,
                index,
            });
        }
    }
    steps
}

/// Does the session hold the prior-stock snapshot this operation reads?
///
/// The map is keyed by the CONSUMER, because it holds the stock before that
/// operation carves. This is the same read `dependencies::state` makes for
/// a Stock edge.
fn has_snapshot(session: &ProjectSession, id: ToolpathId) -> bool {
    session
        .simulation_result()
        .is_some_and(|sim| sim.prior_stocks.contains_key(&id))
}

/// Every toolpath id in one setup.
fn setup_toolpath_ids(session: &ProjectSession, setup_id: SetupId) -> BTreeSet<ToolpathId> {
    let Some((_, setup)) = session.find_setup_by_id(setup_id.0) else {
        return BTreeSet::new();
    };
    setup
        .toolpath_indices
        .iter()
        .filter_map(|&index| session.toolpath_configs().get(index))
        .map(|tc| tc.id)
        .collect()
}

/// `toolpath_id` and everything it depends on, to a fixpoint.
///
/// The closure reads [`edges`], NEVER `primary_edges`. A Stock edge's real
/// requirement is every enabled same-setup predecessor, because that is
/// what the snapshot holds and what the phantom scan gates on.
/// `primary_edges` keeps only the nearest enabled source, which is the
/// card's one connector row, and that pick can disagree with the scan
/// (defect D6). The plan must not inherit D6.
///
/// A Regions edge names an explicit source, which may sit in another setup,
/// so the closure can cross a setup. Plan order still holds: `plan` walks
/// setups then `toolpath_indices` and tests membership, so the closure
/// decides WHICH operations run and never in what order.
///
/// A disabled ancestor enters the set. The walk then drops it, because no
/// scope generates a disabled operation.
fn ancestors(session: &ProjectSession, toolpath_id: ToolpathId) -> BTreeSet<ToolpathId> {
    let all = edges(session);
    let mut set: BTreeSet<ToolpathId> = BTreeSet::from([toolpath_id]);
    loop {
        let mut grew = false;
        for edge in &all {
            let Some(source) = edge.on else { continue };
            if set.contains(&edge.from) && set.insert(source) {
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
    set
}
