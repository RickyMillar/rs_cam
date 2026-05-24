# Agent smoke acceptance baseline — 2026-05-24

## Scope

Agent-created toolpaths and sims compared to `seed_goal_matrix.csv`. This is
an exploratory smoke run, not a complete Tier 1 harness.

**Cases attempted:** 13 of 18 (deliberately skipped AS010 inlay, AS012
alignment_pin_drill, AS016 waterline as repetitive once the 2D and 3D
patterns were clear; user requested coverage of non-repetitive cases).

**Run dir:** `target/acceptance_sweeps/agent_smoke_20260524_0909/`
**Results CSV:** same dir, `results.csv` (21 rows: 2 per case for AS001-AS005
suggest/baseline split, 1 per case for AS006-AS018).

## Repository state

- git HEAD: `07a723d`
- working tree: dirty (pre-existing user changes; nothing reset by this run)
- Mid-run: a second agent landed the **MCP param UX overhaul** prompted
  by this run's findings. The new feature flags
  (`operation_schema`, `param_schema_hints`, `param_schema_optional_nulls`,
  `integer_param_coercion`, `mutation_result_envelope`,
  `valid_param_error_hints`) were active for cases AS006 onward and
  delivered immediate value — see "Reference comparison notes" below.

## Input sanity

- toolpath catalog rows: 23
- goal rows: 299
- smoke cases: 18

## Commands/tools used

MCP tools (rs-cam):
`load_project`, `project_summary`, `inspect_model`, `inspect_stock`,
`inspect_machine`, `list_tools`, `list_toolpaths`, `get_operation_schema`,
`add_toolpath`, `set_toolpath_param`, `set_toolpath_enabled`,
`get_toolpath_params`, `generate_toolpath`, `run_simulation`,
`get_diagnostics`, `get_tool_load_report`, `get_cut_trace`,
`optimize_toolpath`, `remove_toolpath`, `import_model`.

Shell: only `git status`, `python3` for CSV validation, plain `cat >>` for
results-row append.

## Result summary

| Count | Value |
|---|---:|
| cases attempted | 13 |
| cases generated | 11 (AS006 rest precondition not met; AS018 template gap) |
| cases simulated | 11 |
| optimizer runs | 2 (AS011 drill, AS015 scallop) |
| pass | 1 (AS011 drill) |
| warn | 3 (AS004 face, AS008 v_carve, AS009 chamfer) |
| fail | 7 (AS001/2/3/5/7/13/14/15/17 — all deflection_exceeds, most chipload_unmodeled) |
| harness_error / skipped | 2 (AS006 rest, AS018 project_curve) |

The fail bar is `deflection EXCEEDS` and/or `chipload exceeds_low` against
the live tool-load gates. None failed safety in the human sense — the
verdict is "the model fired Exceeds because peak DOC is the full stock
height" (see §"Reference comparison notes").

## Results by case

