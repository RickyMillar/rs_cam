# Chip-thickness policy — evidence and checkpoint questions (TD3 wave A-9)

Date: 2026-08-13
Wave: TD3 Lane A, **A-9 research-only**
Ledger rows: **F-BIPOLAR**, **F-VALID**, **F-MISSAE**, plus axial-DOC floors
and the gate population predicate
Charter: `TECH_DEBT_3_RESEARCH_AND_FIX_PLAN.md` §2 "A-9 research-only" —
deliverable is an evidence document + checkpoint questions **ONLY**.
Implementation belongs to a future programme unless the operator rules
otherwise at this wave's checkpoint.

> The operator review's step 6 is binding on this wave: the chip-thickness
> policy investigation "owns axial DOC floors, bipolar engagement, and the
> gate population predicate; **it must not be folded into a cosmetic graph
> change**"
> (`planning/review_2026-08-04/FEEDS_SPEEDS_ARCHITECTURE_REVIEW_2026-08-07.md`,
> "Recommended sequence" item 6).

**Nothing behavioural changed in this wave.** One new test file was added,
`crates/rs_cam_core/tests/chip_thickness_policy_a9.rs`, whose every
assertion is a *reproduction of the current state*, not a bar.

---

## 0. The standing question, restated against the CURRENT baseline

The simulation measures an arc-mean chip thickness per sample — a real
engagement/force signal. Since the 2026-08-06 unit deletion, **nothing
gates on it**. The gate is engagement-blind by design, and the phrase "the
sim remains the operational arbiter of engagement" is true only in the
sense that nobody asks it.

This wave's questions must be posed against what TD3 Lane A has already
landed, not the pre-TD3 surface. The baseline, with citations:

| what landed | where | consequence for this wave |
|---|---|---|
| The gate observes `effective_feed ÷ (rpm · flutes)` — advance per tooth | `tool_load/chipload.rs:22-72`, `:762-772` | a chip-thickness policy would be a **new** consumer, not a restoration |
| **Checkpoint H3**: one chip-thickness visual survives — the sim-timeline track, relabelled "arc-mean chip thickness", **band shading removed** | `ORCHESTRATION_LOG.md:52`; executed `:279-280` | any re-banding of chip thickness needs *new literature*, not a new opinion |
| **H1-c killed on evidence**: converting the *band* to chip-thickness units is "**Not available**" — no wood chart in the shipped LUT publishes an engagement condition for its chipload column | `HEATMAP_VOCAB_CENSUS.md:372`, citing `CHIPLOAD_LITERATURE_VERDICT.md` §2.3 | the sourcing bar for question Q1 below is already known to be unmet by the shipped corpus |
| **Checkpoint J**: `arc_fit_ratio_for_op` retired, `adaptive_feed_modulation` default-**ON** | `ORCHESTRATION_LOG.md:74-78`, `:2103` | the DropCutter DOC-derate residuals are **absorbed** by modulation, not gone (`:2273-2277`) — see §4 |
| **Checkpoint K**: `ChipBounds` epsilon contract (`BOUNDARY_EPSILON_REL = 8.0 * f64::EPSILON`), `ceiling_advisory`, `lut_query_for` single routing | `ORCHESTRATION_LOG.md:82-86`; `tool_load/boundary.rs` | K touched the **operator** (`>` → epsilon-inclusive) of `is_bipolar_engagement`, **not its yardstick** (`:2828-2830`) |

And the standing instruction the resolution rider produced: **the sim cell
belongs beside every chipload verdict** (`ORCHESTRATION_LOG.md:2504-2507`),
because A-6 §3 measured the band a verdict is compared against drifting
**14.16 %** across a DOC sweep, with one identical feed of 437.4140 mm/min
reading `Within` at a coarse cell and `Exceeds(High)` at a fine one
(`LUT_BOUNDARY_EVIDENCE.md:280-311`).

---

## 1. F-BIPOLAR — where arc-mean chip thickness is still consumed

### 1.1 The census

Four production consumers of the arc-mean chip-thickness quantity survive.
Two make a *physical claim*; two are report-only.

| # | consumer | site | physical claim | survives the advance-per-tooth vocabulary? |
|---|---|---|---|---|
| 1 | `is_bipolar_engagement` | `tool_load/chipload.rs:265-297` | "engagement varies enough across one toolpath that no single feed/RPM scaling fixes both ends" | **claim yes, comparison no** — see §1.2 |
| 2 | `min_doc_chipload_floor` (the axial-DOC burn floor) | `feeds/cutter_constraints.rs:386-462` | "below this axial DOC the arc-mean chip collapses under the vendor `chipload_min` → rubbing/burn" | **no, and it has never fired** — see §1.3 |
| 3 | `display::arc_mean_chip_thickness` → sim-timeline track | `tool_load/display.rs:59-63`; `rs_cam_viz/src/ui/sim_timeline.rs:440` | none — H3 removed the band | **yes**, because it makes no comparison |
| 4 | trace summary + MCP export | `simulation_cut.rs:1107-1113`, `:1140-1152`; `rs_cam_viz/src/app/mcp.rs:5201-5202` | none — raw report | **yes**, with the caveat in §2.5 |

There is exactly one non-consumer worth naming: **the chipload gate itself
still reads the field**, but only as a *sample-validity predicate*
(`chipload.rs:760-766`). That is F-VALID and is §2.

**One correction to A-2's handover note.** `ORCHESTRATION_LOG.md:427-432`
records that `is_bipolar_engagement` "is now the **last** consumer of that
comparison in the repository". That is true for the *measured*
chip-thickness signal, and it is the sharper of the two statements — but
it is not the whole census. `min_doc_chipload_floor`
(`feeds/cutter_constraints.rs:386-462`) makes the same category of
comparison, chip thickness against an advance band, on a **predicted**
chip rather than a simulated sample, in `feeds/` rather than `tool_load/`.
The operator review named it first of its three locations. It is included
here because step 6 assigns it to this wave, and because §1.3 shows it is
dead — which is the reason nobody found it by following the measured
signal.

