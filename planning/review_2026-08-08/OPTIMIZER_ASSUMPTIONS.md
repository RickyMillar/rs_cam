# A-8 — optimizer assumptions, and the first measured retarget outcome

Wave: TD3 **A-8** (Lane A). Date: **2026-08-13**.
Branch `tech-debt-3`, parent **`1c0a4acc`**.
Seeds: the operator review's `[medium] The optimizer initially evaluates a
different feed world than production simulation`, and TD2 close-out ledger row
**F-OPT**.

Simulation cell for every measured number in this document: **0.5 mm** (the
optimizer's own `refined_resolution_mm` — the cell every *reported* candidate
verdict is taken at). Dev profile, `cargo test -p rs_cam_core`.

---

## 0. What this wave was asked for, and what it returned

| charter item | outcome |
|---|---|
| 1. Stamp simulation assumptions onto optimizer results | **SHIPPED.** `OptimizeOutcome::assumptions: Option<SimAssumptionStamp>`, stamped on every outcome including refusals. Five sentries, 0.00 s. |
| 2. The first real retarget/reconciliation fixture | **SHIPPED and it MEASURED A DEFECT.** A real sim-produced `Exceeds(High)` → the real retargeter → re-sim. On a deep pass the retarget lands **exactly on its own target** and is **still `Exceeds`**, at **1.6667×** the bar the gate applies. A control arm at shallow DOC reconciles to `Within`, which attributes the failure to one mechanism. |
| 3. Census the optimizer's post-K assumption surface | **DONE.** The optimizer subtree reaches **zero** of Checkpoint K's artifacts by name. Six gate-verdict re-decisions bypass the epsilon contract. Two retired-vocabulary docs fixed; one live consumer of the retired measure reported, not touched. |
| 4. Deliverable + log entry | this file + `ORCHESTRATION_LOG.md` §3.1. |

**Research-first clause: NOT triggered as a stop.** The measurement does not
contradict the review's reading — it *sharpens* it. The review said the
optimizer "evaluates a different feed world". It does, and the difference is
larger and more mechanical than the review's two flags: a third divergence,
in the chipload **band** rather than the feed model, makes the retarget stage
structurally unable to succeed on any pass deeper than ~1.67 × diameter. That
is new, it is number-moving to fix, and it is **Checkpoint request 1** below
rather than a fix in this wave.

---

## 1. The stamp

### 1.1 Shape

`crates/rs_cam_core/src/tool_load/optimize/outcome.rs`

```
OptimizeOutcome
└── assumptions: Option<SimAssumptionStamp>
    ├── candidates: CandidateSimAssumptions
    │     coarse_resolution_mm, refined_resolution_mm, auto_resolution,
    │     adaptive_feed_modulation, modulation_strategy,
    │     modulation_aggressiveness, use_predicted_feed_in_gates
    ├── baseline:   BaselineTraceAssumptions
    │     resolution_mm: Option<f64>            (always None — see §1.4)
    │     adaptive_feed_modulation: Option<bool> (Some(true) or None, never Some(false))
    │     sample_step_mm: f64
    ├── kinematics: KinematicsSource            (ProfileDeclared | GenericWoodRouterFallback)
    ├── lut_query:  Option<LutQueryStamp>       (Routed{declared,queried} | Refused{..})
    └── boundary_epsilon_rel: f64
