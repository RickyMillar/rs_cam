//! **Phase F1 research module — direction-field iso-level tool paths.**
//!
//! Research-only. Nothing here is on a production path: no operation, no
//! generator, no GUI surface and no MCP tool reaches this module, and none
//! should until the Phase F1 evidence in
//! `planning/conformal_finish_2026-08-28/PROGRAMME.md` says what it is worth.
//! It follows the precedent of [`crate::scallop_isofield`] — an unshipped
//! research candidate that documents its own limitations rather than
//! pretending to be a feature.
//!
//! # Source
//!
//! **Zou, Wang & Feng, "Length-optimal tool path planning for freeform
//! surfaces with preferred feed directions", arXiv:2009.02660 (v1, 2020).**
//! Equation and section numbers below refer to that paper as transcribed in
//! `planning/conformal_finish_2026-08-28/paper_2009.02660_extraction.md`.
//!
//! The method: represent every tool path implicitly as an iso-level curve of
//! one scalar field φ on the surface, and obtain φ from a single global
//! linear solve.
//!
//! 1. A per-triangle preferred feed direction field `D` (a *line* field —
//!    opposite feed directions machine identically, §4.2), oriented
//!    consistently by BFS propagation with unfold–translate–fold transport
//!    into singular regions (§4.2).
//! 2. A target vector field `V` (Eq. 13): direction `D` rotated 90° about the
//!    surface normal, magnitude `√((k_s + 1/r)/8)` where `k_s` is the normal
//!    curvature perpendicular to the feed direction and `r` the ball radius.
//!    For a ball-end cutter Eq. 5 collapses to the classic iso-scallop Eq. 2,
//!    so `r₁ = r` exactly — the paper's own best case.
//! 3. `min_φ ∫‖∇φ − V‖²` (Eq. 14), whose Euler–Lagrange equation is the
//!    Poisson equation `Δφ = ∇·V` (Eq. 15), discretised with the cotangent
//!    Laplacian (Eq. 16) and cotangent divergence (Eq. 17).
//! 4. Level values from the constant-scallop rule (Eqs. 7–9), then
//!    marching-triangles extraction of each iso-level curve (§3.3).
//!
//! # Repo-authored adaptations — every gap fill, labelled
//!
//! These are the `[REPO]` rows of `research/conformal_finish_2026-08-28.md`
//! §A.2. **None of them is attributable to Zou et al.**
//!
//! * **[REPO] The `D` field itself.** The paper does not compute `D`; it is
//!   "supposed to have been pre-assigned" (§3.2). For a ball-end the
//!   side-step `√(8h/(k_s + 1/r))` is widest where `k_s` is smallest, so this
//!   module feeds along the **maximum signed principal direction t₁**, which
//!   leaves the minimum signed curvature κ₂ perpendicular to the feed. That
//!   derivation is this repo's, not the paper's (design note §C.6).
//! * **[REPO] Boundary conditions.** The paper never states any (extraction
//!   gap 2). Read here as pure Neumann — the natural boundary condition of
//!   Eq. 14 — with one pinned vertex per connected component to remove the
//!   constant nullspace.
//! * **[REPO] Extraction schedule.** "A certain number of points" (§3.3) is
//!   read as *every* edge crossing of the current curve, with the paper's own
//!   conservative minimum rule, starting at `min φ + ½·increment`.
//! * **[REPO] Degeneracy guard.** `√(k_s + 1/r)` is undefined where
//!   `k_s + 1/r ≤ 0`; the paper never mentions it (gap 5). Clamped to the
//!   smallest valid magnitude on the region and **counted** in the report.
//! * **[REPO] `k_s` estimator.** Rusinkiewicz 2004 per-vertex second
//!   fundamental form, already implemented in [`crate::crest_lines`]; the
//!   paper names no estimator (gap 6).
//! * **[REPO] Eq. 17 index typo.** The paper sums over incident triangles `k`
//!   but writes `V_j` inside. Resolved as the standard cotangent divergence
//!   of Botsch et al. (the paper's own ref [27]) — see [`assemble_poisson`].
//! * **[REPO] Direction-field smoothing.** §4.2 post-processes with
//!   "Laplacian smoothing" and states no weights, iteration count or stopping
//!   rule (gap 8). A fixed, bounded project-and-renormalise pass is used here.
//! * **[REPO scope cut] No segmentation.** §4.3 segments the surface where
//!   the field changes abruptly; the authors call the resulting patch borders
//!   a "serious limitation". F1 runs on a single smoothly-varying region and
//!   **counts** orientation inconsistencies instead of segmenting.
//!
//! # Numerics
//!
//! The Poisson system is solved **matrix-free** with a Jacobi-preconditioned
//! conjugate gradient over adjacency lists in `f64` — no new dependency, for
//! the same reasons [`crate::scallop_isofield`] chose matrix-free fast
//! sweeping for its Eikonal solve: deterministic, no heap surprises, nothing
//! added to a manifest. The assembled matrix is the Dirichlet-energy Hessian,
//! so it is positive semi-definite by construction and SPD once pinned, even
//! where obtuse triangles give negative cotangent weights.
//!
//! # Limitations a reader must not lose
//!
//! * **Curvature signs inherit `crest_lines`' +Z-forced vertex normals**
//!   (`crest_lines.rs:264-297`): convex-upward reads positive, concave reads
//!   negative. That is the right convention for the terrain regions F1
//!   targets and for both test fixtures, and it is **wrong for a region whose
//!   surface faces away from +Z**.
//! * The scallop constraint is **soft**. Eq. 14 fits `‖∇φ‖` to the
//!   constant-scallop magnitude in least squares; it does not enforce it. The
//!   paper's own measured scallop error is < 4 % over constraints
//!   0.01–0.1 mm (§5.2), which is the band F1's `h = 0.03 mm` sits in.
//! * Output points are **cutter-contact points on the mesh surface**. The
//!   cutter-centre conversion (drop-cutter projection, the rs_cam convention)
//!   belongs to the evidence instrument, not here.
//! * Curvature is estimated on the **whole** mesh and then restricted, so a
//!   region border keeps the curvature of its surroundings; the Poisson solve
//!   and the extraction run on the induced submesh only.

use std::collections::{HashMap, VecDeque};

use crate::crest_lines::{Curvature, compute_curvature, vertex_adjacency};
use crate::geo::{P3, V3};
use crate::marching_squares::CHAIN_EPS;
use crate::mesh::TriangleMesh;
use crate::pencil_dihedral::{EdgeKey, build_edge_adjacency};

/// The three edges of a triangle as (local corner a, local corner b).
const TRI_EDGES: [(usize, usize); 3] = [(0, 1), (1, 2), (2, 0)];

/// Triangles below this area (mm²) are dropped from the region — the same
/// guard `crest_lines::point_areas` applies, and the one that keeps a
/// cotangent from blowing up on a sliver.
const MIN_TRIANGLE_AREA_MM2: f64 = 1e-18;

/// Below this, a vector is treated as having no direction.
const EPS_VEC: f64 = 1e-12;

/// Below this, `k_s + 1/r` is treated as non-positive (Eq. 13 degeneracy).
const EPS_DENOM: f64 = 1e-12;

// ---------------------------------------------------------------------------
// Parameters
// ---------------------------------------------------------------------------

/// Tunables for one direction-field solve.
#[derive(Debug, Clone)]
pub struct FieldParams {
    /// Ball-end cutter radius `r` (mm). **[SOURCE §5.1]** for a ball-end the
    /// effective cutting radius `r₁` of Eq. 5 *is* the tool radius.
    pub ball_radius_mm: f64,
    /// Scallop-height constraint `h` (mm). The paper validates 0.01–0.1 mm
    /// (§5.2); F1 uses 0.03.
    pub scallop_h_mm: f64,
    /// Curvature-tensor smoothing iterations passed through to
    /// [`crate::crest_lines`] (Rusinkiewicz tensor diffusion, geometry
    /// untouched).
    pub curvature_smoothing_iters: usize,
    /// **[REPO]** Line-field Laplacian smoothing iterations (§4.2 asks for
    /// "Laplacian smoothing" and states no count). Fixed and bounded so the
    /// pass is deterministic and cheap; 0 disables it.
    pub direction_smoothing_iters: usize,
    /// **[REPO]** A triangle is *singular* (no preferred direction, §4.2 —
    /// e.g. a flat or umbilic region) when `|κ₁ − κ₂|` is at or below
    /// `max(isotropy_abs_tol, isotropy_rel_tol · max(|κ₁|, |κ₂|))`. The
    /// relative arm is what catches a numerically-noisy umbilic such as a
    /// tessellated sphere; the absolute arm catches an exactly flat region.
    pub isotropy_rel_tol: f64,
    /// Absolute isotropy floor (1/mm) — see [`FieldParams::isotropy_rel_tol`].
    pub isotropy_abs_tol: f64,
    /// Conjugate-gradient iteration cap (deterministic upper bound).
    pub cg_max_iters: usize,
    /// Conjugate-gradient stopping tolerance, relative to `‖rhs‖`.
    pub cg_rel_tolerance: f64,
    /// Hard cap on the number of extracted levels.
    pub max_levels: usize,
    /// **[REPO]** Floor on one level increment, as a fraction of the field's
    /// own range. A crossing that lands in a near-zero-gradient triangle would
    /// otherwise drive the increment to zero and stall the schedule; floored
    /// increments are counted in [`FieldReport::floored_increments`].
    pub min_level_increment_fraction: f64,
}

impl Default for FieldParams {
    fn default() -> Self {
        Self {
            ball_radius_mm: 3.0,
            scallop_h_mm: 0.03,
            curvature_smoothing_iters: 3,
            direction_smoothing_iters: 10,
            isotropy_rel_tol: 0.10,
            isotropy_abs_tol: 1e-6,
            cg_max_iters: 5000,
            cg_rel_tolerance: 1e-10,
            max_levels: 4096,
            min_level_increment_fraction: 1e-3,
        }
    }
}

