//! W6.5: the runner of the Volumetric shape. A volumetric op runs a planner
//! that owns its own stock (3D Rough, `Adaptive3d`).
//!
//! The planner owns more than the runner does, and the shape says so: each
//! op states [`VolumetricOp::OWNS`] (what its planner owns) and
//! [`VolumetricOp::HEIGHTS`] (how it reads the resolved heights). The
//! sentry `the_declaration_matches_the_row` holds both equal to the
//! registry row.
//!
//! [`run_volumetric`] owns, for every volumetric op:
//!
//! - the read of the territory (`TerritoryXy::PlannerBounds`: the outline
//!   of the planner pre-clear and the centre containment, W6.2);
//! - the seed stock: `Territory::unguarded_stock`, RAW (C3). The W2.1
//!   guard of `Territory::reference_stock` can drop a stock the planner
//!   must seed from, and that changes output;
//! - the top Z and the floor, from [`HeightsPolicy`];
//! - the planner-only channel `segment_merge_tolerance`, when the planner
//!   owns the merge;
//! - the cancel closure and the debug context;
//! - the semantic trace, the spans and the attach of `Produced`.
//!
//! The op owns the config-to-planner param build. The planner keeps its
//! own stock, Z ladder, level anchor, entries, stay-down links, segment
//! merge, centre clip and region order (`adaptive3d/`). The runner builds
//! no ladder and does not call `run_layered`.

use crate::dexel_stock::TriDexelStock;
use crate::interrupt::CancelCheck;
use crate::polygon::Polygon2;
use crate::toolpath::Toolpath;
use crate::trace::debug_trace::ToolpathDebugContext;
use crate::trace::produced::Produced;
use crate::trace::semantic_trace::ToolpathSemanticContext;
use crate::trace::toolpath_spans::Span;

use super::shared::generated_with_spans;
use super::{ExecutionContext, GeneratedToolpath, MeshInput, OperationError, Prepared};

/// What a planner that owns its stock states about itself (W6.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PlannerOwns {
    /// It seeds its own dexel stock from the seed stock
    /// (`RestUse::Planner`).
    pub stock: bool,
    /// It anchors its levels on its own stock (G-LADDERANCHOR).
    pub level_anchor: bool,
    /// It plans its entries; the dressup entry is forced off
    /// (`DressupPolicy::FORCE_NO_ENTRY`).
    pub entries: bool,
    /// It plans its stay-down links; no shared link stage and no rapid
    /// reorder (`LinkCeilingNeed::NoLink`).
    pub links: bool,
    /// It applies the segment merge before it stamps (G-PLANSIMGAP).
    pub segment_merge: bool,
}

/// Where the planner's top Z comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TopAnchor {
    /// The top of the stock box. A pinned `top_z` is NOT read (heights
    /// audit 2026-06-12, finding 2; the F-027 shape; the F-034 calibration
    /// anchor). Roughing starts from the real material top.
    StockBox,
}

/// How the planner reads the resolved heights.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct HeightsPolicy {
    pub top: TopAnchor,
    /// A pinned bottom Z floors the plan. True exactly when the row's
    /// `Capabilities::floor_owner` is `FloorOwner::HeightsBottom`.
    pub pinned_bottom: bool,
}

/// What the runner resolves once and gives to the planner.
pub(crate) struct VolumetricInputs<'a> {
    pub surface: MeshInput<'a>,
    /// The outline of the planner pre-clear (W6.2 `PlannerBounds`).
    pub outline: Option<&'a Polygon2>,
    /// The centre containment (G-BOUNDARYPHANTOM).
    pub centre: Option<&'a [Polygon2]>,
    /// The world stock XY box `(x_min, y_min, x_max, y_max)` (F-027).
    pub world_stock_xy: (f64, f64, f64, f64),
    /// The seed stock, raw (not guarded). `None`: a fresh block.
    pub seed_stock: Option<&'a TriDexelStock>,
    /// The top Z, from [`HeightsPolicy::top`].
    pub top_z: f64,
    /// The floor, from [`HeightsPolicy::pinned_bottom`]. `None`: the
    /// surface sets the floor.
    pub z_floor: Option<f64>,
    /// The retract plane.
    pub safe_z: f64,
    /// The segment merge tolerance, when the planner owns the merge and
    /// the dressup is on.
    pub segment_merge_tolerance: Option<f64>,
}

/// What the planner returns.
pub(crate) struct VolumetricOutput<A> {
    pub toolpath: Toolpath,
    /// The planner's per-move events, for the spans and the semantic trace.
    pub annotations: Vec<A>,
    /// The side channel: `planner_engagement` and `area_regions`.
    pub produced: Produced,
}

/// One volumetric op. See the module doc for what the runner owns.
pub(crate) trait VolumetricOp {
    /// What the planner owns.
    const OWNS: PlannerOwns;
    /// How the planner reads the heights.
    const HEIGHTS: HeightsPolicy;
    /// The planner's per-move event.
    type Annotation;

