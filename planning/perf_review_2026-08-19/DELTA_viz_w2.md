# DELTA — viewport wave 2 (V8, V13)

Lane: viewport. Branch `tech-debt-3`. Scope: `crates/rs_cam_viz/` only.
Findings: **V8** (no per-object dirty flags) and the correctness bug inside
**V13** (BREP hover highlight never re-uploads), plus the linear
`selected_faces` test V13 also names.

---

## 1. What landed

### V8 — content keys instead of dirty bits

The review prescribed "per-resource dirty bits + per-toolpath GPU data keyed
by result generation". The landed fix implements the second half literally and
**replaces the first half with content keys**, for reasons worth recording:

- A dirty-bit scheme requires each of the ~40 `pending_upload = true` sites to
  name the right resource. A site that names too few silently stales the
  viewport — the exact class of defect V13 already *is*. A content key cannot
  be staled by a forgetful setter, because it is derived from the values the
  buffer was built from, not from a promise about them.
- A dirty-bit scheme still costs a full rebuild whenever a coarse "everything"
  bit fires, and a lot of sites genuinely do not know what they changed.
  Content keys make a coarse fire cheap: the pass looks, finds nothing moved,
  and rebuilds nothing.
- Zero churn across the ~40 setters, which the review explicitly lists as
  "already good — consistent, don't regress". They are untouched.

The keys live in a new module, `crates/rs_cam_viz/src/render/upload_cache.rs`,
one struct per cached resource, each documenting its inputs against the code
that consumes them. Keyed resources and their key inputs:

| Resource | Key inputs |
|---|---|
| `mesh_data_list` (STL, indexed) | plain-mesh `Arc` identity + triangle count per model; setup (id, face_up, z_rotation); `StockConfig` |
| `enriched_mesh_data_list` (STEP/BREP) | as above for enriched meshes, **plus** selected face list and hovered face |
| collision markers | the marker positions themselves + display shift |
| rest-depth heatmap | selected toolpath id + its `AnnotatedToolpath` `Arc` identity + display shift |
| per-toolpath lines (`ToolpathGpuData.upload_key`) | `Arc` identity of the result; palette index; `selected`; colour mode; span filter; display shift; feed rate; AdvancePerTooth source (trace ptr + `gui.edit_counter`); resolved entry-preview config; tool-profile tool |

Deliberately **not** keyed, because the key comparison would cost more than the
rebuild: stock wireframe + solid stock, origin axes, fixture/keep-out/pin
lines, polygon (DXF/SVG) outlines, height planes. These are tens to a few
thousand vertices each. The sim mesh keeps its existing in-place
`update_colors_if_changed` path, untouched.

Two secondary effects fell out of the same change:

- The AdvancePerTooth colour mode's two full cut-trace scans
  (`build_advance_bands` + `advance_per_tooth_per_move`, V11) are now built on
  first use inside the toolpath loop instead of before it. When every toolpath
  key is unchanged, neither scan runs.
- The rest-heatmap block's full `AnnotatedToolpath` deep clone (V9 — taken
  whenever a display shift is in play, which is the normal config for an
  identity setup with a non-zero stock origin) now happens once per result
  generation instead of once per upload pass.

### V13 — the hover highlight, and the per-triangle selection test

`last_hover_face` is written in `viewport.rs` but nothing marked an upload
pending, so the BREP hover highlight only appeared when some unrelated event
happened to trigger an upload, and then showed a face the pointer had been
over on an earlier frame. Fixed by setting `pending_upload` **on a change of
hovered face only**, paired with a `request_repaint` gated on the same
condition (the pointer can come to rest on a new face and produce no further
egui repaint of its own to carry the upload — G-LV.1's rule is "no
*unconditional* repaint", and this one is conditional on a state change that
converges in one frame).

Ordering, stated precisely, because the review's wording deserves an exact
answer: `upload_gpu_data` runs near the top of `update()`, the viewport draws
during the UI pass below it, so the hover pick for frame *N* is consumed by
the upload at the top of frame *N+1*. The highlight therefore appears **one
repaint after the pointer enters a face** and is then correct and stable for
as long as the pointer rests there. It is no longer "showing the wrong face";
it lags by one frame. Eliminating even that lag would mean adding a second
upload pass after `handle_events` in `update()` — which would change frame
ordering for all ~40 upload sites, not just this one — and I judged that a
worse trade than a 16 ms lag on a hover tint. Recorded as a deliberate choice,
not an oversight.

`mesh_render.rs`: `selected_faces.contains(&face_id)` ran once per triangle, a
linear scan of the selection list — O(triangles × selected). Now one
`HashSet` build up front, O(triangles).

---

## 2. Upload counts — before / after

Derived from the code, not measured on a GPU (there is no criterion harness
for the GUI loop; the review says so, and the counter added in §3 is the
runtime instrument that replaces one).