```

Stamped at `optimize/mod.rs:159`, immediately beside F4.3's
`MachineSnapshot::of`, and **outside** `optimize_toolpath_inner` — so a
`Skipped` or `NoSafeImprovement` outcome that never simulated still names the
operating point its refusal was decided at. That is F4.3's own precedent, and
the reason is the same: a refusal is also a claim about the world.

### 1.2 Which report-only contract family, and why

The `ToolpathStats` families named in `compute/config.rs` are three:

1. the **`truncated_core_mm2` family** — `None` = *not measured*, a present
   value = measured (eleven fields share it);
2. `derived_stepovers` — a `Vec`, where empty means "nothing derived", not
   "measured zero";
3. `zero_removal` / `boundary_clip_dropped` — `None` deliberately conflates
   "not measured" with "nothing to report", because neither supports a ratio.

**The stamp takes family 1.** `assumptions: None` means *not stamped* and is
reachable exactly two ways: an outcome built by a constructor and not passed
through `optimize_toolpath` (all five constructors leave it `None`), or an
outcome deserialized from a record written before the field existed. It never
means "no assumptions" and never means "the defaults".

Family 3 was rejected on purpose. Its licence is that the two cases *cannot*
be told apart. Here they can, and the inner `Option`s keep them apart — see
§1.4, where the honest answer to two of three baseline questions is "the
trace does not record it", which is a different statement from "it was off".

Family 2 does not apply: there is no collection.

Report-only in the strict sense used elsewhere in this repo: **no gate
consumes it, no candidate is ranked on it, no verdict changes on it.**

### 1.3 Instrument integrity — the stamp cannot drift from the sim

The candidate `SimulationOptions` literal used to live inline in
`evaluate_candidate_inner`. A stamp built from a second copy of those values
is §0 rule 5's failure mode exactly: a docstring waiting to become a lie you
then cite.

The literal was hoisted to `candidate::candidate_sim_options(resolution)`
(`candidate.rs:160`), the one construction site. `CandidateSimAssumptions
::observed()` calls **that function** and reads the fields off the returned
options. The two resolutions come from `search_policy().stages`, which is
where both call sites get them. Nothing was duplicated; the literal moved.

### 1.4 What the stamp refuses to claim

* **`baseline.resolution_mm` is always `None`.** `SimulationCutTrace` carries
  `sample_step_mm` and a `SimulationProvenance` of geometry hashes. It carries
  **no dexel cell** — the cell survives only on `SimulationCutArtifact`, which
  the optimizer is never handed. So the resolution the *baseline candidate*
  (index 0) was measured at is genuinely unrecoverable, and the stamp says so
  rather than borrowing `refined_resolution_mm`. The two are known to differ
  whenever the user's on-screen sim did not happen to run at 0.5 mm.
* **`baseline.adaptive_feed_modulation` is `Some(true)` or `None`, never
  `Some(false)`.** `Some(true)` is a positive observation:
  `modulation_summaries` is written by `apply_adaptive_feed_modulation` and by
  nothing else. An *empty* map has two causes that cannot be separated here —
  modulation was off, or the trace was round-tripped through serde, where the
  map is `#[serde(skip)]`. `None` is therefore "not measured".
* **`boundary_epsilon_rel` records the crate constant, not a promise the
  optimizer used it.** §3.2 measures that it does not.

### 1.5 Sentries — `crates/rs_cam_core/tests/optimizer_assumption_stamp_a8.rs`

Five tests, **all pass, 0.00 s** (they ride the `Material::Custom` refusal,
which skips *after* the evaluation context is built and *before* any candidate
is generated — so the LUT routing is observable for free and no sim runs).

| test | asserts |
|---|---|
| `an_optimizer_result_carries_its_simulation_assumptions` | a result carries the stamp; every block present; `baseline.resolution_mm` is `None` with the reason in the message |
| `two_results_from_different_assumption_sets_carry_different_stamps` | stamps differ, **and** differ independently on kinematics source, baseline modulation evidence, baseline sampling scale, and the LUT query — so the inequality cannot be carried by one field |
| `baseline_modulation_is_observed_positively_or_not_at_all` | `Some(true)` on a modulated trace; `None` (never `Some(false)`) on one that records nothing |
| `the_stamp_names_the_lut_family_the_band_was_negotiated_under` | Pocket → Pocket, not rerouted; Adaptive3d declares `Adaptive`, is queried as `Pocket`, and `is_rerouted()` says so |
| `the_optimizer_scores_candidates_at_a_different_operating_point_than_the_library_default` | the F-036b divergence, pinned as a fact (§4) |

**Red-first.** Every one of these is a **compile error** at the parent:
`git show 1c0a4acc:.../outcome.rs | grep -c assumptions` → **0**. The field and
all four stamp types are new, so the claim cannot be made at all before the
change rather than being made wrongly. That is the strongest available form of
the reproduction and it is the same shape as the S-GAP3 probe (`E0027` at the
child, clean at the parent).

---

## 2. The retarget/reconciliation fixture — F-OPT discharged, with a defect

`crates/rs_cam_core/src/tool_load/optimize/retarget_reconciliation_a8.rs`.

