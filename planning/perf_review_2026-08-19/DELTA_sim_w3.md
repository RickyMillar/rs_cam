# Wave 3 (SIM) — S5 fixpoint prefix memoization

Captured 2026-08-20 against `PERF_REVIEW.md` S5 and the wave-2 numbers in
`BASELINES.md`. Every cargo invocation serialized behind
`flock /tmp/rs_cam_cargo.lock` with a ≥20 GB available-memory gate.

**Do not read this file as a replacement for `BASELINES.md`.** It is one
lane's delta.

Commits: `2ed9df04` (implementation), `55847d4f` (sentries + bench arm).

---

## THE HARD CONSTRAINT — held

`sim_metrics_match_golden`, `sim_metrics_3d_match_golden`,
`perf_golden_depth_level_geometry` and the whole `-p rs_cam_core` suite are
green and **unchanged**. **Nothing was re-baselined.** S5 is not a
metric-changing fix and does not claim to be one: it decides *whether* a
toolpath is simulated again, never *how*.

---

## 1. What the finding said, and the one thing it got wrong

> `controller/events/compute.rs:1439-1455` → `run_simulation_with_all` →
> `simulate.rs:676` fresh grid + full replay. k rest ops ⇒ k full-project sims
> (O(k²) op-sims); toolpaths 1..j-1 byte-identical across rounds, nothing
> memoized.
>
> Fix: provenance hashes already exist (`hash_toolpath` :381, tool hash
> :421-429, `operation_config_hash` :368). Cache prefix-hash → post-carve grid.

The diagnosis is exactly right and the numbers back it: the 0C wanaka run is
3 rounds / 2 simulations at 0.4 mm, and a standalone `run_simulation` at that
resolution is **84 s** of the 394 s `generate_all` — so round 2's replay of
round 1's work is a large, real, measurable fraction of the wall clock.

### CORRECTION — "the provenance hashes already exist" closes nothing

The three named hashes are **not** a sufficient cache key, and using them as
one would have shipped a wrong-answer bug rather than a slow one.

1. **The tool hash carries no cutter shape.**
   `build_simulation_provenance` (`simulate.rs:421-429`) hashes
   `diameter, length, shank_diameter, shank_length, holder_diameter,
   stickout, flute_count`. A Ø6 **flat** end mill and a Ø6 **ball** nose of
   the same length and assembly hash **equal**. The cutter's radial profile
   is what `RadialProfileLUT::from_cutter` is built from and therefore what
   every stamp in the prefix consumed. Keyed on the provenance hash, swapping
   flat→ball on toolpath 1 between rounds would have resumed onto a grid
   carved by the wrong cutter, silently.
2. **`hash_toolpath` hashes moves only** — not spans, not `MoveIntent`. Both
   feed the simulation: `span_paths_by_move()` stamps per-sample ancestry and
   `transit_moves_bitmap_from_intents()` drives transit classification
   (`simulate.rs`, the `spans_valid` block). Two toolpaths with identical
   move geometry and different spans produce different sample streams.
3. **Nothing in the trio covers the request-global inputs** — resolution,
   stock bbox, metric options, spindle RPM, rapid feed, the reference model
   mesh — nor the per-group `local_stock_bbox` / `local_to_global`. The
   `stock_hash` and `machine_hash` in the same function do cover some of
   these, but they are not among the three the finding names, and neither
   covers the model mesh or the group transforms.

What landed instead is a purpose-built key with its closure stated in
`sim_prefix.rs`'s module doc and a per-input table (§3 below). Pointer-keyed
inputs use `Weak`, per the G8 precedent; the tool — the one input with no
stable identity — is keyed parametrically **and** by probe (§3.2).

---

## 2. What is memoized, and why resume is bit-identical

The memo holds **one** snapshot of every loop-carried accumulator in
`run_simulation_with_phase`, taken immediately after the request's **last**
toolpath entry has carved and *before* that group's end-of-group work. The
next simulation whose group/entry sequence starts with the recorded one
restores that state and continues from the first new entry.

