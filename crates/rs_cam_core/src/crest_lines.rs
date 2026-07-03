//! Curvature-based valley crest-line extraction for pencil finishing.
//!
//! The relief meshes we finish are dense, noisy heightfields. The mesh-dihedral
//! crease detector fires on every triangulation crease and fragments; the
//! drainage detector traces hydrology, not the concave seams a pencil tool
//! cleans. This module implements the *faithful* CAM/graphics method —
//! ridge/valley line extraction from surface curvature:
//!
//! - **Rusinkiewicz 2004** ("Estimating Curvatures and Their Derivatives on
//!   Triangle Meshes", 3DPVT): per-vertex second fundamental form (principal
//!   curvatures κ1 ≥ κ2 with directions t1, t2) and the derivative-of-curvature
//!   tensor, both estimated per-face and area/angle-weighted to vertices.
//! - **Ohtake, Belyaev & Seidel 2004** ("Ridge-Valley Lines on Meshes via
//!   Implicit Surface Fitting", TOG 23(3)) and **Yoshizawa, Belyaev & Seidel
//!   2005** ("Fast and Robust Detection of Crest Lines on Meshes", SPM): a crest
//!   line is the zero set of the *extremality* e = ∂κ/∂t (the directional
//!   derivative of a principal curvature along its own direction). A **valley**
//!   is the zero set of the minimal-curvature extremality e₂ = ∂κ₂/∂t₂ where
//!   κ₂ < 0 (concave) and the concave direction dominates.
//!
//! Pipeline:
//! 1. Mixed Voronoi corner areas (Meyer et al. 2003) for weighting.
//! 2. Per-vertex curvature tensor (Rusinkiewicz), normals oriented +Z (the
//!    relief is a heightfield approached from above, so this fixes a stable
//!    sign convention independent of STL winding: a valley reads κ₂ < 0).
//! 3. **Curvature-tensor smoothing** (the literature denoise — we smooth the
//!    tensor field, never the geometry, so genuine valleys keep their depth
//!    while triangulation noise averages out).
//! 4. Diagonalise → signed (κ₁ ≥ κ₂) with directions; derivative-of-curvature
//!    tensor → extremality e₂ = ∂κ₂/∂t₂ per vertex.
//! 5. March each triangle: orient t₂ consistently across its three vertices,
//!    find the two edges where e₂ changes sign, interpolate the crossing points
//!    into a segment, keep it only where it is concave (κ₂ < 0) and the concave
//!    direction dominates (|κ₂| ≥ |κ₁|) and sharp enough (|κ₂| ≥ `valley_saliency`).
//! 6. Chain coincident edge-crossings into coherent valley centreline polylines.
//!
//! `valley_saliency` is THE significance dial: it is the minimum |κ₂| (curvature,
//! units 1/mm) a valley must reach to be traced. Low → every concave seam; high
//! → only the deep sharp valleys. A flat basin (a lake) has κ₂ ≈ 0 and drops out
//! at any positive threshold.
//!
//! Output: `Vec<Vec<P3>>` valley centrelines (world XY at edge crossings, Z at
//! the interpolated surface) feeding the existing pencil fair → drop-cutter lift
//! → offset → link → emit pipeline unchanged. The crest line already sits at the
//! valley floor, so no bisector positioning is needed.

use std::collections::HashMap;

use nalgebra::{Matrix3, Matrix4, Vector3 as NaVector3, Vector4};
use tracing::info;

use crate::geo::{P3, V3};
use crate::mesh::TriangleMesh;

/// Tunables for curvature-based valley detection.
#[derive(Debug, Clone)]
pub struct CrestParams {
    /// Minimum concave curvature |κ₂| (1/mm) a valley must reach to be traced —
    /// THE significance dial. `0.05` keeps valleys sharper than a ~20 mm radius;
    /// raise toward the tool-radius reciprocal to keep only deep sharp grooves,
    /// lower toward zero to trace every concave seam.
    pub valley_saliency: f64,
    /// Curvature-tensor smoothing iterations (1-ring tensor diffusion). Suppresses
    /// triangulation/scan noise the literature way — by smoothing the curvature
    /// field, not the mesh. More iterations = smoother, fewer spurious fragments,
    /// at the cost of blurring nearby valleys together.
    pub smoothing_iters: usize,
    /// Minimum traced polyline length (mm); shorter chains are dropped as noise.
    pub min_line_length: f64,
}

impl Default for CrestParams {
    fn default() -> Self {
        Self {
            valley_saliency: 0.05,
            smoothing_iters: 3,
            min_line_length: 2.0,
        }
    }
}

/// An undirected mesh edge identified by sorted vertex indices — the identity of
/// a crest-line crossing point (a crossing always lies on a mesh edge, and the
/// two triangles sharing that edge compute the identical crossing, so the edge
/// key chains them).
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
struct EdgeKey(u32, u32);

impl EdgeKey {
    fn new(a: u32, b: u32) -> Self {
        if a <= b { Self(a, b) } else { Self(b, a) }
    }
}

/// Per-vertex curvature field after diagonalisation, holding exactly what the
/// valley march reads: the minimal principal direction t₂ (for sign-consistent
/// orientation across a triangle), both signed principal curvatures (κ₁ ≥ κ₂,
/// for the concave-dominance test), and the minimal-curvature extremality.
struct Curvature {
    /// Minimal principal direction (t₂) per vertex.
    pdir2: Vec<V3>,
    /// Maximal principal curvature κ₁ (signed, κ₁ ≥ κ₂).
    k1: Vec<f64>,
    /// Minimal principal curvature κ₂ (signed) — most negative in concavities.
    k2: Vec<f64>,
    /// Minimal-curvature extremality e₂ = ∂κ₂/∂t₂ (gauge tied to `pdir2`'s sign).
    emin: Vec<f64>,
}

