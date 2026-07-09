# Unified Finish v3 — back to the original vision (claims + fused router)

Handoff prompt. Paste the short prompt at the bottom into a fresh session;
this file is the full brief. Branch `experiment/adaptive-spiral`.

## The vision (user, 2026-07-09, verbatim intent)

The project's original idea, which P2 drifted away from:

1. **First pass: a small ball (Ø2) that takes MOST of the material** —
   already finishing, not roughing, at whatever cusp it holds.
2. **Pencil is baked in, and claims first**: find the valleys/creases,
   say "pencil will handle this", and subtract them from the map.
3. **Classify what's left**: wall-like steep rest → contour; everything
   else → scallop.
4. **Fuse the whole small-tool pass into ONE toolpath without loads of
   retracts** — running pencil/contour/scallop as three serial ops
   creates linking that doesn't need to happen "if you are already
   finishing": the tool is at the surface; adjacent work should chain.

P2's unified op instead converged to "raster shallows + scallop
mid-steep, one 1 mm tip everywhere". It wins its A/B tests but it is a
single-tool band decomposition, not the cascade. The drift was
data-driven overfit to wanaka (see "why the drift happened") — the
architecture above is right for the general case.

## Architecture

**Op A — Ø2 ball bulk finish.** All-over scallop (or advisor-chosen
pattern) with the Ø2 ball. On smooth terrain this covers most of the
part; on textured flanks (wanaka) it will still bridge concavities and
hand a large rest share to Op B — that's fine and expected.

**Op B — fused rest finish (the new op).** One toolpath, one small tool
(the 1 mm tapered tip), three strategies, claims pipeline:

1. **Rest analysis on Op A's machined stock** (shipped: generic
   rest-analysis, `DerivedRestRegions`). Prefer machined-stock reference
   over analytic — the R2 validation showed the analytic Ø-reference
   overestimates ~14× when the finish reuses the tool family.
2. **Pencil claims creases/valleys FIRST**: ridge/valley extraction
   (R2 pattern — NMS + prominence + per-branch median gate, shipped in
   the pencil work) produces crease corridors; subtract corridor
   polygons from the rest regions.
