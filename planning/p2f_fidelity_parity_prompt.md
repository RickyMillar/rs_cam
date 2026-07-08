# Handoff prompt — P2.f: band fidelity + live parity (before P3)

> ## STATUS UPDATE 2026-07-09 overnight (read this first)
>
> Task 1 is DONE headlessly (instrument + scallop chord-refinement fix +
> honest re-measure + sweep anchors re-run) — see the 2026-07-08/09
> entries in `planning/finishing_stack_review_2026-07.md` and the P2.f
> rows in `planning/unified_finishing_pass_plan.md`. Headline: root
> cause was chord infidelity (ring Z was always exact); post-fix B hits
> A-parity on the overcut histograms (mean|B−A| 0.054→0.012 mm), honest
> time finish −13.1% / project −10.1%, collisions 0. Artifacts + diff
> maps: `target/p2f_fidelity/`. REMAINING for Task 1: the user's eyeball
> on the live stock (GUI must be restarted on the fixed binary first).
>
> Task 2 was SOLVED by live G-code forensics (GUI was still open
> overnight; per-setup export in the session scratchpad): descent
> splits WORK live — the frame-mismatch theory is dead. The live
> entry_s 6× + "entry moves cutting through stock" was role-default
> RAMP ENTRIES on the MCP-added op (`for_role(Finish)` → Ramp;
> `REG_UNIFIED_FINISH` had ANY_DRESSUP so nothing stripped it —
> DropCutter's documented diagonal-trench failure mode; `emit_ramp`'s
> target-relative rapid floor is also the prime 60-collision suspect).
> FIXED: UnifiedFinish now strip_all in the registry + pin tests.
> REMAINING live (user + GUI on the fixed binary): re-add the unified
> op, confirm entry_s ≈ headless + collisions ≈ 4, the
> retract_strategy no-op question, and the eyeball on the
> chord-refined stock.
>
> The original prompt below is kept for context; its Task 1 is done and
> its Task 2 premise is corrected above.

Paste everything below into a fresh session.

---

Continue the unified finishing pass: P2.f — fix the BAND-FIDELITY defect
first (user-caught, invalidates part of the P2.e quality claim), then the
LIVE-PARITY cluster, then emission quality. Read the 2026-07-08 entries in
`planning/finishing_stack_review_2026-07.md` (five of them — measurement,
live validation, uncut-band addendum, root cause) and the P2.d/P2.e rows
in `planning/unified_finishing_pass_plan.md` first. Branch
`experiment/adaptive-spiral`.

## State

P2.b→P2.e all committed (08af463, 4ac80f1, 23616b2 + this session's
measurement commit). Headless verdicts vs pinned A (8919.5 s project /
6883.4 s finish, pin re-verified drift 0.0):

- B (UnifiedFinish, locked defaults 45/75): finish −16.2%, project
  −12.5% (−1116.9 s), collisions 0, removed 9720 vs 9988 mm³ (−2.7%).
- C (same strategies as standalone slope-windowed ops): +44.0% finish,
  leftover mean 3.4× worse — one-at-a-time can't see steepness
  (offset-surface windows); unified's gain is structural. C also showed a
  suspicious 3.4× removed+overcut signature — possible standalone
  slope-window rim-guard defect (ledger, unchased).
- Harness (`tests/p2c_headless_ab_wanaka.rs`): B-only pinned run,
  removed-volume column, stock-vs-model deviation stats, branches
  A/C (`p2e_branch_a_remeasure`, `p2e_separate_ops_branch_c`), Tier-2
  threshold sweep; Tier-1 conditioning sweep in
  `finish_planner_wanaka_decompose.rs`.

LIVE validation (GUI + MCP, wanaka LIVE-ONLY — the op was added live,
index 8 "Unified Finish 6 (live A/B)"; original Finish 6 disabled; DO NOT
save the project) found what headless missed:

## Task 1 — band-fidelity defect (TOP: user-caught "smooshed mountains")

The mid-steep flank comes out as coarse bars — detail beheaded. Root
cause (verified by scrubbing the live sim: the unified op CUTS the bars;
at op start the area is rough blobs): **scallop generates on an internal
heightmap at `cell = radius/4` where `TaperedBallEndmill::radius()`
returns the SHANK radius (3 mm) → 0.75 mm cells**; ring points
interpolate that grid, so texture finer than the cell (wanaka: 0.3–1 mm)
is blurred out of the generation surface. Waterline: same disease via
`sampling = 0.5` marching squares. Branch A's raster never suffers it —
exact per-point drop-cutter at 0.3 mm. PRE-EXISTING standalone
scallop/waterline limitation, exposed by the unified op; headless B's
leftover mean +16% vs A (0.314 vs 0.270 mm) WAS this, hiding in the
average.

