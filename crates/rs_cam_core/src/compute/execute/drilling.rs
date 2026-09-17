//! The drilling family: drill-target resolution, pick matching and the two
//! drill adapters.
//!
//! Holds the refusal messages the GUI and the diagnostics adapters read, the
//! emission-frame conversions a pick needs, and
//! `generate_drill` / `generate_alignment_pin_drill`. Split out of
//! `compute/execute.rs` (P4).

use std::sync::atomic::Ordering;

use crate::compute::catalog::OperationConfig;
use crate::compute::tool_config::ToolConfig;
use crate::geo::BoundingBox3;
use crate::io::dxf_input::DrillTarget;
use crate::tool::{MillingCutter, ToolDefinition};

use super::shared::generated_with_drill_spans;
use super::{ExecutionContext, GeneratedToolpath, OperationError};

/// Build the [`crate::ops::drill_op::DrillOp`] for a drilling-cycle operation,
/// re-resolving hole positions from the same inputs the toolpath
/// generator just consumed.
///
/// Returns `None` for non-drill ops. The caller pairs this with the
/// `AnnotatedToolpath` produced by [`execute_operation_annotated`] to
/// form a [`crate::ops::drill_op::OpData::DrillOp`] — the dual-representation
/// invariant.
///
/// Hole-source asymmetry (§6.E):
/// - `OperationConfig::Drill`: holes are the model's drill targets
///   (`HoleSource::ModelDerived`); re-resolved every regenerate. An
///   explicit pick round-trips as a `Snapshot` instead.
/// - `OperationConfig::AlignmentPinDrill`: holes are snapshotted in
///   `cfg.holes`; `HoleSource::Snapshot` round-trips through project IO.
///
/// `setup_transform` is the same value the generation pass was given (see
/// [`ExecutionContext::setup_transform`]) and must stay that way: the
/// dual-representation invariant is that this view names the holes the
/// emitted toolpath actually drills, so a caller that transforms one and
/// not the other has published two different sets of coordinates for one
/// operation.
pub fn build_drill_op_for_config(
    op: &OperationConfig,
    drill_targets: &[DrillTarget],
    tool_def: &ToolDefinition,
    tool_cfg: &ToolConfig,
    stock_bbox: &BoundingBox3,
    material: crate::material::Material,
    setup_transform: Option<&crate::compute::transform::SetupTransformInfo>,
) -> Option<crate::ops::drill_op::DrillOp> {
    use crate::ops::drill_op::{DrillHole, DrillOp, HoleSource, ToolProfile};

    let tool_diameter_mm = tool_def.radius() * 2.0;
    let flute_count = tool_cfg.flute_count;

    match op {
        OperationConfig::Drill(cfg) => {
            // Selected targets (DXF picks) drill exactly those and round-trip
            // as a snapshot; otherwise every target the model exposes.
            let hole_xys = drill_holes_for_config(cfg, drill_targets, setup_transform).ok()?;
            let hole_source = if cfg.selected_holes.is_some() {
                HoleSource::Snapshot(hole_xys.clone())
            } else {
                HoleSource::ModelDerived
            };
            let top_z = stock_bbox.max.z;
            let bottom_z = top_z - cfg.depth;
            let holes = hole_xys
                .into_iter()
                .map(|xy| DrillHole {
                    xy,
                    top_z,
                    bottom_z,
                })
                .collect();
            Some(DrillOp {
                holes,
                hole_source,
                tool_profile: ToolProfile::Flat,
                tool_diameter_mm,
                cycle: cfg.cycle.to_core(cfg),
                feed_rate_mm_min: cfg.feed_rate,
                spindle_rpm: cfg.spindle_rpm.unwrap_or(0),
                flute_count,
                material,
                // Same expression `generate_drill` passes to
                // `DrillParams::retract_z`, so the summary models the
                // cycle this op emits (R-2).
                retract_z_mm: crate::compute::config::effective_safe_z(cfg.retract_z, top_z),
            })
        }
        OperationConfig::AlignmentPinDrill(cfg) => {
            // Both frame corrections `generate_alignment_pin_drill` applies,
            // applied identically here — this view has to name the holes
            // that toolpath drills, not the raw config numbers.
            let pin_holes =
                pin_holes_in_emission_frame(cfg, stock_bbox, setup_transform, drill_targets);
            let hole_xys = pin_holes.ok()?;
            if hole_xys.is_empty() {
                return None;
            }
            let top_z = stock_bbox.max.z;
            let bottom_z = stock_bbox.min.z - cfg.spoilboard_penetration;
            let cycle = cfg.drill_cycle(top_z - bottom_z);
            let holes = hole_xys
                .iter()
                .map(|&xy| DrillHole {
                    xy,
                    top_z,
                    bottom_z,
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
                retract_z_mm: crate::compute::config::effective_safe_z(cfg.retract_z, top_z),
            })
        }
        _ => None,
    }
}

