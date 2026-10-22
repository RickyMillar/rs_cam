//! The flat regions of a mesh as `Face` items (geometry selection P4.7).
//!
//! An STL has no BREP faces. This source groups the flat triangles of a
//! mesh by facing and plane Z, unions each group, and gives one `Face`
//! item for each merged polygon, with its own holes. A face pick, a face
//! rule and the face filter then work on an STL as on a STEP model.
//!
//! The method comes from the basis fork
//! (`refs/remotes/basis/feature-web-server:crates/rs_cam_server/src/face_region.rs`):
//! collect the flat triangles at one height and merge them with
//! [`Polygon2::union_all`], which also closes the T-junction seams that an
//! edge walk trips on. The fork merges the region under one click; this
//! source merges every region once, so the items have stable keys.
//!
//! The source is pure: it reads no cache and no session. The session reads
//! it through `maps::geom_cache::cached_flat_regions` (built once per mesh)
//! and `LoadedModel::face_items` (no build).

use crate::geo::{P2, V3};
use crate::geometry::enriched_mesh::HORIZONTAL_FACE_MIN_NORMAL_Z;
use crate::mesh::TriangleMesh;
use crate::polygon::Polygon2;

use super::item::{FaceItem, Item, ItemError, ItemSet, Shape, SourceRef};

/// The Z band of one flat region (mm): every vertex of a triangle of the
/// region is within this distance of the region Z. Lead ruling L-7.
///
/// This is not a new tolerance. Two sources in the repository give 0.01 mm:
///
/// - the basis fork face region, `HEIGHT_TOLERANCE_MM` in
///   `refs/remotes/basis/feature-web-server:crates/rs_cam_server/src/face_region.rs`
///   ("Triangles within this of the picked height are the same face");
/// - `CLOSE_TOL_MM` in `io/dxf_input.rs`, the distance at which two
///   drawing points are one point.
pub const FLAT_REGION_Z_BAND_MM: f64 = 0.01;

/// One flat triangle of the mesh.
struct FlatTriangle {
    /// The index of the triangle in `TriangleMesh::triangles`.
    index: usize,
    /// The mean Z of the three vertices.
    z: f64,
    /// True when the normal points to +Z.
    up: bool,
    /// The XY ring of the triangle, counter-clockwise.
    ring: Polygon2,
    /// The XY centroid of the triangle.
    centroid: P2,
    /// The lowest and the highest vertex Z.
    z_min: f64,
    z_max: f64,
}

/// The flat triangles of `mesh`: the triangles whose unit normal has
/// `|z| >= HORIZONTAL_FACE_MIN_NORMAL_Z` (the one flat rule of every face
/// source) and whose three vertices are within [`FLAT_REGION_Z_BAND_MM`]
/// of their mean Z. A triangle with no area, or with a bad vertex index,
/// is not flat.
fn flat_triangles(mesh: &TriangleMesh) -> Vec<FlatTriangle> {
    let mut flats = Vec::new();
    for (index, tri) in mesh.triangles.iter().enumerate() {
        let corner = |k: usize| {
            tri.get(k)
                .and_then(|&v| mesh.vertices.get(v as usize))
                .copied()
        };
        let (Some(a), Some(b), Some(c)) = (corner(0), corner(1), corner(2)) else {
            continue;
        };
        let normal = (b - a).cross(&(c - a));
        let length = normal.norm();
        if !(length > 0.0 && length.is_finite()) {
            continue;
        }
        let normal_z = normal.z / length;
        if normal_z.abs() < HORIZONTAL_FACE_MIN_NORMAL_Z {
            continue;
        }
        let z = (a.z + b.z + c.z) / 3.0;
        let z_min = a.z.min(b.z).min(c.z);
        let z_max = a.z.max(b.z).max(c.z);
        if z_max - z > FLAT_REGION_Z_BAND_MM || z - z_min > FLAT_REGION_Z_BAND_MM {
            continue;
        }
        // The XY winding follows the sign of the normal Z: an up triangle
        // is counter-clockwise in XY. The union wants counter-clockwise
        // exteriors.
        let mut pts = vec![P2::new(a.x, a.y), P2::new(b.x, b.y), P2::new(c.x, c.y)];
        let up = normal_z > 0.0;
        if !up {
            pts.reverse();
        }
        flats.push(FlatTriangle {
            index,
            z,
            up,
            ring: Polygon2::new(pts),
            centroid: P2::new((a.x + b.x + c.x) / 3.0, (a.y + b.y + c.y) / 3.0),
            z_min,
            z_max,
        });
    }
    flats
}

/// One group of flat triangles: one facing, one plane Z.
struct Group {
    z: f64,
    up: bool,
    /// The members, in triangle index order.
    members: Vec<usize>,
}