Bit-identity here is **by construction, not by argument**. Nothing is
recomputed, approximated or re-derived: the restored values *are* the
previous run's values, and the per-entry code path that runs afterwards is
byte-for-byte the code that ran before. This is a materially weaker claim to
have to defend than S2's ("this skip is exact") or S3's ("this
reassociation preserves every bit"), and it is why S5 needed no threshold, no
tolerance and no golden re-baseline. The only thing that can go wrong is
**forgetting a piece of state** — which is what the enumeration below and the
whole-result fingerprint sentry exist for.

### 2.1 The output enumeration — reuse-or-prove, per artifact

Every value that crosses a toolpath iteration in `run_simulation_with_phase`,
and its disposition. "Snapshotted" means the field is in
`sim_prefix::PrefixState` and is restored verbatim.

| Artifact | Produced per | Disposition |
|---|---|---|
| `total_moves` | entry | **Snapshotted** — boundary `start_move`/`end_move` and `rapid_collision_move_indices` are offsets into it |
| `boundary_index` | entry | **Snapshotted** |
| `boundaries` (id, name, tool_name, move range, direction) | entry | **Snapshotted** |
| `checkpoints` (composited mesh + `global_stock.checkpoint()`) | entry | **Snapshotted**, `Arc`-shared with the result they came from (§4) |
| `cut_samples` (the whole dexel sample stream) | entry | **Snapshotted** — the dominant memory item, and the one real copy |
| `drill_samples` / `drill_summaries` (§6.E PR2) | drill entry | **Snapshotted** |
| `composite_mesh` | group end | **Snapshotted** |
| `global_stock` (playback/checkpoint parallel grid) | entry | **Snapshotted** |
| `column_deviations` | group end | **Snapshotted**; the reference model mesh is in the key by `Weak` identity |
| `rapid_collisions` + `rapid_collision_move_indices` | entry | **Snapshotted** |
| `prior_stocks` — real per-entry snapshots | entry | **Snapshotted**, restricted to replayed entry ids |
| `prior_stocks` — F.4 *phantom* snapshot | group | **NOT snapshotted — re-derived** from the live request (§3.3). This is the one place the naive "restore everything" answer is wrong. |
| `global_drill_ops` | drill entry | **Snapshotted** (currently `let _ =`d at the end; snapshotted anyway so a future consumer cannot silently diverge) |
| `group_stock` + `group_drill_ops` | within a group | **Snapshotted** for the resume group; groups before it are fully inside the restored global accumulators |
| `model_index` (`SpatialIndex::build_auto`) | run | **Proved not carried** — a pure function of `request.model_mesh`, rebuilt each run, read only by the group-end deviation pass |
| `sample_step_mm`, `resolution_clamped`, `column_grid_cell_mm`, `global_bbox` | run | **Proved not carried** — pure functions of `stock_bbox` + `resolution`, both in the key |
| `mesh`, `deviations` (final) | run end | **Proved not carried** — derived after the loop from `composite_mesh` and `model_mesh` |
| `cut_trace` (+ provenance, drill slots) | run end | **Proved not carried** — built after the loop from `cut_samples` and the *live* request; provenance is stamped from the live request, so a resumed run's provenance is the current project's, exactly as a replay's would be |
| `apply_kinematics_cycle_time` (F-034/F-035 runtime + predicted feeds) | run end | **Proved not carried** — a post-loop rewrite of the trace from the live request. `request.kinematics` is therefore deliberately **out of the key**: a machine-profile change invalidates nothing the prefix owns. |

**The fixpoint's intermediate simulation is NOT a lighter path.** The review
left this open; it is worth stating with evidence, because it is what forced
the full-fidelity contract above. `settle_generate_all_round`
(`controller/events/compute.rs`, the `Next::Simulate` arm) calls
`run_simulation_with_all`, which is the same
`ComputeBackend::submit_simulation` → `execute::run_simulation_with_phase` →
`simulate::run_simulation_memoized` path a user's **Run Simulation** button
takes (`controller/events/mod.rs`, `AppEvent::RunSimulation`). The result is
applied to `AppState::simulation` in full — mesh, checkpoints, playback data,
cut trace, prior stocks (`controller/events/compute.rs`, the
`ComputeMessage::Simulation` arm) — and is live to the viewport, to
`get_diagnostics` and to `screenshot_simulation` between rounds. There is no
"internal" fidelity to scope down to.

### 2.2 The sentry that enforces the enumeration

`tests/sim_prefix_memo_s5.rs::resumed_run_is_bit_identical_to_a_full_replay`
fingerprints the **entire** `SimulationResult` — mesh vertices/indices/colors,
per-vertex and per-column deviations, every boundary field, every checkpoint's
mesh *and* dexel grid (segment `enter`/`exit` plus `conservative_top`), rapid
collisions, `prior_stocks` (sorted by id so map order cannot leak in), and the
cut trace via its own `Serialize` impl — as **bit patterns, no tolerance
anywhere**. It compares a resumed 5-op run against a from-scratch replay of
the identical request.

Serialising the trace rather than hand-listing its fields is deliberate: a
field added to `SimulationCutTrace` later is covered automatically, which a
hand-written comparison would not be.

Twelve sentries in total; inventory in §7.

---

## 3. The key

### 3.1 Closure

| Input | Keyed by |
|---|---|
| `stock_bbox` (6 f64), `stock_top_z`, `resolution`, `rapid_feed_mm_min` | `f64::to_bits`, global scalar |
| `spindle_rpm`, `metric_options.{enabled, capture_arc_engagement}` | global scalar |
| `groups.len()` | global scalar |
| `model_mesh` | `Weak<TriangleMesh>` identity |
| group order + count | position in the key vector |
| per-group `direction`, `local_stock_bbox`, `local_to_global` (all 8 fields, floats by bits) | group scalar |
| per-entry toolpath geometry + spans + move intents | `Weak<AnnotatedToolpath>` identity |
| per-entry `semantic_trace` | `Weak<ToolpathSemanticTrace>` identity |
| per-entry `drill_op` | `Weak<DrillOp>` identity |
| per-entry cutter + assembly | `hash_tool` (§3.2) |
| per-entry `id`, `name`, `tool_summary`, `flute_count`, `spindle_rpm`, `metrics_not_applicable`, `operation_config_hash` | entry scalar |
| `phantom_prior_stock` | **excluded by design** — re-derived (§3.3) |
| `kinematics` | **excluded by design** — post-loop only (§2.1) |

**Invalidation argument.** There is no explicit invalidation and none is
needed: every input to the memoized prefix is in the key, so a changed input
misses. Concretely —

* a **regenerated toolpath** produces a new `Arc<AnnotatedToolpath>` (the GUI
  stores `ToolpathResult::annotated` fresh on every completion), so it misses
  on pointer identity;
* a **parameter edit that does not move geometry** (a feed-rate change) still
  moves `operation_config_hash`, which is in the entry scalar;
* a **tool edit** moves `hash_tool`;
* a **resolution / stock / setup-transform / metric-option change** moves the
  global or group scalar;
* **reordering, inserting or removing** a toolpath changes the ordered key
  vector, and the match is a strict prefix comparison in order;
* a **disabled** toolpath simply is not in the request, which shortens the
  sequence and truncates the match at that position.

**Pointer keys are `Weak`, never bare pointers.** A bare `Arc::as_ptr` key is
ABA-unsound and `DELTA_gen_w4.md` (G8) *observed* address reuse on this
machine. A live `Weak` keeps the allocation reserved, so no other `Arc` can
be handed that address while the entry exists; lookup upgrades and compares
with `Arc::ptr_eq`. Identity implies content because none of the three types
has interior mutability and the tree contains no `Arc::make_mut` /
`Arc::get_mut` on any of them (the four `make_mut` sites are on a rest grid,
a polygon set and two cut traces — verified by grep across `crates/*/src`).

### 3.2 The tool is the one input with no stable identity — and how it is keyed

`SimToolpathEntry::tool` is a `ToolDefinition` **by value**, rebuilt from the
session's `ToolConfig` on every request (`build_cutter(&tp.tool)` in the viz
worker), so there is no `Arc` to pin. Its `cutter: Box<dyn MillingCutter>` is
not `Serialize`, and the trait carries no `Any` bound — the same wall
`DELTA_gen_w4.md` hit when it declined full devirtualization.

