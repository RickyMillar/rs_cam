# Investigation review pass
Reviewed by: Claude (work account), 2026-09-16, after the 10-item swarm.

## Verdict corrections
- **I01, viz `save_project` — TRUE_DUP → DRIFTED_DUP (dead).** The finding's own items 5/6 list divergent fields (`visible`/`locked`/`auto_regen` vs `boundary_inherit`). "No production caller" is reachability, not behaviour equality; a viz-writer round-trip loses `boundary_inherit`. Cleanup action (delete) unchanged.
- **I10 pair 4 — verdict DRIFTED_DUP right; evidence understated.** `job.rs:150-174` `ToolDef` has 12 fields; `sweep.rs:393-410` `SerializableToolDef` has 8 — already drops `shank_diameter`, `shank_length`, `holder_diameter`, `holder_length`. Sweep baselines lose shank/holder geometry that deflection and reach modelling read. **Live bug, not a quick win.** Also `tool_type` is `CliToolType` (job.rs) vs `String` (sweep.rs) — needs `CliToolType: Serialize` with matching string form.
- **I02 walk_rows drift statement incomplete**: reach's copy drops rows at a *different site*, not just without instrumentation. The merged `walk_rows(Option<&AtomicU64>)` moves reach's drop site — the plan must confirm whether that is a latent perf regression before merging.
- All other verdicts well-evidenced. I05 pair 5 confirmed correct (`default_keep_out_size() = 20.0` both sides); the Fixture `size_y` 15.0-vs-30.0 is the only default divergence.

## Cross-item sequencing (conflicts resolved)
1. **slugify has two proposed homes** (I06a: `app/export.rs`; I10: `ui/str_util.rs`) — resolve to ONE module: `rs_cam_viz/src/ui/components/format.rs`, shared with I07's `format_cycle`/`format_delta`. `ui/components/` is the documented shared home.
2. **I05 pairs 4+5 vs I01 steps 4-5 touch the same code** — I01 step 5 deletes `state::job::{JobState, Setup, Fixture, KeepOutZone}` outright, so I05 pairs 4+5 become a deletion, not a merge. Only the `size_y` reconciliation survives as separate work.

Correct order (→ encoded in CLEANUP_PLAN.md):
1. I01 step 1 fallback narrowing (live bug) 2. I05 size_y reconciliation 3. I01 step 2 pin migration (silent data loss) 4. I10 pair 4 tool fields (live evidence loss) 5. I04 artifact_io + pid/seq naming 6. I01 steps 3-5 7. I05 pairs 6/3/1 8. I02 memo/ + I08 A/E/F + I09 P2 + I03 deletion + I06b 9. shared-home merges last (I06a+I07+I10p1 → format.rs; bench helper).

## Gaps for the cleanup plan
- **I01 step 1 doesn't fix the legacy mis-load**: a pre-v1 file omitting `post`/`machine` parses in core and never reaches the fallback/legacy loader. Core needs a version/marker check refusing pre-v1 files.
- **Viz sentry imports a type I01 deletes**: `boundary_controls_always_visible_g_boundaryinherit.rs:34` imports viz `ProjectToolpathSection` — port to core's type as part of I01 step 4.
- **I01 step 4/5 deletes I05's proof tests** (`io/project.rs:1824-1911` legacy round-trips) — the round-trip must move to a test over the new legacy→core converter.
- **I03 stale-doc references**: `rest_heatmap_mesh.rs:18`, `planning/PROGRESS.md:903`, `planning/DEXEL_Z_ONLY_INVESTIGATION.md:25` cite the dead path.
- **I08 pair A rests on hand-trace**: run the project_curve tests against the shared pencil resampler before deleting the copy.
- **No finding owns the sweep-script fix**: `scripts/duplicate_sweep.py` must drop inline `#[cfg(test)]` chunks (needed for the re-run verification step).
- **I05 pair 6 signature mismatch**: core takes `&[Polygon2]`, viz copy takes `Option<&[Polygon2]>` — call sites need a `?`/wrapper.

## Spot-check results
- Confirmed — I01 fallback at `controller/io.rs:526` (no error discrimination on `session_err`).
- Confirmed — I04 writers (`simulation_cut.rs:1758-1788` pid+WRITE_SEQ; `debug_trace.rs:530-548` still `"{ms}_{stem}.json"`). Line numbers exact.
- Confirmed with drift — I10 CLI mirror starts at `sweep.rs:360` (finding said 368); field-count error reported above.
- Confirmed — I05 pair 6 ranges and `pub(crate)` visibility.
- Confirmed — I03 dead path `z_grid_to_solid_mesh_heightmap` has no call site.