3. **Classify the remaining rest islands**: wall-like (steep, smooth,
   tall — contour's home turf) vs open patch (scallop). The existing
   `finish_planner::decompose` slope bands are the starting point but
   the rule must be per-island geometry (steepness + smoothness +
   vertical extent), not slope alone — wanaka proved slope-only routes
   textured flanks to contour where it loses badly.
4. **Route everything as one path**: build the segment graph across all
   three strategies' outputs and order/link it to minimize retracts.
   The pieces exist: `link_kinematics` on `ComputeRequest` (quantitative
   linker — machine-envelope link costing, built for pencil hookup,
   never promoted to a cross-strategy router); the region-serpentine
   stay-down logic (6540045); the P2.d band-crossing router work.
   Data for why this is the payoff: contour's wanaka band cost 1 606 s
   in per-level entries alone; serpentine cut unified entry 489→150 s
   purely by not lifting between adjacent rows.

**Emission**: one `Toolpath` with per-strategy `Region` spans (labelled),
so the fidelity instrument, per-band timing attribution (P2.g Task 2),
and the GUI can all see who cut what.

## Why the drift happened (don't relearn this)

- `p2f_ball_rest_share_probe`: on wanaka's dendritic flanks every ball
  Ø2–6 returns 87–96 % of the mid-steep band as rest AREA (floor floats
  over concavities). So the cascade degenerated to "tip does the flank
  anyway" and the band decomposition followed. Note the user's point:
  rest AREA ≠ rest VOLUME — the Ø2 pass still removes most material.
- Contour "is DEAD" was a WANAKA verdict (textured flanks, no smooth
  walls, per-level plunge cost). It has never been tested on a
  wall-heavy part. v3 needs that part (see validation).
- Pencil valley-targeting works and is validated (669 mm vs 9 265 mm
  naive) but was never wired into the unified op.

## Validation plan

- **Second test part with real walls + smooth steeps** (new harness
  fixture — generate or import; must exercise contour honestly).
  Wanaka stays as the adversarial case, not the whole world.
- Instrument: FIDELITY-COLUMNS, **group-filtered** (bottom-setup
  cross-attribution poisons the <-.5 tail otherwise), once the
  open contradiction below is resolved.
- Metrics: total time (v3 two-op stack vs D all-over vs current unified),
  retract/link count + entry_s (the router's KPI), per-strategy quality
  histograms, collisions 0.

## OPEN TAILS — do these FIRST (both sit under v3's foundations)

> **STATUS 2026-07-09 late: BOTH TAILS CLOSED.** (1) Three-way probe
> (`p2g_three_way_probe`): instrument TRUSTWORTHY, gap REAL — pre==read
> toolpath (same Arc, 0 move diffs), sim tops == re-stamp EXACTLY (0.0 µm,
> 159 792 columns), and the cross-branch stock delta equals the re-stamp
> delta to the digit; the "envelopes equal" contradiction was a CROSS-RUN
> comparison of different regenerations. (2) The 20 collisions did NOT
> reproduce headlessly (0, fresh generation) — mechanism was STALENESS:
> live v2's cached result (descents baked against pre-13:27 stock with
> "3D Finish 6" enabled) survived the disable. FIX:
> `invalidate_result_chain` in session mutation + sentries. Full verdicts:
> `planning/p2g_quality_matrix_prompt.md` (VERDICT block) and the design
> doc `planning/unified_v3_design.md`.

1. **The instrument contradiction** (P2.g Task 1, reopened): the sim's
   stamped stocks show unified 48.5 % on-size vs D 67.7 % band-wide
   (real, paired, reproduces on the live v2 op), but the exact
   envelopes of the same toolpaths differ by mean −1.3 µm at every
   lattice phase. Prime suspect: the air-cut filter mutates the stored
   toolpath post-sim → as-stamped ≠ as-read. Run the THREE-WAY probe
   designed in `planning/p2g_quality_matrix_prompt.md` (late-night
   status block): per window cell in one run — (a) envelope of the
   post-sim toolpath, (b) re-stamp of it, (c) the sim's own column
   tops (group-filtered), (d) envelope of the toolpath captured BEFORE
   the first sim. Whichever pair diverges is the mechanism. v3 is
   judged by this instrument; it must be trustworthy first.
2. **20 rapid collisions in the live v2 unified op** (GUI sim
   2026-07-09 night, headless colfix run had 0 on the old file):
   all in TP15, local moves 8580/9193/9252/9310/9992/10356 (early
   cluster) and 79909–120576 (late cluster, worst z=19.996 at 119234).
   Diagnostic: inter-region rapids not lifting to safe-Z — i.e. the
   exact linking machinery v3's router leans on hardest. Reproduce
   headless on the CURRENT project file, fix, add sentry.

## Traps / standing rules

- `planning/airrun_2026-06-01/wanaka.toml` is USER-MODIFIED on disk
  (live session 2026-07-09 13:27): "3D Finish 6" DISABLED, enabled
  finish is "Unified Finish 6 (live v2)" (tool id 2, dial-identical to
  harness B75, 127 235 moves). Do NOT commit or revert it. Harness
  tests pinned to FINISH_OP_NAME="3D Finish 6" silently measure the
  live op twice — the column-overlay probe targets the enabled finish
  op with an assert; the acceptance/fidelity/chain probes still need
  the same retarget before any rerun.
- wanaka LIVE-ONLY: never `save_project`. Commit only gate-green
  milestones (P1/P2.f precedent). Cargo: heavy jobs exclusive
  (`free -g` + bracketed pgrep), never workspace-wide `cargo test`,
  core lib needs `--no-fail-fast` past the 3 known reds. Zero-warning
  clippy; `cargo fmt` (never standalone rustfmt). Long runs → file +
  background watcher, never `| tail`.
- LESSON (8×, hard-won): ground truth validated only against artifacts
  computed from the same inputs is circular; the sim's stock and the
  stored toolpath are not guaranteed to be the same object; and the
  project file evolves under live sessions — assert what you target.

## Build order (proposal, revise after research)

1. Tails (above): three-way probe → verdict on instrument; collision
   repro → fix + sentry.
2. Research pass (this session's job): read the claims/router-relevant
   code (`finish_planner`, `crease_paths`, `region_mask`, pencil ridge
   extraction, `unified_finish`, `link_kinematics`, serpentine/router
   sites, rest-analysis plumbing); write the v3 design doc with the
   per-island classification rule, corridor-claim geometry, router
   graph design, and the wall-part fixture plan; risk register; A/B
   harness plan (v3 stack vs D vs current unified on both parts).
3. Implement in slices, each headless-validated then live-validated:
   claims pipeline → fused emission with spans → router → advisor
   integration.

## Paste-prompt for the next session

> Read `planning/unified_v3_design_prompt.md` and work it in order:
> first the two OPEN TAILS (three-way instrument probe, then the 20
> rapid collisions in the live v2 op), then the v3 research pass and
> design doc. Wanaka.toml is user-modified — never commit, revert, or
> save over it. Commit gate-green milestones as you go.