`hash_tool` therefore keys on the trait's own observable surface, in two
deliberately redundant halves:

1. **Parametric** — `geometry_hint()` is a complete parameterisation of every
   shipped shape (`Flat`, `Ball`, `Bull{corner_radius}`,
   `VBit{included_angle, tip_diameter}`,
   `TaperedBall{tip_radius, taper_angle_deg}`), plus `diameter`, `radius`,
   `envelope_radius_mm`, `cusp_radius`, `length`, `helix_deg`,
   `corner_radius_mm`, the four assembly dimensions, `flute_count` and
   `tool_material`.
2. **Probed** — `height_at_radius` at **64** radii (this *is* the profile
   `RadialProfileLUT::from_cutter` samples, i.e. the exact function the stamp
   kernel consumes), `engagement_radius_mm` at 9 depths, and `chip_geometry`
   at 6 (doc, arc, feed-per-tooth) points.

**Stated residual risk, not hidden:** a future cutter shape whose behaviour is
not a function of the hint plus these probes would need this extended. The
probe half is what makes that unlikely rather than merely unchecked — such a
shape would have to agree at *none* of 79 evaluations to collide.
`tool_key_separates_every_shipped_shape` pins it, and it opens with a
**positive control**: identical tools must produce a hit, so the four
"different tools must miss" assertions cannot pass by the cache never hitting.

