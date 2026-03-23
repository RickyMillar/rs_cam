//! Horizontal (flat-area) finishing operation.
//!
//! Detects flat (nearly horizontal) regions of a mesh and generates zigzag
//! raster toolpaths that machine only those areas. Useful for getting a clean
//! surface on plateaus, ledges, and pocket floors after a roughing or general
//! 3D finishing pass.

use crate::dropcutter::point_drop_cutter;
use crate::geo::{BoundingBox3, P3, V3};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::tool::MillingCutter;
use crate::toolpath::Toolpath;

/// Parameters for horizontal (flat-area) finishing.
pub struct HorizontalFinishParams {
    /// Maximum slope angle in degrees to consider a surface "flat".
    /// A perfectly horizontal face has slope 0; typical default is 5.0.
    pub angle_threshold: f64,
    /// Distance between adjacent raster lines (mm).
    pub stepover: f64,
    /// Feed rate for cutting moves (mm/min).
    pub feed_rate: f64,
    /// Feed rate for plunge moves (mm/min).
    pub plunge_rate: f64,
    /// Safe Z for rapid travel above the workpiece (mm).
    pub safe_z: f64,
    /// Extra material to leave on the surface (mm). Added to the drop-cutter Z.
    pub stock_to_leave: f64,
}

impl Default for HorizontalFinishParams {
    fn default() -> Self {
        Self {
            angle_threshold: 5.0,
            stepover: 1.0,
            feed_rate: 1000.0,
            plunge_rate: 300.0,
            safe_z: 10.0,
            stock_to_leave: 0.0,
        }
    }
}

/// A flat triangle with its averaged Z height and XY bounding box.
struct FlatTriInfo {
    avg_z: f64,
    bbox: BoundingBox3,
    face_index: usize,
}

/// A group of flat triangles at similar Z heights.
struct FlatRegion {
    tris: Vec<FlatTriInfo>,
    bbox: BoundingBox3,
    representative_z: f64,
}

