# V1, V2, V3 — the limit rows read their own bound, and Readiness shows them

Written 2026-09-18. Steps V1, V2 and V3 of `SURFACE_IMPL.md` §2, on the two
operator rulings of 2026-09-18 (`RESUME_PLAN.md` §7).

The core half is unchanged apart from one doc comment. **V1–V3 add no
number.** Every bound on the screen is `CriterionStatus::bound`, which is the
value the gate itself judged against (S4).

---

## 1. The red run

Both sentries were written against the shipped surface and run before any
source change.

`the_limit_rows_read_their_own_bound_g_ownbound`:

```
test result: FAILED. 1 passed; 6 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.29s
```

The one pass was the non-vacuity arm, which is what makes the six failures
mean something: the fixture DID model three gates with positive peaks. The
badge strip it painted was:

```
"Tool load:"
"advance/tooth BURN"
"power 7%\u{2248}"
"L/D 0%"
```

Three rows for five criteria, an approximation mark on a row inside its
bound, and a deflection row labelled with a ratio.

`readiness_shows_the_limits_g_readylimits`:

```
test result: FAILED. 1 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.65s
```

with the failure message

```
the Readiness page painted the retired "within" pill ("within 1/1") on the
InsideTheRigidityCap fixture. It counts toolpaths, which is not the question
this page asks.
```

Two arms were written after the repair and so never ran against the old
renderer:

- `every_modelled_row_names_its_setting_and_its_bound_g_ownbound` was
  tightened, from "the setting appears somewhere in the painted text" to "the
  caption under the row opens with the setting". Its LOOSER form was one of
  the six red failures above, and the tightened form is strictly stronger.
- `every_row_states_the_population_it_measured_g_ownbound` is new. The red
  strip quoted above carries no population on any row, so it fails there by
  construction.

---

## 2. What the deflection row read, before and after

One fixture, one Ø6 two-flute end mill at 45 mm stickout, pocketing generic
hardwood. The deflection gate measured a peak tip deflection of **0.0050 mm**.

| | Denominator | Painted |
|---|---|---|
| Before | `DEFLECTION_SAFE_LD_RATIO = 4.0`, an L over D RATIO | `L/D 0%` |
| After | `CriterionStatus::bound = 0.2000 mm`, which the gate set from `deflection::EXCEEDS_BOUND_MM` | `deflection 2%` |

The percent CHANGES on screen, and that is the fix. The two denominators
differ by a factor of 20 and share no unit. Defect class 2
(`RESUME_PLAN.md` §9): a quantity divided by a fraction of a DIFFERENT
quantity.

The other two caps behaved differently, and both are worth recording:

- **The power cap agreed.** The GUI built `max_power_kw × safety_factor` =
  0.8 × 0.75 = 0.6000 kW, and the gate's own bound is 0.6000 kW. The row still
  reads 7 %. The GUI was recomputing a value core already published, which is
  a duplicate rather than a defect — until a machine model publishes a
  non-constant power curve, when the GUI's `match` on `PowerModel` and the
  gate's `power_at_rpm(rpm)` would part company.
- **The chipload cap agreed on this fixture.** The GUI read the end of
  `SimulationState::cached_chipload_envelopes`, which matched the band
  ceiling of 0.0550 mm/tooth, so the row still reads 94 %. That cache is
  still read by `sim_timeline.rs`; only `sim_diagnostics.rs` stopped
  reading it.

The whole strip, before and after, on the same fixture:

```
before                        after
"Tool load:"                  "Tool load"
"advance/tooth BURN"          "chipload 94%"
                              "vendor row · 5954 of 8262 samples"
"power 7%≈"                   "power 7%"
                              "machine · 5954 of 27507 samples"
"L/D 0%"                      "deflection 2%"
                              "tool · 5954 of 27507 samples"
                              "depth of cut 67%"
                              "machine · 7660 of 27507 samples"
                              "gantry push —"
                              "no machine-side thrust rating is published; register T-10"
```

(The BURN in the "before" column is the fixture at its default feed, which
sits under the vendor floor. §6 explains why the fixture now feeds inside the
band.)

The hover of the depth row, which is the one a reader should check first
because its bound is the weakest:

```
Within bounds (peak 0.8000 / limit 1.2000 (67%)) — approximate: peak engaged
depth 0.800 mm against the machine rigidity factor 0.20 times the tool
diameter 6.00 mm
limit 1.2000 mm, from the machine rigidity factor 0.20 times the tool
diameter 6.00 mm, which is 1.20 mm; a rule of thumb with no published source.
The machine sets it.
```

Every figure in it is formatted from the value it describes, through
`CriterionStatus::bound_clause` and `BoundSource::clause`. No number in that
string is typed in the GUI.

---

## 3. What shipped

