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
use crate::toolpath::ClosedPassSide;

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
    ///
    /// D44: a 3D Rough may state its own: the regions it left to a rest
    /// pass (Geodesic Adaptive's left pieces, `adaptive3d/rest_regions.rs`),
    /// only when a `DerivedRestRegions` boundary names it (`run_volumetric`). The
    /// generic analysis then keeps them and attaches only its grid
    /// (precedence per field, `compute/execute.rs`).
    ///
    /// D48 (G-RESTFRAME): every producer publishes TOOL-CENTRE positions,
    /// for a consumer with `Center` containment. The rest field grows its
    /// mask of drop-cutter cells by the tip radius, so the consumer reaches
    /// the region edge (`machining_region_polygons`). A 3D Rough clips its
    /// left pieces to its own containment polygons
    /// (`Adaptive3dGeometry::centre_boundary`), so a consumer tool no
    /// larger than the rough's stays inside the rough's boundary. XY only:
    /// a region carries no Z range; [`Self::rest_regions_top_z`] bounds
    /// them all from above.
    pub rest_regions: Option<Arc<Vec<Polygon2>>>,
    /// D50: the top Z of the stock a 3D Rough left in
    /// [`Self::rest_regions`]: the top of the slab of its highest left
    /// piece (the level above that piece's level, or the level anchor).
    /// One value for the whole set, not one per region. A consumer on
    /// remaining stock starts its levels no higher
    /// (`Adaptive3dDepth::rest_top_z`). `None` when the regions are not a
    /// 3D Rough's own (the rest field states no top) or there are none.
    pub rest_regions_top_z: Option<f64>,
    /// The regions that a 3D Rough with By Area ordering detected (label
    /// grid + region list), for the viewport overlay and MCP
    /// `inspect_spans`. `None` for every other operation and for Global
    /// ordering. Evidence only: no downstream stage reads it.
    pub area_regions: Option<Arc<crate::adaptive3d::AreaRegionMap>>,
    /// P6.0c (SAFETY): the stated material side of each closed pass, keyed
    /// by its plunge target. The 2.5D kernels state it at the emit
    /// ([`crate::toolpath::Toolpath::emit_closed_contour_with_intent`]) and
    /// the Layered runner puts it here. The lead-in/out dressup reads it to
    /// put each lead arc on the air side. Empty for every op with no closed
    /// pass of a stated side.
    pub closed_passes: Vec<ClosedPassSide>,
    /// G-CENTRECUT: the first entry where a shared entry builder could not
    /// keep a non-centre-cutting tool inside its limits (a straight plunge
    /// into stock deeper than the blade protrusion). The session stage
    /// after generation (`JobPhases::run_stages`) records it as a warning
    /// (K6). `None` for every other toolpath.
    pub non_centre_cutting_refusal: Option<crate::compute::tool_config::NonCentreCuttingRefusal>,
    /// CONTRACT K4 (feeds contract P3): one record per entry of the final
    /// moves, under the entry contract of the op's row. `JobPhases::run_stages`
    /// builds them once, after the boundary clip and the entry split
    /// (`compute::catalog::entry_records`). No stage that writes moves
    /// reads them. Empty before that stage and for a toolpath with no
    /// entry.
    pub entries: Vec<crate::compute::catalog::EntryRecord>,
}

impl Produced {
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
    /// - `rest_regions_top_z` (if present) shifts by `shift.z`.
    /// - `area_regions` (if present): the grid origin, the region boxes and
    ///   anchors shift in XY; the top Z and the region Z ranges shift by
    ///   `shift.z`. The labels are cell-indexed and do not change.
    /// - `closed_passes`: each plunge target shifts; the stated side does
    ///   not (a shift keeps the travel direction).
    /// - `non_centre_cutting_refusal`: the plunge target shifts.
    /// - `entries`: each point of each record shifts; the kind and the
    ///   feed do not.
    #[must_use]
    pub fn translated(&self, shift: P3) -> Self {
        let Self {
            planner_engagement,
            rest_grid,
            rest_regions,
            rest_regions_top_z,
            area_regions,
            closed_passes,
            non_centre_cutting_refusal,
            entries,
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

        let rest_regions_top_z = rest_regions_top_z.map(|z| z + shift.z);

        let area_regions = area_regions.clone().map(|mut map| {
            Arc::make_mut(&mut map).translate(shift.x, shift.y, shift.z);
            map
        });

        let closed_passes = closed_passes
            .iter()
            .map(|pass| ClosedPassSide {
                start: P3::new(
                    pass.start.x + shift.x,
                    pass.start.y + shift.y,
                    pass.start.z + shift.z,
                ),
                material: pass.material,
            })
            .collect();

        // G-CENTRECUT: the plunge target shifts; the depths do not.
        let non_centre_cutting_refusal = non_centre_cutting_refusal.map(|mut refusal| {
            refusal.at = P3::new(
                refusal.at.x + shift.x,
                refusal.at.y + shift.y,
                refusal.at.z + shift.z,
            );
            refusal
        });

        // K4: the points of each entry record shift.
        let entries = entries
            .iter()
            .map(|record| record.translated(shift))
            .collect();

        Self {
            planner_engagement,
            rest_grid,
            rest_regions,
            rest_regions_top_z,
            area_regions,
            closed_passes,
            non_centre_cutting_refusal,
            entries,
        }
    }
}
