# C. A BVH for mesh ray picks, and its cache (design, 2026-10-28)

Design only. No product file changes. HEAD `b5aa2a45`. The code blocks are
design, not compiled. Scope: REPORT D.1 step 2 (SEL-2).

## Files read

| File | Lines | What it gave |
|---|---|---|
| `crates/rs_cam_core/src/mesh.rs` | 1-48, 340-470, 822 | `TriangleMesh` (`faces: Vec<Triangle>`), `from_raw`, `ray_pick_triangle` :373-392, `SpatialIndex` :411, `make_test_hemisphere` :822 |
| `crates/rs_cam_core/src/geo.rs` | 18-60, 100-240 | `BoundingBox3::ray_intersect` :108 (returns the EXIT t when the origin is inside; parallel threshold `1e-12`), `Triangle` :151 (144 B: 3 `P3` + normal + bbox), `Triangle::ray_intersect` :202 (Möller–Trumbore, barycentric `eps = 1e-10`, `t >= 0`) |
| `crates/rs_cam_viz/src/interaction/picking.rs` | 185-310 | `pick` :199 (plain click: enriched only), `pick_model_face` :233-287, the two `ray_pick_triangle` calls :258 and :274 |
| `crates/rs_cam_core/src/maps/memo.rs` | 1-223 | `MeshMemo` (`Weak` identity, sweep on insert, oldest-first evict at `CAPACITY`), `CacheCounters` |
| `crates/rs_cam_core/src/maps/geom_cache.rs` | 1-200, 270-430 | the identity argument, `CAPACITY = 4` :115, `Entry` :146, `cached_*` builds, `peek_flat_regions` :415 never builds, `LazyIndex` |
| `crates/rs_cam_core/src/geometry/enriched_mesh.rs` | 109-131, 168-177 | `EnrichedMesh.mesh: Arc<TriangleMesh>`, `mesh_arc`, `apply_uniform_scale` makes a NEW `Arc` via `from_raw` |
| `crates/rs_cam_core/src/session/mod.rs` | 644-674, 780-815, 844-885 | `LoadedModel.mesh`, `adopt_geometry` replaces `self.mesh`, `face_items` / `faces_ready` / `face_items_built` |
| `crates/rs_cam_core/src/session/selection_status.rs` | 1909-1980 | the `build_model_faces` job: `capture_*`, `execute_*` |
| `crates/rs_cam_core/src/session/command.rs` | 894, 1231, 2392-2432, 2586, 3108-3122 | the Job row, args, `JobHandle`, `start` |
| `crates/rs_cam_viz/src/controller/events/model_faces.rs` | 1-130 | the faces pump on the Job lane |
| `crates/rs_cam_viz/src/compute/worker.rs` :1464, :1797; `app/mcp/commands.rs` :1809, :2847; `controller/events/compute.rs` :1453; `controller.rs` :254 | — | every site the faces Job row touches |
| `crates/rs_cam_viz/src/controller/workflow_tests.rs` | 297 | `LoadedModel.mesh` and `enriched.mesh_arc()` are one `Arc` |
| `crates/rs_cam_core/tests/common/folder_maps.rs` | 1-46, 215-270 | F140 scans only folders that hold a `CLAUDE.md` |
| `CLAUDE.md` files | root, core, `maps/`, `surface/`, `budget/`, `session/`, `tests/` | layout, sentries, test rules |
| `REPORT.md` | B.1, B.2 SEL-2, C.2 rows 410-411, D.1 step 2, D.2 | the probe numbers and the crate table |
| `probe/src/main.rs`, `probe/run.log` | all | ray grid method; edge ties |
| `design/B_input_and_box_select.md` | 460-500 | `HoverResult.surface_point` needs a ray hit on STL models too |

**The tie reading is correct.** `ray_pick_triangle` walks `mesh.faces` in
index order and replaces the best only on `t < best_t` (strict). So on two
bit-equal `t` values, the lower index wins. Note: the probe prints "ties"
within `1e-9`, not bit-equal. Ray 304 on the terrain shows `[7728, 7729]`
with best `7729`: there, 7729 has the smaller `t` by less than `1e-9`. The
rule "lowest index wins" holds only for bit-equal `t`.

## 1. Placement

| Option | Paths that change | Root-file rule | Verdict |
|---|---|---|---|
| A. `src/mesh/bvh.rs`, owned by `src/mesh.rs` with `pub mod bvh;` (no `mod.rs`) | none: every `crate::mesh::…` path stays | `mesh.rs` stays one of the 8 root files | **Recommended** |
| B. `mesh.rs` → `mesh/mod.rs` + `mesh/bvh.rs` | none | the spine loses a root file; core `CLAUDE.md` text changes | No gain over A |
| C. New `src/spatial/` with the BVH; `SpatialIndex` stays | none now | — | A folder of one file; the BVH is far from its mesh |
| D. `src/spatial/` with the BVH AND `SpatialIndex` | 190 files name `mesh::SpatialIndex` (`rg -l`) | — | Large blast radius for no pick gain; park (Q4) |