/// Rotate the coordinate system (`old_u`, `old_v`) so its implied normal aligns
/// with `new_norm`, keeping the frame as close to the original as possible
/// (Rusinkiewicz 2004, `rot_coord_sys`).
fn rot_coord_sys(old_u: V3, old_v: V3, new_norm: V3) -> (V3, V3) {
    let old_norm = old_u.cross(&old_v);
    let ndot = old_norm.dot(&new_norm);
    if ndot <= -1.0 {
        return (-old_u, -old_v);
    }
    let perp_old = new_norm - ndot * old_norm;
    let dperp = (old_norm + new_norm) / (1.0 + ndot);
    (
        old_u - dperp * old_u.dot(&perp_old),
        old_v - dperp * old_v.dot(&perp_old),
    )
}

/// Re-express a curvature tensor (`ku`, `kuv`, `kv`) given in the (`old_u`,
/// `old_v`) frame into the (`new_u`, `new_v`) frame (Rusinkiewicz `proj_curv`).
fn proj_curv(
    old_u: V3,
    old_v: V3,
    ku: f64,
    kuv: f64,
    kv: f64,
    new_u: V3,
    new_v: V3,
) -> (f64, f64, f64) {
    let (r_new_u, r_new_v) = rot_coord_sys(new_u, new_v, old_u.cross(&old_v));
    let u1 = r_new_u.dot(&old_u);
    let v1 = r_new_u.dot(&old_v);
    let u2 = r_new_v.dot(&old_u);
    let v2 = r_new_v.dot(&old_v);
    (
        ku * u1 * u1 + kuv * 2.0 * u1 * v1 + kv * v1 * v1,
        ku * u1 * u2 + kuv * (u1 * v2 + u2 * v1) + kv * v1 * v2,
        ku * u2 * u2 + kuv * 2.0 * u2 * v2 + kv * v2 * v2,
    )
}

/// Re-express a derivative-of-curvature cubic form (4 coefficients) from the
/// (`old_u`, `old_v`) frame into (`new_u`, `new_v`) (Rusinkiewicz `proj_dcurv`).
fn proj_dcurv(old_u: V3, old_v: V3, d: [f64; 4], new_u: V3, new_v: V3) -> [f64; 4] {
    let (r_new_u, r_new_v) = rot_coord_sys(new_u, new_v, old_u.cross(&old_v));
    let u1 = r_new_u.dot(&old_u);
    let v1 = r_new_u.dot(&old_v);
    let u2 = r_new_v.dot(&old_u);
    let v2 = r_new_v.dot(&old_v);
    [
        d[0] * u1 * u1 * u1
            + 3.0 * d[1] * u1 * u1 * v1
            + 3.0 * d[2] * u1 * v1 * v1
            + d[3] * v1 * v1 * v1,
        d[0] * u1 * u1 * u2
            + d[1] * (u1 * u1 * v2 + 2.0 * u2 * u1 * v1)
            + d[2] * (u2 * v1 * v1 + 2.0 * u1 * v1 * v2)
            + d[3] * v1 * v1 * v2,
        d[0] * u1 * u2 * u2
            + d[1] * (u2 * u2 * v1 + 2.0 * u1 * u2 * v2)
            + d[2] * (u1 * v2 * v2 + 2.0 * u2 * v2 * v1)
            + d[3] * v1 * v2 * v2,
        d[0] * u2 * u2 * u2
            + 3.0 * d[1] * u2 * u2 * v2
            + 3.0 * d[2] * u2 * v2 * v2
            + d[3] * v2 * v2 * v2,
    ]
}

/// Diagonalise the 2×2 curvature tensor (`ku`, `kuv`, `kv`) in frame (`old_u`,
/// `old_v`), returning *signed* `(kmax, kmin, dir_max, dir_min)` with `kmax ≥
/// kmin`. Jacobi rotation after re-aligning the frame to `new_norm`
/// (Rusinkiewicz `diagonalize_curv`, but kept signed rather than by magnitude so
/// callers can pick the concave minimum directly).
fn diagonalize(
    old_u: V3,
    old_v: V3,
    ku: f64,
    kuv: f64,
    kv: f64,
    new_norm: V3,
) -> (f64, f64, V3, V3) {
    let (ru, rv) = rot_coord_sys(old_u, old_v, new_norm);
    let (mut c, mut s, mut tt) = (1.0_f64, 0.0_f64, 0.0_f64);
    if kuv.abs() > 1e-20 {
        let h = 0.5 * (kv - ku) / kuv;
        tt = if h < 0.0 {
            1.0 / (h - (1.0 + h * h).sqrt())
        } else {
            1.0 / (h + (1.0 + h * h).sqrt())
        };
        c = 1.0 / (1.0 + tt * tt).sqrt();
        s = tt * c;
    }
    // Eigenvalue/eigenvector pairs of [[ku,kuv],[kuv,kv]] in the (ru,rv) basis.
    let ka = ku - tt * kuv;
    let kb = kv + tt * kuv;
    let da = c * ru - s * rv;
    let db = s * ru + c * rv;
    if ka >= kb {
        (ka, kb, da, db)
    } else {
        (kb, ka, db, da)
    }
}