### 1.2 Consumer 1 — `is_bipolar_engagement`

The predicate compares each sample's raw `effective_chip_thickness_mm`
against `cl_min`/`cl_max` — a vendor **advance** band. Its own docstring
(`chipload.rs:231-256`) already states the position this wave endorses:

> the samples are the right quantity and the **bounds** are the wrong
> yardstick for them.

The reasoning for *not* transferring the deletion holds and this wave
found nothing against it: the gate's observation could become an advance
per tooth because the sample's own arc cancels out of the normalisation
(`cl_norm = fz · f(arc_sample) · f_lut / f(arc_sample) = fz · f_lut`,
pinned by `tests/feed_explanation_snapshot_b3.rs`). This predicate is
*about* that arc. Re-expressing it in advance per tooth would leave it
reading only the kinematic feed map and go near-vacuous.

**Consumer and blast radius.** One call site: the optimizer pre-flight
classifier, `tool_load/optimize/preflight.rs:102-111`, producing
`RefuseReason::BipolarEngagement` with a per-op-family prescription
(`optimize/refusal.rs:73-101`). It is a **refusal**, not an advisory —
when it fires the optimizer declines to search at all.

**Two properties that matter for policy, both measured here:**

- **A vacuous chip population silently means "not bipolar."**
  `chipload.rs:291-295` returns `false` when `total == 0`, which is
  indistinguishable from a measured non-bipolar toolpath. Every emptying
  path in §2 therefore reads as an exoneration. This is the X-VAC shape
  §0 rule 4 names, on a refusal.
- **K fixed the operator, not the yardstick.** The comparison now uses
  `boundary::below_low` / `exceeds_high` at zero tolerance
  (`chipload.rs:279-287`), so the *boundary semantics* are correct while
  the *quantities being compared* are still a chip thickness against an
  advance band. The code comment says so explicitly. Do not read K as
  having addressed F-BIPOLAR.

### 1.3 Consumer 2 — the axial-DOC chipload floor. **It never fires at any shipped default stepover, and the one place it does fire is above every default.**

This is the wave's largest new finding, and it is a dead-at-the-defaults
finding, not a wrong-number finding.

> **Corrected by measurement, 2026-08-13.** The first draft of this
> section and of the probe both said "structurally unreachable". The
> probe's first run **falsified that** on one row and the claim was
> narrowed rather than the test loosened. What is stated below is what
> the run produced.

`min_doc_chipload_floor` (`cutter_constraints.rs:386-462`) binary-searches
the minimum axial DOC at which a *predicted* arc-mean chip thickness meets
the matched row's `chipload_min`. It applies only to ball / tapered-ball
(flat / bull / V-bit return `None` at `:400-406`). Its result reaches
Suggest as `min_doc_chipload_floor_mm`, and through it
`AxialBindingConstraint::SafeBandEmpty`,
`SuggestWarning::AxialEnvelopeSafeBandEmpty` (`feeds/suggest.rs:1364-1390`)
and `SuggestWarning::AxialDocBelowBurnFloor` (`:1408-1416`).

**Measured (this wave, Python transcription of `chip_at` verified
line-by-line against `cutter_constraints.rs:409-430`, then re-run through
production code by the probe in
`crates/rs_cam_core/tests/chip_thickness_policy_a9.rs`):**

**(a) The function is unimodal in `ap`, and the docstring's monotonicity
claim is false.** The docstring at `:404-407` says "As ap grows,
engaged_radius grows, the radial_woc_mm covers a smaller arc fraction, and
the arc-mean chip thickness **rises monotonically**". It does not. For a
Ø6 ball (R = 3), stickout 30, `radial_woc` 0.5 mm, `fz` 0.10:

| ap (mm) | r_eng | arc (rad) | chip (code) |
|---:|---:|---:|---:|
| 0.003 | 0.1341 | 3.14159 | **0.000000** |
| 0.020 | 0.3458 | 2.27104 | 0.038947 |
| 0.0265 | — | 1.9713 | **0.041825 ← peak** |
| 0.100 | 0.7681 | 1.02250 | 0.021344 |
| 1.000 | 2.2361 | 0.35124 | 0.003013 |
| 3.000 | 3.0000 | 0.26180 | 0.001692 |
| 28.500 | 3.0000 | 0.26180 | **0.001692 (plateau)** |

The peak sits at arc ≈ 1.9713 rad with a factor of **0.4183 × fz** —
which is exactly the value the A-1 fixture already pinned as "peaks near
arc ≈ 2.0 rad (≈ 0.418)" (`HEATMAP_VOCAB_CENSUS.md:255-270`). The
monotone-increasing assumption holds only when `arc` at `r_eng = R` is
still above 1.9713 rad, i.e. **`radial_woc ≥ 0.6275 D`**. Every shipped
call site is below that (see (c)).

**(b) The bracket's upper end is on the plateau, which is the MINIMUM of
the function over the bracket.** `BINSEARCH_UPPER_FRACTION = 0.95`
(`:81`) puts the upper bracket at `stickout × 0.95` — 28.5 mm on a 30 mm
stickout — far past the point where a ball's `width_at_height` saturates
at R (`tool/ball.rs:77-84`). So the guard at `:433-437`
(`if chip_hi < chipload_min_mm_per_tooth { return None }`) compares the
*smallest* chip over the bracket against `cl_min`, and fires.

