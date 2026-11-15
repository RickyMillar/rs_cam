//! W6.5: the runner of the Curve shape. A curve op projects 2D curves onto
//! a surface and cuts along them (Project Curve).
//!
//! The op writes:
//!
//! - [`CurveOp::plan`]: one curve, projected through the height query, in
//!   the frame of the setup;
//! - [`CurveOp::link`]: its own link body (the op chooses the relink
//!   policy). The default links nothing. The body returns the link
//!   totals when its stage ran.
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
//! - the record of the link totals, only when the link stage ran
//!   (G-LINKVISIBLE, D2);
//! - the cut-run spans and the depth-run annotation;
//! - P35: the top of the pass ladder ([`ladder_top_z`]): the op top Z, or
//!   the stock top under the curve for an op from remaining stock whose
//!   rest rule is `RestUse::LadderTop`.

use crate::compute::catalog::OperationType;
use crate::compute::toolpath_stats::BuriedCurveFinding;
use crate::finish::surface_link::LinkCeiling;
use crate::finish::unified_finish::RelinkTotals;
use crate::geometry::region_set::RegionSet;
use crate::interrupt::CancelCheck;
use crate::machine::kinematics::LinkKinematics;
use crate::polygon::Polygon2;
use crate::toolpath::Toolpath;

use super::height_field::HeightQuery;
use super::shared::{
    generated_with_cut_run_spans, generated_with_spans, op_link_ceiling, with_depth_run_annotation,
};
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

    /// Join the planned curves. The totals are `Some` only when the link
    /// stage ran; the runner records them (G-LINKVISIBLE). The default
    /// links nothing.
    fn link(
        &self,
        planned: Toolpath,
        _query: HeightQuery<'_>,
        _frame: CurveFrame,
        _link: &CurveLink<'_>,
    ) -> (Toolpath, Option<RelinkTotals>) {
        (planned, None)
    }

    /// P33: the passes of the op over the planned curves, the first pass
    /// first. `planned` is every curve at the full depth, and `top_z` is
    /// the top of the pass ladder: the op top Z, or (P35) the stock top
    /// under the curve for an op from remaining stock ([`ladder_top_z`]).
    /// The last pass is the full depth. The default is one pass, `planned`
    /// itself.
    fn passes(&self, planned: Toolpath, _top_z: f64, _frame: CurveFrame) -> Vec<Toolpath> {
        vec![planned]
    }
}

/// THE runner of a curve op. See the module doc for what it owns.
pub(crate) fn run_curve<O: CurveOp>(
    op: &O,
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
) -> Result<GeneratedToolpath, OperationError> {
    let op_type: OperationType = prepared.op_type();
    let curves = prepared.curves()?.as_slice();
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
        prepared.territory().unguarded_stock(),
        ctx.tool_def,
    ) {
        ctx.record(&finding);
    }
    let link = CurveLink {
        regions: prepared.territory().regions(),
        ceiling: op_link_ceiling(op_type, ctx, prepared.territory()),
        kinematics: ctx.link_kinematics.as_ref(),
    };
    // P33: the op's passes, each linked alone, so no link joins two passes
    // and no link stage puts a deep pass before a shallow pass. An op with
    // one pass makes the one `link` call of before.
    let mut linked = Toolpath::new();
    let mut totals: Option<RelinkTotals> = None;
    let mut later_pass_starts: Vec<usize> = Vec::new();
    let top_z = ladder_top_z(op_type, ctx, prepared, &planned);
    for (pass_index, pass) in op.passes(planned, top_z, frame).into_iter().enumerate() {
        if pass_index > 0 {
            later_pass_starts.push(linked.moves.len());
        }
        let (pass_linked, pass_totals) = op.link(pass, query, frame, &link);
        linked.moves.extend(pass_linked.moves);
        totals = match (totals, pass_totals) {
            (Some(mut sum), Some(more)) => {
                sum.absorb(&more);
                Some(sum)
            }
            (sum, more) => sum.or(more),
        };
    }
    ctx.record(&totals);
    let generated = if later_pass_starts.is_empty() {
        generated_with_cut_run_spans(linked, O::SPAN_LABEL)
    } else {
        // The rapid-order dressup may not move a chain to another pass: a
        // barrier before each later pass (`SpanKind::RapidOrderBarrier`).
        let mut spans = crate::compute::spans::spans_from_cutting_runs(&linked, O::SPAN_LABEL);
        spans.extend(later_pass_starts.into_iter().map(|start| {
            crate::trace::toolpath_spans::Span::boundary(
                start,
                crate::trace::toolpath_spans::SpanKind::RapidOrderBarrier,
            )
        }));
        generated_with_spans(linked, spans)
    };
    Ok(with_depth_run_annotation(generated, ctx.semantic_ctx))
}

/// P35: the top of the pass ladder of a curve op.
///
/// A Fresh op, and an op whose rest rule is not `RestUse::LadderTop`: the
/// op top Z (`heights.top_z`), as before. A `LadderTop` op from remaining
/// stock: the stock top under the curve ([`stock_top_under`]), whatever
/// the heights state, so the first pass is one step below the highest
/// material and the operator states no top. Where the stock grid holds no
/// answer under any sample, the op top Z.
fn ladder_top_z(
    op_type: OperationType,
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    planned: &Toolpath,
) -> f64 {
    use crate::compute::catalog::RestUse;
    if op_type.capabilities().rest_use != RestUse::LadderTop {
        return ctx.heights.top_z;
    }
    prepared
        .territory()
        .unguarded_stock()
        .and_then(|stock| {
            stock_top_under(
                planned,
                stock,
                crate::tool::MillingCutter::radius(ctx.tool_def),
            )
        })
        .unwrap_or(ctx.heights.top_z)
}

/// P35: the highest material top in the footprint of the tool over every
/// fed move of `planned`: `standing_material::column_top` with the tool
/// radius as the reach, so each cell whose centre lies within `R + cell`
/// of a sample counts (the safe side: no material in the tool disc stands
/// above the result). `None`: no fed sample has material or a grid answer
/// in its footprint.
fn stock_top_under(
    planned: &Toolpath,
    stock: &crate::dexel_stock::TriDexelStock,
    tool_radius: f64,
) -> Option<f64> {
    use super::standing_material::{ColumnTop, column_top};
    planned
        .moves
        .iter()
        .filter(|m| m.move_type != crate::toolpath::MoveType::Rapid)
        .filter_map(|m| {
            let xy = crate::geo::P2::new(m.target.x, m.target.y);
            match column_top(stock, xy, tool_radius) {
                ColumnTop::Top(z) => Some(z),
                ColumnTop::NoEvidence | ColumnTop::Empty => None,
            }
        })
        .fold(None, |top: Option<f64>, z| {
            Some(top.map_or(z, |t| t.max(z)))
        })
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        clippy::expect_used,
        reason = "test code: a panic is how a test reports a failure"
    )]

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
        let cutter = build_cutter(&ToolConfig::test_default(
            ToolId::from_raw(0),
            ToolType::EndMill,
        ))
        .unwrap();
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