/// Mixed Voronoi corner areas (Meyer, Desbrun, Schröder & Barr 2003). Returns
/// `(pointareas, cornerareas)`: the per-vertex mixed area and the per-face
/// per-corner area used as the vertex-accumulation weight.
#[allow(clippy::indexing_slicing)] // tri vertex indices validated on mesh load
fn point_areas(mesh: &TriangleMesh) -> (Vec<f64>, Vec<[f64; 3]>) {
    let nv = mesh.vertices.len();
    let mut pointareas = vec![0.0_f64; nv];
    let mut cornerareas = vec![[0.0_f64; 3]; mesh.triangles.len()];

    for (fi, tri) in mesh.triangles.iter().enumerate() {
        let p0 = mesh.vertices[tri[0] as usize];
        let p1 = mesh.vertices[tri[1] as usize];
        let p2 = mesh.vertices[tri[2] as usize];
        let e = [p2 - p1, p0 - p2, p1 - p0];
        let area = 0.5 * e[0].cross(&e[1]).norm();
        if area < 1e-18 {
            continue;
        }
        let l2 = [
            e[0].norm_squared(),
            e[1].norm_squared(),
            e[2].norm_squared(),
        ];
        // Barycentric weights for the (obtuse-aware) mixed area.
        let ew = [
            l2[0] * (l2[1] + l2[2] - l2[0]),
            l2[1] * (l2[2] + l2[0] - l2[1]),
            l2[2] * (l2[0] + l2[1] - l2[2]),
        ];
        let ca = &mut cornerareas[fi];
        if ew[0] <= 0.0 {
            ca[1] = -0.25 * l2[2] * area / e[0].dot(&e[2]);
            ca[2] = -0.25 * l2[1] * area / e[0].dot(&e[1]);
            ca[0] = area - ca[1] - ca[2];
        } else if ew[1] <= 0.0 {
            ca[2] = -0.25 * l2[0] * area / e[1].dot(&e[0]);
            ca[0] = -0.25 * l2[2] * area / e[1].dot(&e[2]);
            ca[1] = area - ca[2] - ca[0];
        } else if ew[2] <= 0.0 {
            ca[0] = -0.25 * l2[1] * area / e[2].dot(&e[1]);
            ca[1] = -0.25 * l2[0] * area / e[2].dot(&e[0]);
            ca[2] = area - ca[0] - ca[1];
        } else {
            let scale = 0.5 * area / (ew[0] + ew[1] + ew[2]);
            for j in 0..3 {
                ca[j] = scale * (ew[(j + 1) % 3] + ew[(j + 2) % 3]);
            }
        }
        pointareas[tri[0] as usize] += ca[0];
        pointareas[tri[1] as usize] += ca[1];
        pointareas[tri[2] as usize] += ca[2];
    }
    (pointareas, cornerareas)
}

/// Area-weighted per-vertex normals, each oriented to point +Z (the relief is a
/// heightfield cut from above). Orienting up fixes a stable curvature sign so a
/// concave valley reliably reads κ₂ < 0 regardless of STL winding.
#[allow(clippy::indexing_slicing)] // tri vertex indices validated on mesh load
fn vertex_normals(mesh: &TriangleMesh) -> Vec<V3> {
    let mut normals = vec![V3::zeros(); mesh.vertices.len()];
    for (tri, face) in mesh.triangles.iter().zip(mesh.faces.iter()) {
        let p0 = mesh.vertices[tri[0] as usize];
        let p1 = mesh.vertices[tri[1] as usize];
        let p2 = mesh.vertices[tri[2] as usize];
        // Weight by face area (cross-product magnitude) and orient +Z.
        let mut fn_area = (p1 - p0).cross(&(p2 - p0));
        if fn_area.dot(&face.normal) < 0.0 {
            // keep handedness consistent with the stored face normal
            fn_area = -fn_area;
        }
        if fn_area.z < 0.0 {
            fn_area = -fn_area;
        }
        for &v in tri {
            normals[v as usize] += fn_area;
        }
    }
    for n in &mut normals {
        let len = n.norm();
        *n = if len > 1e-15 {
            let nn = *n / len;
            if nn.z < 0.0 { -nn } else { nn }
        } else {
            V3::new(0.0, 0.0, 1.0)
        };
    }
    normals
}

/// Solve the symmetric 3×3 normal-equation system `w·x = m` (curvature fit).
fn solve3(w: [[f64; 3]; 3], m: [f64; 3]) -> Option<[f64; 3]> {
    let mat = Matrix3::new(
        w[0][0], w[0][1], w[0][2], w[1][0], w[1][1], w[1][2], w[2][0], w[2][1], w[2][2],
    );
    let rhs = NaVector3::new(m[0], m[1], m[2]);
    mat.lu().solve(&rhs).map(|x| [x.x, x.y, x.z])
}

/// Solve the symmetric 4×4 normal-equation system `w·x = m` (dcurv fit).
fn solve4(w: [[f64; 4]; 4], m: [f64; 4]) -> Option<[f64; 4]> {
    #[allow(clippy::indexing_slicing)] // fixed 4×4 literal indices
    let mat = Matrix4::from_fn(|i, j| w[i][j]);
    let rhs = Vector4::new(m[0], m[1], m[2], m[3]);
    mat.lu().solve(&rhs).map(|x| [x.x, x.y, x.z, x.w])
}

