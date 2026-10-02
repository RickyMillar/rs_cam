//! Pocket detection on a tool-CL height grid: a join tree (merge tree) of
//! the sublevel sets, simplified by a persistence threshold (h-minima) and a
//! minimum area.
//!
//! # What a pocket is
//!
//! A pocket is a node of the join tree. The tree grows as a water level
//! rises from the lowest cell. Each local minimum starts a leaf. Where two
//! components meet, the level is their saddle. When both components are
//! significant, the tree closes both and opens a parent node for the cells
//! above the saddle. A component is weak when its depth below the saddle is
//! less than `persistence_h_mm`, or its area at the saddle is less than
//! `min_area_mm2`. A weak component does not close: it joins the other
//! component, and its cells take that component's pocket.
//!
//! So each cell has exactly one pocket. A leaf holds the cells below its
//! saddle. A parent holds the cells above the saddles of its children and
//! below its own saddle.
//!
//! # The edges and the top
//!
//! - A masked cell (`FlowField::nodata`) is not material and is a wall. The
//!   caller masks the cells that the model does not cover. The drop cutter
//!   clamps those cells to the model floor, and they would make one deep
//!   ring around the board.
//! - A cell at or above `top_z` has no material. It gets no pocket and it is
//!   a wall.
//! - The grid border is a wall, not an outlet. A valley that runs off the
//!   edge of the board is a pocket. This is the difference from
//!   [`super::flow_accum::priority_flood_epsilon`], which drains every cell
//!   next to the border.
//! - A component that is still open when all cells are in is a root. Its
//!   saddle is `top_z`. A root is always a pocket, also when it is weak,
//!   because it holds material that a rough must cut.
//! - A flat plane below `top_z` is one root pocket with depth
//!   `top_z - z`. A flat plane at or above `top_z` has no pocket.
//!
//! # Order and ties
//!
//! The cells go in by `(z, index)` with `f64::total_cmp`, so a flat floor
//! has one deterministic order. Connectivity is 8-connected, the same as
//! [`super::flow_accum::neighbour`] and the By Area material detector. When
//! two weak components meet, the one with less persistence joins the other.
//!
//! # References
//!
//! - Join tree by union-find over cells sorted by height: Carr, Snoeyink &
//!   Axen (2003), "Computing contour trees in all dimensions".
//! - h-minima (dynamics / persistence of a regional minimum): Soille (2003),
//!   *Morphological Image Analysis*, §6.3; Grimaud (1992) for the dynamics.
//! - The grid type and the neighbour walk are those of `flow_accum`
//!   (Barnes, Lehman & Soille 2014).
//!
//! **Test door.** The research probe
//! `tests/pocket_merge_tree_census_by_area.rs` is the only caller. No
//! production path reads it. The By Area planner does not use it yet.

use super::flow_accum::{FlowField, neighbour};

/// The two dials of the simplification.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MergeTreeParams {
    /// A component whose depth below its saddle is less than this joins
    /// its neighbour.
    pub persistence_h_mm: f64,
    /// A component whose area at its saddle is less than this joins its
    /// neighbour.
    pub min_area_mm2: f64,
}

/// One node of the simplified join tree.
#[derive(Debug, Clone, PartialEq)]
pub struct Pocket {
    /// The index of this pocket in [`PocketTree::pockets`].
    pub id: usize,
    /// The lowest cell Z of the pocket and of all its descendants.
    pub min_z: f64,
    /// The Z where the pocket joins its parent. `top_z` for a root.
    pub saddle_z: f64,
    /// `saddle_z - min_z`.
    pub depth_mm: f64,
    /// The area of the cells that carry this pocket's label.
    pub area_mm2: f64,
    /// The area of the whole component at the saddle: this pocket and all
    /// its descendants.
    pub basin_area_mm2: f64,
    /// `[x_min, y_min, x_max, y_max]` of the labelled cell centres.
    pub bbox_xy: [f64; 4],
    /// The pocket this one joins at its saddle. `None` for a root.
    pub parent: Option<usize>,
    /// The pockets that join this one.
    pub children: Vec<usize>,
    /// The number of labelled cells.
    pub cell_count: usize,
}

