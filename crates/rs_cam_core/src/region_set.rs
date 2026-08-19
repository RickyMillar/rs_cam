//! `RegionSet`: a single type for "the set of disjoint machining-region
//! polygons a toolpath is confined to".
//!
//! Before this module, the same three operations — point-in-any-region
//! containment, union-and-collapse-to-one-polygon, and per-region
//! keep-out-subtract-then-offset — were hand-rolled in four-plus places:
//! `session/compute.rs::resolve_derived_region_polygons` (+ the union
//! collapse in `resolve_generation_inputs`), the GUI worker's two copies in
//! `compute/worker/execute/mod.rs` (the adaptive3d pre-boundary union and the
//! `pre_boundary_regions` per-region block), and the seven mesh-finish ops'
//! `regions.iter().any(|r| r.contains_point(p))` closures. `RegionSet` is the
//! one place that logic lives now.
//!
//! Regions are disjoint by construction (they come from a marching-squares
//! rest-region detector) — `RegionSet` does not itself enforce or rely on
//! non-overlap beyond "membership is inside ANY region".

use crate::boundary::subtract_keepouts;
use crate::geo::P2;
use crate::polygon::{Polygon2, largest_by_area, offset_polygon};
use std::borrow::Cow;

/// A set of disjoint machining-region polygons.
///
/// `RegionSet::from_slice` borrows (no clone) for read-only hot paths like
/// containment tests during generation; `RegionSet::new` takes ownership for
/// call sites building a fresh set (e.g. `processed`'s output).
#[derive(Debug, Clone)]
pub struct RegionSet<'a> {
    regions: Cow<'a, [Polygon2]>,
}

