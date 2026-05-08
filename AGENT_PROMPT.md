# Agent prompt — cutting calc data gaps

You are picking up the cutting-calc + optimizer gap-closure work for rs_cam.
The plan is fully written at `planning/cutting-calcs-data-gaps.md` — **read it
first before doing anything else**. It is self-contained: 14 gaps across 5
categories (Code / Routing / Data / Model / Cross-cutting), each with a
symptom, root cause, fix shape, validation gate, and status field.

## Working environment

Worktree: `/home/ricky/work/rs_cam_cutting_calcs`, branch
`cutting-calcs-gaps`, branched from `master` at `5776f5b`. **Do all work
there.** Do not commit on master.

The main repo at `/home/ricky/personal_repos/rs_cam` is the GUI's working tree
— the operator may have it open, may be editing dressup configs, may be
running simulations. Stay out of its `target/` cache. The shared object
database is fine.

## What just landed (your starting context)

The previous session merged a 5-commit optimizer redesign into master:

- **A2** (engaged-diameter LUT lookup for tapered tools)
- **B**  (extracted Stage 0 / Stage 1 helpers + steady-state filter)
- **#1** (pre-flight: bipolar refusal + DeflectionSetupLocked)
- **#2** (Stage F re-target — RCTF-compensated chipload solver)
- **#3** (per-candidate `gate_deltas` + new `TradeOff` outcome variant)

End-to-end MCP validation on wanaka confirmed every TP refuses or skips for
the right reason. The redesign **is not the gap-closing work**. It built
the framework that lets the optimizer be honest about what it can't fix; this
work fixes what it should be able to fix but doesn't.

## How to work through the gaps

**One gap at a time. One commit per gap (or tight commit set).** Don't
bundle. The doc is your tracker — update the status field on each gap as you
move it through the lifecycle.

For each gap you pick (priority order in the doc — start with G5+G6+G7 as a
single tightly-scoped routing change):

1. **Re-verify the root cause** before changing anything. The audit was a
   static read; the runtime may differ. Read the named files. Inspect the
   referenced LUT JSON. Run an MCP call against wanaka if the gap claims a
   specific outcome. Do not trust your past self.

2. **Find good references** before designing the fix. The gap descriptions
   are sparse on purpose — they don't claim to be the right answer, only
   the right question. Check:
   - `architecture/` for any design docs covering the affected subsystem
   - `research/` (especially `feeds_and_speeds_integration_plan.md`) for
     machining-side rationale (RCTF math, chip-thickness calibration,
     pass-role semantics)
   - `crates/rs_cam_core/src/feeds/vendor_lut.rs` and the JSON files under
     `crates/rs_cam_core/data/vendor_lut/observations/` for LUT-shape
     ground truth
   - The archived design at `planning/archive/optimizer_redesign_2026-05-08.md`
     — it explicitly documents which primitives the redesign relies on
     (`chipload_midpoint`, `radial_chip_thinning_factor`,
     `lookup_diameter_at`, `RefuseReason::BipolarEngagement`) and where they
     live
   - For the deflection model rewrite (G13), real machining references:
     handbook formulas for cantilever tip deflection
     (`δ = F · L³ / (3·E·I)`), carbide vs HSS modulus values, chipload-derived
     transverse force estimates from peak `mrr × Kc / engaged_width`

3. **Plan the fix**, write the plan into the gap's entry under a `**Plan:**`
   sub-heading. This is for the operator to review before you sink time into
   it. Don't ask permission for trivial fixes; do flag any change that:
   - Touches more than one crate
   - Changes a public API surface
   - Adds a dependency
   - Crosses into G13 (the deflection rewrite is a multi-day model change,
     not a quick patch)

4. **Implement.** Stay inside `crates/rs_cam_core/src/tool_load/` and
   `crates/rs_cam_core/src/feeds/` for almost everything. Op-spec
   accessor changes touch `crates/rs_cam_core/src/compute/operation_configs.rs`
   and `crates/rs_cam_core/src/compute/catalog.rs`. UI/MCP consumer updates
   should be minimal — most gaps don't need them.

