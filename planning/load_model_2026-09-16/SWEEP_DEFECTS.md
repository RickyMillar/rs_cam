# SWEEP_DEFECTS — the three repeat defect classes, searched on purpose

Date: 2026-09-17. Read-only sweep. No build ran; no test ran. Every claim
below is read off the source.

Citations name the SYMBOL first. Paths and lines are a convenience and the
tree moved on 2026-09-17.

---

## Class 1 — a number computed at one state and consumed at another

### Confirmed, ranked by consequence

#### 1-A. `feeds::efficiency::cut_efficiency` builds one struct from TWO operating points

`crates/rs_cam_core/src/feeds/efficiency.rs:157` (body), `:361`
(`force_headroom`). Renders at `ui::feeds::compare::draw_inspector_rail`
(`crates/rs_cam_viz/src/ui/feeds/compare.rs:164`, verdict line `:533`) and at
`ui::feeds::explore::read_cut_efficiency`
(`crates/rs_cam_viz/src/ui/feeds/explore.rs:180`).

Every field except one reads `result: &FeedsResult` — and the caller passes
`explain.recommended`, which `explain_payload::explain` sets to a bare
`feeds::calculate` output that has never seen `enforce_invariants`:

- `advance_per_tooth_mm`, `specific_energy_j_per_mm3`, `ploughing_share`,
  `verdict`, `band`, `rubbing_floor_mm`, `deflection_ceiling_mm`,
  `wear_ratio_vs_band_mid`, `time_ratio_vs_band_mid` — the RECOMMENDED point.
- `force_headroom` — `force_headroom(operation, …)`, which calls
  `predict::predict_peak_deflection_um(operation, …)`. That is the operation's
  CURRENT depth, stepover, feed and RPM.

The UI prints the two on one line: `"{verdict} · {headroom_phrase}"`.

**What the wrong value is.** The burn verdict describes the cut Apply would
write. The force headroom describes the cut the operator has now. Before an
Apply these are different cuts. `ApplyScope`'s own doc comment
(`feeds/suggest/apply.rs:22`) measures the gap on the shipped default fixture:
the raw recommendation is 4.445 mm of DOC where the funnel writes 1.27 mm, a
factor of 3.50. Deflection is close to linear in `ap`, so the headroom figure
beside an in-band burn verdict can be a `3.5×` different cut.

**How wrong.** Up to the full Apply delta — 3.5× on depth on the shipped
fixture, and unbounded in general, because the two sides move independently.

**Who sees it.** The operator, on the Feeds & Speeds inspector rail and on the
Explore chart's verdict corridor.

**Smallest correction.** Give `force_headroom` the same operating point as the
rest of the struct — derive it from `result`, not from `operation` — or split
`CutEfficiency` so the two points are separately named.

#### 1-B. The depth staircase snap is bypassed whenever any DPP clamp fires

`feeds::suggest::apply::apply_feeds_subset`
(`crates/rs_cam_core/src/feeds/suggest/apply.rs:107`) snaps the proposed depth
through `ops::depth::realised_step_down` — the T-12 fix whose stated purpose is
"the number the engine writes, reasons about and shows the operator equal the
number the machine will cut".

That snap runs BEFORE `invariants::enforce_invariants`. Four passes downstream
rewrite `depth_per_pass` to an arbitrary real number and none re-snaps:

- `axial_envelope::pick_axial_envelope` (clamps to `safe_max_doc_mm`),
- `invariants::clamp_dpp_to_rigidity` (writes `factor × tool.diameter`),
- `invariants::clamp_dpp_to_cutting_length` (writes `tool.cutting_length`),
- `invariants::backoff_dpp_for_deflection` (writes `dpp × 0.8ⁿ`).

`realised_step_down` has exactly one production call site, and it is the one
above. The guard exists and is bypassed on every path a clamp fires.

**What the wrong value is.** The reported depth. Generation steps
`total / ceil(total / per_pass)`, so on a 12 mm pocket only 4.00, 3.00, 2.40,
2.00, 1.71, 1.50 and 1.33 are reachable. A deflection back-off that lands on
3.20 mm is reported as 3.20 and cut at 3.00.

**How wrong.** The T-12 note measures the same arithmetic at 17 % (2.90
reported, 2.40 cut). A back-off landing just above a staircase tread is the
worst case; 6–17 % is the working range.

