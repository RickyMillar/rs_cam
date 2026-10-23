//! "Cut past open sides" (operator ruling 2026-10-23, opt-in; the
//! industry references are the FreeCAD Pocket "Extensions" and the Fusion
//! 2D Pocket stock outline): THE area rule of a Pocket on model faces.
//!
//! A Pocket of a model face (a STEP face, a STEP Z ring or an STL flat
//! face) cuts inside the face. At a side with no wall, the stock beyond the
//! side stays, and the tool radius leaves a fin of stock at each outer
//! corner. With the setting on, the area of each face grows out through
//! each side that has no wall, to the stock edge. A side against a wall
//! does not move.
//!
//! # The rule
//!
//! For a face at the plane Z `z`, in the setup frame:
//!
//! 1. The material above the face is the XY projection of each part of a
//!    mesh triangle that is higher than `z + CAD_ROUNDING_MM`.
//!    [`CAD_ROUNDING_MM`] (0.01 mm) is the length below which two lengths
//!    are one value; it is also the Z band of one STL flat face
//!    (`FLAT_REGION_Z_BAND_MM`, the same 0.01 mm), so a triangle of the
//!    face itself is never material above the face. Each triangle is
//!    clipped at that plane: only its part above the plane counts.
//! 2. The projection grows by `CAD_ROUNDING_MM / 2` on each side. That
//!    closes each gap narrower than `CAD_ROUNDING_MM` between two
//!    triangles (a seam of the tessellation is not a gap in the model), so
//!    the area cannot leak through a seam. It moves no side of the face:
//!    step 4 adds the face back.
//! 3. The allowed area is the stock rectangle grown by the tool radius
//!    (as the P6.4 open path extend grows it, so the tool centre reaches
//!    the stock edge and the tool clears it) less the material above.
//! 4. The area of the face is the part of the allowed area that holds the
//!    face (the part that overlaps the face most), added to the face, less
//!    the holes of the face. A hole of the face stays an island.
//!
//! The rule needs the floor "Each picked face" (`Floor::EachFace`): only
//! then does each area cut down to its own face (less the floor depth `d`),
//! so steps 1 to 4 read the material above `z - d`. Under any other floor
//! all areas cut to one floor, and a grown area of a high face could cut
//! into a lower step; so the rule does not run, and the areas stay the
//! faces. With `d > 0` the face itself is material above `z - d`, so no
//! area grows (the op then gives the note below).
//!
//! A face whose area grows by no more than its perimeter times
//! `CAD_ROUNDING_MM` (a band narrower than the CAD rounding along its
//! sides) has no open side: its area does not change. When no face of
//! the op has an open side, the op gives one [`NoteCause::NoOpenSide`]
//! note. That is not a refusal: the op cuts each face as before.
//!
//! # Faces at one Z
//!
//! Two faces at one Z (within `CAD_ROUNDING_MM`) whose areas are the same
//! part of the allowed area give ONE area: the area of the first face,
//! with the holes of both. So the op does not cut a face twice. Faces at
//! two Z values never merge: each area cuts down to its own face Z ("Each
//! picked face"), and the area of a higher face can lie over a lower face,
//! where it cuts only down to the higher Z.
//!
//! # The readers
//!
//! The prologue runs the rule after the view (`execute::prologue`), so the
//! per-face floors, the generation and the gouge guard read the grown
//! areas. `selection_status` runs the same rule on the same mesh and tool,
//! so the viewport highlight (`SelectionReport::extended`) shows the area
//! that the op cuts.

use std::collections::BTreeMap;

use crate::geo::{P2, P3};
use crate::mesh::TriangleMesh;
use crate::polygon::{Polygon2, difference_all, offset_polygon};
use crate::selection::{
    CAD_ROUNDING_MM, ItemRef, NoteCause, OpItems, Regions, ResolvedSelection, SelectionFrame,
    SelectionNote, StockXy,
};

