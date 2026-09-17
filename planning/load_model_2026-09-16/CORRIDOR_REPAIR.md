# Corridor sentry repair — the fixture re-derived after T-17

Written 2026-09-18. Scope: one file,
`crates/rs_cam_viz/tests/the_corridor_bounds_the_band_g_corridor.rs`.
Nothing is committed.

## 1. The confirmed refusal

A temporary probe rebuilt `measured()`'s inputs and called
`rs_cam_core::feeds::force::chipload_cap_for_deflection_with_reason`
directly. The function is `pub` and reachable from `rs_cam_viz`, so the
variant is read, not reconstructed.

```
LONG_AND_THIN D=1.500 L=90 | ap=1.0500 ae=0.5250 rpm=18000
  compliance=8.751449e-2 budget_F=2.2853 edge_F=2.7825
  outcome=Err(EdgeForceOverBudget) | ceiling=None chart_top=0.116667
```

The variant is **`DeflectionCapRefusal::EdgeForceOverBudget`**, as the brief
expected. The two force numbers:

| Quantity | Value | How it is formed |
|---|---|---|
| `budget_force_n` | **2.2853 N** | `EXCEEDS_BOUND_MM / compliance` = `0.200 / 8.751449e-2` |
| `edge_force_n` | **2.7825 N** | `axial_doc_mm · f_edge` = `1.0500 · 2.650` |

`edge_force_n >= budget_force_n`, so the guard fires. The edge force is
feed-independent: it stands at a feed of zero. No feed rescues this cut.

Commit `93dd145c` multiplied every fluted section's compliance by 2.441.
Before it, this fixture's compliance was about `3.585e-2` and the budget
force about `5.58 N`, well clear of the same `2.7825 N` edge force. The
fixture had no margin on the refusal side and the change consumed all of it.

## 2. The window

Both arm 1 and arm 4 need a ceiling that EXISTS and is ON chart. Write the
tip compliance at unit load as `C` (mm/N). Both bounds are bounds on `C`:

- `C >= C_refuse = EXCEEDS_BOUND_MM / (ap · F_edge)` — the model refuses.
- `C <= C_chart = EXCEEDS_BOUND_MM / (ap · (chart_top · Ks · sin θ + F_edge))`
  — the ceiling leaves the chart.

`Ks = 24.975 N/mm²`, `F_edge = 2.650 N/mm`, `sin θ_peak = 0.95394`,
`EXCEEDS_BOUND_MM = 0.200` on every row below.

`rise` is `C_refuse / C`: the factor the compliance may rise by before the
model refuses. `fall` is `C / C_chart`: the factor it may fall by before the
ceiling leaves the chart. T-17 was a rise of 2.441.

### 2.1 The grid the brief asked for

Diameter 1.00 to 4.00 mm in 0.25 mm steps, stickout 40 to 120 mm in 10 mm
steps. `ap` and `ae` come from the preview, so they scale with the diameter;
the RPM is 18000 on every row. `REFUSE` is `EdgeForceOverBudget`.

Each cell holds the ceiling in mm/tooth, or `REFUSE`. **Bold** marks a cell
that satisfies both conditions (a ceiling exists and `ceiling <= chart_top`).
`chart_top = 0.116667` mm/tooth on every row.

