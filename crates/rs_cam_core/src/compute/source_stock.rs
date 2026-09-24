//! The identity of the stock a rest operation reads (G-RESTRES, G-RESTSTALE).
//!
//! `planning/rest_stock_identity_2026-09-24/PLAN.md` §3.2.
//!
//! A `prior_stocks` snapshot is the stock before one toolpath carves. Its
//! key names the consumer, and the key alone says nothing about HOW the
//! snapshot was made. Two snapshots under one key can differ in two ways:
//!
//! - the cell size the simulation was asked for, and
//! - the toolpaths the simulation carved before the consumer.
//!
//! [`SourceStock`] records both. The simulator writes one per snapshot
//! ([`snapshot_sources`]), from the request it carved. A rest generation
//! copies the record it read onto its result. The session compares a
//! record with the current project state; an unequal record is a stale
//! snapshot, or a stale result.
//!
//! # The identity of a carved toolpath is its content
//!
//! A [`SourceEntry`] holds a content digest of the toolpath the simulation
//! carved, not a revision or a counter. A regenerate that gives the same
//! moves keeps the identity, so a re-simulation or a regenerate that
//! changes nothing stales nothing. The entry also holds a `Weak` to the
//! carved `Arc`. The comparison reads the pointer first, and hashes only
//! when the pointer differs.
//!
//! The digest ([`carve_digest`]) reads the move GEOMETRY and not the feed.
//! The dexel carve reads no feed, and the adaptive feed modulation rewrites
//! the feeds of every carved toolpath after each simulation: a feed in the
//! digest would stale every rest result after every simulation.

use std::collections::HashMap;
use std::sync::{Arc, Weak};

use crate::compute::simulate::SimulationRequest;
use crate::dexel_stock::TriDexelStock;
use crate::ids::ToolpathId;
use crate::trace::toolpath_spans::AnnotatedToolpath;

/// One toolpath a simulation carved before the consumer.
#[derive(Debug, Clone)]
pub struct SourceEntry {
    /// The carved toolpath's id.
    pub id: ToolpathId,
    /// A content digest of the carved moves. Two entries with equal ids
    /// and equal digests carved the same material.
    pub output: u64,
    /// The carved `Arc`. A fast path only: equality never reads it.
    pub carved: Weak<AnnotatedToolpath>,
}

impl PartialEq for SourceEntry {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.output == other.output
    }
}

impl SourceEntry {
    /// The entry for one carved toolpath.
    #[must_use]
    pub fn of(id: ToolpathId, annotated: &Arc<AnnotatedToolpath>) -> Self {
        Self {
            id,
            output: carve_digest(&annotated.toolpath),
            carved: Arc::downgrade(annotated),
        }
    }

    /// Does `current` carry the moves this entry carved?
    ///
    /// The pointer answers the common case. A different `Arc` with the
    /// same moves is still a match.
    #[must_use]
    pub fn matches(&self, current: &Arc<AnnotatedToolpath>) -> bool {
        if let Some(carved) = self.carved.upgrade()
            && Arc::ptr_eq(&carved, current)
        {
            return true;
        }
        carve_digest(&current.toolpath) == self.output
    }

    /// Point the fast path at `current` when it carries the same moves, so
    /// the next comparison reads the pointer and does not hash.
    pub fn refresh(&mut self, current: &Arc<AnnotatedToolpath>) {
        if self.matches(current) {
            self.carved = Arc::downgrade(current);
        }
    }
}

/// A digest of what a toolpath CARVES: every move's target, its kind, and an
/// arc's centre offsets. Feeds are left out on purpose (module doc).
///
/// FNV-1a, the hash `StockSnapshotStamp::of` uses. Equal within one build,
/// not across releases.
#[must_use]
pub fn carve_digest(toolpath: &crate::toolpath::Toolpath) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h: u64 = FNV_OFFSET_BASIS;
    let mut eat = |word: u64| {
        for byte in word.to_le_bytes() {
            h ^= u64::from(byte);
            h = h.wrapping_mul(FNV_PRIME);
        }
    };
    eat(toolpath.moves.len() as u64);
    for motion in &toolpath.moves {
        eat(motion.target.x.to_bits());
        eat(motion.target.y.to_bits());
        eat(motion.target.z.to_bits());
        match motion.move_type {
            crate::toolpath::MoveType::Rapid => eat(0),
            crate::toolpath::MoveType::Linear { .. } => eat(1),
            crate::toolpath::MoveType::ArcCW { i, j, .. } => {
                eat(2);
                eat(i.to_bits());
                eat(j.to_bits());
            }
            crate::toolpath::MoveType::ArcCCW { i, j, .. } => {
                eat(3);
                eat(i.to_bits());
                eat(j.to_bits());
            }
        }
    }
    h
}

