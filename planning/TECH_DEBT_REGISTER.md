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
| T-8 | The power derate thins the chip, and only half the power responds | open — one sentry red |
| T-9 | A feed clamped onto a ceiling ships one rounding step above it | open |

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

**Fix:** R4 — make the derate direction per-derate. Deflection-driven derates
reduce chipload; feed-cap and power derates traverse the constant-chipload
line by reducing RPM, which cuts BOTH power terms (at fixed chipload, feed
is proportional to RPM, so the shear term is too, and the edge term is
proportional to `Vc`). Needs its own authorisation, for the same reason R1
did — it moves recommended spindle speeds.

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

## How to add an entry

State what no gate can fail on, why the compiler or the suite cannot see it,
and what it costs if left. If a gate *could* catch it, fix the gate instead
and record that here rather than the symptom.
