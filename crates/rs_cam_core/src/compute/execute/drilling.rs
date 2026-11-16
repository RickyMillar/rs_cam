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
use crate::compute::config::ResolvedHeights;
use crate::compute::floor::HoleSpan;
use crate::compute::tool_config::ToolConfig;
use crate::geo::BoundingBox3;
use crate::selection::{Holes, KEY_MATCH_EPS_MM, ResolvedSelection, SetupFrame};
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
/// P6.5) with the same tool diameter and size tolerance, so it names the
/// holes the emitted toolpath drills.
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
#[expect(
    clippy::too_many_arguments,
    reason = "each argument is one generator input this view must mirror"
)]
pub fn build_drill_op_for_config(
    op: &OperationConfig,
    selection: &ResolvedSelection<SetupFrame>,
    alignment_pins: &[crate::compute::alignment_pins::AlignmentPin],
    tool_def: &ToolDefinition,
    tool_cfg: &ToolConfig,
    stock_bbox: &BoundingBox3,
    heights: &ResolvedHeights,
    initial_stock: Option<&crate::dexel_stock::TriDexelStock>,
    material: crate::material::Material,
) -> Option<crate::ops::drill_op::DrillOp> {
    use crate::ops::drill_op::{DrillHole, DrillOp, HoleSource, ToolProfile};

    let tool_diameter_mm = tool_def.radius() * 2.0;
    let flute_count = tool_cfg.flute_count_planning();
    // THE item pipeline of the prologue (H1), with no mesh: a drill op
    // reads no step that needs one.
    let holes_of = || {
        let tool = crate::selection::OpTool {
            cutter: tool_def,
            config: tool_cfg,
        };
        let outcome = crate::selection::op_items(selection, op, Some(tool), None).ok()?;
        outcome.items.holes().cloned()
    };

    match op {
        OperationConfig::Drill(cfg) => {
            // R-B, F19: the per-hole floor rule of the prologue, on the same
            // heights.
            let fitted = holes_of()?;
            let spans = crate::compute::floor::hole_floors(
                selection,
                &fitted,
                &cfg.floor,
                &crate::compute::floor::HoleFloorInputs::new(heights, stock_bbox.min.z),
            )
            .ok()?;
            let holes = drill_hole_columns(
                cfg,
                &selected_holes(&fitted, &spans),
                DrillTops {
                    top_z: heights.top_z,
                    stock_top_z: stock_bbox.max.z,
                },
                column_stock(op, initial_stock),
                tool_def.radius(),
            );
            Some(DrillOp {
                holes,
                hole_source: HoleSource::ModelDerived,
                tool_profile: ToolProfile::Flat,
                tool_diameter_mm,
                cycle: cfg.cycle.to_core(cfg).ok()?,
                feed_rate_mm_min: cfg.feed_rate?,
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
            let cycle = cfg.drill_cycle(top_z - bottom_z).ok()?;
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
                feed_rate_mm_min: cfg.feed_rate?,
                spindle_rpm: cfg.spindle_rpm.unwrap_or(0),
                flute_count,
                material,
            })
        }
        OperationConfig::Face(_)
        | OperationConfig::Pocket(_)
        | OperationConfig::Profile(_)
        | OperationConfig::Adaptive(_)
        | OperationConfig::VCarve(_)
        | OperationConfig::Inlay(_)
        | OperationConfig::InlayPlug(_)
        | OperationConfig::Zigzag(_)
        | OperationConfig::Trace(_)
        | OperationConfig::Bore(_)
        | OperationConfig::Chamfer(_)
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
        | OperationConfig::HorizontalFinish(_)
        | OperationConfig::ProjectCurve(_) => None,
    }
}

