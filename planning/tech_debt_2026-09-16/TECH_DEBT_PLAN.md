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
| 2 | L1 | A | S | the load-time dressup migration rewrites a v3 operator value and reports only to `tracing::info!`; must push a `ProjectLoadWarning` like its sibling `parse_tool_type` | W1 |
| 3 | L2 (report arm) | A | S | `stock_to_leave_radial` is inert in the planner but still `ParamDef::required` and emits no `DeprecatedDialFinding` — the exact hole that machinery exists to close | W1 |
| 4 | L3 + L4 (+D1) | B | S | `_legacy_feeds_auto` and the top-level `toolpaths` pre-setup reader read pre-v3 shapes that `check_format_version` already refuses; delete both | W1 (ruled: delete L4 too) |
| 5 | L6 | B | M | the whole `machine_ref` chain is dead end to end (nothing writes it, save persists it, load drops it, `SetMachineRef` has no constructor, `machine_library_link_cleared` is always `null` on the wire) | W1 |
| 6 | Q2 | B | S | the only production file-wide `#![allow(clippy::indexing_slicing)]` (`cli/sweep.rs`, ~18 sites) | W1 |
| 7 | L5 | B | S | `standing_material_mm2` is still emitted beside `truncated_core_mm2` although core documents the old name as measuring the wrong quantity | W1 (ruled) |
| 8 | S2 | C | L | `viz.rs` `toolpath_to_3d_html` + `simulation_3d_html`: 765 dead lines | W2 ✅ d76fa7bc |
| 9 | S3 | C | M | `rs_cam_viz/src/io/presets.rs`: a whole dead module (280 lines, 9 tests) | W2 ✅ |
| 10 | D2 | C | S | `compute/semantic_helpers.rs` is dead and `pub`-re-exported; `spans.rs` carries its own drifted `CutRun`/`cutting_runs` | W2 ✅ 73d4ce1c |
| 11 | S4, S5, S6, S8, S9, S11–S24 | C | M | 64 confirmed-dead pub items and private helpers, ~900 lines across core and viz (per-file groups in `S_*.md`) | W2 core ✅ S5 ebfa0a49 · S6 1cf332ae · S8 5e11d53d · S9 e15d4e1e · S11 c90da49a · S12 d6ba1f04 · S13 ff95800c · S14 6d1fc0c5 · S15 7f4a0ddb · S16 818342bb · S17 4ba62234 · S21 62ce17da · S23 a65c9469 + f8a869df; viz rows open |
| 12 | S26, S27, S28 | C | S | 26 `allow(dead_code)` attributes that cannot fire (items are `pub` in a `pub mod` of a lib crate) and claim a live MCP surface is dead | W2 |
| 13 | S25 | C | M | 29 `pub` items used only from tests and not declared fixtures → `pub(crate)` + `#[cfg(test)]` scope or move into the test | W2 core ✅ dab59c0e; viz + power-calcs rows open |
| 14 | L8 | C | S | three `Command` variants with no production constructor (`ReplaceSetupsAndToolpaths`, `SetProjectName`, `SetMachineRef`) | W1 (ruled) |
| 15 | L7 | C | S | a trace with no `provenance` block is treated as fresh; the one producer is the CLI | W1 (ruled) |
| 16 | D11 | D | M | viz mirrors core's two simulation structs by hand (the class that dropped 11 fields in C04); inherits G-MCPSIMMIRROR / WP28 | W3 |
| 17 | D13 | D | M | each finding's operator sentence is written twice (`from_generation.rs` vs `narrate.rs`) and the copies already differ | W3 |
| 18 | D12 | D | M | two "mesh surface Z at (x, y)" readers with different containment tests (`monge.rs` vs `reach_map.rs`) | W3 |
| 19 | D4, D5, D6, D7, D8, D9 | D | S each | CLI string coercers; panic-payload readers; library `list_in`/`rename_in`; four `polyline_length`s; four cache counter scaffolds; two dashed-line emitters | W3 |
| 20 | Q5 | D | M | the `never_cancel` + `expect("… never cancelled")` idiom copied 26 times, each with its own allow | W3 |
| 21 | Q6 | D | S | a test re-implements the flat-shelf histogram verbatim, so it cannot catch drift | W3 |
| 22 | S29 | E | L | 199 `pub` items with own-file-only callers → compiler-checked demotion, one crate per cycle (30-row sample: 0 false positives) | W4 |
| 23 | S30, L12, L13 | E | S | `setups_mut` visibility; `MachineProfile::from_key` pub for a deleted loader; `simulation.rs` calls itself legacy yet is the live `StockMesh` path | W4 |
| 24 | L9, L10, L11 | E | S | `parse_lenient` aliases of a deleted loader (also the MCP mutation parser); two duplicated wire keys | W1 (ruled) |

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
5. **L7** — a trace with no `provenance` block is stale, not fresh.
6. **L8** — delete the three no-constructor `Command` variants with their
   registry rows, `CommandId` arms, `fmt` arms and the two tests.

## For the power-calcs owner (not touched here)

Q3 (`classify_3d_terrain` unreachable), Q4, Q11/T-5, S7/T-1
(`enumerate_matching_rows`), S10 (`load_dir`), S22, S33 (live only through
uncommitted work — do not delete), D15, L16, five S25 rows, eight S29 rows.

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
| S3 | — | `io/presets.rs` (280 lines) went out in `0761b14d`; this commit removes the dangling `pub mod presets;` that left the tree unbuildable |

## Progress

- [ ] W1  - [ ] W2  - [ ] W3  - [ ] W4  - [ ] ruled items  - [ ] review + re-scan