/// The result of [`build_pocket_tree`].
#[derive(Debug, Clone, PartialEq)]
pub struct PocketTree {
    /// Row-major, one entry per cell. `None` is a masked cell or a cell
    /// with no material.
    pub labels: Vec<Option<usize>>,
    pub pockets: Vec<Pocket>,
}

impl PocketTree {
    /// The roots, deepest first.
    #[must_use]
    pub fn roots(&self) -> Vec<usize> {
        let mut roots: Vec<usize> = self
            .pockets
            .iter()
            .filter(|p| p.parent.is_none())
            .map(|p| p.id)
            .collect();
        roots.sort_by(|&a, &b| self.deeper_first(a, b));
        roots
    }

    /// The order of a depth-first rough, pocket by pocket: children before
    /// parents (post-order), and among siblings the one with the lowest
    /// `min_z` first. The roots go in the same order.
    #[must_use]
    pub fn cut_order(&self) -> Vec<usize> {
        let mut order = Vec::with_capacity(self.pockets.len());
        for root in self.roots() {
            self.post_order(root, &mut order);
        }
        order
    }

    fn post_order(&self, id: usize, order: &mut Vec<usize>) {
        let Some(p) = self.pockets.get(id) else {
            return;
        };
        let mut kids = p.children.clone();
        kids.sort_by(|&a, &b| self.deeper_first(a, b));
        for kid in kids {
            self.post_order(kid, order);
        }
        order.push(id);
    }

    fn deeper_first(&self, a: usize, b: usize) -> std::cmp::Ordering {
        let za = self.pockets.get(a).map_or(f64::INFINITY, |p| p.min_z);
        let zb = self.pockets.get(b).map_or(f64::INFINITY, |p| p.min_z);
        za.total_cmp(&zb).then(a.cmp(&b))
    }
}

/// A node while the tree grows.
struct Node {
    min_z: f64,
    /// The cell count of the component when the node closed.
    basin_cells: usize,
    saddle_z: Option<f64>,
    parent: Option<usize>,
    /// The node this one joined as a weak component.
    alias: Option<usize>,
}

/// A union-find root: the open node of the component and its size.
struct Component {
    node: usize,
    cells: usize,
}

fn find(uf: &mut [usize], mut i: usize) -> usize {
    // SAFETY: every entry of `uf` is a cell index below `uf.len()`.
    #[allow(clippy::indexing_slicing)]
    {
        let mut root = i;
        while uf[root] != root {
            root = uf[root];
        }
        while uf[i] != root {
            let next = uf[i];
            uf[i] = root;
            i = next;
        }
        root
    }
}

fn resolve(nodes: &[Node], mut n: usize) -> usize {
    while let Some(a) = nodes.get(n).and_then(|node| node.alias) {
        n = a;
    }
    n
}