/// Generate a toolpath that machines only the flat (horizontal) areas of a mesh.
///
/// Algorithm:
/// 1. Classify triangles as flat based on face normal vs angle threshold.
/// 2. Group flat triangles by similar Z height.
/// 3. For each region, raster across the XY bounding box, including only points
///    where the underlying triangle is flat.
/// 4. Insert rapids to skip non-flat stretches; retract between regions.
#[allow(clippy::indexing_slicing)] // mesh vertex/face indexing is bounded by mesh structure
pub fn horizontal_finish_toolpath(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &HorizontalFinishParams,
) -> Toolpath {
    let mut tp = Toolpath::new();

    let threshold_cos = params.angle_threshold.to_radians().cos();

    // ── Step 1: classify flat triangles ──────────────────────────────
    let mut flat_tris: Vec<FlatTriInfo> = Vec::new();

    for (fi, tri_indices) in mesh.triangles.iter().enumerate() {
        let v0 = mesh.vertices[tri_indices[0] as usize];
        let v1 = mesh.vertices[tri_indices[1] as usize];
        let v2 = mesh.vertices[tri_indices[2] as usize];

        let e1: V3 = v1 - v0;
        let e2: V3 = v2 - v0;
        let cross = e1.cross(&e2);
        let len = cross.norm();
        if len < 1e-15 {
            continue; // degenerate triangle
        }
        let normal = cross / len;

        // Flat if the Z component of the normal is close to +/-1.
        if normal.z.abs() > threshold_cos {
            let avg_z = (v0.z + v1.z + v2.z) / 3.0;
            let bbox = BoundingBox3::from_points([v0, v1, v2]);
            flat_tris.push(FlatTriInfo {
                avg_z,
                bbox,
                face_index: fi,
            });
        }
    }

    if flat_tris.is_empty() {
        return tp;
    }

    // ── Step 2-3: group flat triangles by similar Z ──────────────────
    // Sort by Z so we can sweep and cluster.
    flat_tris.sort_by(|a, b| {
        a.avg_z
            .partial_cmp(&b.avg_z)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let z_tolerance = params.stepover / 2.0;
    let mut regions: Vec<FlatRegion> = Vec::new();
    let mut region_start = 0;

    while region_start < flat_tris.len() {
        let base_z = flat_tris[region_start].avg_z;
        let mut region_end = region_start;

        // Extend the region while triangles are within tolerance of the base Z.
        while region_end < flat_tris.len()
            && (flat_tris[region_end].avg_z - base_z).abs() <= z_tolerance
        {
            region_end += 1;
        }

        // Drain this region's triangles out of the sorted vec.
        let tris: Vec<FlatTriInfo> = flat_tris.drain(region_start..region_end).collect();

        let mut region_bbox = BoundingBox3::empty();
        let mut z_sum = 0.0;
        for t in &tris {
            region_bbox.expand_to(t.bbox.min);
            region_bbox.expand_to(t.bbox.max);
            z_sum += t.avg_z;
        }
        let representative_z = if !tris.is_empty() {
            z_sum / tris.len() as f64
        } else {
            0.0
        };

        regions.push(FlatRegion {
            tris,
            bbox: region_bbox,
            representative_z,
        });

        // drain() shifted remaining items; restart from index 0.
        region_start = 0;
    }

    // ── Step 4-5: raster each region ─────────────────────────────────
    // Build a set of flat face indices for quick membership tests.
    let mut flat_face_set = vec![false; mesh.faces.len()];
    for region in &regions {
        for t in &region.tris {
            flat_face_set[t.face_index] = true;
        }
    }

    // Sort regions from highest Z to lowest (machine top shelves first to avoid
    // collisions, conventional for multi-level finishing).
    regions.sort_by(|a, b| {
        b.representative_z
            .partial_cmp(&a.representative_z)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let cutter_radius = cutter.radius();

    for region in &regions {
        let bbox = region.bbox.expand_by(cutter_radius);

        // Number of raster lines (Y direction) and sample points (X direction)
        let n_lines = ((bbox.max.y - bbox.min.y) / params.stepover).ceil() as usize + 1;
        let step_x = params.stepover;
        let n_cols = ((bbox.max.x - bbox.min.x) / step_x).ceil() as usize + 1;

        for line_idx in 0..n_lines {
            let y = bbox.min.y + line_idx as f64 * params.stepover;

            // Zigzag: alternate scan direction per line.
            let forward = line_idx % 2 == 0;

            // Collect feed-segments (contiguous runs of flat points).
            let mut segment: Vec<P3> = Vec::new();

            let col_range: Box<dyn Iterator<Item = usize>> = if forward {
                Box::new(0..n_cols)
            } else {
                Box::new((0..n_cols).rev())
            };

            for col_idx in col_range {
                let x = bbox.min.x + col_idx as f64 * step_x;

                // Drop cutter to find Z on the mesh surface.
                let cl = point_drop_cutter(x, y, mesh, index, cutter);
                if !cl.contacted {
                    // Off the mesh — flush any accumulated segment.
                    if !segment.is_empty() {
                        tp.emit_path_segment(
                            &segment,
                            params.safe_z,
                            params.feed_rate,
                            params.plunge_rate,
                        );
                        segment.clear();
                    }
                    continue;
                }

                // Check if the triangle(s) under this point are flat.
                // Query the spatial index for triangles near this point and check
                // if any flat triangle contains this XY coordinate.
                let is_flat =
                    is_point_over_flat_triangle(x, y, mesh, index, cutter_radius, &flat_face_set);

                if is_flat {
                    let z = cl.z + params.stock_to_leave;
                    segment.push(P3::new(x, y, z));
                } else {
                    // Not flat — flush segment, skip this point.
                    if !segment.is_empty() {
                        tp.emit_path_segment(
                            &segment,
                            params.safe_z,
                            params.feed_rate,
                            params.plunge_rate,
                        );
                        segment.clear();
                    }
                }
            }

            // Flush any remaining segment at end of line.
            if !segment.is_empty() {
                tp.emit_path_segment(
                    &segment,
                    params.safe_z,
                    params.feed_rate,
                    params.plunge_rate,
                );
            }
        }
    }

    // Final retract to safe Z.
    tp.final_retract(params.safe_z);

    tp
}

#[allow(clippy::indexing_slicing)] // tri_idx bounded by flat_face_set.len() check
/// Check whether the point (x, y) lies over at least one flat triangle.
///
/// Uses the spatial index to find nearby triangles, then tests XY containment
/// against those marked as flat in `flat_face_set`.
fn is_point_over_flat_triangle(
    x: f64,
    y: f64,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter_radius: f64,
    flat_face_set: &[bool],
) -> bool {
    let candidates = index.query(x, y, cutter_radius);
    for &tri_idx in &candidates {
        if tri_idx < flat_face_set.len()
            && flat_face_set[tri_idx]
            && mesh.faces[tri_idx].contains_point_xy(x, y)
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::mesh::{SpatialIndex, make_test_flat};
    use crate::tool::BallEndmill;

    #[test]
    fn test_horizontal_finish_flat_mesh() {
        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build_auto(&mesh);
        let cutter = BallEndmill::new(10.0, 25.0);

        let params = HorizontalFinishParams {
            angle_threshold: 5.0,
            stepover: 5.0,
            feed_rate: 1000.0,
            plunge_rate: 300.0,
            safe_z: 10.0,
            stock_to_leave: 0.0,
        };

        let tp = horizontal_finish_toolpath(&mesh, &index, &cutter, &params);

        assert!(
            !tp.moves.is_empty(),
            "Flat mesh should produce a non-empty toolpath"
        );

        // Verify we have both rapid and linear moves.
        let has_rapid = tp
            .moves
            .iter()
            .any(|m| m.move_type == crate::toolpath::MoveType::Rapid);
        let has_linear = tp
            .moves
            .iter()
            .any(|m| matches!(m.move_type, crate::toolpath::MoveType::Linear { .. }));
        assert!(has_rapid, "Toolpath should contain rapid moves");
        assert!(has_linear, "Toolpath should contain linear feed moves");
    }

    #[test]
    fn test_horizontal_finish_default_params() {
        let params = HorizontalFinishParams::default();
        assert!((params.angle_threshold - 5.0).abs() < 1e-10);
        assert!(params.stepover > 0.0);
    }

    #[test]
    fn test_horizontal_finish_no_flat_triangles() {
        // Build a steep V-shaped mesh: two triangles angled at 45 degrees.
        let vertices = vec![
            P3::new(0.0, 0.0, 0.0),
            P3::new(10.0, 0.0, 10.0),
            P3::new(0.0, 10.0, 0.0),
            P3::new(10.0, 10.0, 10.0),
        ];
        let triangles = vec![[0, 1, 2], [1, 3, 2]];
        let mesh = TriangleMesh::from_raw(vertices, triangles);
        let index = SpatialIndex::build_auto(&mesh);
        let cutter = BallEndmill::new(10.0, 25.0);

        let params = HorizontalFinishParams {
            angle_threshold: 5.0,
            stepover: 2.0,
            feed_rate: 1000.0,
            plunge_rate: 300.0,
            safe_z: 10.0,
            stock_to_leave: 0.0,
        };

        let tp = horizontal_finish_toolpath(&mesh, &index, &cutter, &params);

        assert!(
            tp.moves.is_empty(),
            "Steep mesh should produce an empty toolpath, got {} moves",
            tp.moves.len()
        );
    }

    #[test]
    fn test_horizontal_finish_stock_to_leave() {
        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build_auto(&mesh);
        let cutter = BallEndmill::new(10.0, 25.0);
        let stock_leave = 0.5;

        let params = HorizontalFinishParams {
            angle_threshold: 5.0,
            stepover: 5.0,
            feed_rate: 1000.0,
            plunge_rate: 300.0,
            safe_z: 10.0,
            stock_to_leave: stock_leave,
        };

        let tp = horizontal_finish_toolpath(&mesh, &index, &cutter, &params);
        assert!(!tp.moves.is_empty());

        // All linear feed moves that aren't plunges or retracts should have Z >= stock_to_leave.
        for m in &tp.moves {
            if let crate::toolpath::MoveType::Linear { .. } = m.move_type {
                assert!(
                    m.target.z >= stock_leave - 1e-6,
                    "Feed move Z={} should be >= stock_to_leave={}",
                    m.target.z,
                    stock_leave
                );
            }
        }
    }
}