impl FieldParams {
    /// Defaults with the two physical dials set.
    #[must_use]
    pub fn new(ball_radius_mm: f64, scallop_h_mm: f64) -> Self {
        Self {
            ball_radius_mm,
            scallop_h_mm,
            ..Self::default()
        }
    }
}

// ---------------------------------------------------------------------------
// Result + report
// ---------------------------------------------------------------------------

/// The extracted iso-level curves and the field they came from.
///
/// Points are **cutter-contact points on the mesh surface** (each lies on a
/// mesh edge, linearly interpolated). Converting them to cutter-centre points
/// is the caller's job — see the module header.
#[derive(Debug, Clone, Default)]
pub struct FieldPathResult {
    /// The level values `{l_i}`, ascending.
    pub levels: Vec<f64>,
    /// Every extracted polyline, in level order. A closed curve repeats its
    /// first point as its last.
    pub polylines: Vec<Vec<P3>>,
    /// For each entry of [`FieldPathResult::polylines`], the index into
    /// [`FieldPathResult::levels`] it was extracted at.
    pub polyline_levels: Vec<usize>,
    /// The solved scalar field, one value per entry of
    /// [`FieldPathResult::vertex_ids`].
    pub phi: Vec<f64>,
    /// Global mesh vertex ids for [`FieldPathResult::phi`].
    pub vertex_ids: Vec<u32>,
}

impl FieldPathResult {
    /// The polylines extracted at level index `level_idx`.
    #[must_use]
    pub fn polylines_at(&self, level_idx: usize) -> Vec<&Vec<P3>> {
        self.polylines
            .iter()
            .zip(self.polyline_levels.iter())
            .filter(|&(_, &li)| li == level_idx)
            .map(|(p, _)| p)
            .collect()
    }
}

/// Everything the F1 contract wants counted rather than assumed.
///
/// A count of zero means *measured zero*; these are all measured on every
/// call, so there is no "not measured" arm to conflate with clean.
#[derive(Debug, Clone, Default)]
pub struct FieldReport {
    /// Triangles actually solved on (after dropping out-of-range indices,
    /// duplicates and slivers below [`MIN_TRIANGLE_AREA_MM2`]).
    pub region_triangles: usize,
    /// Distinct vertices referenced by those triangles.
    pub region_vertices: usize,
    /// Vertex-connectivity components of the region. One vertex is pinned per
    /// component — a region with several components is still solvable, but the
    /// level values of different components are not comparable.
    pub vertex_components: usize,
    /// BFS seeds used by the orientation pass (§4.2 uses several seeds when
    /// directions cannot all be flipped into agreement).
    pub direction_seeds: usize,
    /// Triangles found singular by the isotropy test (§4.2 "singular
    /// regions"): no preferred direction of their own.
    pub degenerate_triangles: usize,
    /// Triangles whose direction was **copied** from a parent through the
    /// unfold–translate–fold transport of §4.2 Fig. 6.
    pub transported_triangles: usize,
    /// Interior edges whose two triangles disagree about orientation after
    /// propagation — the unresolvable loops of §4.3. Counted once per edge and
    /// **not** acted on: segmenting the surface is out of F1 scope by design.
    pub orientation_inconsistencies: usize,
    /// Region triangles the orientation pass never reached (should be 0).
    pub unoriented_triangles: usize,
    /// Triangles where `k_s + 1/r ≤ 0` and the Eq. 13 magnitude was clamped.
    pub clamped_magnitude_triangles: usize,
    /// Smallest `‖V‖` on the region after clamping.
    pub min_target_magnitude: f64,
    /// Largest `‖V‖` on the region.
    pub max_target_magnitude: f64,
    /// Area-unweighted mean `‖V‖` on the region.
    pub mean_target_magnitude: f64,
    /// Conjugate-gradient iterations taken.
    pub cg_iterations: usize,
    /// Final CG residual, relative to `‖rhs‖`.
    pub cg_residual: f64,
    /// Whether CG reached [`FieldParams::cg_rel_tolerance`] inside the cap.
    pub cg_converged: bool,
    /// Number of curve components extracted at each level, in level order.
    pub level_component_counts: Vec<usize>,
    /// Closed components (first point within [`CHAIN_EPS`] of the last).
    pub closed_loops: usize,
    /// Triangles the level actually passed through but which did **not**
    /// produce exactly two edge crossings — saddles through a vertex, and
    /// degenerate configurations. Counted, never silently dropped: contract
    /// item 4 depends on this number.
    pub degenerate_crossings: usize,
    /// Level increments that hit
    /// [`FieldParams::min_level_increment_fraction`] instead of the
    /// constant-scallop rule.
    pub floored_increments: usize,
    /// Whether [`FieldParams::max_levels`] stopped the schedule before the
    /// field's maximum was reached.
    pub level_cap_hit: bool,
    /// Total polylines emitted across all levels.
    pub total_polylines: usize,
}

// ---------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------

/// Every triangle of `mesh`, as a region index list.
#[must_use]
pub fn all_triangles(mesh: &TriangleMesh) -> Vec<u32> {
    (0..mesh.triangles.len() as u32).collect()
}

/// The triangles of `mesh` whose index and centroid satisfy `pred` — the
/// predicate form of region restriction.
pub fn triangles_where<F>(mesh: &TriangleMesh, pred: F) -> Vec<u32>
where
    F: Fn(usize, P3) -> bool,
{
    mesh.faces
        .iter()
        .enumerate()
        .filter_map(|(i, f)| {
            let c = centroid(&f.v);
            if pred(i, c) { Some(i as u32) } else { None }
        })
        .collect()
}

/// Solve the direction field, the Poisson system and the iso-level schedule on
/// the triangles named by `region_triangles`, with default tunables.
///
/// Indices are into `mesh.triangles`; duplicates and out-of-range entries are
/// dropped. An empty or unusable region returns an empty result and a report
/// whose counts say so — this function never panics.
#[must_use]
pub fn solve_field_paths(
    mesh: &TriangleMesh,
    region_triangles: &[u32],
    ball_radius_mm: f64,
    scallop_h_mm: f64,
) -> (FieldPathResult, FieldReport) {
    let params = FieldParams::new(ball_radius_mm, scallop_h_mm);
    solve_field_paths_with(mesh, region_triangles, &params)
}

/// [`solve_field_paths`] with explicit tunables.
#[must_use]
pub fn solve_field_paths_with(
    mesh: &TriangleMesh,
    region_triangles: &[u32],
    params: &FieldParams,
) -> (FieldPathResult, FieldReport) {
    let mut report = FieldReport::default();
    let Some(region) = build_region_mesh(mesh, region_triangles) else {
        return (FieldPathResult::default(), report);
    };
    report.region_triangles = region.tris.len();
    report.region_vertices = region.verts.len();

    // [REPO] Curvature is estimated on the WHOLE mesh, then read by global
    // vertex id, so a region border keeps the curvature of its surroundings.
    let adjacency = vertex_adjacency(mesh);
    let curvature = compute_curvature(mesh, &adjacency, params.curvature_smoothing_iters);

    let directions = orient_direction_field(&region, &curvature, params, &mut report);
    let target = build_target_field(&region, &curvature, &directions, params, &mut report);
    solve_and_extract(&region, &target, params, report)
}

/// Solve and extract for a **caller-supplied** target vector field `V`,
/// bypassing the curvature-derived direction field.
///
/// `target` is called once per region triangle as
/// `target(global_triangle_index, centroid, unit_normal)` and must return `V`
/// in world coordinates; the component along the triangle normal is discarded
/// (`V` is a tangent field). The normal handed in is oriented +Z, matching the
/// convention [`crate::crest_lines`] uses for curvature signs.
///
/// This is how the F1 evidence instrument substitutes an analytic or
/// externally-computed field, and how the closed-form tests in this module
/// assert the solver and the extractor separately from the direction field.
/// The direction-field counters of [`FieldReport`] stay zero on this path.
pub fn solve_paths_with_target<F>(
    mesh: &TriangleMesh,
    region_triangles: &[u32],
    target: F,
    params: &FieldParams,
) -> (FieldPathResult, FieldReport)
where
    F: Fn(usize, P3, V3) -> V3,
{
    let mut report = FieldReport::default();
    let Some(region) = build_region_mesh(mesh, region_triangles) else {
        return (FieldPathResult::default(), report);
    };
    report.region_triangles = region.tris.len();
    report.region_vertices = region.verts.len();

    let mut field = Vec::with_capacity(region.tris.len());
    for (local, &global) in region.tri_ids.iter().enumerate() {
        let n = region.normal(local);
        let c = region.centroid(local);
        let v = target(global, c, n);
        field.push(v - n * v.dot(&n));
    }
    summarise_magnitudes(&field, &mut report);
    solve_and_extract(&region, &field, params, report)
}

/// The shared back half: assemble the Poisson system for `target`, solve it,
/// and run the level schedule + marching-triangles extraction.
fn solve_and_extract(
    region: &RegionMesh,
    target: &[V3],
    params: &FieldParams,
    mut report: FieldReport,
) -> (FieldPathResult, FieldReport) {
    let (laplacian, rhs) = assemble_poisson(region, target);
    let pinned = pin_one_vertex_per_component(region, &mut report);
    let solve = cg_solve(&laplacian, &rhs, &pinned, params);
    report.cg_iterations = solve.iterations;
    report.cg_residual = solve.residual;
    report.cg_converged = solve.converged;

    let mut result = FieldPathResult {
        phi: solve.x.clone(),
        vertex_ids: region.vert_ids.clone(),
        ..FieldPathResult::default()
    };
    extract_levels(region, &solve.x, target, params, &mut result, &mut report);
    report.total_polylines = result.polylines.len();
    (result, report)
}