/// How one `prior_stocks` snapshot was made.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SourceStock {
    /// The cell size the simulation was ASKED for, in mm.
    ///
    /// The request value, not the cell the grid cap may coarsen it to.
    /// The cap is a pure function of the request and the stock, so equal
    /// requests give equal grids. The cell the grid actually used is on
    /// `ToolpathStats::stock_snapshot`.
    pub cell_mm: f64,
    /// Every toolpath the simulation carved before the consumer, in carve
    /// order: every earlier setup group, then the rows above it.
    pub after: Vec<SourceEntry>,
}

/// One [`SourceStock`] per snapshot the simulation keeps.
///
/// The simulator runs its groups in request order on one stock. A
/// snapshot keyed by a carved entry holds everything before that entry; a
/// phantom snapshot (`SimGroupEntry::phantom_prior_stock`) holds everything
/// before its slot.
#[must_use]
pub fn snapshot_sources(
    request: &SimulationRequest,
    prior_stocks: &HashMap<ToolpathId, Arc<TriDexelStock>>,
) -> HashMap<ToolpathId, SourceStock> {
    let mut out: HashMap<ToolpathId, SourceStock> = HashMap::new();
    let mut carved: Vec<SourceEntry> = Vec::new();
    for group in &request.groups {
        let phantom = group.phantom_prior_stock;
        for (k, entry) in group.toolpaths.iter().enumerate() {
            if let Some((phantom_k, phantom_id)) = phantom
                && phantom_k == k
                && prior_stocks.contains_key(&phantom_id)
            {
                let _ = out.insert(
                    phantom_id,
                    SourceStock {
                        cell_mm: request.resolution,
                        after: carved.clone(),
                    },
                );
            }
            if prior_stocks.contains_key(&entry.id) {
                let _ = out.insert(
                    entry.id,
                    SourceStock {
                        cell_mm: request.resolution,
                        after: carved.clone(),
                    },
                );
            }
            carved.push(SourceEntry::of(entry.id, &entry.annotated));
        }
        if let Some((phantom_k, phantom_id)) = phantom
            && phantom_k == group.toolpaths.len()
            && prior_stocks.contains_key(&phantom_id)
        {
            let _ = out.insert(
                phantom_id,
                SourceStock {
                    cell_mm: request.resolution,
                    after: carved.clone(),
                },
            );
        }
    }
    out
}

/// The wire form of a [`SourceStock`], as MCP `get_diagnostics` and the
/// CLI's `tp_*.json` publish it (G-RESTRES, ruling Q8: the full record on
/// MCP and the CLI).
///
/// Digests travel as 16-digit hex strings: a JSON number cannot carry a
/// `u64` exactly.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SourceStockWire {
    /// The cell the simulation was asked for, in mm.
    pub cell_mm: f64,
    /// The content digest of the snapshot the generation read
    /// (`ToolpathStats::stock_snapshot`), or `None` when none was stamped.
    pub stock_digest: Option<String>,
    /// The toolpaths carved before this one, in carve order.
    pub after: Vec<SourceEntryWire>,
}

/// One carved toolpath on the wire.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SourceEntryWire {
    /// The toolpath id.
    pub id: ToolpathId,
    /// The content digest of the moves the simulation carved.
    pub output: String,
}

impl SourceStockWire {
    /// The wire form of one record and the snapshot stamp beside it.
    #[must_use]
    pub fn of(
        source: &SourceStock,
        stamp: Option<&crate::compute::toolpath_stats::StockSnapshotStamp>,
    ) -> Self {
        Self {
            cell_mm: source.cell_mm,
            stock_digest: stamp.map(|s| format!("{:016x}", s.digest)),
            after: source
                .after
                .iter()
                .map(|e| SourceEntryWire {
                    id: e.id,
                    output: format!("{:016x}", e.output),
                })
                .collect(),
        }
    }
}