/// Build the simplified join tree of `field` below `top_z`.
///
/// `field.z` is the tool-CL height of each cell (the drop-cutter grid of the
/// roughing tool). See the module doc for the pocket definition.
// SAFETY: every index is a cell index below `field.len()` (from `0..n`, from
// the sorted order of those, or from `neighbour`), a node index below
// `nodes.len()`, or a component index taken from a union-find root.
#[allow(clippy::indexing_slicing)]
#[must_use]
pub fn build_pocket_tree(field: &FlowField, top_z: f64, params: &MergeTreeParams) -> PocketTree {
    let n = field.len();
    let cell_area = field.cell * field.cell;
    let is_material = |i: usize| !field.nodata[i] && field.z[i].is_finite() && field.z[i] < top_z;

    let mut order: Vec<usize> = (0..n).filter(|&i| is_material(i)).collect();
    order.sort_by(|&a, &b| field.z[a].total_cmp(&field.z[b]).then(a.cmp(&b)));

    let mut uf: Vec<usize> = (0..n).collect();
    let mut inserted = vec![false; n];
    let mut comp: Vec<Option<Component>> = (0..n).map(|_| None).collect();
    let mut nodes: Vec<Node> = Vec::new();
    let mut cell_node: Vec<Option<usize>> = vec![None; n];

    let weak = |depth: f64, cells: usize| {
        depth < params.persistence_h_mm || (cells as f64) * cell_area < params.min_area_mm2
    };

    for &c in &order {
        let z = field.z[c];
        // The distinct components beside `c`.
        let mut roots: Vec<usize> = Vec::with_capacity(8);
        for k in 0..8 {
            if let Some((j, _)) = neighbour(field, c, k)
                && inserted[j]
            {
                let r = find(&mut uf, j);
                if !roots.contains(&r) {
                    roots.push(r);
                }
            }
        }

        if roots.is_empty() {
            nodes.push(Node {
                min_z: z,
                basin_cells: 0,
                saddle_z: None,
                parent: None,
                alias: None,
            });
            comp[c] = Some(Component {
                node: nodes.len() - 1,
                cells: 1,
            });
            cell_node[c] = Some(nodes.len() - 1);
            inserted[c] = true;
            continue;
        }

        // Merge the components one by one into the first.
        let mut acc = roots[0];
        for &other in roots.iter().skip(1) {
            let (Some(ca), Some(cb)) = (comp[acc].take(), comp[other].take()) else {
                continue;
            };
            let da = z - nodes[ca.node].min_z;
            let db = z - nodes[cb.node].min_z;
            let wa = weak(da, ca.cells);
            let wb = weak(db, cb.cells);
            let cells = ca.cells + cb.cells;
            let node = if !wa && !wb {
                // Both significant: close both, open a parent.
                let parent = nodes.len();
                nodes.push(Node {
                    min_z: nodes[ca.node].min_z.min(nodes[cb.node].min_z),
                    basin_cells: 0,
                    saddle_z: None,
                    parent: None,
                    alias: None,
                });
                for (child, child_cells) in [(ca.node, ca.cells), (cb.node, cb.cells)] {
                    nodes[child].saddle_z = Some(z);
                    nodes[child].basin_cells = child_cells;
                    nodes[child].parent = Some(parent);
                }
                parent
            } else {
                // The weak one joins the other. When both are weak, the one
                // with less persistence joins.
                let a_joins = if wa && wb { da < db } else { wa };
                let (keep, gone) = if a_joins {
                    (cb.node, ca.node)
                } else {
                    (ca.node, cb.node)
                };
                nodes[gone].alias = Some(keep);
                let gone_min = nodes[gone].min_z;
                nodes[keep].min_z = nodes[keep].min_z.min(gone_min);
                keep
            };
            // Union by size.
            let (big, small) = if ca.cells >= cb.cells {
                (acc, other)
            } else {
                (other, acc)
            };
            uf[small] = big;
            comp[big] = Some(Component { node, cells });
            acc = big;
        }

        uf[c] = acc;
        inserted[c] = true;
        if let Some(cc) = comp[acc].as_mut() {
            cc.cells += 1;
            cell_node[c] = Some(cc.node);
        }
    }

    // Close the roots at the top.
    for slot in comp.iter().flatten() {
        let node = &mut nodes[slot.node];
        if node.saddle_z.is_none() {
            node.saddle_z = Some(top_z);
            node.basin_cells = slot.cells;
        }
    }

    // Compact: one pocket per live node (a node with no alias).
    let mut pocket_of_node: Vec<Option<usize>> = vec![None; nodes.len()];
    let mut pockets: Vec<Pocket> = Vec::new();
    for (i, node) in nodes.iter().enumerate() {
        if node.alias.is_some() {
            continue;
        }
        let saddle = node.saddle_z.unwrap_or(top_z);
        pocket_of_node[i] = Some(pockets.len());
        pockets.push(Pocket {
            id: pockets.len(),
            min_z: node.min_z,
            saddle_z: saddle,
            depth_mm: saddle - node.min_z,
            area_mm2: 0.0,
            basin_area_mm2: node.basin_cells as f64 * cell_area,
            bbox_xy: [
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ],
            parent: None,
            children: Vec::new(),
            cell_count: 0,
        });
    }
    for (i, node) in nodes.iter().enumerate() {
        let Some(pid) = pocket_of_node[i] else {
            continue;
        };
        if let Some(parent) = node.parent
            && let Some(ppid) = pocket_of_node[resolve(&nodes, parent)]
        {
            pockets[pid].parent = Some(ppid);
            pockets[ppid].children.push(pid);
        }
    }

    let mut labels: Vec<Option<usize>> = vec![None; n];
    for i in 0..n {
        let Some(node) = cell_node[i] else {
            continue;
        };
        let Some(pid) = pocket_of_node[resolve(&nodes, node)] else {
            continue;
        };
        labels[i] = Some(pid);
        let x = field.ox + (i % field.nx) as f64 * field.cell;
        let y = field.oy + (i / field.nx) as f64 * field.cell;
        let p = &mut pockets[pid];
        p.cell_count += 1;
        p.area_mm2 += cell_area;
        p.bbox_xy[0] = p.bbox_xy[0].min(x);
        p.bbox_xy[1] = p.bbox_xy[1].min(y);
        p.bbox_xy[2] = p.bbox_xy[2].max(x);
        p.bbox_xy[3] = p.bbox_xy[3].max(y);
    }

    PocketTree { labels, pockets }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    const TOP: f64 = 10.0;

    fn field(nx: usize, ny: usize, f: impl Fn(f64, f64) -> f64) -> FlowField {
        let mut z = Vec::with_capacity(nx * ny);
        for r in 0..ny {
            for c in 0..nx {
                z.push(f(c as f64, r as f64));
            }
        }
        FlowField {
            nx,
            ny,
            ox: 0.0,
            oy: 0.0,
            cell: 1.0,
            z,
            nodata: vec![false; nx * ny],
        }
    }

    /// Two V-shaped troughs along Y with floors at 0 (x = 10) and 1
    /// (x = 30), split by a ridge of height `ridge` at x = 20. The outer
    /// rims are at 9.
    fn two_bowls(ridge: f64) -> FlowField {
        field(41, 21, |x, _| {
            if x <= 10.0 {
                9.0 - 0.9 * x
            } else if x <= 20.0 {
                ridge * (x - 10.0) / 10.0
            } else if x <= 30.0 {
                ridge + (1.0 - ridge) * (x - 20.0) / 10.0
            } else {
                1.0 + 0.8 * (x - 30.0)
            }
        })
    }

    fn params(h: f64, area: f64) -> MergeTreeParams {
        MergeTreeParams {
            persistence_h_mm: h,
            min_area_mm2: area,
        }
    }

    #[test]
    fn two_bowls_split_by_a_high_ridge() {
        // The right bowl's floor is 1, the ridge 7: persistence 6 >= 2.
        let t = build_pocket_tree(&two_bowls(7.0), TOP, &params(2.0, 10.0));
        assert_eq!(t.pockets.len(), 3, "two leaves and their parent");
        let roots = t.roots();
        assert_eq!(roots.len(), 1);
        let root = &t.pockets[roots[0]];
        assert_eq!(root.children.len(), 2);
        assert!((root.saddle_z - TOP).abs() < 1e-9);
        let leaves: Vec<&Pocket> = t.pockets.iter().filter(|p| p.parent.is_some()).collect();
        for leaf in &leaves {
            assert!(leaf.saddle_z > 6.0 && leaf.saddle_z <= 7.0 + 1e-9);
            assert!(leaf.depth_mm > 5.0);
        }
        // The left bowl (floor 0) is the deeper leaf and goes first.
        let order = t.cut_order();
        assert_eq!(order.len(), 3);
        assert!(t.pockets[order[0]].min_z.abs() < 1e-9);
        assert_eq!(order[2], roots[0], "the parent goes last");
        // Each cell below the ridge is in a leaf.
        let left_floor = 10 * 41 + 10;
        let right_floor = 10 * 41 + 30;
        assert_ne!(t.labels[left_floor], t.labels[right_floor]);
        assert!(t.labels[left_floor].is_some());
    }

    #[test]
    fn two_bowls_join_below_a_low_ridge() {
        // The ridge is 2: the right bowl's persistence is 1 < h = 2.
        let t = build_pocket_tree(&two_bowls(2.0), TOP, &params(2.0, 10.0));
        assert_eq!(t.pockets.len(), 1, "one pocket, not zero");
        let p = &t.pockets[0];
        assert!(p.parent.is_none());
        assert!(p.min_z.abs() < 1e-9);
        assert!((p.depth_mm - TOP).abs() < 1e-9);
        assert!(t.labels.iter().all(|l| *l == Some(0)));
    }

    #[test]
    fn two_weak_bowls_leave_one_pocket() {
        // Both bowls are weak at a large h: still one root pocket.
        let t = build_pocket_tree(&two_bowls(7.0), TOP, &params(50.0, 10.0));
        assert_eq!(t.pockets.len(), 1);
        assert!(t.pockets[0].min_z.abs() < 1e-9, "the deeper floor survives");
    }

    #[test]
    fn a_small_dimple_joins_its_neighbour() {
        // A plane at 5 with a 3x3 dimple 3 mm deep at (10, 10): deep enough
        // for h, but 9 mm² is below the 20 mm² area floor.
        let f = field(21, 21, |x, y| {
            if (x - 10.0).abs() <= 1.0 && (y - 10.0).abs() <= 1.0 {
                2.0
            } else {
                5.0
            }
        });
        let t = build_pocket_tree(&f, TOP, &params(1.0, 20.0));
        assert_eq!(t.pockets.len(), 1);
        assert!((t.pockets[0].min_z - 2.0).abs() < 1e-9);
        // With a smaller area floor the dimple is strong, but it has no
        // rival basin: it is the one root, and the plane joins it.
        let t = build_pocket_tree(&f, TOP, &params(1.0, 5.0));
        assert_eq!(t.pockets.len(), 1);
        assert_eq!(t.pockets[0].cell_count, 21 * 21);
    }

    #[test]
    fn a_dimple_beside_a_bowl_splits_only_when_large_enough() {
        // A deep bowl at x = 30 and a 5x5 dimple at x = 8, on a plane that
        // rises away from x = 19. The slope keeps the saddle off a flat.
        let f = field(41, 21, |x, y| {
            let plane = 6.0 + 0.05 * (x - 19.0).abs();
            let bowl = plane.min(((x - 30.0).powi(2) + (y - 10.0).powi(2)).sqrt() * 0.8);
            if (x - 8.0).abs() <= 2.0 && (y - 10.0).abs() <= 2.0 {
                3.0
            } else {
                bowl
            }
        });
        let small = build_pocket_tree(&f, TOP, &params(1.0, 40.0));
        assert_eq!(small.pockets.len(), 1, "25 mm² < 40 mm²: merged");
        let large = build_pocket_tree(&f, TOP, &params(1.0, 20.0));
        assert_eq!(large.pockets.len(), 3, "25 mm² >= 20 mm²: a leaf");
        let dimple = large.labels[10 * 41 + 8].unwrap();
        let p = &large.pockets[dimple];
        assert!((p.min_z - 3.0).abs() < 1e-9);
        assert!((p.area_mm2 - 25.0).abs() < 1e-9);
        assert!(p.parent.is_some());
    }

    #[test]
    fn a_flat_plane_is_one_root_below_the_top_and_none_above() {
        let below = build_pocket_tree(&field(10, 10, |_, _| 4.0), TOP, &params(1.0, 1.0));
        assert_eq!(below.pockets.len(), 1);
        assert!((below.pockets[0].depth_mm - 6.0).abs() < 1e-9);
        let above = build_pocket_tree(&field(10, 10, |_, _| TOP), TOP, &params(1.0, 1.0));
        assert!(above.pockets.is_empty());
        assert!(above.labels.iter().all(Option::is_none));
    }

    #[test]
    fn a_masked_cell_is_a_wall_with_no_label() {
        let mut f = two_bowls(7.0);
        // Mask the whole ridge column: the two bowls never meet.
        for r in 0..f.ny {
            f.nodata[r * f.nx + 20] = true;
        }
        let t = build_pocket_tree(&f, TOP, &params(2.0, 10.0));
        assert_eq!(t.roots().len(), 2);
        assert!(t.labels[10 * 41 + 20].is_none());
    }
}