/// Estimate the per-vertex principal-curvature tensor (Rusinkiewicz 2004),
/// smooth the tensor field, diagonalise it, and compute the minimal-curvature
/// extremality e₂ = ∂κ₂/∂t₂ from the derivative-of-curvature tensor.
#[allow(clippy::indexing_slicing)] // all indices are mesh vertex/face indices or fixed 0..3
fn compute_curvature(mesh: &TriangleMesh, adj: &[Vec<u32>], smoothing_iters: usize) -> Curvature {
    let nv = mesh.vertices.len();
    let normals = vertex_normals(mesh);
    let (pointareas, cornerareas) = point_areas(mesh);

    // Initial per-vertex tangent frame: an arbitrary edge projected into the
    // tangent plane (Rusinkiewicz initialisation).
    let mut pdir1 = vec![V3::zeros(); nv];
    for tri in &mesh.triangles {
        pdir1[tri[0] as usize] = mesh.vertices[tri[1] as usize] - mesh.vertices[tri[0] as usize];
        pdir1[tri[1] as usize] = mesh.vertices[tri[2] as usize] - mesh.vertices[tri[1] as usize];
        pdir1[tri[2] as usize] = mesh.vertices[tri[0] as usize] - mesh.vertices[tri[2] as usize];
    }
    let mut pdir2 = vec![V3::zeros(); nv];
    for v in 0..nv {
        let n = normals[v];
        let p1 = pdir1[v].cross(&n);
        let len = p1.norm();
        let p1 = if len > 1e-12 {
            p1 / len
        } else {
            // degenerate; pick any tangent
            let a = if n.x.abs() < 0.9 {
                V3::new(1.0, 0.0, 0.0)
            } else {
                V3::new(0.0, 1.0, 0.0)
            };
            (a - n * a.dot(&n)).normalize()
        };
        pdir1[v] = p1;
        pdir2[v] = n.cross(&p1);
    }

    // Accumulate the curvature tensor (ku, kuv, kv) in each vertex's frame.
    let mut cu = vec![0.0_f64; nv];
    let mut cuv = vec![0.0_f64; nv];
    let mut cv = vec![0.0_f64; nv];
    for (fi, tri) in mesh.triangles.iter().enumerate() {
        let p0 = mesh.vertices[tri[0] as usize];
        let p1 = mesh.vertices[tri[1] as usize];
        let p2 = mesh.vertices[tri[2] as usize];
        let e = [p2 - p1, p0 - p2, p1 - p0];
        let t = {
            let l = e[0].norm();
            if l < 1e-12 {
                continue;
            }
            e[0] / l
        };
        let n = e[0].cross(&e[1]);
        let b = {
            let bb = n.cross(&t);
            let l = bb.norm();
            if l < 1e-12 {
                continue;
            }
            bb / l
        };
        // Fit II from the variation of vertex normals along the three edges.
        let mut w = [[0.0_f64; 3]; 3];
        let mut m = [0.0_f64; 3];
        for j in 0..3 {
            let u = e[j].dot(&t);
            let vv = e[j].dot(&b);
            w[0][0] += u * u;
            w[0][1] += u * vv;
            w[2][2] += vv * vv;
            let dn = normals[tri[(j + 2) % 3] as usize] - normals[tri[(j + 1) % 3] as usize];
            let dnu = dn.dot(&t);
            let dnv = dn.dot(&b);
            m[0] += dnu * u;
            m[1] += dnu * vv + dnv * u;
            m[2] += dnv * vv;
        }
        w[1][1] = w[0][0] + w[2][2];
        w[1][2] = w[0][1];
        w[1][0] = w[0][1];
        w[2][1] = w[1][2];
        let Some(face_ii) = solve3(w, m) else {
            continue;
        };
        // Push the face tensor out to each vertex's frame, weighted by the
        // corner's share of the vertex's mixed area.
        for j in 0..3 {
            let vj = tri[j] as usize;
            let pa = pointareas[vj];
            if pa < 1e-18 {
                continue;
            }
            let (ku, kuv, kv) = proj_curv(
                t, b, face_ii[0], face_ii[1], face_ii[2], pdir1[vj], pdir2[vj],
            );
            let wt = cornerareas[fi][j] / pa;
            cu[vj] += wt * ku;
            cuv[vj] += wt * kuv;
            cv[vj] += wt * kv;
        }
    }

    // --- Curvature-tensor smoothing (the literature denoise). Diffuse the
    // tensor over the 1-ring, rotating each neighbour's tensor into the centre's
    // frame first, weighted by the neighbour's mixed area. Geometry untouched.
    for _ in 0..smoothing_iters {
        let (scu, scuv, scv) = (cu.clone(), cuv.clone(), cv.clone());
        for v in 0..nv {
            let nbrs = &adj[v];
            if nbrs.is_empty() {
                continue;
            }
            let mut aku = scu[v] * pointareas[v];
            let mut akuv = scuv[v] * pointareas[v];
            let mut akv = scv[v] * pointareas[v];
            let mut wsum = pointareas[v];
            for &nb in nbrs {
                let nb = nb as usize;
                let (ku, kuv, kv) = proj_curv(
                    pdir1[nb], pdir2[nb], scu[nb], scuv[nb], scv[nb], pdir1[v], pdir2[v],
                );
                let wt = pointareas[nb];
                aku += ku * wt;
                akuv += kuv * wt;
                akv += kv * wt;
                wsum += wt;
            }
            if wsum > 1e-18 {
                cu[v] = aku / wsum;
                cuv[v] = akuv / wsum;
                cv[v] = akv / wsum;
            }
        }
    }

    // Diagonalise → signed principal curvatures and directions per vertex.
    let mut k1 = vec![0.0_f64; nv];
    let mut k2 = vec![0.0_f64; nv];
    for v in 0..nv {
        let (kmax, kmin, dmax, dmin) =
            diagonalize(pdir1[v], pdir2[v], cu[v], cuv[v], cv[v], normals[v]);
        k1[v] = kmax;
        k2[v] = kmin;
        pdir1[v] = dmax;
        pdir2[v] = dmin;
    }

    // --- Derivative-of-curvature tensor (Rusinkiewicz need_dcurv) → extremality.
    let mut dcurv = vec![[0.0_f64; 4]; nv];
    for (fi, tri) in mesh.triangles.iter().enumerate() {
        let p0 = mesh.vertices[tri[0] as usize];
        let p1 = mesh.vertices[tri[1] as usize];
        let p2 = mesh.vertices[tri[2] as usize];
        let e = [p2 - p1, p0 - p2, p1 - p0];
        let t = {
            let l = e[0].norm();
            if l < 1e-12 {
                continue;
            }
            e[0] / l
        };
        let n = e[0].cross(&e[1]);
        let b = {
            let bb = n.cross(&t);
            let l = bb.norm();
            if l < 1e-12 {
                continue;
            }
            bb / l
        };
        // Each vertex's (already diagonal) principal tensor, in this face frame.
        let mut fcurv = [[0.0_f64; 3]; 3];
        for j in 0..3 {
            let vj = tri[j] as usize;
            let (a, bb, c) = proj_curv(pdir1[vj], pdir2[vj], k1[vj], 0.0, k2[vj], t, b);
            fcurv[j] = [a, bb, c];
        }
        // Fit the cubic form from the variation of the curvature tensor along edges.
        let mut w = [[0.0_f64; 4]; 4];
        let mut m = [0.0_f64; 4];
        for j in 0..3 {
            let df = [
                fcurv[(j + 2) % 3][0] - fcurv[(j + 1) % 3][0],
                fcurv[(j + 2) % 3][1] - fcurv[(j + 1) % 3][1],
                fcurv[(j + 2) % 3][2] - fcurv[(j + 1) % 3][2],
            ];
            let u = e[j].dot(&t);
            let vv = e[j].dot(&b);
            let (u2, v2, uv) = (u * u, vv * vv, u * vv);
            // Normal equations for AᵀA where A rows are [u,v,0,0],[0,u,v,0],[0,0,u,v].
            w[0][0] += u2;
            w[0][1] += uv;
            w[1][1] += u2 + v2;
            w[1][2] += uv;
            w[2][2] += u2 + v2;
            w[2][3] += uv;
            w[3][3] += v2;
            m[0] += u * df[0];
            m[1] += vv * df[0] + u * df[1];
            m[2] += vv * df[1] + u * df[2];
            m[3] += vv * df[2];
        }
        // Symmetrise.
        w[1][0] = w[0][1];
        w[2][1] = w[1][2];
        w[3][2] = w[2][3];
        let Some(face_d) = solve4(w, m) else {
            continue;
        };
        for j in 0..3 {
            let vj = tri[j] as usize;
            let pa = pointareas[vj];
            if pa < 1e-18 {
                continue;
            }
            let pd = proj_dcurv(t, b, face_d, pdir1[vj], pdir2[vj]);
            let wt = cornerareas[fi][j] / pa;
            for k in 0..4 {
                dcurv[vj][k] += wt * pd[k];
            }
        }
    }

    // Extremality of the minimal curvature along its own direction is the pure
    // vvv component of the derivative-of-curvature cubic form in the principal
    // frame (pdir1 = t₁ = u, pdir2 = t₂ = v): e₂ = ∂κ₂/∂t₂ = dcurv[3].
    let emin = dcurv.iter().map(|d| d[3]).collect();

    Curvature {
        pdir2,
        k1,
        k2,
        emin,
    }
}

