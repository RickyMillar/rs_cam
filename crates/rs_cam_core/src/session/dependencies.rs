//! The one dependency function: which toolpath depends on which, and how
//! current that dependency is.
//!
//! `PLAN.md` §4.1 of `planning/gen_sim_rest_ux_2026-09-18/`. Three surfaces
//! used to re-derive this rule: the invalidation walker
//! ([`ProjectSession::invalidate_output_dependents_of_set`]), the card badge
//! and the MCP `list_toolpaths` row. They now read [`edges`].
//!
//! The module is pure. It takes `&ProjectSession`, returns values, mutates
//! nothing and names no GUI type.
//!
//! # The three declarations
//!
//! A consumer declares its own edges; a source declares none.
//!
//! - [`EdgeKind::Stock`]: `stock_source == StockSource::FromRemainingStock`.
//!   The consumer starts from the material the ops above it left.
//! - [`EdgeKind::Regions`]: `boundary.enabled` plus
//!   `BoundarySource::DerivedRestRegions { source_toolpath_id }`. The
//!   consumer machines the rest regions another op's result carries.
//! - [`EdgeKind::PrevTool`]: `OperationConfig::Rest(cfg)`. The consumer
//!   clears what the cutter named by `cfg.prev_tool_id` left behind.
//!
//! # Two asymmetries a reviewer must not "fix"
//!
//! **A Stock edge is NOT filtered on the source's `enabled` flag; a PrevTool
//! edge IS.** A Stock edge asks "did the material above me move", and an op
//! that was just switched off moves it: `set_toolpath_enabled` writes
//! `enabled = false` and THEN seeds the walker, so the walker must still see
//! the edge. A PrevTool edge asks "does a predecessor with that cutter
//! exist", and a disabled op is not one.
//!
//! **A Stock consumer must be enabled; a Regions or PrevTool consumer need
//! not be.** The first mirrors the walker's rule (a), which drops only
//! enabled `FromRemainingStock` ops. The second two mirror rule (b), which
//! drops a consumer whose source's OUTPUT moved whether or not the consumer
//! is switched on.
//!
//! # Rule (c) over-approximates, on purpose
//!
//! The generator derives `prev_tool_radius` from the TOOL table, not from
//! the predecessor's result, so a Rest consumer whose predecessor merely
//! regenerates has unchanged generation inputs. The cost is bounded: rule
//! (a) already covers every `FromRemainingStock` Rest op, so rule (c) adds
//! drops only for a `Fresh` Rest op with a resolved predecessor, and those
//! regenerate in seconds. Read this as a decision, not as an oversight.
//!
//! # The PrevTool twin
//!
//! `rs_cam_viz::state::rest_dependency::rest_predecessors` holds the same
//! rule for the card. W3 of the programme deletes the viz copy and re-points
//! its two callers at this module. Until then the two must agree.

use std::collections::HashMap;

use crate::compute::catalog::OperationConfig;
use crate::compute::config::{BoundarySource, StockSource};
use crate::ids::ToolpathId;
use crate::session::ProjectSession;

/// What a consumer asks of its source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EdgeKind {
    /// The material the ops above the consumer left.
    Stock,
    /// The rest regions the source's result carries.
    Regions,
    /// The predecessor that ran the coarse cutter.
    PrevTool,
}

/// How current a dependency is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EdgeState {
    /// The source is current, and for a Stock edge the snapshot exists.
    Ready,
    /// The source is not current, or the snapshot is missing. A wait, not
    /// a fault.
    Pending,
    /// The declaration resolves to no usable source.
    Broken,
}

/// One dependency, read forward: `from` depends on `on`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Edge {
    /// The consumer. It declares the edge.
    pub from: ToolpathId,
    /// The source, or `None` when the declaration resolves to no toolpath.
    /// The state of an `on: None` edge is always [`EdgeState::Broken`].
    pub on: Option<ToolpathId>,
    /// What the consumer asks for.
    pub kind: EdgeKind,
}

