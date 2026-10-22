//! The stock source: the stock outline as an item.
//!
//! The stock is per setup, so this source runs at resolution, not at load
//! (DESIGN 2.1). P2.1 adds it; P2.3b reads it for Face.

use crate::geo::BoundingBox3;
use crate::polygon::Polygon2;

use super::item::{Item, ItemError, ItemSet, Shape, SourceRef};

/// One `Loop`: the XY rectangle of `stock_bbox` (in the emission frame),
/// from [`SourceRef::Stock`], with no layer.
pub fn stock_items(stock_bbox: &BoundingBox3) -> Result<ItemSet, ItemError> {
    let rect = Polygon2::rectangle(
        stock_bbox.min.x,
        stock_bbox.min.y,
        stock_bbox.max.x,
        stock_bbox.max.y,
    );
    let item = Item::new(SourceRef::Stock, None, Shape::Loop(rect))?;
    Ok(ItemSet::new(vec![item]))
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;
    use crate::geo::P3;
    use crate::selection::item::ItemKind;

    #[test]
    fn the_stock_is_one_loop_of_its_rectangle() {
        let bbox = BoundingBox3 {
            min: P3::new(-10.0, -5.0, -3.0),
            max: P3::new(10.0, 5.0, 0.0),
        };
        let set = stock_items(&bbox).unwrap();
        assert_eq!(set.len(), 1);
        let item = set.iter().next().unwrap();
        assert_eq!(item.kind(), ItemKind::Loop);
        assert_eq!(item.source(), SourceRef::Stock);
        assert_eq!(set.bbox(), Some([-10.0, -5.0, 10.0, 5.0]));
    }
}