/// F30 (after F19): the XY and the depth base of each hole of a Drill whose
/// floor is a depth below the Top Z ([`crate::compute::floor::Floor::Top`]),
/// by the prologue rule ([`crate::selection::op_items`], then the per-hole
/// floors). The model gouge text names the depth to type from
/// the base of the hole it gouges. Empty for every other op and floor.
#[must_use]
pub fn drill_depth_bases(
    op: &OperationConfig,
    selection: &ResolvedSelection<SetupFrame>,
    tool_def: &ToolDefinition,
    tool_cfg: &ToolConfig,
    stock_bbox: &BoundingBox3,
    heights: &ResolvedHeights,
) -> Vec<([f64; 2], f64)> {
    let OperationConfig::Drill(cfg) = op else {
        return Vec::new();
    };
    if cfg.floor.depth_below_top().is_none() {
        return Vec::new();
    }
    let tool = crate::selection::OpTool {
        cutter: tool_def,
        config: tool_cfg,
    };
    let Ok(outcome) = crate::selection::op_items(selection, op, Some(tool), None) else {
        return Vec::new();
    };
    let items = outcome.items;
    let Some(holes) = items.holes() else {
        return Vec::new();
    };
    crate::compute::floor::hole_floors(
        selection,
        holes,
        &cfg.floor,
        &crate::compute::floor::HoleFloorInputs::new(heights, stock_bbox.min.z),
    )
    .map(|spans| {
        holes
            .as_slice()
            .iter()
            .zip(spans)
            .map(|(h, span)| ([h.xy.x, h.xy.y], span.base_z))
            .collect()
    })
    .unwrap_or_default()
}

