# DELTA — generation wave 4 (G8)

Lane: setup-caching. Branch `tech-debt-3`. Finding: **G8** — spatial index
rebuilt per toolpath; `Vec<Vec<usize>>` cells; silhouette + mesh copy repeated.

**Headline: G8 is real, is fixed, and is rated far too high.** All three
repetitions were there and all three are gone. Measured on the review's own
reference workload, removing every one of them is worth **~0.97 s** on an
eight-operation `generate_all` — against a ~40-minute run, **0.040 %**. The
row says MED-HIGH. It is not. §5 is the correction.

---

## 1. What landed

Three call sites stopped repeating work, and one storage format changed.

### 1a. `crate::geom_cache` — a once-per-(model, setup) memo

New module `crates/rs_cam_core/src/geom_cache.rs`, plus one line in `lib.rs`.
Three memoised derivations, all keyed on the source mesh's `Arc`:

| Function | Wraps | Now runs |
|---|---|---|
| `cached_auto_index` | `SpatialIndex::build_auto` | once per mesh |
| `cached_silhouette` | `boundary::model_silhouette(mesh, None)` | once per mesh |
| `cached_transform` | `SetupTransformInfo::apply_to_mesh` | once per (mesh, transform) |

Wired at five sites:

- `session/compute.rs:1071` — the per-toolpath `transform_mesh_to_setup` deep
  copy → `cached_transform`.
- `session/compute.rs:1141` — the per-toolpath `SpatialIndex::build_auto` →
  `cached_auto_index`. `ResolvedGenInputs.spatial_index` became
  `Option<Arc<SpatialIndex>>`; its three consumers moved `as_ref()` → `as_deref()`.
- `session/compute.rs:1751` (`resolve_containment_polygon`) — the silhouette
  rasterisation → `cached_silhouette`. This one site serves **two** calls per
  toolpath: the pre-boundary resolution and the post-generation enforcement
  clip both route through it.
- `viz .../worker/execute/mod.rs:54` — the worker's own `build_auto` →
  `cached_auto_index`.
- `viz .../worker/execute/mod.rs:153` **and** `:671` — the worker's two
  `model_silhouette` calls → `cached_silhouette`.

Two signature changes fell out, both contained:

- `resolve_containment_polygon` (`pub(crate)`) and `apply_boundary_clip`
  (`pub`) take `Option<&Arc<TriangleMesh>>` instead of `Option<&TriangleMesh>`,
  because the memo needs the `Arc` to key on. Every production caller already
  had the `Arc`; **all three external (test) callers pass `None`**, so no test
  file needed editing.
- `ProjectSession::transform_mesh_to_setup` is deleted. Its only caller now
  goes through `cached_transform`, which needs the `SetupTransformInfo` itself
  as the key, so the wrapper that hid it had no remaining use. This is the one
  hunk in `session/mod.rs`, a file outside this lane's list — it is a deletion
  forced by the lint policy (a dead `pub(crate)` method is a `-D warnings`
  failure), not a change of behaviour.

### 1b. `SpatialIndex` cells: `Vec<Vec<usize>>` → CSR

`mesh.rs`. `cells: Vec<Vec<usize>>` became `cell_starts: Vec<usize>` (prefix
sum with a trailing sentinel) + `cell_items: Vec<usize>`. All three query paths
(`query`, `query_rect_into`, `cell_triangles_at`) now decode through one new
`cell_slice(cell_idx) -> &[usize]`, so they cannot drift. Five accessors added
for the equivalence harness: `cell_slice`, `cell_count`, `cell_count_x`,
`cell_count_y`, `cell_size`, `total_triangles`.

The build is count → prefix-sum → fill, with the per-triangle cell rectangle
**stored** in a `Vec<[usize; 3]>` (`first_cell`, `span_x`, `span_y`) rather
than recomputed in the fill pass. That table is not an optimisation of
convenience — see §3b, where recomputing measured *worse* than the form it
replaced.