Rust 2018 module layout lets `src/mesh.rs` own `src/mesh/bvh.rs`. The
folder `mesh/` then holds one file. F140 scans only folders that hold a
`CLAUDE.md`, but the operator rule is one map per folder. Add a short map:

```text
crates/rs_cam_core/src/mesh/CLAUDE.md  (new, about 15 lines)
# `mesh/` — the children of `mesh.rs`
## Files
- `bvh.rs` — `MeshBvh`, the ray-pick BVH over a `TriangleMesh` (SEL-2).
  The parent `mesh.rs` keeps `TriangleMesh`, `ray_pick_triangle` and
  `SpatialIndex`.
## Invariants
- `MeshBvh::cast_ray` equals `ray_pick_triangle` on every ray the sentry
  casts: the same `Triangle::ray_intersect`, the same gate, lowest index on
  a tie. The pad proof covers hits with `det` well above `RAY_BARY_EPS`; a
  graze near that floor is the known limit, not a broken invariant.
- A node box is padded by `RAY_PAD` (derived from `RAY_BARY_EPS`); it never
  rejects a triangle that the triangle test accepts.
- No `HashMap` and no rayon in the build.
## Sentries
- `cargo test -p rs_cam_core -q --test bvh_pick_equals_brute_force_sel2 --test a_ray_on_a_shared_edge_takes_the_lower_index_sel2`
- `cargo test -p rs_cam_core -q --lib mesh::bvh`
```

Core `CLAUDE.md` changes: line 9 "8 files and 27 folders" becomes "8 files
and 28 folders", and the table gains one row:

```text
| `mesh/` | The children of `mesh.rs`: the ray-pick BVH | `mesh/CLAUDE.md` |
```

The cache goes in `maps/geom_cache.rs` (section 3). The session door goes
in `session/mod.rs`; the job goes in a new `session/pick_index.rs`.

## 2. The type

### 2.1 Constants (derived, not typed from memory)

```rust
// crates/rs_cam_core/src/geo.rs — the triangle test names its tolerance.
/// The barycentric tolerance of `Triangle::ray_intersect`. The BVH pad
/// derives from it, so the two cannot drift.
pub const RAY_BARY_EPS: f64 = 1e-10;   // today's literal `let eps = 1e-10;` at :203
```

Pad derivation. The triangle test accepts the point
`P = w0·v0 + w1·v1 + w2·v2` with `w1 = u >= -ε`, `w2 = v >= -ε`,
`w0 = 1 - u - v >= -ε`, and `Σw = 1`. On one axis `a`,
`P_a - max_i x_i = Σ w_i (x_i - max)`. A term with `w_i >= 0` is at most 0.
At most two weights are negative, and each such term is at most
`ε · extent_a(triangle)`. So `P` lies at most `2ε · extent_a` outside the
triangle box. A node extent is at least each triangle extent. The design
takes `4ε · node_extent_a` (factor 2 for the rounding of `u`, `v`) plus
`8 ulp` of the largest coordinate magnitude on that axis for the slab
arithmetic:

```rust
const RAY_PAD_REL: f64 = 4.0 * RAY_BARY_EPS;
const RAY_PAD_ULPS: f64 = 8.0 * f64::EPSILON;
fn pad(lo: f64, hi: f64) -> f64 {
    RAY_PAD_REL * (hi - lo) + RAY_PAD_ULPS * lo.abs().max(hi.abs())
}
```

The parity sentry (section 5) is the arbiter of the pad. The tie test adds
one ray that misses the triangle by `0.5ε` in `u`: brute force hits it, so
the BVH must hit it too.

### 2.2 Layout and API

