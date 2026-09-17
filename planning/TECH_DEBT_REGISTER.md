# Tech debt register

**Living document. Append, do not date-stamp and archive.**

`TECH_DEBT_AUDIT.md` (2026-04-11) and `TECH_DEBT_REVIEW_2026-06-10.md` are
point-in-time audits — evidence of what was measured then, per
`planning/CLAUDE.md`. This file is different: an open register of debt found
while doing other work, added to as it is found.

**The common thread: every entry here is something no gate can fail on.**
Clippy is green, the tests pass, and the defect persists. That is what makes
them worth writing down — anything the compiler or a sentry catches does not
need a register.

| ID | What | Status |
|---|---|---|
| T-1 | `enumerate_matching_rows` is dead; `pub` hides it | **closed** `d641996c` — deleted (feeds wave, 2026-09-17) |
| T-2 | LH-1's guard is syntactic and a closure defeats it | open |
| T-3 | Cross-crate sentries never run in a per-crate gate | open |
| T-4 | `predict_peak_deflection_um` returns `0.0` for every refusal | **closed** 2026-09-18 — the refusal is `Result<_, DeflectionUnmodeled>`; a V-bit figure carries `DeflectionCaveat::FluteReliefUnmodeled`; the back-off states its abstention (load-model programme, see the entry) |
| T-5 | `feeds/mod.rs` 4 144 lines, `suggest.rs` 5 667 | open — `calculate` (1 302 lines) has no mechanical seam: 16 `let mut` locals cross its 19 step banners and `effective_d` is rebound mid-way (FEEDS_WAVE FW-22, 2026-09-17) |
| T-6 | Two implementations of one physical model | closed |
| T-7 | Two definitions of "teeth in cut", differing by helix wrap | **withdrawn** — the premise fails |
| T-8 | The power derate thins the chip, and only half the power responds | **closed** `a7c17be7` |
| T-9 | A feed clamped onto a ceiling ships one rounding step above it | open |
| T-10 | No gantry feed-force limit exists; the steppers are unmodelled | open — needs a thrust rating |
| T-11 | Feed modulation multiplies mm by a fraction of a different quantity | **closed** `809d28b9` |
| T-12 | A depth recommendation is dropped for 14 of 24 operations, silently | **closed** `ad2f749b` |
| T-13 | `F_edge` is applied per mm of depth to an edge that is longer than that | open — needs a literature anchor |
| T-14 | A drop-cutter finishing pass measures 42.5 mm of axial engagement | open — R1 made it load-bearing |
| T-15 | Pass 9 can raise a feed the power ladder just clamped | open — reachable by hand TODAY |
| T-16 | The deflection bending diameter cites a source that does not say it | **closed** — its per-flute table is itself superseded, see T-17 |
| T-17 | The deflection integrator gives a fluted end mill a solid cross-section | **closed** `93dd145c` — flat 0.80, every fluted shape; V-bit still open under T-4 |
| T-18 | Three feed lifts cap against the gantry TRAVEL rate, not the cutting ceiling | open — latent on shipped presets only |

---

## T-1 — a public function with no callers, and no warning

`crates/rs_cam_core/src/feeds/vendor_lookup.rs:315`
`enumerate_matching_rows` has **zero callers** across `crates/` since D-1
(commit `500c21a1`) deleted the sibling-row scan. The only remaining mention
in the repository is a historical note in a comment.

**Why nothing catches it:** it is `pub` in a library crate, so it is part of
the public API and `dead_code` does not fire. The compiler cannot know a
library's callers.

**Cost if left:** this is exactly how the feeds area reached 28 800 lines. A
`pub` function that nothing calls reads as load-bearing to the next person,
who works around it rather than deleting it.

**Fix:** delete it, or demote it to `pub(crate)` and let the compiler decide.
Deliberately left in place during D-1 to keep that change behaviour-neutral.

---

## T-2 — a sentry that checks for a string, not for the thing

`crates/rs_cam_core/tests/air_cut_denominators_lh1.rs` asserts

```rust
!src.contains("air_cut_time_s / ")
```

to stop a surface dividing air-cut time by hand instead of calling
`AirCutRatios::air_cut_pct_of_total_runtime()`.

**It does not work.** Moving the division into a closure applied to the field
— `pct_of_total(s.air_cut_time_s)`, where the closure divides — contains no
such text. I introduced exactly the defect the sentry exists to prevent, in
commit `236be682`, and it went unnoticed for a day. Fixed in `5b716c47`.

There is a second edge, found while fixing it: the sentry scans raw source
including comments, so a comment that *explains* the trap by quoting the
pattern also fails the test.

**Cost if left:** the guard reads as protection and is not. A negative
source-scan that a one-line refactor evades is worse than no guard, because
nobody investigates a green test.

**Fix:** check semantically — that the call to the named ratio is present —
rather than that one spelling of its absence is missing. Or lint on the AST.

---

## T-3 — core tests scan viz sources, and a per-crate gate never runs them

Ten tests under `crates/rs_cam_core/tests/` read files under
`crates/rs_cam_viz/src/`, asserting the GUI names its denominators, routes
through the apply funnel, and so on. They are correct and valuable.

**`cargo test -p rs_cam_viz` does not run them.** A GUI change can break a
core test, and the obvious gate for a GUI change will not say so. That is how
T-2 survived a day: the declutter work ran the viz suite repeatedly and never
the core one.

**Cost if left:** a whole class of cross-crate invariant is only enforced by
a gate nobody runs after a UI change.

**Fix:** no clean one — the test must live where it can `include_str!` both
crates. Mitigation: list the scanned viz paths somewhere a viz developer will
see, or move these to a workspace-level test target that both gates run.

---

## T-4 — a sentinel where a `Result` belongs

`crates/rs_cam_core/src/feeds/predict.rs:129`
`predict_peak_deflection_um` returns `DeflectionPrediction { predicted_um }`
and, by its documented contract, reports **every refusal as `0.0`** — drill
operations, missing Kc, no stickout, no DPP. A genuine zero deflection and
"this model does not apply" are indistinguishable to every caller.

**Why nothing catches it:** the type is honest about its shape and silent
about its meaning. Callers compile fine either way.

**Cost if left:** each caller invents its own interpretation. `feeds::
efficiency` maps `0.0` to `None` and abstains, which is conservative and
correct — but that is a decision made at the wrong layer, and the next
caller may just as reasonably publish "100 % headroom".

**Fix:** `Result<DeflectionPrediction, UnmodelledReason>`, matching the
refusal vocabulary `tool_load` already uses.

### Closed 2026-09-18