### 2.1 Why it is in `src/` and not `tests/`

`run_retarget_stage` assembles the retargeter from `pub(crate)` parts —
`context::find_matched_lut_row`, `search_policy()`, `SearchPolicy`. An
integration test cannot build it the way production does; reproducing the
construction in `tests/` would measure the reproduction. This module calls the
same functions the production stage calls. Declared `#[cfg(test)] mod
retarget_reconciliation_a8;` at `optimize/mod.rs:59`.

### 2.2 The gap it closes

F-OPT: *"Its own tests build synthetic verdicts and stayed green, so **no
evidence measures a real retarget outcome moving.**"* True — every existing
chipload-retarget test hand-builds a `ChiploadVerdict` and a `MatchedRow`.
Those tests assert the arithmetic, and the arithmetic is right. What none did
was let a **simulation** produce the verdict and then ask whether the feed the
retargeter picks reconciles when the sim is re-run.

### 2.3 Fixture

Ø6.35 flat end mill (2 flutes), hard maple, 30 × 30 mm pocket in 44 × 44 × 25
stock, single pass, 18 000 rpm, commanded feed 12 000 mm/min (= 0.3333
mm/tooth — well above any Ø6 hardwood band, so the **high** side trips
genuinely). Machine feed ceiling raised to 24 000 so a machine clamp cannot
quietly become the binding constraint.

The **high** side is deliberate: **F-MISSAE** demotes the low/burn side to an
advisory on 176 of 252 shipped rows via
`ChipBoundsSource::low_side_is_advisory`, and the retargeter fires only on
`Exceeds`. A burn-side fixture would usually never reach the retargeter — which
is exactly the blocker A-5 recorded, routed here, and worked around rather than
argued with.

Matched row on both arms: **`amana-flat-hardwood-pocket-6000-2f`**, calibrated
d = 6.000 mm, Ø-scale ×1.035, Janka-scale ×1.000, `is_extrapolated = false`.
Gate steady-state population **n = 452 / 447** — non-vacuous (X-VAC), read from
`FeedExplanation::gate.sample_count`, deliberately **not** from
`triggering.evidence.sample_range.len()`, which is 1 by construction and would
make the guard vacuous in exactly the way X-VAC warns about.

Comparability: the two arms differ **only in `feed_rate`**; feed moves no
geometry. S-4's `StockSnapshotStamp` is asserted equal across arms — and the
file states plainly that on a fresh-stock op both are `None`, so `None == None`
carries no information on its own and the load-bearing argument is the
one-field delta. The gate's ceiling is also asserted unmoved between arms.

### 2.4 The measurement

```
=== cell 0.5 mm, DOC 20 mm  (DOC/Ø = 3.15) ===
arm A (baseline)   feed  12000.0  observed 0.33333  gate ceiling 0.02847  n=452  Exceeds
row max (Ø/Janka-scaled) 0.05694   gate's DOC-derated ceiling 0.02847   derate ×0.5000
retarget target 0.04745 = row max / headroom 1.20    target / gate ceiling = 1.6667
retarget feed     1708.1  (0.1423× baseline)
arm B (retargeted) feed   1708.1  observed 0.04745  gate ceiling 0.02847  n=452  Exceeds
reconciliation: observed 0.04745 vs gate ceiling 0.02847 -> 1.6667x (STILL OVER)

=== cell 0.5 mm, DOC 3 mm  (DOC/Ø = 0.47)  — CONTROL ===
arm A (baseline)   feed  12000.0  observed 0.33333  gate ceiling 0.05694  n=447  Exceeds
row max (Ø/Janka-scaled) 0.05694   gate's DOC-derated ceiling 0.05694   derate ×1.0000
retarget target 0.04745 = row max / headroom 1.20    target / gate ceiling = 0.8333
retarget feed     1708.1  (0.1423× baseline)
arm B (retargeted) feed   1708.1  observed 0.04745  gate ceiling 0.05694  n=447  Within
reconciliation: observed 0.04745 vs gate ceiling 0.05694 -> 0.8333x (inside)
```

**Read the retarget feed column.** It is **1708.1 mm/min on both arms.** The
retargeter emits the identical feed for a 3 mm pass and a 20 mm pass, while the
gate's ceiling halves between them. That single number is the mechanism.