**(c) Consequence: `None` on 0 of 24 shipped rows × every shipped
stepover.** The shipped LUT carries 24 distinct (diameter, chipload band)
triples across `ball_nose` + `tapered_ball_nose`
(`data/vendor_lut/observations/`; 36 rows, 32 with a `chipload_min`).
The three radial-WOC fractions any shipped call site can reach on a
ball-tipped tool are `feeds/suggest.rs:1545` (3D-finish family, 0.15 D),
`:1525` (ProjectCurve, 0.20 D) and `:1482` (Adaptive3d, 0.40 D). With
`fz` set to each row's **own chipload MAXIMUM** — the most aggressive
advance still inside the vendor band:

| stepover | rows where the floor is non-`None` | feed required, as a multiple of the row's own band MAX |
|---|---|---|
| 0.15 D (3D finish) | **0 / 24** | 7.0× – 18.8× |
| 0.20 D (ProjectCurve) | **0 / 24** | 4.1× – 10.9× |
| 0.40 D (Adaptive3d) | **0 / 24** | 1.3× – 3.5× |
| 0.6275 D (geometric optimum, above every shipped default) | 2 / 24 | 1.0× – 2.4× |
| 1.0 D (full slot) | **0 / 24** | ~10¹⁵× — see (d) |

At every stepover a shipped call site **defaults** to, the burn floor can
only produce a value when the operator is already running far **above**
the row's chipload maximum — a condition the chipload gate itself reports
as `Exceeds(High)`.

**Confirmed through production code**, not the transcription:
`crates/rs_cam_core/tests/chip_thickness_policy_a9.rs` drives
`lookup_best` on the embedded LUT and then `cutter_axial_constraints`.
Over **36** (row × shipped-default stepover) combinations with `fz` at
each row's band maximum, `min_doc_chipload_floor_mm` is `None` in every
one, `safe_band_is_empty()` is `None` in every one, and
`binding_constraint` is never `SafeBandEmpty`.

**But the bound is dead at the defaults, not dead code.** At 0.6275 D the
probe reaches it:

```text
A-9 probe: reachable @ 0.6275 D — ball 1.0 softwood floor=0.23259 safe_band_empty=true
```

That is the `amana-ball-softwood-parallel-1000-2f-zrn` row — chipload band
0.0191 – 0.0508 mm/tooth, a 2.66× band, the widest ball band the LUT
carries — and it lands as `SafeBandEmpty`, i.e. the state Suggest surfaces
as `AxialEnvelopeSafeBandEmpty`. Two of the 24 distinct rows clear the
analytic bar at that fraction; the probe's eight ball queries contain one
of them.

Two of the three call sites take the operation's own stepover when it is
set (`radial.unwrap_or(...)`, `feeds/suggest.rs:1482`, `:1545`), so an
operator-set Adaptive3d stepover above ~0.63 D **can** reach this. The
honest statement is therefore: *the bound is unreachable at every default,
reachable only above them, and on 22 of 24 rows not reachable at all.*

**(d) The floor's chip model omits the `arc ≥ π` branch its own comment
cites.** `cutter_constraints.rs:424-428` says "Same closed-form mean as
`flat_chip_geometry_for_radius`" and writes
`h_max = feed_per_tooth_mm * arc.sin().abs().max(0.0)`. The canonical
function (`tool/mod.rs:125-130`) writes
`if arc >= PI { feed_per_tooth_mm } else { fz * arc.sin().abs() }`. At a
full slot the canonical model reports `0.6366 × fz` (the A-1 fixture's
"0.637 at a full slot"); the floor's copy reports `fz · sin(π)` ≈
`1.2e-16 × fz`, a hard zero. A slotting ball therefore gets **no floor at
all** for a reason that is a transcription omission.

**(e) The X-VAC shape is `None`, and the docstring claims the opposite
outcome.** `:433-437` says returning `None` is how the case "there is no
axial that satisfies the floor" reaches the consumer, "via SafeBandEmpty
downstream". It does not: the consumer at `:236-240` is
`if let Some(floor) = min_doc_chipload_floor_mm && floor > safe_max`, so
`None` skips it entirely and the binding stays Deflection / VendorAp /
Scallop. `safe_band_is_empty()` (`:153-159`) returns `None`, not
`Some(true)`. **The one case the model claims to report is exactly the
case it reports as "not measured."**

**(f) The existing test is vacuous and says so.**
`safe_band_empty_detected_on_chipload_floor_above_max`
(`cutter_constraints.rs:740-777`) accepts `Some(true)`, `Some(false)` and
`None` alike; its comment reads "The assertion that matters is that
SafeBandEmpty **CAN** be triggered, not that this specific case does" —
and it does not assert that either. Until this wave, nothing in the suite
had ever observed this bound produce a value; the probe's
`the_axial_floor_only_becomes_reachable_above_every_shipped_default` is
the first test that does.

---

## 2. F-VALID — the validity conditions of the measurement itself

`chip_thickness_stats` (`dexel_stock/simulation.rs:672-693`) is the
**only** producer. Three lines decide everything the signal can be:
`:679` `let arc = arc_engagement_radians?;`, `:681-687` the
`cutter.chip_geometry(...)` call, and `:688` `.ok()` — **every
`EngagementError`, both `Unsupported` and `OutOfRange`, is discarded with
no reason retained.** The cutters build informative reason strings; no
consumer ever sees one.

### 2.1 Thirteen silent emptying paths, in three tiers

**Tier 1 — trace-wide (1 path).**

| condition | site | what it drops |
|---|---|---|
| `capture_arc_engagement == false` | `stamping.rs:740-753`; flag `simulation_cut.rs:12-17`, `#[derive(Default)]` → **`false`** | **every chip-thickness reading in the whole trace.** `compute/simulate.rs:1391,1418` construct it `false`; the viz layer force-enables it (`rs_cam_viz/src/controller/events/simulation.rs:247`, `ui/sim_op_list.rs:68`, `app/mcp.rs:3911`). A headless/metrics-off sim yields a 100 %-vacuous chip population, marked only by `SimulationProvenance::captured_arc_engagement` |