```rust
// crates/rs_cam_core/src/mesh/bvh.rs
use std::mem::size_of;
use crate::geo::{P3, V3, RAY_BARY_EPS};
use crate::mesh::{TriangleMesh, ray_pick_triangle};

/// The leaf size the probe sweep sets (Q2). A tuning dial for speed AND
/// for memory (nodes ≈ 2·leaves − 1).
pub const DEFAULT_LEAF_SIZE: usize = 8;

/// One node. Depth-first pre-order: the left child of an interior node is
/// the next node.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Node {
    lo: [f64; 3],   // padded box
    hi: [f64; 3],
    /// Leaf: the first slot in `order`. Interior: the index of the right child.
    start: u32,
    /// Leaf: the triangle count (> 0). Interior: 0.
    count: u32,
}

/// A bounding-volume hierarchy over the triangles of one `TriangleMesh`,
/// for ray picks. It holds indices only; every cast takes the mesh.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshBvh {
    nodes: Vec<Node>,
    /// Triangle indices, leaf-major.
    order: Vec<u32>,
    /// `mesh.faces.len()` at build. A cast on another mesh falls back.
    triangle_count: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum BvhError {
    #[error("{0} triangles exceed the u32 index of the pick BVH")]
    TooManyTriangles(usize),
}

impl MeshBvh {
    pub fn build(mesh: &TriangleMesh) -> Result<Self, BvhError>;
    pub fn build_with_leaf(mesh: &TriangleMesh, leaf_size: usize) -> Result<Self, BvhError>;

    /// The nearest hit: the SAME contract as `ray_pick_triangle` — nearest
    /// `t`, the lowest triangle index on a bit-equal `t`, `None` on a miss.
    pub fn cast_ray(&self, mesh: &TriangleMesh, origin: &P3, dir: &V3) -> Option<(usize, f64)>;

    /// Every hit, sorted by (`t`, index). For the later "select other" list
    /// (SEL-5 / SEL-9).
    pub fn cast_ray_all(&self, mesh: &TriangleMesh, origin: &P3, dir: &V3) -> Vec<(usize, f64)>;

    /// Later hook for a box select of mesh faces: every triangle whose
    /// padded leaf box is not fully outside one of the planes (a superset;
    /// the caller applies its own rule). Not in the first commit.
    /// `Plane`: a type to add in that commit, or `[f64; 4]`.
    pub fn query_frustum(&self, planes: &[Plane; 6], out: &mut Vec<usize>);

    /// Heap bytes, from `size_of` of the real types (budget rule).
    pub fn heap_bytes(&self) -> usize {
        self.nodes.capacity() * size_of::<Node>() + self.order.capacity() * size_of::<u32>()
    }
    pub fn node_count(&self) -> usize;
}
```

### 2.3 Build

- `n = mesh.faces.len()`; `u32::try_from(n)` else `TooManyTriangles`.
- `n == 0`: return `nodes: Vec::new()` before any `build_node` call. A
  leaf with `count = 0` reads as an interior node whose right child is
  itself, and the cast would loop. Clamp `leaf_size.max(1)`: a leaf size of
  0 recurses without end on a range of one.
- `centroid[i]` = the centre of `mesh.faces[i].bbox`.
- `order = (0..n as u32)`. A recursive `build_node(range)` (depth at most
  `⌈log2 n⌉ ≤ 32`, so recursion is safe):
  1. Push a placeholder node; take the union of `faces[order[k]].bbox` over
     the range; pad each axis by `pad(lo, hi)`.
  2. `range.len() <= leaf_size`: write a leaf; return.
  3. Axis = the longest extent of the centroid bounds (first axis on a tie).
  4. Object median: `order[range].select_nth_unstable_by(len / 2, cmp)` with
     `cmp = |a, b| c[a][axis].total_cmp(&c[b][axis]).then(a.cmp(b))`. The
     comparator is a total order, so the left and right SETS are unique.
  5. Build left, then right; write the interior node with `start = right`.
- A count split always terminates, also when all centroids are equal.
  Leaf sizes fall in `(L/2, L]`.
- Median, not SAH: one ray per frame at a few µs does not pay for SAH build
  time. Revisit only if the probe shows a cast above 0.1 ms.
- Complexity: `O(n log(n/L))` time (one linear select per level);
  `O(n)` memory. Single thread. **No rayon**: a parallel build needs a shared
  node counter, so node indices change from run to run. The results stay
  correct (2.4), but the layout is not reproducible, and the `--lib` test
  pins a reproducible layout. The build runs off the frame (section 3), so
  its time is not on the frame.
- Indexing: the builder uses `get` / `get_mut` where it is cheap; the inner
  select and the leaf write take
  `#[expect(clippy::indexing_slicing, reason = "indices come from 0..n of this mesh and the node vector this builder grew")]`
  on the smallest function.

### 2.4 Cast, and why it equals brute force

