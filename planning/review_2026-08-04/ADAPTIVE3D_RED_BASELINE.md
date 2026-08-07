# W0 / H0 / R3 — baseline for the three permanent `adaptive3d` reds

Date: 2026-08-04
Revision measured: `9f52378` (branch `experiment/adaptive-spiral`), working tree
carrying only the untracked/never-staged planning paths noted in the plan.
Basis: `TECH_DEBT_2_RESEARCH_AND_FIX_PLAN.md` §H0 / R3.
Status: **RESEARCH ONLY.** No production code, no existing test, and no fixture
was modified to produce this document. All three verdicts below are *proposals*
for human Checkpoint A.

Artifacts referenced here live under `planning/review_2026-08-04/artifacts/w0/`:

| File | What it is |
|---|---|
| `path_fixture_moves.txt` | derived move tables for tests 1 and 2, pre- and post-drape |
| `peck_fixture_xz.svg` | test 1 — XZ cross-section, expected vs actual plunge ladder |
| `rapid_fixture_xz.svg` | test 2 — XZ profile, requested vs emitted path |
| `parity_agent_search_run1.txt` | verbatim `--nocapture` transcript of the test 3 failure |
| `parity_bisect_evidence.txt` | test 3 bisect probe log + directional counters either side of the first bad commit |
| `parity_evidence.svg` | test 3 — the three-panel evidence figure behind the `FIX_CODE` verdict |
| `parity_violations_agent_search.svg` | test 3 — the harness's 20 sampled interior violations (a row-major sample, not the full field) |
| `render_*.py` | standalone generators for the above; they read source and transcripts only and touch no crate file |

## 0. Summary

| # | Test | Deterministic? | Mechanism class | Proposed verdict |
|---|---|---|---|---|
| 1 | `adaptive3d::path::tests::peck_plunge_progresses_when_depth_per_pass_equals_retract_clearance` | **yes** — 3/3 identical | fixture assumption invalidated by a later production fix; the asserted contract is still correct and still implemented, but the fixture no longer reaches it | `FIX_TEST` |
| 2 | `adaptive3d::path::tests::rapid_segment_lifts_to_safe_z_before_traverse` | **yes** — 3/3 identical | same fixture invalidation; the test's geometric landmark move no longer exists | `FIX_TEST` |
| 3 | `adaptive3d::tests::planner_sim_dexel_parity_agent_search` | **yes** — 3/3 bit-identical | real planner/emitter divergence: the emitter gained a drape transform (`fa27b08`) that the planner's mirror stamp never applied. **Bisected.** | `FIX_CODE` |

**All three reds have the same first bad commit, `fa27b08`** (2026-06-16,
*hold stock-to-leave over textured meshes — drape / gouge guard*): bisected for
test 3 over 343 revisions, and the closed-form cause of tests 1 and 2. They are
not three independent defects — they are one un-mirrored transform, showing up
once in the emitter's own unit tests and once in the planner/simulator parity
gate. `fa27b08` is itself a correct fix and is **not** proposed for reverting.

---

## 1. `peck_plunge_progresses_when_depth_per_pass_equals_retract_clearance`

### 1.1 Command, duration, result, failure text

```
cargo test -p rs_cam_core --lib \
  adaptive3d::path::tests::peck_plunge_progresses_when_depth_per_pass_equals_retract_clearance \
  -- --exact --nocapture
```

| | |
|---|---|
| Duration | 0.00 s test body; 0.24 s wall on a warm binary (29.4 s including the cold `rs_cam_core` build) |
| Runs | 3 |
| Result | **FAILED, 3/3 — deterministic**, byte-identical failure text each time |

```
thread 'adaptive3d::path::tests::peck_plunge_progresses_when_depth_per_pass_equals_retract_clearance'
panicked at crates/rs_cam_core/src/adaptive3d/path.rs:1676:9:
assertion `left == right` failed
  left: 1
 right: 2
```

There is no wall-clock component and no parallelism in this test; flakiness is
excluded by construction as well as by repetition. The failure is semantic.

This is exactly the value predicted from source before the test was run
(§1.4) — the closed-form derivation and the observed failure agree at
`left: 1`.

### 1.2 Provenance and first-bad commit

| Item | Value |
|---|---|
| Test source | `crates/rs_cam_core/src/adaptive3d/path.rs:1655-1677` |
| Test introduced | `072c11a` — *fix(unification): land F-001/F-002/F-003/F-007/F-008/F-013 batch*, 2026-05-25 |
| Test body changed since | **never** — `git log -S` on the test name returns exactly one commit |
| Production under test | `segments_to_toolpath` (`path.rs:1193`), `emit_peck_plunge` (`path.rs:39`), `drape_point` (`path.rs:1140`) |
| **First bad commit** | **`fa27b08`** — *fix(adaptive3d): hold stock-to-leave over textured meshes (drape / gouge guard)*, 2026-06-16 |
| Bisected? | **Not bisected by build for *this* test, and it does not need to be.** `git log -S "drape_point"` returns `fa27b08` as the sole commit that ever introduced the mechanism; the failure is closed-form arithmetic on the fixture constants (§1.4) and the predicted value matched the observed one exactly. Independently, the §3 build bisect — 9 probes over 343 revisions — landed on **the same commit**. Three reds, one cause, one commit. |
| First written acknowledgement of the red | `4b105da` (2026-07-06) — "Known reds (all pre-existing, HEAD-verified)". Three weeks of silent carry. |