| Case | Operation | Goal | Suggest defaults | Sim verdict | Optimize | Verdict | Notes |
|---|---|---|---|---|---|---|---|
| AS001 | pocket (2D) | vendor_lut hardwood | feed 770 dpp 4.2 stepover 2.1 | chipload **Unmodeled** / defl **Exceeds 374µm** / 36 rapid collisions / air-cut 82% | skipped (sim fail) | **fail** | first hit of all 4 systemic 2D issues |
| AS002 | adaptive (2D) | vendor_lut softwood | dpp 9 (=1.5×D), feed 911 | chipload **Unmodeled** / defl **Exceeds 408µm** / 70 collisions / air-cut 75% | skipped | **fail** | dpp=1.5D matches `adaptive_doc_factor` |
| AS003 | profile (2D) | community hardwood | dpp 4.8, side outside | chipload **Unmodeled** / defl **Exceeds 406µm** / 1 collision / air-cut 55% | n/a | **fail** | side=outside default; tabs off |
| AS004 | face (2D) | community mdf | dpp 4.2 | chipload **Unmodeled** / defl **Within 160µm** / **0 collisions** / air-cut 58% | n/a | **warn** | first deflection Within; air-cut driven by stock_offset=5 perimeter |
| AS005 | zigzag (2D) | community mdf | same as pocket | chipload Unmodeled / defl Exceeds 354µm / 60 collisions / air-cut 72% | n/a | **fail** | same pattern as AS001 |
| AS006 | rest | community hardwood | n/a — generate blocked | n/a | n/a | **harness_error** | `prev_tool_id=1` accepted via new integer coercion; generate-time error: "Rest machining requires an earlier enabled operation in the same setup using the previous tool on the same model" — **not surfaced in static validation** |
| AS007 | trace | effective dia | tool mismatch (6mm vs csv 3mm) | chipload Unmodeled / defl Exceeds 368µm / 3 collisions / air-cut 67% | n/a | **fail** | template lacks 3mm — used 6mm with note |
| AS008 | v_carve | effective dia | 136k moves at 0.4 stepover | chipload Unmodeled / **defl Within 78 nm** (validated) / 0 own collisions / air-cut 84% | n/a | **warn** | V-bit physics correct; new diagnostic_delta caught feed=3.8× LUT |
| AS009 | chamfer | effective dia | width=1 default | chipload Unmodeled / defl **Within 29µm** / 2 collisions (likely inherited from disabled prior) / **air-cut 0.19%** | n/a | **warn** | clean cut, engagement 0.86 |
| AS011 | drill | drill_native_gates | depth=12 peck=3 feed=250 | drill_gates **ALL WITHIN** (chip_welding 4/8, peck 1/2, plunge 83/50) / 0 collisions | **Skipped** (steady_state_samples_not_present) | **pass** | only clean run; optimizer correctly declined |
| AS013 | adaptive3d | vendor_lut softwood | dpp 3 stepover 1.2 feed 2500 | chipload **Exceeds LOW** (0.016 vs 0.032 vendor floor) / defl Exceeds 573µm / **1041 rapid collisions** / air-cut 90% | n/a | **fail** | **first case where chipload model actually fired** — chip-thinning from low engagement |
| AS014 | drop_cutter | vendor_lut hardwood | stepover 0.3 (4× LUT 0.075) | chipload Exceeds LOW (0.00143 vs 0.0113) / defl Exceeds 401µm / **0 rapid collisions** | n/a | **fail** | ball-nose chip thinning at light engagement |
| AS015 | scallop | finish-quality | scallop_height 0.03 | chipload Exceeds LOW (0.002 vs 0.0125 extrapolated) / defl Exceeds 431µm / 52 collisions | **no_safe_improvement** + `deflection_setup_locked` + "shorten stickout below 12mm" | **fail** | **OPTIMIZER GOLD STANDARD** — declined to coarsen scallop to fake an improvement |
| AS017 | horizontal_finish | finish-quality | stepover 1.2 angle_threshold 10 | chipload **Unmodeled w/ `no_vendor_data`** / defl Exceeds 524µm / 164 collisions | n/a | **fail** | new chipload reason "no_vendor_data" — system correctly knows it has no LUT row |
| AS018 | project_curve | effective dia | n/a | n/a | n/a | **skipped** | template only has the surface STL, no curve to project |

## Top findings

Ranked by impact for product / harness work, with my best guess at root
cause.

### 1. Chipload model is selectively blind on 2D ops — **product bug**
7 of 7 attempted 2D cases (pocket, adaptive, profile, face, zigzag, trace,
v_carve, chamfer) returned `chipload: Unmodeled` with reason
`steady_state_samples_not_present`, even on toolpaths with 5,998 (adaptive)
or 136,210 (v_carve) sim moves and clearly continuous engagement. Meanwhile
on 3D ops (AS013 adaptive3d, AS014 drop_cutter, AS015 scallop) the same
gate fires correctly with `exceeds_low` and a real engagement-corrected
`observed_mm_per_tooth`. The "no steady state" detector is misfiring on
2D — likely the steady-state classifier is keyed on something 2D paths
don't satisfy. **High-value fix**: an agent (or user) gets zero useful
chipload signal on the most common op family.

### 2. Deflection gate fires on stock-above-cutter, not commanded DOC — **product / model issue**
9 of 13 attempted cases reported `peak_axial_doc_mm = 12.0` (or 10.5 on
the 10mm plate, 30.0 on the 30mm terrain), even when commanded
`depth_per_pass=2`. The deflection model multiplies that by an isotropic
Kc and concludes ~400–570µm tip deflection, blowing past the 200µm
gate. Memory note confirms this is the known "stock-above-cutter"
measurement issue in `peak_axial_doc_mm`. **Recommendation**: either gate
the deflection model on the *commanded* DOC at sample-time, or split the
per-sample peak into "engaged DOC" vs "stock height above cutter" so
deflection consumes the right one. Today the gate is effectively
unusable as a binary pass/fail for any op cutting a deep pocket.