```rust
pub fn cast_ray(&self, mesh: &TriangleMesh, origin: &P3, dir: &V3) -> Option<(usize, f64)> {
    if mesh.faces.len() != self.triangle_count {
        return ray_pick_triangle(mesh, origin, dir);   // wrong mesh: keep the contract
    }
    mesh.bbox.ray_intersect(origin, dir)?;             // the SAME gate as brute force
    let inv = V3::new(1.0 / dir.x, 1.0 / dir.y, 1.0 / dir.z);
    let mut best: Option<(usize, f64)> = None;
    let mut stack: Vec<u32> = Vec::with_capacity(64);  // depth ≤ 32
    stack.push(0);
    while let Some(i) = stack.pop() {
        let Some(node) = self.nodes.get(i as usize) else { continue };
        let Some(entry) = slab_entry(node, origin, &inv) else { continue };
        if best.is_some_and(|(_, bt)| entry > bt) {    // STRICT: a tie still enters
            continue;
        }
        if node.count > 0 {
            for &tri in self.leaf(node) {
                let tri = tri as usize;
                if let Some(t) = mesh.faces.get(tri).and_then(|f| f.ray_intersect(origin, dir))
                    && better(t, tri, best)
                {
                    best = Some((tri, t));
                }
            }
        } else {
            stack.push(node.start);  // right
            stack.push(i + 1);       // left, popped first
        }
    }
    best
}

/// The brute-force order as a comparator: nearer, else lower index.
fn better(t: f64, tri: usize, best: Option<(usize, f64)>) -> bool {
    best.is_none_or(|(bi, bt)| t < bt || (t == bt && tri < bi))
}

/// `Some(max(entry, 0))` when the ray meets the box at `t >= 0`.
/// Not `BoundingBox3::ray_intersect`: that one returns the EXIT t when the
/// origin is inside the box, and a prune on it would skip a nearer
/// triangle (a zoomed camera sits inside the model box).
fn slab_entry(node: &Node, o: &P3, inv: &V3) -> Option<f64> {
    let (mut t0, mut t1) = (0.0_f64, f64::INFINITY);
    for a in 0..3 {
        if !inv[a].is_finite() {                       // dir[a] is 0 or subnormal
            if o[a] < node.lo[a] || o[a] > node.hi[a] { return None; }
            continue;
        }
        let (n, f) = ((node.lo[a] - o[a]) * inv[a], (node.hi[a] - o[a]) * inv[a]);
        t0 = t0.max(n.min(f));
        t1 = t1.min(n.max(f));
        if t0 > t1 { return None; }
    }
    Some(t0)
}
```

The argument, in four steps:

1. The gate is the same call as in brute force, so the two miss the same
   rays at the gate. (An empty mesh has an inverted box; that gate passes
   any ray with no zero component, and both then return `None` because no
   triangle exists.)
2. The padded box holds every point the triangle test accepts (2.1). Limit:
   when `det` is near the `RAY_BARY_EPS` floor, the rounding of `u`, `v`, `t`
   grows by about `1/det` and can pass the pad. The box has volume, so such a
   ray usually still enters the leaf, but the proof does not cover it. So the
   slab test never rejects a node that holds a triangle brute force hits.
3. A hit point lies at least `pad` inside its leaf box, so the box entry is
   below the hit `t`. The strict prune `entry > best_t` keeps every node that
   can hold a triangle with `t <= best_t`, ties included.
4. Every `t` comes from the same `Triangle::ray_intersect` with the same
   arguments, so each `t` is bit-equal to the brute-force `t`. `better` is a
   total order on (`t`, index). The minimum of a total order does not depend
   on the visit order. So leaf size, split rule and child order are free
   tuning parameters.

`cast_ray_all`: the same gate and slab test, no prune; collect every
`(tri, t)`; sort by `t.total_cmp`, then index. It equals the brute-force
`filter_map` over all faces with the same sort.

### 2.5 Memory (arithmetic from the current field lists)

`size_of::<Node>()` = 6·8 + 2·4 = 56 bytes (align 8). A `--lib` test asserts
`heap_bytes` against `size_of`, and the module doc cites that test, not the
number. Leaves = `2^d` where `d` is the first depth with `n / 2^d <= L`.

| Mesh | n | L | leaves | nodes | nodes × 56 B | `order` 4n | BVH total | mesh (faces 144 B + tri 12 B + verts) |
|---|---|---|---|---|---|---|---|---|
| rivmap terrain | 131 962 | 8 | 32 768 | 65 535 | 3.7 MB | 0.5 MB | 4.2 MB | about 21 MB |
| `tests/fixtures/terrain.stl` | 214 997 | 8 | 32 768 | 65 535 | 3.7 MB | 0.9 MB | 4.5 MB | about 34 MB |
| hemisphere 800² | 1 280 000 | 8 | 262 144 | 524 287 | 29.4 MB | 5.1 MB | 34.5 MB | about 215 MB |
| hemisphere 800² | 1 280 000 | 4 | 524 288 | 1 048 575 | 58.7 MB | 5.1 MB | 63.8 MB | about 215 MB |
| hemisphere 800² | 1 280 000 | 1 | 1 280 000 | 2 559 999 | 143 MB | 5.1 MB | 148 MB | about 215 MB |