Fix candidates (pick after a look; (1) is the favourite):
1. Exact-Z re-sampling of emitted points: ring/contour PLACEMENT on the
   coarse map, but re-drop-cutter each emitted point's Z (≈1 query per
   output point — raster-equivalent fidelity and cost).
2. Cell from TIP radius for tapered tools: `max(tip_radius/4, tolerance)`
   (0.25 mm here) — right regardless, but a floor, not a cure.
3. Waterline sampling clamped ≤ classification cell, or textured
   VerySteep falls back to scallop.

ACCEPTANCE (build the instrument first): add a per-band leftover
HISTOGRAM vs A to the harness (blunt means hid this twice) + a stock
PNG render from the headless chain (the sweep infra has 6-view stock
composites — reuse) so this class is visible without the GUI. Then:
histogram parity with A on the textured flank, B time re-measured (the
−16.2% may shrink; report honestly), collisions 0, and the user's
eyeball on the live stock. NOTE: the P2.e waterline-75 lock was scored
WITHOUT fidelity — re-run the sweep anchor rows after the fix; the lock
may move.

## Task 2 — live-parity cluster (worker path, identity setups)

Live unified op vs headless, same config: entry_s 3126 vs 489 (the
descent pass `optimize_entry_descents` runs in the GUI worker —
`crates/rs_cam_viz/src/compute/worker/execute/mod.rs` ~line 718 — but
does NOT split this op's plunges), 60 rapid collisions vs 0, spread
across the whole move range. Rivers (Setup 1, face Bottom, zero-rooted
frame) descends identically live and headless — the failure is specific
to Setup 2 (identity setup, world-frame stock per F-024). Suspect:
frame mismatch between `req.prior_stock` / `req.heights.top_z` and the
worker's toolpath frame. Audit, fix, re-validate live: entry_s ≈ 500,
collisions back to baseline 4. Related small fix: `retract_strategy`
is a NO-OP on unified output (byte-identical toolpaths) — make it apply
or hide it for the op.

## Task 3 — emission quality + tails

- Region-clipped raster retracts PER ROW (A's unclipped raster
  serpentines at the surface) — serpentine-connect row segments within
  a region; kills most of B's 4× rapid distance (41.9 vs 10 km).
- Crease/pencil integration into the op (user asked "where are the
  pencil moves" — currently by-design empty; corridor rule ready).
- min_area floor: verify what the planner actually gets for tapered
  tools (radius() = shank/2 = 3.0 here, so conditioning was Ø6-class;
  earlier "60 regions" claim was misread from span labels — measure the
  real region count by exposing `UnifiedFinishReport.route` somewhere
  debuggable) and add a tool-independent floor if small tools shred.
- offset_polygon root fix + C's slope-window rim-guard suspicion:
  separate passes, still tracked.

## MCP driving notes (from tonight)

- `add_toolpath` accepts `"unified_finish"` (docstring list is stale).
- `set_toolpath_heights` takes ABSOLUTE Z in the op's emission frame —
  wanaka Setup 2 is setup-local 0..25 (stock top 25): Finish 6
  equivalents are clearance 41.02 / retract 31.02 / feed 27 / top 25 /
  bottom −25. Pinning the stock_top-relative offsets as world absolutes
  puts the retract INSIDE the stock (12 k collisions, self-inflicted).
- `set_dressup_field` enum values as bare strings (`minimum`, not
  `"minimum"`).
- Unfiltered `get_cut_trace`/`inspect_collisions` overflow → saved file
  + `jq`.

## Standing rules

- wanaka (`planning/airrun_2026-06-01/wanaka.toml`) LIVE-ONLY; never
  `save_project`; commit only when the user asks.
- Cargo: heavy jobs exclusive — `free -g` + `pgrep -f
  "bin/[c]argo|[r]ustc --crate-name"` (bracket a char) before launch;
  never workspace-wide `cargo test` (lib target needs `--no-fail-fast`
  past the 3 known reds: adaptive3d peck / rapid_segment /
  planner_sim_dexel_parity); `cargo check` concurrent OK >20 GB free.
  Zero-warning clippy; `cargo fmt` pre-commit only.
- LONG RUNS: never pipe through `tail`; redirect to a file and Monitor
  with `tail -f | grep --line-buffered`.
- Sonnet agents implement to precise specs (no cargo); Fable reviews
  line-by-line + runs gates. Analysis/design stays with Fable.
- THE LESSON OF THE DAY (three times): blunt averages and small
  composites hid what the user's eyeball caught in minutes. Build the
  per-band histogram + headless stock render BEFORE claiming quality
  parity again.
