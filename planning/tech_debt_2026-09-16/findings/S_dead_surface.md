# S — dead and over-exposed production surface

**Tree under test:** working tree on `master` at `687cac85`, with
`compute/catalog.rs`, `compute/operation_configs.rs`, `depth.rs` and
`feeds/suggest.rs` modified. The instrument ran on the committed baseline.
Line numbers below are working-tree numbers; that explains the ~16-line drift
in `catalog.rs`.

**Verification method.** For each candidate the check counts `rg -nw` hits in
the item's own file and everywhere else under `crates/`, and splits each hit
into code and comment. A doc-comment mention is not a caller. The workspace is
exactly the four crates under `crates/` (root `Cargo.toml` `members`), so the
search covers every member.

**Blind spots closed.** Every dead `fn` row is a free function or an inherent
method. Rust forbids `pub fn` inside `impl Trait for X`, so no row is live
through a trait. The workspace has no `pub use …::*` glob re-export and no
`paste!` or `concat_idents!`, so no row is live through a glob or a macro-built
name. `rs_cam_mcp` has no `[[bin]]` target.

**Tier A and tier B: nothing found.** No row on this axis changes machined
output or loses data, and no row breaks a workspace rule. No
`FEATURE_CATALOG.md` entry names a dead item. The axis is C, D, E and F.