At `L = 8` the BVH is at most about 21 % of its mesh (worst case
`4n/L` nodes, `≤ 32 B` per triangle against `≥ 156 B` per triangle of mesh).
It does not enter `SimulationEstimate`; the budget estimators cover the
simulation preflight only. Section 3 bounds the cache.

## 3. Ownership and cache

**Recommendation:** a core cache keyed by mesh identity, in
`maps/geom_cache.rs`, with its own table; a session door; a Job row that
builds off the frame; brute force until the build lands.

| Option | Invalidation | Verdict |
|---|---|---|
| Field on `LoadedModel` (`OnceLock<Arc<MeshBvh>>`) | every site that sets `mesh` must reset it; `adopt_geometry` would guard one of them | No: the identity memo needs no reset at all |
| Slot in `geom_cache::Entry` (shared table, `CAPACITY = 4`) | identity | No: see the thrash hazard below |
| **Own table in `geom_cache.rs`, `Weak` identity, no count cap** | identity | **Yes** |
| GUI-side cache | identity or revision | No: MCP and a future `pick_at` would not share it |

**The key.** `MeshMemo` holds a `Weak<TriangleMesh>` and compares with
`Arc::ptr_eq` (the ABA argument in `geom_cache.rs` :19-41). Each revision
makes a new `Arc`, so the old entry misses with no explicit drop:

- rescale of a STEP model: `apply_uniform_scale` → `Arc::new(TriangleMesh::from_raw(..))` (`enriched_mesh.rs:177`);
- re-import, reload, relink, rescale: `adopt_geometry` sets `self.mesh = mesh` from a fresh load (`session/mod.rs:790-806`);
- STL units: a fresh `from_stl_scaled` inside the import door.

**The thrash hazard of a count cap.** The shared table also takes the
setup-transformed meshes of a generation (`cached_transform` output is an
entry key of `cached_auto_index`). With a cap, five visible models (or one
model and a generation) evict a BVH; the pump then rebuilds it, which evicts
the next one, in a loop. So the pick table has no count cap. The liveness
sweep bounds it. `MeshMemo::sweep` runs on `write` only, so `peek_bvh` also
sweeps on a miss (the table is behind a `Mutex`; add a `pub(crate)` sweep to
`memo.rs`). Then an entry lives only while its mesh lives, and its size is at
most about 21 % of that mesh (2.5). An undo snapshot that keeps an old mesh
alive keeps its BVH only if a pick built one for it.

```rust
// crates/rs_cam_core/src/maps/geom_cache.rs  (additions)
/// No count cap: the dead-mesh sweep bounds the pick table (see above).
const PICK_CAPACITY: usize = usize::MAX;
type PickTable = MeshMemo<(), Arc<MeshBvh>, PICK_CAPACITY>;
fn pick_table() -> &'static Mutex<PickTable> { /* OnceLock, as `table()` */ }
static BVH: crate::maps::memo::CacheCounters = crate::maps::memo::CacheCounters::new();

/// The pick BVH of `mesh`, built at most once per mesh. BUILDS on a miss:
/// call it only off the frame (the job, a test). `None` only on
/// `BvhError::TooManyTriangles` (a warning once).
#[must_use]
pub fn cached_bvh(mesh: &Arc<TriangleMesh>) -> Option<Arc<MeshBvh>>;

/// The cached pick BVH of `mesh`, or `None`. Never builds; one lock.
#[must_use]
pub fn peek_bvh(mesh: &Arc<TriangleMesh>) -> Option<Arc<MeshBvh>>;

// test-support doors, as the others: `bvh_stats() -> (builds, hits)`,
// `bvh_cache_len()`, `clear_bvh()`.
```

```rust
// crates/rs_cam_core/src/session/mod.rs, impl LoadedModel
/// The mesh a ray pick tests: the enriched mesh, else the plain mesh.
/// (One `Arc` today: `workflow_tests.rs:297`; this form does not rely on it.)
pub fn pick_mesh(&self) -> Option<&Arc<TriangleMesh>> {
    self.enriched_mesh.as_ref().map(|e| &e.mesh).or(self.mesh.as_ref())
}
/// True when a ray pick on this model runs on the BVH. Frame-safe.
pub fn pick_index_ready(&self) -> bool;
/// The nearest triangle under the ray (model frame), with the contract of
/// `mesh::ray_pick_triangle`. The BVH when cached, else brute force: one
/// answer, two speeds. Never builds.
pub fn ray_pick(&self, origin: &P3, dir: &V3) -> Option<(usize, f64)> {
    let mesh = self.pick_mesh()?;
    match crate::maps::geom_cache::peek_bvh(mesh) {
        Some(bvh) => bvh.cast_ray(mesh, origin, dir),
        None => crate::mesh::ray_pick_triangle(mesh, origin, dir),
    }
}
```