/// The inputs of THE area rule: the model mesh in the frame of the
/// selection, the stock rectangle and the tool radius of the op.
pub struct OpenSidesProbe<'a> {
    pub mesh: &'a TriangleMesh,
    /// The stock rectangle of the selection as resolved
    /// (`ResolvedSelection::stock_xy`), NOT grown by the P6.4 extend: the
    /// rule grows it by the tool radius once.
    pub stock: StockXy,
    /// The tool radius (mm). The stock rectangle grows by it.
    pub tool_radius: f64,
    /// The depth below each face of the floor "Each picked face"
    /// (`Floor::EachFace { depth }`): each area cuts down to its face Z
    /// less this depth, so the rule reads the material above that Z. The
    /// caller gives no probe under any other floor (see the module doc).
    pub face_depth: f64,
}

/// One face area of the view: its index in [`Regions::as_slice`] and its
/// plane Z.
struct FaceArea {
    index: usize,
    z: f64,
}

/// THE area rule (see the module doc). It puts the grown area of each
/// face of `items` in place, merges the faces at one Z that share one
/// area, and returns the notes: one [`NoteCause::NoOpenSide`] note when the
/// view has a face and no face has an open side.
///
/// It does nothing for a view with no closed areas and for a view with no
/// face.
pub fn grow_past_open_sides<F: SelectionFrame>(
    resolved: &ResolvedSelection<F>,
    items: &mut OpItems,
    probe: &OpenSidesProbe<'_>,
) -> Vec<SelectionNote> {
    let Some(regions) = items.regions() else {
        return Vec::new();
    };
    let faces: Vec<FaceArea> = (0..regions.as_slice().len())
        .filter(|&index| !regions.is_stock(index))
        .filter_map(|index| regions.plane_z(index).map(|z| FaceArea { index, z }))
        .collect();
    if faces.is_empty() {
        return Vec::new();
    }
    let grown_stock = probe.stock.grown(probe.tool_radius);
    let stock_ring = Polygon2::rectangle(
        grown_stock.x_min,
        grown_stock.y_min,
        grown_stock.x_max,
        grown_stock.y_max,
    );
    let mut grown: Vec<(usize, Polygon2)> = Vec::new();
    let mut merged: Vec<usize> = Vec::new();
    for plane in planes(&faces) {
        let material = material_above(probe.mesh, plane.z - probe.face_depth + CAD_ROUNDING_MM);
        let allowed = difference_all(std::slice::from_ref(&stock_ring), &material);
        // For each part of the allowed area: the faces it holds, in index
        // order.
        let mut by_part: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for &index in &plane.members {
            let Some(face) = regions.as_slice().get(index) else {
                continue;
            };
            if let Some(part) = part_of(&allowed, face) {
                by_part.entry(part).or_default().push(index);
            }
        }
        for (part, members) in by_part {
            let Some(area) = allowed.get(part) else {
                continue;
            };
            let Some(polygon) = grown_area(regions, area, &members) else {
                continue;
            };
            let Some((&first, rest)) = members.split_first() else {
                continue;
            };
            grown.push((first, polygon));
            merged.extend_from_slice(rest);
        }
    }
    let notes = if grown.is_empty() {
        no_open_side_note(resolved, regions, &faces)
    } else {
        Vec::new()
    };
    merged.sort_unstable();
    items.grow_regions(grown, &merged);
    notes
}

/// The faces of one plane Z: the Z of the lowest of them, and their
/// indices.
struct Plane {
    z: f64,
    members: Vec<usize>,
}

/// The faces grouped by plane Z: two Z values within [`CAD_ROUNDING_MM`]
/// are one plane.
fn planes(faces: &[FaceArea]) -> Vec<Plane> {
    let mut order: Vec<&FaceArea> = faces.iter().collect();
    order.sort_by(|a, b| a.z.total_cmp(&b.z).then(a.index.cmp(&b.index)));
    let mut planes: Vec<Plane> = Vec::new();
    for face in order {
        match planes.last_mut() {
            Some(plane) if face.z - plane.z <= CAD_ROUNDING_MM => plane.members.push(face.index),
            _ => planes.push(Plane {
                z: face.z,
                members: vec![face.index],
            }),
        }
    }
    for plane in &mut planes {
        plane.members.sort_unstable();
    }
    planes
}

