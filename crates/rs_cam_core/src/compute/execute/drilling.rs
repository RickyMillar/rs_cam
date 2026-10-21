//! The drilling family: the hole positions of a drill op and the two drill
//! adapters.
//!
//! The holes come from the selection of the op (geometry selection P2.3c):
//! the prologue gives each adapter [`Prepared::holes`], the points and
//! circle centres of the selected items, already in the emission frame
//! (`ResolvedSelection::to_setup_frame`). A stale pick, an empty selection
//! and every other selection refusal stop the op in the prologue, with the
//! item and the fix (`OperationError::Selection`). This file holds the
//! G-DRILLTWICE dedup, the D8 hole tops, `generate_drill` /
//! `generate_alignment_pin_drill` and the drill-op view the session builds
//! beside the toolpath. Split out of `compute/execute.rs` (P4).

use crate::compute::catalog::{OperationConfig, RestUse};
use crate::compute::tool_config::ToolConfig;
use crate::geo::BoundingBox3;
use crate::selection::{Holes, KEY_MATCH_EPS_MM, ResolvedSelection};
use crate::tool::{MillingCutter, ToolDefinition};

use super::shared::generated_with_drill_spans;
use super::{ExecutionContext, GeneratedToolpath, OperationError, Prepared};

/// Build the [`crate::ops::drill_op::DrillOp`] for a drilling-cycle operation,
/// from the same inputs the toolpath generator just consumed.
///
/// Returns `None` for non-drill ops, and for a drill op whose selection
/// refuses (the generator refused it too). The caller pairs this with the
/// `AnnotatedToolpath` produced by [`execute_operation_annotated`](super::execute_operation_annotated) to
/// form a [`crate::ops::drill_op::OpData::DrillOp`] — the dual-representation
/// invariant.
///
/// `selection` is the resolved selection the generator was given
/// (`ExecutionContext::selection`), in the emission frame. This view runs
/// the same pure [`crate::selection::view`](fn@crate::selection::view) over it with the same
/// `ItemUse`, then the same hole fit step ([`crate::selection::fit_holes`],
/// P6.5) with the same tool diameter, so it names the holes the emitted
/// toolpath drills.
///
/// Hole-source asymmetry (§6.E):
/// - `OperationConfig::Drill`: the holes of its selection
///   (`HoleSource::ModelDerived`); a pick is a match key, so it resolves
///   again at every regenerate.
/// - `OperationConfig::AlignmentPinDrill`: the live stock pins of its
///   diameter (`alignment_pins`, W6.5) and the extra holes of its
///   selection, recorded as a `HoleSource::Snapshot` of the holes this
///   generation drilled.
///
/// `initial_stock` is the stock the generator was seeded with (`None` on a
/// fresh pass). A Drill reads it through [`drill_hole_columns`], as
/// `generate_drill` does, so both views name the same hole tops (D8).
#[allow(clippy::too_many_arguments)] // SAFETY: each argument is one generator input this view must mirror
pub fn build_drill_op_for_config(
    op: &OperationConfig,
    selection: &ResolvedSelection,
    alignment_pins: &[crate::compute::alignment_pins::AlignmentPin],
    tool_def: &ToolDefinition,
    tool_cfg: &ToolConfig,
    stock_bbox: &BoundingBox3,
    initial_stock: Option<&crate::dexel_stock::TriDexelStock>,
    material: crate::material::Material,
) -> Option<crate::ops::drill_op::DrillOp> {
    use crate::ops::drill_op::{DrillHole, DrillOp, HoleSource, ToolProfile};

    let tool_diameter_mm = tool_def.radius() * 2.0;
    let flute_count = tool_cfg.flute_count;
    // The prologue's two steps: the view, then the hole fit (P6.5, O-3).
    let holes_of = || {
        let item_use = op.capabilities().item_use;
        let (mut items, _) = crate::selection::view(selection, item_use).ok()?;
        crate::selection::fit_holes(selection, &mut items, item_use, tool_def.diameter()).ok()?;
        items.holes().cloned()
    };

    match op {
        OperationConfig::Drill(cfg) => {
            let hole_xys = selected_hole_xys(&holes_of()?);
            let holes = drill_hole_columns(
                cfg,
                &hole_xys,
                stock_bbox,
                column_stock(op, initial_stock),
                tool_def.radius(),
            );
            Some(DrillOp {
                holes,
                hole_source: HoleSource::ModelDerived,
                tool_profile: ToolProfile::Flat,
                tool_diameter_mm,
                cycle: cfg.cycle.to_core(cfg),
                feed_rate_mm_min: cfg.feed_rate,
                spindle_rpm: cfg.spindle_rpm.unwrap_or(0),
                flute_count,
                material,
            })
        }
        OperationConfig::AlignmentPinDrill(cfg) => {
            // The same two sources `generate_alignment_pin_drill` drills,
            // through the same function: this view has to name the holes
            // that toolpath drills, not the raw config numbers.
            let hole_xys =
                pin_holes_in_emission_frame(cfg, alignment_pins, stock_bbox, &holes_of()?);
            if hole_xys.is_empty() {
                return None;
            }
            let top_z = stock_bbox.max.z;
            let bottom_z = stock_bbox.min.z - cfg.spoilboard_penetration;
            let cycle = cfg.drill_cycle(top_z - bottom_z);
            // Same expression `generate_alignment_pin_drill` passes to
            // `DrillParams::retract_z`, so the summary models the cycle
            // this op emits (R-2).
            let retract_z = crate::compute::config::effective_safe_z(cfg.retract_z, top_z);
            let holes = hole_xys
                .iter()
                .map(|&xy| DrillHole {
                    xy,
                    top_z,
                    bottom_z,
                    retract_z,
                })
                .collect();
            Some(DrillOp {
                holes,
                hole_source: HoleSource::Snapshot(hole_xys),
                tool_profile: ToolProfile::Flat,
                tool_diameter_mm,
                cycle,
                feed_rate_mm_min: cfg.feed_rate,
                spindle_rpm: cfg.spindle_rpm.unwrap_or(0),
                flute_count,
                material,
            })
        }
        _ => None,
    }
}

