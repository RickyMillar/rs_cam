# EXECUTION_SUMMARY.md — tech-debt programme 2, 2026-09-16/17

Orchestrated by Claude Fable 5.1. Evidence baseline `687cac85`; ranked plan
`bdef37f1`; rulings `0761b14d`; fix range `0761b14d..HEAD`. Four read-only
Opus triage agents, five Opus fix agents (W1, W2a, W2b, W3, W4), one Opus
completeness reviewer. Every item is one commit on `master` with its hash in
`TECH_DEBT_PLAN.md`.

## Operator rulings

- 2026-09-16 (programme 1): no legacy project-format support.
- 2026-09-16 (this programme): "we don't need to support anything legacy if
  a breaking change is needed; we are still in early development" — every
  ruled item took the breaking answer; each commit body names the break.
- `feeds/**` and `tool_load/**` belong to the power-calcs agent; findings
  there are listed for that owner and were not touched.

## Instruments, before → after

| instrument | before | after |
|---|---|---|
| `pub` items with zero references (dead) | 65 | 8 |
| `pub` items used only in their own file | 208 | 55 |
| near-duplicate src pairs at cosine ≥ 0.88 | 137 | 110 |
| legacy / compat / migration lines in production | 526 | 479 |
| `allow(` in production code | 725 | 677 |
| functions ≥ 250 lines | 89 | 88 (not a target) |
| TODO markers | 12 | 10 |

Fix range: 65 commits, 168 files, +2612 / −4144 lines (net −1532; the
additions are one-home helpers, test doors and doc lines).

## What landed, per wave

**W1 — tier A and B, plus the ruled deletions (12 commits).**
Q1 `a98b7fca`: both add-toolpath doors now pass `SuggestContext.model_bbox`
(new `ProjectSession::model_bbox`), so the runtime-sanity stepover back-off
can fire; eight production sites still pass a default context (two behind
the other account's `draw_toolpath_panel`, one in the CLI smoke baseline). L1 `625a4b6d`: the dressup migration
reports through `ProjectLoadWarning::DressupsNormalized`. L2 `dbc00a15`:
`stock_to_leave_radial` deleted from config, catalog, file, MCP and CLI
schemas. L3+L4 `9c720ea3`: both pre-v3 readers gone; saved files lose the
empty `toolpaths = []` line. L6 `d496e5df`: the `machine_ref` chain and
`Command::SetMachineRef` gone; `machine_library_link_cleared` leaves the
wire. L8 `cd6bba1a`: `ReplaceSetupsAndToolpaths` and `SetProjectName`
deleted. L5+L10+L11 `5e4165ce`: `standing_material_mm2`,
`air_cut_percentage`, `ProjectDiagnostics::verdict` retired from the MCP
and CLI report wires. L9 `b0691763`: eight tool-type aliases refused;
canonical tokens only. L7 `3ead7135`: a trace with no provenance block is
stale. Q2 `4a8b9871`: the CLI's file-wide index allow gone. Overlap
`7a81ee4d`: five dead items in W1-owned files.

**W2a — dead code in core (18 commits, −1799 lines).** `viz.rs` HTML
renderers (−772), `compute/semantic_helpers.rs` (−158),
`metrology/ownership.rs` (−254, widened from one fn to the whole module),
`mesh::from_stl_bytes`, three dexel quad emitters, seven finishing entry
points, and 30 smaller items; 21 test doors documented.

**W2b — dead code in viz and mcp (8 commits).** `io/presets.rs` (−280),
eight `SimulationState` read doors (−173), five view-state items, colours,
the inert `allow(dead_code)` attributes in `rs_cam_mcp/src/server.rs`.
Correction: two of the three "dead" `ValidationTool` fields were live.

**W3 — drift-prone merges (14 commits).** D11 one `SimulationResult`
record; D13 five finding sentences with one `message()` home; D12 one
surface-Z reader; D5 one panic-payload reader (three copies); D7 one
polyline-length home (2D and XY variants); D8 `memo::CacheCounters`; D9 one
dashed-line walk; Q6 the histogram test calls production; D6
`named_toml_library.rs`; Q5 `interrupt::run_uncancellable` replaces 26
copies and 17 allows; D4 one CLI value coercer.

**W4 — visibility (9 commits).** 127 `pub` items demoted across four crates
(compiler-checked; 21 restored with a doc line naming the consumer; 24
marked "Test door"); `setups_mut` behind `#[cfg(test)]`; `simulation.rs`
doc corrected.

## Contradictions the agents recorded (all in the plan)

- D11: neither proposed shape fit; `SimulationRequest.groups` is an adapter
  (F-024 identity frame), not a mirror — left with G-MCPSIMMIRROR/WP28.
- D12: the finding's direction was inverted (`monge` was the stricter
  reader); the `reach_map` home won, delta 9e-9 barycentric, all
  reach/metrology sentries green.