/// The XY projection of the parts of the mesh triangles above the plane
/// `z`, grown by `CAD_ROUNDING_MM / 2` (see the module doc, steps 1 and
/// 2).
fn material_above(mesh: &TriangleMesh, z: f64) -> Vec<Polygon2> {
    let rings: Vec<Polygon2> = mesh
        .triangles
        .iter()
        .filter_map(|tri| {
            let [a, b, c] = tri.map(|i| mesh.vertices.get(i as usize).copied());
            clipped_above(&[a?, b?, c?], z)
        })
        .collect();
    let union = Polygon2::union_all(&rings);
    let closed: Vec<Polygon2> = union
        .iter()
        .flat_map(|poly| {
            // `offset_polygon`: a negative distance grows the polygon.
            let out = offset_polygon(poly, -CAD_ROUNDING_MM / 2.0);
            if out.is_empty() {
                vec![poly.clone()]
            } else {
                out
            }
        })
        .collect();
    Polygon2::union_all(&closed)
}

/// The XY ring of the part of the triangle `tri` at or above the plane
/// `z` (a clip of the triangle by the plane), or `None` when no part is
/// above it or the part has no area in XY (a vertical triangle).
fn clipped_above(tri: &[P3; 3], z: f64) -> Option<Polygon2> {
    if tri.iter().all(|p| p.z < z) {
        return None;
    }
    let mut ring: Vec<P2> = Vec::with_capacity(4);
    for (k, p) in tri.iter().enumerate() {
        let q = tri.get((k + 1) % 3)?;
        if p.z >= z {
            ring.push(P2::new(p.x, p.y));
        }
        if (p.z >= z) != (q.z >= z) {
            let t = (z - p.z) / (q.z - p.z);
            ring.push(P2::new(p.x + (q.x - p.x) * t, p.y + (q.y - p.y) * t));
        }
    }
    if ring.len() < 3 {
        return None;
    }
    let mut poly = Polygon2::new(ring);
    // A projected area at the rounding error of the shoelace sum is a
    // vertical triangle: it has no area in XY.
    let [x0, y0, x1, y1] = poly.bbox();
    let extent = (x1 - x0).max(y1 - y0);
    if poly.area() <= f64::EPSILON * extent * extent {
        return None;
    }
    poly.ensure_winding();
    Some(poly)
}