### 3.3 Phantom prior stock — the input that must NOT be keyed and must NOT be restored

`SimGroupEntry::phantom_prior_stock` (F.4) records the one position at which a
*not yet generated* `FromRemainingStock` op receives a `prior_stocks`
snapshot. **It moves down the group on every fixpoint round** — round 1 gives
it to op j+1, round 2 to op j+2 — which is precisely the thing that changes
between the two runs this cache exists to connect. Putting it in the key
makes the memo hit **zero times** on its only intended workload.

Both alternatives are wrong for the same reason from opposite directions:

* keying on it ⇒ never hits (a silent no-op — the `DELTA_sim_w2.md` §2e
  failure mode);
* restoring it ⇒ the previous round's phantom entry, keyed by an id the
  current request's phantom is no longer for, survives into `prior_stocks`.
  A full replay would never produce that key. That is a **wrong result**, on
  the exact map the rest-machining generators read.

What landed: the snapshot stores `prior_stocks` **restricted to the ids of
the entries actually replayed** (`simulate::replayed_prior_stocks`), and the
phantom is re-derived from the *live* request after restore
(`simulate::rederive_phantom_prior_stocks`). Inside the replayed region the
phantom shares its `Arc` with the entry at that position — exactly what the
live path does (`Arc::clone(&pre_carve_stock)`) — so it is recovered from the
restored map; at or beyond the resume point the normal loop inserts it.

**One case is refused rather than approximated.** `phantom_k ==
toolpaths.len()` on a group *strictly before* the resume group is that
group's fully carved stock, which the run drops when it moves on. The lookup
returns a miss and ticks `phantom_refusals`. It cannot arise in the shape S5
targets (a single-setup project resumes inside its only group), and a missed
hit costs a replay, not correctness.
`phantom_prior_stock_is_rederived_not_restored` and
`a_tail_phantom_on_an_earlier_group_refuses_the_hit` pin both halves — the
latter with a paired case on the same fixture shape that *does* hit, so the
refusal is a rule and not a blanket.

---

## 4. Memory — the policy, and the type change that made it cheap

The naive version of this doubles the simulator's peak: a prefix snapshot is
most of a full result, and the heaviest per-toolpath artifact by a wide
margin is the **checkpoint** — a marching-cubes mesh plus a full
`TriDexelStock` clone, one per toolpath.