### 3. Rapid collisions everywhere — **path-planning issue, separate from above**
Per-op rapid_collision_count: AS001 36, AS002 70, AS003 1, AS005 60,
AS007 3, AS013 **1041**, AS014 0, AS015 52, AS017 164. These are
collisions where the *rapid* moves intersect uncleared stock. AS013's
1041 is severe; AS017 (horizontal_finish on stepped block) is 164. The
collision detector clearly works (drill, drop_cutter, face all show 0).
The path generator is producing rapids that go through stock. Likely
ties to known lift/bridge issues in `planning/AGENTSEARCH_INVESTIGATION_LOG.md`.

### 4. MCP overhaul payoff is real — **caught issues at set-time**
After the mid-run overhaul landed, `set_toolpath_param` started returning
`diagnostic_delta` entries that caught:

- `feeds.feed_vs_lut.high` (AS008 V-carve feed 3.8× recommendation;
  AS013 adaptive3d feed 2.7× recommendation)
- `feeds.stepover_vs_lut` (AS014 drop_cutter stepover 4× recommendation;
  AS017 horizontal_finish stepover 6.7×)
- `geom.plunge_exceeds_feed` (AS006 rest defaults plunge 527 > feed 385)
- `efficiency.very_fine_stepover` (AS008 v_carve, AS010 inlay defaults,
  AS014 dropcutter, AS015 scallop)

These showed up *immediately on the mutation* with no extra MCP call.
The `gui_banners` field also surfaced — AS006 added a "Plunge rate ...
unusual" banner.

The new `get_operation_schema` tool used 7 times (rest, trace, chamfer,
inlay, drill, adaptive3d, drop_cutter, scallop, horizontal_finish,
project_curve) — every time it returned the full param list including
Optional/null fields like `prev_tool_id`, `surface_model_id`,
`spindle_rpm`. Before the overhaul I was guessing param names from
defaults and hitting "unknown parameter" errors with no recovery hint.
After the overhaul, agent ergonomics on params dropped from "blocking
friction" to "non-issue".

### 5. Rest precondition not in static validation — **product gap**
`set_toolpath_param(prev_tool_id=1)` succeeded silently. Only
`generate_toolpath` returned "Rest machining requires an earlier enabled
operation in the same setup using the previous tool on the same model".
The mutation envelope returned `warnings: []` and `diagnostic_delta: []`
on the prev_tool_id set. **Same fix shape as the existing
`geom.plunge_exceeds_feed` rule** — register the precondition in
static_validation so the agent (and the GUI user) sees it on the param
set, not on generate. The user noted drill has a similar pattern.

### 6. Drill chip_welding threshold appears not to read material — **possible product issue**
AS011 stock is hardwood; chip_welding observed=4 D/d vs threshold=**8.0**.
The 8 D/d threshold is the *softwood* value per CLAUDE.md (hardwood/MDF
should be 5.0). The gate happened to pass either way (4 < 5 < 8) but
the threshold lookup looks wrong. Worth a one-liner check at
`tool_load/verdict.rs` to see whether material is being threaded into
the drill-gates lookup.

### 7. Test_data templates don't match the smoke CSV — **harness issue**
- `ux_2d_star.toml` has no 3mm tool (needed by AS007 trace, AS009 chamfer)
- `ux_2d_pocket.toml` material is hardwood (CSV says softwood/mdf for
  AS002/AS004/AS005); we got the material wrong in 3/13 cases
- `ux_3d_terrain.toml` has a broken `Rivers (back) (copy)` toolpath that
  fires "Selected model is missing" on every load/sim — this should be
  fixed in the template
- `ux_3d_terrain.toml` has no source curve for `project_curve` (AS018)

Fix: regenerate the templates from the CSV's `tool_name` /
`material_family` / required-models columns rather than hand-curating.

### 8. Optimizer gold-standard on AS015 scallop — **product working as designed**
The case the Wanaka review flagged as the most likely "BS" optimizer
output (finish op where coarsening stepover/scallop fakes a cycle-time
improvement) returned:

> `kind: no_safe_improvement`
> `reason: deflection_setup_locked`
> `explanation: "predicted tip deflection 431 µm at peak load (above 200 µm limit) — feed/RPM/DOC/stepover alone can't bring this under threshold for this setup; shorten stickout below ~12 mm or use a stiffer tool/material"`

This is the highest-quality optimizer output observed. It correctly
declined to manipulate cycle time, named the failure mode, and pointed
the operator at the physical lever. **Treat this as the optimizer
acceptance bar** — any future optimizer regression that goes back to
recommending coarser stepover here is a fail.

## Reference comparison notes

### Nominal vs corrected chipload