/// The XY and the span (R-B: depth base and floor) of each selected hole,
/// in selection order, with every repeat dropped ([`unique_holes`]: the
/// first copy keeps its span). `holes` are the FITTED holes (P6.5): the
/// prologue's fit step left out each circle that is not of the tool size,
/// so the XY alone is the whole hole (the tool makes its size). The holes are in the emission frame
/// already: the frame door (`ResolvedSelection::to_setup_frame`) moved
/// each point and circle centre as it moved the drawing, so a hole lands
/// where the feature that locates it lands (G-DRILLPICK-FRAME). `spans`
/// is index-aligned with `holes` ([`crate::compute::floor::hole_floors`]).
fn selected_holes(holes: &Holes, spans: &[HoleSpan]) -> Vec<([f64; 2], HoleSpan)> {
    let pairs: Vec<([f64; 2], HoleSpan)> = holes
        .as_slice()
        .iter()
        .zip(spans)
        .map(|(h, span)| ([h.xy.x, h.xy.y], *span))
        .collect();
    let kept = unique_holes(pairs.iter().map(|(xy, _)| *xy).collect());
    kept.into_iter()
        .filter_map(|xy| pairs.iter().find(|(p, _)| *p == xy).copied())
        .collect()
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
pub(crate) fn pin_holes_in_emission_frame(
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
/// F19 (one rule): the cut starts at the resolved Top Z `top_z` of the op
/// (auto: the higher of the highest hole rim and the stock top; else the
/// pinned top), as the Bore does. Each hole stops at the floor of its span
/// (R-B: `compute::floor::hole_floors`, a depth from its own rim or its
/// own hole floor).
///
/// On fresh stock (`initial_stock == None`) every hole runs from `top_z`
/// to its floor.
///
/// On the remaining stock the top of each hole is the highest material in
/// the reach of the tool over the hole (`standing_material::column_top`
/// with the tool radius), capped at `top_z`. A hole in a cleared pocket
/// then starts at the pocket floor, not in the air above it.
///
/// P36 A1 (the R-plane rule): the R-plane of a hole is the clearance above
/// the material that STANDS over the hole, not above the cut start. The
/// two differ when the Top Z is pinned below the wood: the wanaka Drill
/// (Top Z 16.5, stock to 25.5 over one hole) sent a rapid to Z 21.5.
///
/// - With stock evidence the standing top is the column top, not capped.
/// - With no evidence (fresh stock, or a hole that the grid cannot see) it
///   is [`crate::dressup::fresh_stock_top_z`] (G-PECKSPLIT, ruling R1): a
///   pinned top below the stock top does not lower a rapid plane.
/// - A standing top at or below `top_z` keeps the clearance a fresh hole
///   has above `top_z`, so the first peck of a hole in a cleared pocket
///   starts that clearance above the floor (D8). A standing top above
///   `top_z` is a fresh column of that height:
///   `effective_safe_z(cfg.retract_z, standing top)`. The two agree at
///   `top_z`.
///
/// A hole whose column holds no material down to its bottom has nothing
/// to cut and is dropped. The stock is read only when the W2.1
/// guard (`StockReference::resolve`) finds it in the frame of the holes.
fn drill_hole_columns(
    cfg: &crate::compute::operation_configs::DrillConfig,
    holes_in: &[([f64; 2], HoleSpan)],
    tops: DrillTops,
    initial_stock: Option<&crate::dexel_stock::TriDexelStock>,
    tool_radius: f64,
) -> Vec<crate::ops::drill_op::DrillHole> {
    use super::standing_material::{ColumnTop, column_top};
    use crate::ops::drill_op::DrillHole;

    if holes_in.is_empty() {
        return Vec::new();
    }
    let top_z = tops.top_z;
    let clearance = crate::compute::config::effective_safe_z(cfg.retract_z, top_z) - top_z;
    // The standing top where the stock gives no answer.
    let fresh_top = crate::dressup::fresh_stock_top_z(tops.stock_top_z, top_z);
    let r_plane_over = |standing_top: f64| {
        if standing_top > top_z {
            crate::compute::config::effective_safe_z(cfg.retract_z, standing_top)
        } else {
            standing_top + clearance
        }
    };
    let deepest = holes_in
        .iter()
        .map(|(_, span)| span.floor_z)
        .fold(f64::INFINITY, f64::min);
    let holes_bbox = holes_in.iter().fold(
        BoundingBox3 {
            min: crate::geo::P3::new(f64::INFINITY, f64::INFINITY, deepest),
            max: crate::geo::P3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, top_z),
        },
        |mut b, ([x, y], _)| {
            b.min.x = b.min.x.min(*x);
            b.min.y = b.min.y.min(*y);
            b.max.x = b.max.x.max(*x);
            b.max.y = b.max.y.max(*y);
            b
        },
    );
    let stock =
        crate::surface::stock_reference::StockReference::resolve(initial_stock, &holes_bbox)
            .stock();
    let mut dropped = 0_usize;
    let holes: Vec<DrillHole> = holes_in
        .iter()
        .filter_map(|&(xy, span)| {
            let bottom_z = span.floor_z;
            // The cut start, and the top of the material that stands over
            // the hole (the R-plane reads the second).
            let (hole_top, standing_top) = match stock
                .map(|s| column_top(s, crate::geo::P2::new(xy[0], xy[1]), tool_radius))
            {
                None | Some(ColumnTop::NoEvidence) => (top_z, fresh_top),
                Some(ColumnTop::Top(t)) => (t.min(top_z), t),
                Some(ColumnTop::Empty) => (bottom_z, bottom_z),
            };
            if hole_top <= bottom_z {
                dropped += 1;
                return None;
            }
            Some(DrillHole {
                xy,
                top_z: hole_top,
                bottom_z,
                retract_z: r_plane_over(standing_top),
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

/// The two tops a Drill reads (P36 A1): where the cut starts, and where the
/// stock box ends. Named fields, so a caller cannot swap them.
#[derive(Debug, Clone, Copy)]
struct DrillTops {
    /// The resolved Top Z of the op (F19): the cut start.
    top_z: f64,
    /// The top of the stock box in the emission frame.
    stock_top_z: f64,
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
    // R-B, F19: each hole with its span from the prologue (THE per-hole
    // floor rule, `compute::floor::hole_floors`).
    let spans = prepared.hole_spans().ok_or_else(|| {
        OperationError::Internal("the Drill prologue gave no hole spans".to_owned())
    })?;
    let holes_in = selected_holes(prepared.holes()?, spans);
    // D8 (W3.4, `RestUse::StockColumn`): each hole starts at the remaining
    // stock top over it, capped at the Top Z. `build_drill_op_for_config`
    // reads the same columns through the same rule.
    let holes = drill_hole_columns(
        cfg,
        &holes_in,
        DrillTops {
            top_z: ctx.heights.top_z,
            stock_top_z: ctx.stock_bbox.max.z,
        },
        column_stock(op, prepared.territory().unguarded_stock()),
        ctx.tool_def.radius(),
    );
    let generated = generated_with_drill_spans(crate::ops::drill::drill_toolpath_at_holes(
        &holes,
        cfg.cycle.to_core(cfg)?,
        op.cut_feeds()?.feed_rate,
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
    let cycle = cfg.drill_cycle(depth)?;
    let params = crate::ops::drill::DrillParams {
        depth,
        top_z: ctx.stock_bbox.max.z,
        cycle,
        feed_rate: op.cut_feeds()?.feed_rate,
        safe_z: ctx.heights.retract_z,
        retract_z: crate::compute::config::effective_safe_z(cfg.retract_z, ctx.stock_bbox.max.z),
    };
    let generated = generated_with_drill_spans(crate::ops::drill::drill_toolpath(&holes, &params));
    if let Some(sem) = ctx.semantic_ctx {
        crate::compute::annotate::annotate_drill_spans(&generated.spans, &generated.toolpath, sem);
    }
    Ok(generated)
}
