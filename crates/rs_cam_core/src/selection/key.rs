//! The match key of an item (DESIGN 9.3).
//!
//! A stored pick holds the key of the item it names. A re-import computes
//! the keys again; a pick matches an item when the two keys match. A moved
//! item is lost and flagged, not guessed.

use serde::{Deserialize, Serialize};

use crate::geo::P2;

use super::item::{ItemKind, Shape, SourceRef};

/// The distance (mm) at which two key anchors, or two key measures, are
/// equal. It is the drill pick rule of today, `DRILL_PICK_MATCH_EPS_MM`
/// (`compute/execute/drilling.rs`); P2.3c moves that constant here.
pub const KEY_MATCH_EPS_MM: f64 = crate::compute::execute::DRILL_PICK_MATCH_EPS_MM;

/// The identity of an item across a re-import of an unchanged file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchKey {
    pub source: SourceRef,
    pub kind: ItemKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    /// Loop, Face: the area centroid of the outer ring. Path: the first
    /// vertex. Point: the point. Circle: the centre.
    pub anchor: [f64; 2],
    /// Loop, Face: `sqrt(|area|)` in mm (a face: the area net of its own
    /// inner loops). Path: the length. Circle: the diameter. Point: 0.
    pub measure: f64,
    /// Face only: the BREP id, a tie-break when two faces share anchor and
    /// measure. The key has no Z: a face that moves in Z still matches.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brep: Option<u16>,
}

impl MatchKey {
    /// The key of `shape`. Only `Item::new` calls this, so a key always
    /// matches its shape.
    pub(super) fn of(source: SourceRef, layer: Option<&str>, shape: &Shape) -> Self {
        let (anchor, measure, brep) = match shape {
            Shape::Loop(p) => (area_centroid(&p.exterior), p.area().abs().sqrt(), None),
            Shape::Path(p) => (
                p.exterior.first().map_or([0.0, 0.0], |q| [q.x, q.y]),
                path_length(&p.exterior),
                None,
            ),
            Shape::Point(p) => ([p.x, p.y], 0.0, None),
            Shape::Circle {
                centre, diameter, ..
            } => ([centre.x, centre.y], *diameter, None),
            Shape::Face(face) => (
                area_centroid(&face.rings.exterior),
                face.rings.area().abs().sqrt(),
                face.brep.map(|b| b.0),
            ),
        };
        Self {
            source,
            kind: shape.kind(),
            layer: layer.map(str::to_owned),
            anchor,
            measure,
            brep,
        }
    }

    /// True when the two keys name one item: the source is equal and
    /// [`Self::names_same_geometry`] holds.
    pub fn matches(&self, other: &MatchKey) -> bool {
        self.source == other.source && self.names_same_geometry(other)
    }

    /// True when the two keys name one item of ONE source: kind, layer and
    /// the BREP id are equal, and anchor and measure agree within
    /// [`KEY_MATCH_EPS_MM`]. The source is not compared. A stored pick
    /// names its source as [`SourceRef::ToolpathModel`], but the session
    /// binds the items to [`SourceRef::Model`]; the resolver finds the
    /// item set of the pick's source first and then calls this.
    pub fn names_same_geometry(&self, other: &MatchKey) -> bool {
        self.kind == other.kind
            && self.layer == other.layer
            && self.brep == other.brep
            && (self.anchor[0] - other.anchor[0]).abs() < KEY_MATCH_EPS_MM
            && (self.anchor[1] - other.anchor[1]).abs() < KEY_MATCH_EPS_MM
            && (self.measure - other.measure).abs() < KEY_MATCH_EPS_MM
    }
}

/// The area centroid of a ring. A ring of no area gives the mean of its
/// vertices.
fn area_centroid(ring: &[P2]) -> [f64; 2] {
    let n = ring.len();
    let mut twice_area = 0.0;
    let mut cx = 0.0;
    let mut cy = 0.0;
    for (i, a) in ring.iter().enumerate() {
        let Some(b) = ring.get((i + 1) % n) else {
            continue;
        };
        let cross = a.x * b.y - b.x * a.y;
        twice_area += cross;
        cx += (a.x + b.x) * cross;
        cy += (a.y + b.y) * cross;
    }
    if twice_area.abs() > f64::EPSILON {
        [cx / (3.0 * twice_area), cy / (3.0 * twice_area)]
    } else if n > 0 {
        let k = n as f64;
        let (sx, sy) = ring.iter().fold((0.0, 0.0), |(x, y), p| (x + p.x, y + p.y));
        [sx / k, sy / k]
    } else {
        [0.0, 0.0]
    }
}

/// The length of an open polyline.
fn path_length(pts: &[P2]) -> f64 {
    pts.windows(2)
        .map(|w| match w {
            [a, b] => (b - a).norm(),
            _ => 0.0,
        })
        .sum()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::polygon::Polygon2;
    use crate::selection::item::Item;

    #[test]
    fn a_moved_item_does_not_match() {
        let ring = |dx: f64| {
            Polygon2::new(vec![
                P2::new(dx, 0.0),
                P2::new(dx + 4.0, 0.0),
                P2::new(dx + 4.0, 4.0),
                P2::new(dx, 4.0),
            ])
        };
        let key = |dx: f64| {
            Item::new(
                SourceRef::ToolpathModel,
                Some("L".into()),
                Shape::Loop(ring(dx)),
            )
            .unwrap()
            .key()
            .clone()
        };
        assert_eq!(key(0.0).anchor, [2.0, 2.0]);
        assert!((key(0.0).measure - 4.0).abs() < 1e-12);
        assert!(key(0.0).matches(&key(0.0)));
        assert!(!key(0.0).matches(&key(0.01)));
    }
}