**The build thread.** Not on the frame: 62 ms (terrain) to 451 ms (1.28 M)
in the probe is a visible hitch. Not at import either: the CLI and the MCP
never cast a ray, so they must not pay for a BVH. The build is a Job row on
the GUI Job lane, the `build_model_faces` pattern line for line:

```rust
// session/command.rs — one row
(Job, BuildModelPickIndex, "build_model_pick_index", BuildModelPickIndexArgs,
 ModelPickIndexBuilt,
 Surfaces {
     gui: Reach::Reached,
     mcp: Reach::Skip("MCP picks by key; it casts no screen ray"),
     cli: Reach::Skip("the batch CLI casts no ray"),
 }),
pub struct BuildModelPickIndexArgs { pub model_id: crate::ids::ModelId }

// session/pick_index.rs (new)
pub struct BuildModelPickIndexHandle { model_id: ModelId, mesh: Arc<TriangleMesh> }
pub struct ModelPickIndexBuilt { pub model_id: ModelId, pub nodes: usize }
impl ProjectSession {
    /// Step (i): capture the pick mesh. A READ: no result drops, no revision moves.
    pub(crate) fn capture_build_model_pick_index(&self, id: ModelId)
        -> Result<BuildModelPickIndexHandle, SessionError>;   // ModelNotFound; no mesh → InvalidParam
}
/// Step (ii): fill the cache (`cached_bvh`). Reads no session. A set cancel
/// flag stops it before the build.
pub fn execute_build_model_pick_index(h: &BuildModelPickIndexHandle, cancel: &AtomicBool)
    -> Result<ModelPickIndexBuilt, SessionError>;
```

The pick needs no one-frame notice. Until the job lands, `ray_pick` runs brute
force, which is today's behaviour; the answer is the same, only slower for
about 0.1-0.5 s. The Job lane runs one GUI job at a time; while a tier-map
preview runs, the pick stays on brute force.

## 4. The viz call site

`pick_model_face` (`picking.rs:233-287`), minimal diff. The frame
transform, the `ctx.shows` filter and the `meshes` gate stay.

```rust
-use rs_cam_core::mesh::ray_pick_triangle;
 ...
     for model in session.models().iter().filter(|m| ctx.shows(m.id)) {
         if let Some(enriched) = &model.enriched_mesh {
-            let mesh = enriched.as_mesh();
-            // Fast AABB rejection
-            if mesh.bbox.ray_intersect(&model_origin, &model_dir).is_none() {
-                continue;
-            }
-            if let Some((tri_idx, t)) = ray_pick_triangle(mesh, &model_origin, &model_dir)
+            if let Some((tri_idx, t)) = model.ray_pick(&model_origin, &model_dir)
                 && best.as_ref().is_none_or(|(b, _)| t < *b)
             { /* unchanged */ }
         } else if meshes
             && model.faces_are_flat_regions()
-            && let Some(mesh) = &model.mesh
-            && mesh.bbox.ray_intersect(&model_origin, &model_dir).is_some()
-            && let Some((_, t)) = ray_pick_triangle(mesh, &model_origin, &model_dir)
+            && let Some((_, t)) = model.ray_pick(&model_origin, &model_dir)
             && best.as_ref().is_none_or(|(b, _)| t < *b)
         { /* unchanged */ }
```

The removed AABB lines repeat the gate inside `ray_pick_triangle`, so their
removal changes no answer.

**The pump.** `controller/events/model_pick_index.rs` (new), called beside
`process_model_faces` on every pump:

- If `state.is_job_running()`, return.
- Take the first model in session order with `pick_mesh().is_some()`,
  `!pick_index_ready()`, and not in `failed_pick_index_models`. The rule
  does not read visibility: `ctx.shows` comes from app state in
  `viewport.rs:27`, not from the controller. An unneeded BVH costs about
  20 % of its mesh, once.
- Submit `Job::BuildModelPickIndex`. The answer needs no upload, because
  no draw changes. On a failure, insert the id into the failed set and push
  one warning, so a failure cannot loop.

The rule builds for every mesh model, STL included, because
`HoverResult.surface_point` (design B §5) and the orbit pivot (A, CAM-1) cast
on the terrain outside a pick mode too.

**The hover.** Design B owns `HoverResult`. This design gives it one thing:
`LoadedModel::ray_pick` returns `(tri, t)` in the model frame. The surface
point is `model_origin + model_dir * t`, moved to the display frame by
`SetupFrame::transform_point`. With the BVH a pick costs microseconds, so the
one-pick-per-frame rule of B is for clarity, not for time.

