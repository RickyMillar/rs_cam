//! The identity of the stock a rest operation reads (G-RESTRES, G-RESTSTALE).
//!
//! `planning/rest_stock_identity_2026-09-24/PLAN.md` §3.2.
//!
//! A `prior_stocks` snapshot is the stock before one toolpath carves. Its
//! key names the consumer, and the key alone says nothing about HOW the
//! snapshot was made. Two snapshots under one key can differ in three ways:
//!
//! - the cell size the simulation was asked for,
//! - the toolpaths the simulation carved before the consumer, and
//! - (S2) the setup stock changes the simulation applied before it.
//!
//! [`SourceStock`] records all three. The simulator writes one per snapshot
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

/// One thing a simulation did to the stock before the consumer: a carved
/// toolpath, or (S2) a setup stock change.
#[derive(Debug, Clone, PartialEq)]
pub enum SourceEntry {
    /// A toolpath the simulation carved.
    Carved(CarvedEntry),
    /// A stock change the simulation applied before a setup's first
    /// toolpath (S2).
    StockChange(StockChangeEntry),
}

/// One toolpath a simulation carved before the consumer.
#[derive(Debug, Clone)]
pub struct CarvedEntry {
    /// The carved toolpath's id.
    pub id: ToolpathId,
    /// A content digest of the carved moves. Two entries with equal ids
    /// and equal digests carved the same material.
    pub output: u64,
    /// The carved `Arc`. A fast path only: equality never reads it.
    pub carved: Weak<AnnotatedToolpath>,
}

impl PartialEq for CarvedEntry {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.output == other.output
    }
}

/// One stock change a simulation applied before the consumer (S2).
///
/// The digest is [`crate::compute::stock_change::StockChange::effect_digest`]:
/// the op, the geometry and the material of an `Add`. The model geometry
/// is not in it. A model refresh under a stock change drops the dependent
/// rest results through the invalidation rule instead
/// (`ProjectSession::drop_results_for_model`), so the record need not hash
/// a mesh after every command.
#[derive(Debug, Clone, PartialEq)]
pub struct StockChangeEntry {
    /// The `SetupData::id` of the setup that owns the change.
    pub setup_id: usize,
    /// The change id, unique within the setup.
    pub id: crate::ids::StockChangeId,
    /// The effect digest of the change the simulation applied.
    pub effect: u64,
}

impl SourceEntry {
    /// The entry for one carved toolpath.
    #[must_use]
    pub fn of(id: ToolpathId, annotated: &Arc<AnnotatedToolpath>) -> Self {
        Self::Carved(CarvedEntry {
            id,
            output: carve_digest(&annotated.toolpath),
            carved: Arc::downgrade(annotated),
        })
    }

    /// The entry for one applied stock change.
    #[must_use]
    pub fn of_stock_change(resolved: &crate::compute::stock_change::ResolvedStockChange) -> Self {
        Self::StockChange(StockChangeEntry {
            setup_id: resolved.setup_id,
            id: resolved.change.id,
            effect: resolved.effect_digest(),
        })
    }

    /// The carved toolpath's id, or `None` for a stock change.
    #[must_use]
    pub fn toolpath_id(&self) -> Option<ToolpathId> {
        match self {
            Self::Carved(carved) => Some(carved.id),
            Self::StockChange(_) => None,
        }
    }
}

impl CarvedEntry {
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
    /// Everything the simulation did to the stock before the consumer, in
    /// order: per group, its stock changes (S2), then its carved toolpaths;
    /// every earlier setup group, then the consumer's own group up to the
    /// rows above it.
    pub after: Vec<SourceEntry>,
}