`fa27b08`'s own commit body lists the sentries it verified —
"F-024/F-027/F-029-031/F-038/F-038b/lift-bridge/monotonicity/subtool" — and
does **not** mention `rs_cam_core --lib`. The two unit tests in the very file it
edited were never re-run. This is unreported collateral, not an accepted
trade-off.

### 1.3 Intended contract, in operator language

> When the operator sets a peck depth equal to the peck retract clearance
> (0.5 mm, `PECK_CLEARANCE_MM`), the entry plunge must still walk down the hole.
> It must not retract exactly as far as it just descended and try again from the
> same height forever.

That is a hang/livelock guard, not a machining-quality dial. The production code
still honours it: `emit_peck_plunge` advances `current_z = next_z` — the
**committed cut floor** — rather than `retract_z`. `path.rs:53-56` documents
precisely this. **The contract is intact and correctly implemented at HEAD.**

### 1.4 Minimal reproducer and mechanism

The fixture is:

| Constant | Value | Source |
|---|---|---|
| mesh | `make_test_flat(100.0)` — a flat quad at **z = 0**, x,y ∈ [-50, 50] | `path.rs:1607`, `mesh.rs:769` |
| `stock_to_leave` | 0.5 | `minimal_params`, `path.rs:1623` |
| `safe_z` | 1.0 (overridden in the test) | `path.rs:1658` |
| `depth_per_pass` | 0.5 (overridden in the test) | `path.rs:1659` |
| segment | `Rapid(P3::new(0, 0, 0))` | `path.rs:1661` |

`fa27b08` inserted, at `path.rs:1261`, a shadowing rebind on the `Rapid` arm:

```rust
let entry_owned = drape_point(entry, mesh, index, cutter, params.stock_to_leave);
let entry = &entry_owned;
```

`drape_point` returns `z.max(drop_cutter(x, y) + stock_to_leave)`. The fixture's
entry XY (0, 0) sits on the flat mesh, so `drop_cutter = 0.0` and the entry
becomes **z = +0.5**, not 0.0.

`emit_peck_plunge` then runs `while current_z - entry.z > dpp + 1e-6`, i.e.
`1.0 - 0.5 > 0.5 + 1e-6` → **false**. The loop body never executes, so the
single terminal `feed_to_with_intent(entry, …, EntryPlunge)` is the only
`EntryPlunge` emitted.

| | entries | moves | `EntryPlunge` count |
|---|---|---|---|
| `fa27b08^` (pre-drape) | (0, 0, 0.0) | 5 | **2** ✅ |
| HEAD (post-drape) | (0, 0, **+0.5**) | 3 | **1** ❌ |

**Cross-check that the model is right, not just plausible:** the same reasoning
predicts that the third test in this module,
`rapid_segment_skips_redundant_lift_when_already_at_safe_z`, is *unaffected* —
its `tp` is empty when the `Rapid` arrives, so `lift_to_safe_z` is a no-op and
its only assertion is on `moves[0]`, the approach rapid to
`(entry.xy, safe_z = 10.0)`, whose Z the drape cannot touch. That test is green.
A model of the mechanism that gets the green/red split right across all three
tests in one module is worth more than one that only explains the failures.

Derivation and both move tables:
`planning/review_2026-08-04/artifacts/w0/path_fixture_moves.txt`
(generated by `artifacts/w0/render_path_fixture.py`, a standalone mirror of the
`path.rs` arithmetic — it touches no crate source).

Render (XZ cross-section at the entry XY, both branches side by side):
`planning/review_2026-08-04/artifacts/w0/peck_fixture_xz.svg`

### 1.5 The test is also **vacuous**, not merely red

This matters more than the red itself. Because the loop never iterates, the
fixture no longer exercises the "`dpp == PECK_CLEARANCE_MM` must still progress"
condition the test is *named* for. Reverting `emit_peck_plunge`'s progression
fix (`current_z = next_z` → `current_z = retract_z`) would **not** change this
test's output at HEAD: with zero iterations there is nothing to not-progress.
The livelock guard has been unguarded since 2026-06-16.

Any Checkpoint-A fix must therefore restore non-vacuity, not merely restore
green.

### 1.6 Proposed verdict: `FIX_TEST`

Rationale: the production contract is right, the production code honours it, and
the *fixture* is the thing that stopped being coherent. `make_test_flat` at
z = 0 is the **finished surface**; asking the planner to enter at z = 0 while
holding 0.5 mm of stock above that surface is a contradiction under
post-`fa27b08` semantics. Weakening the assertion to `entry_plunges == 1` would
green the gate while permanently deleting the guard — explicitly forbidden by
plan §H0 "Preferred fix shape" item 1.

### 1.7 Proposed red-first sentry

Name: `peck_plunge_loop_progresses_when_dpp_equals_peck_clearance`
(replaces the current test in place; the old failure text goes in the commit
body per plan §H0 item 3).

Shape:

1. Put the fixture mesh **below** every commanded Z so the drape is provably a
   no-op — e.g. a flat mesh at z = -50, or assert `drape_point` returned the
   input. Do not disable the drape.
2. `safe_z = 5.0`, `entry.z = 0.0`, `depth_per_pass = PECK_CLEARANCE_MM = 0.5`,
   so the loop must iterate **10** times.
3. **Non-vacuity assertion, pre-registered:** `entry_plunges >= 3`. This is the
   assertion that fails on the parent revision if `current_z = next_z` is
   reverted to `current_z = retract_z` — under that revert the loop never
   descends past the first peck and the emission either stalls or is bounded by
   the test's own move cap.