| Ø mm | L=40 | L=50 | L=60 | L=70 | L=80 | L=90 | L=100 | L=110 | L=120 |
|---|---|---|---|---|---|---|---|---|---|
| 1.00 | REFUSE | REFUSE | REFUSE | REFUSE | REFUSE | REFUSE | REFUSE | REFUSE | REFUSE |
| 1.25 | REFUSE | REFUSE | REFUSE | REFUSE | REFUSE | REFUSE | REFUSE | REFUSE | REFUSE |
| 1.50 | REFUSE | REFUSE | REFUSE | REFUSE | REFUSE | REFUSE | REFUSE | REFUSE | REFUSE |
| 1.75 | **0.04218** | **0.04076** | **0.03868** | **0.03586** | **0.03226** | **0.02790** | **0.02280** | **0.01705** | **0.01077** |
| 2.00 | 0.11782 | **0.11422** | **0.10904** | **0.10219** | **0.09369** | **0.08371** | **0.07252** | **0.06048** | **0.04796** |
| 2.25 | 0.21421 | 0.20613 | 0.19475 | 0.18014 | 0.16271 | 0.14315 | 0.12230 | **0.10104** | **0.08012** |
| 2.50 | 0.33295 | 0.31646 | 0.29394 | 0.26614 | 0.23452 | 0.20089 | 0.16706 | 0.13449 | **0.10423** |
| 2.75 | 0.47493 | 0.44392 | 0.40317 | 0.35525 | 0.30371 | 0.25209 | 0.20318 | 0.15875 | 0.11960 |
| 3.00 | 0.64014 | 0.58581 | 0.51767 | 0.44198 | 0.36547 | 0.29348 | 0.22919 | 0.17383 | 0.12726 |
| 3.25 | 0.82751 | 0.73808 | 0.63197 | 0.52132 | 0.41652 | 0.32380 | 0.24544 | 0.18106 | 0.12900 |
| 3.50 | 1.03478 | 0.89567 | 0.74063 | 0.58953 | 0.45543 | 0.34349 | 0.25340 | 0.18228 | 0.12659 |
| 3.75 | 1.25854 | 1.05297 | 0.83906 | 0.64454 | 0.48241 | 0.35399 | 0.25491 | 0.17925 | 0.12151 |
| 4.00 | 1.49432 | 1.20443 | 0.92399 | 0.68592 | 0.49872 | 0.35716 | 0.25177 | 0.17343 | **0.11484** |

Two bands, not one. Below Ø1.75 mm the model refuses at every stickout in
the grid. Between Ø1.75 and about Ø2.6 mm the ceiling is on chart over a
useful range of stickout. Above that the ceiling is on chart only in the
bottom-right corner, where the refusal bound is close again.

### 2.2 The diameter sweep at 120 mm stickout

This sweep locates the balance point. It is the pair of margins, not the
ceiling, that decides the fixture.

| Ø mm | ceiling | rise (to refusal) | fall (to off chart) |
|---|---|---|---|
| 1.60 | REFUSE | 0.8925 | 2.2956 |
| 1.65 | REFUSE | 0.9601 | 2.1341 |
| 1.70 | 0.00315 | 1.0283 | 1.9925 |
| 1.75 | 0.01077 | 1.0969 | 1.8679 |
| 1.80 | 0.01840 | 1.1654 | 1.7581 |
| 1.85 | 0.02597 | 1.2334 | 1.6611 |
| 1.90 | 0.03344 | 1.3007 | 1.5752 |
| 1.95 | 0.04079 | 1.3667 | 1.4991 |
| **2.00** | **0.04796** | **1.4312** | **1.4316** |
| 2.05 | 0.05493 | 1.4938 | 1.3716 |
| 2.10 | 0.06165 | 1.5543 | 1.3182 |
| 2.15 | 0.06811 | 1.6123 | 1.2707 |
| 2.20 | 0.07427 | 1.6678 | 1.2285 |
| 2.25 | 0.08012 | 1.7204 | 1.1910 |
| 2.30 | 0.08564 | 1.7700 | 1.1576 |
| 2.35 | 0.09082 | 1.8165 | 1.1279 |
| 2.40 | 0.09565 | 1.8599 | 1.1016 |
| 2.45 | 0.10012 | 1.9001 | 1.0783 |
| 2.50 | 0.10423 | 1.9371 | 1.0577 |
| 2.55 | 0.10799 | 1.9708 | 1.0396 |
| 2.60 | 0.11139 | 2.0015 | 1.0237 |

### 2.3 The stickout sweep at Ø2.0 mm

| L mm | C mm/N | ceiling | rise | fall |
|---|---|---|---|---|
| 40 | 2.618e-2 | 0.11782 (off chart) | 2.0593 | 0.9950 |
| 50 | 2.660e-2 | 0.11422 | 2.0269 | 1.0108 |
| 60 | 2.722e-2 | 0.10904 | 1.9803 | 1.0346 |
| 70 | 2.810e-2 | 0.10219 | 1.9187 | 1.0679 |
| 80 | 2.926e-2 | 0.09369 | 1.8423 | 1.1121 |
| 90 | 3.076e-2 | 0.08371 | 1.7526 | 1.1691 |
| 100 | 3.263e-2 | 0.07252 | 1.6520 | 1.2402 |
| 110 | 3.492e-2 | 0.06048 | 1.5438 | 1.3272 |
| **120** | **3.767e-2** | **0.04796** | **1.4312** | **1.4316** |
| 130 | 4.091e-2 | 0.03533 | 1.3177 | 1.5549 |
| 140 | 4.470e-2 | 0.02292 | 1.2060 | 1.6989 |
| 150 | 4.907e-2 | 0.01097 | 1.0986 | 1.8650 |
| 175 | 6.282e-2 | REFUSE | 0.8581 | 2.3876 |
| 200 | 8.113e-2 | REFUSE | 0.6645 | 3.0835 |