/// Move a picked drill target from the world/model frame into the frame the
/// toolpath emits in.
///
/// G-DRILLPICK-FRAME (2026-08-19). `selected_holes` are raw model/DXF
/// coordinates — the viz picker maps `LoadedModel::drill_targets` straight
/// through — but the model's *polygons* are setup-transformed before
/// generation, and the toolpath emits in the setup frame: world for an
/// identity setup, zero-rooted setup-local otherwise. Consumed verbatim,
/// the picks were off by the whole setup transform on any non-identity
/// setup: on a `Bottom` flip of 240x250 stock a target picked at world
/// (30, 40) belongs at setup-local (50, 185), and the op drilled (30, 40).
///
/// The picks stay STORED in the world frame and are converted here, so no
/// saved project changes meaning. `None` — an identity setup — is the
/// no-op that keeps that true: world already IS the emission frame, so
/// there is nothing to apply and nothing to double-apply.
///
/// This is
/// [`crate::compute::transform::SetupTransformInfo::apply_to_drawing_polygons`]
/// for a bare point — deliberately the SAME door, because a pick is read off
/// the drawing and must land wherever that drawing's own points land. If the
/// two ever diverge, a drill and the feature that locates it end up in
/// different places on the same part.
///
/// That is also why the 2026-08-22 work-plane rule reaches here: on a lateral
/// setup a drawing is consumed verbatim in the work plane, so a target picked
/// off it is too. Nothing about the identity/`Bottom` cases changed.
fn pick_to_emission_frame(
    xy: [f64; 2],
    setup_transform: Option<&crate::compute::transform::SetupTransformInfo>,
) -> [f64; 2] {
    match setup_transform {
        Some(info) => {
            let local = info.drawing_to_local(crate::geo::P2::new(xy[0], xy[1]));
            [local.x, local.y]
        }
        None => xy,
    }
}

/// Every hole an [`crate::compute::operation_configs::AlignmentPinDrillConfig`]
/// drills, expressed in the emission frame — the ONE place the pin-drill's
/// two differently-framed hole sources are reconciled, shared by the
/// generator and by [`build_drill_op_for_config`] so the two cannot drift.
///
/// The two sources genuinely are in different frames:
///
/// - `cfg.holes` snapshots `StockConfig::alignment_pins`, which are
///   dimensioned STOCK-RELATIVE (X0Y0 at the stock's min corner) — the
///   frame the export datum converges on, because the pins are what
///   physically registers a flip. `ctx.stock_bbox` IS the stock in the
///   emission frame, so translating by its min corner serves both cases:
///   world for an identity setup, a provable no-op for a non-identity one
///   (`min == (0,0)`), which is why non-identity was accidentally correct
///   (G-PINDRILL-FRAME).
/// - `cfg.selected_holes` are picks in the WORLD frame and take the setup
///   transform instead, never the stock-relative translation
///   (G-DRILLPICK-FRAME). Applying both would move them twice.
///
/// The picks are also re-resolved against the model's current targets, and
/// this function refuses when one names none of them — the same
/// G-DRILLPICKSTALE contract [`drill_holes_for_config`] carries, through the
/// same two shared functions, because the two families store the identical
/// frozen coordinate. `cfg.holes` is a stock snapshot and is untouched by
/// that check.
fn pin_holes_in_emission_frame(
    cfg: &crate::compute::operation_configs::AlignmentPinDrillConfig,
    stock_bbox: &BoundingBox3,
    setup_transform: Option<&crate::compute::transform::SetupTransformInfo>,
    drill_targets: &[DrillTarget],
) -> Result<Vec<[f64; 2]>, OperationError> {
    let stock_origin = stock_bbox.min;
    let mut holes: Vec<[f64; 2]> = cfg
        .holes
        .iter()
        .map(|h| [h[0] + stock_origin.x, h[1] + stock_origin.y])
        .collect();
    if let Some(selected) = &cfg.selected_holes {
        if let Some(msg) = stale_drill_picks_refusal(selected, drill_targets) {
            return Err(OperationError::MissingGeometry(msg));
        }
        let resolved = resolve_drill_picks(selected, drill_targets);
        holes.extend(
            resolved
                .into_iter()
                .map(|xy| pick_to_emission_frame(xy, setup_transform)),
        );
    }
    Ok(holes)
}

