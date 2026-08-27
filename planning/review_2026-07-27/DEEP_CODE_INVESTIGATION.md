# Deep code investigation — 2026-07-27

Scope: `rs_cam` at `2ca6d0d`.

## Findings

### 1. `[high]` `apply_link_moves` treats arc chords as already swept

`crates/rs_cam_core/src/dressup.rs:1215-1239`

`bridge_corridor_is_swept` accepts every non-rapid move, then measures coverage against the straight segment between its endpoints. For `ArcCW`/`ArcCCW`, that segment is the chord—not the cutter’s actual arc path.

I reproduced this with a semicircular cut from `(-5,0)` to `(5,0)`. A candidate bridge across the unswept diameter was approved and emitted as:

```text
Linear feed: (5,0,-1) -> (-5,0,-1)
```

This bypasses the recently added corridor safety check and can gouge uncut material.

**Recommended immediate fix:** conservatively exclude arcs from swept-history evidence. Then add proper arc linearization using `arc_util::linearize_arc` and a regression test for the semicircle/chord case.

---

### 2. `[high]` TSP reassembly can change or delete the first cut of every segment

`crates/rs_cam_core/src/tsp.rs:49-50,75-76,383-393`

`split_into_segments` records `Segment::start` as the first non-rapid move’s **target**, not that move’s source. `rebuild_group` rapids above that target and then replays the move.

If a fragment begins with a lateral cutting move after a rapid at cutting Z, the lateral move becomes a vertical plunge at its endpoint. A probe with two 10 mm lateral cuts produced:

```text
original: (0,0,-1) -> (10,0,-1)
rebuilt:  (10,0,10) -> (10,0,-1)

original: (20,0,-1) -> (30,0,-1)
rebuilt:  (30,0,10) -> (30,0,-1)
```

The same issue can invalidate a first arc because its `i/j` offsets remain relative to the original source.

**Recommended fix:** preserve each fragment’s true entry source and required approach moves. Add a structural test comparing complete swept moves `(source, move type, target)`, not only move targets or total cutting distance.

---

### 3. `[high]` The 2-opt cost model is confirmed wrong—and the proposed endpoint correction alone is insufficient

`crates/rs_cam_core/src/tsp.rs:325-341` versus `tsp.rs:359-395`

The current formula assumes every segment reverses orientation. Reassembly preserves each segment forward.

Additionally, reversing a block changes **all internal directed junctions**, not only the two boundary edges. Therefore changing `order[j].end` to `.start` is not enough.

A deterministic four-segment probe showed:

```text
nearest-neighbor order: [0, 2, 1, 3]
actual XY junction cost: 34.7963 mm

2-opt result:           [0, 2, 3, 1]
actual XY junction cost: 40.7144 mm
```

The incorrect formula believed the accepted swap improved `13.1529 → 13.0000`, while actual total travel worsened by 17%.

**Recommended immediate fix:** disable the 2-opt phase while replacing it. A forward-only relocate/or-opt neighborhood is a better fit: it preserves traversal direction, changes only a small number of directed edges, and can run in roughly O(N²).

---

### 4. `[high]` `filter_air_cuts` can delete cuts crossing remaining material

`crates/rs_cam_core/src/dressup.rs:1446-1564`

Classification samples only:

- source,
- target,
- and, for arcs, the circle center—which is not on the arc.

The `_tool_radius` argument remains unused.

I reproduced a line from `x=10` to `x=90` whose endpoints were cleared but whose middle crossed a retained material island at `x=40..60`. The filter removed both cutting moves:

```text
cutting moves before=2
cutting moves after=0
```

Arc circumferences can fail identically when endpoints and center are in air. Cutter-flank contact is also missed when the centerline is clear but material lies inside the cutter radius.

**Recommended fix:**

- sample linear moves at dexel-scale spacing;
- linearize arcs along their actual sweep;
- conservatively inspect the cutter-radius neighborhood;
- only classify a move as air when every swept sample is clear.

Also, `ShorterThanAirPath` computes chord lengths for arcs at `dressup.rs:1604-1617`, so its cost decision is incorrect even after classification succeeds.

---

### 5. `[high]` The positional `apply_dressups` API has already enabled a stock-top wiring error

`crates/rs_cam_core/src/compute/execute.rs:2173-2187`

`apply_entry` explicitly requires the top of **uncut stock**:

- contract: `dressup.rs:48-52`
- rapid safety floor: `dressup.rs:340-341,401-402`

The session path correctly supplies:

```text
session/compute.rs:1456 — emission_stock_bbox.max.z
```

The GUI worker supplies:

```text
compute/worker/helpers.rs:60 — req.heights.top_z
```

A manually lowered machining `top_z` is not necessarily the physical stock top. In that case, GUI ramp/helix entry can calculate a rapid floor below uncut material, while the session path remains safe.

The same API also prevents the already-available `link_kinematics` from reaching TSP, `apply_link_moves`, and `AirBridgePolicy`.

**Recommended fix:** introduce a `DressupContext`/params struct with semantically named fields, including:

- physical stock top,
- machining heights,
- cutter geometry,
- machine/link kinematics,
- machining boundary,
- prior/feed-optimization stock,
- transform capabilities and trace contexts.

Fix the GUI stock-top argument independently rather than waiting for the refactor.

---

### 6. `[medium]` Stay-down decisions remain inconsistent despite the boundary fix