### 3.1 One renderer, two surfaces

`ui/sim_diagnostics.rs`:

```rust
pub(crate) struct LimitRow<'a> {
    pub status: CriterionStatus<'a>,
    pub burn_risk: bool,
    pub source: Option<String>,
}
pub(crate) fn limit_rows(verdict: &ToolpathLoadVerdict) -> Vec<LimitRow<'_>>;
pub(crate) fn draw_limit_rows(ui: &mut egui::Ui, rows: &[LimitRow<'_>]);
```

`limit_rows` iterates `ToolpathLoadVerdict::criteria()` instead of naming
three gates, so the depth-of-cut row (S3) and the gantry-push row (S1) reach
the screen, and a sixth gate needs no edit here.

`draw_limit_rows` paints, per row, one face and one caption:

- **face** — `<CriterionKind::label()> <percent | — | ∅ | BURN | OK | FAIL>`.
- **caption** — `BoundSource::setting()`, then the population as
  `{contributing} of {offered} {PopulationUnit::plural()}`, then the toolpath
  name when the surface folds several toolpaths into one column.
- **hover** — the existing verdict sentence, then `bound_clause()`, then
  "The {setting} sets it."

The Simulation Inspector calls it with one verdict's rows. The Readiness page
calls it with the fold in §3.3. Neither gets a second renderer, so the 240-pt
rail and the 560-pt column cannot disagree.

### 3.2 The label is core's word

The face label is `CriterionKind::label()`. `SURVEY_UI.md` §4 counted the same
three verdicts rendering in six vocabularies — `advance/tooth` / `advance/t` /
`Chip`, `L/D` / `defl` / `deflection` — and noted that a shared scale has to
pick one. Picking core's means the GUI, the CLI and the MCP name one row one
way, and it is what removes `L/D` from the screen.

`criterion_short_label` in `sim_op_list.rs` keeps its own abbreviations: that
is a triage chip on a toolpath row, not a limit reading, and it has no room
for `depth of cut`.

### 3.3 Readiness: the worst row per LIMIT, not a selected toolpath

`readiness_panel.rs` gained `worst_limit_rows(state, &report)`.

**Why worst-per-kind and not a selected toolpath.** `draw_readiness_layout`
builds ONE centred column with no rail and no viewport, so the page shows no
toolpath selection and offers no way to change one. A reading scoped to
`state.selection` would answer a question the page does not ask, and would
change under the operator with nothing on screen saying why. The page asks
"is this safe to cut?" about the JOB, so the fold is per limit across every
toolpath, and each row names the toolpath its reading came from as soon as the
project holds more than one.

Worst is: a measured row beats a vacuous one, a vacuous one beats an
unmodelled one, and between two measured rows the one closer to its own bound
wins. The vacuous tier is deliberate — X-VAC keeps a verdict that rests on
nothing out of the top slot.

### 3.4 What was deleted

| Symbol | File | Why |
|---|---|---|
| `DEFLECTION_SAFE_LD_RATIO` | `sim_diagnostics.rs` | a GUI-owned limit, and the wrong quantity |
| `chipload_cap`, `power_cap_kw`, `deflection_cap` | `sim_diagnostics.rs` | the row reads `status.bound` |
| the `cap` parameter of `verdict_badge` and `verdict_tooltip` | `sim_diagnostics.rs` | same |
| `pct_of_cap` | `sim_diagnostics.rs` | replaced by `pct_of_bound`, which takes the row's own bound |
| `drill_gate_badge` and the `Drill gates:` strip | `sim_diagnostics.rs` | a drill's three gates are `CriterionStatus` rows in `criteria()`; the strip was a fourth rendering of them |
| the `session` and `gui` parameters of `draw_toolpath_section` | `sim_diagnostics.rs` | only the caps used them |
| the `"Tool load"` `check_row` and its three `CountPill::verdict` calls | `readiness_panel.rs` | V3 |
| the `summary` binding and the `CountPill` import | `readiness_panel.rs` | the pills were their only reader in this file |

`load_status` still folds into the Readiness headline verdict and into
`first_unmet_action`, so the tier the pills carried is not lost.

### 3.5 The one core change

`tool_load/verdict.rs`, `Confidence`'s doc comment. **Doc only — 22 lines
added, 2 removed, every changed line is a `///`.** It records the ruling of
2026-09-18 and its reason: the consequence of a weak input is carried by
BEHAVIOUR now (`BoundSource::gates_export`, read through
`CriterionStatus::refuses_export`), so the face does not have to carry it, and
the hover states which input is approximate. The old wording told a UI it
*must render `Approximate` differently*, which is what `verdict_badge`
obeyed with `WARNING_MILD` and a `≈`.

---

## 4. Rows that do not apply