5. **Validate against the gap's gate.** This is the part that actually
   matters. The gate is written into the doc deliberately — it names a
   specific MCP call against the wanaka project (or a fixture) and the
   observable outcome change expected. **Run it.** Don't substitute "the
   unit tests pass" for "the validation gate passes." Capture the before
   and after MCP output in your commit message.

6. **Update the doc.** Flip the gap's `Status` field. If you found something
   the audit missed (a sub-gap, a related routing issue, a data row that
   was lurking), append it as a new gap with an `(opened YYYY-MM-DD)`
   marker.

7. **Commit and report back.** Each commit message should:
   - Title with the gap number: `fix(optimizer): G5 widen pass_role lookup for tapered ball`
   - Body explaining what changed
   - **Validation gate** section with the MCP call + outcome diff
   - End with the standard `Co-Authored-By` trailer

   Then stop. Tell the operator what changed, what the gate showed, and
   propose the next gap or pause for direction. The operator wants
   reviewable increments and may want to verify in the live GUI before
   you continue.

## Validation infrastructure (read this section even if you're tempted to skip)

The doc lists what's available; here is how to actually drive it.

### MCP server — primary validation surface

The operator will have the GUI open with the wanaka project loaded. You can
call MCP tools directly. The gates you'll use most:

- `mcp__rs-cam__list_toolpaths` — TP index → operation + tool
- `mcp__rs-cam__optimize_toolpath` — full pipeline. Returns `OptimizeOutcome`
  JSON: `Ranked` / `TradeOff` / `NoSafeImprovement` / `Skipped`. Each
  candidate carries `verdict` (per-criterion `kind`/`peak`) and `gate_deltas`.
- `mcp__rs-cam__get_tool_load_report` — gate verdicts only. Use to confirm
  baseline state before running optimize.
- `mcp__rs-cam__narrate_toolpath` — Z-level structure, peak axial DOC,
  air-cut %. Read this before the raw cut trace.
- `mcp__rs-cam__inspect_spans` — per-span chipload / cycle time. Useful
  for diagnosing why a chipload verdict came out the way it did.

The MCP tools will not be available the moment your context starts. They are
deferred — you load them via `ToolSearch` with
`select:mcp__rs-cam__optimize_toolpath` (etc.). They surface as
`mcp__rs-cam__*` in system reminders.

If MCP isn't reachable when you start (server not running, project not
loaded), say so and ask the operator to bring it up. Don't try to spin up
a GUI yourself — the operator runs it.

### Wanaka regression cases (canonical fixtures)

After each change, every wanaka TP should still resolve to its expected
outcome:

| TP | Op + Tool | Expected | Why |
|---|---|---|---|
| 0 | Pin Drill | `Skipped(SteadyStateSamplesNotPresent)` | Drill cycle (correct) |
| 1 | Back Rough / FlatEnd 6mm | `DeflectionSetupLocked` (until G13) | L/D 7.5 |
| 2 | Holes / Drill | `Skipped` | Drill cycle |
| 3 | Rivers (back) (copy) / FlatEnd | depends on G7 | Project Curve |
| 4 | Rivers (back) / TaperedBall | `NoImprovementFound` (until G5) → Stage F retarget after | Project Curve, has LUT |
| 5 | Lakes (back, inside) / TaperedBall | `NoImprovementFound` (until G5) → Stage F or Ranked after | Scallop, has LUT |
| 6 | 3D Rough 6 / FlatEnd | `DeflectionSetupLocked` | Same tool as TP 1 |
| 7 | 3D Finish 6 / TaperedBall | depends on G2/G6 | DropCutter |

**Don't break what works.** TPs 0 and 2 must stay `Skipped`. TPs 1 and 6 must
stay `DeflectionSetupLocked` until G13 lands. Any regression on those is a
bug in your change.

### Param sweep system — secondary

For Stage 1 knob fixes (G1, G2, G3, G4) you can sweep a single parameter
across an op fixture and confirm the optimizer can move it:

```
cargo test --test param_sweep -- sweep_<op>_<tool>
```

Or a one-off:

```
cargo run -p rs_cam_cli -- sweep <fixture.toml> --param stepover \
  --values "0.5,1.0,2.0,3.0" --output-dir target/param_sweeps/<gap>
```