We were unable to compute nominal `fz = feed / (rpm × flutes)` directly
because `get_toolpath_params` does not expose RPM (the schema shows
`spindle_rpm: option<u32>` but in practice it was `null` for every case;
RPM is being auto-selected somewhere we couldn't query). The sim-side
corrected chipload (`per_sample_peak_chipload_mm_per_tooth`) was visible
on every case and produced sensible numbers (0.0014–0.0833 mm/t).

The clean compare we *could* make:

| Case | Goal band (vendor LUT) | Observed corrected chipload | Verdict |
|---|---|---|---|
| AS013 adaptive3d | 0.032–0.055 mm/t | 0.0163 mm/t (median_low) | exceeds_low ✓ correct |
| AS014 drop_cutter | 0.0113–0.0227 mm/t | 0.00143 mm/t | exceeds_low ✓ correct |
| AS015 scallop | 0.0125–0.02 mm/t (extrapolated) | 0.002 mm/t | exceeds_low ✓ correct |

All three 3D cases agreed with vendor-LUT expectations. The seven 2D
cases where the model returned `Unmodeled` are uncovered by this run
— follow up requires fixing the steady-state classifier.

### Finish quality (scallop) check
AS015 baseline scallop_height=0.03 mm is squarely in the
`good_light_sanding` band (rough rule of thumb 0.01–0.05). The optimizer
did not recommend a coarser scallop. Direct quality check passes.

### Drill gates
AS011 baseline: depth 12 mm on a 3 mm drill = D/d = 4. Goal row's
hardwood threshold is 5; we should be "approaching" not "low" or
"exceeds". The verdict came back `chip_welding: within (4 vs 8)`. The
**value** is right, but the **threshold** is the softwood number — see
finding #6.

### V-bit / effective-diameter
AS008 v_carve: deflection 78 nm WITHIN (validated confidence) is
physically reasonable — V-bit max engaged diameter is small at any depth
short of the tool's full cone. Chipload Unmodeled here is the right
answer for an effective-diameter cut where the system has no LUT row.

## Recommended next steps

Priority order:

1. **Investigate chipload's `steady_state_samples_not_present` false-negative
   on 2D**. Single highest-value product fix surfaced by this run; without
   it, the chipload gate is non-functional on 7 of 22 op kinds.
2. **Resolve the `peak_axial_doc_mm` ambiguity** (stock-above-cutter vs
   commanded DOC) at the deflection-gate consumer side. Without this,
   the deflection gate cannot be trusted as a binary pass/fail for any
   pocket op deeper than a single pass.
3. **Add static-validation rules for op preconditions** (rest needs a
   prior-tool toolpath; drill has a similar gap per user note;
   project_curve needs a curve model). Same shape as the existing
   `geom.plunge_exceeds_feed` rule.
4. **Fix drill `chip_welding` threshold lookup** to honour material —
   confirm against `tool_load/verdict.rs`. One-liner if confirmed.
5. **Regenerate `test_data/ux_*.toml` templates from the smoke CSV** so
   tools / materials / model contents match what each case expects.
   Currently 3 of 13 cases had material mismatches and several had tool
   mismatches.
6. **Investigate why rapid collisions are so common** — 1041 on AS013
   adaptive3d alone. Probably ties to the lift/bridge issues already
   tracked in `planning/AGENTSEARCH_INVESTIGATION_LOG.md` but worth a
   focused look.
7. **Bake AS015's `deflection_setup_locked` narrative as an acceptance
   test** — pin this as the optimizer gold-standard so any future
   refactor that regresses it fails CI.
8. **Build Tier 1 acceptance harness** per `SUGGEST_SIM_OPTIMIZE_SWEEP_PLAN.md`
   §"New acceptance runner" using the lessons learned here (and the now-
   working `get_operation_schema`).

## Top blockers for Tier 1 (updated from Tier 0 baseline)

The three Tier 1 blockers from `2026-05-24_tier0_baseline.md` still
stand:

1. No goal-matrix consumption path.
2. No optimizer invocation in the sweep.
3. No sim-verdict labelling.

This run partially answered (2): we now have two real optimizer outputs
to compare against (`Skipped` for drill, `no_safe_improvement` with
`deflection_setup_locked` for scallop). The Tier 1 harness can use these
as labeled assertion targets right away.

A new Tier 1 blocker also surfaced:

4. **Most "exceeds" verdicts are dominated by the `peak_axial_doc`
   ambiguity** (finding #2). Until that's resolved, any sweep that asserts
   "in-band config → Within verdict" will fail on >50% of pocket / 3D
   cases for reasons unrelated to the actual cutting load.
