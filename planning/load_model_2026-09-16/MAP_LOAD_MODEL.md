# The load model, end to end

Read-only map. It records what the code does on 2026-09-17. It proposes no
change.

Cite order in this file: **symbol first, path and line second**. The tree moved
on 2026-09-17, so a line number can drift. A symbol does not.

Scope: the path from an operation's parameters to an emitted G-code file, for
the three load physics (chipload, power, deflection) plus the drill and plunge
gates.

---

## 1. The stage map

### 1.1 The stages in order

```
 A. CALCULATE            feeds::calculate
        |                crates/rs_cam_core/src/feeds/mod.rs:1196
        |  writes rpm, chip_load, feed, plunge, ramp, ap, ae, power_kw,
        |  available_power_kw, chipload_bounds, matched_lut_row, warnings
        v
 B. APPLY + INVARIANTS   suggest::apply::apply_feeds_subset
        |                crates/rs_cam_core/src/feeds/suggest/apply.rs:66
        |  -> suggest::invariants::enforce_invariants
        |     crates/rs_cam_core/src/feeds/suggest/invariants.rs:108
        |  9 passes on a SCRATCH CLONE of the operation. See 1.3.
        v
 C. GENERATE             the operation generator builds the toolpath IR
        |  reads feed, plunge, rpm, ap, ae. Writes move feeds.
        v
 D. SIMULATE             compute::simulate::run_simulation
        |  writes SimulationCutSample: axial_doc_mm, axial_engagement_mm,
        |  arc_engagement_radians, radial_woc_fraction, spindle_rpm,
        |  flute_count, feed_rate_mm_min, effective_chip_thickness_mm.
        |  Writes trace.predicted_feeds when kinematics are on.
        v
 E. MODULATE             session::compute::simulation::apply_adaptive_feed_modulation
        |                crates/rs_cam_core/src/session/compute/simulation.rs:457
        |  -> dressup::feed_modulation::adaptive_feed_modulate
        |     crates/rs_cam_core/src/dressup/feed_modulation.rs:692
        |  REWRITES per-move feeds in the cached toolpath IR.
        |  OVERWRITES trace.predicted_feeds.
        |  Re-integrates cycle time. Refreshes the provenance hashes.
        v
 F. VERDICT              tool_load::evaluate_toolpath
        |                crates/rs_cam_core/src/tool_load/mod.rs:506
        |  chipload::evaluate, power::evaluate, deflection::evaluate,
        |  drill_gates::evaluate. Writes no project state.
        v
 G. EXPORT DECISION      gcode::enforce_load_policy
                         crates/rs_cam_core/src/gcode/mod.rs (after
                         project_load_report, same file)
           Refuses, or emits.
```

The optimizer (`tool_load::optimize`, `crates/rs_cam_core/src/tool_load/optimize/mod.rs`)
is a loop over B, C, D and F on candidate parameter sets. It writes nothing
until the operator accepts a candidate.

### 1.2 What each stage may overwrite

| Stage | May overwrite a value an earlier stage computed | Detail |
|---|---|---|
| A `feeds::calculate` | its own intermediates, many times | `rpm` is written at step 1, overwritten by the vendor row at step 2, by the MaxSpeed speed-up at 2b, by the diameter tier at 2b', by the drill band at 2c, by the power ladder rung 1, and by the drill envelope follow-down at 9c. `ap` and `ae` are written at step 3, overwritten by the user override, the flute guard, the slot cap and ladder rungs 2 and 3. `feed` is written at step 5 and overwritten at steps 6, 7, 9, 9b and 9c. |
| B `enforce_invariants` | **every geometry number stage A produced, and then the feed** | Pass 0 rewrites DPP from the axial envelope. Passes 2, 3, 4, 5 and 6 rewrite the stepover and the DPP. Pass 9 then **re-derives the feed** from the final geometry, discarding stage A's feed. |
| C generate | nothing in the load model | It consumes. |
| D simulate | nothing on the operation | It writes only the trace. |
| E modulate | **the per-move feed in the toolpath IR, and `trace.predicted_feeds`** | This is the only stage that overwrites a measurement. `trace.predicted_feeds` held the kinematics prediction from stage D; the loop at `simulation.rs:754` replaces each modulated entry with the modulator's own answer. |
| F verdict | nothing | Read-only, by contract. |
| G export | nothing | Refuse or emit. |

**The answer to "which stage may overwrite an earlier stage's value" is B and
E.** B overwrites the recommendation. E overwrites the measurement the gate
then grades.

### 1.3 The nine invariant passes, in their fixed order

`enforce_invariants` (`invariants.rs:108`) states that the order is load
bearing. The passes are:

1. Pass 0 — `pick_axial_envelope` (`suggest/axial_envelope.rs:263`). Clamps DPP
   to the cutter axial envelope. When it moves DPP it re-derates
   `chipload_bounds` through `recompute_chipload_bounds_for_dpp`.
2. Pass 1 — `clamp_plunge_to_feed`.
3. Pass 2 — `clamp_stepover_to_diameter`.
4. Pass 3 — `backoff_stepover_for_runtime`. Raises the stepover.
5. Pass 4 — `clamp_dpp_to_rigidity`. Roughing only.
6. Pass 5 — `clamp_dpp_to_cutting_length`.
7. Pass 6 — `backoff_dpp_for_deflection`. Roughing only.
8. Pass 7 — the adaptive3d entry and clearing rewrites, then
   `check_plunge_entry_stability`.
9. Pass 9 — `rescale_feed_to_final_geometry`
   (`suggest/adaptive_entry.rs:338`). Re-derives the feed against the final
   `ap` and `ae`, re-applies the machine feed ceiling, re-applies the rubbing
   floor, then re-runs pass 1.

Pass 8, the arc-fit chipload feed recalibration, was retired on 2026-08-13.
The slot is empty and the code says so.

---

## 2. The two evaluations of the same physics

### 2.1 Power — one function, one coefficient pair, two engagement sources

| | Pre-cut (stage A) | Post-cut (stage F) |
|---|---|---|
| Owner | `feeds::calculate` step 6 and the final `actual_power` | `tool_load::power::evaluate` (`power.rs:261`) |
| Model | `PowerTerms::of` via `power_model_terms` (`feeds/mod.rs:1125`) | `PowerTerms::of` via `predicted_power_kw` (`power.rs:256`) |
| Coefficients | `force::affine_coefficients_for_kc`, then `GRAIN_ANISOTROPY_FACTOR = 2.0` | identical |
| Quantity | `P = shear·feed + edge`, kW | identical |
| Ceiling | `machine.power_at_rpm(rpm) × machine.safety_factor` | `machine.power_at_rpm(sample.spindle_rpm) × machine.safety_factor` |

**Same function. Same coefficients. Same definition.** The differences are the
inputs only:

- **Cross-section.** Pre uses `ToolGeometryHint::mrr_cross_section_mm2(ap, ae)`
  at the commanded `ap` and `ae`. Post uses
  `MillingCutter::mrr_cross_section_mm2(s.axial_doc_mm, radial_width)`, where
  `radial_width = (arc/π) × engagement_radius × 2` from the dexel.
- **Immersion.** Pre derives ψ from `force::immersion_angle(ae, effective_d/2)`.
  Post reads the sample's own `arc_engagement_radians`.
- **Engaged diameter.** Pre uses the chip-thinning `effective_d` of step 5.
  Post uses `2 × tool.engagement_radius(s.axial_doc_mm)`.
- **RPM.** Pre uses the one recommended RPM. Post uses each sample's RPM.
- **Feed.** Pre uses the final commanded feed. Post uses
  `effective_feed_for_sample` (`tool_load/mod.rs:65`), which is the modulated
  or kinematics-predicted feed when the map carries one.
- **Population.** Post drops phantom-transit samples and configured-entry
  samples, and drops samples with `radial_woc_fraction < 0.02`. Pre has no
  population.

**A third and a fourth site hold the same physics.**

- `feed_modulation::max_safe_feed_for_move` (`feed_modulation.rs:398`) builds
  `PowerTerms::of` itself, with `cross_section = axial_mm × radial_width` and
  `radial_width = woc_eff × engagement_diameter_mm`. It uses the same solver.
  **It agrees.**
- `preview_power_kw` in the GUI nomogram
  (`crates/rs_cam_viz/src/ui/feeds/explore.rs:1243`) computes
  `rec.power_kw × explore_feed / rec.feed_rate_mm_min`. That is the **pre-R1
  linear model**. It scales the feed-free edge term with the feed, which the
  two-term model forbids. **It does not agree**, and its own doc comment says
  it is "not the same as the calc".

**The two denominators the GUI divides by differ.** `FeedsResult::available_power_kw`
(`feeds/mod.rs:456`) is the gate axis, `power_at_rpm(rpm) × safety_factor`. The
nomogram divides by `MachineEnvelope::max_power_kw`
(`feeds/explain_payload.rs:31`), which is the RATED power: no RPM curve and no
safety factor. I found no consumer of `available_power_kw` in `rs_cam_viz`.

### 2.2 Chipload — the same expression, a different row and a different derate

The gate's quantity is stated in `tool_load/CLAUDE.md` and in the `chipload.rs`
module header: **advance per tooth**, `effective_feed / (rpm × flutes)`. It is
not the dexel chip thickness.