### 2.5 The mechanism

`run_retarget_stage` (`optimize/mod.rs:533-534`) hands
`ChiploadFeedRetargeter` the **raw** matched row:

```rust
lut_chipload_min: row.chip_load_min_mm.unwrap_or(f64::NAN),
lut_chipload_max: row.chip_load_max_mm.unwrap_or(f64::NAN),
```

Those are diameter- and hardness-scaled (`LookupResult`'s docs say "Scaled
lower/upper bound") but **not DOC-derated**. The chipload gate compares
against `geometry::derate_chipload_bounds(result.chip_load_min_mm,
result.chip_load_max_mm, doc_ratio, AllowHalfBand)` at
`tool_load/chipload.rs:581-586`, where `doc_ratio` is the **measured peak
axial DOC** over the engaged diameter.

`doc_derating_scale` is 1.0 at `DOC/Ø ≤ 1`, falls linearly to 0.75 at 2 and
0.50 at 3, and is flat at 0.50 beyond. The retargeter's high-side target is
`row_max / 1.2 = 0.8333 × row_max`. So the target is reachable only while

```
doc_derating_scale(DOC/Ø)  >  0.8333      i.e.   DOC/Ø  <  ~1.67
```

Past that the retargeter is aiming at a number **above the bar it will be
judged by**, and a retarget that lands perfectly on its own target still reads
`Exceeds(High)` on the re-simulation. The worst case is `1/0.8333/0.5 =
1.6667×`, reached at `DOC/Ø ≥ 3` and measured above to the digit.

**The gate's own ceiling is already in the retargeter's hand.** It is
`ChiploadMetric::bounds.max_mm_per_tooth` on the very `ChiploadVerdict` the
retargeter is passed (`verdict.rs:809`). The retargeter reads the injected raw
row instead.

### 2.6 What this costs in production

The retarget candidate goes through `evaluate_candidate` like every other:
apply → **regenerate** → **full project simulate** → gate. Past the knee it is
a guaranteed-rejected candidate costing a complete generate and simulate.

How often is past the knee? Stated precisely rather than asserted, because the
threshold is close to shipped defaults: the generic wood router's
`RigidityProfile::adaptive_doc_factor` is **1.5**, i.e. adaptive roughing sits
at 1.5 × Ø and is *just inside* the 1.67 boundary (derate 0.875 vs the 0.8333
the target needs — about 5 % of margin). What is outside: any user-set DOC
above ~1.67 × Ø, full-depth single-pass profile and pocket work on stock
thicker than 1.67 × the cutter (the fixture's own shape, and an ordinary thing
to ask a Ø3 or Ø6 cutter to do), and every deep-slotting case. The derate
itself starts biting at `DOC/Ø > 1.0`; only the *sign* of the comparison flips
at 1.67. **A-8 did not measure a shipped-project population** — that is a
coverage gap, named in §7.

It is not a *safety* failure: the candidate is correctly rejected, so nothing
unsafe is recommended. It is a **capability** failure that presents as "the
optimizer found no safe improvement", and it is a plausible single reason
F-OPT could record that no evidence had ever seen a retarget outcome move.

### 2.7 Disposition

**NOT FIXED.** Re-pointing the retargeter at `triggering.bounds` moves every
optimizer outcome on every deep pass. Checkpoint request 1, §6.

The two tests pin the **defect**, in their own words, and say so:

* `a_retarget_reconciles_while_the_doc_derate_is_inactive` (control)
* `a_retarget_cannot_reconcile_once_the_doc_derate_engages` (the finding)

When the ruling lands, the second fails. It must be **inverted deliberately
with old/new recorded** — not deleted, not re-baselined. The control is what
makes the inversion attributable.

---

## 3. Census — the optimizer's post-Checkpoint-K assumption surface

### 3.1 Checkpoint K's artifacts reach the optimizer zero times by name

Across all 22 files (~12.8k lines) of `tool_load/optimize/**`:

| Checkpoint K artifact | occurrences in `optimize/**` |
|---|---|
| `vendor_normalize::lut_query_for` | **0 direct** (1 transitive, via `chipload::matched_chip_envelope`) |
| `tool_load::boundary::{exceeds_high, below_low, is_at_bound, slack_for}` | **0** |
| `ChipBounds::{exceeds_high, below_low, contains, is_at_max}` | **0** |
| `BOUNDARY_EPSILON_REL` | **0** |
| `ChiploadVerdict::Within::ceiling_advisory` (read) | **0** (10 sites, all `None` in test fixtures) |
| `CommandedStage::clamped_to` | **0** |

**LUT routing is correct, by inheritance only.** `context::find_matched_lut_row`
delegates to `chipload::matched_chip_envelope`, which calls `lut_query_for`
internally (`chipload.rs:131`). There is exactly one LUT resolution in the
subtree and **no** call to `find_best_row_for_geometry` or
`find_best_chip_envelope_row`. So A-7's unification *did* reach the optimizer's
row selection — the routing is right, nothing said so, and
`SimAssumptionStamp::lut_query` now does.

### 3.2 Six gate-verdict re-decisions bypass the epsilon contract — REPORTED

| # | site | code |
|---|---|---|
| 1 | `optimize/delta.rs:183` | `approach_to_max.observed_mm_per_tooth > approach_to_max.bounds.max_mm_per_tooth` |
| 2 | `optimize/delta.rs:190` | `min_metric.observed_mm_per_tooth < strict_min` |
| 3 | `optimize/delta.rs:206` | `*peak_kw > *available_kw` |
| 4 | `optimize/delta.rs:216` | `*peak_mm > bounds.exceeds_mm` |
| 5 | `optimize/narrative.rs:683` | verbatim duplicate of #1 |
| 6 | `optimize/narrative.rs:703` | verbatim duplicate of #2 |

`ChipBounds::exceeds_high` / `::below_low` (`verdict.rs:755,763`) exist for
exactly these comparisons and carry the Checkpoint K epsilon.

Consequence, which is the G-CHIP-ULP shape relocated: a candidate parked on the
band ceiling by the rubbing-floor clamp gets `Within` from the epsilon-aware
gate, and then #1/#5 independently re-decide it as a **strict breach** — routing
it to `MarginalSafe` / `band_admitted: true` on float noise. Checkpoint K fixed
the gate; it did not reach the optimizer's tier dispatch.

Not fixed here: #1–#6 change which tier an outcome lands in, i.e. whether the
modal auto-recommends or demands "verify on a scrap". Checkpoint request 2.

Lower-severity, same pattern, listed for completeness: `rank.rs:62-72,92-94,
109-111` (ad-hoc `1e-9` where `slack_for` is the contract);
`bounds.rs:33-53` (`Interval::contains`/`intersect`, axis bounds not gate
bounds); ad-hoc `1e-6` float epsilons at `retarget/chipload.rs:117,172`,
`retarget/power.rs:96`, `retarget/deflection.rs:105`.

### 3.3 Retired arc-fit / chip-thickness vocabulary — three hits, two fixed

The 2026-08-06 unit deletion landed cleanly in this subtree. **Zero** hits for
`arc_fit`, `arc_fit_ratio_for_op`, `arc-mean`, `engagement arc`,
`ae normalis`, `VendorLutMissingAe`.

| site | kind | disposition |
|---|---|---|
| `context.rs` docstring on `find_matched_lut_row` | named the deleted `routed_lookup_family` | **FIXED** — now names `lut_query_for`, and states that the row returned is the RAW row, not the band the gate judges by (§2.5) |
| `preflight.rs:168` comment | glossed feed-per-tooth as "(chip thickness)" | **FIXED** — the arithmetic three lines below was always an advance per tooth; the gloss was the retired vocabulary |
| `preflight.rs:107` `is_bipolar_engagement(&steady.samples, cl_min, cl_max)` | **live code**, a real consumer of `effective_chip_thickness_mm` | **REPORTED, NOT TOUCHED** — this is ledger row **F-BIPOLAR**, already owned by "the optimizer pre-flight lane". A-8 confirms its live population: the optimizer *refuses to optimize* based on the retired arc-mean measure while the gate it defers to observes advance per tooth, and the bounds it compares against (`row.chip_load_min_mm`/`max_mm`) are the same raw un-derated pair §2.5 indicts. F-BIPOLAR's re-open condition is unchanged and A-8 does not meet it |
| `mod.rs:1036` `effective_chip_thickness_mm: Some(0.04)` | test fixture field | left; the field still exists and is still consumed |

Also stale, **outside my territory, reported not touched**: `chipload.rs:103`
and `tool_load/mod.rs:370` both still name `routed_lookup_family`.

### 3.4 `SuggestAggressiveness` — census of a dial that is now inert

A-5i found `SuggestAggressiveness` became inert on the pre-simulation feed
path (it acted only through the retired pass-8 lift). The charter asked what it
now means anywhere the optimizer consumes or displays it.

**It appears nowhere in `optimize/**`.** Zero occurrences. The optimizer has
never had an aggressiveness dial and cannot express the Suggest side's
target-selection posture. The three superficially-matching hits are unrelated:
`candidate.rs`'s `modulation_aggressiveness: 1.0` (a `SimulationOptions`
field, now stamped) and two identical policy-rationale strings at
`policy.rs:218,300` that use the English word.