The signature is now
`Result<DeflectionPrediction, DeflectionUnmodeled>`. `DeflectionUnmodeled`
carries nine variants and maps onto `tool_load::verdict::UnmodeledReason`
through `as_unmodeled_reason()`, so the pre-simulation and post-simulation
halves print one vocabulary. Each variant also carries a `clause()`, so one
place words each finding.

What changed beyond the type:

- **The blanket V-bit guard is deleted.** Its stated reason was that the
  post-simulation integrator handles V-bits, and since `3b0dc487` this
  predictor CALLS that integrator; `cutter_constraints::invert_deflection`
  has read the same path for a V-bit throughout. A V-bit now returns `Ok`
  carrying `DeflectionCaveat::FluteReliefUnmodeled`. The figure is a
  modelled FLOOR, not a bound: the cone is modelled and the flute relief is
  not, and that gap is non-conservative. No consumer may read a caveated
  value as proof that a cut is inside a budget.
- **The two undocumented exits are stated.** `Material::Custom` refused
  even with a positive `kc`, and a non-positive stickout refused, both
  hidden inside a `.map_or(0.0, …)`. They are `MaterialCustom` and
  `NoStickout` now, and the module header lists the full set.
- **The back-off ACTS on the refusal.** `backoff_dpp_for_deflection` emits
  `SuggestWarning::DeflectionBackoffUnmodeled { dpp_mm, reason }` instead of
  passing through in silence, and
  `SuggestWarning::DeflectionBackoffFigureIsAFloor` when the figure it ran
  on is caveated. Both render through the existing rationale tree.
- **`CutterOpProfile::predictions` is deleted**, not migrated. `rg` found
  no reader in production or test.

Sentry: `tests/a_refused_deflection_is_not_a_zero_g_t4.rs`, six arms, with
a non-vacuity anchor and a 27-point sweep that no `Ok` may answer with a
zero. Confirmed red by injection on two paths.

Report: `planning/load_model_2026-09-16/T4_IMPLEMENTATION.md`.

**Still open, and outside T-4:** `f_V`, the V-bit equivalent-diameter
fraction, has no source. Under the no-bench-rig rule it stays unmodelled;
the caveat makes the gap visible rather than closing it.

---

## T-5 — two files where a reader looks first

`crates/rs_cam_core/src/feeds/mod.rs` is 4 144 lines with 29 top-level
items: the `calculate` pipeline, the rubbing-floor policy and the domain's
shared types. `crates/rs_cam_core/src/feeds/suggest.rs` is 5 667.

**Cost if left:** they are the least navigable files in the area a reader
enters first. Nothing is broken; everything is slower.

**Fix:** its own package. Explicitly out of scope for the cut-efficiency
programme (`load_model_2026-09-16/REVIEW.md` D-4) so it does not ride along
with unrelated work.

---

## T-6 — the same physics implemented twice

The recurring structural defect in this area, and the one that motivated the
2026-09-16 programme.

- **`force.rs` vs `power.rs`** — an affine, chip-thickness-aware force model
  and a constant-specific-energy power model, disagreeing by 2–4× in the
  regime wood routing actually runs in (8.6× once the anisotropy factor is
  carried). **Closed by R1** (2026-09-16): `tool_load::power` now builds both
  its terms from `force::affine_coefficients_for_kc`, and the literature
  constants live in `force.rs` alone.
- **`feed_modulation` held its own copy** of the deflection inversion that
  `force.rs` should own. Closed by N-2 (`991e6af7`): one implementation,
  two views.
- **`feed_modulation` held a second copy of the power formula too.** Closed
  by R1 the same way: the solver calls `PowerTerms::feed_for_kw` instead of
  inverting its own expression.

**Why nothing catches it:** both copies compile, both are tested, and each
test is consistent with its own copy. Divergence is only visible to someone
comparing them deliberately.

**Fix pattern that worked:** extract to the module that owns the model and
have the second caller consume it, rather than writing a third copy. Keep a
test that pins the extraction as behaviour-neutral — for N-2, an existing
test that had to pass **byte-identical and unmodified**.

---

## T-7 — two definitions of "teeth in cut", one of them helix-blind

`crates/rs_cam_core/src/tool/mod.rs:215` computes

```rust
instantaneous_flutes_in_cut = (arc_engagement_radians + helix_wrap) / flute_pitch
```

where `flute_pitch = 2π/z`, so the first term is exactly the duty cycle
`z·ψ/2π` that R1's edge power term uses
(`crates/rs_cam_core/src/tool_load/power.rs`, `PowerTerms::of`). The second
term is not: `helix_wrap = ap·tan(helix)/r` accounts for a helical edge
entering the cut before the previous one leaves.

The power model carries no helix wrap. On a 6 mm 2-flute 30° cutter at
4 mm DOC the wrap is `4·tan(30°)/3 = 0.77 rad`, against a half-immersion
`ψ` of 1.57 — so the true teeth-in-cut count is about 49 % higher than the
duty cycle power uses, and the edge term is understated by the same share.
Deep cuts with high-helix tools are where the gap is widest.

**Why nothing catches it:** both expressions are correct answers to slightly
different questions, both are tested against themselves, and no sentry
compares them. R1's own sentries pin `z·ψ/2π` deliberately, because that is
the factor `ADVICE.md` §2 derived.

**Cost if left:** the understatement lands inside the EDGE term, which
carries 83 % of the cutting power at the 0.038 mm/tooth the reference
fixture runs at (`load_model_2026-09-16/SPEC.md`, R1 shipped). A ~50 %
under-read of engaged cutting edge in the term that dominates the bill is
not a rounding difference — it under-reads exactly the cuts most likely to
stall a hobby spindle. And a reader who finds both expressions has no way
to tell which one the engine means.

**WITHDRAWN 2026-09-17. The entry above is wrong, and the fix it proposes
would have made the model worse.** Tested before writing any Rust, in
`planning/load_model_2026-09-16/derate_levers.py` rule 6, and visible in the
bench artifact.

**A helix moves engagement in TIME, not in amount.** Model the tool as a
stack of thin axial slices. Each slice is locally a straight edge and sees
the same immersion arc `ψ`. All a helix does is give each slice a different
phase, because the spiral reaches the wood lower-first. Measured mean engaged
edge over one revolution, Ø12 4-flute at 4 mm depth:

| helix | wrap | mean engaged edge |
|---|---|---|
| 0° | 0.000 | 3.224 mm |
| 20° | 0.243 | 3.224 mm |
| 30° | 0.385 | 3.224 mm |
| 45° | 0.667 | 3.224 mm |

Flat, and equal to the closed form `ap · z·ψ/2π` — which is exactly what
`power.rs` already uses. **Adding the wrap would OVERSTATE mean power by
about 50 %, not correct an under-read.**

