# Pencil linking efficiency — diagnosis + plan of attack (2026-09-04)

> Status: PLAN ONLY, not started. Operator paused to hand the machine to
> another agent for heavy sims; resume in the morning. Do not execute
> before re-reading this and confirming with the operator.

## The complaint

The pencil pass "often isn't worth the time it takes." Raster + scallop
finishes are fine; pencil is the expensive one.

## What the measurement showed (wanaka200_iso_scallop.toml, R1.0 tapered ball)

Added a throwaway pencil op (index 9) after the R1.5 scallop, measured at
0.3 mm sim.

- **Dihedral detector (default):** ~4.3 h; **94% of centreline points float
  over creases the R1.0 tool cannot reach** (`geom.tip_float`, 78,416 /
  83,379), leaves up to 4.24 mm uncut. Tool-blind detector.
- **Rest detector (tool-aware):** cutting −39%, float count −65% — BUT
  **8.9 h total, of which 91% (28,986 s) is ENTRY motion and only ~23 min is
  actual cutting.** `runtime_by_intent`: entry_s 28,986 / cutting_s 1,418 /
  linking_s 723 / rapid_s 776. It shatters into thousands of tiny scattered
  targets, each getting its own entry.

Either way: **the cutting is a sliver; the approach overhead is everything.**

## The code gap (why)

Pencil is the **last surface op still on its own hand-rolled linker**
(`pencil::emit_paths_with_entry_stock`): greedy nearest-neighbour order
(`order_paths_nearest`) + *pairwise* linking, capped at `hookup_distance =
5 mm`, gouge-safe surface-follow only (`build_surface_link`) — else a full
entry. `project_curve` (`compute/execute.rs:2015`), `scallop`, and
`unified_finish` all route through the **generic
`surface_link::relink_fragments`** with a dexel `LinkCeiling` (clearance
hops over the stock + global reorder).

## The correction that shapes the plan (advisor)

The 91% is **NOT the retract — it is the entry RAMP.** `plan_entry_ramp`
(`pencil.rs:1108–1231`) emits a *multi-lap zig-zag* whose lap count scales
with descent depth (safe-Z → valley floor). That is the 30,752 s of slow
(259 mm/min) helical laps.

Consequence: **routing wholesale through `relink_fragments` does NOT fix it**
— its hops arrive from ABOVE, so each fragment still needs a ramp on
landing. **The only link that removes a ramp is one that arrives AT THE
FLOOR** — a surface-following feed at depth, which pencil already has
(`build_surface_link`) but caps at 5 mm. So the lever is extending the
at-depth link / adding a middle tier, NOT swapping linkers.

## Plan of attack (measure the binding constraint FIRST)

1. **Instrument (≈20 lines, commit).** Pencil's emitter does not publish why
   each transition fails to link. Add four counters to
   `emit_paths_with_entry_stock`: `linked` / `too_far` (gap > cap) /
   `off_surface` (surface-follow would gouge) / `ceiling_refused`.

2. **One regen at hookup 5 mm → 30 mm, read the counters.** Names the
   constraint in a single run:
   - **`too_far` dominates** → reach/ordering. Extend the at-depth link cap;
     check whether `relink_fragments` reorder can REVERSE fragments (its
     project_curve mode is forward-only-no-reversal, which is wrong for
     bidirectional pencil cuts — `order_paths_nearest` may already be
     better). If no reversal support, keep pencil's ordering.
   - **`off_surface` dominates** → gouge (floor-follow crosses a ridge). No
     hookup distance fixes this. Add a **clearance-hop tier BETWEEN
     surface-follow and full ramp** (reuse `plan_link_lift`'s ceiling read /
     the generic `LinkCeiling`) — hop over the ridge; ramp ONLY for
     genuinely isolated fragments.

3. **Apply the fix step 2 points to**, in pencil's own emitter (≈50 lines),
   preserving G-ENTRYLOAD / G-LINKLOAD stock-safety.

4. **Measure.** Primary bar: **entry COUNT and `entry_s`** (not retract
   distance). Target: `entry_s` ≤ 25% of current AND `tip_float_points`
   unchanged (linking must not change what is cut) AND rapid collisions 0.