**Who sees it.** The operator, in the DOC field, in the pill preview
(`apply::preview_field_applies` reads the funnel's scratch clone, so it
inherits the unsnapped number), and in the power ladder's own account of the
depth it derated against.

**Smallest correction.** Re-snap through `realised_step_down` once after
`enforce_invariants` returns, not before it runs.

#### 1-C. `FeedsResult::power_kw` and `FeedsResult::mrr_mm3_min` are frozen at the calculator's geometry

`feeds::calculate` (`crates/rs_cam_core/src/feeds/mod.rs:2429` for
`actual_power`, `:2433` for `mrr`, published at `:2475`/`:2481`).

This is the known-open POWER instance. `mrr_mm3_min = ap × ae × feed` is the
same defect on the same struct and has not been named: all three factors are
rewritten by `enforce_invariants`, and `mrr` is rendered by
`ui::feeds::compare::rail_mrr_row`
(`crates/rs_cam_viz/src/ui/feeds/compare.rs:370`) with no staleness hover.

The DOC and WOC rows beside it DO carry the G-FEEDSLABEL hover ("the
recommended DOC / WOC are the RAW calculator values"). The MRR row and the
efficiency row do not, although they are functions of exactly those two raw
values. The disclosure was fitted to the two fields the review found, not to
the quantity they feed.

**What the wrong value is.** MRR at the pre-clamp `ap` and `ae` and the
pre-pass-9 feed. On the Ø12 pocket where depth is clamped ~3.5×, the rail
overstates material removal rate by the same factor.

**How wrong.** ~3.5× on the shipped Ø12 fixture; the power figure is already
ledgered at the same factor.

**Who sees it.** The operator, on the inspector rail.

**Smallest correction.** Publish both against the post-`enforce_invariants`
operating point, as `feeds::profile::CutterOpProfile` already does for its
predictions (it evaluates at `suggested_operation`, not at the input op).

### Suspected — could not confirm without running the engine

- **Semantic and per-kinematics rows keep the naive clock.**
  `simulation_cut::reporting::rebase_cutting_times` states it itself: under feed
  modulation `SimulationSemanticCutSummary` rows, `SimulationCutHotspot` and
  `KinematicsSummary::cutting_runtime_s` are not rebased, so the per-class times
  no longer sum to the toolpath's. `SimulationSemanticCutSummary` reaches an
  agent through the MCP `get_cut_trace` response
  (`crates/rs_cam_viz/src/app/mcp/simulation.rs:646`). Documented, unfixed, and
  the direction of error is a 1.76× inversion on the wanaka rough.
- **Engagement metrics against the kinematics runtime.**
  `compute::simulate::apply_kinematics_cycle_time` says outright that the trace
  "mixes a kinematics-aware runtime with engagement / chipload / DOC metrics
  that still reflect the naive segment timing" (F-035 is named as the finding
  that would reconcile them). `AirCutRatios` was repaired for the modulation
  path; the engagement averages were deliberately left.
- **Plunge-stress gate against a dressed toolpath.**
  `session::compute::diagnostics::plunge_stress_offenders_for_session` reads
  `tc.operation.plunge_rate()`. Entry dressups emit ramp and helix descents at
  their own feeds. Whether any emitted descent carries a feed above the cap
  while the configured plunge rate sits under it needs a run to decide.

### Checked and clean

- `feeds::profile::CutterOpProfile::build` — evaluates predictions and the
  axial envelope at `suggested_operation` (post-invariant) and falls back to
  the input op only when Suggest refused. This is the correct pattern.
- `dressup::feed_modulation::constrained_max_feed_for_move` — recomputes the
  power terms per move from that move's own measured axial mm and immersion
  angle. No frozen operating point.
- `feeds::suggest::invariants::rescale_feed_to_final_geometry` (pass 9) — the
  2026-08 fix, still last in the chain, still anchored on the unrounded
  `CalculatorOperatingPoint`.
- `session::compute::diagnostics` feeds path — `feeds_result_for_toolpath`
  recomputes against `tc.operation`, so its warnings sit on the current state.

---

## Class 2 — a quantity multiplied by a fraction of a DIFFERENT quantity

### Confirmed, ranked by consequence

#### 2-A. `ui::feeds::explore::preview_power_kw` scales an AFFINE power model by a feed ratio

`crates/rs_cam_viz/src/ui/feeds/explore.rs:1243`:

```rust
(rec.power_kw * (explore_feed / rec.feed_rate_mm_min)).max(0.0)
```

Since R1 (2026-09-16) power is two-term:
`PowerTerms::kw_at_feed(f) = shear_kw_per_mm_min · f + edge_kw`
(`tool_load/power.rs:216`). `power.rs`'s own module header states it in
capitals: **"The edge term contains no feed."** Wood routing is
edge-dominated, which is the whole reason R1 exists.

The ratio `explore_feed / rec.feed_rate_mm_min` is a fraction of the SHEAR
term's basis. It is applied to the total, so the edge floor is scaled as
though it were feed-proportional.

Two further mismatches on the same three lines:

- The preview ignores the explored RPM entirely. `edge_kw` carries
  `Vc = π·D·n`, so dragging the RPM slider must move power and does not.
- `power_pct` (`:1135`) divides by `env.max_power_kw`, the machine's RATED
  power. The gate's ceiling is `machine.power_at_rpm(rpm) × safety_factor`
  (`feeds/mod.rs:1898`, `tool_load/power.rs`). On a
  `PowerModel::VfdConstantTorque` spindle below rated RPM, `power_at_rpm`
  derates linearly (`machine/mod.rs:311`), so the two denominators differ by
  `(rpm/rated_rpm) × safety_factor`.

This is the F-2 defect. F-2 moved `FeedsResult::available_power_kw` onto the
gate's axis because the modal "showed 1/safety_factor (1.25×–1.33×) more
headroom than the verdict would allow". The Explore panel's own headroom bar
was not moved with it.

**What the wrong value is.** The kW figure and the "% of cap" beside it.
Dragging the explore feed DOWN under-reports power by the share of `edge_kw`
removed; on wood that share is the majority of the total. Dragging RPM does
nothing. The percentage is then divided by a ceiling that is too generous by
`1/((rpm/rated_rpm)·safety_factor)`.

**How wrong.** At half the recommended feed with an edge-dominated cut the kW
figure reads roughly half where the true value is ~70–85 % (the ploughing
share `efficiency.rs` documents for normal wood chiploads). Compounding the
denominator: a VFD spindle at half rated RPM with `safety_factor = 0.8` puts
the bar at ~40 % where the verdict reads 100 %.

**Who sees it.** The operator, live, while dragging the sliders that decide
the recipe — and the colour thresholds at `>70 %` / `>90 %` are driven off
this number, so the panel goes amber and red at the wrong places.

**Smallest correction.** Build `PowerTerms` from the explored point and call
`kw_at_feed`, and divide by `machine.power_at_rpm(explore.rpm) ×
machine.safety_factor` (the envelope already carries `safety_factor`).

#### 2-B. `tool_load::optimize::narrative::suggest_for_gate` caps stepover by a POWER ratio

`crates/rs_cam_core/src/tool_load/optimize/narrative.rs:461`:

```rust
(GateKind::Power, _) => {
    let current_stepover = candidate.params.stepover()?;
    let ceiling = current_stepover * ratio * 0.95;   // ratio = bound / observed
```

`ratio` is a fraction of POWER. It is applied to a stepover. The two are the
same quantity only if power is proportional to stepover, and after R1 it is
not: the edge term is `F_edge · ap · Vc · (z·ψ/2π)` with
`cos ψ = 1 − 2·ae/D` (`feeds/force.rs:242`). For small immersion
`ψ ≈ 2·√(ae/D)`, so the edge term goes as `√ae` while the shear term goes as
`ae`.

**What the wrong value is.** The operator-facing advice "cap stepover at X".
It is derived on the assumption that halving stepover halves power. On an
edge-dominated wood cut at, say, `ae/D = 0.1`, halving stepover removes only
about 29 % of the edge term.

**How wrong.** In the edge-dominated limit the needed stepover factor is
`ratio²`, not `ratio`. Asking for a 20 % power reduction (`ratio = 0.8`) the
advice says "stepover × 0.76"; the cut actually needs about × 0.64. The
operator follows the advice, re-runs, and the power gate still exceeds.

**Who sees it.** The operator, as an `OperatorSuggestion::CapAxisAt` in the
optimiser's outcome narrative.

**Smallest correction.** Invert the shipped model instead of the ratio —
`PowerTerms::feed_for_kw` already does this for the feed axis in
`dressup::feed_modulation`, whose comment says "the cap is an affine
inversion, not a ratio". The stepover axis needs the same treatment through
`force::immersion_angle`.

