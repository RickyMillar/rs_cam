# Handoff prompt — pencil valley-targeting investigation (Fable, personally)

Paste everything below into a fresh session.

---

Investigate and fix the pencil operation's valley-targeting on real relief parts. **You
(Fable) do this investigation personally — do NOT delegate the analysis to Opus or Sonnet
agents.** Opus review agents have repeatedly failed to find the flaws in this code
(three separate passes missed them); the user has explicitly reserved this for you.
Sonnet agents are fine for mechanical fixes AFTER you've diagnosed and specified them.

## The user's observation (wanaka terrain, 2026-07-07, GUI)

- The terrain has deep valleys of rest material (now VISIBLE via the rest heatmap
  overlay — legend showed 0.15→3.04 mm on the mountainous half). In Fusion360, pencil
  traces those valley creases. Ours "seems to be picking non-ideal spots to mill."
- It "always does 3 equally spaced passes next to each other": that's
  `num_offset_passes = 1` → centerline + one offset each side at a fixed
  `offset_stepover` (0.5 mm) — completely blind to how WIDE each valley's rest blob is.
- Overall: "it seems like it's not doing what I want."

## History you must absorb first

- `planning/finishing_stack_review_2026-07.md` — the full P2 ledger (§P2 + change log).
- Memory `project_multitool_finishing`: 2026-07-03 diagnosis found ALL THREE original
  detectors tool-radius-blind; detector #4 (rest-depth field) was designed as the fix
  (`planning/pencil_restdepth_detector_prompt.md`) and LANDED — the user's complaint is
  about the rest_depth detector's OUTPUT, so the remaining flaw is in the rest field →
  centerline extraction → routing chain, not in "which detector".
- Commits `6a7e164` + `98a472e` (2026-07-06/07): selective finishing shipped. Key new
  instruments: rest heatmap renders live in the viewport for ANY op via
  `RestAnalysisConfig` (`set_rest_analysis_config` over MCP), regions via `RegionSet`,
  `AnnotatedToolpath.rest_regions/rest_grid`.
- Related open question (same root): region QUALITY. Self-probe depth threshold 0.15
  turned half the part into ONE region; pencil-derived regions were tight islands.
  "What is a detail region / where exactly should a fine tool go" is the shared core of
  both this investigation and the P2.4 advisor.

## Method — live diagnostic loop, one variable at a time

GUI runs with `--mcp` (ask the user to launch if not connected). Project:
`planning/airrun_2026-06-01/wanaka.toml` — **LIVE-ONLY, NEVER save_project**.

1. **See the field**: pencil op (Ø2 tapered ball, detector rest_depth, reference 6mm
   Ball Nose id 12) → generate → heatmap screenshot (`set_ui_view` + `screenshot_gui`).
   The heatmap IS the rest field — judge it first: does it light the deep valleys?
2. **See the centerlines vs the field**: overlay the pencil toolpath on the heatmap.
   Where do centerlines run relative to the red (deep-rest) spines? Suspects, in order:
   - `rest_field.rs` centerline extraction (medial/skeleton of the mask vs the DEEPEST
     path — a wide shallow blob's medial line is not the valley bottom);
   - the mask threshold semantics (depth threshold vs valley SALIENCY);
   - `pencil.rs` routing: `order_paths_nearest`, `min_cut_length`/`hookup_distance`
     filtering, and the fixed 3-pass offset layout (`num_offset_passes`,
     `offset_stepover`) — width-blind by construction. The rest field KNOWS the local
     rest width (`route_width_factor` exists); passes should derive count/spacing from
     it, or the offsets should be replaced by region-confined cutting.
3. **Compare to ground truth**: `run_simulation` + `screenshot_simulation` after the
   coarse op; the machined-stock reference (`RestReference` via FromRemainingStock) is
   the honest field — the R1/R2 reference modes landed earlier (commits 1bcc6c9/6e68068).
4. Small synthetic fixtures (V-groove, hemisphere pocket) in core tests to pin whatever
   you find — the pencil sentries exist; extend them.

Change ONE thing per iteration; screenshot each step; log findings + evidence in
`planning/pencil_investigation_2026-07.md` as you go (create it).

## Standing rules

- wanaka is LIVE-ONLY (no save from GUI/MCP). Commit only when the user asks.
- Cargo: heavy jobs exclusive — `free -g` + `pgrep -f "bin/[c]argo|[r]ustc --crate-name"`
  first, one at a time, never workspace-wide `cargo test`; `cargo check` may run
  concurrently when >20 GB free. Targeted `--lib` filters; zero-warning clippy;
  `cargo fmt` only pre-commit.
- Known reds (pre-existing, don't chase): adaptive3d peck/rapid ×2,
  planner_sim_dexel_parity.
- MCP generate calls have NO timeout — if a generate seems hung, check `list_toolpaths`
  status read-only instead of re-firing (cancel/timeout is an open follow-up).
- Keep `planning/finishing_stack_review_2026-07.md` + memory current.
