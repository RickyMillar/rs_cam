# Handoff prompt — unified finishing pass, P0 probe

Paste everything below into a fresh session.

---

Run the P0 probe of `planning/unified_finishing_pass_plan.md` (read it FIRST — it is
the plan + tracking doc; this session's job is its "P0 — PROBE" phase, then record
the decision).

## Question the probe answers

What fraction of the wanaka FINISHING wall-clock is linking overhead (air-cut feed
links + retracts + plunges + rapids) vs cutting-in-material — measured with the
F-034 kinematics-aware cycle-time integrator (real machine profile: $$-imported
per-axis accels 500/500/270, junction deviation 0.020)? Decision gate: overhead
share ≥ ~15–20% → proceed to P1 (quantitative linker) + P2 (planner); below →
morphed spiral proceeds as a quality-only feature (P3 direct) and the planner is
deprioritized.

## Method

1. GUI runs with `--mcp` (ask the user to launch/relaunch). Project:
   `planning/airrun_2026-06-01/wanaka.toml` — **LIVE-ONLY, NEVER save_project**.
2. Rebuild the op chain from fresh load: `generate_all` (3 fresh ops succeed, rest
   ops error — expected), then the F.4 ladder: `run_simulation` → regenerate the
   first errored op → sim → next … until `3D Finish 6` (index 7) is Done. Ladder
   works since 37ef782; disabled ops don't block it. Add the finishing detail ops
   the probe needs on setup 2 (index/params from the plan's evidence table context):
   a selective fine pass and/or a `Pencil` (detector `rest_depth`, tool Ø2 tapered
   ball = tools-array index 4, reference machined stock via
   `set_stock_source(from_remaining_stock)` after a sim — see the R2 recipe in
   `planning/pencil_investigation_2026-07.md`).
3. `run_simulation`, then read `get_cut_trace` summary: `cutting_runtime_s`,
   `air_cut_time_s`, `rapid_runtime_s`, per-toolpath `total_runtime_s`. Check
   whether per-`MoveIntent` TIME (Retract / Linking / EntryPlunge / FinishingCut)
   is exposed anywhere; if not, a SMALL core addition aggregating integrator time
   by MoveIntent class per toolpath is in-scope (Sonnet agent OK for that after you
   spec it; verify gates yourself).
4. Decompose per finishing op AND for the finishing subset as a whole:
   %cutting-in-material / %air-links / %retract+plunge / %rapids. Sanity-check the
   integrator numbers against naive distance/feed to catch nonsense.
5. Write numbers + verdict into the plan doc's "P0 results log" + tick the tracking
   boxes; update memory (`project_multitool_finishing` or a new
   `project_unified_finishing` memory) with the decision.

## Standing rules

- wanaka LIVE-ONLY (no save from GUI/MCP). Commit only when the user asks.
- Cargo: heavy jobs exclusive — `free -g` + `pgrep -f "bin/[c]argo|[r]ustc --crate-name"`
  first (bracket both patterns — self-match trap), one at a time, never
  workspace-wide `cargo test`; `cargo check` may run concurrently when >20 GB free.
  Zero-warning clippy; `cargo fmt` only pre-commit.
- MCP generates: pass `timeout_s` (e.g. 120) and poll `list_toolpaths`; abort with
  `cancel_generation` (both landed 482be35). Do NOT re-issue generate for the same
  index while in flight (it cancels-and-requeues).
- Screenshots: `screenshot_toolpath` with `include_rapids: false` (note F.5b: linking-
  intent rapid-overs still draw — known, filed).
- Known reds (pre-existing, don't chase): adaptive3d peck/rapid ×2,
  planner_sim_dexel_parity.
- Implementation via Sonnet agents after YOU spec precisely; you review diffs and
  run all gates. Analysis/design decisions stay with you.
- Keep `planning/unified_finishing_pass_plan.md` + memory current as you go.
