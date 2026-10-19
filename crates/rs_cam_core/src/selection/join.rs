//! The join of open pieces: ONE kernel for every drawing door (geometry
//! selection P4.8, ruling O-5).
//!
//! A drawing often draws one outline as separate open pieces: LINE and ARC
//! entities in a DXF, or open subpaths in an SVG. [`join_open_pieces`] joins
//! the pieces end to end where two ends meet within a tolerance. A chain
//! that comes back to its start is a closed ring; other chains are open
//! paths. No chord closes a gap: two ends join only when they meet.
//!
//! The DXF door has used this rule since P1b. P4.8 moved it here from
//! `io/dxf_input.rs` without a change, so the SVG door and the P4.9 join
//! tool use the same rule. [`join_open_pieces`] also gives the counts of the
//! import report ([`JoinCounts`], one row per layer).

use std::sync::Arc;

use crate::geo::P2;
use crate::polygon::Polygon2;

/// The default join tolerance, mm. Two piece ends closer than this meet:
/// the join connects them, and a chain whose two ends meet is closed.
/// Hand-drawn CAD rarely snaps to exact coincidence. 0.01 mm is well below
/// the kerf of any CAM operation on this geometry.
pub const CLOSE_TOL_MM: f64 = 0.01;

/// One open piece of a drawing: its source index, its layer and its points
/// (at least 2) in the units of the door.
#[derive(Debug, Clone)]
pub struct OpenPiece {
    /// The source order (the DXF entity index, the SVG subpath index). A
    /// lower index wins at a branch point, and a chain takes the lowest
    /// index of its pieces.
    pub index: usize,
    /// The layer. Only pieces of one layer join (DESIGN D14). Pieces with
    /// no layer (an SVG subpath outside a named group) are one layer.
    pub layer: Option<Arc<str>>,
    pub pts: Vec<P2>,
}

/// One shape of the join: a chain of one or more pieces.
#[derive(Debug, Clone)]
pub struct JoinedShape {
    /// The lowest source index of its pieces.
    pub index: usize,
    /// The layer of its pieces.
    pub layer: Option<Arc<str>>,
    /// A closed ring (wound CCW) when the chain comes back to its start,
    /// else an open path.
    pub polygon: Polygon2,
    /// The number of pieces in the chain.
    pub pieces: usize,
}

/// The counts of one join: what went in and what came out.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JoinCounts {
    /// The open pieces that went into the join.
    pub pieces: usize,
    /// The shapes that came out (one per chain).
    pub shapes: usize,
    /// The shapes made of two or more pieces.
    pub joined: usize,
    /// The shapes that close: the two ends of the chain meet.
    pub closed_by_join: usize,
    /// The shapes that stay open paths.
    pub open_left: usize,
}

impl JoinCounts {
    fn add_shape(&mut self, shape: &JoinedShape) {
        self.pieces += shape.pieces;
        self.shapes += 1;
        if shape.pieces >= 2 {
            self.joined += 1;
        }
        if shape.polygon.closed {
            self.closed_by_join += 1;
        } else {
            self.open_left += 1;
        }
    }

    /// The sum of two counts.
    pub fn plus(self, other: Self) -> Self {
        Self {
            pieces: self.pieces + other.pieces,
            shapes: self.shapes + other.shapes,
            joined: self.joined + other.joined,
            closed_by_join: self.closed_by_join + other.closed_by_join,
            open_left: self.open_left + other.open_left,
        }
    }
}

/// The join counts of one layer.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LayerJoin {
    /// The layer name; `None` for pieces with no layer.
    pub layer: Option<String>,
    #[serde(flatten)]
    pub counts: JoinCounts,
}

/// Join the open pieces and count the result.
///
/// Returns the shapes in the order of their index (see
/// [`chain_open_pieces`] for the rule), and one [`LayerJoin`] row for each
/// layer that has a piece, in the order of the first shape of the layer.
/// `tol` is in the units of the pieces.
pub fn join_open_pieces(pieces: &[OpenPiece], tol: f64) -> (Vec<JoinedShape>, Vec<LayerJoin>) {
    let shapes: Vec<JoinedShape> = chain_open_pieces(pieces, tol)
        .into_iter()
        .filter(|chain| chain.pts.len() >= 2)
        .map(|chain| JoinedShape {
            index: chain.index,
            layer: chain.layer,
            polygon: polygon_from_unknown_closure(chain.pts, tol),
            pieces: chain.pieces,
        })
        .collect();
    let mut layers: Vec<LayerJoin> = Vec::new();
    for shape in &shapes {
        let name = shape.layer.as_deref().map(str::to_owned);
        let row = match layers.iter_mut().find(|row| row.layer == name) {
            Some(row) => row,
            None => {
                layers.push(LayerJoin {
                    layer: name,
                    counts: JoinCounts::default(),
                });
                // SAFETY: the push above made the list non-empty.
                #[allow(clippy::expect_used)]
                layers.last_mut().expect("just pushed")
            }
        };
        row.counts.add_shape(shape);
    }
    (shapes, layers)
}

