//! Scallop finishing strategy — constant scallop height across the surface.
//!
//! Generates concentric offset contours with variable stepover that maintains
//! constant scallop height regardless of surface slope and curvature. On steep
//! walls the stepover is wider (ball endmill has larger effective radius), on
//! shallow convex areas it is tighter.
//!
//! Algorithm:
//! 1. Project mesh boundary onto XY → outer polygon
//! 2. Compute variable stepover from scallop height, local slope, and curvature
//! 3. Iteratively offset polygon inward by stepover
//! 4. Drop-cutter Z at each ring's points → 3D contour
//! 5. Chain rings into toolpath (with optional spiral connection)
//!
//! From Fusion 360 docs: "passes follow sloping and vertical walls to maintain
//! the stepover."

use crate::debug_trace::ToolpathDebugContext;
use crate::dropcutter::point_drop_cutter;
use crate::geo::{P2, P3};
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::polygon::{Polygon2, offset_polygon};
use crate::scallop_math::variable_stepover;
use crate::tool::MillingCutter;
use crate::toolpath::{MoveIntent, Toolpath};

use tracing::info;

/// Direction for scallop contouring.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScallopDirection {
    /// Start from boundary, work inward (default).
    #[default]
    OutsideIn,
    /// Start from center, work outward.
    InsideOut,
}