The 49 % in the entry above is real arithmetic. It is arithmetic about a
different question: how many teeth touch AT ONCE, which sets peak force and
smoothness. That is the question `tool/mod.rs` is answering, and it answers
it correctly. The two expressions were never in conflict.

**A second reason the entry overstated the risk:**
`instantaneous_flutes_in_cut` has exactly one consumer in the whole
workspace — a cache-invalidation hash in `compute/sim_prefix.rs`. No physics
reads it. The "two competing definitions a reader cannot tell apart" is one
definition and one value that is hashed and discarded.

**What survives, and it is much smaller.** A helical edge is physically
LONGER than the depth it covers, by `1/cos(β)` — 15.5 % at 30°, not 49 %.
Logged as T-13 rather than left inside a withdrawn entry.

**The lesson worth keeping:** this entry was written from a correct
observation (two expressions, one has a term the other lacks) and an
incorrect inference (therefore one is wrong). A twenty-line numerical model
settled it in minutes and saved a feed-moving recalibration in the wrong
direction. Test the premise before scheduling the fix.

**Superseded fix note:** give `feeds::force` one `teeth_in_cut(z, ψ, ap,
helix, r)` and have both consume it.

---

## T-8 — the power derate thins the chip, and only half the power responds

`crates/rs_cam_core/src/feeds/mod.rs` Step 6 caps a power-limited cut by
scaling the feed at constant RPM. R1 makes the consequence measurable: the
edge term carries no feed, so thinning the chip reduces only the shear half
of the bill while raising the specific energy `u = P/MRR`, and it pushes the
chipload toward the rubbing floor. `ChiploadClampedToFloor` already fires for
this reason. `ADVICE.md` §3 rates the power derate's direction "wrong-ish"
and R4 proposes traversing the constant-chipload line by dropping RPM
instead.

R1 makes this newly consequential rather than theoretical: before R1 the
power branch never fired on a shipped preset, and it now fires on three
(`tests/power_ceiling_parity_f2.rs`,
`the_power_ceiling_binds_on_three_shipped_fixtures`).

**Measured, 2026-09-16.** Ø12 4-flute bull nose (1.5 mm corner) in white
oak, Shapeoko-class 1.5 kW VFD, free-run engagement ap 8.400 / ae 4.200 at
9 000 RPM. `calculate` reports:

```text
PowerLimited { required_kw: 0.761, available_kw: 0.450 }
power_limit derate: 0.058                 <- a 17x feed reduction
ChiploadClampedToFloor { requested: 0.008546, floor: 0.025 }
final: feed 900 mm/min, fpt 0.0250, power 0.487 kW  <- still over 0.450
```

The edge term alone at that geometry is 0.431 kW — 96 % of the whole
budget — so feed has almost nothing left to give. The derate drives the
chipload to 0.0085 mm/tooth, Step 9b clamps it back up to the 0.025 floor,
and the shipped recipe is **both** rubbing-adjacent and over the power
ceiling. Two warnings fire and neither constraint is honoured.

`tests/literature_matrix` cell `bull_12mm_pocket_oak` is the sentry that
catches it: it moved `moderate` -> **`major`** at R1, on
`anti.bull_misclassified_as_ball_chipload` (`fpt < 0.030`), because the
derate took fpt from 0.0625 to 0.0250. **That failure is open, not
absorbed.** The anti-pattern was not weakened and the cell was not
re-pinned.

**Why nothing catches the direction itself:** the clamp is arithmetically
correct against the ceiling it is given, and every assertion about the
clamp passes. No assertion states which lever a power limit should pull.

**CLOSED by `a7c17be7`** (2026-09-16). The engagement ladder shipped and
`bull_12mm_pocket_oak` went from `major` to `moderate` without re-pinning.
Pinned by `tests/the_power_ladder_pulls_the_right_lever_g_ladder.rs`, whose
VFD arm is the only coverage of the refuse-the-traverse case — no literature
matrix cell routes to a VFD.

**One correction the implementation forced on the spec.** `DERATE_SPEC.md`
said the feed is the wrong lever for this constraint and the first
implementation removed it entirely. That shipped recipes 131 % over the
ceiling. The feed is not a forbidden lever, it is a LAST lever: it goes after
the RPM and the geometry, where the cut it has to make is small. The spec's
reasoning about WHY the feed is a poor first choice stands; its implied
conclusion that it should never be used did not survive
`power_ceiling_parity_f2`.

**Still open, carried from T-12:** a proposed depth is not yet snapped to a
realisable `total / n`. The ladder chooses a continuous `ap`, and every 2.5D
operation realises `total / ceil(total / dpp)`. The ladder's own arithmetic is
therefore right about the depth it names and approximate about the depth the
machine will cut.

**Original fix note:** specified in `planning/load_model_2026-09-16/DERATE_SPEC.md`. Make
the derate direction per-derate. Deflection-driven derates reduce chipload.
The feed-cap derate traverses the constant-chipload line by reducing RPM.
The power derate branches on `PowerModel`.

**Correction, measured after R1:** an earlier version of this entry said the
power derate must also traverse the constant-chipload line. That is true
only on `PowerModel::ConstantPower`. On `PowerModel::VfdConstantTorque` the
available power scales with RPM exactly as the required power does, so
utilisation stays at 120 % at 9 000, 6 000, 4 500 and 3 000 rpm. The
traverse achieves nothing there. Only a smaller depth of cut helps
(120 % -> 85 % -> 57 % -> 36 % at ap 8.4 -> 6.0 -> 4.0 -> 2.5 mm).

Needs its own authorisation, for the same reason R1 did — it moves
recommended spindle speeds.

---

## T-9 — a feed clamped onto a ceiling ships one rounding step above it

`crates/rs_cam_core/src/feeds/suggest.rs:874` writes
`round_suggestion_value(result.feed_rate_mm_min, 1.0)` — the commanded feed
is quantised to a whole mm/min, upward as often as downward.

That was harmless while no recommendation sat near a limit. R1's power
clamp lands the recommendation EXACTLY on the gate ceiling, so the rounding
now decides whether the shipped recipe is inside it. Measured: Shapeoko
(1.5 kW VFD) / Ipe / Ø12 slot, calculator 323.6993 mm/min -> shipped
324.0000, which is +0.093 % of feed and +0.013 % of power — over the
ceiling (`tests/suggest_power_ceiling_after_pass9_g_suggest_powerstale.rs`,
`shipped_presets_stay_clear_of_the_ceiling_after_the_rescale`, whose bound
is now 0.2 % to allow exactly one such step).

**Why nothing catches it:** every clamp in `calculate` is satisfied at the
value `calculate` returns. The rounding happens afterwards, in the apply
path, and nothing re-checks a limit after it.