**Tier 2 — production, per sample (7 paths).**

| # | condition | site | note |
|---|---|---|---|
| 2 | non-cutting emitter (rapids, retract-feed linears) | `stamping.rs:820-823`; `simulation.rs:187-238` | `effective_chip_thickness_mm: None` hard-set |
| 3 | degenerate / pure-vertical segment | `stamping.rs:513`, return `:582` | **returns `radial = 1.0`** when volume was removed, so the sample survives every air-cut filter and *then* dies on the chip predicate |
| 4 | `CutKinematics::Plunge` forces `axial_engagement_mm = 0.0` | `simulation.rs:474-479` → `tool/mod.rs:115` `OutOfRange` | a **second, independent** kill for the same samples as #3 |
| 5 | **fresh-material floor** `FRESH_MATERIAL_THRESHOLD_MM = 0.05` | `stamping.rs:215`, applied `:702` | gates `radial_engagement`, from which the arc is derived — so it gates chip thickness **indirectly**, not directly. A 0.02 mm spring pass removes real material and produces **zero** chip samples |
| 6 | **perp-coverage gate** `PERP_COVERAGE_GATE = 0.95` | `stamping.rs:181`, applied `:702` | with `COVERAGE_SUBSAMPLES_PER_AXIS = 4` (`stamping.rs:29`) this is effectively `coverage == 1.0` |
| 7 | cutter declines the geometry (4 shape guards) | see §2.2 | the largest reach; see §2.3 |
| 8 | `max_penetration` accumulates only when `removed_here > 1e-6` | `stamping.rs:715-718` | a re-cut over cleared ground reports `axial_doc = 0.0` → `OutOfRange` |

**Tier 3 — consumption, per sample (5 paths), if a future gate reuses the
existing filter stack.**

| # | filter | constant | site |
|---|---|---|---|
| 9 | `!is_cutting` | — | `chipload.rs:311-317` |
| 10 | air-cut | `radial_woc_fraction < 0.02` | `chipload.rs:311-317` (same 0.02 as `SimulationCutIssueKind::AirCut`, `simulation_cut.rs:877`) |
| 11 | steady-state feed | `STEADY_STATE_FEED_FRACTION = 0.95` | `chipload.rs:206`, applied `:306`, `:318` |
| 12 | phantom transit | — | `chipload.rs:806-808`; `locality.rs:194-217` |
| 13 | configured entry | — | `chipload.rs:809-826`; `locality.rs:238-245` |

Note on 10: because the arc is `acos(1 − 2·radial)` (`stamping.rs:741-747`),
the air-cut filter and the arc are **correlated, not independent** — the
chip signal inherits every pathology of the radial channel.

Note on 12/13: with `span_lookup == None`, `is_phantom_transit` degrades
to bare `sample.in_transit_span` and `is_configured_entry` returns `false`
(`locality.rs:216`, `:241-243`) — every transit sample is dropped with no
Entry split.

### 2.2 Which cutter shapes support `chip_geometry` (guards verbatim)

Universal guard, all five shapes — `tool/mod.rs:114-123`:

```rust
if radius <= 0.0 || axial_doc_mm <= 0.0 || arc_engagement_radians <= 0.0
    || feed_per_tooth_mm < 0.0 || flute_count == 0
{ return Err(EngagementError::OutOfRange { reason: "non-positive engagement inputs" }) }
```

Note the asymmetry: `feed_per_tooth_mm` is rejected only if **strictly
negative**; `0.0` is accepted (see §2.5).

| shape | guard | site | reason string |
|---|---|---|---|
| `FlatEndmill` | *(none)* | `tool/flat.rs:46-62` | only `OutOfRange` reachable |
| `VBitEndmill` | `axial_doc_mm <= 0.0` | `tool/vbit.rs:87-92` | "tip-only engagement — chip area undefined at single-point contact" |
| `BallEndmill` | `axial_doc_mm >= self.radius()` | `tool/ball.rs:49-53` | "engagement past hemisphere pole — chip thickness varies along the engaged arc and is not modeled" |
| `BullNoseEndmill` | `axial_doc_mm <= self.corner_radius` | `tool/bullnose.rs:76-80` | "toroidal engagement at corner not modeled" |
| `TaperedBallEndmill` | `(axial_doc_mm − h_contact()).abs() <= 0.05` | `tool/tapered_ball.rs:126-131` | "ball/cone transition zone" |

**Bullnose is the worst.** A 6 mm bull with a 1 mm corner radius is
chip-blind for **every** DOC ≤ 1.0 mm — i.e. the entire finishing regime.
Ball is chip-blind at DOC ≥ its own radius. Tapered ball carries a
**±0.05 mm bare-literal dead band** around the ball/cone transition
(`tapered_ball.rs:127`), numerically equal to `FRESH_MATERIAL_THRESHOLD_MM`
without being related to it.

There is **no drill cutter type implementing `MillingCutter`** — drill ops
are excluded upstream (`chipload.rs:465-472`), not by a cutter refusal.

### 2.3 What `sim_measurability` already covers, and the gap

`SimMetric::ChipEngagement` **exists and is already wired**
(`sim_measurability.rs:87-95`), and is a member of `ENGAGEMENT_DERIVED`
(`:121-125`) receiving the same verdict as `RadialEngagement` / `AirCut`
(`:337-344`). Its two `MeasurabilityReason` conditions are
`BelowFreshMaterialFloor` and `CellTooCoarseForTipContact` (`:157-192`),
with `NOT_MEASURABLE_BLIND_FRACTION = 0.5`, `DEGRADED_BLIND_FRACTION = 0.1`
(`:288`, `:292`). So **a chip-thickness gate would get abstention for
free — for two of the thirteen paths.**