Relevant implementations:

- `pencil.rs:827-904`
- `unified_finish.rs:1472-1549`
- `surface_link.rs:142-305`
- `dressup.rs:1168-1395`

Current state:

- `unified_finish::choose_link` checks the machining boundary.
- `surface_link::relink_fragments` now checks its region boundary at `surface_link.rs:265`.
- `pencil::emit_paths` has no boundary input.
- `apply_link_moves` uses a different already-swept-path oracle—and currently has the arc-chord hole above.
- Pencil retracts for a zero-length gap (`pencil.rs:861`), while `relink_fragments` keeps the tool down (`surface_link.rs:242`).
- `apply_link_moves` uses distance/cap only and can choose a slower link because machine kinematics are unavailable.

These are not all the same safety problem: mesh-following links and already-cleared straight links need different safety oracles. Consolidate the shared candidate construction, boundary gate, cost comparison, verdict/reason reporting, and tie policy—not the geometry-specific proof itself.

One remaining risk is Unified Finish crease emission at `unified_finish.rs:886`: it calls Pencil’s boundary-unaware emitter with the default 5 mm hookup distance.

---

### 7. `[medium]` Test-vacuity protections improved, but concrete gaps remain

`capability_link_moves_safety.rs` now has good additions:

- capability guard,
- `expect_links_applied`,
- maximum-depth bound,
- structural swept-segment checks,
- fixture cardinality/depth-level checks.

Remaining gaps:

1. `adaptive3d_keep_down_link_f038b.rs:422-523` does not discriminate the cutter-length guard. Observed:

   ```text
   short cutter on/off = 24/27
   long cutter on/off  = 24/27
   ```

   Yet `long_drop >= short_drop` passes. Deleting the cutter-length guard would likely remain green.

2. Chamfer and DropCutter link-neutrality tests explicitly expect no links at `capability_link_moves_safety.rs:661,1993`. They pin capability plumbing but cannot detect deletion of link behavior.

3. ProjectCurve’s neutrality gate at `capability_link_moves_safety.rs:1085-1095` computes `max_d` but only bounds the fraction of changed cells. A deep, narrow gouge can still pass. Use the exact swept-move oracle plus a depth bound.

4. The Scallop test contains stale contradictory reasoning around `capability_link_moves_safety.rs:1266-1301`: it claims monotonic dexel removal makes any material difference proof of changed geometry, while later sections correctly acknowledge fractional blending is order-dependent.

A mutation-oriented rule would help: every feature test should prove the feature fired and would fail if its transform were replaced by identity.

---

### 8. `[medium]` `ray_blend_above/below` are order-dependent by construction; this is known but operationally significant

`crates/rs_cam_core/src/dexel.rs:97-153`

For a ray `[0,10]`:

- same surface `6`, `f=0.5`: `10 → 8 → 7`, so it is not idempotent;
- surface `6` then `4`: final top `6.0`;
- surface `4` then `6`: final top `6.5`, so it is not commutative.

Material amount is still monotonic—it never increases—but “final stock equals the order-independent union of swept volumes” is false at partial coverage.

The current branch already acknowledges this in:

- `capability_link_moves_safety.rs:355-359,1683-1709`
- `planning/unified_v3_design.md:827+`

No production assertion explicitly assumes idempotence, but the effect propagates operationally through simulation `prior_stocks`, which feed remaining-stock generation and air-cut filtering. `coverage_max` only stores the maximum observed coverage and cannot reconstruct sub-cell overlap.

A real fix requires sub-cell-resolved occupancy/rays or another union-preserving representation. Canonicalizing stamp segmentation/order can reduce drift but cannot make the current blend algebra correct.

---

### 9. `[medium]` The 500-segment cutoff and XY objective are real limitations, but should be addressed after TSP correctness

`crates/rs_cam_core/src/tsp.rs:307-317`

Groups above 500 still receive nearest-neighbor optimization, but no local improvement. That excludes the reported 1,348/1,535-fragment groups.

The current code’s documented objective is rapid XY distance, so this is an objective mismatch rather than a hidden implementation bug. For cycle-time optimization, use a directed edge cost based on `retract_link_time` or an equivalent kinematic model.

Do not simply raise the threshold: the current reversal is both cubic in implementation and incorrect. A forward-only relocate heuristic should scale far better.

## Recommended order

1. **Block the arc-chord gouge:** ignore arcs in `bridge_corridor_is_swept` until properly linearized.
2. **Fix TSP geometry preservation:** retain each fragment’s true source/approach.
3. **Disable or replace directed-invalid 2-opt.**
4. **Make air-cut classification swept-path and tool-radius aware.**
5. **Fix GUI physical stock-top wiring.**
6. Introduce `DressupContext` and shared link decision/cost components.
7. Strengthen deletion-sensitive tests.
8. Treat dexel sub-cell order independence as a dedicated representation change.

## Verification

- `cargo test -p rs_cam_core -q tsp::tests` — 10 passed.
- Focused `filter_air_cuts` tests — 6 passed.
- Focused `surface_link` tests — 5 passed.
- Adaptive3D cap probe — default/override entries `42/2`.
- Adaptive3D cutter-length probe — short and long both `24/27`, confirming the test gap.
- Three standalone probes confirmed:
  - TSP travel worsening,
  - TSP first-cut source loss,
  - arc-chord link approval.
- One standalone probe confirmed air-filter deletion across a material island.