**Cost if left:** small in magnitude and unbounded in principle — the same
rounding applies to any clamp the calculator lands on exactly, and the next
limit to become binding will meet it too.

**Fix:** round the commanded feed DOWN when the recommendation was clamped
by a ceiling, or re-check the limits after quantisation. Not done here
because it moves recommended feeds by up to 1 mm/min across the whole
matrix, for a defect worth 0.013 %.

---

## T-10 — no gantry feed-force limit exists; the steppers are unmodelled

`rs_cam_core` models tool deflection, spindle power and chipload. It does
not model the force the gantry must push to make the cut. There is no
thrust, stall or feed-force limit anywhere in the crate.

A stepper gantry does not fail by running out of watts. It fails by losing
steps when the cutting force is more than the axis thrust. Measured on the
reference cut (Ø12 4-flute bull nose, ap 8.4, ae 4.2, `GenericHardwood`,
Kc 35.1) the feed
force is 22-34 N. As wattage that is 1.4-3.3 W, under 1 % of the spindle
power — which is why a power model cannot see it. Against a hobby gantry's
thrust it is significant.

The force does not respond to the feed rate at all. At a constant chip
thickness of 0.0625 mm/tooth the feed can go from 1 125 to 4 500 mm/min
and the gantry force does not move, while the spindle power goes from 269
to 1 076 W.

It responds to the DEPTH and the WIDTH, and barely to the chip thickness.
Cut each dial to a third of its value and the push moves by -67 % (depth),
-45 % (width) and -6 % (chipload). At a chipload of zero the push is still
93 % of its full value, because the edge term carries the load and the edge
term contains no chipload.

**Why nothing catches it:** the limit is not weakly enforced. It is absent.
No test can fail on a constraint the crate does not state.

**Cost if left:** the engine recommends a cut that a light machine cannot
push, and it reports no warning. The user finds out when the machine loses
position in the middle of a job.

**Research done 2026-09-16** — `planning/load_model_2026-09-16/THRUST_RESEARCH.md`.
Two of the three unknowns are now answered.

1. **The axis thrust — still the blocker.** `MachineProfile` carries no such
   field, and no maker publishes a rating. The research gives ballpark
   figures that span 20:1 across machine classes: a belt gantry skips at a
   measured 85 N, a ballscrew is a derived 1 074 N and a rack and pinion a
   derived 1 537 N. A skip happens at 0.5 to 0.6 of stall. One constant for
   every machine is therefore not an option, and every non-belt figure is
   derived arithmetic that nobody has measured.
2. **The feed-force ratio — answered, and 0.5 was wrong.** It is geometry,
   not a fitted constant: the peak of
   `-(F_t*cos(phi) + K_r*F_t*sin(phi))` over the engagement arc. The peak
   ratio runs 0.53 to 1.0 and never approaches 0.5. In the adaptive band it
   is 0.79 to 0.97, so the placeholder understated the load by about two.
3. **The lever — answered.** Reduce the depth, then the width. Never the
   feed rate, and thinning the chip moves the push by 6 %.

**Acceleration is NOT a usable proxy for thrust (tested 2026-09-16).** The
suggestion is a good one — `MachineKinematics` already carries per-axis
acceleration from GRBL `$120/$121/$122`, so the number is published and
plumbed. The physics is `F = m·a`. The numbers refuse it:

| | implies | of the measured 85 N |
|---|---|---|
| X carriage (~4 kg) @ 250 mm/s², stock | 1.0 N | **1.2 %** |
| X carriage @ 800, community tuned | 3.2 N | 3.8 % |
| Y gantry (~12 kg) @ 250 | 3.0 N | 3.5 % |

Inverted, the measured 85 N would allow about 21 000 mm/s² on the X axis
against a stock setting of 250. The acceleration limit on these machines is
not set by the motor. It is set by ringing, belt stretch and surface finish.
`m·a` would understate the thrust by 30 to 85 times.

**The method works where the value was tuned to the skip point.** The usual
GRBL procedure — raise `$120` until the axis loses position, then back off —
produces an acceleration that IS `F_thrust / m` by construction. A factory
default is not that value. So an accel-derived thrust is only valid for a
machine whose owner tuned it that way, and the engine cannot tell which is
which from the number alone.

**A cheap measurement this does unlock.** Raising `$120` unloaded until the
axis skips gives `F = m·a_skip` with no force gauge, using the controller as
the instrument. It needs the moving mass, which can be weighed. That is more
repeatable than the fish-scale pull that produced the only measured figure in
`THRUST_RESEARCH.md`.

**Related finding.** The cutting force on the reference cut is about 15 times
the inertial force at stock acceleration. The gantry drive spends nearly all
its effort on the cut, not on moving the mass — so a lost step happens in the
cut, not in a corner, and the acceleration budget is nowhere near binding.

**What is still missing:** a measured skip threshold for any machine that is
not belt driven, and a check on whether the frame rather than the motor sets
the real limit. The one piece of evidence on that point, a Shapeoko belt
thread, says the drive compliance dominated.

---

## T-11 — feed modulation scales its depth by a fraction of the wrong quantity

`feed_modulation.rs:577` computes the axial depth a move cuts at:

```rust
axial * engagement.axial_doc_fraction.clamp(0.0, 1.0)
```

`axial` is `ctx.nominal_axial_doc_mm`, an absolute depth in mm: the maximum
`axial_engagement_mm` over the toolpath's cutting samples
(`session/compute.rs:2373-2379`).

`axial_doc_fraction` is NOT a fraction of that depth. The dexel writes it as
a fraction of the FLUTE LENGTH (`dexel_stock/simulation.rs:1106`):

```rust
axial_doc_fraction: Some((axial_engagement_mm / flute_length).clamp(0.0, 1.0)),
```

The product is therefore `mm x (mm / flute_length)`, which is not a depth.
The field's own doc comment names the correct source two lines up: "Use
`axial_doc_mm` on the sample for the absolute reading."

**The error factor is `flute_length / nominal_axial_doc_mm`**, and it always
makes the depth too SHALLOW, because a pass is always shallower than the
flute. A 2 mm pass on a tool with a 25 mm flute gives `2.0 x 0.08 = 0.16 mm`
against a true 2.0 mm: 12.5x too shallow.

`effective_axial_mm` feeds both the deflection cap and the power cap in the
constrained-max solver. A depth that reads 12x too shallow makes both caps far
too permissive, so the modulator allows a feed the machine should not be given.