| id | tier | cost | file:line | item | verdict | OWNER |
|---|---|---|---|---|---|---|
| S1 | C | S | `crates/rs_cam_viz/src/render/colors.rs:49`; `ui/tokens.rs:35` | `COLLISION_POINT`, `SPACE_0` | DEAD, but HOLD — an active plan names both | ui-premium account |
| S2 | C | L | `crates/rs_cam_core/src/viz.rs:369`, `:684` | `toolpath_to_3d_html`, `simulation_3d_html` | DEAD (765 lines) | — |
| S3 | C | M | `crates/rs_cam_viz/src/io/presets.rs` (whole file) | 6 `pub` items + 9 tests | DEAD module (280 lines) | — |
| S4 | C | M | `crates/rs_cam_viz/src/state/simulation.rs:1253…1838` | 8 `SimulationState` read doors | DEAD (162 lines) | — |
| S5 | C | M | `crates/rs_cam_core/src/mesh.rs:137` | `from_stl_bytes` | DEAD (87 lines) | — |
| S6 | C | S | `crates/rs_cam_core/src/dexel_mesh_mc.rs:342,371,399` | `push_quad_top`, `push_quad_bottom`, `push_vertical_quad` | DEAD, private, 90 lines | — |
| S7 | C | S | `crates/rs_cam_core/src/feeds/vendor_lookup.rs:311` | `enumerate_matching_rows` (**T-1**) | DEAD, 21 lines | power-calcs |
| S8 | C | S | `crates/rs_cam_core/src/compute/config.rs:1315,1334,1760` | `untouched_material`, `reached_uncut_estimate`, `ALL_SIMPLE` | DEAD (44 lines) | — |
| S9 | C | S | `crates/rs_cam_core/src/svg_input.rs:62` | `load_svg_data_mm` | DEAD (28 lines) | — |
| S10 | C | S | `crates/rs_cam_core/src/feeds/vendor_lut.rs:395` | `load_dir` | DEAD (28 lines) | power-calcs |
| S11 | C | S | `crates/rs_cam_core/src/spiral_finish.rs:104,311`; `ramp_finish.rs:457,881`; `scallop.rs:2807`; `pencil.rs:2495`; `adaptive/mod.rs:281` | 7 `_with_cancel` / `_annotated` entry points | DEAD (98 lines) | — |
| S12 | C | S | `crates/rs_cam_core/src/sim_measurability.rs:258,457` | `publishable`, `engagement_is_floor_prone` | DEAD (27 lines) | — |
| S13 | C | S | `crates/rs_cam_core/src/tool_library.rs:151,170,212` | `save_library`, `append_tool`, `all_tools` | DEAD (26 lines) | — |
| S14 | C | S | `crates/rs_cam_core/src/semantic_trace.rs:872,957,965,976` | 4 writer / query doors | DEAD (25 lines) | — |
| S15 | C | S | `crates/rs_cam_core/src/enriched_mesh.rs:163,271,279` | `edges_for_face`, `edges_between`, `edge_chains_2d` | DEAD (23 lines) | — |
| S16 | C | S | `crates/rs_cam_core/src/session/reach.rs:184` | `reach_map_for` | DEAD (22 lines) | — |
| S17 | C | S | `crates/rs_cam_core/src/metrology/ownership.rs:68` | `from_labels` | DEAD (19 lines) | — |
| S18 | C | S | `crates/rs_cam_viz/src/state/job.rs:315`; `runtime.rs:58,83`; `viewport.rs:32`; `controller.rs:269` | 5 view-state items, incl. `ToolpathView` | DEAD (56 lines) | — |
| S19 | C | S | `crates/rs_cam_viz/src/ui/properties/mod.rs:69`; `components/kv_row.rs:246`; `sim_debug.rs:154` | `mcp_highlight_effect`, `row_hover_tint`, `json_f64` | DEAD (41 lines) | — |
| S20 | C | S | `crates/rs_cam_viz/src/render/colors.rs:26,37,40,79,80` | 5 colours; the two S1 rows are held | DEAD (8 lines) | — |
| S21 | C | S | `crates/rs_cam_core/src/fingerprint.rs:839`; `compute/catalog.rs:929,2705` | `save_mesh_composite_png`, `ui_style`, `new_default_with_ctx` | DEAD (28 lines) | — |
| S22 | C | S | `crates/rs_cam_core/src/tool_load/mod.rs:566`; `optimize/axes.rs:225` | `evaluate_project`, `active_axes` | DEAD (23 lines) | power-calcs |
| S23 | C | S | `crates/rs_cam_core/src/tool/mod.rs:56,66`; `grid2.rs:172,200`; `geo.rs:70`; `geom_cache.rs:307`; `dexel.rs:196`; `debug_trace.rs:441`; `diagnostics/mod.rs:295`; `machine_library.rs:87`; `tier_islands.rs:336`; `session/compute.rs:5785`; `session/mod.rs:1878` | 13 small accessors and helpers | DEAD (76 lines) | — |
| S24 | C | S | `crates/rs_cam_viz/src/ui/properties/operations/mod.rs:2073,2075,2077` | `ValidationTool.tool_type`, `.diameter`, `.cutting_length` | DEAD fields | — |
| S25 | C | M | 29 rows, `crates/*/src` | test-only `pub` API that is not a declared fixture | FIXTURE leak | 5 rows power-calcs |
| S26 | C | S | `crates/rs_cam_mcp/src/server.rs` (16 sites) | inert `#[allow(dead_code)]` | LIVE-false-positive | — |
| S27 | C | S | `crates/rs_cam_core/src/feeds/vendor_lut.rs` (9 sites) | inert `#[allow(dead_code)]` on serde fields | LIVE-false-positive | power-calcs |
| S28 | C | S | `crates/rs_cam_viz/src/compute/worker.rs:120` | `metrics_not_applicable` allow | LIVE-false-positive | — |
| S31 | D | S | `crates/rs_cam_core/src/tool_library.rs` | `*_in(dir,…)` / `*_library(…)` wrapper pairs | drift-prone | — |
| S29 | E | L | 199 rows across 4 crates | `pub` with own-file-only callers | VISIBILITY | 8 rows power-calcs |
| S30 | E | S | `crates/rs_cam_core/src/session/mod.rs:1906` | `setups_mut` | VISIBILITY, sentry-held | — |
| S32 | F | S | `crates/rs_cam_mcp/src/server.rs` | file name contradicts the crate contract | naming drift | — |
| S33 | — | S | `crates/rs_cam_core/src/depth.rs:68`, `:101` | `realised_step_down`, `roughing_pass_count` | LIVE-false-positive | power-calcs |

---

## Evidence

### S1 — two dead tokens an active plan still wants
`COLLISION_POINT` (`render/colors.rs:49`) and `SPACE_0` (`ui/tokens.rs:35`)
have zero call sites. Both appear in `planning/ui_premium_2026-09-13/`:
`PLAN.md:601` makes `COLLISION_POINT` the reference colour of a planned
palette-distance sentry, and `DESIGN_SPEC.md:58` gives `SPACE_0` the zero row
of the spacing scale. Neither document claims the token ships today, so this is
a HOLD, not a doc defect. The ui-premium account owns that plan. Keep both
constants and ask that account before any deletion.

### S2 — `viz.rs` ships four HTML emitters and two are dead
`crates/rs_cam_core/src/viz.rs` is 2 102 lines and defines six `pub` entry
points. Four are live: `toolpath_to_svg`, `toolpath_standalone_3d_html`,
`stacked_simulation_3d_html` and `stock_mesh_to_3d_html`, called from
`rs_cam_cli/src/main.rs`, `sweep.rs`, `run.rs`, `controller/io.rs` and
`rs_cam_viz/src/app/mcp.rs`. Two have zero callers:
`toolpath_to_3d_html` (112 lines) and `simulation_3d_html` (653 lines).
Delete both. Private helpers that only these two call become new `dead_code`
warnings on the next build. The compiler names that cascade. Do not enumerate
it by hand.

