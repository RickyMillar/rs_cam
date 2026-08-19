# Wave 2 (GEN) — G2: hoist the 2.5D geometry out of the Z loop

Captured 2026-08-20, same machine as Phase 0 and Wave 1, criterion release
builds, every cargo invocation serialized behind `flock /tmp/rs_cam_cargo.lock`.

> **Do not merge this into `BASELINES.md` verbatim** — three lanes ran
> concurrently on 2026-08-19/20 and the consolidation is the orchestrator's.

## What landed

The review's prescription, unchanged: every depth-stepped 2.5D operation was
recomputing its full 2D geometry once per Z level, and `cut_depth` reaches the
output in exactly one place — the Z stamp in the emitter. The fix splits each
family into a **Z-independent half** and a **Z-dependent half** and calls the
first one once, above `toolpath_at_levels_with_cancel`.

| Family | Z-independent half (once) | Z-dependent half (per level) |
|---|---|---|
| pocket / Contour | `pocket_contours_reported_with_cancel` — compensation offset + the whole `OffsetRingSet` cascade | `pocket_contours_to_toolpath` (was private `contours_to_toolpath`) |
| pocket / Zigzag, zigzag | `zigzag_lines_reported` — wall inset + scan-line build | `lines_to_toolpath` (already `pub`) |
| profile | `profile_path_reported` (**new**) — compensation offset, both compensation modes | `profile_path_to_toolpath` (was private `contour_to_toolpath`) |
| trace | `trace_compensated_polygons_reported` (**new**) | `trace_polygons_at_z` (**new**) |
| rest | `rest_segments` (**new**) — inward offset, scan lines, **and the per-sample containment walk** | `rest_segments_to_toolpath` (**new**) |

Call sites moved: `compute/execute.rs` `generate_pocket` / `generate_profile` /
`generate_zigzag` / `generate_trace` / `generate_rest`, plus
`trace::trace_toolpath_with_cancel` (which did its own depth stepping).
`pocket::pocket_toolpath_at_levels_reported_with_cancel` is a new entry point
that packages the pocket hoist so `execute.rs` and the bench share one
implementation rather than two copies of the same idea.

Every public `*Params` in these five modules gained `Copy` so the per-level
restamp is `Params { cut_depth: z, ..*base }` rather than a re-listed literal.
That is not cosmetic: a re-listed literal is how a field silently goes missing
when someone adds one.

## The two acceptance numbers

### Correctness — the Phase 0 golden, UNCHANGED

```text
cargo test -p rs_cam_core --test perf_golden_depth_level_geometry
   per_level_xy_is_invariant ................ ok
   depth_stepped_fingerprints_match_golden .. ok
```

Not re-baselined; not touched. All six fingerprints
(`pocket_L1/L8`, `profile_L1/L8`, `zigzag_L1/L8`) match byte for byte.

**But that golden cannot see the hoist, and this is worth saying plainly.**
It pins the *pre-fix call shape* — `pocket_toolpath` inside the per-level
closure — and the hoist is a *different call shape*. The golden passing proves
the hoist did not damage the emitters; it does not prove the hoisted
composition emits the same thing. So five new property sentries were added
alongside it, each comparing hoisted against naive **bit for bit** (raw `u64`
patterns on x/y/z plus `move_type` and `intent`, so a last-ULP divergence
cannot hide and `-0.0 == 0.0` cannot mask one):

| Sentry | Covers |
|---|---|
| `pocket::tests::the_hoisted_cascade_emits_exactly_what_the_per_level_one_did` | 600-vertex jittered ring, L = 1 and L = 5 |
| `profile::tests::the_hoisted_contour_emits_exactly_what_the_per_level_offset_did` | 4 arms: {Outside, Inside} × {software, in-control compensation} |
| `zigzag::tests::the_hoisted_scan_lines_emit_exactly_what_the_per_level_build_did` | 23° raster, L = 5 |
| `trace::tests::the_hoisted_compensation_emits_exactly_what_the_per_level_offset_did` | all three `TraceCompensation` modes, L = 4 |
| `rest::tests::the_full_zigzag_fallback_still_matches_the_zigzag_op_exactly` | the one branch that genuinely changed code path (below) |
| `rest::tests::rest_segments_are_independent_of_cut_depth` | G2's premise for rest, stated as a property |

All are properties, not pinned constants, so they cannot rot and never need
re-baselining.

### Speed — the ratio is the deliverable