The gap is precise and load-bearing. `classify_engagement`'s blindness
predicate is `s.engagement.radial_woc_fraction > 0.0 { continue; }`
(`sim_measurability.rs:414`) — *not* "chip thickness is `None`". It
therefore catches Tier-2 #5 and #6 and **nothing else**. A bullnose
finishing at DOC ≤ its corner radius has a healthy non-zero
`radial_woc_fraction` on every sample, so `classify_engagement` returns
`Measurable` while **100 % of the chip population is `None`**. Same for a
metrics-off trace (Tier 1), ball-past-pole, and the tapered-ball dead
band.

Two further optimistic defaults: `if removing == 0 { return Measurable }`
(`:407-409`), and `MeasurabilityReport::for_metric` returns `Measurable`
for a toolpath with no row (`:357-364`, documented as intentional).

The variant's docstring also currently says "**NOT the chipload gate** …
an abstention here does not and should not disarm it" (`:89-94`). A new
arc-mean-chip gate would be the first consumer that genuinely *should* be
disarmed by `ChipEngagement`, so the mechanism is free but the wording is
not.

### 2.4 The empty population renders as a clean pass

If Tier-3 #12/#13 remove every remaining sample while `valid_count > 0`,
`burn_samples` is empty (`chipload.rs:828`), `median_sample` is `None`
(`:930-937`), `peak_below` is `None` (`:944-951`), and the gate returns
**`ChiploadVerdict::Within` with `approach_to_min: None`** and an
`approach_to_max` built from `peak_in_range = (0.0, 0)` with
`SampleEvidence::empty()` (`:996-1019`). A fully vacuous population
renders as a clean "Within", not an abstention — the exact X-VAC shape
§0 rule 4 forbids trusting.

Compounding it, `valid_count` is **published as `sample_count`** (`:895`)
and deliberately overstates the true denominator, because it increments
before the transit skip; the code says so verbatim at `:776-799`.

And the refusal reason when `valid_count == 0` is a **plurality vote, not
a truth** (`:902-921`): three mutually exclusive per-sample failure modes
are reduced to whichever counter is largest, with ties breaking toward
`ArcEngagementNotCaptured`. A trace that is 49 % missing-arc / 51 %
unsupported-cutter reports only the second.

### 2.5 Hard zeros dressed as measurements

- **`spindle_rpm == 0` with `flute_count > 0`.** `stamping.rs:835-845`
  returns `chipload_mm_per_tooth = 0.0`, which becomes
  `feed_per_tooth_mm` at `simulation.rs:496`. The universal guard rejects
  only `< 0.0`, so `0.0` passes, `h_max = 0.0`, `mean = 0.0`, and the
  sample carries `effective_chip_thickness_mm = Some(0.0)`. It passes the
  validity predicate, counts in `valid_count`, lands in `burn_samples`
  and in `is_bipolar_engagement` as a below-min reading, and renders on
  the timeline track. `no_divisor_count` catches this for the *advance*
  observation but not for the chip value.
- **The asymptotic collapse is real, guaranteed, and indistinguishable
  from the fake zero.** `chipload.rs:322-331` states it verbatim: the
  arc-average collapses to 0 as `arc → 0` by definition, and "on a real
  toolpath there are always *some* low-arc transient samples … but they
  aren't *rubbing* in the burn-risk sense". The existing gate mitigates
  only by taking a **median**. Any new chip-thickness gate must re-derive
  that mitigation, and a mean/min statistic would be dominated by these.
- **Zero → `None` inversion at summary level.** `simulation_cut.rs:1148-1152`
  reports `peak_chip_thickness_mm: None` when the peak is `0.0`, while
  `average_mean_chip_thickness_mm` (`:1140-1147`) uses a runtime
  denominator and reports `Some(0.0)` for the same toolpath — an
  internally inconsistent pair, exported as-is over MCP
  (`rs_cam_viz/src/app/mcp.rs:5201-5202`).
- **Fabricated values in `src/`, not `tests/`**: `tool_load/deflection.rs:460`
  and `tool_load/power.rs:445` both build
  `effective_chip_thickness_mm: Some(feed_mmpm / (18_000.0 * 2.0))` — a
  hardcoded 18 000 RPM / 2-flute assumption; `narrate.rs:2345` builds
  `Some(0.0)`.
- **`EngagementMode::Slot` is hardcoded** at the one producer
  (`simulation.rs:686`). No shipped `chip_geometry` impl reads `mode`
  (all five bind `_mode`), so it is inert today — but it means the sim's
  chip signal is climb/conventional-blind by construction.

---

## 3. F-MISSAE — what an engagement-aware LOW-side policy would need

### 3.1 The current mechanism, measured

`low_side_is_advisory()` (`tool_load/verdict.rs:656-671`) keys on the
`ChipBoundsSource` variant **only** — nothing about the operation, the
engagement, or the observation:

```rust
ChipBoundsSource::VendorLut => false,
ChipBoundsSource::VendorLutExtrapolated
| ChipBoundsSource::VendorLutPointPreset
| ChipBoundsSource::VendorLutMissingAe => true,
```

A demoted low side becomes `ChiploadVerdict::Within { burn_advisory: Some(..) }`
(`chipload.rs:958-993`). The `ChiploadFeedRetargeter` matches only the two
`Exceeds` arms and returns `None` for everything else
(`optimize/retarget/chipload.rs:74-88`). **So on a demoted row the burn
side produces a verdict of `Within`, an advisory string, and no optimizer
action whatsoever.**

The advisory *is* rendered — `diagnostics/adapters/from_tool_load.rs:270-277`
prints "Observed feed-per-tooth *x* mm is BELOW the *y* mm/tooth burn
floor — not refused because the floor's provenance is *row_id* (advisory
only)". It is `Severity::Info` on `LOAD_CHIPLOAD_WITHIN` by deliberate
choice (`:246-252`).