**Why no gate can fail on it:** every fixture sets `axial_doc_fraction: 1.0` —
`constrained_max_modulation_f039.rs` at lines 97, 136, 172, 220, 278, 311 and
316, and the in-module unit tests at `feed_modulation.rs:901` and `:965-997`.
At exactly 1.0 the wrong expression returns `nominal x 1.0`, which is the right
answer. The one value that hides the defect is the only value under test. The
unit test at `feed_modulation.rs:913` even writes the assumption down:
`let ap = 2.0; // nominal x axial_doc_fraction`.

**Cost if left:** the modulator is the component that decides the feed for
every move of an adaptive toolpath. It currently believes the cut is about ten
times shallower than it is.

**Interaction with the derate work:** `effective_axial` is proportional to
`nominal^2`, so the modulator's response to a depth change is quadratic, not
linear. Halving an operation's depth moves the modulator's effective depth by
about four. Any reasoning about "reduce the depth" that passes through the
modulator is wrong until this is fixed. See
`planning/load_model_2026-09-16/IMPLEMENTATION_PLAN.md`.

**CLOSED by `809d28b9`** (2026-09-16). `PerMoveEngagement` now carries
`axial_doc_mm` and the solver reads it. The multiplication is deleted, not
corrected, so no expression with the wrong units survives. Pinned by
`tests/modulation_reads_the_depth_in_mm_g_axialunits.rs`, which never uses a
fraction of 1.0.

**One thing to know if this is ever revisited:** closing it changed no
existing test outcome. The pipeline fixture that drives real modulation
supplies neither `deflection_inputs` nor `power_inputs`, so the depth never
reached a cap in the suite. Production supplies both. The defect was
therefore live in shipped behaviour and invisible to every gate, and the new
sentry is the only coverage. A fixture that drives the modulator with the
caps ENABLED, off a real simulated trace, is still missing.

**Original fix note:** read the absolute per-sample `axial_engagement_mm` that the dexel
already measures, exactly as the field doc directs. Multiplying the fraction
by `flute_length` recovers the same number and is the smaller change, but it
re-derives a value the sample already carries. Then re-pin the fixtures at a
fraction that is NOT 1.0, or the replacement is equally unguarded.

**Found by:** a read-only survey during the derate-lever work, 2026-09-16.
Verified through all four hops: the dexel producer, the time-weighted mean at
`session/compute.rs:2308`, the `nominal_axial` maximum at `:2373`, and the
multiplication at `feed_modulation.rs:577`. Read statically. Not benched.

---

## T-12 — a depth recommendation is dropped for 14 of 24 operations, silently

`OperationParams::set_depth_per_pass` returns a `bool`, and its own doc
comment says why (`compute/catalog.rs:677-681`):

> Returns `false` when this config has no such field, so the caller can refuse
> instead of discarding the value.

The caller discards it. `feeds/suggest.rs:877` calls
`scratch.set_depth_per_pass(...)` and ignores the result. The copy-back at
`feeds/suggest.rs:936` then reads `depth_per_pass()`, gets `None` for a config
that has no such field, and writes nothing.

Exactly **10** configs override `depth_per_pass` and `set_depth_per_pass`
(counted in `compute/operation_configs.rs`). The trait default returns `None`
and `false`. There are 24 operation types. So for 14 of them the calculator
computes an axial depth, writes it to a scratch clone, and the value
evaporates. No warning is raised.

**Why no gate can fail on it:** the apply path reports success, because it did
apply everything it was able to apply. The refusal signal exists in the type
and is thrown away one line after it is produced. This is the same shape as
the other entries here: an absence rendered as a success.

**Cost if left:** it is about to become load-bearing. The derate work needs a
power-limited cut to recommend a shallower pass, and for 14 of 24 operations
that recommendation would be silently discarded while the warning still says
the cut is over power. The user would see a problem, no fix, and no statement
that the fix was dropped.

**A second reason the value cannot be trusted as written.** Every 2.5D
operation uses `DepthDistribution::Even` (`depth.rs:16-19`), so the realised
depth is `total / ceil(total / depth_per_pass)`, not `depth_per_pass`. The
realised depth is a staircase. On a 12 mm pocket only 4.00, 3.00, 2.40, 2.00,
1.71, 1.50 and 1.33 mm are reachable. Asking for 2.90 lands on 2.40, a 20 %
step. Asking for 2.60 or 2.40 then changes nothing at all. **A proposal
expressed as a percentage cut cannot be honoured**, and a caller that assumes
it was gets a depth up to 20 % away from the one it reasoned about.

**CLOSED by `ad2f749b`** (2026-09-16). The funnel honours the `bool` and
raises `SuggestWarning::CutGeometryFieldNotHeld`, pinned by
`tests/apply_reports_a_field_it_cannot_hold_g_notheld.rs`. The second half —
snapping a proposed depth to a realisable `total / n` — is NOT done, and is
carried into the derate work's engagement ladder rather than here, because
only a caller that proposes a depth needs it.

**Original fix note:** honour the `bool`. When a config refuses the write, raise a typed
`SuggestWarning` naming the field and the operation, exactly as the five
existing axial clamps already do. Separately, snap any proposed depth to a
realisable `total / n` before proposing it, so the number the engine reasons
about is the number the machine will cut.

**Found by:** a read-only survey during the derate-lever work, 2026-09-16.
Verified: the trait defaults at `compute/catalog.rs:673-681`, the 10 overrides,
the discarded return at `feeds/suggest.rs:877`, the `None` guard at `:936`, and
the `Even` distribution at `depth.rs:16-19`. Read statically. Not benched.

---

## T-13 — `F_edge` is applied per mm of depth to an edge that is longer

`feeds/force.rs` documents `F_edge` as "N/mm of **axial engagement**", and
the load model applies it that way: the edge power term is
`F_edge · ap · Vc · duty`.

The coefficient comes from the woodresearch.sk **quasi-orthogonal** fit,
where the cutting edge is straight and parallel to the axis. On that tool a
millimetre of axial depth is a millimetre of edge. On a helical router bit it
is not: the edge spirals, so it is `1/cos(β)` longer than the depth it
covers. At a 30° helix that is **15.5 %** more edge per mm of depth; at 45°
it is 41 %.

If the ploughing force arises per unit of EDGE, as the physical picture of a
rounded edge rubbing suggests, the edge term is under-read by that factor on
every helical tool — which is every router bit we model.

**Why nothing catches it:** the coefficient and its consumer agree with each
other. Both say "per mm of axial engagement", consistently, and every sentry
pins that consistency. Nothing asks whether the calibration tool and the
modelled tool have the same edge geometry.

**Why this is NOT simply a bug to fix.** Two things must be true, and only
the first is settled:

1. The geometry is certain: a helical edge is `1/cos(β)` longer. That is
   trigonometry.
