//! The Bore adapter (geometry selection P6.6, lead ruling L-11): a helix
//! mills each selected circle to its drawn size. It is a milling op, not a
//! drill cycle: the simulation sweeps its moves and the export writes them
//! as moves.
//!
//! The prologue gives the fitted holes ([`Prepared::holes`]): the hole fit
//! step (`selection::fit_holes`, rule `HoleFitRule::LargerThanTool`) keeps
//! only the circles larger than the tool, up to two tool diameters. A
//! point states no size, so the view does not serve it to a Bore.
//!
//! The floor is the op floor (`FloorOwner::OpFloor`, D9):
//! `ResolvedHeights::bottom_z`. The hole top is the resolved Top Z; on the
//! remaining stock (`RestUse::StockColumn`, D8) it is the column top over
//! the circle, read with the hole radius (the helix sweeps the whole
//! disc), and a hole with no material above its floor is dropped.

use crate::compute::catalog::{OperationConfig, RestUse};
use crate::geo::{BoundingBox3, P2, P3};
use crate::ops::bore::{BoreHole, BoreParams, bore_toolpath};
use crate::selection::KEY_MATCH_EPS_MM;
use crate::tool::MillingCutter;

use super::shared::{generated_with_cut_run_spans, with_depth_run_annotation};
use super::standing_material::{ColumnTop, column_top};
use super::{ExecutionContext, GeneratedToolpath, OperationError, Prepared};

/// The span label of a bore.
const SPAN_LABEL: &str = "Bore";

/// Bore family adapter: the fitted circles of the op's selection.
pub(crate) fn generate_bore(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Bore, "generate_bore");
    let mut circles: Vec<(P2, f64)> = Vec::new();
    for hole in prepared.holes()?.as_slice() {
        let Some(d) = hole.diameter else {
            return Err(OperationError::Internal(
                "the Bore fit step gave a hole with no diameter (a point); a Bore serves \
                 circles only"
                    .to_owned(),
            ));
        };
        // Two circles of one size on one centre are one hole: bore it once
        // (G-DRILLTWICE for a Bore).
        let repeat = circles.iter().any(|(c, e)| {
            (c.x - hole.xy.x).abs() < KEY_MATCH_EPS_MM
                && (c.y - hole.xy.y).abs() < KEY_MATCH_EPS_MM
                && (e - d).abs() < KEY_MATCH_EPS_MM
        });
        if !repeat {
            circles.push((hole.xy, d));
        }
    }
    let floor_z = ctx.heights.bottom_z;
    let top_z = ctx.heights.top_z;
    let stock = (op.capabilities().rest_use == RestUse::StockColumn)
        .then(|| prepared.territory().unguarded_stock())
        .flatten();
    let holes = bore_holes(&circles, top_z, floor_z, stock);
    // The helix is short; the adapter polls once before the kernel.
    if ctx.is_cancelled() {
        return Err(OperationError::Cancelled);
    }
    let params = BoreParams {
        tool_diameter: ctx.tool_def.diameter(),
        pitch: cfg.pitch,
        climb: cfg.climb,
        finish_allowance: cfg.finish_allowance,
        helix_feed: op.entry_feed_rate().unwrap_or_else(|| op.plunge_rate()),
        feed: op.feed_rate(),
        safe_z: ctx.heights.retract_z,
        start_clearance: crate::dressup::ENTRY_CONTACT_CLEARANCE,
    };
    let toolpath = bore_toolpath(&holes, &params)
        .map_err(|refusal| OperationError::InvalidParam(format!("Bore: {refusal}")))?;
    Ok(with_depth_run_annotation(
        generated_with_cut_run_spans(toolpath, SPAN_LABEL),
        ctx.semantic_ctx,
    ))
}

/// The holes to bore, with their tops (D8). On fresh stock (`stock` is
/// `None`) each hole runs from `top_z` to `floor_z`. On the remaining stock
/// the top of a hole is the highest material within the hole radius of
/// its centre (`column_top`), capped at `top_z`; a hole with no material
/// above its floor is dropped, and a hole that the grid cannot see keeps
/// `top_z`. The stock is read only when the W2.1 guard finds it in the
/// frame of the holes.
fn bore_holes(
    circles: &[(P2, f64)],
    top_z: f64,
    floor_z: f64,
    stock: Option<&crate::dexel_stock::TriDexelStock>,
) -> Vec<BoreHole> {
    let Some(first) = circles.first() else {
        return Vec::new();
    };
    let holes_bbox = circles.iter().fold(
        BoundingBox3 {
            min: P3::new(first.0.x, first.0.y, floor_z),
            max: P3::new(first.0.x, first.0.y, top_z),
        },
        |mut b, (c, d)| {
            let r = d / 2.0;
            b.min.x = b.min.x.min(c.x - r);
            b.min.y = b.min.y.min(c.y - r);
            b.max.x = b.max.x.max(c.x + r);
            b.max.y = b.max.y.max(c.y + r);
            b
        },
    );
    let stock =
        crate::surface::stock_reference::StockReference::resolve(stock, &holes_bbox).stock();
    let mut dropped = 0_usize;
    let holes: Vec<BoreHole> = circles
        .iter()
        .filter_map(|&(centre, diameter)| {
            let top = match stock.map(|s| column_top(s, centre, diameter / 2.0)) {
                None | Some(ColumnTop::NoEvidence) => top_z,
                Some(ColumnTop::Top(t)) => t.min(top_z),
                Some(ColumnTop::Empty) => floor_z,
            };
            if top <= floor_z {
                dropped += 1;
                return None;
            }
            Some(BoreHole {
                centre,
                diameter,
                top_z: top,
                floor_z,
            })
        })
        .collect();
    if dropped > 0 {
        tracing::info!(
            holes = holes.len(),
            dropped,
            "bore: dropped holes with no remaining stock above their floor"
        );
    }
    holes
}