    /// Build the planner params from the config and run the planner.
    fn plan(
        &self,
        inputs: &VolumetricInputs<'_>,
        cancel: &dyn CancelCheck,
        debug: Option<&ToolpathDebugContext>,
    ) -> Result<VolumetricOutput<Self::Annotation>, OperationError>;

    /// The spans of the planned toolpath.
    fn spans(&self, annotations: &[Self::Annotation], toolpath: &Toolpath) -> Vec<Span>;

    /// Write the semantic trace.
    fn annotate(
        &self,
        annotations: &[Self::Annotation],
        toolpath: &Toolpath,
        sem: &ToolpathSemanticContext,
    );
}

/// THE runner of a volumetric op. See the module doc for what it owns.
pub(crate) fn run_volumetric<O: VolumetricOp>(
    op: &O,
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
) -> Result<GeneratedToolpath, OperationError> {
    let surface = prepared.mesh()?;
    let (outline, centre) = prepared.territory().planner_bounds();
    let bbox = ctx.stock_bbox;
    let inputs = VolumetricInputs {
        surface,
        outline,
        centre,
        world_stock_xy: (bbox.min.x, bbox.min.y, bbox.max.x, bbox.max.y),
        seed_stock: prepared.territory().unguarded_stock(),
        top_z: match O::HEIGHTS.top {
            TopAnchor::StockBox => bbox.max.z,
        },
        z_floor: (O::HEIGHTS.pinned_bottom && ctx.heights.bottom_pinned)
            .then_some(ctx.heights.bottom_z),
        safe_z: ctx.heights.retract_z,
        segment_merge_tolerance: if O::OWNS.segment_merge {
            ctx.segment_merge_tolerance
        } else {
            None
        },
    };
    let cancel = ctx.cancel_fn();
    let VolumetricOutput {
        toolpath,
        annotations,
        produced,
    } = op.plan(&inputs, &cancel, ctx.debug_ctx)?;
    if let Some(sem) = ctx.semantic_ctx {
        op.annotate(&annotations, &toolpath, sem);
    }
    let spans = op.spans(&annotations, &toolpath);
    let mut generated = generated_with_spans(toolpath, spans);
    // W6.2: the planner's side channel rides the result. The feed
    // modulator reads the engagement samples after the dressups (Stage 4);
    // the viewport overlay and MCP read the By Area detection.
    generated.produced = produced;
    Ok(generated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compute::catalog::{EntryStylePolicy, OperationType, RestUse};
    use crate::compute::execute::rough_3d::Adaptive3dRough;
    use crate::compute::execute::shared::LinkCeilingNeed;

    /// The declaration of a volumetric op against its registry row.
    fn mismatches(op: OperationType, owns: PlannerOwns, heights: HeightsPolicy) -> Vec<String> {
        let row = op.registry_entry();
        let caps = op.capabilities();
        let mut out = Vec::new();
        let mut check = |name: &str, declared: bool, row_says: bool| {
            if declared != row_says {
                out.push(format!(
                    "{}: OWNS/HEIGHTS.{name} is {declared}, the row says {row_says}",
                    op.name()
                ));
            }
        };
        check(
            "entries",
            owns.entries,
            row.dressup_policy.entry == EntryStylePolicy::ForceNone,
        );
        check(
            "segment_merge",
            owns.segment_merge,
            caps.transform.planner_applies_segment_merge,
        );
        check(
            "links",
            owns.links,
            caps.link_ceiling == LinkCeilingNeed::NoLink && !caps.transform.allows_rapid_reorder,
        );
        check("stock", owns.stock, caps.rest_use == RestUse::Planner);
        check(
            "pinned_bottom",
            heights.pinned_bottom,
            match caps.floor_owner {
                crate::compute::catalog::FloorOwner::HeightsBottom => true,
                crate::compute::catalog::FloorOwner::OpFloor
                | crate::compute::catalog::FloorOwner::OpDial => false,
            },
        );
        check(
            "top",
            matches!(heights.top, TopAnchor::StockBox),
            caps.top_on_stock_box,
        );
        out
    }

    /// W6.5: `OWNS` and `HEIGHTS` agree with the flags that live in
    /// the registry row, so a wrapper never claims what the row denies.
    #[test]
    fn the_declaration_matches_the_row() {
        let found = mismatches(
            OperationType::Adaptive3d,
            Adaptive3dRough::OWNS,
            Adaptive3dRough::HEIGHTS,
        );
        assert!(found.is_empty(), "{found:#?}");
        // Non-vacuity: a flipped flag is caught.
        let flipped = PlannerOwns {
            links: !Adaptive3dRough::OWNS.links,
            ..Adaptive3dRough::OWNS
        };
        assert_eq!(
            mismatches(OperationType::Adaptive3d, flipped, Adaptive3dRough::HEIGHTS).len(),
            1,
            "a flipped declaration must fail the check"
        );
    }
}