/// The refusal a `Drill` op gives when the model exposes no
/// [`DrillTarget`] and nothing is picked. The GUI's static validator
/// (`validate_toolpath`) and the core precondition adapter print the same
/// sentence, so the Generate button, the diagnostics ribbon and the
/// generator agree (G-DRILLCENTROID, UX-R03-004).
pub const NO_DRILL_TARGETS_MSG: &str =
    "No drill targets — pick points/circles or import a drawing with circles";

/// The refusal for an explicit selection that is empty.
pub const NO_DRILL_TARGETS_SELECTED_MSG: &str =
    "No drill targets selected (pick points/holes in the viewport or choose a layer)";

/// The refusal a `Drill` op earns from its selection and the model's target
/// count alone, before any coordinate is read — `None` means it has a hole
/// source. ONE predicate for the generator ([`drill_holes_for_config`]), the
/// core precondition adapter and the GUI's static validator, so the Generate
/// button, the diagnostics ribbon and the generator cannot drift
/// (G-DRILLCENTROID).
pub fn drill_targets_refusal(
    cfg: &crate::compute::operation_configs::DrillConfig,
    drill_target_count: usize,
) -> Option<&'static str> {
    match &cfg.selected_holes {
        Some(picked) if !picked.is_empty() => None,
        Some(_) => Some(NO_DRILL_TARGETS_SELECTED_MSG),
        None => (drill_target_count == 0).then_some(NO_DRILL_TARGETS_MSG),
    }
}

/// The distance at which a stored pick and a [`DrillTarget`] are the same
/// point.
///
/// This is NOT a new tolerance. The GUI picker already compared picks to each
/// other at this value in two places — the panel's `TARGET_EPS` and the
/// viewport toggle's `EPS` — because a pick IS a copy of a target's `(x, y)`
/// and the two therefore agree bit-for-bit. Both sites now read this
/// constant, so the picker, the toggle and the generator cannot drift.
pub const DRILL_PICK_MATCH_EPS_MM: f64 = 1e-6;

/// Does `pick` name `target`?
pub fn drill_pick_matches(pick: [f64; 2], target: &DrillTarget) -> bool {
    (pick[0] - target.x).abs() < DRILL_PICK_MATCH_EPS_MM
        && (pick[1] - target.y).abs() < DRILL_PICK_MATCH_EPS_MM
}

/// The phrase every stale-pick refusal carries, so the operator can tell a
/// stale pick from the two refusals [`drill_targets_refusal`] already gives.
pub(crate) const STALE_DRILL_PICKS_PHRASE: &str = "no longer";

