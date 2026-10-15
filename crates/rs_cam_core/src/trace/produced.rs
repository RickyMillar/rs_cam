//! The typed side channel of a generated toolpath (W6.2, AUDIT.md "the
//! kernel returns more than paths").
//!
//! A kernel returns more than its moves. The 3D Rough planner predicts its
//! engagement, a rest detector builds a rest grid and its region polygons,
//! and a By Area rough detects its areas. Other stages read these: the feed
//! modulator, the GUI overlays, the derived-boundary source of a later op
//! and MCP. [`Produced`] holds all of them in one struct, so a kernel states
//! what it produces in one place and a reader asks for one field.
//!
//! Every field is in the emission frame. [`Produced::translated`] is the one
//! place that moves them to another frame.

use std::sync::Arc;

use crate::geo::P3;
use crate::polygon::Polygon2;
use crate::surface::rest_field::RestGrid;

/// What a kernel produced beside its moves. Each field is empty or `None`
/// when no kernel produced it. `Arc` keeps a clone of the generated result
/// (simulation, result caching) cheap.
///
/// Frame contract: every field holds emission-frame coordinates and must be
/// re-framed wherever `toolpath.moves` are re-framed — see
/// [`Self::translated`].
#[derive(Debug, Clone, Default)]
pub struct Produced {
    /// Stage 4 — planner-predicted leading-arc engagement `(cut_point,
    /// α/2π)` for adaptive ContourSpiral toolpaths; empty otherwise. The
    /// feed modulator looks these up by `Move.target` position (the
    /// samples are positional, so they survive simplify / arcfit / dressup
    /// / TSP-reorder reshaping without per-move re-indexing). See
    /// `planning/ADAPTIVE_CLEARING_ALGO_REVIEW_2026-06-12.md` §"Stage 4".
    pub planner_engagement: Vec<(P3, f64)>,
    /// Rest-depth heatmap grid for the GUI overlay — populated by the
    /// pencil RestDepth detector, the generic post-generation rest
    /// analysis, and UnifiedFinish's claims detector (v3 S1 §2.4
    /// carry-through); `None` otherwise.
    pub rest_grid: Option<Arc<RestGrid>>,
    /// Machining-region polygons derived from the rest-depth field
    /// ([`crate::surface::rest_field::RestFieldResult::region_polygons`]) — same
    /// producers as [`Self::rest_grid`]; `None` otherwise. XY polygons.
    /// This is the derived-boundary source for selective finishing (P2.2
    /// `BoundarySource::DerivedRestRegions`).
    pub rest_regions: Option<Arc<Vec<Polygon2>>>,
    /// The regions that a 3D Rough with By Area ordering detected (label
    /// grid + region list), for the viewport overlay and MCP
    /// `inspect_spans`. `None` for every other operation and for Global
    /// ordering. Evidence only: no downstream stage reads it.
    pub area_regions: Option<Arc<crate::adaptive3d::AreaRegionMap>>,
}

impl Produced {
    /// True when no rest detector attached its output. The dispatch runs
    /// the generic rest analysis only then (P2.5: one source per toolpath).
    pub fn has_no_rest_output(&self) -> bool {
        self.rest_grid.is_none() && self.rest_regions.is_none()
    }

    /// Re-frame every coordinate-bearing field by `shift`.
    ///
    /// Destructures `self` field by field on purpose: a new field does not
    /// compile here until its author decides how it transforms.
    ///
    /// - `planner_engagement` cut points shift; the angle fraction does not.
    /// - `rest_grid` (if present) has `origin_x`/`origin_y` shifted and
    ///   every `surface_z` cell shifted by `shift.z`; `NaN` cells stay
    ///   `NaN` since `NaN + x == NaN`. `rest` values do not change — rest
    ///   depth is a scalar, not a coordinate.
    /// - `rest_regions` (if present): every exterior and hole point shifts
    ///   by `shift.x`/`shift.y` (the polygons carry no Z).
    /// - `area_regions` (if present): the grid origin, the region boxes and
    ///   anchors shift in XY; the top Z and the region Z ranges shift by
    ///   `shift.z`. The labels are cell-indexed and do not change.
    #[must_use]
    pub fn translated(&self, shift: P3) -> Self {
        let Self {
            planner_engagement,
            rest_grid,
            rest_regions,
            area_regions,
        } = self;

        let planner_engagement = planner_engagement
            .iter()
            .map(|(p, alpha)| (P3::new(p.x + shift.x, p.y + shift.y, p.z + shift.z), *alpha))
            .collect();

        let rest_grid = rest_grid.clone().map(|mut grid| {
            let g = Arc::make_mut(&mut grid);
            g.grid.origin_x += shift.x;
            g.grid.origin_y += shift.y;
            for z in &mut g.surface_z {
                *z += shift.z as f32;
            }
            grid
        });

        let rest_regions = rest_regions.clone().map(|mut regions| {
            let polys = Arc::make_mut(&mut regions);
            for poly in polys.iter_mut() {
                for p in &mut poly.exterior {
                    p.x += shift.x;
                    p.y += shift.y;
                }
                for hole in &mut poly.holes {
                    for p in hole.iter_mut() {
                        p.x += shift.x;
                        p.y += shift.y;
                    }
                }
                // `exterior` moved — see the mutation contract on `Polygon2`.
                poly.invalidate_bbox();
            }
            regions
        });

        let area_regions = area_regions.clone().map(|mut map| {
            Arc::make_mut(&mut map).translate(shift.x, shift.y, shift.z);
            map
        });

        Self {
            planner_engagement,
            rest_grid,
            rest_regions,
            area_regions,
        }
    }
}