2. Whether the ploughing force scales with edge length is NOT certain. A
   helical edge cuts obliquely, which changes the effective rake, the chip
   flow direction and the force distribution along the edge. Part of the
   force on an inclined edge is axial and does no work against rotation.
   Oblique-cutting mechanics may already absorb some or all of the extra
   length.

**What it needs:** a literature anchor for oblique wood cutting of the same
standard `force.rs` holds for the affine fit, or a measurement. Until then
this is a question, not a defect, and it must not be "fixed" by multiplying
by `1/cos(β)` and hoping.

**Found by:** withdrawing T-7, 2026-09-17. T-7 claimed a 49 % helix
under-read that turned out to be about a different quantity. This is the
residue that survived the test, and it is a quarter the size.

---

## T-14 — a finishing pass measures 42.5 mm of axial engagement

Smoke case `AS014` is a `drop_cutter` 3D finishing pass. It reports
`peak_axial_doc_mm = 42.465`. A drop cutter traces a surface; its axial
engagement should be a fraction of a millimetre.

**The reading is not new. It is not a regression.** The June 2026 baseline
records 42.500 for the same case. It has been wrong for at least three months
and nothing looked at it, because nothing depended on it.

**R1 made it depend on it.** The two-term power model's edge term is
proportional to `ap`, so the power now scales with this reading:

| AS014 | June | September | |
|---|---|---|---|
| `peak_axial_doc_mm` | 42.500 | 42.465 | **unchanged** |
| `power_peak_kw` | 0.0284 | 1.0065 | **35x**, `within` → `exceeds` |
| `deflection_peak_mm` | 0.4426 | 0.0984 | 0.2x, `exceeds` → `within` |

The depth did not move. The verdict did.

R1's documented amplification at the reference fixture is 8.6x at `ap` 8.4 mm.
AS014 shows 35x at a depth reading five times larger, which is what the model
predicts: **the amplification scales with the depth reading, so a wrong depth
now produces a proportionally wrong power.**

**Why nothing catches it:** the depth reading is an input to three gates and
the output of none. No sentry asserts that a finishing pass engages shallowly,
because until R1 a bad axial reading changed the power bill only weakly.

**Cost if left:** a smoke case reports `exceeds` on spindle power for a cut
that almost certainly draws about 30 W. That is a false alarm on the surface
the operator is meant to trust, and it is the first one this programme has
produced.

**What it is NOT:** it is not a reason to revisit R1. The power model is
correct and is corroborated against two wood datasets. It is faithfully
amplifying a bad input, which is what a correct model does.

**Fix:** find why a drop cutter measures 42.5 mm. Candidates, none verified:
the entry plunge counted as cutting engagement; the full stock height counted
when the tool first meets the surface; or `axial_engagement_mm` measuring the
flute contact envelope rather than the material actually removed. AS013
(`adaptive3d`, 14.766 → 3.743) and AS015 (`scallop`, 3.488 → 6.753) moved
between the two baselines while AS014 held, so whatever is wrong with AS014 is
specific to it rather than a shared axial-measurement change.

**Found by:** the structure session's re-cut baseline of 2026-09-17, which
flagged AS014 for this session. Verified here by aligning both baselines on
their headers — September added a `resolution_mm` column, so a positional
comparison would have compared the wrong fields.

---

## T-15 — pass 9 can raise a feed the power ladder just clamped

`rescale_feed_to_final_geometry` re-derives the feed at the geometry the
operation actually ships. It is wired in at `feeds::suggest::invariants`.

Its geometry term is `geometry_feed_factor`, which **ignores its `ae`
argument** and returns `depth_tier_multiplier(ap, diameter)` alone. That
multiplier RISES as the depth falls:

| `ap / D` | multiplier |
|---|---|
| over 3.0 | 0.45 |
| over 2.0 | 0.50 |
| over 1.0 | 0.75 |
| 1.0 or less | **1.00** |

So a clamp that lowers the depth across a tier boundary makes pass 9 **raise**
the feed, by up to 2.22x.

**Pass 9 does not re-check the power ceiling, and says why in its own doc:**

> It is not re-checked here because `tests/power_ceiling_parity_f2.rs` measured
> the power branch never firing at all ... peak utilisation 23.6 % ... On a
> profile where power does bind this pass can over-feed.

That is the same 23.6 % that justified deleting the power bar from the Feeds
card, and it is stale for the same reason: it was measured before R1 rebuilt
the power model about 8.6x higher. Re-measured after R1 across 162 recipes the
spread is median 17.8 %, p90 89.4 %, peak 100.0 %.

**So the condition the author wrote down as the failure case is now met.** The
power branch does bind. The power ladder added in this programme reduces `ap`
and `ae` as rungs 2 and 3, which is exactly the input pass 9 re-reads.

## What is verified, and what is not

Verified: `geometry_feed_factor` ignores `ae`; the multiplier rises as depth
falls; pass 9 is called; pass 9 does not re-check power; the 23.6 % is stale.

**NOT verified: a shipped case where it actually over-feeds.** Three fixtures
were built to force one and none crossed a tier boundary.

The reason is worth more than the attempt. **The rigidity cap holds `ap / D`
at about 0.20, far below the first tier boundary at 1.0.** So the multiplier
is 1.00 before and after every clamp, the depth-tier term never moves, and the
hazard stays latent.

**Correction, 2026-09-17, on operator challenge.** The paragraph above
originally read "the hazard stays latent". That is too strong, and it repeats
the exact error that made the 23.6 % stale.

Three preset machines, a handful of tools and one material is a sample. It is
evidence about the space that was sampled and nothing more. Specifically:

- **`depth_per_pass` is a user-editable field.** Nothing stops an operator
  setting `ap / D` above 1.0 by hand on a tool that can take it. The rigidity
  cap clamps a SUGGESTED depth; it does not bound what a person may type.
- A custom `MachineProfile` may carry any `doc_roughing_factor`. The shipped
  0.20 and 0.25 are two points, not a range.
- Adaptive operations use `adaptive_doc_factor`, which ships at 1.5 to 2.0 —
  an order of magnitude above the roughing factor, and already close to the
  first tier boundary.

**So the honest statement is: not observed on the presets sampled, and
reachable by hand today.** It becomes far more likely if the rigidity cap is
replaced with a measured stiffness, which is this programme's plan, so the two
should still land together.

**Cost if left:** a silent over-feed on a path the operator cannot see — pass
9's rationale entry reports a rescale, not a power breach. The frequency is
unknown, and "unknown" is the correct word rather than "zero".

## The general rule this entry now carries