/// Refuse when a stored pick names no target on the model (G-DRILLPICKSTALE,
/// F4.8). `None` means every pick resolves, or that there is nothing to
/// resolve against.
///
/// # Why a pick can go stale
///
/// A pick is stored as a raw XY COORDINATE, not as a reference into the
/// model's targets — [`DrillTarget`] carries no id, and an index is worse
/// than a coordinate because deleting one hole shifts every later index and
/// would drill the wrong hole silently. So the coordinate IS the only
/// identity a pick has. F4.4 (G-RELOADTARGETS) made every GUI refresh door
/// replace `LoadedModel::drill_targets`, so the record follows the file and
/// the picks do not. An operator who moves a hole in CAD and reloads used to
/// get the previous version's position, with nothing said.
///
/// # Why an empty target list is exempt
///
/// An empty list means two different things at this seam, and this function
/// cannot tell them apart:
///
/// - the model exposes no targets, or
/// - the CALLER resolved no model — [`execute_operation_annotated`] passes
///   `&[]` deliberately, and states that a `Drill` op on that path drills
///   only what it carries in `selected_holes`.
///
/// With no target to compare against there is no evidence either way, so the
/// picks stand. The residual that leaves — the operator DELETES every hole
/// from the drawing, so the list goes empty and the stale picks stand — needs
/// a "was a model resolved" signal this seam does not carry. It is recorded
/// in `planning/ui_fix_2026-09-09/reports/F4.8.md`.
///
/// # Why the message says Clear, and not "re-pick"
///
/// The viewport draws one marker per model TARGET and colours it by whether
/// a pick names it (`app/gpu_upload.rs`, the drill-marker loop). A pick that
/// names no target is drawn nowhere, so the operator cannot see it and a
/// click cannot toggle it off — `AppEvent::ToggleDrillTarget` fires from a
/// target. Picking the moved hole ADDS it and leaves the stale coordinate in
/// the vector, so the op would refuse again. Clear is the only instruction
/// that works.
pub fn stale_drill_picks_refusal(
    picks: &[[f64; 2]],
    drill_targets: &[DrillTarget],
) -> Option<String> {
    if drill_targets.is_empty() {
        return None;
    }
    let mut stale = 0_usize;
    for &pick in picks {
        if !drill_targets.iter().any(|t| drill_pick_matches(pick, t)) {
            stale += 1;
        }
    }
    if stale == 0 {
        return None;
    }
    let total = picks.len();
    Some(format!(
        "Picked drill holes {STALE_DRILL_PICKS_PHRASE} match this model: \
         {stale} of {total} picks name no drill target. The drawing changed \
         after the pick. The viewport draws a marker only at a target, so a \
         stale pick is not on screen and a click cannot remove it. Press \
         Clear, then pick the holes again."
    ))
}

/// Move each pick onto the target it names. The result is still in the
/// model frame; both callers map it through [`pick_to_emission_frame`].
///
/// Resolving THROUGH the target makes the model the source of truth: a pick
/// that still names a target follows that target. Every pick reaching here
/// has already cleared [`stale_drill_picks_refusal`], so the fallback arm
/// (keep the pick) is the no-targets case that function exempts.
fn resolve_drill_picks(picks: &[[f64; 2]], drill_targets: &[DrillTarget]) -> Vec<[f64; 2]> {
    picks
        .iter()
        .map(|&pick| {
            drill_targets
                .iter()
                .find(|t| drill_pick_matches(pick, t))
                .map_or(pick, |t| [t.x, t.y])
        })
        .collect()
}