**Consequence for A-9 / the destination of Checkpoint J-1 disposition (c):**
the operator's aggressiveness control has no surviving expression on *either*
the pre-sim feed path or the optimizer path. Disposition (c) was ruled the
destination on the argument that the optimizer retargets from the measured
gate observation — §2 measures that route as structurally broken past
`DOC/Ø ≈ 1.67`, and this census shows it carries no aggressiveness input to
restore the dial through even if it worked. Both are inputs to A-9, not
findings A-9 has to rediscover.

### 3.5 Two provenance gaps found while stamping

* **`machine_snapshot` is written and never read.** Populated at
  `optimize/mod.rs:158` and asserted in one unit test; **no** consumer in
  `rs_cam_viz` or the MCP path reads it. F4.3's goal — "narratives stay
  reconcilable with the profile that actually bounded the search" — is met only
  for an agent reading raw MCP JSON. The GUI drops it. `assumptions` is
  serialized on the same wire and will have the same GUI gap until a renderer
  is added. **Reported; a GUI renderer is Checkpoint request 4.**
* **The optimizer takes no resolution parameter at all.** Neither
  `optimize_toolpath` nor `optimize_project` accepts one; both flags and both
  cells are hard-coded/policy constants with `auto_resolution: false`. So the
  optimizer *ignores* the resolution the user's on-screen baseline sim ran at,
  while ranking candidates against that baseline's cycle time. Nothing recorded
  which resolution produced which number; §1.1 now records both sides, including
  that one of them is unknowable (§1.4).

