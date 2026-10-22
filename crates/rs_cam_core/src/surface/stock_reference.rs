//! The one guard that decides if the remaining stock can be a rest reference
//! (W2.1).
//!
//! A rest pass compares the model against what an earlier tool left. The
//! remaining stock is the best reference, but only when its frame is the
//! frame of the mesh. A stock in a different XY frame reads garbage rest
//! (the F-024 lesson). [`StockReference::resolve`] is the only place that
//! makes this decision. The territory resolver
//! (`compute::execute::Territory::resolve`, W6.2) calls it once per op; the
//! unified finish claims, the generic rest analysis and the pencil
//! `RestDepth` detector read that one answer. The drill calls it against
//! the bbox of its holes.
//!
//! The rule: the stock is the reference only when it exists and its XY bbox
//! overlaps the mesh XY bbox (inclusive). In all other cases the result is
//! [`StockReference::Absent`] with its reason. The caller then uses its own
//! tool tier (a stated prior tool, or the bare-surface self-probe). That tier
//! is a caller dial, not part of this guard.

use tracing::warn;

use crate::dexel_stock::TriDexelStock;
use crate::geo::BoundingBox3;

/// Why no remaining stock is the rest reference.
#[derive(Debug, Clone, Copy)]
pub enum AbsentReason {
    /// No stock snapshot is in scope (a fresh-stock op, or no prior sim).
    NoStockInScope,
    /// A stock is in scope, but its XY bbox does not overlap the mesh XY
    /// bbox. The two are in different frames, or the stock is another part.
    NoXyOverlap {
        stock: BoundingBox3,
        mesh: BoundingBox3,
    },
}

/// The result of the stock-versus-mesh guard. Not `Debug`:
/// `TriDexelStock` is not `Debug`.
#[derive(Clone, Copy)]
pub enum StockReference<'a> {
    /// The remaining stock, in the frame of the mesh.
    Stock(&'a TriDexelStock),
    /// No stock can be the reference. The caller uses its tool tier.
    Absent(AbsentReason),
}

impl<'a> StockReference<'a> {
    /// Apply the guard. `mesh_bbox` is the bbox of the mesh that the op
    /// drops against. The stock bbox and the mesh bbox arrive in the same op
    /// frame; this function transforms nothing. A non-overlap is logged
    /// every time, because it is a frame defect or a wrong stock.
    #[must_use]
    pub fn resolve(stock: Option<&'a TriDexelStock>, mesh_bbox: &BoundingBox3) -> Self {
        let Some(stock) = stock else {
            return Self::Absent(AbsentReason::NoStockInScope);
        };
        let (sb, mb) = (&stock.stock_bbox, mesh_bbox);
        let overlap = sb.min.x <= mb.max.x
            && sb.max.x >= mb.min.x
            && sb.min.y <= mb.max.y
            && sb.max.y >= mb.min.y;
        if overlap {
            return Self::Stock(stock);
        }
        warn!(
            stock = format!(
                "[{:.1},{:.1}]..[{:.1},{:.1}]",
                sb.min.x, sb.min.y, sb.max.x, sb.max.y
            ),
            mesh = format!(
                "[{:.1},{:.1}]..[{:.1},{:.1}]",
                mb.min.x, mb.min.y, mb.max.x, mb.max.y
            ),
            "rest reference: stock/mesh XY frames do not overlap; \
             the caller falls back to its tool tier"
        );
        Self::Absent(AbsentReason::NoXyOverlap {
            stock: *sb,
            mesh: *mb,
        })
    }

    /// The stock when the guard accepted it, else `None`.
    #[must_use]
    pub const fn stock(self) -> Option<&'a TriDexelStock> {
        match self {
            Self::Stock(stock) => Some(stock),
            Self::Absent(_) => None,
        }
    }
}

#[cfg(test)]
#[expect(
    clippy::panic,
    reason = "a test asserts the variant by panicking on the wrong one"
)]
mod tests {
    use super::*;
    use crate::geo::P3;

    fn bbox(min: [f64; 3], max: [f64; 3]) -> BoundingBox3 {
        BoundingBox3 {
            min: P3::new(min[0], min[1], min[2]),
            max: P3::new(max[0], max[1], max[2]),
        }
    }

    #[test]
    fn an_overlapping_stock_is_the_reference() {
        let mesh = bbox([0.0, 0.0, -5.0], [20.0, 20.0, 0.0]);
        let stock = TriDexelStock::from_bounds(&bbox([-2.0, -2.0, -6.0], [22.0, 22.0, 0.0]), 1.0);
        let got = StockReference::resolve(Some(&stock), &mesh);
        assert!(matches!(got, StockReference::Stock(s) if std::ptr::eq(s, &stock)));
        assert!(got.stock().is_some());
    }

    #[test]
    fn a_stock_that_only_touches_the_mesh_edge_is_the_reference() {
        // The guard is inclusive: a shared edge counts as an overlap.
        let mesh = bbox([0.0, 0.0, -5.0], [20.0, 20.0, 0.0]);
        let stock = TriDexelStock::from_bounds(&bbox([20.0, 0.0, -6.0], [40.0, 20.0, 0.0]), 1.0);
        assert!(
            StockReference::resolve(Some(&stock), &mesh)
                .stock()
                .is_some()
        );
    }

    #[test]
    fn a_stock_off_the_mesh_in_xy_is_absent_with_both_bboxes() {
        let mesh = bbox([0.0, 0.0, -5.0], [20.0, 20.0, 0.0]);
        let stock = TriDexelStock::from_bounds(&bbox([50.0, 0.0, -6.0], [70.0, 20.0, 0.0]), 1.0);
        match StockReference::resolve(Some(&stock), &mesh) {
            StockReference::Absent(AbsentReason::NoXyOverlap {
                stock: sb,
                mesh: mb,
            }) => {
                assert!((sb.min.x - stock.stock_bbox.min.x).abs() < 1e-12);
                assert!((mb.max.x - 20.0).abs() < 1e-12);
            }
            StockReference::Stock(_) => panic!("expected NoXyOverlap, got Stock"),
            StockReference::Absent(reason) => panic!("expected NoXyOverlap, got {reason:?}"),
        }
    }

    #[test]
    fn no_stock_is_absent_with_no_stock_in_scope() {
        let mesh = bbox([0.0, 0.0, -5.0], [20.0, 20.0, 0.0]);
        let got = StockReference::resolve(None, &mesh);
        assert!(matches!(
            got,
            StockReference::Absent(AbsentReason::NoStockInScope)
        ));
        assert!(got.stock().is_none());
    }
}