/// Build the 1-ring vertex adjacency (deduplicated neighbour lists).
#[allow(clippy::indexing_slicing)] // tri vertex indices validated on mesh load
fn vertex_adjacency(mesh: &TriangleMesh) -> Vec<Vec<u32>> {
    let mut adj: Vec<Vec<u32>> = vec![Vec::new(); mesh.vertices.len()];
    let mut push = |a: u32, b: u32| {
        let list = &mut adj[a as usize];
        if !list.contains(&b) {
            list.push(b);
        }
    };
    for tri in &mesh.triangles {
        let (a, b, c) = (tri[0], tri[1], tri[2]);
        push(a, b);
        push(b, a);
        push(b, c);
        push(c, b);
        push(c, a);
        push(a, c);
    }
    adj
}

/// One valley crossing on a mesh edge: the edge identity and its world point.
struct Crossing {
    key: EdgeKey,
    pt: P3,
}

/// March each triangle for the zero set of the minimal-curvature extremality and
/// emit valley segments (each connecting two edge crossings). Orients t₂ (hence
/// e₂'s sign) consistently across the triangle's three vertices, finds the two
/// edges where e₂ changes sign, and keeps the segment only where it is concave
/// (κ₂ < 0), the concave direction dominates (|κ₂| ≥ |κ₁|), and it is sharp
/// enough (|κ₂| ≥ `saliency`).
#[allow(clippy::indexing_slicing)] // tri/loop indices bounded to mesh data and 0..3
fn march_valley_segments(
    mesh: &TriangleMesh,
    cv: &Curvature,
    saliency: f64,
) -> (Vec<(EdgeKey, EdgeKey)>, HashMap<EdgeKey, P3>) {
    let mut segments: Vec<(EdgeKey, EdgeKey)> = Vec::new();
    let mut positions: HashMap<EdgeKey, P3> = HashMap::new();

    for tri in &mesh.triangles {
        let vi = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
        // Reference direction from vertex 0; flip each vertex's t₂ (and the sign
        // of its extremality) to agree, so e₂ is comparable across the triangle.
        let t_ref = cv.pdir2[vi[0]];
        let mut e = [0.0_f64; 3];
        for j in 0..3 {
            let flip = cv.pdir2[vi[j]].dot(&t_ref) < 0.0;
            e[j] = if flip {
                -cv.emin[vi[j]]
            } else {
                cv.emin[vi[j]]
            };
        }

        // Edges of the triangle as (local a, local b).
        const TRI_EDGES: [(usize, usize); 3] = [(0, 1), (1, 2), (2, 0)];
        let mut crossings: Vec<(Crossing, f64, f64)> = Vec::new(); // (crossing, k2, k1) at it
        for &(a, b) in &TRI_EDGES {
            let ea = e[a];
            let eb = e[b];
            // Sign change (strict) → one crossing on this edge.
            if (ea > 0.0 && eb < 0.0) || (ea < 0.0 && eb > 0.0) {
                let denom = ea - eb;
                if denom.abs() < 1e-30 {
                    continue;
                }
                let s = ea / denom; // in (0,1): position from a toward b
                let pa = mesh.vertices[vi[a]];
                let pb = mesh.vertices[vi[b]];
                let pt = P3::new(
                    pa.x + (pb.x - pa.x) * s,
                    pa.y + (pb.y - pa.y) * s,
                    pa.z + (pb.z - pa.z) * s,
                );
                let k2 = cv.k2[vi[a]] + (cv.k2[vi[b]] - cv.k2[vi[a]]) * s;
                let k1 = cv.k1[vi[a]] + (cv.k1[vi[b]] - cv.k1[vi[a]]) * s;
                let key = EdgeKey::new(tri[a], tri[b]);
                crossings.push((Crossing { key, pt }, k2, k1));
            }
        }

        // A regular crest cell has exactly two crossings → one segment.
        if crossings.len() != 2 {
            continue;
        }
        let (c0, k2_0, k1_0) = &crossings[0];
        let (c1, k2_1, k1_1) = &crossings[1];
        let k2_mid = 0.5 * (k2_0 + k2_1);
        let k1_mid = 0.5 * (k1_0 + k1_1);
        // Valley: concave (κ₂ < 0), concave direction dominates (rejects ridges
        // and convex saddles), and sharp enough to clear the saliency dial.
        let concave = k2_mid < 0.0;
        let dominant = k2_mid.abs() >= k1_mid.abs();
        let sharp = k2_mid.abs() >= saliency;
        if !(concave && dominant && sharp) {
            continue;
        }
        positions.entry(c0.key).or_insert(c0.pt);
        positions.entry(c1.key).or_insert(c1.pt);
        if c0.key != c1.key {
            segments.push((c0.key, c1.key));
        }
    }
    (segments, positions)
}