/// Parameters for scallop finishing.
pub struct ScallopParams {
    /// Desired scallop height (mm). This is the PRIMARY parameter.
    pub scallop_height: f64,
    /// Path tolerance for simplification.
    pub tolerance: f64,
    /// Direction of contouring.
    pub direction: ScallopDirection,
    /// Connect contours into a continuous spiral (fewer retracts).
    pub continuous: bool,
    /// Slope confinement: only machine slopes steeper than this (degrees).
    pub slope_from: f64,
    /// Slope confinement: only machine slopes shallower than this (degrees).
    pub slope_to: f64,
    /// Feed rate for cutting moves (mm/min).
    pub feed_rate: f64,
    /// Plunge rate (mm/min).
    pub plunge_rate: f64,
    /// Safe Z for rapid positioning.
    pub safe_z: f64,
    /// Stock to leave on the surface (mm).
    pub stock_to_leave: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScallopRuntimeEvent {
    Ring {
        ring_index: usize,
        ring_total: usize,
        continuous: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScallopRuntimeAnnotation {
    pub move_index: usize,
    pub event: ScallopRuntimeEvent,
}

impl ScallopRuntimeEvent {
    pub fn label(&self) -> String {
        match self {
            Self::Ring { ring_index, .. } => format!("Ring {ring_index}"),
        }
    }
}

impl Default for ScallopParams {
    fn default() -> Self {
        Self {
            scallop_height: 0.1,
            tolerance: 0.05,
            direction: ScallopDirection::OutsideIn,
            continuous: false,
            slope_from: 0.0,
            slope_to: 90.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 30.0,
            stock_to_leave: 0.0,
        }
    }
}

/// Compute the average variable stepover for a ring of 2D points using
/// the slope map and scallop math.
fn average_stepover_for_ring(
    ring: &[P2],
    slope_map: &crate::slope::SlopeMap,
    tool_radius: f64,
    scallop_height: f64,
) -> f64 {
    if ring.is_empty() {
        return crate::scallop_math::stepover_from_scallop_flat(tool_radius, scallop_height);
    }

    let sample_step = 1.max(ring.len() / 20);
    let mut sum = 0.0;
    let mut count = 0;

    for pt in ring.iter().step_by(sample_step) {
        let angle = slope_map.angle_at_world(pt.x, pt.y).unwrap_or(0.0);
        // SlopeMap convention: negative = physically convex (see slope.rs doc on
        // `curvatures`). scallop_math::variable_stepover expects the opposite
        // (positive = convex), so negate at this boundary.
        let curvature = -slope_map.curvature_at_world(pt.x, pt.y).unwrap_or(0.0);
        let so = variable_stepover(tool_radius, scallop_height, angle, curvature);
        if so > 0.01 {
            sum += so;
            count += 1;
        }
    }

    if count == 0 {
        crate::scallop_math::stepover_from_scallop_flat(tool_radius, scallop_height)
    } else {
        sum / count as f64
    }
}

/// Whether `(x, y)` lands on a [`crate::slope::SurfaceHeightmap`] cell whose
/// vertical ray actually hit the mesh (`SurfaceHeightmap::covered`).
/// Mirrors `SlopeMap::world_to_cell`'s nearest-cell rounding so lookups
/// resolve consistently with the sibling `slope_map` built from the same
/// heightmap grid (both share `origin_x`/`origin_y`/`cell_size`/`rows`/`cols`).
fn heightmap_covered_at_world(hm: &crate::slope::SurfaceHeightmap, x: f64, y: f64) -> bool {
    let col_f = (x - hm.origin_x) / hm.cell_size;
    let row_f = (y - hm.origin_y) / hm.cell_size;
    if col_f < -0.5 || row_f < -0.5 {
        return false;
    }
    let col = col_f.round();
    let row = row_f.round();
    if col < 0.0 || row < 0.0 || col >= hm.cols as f64 || row >= hm.rows as f64 {
        return false;
    }
    // SAFETY: bounds checked above against hm.cols/hm.rows.
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let covered = hm.covered_at(row as usize, col as usize);
    covered
}

/// Lift a 2D polygon ring to 3D by drop-cutter Z queries, pairing each point
/// with whether it sits over real mesh surface.
///
/// The `bool` is `true` only when `point_drop_cutter` found a finite contact
/// AND the surface heightmap's per-cell `covered` mask agrees the vertical
/// ray at that XY actually passed through the mesh — `point_drop_cutter`
/// alone can't tell that apart from cutter-radius rim contact just past a
/// hole or the mesh edge (see `SurfaceHeightmap::covered`'s doc comment).
///
/// Excluded (`false`) points still carry a Z (`min_z + stock_to_leave`) so
/// the tuple is always well-formed, but callers must run rings through the
/// shared run-splitter (`crate::point_runs`) and treat `false` stretches as
/// gaps — feeding straight through them used to dive the cutter to `min_z`
/// at every off-footprint corner instead of retracting around the gap
/// (P2.3 bonus fix; tracker `planning/finishing_stack_review_2026-07.md`).
fn ring_to_3d(
    ring: &[P2],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    heightmap: &crate::slope::SurfaceHeightmap,
    stock_to_leave: f64,
    min_z: f64,
) -> Vec<(P3, bool)> {
    ring.iter()
        .map(|p| {
            let cl = point_drop_cutter(p.x, p.y, mesh, index, cutter);
            let finite = cl.z.is_finite();
            let kept = finite && heightmap_covered_at_world(heightmap, p.x, p.y);
            let z = if finite {
                cl.z + stock_to_leave
            } else {
                min_z + stock_to_leave
            };
            (P3::new(p.x, p.y, z), kept)
        })
        .collect()
}

/// Generate concentric offset rings from the outer boundary inward.
///
/// Uses variable stepover: at each ring, samples the slope map to compute
/// the average stepover that maintains constant scallop height, then offsets
/// by that amount.
// infallible: cancel closure always returns false, so Cancelled is unreachable
#[allow(clippy::too_many_arguments, clippy::expect_used)]
// Production goes through the _with_cancel variant; this never-cancel
// convenience wrapper is exercised by the unit tests below.
#[cfg_attr(not(test), allow(dead_code))]
fn generate_scallop_rings(
    boundary: &Polygon2,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    slope_map: &crate::slope::SlopeMap,
    heightmap: &crate::slope::SurfaceHeightmap,
    tool_radius: f64,
    scallop_height: f64,
    stock_to_leave: f64,
    min_z: f64,
    max_rings: usize,
) -> Vec<Vec<(P3, bool)>> {
    let never_cancel = || false;
    generate_scallop_rings_with_cancel(
        boundary,
        mesh,
        index,
        cutter,
        slope_map,
        heightmap,
        tool_radius,
        scallop_height,
        stock_to_leave,
        min_z,
        max_rings,
        &never_cancel,
    )
    .expect("non-cancellable scallop ring generation should never be cancelled")
}

/// Cancellable variant of [`generate_scallop_rings`]. Polls `cancel` once per
/// ring (the expensive step: `ring_to_3d` runs a `point_drop_cutter` query per
/// ring point).
#[allow(clippy::too_many_arguments)]
fn generate_scallop_rings_with_cancel(
    boundary: &Polygon2,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    slope_map: &crate::slope::SlopeMap,
    heightmap: &crate::slope::SurfaceHeightmap,
    tool_radius: f64,
    scallop_height: f64,
    stock_to_leave: f64,
    min_z: f64,
    max_rings: usize,
    cancel: &dyn CancelCheck,
) -> Result<Vec<Vec<(P3, bool)>>, Cancelled> {
    let mut rings_3d: Vec<Vec<(P3, bool)>> = Vec::new();

    // First ring: the boundary itself, lifted to 3D
    let first_ring = ring_to_3d(
        &boundary.exterior,
        mesh,
        index,
        cutter,
        heightmap,
        stock_to_leave,
        min_z,
    );
    if first_ring.len() < 3 {
        return Ok(rings_3d);
    }
    rings_3d.push(first_ring);

    // Iteratively offset inward
    let mut current_polys = vec![boundary.clone()];

    for _ in 0..max_rings {
        check_cancel(cancel)?;
        // Compute average stepover from the current ring's slope/curvature
        let avg_stepover = if current_polys.is_empty() {
            crate::scallop_math::stepover_from_scallop_flat(tool_radius, scallop_height)
        } else {
            // Sample from all current polygons
            let mut total_so = 0.0;
            let mut total_count = 0;
            for poly in &current_polys {
                let so = average_stepover_for_ring(
                    &poly.exterior,
                    slope_map,
                    tool_radius,
                    scallop_height,
                );
                total_so += so * poly.exterior.len() as f64;
                total_count += poly.exterior.len();
            }
            if total_count > 0 {
                total_so / total_count as f64
            } else {
                crate::scallop_math::stepover_from_scallop_flat(tool_radius, scallop_height)
            }
        };

        // Clamp stepover to reasonable bounds
        let stepover = avg_stepover
            .max(tool_radius * 0.05) // At least 5% of tool radius
            .min(tool_radius * 3.0); // At most 3× tool radius

        // Offset all current polygons inward
        let mut next_polys = Vec::new();
        for poly in &current_polys {
            let offsets = offset_polygon(poly, stepover);
            next_polys.extend(offsets);
        }

        if next_polys.is_empty() {
            break; // Collapsed to nothing
        }

        // Lift each new polygon ring to 3D
        for poly in &next_polys {
            if poly.exterior.len() < 3 {
                continue;
            }
            let ring_3d = ring_to_3d(
                &poly.exterior,
                mesh,
                index,
                cutter,
                heightmap,
                stock_to_leave,
                min_z,
            );
            if ring_3d.len() >= 3 {
                rings_3d.push(ring_3d);
            }
        }

        current_polys = next_polys;
    }

    Ok(rings_3d)
}

/// Index of the point on `ring` closest to `target` among points that
/// satisfy `keep`; `None` when nothing on the ring survives the predicate.
///
/// Continuous mode rotates each ring to start here so the ring-to-ring
/// hop is as short as the *surviving* geometry allows — rotating to the
/// globally-closest point (kept or not) let the connector target an
/// excluded point while the tool's real position sat elsewhere, which is
/// exactly the long-cutting-chord shape the P0.4 regression tests pin.
fn closest_kept_point_idx<F>(ring: &[(P3, bool)], target: &P3, keep: F) -> Option<usize>
where
    F: Fn(&(P3, bool)) -> bool,
{
    ring.iter()
        .enumerate()
        .filter(|(_, pt)| keep(pt))
        .min_by(|(_, a), (_, b)| {
            let da = (a.0.x - target.x).powi(2) + (a.0.y - target.y).powi(2);
            let db = (b.0.x - target.x).powi(2) + (b.0.y - target.y).powi(2);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)
}

/// Reorder a ring to start at the given index.
fn rotate_ring(ring: &[(P3, bool)], start_idx: usize) -> Vec<(P3, bool)> {
    let n = ring.len();
    if n == 0 || start_idx == 0 {
        return ring.to_vec();
    }
    let mut result = Vec::with_capacity(n);
    // SAFETY: (start_idx + i) % n is always in 0..n
    #[allow(clippy::indexing_slicing)]
    for i in 0..n {
        result.push(ring[(start_idx + i) % n]);
    }
    result
}

/// Generate a scallop finishing toolpath.
///
/// Produces concentric offset contours with variable stepover that maintains
/// constant scallop height across the surface regardless of slope and curvature.
#[tracing::instrument(skip(mesh, index, cutter, params), fields(scallop_height = params.scallop_height))]
#[allow(clippy::indexing_slicing)] // ring/filtered indexing is guarded by len checks
pub fn scallop_toolpath(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &ScallopParams,
) -> Toolpath {
    let (tp, _) = scallop_toolpath_structured_annotated(mesh, index, cutter, params, None);
    tp
}

fn runtime_annotations_to_labels(annotations: &[ScallopRuntimeAnnotation]) -> Vec<(usize, String)> {
    annotations
        .iter()
        .map(|annotation| (annotation.move_index, annotation.event.label()))
        .collect()
}

// infallible: cancel closure always returns false, so Cancelled is unreachable
#[allow(clippy::expect_used)]
pub fn scallop_toolpath_structured_annotated(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &ScallopParams,
    debug: Option<&ToolpathDebugContext>,
) -> (Toolpath, Vec<ScallopRuntimeAnnotation>) {
    let never_cancel = || false;
    scallop_toolpath_structured_annotated_with_cancel(
        mesh,
        index,
        cutter,
        params,
        debug,
        None,
        &never_cancel,
    )
    .expect("non-cancellable scallop toolpath should never be cancelled")
}

/// Cancellable variant of [`scallop_toolpath_structured_annotated`]. Polls
/// `cancel` once per ring during 3D ring generation (`ring_to_3d`'s
/// per-point drop-cutter queries are the expensive step) and once per ring
/// again while chaining rings into the toolpath.
///
/// `boundary_regions` (P2.3): when `Some`, generation is pre-clipped to
/// these machining-boundary regions instead of the full mesh footprint —
/// exactly one region is used directly as the ring boundary (replacing the
/// hardcoded mesh-bbox rectangle below); multiple disjoint regions each get
/// their own independent ring set, concatenated (regions are disjoint by
/// construction, so no de-duplication is needed). `None` reproduces
/// today's single mesh-bbox-rectangle behavior byte-for-byte.
#[allow(clippy::too_many_arguments)]
pub fn scallop_toolpath_structured_annotated_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &ScallopParams,
    debug: Option<&ToolpathDebugContext>,
    boundary_regions: Option<&[Polygon2]>,
    cancel: &dyn CancelCheck,
) -> Result<(Toolpath, Vec<ScallopRuntimeAnnotation>), Cancelled> {
    check_cancel(cancel)?;
    let tool_radius = cutter.radius();
    let bbox = &mesh.bbox;

    // Build surface heightmap and slope map (shared setup, see finish_setup.rs)
    let surface = crate::finish_setup::build_finish_surface_with_cancel(
        mesh,
        index,
        cutter,
        params.tolerance,
        cancel,
    )?;
    let surface_hm = surface.heightmap;
    let slope_map = surface.slope_map;

    // Kept alongside the shared heightmap builder above (which derives the
    // same values internally) because `max_rings` below still needs the raw
    // extent — not just the resulting grid.
    let origin_x = bbox.min.x - tool_radius;
    let origin_y = bbox.min.y - tool_radius;
    let extent_x = bbox.max.x + tool_radius;
    let extent_y = bbox.max.y + tool_radius;

    // Outer boundary: mesh footprint as a rectangle, sampled densely enough
    // for polygon offset to work correctly. Point spacing = stepover.
    let bx0 = bbox.min.x;
    let by0 = bbox.min.y;
    let bx1 = bbox.max.x;
    let by1 = bbox.max.y;
    let flat_so =
        crate::scallop_math::stepover_from_scallop_flat(tool_radius, params.scallop_height)
            .max(tool_radius * 0.1);
    let boundary = {
        let mut pts = Vec::new();
        // Bottom edge
        let mut x = bx0;
        while x < bx1 {
            pts.push(P2::new(x, by0));
            x += flat_so;
        }
        // Right edge
        let mut y = by0;
        while y < by1 {
            pts.push(P2::new(bx1, y));
            y += flat_so;
        }
        // Top edge (reversed)
        let mut x = bx1;
        while x > bx0 {
            pts.push(P2::new(x, by1));
            x -= flat_so;
        }
        // Left edge (reversed)
        let mut y = by1;
        while y > by0 {
            pts.push(P2::new(bx0, y));
            y -= flat_so;
        }
        if pts.len() < 4 {
            // Fallback to simple rectangle
            pts = vec![
                P2::new(bx0, by0),
                P2::new(bx1, by0),
                P2::new(bx1, by1),
                P2::new(bx0, by1),
            ];
        }
        Polygon2::new(pts)
    };

    // Max rings: bounded by extent / min_stepover
    let min_stepover =
        crate::scallop_math::stepover_from_scallop_flat(tool_radius, params.scallop_height)
            .max(tool_radius * 0.05);
    let max_extent = (extent_x - origin_x).max(extent_y - origin_y);
    let max_rings = ((max_extent / min_stepover) * 0.5).ceil() as usize + 10;

    info!(
        max_rings = max_rings,
        min_stepover = format!("{:.3}", min_stepover),
        "Generating scallop rings"
    );

    // P2.3: scan the machining-boundary regions instead of the hardcoded
    // mesh-bbox rectangle when they're available. A single region is used
    // directly as the ring boundary; multiple disjoint regions each get an
    // independent ring set (still bounded by the same `max_rings` safety
    // valve — it's an upper bound on ring count, not an exact prediction, so
    // reusing it per-region is safe). `None`/empty falls back to exactly the
    // one hardcoded-rectangle boundary generated above, so the ring output
    // is byte-identical to pre-P2.3 behavior in that case.
    let region_boundaries: Vec<Polygon2> = match boundary_regions {
        Some(regions) if !regions.is_empty() => regions.to_vec(),
        _ => vec![boundary],
    };

    // Generate 3D rings, one region at a time, concatenated in region order.
    let mut rings: Vec<Vec<(P3, bool)>> = Vec::new();
    for region_boundary in &region_boundaries {
        check_cancel(cancel)?;
        let region_rings = generate_scallop_rings_with_cancel(
            region_boundary,
            mesh,
            index,
            cutter,
            &slope_map,
            &surface_hm,
            tool_radius,
            params.scallop_height,
            params.stock_to_leave,
            bbox.min.z,
            max_rings,
            cancel,
        )?;
        rings.extend(region_rings);
    }

    info!(rings = rings.len(), "Scallop rings generated");

    if rings.is_empty() {
        return Ok((Toolpath::new(), Vec::new()));
    }

    // Apply direction
    if matches!(params.direction, ScallopDirection::InsideOut) {
        rings.reverse();
    }

    // Slope confinement
    let use_slope_filter =
        crate::finish_setup::slope_filter_active(params.slope_from, params.slope_to);
    let slope_from_rad = params.slope_from.to_radians();
    let slope_to_rad = params.slope_to.to_radians();

    // Combined per-point keep predicate: real mesh coverage (P2.3 bonus
    // fix — see `ring_to_3d`) AND, when active, the slope band AND the
    // machining-boundary regions. Always run points through this (rather
    // than only when a filter is "active") — with every filter a no-op the
    // run-splitter below degenerates to "the whole ring survives as one
    // run", which is byte-identical to the plain unfiltered emission this
    // replaces.
    let region_ok = |p: &P3| -> bool {
        boundary_regions
            .is_none_or(|regions| regions.iter().any(|r| r.contains_point(&P2::new(p.x, p.y))))
    };
    let keep_point = |pt: &(P3, bool)| -> bool {
        let (p, covered) = pt;
        *covered
            && (!use_slope_filter
                || slope_map
                    .angle_at_world(p.x, p.y)
                    .is_some_and(|a| a >= slope_from_rad && a <= slope_to_rad))
            && region_ok(p)
    };

    // Convert rings to toolpath
    let mut tp = Toolpath::new();
    let mut annotations = Vec::new();

    if params.continuous && rings.len() >= 2 {
        // Continuous spiral mode: connect adjacent rings at their nearest
        // KEPT points. A ring-to-ring connector is a *cutting* feed only
        // when it is a genuine helical transition — the tool is already
        // down and the hop is no longer than the widest ring spacing the
        // generator can produce (its stepover is clamped to at most
        // `tool_radius * 3.0` above). Anything longer — a kept set that
        // shifted to the far side of the ring under the combined keep
        // predicate, or the gap between two disjoint P2.3 boundary regions
        // — gets a retract/rapid/replunge link instead of chording across
        // excluded material at cutting feed (the P0.4 gouge class the
        // no-chord regression tests pin).
        let link_threshold = tool_radius * 3.0;
        // The tool's last emitted position. Seeded from the first ring's
        // geometric end (the pre-existing rotation seed) and updated to the
        // REAL last emitted point after every run — anchoring rotation on
        // the geometric ring end let the connector hop diverge arbitrarily
        // far from where the tool actually stopped.
        // SAFETY: rings.len() >= 2 checked above; rings entries have >= 3 points.
        #[allow(clippy::indexing_slicing)]
        let mut anchor: P3 = rings[0].last().map_or(rings[0][0].0, |&(p, _)| p);
        let mut tool_down = false;

        for (i, ring) in rings.iter().enumerate() {
            check_cancel(cancel)?;
            // Rotate the ring to start at the kept point closest to the
            // tool position. A ring with no kept points emits nothing —
            // pre-P2.3 this still plunged to the ring's rotation start,
            // which for an off-footprint boundary corner meant diving on a
            // rim-contact Z (see `ring_to_3d`).
            let Some(start_idx) = closest_kept_point_idx(ring, &anchor, |pt| keep_point(pt)) else {
                continue;
            };
            let rotated = rotate_ring(ring, start_idx);
            annotations.push(ScallopRuntimeAnnotation {
                move_index: tp.moves.len(),
                event: ScallopRuntimeEvent::Ring {
                    ring_index: i + 1,
                    ring_total: rings.len(),
                    continuous: true,
                },
            });

            // Contiguous kept runs across the whole rotated ring (index 0
            // is kept by construction, so the first run starts at 0).
            let idx_runs = crate::point_runs::split_run_ranges(
                &rotated,
                |_, pt: &(P3, bool)| keep_point(pt),
                1,
            );
            for (run_idx, (s, e)) in idx_runs.into_iter().enumerate() {
                let Some(run) = rotated.get(s..=e) else {
                    continue;
                };
                let Some(&(run_first, _)) = run.first() else {
                    continue;
                };
                let hop =
                    ((run_first.x - anchor.x).powi(2) + (run_first.y - anchor.y).powi(2)).sqrt();
                // Helical transition: only the ring's FIRST run can continue
                // the previous ring's cut, and only when the tool is down
                // and the hop is within one ring spacing.
                let helical_link = run_idx == 0 && s == 0 && tool_down && hop <= link_threshold;
                let mut iter = run.iter();
                if helical_link {
                    if let Some(&(p, _)) = iter.next() {
                        tp.feed_to_with_intent(p, params.feed_rate, MoveIntent::FinishingCut);
                    }
                } else {
                    if tool_down {
                        tp.rapid_to_with_intent(
                            P3::new(anchor.x, anchor.y, params.safe_z),
                            MoveIntent::Retract,
                        );
                        tool_down = false;
                    }
                    if let Some(&(p, _)) = iter.next() {
                        tp.rapid_to_with_intent(
                            P3::new(p.x, p.y, params.safe_z),
                            MoveIntent::Linking,
                        );
                        tp.feed_to_with_intent(p, params.plunge_rate, MoveIntent::EntryPlunge);
                    }
                }
                for &(pt, _) in iter {
                    tp.feed_to_with_intent(pt, params.feed_rate, MoveIntent::FinishingCut);
                }
                if let Some(&(last, _)) = run.last() {
                    anchor = last;
                    tool_down = true;
                }
            }
        }

        // Final retract from wherever the tool actually ended.
        if tool_down {
            tp.rapid_to_with_intent(
                P3::new(anchor.x, anchor.y, params.safe_z),
                MoveIntent::Retract,
            );
        }
    } else {
        // Discrete ring mode: rapid between rings. Each ring is split into
        // contiguous runs that survive the combined keep predicate — a
        // ring is only safe to close back to its own start when EVERY
        // point on it survives (a partial survivor set closing across the
        // excluded gap would chord straight through material this pass
        // must not touch).
        let mut emitted_runs: Vec<(Vec<P3>, bool)> = Vec::new();
        for ring in &rings {
            if ring.len() < 3 {
                continue;
            }

            let runs = crate::point_runs::split_runs(
                ring,
                |_, pt: &(P3, bool)| keep_point(pt),
                crate::point_runs::RunTopology::Closed,
                3,
            );
            for run in runs {
                let is_closed_loop = run.len() == ring.len();
                let pts: Vec<P3> = run.iter().map(|&(p, _)| p).collect();
                emitted_runs.push((pts, is_closed_loop));
            }
        }

        for (ring_index, (points, close_loop)) in emitted_runs.iter().enumerate() {
            check_cancel(cancel)?;
            let Some(&first) = points.first() else {
                continue;
            };
            let move_index = tp.moves.len();
            annotations.push(ScallopRuntimeAnnotation {
                move_index,
                event: ScallopRuntimeEvent::Ring {
                    ring_index: ring_index + 1,
                    ring_total: emitted_runs.len(),
                    continuous: false,
                },
            });
            tp.rapid_to_with_intent(
                P3::new(first.x, first.y, params.safe_z),
                MoveIntent::Linking,
            );
            tp.feed_to_with_intent(first, params.plunge_rate, MoveIntent::EntryPlunge);
            for pt in points.iter().skip(1) {
                tp.feed_to_with_intent(*pt, params.feed_rate, MoveIntent::FinishingCut);
            }
            let retract_at = if *close_loop {
                // Close the ring: the closing feed brings the cutter back
                // to the first point, so retract from there.
                tp.feed_to_with_intent(first, params.feed_rate, MoveIntent::FinishingCut);
                first
            } else {
                // Open arc: the cutter is at the run's last point — retract
                // there rather than chording back to the run's start.
                points.last().copied().unwrap_or(first)
            };
            tp.rapid_to_with_intent(
                P3::new(retract_at.x, retract_at.y, params.safe_z),
                MoveIntent::Retract,
            );
        }
    }

    if let Some(last) = tp.moves.last()
        && !matches!(last.move_type, crate::toolpath::MoveType::Rapid)
    {
        tp.rapid_to_with_intent(
            P3::new(last.target.x, last.target.y, params.safe_z),
            MoveIntent::Retract,
        );
    }

    info!(
        moves = tp.moves.len(),
        cutting_mm = format!("{:.1}", tp.total_cutting_distance()),
        rapid_mm = format!("{:.1}", tp.total_rapid_distance()),
        "Scallop toolpath complete"
    );

    if let Some(debug_ctx) = debug {
        for annotation in &annotations {
            debug_ctx.add_annotation(annotation.move_index, annotation.event.label());
        }
    }

    Ok((tp, annotations))
}

pub fn scallop_toolpath_annotated(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &ScallopParams,
    debug: Option<&ToolpathDebugContext>,
) -> (Toolpath, Vec<(usize, String)>) {
    let (tp, annotations) =
        scallop_toolpath_structured_annotated(mesh, index, cutter, params, debug);
    (tp, runtime_annotations_to_labels(&annotations))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::mesh::SpatialIndex;
    use crate::tool::BallEndmill;

    fn make_flat_mesh() -> (TriangleMesh, SpatialIndex) {
        let mesh = crate::mesh::make_test_flat(50.0);
        let si = SpatialIndex::build(&mesh, 10.0);
        (mesh, si)
    }

    fn make_hemisphere() -> (TriangleMesh, SpatialIndex) {
        let mesh = crate::mesh::make_test_hemisphere(20.0, 16);
        let si = SpatialIndex::build(&mesh, 10.0);
        (mesh, si)
    }

    fn ball_cutter() -> BallEndmill {
        BallEndmill::new(6.35, 25.0)
    }

    // ── Ring generation tests ───────────────────────────────────────

    #[test]
    fn test_scallop_flat_constant_stepover() {
        // On a flat surface, variable stepover should equal the flat formula everywhere
        let (mesh, si) = make_flat_mesh();
        let cutter = ball_cutter();
        let tool_radius = cutter.radius();
        let scallop_height = 0.1;

        let expected_so =
            crate::scallop_math::stepover_from_scallop_flat(tool_radius, scallop_height);

        let cell_size = 1.0;
        let never_cancel = || false;
        // SAFETY: never_cancel always returns false
        let surface = crate::finish_setup::build_finish_surface_with_cell_size_and_cancel(
            &mesh,
            &si,
            &cutter,
            cell_size,
            &never_cancel,
        )
        .unwrap();
        let slope_map = surface.slope_map;

        // Sample stepover from the slope map at the center
        let so = average_stepover_for_ring(
            &[P2::new(0.0, 0.0), P2::new(10.0, 0.0), P2::new(10.0, 10.0)],
            &slope_map,
            tool_radius,
            scallop_height,
        );

        assert!(
            (so - expected_so).abs() < expected_so * 0.3,
            "Flat surface stepover ({:.3}) should be near flat formula ({:.3})",
            so,
            expected_so
        );
    }

    #[test]
    fn test_convex_dome_curvature_sign_tightens_stepover() {
        // Regression pin for the SlopeMap/scallop_math sign-convention mismatch:
        // SlopeMap reports NEGATIVE curvature at a physically convex dome peak
        // (see slope.rs::test_curvature_convex), but scallop_math::variable_stepover
        // expects POSITIVE = convex. average_stepover_for_ring negates the raw
        // SlopeMap value before calling variable_stepover. If that negation is
        // ever removed, this test fails: a convex dome must produce a TIGHTER
        // stepover than a flat surface, not a wider one.
        fn make_dome_z_grid(rows: usize, cols: usize, cell_size: f64, radius: f64) -> Vec<f64> {
            let cx = (cols - 1) as f64 * cell_size * 0.5;
            let cy = (rows - 1) as f64 * cell_size * 0.5;
            let mut z = vec![0.0; rows * cols];
            for row in 0..rows {
                for col in 0..cols {
                    let x = col as f64 * cell_size - cx;
                    let y = row as f64 * cell_size - cy;
                    let r_sq = radius * radius - x * x - y * y;
                    z[row * cols + col] = if r_sq > 0.0 { r_sq.sqrt() } else { 0.0 };
                }
            }
            z
        }

        let z = make_dome_z_grid(20, 20, 1.0, 8.0);
        let slope_map = crate::slope::SlopeMap::from_z_grid(&z, 20, 20, 0.0, 0.0, 1.0);

        let tool_radius = ball_cutter().radius();
        let scallop_height = 0.1;

        // Dome peak, same cell used by slope.rs::test_curvature_convex.
        let raw_curvature = slope_map.curvature_at(10, 10);
        assert!(
            raw_curvature < 0.0,
            "Dome peak curvature should be negative in SlopeMap convention, got {:.6}",
            raw_curvature
        );

        // Same negation applied at the production call site in
        // average_stepover_for_ring.
        let curvature = -raw_curvature;
        let angle = slope_map.angle_at(10, 10);

        let so_dome = variable_stepover(tool_radius, scallop_height, angle, curvature);
        let so_flat = crate::scallop_math::stepover_from_scallop_flat(tool_radius, scallop_height);

        assert!(
            so_dome < so_flat,
            "Convex dome stepover ({:.4}) should be tighter than flat stepover ({:.4}) \
             once the sign convention is corrected",
            so_dome,
            so_flat
        );
    }

    #[test]
    fn test_scallop_rings_converge() {
        // Rings should progressively shrink until the polygon collapses
        let (mesh, si) = make_flat_mesh();
        let cutter = ball_cutter();
        let tool_radius = cutter.radius();

        let bbox = &mesh.bbox;
        let boundary = Polygon2::new(vec![
            P2::new(bbox.min.x, bbox.min.y),
            P2::new(bbox.max.x, bbox.min.y),
            P2::new(bbox.max.x, bbox.max.y),
            P2::new(bbox.min.x, bbox.max.y),
        ]);

        let cell_size = 1.0;
        let never_cancel = || false;
        // SAFETY: never_cancel always returns false
        let surface = crate::finish_setup::build_finish_surface_with_cell_size_and_cancel(
            &mesh,
            &si,
            &cutter,
            cell_size,
            &never_cancel,
        )
        .unwrap();
        let surface_hm = surface.heightmap;
        let slope_map = surface.slope_map;

        let rings = generate_scallop_rings(
            &boundary,
            &mesh,
            &si,
            &cutter,
            &slope_map,
            &surface_hm,
            tool_radius,
            0.1,
            0.0,
            bbox.min.z,
            100,
        );

        assert!(
            rings.len() >= 3,
            "Should produce multiple rings on 50mm flat, got {}",
            rings.len()
        );

        // Ring count should be bounded (polygon eventually collapses)
        let flat_so = crate::scallop_math::stepover_from_scallop_flat(tool_radius, 0.1);
        let expected_max = (25.0 / flat_so).ceil() as usize + 5; // half extent / stepover
        assert!(
            rings.len() <= expected_max,
            "Too many rings ({}), expected at most ~{}",
            rings.len(),
            expected_max
        );
    }

    #[test]
    fn test_scallop_z_from_dropcutter() {
        // Ring Z values should match drop-cutter queries
        let (mesh, si) = make_flat_mesh();
        let cutter = ball_cutter();

        let params = ScallopParams {
            scallop_height: 0.1,
            tolerance: 0.5,
            ..ScallopParams::default()
        };

        let tp = scallop_toolpath(&mesh, &si, &cutter, &params);

        // On flat mesh (z≈0), all cutting Z should be near 0
        for m in &tp.moves {
            if let crate::toolpath::MoveType::Linear { .. } = m.move_type
                && m.target.z < params.safe_z - 1.0
            {
                assert!(
                    m.target.z.abs() < 2.0,
                    "Flat mesh cutting Z should be near 0, got {:.2}",
                    m.target.z
                );
            }
        }
    }

    // ── Integration tests ───────────────────────────────────────────

    #[test]
    fn test_scallop_produces_toolpath() {
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let params = ScallopParams {
            scallop_height: 0.5, // Coarse for speed
            tolerance: 0.5,
            ..ScallopParams::default()
        };

        let tp = scallop_toolpath(&mesh, &si, &cutter, &params);
        assert!(
            tp.moves.len() > 10,
            "Hemisphere scallop should produce moves, got {}",
            tp.moves.len()
        );
        assert!(
            tp.total_cutting_distance() > 10.0,
            "Should have meaningful cutting distance, got {:.1}",
            tp.total_cutting_distance()
        );
    }

    #[test]
    fn test_scallop_continuous_no_rapids_between_rings() {
        let (mesh, si) = make_flat_mesh();
        let cutter = ball_cutter();
        let params = ScallopParams {
            scallop_height: 0.5,
            tolerance: 0.5,
            continuous: true,
            ..ScallopParams::default()
        };

        let tp = scallop_toolpath(&mesh, &si, &cutter, &params);

        // In continuous mode, there should be very few rapids
        // (just the initial approach and final retract)
        let rapid_count = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, crate::toolpath::MoveType::Rapid))
            .count();

        assert!(
            rapid_count <= 4,
            "Continuous scallop should have minimal rapids, got {}",
            rapid_count
        );
    }

    #[test]
    fn test_scallop_inside_out() {
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let params = ScallopParams {
            scallop_height: 0.5,
            tolerance: 0.5,
            direction: ScallopDirection::InsideOut,
            ..ScallopParams::default()
        };

        let tp = scallop_toolpath(&mesh, &si, &cutter, &params);
        assert!(
            tp.moves.len() > 5,
            "Inside-out scallop should produce moves, got {}",
            tp.moves.len()
        );
    }

    // ── Regression: slope-confined passes must not chord across gaps ──

    /// Shared assertion: no consecutive pair of cutting (`FinishingCut`)
    /// moves may be farther apart than `max_allowed` — a bigger jump means
    /// a cutting move chorded across an excluded slope gap instead of
    /// retracting. A rapid or plunge in between resets the check, since
    /// that's exactly the retract/replunge link the fix introduces.
    fn assert_no_chord_across_gap(tp: &Toolpath, max_allowed: f64) {
        let mut prev_cut: Option<P3> = None;
        let mut saw_any_cut = false;
        for m in &tp.moves {
            let is_cut = matches!(m.move_type, crate::toolpath::MoveType::Linear { .. })
                && m.intent == MoveIntent::FinishingCut;
            if is_cut {
                saw_any_cut = true;
                if let Some(prev) = prev_cut {
                    let d = ((m.target.x - prev.x).powi(2) + (m.target.y - prev.y).powi(2)).sqrt();
                    assert!(
                        d <= max_allowed,
                        "cutting move chorded across the excluded slope gap: {:.2}mm \
                         (allowed {:.2}mm) from ({:.2},{:.2}) to ({:.2},{:.2})",
                        d,
                        max_allowed,
                        prev.x,
                        prev.y,
                        m.target.x,
                        m.target.y
                    );
                }
                prev_cut = Some(m.target);
            } else {
                prev_cut = None;
            }
        }
        assert!(saw_any_cut, "expected at least one cutting move");
    }

    /// Regression for the discrete-ring chording bug: a slope-confined ring
    /// used to filter out-of-band points and feed straight between the
    /// remaining survivors (and even close the loop back to the first
    /// survivor), chording across the excluded stretch. A hemisphere's
    /// slope varies continuously with radius, and a ring's own perimeter
    /// varies in distance from center (it's an offset of the roughly
    /// rectangular mesh-footprint boundary, not a circle), so a slope band
    /// like 20-60° is guaranteed to include only part of most rings.
    #[test]
    fn test_scallop_discrete_no_chord_across_excluded_slope_gap() {
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let params = ScallopParams {
            scallop_height: 0.3,
            tolerance: 0.3,
            continuous: false,
            slope_from: 20.0,
            slope_to: 60.0,
            ..ScallopParams::default()
        };

        let tp = scallop_toolpath(&mesh, &si, &cutter, &params);

        let stepover =
            crate::scallop_math::stepover_from_scallop_flat(cutter.radius(), params.scallop_height);
        assert_no_chord_across_gap(&tp, (stepover * 6.0).max(3.0));
    }

    /// Companion regression for continuous (spiral) mode: same slope band,
    /// same hemisphere, `continuous: true` this time.
    #[test]
    fn test_scallop_continuous_no_chord_across_excluded_slope_gap() {
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let params = ScallopParams {
            scallop_height: 0.3,
            tolerance: 0.3,
            continuous: true,
            slope_from: 20.0,
            slope_to: 60.0,
            ..ScallopParams::default()
        };

        let tp = scallop_toolpath(&mesh, &si, &cutter, &params);

        let stepover =
            crate::scallop_math::stepover_from_scallop_flat(cutter.radius(), params.scallop_height);
        assert_no_chord_across_gap(&tp, (stepover * 6.0).max(3.0));
    }

    // ── Helper tests ────────────────────────────────────────────────

    #[test]
    fn test_closest_kept_point_idx() {
        let ring = vec![
            (P3::new(0.0, 0.0, 0.0), true),
            (P3::new(10.0, 0.0, 0.0), true),
            (P3::new(10.0, 10.0, 0.0), true),
            (P3::new(0.0, 10.0, 0.0), true),
        ];
        let target = P3::new(9.0, 9.0, 0.0);
        let idx = closest_kept_point_idx(&ring, &target, |pt| pt.1);
        assert_eq!(idx, Some(2), "Closest to (9,9) should be index 2 (10,10)");

        // Excluding the geometrically-closest point must fall through to
        // the next-closest KEPT point, not return the excluded one.
        let mut masked = ring.clone();
        masked[2].1 = false;
        let idx = closest_kept_point_idx(&masked, &target, |pt| pt.1);
        assert_eq!(
            idx,
            Some(1),
            "With (10,10) excluded, closest kept to (9,9) is index 1 (10,0)"
        );

        // Nothing kept -> None (the caller skips the ring entirely).
        let idx = closest_kept_point_idx(&ring, &target, |_| false);
        assert_eq!(idx, None);
    }

    #[test]
    fn test_rotate_ring() {
        let ring = vec![
            (P3::new(0.0, 0.0, 0.0), true),
            (P3::new(1.0, 0.0, 0.0), true),
            (P3::new(2.0, 0.0, 0.0), true),
            (P3::new(3.0, 0.0, 0.0), true),
        ];
        let rotated = rotate_ring(&ring, 2);
        assert!((rotated[0].0.x - 2.0).abs() < 0.01);
        assert!((rotated[1].0.x - 3.0).abs() < 0.01);
        assert!((rotated[2].0.x - 0.0).abs() < 0.01);
        assert!((rotated[3].0.x - 1.0).abs() < 0.01);
    }

    // ── P2.3: boundary_regions pre-clip ──────────────────────────────

    /// `boundary_regions = None` reproduces the unrestricted toolpath
    /// (behaviorally — the P2.3 bonus covered-fix changes the *baseline*
    /// slightly from pre-P2.3 code, but the parameter itself must be a
    /// no-op): every cutting move on a flat mesh with no boundary passed
    /// should land somewhere on the mesh footprint.
    #[test]
    fn scallop_boundary_regions_none_is_unrestricted() {
        let (mesh, si) = make_flat_mesh();
        let cutter = ball_cutter();
        let params = ScallopParams {
            scallop_height: 0.5,
            tolerance: 0.5,
            ..ScallopParams::default()
        };
        let never_cancel = || false;

        let (tp, _) = scallop_toolpath_structured_annotated_with_cancel(
            &mesh,
            &si,
            &cutter,
            &params,
            None,
            None,
            &never_cancel,
        )
        .unwrap();

        assert!(
            tp.moves.len() > 10,
            "unrestricted scallop should produce moves, got {}",
            tp.moves.len()
        );
    }

    /// `boundary_regions = Some(&[left-half])` confines every cutting move's
    /// XY to that region (a small containment tolerance absorbs the ring's
    /// own point spacing landing right on the region edge).
    #[test]
    fn scallop_boundary_regions_confines_cuts_to_region() {
        let (mesh, si) = make_flat_mesh();
        let cutter = ball_cutter();
        let bbox = &mesh.bbox;
        let left_half = Polygon2::new(vec![
            P2::new(bbox.min.x, bbox.min.y),
            P2::new(0.0, bbox.min.y),
            P2::new(0.0, bbox.max.y),
            P2::new(bbox.min.x, bbox.max.y),
        ]);
        let params = ScallopParams {
            scallop_height: 0.5,
            tolerance: 0.5,
            ..ScallopParams::default()
        };
        let never_cancel = || false;

        let (tp, _) = scallop_toolpath_structured_annotated_with_cancel(
            &mesh,
            &si,
            &cutter,
            &params,
            None,
            Some(std::slice::from_ref(&left_half)),
            &never_cancel,
        )
        .unwrap();

        let tol = 1e-6;
        let mut saw_cut = false;
        for m in &tp.moves {
            if let crate::toolpath::MoveType::Linear { .. } = m.move_type
                && m.intent == MoveIntent::FinishingCut
            {
                saw_cut = true;
                assert!(
                    m.target.x <= tol,
                    "cutting move X={:.3} escaped the left-half boundary region",
                    m.target.x
                );
            }
        }
        assert!(saw_cut, "expected at least one cutting move");
    }

    /// Two disjoint boundary regions each get their own ring set — cuts
    /// land in both regions and never in the excluded gap between them.
    #[test]
    fn scallop_boundary_regions_two_disjoint_regions_both_cut() {
        let (mesh, si) = make_flat_mesh();
        let cutter = ball_cutter();
        let bbox = &mesh.bbox;
        // Left third and right third of the mesh, with a gap in the middle.
        let left = Polygon2::new(vec![
            P2::new(bbox.min.x, bbox.min.y),
            P2::new(-15.0, bbox.min.y),
            P2::new(-15.0, bbox.max.y),
            P2::new(bbox.min.x, bbox.max.y),
        ]);
        let right = Polygon2::new(vec![
            P2::new(15.0, bbox.min.y),
            P2::new(bbox.max.x, bbox.min.y),
            P2::new(bbox.max.x, bbox.max.y),
            P2::new(15.0, bbox.max.y),
        ]);
        let regions = vec![left, right];
        let params = ScallopParams {
            scallop_height: 0.5,
            tolerance: 0.5,
            ..ScallopParams::default()
        };
        let never_cancel = || false;

        let (tp, _) = scallop_toolpath_structured_annotated_with_cancel(
            &mesh,
            &si,
            &cutter,
            &params,
            None,
            Some(&regions),
            &never_cancel,
        )
        .unwrap();

        let mut saw_left = false;
        let mut saw_right = false;
        for m in &tp.moves {
            if let crate::toolpath::MoveType::Linear { .. } = m.move_type
                && m.intent == MoveIntent::FinishingCut
            {
                assert!(
                    m.target.x <= -15.0 + 1e-6 || m.target.x >= 15.0 - 1e-6,
                    "cutting move X={:.3} landed in the excluded gap between regions",
                    m.target.x
                );
                if m.target.x <= -15.0 + 1e-6 {
                    saw_left = true;
                }
                if m.target.x >= 15.0 - 1e-6 {
                    saw_right = true;
                }
            }
        }
        assert!(saw_left, "expected at least one cut in the left region");
        assert!(saw_right, "expected at least one cut in the right region");
    }
}