`limit_rows` drops a row whose reason is `NotApplicableForOp`. A gate that has
no meaning for the operation is a different statement from a limit that was
not measured, and the partition already exists in core (`all_not_applicable`,
`is_drill_not_applicable`).

The consequence is the one `SURFACE_IMPL.md` asked for: five rows for a
milling toolpath, the three drill rows for a drill, and no page of dashes in
either direction.

When EVERY row is dropped — reachable, because the optimizer path leaves
`drill_gates` as `None` on a drill op — the block states the absence rather
than painting nothing. A blank there reads as a clean cut, which is defect
class 3. A unit test in `sim_diagnostics.rs` pins that case.

**Not measured:** the drill arm has no rendered sentry. The fixture is a
pocket, and a drill fixture needs `DrillOp`, `emit_drill_samples` and
`drill_gates::evaluate` in a viz integration test. The reasoning above is
sound — core's `criteria()` already publishes the three drill rows, and
`a_criterion_carries_its_own_bound_g_s4bound` pins them — but it is reasoning,
not a measurement. It belongs in the V5 LOOK.

---

## 5. What the plan did not anticipate

### 5.1 The statistic has no door on `CriterionStatus`

`SURFACE_IMPL.md` V2 asks the row to print "the statistic through
`ObservedStatistic::label` (S2 of the review)". **It cannot, without a core
change.** Measured 2026-09-18:

- `ObservedStatistic` (`feeds/feed_explanation.rs`) carries `Median` and
  `Peak` with operator-facing labels, and it is produced only inside
  `chipload::evaluate_with_explanation`, onto `FeedExplanation`.
- `ChiploadStatistic` (`tool_load/verdict.rs`) sits on `SampleEvidence`, and
  `CriterionStatus` carries `sample_range`, not the evidence.
- So a renderer holding a `CriterionStatus` has no statistic at all, for any
  kind, and the four non-chipload gates never produce one.

Deriving it in the GUI from `status.kind` would put the choice of order
statistic in the renderer, which is the shape S4 just removed for the bound.
**The row therefore does not print its statistic.** The follow-up is one field
on `CriterionStatus` — `statistic: Option<ObservedStatistic>` — filled by each
gate from the arm it took, which is the same shape S4 used for `bound`. It is
a core step, so it sits outside V1–V3's file set.

### 5.2 The population needed no new wording, but it needed a format

`REVIEW_DESIGN.md` W4 says "every row prints its population. Reuse
`vacuity_clause`; do not word a second one." `vacuity_clause` is empty when
the population is NOT vacuous, so it cannot state a healthy population. The
caption formats `contributing`, `offered` and `PopulationUnit::plural()` —
core's own fields and core's own word for the unit — and the vacuous case
still takes `vacuity_clause` through the `∅` branch, unchanged. No second
vacuity wording exists.

### 5.3 The Readiness rows lost their tier glyph

The other Readiness rows are `check_row`s: a `✓`/`⚠` glyph, a bold label and a
detail. The limit rows are not, so the tool-load block now has a plain
`Tool load` label and coloured rows under it. The colour per row is a finer
signal than one glyph for five limits, and the headline banner still carries
the fold. It is a visible change of shape on that page and the operator
should see it in V5.

### 5.4 One `≈` is still on an operator-facing surface

`toolpath_status_flags` (`sim_op_list.rs:1129`) pushes a `≈ advance/t` triage
chip for a `Within` + `Approximate` criterion. It is a per-toolpath triage
flag on the Simulation op list, not a limit reading, and the brief's decision
list did not name it. It was left alone rather than deleted, because deleting
it removes a signal no other surface replaces. **A ruling is owed:** either it
goes the way the badge's mark went, or the ruling is explicitly about limit
rows only. `criterion_detail`, which is that chip's hover, already keeps the
confidence reason and needed no change.

---

## 6. The fixture, and the window it sits in

`tests/limits_fixture/mod.rs` builds ONE state used by both sentries: the
shipped `test_data/ux_2d_pocket.toml`, plus one pocket in its Ø6 end mill,
`generate_toolpath` then `run_simulation` with metrics, and the resulting
trace installed in `SimulationState::results`.

**Why the shipped path and not a hand-built report.** A hand-built
`ToolLoadReport` would let the test agree with itself — the fixture would
carry the bound the assertion expects. A hand-built TRACE is also impossible:
`gcode::sim_trace_is_fresh` compares `SimulationProvenance::toolpath_hashes`
against `hash_toolpath` of each toolpath's cached result, and `hash_toolpath`
is `pub(crate)`. Any trace assembled in `rs_cam_viz` reads STALE, and every
row would paint `—` for a reason that has nothing to do with the code under
test.