/// The XY of each selected hole, in selection order, with every repeat
/// dropped ([`unique_holes`]). `holes` are the FITTED holes (P6.5): the
/// prologue's fit step left out each circle that is not of the tool size,
/// so the XY alone is the whole hole (the tool makes its size). The holes are in the emission frame
/// already: the frame door (`ResolvedSelection::to_setup_frame`) moved
/// each point and circle centre as it moved the drawing, so a hole lands
/// where the feature that locates it lands (G-DRILLPICK-FRAME).
fn selected_hole_xys(holes: &Holes) -> Vec<[f64; 2]> {
    unique_holes(holes.as_slice().iter().map(|h| [h.xy.x, h.xy.y]).collect())
}

/// Every hole an [`crate::compute::operation_configs::AlignmentPinDrillConfig`]
/// drills, expressed in the emission frame — the ONE place the pin-drill's
/// two differently-framed hole sources are reconciled, shared by the
/// generator and by [`build_drill_op_for_config`] so the two cannot drift.
///
/// The two sources genuinely are in different frames:
///
/// - the pins of `cfg.diameter` (`pin_holes_of_diameter` over the live
///   `StockConfig::alignment_pins`, R1) are dimensioned STOCK-RELATIVE (X0Y0 at the stock's min corner) — the
///   frame the export datum converges on, because the pins are what
///   physically registers a flip. `ctx.stock_bbox` IS the stock in the
///   emission frame, so translating by its min corner serves both cases:
///   world for an identity setup, a provable no-op for a non-identity one
///   (`min == (0,0)`), which is why non-identity was accidentally correct
///   (G-PINDRILL-FRAME).
/// - `extras`, the holes of the op's selection (default: none, decision
///   D6), are in the emission frame already: the frame door moved them
///   (G-DRILLPICK-FRAME). They never take the stock-relative translation;
///   applying both would move them twice.
fn pin_holes_in_emission_frame(
    cfg: &crate::compute::operation_configs::AlignmentPinDrillConfig,
    pins: &[crate::compute::alignment_pins::AlignmentPin],
    stock_bbox: &BoundingBox3,
    extras: &Holes,
) -> Vec<[f64; 2]> {
    let stock_origin = stock_bbox.min;
    let mut holes: Vec<[f64; 2]> =
        crate::compute::alignment_pins::pin_holes_of_diameter(pins, cfg.diameter)
            .iter()
            .map(|h| [h[0] + stock_origin.x, h[1] + stock_origin.y])
            .collect();
    holes.extend(extras.as_slice().iter().map(|h| [h.xy.x, h.xy.y]));
    // A selected hole on a stock pin names the same hole twice.
    unique_holes(holes)
}