`cargo bench -p rs_cam_core --bench hot_paths -- gen_depth`, one run,
2026-08-20. The bench now carries **both** shapes: the unchanged `L1`/`L20`
arms measure the pre-fix composition (they are the same code as Phase 0 — the
hoist does not make that shape faster, it makes it avoidable), and the new
`*_hoisted` arms measure what `execute.rs` now does.

| Bench | Phase 0 | This run | vs Phase 0 |
|---|---:|---:|---:|
| `gen_depth/pocket/L1` | 33.310 ms | 32.590 ms | −2.2% |
| `gen_depth/pocket/L20` | 738.19 ms | 648.91 ms | −12.1% |
| `gen_depth/pocket/L1_hoisted` | — | **32.399 ms** | — |
| `gen_depth/pocket/L20_hoisted` | — | **33.780 ms** | **−95.4% / 21.9× faster** |
| `gen_depth/profile/L1` | 2.0163 ms | 1.8310 ms | −9.2% |
| `gen_depth/profile/L20` | 45.429 ms | 36.730 ms | −19.1% |
| `gen_depth/profile/L1_hoisted` | — | **1.8523 ms** | — |
| `gen_depth/profile/L20_hoisted` | — | **2.0904 ms** | **−95.4% / 21.7× faster** |
| `gen_depth/zigzag/L1` | 1.9364 ms | 1.7982 ms | −7.1% |
| `gen_depth/zigzag/L20` | 39.448 ms | 35.657 ms | −9.6% |
| `gen_depth/zigzag/L1_hoisted` | — | **1.7861 ms** | — |
| `gen_depth/zigzag/L20_hoisted` | — | **1.8212 ms** | **−95.4% / 21.7× faster** |

**The L20/L1 ratio, which is the number the review asked for:**

| Operation | Phase 0 | Naive shape, this run | **Hoisted shape** |
|---|---:|---:|---:|
| pocket | 22.2× | 19.9× | **1.04×** |
| profile | 22.5× | 20.1× | **1.14×** |
| zigzag | 20.4× | 19.8× | **1.01×** |

The predicted collapse happened: 20–22× → **1.0–1.1×**. What is left above 1.0
is exactly what the review said would be left — per-level emission and the
inter-level retract. Profile's 1.14× is the largest residue because its
geometry is a single contour (cheap to compute, so emission is a bigger share
of the total); pocket's 1.04× is the smallest because its cascade dominates
everything else. Both are the shape the prediction implies, not noise.

**Read the naive-arm drift honestly.** Those arms are byte-identical code to
Phase 0 and this change cannot have made them faster; the 2–19% is Wave 1's
landed G4 `Polygon2` bbox cache (which the offset path benefits from) plus
machine state. That is precisely why the ratio above is quoted **within one
run** rather than against the Phase 0 absolute — `L20_hoisted / L1` and
`L20 / L1` were measured minutes apart under identical conditions.

**Machine state.** Not an idle box, same as Wave 1: an editor-driven
`cargo check --workspace` runs continuously in this repo and two other agents
were editing the tree; available memory read 12–18 Gi against the 20 Gi house
rule. Every ratio quoted is a within-run comparison, so contention cancels.

## The `offset_library_failures` × L over-count

The brief asked whether the reporting channel needs adjusting. **It does not,
and that is the finding.**

`ToolpathStats::offset_library_failures` is documented (CLAUDE.md, and
`polygon.rs:48`) as counting offset **calls**, with the caveat that "a
depth-stepped op re-offsets the same geometry once per Z level, so one bad ring
on a ten-level pocket reports ten". The channel's contract was always
"calls that failed"; the ×L was not the channel over-counting, it was the
**compute** genuinely making L× the calls. Remove the redundant calls and the
count corrects itself with no change to `record_offset_library_failures`, to
`GenerationFindings`, or to any consumer. The number now reads one per failing
offset call, which is what the doc-comment always claimed it counted.

Two consequences worth flagging to whoever consolidates:

1. **The CLAUDE.md caveat is now stale for these five families.** The sentence
   "a depth-stepped op re-offsets the same geometry once per Z level, so one bad
   ring on a ten-level pocket reports ten" no longer describes pocket, profile,
   zigzag, trace or rest. It still describes any family that has not been
   hoisted. I did not edit CLAUDE.md — it is outside this lane's file ownership
   and three lanes are live — but it should be corrected in the consolidation.
2. **`truncated_core_mm2` was multiplied by L too, and nobody had said so.**
   `generate_pocket` accumulated `report.truncated_core_mm2` inside the
   per-level closure, so a cascade that hit a bound reported its standing area
   once per Z level — a 20-level pocket reported 20× the material it actually
   left standing. That is a *quantitative* finding on a surface that reaches
   narrate, the diagnostics list and MCP, not just a count. The hoist fixes it
   for the same reason and by the same mechanism. `boundary_clip_dropped` and
   the other report-only findings are unaffected (they are not accumulated
   per level).

