# `mesh/` — the children of `mesh.rs`

## Files

- `bvh.rs` — `MeshBvh`, the ray-pick BVH over a `TriangleMesh` (SEL-2).
  The parent `mesh.rs` keeps `TriangleMesh`, `ray_pick_triangle` and
  `SpatialIndex`.

## Invariants

- `MeshBvh::cast_ray` equals `ray_pick_triangle` on every ray the sentry
  casts: the same `Triangle::ray_intersect`, the same box gate, the lowest
  index on a bit-equal `t`. The proof covers hits with `|det|` well above
  `RAY_BARY_EPS`; a graze near that floor is the known limit.
- A node box is padded by `pad` (derived from `RAY_BARY_EPS`), and the
  prune has the slack `RAY_PRUNE_REL`. Neither may reject a triangle that
  the triangle test accepts. A flat axis at 0 has a pad of 0: the slack
  covers it.
- No `HashMap` and no rayon in the build: two builds give one layout.

## Sentries

- `cargo test -p rs_cam_core -q --test bvh_pick_equals_brute_force_sel2 --test a_ray_on_a_shared_edge_takes_the_lower_index_sel2`
- `cargo test -p rs_cam_core -q --lib mesh::bvh`
- The cache and the session door (`maps::geom_cache` pick table, `LoadedModel::ray_pick`): `a_reimport_drops_the_pick_index_sel2` (`--features test-support`).