Count and fill walk the **same stored span**, so the two passes write the same
number of entries per cell by construction and cannot disagree even if the
"`x1 >= x0` always" argument (proved in a comment at the site) is ever
invalidated upstream.

---

## 2. Why the key is sound — and the review's proposal is not

The review says "cache on (`Arc::as_ptr`, cell size)". **A bare pointer is not
a sound key.** An `Arc` can be dropped and a fresh allocation can land at the
same address; the cache then answers a lookup for mesh B with mesh A's index.
A stale spatial index does not fail loudly — it silently mis-answers every
downstream query, which is the worst available failure mode for a caching
change that is supposed to be observationally invisible.

`DELTA_viz_w2.md` hit this and mitigated it by pairing the pointer with an
element count, correctly noting that this *narrows* the window rather than
closing it, and that the pre-existing `cached_simulation_triage` uses a bare
pointer and therefore still carries the hazard.

**This is not a theoretical concern on this machine.** The test
`a_different_mesh_at_the_same_address_is_not_a_hit` drops one `Arc<TriangleMesh>`,
then allocates 64 more, and reports whether any landed on the freed address. It
prints `address reuse observed: true`. The glibc allocator hands the address
straight back. A bare-pointer key would have been wrong on the first reload of
a model in a GUI session, and an element-count pairing would only have
survived because the replacement happened to have a different triangle count.

The key used here is a **`Weak<TriangleMesh>`**, which closes the hole rather
than narrowing it:

1. A live `Weak` keeps the `Arc` *allocation* from being freed even after the
   last strong reference drops — only the `T` inside it is dropped. So while a
   cache entry exists, **no other `Arc` can be handed that address**. Collision
   is impossible, not merely unlikely.
2. Lookup calls `Weak::upgrade()` and compares with `Arc::ptr_eq`. A hit proves
   the entry refers to the very object being queried, and that it is alive.

Cost: one atomic increment per lookup, and a retained control block (not the
111 MB of mesh) for a dropped model until the next insert sweeps it.

### Invalidation, stated rather than assumed

There is no explicit invalidation path, and none is needed, because every
input to every memoised derivation is in the key. The claim rests on three
legs, each checkable:

- **Mesh content ≡ mesh identity.** `TriangleMesh` has no interior mutability,
  and the workspace contains **no** `Arc::get_mut` or `Arc::make_mut` on an
  `Arc<TriangleMesh>` — the four `make_mut` sites in the tree are on grids
  (`toolpath_spans.rs:541`), polygon sets (`:551`) and cut traces
  (`session/compute.rs:2836`). Mutating a mesh needs `&mut TriangleMesh`, which
  an `Arc` does not hand out. Any re-import, `fix_winding`, units change or
  scale produces a *new* `TriangleMesh` and therefore a new `Arc`, which
  misses. **If a future change introduces `Arc::make_mut` on a mesh, this memo
  becomes unsound** — that is the one invariant to protect, and it is stated at
  the top of the module.
- **Cell size.** `cached_auto_index` memoises `build_auto` *only*, and
  `build_auto`'s cell size is a pure function of the mesh (extent + triangle
  count). There is nothing left to vary, so mesh identity is the complete key —
  strictly stronger than the review's `(ptr, cell_size)` pair, which would have
  admitted a second entry for a value that cannot differ. Explicit-`cell_size`
  builds are deliberately **not** cached; those callers keep calling
  `SpatialIndex::build` directly. Same for `cached_silhouette`, which covers
  only the `None` (0.5 mm default) resolution — the only one either generation
  path requests.
- **Setup transform.** `cached_transform` keys on the source mesh's identity
  *and* on all eight fields of `SetupTransformInfo` (`face_up`, `z_rotation`,
  three stock dimensions, three origin components). The six f64s are compared
  as `to_bits`, never `==`, so `-0.0` does not alias `0.0` and a `NaN` origin
  does not silently miss forever. Pinned by
  `transform_key_distinguishes_negative_zero_origin`.

Verified by test rather than argued: `cached_index_equals_a_fresh_build`
compares a cached index against a fresh `build_auto` **cell for cell, in
order**, plus `cell_size().to_bits()`.