impl<'a> RegionSet<'a> {
    /// Build an owned `RegionSet` from a freshly constructed `Vec`.
    pub fn new(regions: Vec<Polygon2>) -> RegionSet<'static> {
        RegionSet {
            regions: Cow::Owned(regions),
        }
    }

    /// Borrow an existing slice without cloning — the hot-path constructor
    /// for read-only use (containment checks, re-slicing for a downstream
    /// `&[Polygon2]`-typed callee).
    pub fn from_slice(regions: &'a [Polygon2]) -> RegionSet<'a> {
        RegionSet {
            regions: Cow::Borrowed(regions),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }

    pub fn len(&self) -> usize {
        self.regions.len()
    }

    pub fn as_slice(&self) -> &[Polygon2] {
        &self.regions
    }

    /// Is `p` inside any region in the set?
    ///
    /// The per-region AABB pre-filter (PERF_REVIEW 2026-08-19, G4) lives
    /// *inside* `Polygon2::contains_point`, which rejects on its cached
    /// exterior box before ray-casting. Regions are disjoint by construction,
    /// so for a set of `m` regions a query now costs `m` box tests plus at
    /// most the ray casts of the boxes it actually lands in — the `m × V`
    /// factor this `.any()` used to layer on is gone. Do **not** re-test the
    /// box here: it would double the check on the region that hits.
    pub fn contains(&self, p: &P2) -> bool {
        self.regions.iter().any(|r| r.contains_point(p))
    }

    /// Union every region into one polygon, but only return it when the
    /// union collapses to exactly one connected component. `None` covers
    /// both the empty set and a union that stays multi-polygon — callers
    /// that need a single containment polygon (adaptive3d's internal-stock
    /// pre-clip) treat both cases the same way: skip the optimization.
    pub fn single_union(&self) -> Option<Polygon2> {
        let mut unioned = Polygon2::union_all(&self.regions);
        (unioned.len() == 1).then(|| unioned.remove(0))
    }

    /// Per-region processing: subtract `keep_outs` from each region, then
    /// offset by `-offset` (mirrors the boundary inset sign convention used
    /// throughout `boundary.rs`/`session/compute.rs`), keeping the
    /// largest-by-area piece of each region's offset result. A region that
    /// collapses entirely under the offset is dropped from the returned set
    /// rather than treated as an error — the same "vanished, not
    /// boundary-collapsed" semantics as the pre-`RegionSet` callers used.
    pub fn processed(&self, keep_outs: &[Polygon2], offset: f64) -> RegionSet<'static> {
        let out = self
            .regions
            .iter()
            .filter_map(|region| {
                let mut poly = region.clone();
                if !keep_outs.is_empty() {
                    poly = subtract_keepouts(&poly, keep_outs);
                }
                if offset.abs() > 1e-9 {
                    let offset_polys = offset_polygon(&poly, -offset);
                    poly = largest_by_area(&offset_polys)?.clone();
                }
                Some(poly)
            })
            .collect();
        RegionSet::new(out)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Polygon2 {
        Polygon2::rectangle(x0, y0, x1, y1)
    }

    #[test]
    fn empty_set_is_empty_and_never_contains() {
        let rs = RegionSet::new(Vec::new());
        assert!(rs.is_empty());
        assert_eq!(rs.len(), 0);
        assert!(!rs.contains(&P2::new(0.0, 0.0)));
        assert!(rs.single_union().is_none());
    }

    #[test]
    fn contains_is_inside_any_region() {
        let regions = vec![square(0.0, 0.0, 1.0, 1.0), square(10.0, 10.0, 11.0, 11.0)];
        let rs = RegionSet::from_slice(&regions);
        assert!(rs.contains(&P2::new(0.5, 0.5)));
        assert!(rs.contains(&P2::new(10.5, 10.5)));
        assert!(!rs.contains(&P2::new(5.0, 5.0)));
    }

    #[test]
    fn from_slice_does_not_clone_the_backing_data() {
        let regions = vec![square(0.0, 0.0, 1.0, 1.0)];
        let rs = RegionSet::from_slice(&regions);
        // The borrowed slice must be the same allocation, not a copy.
        assert!(std::ptr::eq(rs.as_slice().as_ptr(), regions.as_ptr()));
    }

    #[test]
    fn single_union_collapses_touching_regions() {
        // Two abutting squares sharing an edge union into one polygon.
        let regions = vec![square(0.0, 0.0, 1.0, 1.0), square(1.0, 0.0, 2.0, 1.0)];
        let rs = RegionSet::new(regions);
        let union = rs.single_union();
        assert!(
            union.is_some(),
            "abutting squares should union to one polygon"
        );
    }

    #[test]
    fn single_union_none_for_disjoint_regions() {
        let regions = vec![square(0.0, 0.0, 1.0, 1.0), square(10.0, 10.0, 11.0, 11.0)];
        let rs = RegionSet::new(regions);
        assert!(rs.single_union().is_none());
    }

    #[test]
    fn processed_applies_keepout_and_offset() {
        // NOTE: `processed`'s offset sign mirrors `BoundaryConfig::offset`'s
        // convention (inherited verbatim from the pre-`RegionSet` callers,
        // which pass `offset_polygon(poly, -offset)`) — negative `offset`
        // shrinks inward, positive grows outward. See `offset_polygon`'s own
        // tests (`offset_polygon(&sq, 2.0)` is documented "inward by 2").
        let regions = vec![square(0.0, 0.0, 10.0, 10.0)];
        let rs = RegionSet::from_slice(&regions);
        let keep_out = square(4.0, 4.0, 6.0, 6.0);
        let processed = rs.processed(&[keep_out], -1.0);
        assert_eq!(processed.len(), 1);
        // Shrink inward by 1.0 should leave the region below its original area.
        assert!(processed.as_slice()[0].area() < regions[0].area());
    }

    #[test]
    fn processed_drops_regions_that_collapse_under_offset() {
        // A region small enough that a large inward shrink eats it entirely.
        let regions = vec![square(0.0, 0.0, 1.0, 1.0), square(10.0, 10.0, 20.0, 20.0)];
        let rs = RegionSet::from_slice(&regions);
        let processed = rs.processed(&[], -0.6);
        // The 1x1 region collapses under a 0.6 inward shrink (half-width
        // 0.5 < 0.6); the 10x10 region survives.
        assert_eq!(processed.len(), 1);
        assert!(processed.as_slice()[0].area() > 0.0);
    }
}