/// Drop every hole that repeats an earlier one, and keep the first order.
///
/// G-DRILLTWICE: a hole is an XY position, and a drill cycle drills it once.
/// Two sources can name one position twice:
///
/// - a drawing with two circles on one centre (a counterbore drawn as two
///   rings, or a CAD export that writes an entity twice). Each circle is
///   its own item, and both are real items to pick, because their
///   diameters can differ;
/// - a selected hole that lands on a stock alignment pin.
///
/// Without this, `drill_toolpath` ran the whole peck cycle once per copy,
/// and the rapid-order pass put the copies side by side, so the machine
/// drilled each hole two times.
///
/// The tolerance is [`KEY_MATCH_EPS_MM`], the distance at which two match
/// keys name one point. It is also the distance at which
/// `spans::drill_hole_sections` groups moves into one hole.
fn unique_holes(holes: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    let total = holes.len();
    let mut out: Vec<[f64; 2]> = Vec::with_capacity(total);
    for xy in holes {
        let repeat = out.iter().any(|h| {
            (h[0] - xy[0]).abs() < KEY_MATCH_EPS_MM && (h[1] - xy[1]).abs() < KEY_MATCH_EPS_MM
        });
        if !repeat {
            out.push(xy);
        }
    }
    if out.len() < total {
        tracing::info!(
            holes = out.len(),
            repeats = total - out.len(),
            "drill: dropped hole positions that repeat an earlier hole"
        );
    }
    out
}

/// The hole tops a Drill cuts: D8 (W3.4, `RestUse::StockColumn`).
///
/// On fresh stock (`initial_stock == None`) every hole runs from the stock
/// box top to `box top - depth`, with the R-plane
/// `effective_safe_z(cfg.retract_z, box top)`, as before W3.4.
///
/// On the remaining stock the top of each hole is the highest material in
/// the reach of the tool over the hole (`standing_material::column_top`
/// with the tool radius), capped at the box top. A hole in a cleared pocket
/// then starts at the pocket floor, not in the air above it. The bottom
/// stays at `box top - depth`: the depth dial names the design bottom of
/// the hole. The R-plane keeps the clearance it has above the box top on
/// fresh stock, so the first peck starts that clearance above the floor.
///
/// A hole whose column holds no material down to its bottom has nothing
/// to cut and is dropped. A hole that the grid cannot see keeps the box
/// top (no evidence, so no change). The stock is read only when the W2.1
/// guard (`StockReference::resolve`) finds it in the frame of the holes.
fn drill_hole_columns(
    cfg: &crate::compute::operation_configs::DrillConfig,
    hole_xys: &[[f64; 2]],
    stock_bbox: &BoundingBox3,
    initial_stock: Option<&crate::dexel_stock::TriDexelStock>,
    tool_radius: f64,
) -> Vec<crate::ops::drill_op::DrillHole> {
    use super::standing_material::{ColumnTop, column_top};
    use crate::ops::drill_op::DrillHole;

    if hole_xys.is_empty() {
        return Vec::new();
    }
    let box_top = stock_bbox.max.z;
    let bottom_z = box_top - cfg.depth;
    let clearance = crate::compute::config::effective_safe_z(cfg.retract_z, box_top) - box_top;
    let holes_bbox = hole_xys.iter().fold(
        BoundingBox3 {
            min: crate::geo::P3::new(f64::INFINITY, f64::INFINITY, bottom_z),
            max: crate::geo::P3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, box_top),
        },
        |mut b, &[x, y]| {
            b.min.x = b.min.x.min(x);
            b.min.y = b.min.y.min(y);
            b.max.x = b.max.x.max(x);
            b.max.y = b.max.y.max(y);
            b
        },
    );
    let stock =
        crate::surface::stock_reference::StockReference::resolve(initial_stock, &holes_bbox)
            .stock();
    let mut dropped = 0_usize;
    let holes: Vec<DrillHole> = hole_xys
        .iter()
        .filter_map(|&xy| {
            let top_z = match stock
                .map(|s| column_top(s, crate::geo::P2::new(xy[0], xy[1]), tool_radius))
            {
                None | Some(ColumnTop::NoEvidence) => box_top,
                Some(ColumnTop::Top(t)) => t.min(box_top),
                Some(ColumnTop::Empty) => bottom_z,
            };
            if top_z <= bottom_z {
                dropped += 1;
                return None;
            }
            Some(DrillHole {
                xy,
                top_z,
                bottom_z,
                retract_z: top_z + clearance,
            })
        })
        .collect();
    if dropped > 0 {
        tracing::info!(
            holes = holes.len(),
            dropped,
            "drill: dropped holes with no remaining stock above their bottom"
        );
    }
    holes
}