| | Pre-cut | Post-cut |
|---|---|---|
| Owner | `feeds::calculate` step 2 and step 9b | `chipload::evaluate_inner` (`chipload.rs:442`) |
| Observed value | `feed / (rpm × flutes)` at the final feed | `achieved_feed_per_tooth_mm` -> `display::achieved_advance_per_tooth` |
| Band row | `vendor_lookup::find_best_row_for_geometry` — the **recipe** resolver | `chipload::matched_chip_envelope` -> `find_best_chip_envelope_row` — the **envelope** resolver |
| Derate | `geometry::derate_chipload_bounds(..., RequireBoth)` | `geometry::derate_chipload_bounds(..., AllowHalfBand)` |
| Derate ratio | `commanded ap / engaged_diameter_at_doc(commanded ap)` | `peak steady-state axial_doc_mm / tool.lookup_diameter_at(that DOC)` |
| Statistic | one number | median on the low side, peak on the high side |

The expression for the number is the **same**. Four things differ, and each one
can move the verdict:

1. **The row is resolved by two different functions.** The recipe resolver lets
   an RPM-only row win. The envelope resolver excludes RPM-only rows
   (`vendor_lookup.rs:292`). When an RPM-only row wins the recipe,
   `FeedsResult::chipload_bounds` is `None` while the gate still holds a band.
   `feeds::calculate` acknowledges this and consults the envelope resolver for
   the rubbing floor only, through `floor_band_fallback`. It deliberately does
   **not** re-point the recommendation's target at that row.
2. **The bound policy differs.** `RequireBoth` versus `AllowHalfBand`. A row
   with a max and no min gives Suggest no band and gives the gate a usable
   high-side bound.
3. **The DOC ratio has two definitions.** Pre divides the commanded DPP by the
   **LUT-semantics** engaged diameter of `feeds/mod.rs`. Post divides the
   measured peak DOC by `tool.lookup_diameter_at`. A dexel peak that exceeds
   the commanded DPP pushes the post derate down the `doc_derating_scale`
   ladder that pre never reached.
4. **Drill ops are excluded pre and refused post.** Pre forces
   `chipload_doc_ratio = 0.0` for `OperationFamily::Drill`. Post returns
   `Unmodeled(NotApplicableForOp)`.

**The four chip-thickness quantities on one sample.** `feeds/quantities.rs`
names three of them and types them so they cannot be mixed:

- `CommandedFeedMmMin` -> `AdvancePerToothMm::from_commanded` — commanded
  advance per tooth.
- `AchievedFeedMmMin` -> `AdvancePerToothMm::from_achieved` — achieved advance
  per tooth. **This is the gate's quantity and the heat-map's quantity.**
- `ArcMeanChipThicknessMm` from `sample.effective_chip_thickness_mm` — the
  dexel arc-average chip thickness. `display::arc_mean_chip_thickness` says no
  conversion into `AdvancePerToothMm` exists and none may be added. It keeps
  one visual, the unbanded sim-timeline track.
- `FeedsResult::chip_load_mm` — the vendor **target** the recipe aimed at, in
  the same unit as the first two but not an observation.

A fifth quantity is still live but is not a chip thickness:
`is_bipolar_engagement` (`chipload.rs:265`) reads the raw
`effective_chip_thickness_mm` and compares it against the derated band. That
predicate therefore still compares an arc-mean against an advance band. It
decides the `BipolarEngagement` refusal, not the verdict.

### 2.3 Deflection — one function, three different bounds

| | Pre-cut predictor | Pre-cut envelope | Post-cut gate |
|---|---|---|---|
| Owner | `predict::predict_peak_deflection_um` (`feeds/predict.rs:129`) | `cutter_constraints::invert_deflection` (`cutter_constraints.rs:258`) | `deflection::sample_tip_deflection_mm` -> `evaluate` (`deflection.rs:116`) |
| Force | `force::lateral_cutting_force` | same | same |
| Cantilever | `predict::tip_deflection_from_engagement` (`predict.rs:419`) | same | same |
| Kc scaling | raw `Kc`, no anisotropy | raw `Kc` | raw `Kc` |
| Bound | 200 µm (`DEFLECTION_BACKOFF_TARGET_UM`) | 200 µm rough, **50 µm finish** (`DEFAULT_ROUGH/FINISH_DEFLECTION_LIMIT_UM`) | 50 µm = Within(Validated), 200 µm = `EXCEEDS_BOUND_MM` |

**Same function, same coefficients, same definition of δ.** The differences:

- **Engagement source.** The predictor derives ψ from
  `immersion_angle(radial_woc_mm, D/2)`, where `radial_woc_mm` falls back to
  `machine.rigidity.adaptive_woc_factor × D` for adaptive families and to
  `0.35 × D` otherwise (`PREDICTOR_NON_ADAPTIVE_WOC_FRACTION`). The gate reads
  the sample's own arc.
- **Axial source.** The predictor uses `operation.depth_per_pass()`. The gate
  uses `sample.axial_engagement_mm`.
- **Feed per tooth.** The predictor uses
  `operation.feed_rate() / (rpm × flutes)` — commanded. The gate uses
  `effective_feed_for_sample / (rpm × flutes)` — modulated or predicted.
- **V-bit.** The predictor refuses a V-bit and returns 0 µm. The gate handles a
  V-bit through `lookup_diameter_at`.
- **Refusal shape.** The predictor returns **0.0 µm** for every refusal: drill,
  V-bit, no validated Kc, no DPP, zero chipload. The gate returns
  `Unmodeled`. A zero reads as a safe cut to the back-off loop, so the back-off
  never fires on a refusal.
- **The modulator uses a fourth assembly.** `session::compute` builds
  `DeflectionLimitInputs` with `compliance = tool_def.tip_deflection_mm(1.0,
  max_axial, E)` at the toolpath's peak DOC and `max_tip_deflection_mm =
  EXCEEDS_BOUND_MM`. It then calls
  `force::chipload_cap_for_deflection_with_reason` (`force.rs:225`), which
  inverts the same affine model in closed form. It uses `cos ψ = 1 − 2·woc`,
  which is the same relation as `immersion_angle`. **It agrees.**
- `efficiency::deflection_chipload_ceiling_mm` (`feeds/efficiency.rs`) says it
  assembles the inputs "exactly as `session::compute` assembles them", and it
  calls the same solver.

---

## 3. Every consumer of a load number

"Refuse" means the consumer can stop an action, not only colour a number.

| Consumer | Symbol / path | Reads | Does what | Can refuse |
|---|---|---|---|---|
| Export policy | `gcode::enforce_load_policy`, `crates/rs_cam_core/src/gcode/mod.rs` | `ToolLoadReport::exceeded_criteria`, `any_unmodeled` | blocks the write | **YES** |
| Export freshness | `SimEvidenceMeta::resolve` in the same file | the trace hash | rewrites `SimulationRequired` to `StaleSimulation`, which `any_unmodeled` then blocks | **YES**, indirectly |
| Chipload gate | `chipload::evaluate` | achieved advance per tooth vs the derated band | `Within` / `Exceeds` / `Unmodeled` | **YES** |
| Power gate | `power::evaluate` | peak kW vs `power_at_rpm × safety_factor` | same three arms | **YES** |
| Deflection gate | `deflection::evaluate` | peak δ vs 50 / 200 µm | same three arms | **YES** |
| Drill gates | `drill_gates::evaluate` | chip welding, peck adequacy, plunge feed | joins `criteria()`, so a Critical drill trip blocks export | **YES** |
| Suggest validation | `feeds::validate_tool_for_operation` (`feeds/mod.rs:896`) | tool × operation | returns `FeedsError` instead of a recipe | **YES** |
| Power ladder | `feeds::calculate` step 6 | required kW vs the budget | lowers RPM, then `ap`, then `ae`, then the feed | **YES**, it refuses to move the feed when the edge term alone is over budget |
| Rubbing floor | `feeds::effective_rubbing_floor` (`feeds/mod.rs:974`) | commanded advance per tooth | raises the feed to the floor | it clamps; it does not refuse |
| Feed modulator | `feed_modulation::max_safe_feed_for_move` | band, deflection cap, power cap, machine feed, kinematic reach | writes a per-move feed | **YES**, `ModulationError`, and it skips a toolpath with no band |
| Optimizer pre-flight | `optimize::preflight::preflight_classify` | the baseline verdict | short-circuits the search | **YES**, `DeflectionSetupLocked`, `BipolarEngagement` |
| Optimizer retargeters | `optimize::retarget::{chipload,power,deflection}` | the `Exceeds` arms | proposes axis patches | **YES**, `RetargetOutcome::Refused` |
| Optimizer ranking | `optimize::rank`, `tolerance_bands_from_policy` | the gate verdicts, widened | drops candidates | **YES** |
| Viewport heat map | `display::advance_per_tooth_per_move` (`tool_load/display.rs`), consumed at `crates/rs_cam_viz/src/app/gpu_upload.rs:1125` | achieved advance per tooth, worst per move | colours the moves through `ChiploadBandClass` | no |
| Viewport band source | `tool_load::chipload_envelopes_for_session` (`tool_load/mod.rs:255`), consumed at `gpu_upload.rs:1471` | the derated band | supplies the colour thresholds | no |
| Sim timeline | `crates/rs_cam_viz/src/ui/sim_timeline.rs:550` | achieved advance per tooth; arc-mean chip thickness, **unbanded** | draws two tracks | no |
| Issue triage | `crates/rs_cam_viz/src/state/simulation/issue_triage.rs:82` | the envelope map | classifies sim issues | no |
| Feeds modal, compare card | `crates/rs_cam_viz/src/ui/feeds/compare.rs` | `feeds::cut_efficiency` — `CutEfficiency` | verdict face, ploughing share, force headroom | no |
| Feeds modal, nomogram | `crates/rs_cam_viz/src/ui/feeds/explore.rs` | `preview_power_kw`, `MachineEnvelope::max_power_kw`, `cut_efficiency` | a power percentage bar | no |
| Feeds "why" panel | `crates/rs_cam_viz/src/ui/feeds/why.rs:435` | `FeedsWarning::PowerLimited` and the other warnings | renders the rationale | no |
| Readiness / preflight panels | `crates/rs_cam_viz/src/ui/readiness.rs`, `ui/preflight.rs:568` | the verdicts | pre-export summary | no; the block happens at G |
| MCP `get_tool_load_report` | `crates/rs_cam_viz/src/mcp_server.rs:866` | the whole report | returns JSON | no |
| MCP export | `crates/rs_cam_viz/src/mcp_server.rs:846` | `accept_exceeded_tool_load`, `accept_unmodeled_tool_load` | forwards to `enforce_load_policy` | **YES**, unless overridden |
| MCP apply-feeds | `crates/rs_cam_viz/src/mcp_server.rs:1224` | the Suggest funnel | writes through `apply_feeds_subset` | **YES**, it propagates `validate_tool_for_operation` |
| CLI smoke / verdicts | `crates/rs_cam_cli/src/smoke.rs`, `main.rs:343` | `peak_chipload_mm_per_tooth`, peak DOC, MRR | prints, and compares against a baseline | **YES** for the regression baseline only |
| **CLI job-file export** | `crates/rs_cam_cli/src/main.rs:582` and `:624` | **nothing** | passes `ToolLoadReport { per_toolpath: vec![] }` | **NO.** The gate is present at the call site and has no evidence to act on. |

`ToolpathLoadVerdict::criteria` (`tool_load/verdict.rs:316`) is the single
inclusion point. A gate added there participates in export gating
automatically.

---

## 4. The clamps, and where they sit

Reversibility column: "later stage can undo it" means a later stage can raise
the value back above the cap without re-testing the cap.

| Clamp | Caps | Stage | Reports | A later stage can undo it |
|---|---|---|---|---|
| Drill RPM band (`drill_rpm_envelope_for_diameter`, `feeds/mod.rs:1056`) | RPM, by diameter tier | A step 1, re-applied at 2c | no warning; rolls `spindle_scale` back | no; 2c is the last RPM write except the ladder and 9c |
| Milling RPM tier (`milling_rpm_ceiling_for_diameter`, `feeds/mod.rs:1102`) | RPM under `MaxSpeed` only | A steps 2b and 2b' | no warning; scales `spindle_scale` | yes — the ladder rung 1 and the 9c follow-down move RPM afterwards, downward only |
| Vendor `rpm_max` | RPM under `MaxSpeed` | A step 2b | no | same as above |
| Flute guard, `0.8 × flute_length` | `ap` | A step 4 | `DocExceedsFlute` | yes — the ladder rungs never raise `ap`, but stage B pass 0 may raise nothing and only lowers |
| `MIN_AP_MM` 0.05, `MIN_AE_MM` 0.02, `ae ≤ d` | `ap`, `ae` | A step 4 | no | no |
| Slotting cap, `ae > 0.85·d` -> `ap ≤ 0.25·d` | `ap` | A step 4b | `SlottingDetected` | no |
| Power ladder rung 1 (RPM traverse) | RPM, holding the chipload | A step 6 | `PowerLadderReducedCut` | no |
| Power ladder rungs 2 and 3 (`largest_fitting`, `feeds/mod.rs:1171`) | `ap` then `ae`, floors 0.5 mm | A step 6 | `PowerLadderReducedCut` | **YES.** Stage B pass 9 re-derives the feed against the final geometry and never re-tests the power budget. |
| Power ladder rung 4 (feed) | feed, via `PowerTerms::feed_for_kw` | A step 6 | `PowerLimited` + `PowerLadderReducedCut` | **YES**, same reason |
| Machine cutting-feed ceiling | feed | A step 7 | `FeedRateClamped` | no; stage B pass 9 re-applies it |
| `machine.safety_factor` | feed, plunge, ramp | A step 9 | on `FeedsDerates::safety_factor` | pass 9 does not re-apply it; it re-derives from `calc.feed_rate_mm_min`, which already carries it |
| Ball / tapered-ball plunge cap, `150 × tip_d` | plunge | A step 9 | no warning | yes — a drill op overwrites `plunge_rate = feed` at 9c |
| **Rubbing floor** (`effective_rubbing_floor`, `feeds/mod.rs:974`) | feed **upward**, to `min(0.025, derated band max)` | A step 9b | `ChiploadClampedToFloor` with `band_capped_from` | it is re-applied at stage B pass 9, against `context.chipload_bounds` |
| Drill plunge-feed envelope | feed, two-sided; then RPM follows down | A step 9c | `DrillFeedClampedToEnvelope` | no |
| **Axial envelope** (`cutter_axial_constraints`, `cutter_constraints.rs:181`) | DPP, to `min(deflection, vendor ap, scallop)` | B pass 0 | `AxialDocClampedByEnvelope`, `AxialEnvelopeSafeBandEmpty`, `AxialDocBelowBurnFloor` | no; later DPP passes only lower |
| Plunge ≤ feed | plunge | B pass 1, re-run at the end of pass 9 | `PlungeClampedToFeed` | no |
| Stepover ≤ diameter | stepover | B pass 2 | `StepoverClampedToToolDiameter` | **YES.** Pass 3 raises the stepover, capped at `0.5 × D`, so it cannot pass the diameter. |
| Runtime stepover back-off | stepover **upward**, ceiling `0.5 × D` | B pass 3 | `StepoverRaisedForRuntime` | no |
| Rigidity DPP cap | DPP, `rigidity factor × D`, roughing only | B pass 4 | `RoughingDepthClampedToRigidity` | no |
| Cutting-length DPP cap | DPP | B pass 5 | `DepthClampedToCuttingLength` | no |
| **Deflection back-off** | DPP, `×0.8` up to 5 times, floor 0.5 mm, roughing only | B pass 6 | `DppCappedByDeflection` | see the cycle in 5.1 |
| Pass 9 feed ceiling | the re-derived feed | B pass 9 | `FeedRescaledToFinalGeometry { cap_hit }` | no |
| Pass 9 rubbing floor | the re-derived feed, upward | B pass 9 | `FeedClampedToChiploadFloor` | no |
| Modulator chipload max | per-move feed | E | `BindingConstraint::ChiploadMax` | no |
| Modulator deflection cap | per-move feed, bound `EXCEEDS_BOUND_MM` | E | `BindingConstraint::DeflectionMax` | no |
| Modulator power cap | per-move feed, budget `power_at_rpm × safety_factor` | E | `BindingConstraint::PowerMax` | no |
| Modulator machine feed cap | per-move feed | E | `BindingConstraint::MachineMaxFeed` | no |
| Modulator kinematic reach | per-move feed | E | `BindingConstraint::KinematicReach` | no |
| Modulator chipload-min floor | per-move feed, **upward**, applied last | E | `BindingConstraint::ChiploadMin` | no |
| Modulator plunge guard | per-move feed on a vertical move | E, after the strategy | `BindingConstraint::PlungeRate` | no |
| `ToleranceBands` | the gate trigger, not the metric | F, optimizer only | on the verdict | export uses `ToleranceBands::default()`, all zeros |

Two floors exist for the same idea and they are **not** the same number:

- `feeds::effective_rubbing_floor` — `min(0.025, derated band max)`. Used by
  stage A step 9b, by stage B pass 9, and read back by
  `recipe_parked_by_rubbing_floor` at stage F.
- `ChiploadBand::min_mm_per_tooth` — the matched row's own minimum. Used by the
  modulator. `max_safe_feed_for_move` states the choice explicitly: "the
  matched row's own `band.min`, NOT `feeds::effective_rubbing_floor`".

---

## 5. Cycles and ordering hazards

### 5.1 The deflection back-off reads a feed that pass 9 then replaces

`backoff_dpp_for_deflection` (pass 6) calls
`predict_peak_deflection_um(operation, ...)`. That predictor reads
`operation.feed_rate()` and derives `fz = feed / (rpm × flutes)`. The force is
affine in `fz`, so the answer depends on the feed.

Pass 9 then re-derives the feed against the final geometry and writes it.
**Nothing re-runs the deflection prediction at the new feed.** When pass 9
raises the feed, `fz` rises, the lateral force rises, and the deflection the
back-off cleared is no longer the deflection the operation will run at.

This is a value feeding a calculation that later changes that value.

### 5.2 Pass 0 re-derates the band; passes 4, 5 and 6 do not

Pass 0 calls `recompute_chipload_bounds_for_dpp` after it mutates the DPP. The
rigidity clamp, the cutting-length clamp and the deflection back-off also
mutate the DPP, and none of them re-derates `context.chipload_bounds`. Pass 9
then applies the rubbing floor against that stale band. The code says so in
`adaptive_entry.rs`:

> A DPP the rigidity / cutting-length / deflection clamps lowered further is
> *not* re-derated there, which leaves this floor judged against a slightly
> harsher band than the final DOC deserves — conservative in the safe
> direction.

A lower DOC gives a **higher** derate scale, so the re-derated band would be
wider. The stale band is therefore narrower, and the floor it caps to is lower
or equal. The direction of the error is stated and it is the safe one.

### 5.3 Pass 9 raises the feed, and no stage rechecks the power budget

`geometry_feed_factor` (`suggest/adaptive_entry.rs:274`) ignores its `ae`
argument. It returns `geometry::depth_tier_multiplier(ap, diameter)` and
nothing else. That multiplier is a step function of `ap / D`:

| `ap / D` | multiplier |
|---|---|
| ≤ 1.0 | 1.00 |
| 1.0 – 2.0 | 0.75 |
| 2.0 – 3.0 | 0.50 |
| > 3.0 | 0.45 |

So **a clamp that lowers the DPP across a tier boundary makes pass 9 raise the
feed.** The rigidity clamp, the cutting-length clamp and the deflection
back-off all lower the DPP. This is the normal direction of pass 9, not an
edge case.

Pass 9 re-applies the machine feed ceiling and the rubbing floor. **It does not
re-apply the power check.** The stage A power ladder had already lowered the
RPM, then `ap`, then `ae`, then the feed until `required ≤ budget`. A pass-9
feed rise reaches the operation without that budget being retested.

Pass 9's own justification says the power derate "is independent of `ae` /
`ap`". Under the R1 two-term model that is not true. The edge term is
`A·F_edge·ap·Vc·zψ/2π`, which carries `ap` and ψ, and the ladder's rungs 2 and 3
change `ap` and `ae` themselves. The claim held under the pre-R1 linear model.

`FeedsWarning::PowerLimited` still carries the pre-ladder pair, and
`FeedsResult::power_kw` still carries the calculator's own feed, not pass 9's.

### 5.4 `ApplyScope::Speeds` ships a feed reconciled to a geometry it discards

`apply_feeds_subset` (`apply.rs:66`) always runs the **full**
`enforce_invariants` on a scratch clone, whatever the scope. Under
`ApplyScope::Speeds` it then copies only the feed, the plunge and the RPM to
the real operation.

Pass 9 computed that feed from `scratch.depth_per_pass()` and
`scratch.stepover()` — the clamped values. The real operation keeps its own
DPP and stepover. **The operation therefore receives a feed derived for a cut
it is not going to make.** `ApplyScope::Speeds` is the default scope of the MCP
`apply_feeds` tool.

### 5.5 Modulation overwrites the kinematics prediction it also depends on

Stage D writes `trace.predicted_feeds` from the kinematics integrator. Stage E
reads it as the `KinematicReach` cap, picks a feed at or below it, and then
overwrites the same map:

```
for (&key, &(feed, _binding)) in &trace.modulated_feeds {
    trace.predicted_feeds.insert(key, feed);
}
```
`crates/rs_cam_core/src/session/compute/simulation.rs:754`

After stage E the map means "the modulated feed", not "the feed the machine can
reach". Both stay true for a modulated move, because the modulator capped at
the prediction. For a move the modulator **skipped** — a rapid, a tagged
plunge, a move with no feed — the original prediction survives. One map, two
provenances, no field distinguishing them.

### 5.6 Two passes that could run in either order and give different answers

- **Pass 3 (raise the stepover) and pass 6 (lower the DPP).**
  `enforce_invariants` pins the order and the comment says the pinning is
  deliberate: "the stepover runtime back-off runs *before* the DPP-block".
  Since the chip-thinning multiplier was deleted from the feed on 2026-08-19,
  the two passes no longer meet inside pass 9: `geometry_feed_factor` reads
  only `ap`, so a stepover move alone leaves
  `factor_at_final == factor_at_calculator` and pass 9 returns early. The
  ordering still matters for `predict_move_count` and for the operation the
  operator receives; it no longer matters for the feed.
- **Pass 6 and pass 7.** The comment says "the plunge-entry stability warning
  fires *after* the deflection back-off has mutated DPP — both intentional".
  Reversing them changes which DPP the plunge-entry check sees.
- **Ladder rung 2 (`ap`) before rung 3 (`ae`).** Both bisect against the same
  budget, and power rises with both. Shrinking `ap` first and `ae` second gives
  a different final `(ap, ae)` pair than the reverse. The code fixes the order
  and gives no independent reason for that order beyond the rung list.
- **Stage E modulation and stage F verdict.** Modulation runs inside
  `run_simulation`. Every verdict in the session is therefore computed on
  post-modulation feeds. A caller that sets `adaptive_feed_modulation: false`
  gets a verdict on a different recipe from the one the default path grades,
  and the emitted G-code changes with it.

### 5.7 The gate's steady-state filter uses a feed the modulator has changed

`steady_state_samples_for_toolpath` (`chipload.rs:301`) keeps a sample when
`s.feed_rate_mm_min >= 0.95 × operation_feed_rate_mm_min`. `s.feed_rate_mm_min`
is the **pre-modulation** feed the simulator ran at, and
`operation_feed_rate_mm_min` is the operation's stored feed, which modulation
does not change. The filter is therefore internally consistent. The
**observation** on the surviving samples then uses `effective_feed_for_sample`,
which is the modulated feed. The population is chosen on one feed and measured
on another.

### 5.8 Three band resolutions for one toolpath

For a single toolpath, three code paths resolve a chipload band, and they do
not use the same population:

| Path | Peak DOC source | Span filter |
|---|---|---|
| `chipload::evaluate_inner` | steady-state samples, then `is_steady_state_for_gate(s, span_lookup)` | full span ancestry |
| `chipload_envelope_for_toolpath` (`tool_load/mod.rs:290`) | all cutting samples, then `is_steady_state_for_gate(s, None)` | **`None`** — falls back to the `in_transit_span` flag, and applies no feed filter |
| `feeds::calculate` step 2 | the commanded DPP; no samples | not applicable |

`chipload_envelope_for_toolpath` feeds the **viewport colours** and the
**modulator's target band**. Its doc says it exists so "the operator-facing
colors agree with the export verdict". The two populations differ, so the
agreement is not structural. I did not measure whether they differ in practice.

### 5.9 The gate can read `Within` on an empty population

`power::evaluate` and `deflection::evaluate` both carry an X-VAC note. The
deflection note is explicit: with `peak_delta_mm == 0.0` and `any_slot == false`
the confidence tier resolves to `Confidence::Validated`, so an empty population
renders as the most trustworthy label the gate can print. Both gates now carry
`GatePopulation` with `offered` and `contributing`. `tool_load/CLAUDE.md` states
the rule: "A gate with no population proved nothing."

---

## 6. What I could not determine

1. **Whether the gate's band and the viewport band actually differ on a real
   toolpath.** Section 5.8 shows two different sample populations reaching
   `derate_chipload_bounds`. Deciding whether they produce different numbers
   needs a run, and I was told not to run cargo.
2. **Whether pass 9 ever raises a feed past the power budget in practice.**
   The mechanism in 5.3 is in the code and the direction is fixed: a
   tier-crossing DPP reduction raises the feed by up to 1 / 0.45 = 2.22×. What
   I could not determine is how often a power-ladder-limited recipe also gets a
   tier-crossing DPP clamp in the same run. That needs a sweep, and I was told
   not to run cargo.
3. **The order of the power ladder's rungs 2 and 3 relative to each other.**
   The code fixes `ap` before `ae`. I found no statement of why that order is
   correct rather than conventional. `DERATE_SPEC.md` and `derate_levers.py`
   are named as the source; I did not read them, because the task is to map the
   code.
4. **Whether `FeedsResult::available_power_kw` has any reader at all.** My
   search over `crates/rs_cam_viz` and `crates/rs_cam_cli` found none outside
   tests. It may be read through a serialised MCP payload I did not trace.
5. **Whether the entry-spike advisory path and the trip path can disagree about
   the same sample.** Both `power::evaluate` and `deflection::evaluate` route a
   configured-entry sample to an advisory and `continue`. Whether a sample can
   satisfy both `is_configured_entry` and the trip population depends on
   `SpanLookup` ancestry, which I read only through its tests.
6. **The exact order of the simulator's own writes** — whether
   `arc_engagement_radians`, `axial_doc_mm` and `axial_engagement_mm` are
   stamped from one pass or several. I read the gates' consumption of them, not
   `dexel_stock::stamping`.
7. **Whether any non-default `ToleranceBands` reaches export.**
   `project_load_report` hard-codes `ToleranceBands::default()`. I did not check
   whether an optimizer-accepted candidate can carry a widened verdict into a
   later export without re-evaluation.
