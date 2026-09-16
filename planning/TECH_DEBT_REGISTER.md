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
| T-1 | `enumerate_matching_rows` is dead; `pub` hides it | open |
| T-2 | LH-1's guard is syntactic and a closure defeats it | open |
| T-3 | Cross-crate sentries never run in a per-crate gate | open |
| T-4 | `predict_peak_deflection_um` returns `0.0` for every refusal | open |
| T-5 | `feeds/mod.rs` 4 144 lines, `suggest.rs` 5 667 | open |
| T-6 | Two implementations of one physical model | closed |
| T-7 | Two definitions of "teeth in cut", differing by helix wrap | open |
| T-8 | The power derate thins the chip, and only half the power responds | **closed** `348facbb` |
| T-9 | A feed clamped onto a ceiling ships one rounding step above it | open |
| T-10 | No gantry feed-force limit exists; the steppers are unmodelled | open — needs a thrust rating |
| T-11 | Feed modulation multiplies mm by a fraction of a different quantity | **closed** `bf8824ad` |
| T-12 | A depth recommendation is dropped for 14 of 24 operations, silently | **closed** `0dc9141f` |

---

## T-1 — a public function with no callers, and no warning

`crates/rs_cam_core/src/feeds/vendor_lookup.rs:315`
`enumerate_matching_rows` has **zero callers** across `crates/` since D-1
(commit `b68c484d`) deleted the sibling-row scan. The only remaining mention
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
commit `790b033c`, and it went unnoticed for a day. Fixed in `41d0ee5c`.

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
  `force.rs` should own. Closed by N-2 (`ad89f8be`): one implementation,
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

**Fix:** give `feeds::force` one `teeth_in_cut(z, ψ, ap, helix, r)` and have
both consume it, with the helix term either carried by both or by neither.
Deliberately out of R1's scope: adding helix wrap is a second feed-moving
recalibration, and R1 already moved the recommended numbers once.

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

**CLOSED by `348facbb`** (2026-09-16). The engagement ladder shipped and
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

**CLOSED by `bf8824ad`** (2026-09-16). `PerMoveEngagement` now carries
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

**CLOSED by `0dc9141f`** (2026-09-16). The funnel honours the `bool` and
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

## How to add an entry

State what no gate can fail on, why the compiler or the suite cannot see it,
and what it costs if left. If a gate *could* catch it, fix the gate instead
and record that here rather than the symptom.