Output goes to `target/param_sweeps/<gap>/...` with JSON fingerprints,
toolpath SVG, and 6-view stock PNG.

### Verify gate (always before commit)

```
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p rs_cam_core --lib
cargo test -p rs_cam_core --tests
cargo fmt --check
```

`rs_cam_viz` lib has 3 pre-existing failures from codex's perf work
(`controller::tests::ui_harness_records_lane_status_overlay_and_stock_to_leave`,
`controller::workflow_tests::w3_face_z_propagates_to_height_resolution`,
`compute::worker::tests::adaptive3d_semantic_trace_records_runtime_structure`).
**Those stay failing.** Any new failure in rs_cam_viz is yours to investigate.
Run `cargo test -p rs_cam_core` for the trustworthy result.

The workspace clippy gate may have stale failures from perf-agent territory
(`viz/app/mcp.rs`, `viz/app/simulation.rs`). Verify those too are
pre-existing before treating them as your problem.

## Hard rules

- **Do not implement against the audit blindly.** Re-verify each gap's root
  cause before designing the fix. If the audit says "X is missing" and you
  find X is actually present, update the gap doc and pick the next one.
- **Do not skip the validation gate.** A gap is not Done until its specific
  MCP call (or sweep, or fixture test) shows the expected outcome change.
- **Do not bundle multiple gaps in one commit unless the doc explicitly says
  so** (G5+G6+G7 is the only triplet — they share a single routing change).
- **Do not work on G13 without checking in first.** It's a multi-day model
  rewrite. Don't start it with a vague plan.
- **Do not rename existing `OptimizeOutcome` variants or add new ones.** UI
  and MCP consumers branch on the tag string. New variants need full UI/MCP
  arm updates and operator approval.
- **Do not touch the perf-agent's territory** (rs_cam_viz simulation rendering,
  GPU upload throttling, span aggregate caches). If you discover the
  optimizer needs something that crosses into that territory, stop and ask.
- **Do not skip clippy** — the workspace enforces 16 deny lints. `#[allow]`
  is OK with a `// SAFETY:` comment when the pattern is provably safe;
  never file-level `#[allow]`.
- **Do not write new planning docs.** `cutting-calcs-data-gaps.md` is the
  source of truth — update it, don't fork it. New gaps go inline as new
  entries with `(opened YYYY-MM-DD)` markers.

## Soft rules

- Prefer extending existing helpers (`chipload_midpoint`,
  `radial_chip_thinning_factor`, `is_bipolar_engagement`, `classify_one_gate`)
  over creating new ones.
- For each policy change, write tests that pin the new behaviour in
  `crates/rs_cam_core` *before* exercising it via MCP. The MCP gate is for
  end-to-end confidence; unit tests catch regressions.
- If you discover an `Unmodeled` verdict you can't explain from the LUT
  files alone, log the (tool, op, material) tuple inline in the gap entry
  with a short note. The operator will use that to prioritise data
  backfill (Category C).
- For G13 (deflection model), expect to spend most of the time on
  reference + calibration, not coding. The tip-deflection formula is two
  lines; the threshold values that decide Within/Approximate/Exceeds need
  to be defended against real cuts. Capture the calibration data inline in
  the gap.

## Suggested first move

1. Read `planning/cutting-calcs-data-gaps.md` end-to-end.
2. Pick **G5+G6+G7** (the routing widening triplet).
3. Re-verify the root cause: read
   `crates/rs_cam_core/src/tool_load/chipload.rs::routed_lookup_family`,
   then read `data/vendor_lut/observations/tapered_ball_nose.json` (and
   the FlatEnd file for G7) to confirm the LUT actually has the rows the
   audit named.
4. Write the plan into the gap entries.
5. Implement the routing change.
6. Validate: MCP `optimize_toolpath` index 4 and 5 against wanaka should
   no longer return `Unmodeled(NoVendorData)` on chipload — they should
   produce a Stage F retarget candidate or a typed refusal.
7. Commit, report.

Begin by reading the plan doc and confirming the worktree is set up.