**MCP parity.** `rg pick crates/rs_cam_viz/src/app/mcp/` finds only key
picks (`add_pick`, `list_model_items { near }`) and one test that calls
`pick_items` (`commands/selection.rs:2039`). MCP has no screen-coordinate
pick. A future `pick_at(x, y)` builds the ray from the MCP camera and calls
`interaction::picking::pick`, which reaches `LoadedModel::ray_pick`: the same
function, and by 2.4 the same answer whether the BVH is built or not.

## 5. Tests

The parity fixture is `crates/rs_cam_core/tests/fixtures/terrain.stl`
(214 997 triangles), reached by `common::repo_root()` and
`common::require_fixture`. It is NOT the probe mesh
(`planning/fixtures/rivmap100/…/terrain.stl`, 131 962 triangles): a gate test
reads no `planning/` path (`gate_tests_read_no_operator_paths_f155`). The
test profile has no `opt-level`, so brute force is slower than in the probe;
keep the ray count small and time the first run (not measured here).

| Test | Kind | Claim | Red proof (inject once) | Sentry |
|---|---|---|---|---|
| `tests/bvh_pick_equals_brute_force_sel2.rs` | integration | On the fixture: the probe eye (`c + (1.2e, −1.2e, 1.5e)`) over a 10 × 10 grid; an eye INSIDE the box over 8 × 8; 8 × 8 rays straight down (two zero components). On `make_test_hemisphere` and the probe's 70 × 70 grid hemisphere: rays through grid vertices and edge midpoints. `cast_ray == ray_pick_triangle` with `t.to_bits()` equal; `cast_ray_all` equals the sorted brute-force list. | prune with `BoundingBox3::ray_intersect` (inside eye goes red); drop the pad | `mesh/CLAUDE.md` |
| `tests/a_ray_on_a_shared_edge_takes_the_lower_index_sel2.rs` | integration | Two coplanar triangles share an edge; a ray straight down on the edge: both give one `t`; BVH and brute force return index 0. Swap the triangle order: index 0 again (now the other triangle). A fan of six triangles, a ray on the shared vertex: index 0. A ray `0.5·RAY_BARY_EPS` outside in `u`: brute force hits, BVH hits. | `better` with `<=` on index; pad 0 | `mesh/CLAUDE.md` |
| `--lib mesh::bvh::tests` | unit | `n == 0` gives 0 nodes and the cast ends; `leaf_size = 0` builds as 1. Node invariants: every index once in `order`; every leaf count in `(0, L]` and in `(L/2, L]` when `n > L`; a leaf box holds its triangle boxes; a parent box holds its children; `start > i` for an interior node; `nodes = 2·leaves − 1`; two builds equal (`PartialEq`); `heap_bytes` = capacities × `size_of`. Empty mesh: 0 nodes, `None` from both. A zero-area triangle: `None` from both (det below eps); mixed with a real one: equal answers. A cast with another mesh falls back. `TooManyTriangles` is unreachable in a test; the `try_from` is the guard. | build with `select_nth_unstable_by` on centroid only (no index tie) — the reproducibility check may still pass; use the leaf-box check for this red | `mesh/CLAUDE.md` |
| `tests/a_reimport_drops_the_pick_index_sel2.rs` (`test-support`, a cache lock as in `stl_flat_regions_are_face_items_p4_7`) | integration | Write an STL to `CARGO_TARGET_TMPDIR`; load; `cached_bvh`; `AdoptModelGeometry` with inch units: `pick_index_ready()` is false; `ray_pick` still answers (brute force) on the scaled mesh; the `build_model_pick_index` job (`start` + `execute`) fills the cache, and `bvh_stats` counts one build; a second job counts a hit. | key on `Arc::as_ptr` held as `usize` after a drop (ABA) is hard to inject; inject "skip the identity check" in `peek_bvh` | `maps/CLAUDE.md` (test-support list) |
| viz `controller/tests/selection.rs` (new case) | unit | The pump submits `BuildModelPickIndex` for a shown STEP model and not while a job runs; a failed model is not tried again. | remove the failed-set check | viz `controller` folder map |
| existing: `a_click_picks_the_drawn_item_p2_6` (viz), `command_registry_completeness`, `folder_maps_name_real_files_f140` (core and viz twin), `the_test_doors_are_gated_fld0405` | — | the click rule is unchanged; the new row is complete; the maps name real files; the new doors sit behind `test-support` | — | already named |

## 6. Commit sequence

Agents implement; the lead commits with explicit paths. Each commit runs:
its sentries, `cargo test -p rs_cam_core -q --lib`, `cargo test -p rs_cam_viz -q --lib`,
`cargo test -p rs_cam_mcp -q`, clippy (root command), `cargo fmt --all -- --check`,
all through `scripts/cargo_lane.sh` with a `timeout`.

