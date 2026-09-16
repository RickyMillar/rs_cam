# TECH_DEBT_PLAN.md — ranked debt, 2026-09-16

Consolidated from four read-only triage findings (`findings/D_*.md` duplicates
at 0.88, `S_*.md` dead surface, `L_*.md` legacy residue, `Q_*.md` allows /
TODOs / size) over the evidence baseline `687cac85`. Each finding file holds
the code ranges, callers, sentries and proof test for its ids; this file only
ranks and sequences. Register rows (`planning/TECH_DEBT_REGISTER.md`) are
inherited by ID where they overlap.

Rubric: **A** live defect / silent data loss · **B** workspace-rule violation ·
**C** dead production code that reads as load-bearing · **D** drift-prone
duplication · **E** too-wide surface · **F** size and style.

Ownership: `crates/rs_cam_core/src/feeds/**` and `tool_load/**` belong to the
power-calcs agent. Findings there are listed under "For the power-calcs owner"
and are NOT touched by this programme's fix waves.

## Ranked table (above the cut line)

| rank | id | tier | cost | claim | wave |
|---|---|---|---|---|---|
| 1 | Q1 | A | M | `SuggestContext.model_bbox` is never populated by any surface, so the runtime-sanity stepover back-off never fires; `first_model_bbox` exists in the same file as the Suggest call | W1 |
| 2 | L1 | A | S | the load-time dressup migration rewrites a v3 operator value and reports only to `tracing::info!`; must push a `ProjectLoadWarning` like its sibling `parse_tool_type` | W1 ✅ 625a4b6d |
| 3 | L2 (report arm) | A | S | `stock_to_leave_radial` is inert in the planner but still `ParamDef::required` and emits no `DeprecatedDialFinding` — the exact hole that machinery exists to close | W1 ✅ dbc00a15 (deleted, not reported) |
| 4 | L3 + L4 (+D1) | B | S | `_legacy_feeds_auto` and the top-level `toolpaths` pre-setup reader read pre-v3 shapes that `check_format_version` already refuses; delete both | W1 (ruled: delete L4 too) ✅ 9c720ea3 |
| 5 | L6 | B | M | the whole `machine_ref` chain is dead end to end (nothing writes it, save persists it, load drops it, `SetMachineRef` has no constructor, `machine_library_link_cleared` is always `null` on the wire) | W1 ✅ d496e5df |
| 6 | Q2 | B | S | the only production file-wide `#![allow(clippy::indexing_slicing)]` (`cli/sweep.rs`, ~18 sites) | W1 |
| 7 | L5 | B | S | `standing_material_mm2` is still emitted beside `truncated_core_mm2` although core documents the old name as measuring the wrong quantity | W1 (ruled) ✅ 5e4165ce (with L10 + L11) |
| 8 | S2 | C | L | `viz.rs` `toolpath_to_3d_html` + `simulation_3d_html`: 765 dead lines | W2 ✅ d76fa7bc |
| 9 | S3 | C | M | `rs_cam_viz/src/io/presets.rs`: a whole dead module (280 lines, 9 tests) | W2 ✅ 863d9c70 (the 280 lines went out in `0761b14d`) |
| 10 | D2 | C | S | `compute/semantic_helpers.rs` is dead and `pub`-re-exported; `spans.rs` carries its own drifted `CutRun`/`cutting_runs` | W2 ✅ 73d4ce1c |
| 11 | S4, S5, S6, S8, S9, S11–S24 | C | M | 64 confirmed-dead pub items and private helpers, ~900 lines across core and viz (per-file groups in `S_*.md`) | W2 core ✅ S5 ebfa0a49 · S6 1cf332ae · S8 5e11d53d · S9 e15d4e1e · S11 c90da49a · S12 d6ba1f04 · S13 ff95800c · S14 6d1fc0c5 · S15 7f4a0ddb · S16 818342bb · S17 4ba62234 · S21 62ce17da · S23 a65c9469 + f8a869df; viz ✅ S4 3374e3c7 · S18 0092082f · S19 1328105e · S20 89806cea · S24 8d2e6bd5 |
| 12 | S26, S27, S28 | C | S | 26 `allow(dead_code)` attributes that cannot fire (items are `pub` in a `pub mod` of a lib crate) and claim a live MCP surface is dead | W2 viz+mcp ✅ 293165f1; S27 power-calcs open |
| 13 | S25 | C | M | 29 `pub` items used only from tests and not declared fixtures → `pub(crate)` + `#[cfg(test)]` scope or move into the test | W2 core ✅ dab59c0e; viz HELD (see the Wave 2b table); power-calcs open |
| 14 | L8 | C | S | three `Command` variants with no production constructor (`ReplaceSetupsAndToolpaths`, `SetProjectName`, `SetMachineRef`) | W1 (ruled) ✅ cd6bba1a (SetMachineRef went with L6) |
| 15 | L7 | C | S | a trace with no `provenance` block is treated as fresh; the one producer is the CLI | W1 (ruled) ✅ 3ead7135 |
| 16 | D11 | D | M | viz mirrors core's two simulation structs by hand (the class that dropped 11 fields in C04); inherits G-MCPSIMMIRROR / WP28 | W3 ✅ |
| 17 | D13 | D | M | each finding's operator sentence is written twice (`from_generation.rs` vs `narrate.rs`) and the copies already differ | W3 ✅ |
| 18 | D12 | D | M | two "mesh surface Z at (x, y)" readers with different containment tests (`monge.rs` vs `reach_map.rs`) | W3 ✅ |
| 19 | D4, D5, D6, D7, D8, D9 | D | S each | CLI string coercers; panic-payload readers; library `list_in`/`rename_in`; four `polyline_length`s; four cache counter scaffolds; two dashed-line emitters | W3 D5 ✅ D6 ✅ D7 ✅ D8 ✅ D9 ✅ |
| 20 | Q5 | D | M | the `never_cancel` + `expect("… never cancelled")` idiom copied 26 times, each with its own allow | W3 |
| 21 | Q6 | D | S | a test re-implements the flat-shelf histogram verbatim, so it cannot catch drift | W3 ✅ |
| 22 | S29 | E | L | 199 `pub` items with own-file-only callers → compiler-checked demotion, one crate per cycle (30-row sample: 0 false positives) | W4 |
| 23 | S30, L12, L13 | E | S | `setups_mut` visibility; `MachineProfile::from_key` pub for a deleted loader; `simulation.rs` calls itself legacy yet is the live `StockMesh` path | W4 |
| 24 | L9, L10, L11 | E | S | `parse_lenient` aliases of a deleted loader (also the MCP mutation parser); two duplicated wire keys | W1 (ruled) ✅ b0691763 (L9) + 5e4165ce (L10, L11) |