Definitions: a **build** is one construction of a GPU buffer set for a
resource. A **pass** is one call to `upload_gpu_data`, i.e. one
`take_pending_upload()` that returned true.

### 2a. 8-operation `generate_all`

`controller/events/compute.rs:840` sets `pending_upload` on **each** completed
toolpath, so the run fires **at least 8 passes** — more when the fixpoint
loop's interleaved simulations fire their own (`compute.rs:1056,1129`). The
pass count is unchanged by this work; what each pass *does* is what changed.
Submitting a toolpath clears its result (`compute.rs:422`), so at pass *k*
exactly *k* toolpaths have results — hence the 1+2+…+8 before-count below.

| Resource | Before | After |
|---|---|---|
| Model mesh (STL) | 8 | **0** (1 on the first pass after load, then 0) |
| Toolpath line buffers | 1+2+…+8 = **36** | **8** (one per completion; 28 reuses) |
| Rest heatmap (deep clone + mesh) | up to 8 | ≤ 1 |
| Collision markers | 8 (empty-set branch) | ≤ 1 |
| Stock / axes / fixtures / polygons / height planes | 8 | 8 (unchanged, deliberately) |

The 44 → 8 collapse on the two expensive resources is the finding. What each
avoided build costs, on the review's reference workload (wanaka200,
661,212-triangle terrain STL, 12.6k-move passes):

- One mesh build = `transform_mesh` over every vertex in f64 (setup transform),
  area-weighted normal accumulation over 3×661k triangle corners, a 3×661k u32
  index flatten, and two fresh GPU buffers totalling roughly 16 MB. Eight of
  those is ~128 MB of buffer churn per generate, now once.
- One toolpath build = a full `AnnotatedToolpath::translated` deep clone
  (moves + spans + planner engagement) when a display shift is in play, plus
  the per-move span classify and two line buffers (~576 KB at 12k moves).

### 2b. Selection click

A click that moves selection from toolpath A to toolpath B, same setup:

| Resource | Before | After |
|---|---|---|
| Model mesh (STL) | 1 (full 661k-tri re-transform + 16 MB upload) | **0** |
| Enriched mesh (STEP) | 1 | 0, unless the face selection actually changed |
| Toolpath line buffers | N (all visible) | **2** — A loses `selected`, B gains it |
| Rest heatmap | 1 (incl. deep clone) | 1 (the selected toolpath genuinely changed) |
| Collision markers | 1 (incl. the O(n²) density pass) | 0 |

A click that does **not** change selection, or a filter/visibility toggle that
resolves to the same state, rebuilds **nothing**: `UploadStats::is_idle()` is
true for that pass. Before, it rebuilt the entire scene.

A colour-mode or span-filter toggle still rebuilds all N toolpaths — correctly,
since it changes every toolpath's vertex colours — but no longer drags the
model mesh, the collision markers and the rest heatmap along with it.

### 2c. BREP hover (V13)

| | Before | After |
|---|---|---|
| Highlight appears | only if something unrelated triggered an upload | on the next repaint after the face changes |
| Builds per face-boundary crossing | n/a | 1 enriched mesh |
| Builds per pointer move *within* a face | n/a | **0** (the key is the face id, not the pointer) |
| Model mesh / toolpaths dragged along | (would have been all of them) | 0 |

The last row is why the review insisted V8 land first: setting the flag before
per-resource keys existed would have turned a missing highlight into a
full-scene rebuild per pointer move.

---

## 3. The instrument

`RenderResources.upload_stats` (`upload_cache::UploadStats`) counts passes and
per-resource builds/reuses for the life of the process, and each pass logs its
own delta:

```
RUST_LOG=rs_cam_viz=debug cargo run -p rs_cam_viz --bin rs_cam_gui
# ... "gpu upload pass" mesh_builds=0 toolpath_builds=1 toolpath_reuses=7 ...
```

An 8-op `generate_all` now prints eight lines each naming **one** toolpath
build; before the change the same eight lines would have named 1, 2, … 8. The
counter outlives the fix: a future change that reintroduces a full-scene
rebuild shows up as `toolpath_reuses=0` on a pass that should have been idle.

Unit tests for the key semantics are in `upload_cache.rs` (`cargo test -p
rs_cam_viz upload_cache`), asserting the four claims the count table rests on:
a new result generation invalidates only its own key; a selection change moves
only the `selected` flag; colour mode, span filter, entry-preview dials and
the AdvancePerTooth sources are all keyed; and hover moves the enriched key
while the plain-mesh key has no hover input at all. They are pure key
comparisons — no GPU device — which is the only part of this that *can* be
tested in CI.

---

## 4. Honest notes / things I did not do

- **Nothing here is a measured wall-clock speedup.** Every number in §2 is a
  build count derived from the control flow, with the per-build cost described
  rather than timed. There is no GPU-loop harness in this repo and I did not
  add one.
