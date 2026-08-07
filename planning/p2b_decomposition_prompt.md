# Handoff prompt — P2.b: unified-finish decomposition + conditioning

Paste everything below into a fresh session.

---

Build P2.b of the unified finishing pass: the DECOMPOSITION + REGION-CONDITIONING
stage. Read `planning/unified_finish_planner_design.md` FIRST — it is the
reviewed design (decisions §"Decisions (user review, 2026-07-08)", risks
§"Risk register", staging §"Build order"). Tracker:
`planning/unified_finishing_pass_plan.md`. Recent history: P0 probe + P1
linker/descents shipped and live-confirmed (project 11194→9543 s, −14.7%,
collisions at the pre-existing baseline of 4); P2.a prep landed (shared
`region_mask` / `surface_link` / `crease_paths` modules; raster takes
`boundary_regions`). Branch `experiment/adaptive-spiral`, HEAD ≈ c3e29dc.

## P2.b scope (deliberately no toolpath emission)

1. New core module `finish_planner` (e.g. `crates/rs_cam_core/src/finish_planner.rs`):
   `decompose(heightmap/SlopeMap, params) -> PlannedRegions` producing three
   bands — shallow / mid-steep (scallop) / very-steep (waterline) — as CLEAN
   polygons via the shared `region_mask::region_polygons_from_mask`, plus
   crease centerlines from `detect_rest_valleys` with the corridor rule
   (crease stays inside its zone when `half_width < K × tool_radius`,
   default K=2; wider becomes its own region).
2. **R1 conditioning is the point of this phase** (top risk — the
   steep_shallow fragmentation ghost): hysteresis between thresholds (enter
   steep at `steep_threshold_deg` 45°, leave at −10°), morphological close,
   MIN-AREA ABSORPTION (regions under ~a few tool-diameters² merge into the
   surrounding band). Acceptance: wanaka terrain decomposes to O(10)
   regions, not O(100). Unit tests on synthetic fixtures (dome, cliff,
   branching valley — see `make_trench`-style fixtures in rest_field.rs for
   the pattern; margin rings needed, marching squares drops boundary-touching
   loops).
3. Debug/visual surface so the user can SEE regions before anything cuts:
   simplest honest option — reuse the rest-heatmap/region overlay path
   (RegionSet polygons already render; check `rest_heatmap_mesh.rs` + how
   rest_regions reach the GUI overlay) or emit region polygons through the
   existing debug-trace machinery. MCP inspection counts too. Do NOT build
   new GUI panels; reuse what renders polygons today.
4. Params on a new config struct (not yet a registered op — that's P2.c):
   `steep_threshold_deg` (45), `waterline_threshold_deg` (65, advanced),
   `overlap` (from stepover), `corridor_k` (2.0), hysteresis + min-area with
   defaults. One-new-dial spirit: everything else inherits from existing
   strategy params. Ball-tip-only enforcement lands with the op in P2.c.

## Standing rules

- wanaka (`planning/airrun_2026-06-01/wanaka.toml`) is LIVE-ONLY — never
  save_project from GUI/MCP. Commit only when the user asks.
- Cargo: heavy jobs exclusive — check `free -g` + `pgrep -f
  "bin/[c]argo|[r]ustc --crate-name"` (bracketed — self-match trap) first;
  one at a time; never workspace-wide `cargo test`; `cargo check` may run
  concurrently >20 GB free. Zero-warning clippy (16 deny lints); `cargo fmt`
  only pre-commit. Known reds: adaptive3d peck/rapid ×2 +
  planner_sim_dexel_parity.
- **Long BACKGROUND cargo runs get killed by something external on this box
  — run long suites in FOREGROUND chunks (<10 min each).**
- Sonnet agents implement after Fable specs precisely (agents get "no
  cargo"); Fable reviews diffs line-by-line and runs all gates personally.
  Analysis/design stays with Fable.
- Headless validation: `cargo test -p rs_cam_core --test p1_headless_ab_wanaka
  --release -- --ignored --nocapture` rebuilds the wanaka chain in-process
  (F.4 ladder) and prints per-op `runtime_by_intent` + collision gate — use
  this pattern for any A/B; no GUI needed. GUI relaunches only via the user.
- MCP generates: `timeout_s` + poll `list_toolpaths`; never re-issue an
  in-flight index. generate_all may cosmetically report "generation
  cancelled" for a slow first op (second race leg, fix candidate in the
  plan tracker) — the requeued job completes; check `list_toolpaths`.
- Keep the plan tracker, ledger (`planning/finishing_stack_review_2026-07.md`),
  and memory current as you land things.