So `SimulationResult::checkpoints` is now **`Vec<Arc<SimCheckpointMesh>>`**,
and the GUI's `SimCheckpoint` holds that same `Arc` behind `mesh()` /
`stock()` accessors instead of owning a deep copy. The snapshot then shares
its checkpoints with the result it came from and adds **a refcount, not a
copy**. This also removes a deep copy that predates S5: the controller used
to move mesh + grid out of every core checkpoint into a GUI-side struct.

The rest of the policy:

* **At most one snapshot.** No table, no eviction ordering to get wrong.
* **Lookup takes.** `take_match` removes the snapshot whether it hits or
  misses. A miss means it is stale, and a stale snapshot is pure memory.
* **`store` is opt-in per request.** Only the `generate_all` fixpoint ladder
  sets `memoize_prefix: true`. Every other simulation still *uses* a held
  snapshot when it matches (free) but leaves none, so the memo is released at
  the next run instead of being retained for the session.
* **Explicit `clear()`.** `settle_generate_all_round`'s `Next::Finish` arm
  calls `ComputeBackend::clear_sim_prefix_cache`, so the ladder that asked
  for retention is the thing that releases it, at a known point, without
  waiting for a later simulation to consume it. The trait method is defaulted
  to a no-op so no test backend had to change.
* **A hard size ceiling.** `SimPrefixCache::max_bytes`
  (`DEFAULT_MAX_BYTES = 1.5 GB`) against `PrefixState::estimated_bytes`,
  which counts what the snapshot *adds* — `Arc`-shared checkpoints and prior
  stocks at pointer cost. Over it, the snapshot is dropped and
  `size_refusals` ticks. `the_size_ceiling_refuses_rather_than_growing` pins
  that the refusal is real and costs correctness nothing.

**What the snapshot genuinely copies**, in descending size: the prefix's
`cut_samples` (the dominant item — at wanaka scale a 600 k-sample stream,
each with its own `span_path` `Vec`), the `global_stock` and `group_stock`
grids (~8 MB each at 0.4 mm over a 200 mm stock), and `composite_mesh`
(empty until the first group ends, so zero for a single-setup project).
Everything else is pointers or small vectors.

---

## 5. Measured — paired, same session

### 5.1 The bench

`cargo bench -p rs_cam_core --bench hot_paths -- sim_fixpoint_ladder`,
2026-08-20, load average 6.09, 24 GiB available, `pgrep -x cargo` **0** at
launch, both arms in one criterion invocation minutes apart.

New group `sim_fixpoint_ladder`, deliberately built as a **paired
same-session A/B in one criterion group**: both arms run the identical
three-round ladder (2 → 4 → 6 toolpaths over a 100 × 60 × 8 mm stock at
**0.4 mm**, the wanaka reference resolution), and the only difference is
whether each round leaves a prefix snapshot for the next. `memo_off` is the
pre-S5 behaviour exactly — three full replays. Cross-day absolutes on this
box are not comparable (`BASELINES.md`, "Measurement discipline"), which is
why both arms are in the tree rather than one arm plus a stored number.

| Bench | memo_off | memo_on | Speed-up |
|---|---:|---:|---:|
| `sim_fixpoint_ladder/3round_6op_res0.4` | **180.37 ms** [177.32, 184.11] | **100.66 ms** [98.605, 103.63] | **1.79×** |

### 5.2 What the ratio can and cannot be

The ceiling is arithmetic and worth stating so nobody quotes **1.79×** as a
general figure. A `k`-round ladder over an `n`-op project runs `Σ nᵢ`
op-simulations without the memo and only the newly generated ones with it
(plus the per-round end-of-group and end-of-run work, which is paid either
way). For the 2/4/6 ladder that is **12 → 6** op-sims, i.e. a ceiling of
**2.00×** on the ladder as a whole. Measured 1.79× is **90 % of the
achievable**, and the residual is the three rounds' unavoidable
end-of-run work (trace assembly, marching cubes, composite mesh) plus the
snapshot copy.

Two consequences follow, and both are limits rather than caveats:

* **A longer chain wins more.** The ceiling for a `k`-round ladder adding
  `m` ops per round is `(k+1)/2`, so a 5-round rest cascade tops out near
  3×, not 1.79×. The finding's own severity line ("HIGH on rest chains")
  is the right shape.