// ---------------------------------------------------------------------------
// Region submesh + triangle adjacency
// ---------------------------------------------------------------------------

/// The induced submesh of a machining region: local vertex numbering, local
/// triangle numbering, and the triangle-neighbour lookup both the orientation
/// BFS and the extraction need.
///
/// The neighbour lookup is built on top of
/// [`crate::pencil_dihedral::build_edge_adjacency`] — the repo's existing
/// edge→incident-face map — rather than a general half-edge library, which
/// F1 does not need.
pub(crate) struct RegionMesh {
    /// Global mesh triangle index per local triangle.
    pub(crate) tri_ids: Vec<usize>,
    /// Local vertex indices per local triangle.
    pub(crate) tris: Vec<[usize; 3]>,
    /// Global mesh vertex id per local vertex.
    pub(crate) vert_ids: Vec<u32>,
    /// Position per local vertex.
    pub(crate) verts: Vec<P3>,
    /// +Z-oriented unit normal per local triangle.
    pub(crate) normals: Vec<V3>,
    /// Area (mm²) per local triangle.
    pub(crate) areas: Vec<f64>,
    /// Local triangle across each of [`TRI_EDGES`], if that neighbour is also
    /// in the region.
    pub(crate) neighbours: Vec<[Option<usize>; 3]>,
}

impl RegionMesh {
    pub(crate) fn normal(&self, tri: usize) -> V3 {
        self.normals.get(tri).copied().unwrap_or_else(V3::zeros)
    }

    pub(crate) fn area(&self, tri: usize) -> f64 {
        self.areas.get(tri).copied().unwrap_or(0.0)
    }

    pub(crate) fn corners(&self, tri: usize) -> [usize; 3] {
        self.tris.get(tri).copied().unwrap_or([0, 0, 0])
    }

    pub(crate) fn point(&self, local_vertex: usize) -> P3 {
        self.verts
            .get(local_vertex)
            .copied()
            .unwrap_or_else(P3::origin)
    }

    fn global_vertex(&self, local_vertex: usize) -> usize {
        self.vert_ids.get(local_vertex).copied().unwrap_or_default() as usize
    }

    fn centroid(&self, tri: usize) -> P3 {
        let c = self.corners(tri);
        let (a, b, d) = (self.point(c[0]), self.point(c[1]), self.point(c[2]));
        P3::new(
            (a.x + b.x + d.x) / 3.0,
            (a.y + b.y + d.y) / 3.0,
            (a.z + b.z + d.z) / 3.0,
        )
    }
}

fn centroid(v: &[P3; 3]) -> P3 {
    P3::new(
        (v[0].x + v[1].x + v[2].x) / 3.0,
        (v[0].y + v[1].y + v[2].y) / 3.0,
        (v[0].z + v[1].z + v[2].z) / 3.0,
    )
}

/// Build the induced submesh. Returns `None` when nothing usable is left.
#[allow(clippy::indexing_slicing)]
// SAFETY: every index below is either a fixed 0..3 corner index or a local
// index just produced by the local-vertex map in this same function.
pub(crate) fn build_region_mesh(
    mesh: &TriangleMesh,
    region_triangles: &[u32],
) -> Option<RegionMesh> {
    if mesh.triangles.is_empty() || mesh.vertices.len() < 3 {
        return None;
    }
    // Deterministic order: ascending global triangle index, deduplicated.
    let mut wanted: Vec<u32> = region_triangles.to_vec();
    wanted.sort_unstable();
    wanted.dedup();

    let mut vert_map: HashMap<u32, usize> = HashMap::new();
    let mut vert_ids: Vec<u32> = Vec::new();
    let mut verts: Vec<P3> = Vec::new();
    let mut tri_ids: Vec<usize> = Vec::new();
    let mut tris: Vec<[usize; 3]> = Vec::new();
    let mut normals: Vec<V3> = Vec::new();
    let mut areas: Vec<f64> = Vec::new();

    for &g in &wanted {
        let gi = g as usize;
        let Some(tri) = mesh.triangles.get(gi) else {
            continue;
        };
        let Some(p0) = mesh.vertices.get(tri[0] as usize).copied() else {
            continue;
        };
        let Some(p1) = mesh.vertices.get(tri[1] as usize).copied() else {
            continue;
        };
        let Some(p2) = mesh.vertices.get(tri[2] as usize).copied() else {
            continue;
        };
        let cross = (p1 - p0).cross(&(p2 - p0));
        let twice_area = cross.norm();
        let area = 0.5 * twice_area;
        if area < MIN_TRIANGLE_AREA_MM2 {
            continue;
        }
        // +Z-oriented normal, matching `crest_lines`' vertex-normal convention
        // so the curvature signs this module reads stay consistent.
        let mut n = cross / twice_area;
        if n.z < 0.0 {
            n = -n;
        }
        let mut local = [0usize; 3];
        for (slot, &gv) in local.iter_mut().zip(tri.iter()) {
            let next = vert_ids.len();
            let li = *vert_map.entry(gv).or_insert(next);
            if li == next {
                vert_ids.push(gv);
                verts.push(match gv {
                    v if v == tri[0] => p0,
                    v if v == tri[1] => p1,
                    _ => p2,
                });
            }
            *slot = li;
        }
        tri_ids.push(gi);
        tris.push(local);
        normals.push(n);
        areas.push(area);
    }
    if tris.is_empty() {
        return None;
    }

    // Triangle neighbours across shared edges, from the existing edge→face map.
    let edge_map = build_edge_adjacency(mesh);
    let global_to_local: HashMap<usize, usize> = tri_ids
        .iter()
        .enumerate()
        .map(|(local, &global)| (global, local))
        .collect();
    let mut neighbours: Vec<[Option<usize>; 3]> = Vec::with_capacity(tris.len());
    for (local, &global) in tri_ids.iter().enumerate() {
        let mut slots: [Option<usize>; 3] = [None; 3];
        let Some(tri) = mesh.triangles.get(global) else {
            neighbours.push(slots);
            continue;
        };
        for (e, &(a, b)) in TRI_EDGES.iter().enumerate() {
            let key = EdgeKey::new(tri[a], tri[b]);
            let Some(faces) = edge_map.get(&key) else {
                continue;
            };
            // Deterministic pick: lowest local index among region members that
            // is not this triangle. A non-manifold edge therefore contributes
            // one neighbour, not several.
            let mut candidates: Vec<usize> = faces
                .iter()
                .filter_map(|f| global_to_local.get(f).copied())
                .filter(|&l| l != local)
                .collect();
            candidates.sort_unstable();
            slots[e] = candidates.first().copied();
        }
        neighbours.push(slots);
    }

    Some(RegionMesh {
        tri_ids,
        tris,
        vert_ids,
        verts,
        normals,
        areas,
        neighbours,
    })
}

// ---------------------------------------------------------------------------
// Direction field (§4.2)
// ---------------------------------------------------------------------------

/// Per-triangle line-field state during orientation propagation.
struct RawDirection {
    /// Unit feed direction in the triangle plane, sign not yet fixed.
    dir: V3,
    /// `|κ₁ − κ₂|` averaged over the triangle's vertices.
    anisotropy: f64,
    /// Whether the isotropy test called this triangle singular (§4.2).
    degenerate: bool,
}

/// Average the per-vertex maximum-signed principal direction t₁ into the
/// triangle plane, as a line field.
///
/// **[REPO]** `D = t₁`. The paper supplies no direction field (§3.2); this
/// choice is derived in `research/conformal_finish_2026-08-28.md` §A.2 gap 1
/// and belongs to this repo, not to Zou et al.
///
/// `crest_lines` orders principal curvatures by **signed** value, not by
/// magnitude — `diagonalize` (`crest_lines.rs:172-207`) returns
/// `(kmax, kmin, dir_max, dir_min)` with `kmax ≥ kmin` as signed numbers and
/// its own docstring says "kept signed rather than by magnitude", and
/// `compute_curvature` (`crest_lines.rs:452-462`) assigns `k1 = kmax`,
/// `pdir1 = dmax`. So `pdir1` already **is** the direction of maximum signed
/// normal curvature and no re-sort is needed here: feeding along it leaves the
/// minimum signed curvature κ₂ perpendicular, which is what maximises the
/// side-step `√(8h/(k_s + 1/r))`.
#[allow(clippy::indexing_slicing)]
// SAFETY: `c` holds local vertex indices produced by `build_region_mesh`, and
// `gv` is a global vertex id from the same mesh the curvature was computed on;
// both are bounded by construction.
fn raw_direction(
    region: &RegionMesh,
    curvature: &Curvature,
    tri: usize,
    params: &FieldParams,
) -> RawDirection {
    let n = region.normal(tri);
    let c = region.corners(tri);

    let mut reference = V3::zeros();
    let mut sum = V3::zeros();
    let mut aniso = 0.0_f64;
    let mut scale = 0.0_f64;
    for &lv in &c {
        let gv = region.global_vertex(lv);
        let t1 = curvature.pdir1.get(gv).copied().unwrap_or_else(V3::zeros);
        let k1 = curvature.k1.get(gv).copied().unwrap_or(0.0);
        let k2 = curvature.k2.get(gv).copied().unwrap_or(0.0);
        aniso += (k1 - k2).abs() / 3.0;
        scale += k1.abs().max(k2.abs()) / 3.0;

        let projected = t1 - n * t1.dot(&n);
        let len = projected.norm();
        if len <= EPS_VEC {
            continue;
        }
        let unit = projected / len;
        if reference.norm() <= EPS_VEC {
            reference = unit;
        }
        // Line field: compare through |dot|, so a vertex whose t₁ points the
        // other way still reinforces rather than cancels (§4.2).
        sum += if unit.dot(&reference) < 0.0 {
            -unit
        } else {
            unit
        };
    }

    let dir = if sum.norm() > EPS_VEC {
        sum.normalize()
    } else {
        // No usable t₁ anywhere on this triangle: fall back to its longest
        // edge, which is deterministic and in-plane.
        let e = region.point(c[1]) - region.point(c[0]);
        let e = if e.norm() > EPS_VEC {
            e
        } else {
            region.point(c[2]) - region.point(c[0])
        };
        let projected = e - n * e.dot(&n);
        if projected.norm() > EPS_VEC {
            projected.normalize()
        } else {
            V3::new(1.0, 0.0, 0.0)
        }
    };

    let degenerate = aniso <= params.isotropy_abs_tol.max(params.isotropy_rel_tol * scale);
    RawDirection {
        dir,
        anisotropy: aniso,
        degenerate,
    }
}