#### 2-C. Same function: the chipload arms mix the COMMANDED and the ACHIEVED feed axes

`narrative::suggest_for_gate`, the two `GateKind::Chipload` arms, and
`suggestions_for_rpm` just above them (`narrative.rs:400`).

`ratio` is `g.bound / g.observed`. `g.observed` is the gate's number, and
`tool_load::chipload` computes it from `effective_feed_for_sample(s,
&trace.predicted_feeds)` — the ACHIEVED feed the kinematics integrator
predicts. `candidate.params.feed_rate()` is the COMMANDED feed. Multiplying
one by a ratio of the other assumes achieved is proportional to commanded.

The codebase already names the gap: `FeedExplanation::multiplier_clause`
exists precisely to print "×{r} achieved/commanded feed", and
`AchievedFeedStage::median_ratio` is the measured factor.

**What the wrong value is.** `RaiseAxisAbove { axis: Feed, floor }` on the
`ChipSide::Low` arm. On an accel-limited path (short segments, a fine finish)
the achieved feed saturates, so raising the commanded feed to the stated floor
does not raise the achieved chipload to the band. The advice is ineffective
exactly where the low-chipload verdict is most likely — burn risk stays.

**How wrong.** By `1 / median_ratio`. The repo has a measured 1.76× on a
wanaka rough for the sibling clock defect; the same order applies here.

**Who sees it.** The operator, in the optimiser narrative.

**Smallest correction.** Divide the correction by
`FeedExplanation::achieved_feed.median_ratio` when it is `Some`, and withhold
the numeric floor when it is `None`.

### Suspected — could not confirm

- `narrative::suggest_for_gate`, `GateKind::Deflection` arm
  (`narrative.rs:449`): `current_doc × (bound/observed)`. Force is
  `ap·(Ks·h + F_edge)`, linear in `ap`, but the cantilever is integrated over
  the engaged height (`tip_deflection_from_engagement`), so deflection is
  super-linear in `ap`. The advice is therefore optimistic, but by how much
  needs the integrator run.
- `metrology/census.rs:439`: a censored nearest-turn query is imputed at
  `search_bound_mm` and folded into the median coherence length. The
  `censored_fraction` is published beside it, so a reader CAN tell — I count
  this as a guard that holds, but the imputed value does move the median.

### Checked and clean

- `dressup::feed_modulation::effective_axial_mm` — the T-11 fix. Reads
  `engagement.axial_doc_mm` (a measurement in mm) and never multiplies the
  flute-length fraction by a depth.
- `dressup::feed_modulation` power cap — `PowerTerms::feed_for_kw`, an affine
  inversion, with `ψ` from the same `immersion_angle` the deflection cap uses.
- `ops::adaptive_shared::radial_woc_fraction_from_leading_arc` and its GUI
  caller `properties::operations::surface_3d::draw_spiral_load_control` —
  the fraction is a fraction of DIAMETER and is multiplied by `2 · r`.
- `feeds::vendor_lookup` scaling (`diameter_scale × hardness_scale` applied to
  `chipload_min` / `chipload_max`) — both sides are mm/tooth.
- `feeds::geometry` DOC-derating scale applied to `ChiploadBounds` — both
  sides mm/tooth.
- `stock::simulation_cut::accumulate` — every time-weighted mean divides by the
  runtime over which that quantity was OBSERVED, not by the total.

---

## Class 3 — an absence that renders as a reading

### Confirmed, ranked by consequence

#### 3-A. A FAILED collision check is written to `summary.json` as zero collisions and status "ok"

`rs_cam_cli::project` (`crates/rs_cam_cli/src/project.rs`), the collision loop
at `:377` and the two readers at `:424` and `:511`.

The loop inserts into `collision_reports` only on `Ok(check)` AND only when
`!check.collision_report.is_clear()`. The `Err(e)` arm logs `warn!` and inserts
nothing. Both readers then do:

```rust
.map(|r| r.collisions.len()).unwrap_or(0)
```

and `:514` sets `status = if total_collisions > 0 { "error" } else { "ok" }`.

Three distinct states collapse to the number `0` and the word `ok`:

1. checked, clear — correct,
2. no mesh (`SessionError::MissingGeometry`, a 2D op) — not checked,
3. the check ERRORED or was cancelled — not checked.