**The feed window.** The chipload gate judges two statistics against one band:
the MEDIAN against the floor, the per-sample PEAK against the ceiling. On this
fixture the band is 0.0320 to 0.0550 mm/tooth and the simulated peak runs
1.51× the commanded advance, so the feed is bounded on two sides at
**1152 mm/min to 1311 mm/min**. The fixture takes **1225**, the geometric
centre: the advance may rise by 1.07× before the peak passes the ceiling and
fall by 1.06× before the median drops under the floor. `PocketConfig`'s
default 1000 mm/min is 0.0278 mm/tooth — below the floor — and trips the gate,
which would refuse the export the depth arm checks is not refused.

Re-derive those two factors against any load-model change.

**The depth arms.** Both depth-per-pass figures come from
`RigidityProfile::depth_cap_mm`, the one producer the gate and the Suggest
clamp both read: two thirds of the cap for the measured arm, twice it for the
exceeding arm. No arm carries a typed cap.

---

## 7. Verification

Every command through `scripts/cargo_lane.sh`, `rs_cam_viz` with `-j 2`.

| Command | Result |
|---|---|
| `fmt --all -- --check` | clean for every file in this change; two files outside it still differ (§8) |
| `clippy -p rs_cam_viz -j 2 --all-targets -- -D warnings` | `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 25.17s` |
| `check -p rs_cam_core --all-targets` | clean |
| `test -p rs_cam_viz --lib -j 2 -q` | `test result: ok. 409 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.78s` |

The two new sentries:

```
the_limit_rows_read_their_own_bound_g_ownbound
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.77s

readiness_shows_the_limits_g_readylimits
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.10s
```

Every sentry in `SURVEY_UI.md` §4's table:

```
the_inspector_nests_once_dc5
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

the_simulation_page_is_summary_first_dc6
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

inspector_width_is_tab_independent_up4
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.50s

the_chipload_verdict_is_one_row_g_chipverdict
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.13s

the_corridor_bounds_the_band_g_corridor
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.28s

the_feeds_window_fits_the_screen_g_feedsfit
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.36s
```

Every existing viz sentry that names `readiness`, `sim_diagnostics`,
`verdict_badge`, `draw_tool_load_badges` or `Tool load`:

```
chrome_reads_the_kit_up3
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

cycle_time_basis_g_timeest
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

freshness_surfaces_g_freshrender
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

inspector_header_wraps_g_reachwrap
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

load_requests_only_25d_regen_g_loadregen
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

optimize_run_is_non_modal_wp24
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

overlays_registry
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

panels_read_the_token_module_up1
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s

the_feeds_modal_holds_one_scope_dc5a
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

the_workspace_bar_is_a_strip_dc3
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

workspace_menu_complete_g_wsmenu
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

ui_string_hygiene
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
```

**No sentry moved.** No existing assertion was edited, widened or deleted.

---

## 8. Two failures that are not this change, and one file left alone

The working tree is shared. Two sentries fail, and neither reads a file this
change touches.

```
apply_contract_a3
test result: FAILED. 15 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
  pocket_fixture_recipe_fingerprint_is_unmoved
  assertion `left == right` failed: plunge
    left: 793.0
   right: 794.0
```

It measures `feeds::suggest::apply_cut_geometry_to_op`. No file under
`crates/rs_cam_core/src/feeds/`, `machine/` or `material/` is modified in the
working tree, so this is a committed regression at HEAD, not a working-tree
one. This change is a renderer plus one doc comment and cannot move a plunge
rate.

```
component_contracts_up2
test result: FAILED. 20 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.18s
  ui/feeds/compare.rs is allowed 3 hand-rolled chains and holds 4
```

`ui/feeds/compare.rs` belongs to the V4 power-bar session and is modified in
the tree.

**`crates/rs_cam_viz/src/ui/CLAUDE.md` was left unedited.** It was modified by
another session while this work ran, and the standing rule is to stop on a
file another session holds. It names four sentries and neither of the two new
ones would displace them, so nothing is lost by deferring. The orchestrator
may add, within the 40-line cap:

```
- `cargo test -p rs_cam_viz -q --test the_limit_rows_read_their_own_bound_g_ownbound`
```

---

## 9. Files

- `crates/rs_cam_viz/src/ui/sim_diagnostics.rs`
- `crates/rs_cam_viz/src/ui/readiness_panel.rs`
- `crates/rs_cam_core/src/tool_load/verdict.rs` (doc comment only)
- `crates/rs_cam_viz/tests/limits_fixture/mod.rs` (new)
- `crates/rs_cam_viz/tests/the_limit_rows_read_their_own_bound_g_ownbound.rs` (new)
- `crates/rs_cam_viz/tests/readiness_shows_the_limits_g_readylimits.rs` (new)
- `planning/load_model_2026-09-16/V123_IMPLEMENTATION.md` (this file)