* **Nothing here helps a project with no rest ops.** `generate_all` on such
  a project runs `FixpointPlan::single_pass` and never simulates at all.

The wanaka shape (3 rounds / **2** simulations, 8 ops) is different again:
the second simulation's prefix is whatever round 1 generated, so the saving
is that prefix's share of one 84 s simulation. The 0C protocol is what
measures it end-to-end; this bench measures the mechanism.

### 5.3 Memory, measured

`the_snapshot_shares_checkpoints_rather_than_copying_them`, on the 3-op /
0.5 mm sentry fixture:

| | bytes |
|---|---:|
| snapshot's own footprint (`held_bytes`) | **786,048** |
| checkpoint bytes it **shares** rather than copies | **1,561,680** |

`Arc::strong_count` on every checkpoint is **2** while the snapshot is held
and **1** after `clear()` — the sharing claim as an assertion rather than as
prose. Without the `Vec<Arc<SimCheckpointMesh>>` change the snapshot would
have been ~3× its measured size on this fixture, and the ratio grows with
toolpath count because checkpoints are per-toolpath while the grids are not.

---

## 6. An incidental defect found on the way — NOT fixed, NOT mine

`dexel_stock/stamping.rs:504` panics in debug with **"attempt to subtract
with overflow"** when a stamp's bounding box falls entirely outside the grid:

```rust
let col_hi = (col_max as usize).min(grid.cols.saturating_sub(1));
let row_hi = (row_max as usize).min(grid.rows.saturating_sub(1));
…
let bbox_cells = (row_hi + 1 - row_lo).saturating_mul(col_hi + 1 - col_lo);
```

With `row_min > rows - 1` the clamps give `row_lo > row_hi` and
`row_hi + 1 - row_lo` underflows. Reproduced while building this wave's
fixture: a raster pass at `y ∈ [31, 35]` over a stock whose Y extent is
`[0, 24]`. It is **pre-existing** — the `bbox_cells` line is S2's mip block
(`f9f26997`), nothing in S5 touches `dexel_stock/` — and it is reachable from
an ordinary project, because toolpaths legitimately move outside the stock
(profile lead-ins, edge drills, any op whose boundary extends past the
blank). Release builds wrap instead of panicking, so debug and release do not
agree, which is the same class as the `debug_assert!`-in-a-dependency note
CLAUDE.md already carries for the offset library.

Two things in the same expression are worth a second look when it is fixed:

* the `usize` cast of a possibly-**negative** `col_max` yields a huge value
  that `min` clamps to `cols - 1`, so a stamp entirely to the *left* of the
  grid iterates the whole column range instead of none. Results are
  unaffected (per-cell coverage rejects), but the work is not.
* `row_hi + 1` / `col_hi + 1` can overflow in principle for the same reason.

Left unfixed on purpose: `dexel_stock/` is the S3 lane's queued territory and
the wave brief scopes this lane to additive changes there. Routed to the
consolidator rather than patched silently. The S5 fixture avoids it and says
so at the line.

---

## 7. Correctness

* **Both perf goldens green and UNCHANGED** (`sim_metrics_match_golden`,
  `sim_metrics_3d_match_golden`, `perf_golden_depth_level_geometry`, plus both
  non-vacuity companions); **no re-baseline**.
* `cargo test -p rs_cam_core`: **exit 0**, 184 test binaries, **zero
  failures**.
* `cargo test -p rs_cam_viz -q`: **exit 0**, zero failures.
* `cargo clippy --workspace --all-targets -- -D warnings`: **clean**.
* `cargo fmt --check`: clean for this lane's files. The diffs it still
  reports are in `edge_distance.rs`, `face.rs`, `inlay.rs`, `rest.rs`,
  `pushcutter_band_query_g1.rs` and `sub_cell_stamping_fa.rs` — none touched
  by this lane, all pre-existing (the `cargo fmt` cascade over them was
  reverted, per the house rule).

### New nets