---

## 4. The modulation pin — assessment with evidence

**Site:** `candidate.rs:167`, `adaptive_feed_modulation: false`, justified by
the F-036b comment as ranking "against the commanded feed in the IR".

**What changed under it.** Checkpoint J (2026-08-13) flipped
`SimulationOptions::default().adaptive_feed_modulation` `false → true`.
Checkpoint K (g1) flipped the CLI flag from opt-in to opt-out to match. The
optimizer is now the **only shipped non-fixture site pinning `false`** — the
other is `cli smoke`, which pins it explicitly *because* it is a fingerprint
harness.

**Is the pin still right?** Evidence on both sides, honestly:

*For keeping it.* The stated reason was never "match the default" — it was
that the optimizer and the modulator are two independent corrections over the
same engagement summary and running them together conflates their effects.
That argument is untouched by J and K, and it got **stronger**: A-5 measured
`ConstrainedMax` rewriting **100 %** of moves and parking the observation
exactly on the band maximum. A candidate search whose every sample is already
clamped onto the ceiling has no headroom signal left to rank on — the
modulator would have absorbed the very differences the search is looking for.
A-5i's own numbers show the size of the absorption: median Δfeed −40.8 % and
−38.4 % on the two DropCutter fixtures, with modulation turning `Exceeds` into
`Within` at the gate without the denominator divergence going anywhere.

*Against keeping it.* The user-visible consequence is now real, not
theoretical. The GUI pins modulation `true` and the CLI now defaults `true`,
so the **same project** gets one verdict from the optimizer's candidate card
and a different one from the sim the operator is looking at, with no
reconciliation until Apply. That is precisely the review's `[medium]`, one
notch worse than when it was written.