/// Wrap an ordered point list as either a closed polygon (if the head and
/// tail are closer than `tol`) or an open path. The join uses it for every
/// chain, so no open path is auto-closed and no phantom segment spans the
/// whole curve.
#[allow(clippy::indexing_slicing)] // len >= 2 guarded below
pub fn polygon_from_unknown_closure(pts: Vec<P2>, tol: f64) -> Polygon2 {
    if pts.len() < 2 {
        return Polygon2::open_path(pts);
    }
    let head = pts[0];
    let tail = pts[pts.len() - 1];
    if (head - tail).norm() < tol && pts.len() >= 3 {
        let mut poly = Polygon2::new(pts);
        poly.ensure_winding();
        poly
    } else {
        Polygon2::open_path(pts)
    }
}

/// The grid cell of `p` for an end lookup with cells of size `tol`.
fn end_cell(p: P2, tol: f64) -> (i64, i64) {
    // `as` saturates a huge or non-finite value; such a point then only
    // meets points in the same saturated cell, which the distance test
    // still checks.
    ((p.x / tol).floor() as i64, (p.y / tol).floor() as i64)
}

/// One chain of the join, before the closure test.
#[derive(Debug, Clone)]
pub struct Chain {
    pub index: usize,
    pub layer: Option<Arc<str>>,
    pub pts: Vec<P2>,
    /// The number of pieces in the chain.
    pub pieces: usize,
}