### 3.2 The population — and a correction to the ledger's headline number

The ledger and A-5 both quote **176 / 252**
(`TECH_DEBT_2_CLOSEOUT.md:740`; `ARC_FIT_RATIO_EVIDENCE.md:542`). That is
the count of shipped LUT rows carrying neither `ae_min_mm` nor
`ae_max_mm`, and it is correct as a row property — verified this wave by
direct census of `data/vendor_lut/observations/`: **176 of 252**.

It is **not** the number of rows `VendorLutMissingAe` is responsible for
demoting, because the classification at `chipload.rs:615-627` is
**worst-first**: point-preset, then extrapolated, then missing-`ae`.
Censused on the same corpus, for an exact (non-extrapolated) match:

| classification | rows | low side |
|---|---:|---|
| no usable chipload bounds (refused before classification) | 17 | — |
| `VendorLutPointPreset` (raw `min >= max`) | **48** | advisory |
| `VendorLutMissingAe` (has a real band, no `ae`) | **114** | advisory |
| `VendorLut` | **73** | **hard `Exceeds(Low)`** |
| total with both chipload bounds | 235 | 162 advisory / 73 hard |

So: **retiring `VendorLutMissingAe` would promote 114 rows, not 176** —
the other 62 are already demoted by `VendorLutPointPreset` or carry no
usable band, and would stay demoted. And **69 % of chipload-bearing rows
(162 / 235) have an advisory-only burn side** before any extrapolation is
considered; extrapolation is query-dependent and can only raise that.

`ae_rule` is worth reading before any ruling: of the 76 rows that *do*
carry an `ae` window, the rules are repo-authored application windows —
19 × `"scallop driven"`, 8 × `"scallop driven; not specified by source"`,
8 × `"width-at-depth"`, and a long tail of `"10% to 30%D"`-style
stepover bands. This is the same fact that killed H1-c: **`ae` in this
crate is a stepover recommendation, not a vendor measurement condition**,
so the presence or absence of `ae` is evidence about *annotation
completeness*, not about the chipload column's calibration.

### 3.3 Why "no burn problem has been observed" is not evidence

Both routes that could produce burn-side evidence are blocked:

- the **retargeter** never sees a burn trip on 162 / 235 rows (§3.1);
- the **axial burn floor** has never produced a value on any shipped ball
  row (§1.3).

A-5 recorded the first of these as its reason for measuring disposition
(c) through the modulator instead (`ARC_FIT_RATIO_EVIDENCE.md:309-316`,
`:542`), and A-8 re-states it (`OPTIMIZER_ASSUMPTIONS.md:174-176`,
`:556-561`). **Absence of burn evidence in this repo is a property of the
instrument, not of the machining.**

### 3.4 Option space for a low-side policy (options, not a design)

Ordered by how much new sourcing each needs. None is recommended here.

| # | option | what it needs | what it costs |
|---|---|---|---|
| **L0** | **Do nothing; keep demotion, retire the justification's wording only** | nothing — already done in code (`chipload.rs:605-614`) | the label keeps meaning "weakest-annotated row class", which §3.2 shows is about annotation, not calibration. Conservative direction; status quo |
| **L1** | **Retire `VendorLutMissingAe`, keeping `PointPreset` and `Extrapolated`** | a ruling that absence of a repo-authored `ae` window is *not* evidence about the chipload column | promotes **114 rows** to hard `Exceeds(Low)`. Behaviour change with a real blast radius; needs a fixture census of which shipped projects flip |
| **L2** | **Keep the demotion but make it actionable**: let the retargeter fire on `Within { burn_advisory }` as a *soft* target (feed-up suggestion, not a refusal) | no new physics — the retargeter's arithmetic is already `target/observed` | the optimizer starts moving feeds on 69 % of rows using a floor the repo itself calls weakly provenanced. Bounded by making it advisory-ranked rather than gating |
| **L3** | **Raise severity without changing the verdict**: `burn_advisory` promotes the diagnostic from `Info` to `Caution` on `LOAD_CHIPLOAD_WITHIN` | a product decision about badge counts (`from_tool_load.rs:249-252` says so) | moves counts on every surface; no physics claim |
| **L4** | **An engagement-aware low side** — the burn condition is rubbing, which is a *chip thickness* question, not an advance question | a sourced chip-thickness floor. **The shipped corpus does not contain one** (`CHIPLOAD_LITERATURE_VERDICT.md` §2.3), which is why H1-c was ruled unavailable | needs new literature or a bench trial. This is the honest home of the burn question and it is currently unsourced |
| **L5** | **Disclosed derived scaling**: keep an advance band but scale it into a chip-thickness envelope by an explicitly repo-authored factor, declared as such in code and `CREDITS.md` | an operator ruling that a derived factor is admissible here, matching the precedent already set for `D^0.61` / `Janka^-0.5` and the drill thresholds ("correct the citation, keep the number") | a third repo-authored law in the feeds stack. Must never be cited to a vendor |

L4 and L5 are the only two that make F-BIPOLAR's yardstick correct as
well; L0–L3 leave §1.2 exactly where it is.

---

## 4. Axial-DOC floors + the gate population predicate: interaction with a future policy

Four interactions, each with a mechanism.

**4.1 The DOC-derate divergence (F-3 / C-2 / C-5) is absorbed, not fixed.**
Checkpoint J-4 ruled the two DropCutter residuals (1.69× / 1.62×) a
separate row, and A-5i recorded that post-J `Within` 4/4 "does **not**
mean the denominator divergence is gone" — modulation absorbs it at the
gate (`ORCHESTRATION_LOG.md:22`, `:2273-2277`). A chip-thickness policy
inherits this directly: the axial DOC that would feed any chip model is
the same quantity whose derate denominator is divergent, so **a new gate
would sit on top of an absorbed error rather than a resolved one.**