**Assessment: KEEP the pin, and stop it being invisible — which is what this
wave shipped.** The isolation argument survives J and K on its merits; the
default flip did not refute it, it refuted the *comment*. A-8 corrected the
comment at the site (`candidate.rs:137-152`, which now says the premise is
stale and why the pin is nonetheless deliberate) and made the pin legible on
the wire via `CandidateSimAssumptions::adaptive_feed_modulation` +
`diverges_from_library_default_modulation()`.

**NOT FLIPPED.** Flipping is number-moving on every optimizer outcome and needs
an operator ruling. Checkpoint request 3 puts the question with its evidence.

The sentry
`the_optimizer_scores_candidates_at_a_different_operating_point_than_the_library_default`
asserts *both* halves — library default `true`, candidate pin `false`, and the
stamp says they disagree — so the day the ruling lands, the test fails and is
the place the decision is recorded.

`use_predicted_feed_in_gates: false` is a different case and the stamp keeps
them apart: it still agrees with the library default, so F-035's justification
holds unchanged. Rider worth stating: while it is off, the machine kinematics
model cannot reach any candidate verdict at all — which is why
`SimAssumptionStamp::kinematics` records the kinematics *source* rather than
implying a kinematics-aware score.

---

## 5. Verification

* `cargo test -p rs_cam_core --test optimizer_assumption_stamp_a8` — **5
  passed, 0 failed** (0.00 s).
* `cargo test -p rs_cam_core --lib retarget_reconciliation -- --nocapture
  --test-threads=1` — **2 passed, 0 failed** (0.65 s). Full transcript:
  `artifacts/a8/retarget_reconciliation.txt`.
* `cargo clippy -p rs_cam_core --all-targets -- -D warnings` — clean.
* `cargo fmt --check -p rs_cam_core` — clean. Formatted with `-p rs_cam_core`
  specifically to avoid the rustfmt cascade into `rs_cam_viz`, which B-5 is
  editing concurrently.
