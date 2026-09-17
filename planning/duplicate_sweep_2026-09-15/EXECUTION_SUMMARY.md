# EXECUTION_SUMMARY.md — duplicate sweep remediation

Executed 2026-09-16 from `CLEANUP_PLAN.md`. Orchestrated by Claude Fable 5.1;
three parallel Opus agents implemented the items in disjoint file sets, one
Opus agent landed the tail items, one Opus agent ran the read-only
completeness review. Baseline commit `18584671`; every item is one commit
on `master`.

## Rulings applied

- No legacy project-format support. Core writes and reads
  `format_version = 3` only. Old files fail to open; that is intended.
- Fixture `size_y` default is 15.0 everywhere. `size_x` stays 30.0.
- Six plan items were corrected before dispatch (see "Execution
  corrections" at the top of `CLEANUP_PLAN.md`): C01 orphans, C02 scope,
  C03 rescoped to a format-version refusal, C06 answered, C12 keeps the
  live transform carrier, C13 gate replaced by named sentries.

## What landed, per phase

| Phase | Items | Commits |
|---|---|---|
| 0 tooling | C00 | `e9597365` |
| 1 live bugs | C01, C02, C03, C04, C05, C06 | `9da22f20` + `3f9600db`, `54cd4e4e`, `afed18bf` + `b987c268`, `d22beed6`, `b6318de6`, `68670286` |
| 2 I/O consolidation | C10, C11, C12, C13 | `5f367169`, `d044063b`, `51394156`, `07df301b` |
| 3 merges | C20, C21, C22, C23, C24, C25, C26, C27, C28 | `f7fc41b9`, `114f3006`, `9a1d5fc2` + `b3f5db33` + `54cd4e4e`, `68670286`, `43b6db83`, `681f0dd2`, `573f0743`, `28d5c6dd`, `21884390` |
| 3b tail (re-run) | C29, C30, C31 | `24110475`, `c35681c0`, `22c00ef0` |
| 4 docs | C40, C99 | this file |

Net diff for phases 0–3: 74 files, +2367 / −5378 lines (23 commits).

Highlights:

- **Phase 1.** The viz project-load fallback, the legacy session builder
  and the private extension table are gone (C01). Core refuses any
  project whose `format_version` is not 3 with
  `SessionError::UnsupportedFormatVersion` (C03). The CLI sweep baseline
  keeps every job-file field; the mirror had dropped eleven, not four
  (C04). One artifact writer with pid + sequence names serves the trace
  and cut writers (C05).
- **Phase 2.** `crates/rs_cam_viz/src/io/project.rs` (2106 lines) and
  the second data model `state::job::{JobState, Fixture, KeepOutZone,
  FixtureKind}` are deleted. A `SetupFrame { face_up, z_rotation }`
  carrier replaces the viz `Setup` at the four transform sites (C12).
  Core owns one extension table and a typed `ProjectLoadWarning` channel
  (C10). One model reader `io::load_geometry` and one builder
  `io::model_from_geometry` serve both doors; STEP models keep their
  declared units at both doors (C13, ratified by the orchestrator).
- **Phase 3.** New homes: `ui/components/format.rs`, core
  `controller_compensation_for`, one `SimBoundary`, `memo.rs` +
  `grid.rs` for the map caches, `geo::resample_polyline`,
  `compute::spans::RuntimeLabel`, `benches/support/mod.rs`. 625 dead
  lines left `dexel_mesh.rs` (C27).

## Resolutions recorded in the plan

- C11: the legacy round-trip tests had no converter to move to; core
  writes `boundary_inherit`, so the o1b sentry covers the round trip.
- C13: the STEP `units` divergence was resolved rather than stopped on:
  the interactive door recorded `Millimeters` after scaling, which made a
  save write the wrong units for an inch-authored file. Both doors now
  keep the declared units, which the G-STEPUNITS sentry already pinned.
- C23: `finish_surface_cache` stays outside `MeshMemo` (content-keyed,
  no `Weak` identity). Reason in the `memo.rs` module doc.

## Gates

- `cargo clippy --workspace --all-targets --features rs_cam_core/heavy-tests -- -D warnings`: exit 0 on `21884390`.
- `cargo fmt --all -- --check`: clean.
- Every item ran its named proof test and crate clippy before its commit
  (results in each agent's report, summarised in the commit messages).
- The full heavy test gate was NOT run (operator ruling 2026-09-11).

## C99 — verification re-run

Index refreshed (SocratiCode chunk count 19588 → 19519), then
`python3 scripts/duplicate_sweep.py --threshold 0.92` on `21884390`:
report at `rerun_2026-09-16.md`.

| Measure | Before (2026-09-15) | After |
|---|---|---|
| src pairs ≥ 0.92 | 59 | 22 (before C31), **14** final |
| test pairs ≥ 0.92 | mixed in | 418 (reported separately) |

The 22 residual src pairs:

- 12 are the SIBLING / FALSE-POSITIVE answer key (C40): I05 pair 2, I08
  B/C/D, I09 P1/P3, I10 pairs 2/3, the `tier_map_cache` ↔
  `reach_map_cache` statics and stats that C23 keeps per cache by design,
  core `ToolpathDiagnostic` ↔ the CLI's documented report mirror, and the
  `depth_beyond_stock` adapter that delegates to core.
- 5 are out-of-line test modules under `src/` (`tests.rs`,
  `workflow_tests.rs`, `results_parity_tests.rs`,
  `gen_parity_p0_tests.rs`, `unified_finish_*` sentries). C31 teaches
  the script to classify them as test.
- 2 are label tables with different label sets by design
  (`issue_kind_label` vs `issue_kind_short_label`; `format_cycle_time`
  `h:mm:ss` vs `format_cycle` `m:ss.s`).
- 3 were genuine duplicates the first sweep missed: the feeds
  family-label tables (C29, drifted capitalisation), and two viz
  `span_kind_label` copies of core's `SpanKind::label` (C30).

Tail items (Phase 3b):

- C29 `24110475`: the Title-Case copies in `ui/properties/mod.rs` are
  gone; the LUT viewer reads `ui/feeds/shared.rs`. No test pinned either
  spelling, so the shared sentence-case labels win.
- C30 `c35681c0`: core had no `SpanKind::label()` (line 396 was
  `RegionSpanRole::label`; the plan's premise was wrong). The viz table
  moved UP into core as `SpanKind::label()`; both viz copies deleted, five
  call sites delegate, no emitted string changed. Observed, not acted on:
  MCP span replies emit the CamelCase label but accept only the
  snake_case `as_key()` in a filter.
- C31 `22c00ef0`: the script marks a file declared under
  `#[cfg(test)] mod NAME;` (and `NAME/**`) as test scope; 15 such files
  found, the six named ones verified against their parents.

Final re-run on the closed tree (after C29-C31 and R1-R3), same command:
`src pairs: 14 · test pairs: 413` (1180 test-scope chunks dropped). The
14 are the SIBLING / FALSE-POSITIVE answer key plus the two by-design
label pairs listed above; `rerun_2026-09-16.md` is that final report.

## Known hazards

- Two mid-chain commits do not build on their own: `68670286` (C06+C23)
  swept a peer's staged `git rm` of `io/project.rs` before `io/mod.rs`
  dropped the module, so `68670286` and `43b6db83` fail to compile;
  `9da22f20` (C01) leaves one sentry red until `3f9600db`. HEAD is
  consistent. Bisect across this range with care.

## Follow-up candidates (out of scope, not started)

- In-core v3 shims in `session/project_file.rs`: `_legacy_feeds_auto`,
  the one-shot dressup migration, the legacy `machine_ref` drop.
- `format_cycle_time` (readiness, `h:mm:ss`) and `format_cycle`
  (optimize views, `m:ss.s`): decide whether the optimize surfaces
  should adopt the G-TIMEEST spelling.
- CLI `ToolpathDiagnostic` report mirror: derive `Serialize` on core's
  type and extend, if the CLI-local fields can move.
- `FaceSelectionStale` load warning was not ported to core (needs
  face-id validation core does not do).
- `Command::ReplaceSetupsAndToolpaths` and `Command::SetProjectName`
  have no production constructor left (both registry rows are `Skip` on
  GUI and MCP). Deleting them needs an operator ruling.
- A fourth in-core pre-v3 shim: the top-level `toolpaths` pre-setup reader
  in `project_file.rs` (beside the three above).
- C13 runs `check_winding()` on every project load; the project door
  skipped it before. A large model pays that cost per load. Consider a
  lazy report.
- `tests/pushcutter_band_query_g1.rs` holds a third `rolling_field`; a
  `tests/` target cannot import `benches/support`.
- `GridSpec` keeps its two constructors as inherent impls in
  `tier_map.rs` and `reach_map.rs`, away from `grid.rs` (by design).

## Completeness review (Opus, read-only, on `21884390`)

Verdict: the merges hold. No behaviour change beyond what each item
states, no `*_mut` escape hatch, no GUI concern in core, every named
symbol gone or in one home, all 18 named sentry files present. Three
findings needed code, landed as R1-R3 (see Phase 3b in the plan):

- R1 DEFECT: `effects_are_stamped_wp19` still named the two call sites
  C01 deleted; no item gate ran that sentry.
- R2 GAP: the viz writer's round-trip tests died with it; core's own
  round trip pinned only three fields. A core test now pins the rest.
- R3: stale doc comments naming the deleted loader, `JobState` and
  `io/project.rs`; `MODEL_FILE_EXTENSIONS` now comes from core; the
  private `polyline_length` in `conformal_spiral.rs` is gone.

Commits: R1 `d7cd7357`, R2 `01b06992`, R3 `7185497f`. Workspace clippy
gate re-run clean on the closed tree (see Gates).

## Hand-off: three pre-existing red viz tests, not touched by this programme

`cargo test -p rs_cam_viz -q --lib` on the closed tree: 390 passed,
3 failed, all in `controller/tests.rs`, which the programme range
`18584671..HEAD` does not touch. The same three fail identically at the
baseline commit `18584671` (run in a throwaway worktree):

- `the_primary_and_the_builder_agree_about_a_runnable_project_ur3`
  (`:1139`, "fixture 1: the sample project must start ungenerated"):
  `sample_project_into` inserts a `ToolpathRuntime` with a result by
  construction, so the UR3 assertion contradicts its own fixture.
- `simulation_staleness_tracks_edits` (`:867`, "Fresh simulation should
  not be stale").
- `freshness_does_not_outrank_a_collision` (`:5311`, badge chip reads
  "1 safety", the test expects "collision").

All three sit in the UI review session's UR3/UP4 work (`f97327c3`,
2026-09-15). Left for that session. The viz `tests/` integration suite
as a whole was not run (it links many binaries and exceeds the small-gate
allowance); every item ran its named integration sentries instead.