- D9: not a sibling — one walk serves both dash models.
- L7: the CLI cannot emit a provenance block (no `SimulationRequest`);
  inversion only.
- L9: eight aliases, not five.
- S24: two "dead" fields were live.
- S25: `pub(crate)` cannot apply to items called from `tests/`; the "Test
  door" doc line is the recipe.
- S29: the instrument's false-positive rate was 4 of 171, not 0; a second
  blind spot (types exposed by `pub` signatures) is the `private_interfaces`
  class.

## Gates

- Per item: proof test, the whole `--lib` suite of each touched crate, the
  source-scanning sentries naming a changed symbol, crate clippy
  `--all-targets -D warnings`, fmt.
- Wave-end and final: `cargo clippy --workspace --all-targets -- -D warnings`
  clean; `cargo fmt --all -- --check` clean. Final suites: core `--lib`
  2527/0, viz `--lib` 382 passed / 3 known-red, cli 34/0, mcp 29/0.
- The heavy gate was not run (operator ruling 2026-09-11).

## Completeness review (Opus, read-only, on the closed waves)

Verdict: no DEFECT. Nothing in the 65 commits changes machining output, a
gate verdict or a persisted value beyond its item statement; every ruled
name has zero live sites; all 79 source-scanning sentries survived (the
D1-class needle check found no sentry that fell to zero sites); every
"Test door" line checks out; no `*_mut` hatch, GUI type in core, or
parallel flow. Six documentation GAPs and a few leftovers landed as R1–R4:

- R1: the `sim-analysis` skill, the `sim-diagnostics` agent and the
  machinist reference named four deleted `SimulationState` methods; two
  in-source docs still described retired wire keys as populated; two
  planning docs still called `metrology::ownership` shipped; the Q1
  remainder is eight production sites, not six (incl. the CLI smoke
  baseline, which still builds with the stepover back-off inert).
- R2: the CLI job file's two tool-type aliases (`endmill`, `ballnose`)
  are gone under the same ruling as L9.
- R3: an orphaned `TIP_FLOAT_RESOLUTION` const, three allows without the
  `SAFETY:` prefix, two fixtures that still wrote retired keys.
- R4: plan closed.

Commits: R1 `7e53684e`, R2 `f36fa3b3`, R3 `2089e29d`, R4 `545b7b79`.
Corrections found while fixing: no production serde alias for
`standing_material_mm2` ever existed (the only one is the pinned test-side
reader); `geo.rs` had three bare allows, not two.

## Hand-off: five pre-existing red tests, none touched by this programme

- viz controller `..._ur3`, `simulation_staleness_tracks_edits`,
  `freshness_does_not_outrank_a_collision` (UR3/UP4 work, `7b4e18f6`).
- viz `the_simulation_page_is_summary_first_dc6::off_workspace_run_producers_hold_their_recorded_ruling_ur3`:
  UR8 (`4a1d8e27`) cut `readiness_panel.rs` from 3
  `RunSimulation` producers to 1 without updating the ruling table; the
  file is unchanged since before this programme.
- core `adaptive_feed_modulation_pipeline_f036b::modulation_raises_cutting_chipload_toward_band`
  (pre-existing since `7a5fdad4`; power-calcs area).

## Follow-ups (recorded, not scheduled)

- For the power-calcs owner: Q3, Q4, Q11/T-5, S7/T-1, S10, S22, S33, D15,
  L16, five S25 rows, 25 S29 rows, two `stock_to_leave_radial` match arms
  in `feeds/rationale.rs` and `feeds/suggest.rs`.
- Q1 remainder: eight Suggest sites still pass `SuggestContext::default()`;
  the CLI smoke baseline is one of them (re-baseline question).
- W4 residue: the viz runtime-profile reader cluster is dead in production
  but built on a live UI path (deletion-class); two unread re-exports in
  `rs_cam_core::simulation`; 255 rustdoc link warnings predate the
  programme.
- Below the cut line: D14, L14, L15, D10, S31, S32, Q7–Q12.
- The instrument (`scripts/debt_scan.py`) cannot see types bound off a
  return value or exposed by a `pub` signature; add a `private_interfaces`
  pass before the next sweep.