/// The index of the part of `allowed` that overlaps `face` most, or `None`
/// when no part overlaps it (material stands over all of the face).
fn part_of(allowed: &[Polygon2], face: &Polygon2) -> Option<usize> {
    let outline = Polygon2::new(face.exterior.clone());
    allowed
        .iter()
        .enumerate()
        .map(|(i, part)| {
            let overlap: f64 = part.intersection(&outline).iter().map(Polygon2::area).sum();
            (i, overlap)
        })
        .filter(|(_, overlap)| *overlap > 0.0)
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// The grown area of the faces `members` (one plane, one part `area` of
/// the allowed area): the part with the faces added, less the holes of
/// the faces. `None` when no face has an open side (the area grows by no
/// more than a band of `CAD_ROUNDING_MM` along the sides), or when the
/// result is not one polygon (the area then stays the face).
fn grown_area(regions: &Regions, area: &Polygon2, members: &[usize]) -> Option<Polygon2> {
    let faces: Vec<&Polygon2> = members
        .iter()
        .filter_map(|&i| regions.as_slice().get(i))
        .collect();
    let outlines: Vec<Polygon2> = std::iter::once(area.clone())
        .chain(faces.iter().map(|f| Polygon2::new(f.exterior.clone())))
        .collect();
    let holes: Vec<Polygon2> = faces
        .iter()
        .flat_map(|f| f.holes.iter().map(|h| Polygon2::new(h.clone())))
        .collect();
    let filled = Polygon2::union_all(&outlines);
    let result = difference_all(&filled, &holes);
    let [polygon] = <[Polygon2; 1]>::try_from(result).ok()?;
    let face_area: f64 = faces.iter().map(|f| f.area()).sum();
    let perimeter: f64 = faces.iter().map(|f| ring_length(&f.exterior)).sum();
    (polygon.area() - face_area > perimeter * CAD_ROUNDING_MM).then_some(polygon)
}

/// The length of the closed ring `ring`.
fn ring_length(ring: &[P2]) -> f64 {
    ring.iter()
        .zip(ring.iter().cycle().skip(1))
        .map(|(a, b)| (b - a).norm())
        .sum()
}

/// The one [`NoteCause::NoOpenSide`] note of the faces of a view in which
/// no face has an open side.
fn no_open_side_note<F: SelectionFrame>(
    resolved: &ResolvedSelection<F>,
    regions: &Regions,
    faces: &[FaceArea],
) -> Vec<SelectionNote> {
    let first = faces
        .iter()
        .find_map(|f| regions.exterior_item(f.index))
        .and_then(|at| resolved.items().get(at));
    let Some(first) = first else {
        return Vec::new();
    };
    vec![SelectionNote {
        kind: first.item().kind(),
        layer: first.item().layer().map(str::to_owned),
        count: faces.len(),
        first: ItemRef::new(first.identity().clone()),
        item_use: crate::compute::catalog::ItemUse::Regions,
        cause: NoteCause::NoOpenSide,
    }]
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;

    #[test]
    fn a_triangle_clips_at_the_plane_opensides() {
        let tri = [
            P3::new(0.0, 0.0, 0.0),
            P3::new(10.0, 0.0, 10.0),
            P3::new(0.0, 10.0, 10.0),
        ];
        let part = clipped_above(&tri, 5.0).unwrap();
        // The part above Z 5: the strip between x + y = 5 and x + y = 10.
        assert!(
            (part.area() - (50.0 - 12.5)).abs() < 1e-9,
            "{}",
            part.area()
        );
        assert!(clipped_above(&tri, 10.5).is_none());
    }

    #[test]
    fn a_vertical_triangle_has_no_area_opensides() {
        let tri = [
            P3::new(0.0, 0.0, 0.0),
            P3::new(10.0, 0.0, 0.0),
            P3::new(5.0, 0.0, 10.0),
        ];
        assert!(clipped_above(&tri, 1.0).is_none());
    }

    /// Instrument: the time of the projection and the allowed area of one
    /// plane on the mesh at `RS_CAM_OPENSIDES_MESH` (an STL path), at the
    /// bottom, the middle and the top quarter of its Z range. Run:
    /// `RS_CAM_OPENSIDES_MESH=<stl> cargo test -p rs_cam_core --lib
    /// open_sides_time -- --ignored --nocapture`.
    #[test]
    #[ignore = "instrument: times the area rule on a large mesh named by an env var"]
    #[expect(clippy::print_stderr, reason = "an instrument prints its timings")]
    fn open_sides_time_opensides() {
        let path = std::env::var("RS_CAM_OPENSIDES_MESH").unwrap();
        let mesh = TriangleMesh::from_stl(std::path::Path::new(&path)).unwrap();
        let (lo, hi) = (mesh.bbox.min.z, mesh.bbox.max.z);
        let stock = Polygon2::rectangle(
            mesh.bbox.min.x - 3.0,
            mesh.bbox.min.y - 3.0,
            mesh.bbox.max.x + 3.0,
            mesh.bbox.max.y + 3.0,
        );
        for frac in [0.05, 0.5, 0.75] {
            let z = lo + (hi - lo) * frac;
            let start = std::time::Instant::now();
            let material = material_above(&mesh, z + CAD_ROUNDING_MM);
            let allowed = difference_all(std::slice::from_ref(&stock), &material);
            eprintln!(
                "{} triangles, z {z:.2}: {} material polygons, {} allowed parts, {:.3} s",
                mesh.triangles.len(),
                material.len(),
                allowed.len(),
                start.elapsed().as_secs_f64()
            );
        }
    }

    #[test]
    fn faces_within_the_rounding_are_one_plane_opensides() {
        let faces = [
            FaceArea { index: 2, z: 10.0 },
            FaceArea {
                index: 0,
                z: 10.0 + CAD_ROUNDING_MM / 2.0,
            },
            FaceArea { index: 1, z: 20.0 },
        ];
        let planes = planes(&faces);
        assert_eq!(planes.len(), 2);
        assert_eq!(planes[0].members, vec![0, 2]);
        assert_eq!(planes[1].members, vec![1]);
    }
}