/// Group the flat triangles by facing and by plane Z. The triangles go in
/// order of mean Z (then triangle index); a triangle joins the open group
/// of its facing when every vertex is within the band of the group Z, else
/// it opens a new group at its own mean Z. So the group Z is the mean Z of
/// its lowest triangle, and the result does not depend on the input order
/// of two triangles at one Z.
fn group_by_plane(flats: &[FlatTriangle]) -> Vec<Group> {
    let mut order: Vec<usize> = (0..flats.len()).collect();
    order.sort_by(|&i, &j| match (flats.get(i), flats.get(j)) {
        (Some(a), Some(b)) => a.z.total_cmp(&b.z).then(a.index.cmp(&b.index)),
        _ => i.cmp(&j),
    });
    let mut groups: Vec<Group> = Vec::new();
    // The open group of each facing.
    let mut open_up: Option<usize> = None;
    let mut open_down: Option<usize> = None;
    for at in order {
        let Some(tri) = flats.get(at) else {
            continue;
        };
        let open = if tri.up { &mut open_up } else { &mut open_down };
        let joins = open.and_then(|g| groups.get(g)).is_some_and(|g| {
            tri.z_max - g.z <= FLAT_REGION_Z_BAND_MM && g.z - tri.z_min <= FLAT_REGION_Z_BAND_MM
        });
        if joins && let Some(group) = open.and_then(|g| groups.get_mut(g)) {
            group.members.push(at);
            continue;
        }
        *open = Some(groups.len());
        groups.push(Group {
            z: tri.z,
            up: tri.up,
            members: vec![at],
        });
    }
    for group in &mut groups {
        group
            .members
            .sort_by_key(|&at| flats.get(at).map_or(usize::MAX, |t| t.index));
    }
    groups
}

/// The flat regions of `mesh` as `Face` items, bound to
/// [`SourceRef::ToolpathModel`] (the session binds them to the model).
///
/// - One item per merged polygon of each group (one facing, one plane Z),
///   with the holes of the union as its own inner loops.
/// - `plane_z` is the group Z; the normal is `+Z` or `-Z`; `brep` is
///   `None`.
/// - Order: by plane Z, then by the lowest triangle index in the region.
///   The order and the geometry come from the mesh alone, so two loads of
///   one file give equal keys.
///
/// `Err` is a merged polygon that breaks a face invariant: a defect of the
/// union, not of the mesh.
pub fn flat_region_items(mesh: &TriangleMesh) -> Result<ItemSet, ItemError> {
    let flats = flat_triangles(mesh);
    let mut faces: Vec<(f64, usize, FaceItem)> = Vec::new();
    for group in group_by_plane(&flats) {
        let rings: Vec<Polygon2> = group
            .members
            .iter()
            .filter_map(|&at| flats.get(at).map(|t| t.ring.clone()))
            .collect();
        let merged = Polygon2::union_all(&rings);
        // The owner of each merged polygon: the lowest triangle index whose
        // centroid it holds. The members are in index order, so the first
        // hit is the lowest.
        let mut owner: Vec<Option<usize>> = vec![None; merged.len()];
        for &at in &group.members {
            if owner.iter().all(Option::is_some) {
                break;
            }
            let Some(tri) = flats.get(at) else {
                continue;
            };
            if let Some(slot) = merged
                .iter()
                .zip(owner.iter_mut())
                .find(|(poly, slot)| slot.is_none() && poly.contains_point(&tri.centroid))
                .map(|(_, slot)| slot)
            {
                *slot = Some(tri.index);
            }
        }
        let normal = V3::new(0.0, 0.0, if group.up { 1.0 } else { -1.0 });
        for (mut poly, first) in merged.into_iter().zip(owner) {
            if poly.exterior.len() < 3 {
                continue;
            }
            poly.closed = true;
            faces.push((
                group.z,
                first.unwrap_or(usize::MAX),
                FaceItem {
                    rings: poly,
                    plane_z: group.z,
                    normal,
                    brep: None,
                },
            ));
        }
    }
    faces.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let items = faces
        .into_iter()
        .map(|(_, _, face)| Item::new(SourceRef::ToolpathModel, None, Shape::Face(face)))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ItemSet::new(items))
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;
    use crate::geo::P3;

    /// Two up triangles at Z 0 and Z 0.005 (inside the band) and one at
    /// Z 0.02 (outside it).
    fn mesh(z: [f64; 3]) -> TriangleMesh {
        let mut vertices = Vec::new();
        let mut triangles = Vec::new();
        for (k, z) in z.iter().enumerate() {
            let x = 10.0 * k as f64;
            let i = u32::try_from(vertices.len()).unwrap();
            vertices.extend([
                P3::new(x, 0.0, *z),
                P3::new(x + 5.0, 0.0, *z),
                P3::new(x, 5.0, *z),
            ]);
            triangles.push([i, i + 1, i + 2]);
        }
        TriangleMesh::from_raw(vertices, triangles)
    }

    #[test]
    fn the_band_splits_the_planes() {
        let set = flat_region_items(&mesh([0.0, 0.005, 0.02])).unwrap();
        let z: Vec<f64> = set
            .iter()
            .map(|i| match i.shape() {
                Shape::Face(f) => f.plane_z,
                _ => f64::NAN,
            })
            .collect();
        // One group at Z 0 holds two disjoint triangles; Z 0.02 is its own.
        assert_eq!(z, vec![0.0, 0.0, 0.02]);
    }

    #[test]
    fn a_down_face_points_down() {
        let mut m = mesh([1.0, 5.0, 9.0]);
        for tri in &mut m.triangles {
            tri.swap(1, 2);
        }
        let set = flat_region_items(&m).unwrap();
        assert_eq!(set.len(), 3);
        for item in set.iter() {
            let Shape::Face(face) = item.shape() else {
                panic!("a face item");
            };
            assert!(face.normal.z < 0.0);
            assert!(face.rings.signed_area() > 0.0, "the exterior is CCW");
        }
    }
}