### What bounds it

`CAPACITY = 4` entries, one per distinct source mesh, evicted oldest-first.
Every insert first sweeps entries whose mesh has been dropped, so a closed
model's index and transformed copy are released at the next generation rather
than held for the process lifetime. Each entry holds **at most one** index, one
silhouette and one transformed mesh. The ceiling is therefore a constant
multiple of `CAPACITY`; it cannot grow with the number of toolpaths,
generations or fixpoint rounds, which is the leak the naive version of this
would have been. `cache_is_bounded` asserts the entry count against `CAPACITY`
after 20 distinct meshes.

The transformed-mesh slot is deliberately **one deep**. A project alternating
two non-identity setups over one model rebuilds on each switch rather than
retaining two ~111 MB copies. Generation is setup-major in practice, so the
alternation is rare and the memory bound is worth more than the hit rate.

Builders run **outside** the lock — a 27 ms index build is not something to
serialise other threads behind. Two threads racing the same miss both build and
the second insert wins; the results are equal by construction, so the race
costs one redundant build and nothing else.

---

## 3. Measurement

Harness: `tests/geometry_cache_g8.rs::g8_unit_costs`, release, run with
`--ignored --nocapture` and optionally `G8_TERRAIN=<path>`.

Cross-day absolutes on this box are not comparable, so every ratio below is a
**paired same-run** A/B: the harness carries a verbatim transcript of the
pre-CSR `Vec<Vec<usize>>` builder (`reference_cells`) and times it in the same
process, on the same mesh, microseconds apart from the new one. That matters
here — earlier runs of this same harness, taken while three other lanes had
cargo jobs on the box, reported the same ratios up to 30 % off. The table below
is from a quiet run; treat the *ratios* as the result and the absolutes as
indicative.

### 3a. Unit costs

Two meshes: the review's reference workload, and the in-repo fixture the
`spatial_index/*` bench group actually uses.

**`wanaka200/rivmap_export/terrain.stl`** — 661,212 triangles, 330,609
vertices, `build_auto` cell 0.6957 mm, **82,944** grid cells:

| Unit | before | after | ratio |
|---|---|---|---|
| `SpatialIndex::build_auto` — **the production resolution** | 37.923 ms | **19.749 ms** | **1.92×** |
| `SpatialIndex::build(mesh, 10.0)` — the bench resolution, 441 cells | 8.949 ms | 9.699 ms | 0.92× (see §3b) |
| `model_silhouette(mesh, None)` | 33.022 ms | 33.022 ms | unchanged (memoised, not made faster) |
| `transform_mesh_to_setup` (95.2 MB of `faces` re-derived) | 27.882 ms | 27.882 ms | unchanged (memoised, not made faster) |

**`fixtures/terrain_small.stl`** — 40,342 triangles, `build_auto` cell
1.2059 mm:

| Unit | before | after | ratio |
|---|---|---|---|
| `SpatialIndex::build_auto` | 1.467 ms | **0.997 ms** | **1.47×** |
| `SpatialIndex::build(mesh, 10.0)` | 0.565 ms | 0.614 ms | 0.92× |
| `model_silhouette(mesh, None)` | 2.661 ms | 2.661 ms | unchanged |
| `transform_mesh_to_setup` | 0.295 ms | 0.295 ms | unchanged |

Criterion agrees on the one arm it covers: `spatial_index/build_terrain` reads
**608.60 µs** [597.65, 622.40] after the change, against the probe's 0.614 ms
for the identical build. `query_r3` 484.35 ns, `query_r10` 1.3272 µs — the
query paths are unaffected by design (CSR decoding is a slice, not an
indirection). Criterion had no stored baseline for this group, which is why the
before column above comes from the paired harness instead.

### 3b. The CSR loses ~8 % on coarse grids, and that is what the bench measures