`C_refuse = 5.390836e-2` and `C_chart = 2.631109e-2` on every row of this
sweep, because both depend on `ap`, not on the stickout.

### 2.4 The exact window edges

By bisection on the stickout at Ø2.0 mm:

- Below **43.685 mm** the ceiling leaves the chart (`fall < 1`).
- Above **159.705 mm** the model refuses (`rise < 1`).

### 2.5 Ø3.175 mm, checked because it is a real cutter size

| L mm | ceiling | rise | fall |
|---|---|---|---|
| 45 | 0.73291 | 7.5892 | 0.2700 |
| 60 | 0.59803 | 6.3766 | 0.3213 |
| 90 | 0.31587 | 3.8398 | 0.5336 |
| 120 | 0.12899 | 2.1597 | 0.9487 |
| 150 | 0.02819 | 1.2534 | 1.6346 |

Ø3.175 mm reaches the chart only past 120 mm of stickout, and the two
margins never come close to balance. Rejected.

## 3. The chosen fixture

**`LONG_AND_THIN` = Ø2.0 mm, 120 mm stickout.** Two flutes, generic
softwood, as before.

- Ø2.0 mm is a stocked cutter size; 120 mm is a round stickout.
- The ceiling is **0.04796 mm/tooth** against a chart top of **0.11667** —
  41 % of the chart, not at either edge.
- The compliance may **rise by 1.4312x** before the model refuses.
- The compliance may **fall by 1.4316x** before the ceiling goes off chart.

The two margins agree to four significant figures, so the point is the
geometric centre of the window. It is the choice that maximises the smaller
of the two margins. The old fixture's rise margin was **0.8213** — already
past the bound, which is the red.

A 2.441x change like T-17 would still break this fixture. No point inside
the window survives one: the whole window spans `C_refuse / C_chart` =
`5.3908e-2 / 2.6311e-2` = **2.049x**, which is narrower than T-17's step.
The doc block now records the two bounds so the next model change is checked
against them instead of discovered by a panic.

## 4. Every number changed

| Where | Was | Is | Basis |
|---|---|---|---|
| Module doc, STUBBY ceiling | 3.143 mm/tooth | **2.4315** | measured |
| Module doc, STUBBY chart top | 0.1235 | **0.12353** | measured, unchanged in substance |
| Module doc, off-scale factor | twenty-five times | **about twenty times** | 2.4315 / 0.12353 = 19.7 |
| Module doc, reference cut | 9.36 against 0.1176, eighty times off | see §5 — now a cross-reference, not a claim | could not reproduce |
| Module doc, thin fixture | Ø1.5 at 90 mm, 0.0951 against 0.1167 | **Ø2.0 at 120 mm, 0.04796 against 0.11667** | measured |
| Module doc, compliance law | "rises with the cube of stickout" | replaced — see §6 | measured |
| `LONG_AND_THIN` | Ø1.5, 90 mm | **Ø2.0, 120 mm** | §3 |
| `LONG_AND_THIN` doc | "below 1.5 mm the affine inversion refuses" | the full window, both bounds, both margins | §2 |
| `fixture()` comment | "the measured 9.36 mm/tooth ceiling" | **2.4315** | measured |
| Arm 2 doc | eighty times | **twenty times** | measured |
| Arm 2 message | around 9 mm/tooth, two orders of magnitude | **around 2.4 mm/tooth, twenty times** | measured |
| Arm 1 message | "compliance rises with the cube of stickout" | "a thin section", plus a pointer to the window | §6 |
| Arm 4 `.expect` | one line | names the refusal and points at the window | — |