4. Keep the `moves.len()` runaway cap, sized from the pre-registered iteration
   count (e.g. `<= 4 * expected_pecks`), so a livelock still trips a bound
   rather than hanging the suite.
5. Red-first proof required by plan rule 13: patch `emit_peck_plunge` to
   `current_z = retract_z` on a scratch revision, show the new sentry fails,
   revert.

### 1.8 Blast radius

| Surface | Assessment |
|---|---|
| `emit_peck_plunge` | 2 call sites, both in `segments_to_toolpath` (`path.rs:1312`, `path.rs:1444`). Not called from anywhere else in the crate. |
| `minimal_params()` | shared by all 3 tests in `adaptive3d::path::tests`. Changing `stock_to_leave` there would perturb test 3 in that module (`rapid_segment_skips_redundant_lift_when_already_at_safe_z`, currently green). **Prefer a per-test mesh override over editing `minimal_params`.** |
| `legacy_test_mesh()` | 3 call sites, all in `adaptive3d::path::tests`. Its doc comment ("the heightfield never gets queried") is **stale since `fa27b08`** and must be corrected in the same commit. |
| Production | **none** under `FIX_TEST`. |
| Other suites | none — `segments_to_toolpath` is `pub(super)`; no integration test reaches it directly. |

---

## 2. `rapid_segment_lifts_to_safe_z_before_traverse`

### 2.1 Command, duration, result, failure text

```
cargo test -p rs_cam_core --lib \
  adaptive3d::path::tests::rapid_segment_lifts_to_safe_z_before_traverse \
  -- --exact --nocapture
```

| | |
|---|---|
| Duration | 0.00 s test body; 0.24 s wall on a warm binary |
| Runs | 3 |
| Result | **FAILED, 3/3 — deterministic** |

```
thread 'adaptive3d::path::tests::rapid_segment_lifts_to_safe_z_before_traverse'
panicked at crates/rs_cam_core/src/adaptive3d/path.rs:1717:26:
cut1 endpoint not found
```

`path.rs:1717` is the `.expect("cut1 endpoint not found")` on the landmark
search — again exactly the failure predicted from source in §2.4 before the run.

### 2.2 Provenance and first-bad commit

| Item | Value |
|---|---|
| Test source | `crates/rs_cam_core/src/adaptive3d/path.rs:1679-1754` |
| Test introduced | `3ab7605` — *adaptive3d: lift to safe_z before rapid XY traverse in `segments_to_toolpath`* |
| Test body changed since | **never** (`git log -S` returns one commit) |
| Cited regression record | `planning/adaptive_remediation_phase2_probes_2026-04-12.md` (F-5 / F-6) |
| **First bad commit** | **`fa27b08`** (same as §1) |
| Bisected? | **Not bisected by build for *this* test** — same reason as §1.2: `drape_path_to_leave` and `drape_point` were both introduced by exactly one commit, the predicted failure text matched the observed one exactly, and §3.2a's independent build bisect lands on the same `fa27b08`. |

### 2.3 Intended contract, in operator language

> When the toolpath finishes a cut deep in material and the next thing it does is
> a rapid to a new entry point, the tool must come **straight up** to safe height
> first, then traverse across at safe height, then descend. It must never make a
> single diagonal rapid from the bottom of the cut to the next entry — that
> diagonal ploughs through material at rapid feed.

This is a **crash-class machine-safety contract** and it is worth keeping. The
production implementation, the `lift_to_safe_z` closure at `path.rs:1226-1235`,
is unchanged and still correct.

### 2.4 Minimal reproducer and mechanism

Fixture (`path.rs:1689-1700`): `Cut[(0,0,-3) → (5,5,-3)]`, `Rapid(20,20,-2)`,
`Cut[(20,20,-2) → (25,25,-2)]` against the same flat-at-z=0 mesh with
`stock_to_leave = 0.5`.

`fa27b08` added, on the `Cut` arm (`path.rs:1551`),
`drape_path_to_leave(path, …, stock_to_leave, cutter.radius())` — which densifies
the cut and lifts **every** point to `max(z, drop_cutter(x,y) + leave)` — and the
`drape_point` rebind on the `Rapid` arm (§1.4).

Every fixture Z is below the leave plane, so every point is lifted to **+0.5**:

| | cut1 | entry | cut2 |
|---|---|---|---|
| `fa27b08^` | (0,0,**-3**) → (5,5,**-3**) | (20,20,**-2**) | (20,20,**-2**) → (25,25,**-2**) |
| HEAD | (0,0,**+0.5**) → (5,5,**+0.5**) | (20,20,**+0.5**) | (20,20,**+0.5**) → (25,25,**+0.5**) |

The test's first act is to locate its landmark:

```rust
let i = cut1_end.expect("cut1 endpoint not found");
```

No move lands at (5, 5, -3) any more, so the test panics **before** it evaluates
any of its three real assertions (lift-in-place, traverse-at-safe-z,
peck-holds-XY). It is red at the landmark search, not at the contract.

Render: `planning/review_2026-08-04/artifacts/w0/rapid_fixture_xz.svg` —
XZ profile along the x = y diagonal, requested path vs. emitted path, with the
surface and leave planes drawn.
Derivation: `artifacts/w0/path_fixture_moves.txt`.

### 2.5 Is the contract still exercised anywhere?