/// Join the open pieces end to end into chains. Returns each chain with
/// the lowest index of its pieces and its layer, in the order of that
/// index.
///
/// The pieces must arrive in index order. The rule, so the same file always
/// gives the same chains:
///
/// 1. The seed of the next chain is the unused piece of the lowest index.
///    The chain starts with the seed in its drawn direction.
/// 2. The chain grows at its tail while a piece meets the tail, then at
///    its head while a piece meets the head. Two ends meet when they are
///    closer than `tol`.
/// 3. Only an unused piece of the seed's layer can join (DESIGN D14).
/// 4. Where two or more pieces meet an end (a branch point), the piece of
///    the lowest index joins. When both ends of that piece meet, its start
///    end joins. The other pieces start chains of their own.
/// 5. A piece joins reversed when its end, not its start, meets. The
///    joined end point is not repeated in the chain.
/// 6. The chain stops when its tail meets its head (with 3 or more
///    points): it is closed.
#[allow(clippy::indexing_slicing)] // piece indices come from 0..pieces.len()
pub fn chain_open_pieces(pieces: &[OpenPiece], tol: f64) -> Vec<Chain> {
    use std::collections::HashMap;

    // The pieces arrive in index order, so the position in `pieces` sorts
    // like the index.
    let mut ends: HashMap<(i64, i64), Vec<(usize, bool)>> = HashMap::new();
    for (k, piece) in pieces.iter().enumerate() {
        if let (Some(&first), Some(&last)) = (piece.pts.first(), piece.pts.last()) {
            ends.entry(end_cell(first, tol))
                .or_default()
                .push((k, true));
            ends.entry(end_cell(last, tol))
                .or_default()
                .push((k, false));
        }
    }
    // The unused piece of `layer` with an end that meets `p`: the lowest
    // index, its start end first. `true` = the start end meets.
    let find = |p: P2, layer: &Option<Arc<str>>, used: &[bool]| -> Option<(usize, bool)> {
        let (cx, cy) = end_cell(p, tol);
        let mut best: Option<(usize, bool)> = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                let Some(list) = ends.get(&(cx.saturating_add(dx), cy.saturating_add(dy))) else {
                    continue;
                };
                for &(k, at_start) in list {
                    let piece = &pieces[k];
                    if used[k] || piece.layer != *layer {
                        continue;
                    }
                    let end = if at_start {
                        piece.pts[0]
                    } else {
                        piece.pts[piece.pts.len() - 1]
                    };
                    if (end - p).norm() >= tol {
                        continue;
                    }
                    // Lower index wins; at one index the start end wins.
                    let better = match best {
                        None => true,
                        Some((bk, b_start)) => k < bk || (k == bk && at_start && !b_start),
                    };
                    if better {
                        best = Some((k, at_start));
                    }
                }
            }
        }
        best
    };
    let closed =
        |chain: &[P2]| chain.len() >= 3 && (chain[0] - chain[chain.len() - 1]).norm() < tol;

    let mut used = vec![false; pieces.len()];
    let mut chains = Vec::new();
    for seed in 0..pieces.len() {
        if used[seed] {
            continue;
        }
        used[seed] = true;
        let layer = &pieces[seed].layer;
        let mut chain: Vec<P2> = pieces[seed].pts.clone();
        let mut count = 1;

        // Grow the tail.
        while !closed(&chain) {
            let tail = chain[chain.len() - 1];
            let Some((k, at_start)) = find(tail, layer, &used) else {
                break;
            };
            used[k] = true;
            count += 1;
            let pts = &pieces[k].pts;
            if at_start {
                chain.extend(pts.iter().skip(1));
            } else {
                chain.extend(pts.iter().rev().skip(1));
            }
        }
        // Grow the head. The points go in front, so collect them reversed.
        let mut front: Vec<P2> = Vec::new();
        while !closed(&chain) {
            let head = front.last().copied().unwrap_or(chain[0]);
            let Some((k, at_start)) = find(head, layer, &used) else {
                break;
            };
            used[k] = true;
            count += 1;
            let pts = &pieces[k].pts;
            // The piece runs away from the head: from its meeting end to
            // its far end, and `front` holds the head side reversed.
            if at_start {
                front.extend(pts.iter().skip(1));
            } else {
                front.extend(pts.iter().rev().skip(1));
            }
            if let Some(&far) = front.last()
                && (far - chain[chain.len() - 1]).norm() < tol
            {
                break;
            }
        }
        if !front.is_empty() {
            front.reverse();
            front.extend(chain);
            chain = front;
        }
        chains.push(Chain {
            index: pieces[seed].index,
            layer: pieces[seed].layer.clone(),
            pts: chain,
            pieces: count,
        });
    }
    chains
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    fn piece(index: usize, layer: &str, pts: &[(f64, f64)]) -> OpenPiece {
        OpenPiece {
            index,
            layer: Some(Arc::from(layer)),
            pts: pts.iter().map(|&(x, y)| P2::new(x, y)).collect(),
        }
    }

    /// The counts of a join, per layer: a square of four pieces closes, an
    /// L of two pieces stays open, a lone piece stays open, and a gap over
    /// the tolerance does not join.
    #[test]
    fn the_join_counts_each_layer() {
        let pieces = vec![
            piece(0, "A", &[(0.0, 0.0), (10.0, 0.0)]),
            piece(1, "A", &[(10.0, 0.0), (10.0, 10.0)]),
            piece(2, "A", &[(10.0, 10.0), (0.0, 10.0)]),
            piece(3, "A", &[(0.0, 10.0), (0.0, 0.0)]),
            piece(4, "B", &[(20.0, 0.0), (30.0, 0.0)]),
            piece(5, "B", &[(30.0, 0.0), (30.0, 5.0)]),
            piece(6, "B", &[(30.0, 5.3), (30.0, 9.0)]),
        ];
        let (shapes, layers) = join_open_pieces(&pieces, CLOSE_TOL_MM);
        assert_eq!(shapes.len(), 3);
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0].layer.as_deref(), Some("A"));
        assert_eq!(
            layers[0].counts,
            JoinCounts {
                pieces: 4,
                shapes: 1,
                joined: 1,
                closed_by_join: 1,
                open_left: 0,
            }
        );
        assert_eq!(
            layers[1].counts,
            JoinCounts {
                pieces: 3,
                shapes: 2,
                joined: 1,
                closed_by_join: 0,
                open_left: 2,
            }
        );
        // A tolerance of 0.5 joins the 0.3 gap.
        let (shapes, layers) = join_open_pieces(&pieces, 0.5);
        assert_eq!(shapes.len(), 2);
        assert_eq!(layers[1].counts.joined, 1);
        assert_eq!(layers[1].counts.pieces, 3);
        assert_eq!(layers[1].counts.open_left, 1);
    }
}