/// Rotate `v` from the plane of `n_from` into the plane of `n_to` about the
/// shared edge — the **unfold–translate–fold** transport of §4.2 Fig. 6.
///
/// **[SOURCE §4.2]** The paper's own caveat applies: this ignores holonomy, so
/// a field transported around a loop on a curved surface need not close, which
/// is exactly what [`FieldReport::orientation_inconsistencies`] counts.
fn transport(v: V3, edge: V3, n_from: V3, n_to: V3) -> V3 {
    let len = edge.norm();
    if len <= EPS_VEC {
        return v;
    }
    let axis = edge / len;
    let cos_a = n_from.dot(&n_to).clamp(-1.0, 1.0);
    let sin_a = n_from.cross(&n_to).dot(&axis);
    let angle = sin_a.atan2(cos_a);
    // Rodrigues rotation of `v` about `axis` by `angle`.
    let (s, c) = angle.sin_cos();
    let rotated = v * c + axis.cross(&v) * s + axis * (axis.dot(&v) * (1.0 - c));
    // Fold: re-seat in the destination plane so accumulated drift cannot lift
    // the direction off the surface.
    let flat = rotated - n_to * rotated.dot(&n_to);
    if flat.norm() > EPS_VEC {
        flat.normalize()
    } else {
        rotated
    }
}

/// Orient the line field consistently by BFS propagation, copy-transporting
/// into singular regions, then smooth it.
///
/// **[SOURCE §4.2]** for the BFS + flip + copy-transport structure and the
/// multiple-seed rule. **[REPO]** for the isotropy test that decides
/// singularity (gap 9: the paper states no numerical test) and for the bounded
/// smoothing pass (gap 8).
fn orient_direction_field(
    region: &RegionMesh,
    curvature: &Curvature,
    params: &FieldParams,
    report: &mut FieldReport,
) -> Vec<V3> {
    let n = region.tris.len();
    let raw: Vec<RawDirection> = (0..n)
        .map(|t| raw_direction(region, curvature, t, params))
        .collect();
    report.degenerate_triangles = raw.iter().filter(|r| r.degenerate).count();

    // Seed order: the most anisotropic non-degenerate triangle first (the
    // best-determined direction on the region), then everything else by index.
    let mut seed_order: Vec<usize> = (0..n).collect();
    seed_order.sort_by(|&a, &b| {
        let (ra, rb) = (raw.get(a), raw.get(b));
        let da = ra.is_none_or(|r| r.degenerate);
        let db = rb.is_none_or(|r| r.degenerate);
        let aa = ra.map_or(0.0, |r| r.anisotropy);
        let ab = rb.map_or(0.0, |r| r.anisotropy);
        da.cmp(&db).then(ab.total_cmp(&aa)).then(a.cmp(&b))
    });

    let mut dir: Vec<Option<V3>> = vec![None; n];
    let mut queue: VecDeque<usize> = VecDeque::new();
    for &seed in &seed_order {
        if dir.get(seed).copied().flatten().is_some() {
            continue;
        }
        let Some(seed_raw) = raw.get(seed) else {
            continue;
        };
        if let Some(slot) = dir.get_mut(seed) {
            *slot = Some(seed_raw.dir);
        }
        report.direction_seeds += 1;
        queue.push_back(seed);

        while let Some(t) = queue.pop_front() {
            let Some(dt) = dir.get(t).copied().flatten() else {
                continue;
            };
            let nt = region.normal(t);
            let corners = region.corners(t);
            let nbrs = region
                .neighbours
                .get(t)
                .copied()
                .unwrap_or([None, None, None]);
            for (e, &(a, b)) in TRI_EDGES.iter().enumerate() {
                let Some(nb) = nbrs.get(e).copied().flatten() else {
                    continue;
                };
                let (Some(&ca), Some(&cb)) = (corners.get(a), corners.get(b)) else {
                    continue;
                };
                let edge = region.point(cb) - region.point(ca);
                let carried = transport(dt, edge, nt, region.normal(nb));
                match dir.get(nb).copied().flatten() {
                    None => {
                        let Some(nb_raw) = raw.get(nb) else {
                            continue;
                        };
                        let chosen = if nb_raw.degenerate {
                            // §4.2 singular region: copy, do not fit.
                            report.transported_triangles += 1;
                            carried
                        } else if nb_raw.dir.dot(&carried) < 0.0 {
                            -nb_raw.dir
                        } else {
                            nb_raw.dir
                        };
                        if let Some(slot) = dir.get_mut(nb) {
                            *slot = Some(chosen);
                        }
                        queue.push_back(nb);
                    }
                    Some(existing) => {
                        // Count each interior edge once. A disagreement here is
                        // §4.3's unresolvable loop; F1 records it and carries
                        // on rather than segmenting the surface.
                        if t < nb && existing.dot(&carried) < 0.0 {
                            report.orientation_inconsistencies += 1;
                        }
                    }
                }
            }
        }
    }

    let mut out: Vec<V3> = Vec::with_capacity(n);
    for (t, slot) in dir.iter().enumerate() {
        match slot {
            Some(d) => out.push(*d),
            None => {
                report.unoriented_triangles += 1;
                out.push(raw.get(t).map_or_else(|| V3::new(1.0, 0.0, 0.0), |r| r.dir));
            }
        }
    }
    smooth_line_field(region, &mut out, params.direction_smoothing_iters);
    out
}