Partly. `rapid_segment_skips_redundant_lift_when_already_at_safe_z`
(`path.rs:1759`, green) covers only the *negative* case — that no redundant lift
is emitted. **Nothing at HEAD covers the positive case directly**: that a lift IS
emitted, in place, at the pre-traverse XY, before the traverse. The F-5/F-6
crash-class regression has had no focused guard since 2026-06-16.

The nearest surviving coverage is indirect and much weaker: `strategy_comparison_h4.rs`
pre-registers `MAX_RAPID_COLLISIONS_PER_ARM = 0` on the grounds that "a genuine
rapid collision here means the planner drove a rapid through material — a safety
defect". That is a whole-pipeline outcome bar on finishing arms, not a
structural check on `adaptive3d`'s emitter, and it would only catch this
regression if a diagonal rapid happened to clip enough stock to register a
collision at the simulated resolution. It is a backstop, not a substitute.

### 2.6 Proposed verdict: `FIX_TEST`

Same rationale as §1.6, with a stronger note: this is the higher-value of the
two, because the uncovered contract is a rapid-through-material crash class.
Restoring it should be treated as urgent even though the *fix* is a fixture
change.

### 2.7 Proposed red-first sentry

Name: keep `rapid_segment_lifts_to_safe_z_before_traverse` (the name is right);
re-fixture and strengthen.

Shape:

1. Fixture mesh **below** the commanded cut Z (e.g. flat at z = -50) so
   `drape_path_to_leave` is provably inert, and add an explicit guard assertion
   that the cut1 endpoint survived draping — so a future drape-semantics change
   fails loudly at a named assertion instead of at an `.expect`.
2. Keep the three existing structural assertions verbatim.
3. **Add intent assertions** — plan §H0 acceptance gate: "A rapid test verifies
   geometry and intent, not only move count." The lift must carry
   `MoveIntent::Retract` (`path.rs:1232`), the traverse `MoveIntent::Linking`
   (`path.rs:1310`), the descent `MoveIntent::EntryPlunge`. The current test
   checks only `MoveType`. Note plan rule 5: assert the intent the emitter sets
   at source, and record that `arcfit` (R7-H2/W2) can rewrite intents downstream
   — this sentry must run on the pre-dressup emission, which it does.
4. **Second fixture** covering the drape-active case: same segment structure but
   with the mesh **above** the cut Z, asserting that the lift/traverse ordering
   still holds *after* the drape lifts the entry. This is the case that has no
   coverage today and is what would have caught `fa27b08`.
5. Red-first proof: revert the `lift_to_safe_z(&mut tp, params.safe_z)` call on
   the `Rapid`/Plunge arm on a scratch revision; both fixtures must fail.

### 2.8 Blast radius

Identical to §1.8, plus:

| Surface | Assessment |
|---|---|
| `lift_to_safe_z` closure | 6 call sites, all inside `segments_to_toolpath` (Plunge/Helix/Ramp × Rapid/RapidWithFloor). |
| `MoveIntent` assertions | couples the sentry to `toolpath::MoveIntent` naming; W2/R7-H2 (`arcfit` intent inheritance) is the only known thing that rewrites intents, and it runs downstream of this emitter. Flag to W2 so the two do not collide. |

---

## 3. `planner_sim_dexel_parity_agent_search`

### 3.1 Command, duration, result, failure text

```
cargo test -p rs_cam_core --lib \
  adaptive3d::tests::planner_sim_dexel_parity_agent_search -- --exact --nocapture
```

| | |
|---|---|
| Duration | **2.0 s** test body, 2.2 s wall, 84 MB max RSS |
| Runs | 3 |
| Result | **FAILED, 3/3 — bit-identical**, every counter identical across runs |

**This test is not in the ten-minute class.** The plan's `ps -L` liveness advice
was written for `adaptive3d_interior_cell_parity_f029`; this one finishes in two
seconds and needs no special handling. Worth recording, because "slow and
probably flaky" is part of why it was carried rather than investigated.

Measured counters (identical on all three runs):

```
[AgentSearch hemisphere] PARITY: 2692/7921 cells differ > 0.53mm; interior 854;
  planner_higher 464 (sim removed more); sim_higher 2228 (planner removed more);
  max dz 25.000mm

panicked at crates/rs_cam_core/src/adaptive3d/mod.rs:2540:9:
Planner and simulator dexels diverged on 854 INTERIOR cells (total 7921,
threshold 792, max Δ 25.000mm). ...
```

| Quantity | Value |
|---|---|
| Interior divergent cells | **854** |
| Bar | 792 |
| Margin over the bar | **+7.8 %** — it fails narrowly |
| `sim_higher` : `planner_higher` | **2228 : 464 = 4.8 : 1** |
| `max dz` | 25.000 mm = the **full stock height** (stock 0 → 25) |

Full transcript: `artifacts/w0/parity_agent_search_run1.txt`.

Two facts from the same session that the failure text alone does not give:

- The **green sibling is not clean.** `planner_sim_dexel_parity_contour_parallel`
  passes at interior **429** — 54 % of the same bar — with the *same* directional
  skew (`sim_higher` 987 vs `planner_higher` 231) and the *same* 25 mm `max dz`.
  It is the same defect, under the bar.
- `max dz` = full stock height on **both** strategies, so somewhere a whole
  column is cleared in one model and untouched in the other. That is not
  sub-cell blend noise.

### 3.2 Provenance