/// Resolve the drill hole positions for a [`DrillConfig`].
///
/// A hole position comes from a [`DrillTarget`] — a DXF `POINT` entity or a
/// circle/arc centre — or from an explicit pick. Never from a polygon
/// outline.
///
/// When `cfg.selected_holes` is set the user has explicitly picked targets
/// (in the viewport or by layer) — drill exactly those. An empty selection is
/// an error rather than "all targets", so a stale or cleared selection doesn't
/// silently revert to drilling everything.
///
/// When it is `None` (the default), drill every target the model exposes.
/// When the model exposes none, refuse with [`NO_DRILL_TARGETS_MSG`].
///
/// Until 2026-09-10 the `None` arm fell back to the vertex centroid of every
/// closed polygon in the model, so a drawing with no circles or points
/// drilled a hole through the middle of each shape (G-DRILLCENTROID,
/// UX-R03-004). The fallback is removed, not gated: the generator reads
/// targets only. A circle in an SVG still drills, because the 2D import
/// doors classify a circle-like closed ring as a target
/// (`svg_input::circle_like_drill_targets`) — a star or an outline is not
/// one.
///
/// Picks and targets share one frame — the viz picker copies a target's
/// `(x, y)` straight into `selected_holes` — so both take `setup_transform`
/// (G-DRILLPICK-FRAME — see [`pick_to_emission_frame`]).
///
/// A pick is re-resolved against the model's CURRENT targets and the op
/// refuses when one names none of them (G-DRILLPICKSTALE — see
/// [`stale_drill_picks_refusal`]).
pub(super) fn drill_holes_for_config(
    cfg: &crate::compute::operation_configs::DrillConfig,
    drill_targets: &[DrillTarget],
    setup_transform: Option<&crate::compute::transform::SetupTransformInfo>,
) -> Result<Vec<[f64; 2]>, OperationError> {
    if let Some(msg) = drill_targets_refusal(cfg, drill_targets.len()) {
        return Err(OperationError::MissingGeometry(msg.to_owned()));
    }
    if let Some(selected) = &cfg.selected_holes {
        if let Some(msg) = stale_drill_picks_refusal(selected, drill_targets) {
            return Err(OperationError::MissingGeometry(msg));
        }
        let resolved = resolve_drill_picks(selected, drill_targets);
        return Ok(resolved
            .into_iter()
            .map(|xy| pick_to_emission_frame(xy, setup_transform))
            .collect());
    }
    Ok(drill_targets
        .iter()
        .map(|t| pick_to_emission_frame([t.x, t.y], setup_transform))
        .collect())
}

/// Drill family adapter (holes from the model's drill targets or picks).
pub(crate) fn generate_drill(
    ctx: &ExecutionContext<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Drill, "generate_drill");
    // Checkpoint C, Q3: drill is cancellable now. It never touched
    // `ctx.cancel` at all (F-4). A drill cycle is short, so the check is at
    // the entry point rather than per hole — the point is that a pre-set flag
    // must short-circuit before any work, which is the same contract every
    // other cancellable family's first statement provides.
    if ctx.cancel.load(Ordering::SeqCst) {
        return Err(OperationError::Cancelled);
    }
    let holes = drill_holes_for_config(cfg, ctx.drill_targets, ctx.setup_transform)?;
    let cycle = cfg.cycle.to_core(cfg);
    let params = crate::ops::drill::DrillParams {
        depth: cfg.depth,
        top_z: ctx.stock_bbox.max.z,
        cycle,
        feed_rate: op.feed_rate(),
        safe_z: ctx.heights.retract_z,
        retract_z: crate::compute::config::effective_safe_z(cfg.retract_z, ctx.stock_bbox.max.z),
    };
    let generated = generated_with_drill_spans(crate::ops::drill::drill_toolpath(&holes, &params));
    if let Some(sem) = ctx.semantic_ctx {
        crate::compute::annotate::annotate_drill_spans(&generated.spans, &generated.toolpath, sem);
    }
    Ok(generated)
}

/// Alignment-pin drill family adapter (holes from the stock snapshot).
///
/// Cancellable since 2026-08-14 (O-CANC). Like `generate_drill`, the
/// cycle is short, so the poll is at the entry point rather than per
/// hole — but it is the FIRST statement, ahead of the empty-holes
/// refusal, for the same reason rest's is ahead of its prev-tool
/// precondition: a pre-set flag on a pin-drill op with no pins must
/// report "cancelled", not "No alignment pin positions defined". Pinned
/// by `cancellable_families_honour_a_preset_cancel_flag`.
pub(crate) fn generate_alignment_pin_drill(
    ctx: &ExecutionContext<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    if ctx.cancel.load(Ordering::SeqCst) {
        return Err(OperationError::Cancelled);
    }
    let cfg = config_guard!(op, AlignmentPinDrill, "generate_alignment_pin_drill");
    // Stock alignment pins plus any extra targets picked from the model —
    // two sources in two different frames, reconciled in one place so this
    // adapter and `build_drill_op_for_config` cannot disagree about where
    // the op drills. See `pin_holes_in_emission_frame` for both frames and
    // why neither correction may be applied to the other's holes
    // (G-PINDRILL-FRAME, G-DRILLPICK-FRAME).
    let targets = ctx.drill_targets;
    let holes = pin_holes_in_emission_frame(cfg, ctx.stock_bbox, ctx.setup_transform, targets)?;
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