* `cargo test -p rs_cam_core --no-fail-fast`, **unbounded capture** (no
  `| tail`, no `-q` — A-5i's truncation defect is on the record) —
  `artifacts/a8/core_suite_after.txt`: 170 binaries, **2957 passed / 4
  failed**, and the four are **exactly the known red set**:
  `transform_provenance_fingerprints` ×3 (G-XFP) and
  `wanaka_suggest_integration::wanaka_suggest_baseline` (environmental).
  Nothing new went red.
* Red-first: `git show 1c0a4acc:crates/rs_cam_core/src/tool_load/optimize/outcome.rs
  | grep -c assumptions` → **0**. The stamp sentry is a compile error at the
  parent by construction.
* Slot discipline: bracketed `pgrep -af "carg[o]"` + `free -g` before every
  launch; one launch waited out a parallel `cargo test -p rs_cam_viz` and one
  waited out a parallel `param_sweep`. Disk ≥ 100 GB free throughout. No
  release build.
* `planning/airrun_2026-06-01/wanaka.toml` untouched and unstaged. B-5's
  `rs_cam_viz` and `session/compute.rs` edits never staged.

---

## 6. Checkpoint requests

All four are **number-moving or surface-moving**; none was executed.

### Request 1 — the chipload retargeter's target band *(the load-bearing one)*

The retargeter aims at `row_max / 1.2` while the gate judges against
`doc_derating_scale(DOC/Ø) × row_max`. Measured: identical retargeted feed for
a 3 mm and a 20 mm pass; **1.6667×** over the bar at `DOC/Ø ≥ 3`; the
retargeted candidate still `Exceeds`.

Options:

* **(1a)** Re-point the retargeter at `ChiploadMetric::bounds` — the derated
  band already on the verdict it is handed. Smallest change, one site, makes
  the low side consistent too. Moves every retarget outcome on every pass
  deeper than ~1.67 × Ø; retargeted feeds drop by up to 2× where the stage
  currently produces a rejected candidate.
* **(1b)** Keep the raw row and DOC-derate at the retargeter, duplicating
  `derate_chipload_bounds`. Rejected on sight — it is the two-instruments
  failure mode this programme keeps finding.
* **(1c)** Leave it and document that the retarget stage is inoperative past
  the knee. Cheapest; leaves a full generate + simulate burnt per optimize run
  on every roughing op.

Recommendation: **(1a)**, with the fixture's finding-arm inverted in the same
commit and the old/new numbers recorded.

### Request 2 — the six epsilon-bypass sites

`delta.rs:183,190,206,216` and `narrative.rs:683,703` re-decide gate verdicts
with bare comparisons and reach none of Checkpoint K's helpers. Collapsing the
two chipload pairs onto `ChipBounds::exceeds_high` / `::below_low` moves which
**tier** an outcome lands in (auto-recommend vs "verify on a scrap") for
candidates parked on the ceiling. Number-moving on a user-facing decision;
needs a ruling and a before/after.

### Request 3 — the modulation pin

Keep `adaptive_feed_modulation: false` on the candidate sim (A-8's assessment,
§4, with the evidence on both sides), now that it is disclosed rather than
invisible? Or flip it to track the library default and accept that the
modulator absorbs the search's own signal? Related, not folded in: the same
question for `use_predicted_feed_in_gates`, whose justification is currently
sound.

### Request 4 — does the GUI render the stamp?

`machine_snapshot` has been written and never read since F4.3. `assumptions`
is on the same wire and reaches MCP agents today via
`serde_json::to_value(&outcome)`. The review's repair was worded as *"make it
explicit in every optimizer result"* — a JSON field an agent can read is half
of that; the operator staring at the candidate card is the other half. A
renderer in `optimize_modal.rs` / `optimize_project.rs` is a visible-surface
change in **B-5's current territory**, so it is asked rather than taken, and
it carries §0 rule 3's screenshot obligation.

---

## 7. NOT FIXED / NOT EXERCISED, stated

* **NOT FIXED: the retargeter's band (§2.5).** Owner: Checkpoint request 1.
  Re-open condition: the ruling. Reproduction is permanent and executable.
* **NOT FIXED: the six epsilon bypasses (§3.2).** Owner: Checkpoint request 2.
* **NOT FIXED: the modulation pin (§4).** Owner: Checkpoint request 3.
* **NOT FIXED: `is_bipolar_engagement` (§3.3).** Ledger **F-BIPOLAR**, owner
  unchanged ("the optimizer pre-flight lane"). A-8 adds that its population is
  live and that it shares §2.5's raw-band defect, and meets none of its
  re-open conditions.
* **NOT EXERCISED: a rendered GUI surface.** §0 rule 3. The stamp is a wire
  field with no renderer, so there is nothing to screenshot yet; the moment a
  renderer lands (request 4) a screenshot is owed. The GUI was not launched
  this wave — B-5 holds the viz territory.
* **NOT EXERCISED: the retarget path through `optimize_toolpath` end-to-end.**
  §2 drives the retargeter through the *same construction* production uses, on
  a *real* sim verdict, and re-simulates — but it does not run the full search
  orchestration, so it does not observe how `build_outcome` finally presents a
  rejected retarget candidate to the user. Re-open condition: a wave with the
  budget for a full `optimize_toolpath` run on this fixture (the existing
  `optimize_smoke` harness is the shape).
* **NOT EXERCISED: the low/burn side of the retargeter.** F-MISSAE's advisory
  demotion on 176/252 rows makes a burn-side `Exceeds` hard to reach on shipped
  data; §2 deliberately measures the high side. The burn side additionally
  carries the RPM-down compensation branch (`retarget/chipload.rs:147-186`),
  which no fixture in this repo has ever executed against a real verdict.
  Owner: A-9 / F-MISSAE's decider.
* **NOT MEASURED: how many shipped operations sit past the `DOC/Ø ≈ 1.67`
  knee.** §2.6 bounds the question against the shipped
  `adaptive_doc_factor = 1.5` and names the classes that are outside, but no
  census of real projects was run. Owner: whoever rules request 1 — the
  population size is an input to *how urgent* it is, not to *whether* the
  defect is real, which §2.4 settles. Re-open condition: none needed.
* **NOT EXERCISED: `LutQueryStamp::Refused`.** The refusal arm (ProjectCurve on
  bull nose / V-bit / facing bit) is constructed and typed but no sentry drives
  it; the four routed cases are covered. Re-open condition: cheap to add on the
  same zero-sim refusal path.