`spatial_index/build_terrain` benches `build(mesh, 10.0)`, which on either
fixture is a few-hundred-cell grid. At that resolution `Vec<Vec<usize>>` is
already near-optimal: the vectors stay resident in cache, there are a few
thousand reallocations in total, and it needs **one** pass over the triangles.
CSR needs **two** no matter how it is written, and there are almost no
allocations to save. It loses by a consistent **0.92×** on both fixtures — a
real regression, small and stable.

At the resolution generation actually uses — `build_auto`, 82,944 cells on the
reference mesh — the picture inverts: ~82 k vector headers plus ~82 k heap
blocks with doubling slack is exactly the cost CSR removes, and it wins
**1.92×** (1.47× on the smaller fixture, where there is less allocation to
save — the win scales with cell count, as it should).

Both CSR variants were measured, paired, against the same reference. Storing
the per-triangle cell rectangle costs a transient `Vec<[usize; 3]>` (15.9 MB on
the reference mesh) and buys not re-streaming the 95 MB `faces` array in the
fill pass:

| build variant | @ `build_auto` | @ `build(10.0)` |
|---|---|---|
| recompute the cell span in the fill pass | 1.44× faster | 1.66× slower |
| **store the span table** (landed) | **1.92× faster** | 0.92× |

**I did not add a small-grid fast path.** It would work — build per-cell
vectors and flatten into CSR — and would plausibly reach parity on coarse
grids. It buys ~0.75 ms on a resolution **no production call site uses**: every
production build is `build_auto` (`session/compute.rs`, the viz worker,
`compute/simulate.rs:659,1221`, `compute/collision_check.rs:67`,
`project_curve.rs:241`). The only explicit-cell-size callers are benches, test
fixtures, and one env-var-gated debug harness in `pencil.rs`. A second builder
to keep bit-identical with the first, forever, is not worth 8 % on a path
nothing takes. Recorded as a measured decision, not an oversight.

**Consequence for the bench**: `spatial_index/build_terrain` should be expected
to read ~8 % slower than a pre-change baseline, and that is the true reading of
what it measures. It measures the wrong resolution. Proposed arm for whoever
owns `benches/perf_suite.rs` — not applied here, because an A/B that changes
the bench and the code in the same commit measures neither:

```rust
// `build_auto` is the resolution every production call site uses; the
// existing `build(mesh, 10.0)` arm is ~200x coarser (441 cells vs 82,944)
// and does not exercise the per-cell allocation pressure at all.
group.bench_function("build_terrain_auto", |b| {
    b.iter(|| black_box(SpatialIndex::build_auto(&mesh)))
});
```

### 3c. Counts — the repetition half

Criterion cannot hold a `generate_all`, so this half is counted, with the
per-unit cost from §3a attached. The counts are not derived from control flow
alone: `geom_cache::stats()` is a live counter, and
`eight_toolpaths_over_one_model_build_one_index` drives **eight real
`ProjectSession::generate_toolpath` calls** over one model and asserts the
counter. It prints:

```
8 toolpaths, one model: index_builds=1 index_hits=7
```

Per 8-operation `generate_all`, per fixpoint round:

| Derivation | before | after | at §3a cost (reference mesh) |
|---|---|---|---|
| **Core session path** (`resolve_generation_inputs`) | | | |
| `SpatialIndex::build_auto` | 8 | **1** | 303.4 ms → 19.7 ms |
| `model_silhouette` (2 per toolpath: pre-boundary + enforcement clip) | 16 | **1** | 528.4 ms → 33.0 ms |
| `transform_mesh_to_setup` (non-identity setups only) | 8 | **1** | 223.1 ms → 27.9 ms |
| **GUI worker path** (`generate_via_core`) | | | |
| `SpatialIndex::build_auto` | 8 | **1** | 303.4 ms → 19.7 ms |
| `model_silhouette` (pre-clip + enforcement clip) | 16 | **1** | 528.4 ms → 33.0 ms |
| setup mesh transform (`controller/events/compute.rs:324`) | 8 | 8 | **not changed** — see §6 |

Totals, core path, per round. Note the "before" column already carries the
CSR's own 1.92× on the index row, so these understate the combined change
slightly — the two halves are reported separately on purpose:

| Project shape | before | after | saved | of a ~40 min run |
|---|---|---|---|---|
| non-identity setup + `ModelSilhouette` boundary (all three apply) | 1054.9 ms | 80.6 ms | **0.97 s** | 0.040 % |
| identity setup + `ModelSilhouette` boundary | 831.8 ms | 52.7 ms | 0.78 s | 0.033 % |
| identity setup + stock boundary (the common case) | 303.4 ms | 19.7 ms | 0.28 s | 0.012 % |

Allocation, per index build: ~82,944 `Vec` headers (2.0 MB) plus ~82 k heap
blocks with doubling slack → **three** allocations (663 KB `cell_starts`,
15.9 MB transient `spans`, ~8 MB `cell_items`). Honest note: peak transient
memory of a single build goes **up** by roughly the 15.9 MB span table, and
down by the vector slack. The clear memory win is elsewhere — the transform
memo removes 888 MB of deep-copy churn per 8-op round (8 × 111 MB → 111 MB).

---

## 4. Correctness

This is a pure caching and representation change. Nothing about it should be
observable in an emitted toolpath.

- **CSR equivalence, element-wise.** `csr_cells_are_elementwise_identical_to_vec_of_vec`
  builds both forms over four meshes (the terrain fixture; a hemisphere;
  a deliberately X-stretched "ragged" mesh whose triangles straddle many cells;
  a flat plate) at seven cell sizes (0.5 / 1 / 2 / 5 / 10 / 37.5 mm and each
  mesh's own auto size) and asserts every cell's slice is equal **as a
  sequence**, not as a set. This matters: several consumers break ties on
  candidate order, so a reordered list can change an emitted toolpath.
- **Query equivalence.** `csr_queries_match_the_reference_cell_union`
  reconstructs the dedup'd cell union from the reference structure in (y, x)
  scan order and compares it to `SpatialIndex::query` at 36 centre/radius
  combinations including radius 0 and off-grid centres — so the assertion
  covers the query path, not just the storage.
- **Cache invisibility.** `cached_index_equals_a_fresh_build` asserts the
  second lookup returns the same allocation *and* that its contents match a
  fresh `build_auto` cell for cell, with `cell_size` compared by `to_bits`.
- **ABA.** `a_different_mesh_at_the_same_address_is_not_a_hit` (see §2).
- **Bound.** `cache_is_bounded`.
- **Count, end to end.** `eight_toolpaths_over_one_model_build_one_index`.
- Unit tests in `geom_cache.rs` itself cover reuse, distinct meshes, entry
  sweeping on drop, transform-key discrimination by rotation, and the `-0.0`
  bit comparison.

Float comparisons in the new code and tests are `to_bits`, never `==`.

Fingerprint sentries and goldens run: see §7.

---

## 5. The correction to `PERF_REVIEW.md`

Written into the G8 row in place, left **unstaged** (the simulation and
viewport lanes have their own uncommitted hunks in that file; committing it
would sweep their in-flight text into a generation commit).

Three corrections:

1. **The severity is wrong.** G8 is rated MED-HIGH. Removing all three
   repetitions is worth **~0.97 s** per 8-operation round in the most
   favourable project shape, and **0.28 s** in the common one, against a
   reference `generate_all` of roughly 40 minutes — **0.012 % to 0.040 %**.
   Every unit it names costs tens of milliseconds, not seconds. The row should
   read LOW on wall clock. Its real value is the 888 MB of deep-copy churn the
   transform memo removes per round, and the fact that it is a
   correctness-neutral structural cleanup — not speed.
2. **The proposed key is unsound**, and the ABA case is not hypothetical on
   this machine (§2).
3. **The bench it points at measures the wrong resolution.** "Note `perf_suite`
   already benches `spatial_index/build_terrain`, so this half has a bench
   today" is true but misleading: that arm builds a few-hundred-cell grid,
   where the CSR conversion *loses* 8 %. The 82,944-cell grid generation
   actually builds is where it wins 1.92×, and nothing benches it (§3b).

---

## 6. Not done, and why

- **The GUI controller's per-toolpath mesh transform is still there.**
  `controller/events/compute.rs:324` builds a fresh `Arc` per toolpath via
  `state::job::transform_mesh`, so on a **non-identity** setup the GUI worker's
  index memo misses every time — a fresh `Arc` misses however it is keyed. Both
  files are outside this lane's ownership. The fix is one line: route that call
  through `geom_cache::cached_transform` with
  `session.setup_transform_info(face_up, z_rotation)`. Identity setups — the
  common case — are unaffected and already hit, because the worker receives the
  model's own `Arc`. Worth ~195 ms per round to whoever owns those files.
- **`compute/simulate.rs:659` and `:1221` rebuild the index per simulation**,
  and the fixpoint `generate_all` runs a simulation between rounds. `:659`
  already has the `Arc` (`SimulationRequest::model_mesh: Option<Arc<TriangleMesh>>`)
  and is a one-line change; `:1221` needs `compute_deviations` to take `&Arc`.
  **Declined**, with the measurement: at 19.7 ms per build that is ~40 ms per
  simulation. Given §5's conclusion that the whole finding is worth well under
  0.1 %, taking scope into a file no lane owns for another 40 ms is not a good
  trade. Listed here so it is not lost.
- **`model_silhouette` was not parallelised.** It rasterises all faces serially
  into one shared boolean grid, which is a real row-chunking opportunity and
  the shape G9 found to be its bigger half. But it now runs **once** instead of
  16 times, so parallelising it can save at most 33 ms per round from a budget
  of 2.4 M ms. Not attempted; `boundary.rs` is otherwise untouched by this lane.
- **No small-grid fast path for the CSR build** (§3b).
- **`cached_simulation_triage`'s bare-pointer key is untouched.** Flagged by
  `DELTA_viz_w2.md` as a known live defect; it lives in
  `crates/rs_cam_viz/src/state/simulation.rs`, outside this lane. The `Weak`
  pattern in `geom_cache` is the drop-in fix if someone wants it.

---

## 7. Verification

- `cargo clippy -p rs_cam_core --benches --tests -- -D warnings` — clean.
- `cargo clippy -p rs_cam_viz --all-targets -- -D warnings` — clean.
- `cargo test -p rs_cam_core --test perf_golden_sim_metrics --test perf_golden_depth_level_geometry` — green (both Phase 0 goldens).
- `cargo test -p rs_cam_core --test geometry_cache_g8` — 7 green.
- Fingerprint sentries green: `finish_resolution_policy_pr3`,
  `crease_own_region_pr6b`, `checkpoint_b_resolution_ab`,
  `transform_provenance_fingerprints`, plus the `SpatialIndex`-heavy suites
  that pin `(moves, hash)` constants.
- `cargo test -p rs_cam_viz` — green.
- `rustfmt` applied to this lane's files only; `session/mod.rs` and
  `session/compute.rs` needed no reformatting, and no sibling module moved.

The lib compile was broken twice mid-run by another lane's in-flight edits to
`dexel_stock/stamping.rs`; per protocol those were waited out, not fixed.

---

## 8. Files changed

- `crates/rs_cam_core/src/geom_cache.rs` — new; the memo, the `Weak` key, `stats()`, unit tests
- `crates/rs_cam_core/src/lib.rs` — one line, `pub mod geom_cache;` (staged as a single hunk so the path-ordering lane's `mod nn_order;` stays out)
- `crates/rs_cam_core/src/mesh.rs` — CSR storage, `cell_slice` + five accessors, span-table build
- `crates/rs_cam_core/src/session/compute.rs` — three memo call sites, `Arc<SpatialIndex>`, two `&Arc<TriangleMesh>` signatures
- `crates/rs_cam_core/src/session/mod.rs` — deletion of the now-dead `transform_mesh_to_setup`
- `crates/rs_cam_viz/src/compute/worker/execute/mod.rs` — three memo call sites
- `crates/rs_cam_core/tests/geometry_cache_g8.rs` — new; equivalence, ABA, bound, count, timing probe