### S3 — `io/presets.rs` is a dead module, tests included
Nothing outside the file names `presets`, `presets_dir`, `list_presets`,
`save_preset`, `load_preset`, `delete_preset` or `Preset`. The only external
line is `pub mod presets;` in `crates/rs_cam_viz/src/io/mod.rs:5`. The file
carries 9 `#[test]` functions that test only the dead module. Delete the file
and the `pub mod` line. The 40 lines the zero-ref list found are a sixth of
what actually goes.

### S4 — eight orphaned `SimulationState` read doors
`current_boundary_index`, `trace_availability_for_toolpath`,
`toolpath_cut_summary`, `semantic_cut_summary`, `cut_worst_items`,
`cut_hotspots`, `trace_target_for_annotated_span` and `current_item_bbox`.
`app/mcp.rs` and `mcp_bridge.rs` carry no live twin of any of them, so this is
deletion, not consolidation.

### S6 — three private functions the compiler already reports
`#[allow(clippy::too_many_arguments, dead_code)]` sits on each. They are
private, so the allow is the only thing holding the warning down. Zero callers.
Delete all three and both allows.

### S7 — T-1 inherited and re-confirmed
`rg -nw enumerate_matching_rows crates/` returns the definition at
`feeds/vendor_lookup.rs:311` and one historical comment at
`tool_load/optimize/context.rs:139`. The register's diagnosis still holds. The
item is 21 lines. `OWNER: power-calcs`.

### S24 — a comment that states the opposite of the truth
Each of the three fields carries
`#[allow(dead_code)] // surfaced via 'tool_configs' lookup post-PR-3 cutover`.
The allow proves the compiler finds no read. The `tool.tool_type` reads at
lines 2203-2296 are on `ToolConfig`, not on `ValidationTool`. Delete the three
fields and their write sites at 2106-2108.

### S25 — test-only `pub` API
All 35 rows of `test_only_pub_api.md` verify: zero references in any `src`
outside the defining file. Six declare themselves in a doc comment and stay as
fixtures: `composite_panel_layout`, `reset_surface_build_count`,
`PRIZE_CLOSE_BELOW`, `reach_map_for_mesh`, `reset_drop_call_count`,
`generate_all_without_peer`. The other 29 are leaks. `reach_map_for_mesh` alone
has 25 test call sites, so the fixture pattern works when it is declared.
Fix: give each leak a one-line doc that names it a test door, or move it into
the test. `crates/*/tests` is an external crate, so `pub` must stay; the doc
line is what carries the intent. Five rows are power-calcs: `unit_family`,
`predicted_gate_observation_mm`, `write_to`, `preview_field_apply` and
`filtered_out`.

### S26, S27, S28 — allows that cannot fire
`rs_cam_mcp` is lib-only. Every item in `src/server.rs` is `pub` in a `pub mod`
of a library crate, so `dead_code` never fires there and all 16
`#[allow(dead_code)]` are inert. The attributes also mislead. `mcp_bridge.rs`
holds `AddSetupParam`, `ImportModelParam` and the rest in its command enum, and
`app/mcp.rs:4478-4692` reads `req.span_kind`, `req.span_id`, `req.pass_index`
and `req.include_drill_samples`. The attributes claim a live wire surface is
dead. The same argument applies to the 9 serde-provenance fields of
`VendorObservation` (`pub` struct in `pub mod vendor_lut`), and to
`SetupSimToolpath.metrics_not_applicable`, which `compute/worker/execute/mod.rs:91`
reads. Delete the attributes. If one turns out to be load-bearing, the build
says so immediately.

Allows that stay, with the reason:

- `session/compute.rs:1152` `set_toolpath_param` — the sentry
  `setters_have_rows_wp15a` reads the declaration. Documented.
- `viz/mcp_server.rs:86` `tool_router` — the `rmcp` macro needs the field.
- `cli/smoke.rs:44` `SmokeCase`, `cli/job.rs:185` `holder_length` — serde
  schema fidelity. Documented.
- `adaptive3d/path.rs:191,194`, `adaptive3d/clearing.rs:131,263` — strategy-
  specific and test-only fields. Documented.
- `feeds/provenance.rs:289` `ScallopHeight` — enum symmetry. Documented.
- `viz/compute/worker/gen_parity_p0_tests.rs:272` `Ignore` — a test module.
- Every `cfg_attr(not(test), …)` and `cfg_attr(not(feature = …), …)` row
  (`app.rs:61`, `scallop.rs:179`, `session/mod.rs:288,1906`,
  `worker/execute/mod.rs:308`) — a real conditional path.

