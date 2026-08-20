# W5B-F1 — the adaptive3d planner's boundary-ring over-claim

**Lane:** W5B-F1, 2026-08-21. Branch `tech-debt-3`, parent tip `3e2041f3`,
landed as `77ead905`.

**Verdict: root-caused. The standing candidate is REFUTED. The finding is a
test-fixture artifact and does not occur in the shipped configuration.**

The corrections ledger goes to **twelve**.

---

## 1. What the finding said

From `DELTA_sim_w5b_landing.md` §4:

> *adaptive3d's planner claims removal on the boundary ring that its emitted
> toolpath does not deliver, on both clearing strategies, independent of the
> stamp kernel, pre-dating this campaign.*

Measured on the hemisphere parity fixture, outside `mesh bbox ± 1 mm`
(2737 cells):

| fixture | boundary `sim_higher` | % of ring |
|---|---:|---:|
| AgentSearch | 1360 | 49.7% |
| ContourParallel | 568 | 20.8% |

Bit-for-bit identical under `whole_path` and `swept`. Pinned, not fixed, by
`BOUNDARY_ONE_SIDED_CAP_PCT = 55.0`.

**Standing candidate** (from the drape-mirror commit's own "NOT FIXED"
residual, and the direction `ParityResult::sim_higher`'s docstring predicts):
`Cut` segments whose first emitted feed sweeps from the emitter's true tool
position rather than from the planner's raw `last_pos` — an emitter-side
transform missing from `stamp_emitted_segment`'s mirror.

---

## 2. Reproduction

`cargo test -p rs_cam_core --lib planner_sim_dexel_parity -- --nocapture`,
same binary, 1.5 s:

```
[AgentSearch hemisphere]     PARITY: 1399/7921 cells differ > 0.53mm; interior 21/5184;
                             planner_higher 39; sim_higher 1360; max dz 25.000mm
[ContourParallel hemisphere] PARITY:  576/7921 cells differ > 0.53mm; interior  8/5184;
                             planner_higher  8; sim_higher  568; max dz 25.000mm
```

1360 and 568 confirmed exactly. Grid is 89×89 = 7921; interior window 72×72 =
5184; ring 2737.

`max dz 25.000 mm` is the whole stock height (`stock_top_z 25.0` over
`bbox.min.z 0.0`). On those cells one side is fully cleared and the other is
untouched virgin stock. That is not a sub-cell mirror discrepancy of any kind;
it is a wholesale clear.

---

## 3. Spatial distribution — the evidence that kills the candidate

An instrumented pass cross-tabs every cell against the **F-027 border-clear
predicate** in `crates/rs_cam_core/src/adaptive3d/path.rs:345-357`: a cell is
border-cleared when it lies outside `mesh.bbox ± cutter.radius() * 0.5` **and**
is not rescued by a declared `world_stock_xy_bbox`. Zones: `interior` (the
existing window), `ring/not-cleared`, `ring/border-cleared`.

**AgentSearch, `world_stock_xy_bbox: None` (the fixture's setting):**

| zone | agree | `planner_higher` | `sim_higher` |
|---|---:|---:|---:|
| interior | 5163 | 21 | **0** |
| ring / not border-cleared | 1359 | 18 | **0** |
| ring / border-cleared | 0 | 0 | **1360** |

**ContourParallel, `world_stock_xy_bbox: None`:**

| zone | agree | `planner_higher` | `sim_higher` |
|---|---:|---:|---:|
| interior | 5176 | 8 | **0** |
| ring / not border-cleared | 1377 | 0 | **0** |
| ring / border-cleared | 792 | 0 | **568** |

The distribution is **not** clustered at cut-segment starts, and it is **not** a
uniform rim. It is *exactly* the border-clear zone and nothing else:

* **1360 of 1360** and **568 of 568** `sim_higher` cells are inside the zone.
* **Zero** `sim_higher` cells anywhere outside it, on either strategy.
* The zone is 1360 cells on this fixture. AgentSearch over-claims on **100.0%**
  of it; ContourParallel on 41.8% (its contours reach further out, so the
  simulator genuinely clears the other 792 and the two sides agree).

A `last_pos` discrepancy is a *per-cut-segment* effect: it would leave a short
tube's worth of cells at the **start of each Cut segment**, scattered through
the interior as much as the rim, and it could not produce a full-stock-height
delta. Not one of the 1928 cells is at a cut-segment start. **The candidate is
refuted, not partially — it explains 0 of 1928 cells.**

The candidate remains live for part of the 21 / 8 **interior**
`planner_higher` cells (the other direction), where it has never been
separated from the F.a sub-cell blend residual. That is unchanged by this
lane and is recorded as such in the test's doc comment.

---

## 4. Root cause

`adaptive_3d_segments` pre-clears the planner's internal dexel rays for cells
outside the mesh footprint (`path.rs:319-397`). The rationale is sound and
documented: the drop-cutter returns `min_z` beyond the mesh edge, so those
cells look like infinitely deep material the tool can never reach, and the
planner would otherwise burn passes cutting empty space.

F-027 added the inhibition: when `world_stock_xy_bbox` is `Some`, cells inside
the declared stock footprint are **real** material and must keep their planner
material so the planner emits passes that stamp them down DPP at a time.

```rust
// path.rs:345
let border_margin = r * 0.5;
let outside_mesh = x < bbox.min.x - border_margin || ... ;
if !outside_mesh { continue; }
if let Some((wx_min, wy_min, wx_max, wy_max)) = world_xy
    && x >= wx_min && x <= wx_max && y >= wy_min && y <= wy_max { continue; }
ray_subtract_above(material_stock.z_grid.ray_mut(row, col), clear_z);
```

**The parity fixture never declares a world stock footprint.** It hands the
planner an `initial_stock` covering `mesh.bbox ± cutter.radius()` — i.e. it
*does* put real stock on the rim, and the independent simulator replay carries
it — but leaves `world_stock_xy_bbox: None`, so `path.rs` empties those rays.
The planner is not mis-mirroring its emitter. It was told there is no stock
there, and believed it.

**The shipped configuration always declares it.**
`compute::execute::execute_operation` supplies
`world_stock_xy_bbox: Some((stock_bbox.min.x, .min.y, .max.x, .max.y))` on
every adaptive3d call (`crates/rs_cam_core/src/compute/execute.rs:1600-1605`).
Only unit-test call sites pass `None` — `param_sweep.rs`, `end_to_end.rs`,
`agent_search_coverage.rs`, `wanaka_z_layer_render.rs`,
`adaptive3d_keep_down_link_f038b.rs`, `agent_search_axial_doc.rs`,
`adaptive3d_entry_coalescing_f038.rs`, and the parity pair.

---

## 5. The control — the same fixture with the stock footprint declared

One parameter changed, nothing else:

| | `world_xy: None` | `world_xy: Some` |
|---|---:|---:|
| AgentSearch — `sim_higher` | **1360** | **0** |
| AgentSearch — whole-grid divergent | 1399 | **81** |
| AgentSearch — max dz | 25.000 mm | **4.161 mm** |
| AgentSearch — emitted moves | 1154 | 1522 |
| ContourParallel — `sim_higher` | **568** | **0** |
| ContourParallel — whole-grid divergent | 576 | **5** |
| ContourParallel — max dz | 25.000 mm | **1.747 mm** |
| ContourParallel — emitted moves | 3271 | 3949 |
| ContourParallel — interior divergent | 8 | **3** |

The over-claim is **exactly zero** in the shipped configuration, on both
strategies, and the fixture's whole-grid agreement improves by 17× and 115×.
The extra moves are the F-027 intent: the planner now plans passes over the rim
instead of assuming it away.

W5B-F1 therefore **does not occur in production**, and there is nothing in the
planner to fix.

### Why it cannot occur in production

The planner grid is the **union** of `mesh.bbox ± r` with the declared world
stock bbox (`path.rs:214-229`), so the declared stock is always covered. The
only cells that remain border-cleared in production are those in
`mesh.bbox ± r` but **outside** the stock — i.e. where the model overhangs the
stock block. Those cells hold no real material and are outside the simulator's
per-setup dexel grid, so no divergence is observable there and none is wrong.

---

## 6. What landed — `77ead905`

Verdict-safe by the brief's definition, and by a stronger one: **every hunk is
inside `#[cfg(test)] mod tests`.** No production line is touched, so no
generated geometry can move on any fixture and no golden can shift — that is
structural, not a fingerprint comparison. The two pre-existing parity tests
keep `world_xy: None` and emit byte-identical toolpaths (1154 and 3271 moves,
unchanged).

1. **`ParityResult` carries the mechanism cross-tab** —
   `sim_higher_in_border_clear_zone` and `border_clear_zone_total`. The
   `W5B-F1:` line prints it, so the next reader sees the mechanism without
   re-deriving it.
2. **A PRIMARY mechanism bar in `assert_parity_bars`**: every `sim_higher` cell
   must lie inside the border-clear zone. This is the bar that would actually
   catch the defect class `sim_higher`'s docstring names, and it is sensitive
   to **one** cell — where a percentage cap over a 2737-cell ring is not.
3. **The percentage cap survives as a secondary growth pin**, tightened
   **55.0% → 51.0%** against the deterministic 49.7% / 20.8%, with its doc
   comment rewritten from "unexplained open finding" to the root cause.
4. **Two new sentries run the shipped configuration** —
   `planner_sim_dexel_parity_{agent_search,contour_parallel}_world_stock_declared`
   — barring `sim_higher` at **zero**, with a precondition assert that the
   border-clear zone is genuinely empty so the control cannot pass vacuously
   (the empty-population trap in `CLAUDE.md`). These are the only parity tests
   that exercise the configuration the product ships.
5. **`RS_CAM_PARITY_MAP=1`** dumps the ASCII cell map the cross-tab came from.

Verification: `cargo test -p rs_cam_core --lib` **2352 passed, 0 failed**;
`cargo clippy -p rs_cam_core --lib --profile test -- -D warnings` clean;
`cargo fmt --check` clean on `adaptive3d/mod.rs`.

> Caveat on the verification, recorded rather than hidden: another lane was
> editing `crates/rs_cam_core/src/dexel_stock/*` in the same working tree
> during this session, and `cargo clippy -p rs_cam_core --all-targets` could
> not be run to completion because one of its in-flight integration tests
> (`air_cut_family_calibration_w5bf4.rs`) does not currently compile. Only my
> own hunks were staged (that lane's edit to a test helper in the same file was
> excluded from the commit), and clippy was scoped to the lib's unit-test
> target, which is where all of this lane's code lives.

---

## 7. Two things left open, neither this lane's to close

**(a) The `boundary` polygon clear is the production analogue, and is
un-measured.** `path.rs:404-430` performs the same kind of pre-clear for cells
outside a `ToolContainment` boundary polygon, and the post-generation toolpath
clip converts outside-boundary cuts to rapids — so in production, *with a
containment boundary set*, the planner's internal stock should over-claim
outside the boundary in exactly the same direction, by the same mechanism. The
code comment says this is deliberate (without it, "the dexel for those cells is
left unstamped — deeper z-levels then bite through fresh stock with full-depth
axial DOC", `AGENTSEARCH_INVESTIGATION_LOG.md` O5b). The parity fixture sets
`boundary: None`, so **no measurement of that case exists**. It is the one
route by which a W5B-F1-shaped divergence could be live in production, and it
should be measured before anyone concludes from this document that the class
is closed.

**(b) The rim's other direction grew when the border clear was inhibited.**
With the stock footprint declared, AgentSearch's boundary `planner_higher`
(the simulator removing **more** than the planner claims) goes 18 → 60. That
direction is the F.a sub-cell blend / discretisation class and no bar gates it
on the ring. 60 of 1360 zone cells is small and it is the benign direction, but
it is a new number produced by this lane and is recorded so it is not
discovered later as a surprise.

---

## 8. Ledger entry

**Refuted claim #12.** *"W5B-F1's boundary-ring over-claim is `Cut` segments
whose first emitted feed sweeps from the emitter's true tool position rather
than from the planner's raw `last_pos` — an emitter-side transform missing from
the planner's mirror."*

It is not. It is the F-027 border clear firing over a fixture that never
declares its stock footprint, and it explains 1928 of 1928 cells while the
candidate explains 0. The candidate's own direction prediction was correct and
its docstring is not wrong — `sim_higher` **is** the direction a missing
emitter transform would produce. It is simply also the direction a deliberate
pre-clear produces, and the instrument could not tell the two apart. It can
now.