**A sweep that does not fire is evidence about the sweep.** The 23.6 %
justified two separate decisions for months, and it was a measurement over
three presets, ten species and three diameters. It was honest, it was
carefully recorded, and it stopped being true when one model changed
underneath it.

Any finding in this register that rests on "we measured and it did not fire"
should state what was sampled, and should not be read as a statement about
machines, tools or materials outside that sample.

**Fix:** re-check the power ceiling after the rescale, or make the rescale
refuse to raise a feed on a recipe whose `FeedsDerates::power_limit` is below
1.0. The second is cheaper and is enough.

---

## T-16 — the bending diameter cites a source that does not say it

`ENDMILL_CORE_FRACTION = 0.7` (`feeds::predict`) is the bending section the
closed-form deflection model uses. Its comment says:

> The 0.7x reduction is the canonical handbook value for 2- to 3-flute end
> mills (Machinery's Handbook stiffness-correction notes; matches the FSWizard
> "effective root diameter" recommendation).

**A literature search found no source that gives 0.7 as a 2- to 3-flute core
fraction.** Published 2-flute core fractions run 0.54 to 0.60. The number is
neither a core diameter nor an equivalent diameter, and the citation does not
support it.

## Two quantities the sources conflate, and a cantilever needs the second

| | Published range | What it is |
|---|---|---|
| **Core diameter** | 0.47 – 0.85 D | the geometric root between the flutes |
| **Equivalent diameter** | 0.75 – 0.93 D | the solid shaft with the SAME bending compliance |

**The flute-count effect reverses sign between them.** More flutes gives a
LARGER core and a SMALLER equivalent diameter. Mixing the two gets the
correction backwards, which is exactly what this session did on its first
attempt before the research corrected it.

## What the number should be, and what that changes

Equivalent diameter by flute count, derived from Kivanc and Budak's published
deflection tables (Sabanci MSc thesis 2004, Tables 3.1 and 3.2; same work as
IJMTM 44(11):1151-1161). The ratios repeat to four significant figures across
6, 10, 16 and 20 mm and across two materials, so they are scale-free.

| Flutes | Equivalent d | Engine uses | Deflection error |
|---|---|---|---|
| 2 | 0.889 | 0.70 | **2.60x OVER-stated** |
| 3 | 0.841 | 0.70 | 2.08x OVER-stated |
| 4 | 0.748 | 0.70 | 1.30x OVER-stated |

**The engine is CONSERVATIVE, not permissive.** It over-states deflection on
every flute count. So this is not a safety defect. It is an over-restriction:
a 2-flute tool is modelled as bending 2.6 times more than it does, so the
deflection back-off fires when it need not and the recipe is needlessly timid.

## It probably explains a documented anomaly

`DEFLECTION_BACKOFF_TARGET_UM` records that the predictor "over-shoots
post-sim by ~36 %" and treats that bias as a safety margin. **The 4-flute
over-read computed above is +30 %.** The documented bias may simply BE the
missing flute-count term, measured on a 4-flute fixture. If so, correcting the
diameter removes the bias rather than adding a new error, and the back-off
threshold should be revisited in the same change.

## Why this is not a simple swap

Confidence is medium-high for 3 and 4 flutes and **low-medium for 2** — two of
eight 2-flute rows in the source disagree, giving 0.920 against 0.889.

And the change LOOSENS a guard. Deflection numbers all get smaller, so the
back-off fires less. Loosening a safety bound on medium-confidence literature
is a decision for the operator, not for the engine.

**Separately and freely: the comment is wrong today.** A citation that does
not say what it is cited for is a defect on its own, whatever happens to the
number. That half can be corrected without touching behaviour.

## CLOSED 2026-09-17 — and the loosening above does not happen

The paragraphs above assume `bending_diameter_mm` sets the deflection
magnitude. **It has not done so since 2026-06-17.** `feeds::predict` delegates
the magnitude to the shared two-section integrator
(`ToolDefinition::tip_deflection_mm`), and keeps the bending diameter only to
fill `DeflectionBreakdown.i_eff_mm4`, which the rationale tree displays.

So the shipped fix corrects a false citation and a displayed diagnostic. It
moves no guard, loosens no bound and needs no operator ruling. The literature
matrix and `literature_parity` confirm it: unchanged, 50 tests green.

The medium-low confidence at 2 flutes therefore costs nothing here. It becomes
load-bearing the moment T-17 is fixed, and the operator ruling deferred above
belongs to T-17, not to this entry.

## What the search could not settle

**No manufacturer publishes core diameter in any catalogue dimension table.**
Harvey, Guhring, Garr, Fullerton, M.A. Ford, Fastcut and Dormer were all
checked. The figures that exist come from patents and from academic work.

Spread between makers at ONE flute count exceeds the spread across flute
counts — at 4 flutes the published core figures run 0.53 to 0.70, which is
9.8x in stiffness. **There is no universal core constant to find.** Diameter
does not matter, but stick-out ratio does: OSG raised one 3-flute core from
0.38 D to 0.50 D purely because it is a long-flute tool.

---

## T-16 postscript — the replacement table did not survive either

T-16 replaced an unsourced 0.7 with a per-flute table from Kivanc and Budak
(2 -> 0.889, 3 -> 0.841, 4 -> 0.748). A first-principles derivation of the
fluted section, run on 2026-09-17
(`planning/load_model_2026-09-16/FLUTE_SECTION_MATH.md`), then removed the
table as well. Two independent reasons:

1. **The flute-count trend is an artefact.** The derivation reproduces the
   three values only under the "one flute shape, copied N times" family the
   thesis models. A real end mill has a core that RISES with flute count and a
   land fraction that RISES with it, and both push the fraction UP. Under a
   realistic family the trend REVERSES.
2. **The two-flute value is the wrong statistic.** A two-flute section is the
   only non-isotropic one: for N >= 3 the deviatoric part of the inertia
   tensor vanishes by symmetry, but two flutes leave principal values
   differing by 2.4x to 2.7x. The published 0.889 is their ARITHMETIC mean. A
   rotating tool under a fixed-direction load averages COMPLIANCE, so the
   correct statistic is the harmonic mean, about 0.84. Using 0.889
   under-predicts revolution-averaged deflection by 20 % to 26 %.

The operator ruling of 2026-09-17 took a single flute-count-independent
fraction of **0.80 D**, from Kops and Vo, Annals of the CIRP 39(1):93-96
(1990), who measured it from compliance. It is corroborated by the
inscribed-square bound (0.807, which the general section formula reproduces
exactly) and explained by the derivation: the two real trends nearly cancel,
which is why a compliance measurement finds a flat value.