| Item | Value |
|---|---|
| Test source | `crates/rs_cam_core/src/adaptive3d/mod.rs:2516-2548` |
| Harness | `run_planner_sim_parity` -> `run_planner_sim_parity_with_mesh` (`mod.rs:2072-2215`) |
| Sibling | `planner_sim_dexel_parity_contour_parallel` (`mod.rs:2550-2568`) — same harness, `ClearingStrategy3d::ContourParallel`; currently **green**, see §3.1 |
| Introduced | `7e13faf` — *adaptive3d: planner<->sim dexel stamp parity*, 2026-05-07. Commit body records it **passing**: "interior 28/7921 cells diverge", bar then 1 % (= 79). |
| Threshold relaxed 1 % -> 10 % | `be0dcbf` — *feat(sim): F.a sub-cell stamping*, 2026-05-19 |
| First written acknowledgement of the red | `4b105da`, 2026-07-06 |

One thing the history makes plain before any bisect: the bar was already nearly
spent when it was set. At `be0dcbf` the measured interior count is **625**
against a bar of 792 — 79 % consumed on the day the 10 % headroom was granted,
against the 28 cells the test was born with. That is not the cause of the red,
but it is why a single later commit could tip it.

### 3.2a First bad commit — **BISECTED**

| Item | Value |
|---|---|
| Method | `git bisect run` over `be0dcbf..9f52378` (343 revisions, 9 probes) in an **isolated `git worktree`** with a scratch `CARGO_TARGET_DIR`. The main working tree was never checked out, so `planning/airrun_2026-06-01/wanaka.toml` was never touched, staged, or reverted. Worktree removed and pruned afterwards. |
| Probe | the test itself — ~1 s rebuild-and-run per probe after the first |
| Good anchor | `be0dcbf` — **verified good by running it**, interior 625; not merely trusted from its commit body |
| **First bad commit** | **`fa27b08`** — *fix(adaptive3d): hold stock-to-leave over textured meshes (drape / gouge guard)*, 2026-06-16 |
| Parent `7a95614` | **good**, interior **664** |
| `fa27b08` | **bad**, interior **945** |

A single commit moves the gated number 664 -> 945 across a bar of 792.
Probe log: `artifacts/w0/parity_bisect_evidence.txt`.

**All three reds in this document have the same first bad commit.**

### 3.2b The mechanism signature, measured either side of that commit

The directional counters do not merely grow — **they reverse**, on both
strategies, at exactly `fa27b08`:

| | AgentSearch `planner_higher` / `sim_higher` | ContourParallel `planner_higher` / `sim_higher` |
|---|---|---|
| `7a95614` (parent) | 778 / 562 | 502 / **12** |
| `fa27b08` | 443 / **2404** | 113 / **1144** |

`sim_higher` means the simulator's stock top is *higher* — the simulator removed
**less** than the planner's own bookkeeping claims. Before `fa27b08` that was the
minority case on AgentSearch and essentially absent on ContourParallel (12
cells). After it, it dominates both; ContourParallel's rises **95x**.

That is precisely what an emitter-side transform which only ever *raises* Z, not
mirrored in the planner's stamp, must produce.

Evidence figure (panels A/B/C), read before the verdict below was written:
`artifacts/w0/parity_evidence.svg`

### 3.3 Intended contract, in operator language

> The roughing planner keeps its own running picture of how much stock is left,
> and it decides where to cut next from that picture. If a fresh simulation of
> the toolpath the planner actually emitted disagrees with the planner's own
> picture, the planner is planning against a fiction — it will either cut air it
> thinks is solid, or drive into stock it thinks it already removed.

This is the guard for the wanaka "Back Rough Z=10/Z=7" anomaly: the planner
reported each Z level cleared and emitted 4585 mm of cut path per level while a
replay read zero engagement on those passes.

The contract is worth keeping and is not stale. What is questionable is the
**threshold**, not the property — see §3.6.

### 3.4 Fixture, population, resolution (recorded per plan rule 2)

| Item | Value | Source |
|---|---|---|
| Mesh | `make_test_hemisphere(20.0, 16)` | `mod.rs:627` |
| Cutter | `FlatEndmill::new(6.35, 25.0)` — r = 3.175 | `mod.rs:633` |
| Cell size | `max(3.175 / 6, 0.1)` = **0.5292 mm** | `mod.rs:2096` |
| Stock top / bottom | 25.0 / `mesh.bbox.min.z` | `mod.rs:2095` |
| `depth_per_pass` / `stock_to_leave` / `stepover` / `tolerance` | 3.0 / 0.5 / 1.0 / 0.5 | `mod.rs:2114-2117` |
| `max_stay_down_distance_mm` | `Some(0.0)` — keep-tool-down **disabled** | `default_params`, `mod.rs:676` |
| Sim resolution | the *same* `initial_stock` clone; `sample_step_mm` 0.5 | `mod.rs:2098-2146` |
| Population | interior cells only: mesh bbox inset 1.0 mm on each side | `mod.rs:2159-2162` |
| Tolerance | `tol_mm = cell_size` = 0.5292 mm | `mod.rs:2150` |
| Bar | `interior <= total / 10` where `total` is **all** grid cells, not interior cells | `mod.rs:2539` |

Two instrument notes worth recording before anyone reads a number off this test:

- The bar mixes populations. `interior` counts only inset cells; `total` counts
  the whole grid including the boundary ring. A "10 %" bar is therefore
  10 % *of a larger set than the one being counted* — it is looser than it reads.
  This is a plan-rule-7 pre-registration defect in the existing test, independent
  of whether the test is red.