Neither of these is a threshold move and no gate consumes either channel.

## One branch that genuinely changed code path

Everything above is a pure re-association except one thing, and it deserves to
be named rather than buried.

`rest_machining_toolpath`'s "the large tool cannot fit at all" fallback used to
delegate to `crate::zigzag::zigzag_toolpath`, which **re-derived the scan lines
the function had already built** three lines earlier. The hoist could not carry
that delegation across the split (the fallback has to be part of the
Z-independent half), so it now emits the lines it already has through
`emit_rest_segment`. Those are two different code paths reaching the same
`emit_path_segment_with_intent(ClearingCut)` envelope, so "they agree" is a
claim rather than a tautology, and
`rest::tests::the_full_zigzag_fallback_still_matches_the_zigzag_op_exactly`
asserts it bit for bit on a fixture that actually takes the fallback (asserted,
not assumed — the test first checks the large-tool offset really does collapse).

## Cancellation — preserved, and slightly improved

`toolpath_at_levels_with_cancel` remains the single choke point for flat-2D
depth-stepping cancellation. Nothing about the level loop changed: it still
polls as its first statement and again before each level, and pocket's cascade
still polls per offset ring (once now instead of once per level, which is the
point).

The one thing that moved is that hoisted geometry now runs *before* the level
loop's entry poll, so each of the five adapters got an explicit
`ctx.cancel.load(...)` check ahead of its geometry work, and
`trace_toolpath_with_cancel` got a `check_cancel(cancel)?` as its first
statement. Without those, a pre-set flag would still have produced
`Err(Cancelled)` — but only *after* doing the geometry. Behaviour is identical;
promptness is strictly better. `cancellable_families_honour_a_preset_cancel_flag`
still passes for all 24 families.

## Verification

| Gate | Result |
|---|---|
| `cargo clippy -p rs_cam_core --benches --tests -- -D warnings` | clean |
| `perf_golden_depth_level_geometry` | 2/2, golden file untouched |
| `perf_golden_sim_metrics` | see full-suite line below (SIM lane owns it) |
| `pocket_lift_bridge_b1` | 1/1 |
| `vcarve_lift_bridge_b1` | 1/1 |
| `boundary_clip_escape_f1` | 4/4 |
| `boundary_clip_invalidates_spans` | 2/2 |
| `adversarial_2d_campaign_r2` | 6/6 (1 ignored) |
| `offset_candidates_m5` | 6/6 (5 ignored) |
| `offset_growth_m5` | 5/5 (3 ignored) |
| `offset_polygon_degenerate_inputs_r1` | 2/2 |
| `skipped_boundary_offset_f8` | 4/4 |
| `generic_rest_routing_pr7` | 6/6 |
| `rest_routing_probe_e9` | 3/3 |
| `zero_removal_rest_pass_a4` | 2/2 |
| `rest_grid_resolution_c9` | 2/2 |
| `project_curve_depth_sign` | 4/4 |
| `arcfit_intent_boundary_f1` | 5/5 |
| `cargo test -p rs_cam_core --lib` | 2296/2296 before the new sentries, 2302/2302 after |

Two integration targets (`generic_rest_routing_pr7`, `rest_routing_probe_e9`)
failed to *compile* on the first attempt against an in-flight edit in the SIM
lane's `dexel_stock/simulation.rs`; both pass on retry once that lane's tree
settled. Nothing in this wave touches that file.

## Not done, deliberately

- **`face.rs` has the same defect and is not fixed.** `face_toolpath_with_cancel`
  calls `zigzag_toolpath`/`oneway_toolpath` per Z level, each re-insetting the
  same facing rectangle. It is outside this lane's file ownership, and the win
  is small (the geometry is a rectangle, not a 1400-vertex cascade) — but it is
  the same finding and should be swept up when someone owns that file.
- **`generate_adaptive` is NOT a G2 site**, despite looking like one. Its
  per-level params carry `initial_stock` and engagement state, so its geometry
  is genuinely Z-dependent. Do not "fix" it.
- **Per-Z-level parallelism** (review's "missed parallelism" item 5, which the
  review itself gates on G2 landing) is now unblocked but not attempted. With
  the ratio at 1.0–1.1× the remaining per-level cost is emission, so the payoff
  is much smaller than it looked before the hoist — the parallelism item should
  be re-costed against these numbers rather than the Phase 0 ones.