**The lesson is not about deflection.** T-16 shipped a sentry whose direction
arm asserted that the fraction FALLS with flute count. That arm was written to
stop someone swapping in core diameters, which rise with flute count — and it
would have done that. It also encoded a law that does not survive contact with
the geometry. A guard against one error became an assertion of another. Prefer
a guard that pins a SOURCE and forbids an unsourced refinement, over one that
pins a trend.

---

## T-17 — the deflection integrator gives a fluted end mill a solid cross-section

`crates/rs_cam_core/src/tool/mod.rs:699`. `ToolDefinition::tip_deflection_mm`
integrates the cutter region section by section:

```rust
let d = self.cutter.lookup_diameter_at(axial_from_tip).max(D_FLOOR_MM);
let i_mm4 = std::f64::consts::PI * d.powi(4) / 64.0;
```

For a flat end mill `lookup_diameter_at` returns the **full cutting
diameter** (`tool/mod.rs:422`, the trait default). So the model bends a solid
cylinder. A real end mill has flutes cut into it, and the flutes are where it
bends.

## The size of it

`I` goes as the fourth power, so the error is the fourth power of the
diameter ratio. The equivalent diameters are the ones T-16 sourced (Kivanc and
Budak, Sabanci MSc thesis 2004, Tables 3.1 and 3.2):

| Flutes | Section used | Section wanted | Deflection error |
|---|---|---|---|
| 2 | 1.000 D | 0.889 D | 1.60x too stiff |
| 3 | 1.000 D | 0.841 D | 2.25x too stiff |
| 4 | 1.000 D | 0.748 D | 3.19x too stiff |

**The sign is the bad one.** The model under-states deflection, so every
deflection guard fires later than it should.

## Why no gate catches it

`tip_deflection_mm` is self-consistent and its own unit tests
(`tool/mod.rs:1205` onward) check ratios between materials and between
stick-outs. Nothing compares the absolute number against a measured or
published tool. The one place in the tree that DID carry a flute-relief term
was the closed-form predictor, and 2026-06-17 retired it from the magnitude
path. The term left the production model on that day and nothing recorded it.

## Do not fix it in `lookup_diameter_at`

That function has 14 other callers and they want the **engagement** diameter:
the chipload LUT query, the stepover, the `doc/d` ratio, the vendor lookup.
For those the full diameter is correct. Engagement diameter and bending
diameter are two quantities, and this register already holds three defects of
that same class (T-11, and the two in `CLEAR_WATER.md` section 3). The
correction belongs inside the region-2 loop of `tip_deflection_mm`, where the
quantity wanted is a bending section.

## What it costs to leave, and what it costs to fix

Leaving it: every deflection number in the product is 1.6x to 3.2x optimistic,
and the axial envelope, the post-sim gate, the Suggest back-off and
`feeds::efficiency` all read it.

Fixing it: deflection rises by the same factors, so guards fire earlier and
recipes get more timid. It also disturbs
`DEFLECTION_BACKOFF_TARGET_UM`, whose recorded "+36 % over-shoot" margin was
measured against the current too-stiff model. Both move in one change or
neither does.

**This needs an operator ruling before it ships**, for the reason T-16 gave:
the 2-flute figure rests on medium-low confidence literature (0.889 against
0.920, two of eight source rows disagreeing), and here it is load-bearing.

---

## T-18 — three feed lifts cap against the travel rate, not the cutting ceiling

A machine profile carries two different feed limits, and they are not
interchangeable:

- `max_feed_mm_min` — the gantry TRAVEL rate. What the axes can move at.
- `cutting_feed_ceiling_mm_min()` (`machine/mod.rs:136`) —
  `min(max_cutting_feed_mm_min ?? 6000, max_feed_mm_min)`. What the machine may
  CUT at. Its own doc says it never exceeds the travel rate.

The second exists only to express the difference. Step 7 of `feeds::calculate`
says so in a comment (`feeds/mod.rs:2223`):

> F4: the calculator emits CUTTING feeds — clamp at the cutting ceiling,
> not the gantry travel rate.

Three later sites then cap against the travel rate instead.

| Site | Caps against | Right? |
|---|---|---|
| `feeds/mod.rs:2225` (Step 7) | `cutting_feed_ceiling_mm_min()` | yes |
| `feeds/suggest/adaptive_entry.rs:421` | `cutting_feed_ceiling_mm_min()` | yes |
| `feeds/mod.rs:2349` (rubbing-floor lift) | `max_feed_mm_min * safety_factor` | **no** |
| `feeds/mod.rs:2381` (drill envelope clamp) | `max_feed_mm_min * safety_factor` | **no** |
| `feeds/suggest/adaptive_entry.rs:470` | `max_feed_mm_min * safety_factor` | **no** |

`adaptive_entry.rs` uses both, 49 lines apart, in one file.

## What it costs

A lift that fires after Step 7 can restore a feed ABOVE the cutting ceiling
Step 7 just enforced. On a profile with `max_feed_mm_min` 10 000, no explicit
`max_cutting_feed_mm_min` and `safety_factor` 0.75, Step 7 clamps at 6 000 and
the lift may target 7 500 — **25 % above the ceiling**, with no warning,
because the lift believes it is under a limit.

## Why no gate catches it

Each site is locally sensible. `max_feed_mm_min * safety_factor` reads like a
machine limit, and it IS one — just not the one that governs a cutting move.
The two quantities have the same units, the same order of magnitude and
similar names. Only the ordering makes it wrong, and no test asserts the
ordering.

## It is latent today, and that is not a defence

Verified: the rubbing floor caps chipload at 0.025 mm/tooth on every shipped
preset, so the lift target cannot currently reach the travel cap. That makes
this unreachable on our sample. It does not make it correct.

The operator ruling of 2026-09-17 applies directly: **a sweep that does not
fire is evidence about the sweep.** A profile with a higher `max_feed_mm_min`,
an explicit `max_cutting_feed_mm_min` well below travel, or a vendor row with
a higher floor, all move the lift toward the cap. The code would then cap a
cutting feed against a traverse rate.

## The fix

Replace all three with `machine.cutting_feed_ceiling_mm_min()`. Decide
deliberately whether the safety factor applies — Step 7 does NOT apply it to
the ceiling, and the three lift sites DO apply it to travel, so the two paths
disagree on that too. Pick one and state why.

## Relation to T-15

Same structure: a late step raises a feed that an earlier step clamped. T-15
is the power ladder's version, this is the machine ceiling's. A fix for either
that only patches its own site leaves the shape intact. Consider one guard
that re-checks every ceiling after the last lift, instead of three patches.

---

## How to add an entry

State what no gate can fail on, why the compiler or the suite cannot see it,
and what it costs if left. If a gate *could* catch it, fix the gate instead
and record that here rather than the symptom.