**4.2 The band a verdict compares against is resolution-dependent — and a
chip model would be doubly so.** A-6 §3 measured the *advance* band
drifting 14.16 % across a DOC sweep, because the gate queries the LUT at
`lookup_diameter_at(peak steady-state axial DOC)` and peak axial DOC is a
dexel measurement (`LUT_BOUNDARY_EVIDENCE.md:282-299`). A chip-thickness
observation adds a **second** cell-size dependence on the observation side
— the perp-coverage gate is explicitly a function of cell size, with the
closed form stated at `stamping.rs:173-180`
(`cell ≲ √(2·R_tip·d − d²)`), plus a systematic downward truncation bias
of roughly one sub-sample of cell per side (`:167-169`; a true full slot
reads ≈ 0.97, not 1.0). **Both sides of the comparison would move with the
cell**, in uncontrolled directions. The standing instruction — record the
cell beside every verdict — becomes a hard requirement, not a discipline.

**4.3 The axial floor and the gate population predicate share a root
cause and a fix shape.** Both are pre-2026-08-06 chip-thickness policies
left standing for good reasons. Both are `None`-shaped rather than
`Some(0)`-shaped, so both read as "not measured" while presenting as
healthy. And both would be resolved *in the same direction* by any answer
to Q1 below — a chip-thickness envelope makes the axial floor
dimensionally sound and makes the gate's chip predicate meaningful again;
a ruling that nothing should gate on chip thickness retires both.

**4.4 Widening the gate predicate is a population change, and the
population is the axial-DOC-blind one.** `chipload.rs:743-759` keeps the
predicate byte-identical so the 2026-08-06 flip table is attributable to
one cause. Widening it (dropping the `effective_chip_thickness_mm.is_none()`
skip, since the observation is now purely kinematic) would admit exactly
the samples §2.2 lists: bullnose below the corner radius, ball past the
pole, tapered ball in the dead band, plunge samples, and everything a
metrics-off trace produced. Those are **real cutting samples with a valid
advance per tooth**, currently excluded for a chip-model reason that no
longer bears on the quantity. It is the right change *for the advance
gate*, it needs its own before/after, and it **must be sequenced against
any chip-thickness decision** — because if a chip gate is added, the same
samples need to be excluded again, for the newly-valid reason.

---

## 5. Checkpoint questions for the operator (TD4 intake)

Ruling any of these does not commit this programme to implementing them —
that is the next programme's charter. What is needed here is direction.

**Q1 — Should anything gate on arc-mean chip thickness again, and under
what sourcing bar?**
The known facts: no vendor in the shipped LUT publishes an engagement
condition for its chipload column (`CHIPLOAD_LITERATURE_VERDICT.md` §2.3);
that fact killed H1-c and made the 2026-08-06 correction a deletion rather
than an inversion; and the repo has twice ruled "correct the citation,
keep the number" for repo-authored constants (drill thresholds, the two
chipload scaling exponents). Options:
(a) **nothing gates on chip thickness** — retire the axial floor and the
bipolar comparison, keep the unbanded visual;
(b) **gate only on a sourced envelope** — leaves everything as-is until
literature or a bench trial exists;
(c) **gate on a disclosed repo-authored envelope**, declared in code and
`CREDITS.md`, never cited to a vendor.
Sub-question: if (c), does the standing prohibition on comparing a chip
thickness to an advance band admit an explicitly-disclosed conversion
factor, or does it forbid the comparison in any form?

**Q2 — The axial-DOC chipload floor is dead at every default. Retire,
repair, or ledger as-is?**
Measured through production code: `None` on 36 / 36 (row × shipped-default
stepover) combinations even at the band maximum, and `SafeBandEmpty`
never binding there (§1.3c) — but **reachable** at ~0.63 D on the widest
shipped ball band, where it produces `floor = 0.23259` and
`safe_band_is_empty() == Some(true)`. Two of the three call sites accept
an operator stepover, so this is reachable in production, just never by
default. Also measured: the docstring's monotonicity claim is false
(§1.3a); the model omits the `arc ≥ π` branch it cites (§1.3d); the one
case it claims to report is the case it reports as unmeasured (§1.3e);
its only test accepts every outcome (§1.3f). Options: (a) delete the bound
and its two Suggest warnings; (b) repair the transcription and bracket and
re-measure what then fires; (c) keep as-is and ledger it. Note (b) is
**not** a safe default: repairing it converts a bound that today fires on
one exotic configuration into one that clamps `depth_per_pass` on ball
finishing generally, and it would still be comparing a chip thickness
against an advance band unless Q1 is answered first.

**Q3 — F-MISSAE: is the absence of a repo-authored `ae` window evidence of
anything?**
The correction this wave adds: retiring `VendorLutMissingAe` promotes
**114** rows, not 176 (§3.2), and `ae` in this crate is a stepover
*recommendation* (19 × "scallop driven", 8 × "width-at-depth", …), not a
vendor measurement condition. Options L0–L5 in §3.4. The narrower question
the operator can rule cheaply: **may the retargeter act on a
`burn_advisory` as a soft target (L2) without the verdict changing?**

**Q4 — May the gate's vestigial sample predicate be widened, and when?**
Dropping the `effective_chip_thickness_mm.is_none()` skip admits real
cutting samples the advance gate can legitimately judge (§4.4), and would
be the right change for the advance gate. It is a population change
needing its own before/after. Should it be sequenced **before** Q1 (fix
the advance gate now, revisit if a chip gate lands) or **after** (avoid
moving the population twice)?