No arm was added, removed or widened. `git diff` shows no line containing
`fn ` or `#[test]`.

## 5. The "9.36 against 0.1176" figure

`rg` finds it in one production file:
`crates/rs_cam_viz/src/ui/feeds/explore.rs:306-307`, in the doc comment on
`Ceiling::OffScale`. It states: *"Measured on the reference fixture (6 mm
two-flute flat, 4.20 DOC, 2.10 WOC, generic softwood) the ceiling is 9.36
mm/tooth against a chart that draws 0.1176 mm/tooth at most."*

That is the sentry's `STUBBY` geometry. `STUBBY` produces exactly 4.20 DOC
and 2.10 WOC. This session measured the same cut and got **2.4315 against
0.12353**. The pair does not reproduce, on either number:

- The ceiling is 3.85x the measured value.
- The chart top 0.1176 corresponds to 18000 RPM; this cut runs at 17000
  RPM, which gives 0.12353. 0.1176 belongs to a different tool.

**It is stale, and I did not change it.** `explore.rs` is a production file
and the brief scopes this session to the sentry. The sentry's module doc now
names the discrepancy and says to re-derive the comment before citing it.
Recommend a follow-up that corrects `explore.rs:306-307`.

## 6. The unexpected finding — the stickout is a weak knob

`ToolDefinition::tip_deflection_mm` models a **stepped** cantilever, not a
plain one:

- a fixed **25 mm** flute section at the cutter diameter, from
  `ToolConfig::new_default`'s `cutting_length: 25.0`;
- a **6.35 mm** shank (`shank_diameter: 6.35`) filling the rest of the
  stickout.

`I` goes with the fourth power of the diameter, so at Ø2 mm the shank is
about 100x stiffer per unit length than the flute section. The flute section
carries nearly all of the compliance, and its length does not change with
the stickout.

Measured at Ø2.0 mm: 40 mm of stickout gives `2.618e-2` mm/N, 120 mm gives
`3.767e-2` mm/N. **3x the length for 1.44x the compliance**, not 27x.

Two doc claims therefore describe a model this code does not use:

- the sentry's module doc, arm 1's message and the `LONG_AND_THIN` doc —
  all fixed in this change;
- `explore.rs:324`, on `Ceiling::OnChart`: *"Compliance rises with the cube
  of stickout, so a long or a thin tool brings the ceiling down onto the
  chart."* **Not fixed** — out of scope, same follow-up as §5.

The practical consequence is that the DIAMETER sets the window, and a
fixture must be tuned on diameter first. That is why §2.2 sweeps diameter.

A second observation: the fixture holds `cutting_length` at the 25 mm
default for every diameter it tries. A 25 mm flute on a Ø1.5 mm cutter is
not a stocked tool. The fixture's behaviour is unchanged in this repair, so
the observation is recorded, not acted on.

## 7. Verification

`scripts/cargo_lane.sh fmt --all -- --check` — exit 0, no output.

```
FMT_EXIT=0
```

`scripts/cargo_lane.sh test -p rs_cam_viz -j 2 --test the_corridor_bounds_the_band_g_corridor`:

```
     Running tests/the_corridor_bounds_the_band_g_corridor.rs (target/debug/deps/the_corridor_bounds_the_band_g_corridor-803c7e2b41b82be2)

running 4 tests
test the_ceiling_decision_is_made_on_the_charts_own_range_g_corridor ... ok
test the_corridor_draws_both_bounds_when_the_ceiling_is_on_chart_g_corridor ... ok
test the_upper_wedge_is_absent_when_the_ceiling_is_off_scale_g_corridor ... ok
test the_rubbing_floor_wedge_is_drawn_g_corridor ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.36s
```

All four arms pass. No wider test set was run. Clippy was not run: the brief
limits this session to the one sentry and the format check.

## 8. Steps not done

None of the work items was skipped. Two items are deliberately left for a
follow-up because they touch a file outside this brief's scope:
`explore.rs:306-307` (§5) and `explore.rs:324` (§6).

## 9. Paths changed

- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_viz/tests/the_corridor_bounds_the_band_g_corridor.rs`
- `/home/ricky/personal_repos/rs_cam/planning/load_model_2026-09-16/CORRIDOR_REPAIR.md` (new)

Nothing is staged. Nothing is committed.