| # | Files | Green alone because |
|---|---|---|
| C1 | `core/src/geo.rs` (`RAY_BARY_EPS`, used by `Triangle::ray_intersect`: same value, bit-identical); `core/src/mesh.rs` (`pub mod bvh;`); `core/src/mesh/bvh.rs`; `core/src/mesh/CLAUDE.md`; `core/CLAUDE.md` (28 folders + row); `core/tests/bvh_pick_equals_brute_force_sel2.rs`; `core/tests/a_ray_on_a_shared_edge_takes_the_lower_index_sel2.rs`; `CREDITS.md` (one line, if the module doc cites a BVH text such as PBRT 4.3) | new code with no caller; tests only |
| C2 | `core/src/maps/geom_cache.rs` (pick table, `cached_bvh`, `peek_bvh`, doors); `core/src/maps/CLAUDE.md`; `core/src/session/mod.rs` (`pick_mesh`, `pick_index_ready`, `ray_pick`); first half of `core/tests/a_reimport_drops_the_pick_index_sel2.rs` (no job yet) | `ray_pick` has no caller yet |
| C3 | the Job row, in ONE commit because the matches are exhaustive: `core/src/session/command.rs` (row, args, `JobHandle` variant + `id` arm, `start` arm, the READ list in the `start` doc); `core/src/session/pick_index.rs`; `core/src/session/mod.rs` (`mod pick_index;` + re-exports); `core/src/session/CLAUDE.md`; `viz/src/compute/worker.rs` (label :1464, execute :1797); `viz/src/app/mcp/commands.rs` (:1809, :2847); `viz/src/controller/events/compute.rs` (:1453 delivery); `viz/src/controller.rs` (`GuiJobTarget::ModelPickIndex`, `failed_pick_index_models`); `viz/src/controller/events/model_pick_index.rs` + its `mod` line; the viz folder map(s); the job half of the reimport test; the viz pump test. `rg BuildModelFaces crates` lists every site; each hit gets a twin. | the registry sentry checks the row; nothing reads the cache yet except the test |
| C4 | `viz/src/interaction/picking.rs` (section 4 diff) | the answer is the same by 2.4; p2_6 stays green |
| — | after C1: a leaf-size sweep `{2, 4, 8, 16}` on the probe meshes as a planning instrument (build ms, cast mean/max µs, `heap_bytes`), in `planning/viewport_parity_2026-10-28/probe/`; it sets `DEFAULT_LEAF_SIZE` in a one-line follow-up | — |

Invariants kept: the result order is a total order on (`t`, index), so it is
deterministic; the build uses no `HashMap` and no rayon; the cache holds live
meshes only and its bytes come from `size_of`; every lint exception is
`#[expect(.., reason = "..")]` on the smallest item, never `#[allow]`; core
stays GUI-free (the job and the cache are core; the pump is viz); no new
dependency.

## 7. Open questions for the lead

| # | Question | Evidence | Rec. |
|---|---|---|---|
| Q1 | Own BVH or `parry3d-f64 0.24` pinned? | 0.24 pulls nalgebra 0.34 beside our 0.33 (probe `versions.txt`); current parry 0.31 is glam-based; parry misses edge-exact rays that our test hits (B.1, 4 rays); a wrapper would still need our triangle test in the leaves to keep parity. The own BVH is about 250 lines with the parity proof in 2.4. | Own BVH. |
| Q2 | Leaf size. | Memory scales with `1/L` (2.5); the cast cost is `O(log n + L)` triangle tests, about 1 µs class at any L in {4, 8}. | Ship 8; the sweep after C1 decides; no number from memory. |
| Q3 | Build thread. | 62-451 ms in the probe; the CLI and MCP never pick. | Job row on the GUI Job lane, lazy per shown mesh model, brute force until it lands. Reject a build at import and a build on the frame. |
| Q4 | Move `SpatialIndex` into `mesh/` beside the BVH? | 190 files name `mesh::SpatialIndex`; `geometry_cache_g8` pins its element order; a move is a path change only. | Not in this work. A later structure package may move it to `mesh/spatial_index.rs`; the paths change in one commit (no alias, per the no-legacy ruling). |
| Q5 | No count cap on the pick table. | A cap of 4 thrashes with five shown models (section 3). | Accept the liveness bound (≤ about 21 % of live mesh bytes at L = 8). |
| Q6 | f32 node boxes to halve node memory? | The brief asks f64; f32 boxes need outward rounding and a second pad proof. | No; keep f64. Revisit only if a mesh above 1 M triangles is a real project. |