/// Every dependency every toolpath declares, in consumer order.
///
/// This is the invalidation walker's input. A consumer with a Stock
/// declaration and several ops above it gets one edge per op above it: the
/// walker's rule is "any upstream change dirties me", and only the full
/// upstream set states that. [`primary_edges`] collapses the set for a
/// surface that draws one line.
pub fn edges(session: &ProjectSession) -> Vec<Edge> {
    let configs = &session.toolpath_configs;
    // Plan position of every toolpath: (setup index, position in that
    // setup's order). "Earlier in the same setup" means earlier in
    // `toolpath_indices`, which is NOT the numeric index order.
    let mut plan_position: HashMap<usize, (usize, usize)> = HashMap::new();
    for (setup_index, setup) in session.setups.iter().enumerate() {
        for (position, &tp_index) in setup.toolpath_indices.iter().enumerate() {
            plan_position.insert(tp_index, (setup_index, position));
        }
    }

    let mut out: Vec<Edge> = Vec::new();
    for (tp_index, tc) in configs.iter().enumerate() {
        // The ops above this one, in plan order.
        let above: &[usize] = match plan_position.get(&tp_index) {
            Some(&(setup_index, position)) => session
                .setups
                .get(setup_index)
                .and_then(|s| s.toolpath_indices.get(..position))
                .unwrap_or(&[]),
            None => &[],
        };

        // Stock. Many edges, no `enabled` filter on the source.
        if tc.enabled && matches!(tc.stock_source, StockSource::FromRemainingStock) {
            let mut found = false;
            for &up_index in above {
                if let Some(up) = configs.get(up_index) {
                    out.push(Edge {
                        from: tc.id,
                        on: Some(up.id),
                        kind: EdgeKind::Stock,
                    });
                    found = true;
                }
            }
            if !found {
                out.push(Edge {
                    from: tc.id,
                    on: None,
                    kind: EdgeKind::Stock,
                });
            }
        }

        // Regions. One edge; the source is explicit.
        if tc.boundary.enabled
            && let BoundarySource::DerivedRestRegions { source_toolpath_id } = tc.boundary.source
        {
            let resolved = configs
                .iter()
                .any(|c| c.id == source_toolpath_id)
                .then_some(source_toolpath_id);
            out.push(Edge {
                from: tc.id,
                on: resolved,
                kind: EdgeKind::Regions,
            });
        }

        // PrevTool. One edge, on the LAST qualifying predecessor. The
        // source IS `enabled`-filtered; see the module doc.
        if let OperationConfig::Rest(cfg) = &tc.operation {
            let resolved = cfg.prev_tool_id.and_then(|prev| {
                above
                    .iter()
                    .rev()
                    .filter_map(|&up_index| configs.get(up_index))
                    .find(|up| up.enabled && up.tool_id == prev.0 && up.model_id == tc.model_id)
                    .map(|up| up.id)
            });
            out.push(Edge {
                from: tc.id,
                on: resolved,
                kind: EdgeKind::PrevTool,
            });
        }
    }
    out
}

/// How current one edge is.
///
/// The Stock arm reads the snapshot under the CONSUMER's id.
/// `SimulationResult::prior_stocks` holds the stock before a toolpath
/// carves, and the generator looks ITSELF up, so the key is `edge.from` and
/// never `edge.on`.
///
/// The Regions arm reads the source result's payload, not the source's
/// `rest_analysis.enabled` flag. Two producers attach `rest_regions`: the
/// op-agnostic analysis and the pencil rest-depth detector. A config read
/// would call a pencil source Broken. The payload read reproduces the
/// generation refusal exactly.
pub fn state(edge: &Edge, session: &ProjectSession) -> EdgeState {
    let Some(source_id) = edge.on else {
        return EdgeState::Broken;
    };
    let Some((source_index, source)) = session.find_toolpath_config_by_id(source_id) else {
        return EdgeState::Broken;
    };
    if !source.enabled {
        return EdgeState::Broken;
    }
    match edge.kind {
        EdgeKind::Stock => {
            // R2, cross-setup stock, is open. Delete this block when the
            // ruling widens the rule. The simulator runs setups
            // sequentially on one stock, so a wider rule is possible.
            if session.setup_of_toolpath_id(source_id) != session.setup_of_toolpath_id(edge.from) {
                return EdgeState::Broken;
            }
            let snapshot = session
                .simulation_result()
                .is_some_and(|s| s.prior_stocks.contains_key(&edge.from));
            if snapshot && session.get_result(source_index).is_some() {
                EdgeState::Ready
            } else {
                EdgeState::Pending
            }
        }
        EdgeKind::Regions => match session.get_result(source_index) {
            None => EdgeState::Pending,
            Some(result) => match result.annotated().rest_regions.as_deref() {
                Some(regions) if !regions.is_empty() => EdgeState::Ready,
                _ => EdgeState::Broken,
            },
        },
        // The generator reads the predecessor's TOOL, not its result, so
        // "has it generated" is the finest question there is to ask.
        EdgeKind::PrevTool => {
            if session.get_result(source_index).is_some() {
                EdgeState::Ready
            } else {
                EdgeState::Pending
            }
        }
    }
}

/// One row per (consumer, kind): the edge a surface draws as one line.
///
/// [`edges`] holds many Stock edges per consumer. This keeps the NEAREST
/// ENABLED source, which is the op the blocked-operation message names, or
/// the `on: None` row when no source above the consumer is enabled.
/// The Regions and PrevTool rows pass through unchanged.
pub fn primary_edges(session: &ProjectSession) -> Vec<Edge> {
    let all = edges(session);
    let mut out: Vec<Edge> = Vec::new();
    for edge in &all {
        if edge.kind != EdgeKind::Stock {
            out.push(*edge);
            continue;
        }
        if out
            .iter()
            .any(|e| e.from == edge.from && e.kind == EdgeKind::Stock)
        {
            continue;
        }
        // The nearest enabled source among this consumer's Stock edges.
        // `edges` emits them in plan order, so the LAST enabled one is
        // the nearest.
        let nearest = all
            .iter()
            .filter(|e| e.from == edge.from && e.kind == EdgeKind::Stock)
            .filter_map(|e| e.on)
            .rfind(|id| {
                session
                    .find_toolpath_config_by_id(*id)
                    .is_some_and(|(_, tc)| tc.enabled)
            });
        out.push(Edge {
            from: edge.from,
            on: nearest,
            kind: EdgeKind::Stock,
        });
    }
    out
}