- Both branches use `resolution_clamped`-free fixed grids and the same
  `initial_stock` object, so plan rule 8 (no cross-resolution comparison) is
  satisfied.

### 3.5 Mechanism candidates

#### Candidate A (leading) — the planner's stamp never learned about the drape

`7e13faf` built this test around one invariant, stated in its own doc comment
(`clearing.rs:436-447`): `stamp_emitted_segment` must apply **the same**
transformations `segments_to_toolpath` applies, "so the planner stamps the SAME
path the simulator will replay". At HEAD it mirrors exactly two of them —
`simplify_path_3d` and `blend_corners_3d` (`clearing.rs:466-467`).

`fa27b08` added a **third** transformation to the emitter and did not add it to
the mirror:

| Emitter (`path.rs`) | Planner mirror (`clearing.rs`) |
|---|---|
| `Cut`: `drape_path_to_leave(path, …)` then `simplify_path_3d` then `blend_corners_3d` (`path.rs:1551-1557`) | `simplify_path_3d` then `blend_corners_3d` — **no drape** (`clearing.rs:466-467`) |
| `Rapid`: `drape_point(entry, …)` shadow-rebind before the plunge (`path.rs:1261`) | stamps the full descent to the **raw** `entry.z` (`clearing.rs:473-486`) |
| `RapidWithFloor`: same shadow is absent, but `emit_peck_plunge` targets the same raw entry | mirrors `descent_floor` faithfully (`clearing.rs:488-505`) |

The drape only ever **raises** Z. So the emitted (and therefore simulated) path
cuts *higher* than what the planner stamped: the planner believes it removed
material the emitted toolpath leaves behind. Predicted signature in the test's
own `eprintln`: `sim_higher` (labelled "planner removed more") dominates
`planner_higher`.

Why this should hit `AgentSearch` harder than `ContourParallel`, i.e. why one
sibling is over the bar and the other is not:

- **Both** strategies lift 2D points to 3D through the same expression,
  `(surf_z + stock_to_leave).max(z_level)`, where `surf_z` comes from
  `SurfaceHeightmap::z_or_bbox_floor_at_world` (`clearing.rs:1598` for
  AgentSearch, `clearing.rs:832` for ContourParallel).
- That heightmap sampler is **nearest-cell**, not interpolated: `world_cell`
  rounds to the closest cell centre (`slope.rs:321-333`).
- The drape calls `point_drop_cutter` at the **exact** XY (`path.rs:1147`), and
  the heightmap is built from `sample_grid_cell`, which is
  `point_drop_cutter` plus a `min_z` clamp (`dropcutter.rs`,
  `slope.rs:170-177`). Same physical model, different sampling.
  **The entire divergence is the emitter evaluating exactly what the planner
  evaluated at a rounded cell centre.**
- On this fixture the quantisation error is not small. Grid cell is 0.5292 mm
  and the parity tolerance is the same 0.5292 mm. On a 20 mm hemisphere the
  cutter-contact surface steepens without bound toward the rim, so a half-cell
  XY offset near the rim moves the drop-cutter Z by well over one cell — every
  such cell is a divergence by construction.
- `AgentSearch` emits far more entry points than `ContourParallel`: it pushes a
  `Rapid`/`RapidWithFloor` per inset polygon **and per hole** of every inset, per
  region, per Z level (`clearing.rs:1764`, `1817`, `2194`, `2220`, `2264`,
  `2294`, `2323` — seven emission sites), against `ContourParallel`'s four
  (`clearing.rs:765`, `904`, `1090`, `1194`). Each entry is a full vertical
  stamped descent whose target Z the mirror gets wrong, and AgentSearch's entries
  sit on region/inset boundaries — precisely the steep, high-aliasing cells.

This candidate was written **before** the test was run or bisected, from source
alone. §3.2a and §3.2b confirm it: the bisect lands on `fa27b08`, and the
directional signature reverses at exactly that commit on both strategies.

#### Candidate B — a sim-side stamping change, not a planner-side omission

`fcedaf4` (2026-07-09) changed dexel stamping accuracy on the simulator side
(`MAX_SUBSEGMENT_Z_DROP_MM`, `LUT_SAMPLES` 256 → 4096). Either would move the
simulator's answer without moving the planner's. **Ruled out as the first-bad
commit**: `fcedaf4`'s own body already lists this test among "the 3 known reds",
so it post-dates the red. It may still be *widening* the gap and must not be
confused with the cause.

#### Candidate C — F.a sub-cell blend drift simply grew past its headroom

`be0dcbf` bumped the bar 1 % → 10 % to absorb predicted F.a edge-cell drift.
If nothing broke and the drift merely grew, the correct answer is a re-measured
threshold, not a code fix. Discriminator: F.a drift is a **boundary/edge-cell**
phenomenon of bounded magnitude (sub-cell blend residual), so it should show a
small `max_dz` — order one cell — and divergences scattered on feature edges.
Candidate A predicts a `max_dz` **larger** than one cell, concentrated on steep
terrain, with a consistent sign. `max_dz` and the violation list printed by the
test discriminate these directly.

#### Candidate D — fixture/threshold artefact

The bar compares an interior count against one tenth of the **whole-grid** count
(§3.4). If the measured interior divergence sits near the bar, part of the
verdict is an artefact of that mismatch. Recorded so the number is read
correctly, not offered as an excuse.