State 3 is the defect. The failure is visible only in the tracing stream; the
machine-readable artifact says the toolpath is clear.

This is the same shape the CLI already found and fixed twice on its own CSV
reader — `crates/rs_cam_cli/src/smoke.rs:321`, `:436` and `:1389` each carry a
note that `parse().unwrap_or(0)` "read an unwritten cell as a clean zero".
The fix was applied to the reader and not to the producer.

**What the wrong value is.** `collision_count: 0`, `status: "ok"` for a
toolpath whose holder-collision check never completed.

**How wrong.** Categorically. There is no numeric error; there is a safety
verdict asserted without evidence.

**Who sees it.** Anyone reading `summary.json` — a batch operator, a CI gate,
or an agent. It is the only machine-readable collision verdict the CLI emits.

**Smallest correction.** Record the refusal. Make `collision_reports` hold an
outcome (`Checked(report)` / `NotApplicable` / `Failed(reason)`) and emit
`status: "unknown"` for anything that is not `Checked`.

#### 3-B. `KinematicsSummary::average_radial_woc_fraction` and `peak_radial_woc_fraction` are bare `f64` beside `Option` siblings

`crates/rs_cam_core/src/stock/simulation_cut.rs:353` and `:357`, filled by
`accumulate::KinematicsAccumulator::finish` (`accumulate.rs:68`).

Their axial neighbours were converted by C2 (2026-07-30) and carry the reasoning
in their own doc comment: "It used to be `0.0`, which consumers were asked *by a
doc comment* to read as 'unknown' rather than 'no axial engagement'; that is
exactly the silent-sentinel class A/M9 retired". `average_axial_doc_fraction`,
`peak_axial_doc_fraction`, `average_arc_radians`,
`average_mean_chip_thickness_mm` and `peak_chip_thickness_mm` are all `Option`.
The two radial fields were left as `f64` with an explicit `0.0` else-branch.

The radial axis has the same absence: `Engagement::radial_woc_fraction` is `0.0`
on a plunge, a drill sample and any `Engagement::default()` fixture, because a
Z-only move has no width of cut. `metrics_not_applicable` covers whole drill
TOOLPATHS; it does not cover a plunge inside a milling toolpath.

**What the wrong value is.** "Average radial engagement 0 %" on a
`CutKinematics::Plunge` block, where the truth is "radial engagement is not a
quantity this class has".

**How wrong.** The number is not merely low, it is the wrong axis. The same
sentinel reaches `SummaryAccumulator::observe` (`accumulate.rs:143`), where
`radial_woc_fraction < 0.02` classifies the sample as AIR CUT — so plunge
seconds inside a milling toolpath inflate `air_cut_time_s`, and the shipped
40 % air-cut threshold fires on them.

**Who sees it.** The operator, on the GUI air-cut banner and the sim-diagnostics
per-kinematics rows; and an agent, through the MCP `get_cut_trace` response
(`app/mcp/simulation.rs:1137`).

**Smallest correction.** Make both radial fields `Option<f64>` with the same
observed-runtime denominator the axial pair already uses, and gate the air-cut
classifier on `Some(_)`.

#### 3-C. `predict::predict_peak_deflection_um` still returns `0.0` on refusal, and the back-off consumes the zero silently

`crates/rs_cam_core/src/feeds/predict.rs:129`. The doc comment states the
contract: "Returns `DeflectionPrediction { predicted_um: 0.0, .. }` for any
refusal case". Nine `return` sites produce it — drill family, V-bit, no
primary-source `Kc`, zero stickout, and more.

The lead's brief lists this one as fixed. It is fixed at ONE consumer:
`efficiency::force_headroom` guards `predicted_um > 0.0` and returns `None`,
which the UI renders as "force headroom not modelled". The refusal contract
itself is unchanged, and the second consumer does not guard:

`invariants::backoff_dpp_for_deflection` (`invariants.rs:387`) loops on
`predicted_um > DEFLECTION_BACKOFF_TARGET_UM`. Its own doc comment says the
loop "short-circuits trivially and no back-off occurs". `iterations` stays 0,
so no `SuggestWarning::DppCappedByDeflection` is pushed — and there is no
"deflection not modelled" warning to push instead.