/// Chain valley segments (sharing edge-crossing nodes) into coherent centreline
/// polylines. Mirrors the dihedral chainer: walk from nodes of degree ≠ 2, then
/// sweep up any pure loops, so a junction-free valley becomes one long polyline.
fn chain_segments(
    segments: &[(EdgeKey, EdgeKey)],
    positions: &HashMap<EdgeKey, P3>,
) -> Vec<Vec<P3>> {
    if segments.is_empty() {
        return Vec::new();
    }
    let mut adj: HashMap<EdgeKey, Vec<EdgeKey>> = HashMap::new();
    let mut visited: HashMap<(EdgeKey, EdgeKey), bool> = HashMap::new();
    for &(a, b) in segments {
        adj.entry(a).or_default().push(b);
        adj.entry(b).or_default().push(a);
        let key = if a <= b { (a, b) } else { (b, a) };
        visited.insert(key, false);
    }
    let mark = |visited: &mut HashMap<(EdgeKey, EdgeKey), bool>, a: EdgeKey, b: EdgeKey| {
        let key = if a <= b { (a, b) } else { (b, a) };
        visited.insert(key, true);
    };
    let seen = |visited: &HashMap<(EdgeKey, EdgeKey), bool>, a: EdgeKey, b: EdgeKey| -> bool {
        let key = if a <= b { (a, b) } else { (b, a) };
        visited.get(&key).copied().unwrap_or(true)
    };

    let pos = |k: &EdgeKey| -> P3 { positions.get(k).copied().unwrap_or_else(P3::origin) };
    let walk = |start: EdgeKey,
                first: EdgeKey,
                adj: &HashMap<EdgeKey, Vec<EdgeKey>>,
                visited: &mut HashMap<(EdgeKey, EdgeKey), bool>|
     -> Vec<P3> {
        let mut chain = vec![pos(&start), pos(&first)];
        mark(visited, start, first);
        let mut prev = start;
        let mut cur = first;
        loop {
            let next = adj.get(&cur).and_then(|nbrs| {
                nbrs.iter()
                    .find(|&&n| n != prev && !seen(visited, cur, n))
                    .copied()
            });
            match next {
                Some(n) => {
                    mark(visited, cur, n);
                    chain.push(pos(&n));
                    prev = cur;
                    cur = n;
                }
                None => break,
            }
        }
        chain
    };

    let mut lines: Vec<Vec<P3>> = Vec::new();

    // Deterministic node order.
    let mut starts: Vec<EdgeKey> = adj
        .iter()
        .filter(|(_, n)| n.len() != 2)
        .map(|(&k, _)| k)
        .collect();
    starts.sort_unstable();
    for start in starts {
        if let Some(nbrs) = adj.get(&start) {
            let mut nbrs = nbrs.clone();
            nbrs.sort_unstable();
            for n in nbrs {
                if !seen(&visited, start, n) {
                    lines.push(walk(start, n, &adj, &mut visited));
                }
            }
        }
    }
    // Pure loops (every node degree 2): seed from any untraced edge.
    let mut loop_starts: Vec<(EdgeKey, EdgeKey)> = visited
        .iter()
        .filter(|(_, v)| !**v)
        .map(|(&k, _)| k)
        .collect();
    loop_starts.sort_unstable();
    for (a, b) in loop_starts {
        if !seen(&visited, a, b) {
            lines.push(walk(a, b, &adj, &mut visited));
        }
    }
    lines
}

/// Total XY/Z length of a polyline (mm).
fn polyline_length(points: &[P3]) -> f64 {
    points
        .windows(2)
        .map(|w| {
            #[allow(clippy::indexing_slicing)] // windows(2) yields len-2 slices
            let (a, b) = (w[0], w[1]);
            ((b.x - a.x).powi(2) + (b.y - a.y).powi(2) + (b.z - a.z).powi(2)).sqrt()
        })
        .sum()
}

/// Detect concave valley centrelines on `mesh` via curvature crest-line
/// extraction. Returned polylines are world XY at edge crossings with Z at the
/// interpolated surface; the caller re-lifts Z gouge-safely with the real cutter.
pub fn detect_valley_lines(mesh: &TriangleMesh, params: &CrestParams) -> Vec<Vec<P3>> {
    if mesh.triangles.is_empty() || mesh.vertices.len() < 3 {
        return Vec::new();
    }
    let adj = vertex_adjacency(mesh);
    let cv = compute_curvature(mesh, &adj, params.smoothing_iters);
    let (segments, positions) = march_valley_segments(mesh, &cv, params.valley_saliency);
    let lines = chain_segments(&segments, &positions);
    let kept: Vec<Vec<P3>> = lines
        .into_iter()
        .filter(|l| l.len() >= 2 && polyline_length(l) >= params.min_line_length)
        .collect();
    info!(
        verts = mesh.vertices.len(),
        tris = mesh.triangles.len(),
        valley_saliency = params.valley_saliency,
        smoothing = params.smoothing_iters,
        segments = segments.len(),
        valley_lines = kept.len(),
        "Curvature detector: extracted valley crest lines"
    );
    kept
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout,
    clippy::print_stderr
)]
mod tests {
    use super::*;
    use crate::geo::P3;

    /// Correctly-wound (+Z normals) V-valley heightfield: trough at y=0 rising to
    /// the ±half_y edges, gentle downhill along +X so it reads as one groove.
    fn make_v_valley(len_x: f64, half_y: f64, slope: f64, nx: usize, ny: usize) -> TriangleMesh {
        let mut verts = Vec::new();
        let sx = len_x / nx as f64;
        let sy = 2.0 * half_y / ny as f64;
        for j in 0..=ny {
            for i in 0..=nx {
                let x = i as f64 * sx;
                let y = -half_y + j as f64 * sy;
                let z = -slope * half_y + slope * y.abs();
                verts.push(P3::new(x, y, z));
            }
        }
        let idx = |i: usize, j: usize| (j * (nx + 1) + i) as u32;
        let mut tris = Vec::new();
        for j in 0..ny {
            for i in 0..nx {
                tris.push([idx(i, j), idx(i + 1, j), idx(i + 1, j + 1)]);
                tris.push([idx(i, j), idx(i + 1, j + 1), idx(i, j + 1)]);
            }
        }
        TriangleMesh::from_raw(verts, tris)
    }