### 3.6 Proposed verdict

**`FIX_CODE`.**

Candidate A is confirmed on three independent lines of evidence:

1. **Bisect** lands on `fa27b08`, the commit that introduced
   `drape_path_to_leave` and `drape_point` into the emitter and nothing into the
   planner's mirror (§3.2a).
2. **Sign reversal** of the directional counters at exactly that commit, on both
   strategies, in the direction an unmirrored raise-only transform must produce
   (§3.2b).
3. **Curvature dependence.** Cut-only probes at HEAD: `ContourParallel flat`
   **0** interior divergent cells and `AgentSearch flat` **21**, against
   `AgentSearch hemisphere` **479** and `ContourParallel hemisphere` **614**.
   Flat meshes are clean because a constant surface makes the nearest-cell
   heightmap lift and the exact drop-cutter drape agree. A defective stamping
   kernel would diverge on flat stock too. It does not.

Candidates B and C are ruled out **as causes**: `fcedaf4` post-dates the red by
its own commit body, and F.a blend drift is a bounded sub-cell effect that can
produce neither a 25 mm `max dz` nor a sign reversal at a single commit.
Candidate D stands as a **separate, additional defect** in the test's bar (§3.4),
recorded as such and not used to excuse the red.

This is **not** `FIX_TEST`. The test is doing exactly the job it was written for:
it detected a real divergence between what the planner believes it removed and
what its own emitted toolpath removes, introduced by a production change. Raising
the threshold to green it would delete the only guard on the wanaka Back Rough
anomaly.

**The green sibling is not evidence of health.** `planner_sim_dexel_parity_contour_parallel`
passes at interior 429 while carrying the same 4.3 : 1 directional skew and the
same 25 mm `max dz`, and its interior count actually *fell* (459 -> 406) across
the commit that broke it 95x harder by the directional measure. The gated number
is a weak detector of this mechanism; the directional counters are the sensitive
one. Any wave that greens §3 must check ContourParallel by the directional
measure too, or it will leave the same defect in place under a passing test.

**Operator-visible consequence, stated so Checkpoint A can weigh the risk.**
`fa27b08` was a *correct* fix — the drape stops the rough gouging below
`stock_to_leave`, measured on wanaka as over-cut cells 67 -> 0. But because the
planner's bookkeeping was not updated to match, the planner now believes it has
removed material the machine will actually leave standing. The planner uses that
bookkeeping to decide whether a Z level or region still holds material
(`material_remaining_at_level` / `material_remaining_in_region`), so the failure
mode is **skipped passes and standing material**, biased to curved and steep
terrain — not a gouge. **Nothing here argues for reverting `fa27b08`.**

**Cost, stated honestly.** The obvious fix — call the same drape inside
`stamp_emitted_segment` — adds one exact `point_drop_cutter` query per stamped
point per Z level to the planner's inner loop. On a 220k-triangle DEM that is not
free. A cheaper alternative worth measuring first is to make the two sides agree
by *sampling* rather than by draping twice: have the planner's `lift` closures
use the same exact `point_drop_cutter` the drape uses instead of the nearest-cell
heightmap lookup (`slope.rs:321-333`). Both are behavioural and output-changing;
choosing between them is implementation work for the approved wave, not a W0
research call.

### 3.7 Proposed red-first sentry

The existing test stays, unweakened, as the outcome gate. Add **two** cheap
sentries that name the mechanism, so a future regression reports *what* broke
rather than only *that* a cell count moved:

1. `planner_stamp_mirrors_emitter_drape` — on a small curved fixture, assert that
   the planner's `final_material_stock` and a fresh replay of
   `segments_to_toolpath`'s output agree cell-for-cell within one cell, **and**
   assert the directional balance: neither `sim_higher` nor `planner_higher` may
   exceed the other by more than a pre-registered ratio. The directional
   counters are the sensitive instrument (§3.2b) — they move 95x where the
   currently-gated interior count moves -12 %. Red-first proof is already
   measured: this assertion fails at `fa27b08` and passes at `7a95614`.
2. `parity_is_curvature_independent` — run the existing cut-only harness on flat
   **and** curved fixtures and require the curved result to stay within a
   pre-registered multiple of the flat one. This is the discriminator that
   separates a sampling mismatch from a kernel bug, and today it exists only as
   an `#[ignore]`d probe.

Also required in the same wave, from §3.4: restate the bar as a fraction of the
**interior** population it actually counts, and re-measure the threshold against
the fixed code rather than copying the existing 10 % pin (plan rule 7 — a changed
instrument requires re-measured thresholds, not copied pins).

**Non-vacuity condition, pre-registered:** the curved fixture must produce a
non-zero drape lift on at least one emitted Cut point, asserted directly.
Otherwise the sentry can pass by testing geometry the drape never touches — which
is exactly how tests 1 and 2 became vacuous.

### 3.8 Blast radius