**Q5 — Does a chip-thickness policy require the DOC-derate divergence
(F-3 / C-2 / C-5) to be resolved first?**
Modulation currently absorbs the DropCutter residuals rather than fixing
them (§4.1), and a chip observation would add a second cell-size
dependence on top of the band's existing 14.16 % drift (§4.2). Is
resolving the denominator a prerequisite for any chip-thickness gate, or
may a chip gate ship with the divergence documented and the cell recorded
beside every verdict?

**Q6 — Does `SimMetric::ChipEngagement`'s detector need widening
regardless of Q1?**
Today `classify_engagement` keys on `radial_woc_fraction <= 0.0` and
therefore reports `Measurable` for a bullnose finishing below its corner
radius, a ball past the pole, a tapered ball in its dead band, and a
metrics-off trace — while 100 % of the chip population is `None`
(§2.3). The unbanded timeline visual (H3) and the MCP export already
present that population. This is a reporting-honesty question independent
of whether anything gates.

---

## 6. NOT EXERCISED / NOT MEASURED — with blockers

Stated plainly rather than glossed.

| item | status | blocker / why |
|---|---|---|
| `is_bipolar_engagement` exercised end-to-end on a real project | **NOT EXERCISED** | `pub(crate)`; unreachable from an integration test. Its behaviour here is read from source + its unit tests, not run |
| A live measurement of how often `RefuseReason::BipolarEngagement` fires in practice | **NOT MEASURED** | needs an optimizer run over a project corpus; out of a research-only wave's budget, and the cargo slot was occupied by A-8 for most of it |
| Whether the axial floor is reachable via a **non**-LUT-row path | **NOT EXERCISED** | `cutter_axial_constraints` takes `lut_row: Option<&LookupResult>`; a caller could synthesise one. No shipped caller does (`feeds/suggest.rs:1457-1560` is the only production caller and always passes the matched row) |
| Whether `is_extrapolated` shifts the §3.2 census | **NOT MEASURED** | extrapolation is query-dependent, not a row property; a static census cannot answer it. It can only *raise* the advisory count, never lower it |
| GUI/MCP render of the burn advisory and the unbanded chip track | **NOT EXERCISED** | §0 rule 3 requires a screenshot for any claim about a visible surface. This document makes no claim about how either *looks*; the advisory strings are read from `from_tool_load.rs` |
| The probe test file's own execution | **DONE, 4/4 green** | §7. Note the first run **failed** and narrowed a claim; the failure is recorded, not smoothed |
| Whether the ~0.63 D reachability boundary is ever crossed by a real operator project | **NOT MEASURED** | needs a project corpus census of Adaptive3d / 3D-finish stepovers on ball tools. The probe proves the boundary exists, not that anyone stands on it |
| Any bench or literature search for a chip-thickness envelope | **NOT ATTEMPTED** | out of scope: S-2 owns the literature lane this programme, and Q1 is the question that would commission it |

---

## 7. Probe results

`crates/rs_cam_core/tests/chip_thickness_policy_a9.rs` — four tests, all
reproductions of current state. Run 2026-08-13 on `tech-debt-3`:

```text
$ cargo test -p rs_cam_core --test chip_thickness_policy_a9 -- --nocapture
running 4 tests
test the_canonical_chip_model_pins_a_full_slot_where_the_floors_copy_reads_zero ... ok
test ball_chip_geometry_refuses_past_the_pole_which_is_what_empties_the_gate ... ok
A-9 probe: reachable @ 0.6275 D — ball 1.0 softwood floor=0.23259 safe_band_empty=true
A-9 probe: 36 (row x shipped-default stepover) combinations exercised
test the_axial_floor_only_becomes_reachable_above_every_shipped_default ... ok
test axial_chipload_floor_is_none_at_every_shipped_default_stepover ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

`cargo clippy -p rs_cam_core --tests -- -D warnings` clean;
`cargo fmt --check -p rs_cam_core` clean. Raw captures in
`artifacts/a9/`.

**The first run failed, and that is recorded rather than smoothed over.**
`axial_chipload_floor_is_none_across_the_whole_shipped_ball_population`
and `safe_band_empty_is_unreachable_on_every_shipped_ball_row` both
asserted the bound was unreachable *including* at 0.6275 D, and both were
falsified by `ball 1.0 softwood @ 0.6275 D` producing `floor=0.23259` and
`safe_band_is_empty() == Some(true)`. The response was to **narrow the
claim to what was measured** and add a third test pinning the reachability
boundary itself — not to loosen the assertion. The pre-fix failure text is
in `artifacts/a9/probe.txt`'s first capture and the corrected claim is
§1.3.

What each test pins, and its retirement condition:

| test | pins | goes red when |
|---|---|---|
| `axial_chipload_floor_is_none_at_every_shipped_default_stepover` | 36 combos: floor `None`, `safe_band_is_empty()` `None`, binding never `SafeBandEmpty` | the floor is repaired, the bracket is changed, a call-site default moves above ~0.63 D, or a ball row's band widens |
| `the_axial_floor_only_becomes_reachable_above_every_shipped_default` | the boundary is real: ≥ 1 shipped ball row produces a floor at 0.6275 D and at least one lands `SafeBandEmpty` | the reachability boundary moves in either direction — deliberately two-sided, so a "fix" that makes the bound *more* dead also trips |
| `the_canonical_chip_model_pins_a_full_slot_where_the_floors_copy_reads_zero` | `chip_geometry` at `arc = π` gives `h_max = fz` and `mean = 2/π · fz` exactly; the floor's transcription gives `< 1e-15 · fz` | the `arc ≥ π` branch is added to `min_doc_chipload_floor`, or the canonical model changes |
| `ball_chip_geometry_refuses_past_the_pole_which_is_what_empties_the_gate` | `BallEndmill::chip_geometry` is `Ok` below the pole and `Unsupported` at and past it, at four depths × four arcs | the pole guard is replaced by a model that covers it |