Cut line. Below it, recorded and not scheduled: D14 (two ~160-line band-dispatch
drivers, L), L14, L15, D10, S31, S32, Q7–Q12, L16, and every F-tier row. Long
functions are not split mechanically (Q11 / T-5 stays with its owner). No
blanket lint wave: the 516 "undocumented allow" headline is ~230 truly bare
rows and none is unsafe (Q8, Q9).

Held: S1 (`COLLISION_POINT`, `SPACE_0`) — dead today but named by the active
ui-premium plan owned by the other account.

## Rulings (operator, 2026-09-16)

"We don't need to support anything legacy if a breaking change is needed.
We are still in early development." All six resolve to the clean answer;
each lands in W1 (the agent that owns the files), one commit each, with the
breaking change stated in the commit body:

1. **L4** — delete the top-level `toolpaths` reader; saved files lose the
   empty `toolpaths = []` line.
2. **L2 deletion arm** — delete `stock_to_leave_radial` from the config,
   catalog, persisted file, MCP param schema and CLI job schema; update
   `toolpath_fields_round_trip_c11.rs` and the wire snapshot.
3. **L5 / L10 / L11** — retire `standing_material_mm2`, `air_cut_percentage`
   and `ProjectDiagnostics::verdict`; update the sentries that pin them
   (`standing_material_channel_am9.rs`, `air_cut_denominators_lh1.rs`,
   `results_parity_tests.rs`) and the CLI report readers.
4. **L9** — delete the five `parse_lenient` aliases; MCP accepts canonical
   tool-type names only; add nothing in their place.
   *Count correction (2026-09-17):* the table holds five TYPES and eight
   ALIASES — `endmill`, `flat`, `ballnose`, `ball`, `bullnose`, `vbit`,
   `taperedballnose`, `tapered_ball`. "Canonical names only" decides it, so
   all eight went.
5. **L7** — a trace with no `provenance` block is stale, not fresh.
   *Implementation note (2026-09-17):* the ruled inversion landed. The CLI
   producer (`cli/src/main.rs`, `SimulationCutTrace::from_samples`) does NOT
   gain a provenance block. It holds no `SimulationRequest`, and core's
   `machine_hash` is `spindle_rpm` + `rapid_feed` of one project while the CLI
   carries a spindle speed per phase. A block built there would either fail
   every comparison — the same answer the inverted arm gives, with more code —
   or read FRESH against a project it was never compared to, which is the
   hole the ruling closes. Nothing deserialises that artifact back today.