### S29 — the 199 over-exposed items
`dead_pub_surface.md` lists 208 rows. The recount moves 8 to DEAD — they sit
in S5, S8, S9, S16, S21 and S23 — because their second mention is a doc
comment, not a caller. One row is a false positive: `roughing_pass_count`
(`depth.rs:101`) is called from
`tests/the_written_depth_is_one_the_machine_cuts_g_stair.rs:167`, an untracked
file of the power-calcs agent. See S33. That leaves 199 visibility rows.

A 30-row sample across the largest files (`ui_command.rs`, `simulation.rs`,
`sim_triage.rs`, `grid2.rs`, `catalog.rs`, `policy.rs`, `session/mod.rs`,
`mesh.rs`, `machine_kinematics.rs`, `tokens.rs`, `response.rs`) gives 30 of 30
with zero other-`src` hits and zero test hits. The 20 `ui_command.rs` rows are
`pub type …Args` aliases used once each, inside `for_each_ui_command!` in the
same file; `crates/rs_cam_viz/tests/command_surface_completeness.rs` names that
macro only in a doc comment, so the aliases need no external visibility.

### S30 — `setups_mut`
`pub(crate)`, `cfg_attr(not(test), allow(dead_code))`, one caller in the
`#[cfg(test)]` module of `session/save.rs`. The sentry
`tests/hatches_are_crate_private_wp7.rs` already holds the boundary, so this is
not a `*_mut` hatch violation. Move it behind `#[cfg(test)]`. The allow then goes too.

### S31 — `tool_library.rs` wrapper pairs
The module pairs a `*_in(dir, …)` form with a `*_library(…)` /
`*_tool(…)` form for each operation. Three wrappers are dead (`save_library`,
`append_tool`, `all_tools`) and two `_in` forms are own-file-only
(`update_tool_at_in`, `dedupe_in`). The module is live overall.

### S33 — two rows the instrument called dead and a peer agent revived
`realised_step_down` (`depth.rs:68`) and `roughing_pass_count`
(`depth.rs:101`) were dead at `687cac85`. They are live only through
uncommitted power-calcs work: the sole production caller of
`realised_step_down` is `feeds/suggest.rs:937`, a file that is `M` in the
working tree, and the tests of both live in
`tests/the_written_depth_is_one_the_machine_cuts_g_stair.rs`, a file that
became untracked during this session. Keep both `pub`. An implementer who
deletes either one before the power-calcs agent commits breaks that branch.

---

## Sweep proposal — one item, S29

`unreachable_pub` does not find these rows. That lint fires only for a `pub`
item inside a *private* module; every one of the 199 sits in a `pub mod`. The
sweep is manual and compiler-checked:

1. Take one crate. Demote each listed `pub` to `pub(crate)`, file by file.
2. Build that crate and its tests. `crates/*/tests` is an external crate, so
   any item a test needs fails to compile. Restore `pub` on that item.
3. Where the compiler accepts `pub(crate)` and the item has only own-file
   callers, demote again to private.

Risk is low: the sample measured a 0-of-30 false-positive rate against tests
and siblings. Estimate, at one crate per build cycle, cargo lane permitting:
`rs_cam_core` 129 rows / 3-4 cycles, `rs_cam_viz` 62 rows / 3 cycles,
`rs_cam_cli` 4 rows and `rs_cam_mcp` 4 rows / 1 cycle each. Roughly 8-9
build cycles for the whole sweep. Do it after the deletions, not before — S2,
S3 and S4 remove rows the sweep would otherwise touch.

---

## Summary — what to delete first

1. `rs_cam_core/src/viz.rs` dead HTML pair (S2) — 765 lines, one file, no
   caller in any crate.
2. `rs_cam_viz/src/io/presets.rs` (S3) — 280 lines and 9 tests, whole module.
3. `rs_cam_viz/src/state/simulation.rs` eight read doors (S4) — 162 lines.
4. The small-item batch S5-S24 — 770 lines across 41 files, including T-1.
5. The 16 + 9 + 1 inert allows (S26-S28) — 26 lines, and they stop a reader
   trusting a comment that contradicts the build.

Aggregate removal: **about 1 977 lines** of production code and its tests
(1 424 verified zero-reference, less the 4 lines S1 holds, plus 206 masked by a
doc comment, 240 further lines of `presets.rs`, 90 private lines in
`dexel_mesh_mc.rs` and 21 for T-1).