/// The stock that sets the hole tops of `op` (C2). The row's `rest_use`
/// states the rule, as for 2D Adaptive: `StockColumn` reads the remaining
/// stock; every other rule drills from the stock box top.
fn column_stock<'s>(
    op: &OperationConfig,
    initial_stock: Option<&'s crate::dexel_stock::TriDexelStock>,
) -> Option<&'s crate::dexel_stock::TriDexelStock> {
    initial_stock.filter(|_| op.capabilities().rest_use == RestUse::StockColumn)
}

/// Drill family adapter: the holes of the op's selection
/// ([`Prepared::holes`]).
///
/// The prologue refuses before this runs when the selection gives no hole
/// (`Empty`), when a pick names no item (`LostPick`, G-DRILLPICKSTALE) and
/// for every other selection refusal. A polygon outline is never a hole
/// source (G-DRILLCENTROID): only points and circles serve `Holes`.
pub(crate) fn generate_drill(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Drill, "generate_drill");
    // Checkpoint C, Q3 (F-4): a drill cycle is short, so the only cancel
    // poll is the dispatch prologue's (W2.2), ahead of any work. The drill
    // kernel has no poll of its own.
    let hole_xys = selected_hole_xys(prepared.holes()?);
    // D8 (W3.4, `RestUse::StockColumn`): each hole starts at the remaining
    // stock top over it. `build_drill_op_for_config` reads the same
    // columns through the same rule.
    let holes = drill_hole_columns(
        cfg,
        &hole_xys,
        ctx.stock_bbox,
        column_stock(op, prepared.territory().unguarded_stock()),
        ctx.tool_def.radius(),
    );
    let generated = generated_with_drill_spans(crate::ops::drill::drill_toolpath_at_holes(
        &holes,
        cfg.cycle.to_core(cfg),
        op.feed_rate(),
        ctx.heights.retract_z,
    ));
    if let Some(sem) = ctx.semantic_ctx {
        crate::compute::annotate::annotate_drill_spans(&generated.spans, &generated.toolpath, sem);
    }
    Ok(generated)
}

/// Alignment-pin drill family adapter: the live stock pins of its diameter
/// and the extra holes of its selection.
///
/// Cancellable since 2026-08-14 (O-CANC). Like `generate_drill`, the
/// cycle is short, so the only poll is the dispatch prologue's (W2.2). It
/// runs ahead of the empty-holes refusal, so a pre-set flag on a pin-drill
/// op with no pins reports "cancelled", not "No alignment pin positions
/// defined". Pinned by `cancellable_families_honour_a_preset_cancel_flag`.
pub(crate) fn generate_alignment_pin_drill(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, AlignmentPinDrill, "generate_alignment_pin_drill");
    // Stock alignment pins plus the extra holes of the selection — two
    // sources in two different frames, reconciled in one place so this
    // adapter and `build_drill_op_for_config` cannot disagree about where
    // the op drills. See `pin_holes_in_emission_frame` for both frames and
    // why neither correction may be applied to the other's holes
    // (G-PINDRILL-FRAME, G-DRILLPICK-FRAME).
    let holes =
        pin_holes_in_emission_frame(cfg, ctx.alignment_pins, ctx.stock_bbox, prepared.holes()?);
    if holes.is_empty() {
        return Err(OperationError::MissingGeometry(
            "No alignment pin positions defined".to_owned(),
        ));
    }
    let stock_z = ctx.stock_bbox.max.z - ctx.stock_bbox.min.z;
    let depth = stock_z + cfg.spoilboard_penetration;
    let cycle = cfg.drill_cycle(depth);
    let params = crate::ops::drill::DrillParams {
        depth,
        top_z: ctx.stock_bbox.max.z,
        cycle,
        feed_rate: cfg.feed_rate,
        safe_z: ctx.heights.retract_z,
        retract_z: crate::compute::config::effective_safe_z(cfg.retract_z, ctx.stock_bbox.max.z),
    };
    let generated = generated_with_drill_spans(crate::ops::drill::drill_toolpath(&holes, &params));
    if let Some(sem) = ctx.semantic_ctx {
        crate::compute::annotate::annotate_drill_spans(&generated.spans, &generated.toolpath, sem);
    }
    Ok(generated)
}