6. **L8** — delete the three no-constructor `Command` variants with their
   registry rows, `CommandId` arms, `fmt` arms and the two tests.

## For the power-calcs owner (not touched here)

Q3 (`classify_3d_terrain` unreachable), Q4, Q11/T-5, S7/T-1
(`enumerate_matching_rows`), S10 (`load_dir`), S22, S33 (live only through
uncommitted work — do not delete), D15, L16, five S25 rows, eight S29 rows.

L2 leaves two live references behind in that territory. `feeds/rationale.rs`
lists `"stock_to_leave_radial"` in two `match` arms (`:360`, `:481`) and names
it in a doc (`:76`); `feeds/suggest.rs:413` names it in a doc example. Both
are string alternatives, so they compile clean and nothing dispatches on them
any more. The power-calcs owner deletes them.

## Waves (disjoint file ownership, one commit per item)

- **W1 — A + B** (Q1, L1, L2-report, L3+L4, L6, Q2). Files: viz
  `controller/events/toolpath.rs`, `app/mcp/commands.rs`, `ui/properties/mod.rs`
  (Suggest call site); core `session/project_file.rs`, `session/save.rs`,
  `session/mod.rs`, `session/command.rs`, `compute/operation_configs.rs`,
  `compute/catalog.rs`, `machine.rs`; cli `sweep.rs`.
- **W2 — C deletions** (S2–S6, S8–S24, S26–S28, S25, D2). Wide but mechanical;
  split core / viz between two agents.
- **W3 — D merges** (D11, D13, D12, D4–D9, Q5, Q6).
- **W4 — E visibility** (S29 per crate, S30, L12, L13).
- **Ruled items** land after the operator answers, in whichever wave owns the
  files.

## Gates for every fix agent

- Item proof test, then the whole `--lib` suite of each touched crate, then the
  79 source-scanning sentries (`evidence/source_scanning_sentries.txt`; run
  `cargo test -p <crate> -q --test <name>` for those in touched crates), then
  crate clippy `--all-targets -D warnings`, then fmt. Item-scoped proofs alone
  missed programme 1's only defect.
- Workspace clippy gate at the end of each wave. No heavy gate.
- Cargo through the lane script; never `git add -A`; stage own files only.

## Wave 2b — landed (viz + mcp, tier C)

One commit per id. The commit hash cannot be written by the commit that
carries the change, so this table is filled by the closing docs commit.

| id | commit | note |
|---|---|---|
| S3 | `863d9c70` | `io/presets.rs` (280 lines) went out in `0761b14d`; this commit removes the dangling `pub mod presets;` that left the tree unbuildable |
| S4 | `3374e3c7` | eight dead `SimulationState` read doors, 170 lines; the `SimulationSemanticCutSummary` import goes with them and `ToolpathDebugBounds2` moves into the test module |
| S18 | `0092082f` | five dead view-state items, 62 lines; the `ToolpathConfig` and `SimulationState` imports and the "Combined view for UI code" header go with them. `transform_heightmap_mesh` is NOT held: `CLEANUP_PLAN.md` C12 planned to rework it, C12 is `[x]` done (`51394156`), and it landed with no caller |
| S19 (viz rows) | `1328105e` | `json_f64` deleted, 7 lines. `row_hover_tint` is HELD: it implements `ui_premium_2026-09-13/DESIGN_SPEC.md:580` §4.7 "Hover tints the whole row one ramp step", and its own doc cites §4.7. Same class as the S1 hold; the ui-premium account owns the call |
| S20 | `89806cea` | five dead colours deleted, 15 lines with their doc lines and the emptied "Mesh face palette" header. `COLLISION_POINT` stays: it is the held S1 row (`ui_premium_2026-09-13/PLAN.md:601`, `AUDIT.md:561`) |
| S24 | `8d2e6bd5` | **FINDING CONTRADICTED.** The finding says the `tool.tool_type` reads at 2203-2296 are on `ToolConfig`. They are not: `tool` binds from `ctx.tools.iter().find(...)`, which is `&ValidationTool`. `tool_type` has six reads and `diameter` four, so both fields are LIVE and both allows were inert. Only `cutting_length` is dead. Fix: delete `cutting_length` and its write site, and delete the three inert `#[allow(dead_code)]` lines |
| S26 + S28 | `293165f1` | 17 inert `#[allow(dead_code)]` attributes deleted: 16 in `rs_cam_mcp/src/server.rs` and one on `SetupSimToolpath.metrics_not_applicable`. No wire type changed and `tests/snapshots/mcp_wire_surface.json` is untouched |
| S25 (viz rows) | HELD | No change. The three rows are not test-only leaks: `INK_00` (`ui_premium_2026-09-13/DESIGN_SPEC.md:138`), `LANE_SCALE` (`DESIGN_SPEC.md:310`, `STATUS.md:169`) and `draw_trace_badge` (`PLAN.md:417`) are each named by the active ui-premium plan, so a "test door" doc line would state the wrong reason. Same class as the S1 hold. `generate_all_without_peer` is already a declared fixture |