    /// A clean V-valley yields one coherent concave centreline along its trough.
    #[test]
    fn curvature_traces_v_valley_trough() {
        let mesh = make_v_valley(40.0, 8.0, 0.6, 60, 32);
        let params = CrestParams {
            valley_saliency: 0.0, // accept any concavity; sign/locus is what we test
            smoothing_iters: 1,
            min_line_length: 5.0,
        };
        let lines = detect_valley_lines(&mesh, &params);
        assert!(!lines.is_empty(), "V-valley should yield ≥1 valley line");
        // The longest line should run along the trough (large X extent, y≈0).
        let longest = lines.iter().max_by(|a, b| {
            polyline_length(a)
                .partial_cmp(&polyline_length(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let longest = longest.unwrap();
        assert!(
            polyline_length(longest) > 15.0,
            "longest valley line {:.1}mm should span the trough",
            polyline_length(longest)
        );
        // Its points should sit near the trough centre y≈0.
        let max_abs_y = longest.iter().map(|p| p.y.abs()).fold(0.0_f64, f64::max);
        assert!(
            max_abs_y < 2.0,
            "valley line should hug the trough centre (y≈0), got max|y|={max_abs_y:.2}"
        );
    }

    /// A tent ridge (peak along y=0) is convex — no valley line may be traced.
    #[test]
    fn curvature_ignores_ridge() {
        let (nx, ny) = (60usize, 32usize);
        let (len_x, half_y, slope) = (40.0, 8.0, 0.6);
        let sx = len_x / nx as f64;
        let sy = 2.0 * half_y / ny as f64;
        let mut verts = Vec::new();
        for j in 0..=ny {
            for i in 0..=nx {
                let x = i as f64 * sx;
                let y = -half_y + j as f64 * sy;
                let z = slope * half_y - slope * y.abs(); // peak at y=0 → ridge
                verts.push(P3::new(x, y, z));
            }
        }
        let idx = |i: usize, j: usize| (j * (nx + 1) + i) as u32;
        let mut tris = Vec::new();
        for j in 0..ny {
            for i in 0..nx {
                tris.push([idx(i, j), idx(i + 1, j), idx(i + 1, j + 1)]);
                tris.push([idx(i, j), idx(i + 1, j + 1), idx(i, j + 1)]);
            }
        }
        let mesh = TriangleMesh::from_raw(verts, tris);
        let params = CrestParams {
            valley_saliency: 0.02,
            smoothing_iters: 1,
            min_line_length: 8.0,
        };
        let lines = detect_valley_lines(&mesh, &params);
        // The ridge crest itself is convex; allow short fragments off the seam
        // but no long coherent valley along it.
        let along_ridge = lines
            .iter()
            .filter(|l| {
                let max_abs_y = l.iter().map(|p| p.y.abs()).fold(0.0_f64, f64::max);
                max_abs_y < 2.0 && polyline_length(l) > 8.0
            })
            .count();
        assert_eq!(along_ridge, 0, "ridge crest must not be traced as a valley");
    }

    /// The saliency dial monotonically thins the traced set: a higher |κ₂|
    /// threshold keeps no more valley lines than a lower one.
    #[test]
    fn saliency_dial_thins_set() {
        let mesh = make_v_valley(40.0, 8.0, 0.6, 60, 32);
        let mk = |sal: f64| CrestParams {
            valley_saliency: sal,
            smoothing_iters: 1,
            min_line_length: 3.0,
        };
        let low: usize = detect_valley_lines(&mesh, &mk(0.0)).len();
        let high: usize = detect_valley_lines(&mesh, &mk(5.0)).len();
        assert!(
            high <= low,
            "raising saliency must not increase valley count: low={low} high={high}"
        );
    }

    /// Opt-in hillshade-overlay validation on the real relief. Renders the
    /// slope-shaded terrain with the detected valley crest lines overlaid in
    /// green so they're visible sitting IN the grooves (top-down Z-colour can't
    /// show valleys on slopes). Sweep WITHOUT recompiling via env vars:
    ///   RS_CAM_CREST_FIXTURE=/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl \
    ///   RS_CAM_CREST_OUT=/tmp/crest.png RS_CAM_CREST_SAL=0.05 RS_CAM_CREST_SMOOTH=3 \
    ///   RS_CAM_CREST_CELL=0.5 \
    ///   cargo test -p rs_cam_core --lib render_crest_hillshade -- --ignored --nocapture
    #[test]
    #[ignore = "needs RS_CAM_CREST_FIXTURE; writes hillshade PNG to RS_CAM_CREST_OUT"]
    fn render_crest_hillshade() {
        use crate::mesh::SpatialIndex;
        use crate::tool::BallEndmill;
        let envf = |k: &str, d: f64| {
            std::env::var(k)
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(d)
        };
        let path = std::env::var("RS_CAM_CREST_FIXTURE").unwrap();
        let out = std::env::var("RS_CAM_CREST_OUT").unwrap_or_else(|_| "/tmp/crest.png".into());
        let sal = envf("RS_CAM_CREST_SAL", 0.05);
        let smooth = envf("RS_CAM_CREST_SMOOTH", 3.0) as usize;
        let cell = envf("RS_CAM_CREST_CELL", 0.5);
        let minlen = envf("RS_CAM_CREST_MINLEN", 2.0);

        let mesh = TriangleMesh::from_stl(std::path::Path::new(&path)).unwrap();
        let index = SpatialIndex::build_auto(&mesh);

        // Hillshade DEM: drop a tiny ball on a grid to read the surface height.
        let bbox = &mesh.bbox;
        let nx = (((bbox.max.x - bbox.min.x) / cell).ceil() as usize).max(1) + 1;
        let ny = (((bbox.max.y - bbox.min.y) / cell).ceil() as usize).max(1) + 1;
        let probe = BallEndmill::new(0.1, 10.0);
        let mut z = vec![f64::NAN; nx * ny];
        let mut valid = vec![false; nx * ny];
        for r in 0..ny {
            for c in 0..nx {
                let x = bbox.min.x + c as f64 * cell;
                let y = bbox.min.y + r as f64 * cell;
                let cl = crate::dropcutter::point_drop_cutter(x, y, &mesh, &index, &probe);
                if cl.contacted {
                    z[r * nx + c] = cl.z;
                    valid[r * nx + c] = true;
                }
            }
        }

        let params = CrestParams {
            valley_saliency: sal,
            smoothing_iters: smooth,
            min_line_length: minlen,
        };
        let t = std::time::Instant::now();
        let lines = detect_valley_lines(&mesh, &params);
        let gen_ms = t.elapsed().as_millis();
        let total_pts: usize = lines.iter().map(|l| l.len()).sum();

        // NW-lit hillshade.
        let ll = -1.0 / 3.0_f64.sqrt();
        let light = [ll, ll, 1.0 / 3.0_f64.sqrt()];
        let mut img = image::RgbImage::from_pixel(nx as u32, ny as u32, image::Rgb([10, 10, 20]));
        for r in 0..ny {
            for c in 0..nx {
                if !valid[r * nx + c] {
                    continue;
                }
                let (a, b) = (c.saturating_sub(1), (c + 1).min(nx - 1));
                let zx = (z[r * nx + b] - z[r * nx + a]) / (2.0 * cell);
                let (a2, b2) = (r.saturating_sub(1), (r + 1).min(ny - 1));
                let zy = (z[b2 * nx + c] - z[a2 * nx + c]) / (2.0 * cell);
                let nrm = (zx * zx + zy * zy + 1.0).sqrt();
                let dot = ((-zx * light[0] - zy * light[1] + light[2]) / nrm).clamp(0.0, 1.0);
                let g = (40.0 + dot * 200.0) as u8;
                let py = (ny - 1 - r) as u32;
                img.put_pixel(c as u32, py, image::Rgb([g, g, g]));
            }
        }
        // Overlay valley crest lines in bright green.
        for line in &lines {
            for p in line {
                let c = ((p.x - bbox.min.x) / cell).round();
                let r = ((p.y - bbox.min.y) / cell).round();
                if c < 0.0 || r < 0.0 || c >= nx as f64 || r >= ny as f64 {
                    continue;
                }
                let py = (ny - 1 - r as usize) as i32;
                let px = c as i32;
                for (ox, oy) in [(0i32, 0i32), (1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let xx = px + ox;
                    let yy = py + oy;
                    if xx >= 0 && yy >= 0 && (xx as usize) < nx && (yy as usize) < ny {
                        img.put_pixel(xx as u32, yy as u32, image::Rgb([60, 255, 90]));
                    }
                }
            }
        }
        img.save(&out).unwrap();
        std::fs::write(
            out.replace(".png", ".txt"),
            format!(
                "saliency={sal} smooth={smooth} cell={cell} grid={nx}x{ny} \
                 valley_lines={} line_points={total_pts} gen_ms={gen_ms}\n",
                lines.len()
            ),
        )
        .ok();
        assert!(
            !lines.is_empty(),
            "rendered {out} | saliency={sal} smooth={smooth} → {} valley lines",
            lines.len()
        );
    }

    /// Repro for the live "no result" on wanaka op 8: the live pencil op runs the
    /// mesh in its setup-local frame (terrain.stl translated ~+20 mm in Z per the
    /// dihedral narration). This applies the same kind of transform and confirms
    /// the detector is invariant to it (it must, since principal curvatures are).
    /// Knobs: RS_CAM_CREST_ZSHIFT (mm, default 20.1), RS_CAM_CREST_ZFLIP (1=flip),
    /// RS_CAM_CREST_XYROT (degrees about Z).
    ///   RS_CAM_CREST_FIXTURE=.../terrain.stl RS_CAM_CREST_ZSHIFT=20.1 \
    ///   cargo test -p rs_cam_core --lib crest_setup_frame_invariance -- --ignored --nocapture
    #[test]
    #[ignore = "needs RS_CAM_CREST_FIXTURE"]
    fn crest_setup_frame_invariance() {
        let envf = |k: &str, d: f64| {
            std::env::var(k)
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(d)
        };
        let path = std::env::var("RS_CAM_CREST_FIXTURE").unwrap();
        let zshift = envf("RS_CAM_CREST_ZSHIFT", 20.1);
        let zflip = envf("RS_CAM_CREST_ZFLIP", 0.0) != 0.0;
        let rot_deg = envf("RS_CAM_CREST_XYROT", 0.0);
        let sal = envf("RS_CAM_CREST_SAL", 0.05);

        let base = TriangleMesh::from_stl(std::path::Path::new(&path)).unwrap();
        let untransformed = detect_valley_lines(
            &base,
            &CrestParams {
                valley_saliency: sal,
                smoothing_iters: 4,
                min_line_length: 4.0,
            },
        );

        // Apply the setup-style transform: optional Z-flip about the bbox centre,
        // a Z-rotation, then a +Z translation — exactly the rigid moves a setup
        // frame composes. Curvature (hence valley detection) must be unchanged.
        let zc = 0.5 * (base.bbox.min.z + base.bbox.max.z);
        let (s, c) = rot_deg.to_radians().sin_cos();
        let verts: Vec<P3> = base
            .vertices
            .iter()
            .map(|p| {
                let z = if zflip { 2.0 * zc - p.z } else { p.z } + zshift;
                P3::new(p.x * c - p.y * s, p.x * s + p.y * c, z)
            })
            .collect();
        // A Z-flip reverses winding; restore CCW so the winding stays outward.
        let tris: Vec<[u32; 3]> = base
            .triangles
            .iter()
            .map(|t| if zflip { [t[0], t[2], t[1]] } else { *t })
            .collect();
        let xformed_mesh = TriangleMesh::from_raw(verts, tris);
        let xformed = detect_valley_lines(
            &xformed_mesh,
            &CrestParams {
                valley_saliency: sal,
                smoothing_iters: 4,
                min_line_length: 4.0,
            },
        );

        eprintln!(
            "crest frame invariance: untransformed={} xformed(zshift={zshift} zflip={zflip} rot={rot_deg})={} sal={sal}",
            untransformed.len(),
            xformed.len()
        );
        assert!(
            untransformed.len() > 100,
            "sanity: base terrain should yield many valley lines, got {}",
            untransformed.len()
        );
        assert!(
            xformed.len() > 100,
            "setup-frame transform must NOT zero out detection: base={} xformed={}",
            untransformed.len(),
            xformed.len()
        );
    }
}