- **Pointer-identity keys carry an ABA hazard.** The cache holds no strong
  reference, so a freed `Arc` allocation could in principle be reused at the
  same address by a replacement mesh, and the pass would reuse a stale buffer.
  Mitigated by pairing each pointer with its element count (an identically
  sized replacement landing at exactly the freed address is required to
  collide). The alternative — keeping an `Arc` clone in the key — would retain
  a deleted model's geometry until the next upload, which is a worse trade for
  a 661k-triangle mesh. The pre-existing simulation caches
  (`cached_simulation_triage`) use the bare pointer, so this is not a new class
  of risk, just a slightly tightened one.
- **A failed buffer allocation is now sticky.** If `try_create_buffer` refuses
  a mesh for exceeding GPU limits, the key is still recorded, so the pass will
  not retry until an input changes. Previously every pass retried and re-logged
  the warning. This is arguably the better behaviour but it is a change, so it
  is written down.
- **The STEP path's 3 unindexed vertices per triangle is structural — left
  alone.** The review floats indexing it "if contained". It is not: the
  enriched path is *flat*-shaded (per-triangle face normal) and per-face-group
  coloured, so two triangles can share a source vertex only when they share
  both normal and face group. Indexing it means either changing the shading
  model to smooth (a visual change, and the wrong one for a BREP part where
  face boundaries should read as creases) or a dedupe pass over
  (position, normal, colour) whose hashing cost on a 661k-triangle mesh would
  plausibly exceed the VRAM it saves. The STL path at `mesh_render.rs:64` is
  indexed precisely because it *is* smooth-shaded. Not attempted; recommend
  the review's V13 row be amended to say so.
- **Not touched, still open in this file:** V9 (apply the display shift at
  vertex emission rather than deep-cloning the annotated toolpath — the clone
  is now once-per-generation instead of once-per-pass, which removes most of
  the sting but not the allocation), V10 (`span_paths_by_move()` heap Vec per
  move), V11 (route the chipload heat-map inputs through
  `cached_chipload_envelopes` — the lazy build here reduces how often they run
  but does not share the timeline's cache), V12 (O(n²) collision density — now
  run far less often, but still O(n²) when it runs).
- **Preserved, verified by reading:** `take_pending_upload` gating semantics
  and all ~40 setter sites (untouched); repaint scheduling — the one repaint
  added is conditional on a hover-face *change*, the G-LV.1 fix and the 100 ms
  MCP heartbeat are untouched; `SimMeshChunk` capacity tracking and
  `ColorFingerprint`; the draw structure (one render pass, buffer order in
  `toolpath_data` is unchanged because reused entries are pushed in the same
  config order); `GpuLimits` / `try_create_buffer` / chunked upload.

---

## 5. Verification

- `cargo clippy -p rs_cam_viz --all-targets -- -D warnings` — clean.
- `cargo test -p rs_cam_viz` — green, exit 0, including `tests/wizard_e2e.rs`
  (12), `tests/mcp_escape_hatches.rs` (15) and the lib suite; the 7 new
  `render::upload_cache::tests` pass.
- `cargo fmt -p rs_cam_viz --check` — clean.
- No `crates/rs_cam_core/` file was touched by this lane, so the Phase 0
  goldens and the core API surface are unaffected; core compiled clean as a
  dependency of every check above.

There is no GPU-loop test harness in this repo, so none of the §2 counts is
asserted by a test. What *is* asserted is the key semantics those counts are
derived from (see §3), and the runtime counter reports the real thing on a
live session.

## 6. A note for the consolidator

Per the honesty rule I wrote the two corrections above (V8's dirty-bits→keys
substitution, V13's "indexing the STEP path is structural") into
`PERF_REVIEW.md` **in place**, under the V8 and V13 rows. That file is
**deliberately not in this lane's commit**: the simulation lane had its own
uncommitted `PERF_REVIEW.md` hunks in the working tree at the same time, and
committing the file would have swept their in-flight text into a viz commit.
The corrections are on disk, unstaged, alongside theirs.

## 7. Files changed

- `crates/rs_cam_viz/src/render/upload_cache.rs` — new; key types, `UploadStats`, unit tests
- `crates/rs_cam_viz/src/render/mod.rs` — module registration; four key fields + `upload_stats` on `RenderResources`
- `crates/rs_cam_viz/src/render/toolpath_render.rs` — `ToolpathGpuData.upload_key`; `EntryPreviewConfig: Clone + PartialEq`
- `crates/rs_cam_viz/src/render/mesh_render.rs` — `HashSet` for the per-triangle selected-face test
- `crates/rs_cam_viz/src/app/gpu_upload.rs` — the guards, the lazy AdvancePerTooth build, the tracing instrument
- `crates/rs_cam_viz/src/app/viewport.rs` — V13 hover dirty + conditional repaint