## Scoping decision the operator must make (honest)

**Linking makes the pencil CHEAP, not USEFUL.** Even perfectly linked, the
R1.0 tool still cuts ~23 min and leaves ~4 mm uncut in the deep creases
(94% tip-float). Best case: ~8.9 h → ~40 min — worth running instead of
never. To actually REACH the detail is a separate call: a smaller tool
(R0.5 is in the library) or the rest detector with its scattered sub-tool
spots FILTERED OUT rather than linked.

## RESULTS — Step 2 measured (2026-09-06)

Fixture: `wanaka200_iso_scallop.toml` id-19 "Pencil measure (R1.0)", rest
detector, **fresh stock**, R1.0 tapered ball, feed 1200, plunge 180, 0.5 mm
sim. id-19 isolated (rough/scallop disabled). This reproduces the summary
baseline exactly (total 31,903 s ≈ 8.9 h; tip_float 27,331 = −65% vs
dihedral's 78,416; air 52%).

`runtime_by_intent` A/B (raise `hookup_distance` only):

| Intent | hookup 5 mm | hookup 30 mm | Δ |
|---|---|---|---|
| entry | 28,986 s (90.9%) | 19,654 s (87.7%) | −9,332 s (−32%) |
| cutting | 1,418 s | 1,411 s | ~0 |
| linking | 723 s | 869 s | +146 s |
| rapid | 776 s | 479 s | −297 s |
| total | 31,903 s | 22,414 s | −30% |
| rapid_collisions | 39 | 26 | −13 |
| tip_float | 27,331 | 27,331 | 0 |
| helix-ramp cutting_runtime | 24,408 s | 17,074 s | −7,334 s |

**Reading:**
1. **too_far is a real binding constraint** — 32% of the entry budget is
   reach-recoverable, tip_float unchanged (linking changes approach cost, not
   what is cut). Extending the at-depth link cap is a genuine lever.
2. **But entry is still 87.7% at 30 mm** — the majority of entries are not
   reach-bound. Splitting the remainder (too_far > 30 vs off_surface gouge)
   needs the counters.
3. **CAVEAT — the fresh-stock baseline likely INFLATES entry cost.** Entries
   are multi-lap HELIX ramps (helix-ramp = 24,408 s of the 28,986 entry_s);
   `plan_entry_ramp` scales lap count with descent depth. On fresh stock every
   entry descends from safe_z through the FULL block to reach the crease →
   deep descent → many laps. On the realistic AFTER-SCALLOP stock the R1.5 has
   already brought the surface near final, so the pencil entry descends only
   the last ~1–2 mm → few laps → far cheaper entries. The "91% entry"
   pathology may be partly a fixture artifact of measuring on fresh stock.
   **Re-measure on after-scallop stock before writing the fix.**

**CAVEAT CONFIRMED by the code (2026-09-06, no cargo).** `plan_entry_ramp`
(pencil.rs:1178–1204): `ceiling = max_conservative_top_z_in_disc(...)` is the
ACTUAL input-stock top over the ramp window (capped at safe_z); `floor` is the
crease bottom; `depth = ceiling − floor`; `laps = ceil(depth / per_lap)`,
clamped to `ENTRY_RAMP_MAX_LAPS`. Lap count is driven by the material column
read from `entry_stock`, NOT geometric safe_z→floor. On fresh stock `depth` is
the full raw-block height → many EntryRamp laps of real removal. On
after-scallop stock the R1.5 leaves `depth` small; when `depth <= budget` the
ramp returns `None` and a cheap plain descent is used (pencil.rs:1193–1197).
`optimize_entry_descents` only rapids the AIR portion (safe_z→ceiling), not the
laps. So the fresh-stock entry cost is inflated; realistic after-scallop
entries are far cheaper. The reach finding (too_far, −32% at 5→30) is durable
regardless; its MAGNITUDE on realistic stock is unmeasured and gates the fix.

**BLOCKER for the realistic measurement:** it needs a fixpoint `generate_all`
(rough→sim→scallop→sim→pencil). 4.8 GiB free with the other agent's cargo
cycling is the exact OOM setup of `5ea6f263` (checkpoint memory scales
ops×columns, uncapped). Do not launch until the lane is quiet or the operator
pauses the other agent.

## RESULTS — realistic (after-scallop) stock (2026-09-06)

Ran the fixpoint chain (rough 5 → scallop 8 → pencil 19, FromRemainingStock,
0.5 mm). Peak GUI RSS ~1.3 GB, no OOM. Pencil id-19 at hookup 5 mm:

| metric | fresh (artifact) | realistic | note |
|---|---|---|---|
| total_s | 31,903 (8.9 h) | 4,985 (83 min) | 6.4× less |
| entry_s | 28,986 (90.9%) | 4,515 (90.6%) | share UNCHANGED |
| cutting_s | 1,418 | 43 | 83 min to cut 43 s |
| linking_s | 723 | 120 | |
| rapid_s | 776 | 306 | |
| move_count | 90,184 | 25,827 | −71% |
| tip_float | 27,331 | 528 | −98% |
| rapid_collisions | 39 | 0 | clean on real stock |

**Both fresh-stock artifacts corrected:** the 8.9 h absolute cost AND the "94%
tip-float / leaves 4 mm uncut" pessimism were fresh-stock only. On realistic
stock the R1.0 reaches nearly everything (528 float pts) — **the pencil is
useful here.** But entry is STILL 90.6% — the pass spends 83 min to cut 43 s.
The pencil fragments into thousands of tiny targets, each getting its own
entry. **Linking is validated on the right fixture; scoping caveat softened —
linking makes it cheap AND it is already useful.**

## REACH LEVER FALSIFIED on realistic stock (2026-09-06)

The 5→30 mm A/B on realistic stock runs OPPOSITE to fresh:

| id-19 | realistic 5 mm | realistic 30 mm |
|---|---|---|
| total_s | 4,985 (83 min) | 20,997 (5.8 h) — 4.2× WORSE |
| entry_s | 4,515 (90.6%) | 18,680 (89.0%) |
| cutting_s | 43 | 117 |
| linking_s | 120 | 280 |
| rapid_s | 306 | 1,920 |
| move_count | 25,827 | 52,693 |
| tip_float | 528 | 4,917 |
| rapid_collisions | 0 | 0 |

**On fresh stock widening the cap helped (−30%); on realistic stock it hurts
(+320%).** So `hookup_distance` is NOT a clean reach lever. Mechanism (to
confirm): on realistic stock the terrain between fragments is the finished
(scalloped) surface. A longer surface-follow candidate crosses more standing
material → G-LINKLOAD lifts it → each lift becomes an `emit_entry_descent`
RAMP, and the floating transit adds tip_float (528→4,917). The binding
constraint on realistic stock is NOT reach (too_far) — it is that the linker
cannot cheaply bridge fragments across finished terrain; every crossing
becomes a lift+ramp.

**Consequence for the fix (REVERSES the earlier plan):** on realistic stock
the entries are SHALLOW (R1.5 left the top near-final), so the earlier
objection to `relink_fragments` ("hops arrive from above → still a deep ramp")
may not hold here — a rapid CLEARANCE HOP over shallow finished terrain plus a
shallow descent could be far cheaper than the current lift→ramp path. The
generic linker (scallop/project_curve template) may now be the right tool,
precisely because descents are shallow. NEEDS the counters + a relink_fragments
A/B to decide. Do NOT ship a hookup_distance change.

## PIVOT — attack the ENTRY, not the linking (2026-09-06, operator-directed)

Operator watched the viewport: "huge entry zig-zags ... if we found more
efficient entry points and angles it wouldn't have to do that." The data
agrees — entry is 90.6% on realistic stock, linking is a dead/harmful lever.
Operator picked BOTH entry levers, measure-per-fragment.

**Code scope (read 2026-09-06):**
- Entry shape/angle: `plan_entry_ramp` (pencil.rs:1132). CORRECTED mechanism
  (advisor 2026-09-06): the zig-zag is NOT caused by a small window.
  `per_lap = (window_len·tanθ).min(budget×0.5)` (line 1199); the `budget/2`
  clamp (0.25 mm for the R1.0 tip) bounds the per-lap bite regardless of window
  length, so `laps = ceil(depth / 0.25)` is fixed by DEPTH and BUDGET. The
  window is 1.78 mm because that is where `window·tanθ = budget/2`; a longer
  window gives LONGER laps, same count — strictly worse. Removing the clamp is
  removing G-ENTRYLOAD (the 3.72 mm white-oak gouge fix). Constants 1042–1067.
- Entry point: `order_paths_nearest` (pencil.rs:567) already reverses a
  fragment to enter the nearer END (611–619) but ignores stock depth.
- `hookup_distance` is read ONLY at 1576 (the linker) — it does NOT change
  centerline geometry (grep-confirmed). So entry work is independent of it.

**Lever A — ramp ANGLE (per-entry cost, not lap count).** Steeper θ →
`window = budget/(2·tanθ)` shorter → each lap shorter → same lap count, less
travel per lap. 8→15° roughly halves per-lap travel. Respects G-ENTRYLOAD by
construction (moves θ, not the bite budget). Smallest possible change. Helps
only if entries are RAMPS (depth > budget), not plain plunges.

**Lever B — shallow entry END (lap count).** Enter the end with least
`ceiling − floor` → smaller depth → fewer laps. Cheap version (advisor): in the
EMITTER, after the run is chosen, probe `max_conservative_top_z_in_disc` at both
ends and reverse if the far end is shallower — one reorder, not a rewrite of
`order_paths_nearest`.

**TWO NUMBERS NEEDED BEFORE CHOOSING (advisor):**
1. Entry count N on fixpoint-5 → mean cost 4,515/N. Small N (~1k, ~4.5 s each)
   = ramps dominate, per-entry prize is real. Large N (~4.5k, ~1 s each) =
   plunges, the fix is COUNT reduction (Lever B / links), not ramp shape.
2. Depth distribution per entry. Most `depth ≤ 0.5 mm` = plain plunges, ramp
   shape is not the cost. Most 1–3 mm = 4–12 laps, angle lever pays.
   Source: `plan_entry_ramp` already logs `step_mm`, `ramp_points` at
   `tracing::debug!` (1285); `depth = step_mm × laps`, `laps ≈ ramp_points /
   window_points`. Needs GUI relaunch with `RUST_LOG=rs_cam_core::pencil=debug`
   OR back-calc from entry spans.

**MEASUREMENT PROTOCOL (mandatory — a confound was hit):** a standalone
`generate_toolpath(9)` does NOT reproduce the fixpoint's after-scallop stock
(fixpoint-5 = 25,827 moves; standalone at the same hookup = 53,420). Every
realistic measurement MUST be a full `generate_all` fixpoint at 0.5 mm.
Baseline (fixpoint-5): total 4,985 s, entry_s 4,515 (90.6%), cutting_s 43,
move_count 25,827, tip_float 528, rapid_collisions 0.

**Bars for the entry fix:** entry_s down materially; tip_float ≤ 528 (must not
change what is cut); rapid_collisions 0; cutting_s and coverage unchanged.

## REPRODUCIBILITY WRINKLE + the robust finding (2026-09-06)

The first realistic fixpoint gave pencil 25,827 moves / entry 4,515 s / total
4,985 s / tip_float 528. Every run AFTER that — two standalone regens (30 then
5) and a SECOND fixpoint at hookup 5 — gives 53,420 moves / entry 18,680 s /
total 20,997 s / tip_float 4,917, stably. So 53,420 is the reproducible result;
25,827 was a one-time first-generation state. Cause (likely): the pencil's rest
field reads the exact prior-stock snapshot, and repeated standalone regens
perturbed the session's accumulated stock. **A clean baseline needs a FRESH
project reload, then ONE fixpoint, no standalone regens.** Do that before any
fix before/after.

**Robust across ALL runs (does not depend on which baseline):**
- Entry is 89–91% of the pencil pass.
- Entries are multi-lap RAMPS, not cheap plunges: N ≈ 2,532 entry spans;
  `entry_load` shows entry removal up to 0.57 mm, `crosses_standing` up to
  1.29 mm, against a 0.25 mm/lap budget → ~2–6 laps each. Mean cost per entry
  is seconds, not sub-second.

**Decision (advisor test satisfied): build the ANGLE lever + SHALLOW-ENTRY-END
lever.** Both attack the ramp cost directly, both respect G-ENTRYLOAD.
Measurement: fresh-reload fixpoint, before vs after, same 0.5 mm.

## IMPLEMENTED — Lever A (2026-09-06)

`ENTRY_RAMP_MAX_ANGLE_DEG` 8.0 → 12.0 (pencil.rs:1058, operator-set cap for the
R1.0 ball in white oak). Shorter laps, SAME lap count, SAME per-lap bite budget
(G-ENTRYLOAD intact). Verified: `pencil_entry_ramp_g_entryload` +
`entry_moves_stock_aware_g_rampterrain` = 10/10 green; `cargo fmt --check` clean;
`cargo clippy -p rs_cam_core` clean. NOT committed (on master; operator call).

**Measurement path (headless, no GUI restart):** CLI `Project` command
(`crates/rs_cam_cli/src/project.rs`) runs the SAME fixpoint ladder as MCP
`generate_all` (project.rs:324) from source, so a `cargo run -p rs_cam_cli`
picks up the 12° build with no GUI/MCP disruption. Needs: a measurement COPY of
the fixture with the pencil op on `from_remaining_stock` (the saved toml has it
`fresh`), and confirm the diagnostics JSON exposes per-op `runtime_by_intent`
(`entry_s`). Before/after = git-stash the constant for the 8° arm.

**Lever B (shallow entry end) — not yet built.** Add after A is measured, only
if the residual warrants (advisor's incremental order).

## SINGLE-TRACE + 12° measured headless (2026-09-06) — root cause is FRAGMENTATION

Operator reframe: pencil should be a SINGLE centerline trace per valley
("carve the center"), coverage beyond that = explicit stepovers. Set
`num_offset_passes` 1→0 (default; the fixture's 1 added left+right = the 3
parallel lines) on a realistic-stock copy, measured headless via CLI `project`
(fresh process — cures the reproducibility flake) on the 12° binary:

| pencil id-19 | 3-line 8° (stable) | single-trace 12° |
|---|---|---|
| move_count | 53,420 | 15,355 (−71%) |
| total_s | 20,997 | 6,231 (−70%) |
| entry_s | 18,680 (89%) | 5,363 (**86.1%**) |
| cutting_s | 117 | **82.8 (1.3%)** |
| collisions | 0 | 0 |

**Single-trace scaled everything down ~3.5× but did NOT change the SHARE —
entry is still 86%, cutting is 82.8 s of a 104-min pass.** So the entry cost is
NOT about parallel passes, and NOT primarily ramp shape. The root is
FRAGMENTATION: the pencil shatters into thousands of tiny scattered rest
fragments, each getting its own entry. entry-ramp travel ≈ 107 m vs 1.66 m of
cutting (65×). Neither single-trace nor the 12° angle reaches this.

**The real fork (operator's call):**
- **Filter the dabs** — raise `min_cut_length` / `min_valley_depth` to drop
  sub-tool scattered fragments, keep only substantial valley traces. Directly
  cuts entry COUNT. Trades a little coverage.
- **Connect into trees** — the operator's "link between valleys" instinct. But
  the linker is broken for this (widening hookup lifts→ramps, worse). Would
  need a real fix to the traversal/linker.
- **Detector mismatch** — `rest` gives scattered dabs; `dihedral` traces the
  actual connected creases (the "tree valleys" mental model) but is tool-blind
  (94% float on fresh; unknown on realistic). Worth checking which matches the
  operator's picture.

**Lever A (12°) is committed-worthy but marginal against fragmentation.** It is
correct and safe; it is not the headline lever. Lever B (shallow entry) same.

## DETECTOR COMPARISON — entry-dominance is UNIVERSAL (2026-09-06)

Single-trace, realistic stock, 12° binary, CLI headless:

| pencil id-19 | rest | dihedral |
|---|---|---|
| move_count | 15,355 | 230,329 |
| total_s | 6,231 (1.7 h) | 88,748 (24.6 h) |
| entry_s | 5,363 (86.1%) | 77,354 (87.2%) |
| cutting_s | 82.8 (1.3%) | 1,323 (1.5%) |
| collisions | 0 | 1 |

Dihedral traces every micro-crease of the noisy organic TIN → 15× the moves,
14× the time; it cuts 16× more but is entry-dominated all the same. **Neither
detector escapes ~86% entry** — the pencil work on this part is inherently
FRAGMENTED (scattered short traces, confirmed by the single-trace screenshot:
hundreds of disconnected stubs, no connected valley-trees). `rest` is the
efficient detector; dihedral is rejected (matches the operator's memory).

**The lever is FILTERING (operator: "filter for real trees").** No detector
change and no linking fix reaches entry-dominance; only reducing the FRAGMENT
COUNT does. Testing `min_cut_length` 2→10 mm on rest single-trace next. The
honest hypothesis: after a good R1.5 scallop the R1.0's leftover is scattered
sub-mm remnants, not connected valleys — so filtering leaves either a few real
traces (the fix) or almost nothing (pencil genuinely not worth it here, which
is the operator's original complaint validated).

## CONCLUSION — no "real trees" to keep on a well-finished part (2026-09-06)

Filter test (rest, single-trace, `min_cut_length` 2→10 mm):

| pencil id-19 | unfiltered (2 mm) | filtered (10 mm) |
|---|---|---|
| move_count | 15,355 | 332 |
| total_s | 6,231 (104 min) | 145 (2.4 min) |
| entry_s | 5,363 (86%) | 121 (83%) |
| cutting_s | 82.8 | 4.0 |

Raising the length filter dropped ~98% of the work — there are almost no
pencil traces ≥10 mm on this part. **The R1.0's leftover after a good R1.5
scallop is scattered sub-10 mm dabs, not connected valleys.** "Filter for real
trees" leaves almost nothing because there ARE no real trees here.

**This is the answer to the original complaint.** The pencil "isn't worth the
time" on a well-finished part because there is little substantial left to cut —
it spends 104 min on entry overhead to remove 82 s of scattered remnants. That
is correct behaviour finding little work, not an algorithm bug. Entry-dominance
is universal (both detectors, all pass counts) because the work is inherently
fragmented.

**Shippable outcomes:**
1. `ENTRY_RAMP_MAX_ANGLE_DEG` 8→12 (DONE, sentries+lint green) — correct, safe,
   marginal against fragmentation.
2. Sensible pencil defaults for cleanup use: `num_offset_passes = 0` (single
   trace) + a raised `min_cut_length` (per-job dial, not a blanket default —
   on parts WITH real valleys a high floor would drop legitimate work).
   Together: 104 min → 2.4 min on this part.
3. Linking / detector / ramp-angle do NOT reach the root; filtering does.

**If the operator wants to CARVE VALLEY CENTERS as a primary feature** (the
"rivers" intent), that is a dedicated valley/river op (project_curve rivers are
already in this project), or a coarser finish that deliberately leaves valley
stock for the pencil — NOT rest-cleanup on an already-finished surface.

## Files

- `crates/rs_cam_core/src/pencil.rs` — `emit_paths_with_entry_stock`
  (~1531), `plan_entry_ramp` (~1108), `plan_link_lift`, `order_paths_nearest`
  (~567), `build_surface_link` use (~1579).
- `crates/rs_cam_core/src/surface_link.rs` — `relink_fragments` (~354),
  `RelinkParams` (~191), `LinkCeiling` (~81), `RelinkReport` (~290).
- `crates/rs_cam_core/src/compute/execute.rs:2015` — the working template
  (project_curve routing through the generic linker with a dexel ceiling).