/// **[REPO]** Bounded Laplacian smoothing of the oriented line field.
///
/// §4.2 asks for "post-processing of the directions using Laplacian smoothing"
/// and states no weights, iteration count or stopping rule (extraction gap 8).
/// This is the simplest thing that is deterministic and cannot run away: a
/// fixed number of Jacobi sweeps, each averaging a triangle's direction with
/// its neighbours' directions transported into its own plane, sign-aligned
/// through the sign of the dot product (a line field, so the alignment is by
/// `|dot|`), then renormalised. No angle-doubling representation is used —
/// on a triangle mesh each triangle carries its own tangent plane, so the
/// transport already supplies the frame the doubling would have provided.
fn smooth_line_field(region: &RegionMesh, dir: &mut [V3], iterations: usize) {
    if iterations == 0 || dir.len() != region.tris.len() {
        return;
    }
    for _ in 0..iterations {
        let previous = dir.to_vec();
        for t in 0..region.tris.len() {
            let Some(current) = previous.get(t).copied() else {
                continue;
            };
            let nt = region.normal(t);
            let corners = region.corners(t);
            let nbrs = region
                .neighbours
                .get(t)
                .copied()
                .unwrap_or([None, None, None]);
            let mut sum = current;
            for (e, &(a, b)) in TRI_EDGES.iter().enumerate() {
                let Some(nb) = nbrs.get(e).copied().flatten() else {
                    continue;
                };
                let Some(dn) = previous.get(nb).copied() else {
                    continue;
                };
                let (Some(&ca), Some(&cb)) = (corners.get(a), corners.get(b)) else {
                    continue;
                };
                let edge = region.point(cb) - region.point(ca);
                let carried = transport(dn, edge, region.normal(nb), nt);
                sum += if carried.dot(&current) < 0.0 {
                    -carried
                } else {
                    carried
                };
            }
            let flat = sum - nt * sum.dot(&nt);
            if flat.norm() > EPS_VEC
                && let Some(slot) = dir.get_mut(t)
            {
                *slot = flat.normalize();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Target vector field V (Eq. 13)
// ---------------------------------------------------------------------------

/// Normal curvature along the unit tangent `u`, from the averaged per-vertex
/// second fundamental form: `II(u,u) = κ₁(u·t₁)² + κ₂(u·t₂)²`.
///
/// `u` lies in the triangle plane, which only approximates each vertex's own
/// tangent plane, so the two projections are renormalised to sum to one — the
/// alternative silently biases `k_s` toward zero wherever the triangle is
/// tilted against the vertex normal.
fn normal_curvature(region: &RegionMesh, curvature: &Curvature, tri: usize, u: V3) -> f64 {
    let corners = region.corners(tri);
    let mut acc = 0.0_f64;
    for &lv in &corners {
        let gv = region.global_vertex(lv);
        let k1 = curvature.k1.get(gv).copied().unwrap_or(0.0);
        let k2 = curvature.k2.get(gv).copied().unwrap_or(0.0);
        let t1 = curvature.pdir1.get(gv).copied().unwrap_or_else(V3::zeros);
        let t2 = curvature.pdir2.get(gv).copied().unwrap_or_else(V3::zeros);
        let a = u.dot(&t1);
        let b = u.dot(&t2);
        let s = a * a + b * b;
        acc += if s > EPS_VEC {
            (k1 * a * a + k2 * b * b) / s
        } else {
            0.5 * (k1 + k2)
        };
    }
    acc / 3.0
}

/// Build the target field `V` of Eq. 13.
///
/// **[SOURCE §3.2, Eq. 13]** direction `D` rotated 90° about the normal,
/// magnitude `√((k_s + 1/r₁)/8)` with `k_s` the normal curvature perpendicular
/// to the feed direction. **[SOURCE §5.1]** for a ball-end `r₁ = r` exactly.
///
/// **[REPO]** guard for the case the paper never mentions (extraction gap 5):
/// where `k_s + 1/r ≤ 0` — for a ball-end, the local gouge condition — the
/// magnitude is clamped to the smallest valid magnitude on the region and the
/// triangle is counted in [`FieldReport::clamped_magnitude_triangles`].
fn build_target_field(
    region: &RegionMesh,
    curvature: &Curvature,
    directions: &[V3],
    params: &FieldParams,
    report: &mut FieldReport,
) -> Vec<V3> {
    let inv_r = if params.ball_radius_mm > EPS_DENOM {
        1.0 / params.ball_radius_mm
    } else {
        0.0
    };
    let mut perp: Vec<V3> = Vec::with_capacity(region.tris.len());
    let mut magnitude: Vec<Option<f64>> = Vec::with_capacity(region.tris.len());

    for t in 0..region.tris.len() {
        let n = region.normal(t);
        let d = directions.get(t).copied().unwrap_or_else(V3::zeros);
        // D rotated 90° about the surface normal (Eq. 12's D⁹⁰°). The step-over
        // direction; also the direction k_s is measured along, because k_s is
        // the normal curvature PERPENDICULAR to the feed direction.
        let rotated = n.cross(&d);
        let unit = if rotated.norm() > EPS_VEC {
            rotated.normalize()
        } else {
            V3::zeros()
        };
        let k_s = normal_curvature(region, curvature, t, unit);
        let denom = k_s + inv_r;
        perp.push(unit);
        magnitude.push(if denom > EPS_DENOM {
            Some((denom / 8.0).sqrt())
        } else {
            None
        });
    }

    let min_valid = magnitude
        .iter()
        .filter_map(|m| *m)
        .fold(f64::INFINITY, f64::min);
    // If nothing on the region is valid the whole region is deeper-concave than
    // the tool; fall back to the flat-surface magnitude and count every
    // triangle as clamped, so the report cannot read healthy.
    let fallback = if min_valid.is_finite() {
        min_valid
    } else {
        (inv_r / 8.0).sqrt()
    };

    let mut field: Vec<V3> = Vec::with_capacity(region.tris.len());
    for (m, u) in magnitude.iter().zip(perp.iter()) {
        let mag = match m {
            Some(v) => *v,
            None => {
                report.clamped_magnitude_triangles += 1;
                fallback
            }
        };
        field.push(*u * mag);
    }
    summarise_magnitudes(&field, report);
    field
}

fn summarise_magnitudes(field: &[V3], report: &mut FieldReport) {
    let mut min = f64::INFINITY;
    let mut max = 0.0_f64;
    let mut sum = 0.0_f64;
    for v in field {
        let m = v.norm();
        min = min.min(m);
        max = max.max(m);
        sum += m;
    }
    report.min_target_magnitude = if min.is_finite() { min } else { 0.0 };
    report.max_target_magnitude = max;
    report.mean_target_magnitude = if field.is_empty() {
        0.0
    } else {
        sum / field.len() as f64
    };
}

// ---------------------------------------------------------------------------
// Poisson assembly (Eqs. 16-17) and matrix-free CG
// ---------------------------------------------------------------------------

/// The assembled system, matrix-free: per-vertex neighbour lists of cotangent
/// weights plus the row sums.
pub(crate) struct SparseLaplacian {
    /// `(neighbour, w_ij)` per vertex, ascending by neighbour for determinism.
    pub(crate) nbr: Vec<Vec<(usize, f64)>>,
    /// `A_ii = Σ_j w_ij`.
    pub(crate) diag: Vec<f64>,
}

/// Cotangent of the angle at `apex` in the triangle `(apex, a, b)`.
fn cotangent(apex: P3, a: P3, b: P3) -> f64 {
    let u = a - apex;
    let v = b - apex;
    let cross = u.cross(&v).norm();
    if cross <= EPS_VEC {
        0.0
    } else {
        u.dot(&v) / cross
    }
}

/// Assemble `Δφ = ∇·V` (Eq. 15) over the region.
///
/// **[SOURCE Eq. 16]** cotangent Laplacian
/// `(Δφ)_i = 1/(2A_i) Σ_j (cot α_ij + cot β_ij)(φ_j − φ_i)`.
/// **[SOURCE Eq. 17 + REPO resolution]** divergence
/// `(∇·V)_i = 1/(2A_i) Σ_k cot θ₁ (e₁·V_k) + cot θ₂ (e₂·V_k)`. The paper sums
/// over incident triangles `k` but writes `V_j` inside the summand
/// (extraction gap 10); read here as the standard cotangent divergence of
/// Botsch et al. — the paper's own ref [27] — with `e₁`, `e₂` the two edges of
/// the incident triangle emanating from `i` and `θ₁`, `θ₂` the angles opposite
/// them.
///
/// **The Voronoi areas cancel.** Eqs. 16 and 17 both carry the same
/// `1/(2A_i)`, so setting them equal leaves
/// `Σ_j w_ij (φ_i − φ_j) = −(integrated divergence)_i` with
/// `w_ij = cot α_ij + cot β_ij` and no area weighting anywhere. That is why
/// this function computes no mixed-Voronoi areas: they would multiply both
/// sides by the same number. The matrix that remains is the Dirichlet-energy
/// Hessian, hence positive semi-definite by construction (its per-triangle
/// blocks are Gram matrices of the linear basis gradients), so negative
/// cotangents from obtuse triangles cannot make it indefinite.
#[allow(clippy::indexing_slicing)]
// SAFETY: `c` holds local vertex indices produced by `build_region_mesh`, and
// `rhs`/`weights` are sized from the same local vertex count.
pub(crate) fn assemble_poisson(region: &RegionMesh, target: &[V3]) -> (SparseLaplacian, Vec<f64>) {
    let nv = region.verts.len();
    let mut weights: HashMap<(usize, usize), f64> = HashMap::new();
    let mut rhs = vec![0.0_f64; nv];

    for t in 0..region.tris.len() {
        if region.area(t) < MIN_TRIANGLE_AREA_MM2 {
            continue;
        }
        let c = region.corners(t);
        let p = [region.point(c[0]), region.point(c[1]), region.point(c[2])];
        let v = target.get(t).copied().unwrap_or_else(V3::zeros);
        for i in 0..3 {
            let j = (i + 1) % 3;
            let k = (i + 2) % 3;
            // Eq. 16: the angle at k is opposite edge (i, j).
            let cot_k = cotangent(p[k], p[i], p[j]);
            let key = if c[i] <= c[j] {
                (c[i], c[j])
            } else {
                (c[j], c[i])
            };
            *weights.entry(key).or_insert(0.0) += cot_k;

            // Eq. 17: e₁ = p_j − p_i with opposite angle at k,
            //         e₂ = p_k − p_i with opposite angle at j.
            let cot_j = cotangent(p[j], p[i], p[k]);
            rhs[c[i]] += cot_k * (p[j] - p[i]).dot(&v) + cot_j * (p[k] - p[i]).dot(&v);
        }
    }

    let mut nbr: Vec<Vec<(usize, f64)>> = vec![Vec::new(); nv];
    let mut diag = vec![0.0_f64; nv];
    let mut edges: Vec<((usize, usize), f64)> = weights.into_iter().collect();
    edges.sort_by(|a, b| a.0.cmp(&b.0));
    for ((a, b), w) in edges {
        if a >= nv || b >= nv || a == b {
            continue;
        }
        nbr[a].push((b, w));
        nbr[b].push((a, w));
        diag[a] += w;
        diag[b] += w;
    }
    for list in &mut nbr {
        list.sort_by(|x, y| x.0.cmp(&y.0));
    }
    // `Σ_j w_ij (φ_i − φ_j) = −(integrated divergence)_i` — see the sign
    // derivation in the doc comment.
    for r in &mut rhs {
        *r = -*r;
    }
    (SparseLaplacian { nbr, diag }, rhs)
}

/// Pin one vertex per vertex-connectivity component.
///
/// **[REPO]** Eq. 14 is a pure-Neumann problem, so φ is determined only up to
/// an additive constant per component and the discrete system is rank
/// deficient. The paper states no boundary conditions at all (extraction
/// gap 2). Pinning the lowest-indexed vertex of each component to zero is the
/// standard fix and keeps the remaining system SPD.
#[allow(clippy::indexing_slicing)]
// SAFETY: union-find arrays are sized from the local vertex count and every
// index below comes from `RegionMesh::corners`, which returns local indices.
fn pin_one_vertex_per_component(region: &RegionMesh, report: &mut FieldReport) -> Vec<bool> {
    let nv = region.verts.len();
    let mut parent: Vec<usize> = (0..nv).collect();
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    for t in 0..region.tris.len() {
        let c = region.corners(t);
        for i in 1..3 {
            if c[0] >= nv || c[i] >= nv {
                continue;
            }
            let (ra, rb) = (find(&mut parent, c[0]), find(&mut parent, c[i]));
            if ra != rb {
                parent[rb] = ra;
            }
        }
    }
    let mut pinned = vec![false; nv];
    let mut seen: HashMap<usize, usize> = HashMap::new();
    for (v, slot) in pinned.iter_mut().enumerate() {
        let root = find(&mut parent, v);
        if let std::collections::hash_map::Entry::Vacant(e) = seen.entry(root) {
            e.insert(v);
            *slot = true;
        }
    }
    report.vertex_components = seen.len();
    pinned
}

/// Outcome of the matrix-free CG solve.
pub(crate) struct CgOutcome {
    pub(crate) x: Vec<f64>,
    pub(crate) iterations: usize,
    pub(crate) residual: f64,
    pub(crate) converged: bool,
}

fn matvec(lap: &SparseLaplacian, x: &[f64], pinned: &[bool], out: &mut [f64]) {
    for (i, slot) in out.iter_mut().enumerate() {
        if pinned.get(i).copied().unwrap_or(false) {
            *slot = 0.0;
            continue;
        }
        let mut acc = lap.diag.get(i).copied().unwrap_or(0.0) * x.get(i).copied().unwrap_or(0.0);
        if let Some(list) = lap.nbr.get(i) {
            for &(j, w) in list {
                acc -= w * x.get(j).copied().unwrap_or(0.0);
            }
        }
        *slot = acc;
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

fn norm(a: &[f64]) -> f64 {
    dot(a, a).sqrt()
}

/// Jacobi-preconditioned conjugate gradient, matrix-free over the adjacency
/// lists, with the pinned rows held at zero (Dirichlet).
///
/// Deterministic: a fixed iteration cap, a fixed tolerance, and no allocation
/// inside the loop. This mirrors [`crate::scallop_isofield`]'s reason for
/// hand-rolling its own solver — no dependency, no heap surprises.
pub(crate) fn cg_solve(
    lap: &SparseLaplacian,
    rhs: &[f64],
    pinned: &[bool],
    params: &FieldParams,
) -> CgOutcome {
    let n = rhs.len();
    let mut x = vec![0.0_f64; n];
    let mut r: Vec<f64> = rhs
        .iter()
        .enumerate()
        .map(|(i, v)| {
            if pinned.get(i).copied().unwrap_or(false) {
                0.0
            } else {
                *v
            }
        })
        .collect();
    let b_norm = norm(&r);
    if b_norm <= f64::MIN_POSITIVE {
        return CgOutcome {
            x,
            iterations: 0,
            residual: 0.0,
            converged: true,
        };
    }
    let precondition = |lap: &SparseLaplacian, r: &[f64], z: &mut [f64]| {
        for (i, slot) in z.iter_mut().enumerate() {
            if pinned.get(i).copied().unwrap_or(false) {
                *slot = 0.0;
                continue;
            }
            let d = lap.diag.get(i).copied().unwrap_or(0.0);
            let d = if d.abs() > EPS_DENOM { d.abs() } else { 1.0 };
            *slot = r.get(i).copied().unwrap_or(0.0) / d;
        }
    };

    let mut z = vec![0.0_f64; n];
    precondition(lap, &r, &mut z);
    let mut p = z.clone();
    let mut rz = dot(&r, &z);
    let mut ap = vec![0.0_f64; n];
    let mut iterations = 0_usize;
    let mut residual = norm(&r) / b_norm;

    while iterations < params.cg_max_iters && residual > params.cg_rel_tolerance {
        matvec(lap, &p, pinned, &mut ap);
        let denom = dot(&p, &ap);
        // Break on NaN as well as on a vanishing denominator.
        if denom.is_nan() || denom.abs() <= f64::MIN_POSITIVE {
            break;
        }
        let alpha = rz / denom;
        for (xi, pi) in x.iter_mut().zip(p.iter()) {
            *xi += alpha * pi;
        }
        for (ri, api) in r.iter_mut().zip(ap.iter()) {
            *ri -= alpha * api;
        }
        precondition(lap, &r, &mut z);
        let rz_next = dot(&r, &z);
        // Break on NaN as well as on a vanishing curvature product.
        if rz.is_nan() || rz.abs() <= f64::MIN_POSITIVE {
            break;
        }
        let beta = rz_next / rz;
        for (pi, zi) in p.iter_mut().zip(z.iter()) {
            *pi = zi + beta * *pi;
        }
        rz = rz_next;
        iterations += 1;
        residual = norm(&r) / b_norm;
    }

    CgOutcome {
        converged: residual <= params.cg_rel_tolerance,
        x,
        iterations,
        residual,
    }
}

// ---------------------------------------------------------------------------
// Marching triangles for an arbitrary per-vertex scalar
// ---------------------------------------------------------------------------

/// One iso-level crossing on a mesh edge.
#[derive(Debug, Clone, Copy)]
struct Crossing {
    /// Edge identity (local vertex ids). Both triangles sharing the edge
    /// compute the identical crossing, so this is the chaining node.
    key: EdgeKey,
    /// World position on the surface.
    pt: P3,
    /// The local triangle that emitted this crossing.
    tri: usize,
}

/// The result of marching one level.
struct MarchResult {
    segments: Vec<(EdgeKey, EdgeKey)>,
    positions: HashMap<EdgeKey, P3>,
    crossings: Vec<Crossing>,
    /// Triangles the level genuinely passed through that did not produce
    /// exactly two crossings — saddles through a vertex and degenerate
    /// configurations. **Counted, not dropped.**
    degenerate: usize,
}

/// Marching triangles for an arbitrary per-vertex scalar `phi` at `level`.
///
/// The mechanism follows `crest_lines::march_valley_segments`
/// (`crest_lines.rs:588-661`), which does this for the valley predicate, with
/// two differences the F1 contract requires: it is generic over the scalar,
/// and a triangle with `crossings.len() != 2` is **counted** rather than
/// silently skipped (`crest_lines.rs:639-641` skips).
///
/// A triangle whose three values are all strictly on one side of the level is
/// not "degenerate" — the level does not pass through it at all — so the count
/// only rises for a triangle with values on both sides that still failed to
/// produce a clean pair of crossings.
#[allow(clippy::indexing_slicing)]
// SAFETY: `c`/`d` are fixed 3-element arrays indexed by the 0..3 corner
// indices of `TRI_EDGES`; `c` holds local vertex indices from
// `build_region_mesh`.
fn march_iso_segments(region: &RegionMesh, phi: &[f64], level: f64) -> MarchResult {
    let mut segments: Vec<(EdgeKey, EdgeKey)> = Vec::new();
    let mut positions: HashMap<EdgeKey, P3> = HashMap::new();
    let mut crossings_out: Vec<Crossing> = Vec::new();
    let mut degenerate = 0_usize;

    for t in 0..region.tris.len() {
        let c = region.corners(t);
        let d = [
            phi.get(c[0]).copied().unwrap_or(0.0) - level,
            phi.get(c[1]).copied().unwrap_or(0.0) - level,
            phi.get(c[2]).copied().unwrap_or(0.0) - level,
        ];
        let has_pos = d.iter().any(|&x| x > 0.0);
        let has_neg = d.iter().any(|&x| x < 0.0);
        if !(has_pos && has_neg) {
            continue;
        }

        let mut found: Vec<Crossing> = Vec::new();
        for &(a, b) in &TRI_EDGES {
            let (da, db) = (d[a], d[b]);
            if !((da > 0.0 && db < 0.0) || (da < 0.0 && db > 0.0)) {
                continue;
            }
            let denom = da - db;
            if denom.abs() < 1e-30 {
                continue;
            }
            let s = da / denom;
            let pa = region.point(c[a]);
            let pb = region.point(c[b]);
            let pt = P3::new(
                pa.x + (pb.x - pa.x) * s,
                pa.y + (pb.y - pa.y) * s,
                pa.z + (pb.z - pa.z) * s,
            );
            found.push(Crossing {
                key: EdgeKey::new(c[a] as u32, c[b] as u32),
                pt,
                tri: t,
            });
        }

        if found.len() != 2 {
            degenerate += 1;
            continue;
        }
        let (c0, c1) = (found[0], found[1]);
        positions.entry(c0.key).or_insert(c0.pt);
        positions.entry(c1.key).or_insert(c1.pt);
        if c0.key != c1.key {
            segments.push((c0.key, c1.key));
            crossings_out.push(c0);
            crossings_out.push(c1);
        }
    }

    MarchResult {
        segments,
        positions,
        crossings: crossings_out,
        degenerate,
    }
}

/// Chain iso-level segments into polylines.
///
/// Connectivity is by **edge identity**, not by proximity: two triangles
/// sharing an edge compute the same crossing point and the same
/// [`EdgeKey`], so chaining is exact. [`CHAIN_EPS`] is used only to decide
/// whether a finished chain closed on itself, matching the tolerance the
/// repo's 2D marching-squares chainer uses for the same question.
/// Start-node order is sorted, as in `crest_lines::chain_segments`, because
/// `HashMap` iteration order is not deterministic (the T14 determinism fix).
fn chain_iso_segments(
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
                let mut options: Vec<EdgeKey> = nbrs
                    .iter()
                    .copied()
                    .filter(|&n| n != prev && !seen(visited, cur, n))
                    .collect();
                options.sort_unstable();
                options.first().copied()
            });
            let Some(n) = next else {
                break;
            };
            mark(visited, cur, n);
            chain.push(pos(&n));
            prev = cur;
            cur = n;
        }
        chain
    };

    let mut lines: Vec<Vec<P3>> = Vec::new();
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
    lines.retain(|l| l.len() >= 2);
    lines
}

/// Whether a chained polyline closed on itself.
fn is_closed(line: &[P3]) -> bool {
    match (line.first(), line.last()) {
        (Some(a), Some(b)) => line.len() > 2 && (b - a).norm() <= CHAIN_EPS,
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Level schedule (Eqs. 7-9, §3.3) and extraction
// ---------------------------------------------------------------------------

/// Gradient of the piecewise-linear `phi` on one triangle.
#[allow(clippy::indexing_slicing)]
// SAFETY: `c` holds local vertex indices produced by `build_region_mesh` and
// the 0..3 literals index its fixed-size array.
fn triangle_gradient(region: &RegionMesh, phi: &[f64], tri: usize) -> V3 {
    let area = region.area(tri);
    if area < MIN_TRIANGLE_AREA_MM2 {
        return V3::zeros();
    }
    let c = region.corners(tri);
    let p = [region.point(c[0]), region.point(c[1]), region.point(c[2])];
    let f = [
        phi.get(c[0]).copied().unwrap_or(0.0),
        phi.get(c[1]).copied().unwrap_or(0.0),
        phi.get(c[2]).copied().unwrap_or(0.0),
    ];
    let n = region.normal(tri);
    let mut acc = V3::zeros();
    for (i, fi) in f.iter().enumerate() {
        let j = (i + 1) % 3;
        let k = (i + 2) % 3;
        acc += n.cross(&(p[k] - p[j])) * *fi;
    }
    acc / (2.0 * area)
}

/// The level increment that yields scallop `h` at one point.
///
/// **Derivation from [SOURCE] Eqs. 7-9.** Eq. 7 relates a level increment to a
/// side-step along `∇φ`: `|l₂ − l₁| = ‖∇φ‖·‖p₂ − p₁‖`. The side-step that
/// yields scallop `h` for a ball-end is Eq. 2/Eq. 5 with `r₁ = r₂ = r`:
/// `‖p₂ − p₁‖ = √(8h/(k_s + 1/r)) = √h / √((k_s + 1/r)/8) = √h / ‖V‖`.
/// Substituting gives
///
/// ```text
/// |Δl| = ‖∇φ‖ · √h / ‖V‖
/// ```
///
/// which is Eq. 9's `|Δl| = √h` exactly when the solve achieved
/// `‖∇φ‖ = ‖V‖`, and corrects for the shortfall where it did not — the
/// constraint is soft (§3.2, §5.2), so this correction is doing real work.
fn increment_at(grad_norm: f64, target_norm: f64, scallop_h_mm: f64) -> Option<f64> {
    if target_norm <= EPS_VEC || scallop_h_mm <= 0.0 {
        return None;
    }
    Some(grad_norm * scallop_h_mm.sqrt() / target_norm)
}

/// Run the level schedule and extract every iso-level curve.
///
/// **[SOURCE §3.3]** the increment is evaluated at points of the current curve
/// and the **smallest** is taken (the paper's conservative rule).
/// **[REPO]** the point set is *every* edge crossing of the curve, not "a
/// certain number of points" (extraction gap 3), the first level is
/// `min φ + ½·increment`, and the increment carries a floor so a near-zero
/// gradient cannot stall the schedule.
fn extract_levels(
    region: &RegionMesh,
    phi: &[f64],
    target: &[V3],
    params: &FieldParams,
    result: &mut FieldPathResult,
    report: &mut FieldReport,
) {
    let mut phi_min = f64::INFINITY;
    let mut phi_max = f64::NEG_INFINITY;
    for &v in phi {
        phi_min = phi_min.min(v);
        phi_max = phi_max.max(v);
    }
    if !phi_min.is_finite() || !phi_max.is_finite() {
        return;
    }
    let range = phi_max - phi_min;
    if range <= f64::MIN_POSITIVE {
        return;
    }
    let floor = (params.min_level_increment_fraction * range).max(f64::MIN_POSITIVE);

    let grad_norm: Vec<f64> = (0..region.tris.len())
        .map(|t| triangle_gradient(region, phi, t).norm())
        .collect();
    let target_norm: Vec<f64> = target.iter().map(V3::norm).collect();

    // First increment: no curve exists yet, so take the conservative minimum
    // over the whole region.
    let mut increment = grad_norm
        .iter()
        .zip(target_norm.iter())
        .filter_map(|(g, m)| increment_at(*g, *m, params.scallop_h_mm))
        .filter(|x| x.is_finite() && *x > 0.0)
        .fold(f64::INFINITY, f64::min);
    if !increment.is_finite() || increment < floor {
        if increment.is_finite() {
            report.floored_increments += 1;
        }
        increment = floor;
    }

    let mut level = phi_min + 0.5 * increment;
    while level < phi_max {
        if result.levels.len() >= params.max_levels {
            report.level_cap_hit = true;
            break;
        }
        let march = march_iso_segments(region, phi, level);
        report.degenerate_crossings += march.degenerate;
        let lines = chain_iso_segments(&march.segments, &march.positions);
        let level_idx = result.levels.len();
        result.levels.push(level);
        report.level_component_counts.push(lines.len());
        for line in lines {
            if is_closed(&line) {
                report.closed_loops += 1;
            }
            result.polylines.push(line);
            result.polyline_levels.push(level_idx);
        }

        // §3.3 step 2-3: the increment for the NEXT curve, from every crossing
        // of this one, minimum taken.
        let mut next = f64::INFINITY;
        for crossing in &march.crossings {
            let g = grad_norm.get(crossing.tri).copied().unwrap_or(0.0);
            let m = target_norm.get(crossing.tri).copied().unwrap_or(0.0);
            if let Some(inc) = increment_at(g, m, params.scallop_h_mm)
                && inc.is_finite()
                && inc > 0.0
            {
                next = next.min(inc);
            }
        }
        if !next.is_finite() {
            // No usable crossing on this level (the curve missed every
            // triangle with a target magnitude); keep the previous increment.
            next = increment;
        }
        if next < floor {
            report.floored_increments += 1;
            next = floor;
        }
        increment = next;
        level += increment;
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::{
        FieldParams, FieldReport, RegionMesh, all_triangles, build_region_mesh, is_closed,
        march_iso_segments, solve_field_paths_with, solve_paths_with_target, triangles_where,
    };
    use crate::geo::{P3, V3};
    use crate::mesh::{TriangleMesh, make_test_flat, make_test_hemisphere};

    const BALL_R: f64 = 3.0;
    const SCALLOP_H: f64 = 0.03;

    /// The classic ball-end iso-scallop side-step (Eq. 2 with `r₁ = r₂ = r`):
    /// `s = √(8h / (k_s + 1/r))`.
    fn iso_scallop_side_step(k_s: f64, r: f64, h: f64) -> f64 {
        (8.0 * h / (k_s + 1.0 / r)).sqrt()
    }

    /// Distance from `p` to the segment `a`-`b`.
    fn point_to_segment(p: P3, a: P3, b: P3) -> f64 {
        let ab = b - a;
        let len2 = ab.norm_squared();
        if len2 <= 1e-18 {
            return (p - a).norm();
        }
        let t = ((p - a).dot(&ab) / len2).clamp(0.0, 1.0);
        (p - (a + ab * t)).norm()
    }

    /// Distance from `p` to a polyline.
    fn point_to_polyline(p: P3, line: &[P3]) -> f64 {
        line.windows(2)
            .map(|w| point_to_segment(p, w[0], w[1]))
            .fold(f64::INFINITY, f64::min)
    }

    fn median(mut values: Vec<f64>) -> f64 {
        values.sort_by(f64::total_cmp);
        if values.is_empty() {
            return f64::NAN;
        }
        values[values.len() / 2]
    }

    /// Median distance between consecutive extracted iso-curves, measured
    /// geometrically (not from the level increment, which would be circular).
    fn median_curve_spacing(
        result: &super::FieldPathResult,
        report: &FieldReport,
        skip_first: usize,
        skip_last: usize,
    ) -> f64 {
        let n = report.level_component_counts.len();
        let mut spacings = Vec::new();
        let upper = n.saturating_sub(skip_last);
        for li in skip_first..upper.saturating_sub(1) {
            let a = result.polylines_at(li);
            let b = result.polylines_at(li + 1);
            let (Some(prev), Some(next)) = (a.first(), b.first()) else {
                continue;
            };
            if prev.len() < 2 || next.len() < 2 {
                continue;
            }
            let mid = next[next.len() / 2];
            let d = point_to_polyline(mid, prev);
            if d.is_finite() {
                spacings.push(d);
            }
        }
        median(spacings)
    }

    // -----------------------------------------------------------------------
    // Flat fixture: k_s = 0 everywhere, so the whole pipeline has a closed
    // form. Both triangles are coplanar and singular, so `V` is one constant
    // vector; `φ = V·x` is then in the piecewise-linear space and is the exact
    // zero-residual minimiser of Eq. 14, so `‖∇φ‖ = ‖V‖` exactly.
    // -----------------------------------------------------------------------

    #[test]
    fn flat_region_target_field_is_curvature_free_and_never_clamps() {
        let mesh = make_test_flat(100.0);
        let params = FieldParams::new(BALL_R, SCALLOP_H);
        let (_result, report) = solve_field_paths_with(&mesh, &all_triangles(&mesh), &params);

        assert_eq!(report.region_triangles, 2);
        assert_eq!(report.vertex_components, 1);
        // A flat region is singular everywhere (§4.2), so the copy-transport
        // path carries the seed direction to the second triangle.
        assert_eq!(report.degenerate_triangles, 2);
        assert_eq!(report.transported_triangles, 1);
        assert_eq!(report.clamped_magnitude_triangles, 0);
        assert_eq!(report.orientation_inconsistencies, 0);
        assert_eq!(report.unoriented_triangles, 0);

        // k_s = 0 ⇒ ‖V‖ = √(1/(8r)), uniform.
        let expected = (1.0 / (8.0 * BALL_R)).sqrt();
        assert!(
            (report.min_target_magnitude - expected).abs() < 1e-9,
            "min |V| {} vs expected {expected}",
            report.min_target_magnitude
        );
        assert!(
            (report.max_target_magnitude - expected).abs() < 1e-9,
            "max |V| {} vs expected {expected}",
            report.max_target_magnitude
        );
    }

    #[test]
    fn flat_region_cg_converges_inside_its_own_cap() {
        let mesh = make_test_flat(100.0);
        let params = FieldParams::new(BALL_R, SCALLOP_H);
        let (_result, report) = solve_field_paths_with(&mesh, &all_triangles(&mesh), &params);
        assert!(
            report.cg_converged,
            "CG did not converge: residual {} after {} iterations",
            report.cg_residual, report.cg_iterations
        );
        assert!(report.cg_residual <= params.cg_rel_tolerance);
        assert!(report.cg_iterations < params.cg_max_iters);
    }

    #[test]
    fn flat_region_iso_curves_are_single_lines_at_iso_scallop_spacing() {
        let mesh = make_test_flat(100.0);
        let params = FieldParams::new(BALL_R, SCALLOP_H);
        let (result, report) = solve_field_paths_with(&mesh, &all_triangles(&mesh), &params);

        assert!(
            report.level_component_counts.len() > 20,
            "expected many levels across a 100 mm square, got {}",
            report.level_component_counts.len()
        );
        assert!(!report.level_cap_hit);
        // A plane has no saddles and no vertex-degenerate crossings.
        assert_eq!(report.degenerate_crossings, 0);
        assert_eq!(report.floored_increments, 0);
        // Every level is one straight chord across the square.
        assert!(
            report.level_component_counts.iter().all(|&c| c == 1),
            "component counts were not all 1: {:?}",
            report.level_component_counts
        );
        assert_eq!(report.closed_loops, 0);

        // Spacing: √(8hr) for k_s = 0.
        let expected = iso_scallop_side_step(0.0, BALL_R, SCALLOP_H);
        let measured = median_curve_spacing(&result, &report, 0, 0);
        assert!(
            (measured - expected).abs() / expected < 0.10,
            "median spacing {measured} vs closed-form {expected}"
        );
    }

    // -----------------------------------------------------------------------
    // Hemisphere, real pipeline: umbilic everywhere (κ₁ = κ₂ = 1/R), so the
    // direction field is singular everywhere and the copy-transport path is
    // exercised. Curvature magnitudes are closed-form; the extracted geometry
    // is NOT, because transported directions on a sphere carry holonomy — the
    // paper's own caveat (§4.2). Closed-form geometry is asserted on the
    // analytic-field fixture below instead.
    // -----------------------------------------------------------------------

    #[test]
    fn hemisphere_reads_umbilic_curvature_and_never_clamps() {
        let radius = 10.0;
        let mesh = make_test_hemisphere(radius, 20);
        let params = FieldParams::new(BALL_R, SCALLOP_H);
        let (result, report) = solve_field_paths_with(&mesh, &all_triangles(&mesh), &params);

        assert!(report.region_triangles > 100);
        assert_eq!(report.vertex_components, 1);
        assert_eq!(report.unoriented_triangles, 0);
        // Umbilic ⇒ no preferred direction anywhere, so directions are copied.
        assert!(
            report.degenerate_triangles > 0,
            "an umbilic sphere should read singular somewhere"
        );
        assert!(
            report.transported_triangles > 0,
            "the unfold-translate-fold copy path was never exercised"
        );
        // A convex dome has k_s = +1/R > 0, so k_s + 1/r is never non-positive.
        assert_eq!(report.clamped_magnitude_triangles, 0);

        // ‖V‖ = √((1/R + 1/r)/8), uniform up to the discrete curvature
        // estimate's error on a coarse lat-long tessellation.
        let expected = ((1.0 / radius + 1.0 / BALL_R) / 8.0).sqrt();
        let err = (report.mean_target_magnitude - expected).abs() / expected;
        assert!(
            err < 0.25,
            "mean |V| {} vs closed-form {expected} (relative error {err})",
            report.mean_target_magnitude
        );

        assert!(
            report.cg_converged,
            "CG did not converge: residual {} after {} iterations",
            report.cg_residual, report.cg_iterations
        );
        assert!(report.cg_iterations < params.cg_max_iters);
        assert!(
            !result.polylines.is_empty(),
            "no iso-level curves extracted on the hemisphere"
        );
        assert_eq!(result.phi.len(), report.region_vertices);
    }

    // -----------------------------------------------------------------------
    // Hemisphere, analytic target field: V = m · (unit meridian direction),
    // m = √((1/R + 1/r)/8). That field is exactly the gradient of
    // φ = m·R·latitude, so the closed-form claims (one closed loop per level,
    // spacing = √(8h/(1/R + 1/r))) are theorems up to mesh resolution rather
    // than hopes about a transported direction field. The apex is excluded so
    // the meridian direction is well defined on the whole region.
    // -----------------------------------------------------------------------

    fn hemisphere_annulus(mesh: &TriangleMesh, radius: f64) -> Vec<u32> {
        triangles_where(mesh, |_, c| {
            let rho = (c.x * c.x + c.y * c.y).sqrt();
            rho > 0.25 * radius
        })
    }

    #[test]
    fn hemisphere_analytic_meridian_field_gives_closed_iso_scallop_rings() {
        let radius = 10.0;
        let mesh = make_test_hemisphere(radius, 36);
        let region = hemisphere_annulus(&mesh, radius);
        assert!(
            region.len() > 500,
            "annulus region too small: {}",
            region.len()
        );

        let magnitude = ((1.0 / radius + 1.0 / BALL_R) / 8.0).sqrt();
        let params = FieldParams::new(BALL_R, SCALLOP_H);
        let (result, report) = solve_paths_with_target(
            &mesh,
            &region,
            |_, c: P3, n: V3| {
                // Unit meridian direction (increasing latitude) at the
                // centroid, projected into the triangle plane.
                let p = V3::new(c.x, c.y, c.z);
                let len = p.norm();
                if len <= 1e-12 {
                    return V3::zeros();
                }
                let unit = p / len;
                let up = V3::new(0.0, 0.0, 1.0);
                let meridian = up - unit * up.dot(&unit);
                if meridian.norm() <= 1e-9 {
                    return V3::zeros();
                }
                let tangent = meridian.normalize();
                let flat = tangent - n * tangent.dot(&n);
                if flat.norm() <= 1e-9 {
                    return V3::zeros();
                }
                flat.normalize() * magnitude
            },
            &params,
        );

        assert_eq!(report.vertex_components, 1);
        assert!(
            report.cg_converged,
            "CG did not converge: residual {} after {} iterations",
            report.cg_residual, report.cg_iterations
        );
        assert!(report.cg_iterations < params.cg_max_iters);
        assert!(
            (report.mean_target_magnitude - magnitude).abs() / magnitude < 1e-6,
            "injected |V| was not preserved: {}",
            report.mean_target_magnitude
        );

        let levels = report.level_component_counts.len();
        assert!(levels > 8, "expected several latitude rings, got {levels}");
        assert!(
            report.level_component_counts.iter().all(|&c| c >= 1),
            "a level extracted no curve: {:?}",
            report.level_component_counts
        );
        let singles = report
            .level_component_counts
            .iter()
            .filter(|&&c| c == 1)
            .count();
        assert!(
            singles * 10 >= levels * 9,
            "iso-curves of a meridian field should be single latitude rings; \
             {singles} of {levels} levels were, counts {:?}",
            report.level_component_counts
        );
        // Latitude rings close.
        let closed = result.polylines.iter().filter(|l| is_closed(l)).count();
        assert!(
            closed * 10 >= result.polylines.len() * 9,
            "{closed} of {} extracted curves closed",
            result.polylines.len()
        );

        let expected = iso_scallop_side_step(1.0 / radius, BALL_R, SCALLOP_H);
        let measured = median_curve_spacing(&result, &report, 1, 1);
        assert!(
            measured.is_finite() && (measured - expected).abs() / expected < 0.15,
            "median ring spacing {measured} vs closed-form {expected}"
        );
    }

    // -----------------------------------------------------------------------
    // Marching triangles: the saddle/degenerate crossing must be COUNTED.
    // -----------------------------------------------------------------------

    fn two_triangle_fixture() -> (TriangleMesh, RegionMesh) {
        let vertices = vec![
            P3::new(0.0, 0.0, 0.0),
            P3::new(1.0, 0.0, 0.0),
            P3::new(0.0, 1.0, 0.0),
            P3::new(2.0, 0.0, 0.0),
            P3::new(3.0, 0.0, 0.0),
            P3::new(2.0, 1.0, 0.0),
        ];
        let triangles = vec![[0, 1, 2], [3, 4, 5]];
        let mesh = TriangleMesh::from_raw(vertices, triangles);
        let region = build_region_mesh(&mesh, &[0, 1]).expect("region builds");
        (mesh, region)
    }

    #[test]
    fn marching_triangles_emits_a_clean_crossing_pair() {
        let (_mesh, region) = two_triangle_fixture();
        // Triangle 0 straddles the level cleanly; triangle 1 is entirely above.
        let phi = vec![-1.0, 1.0, 1.0, 2.0, 3.0, 4.0];
        let march = march_iso_segments(&region, &phi, 0.0);
        assert_eq!(march.segments.len(), 1);
        assert_eq!(march.crossings.len(), 2);
        assert_eq!(march.degenerate, 0);
    }

    #[test]
    fn marching_triangles_counts_the_vertex_saddle_instead_of_dropping_it() {
        let (_mesh, region) = two_triangle_fixture();
        // Triangle 0: a clean pair. Triangle 1: the level passes exactly
        // through vertex 3 (φ = 0) with the other two on opposite sides, so
        // only ONE edge carries a strict sign change. The old
        // `crossings.len() != 2` skip would drop this silently.
        let phi = vec![-1.0, 1.0, 1.0, 0.0, 1.0, -1.0];
        let march = march_iso_segments(&region, &phi, 0.0);
        assert_eq!(march.segments.len(), 1, "the clean triangle still emits");
        assert_eq!(
            march.degenerate, 1,
            "the vertex-saddle triangle must be counted, not dropped"
        );
    }

    #[test]
    fn marching_triangles_ignores_triangles_the_level_misses() {
        let (_mesh, region) = two_triangle_fixture();
        let phi = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let march = march_iso_segments(&region, &phi, 0.0);
        assert!(march.segments.is_empty());
        assert_eq!(
            march.degenerate, 0,
            "a triangle the level never enters is not degenerate"
        );
    }

    #[test]
    fn empty_region_reports_rather_than_panics() {
        let mesh = make_test_flat(10.0);
        let params = FieldParams::default();
        let (result, report) = solve_field_paths_with(&mesh, &[], &params);
        assert!(result.polylines.is_empty());
        assert_eq!(report.region_triangles, 0);
        assert_eq!(report.total_polylines, 0);
    }
}
