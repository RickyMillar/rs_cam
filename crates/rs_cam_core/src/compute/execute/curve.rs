//! W6.5: the runner of the Curve shape. A curve op projects 2D curves onto
//! a surface and cuts along them (Project Curve).
//!
//! The op writes:
//!
//! - [`CurveOp::plan`]: one curve, projected through the height query, in
//!   the frame of the setup;
//! - [`CurveOp::link`]: its own link body (the op chooses the relink
//!   policy). The default links nothing.
//!
//! [`run_curve`] owns, for every curve op:
//!
//! - the Z frame ([`CurveFrame`], from `ExecutionContext::z_flipped`);
//! - the height query (W6.3 `HeightQuery::Mesh`: the mesh and the cutter
//!   of the context);
//! - the cancel, the per-curve loop and the concatenation;
//! - the link inputs, resolved once ([`CurveLink`]): the territory
//!   regions (W6.2), the ONE link ceiling of the op's row
//!   (`op_link_ceiling`, G-LINKCEIL) and the link kinematics;
//! - the engagement guard ([`BuriedCurveFinding`]), on the planned moves
//!   before the link;
//! - the cut-run spans and the depth-run annotation.

use crate::compute::catalog::OperationType;
use crate::compute::toolpath_stats::BuriedCurveFinding;
use crate::finish::surface_link::LinkCeiling;
use crate::geometry::region_set::RegionSet;
use crate::interrupt::CancelCheck;
use crate::machine::kinematics::LinkKinematics;
use crate::polygon::Polygon2;
use crate::toolpath::Toolpath;

use super::height_field::HeightQuery;
use super::shared::{generated_with_cut_run_spans, op_link_ceiling, with_depth_run_annotation};
use super::{ExecutionContext, GeneratedToolpath, OperationError, Prepared};

/// The Z frame of a curve op.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CurveFrame {
    /// True when the setup transform already inverted Z (a Bottom setup).
    /// Then the op must not apply its own Z flip.
    pub z_flipped: bool,
}

/// The link inputs of a curve op. The runner resolves each one once.
pub(crate) struct CurveLink<'a> {
    /// The territory regions (W6.2). A link may not leave them.
    pub regions: Option<&'a RegionSet<'a>>,
    /// The ONE link ceiling of the op's row (G-LINKCEIL).
    pub ceiling: Option<LinkCeiling<'a>>,
    /// The machine envelope of the link cost.
    pub kinematics: Option<&'a LinkKinematics>,
}

/// One curve op. See the module doc for what the runner owns.
pub(crate) trait CurveOp {
    /// The label prefix of the cut-run spans.
    const SPAN_LABEL: &'static str;

    /// The commanded cut depth (mm) below the surface. The engagement
    /// guard compares the measured engagement with it.
    fn commanded_depth_mm(&self) -> f64;

    /// Project and cut one curve.
    fn plan(
        &self,
        curve: &Polygon2,
        query: HeightQuery<'_>,
        frame: CurveFrame,
        cancel: &dyn CancelCheck,
    ) -> Result<Toolpath, OperationError>;

    /// Join the planned curves. The default links nothing.
    fn link(
        &self,
        planned: Toolpath,
        _query: HeightQuery<'_>,
        _frame: CurveFrame,
        _link: &CurveLink<'_>,
    ) -> Toolpath {
        planned
    }
}

/// THE runner of a curve op. See the module doc for what it owns.
pub(crate) fn run_curve<O: CurveOp>(
    op: &O,
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
) -> Result<GeneratedToolpath, OperationError> {
    let op_type: OperationType = prepared.op_type();
    let curves = prepared.polygons()?;
    let query = HeightQuery::Mesh {
        surface: prepared.mesh()?,
        cutter: ctx.tool_def,
    };
    let frame = CurveFrame {
        z_flipped: ctx.z_flipped(),
    };
    let cancel = ctx.cancel_fn();
    let mut planned = Toolpath::new();
    for curve in curves {
        planned
            .moves
            .extend(op.plan(curve, query, frame, &cancel)?.moves);
    }
    if let Some(finding) = BuriedCurveFinding::measure(
        &planned,
        op.commanded_depth_mm(),
        ctx.initial_stock,
        ctx.tool_def,
    ) {
        ctx.record(&finding);
    }
    let link = CurveLink {
        regions: prepared.territory().regions(),
        ceiling: op_link_ceiling(op_type, ctx),
        kinematics: ctx.link_kinematics.as_ref(),
    };
    let linked = op.link(planned, query, frame, &link);
    Ok(with_depth_run_annotation(
        generated_with_cut_run_spans(linked, O::SPAN_LABEL),
        ctx.semantic_ctx,
    ))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::compute::cutter::build_cutter;
    use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
    use crate::dexel_stock::TriDexelStock;
    use crate::geo::{BoundingBox3, P3};

    /// A flat block, top at Z 10, cell 0.25 mm.
    fn block() -> TriDexelStock {
        TriDexelStock::from_bounds(
            &BoundingBox3 {
                min: P3::new(0.0, 0.0, 0.0),
                max: P3::new(20.0, 20.0, 10.0),
            },
            0.25,
        )
    }

    /// One straight cut across the block with the tip at `z`.
    fn cut_at(z: f64) -> Toolpath {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(10.0, 5.0, 15.0));
        tp.feed_to(P3::new(10.0, 5.0, z), 300.0);
        tp.feed_to(P3::new(10.0, 15.0, z), 800.0);
        tp
    }

    /// The threshold of the engagement guard: a cut at the commanded depth
    /// below the stock is silent; a deeper cut is named; no stock, no claim.
    #[test]
    fn the_buried_curve_threshold_is_the_commanded_depth_plus_the_floor() {
        let stock = block();
        let cutter = build_cutter(&ToolConfig::new_default(ToolId(0), ToolType::EndMill));
        let depth = 1.0;
        assert_eq!(
            BuriedCurveFinding::measure(&cut_at(10.0 - depth), depth, Some(&stock), &cutter),
            None,
            "the stock is the surface: the tool cuts the commanded depth"
        );
        let f =
            BuriedCurveFinding::measure(&cut_at(10.0 - depth - 0.5), depth, Some(&stock), &cutter)
                .expect("0.5 mm deeper than commanded is named");
        assert!((f.deepest_engagement_mm - 1.5).abs() < 1e-6, "{f:?}");
        assert!(f.sampled_positions > 0);
        assert_eq!(
            BuriedCurveFinding::measure(&cut_at(8.5), depth, None, &cutter),
            None,
            "no stock: nothing measured, nothing claimed (X-19)"
        );
    }
}