| Test | What it pins |
|---|---|
| `resumed_run_is_bit_identical_to_a_full_replay` | the whole `SimulationResult`, as bit patterns with no tolerance, resumed vs replayed — **and** that the fixture actually cut (>500 samples, >100 mm³) |
| `a_three_round_ladder_resumes_at_increasing_depth` | two consecutive resumes, each re-snapshotting deeper; `hits == 2`, `entries_reused == 6` |
| `a_regenerated_prefix_toolpath_misses_and_the_result_still_matches` | a new `Arc<AnnotatedToolpath>` inside the prefix misses, and the miss is still correct |
| `a_changed_resolution_or_stock_misses` | seven global inputs, one at a time: resolution, stock bbox, stock top, RPM, rapid feed, and both metric-option flags |
| `tool_key_separates_every_shipped_shape` | shape, stickout and flute-count changes all miss — opened by a **positive control** so the misses are not vacuous |
| `phantom_prior_stock_is_rederived_not_restored` | the previous round's phantom key must not survive; the current round's must appear — interior *and* tail position |
| `a_tail_phantom_on_an_earlier_group_refuses_the_hit` | the un-recoverable case refuses and counts it, paired with the recoverable case that still hits |
| `a_multi_group_prefix_resumes_inside_the_last_group` | earlier groups' end-of-group work (composite append, deviations) survives the restore |
| `a_non_storing_run_consumes_the_snapshot_and_leaves_nothing` | the memory bound for ordinary simulations |
| `the_size_ceiling_refuses_rather_than_growing` | the ceiling is a refusal, not a comment |
| `clear_releases_the_snapshot` | the GUI's release point |
| `a_shorter_request_misses_cleanly` | the memo holds state at one depth only |
| `sim_prefix::tests::snapshot_bytes_estimate_ignores_arc_shared_checkpoints` | the size estimate counts what the snapshot *adds* |

Every test that expects a hit asserts on `SimPrefixStats::{hits,
entries_reused}`. **A memoization that never hits is a silent no-op** — S2's
first `REFRESH_VISIT_MULTIPLIER` fired zero times across the whole suite and
everything stayed green because a stale mip is *sound* (`DELTA_sim_w2.md`
§2e). An inert S5 fails red.

---

## 8. Commits and follow-ups

| Commit | Contents |
|---|---|
| `2ed9df04` | `compute/sim_prefix.rs`, the `run_simulation_memoized` restructure, `Vec<Arc<SimCheckpointMesh>>`, the viz worker/controller wiring and the `clear_sim_prefix_cache` release point |
| `55847d4f` | `tests/sim_prefix_memo_s5.rs` (14 sentries) and the `sim_fixpoint_ladder` bench arm |

Planning edits (this file, `PERF_REVIEW.md`'s S5 annotation, the
`BASELINES.md` rows) are left **UNSTAGED** for the consolidator, per the wave
protocol.

### Deferred: `simulate_candidate_isolated` needs a metrics-only mode

`session/compute.rs::simulate_candidate_isolated` runs a **full**
`run_simulation` — global-stock stamping (a second stamp of every move),
checkpoint capture (marching cubes plus a grid clone), composite mesh
extraction, `prior_stocks` snapshotting — and then throws all of it away,
keeping only `cut_trace`. The strategy advisor calls it once per candidate
strategy.

Not taken in this wave, and the reason is specific rather than budgetary: a
metrics-only mode is a *second* orthogonal behaviour switch inside the same
loop S5 just restructured, and — this is the part that matters — **it would
have to enter the prefix cache key**. A prefix carved with checkpoints
suppressed is not interchangeable with one carved with them on; a mode flag
outside the key is exactly the class of defect §3.1 exists to prevent. It
deserves its own wave with its own net rather than a rider on this one.

Sketch for whoever takes it: a `SimulationOutputs` bitset on
`SimulationRequest` (`checkpoints`, `global_stock`, `composite_mesh`,
`prior_stocks`, `deviations`), defaulted to everything;
`simulate_candidate_isolated` asks for `cut_trace` only; the bitset joins
`hash_global_scalar`. Expected win on that call site is most of the non-stamp
cost plus one of the two stamps, i.e. well over half.