| Surface | Assessment |
|---|---|
| `stamp_emitted_segment` (`clearing.rs:449`) | the single planner-side mirror of the emitter. One definition, called only via `push_segment_with_stamp` (`clearing.rs:531`). |
| `push_segment_with_stamp` call sites | **20** in `clearing.rs` (definition at `:531`) across the ContourParallel, ContourSpiral, Adaptive and AgentSearch dispatches — every one changes behaviour if the stamp changes. |
| `ClearZLevelContext` | already carries `mesh`, `index`, `cutter`, `stock_to_leave` (`clearing.rs:265-273`), so mirroring the drape needs **no new plumbing into the context** — only extra parameters on the two helpers. |
| Planner cost | `drape_path_to_leave` densifies to ≤ tool radius and runs `point_drop_cutter` per point. Mirroring it inside the planner adds one drop-cutter query per stamped point per Z level — a real generation-time cost on 3D roughing that must be measured, not assumed. This is the main reason a `FIX_CODE` here needs Checkpoint A rather than being folded into W0. |
| Downstream of planner stock | `material_remaining_at_level` / `material_remaining_in_region` gate whether a Z level is cleared at all (`clearing.rs:597`, `965`, `1394`); `final_material_stock` feeds `FromRemainingStock` chains and rest analysis. Changing planner stamping changes **which levels get cut**, so it is an output-changing behavioural fix, not a metric repair. |
| Sentries at risk | `adaptive3d_interior_cell_parity_f029` (~10 min), the F-024/F-027/F-029-F-031 suite, `planner_sim_dexel_parity_contour_parallel`, `wanaka_final_surface_vs_mesh` (`#[ignore]`, the ground-truth guard `fa27b08` added). Any `FIX_CODE` must re-run all of them. |

---

## 4. Cross-cutting findings

1. **One commit accounts for all three reds.** `fa27b08` is the bisected first
   bad commit for the parity test and the closed-form cause of both `path.rs`
   failures. Its verification note named a hand-picked sentry list
   ("F-024/F-027/F-029-031/F-038/F-038b/lift-bridge/monotonicity/subtool")
   instead of the crate's own `--lib` target, and two of the tests it broke live
   in the file it edited. The practice fix is cheap: `fa27b08`-class commits must
   report `cargo test -p rs_cam_core --lib` explicitly. W0 is evidence of what
   its absence costs — seven weeks, three guards, one line of verification text.
2. **A red test stops being a guard.** Both `path.rs` tests are not merely
   failing — §1.5 shows one of them has been *vacuous* since it went red, and
   §2.5 shows the other's crash-class contract has had no positive coverage at
   all. "Known red" was carried for seven weeks as if it were a cost-free
   annotation. It was not: it deleted a livelock guard and a
   rapid-through-material guard from the gate.
3. **The `legacy_test_mesh` doc comment is a live lie.** It asserts the
   heightfield is never queried; `fa27b08` made the drape query it
   unconditionally. Plan rule P12 (update stale rationales when mechanisms
   change) applies.
4. **`planner_sim_dexel_parity_agent_search`'s bar is mis-pre-registered**
   (§3.4) regardless of the red: it compares an *interior* count against a tenth
   of the *whole-grid* count, so it is looser than it reads. It was also already
   79 % spent when it was set (625 of 792 at `be0dcbf`, against the 28 cells the
   test was born with). Whatever Checkpoint A rules, restate the bar against the
   population it actually measures and re-measure it against fixed code.
5. **A passing sibling was mistaken for a clean one.** `planner_sim_dexel_parity_contour_parallel`
   is green while carrying the same defect, the same direction, and the same
   25 mm `max dz`; its gated count even *fell* across the commit that broke it
   95x harder by the directional measure. Generalisation worth carrying beyond
   W0: when a defect has a directional signature, gate on the direction, not on
   an unsigned count of affected cells.
6. **Two of these three tests would have been caught by a non-vacuity
   assertion**, not by a better threshold. Both went silently vacuous the moment
   their fixture stopped reaching the code under test. Plan rule P13 ("prove
   fixtures can exhibit the defect") is the cheap systemic guard here.

## 5. What Checkpoint A is being asked to rule

| Ask | Detail |
|---|---|
| A1 | Approve `FIX_TEST` for §1 with the non-vacuity bar `entry_plunges >= 3` pre-registered, and approve re-fixturing rather than assertion-weakening. |
| A2 | Approve `FIX_TEST` for §2 **plus** the added second (drape-active) fixture and the `MoveIntent` assertions. Confirm the crash-class framing. |
| A3 | Rule on §3.6 `FIX_CODE` for the parity test, and on **which** of the two convergence options is taken (mirror the drape inside the planner's stamp, or move the planner's lift onto the exact drop-cutter the drape already uses). Both are output-changing on 3D roughing. Note that `fa27b08` itself is correct and is not proposed for reverting. |
| A3b | Confirm the §3 wave must also check `planner_sim_dexel_parity_contour_parallel` by the directional measure, not only by its currently-green count (§4.5). |
| A4 | Confirm that after A1–A3 the `--lib` accepted-red allowlist goes to zero and `wanaka_suggest_baseline` remains the one declared environmental exception (plan rule 9), owned outside this programme. |

## 6. NOT FIXED, STATED

| Item | Owner | Re-open condition |
|---|---|---|
| All three reds | W0 proposes; **operator rules at Checkpoint A**; a later implementation wave executes | Checkpoint A ruling recorded in `ORCHESTRATION_LOG.md` |
| `planner_sim_dexel_parity_contour_parallel` carries the same defect sub-threshold (§3.1, §4.5) | the §3 fix wave | Checkpoint A — it is green today and must not be treated as evidence the fix worked |
| `planner_sim_dexel_parity_agent_search` threshold population defect (§3.4) | folded into the §3 fix wave | Checkpoint A |
| `legacy_test_mesh` stale doc comment | same commit as the §1/§2 fix | Checkpoint A |
| `wanaka_suggest_baseline` | out of W0 scope by plan rule 9 | not this programme |
