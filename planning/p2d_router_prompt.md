# Handoff prompt — P2.d: unified-finish router (+ coastline stencil warm-up)

Paste everything below into a fresh session.

---

Continue the unified finishing pass: FIRST the coastline classification
stencil fix (small, user-requested), THEN P2.d — the ROUTER. Read
`planning/unified_finish_planner_design.md` (decisions, corridor rule, risk
register, "Known classification gap" section) and the P2.c entries in
`planning/unified_finishing_pass_plan.md` +
`planning/finishing_stack_review_2026-07.md` first. Branch
`experiment/adaptive-spiral`, HEAD ≈ 06468a8.

## State

P0+P1 shipped (live −14.7%). P2.b decomposition committed (08af463):
`finish_planner::decompose` — hysteresis/close/min-area conditioning,
crease corridor rule, TRUE-surface classification
(`finish_setup::build_classification_surface_with_cancel` — NEVER classify
on the drop-cutter offset surface; it hides steepness ≤ ball radius).
P2.c committed (06468a8): `unified_finish.rs` orchestrator + registered
`UnifiedFinish` op (24th op, ball-tip-only) +
`ProjectSession::set_toolpath_operation` + A/B harness. CHECKPOINT #1
PASSED: B vs pinned A (8919.5 s / 6883.4 s finish): finish +1.9%, project
+1.4%, collisions 0; **cutting −12%** at better cusp; **entry+rapid
+830 s** from naive band-crossing plunges = P2.d's quantified target.
Also fixed: scallop ring-cascade exponential (offset_polygon concave
vertex inflation; drop-only `decimate_ring_polygon`; h=0.02 30 min→2.9 s;
root offset_polygon fix tracked, separate sweep-validated pass).

## Task 1 — coastline stencil (warm-up, user-caught)

Lake shorelines (~90° single-cell steps) don't classify: central
differences smear a one-cell cliff to `atan(h/(2·cell))` (~34° for a 1 mm
step at 0.75 mm cells). Fix per the design doc's "Known classification
gap": classification-only slope = max of one-sided forward/backward
gradients per axis (single-cell step then reads `atan(h/cell)`).
Generation surfaces untouched. Options: a `SlopeMap::from_z_grid_max_gradient`
constructor or a flag — pick what reads cleanest; steep_shallow keeps its
existing behavior. Re-run the wanaka acceptance
(`cargo test -p rs_cam_core --release --test finish_planner_wanaka_decompose -- --ignored --nocapture`)
and regenerate the SVGs (target/finish_planner_debug/) — the coast ring
should appear; check how min-area treats it (thin-ring absorption is a
P2.e sweep datapoint). Show the user the before/after SVG.

## Task 2 — P2.d router

Design doc "Pipeline" step 4: greedy nearest-neighbour over region
entry/exit points, seeded at the previous op's end; edge cost =
`min(retract_link_time, surface_link_time)` (both in
`machine_kinematics.rs`, P1; `LinkKinematics` bundle; surface candidate via
the shared `surface_link` module, only INSIDE the machining boundary —
never across excluded islands). 2-opt only if greedy leaves >5% on the
table (measure first). Emit chosen links between per-region toolpaths
(surface link = Linking feed moves; retract link = Retract/Rapid), replacing
the naive band concat in `unified_finish.rs`. This likely means the
orchestrator generates PER REGION (not per band) so the router can order
regions — the strategies already accept single-polygon RegionSets.
Acceptance: rerun `p2c_unified_finish_branch_b`
(`cargo test -p rs_cam_core --release --test p2c_headless_ab_wanaka -- --ignored --nocapture p2c_unified_finish_branch_b`)
— target: recover a meaningful share of the +830 s entry/rapid overhead
(getting B below A = full win); collisions ≤ 4 hard gate. A/Bs are cheap
now (~1 min per B chain).

## P2.c tails (after P2.d or opportunistically)

- Crease/pencil integration into the unified op (corridor rule inputs).
- Live GUI validation (needs `cargo build --release -p rs_cam_viz` +
  user relaunch; wanaka LIVE-ONLY, never save_project).
- offset_polygon root fix (sweep-validated separate pass).
- Test-diet: name the 294 s always-on sentry (likely
  adaptive3d_planner_stock_xy_f027), shrink only with red-proof via a
  reachable pre-fix path.
- generate_all cancel race leg 2 (cosmetic).

## Standing rules

- wanaka (`planning/airrun_2026-06-01/wanaka.toml`) LIVE-ONLY; commit only
  when the user asks.
- Cargo: heavy jobs exclusive — `free -g` + `pgrep -f
  "bin/[c]argo|[r]ustc --crate-name"` (bracket a char — self-match trap,
  it bit AGAIN today via pkill) before launch; never workspace-wide
  `cargo test`; `cargo check` concurrent OK >20 GB free. Zero-warning
  clippy; `cargo fmt` pre-commit only. Known reds ×3 (adaptive3d peck /
  rapid_segment / planner_sim_dexel_parity).
- LONG RUNS: never pipe through `tail` (buffers everything — cost us a
  branch-A table today); redirect to a file and `Monitor` it with
  `tail -f | grep --line-buffered`. eprintln/stderr is unbuffered.
- The sysml-service cargo jobs on this box contend hard (the first A/B's
  2 h was partly them) — check `pgrep -fa sysml` before trusting timings.
- Sonnet agents implement to precise specs (no cargo); Fable reviews
  line-by-line + runs all gates. Analysis/design stays with Fable.
- Keep tracker + ledger + memory current as things land.