/// One [`SourceStock`] per snapshot the simulation keeps.
///
/// The simulator runs its groups in request order. A snapshot keyed by a
/// carved entry holds everything before that entry; a phantom snapshot
/// (`SimGroupEntry::phantom_prior_stock`) holds everything before its slot.
///
/// S0 (`compute/stock_carry.rs`): a Z-axis group starts from the final stock
/// of the Z-axis groups before it, so its records list those groups' entries.
/// A lateral group starts from a fresh stock, so its records list only its
/// own entries, and its entries are not in any later group's record.
#[must_use]
pub fn snapshot_sources(
    request: &SimulationRequest,
    prior_stocks: &HashMap<ToolpathId, Arc<TriDexelStock>>,
) -> HashMap<ToolpathId, SourceStock> {
    let mut out: HashMap<ToolpathId, SourceStock> = HashMap::new();
    // The entries the last Z-axis group's final stock holds.
    let mut carried: Vec<SourceEntry> = Vec::new();
    for group in &request.groups {
        let lateral = crate::compute::stock_carry::group_is_lateral(group);
        let mut carved: Vec<SourceEntry> = if lateral { Vec::new() } else { carried.clone() };
        // S2: the group applies its stock changes before its first entry,
        // so every record of the group (the k = 0 phantom included) and of
        // every group that carries from it lists them.
        carved.extend(group.stock_changes.iter().map(SourceEntry::of_stock_change));
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
        if !lateral {
            carried = carved;
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
    /// S4: the stock changes the snapshot holds, in the order the
    /// simulation applied them. Empty when no setup before this one, and
    /// not this one, changes the stock. An empty list is not written, so a
    /// project with no stock change keeps its wire bytes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stock_changes: Vec<StockChangeEntryWire>,
}

/// One applied stock change on the wire.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct StockChangeEntryWire {
    /// The `SetupData::id` of the setup that owns the change.
    pub setup_id: usize,
    /// The change id, unique within the setup.
    pub id: crate::ids::StockChangeId,
    /// The effect digest of the change the simulation applied
    /// (`StockChange::effect_digest`), as 16 hex digits.
    pub effect: String,
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
                .filter_map(|e| match e {
                    SourceEntry::Carved(carved) => Some(SourceEntryWire {
                        id: carved.id,
                        output: format!("{:016x}", carved.output),
                    }),
                    SourceEntry::StockChange(_) => None,
                })
                .collect(),
            stock_changes: source
                .after
                .iter()
                .filter_map(|e| match e {
                    SourceEntry::StockChange(change) => Some(StockChangeEntryWire {
                        setup_id: change.setup_id,
                        id: change.id,
                        effect: format!("{:016x}", change.effect),
                    }),
                    SourceEntry::Carved(_) => None,
                })
                .collect(),
        }
    }
}

#[cfg(test)]
#[allow(
    // SAFETY: test code; a missing record is a failed test.
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::compute::simulate::SimGroupEntry;
    use crate::compute::stock_change::{
        ResolvedStockChange, StockChange, StockChangeOp, StockGeometry,
    };
    use crate::geo::{BoundingBox3, P3};
    use crate::ids::{ModelId, StockChangeId};

    fn resolved(setup_id: usize, id: usize) -> ResolvedStockChange {
        ResolvedStockChange {
            setup_id,
            change: StockChange {
                id: StockChangeId(id),
                name: String::new(),
                enabled: true,
                op: StockChangeOp::Add,
                geometry: StockGeometry::OutlineExtrude {
                    model_ids: vec![ModelId(1)],
                    z_bottom: 0.0,
                    z_top: 1.0,
                },
                material: crate::material::Material::default(),
            },
            sources: Vec::new(),
        }
    }

    fn group(change: ResolvedStockChange, phantom: ToolpathId) -> SimGroupEntry {
        SimGroupEntry {
            toolpaths: Vec::new(),
            direction: crate::dexel_stock::StockCutDirection::FromTop,
            local_stock_bbox: None,
            local_to_global: None,
            phantom_prior_stock: Some((0, phantom)),
            stock_changes: vec![change],
        }
    }

    /// S2: a group's stock changes come first in every record of the group,
    /// and a later Z-axis group's records list them too (the S0 carry).
    #[test]
    fn a_rest_record_lists_the_stock_changes_before_it() {
        let bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(10.0, 10.0, 5.0),
        };
        let request = SimulationRequest {
            groups: vec![
                group(resolved(0, 1), ToolpathId(10)),
                group(resolved(1, 2), ToolpathId(11)),
            ],
            stock_bbox: bbox,
            stock_top_z: 5.0,
            resolution: 1.0,
            spindle_rpm: 18_000,
            rapid_feed_mm_min: 5_000.0,
            model_mesh: None,
            kinematics: None,
            display_stride: 1,
        };
        let stock = Arc::new(TriDexelStock::from_bounds(&bbox, 1.0));
        let prior: HashMap<ToolpathId, Arc<TriDexelStock>> = [
            (ToolpathId(10), Arc::clone(&stock)),
            (ToolpathId(11), stock),
        ]
        .into_iter()
        .collect();
        let sources = snapshot_sources(&request, &prior);
        let entry =
            |setup_id: usize, id: usize| SourceEntry::of_stock_change(&resolved(setup_id, id));
        assert_eq!(sources[&ToolpathId(10)].after, vec![entry(0, 1)]);
        assert_eq!(
            sources[&ToolpathId(11)].after,
            vec![entry(0, 1), entry(1, 2)]
        );

        // S4: the wire lists the two changes, in order, with their digests,
        // and lists no carved toolpath for them.
        let wire = SourceStockWire::of(&sources[&ToolpathId(11)], None);
        assert!(wire.after.is_empty());
        let listed: Vec<(usize, usize, String)> = wire
            .stock_changes
            .iter()
            .map(|c| (c.setup_id, c.id.0, c.effect.clone()))
            .collect();
        assert_eq!(
            listed,
            vec![
                (0, 1, format!("{:016x}", resolved(0, 1).effect_digest())),
                (1, 2, format!("{:016x}", resolved(1, 2).effect_digest())),
            ]
        );
    }
}