**What the wrong value is.** The silence. On a V-bit, or on any material
without a primary-source `Kc`, Suggest writes the rigidity-clamped DPP with no
deflection check and says nothing about having skipped one. The operator cannot
distinguish "the deflection back-off looked and was satisfied" from "the
deflection back-off could not look".

**How wrong.** Categorically, again. The DPP that results is whatever
`clamp_dpp_to_rigidity` allowed — on a Ø12 that is ~2.5 mm with no physical
check behind it.

**Who sees it.** The operator, as an absent warning in the Suggest rationale
tree.

**Smallest correction.** Return `Option<DeflectionPrediction>` (or add a
refusal reason to `DeflectionPrediction`) and give `SuggestWarning` a
"deflection not modelled at this operating point" arm.

### Suspected — could not confirm

- `feed_explanation::FeedExplanation::predicted_gate_observation_mm`
  (`feed_explanation.rs:394`): `median_ratio.unwrap_or(1.0)`. The absence is
  recoverable — `AchievedFeedStage::predicted_feeds_present` and
  `multiplier_clause` both name it, and the doc says so. But the function
  returns `Some(_)` unconditionally, so a consumer that reads only this number
  cannot tell. I could not establish which renderers read it bare.
- `metrology/census::PrizeCell::ratio` (`census.rs:593`):
  `quantiles(ratios).unwrap_or_default()` puts an all-zero quantile set in the
  cell for an empty population, and `gouge_area_frac` falls to `0.0` on zero
  area. `ZoneVerdict::NotMeasurable` guards the zone verdict, but not the
  `PrizeCell`. This is a research instrument, not an operator surface, so the
  consequence is low.
- `tool_load::plunge_stress::check_plunge_stress` returns `None` both for "no
  cap applies to this geometry" and for "checked and within". Both render as no
  finding. Defensible — a flat endmill genuinely carries no flute-tip plunge
  risk in this model — but it is the same shape.

### Checked and clean

- `feeds::efficiency` — the module header is the reference statement of the
  rule ("**Render a `None` as an abstention, never as a zero**"), every field
  refuses independently, and `ui::feeds::compare` prints "force headroom not
  modelled" for the `None`.
- `tool_load::verdict::GatePopulation` — constructed by `chipload`, `power`,
  `deflection` and `drill_gates`. `is_vacuous` / `vacuity_clause` reach the
  diagnostic adapter (`adapters::from_tool_load.rs:407`, `:739`).
  `plunge_stress` has no population and does not need one: it reads a
  configured value, not a sample set.
- `feeds::cutter_constraints::CutterAxialConstraints::is_in_band` — returns
  `Option<bool>`, not a bool.
- `axial_envelope::apply_axial_envelope:46` — the
  `min_doc_chipload_floor_mm.unwrap_or(0.0)` sits inside the `SafeBandEmpty`
  arm, and `cutter_constraints.rs:236` only sets `SafeBandEmpty` inside
  `if let Some(floor)`. Unreachable.
- `tool_load::chipload.rs:663` — `min.unwrap_or(0.0)` feeds
  `gate_band_for_clamp`, whose only consumer is
  `feeds::effective_rubbing_floor`, which reads `max_mm_per_tooth` only.
  Harmless.
- `session::compute` per-move engagement rollup (`compute.rs:1636`) — the
  `axial_doc_fraction.unwrap_or(0.0)` carries an explicit C2 note, keeps the
  time weight, and the absolute `axial_doc_mm` travels beside it.
- `stock::simulation_cut::reporting::rebase_cutting_times` — returns `None`
  rather than writing a zero when there are no cutting samples, and says so.

---

## The one I would fix first

**3-A — the CLI writes `collision_count: 0, status: "ok"` for a collision check
that failed.**

Not because it is the largest number. Because it is the only finding here where
the output is a SAFETY verdict rather than a recipe figure, and where the
consumer is a machine. Every other finding puts a wrong number in front of a
person who can sanity-check it against the cut they are about to make. This one
tells a batch pipeline that a toolpath was cleared when nothing cleared it, and
the only trace of the failure is a `warn!` line in a log nobody reads after the
run.

It is also the cheapest: three lines and one enum, no model change, no
recalibration, and no sentry to re-pin — the correction only adds a state that
did not exist before.

The runner-up is 2-A (`preview_power_kw`), because it is live, operator-facing,
wrong in three independent ways at once, and is the same F-2 defect the repo
already fixed one surface away.