## Wave 3 — landed (tier D, drift-prone duplication)

One commit per id. A commit cannot carry its own hash, so this table is filled
by the closing docs commit, exactly as the Wave 2b table was.

| id | commit | note |
|---|---|---|
| D11 | `d7c01c4c` | The viz `SimulationResult` becomes core's record plus the two viewport-only artifacts (`playback_data`, `cut_trace_path`), so `core_simulation_from_lane`'s twelve-field hand copy becomes a clone with the trace slot cleared. The viz `SimulationRequest`'s three loose fields (`kinematics`, `max_feed_mm_min`, `use_predicted_feed_in_gates`) become core's own `KinematicsContext`; the `.max(1.0)` feed clamp moves to the submit site. **PART OF THE FINDING REFUTED.** The finding's "keep core's structs, add a `SimulationRequestExtras`" does not fit the request: `SimulationRequest.groups` is an ADAPTER, not a mirror — viz `SetupSimToolpath` carries a `ToolConfig` that core's `SimToolpathEntry` replaces with a built `ToolDefinition`, and `build_core_simulation_request` carries the F-024 identity-frame rule (`local_to_global: None` ⇒ `local_stock_bbox: None`). Moving that to the controller is a simulation-flow change, which G-MCPSIMMIRROR / WP28 holds. `groups` and `memoize_prefix` stay viz-side. The C22(b) re-export pattern also does not fit, because neither viz struct was field-identical to core's |
| D13 | `d7d48440` + follow-up | Five findings that BOTH surfaces render — `BoundaryClipDroppedFinding`, `ZeroRemovalFinding`, `InertClaimsDialFinding`, `ClippedBandFinding`, `TipFloatFinding` — gain `fn message(&self) -> String` in `compute/config.rs`, the shape `DepthBeyondStock::message` already uses. The body carries no label and no newline: the narration adds its label and `\n`, the adapter adds its `Diagnostic` id and severity. Which text won, per pair: **boundary clip** — narration's "not with a smaller one" (`diagnostics/ids.rs:183` documents that wording), plus the adapter's "or use a smaller tool"; **zero removal** — the adapter's, pinned by `zero_removal_rest_pass_a4.rs:291` ("removes no material"); **tip float** — the adapter's, with narration's "CANNOT reach" casing, pinned by `pencil_tip_float_channel_d1.rs:380`, and `TIP_FLOAT_PROVENANCE.describe()` in place of the three loose constants (it still carries "vertical residual depth (mm)", pinned at `:382`); **clipped band** — the adapter's, which adds "pin {clip} to the real depth"; **inert claims dial** — the adapter's, which names the FULL territory the pass cut. Every sentry that pins a narration LABEL keeps it. Scope: `DroppedBandFinding` is not in the family — the adapter does not render it — and `truncated_core` / `offset_library_failures` / `ramp_reach_clamp` / `derived_stepover` / `claims_reference` / `deprecated_dial` / `region_cap_truncated` carry no shared finding struct to hold the sentence |
| D12 | `5f50d5db` | `metrology::monge::surface_z` delegates to `reach_map::surface_z_at`, the home the finding names, and its own barycentric walk goes. The home wins because its containment test IS `Triangle::contains_point_xy`, the predicate `dropcutter::point_is_over_mesh_xy` uses, so "is there surface here" and "how high is it" cannot disagree. **FINDING CONTRADICTED on direction.** The finding says the `monge` copy "admits triangles the `reach_map` copy rejects". The opposite holds: `monge` used `BARY_EPS = 1e-9` where the predicate uses `-1e-8`, and a `1e-14` degenerate guard where the predicate uses `1e-15`, so `monge` was the STRICTER of the two and the merge LOOSENS it by 9e-9 of a barycentric coordinate — an edge-of-triangle tolerance, not a metrology shift. The Z value also moves from barycentric vertex interpolation to the triangle's own plane, which agree to floating point on a planar triangle. Measured: `reach_map_p5`, `reach_map_residual_p5_1`, `reach_tolerance_source_p5_1`, `reach_policy_pr4`, `checkpoint_c9_sampled_reach`, `union_coverage_m1`, `band_cell_ownership_g2`, `classification_strategy_m3`, `multitool_preview_u1` and `wanaka_curvature_anisotropy` all pass unchanged. NOT in scope, recorded here: `tests/wanaka_curvature_anisotropy.rs:529` and `tests/valley_prize_census_h0.rs:668` each hold a test-local `fn surface_z` of the deleted shape; the finding names only the two `src` readers |
| D7 | `d8e4b3b5` | `geo` is the home and gains two functions beside `polyline_length`: `polyline_xy_length(&[P3])` and `polyline_length_2d(&[P2])`. `dressup::polyline_xy_len` and `adaptive3d::clearing`'s `polyline_xy_length` / `polyline_length_3d` are deleted and their ten call sites read `geo`. **The 2D/3D split leaves TWO XY functions, not one**, as the wave brief anticipated: `clearing`'s forecaster holds `P2` and `dressup`'s fold holds `P3`, and the two point types are distinct, so the split is in the point type, not in the measure |
| D8 | `c00bc8b3` | `memo::CacheCounters` is the one counter scaffold: `new`, `record_build`, `record_hit`, `read`, `reset`. The four caches keep their own `static` of it and their own public stats struct, which is C23's rule for the capacity and the key. `geom_cache` holds three, one per memoised product. `finish_surface_cache` is still not a member of the memo TABLE — it keys by content — but the counters are independent of the table, so it shares them. `cache_len`'s `t.len()` vs `t.entry_count()` drift is NOT closed: the two tables are different types and `finish_surface_cache` has no `entry_count`. One sentry could not run: `checkpoint_b_resolution_ab` names `geom_cache` but needs `heavy-tests`, and the 2026-09-11 operator ruling forbids that gate |
| D9 | `91cc6ac2` | NOT a SIBLING — the two dash models reduce to one walk. `render::toolpath_render::push_dashed_line` is the emitter, with `dash_len` / `gap_len` as parameters; `app.rs`'s `push_dashed_line_vertices` is deleted and its one caller (`app/gpu_upload.rs:684`, the alignment-pin axis) reads the render one. `push_dashed_segment`'s centre-gapped thirds are the SAME walk with the dash model expressed as a fraction of the segment: a 0.35 dash and a 0.30 gap put dashes on `[0, 0.35]` and `[0.65, 1]`, which is exactly what it drew. The fraction is why the gap reads the same on a 2 mm link move and a 200 mm one, so the two dash models stay two — but the walk is one. `dashed_segment_emits_two_sub_segments` passes unchanged (4 vertices, 3.5 and 6.5 on a 10 mm segment) |
| Q6 | `1edf52ae` | The flat-shelf histogram comes out of `adaptive3d::path::adaptive_3d_segments` into `path::flat_shelf_levels`, and `test_flat_area_detection_finds_shelf` calls it. The production behaviour is unchanged: the function keeps the bin size, the 2% threshold, the working-range test and the one-bin `too_close` test, and the caller keeps the `debug!`, the extend, the sort and the dedup. The test passes an empty `existing_levels`, which is what its inline copy did by omitting the check. The C2 audit note on the over-counted denominator moves onto the function |
| D6 | — | New `core/src/named_toml_library.rs` holds the mechanics both on-disk libraries share: `name_is_valid`, `path_in`, `list_in` and `rename_in`. Each library keeps its public names as one-line delegations. The error enum could NOT be a `From<std::io::Error>` bound as the finding proposed: `rename_in` also raises `InvalidName` and `AlreadyExists`, and both messages name what the library stores ("catalog", "machine"). A `LibraryError` trait with three constructors carries them instead, so no operator message changes. `library_dir` stays per library — the two read different environment variables |
| D5 | `78712bc2` | `rs_cam_core::panic_message` becomes `pub`; the viz worker copy and the viz panic hook both delegate. The finding names two readers; a third stood in `rs_cam_viz/src/bin/main.rs:140`. Core's fallback text `"non-string panic payload"` wins over viz's `"unknown panic"`, per the finding; no test pins either string |

## Progress

- [ ] W1  - [ ] W2  - [ ] W3  - [ ] W4  - [ ] ruled items  - [ ] review + re-scan
