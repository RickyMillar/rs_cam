//! Test fixtures only (`test-support`): the values a test fixture states
//! for the fields that S11 left unset.
//!
//! S11 (operator ruling 2026-10-23) removed the underived defaults: a new
//! op has no floor, no drawing depth and no feeds, and a stock with no
//! model has no thickness. A test fixture that cuts must state those
//! values. The values here are the struct values before S11, so a fixture
//! that read them before cuts the same toolpath now (G-OPGOLDEN does not
//! move). They are fixture statements, not product defaults: no product
//! door reads this module.

use crate::compute::catalog::{OperationConfig, OperationType};
use crate::compute::config::DressupConfig;
use crate::compute::floor::Floor;

/// The spindle speed (RPM) a fixture states: the post `spindle_speed`
/// that an op with no RPM read before S11.
pub const FIXTURE_SPINDLE_RPM: u32 = 18_000;

/// The stock thickness (mm) a fixture states when it gives none: the
/// `StockConfig` default before S11.
pub const FIXTURE_STOCK_Z_MM: f64 = 25.0;

/// The link max distance (mm) and the feed-optimisation ceiling (mm/min)
/// of a fixture: the `DressupConfig` values before S11.
pub const FIXTURE_LINK_MAX_DISTANCE_MM: f64 = 10.0;
/// See [`FIXTURE_LINK_MAX_DISTANCE_MM`].
pub const FIXTURE_FEED_MAX_RATE: f64 = 3000.0;

/// The feed and plunge (mm/min) of each op before S11.
#[must_use]
pub const fn fixture_feeds(op_type: OperationType) -> (f64, f64) {
    match op_type {
        OperationType::Face | OperationType::Adaptive | OperationType::Adaptive3d => {
            (1500.0, 500.0)
        }
        OperationType::Trace
        | OperationType::Chamfer
        | OperationType::VCarve
        | OperationType::Inlay
        | OperationType::InlayPlug
        | OperationType::Pencil
        | OperationType::ProjectCurve => (800.0, 400.0),
        OperationType::Drill | OperationType::AlignmentPinDrill => (300.0, 300.0),
        OperationType::Bore
        | OperationType::Pocket
        | OperationType::Profile
        | OperationType::Zigzag
        | OperationType::DropCutter
        | OperationType::Waterline
        | OperationType::Scallop
        | OperationType::UnifiedFinish
        | OperationType::SteepShallow
        | OperationType::RampFinish
        | OperationType::SpiralFinish
        | OperationType::RadialFinish
        | OperationType::HorizontalFinish => (1000.0, 500.0),
    }
}

/// The floor of each floor op before S11.
#[must_use]
pub const fn fixture_floor(op_type: OperationType) -> Option<Floor> {
    match op_type {
        OperationType::Pocket | OperationType::Zigzag | OperationType::Bore => {
            Some(Floor::Top { depth: 3.0 })
        }
        OperationType::Profile | OperationType::Adaptive => Some(Floor::Top { depth: 6.0 }),
        OperationType::Drill => Some(Floor::Top { depth: 10.0 }),
        OperationType::Face
        | OperationType::Trace
        | OperationType::Chamfer
        | OperationType::VCarve
        | OperationType::Inlay
        | OperationType::InlayPlug
        | OperationType::AlignmentPinDrill
        | OperationType::Adaptive3d
        | OperationType::DropCutter
        | OperationType::Waterline
        | OperationType::Pencil
        | OperationType::Scallop
        | OperationType::UnifiedFinish
        | OperationType::SteepShallow
        | OperationType::RampFinish
        | OperationType::SpiralFinish
        | OperationType::RadialFinish
        | OperationType::HorizontalFinish
        | OperationType::ProjectCurve => None,
    }
}

/// Fill each unset value of `op` with the fixture value. A set value
/// stays.
pub fn fill_unset(op: &mut OperationConfig) {
    let op_type = op.op_type();
    let (feed, plunge) = fixture_feeds(op_type);
    if op.feed_rate().is_none() {
        op.set_feed_rate(feed);
    }
    if op.plunge_rate().is_none() {
        op.set_plunge_rate(plunge);
    }
    if op.spindle_rpm().is_none() {
        op.set_spindle_rpm(Some(FIXTURE_SPINDLE_RPM));
    }
    if op.floor().is_some_and(|floor| floor.is_unset())
        && let Some(floor) = fixture_floor(op_type)
    {
        let _ = op.set_floor(floor);
    }
    match op {
        OperationConfig::Trace(c) => {
            c.depth.get_or_insert(1.0);
        }
        OperationConfig::VCarve(c) => {
            c.max_depth.get_or_insert(5.0);
        }
        OperationConfig::Inlay(c) => {
            c.pocket_depth.get_or_insert(3.0);
            // `2.0 x (1 + 1 / (2 x 0.4))`: the band from the Pocket
            // stepover 2.0 before S11.
            c.floor_overlap.get_or_insert(4.5);
        }
        OperationConfig::InlayPlug(c) => {
            c.pocket_depth.get_or_insert(3.0);
        }
        OperationConfig::ProjectCurve(c) => {
            c.depth.get_or_insert(1.0);
        }
        OperationConfig::Face(c) => {
            c.stock_offset.get_or_insert(5.0);
        }
        OperationConfig::Drill(_)
        | OperationConfig::Bore(_)
        | OperationConfig::AlignmentPinDrill(_)
        | OperationConfig::Chamfer(_)
        | OperationConfig::Pocket(_)
        | OperationConfig::Profile(_)
        | OperationConfig::Adaptive(_)
        | OperationConfig::Zigzag(_)
        | OperationConfig::DropCutter(_)
        | OperationConfig::Adaptive3d(_)
        | OperationConfig::Waterline(_)
        | OperationConfig::Pencil(_)
        | OperationConfig::Scallop(_)
        | OperationConfig::UnifiedFinish(_)
        | OperationConfig::SteepShallow(_)
        | OperationConfig::RampFinish(_)
        | OperationConfig::SpiralFinish(_)
        | OperationConfig::RadialFinish(_)
        | OperationConfig::HorizontalFinish(_) => {}
    }
}

/// The registry op of `op_type` with every unset value filled
/// ([`fill_unset`]).
#[must_use]
pub fn fixture_op(op_type: OperationType) -> OperationConfig {
    let mut op = OperationConfig::new_default(op_type);
    fill_unset(&mut op);
    op
}

/// Fill the unset dressup values of `dressups` with the fixture values.
pub fn fill_unset_dressups(dressups: &mut DressupConfig) {
    if let Some(link) = dressups.link_moves.as_mut() {
        link.max_distance
            .get_or_insert(FIXTURE_LINK_MAX_DISTANCE_MM);
    }
    dressups.feed_max_rate.get_or_insert(FIXTURE_FEED_MAX_RATE);
}

/// Fill an unset stock thickness with [`FIXTURE_STOCK_Z_MM`]. The bottom
/// stays, so the stock top rises by the thickness.
pub fn fill_unset_stock(stock: &mut crate::compute::StockConfig) {
    stock.z.get_or_insert(FIXTURE_STOCK_Z_MM);
}

/// [`fill_unset`], but the floor stays as it is: a fixture whose claim
/// reads the default floor (`Floor::Unset`, the each-face rule) states
/// only the feeds and the depths.
pub fn fill_unset_keep_floor(op: &mut OperationConfig) {
    let floor = op.floor();
    fill_unset(op);
    if let Some(floor) = floor {
        let _ = op.set_floor(floor);
    }
}
