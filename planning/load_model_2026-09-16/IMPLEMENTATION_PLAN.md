# How to build the derate-lever fix

Companion to `DERATE_SPEC.md`, which says WHAT the rules are, and to
`THRUST_RESEARCH.md`, which supplies the constants. This document says
where the code goes.

Read `derate_levers.py` first if you want the rules to be self-evident. It
runs in a second and every rule below is one of its checks.

---

## The problem, in one paragraph

The engine has one lever — reduce the feed rate — and it pulls it for every
constraint. A power limit on a constant-torque VFD does not respond to it. A
machine feed cap responds to it by thinning the chip toward rubbing, which is
the opposite of what is wanted. Only tool deflection genuinely wants a thinner
chip, and that is the one case the current code gets right.

---

## The good news: the seam already exists

This looked like new machinery. It is not. Three pieces are already in place.

**1. The constant-chipload traverse is already written, and already runs.**
`feeds/mod.rs:1397` "Step 2b: Spindle-speedup along the constant-chipload
line" raises the RPM under `SpindleStrategy::MaxSpeed` and lets the feed
formula scale the feed with it, so the chipload is preserved. That is exactly
the manoeuvre the feed-cap and router-power derates need. It needs to run
downward as well as upward.

**2. The two axes are already separated, deliberately.**
`FeedsDerates::combined_factor` (`feeds/mod.rs:602`) composes the CHIPLOAD
multipliers and intentionally excludes `spindle_speedup`, with this comment:

> Spindle speedup walks the constant-chipload line (RPM and feed scale
> together), so chipload is unchanged.

So a traverse belongs on `spindle_speedup` and a genuine chipload cut belongs
on `power_limit` / `feed_clamp`. Put each on the right axis and the
`effective_chip_load_mm` identity keeps holding for free.

**3. Two tests already pin that identity**, and will catch a traverse booked
to the wrong axis: `chipload_thinning_magnitude_survey.rs:653` and
`vendor_sidebyside_chipload.rs:355`, both computing
`target × combined_factor() × spindle_speedup`.

**Consequence for the plan: do not invent a new derate field for a traverse.**
Lower `spindle_speedup` below 1.0. If you find yourself adding a factor to
`combined_factor` for a case where the chipload does not change, you are on
the wrong axis and those two tests will say so.

---

## Phase 1 — the power derate branches on the spindle type

**Fixes the red cell. Do this first.**

**Where:** `feeds/mod.rs`, Step 6, the `feed_for_kw` block at ~1800.

**Now:** when power is exceeded, `power_factor = commanded_cap / sf /
raw_feed` and the feed is scaled at constant RPM.

**Change it to branch on `machine.power`:**

| `PowerModel` | Lever |
|---|---|
| `ConstantPower` | Traverse. Lower RPM and feed together, book it on `spindle_speedup`. |
| `VfdConstantTorque`, below `rated_rpm` | **The traverse is useless.** Available and required power both scale with RPM, so utilisation does not move. Go to the engagement ladder below. |
| `VfdConstantTorque`, at or above `rated_rpm` | Flat above the knee, so a traverse helps down to `rated_rpm` and no further. **Unreachable today — see below.** |

The third row is correctness for user-defined profiles only. `power_at_rpm`
(`machine.rs:259`) is `rated × min(n, rated_rpm) / rated_rpm`, so the curve is
flat above the rated speed. But the only shipped VFD preset sets
`rated_rpm = 24 000` and `max_rpm = 24 000` (`machine.rs:186-193`), so the flat
region sits exactly at the top of the range and cannot be entered. Write the
branch, but do not expect a test fixture to exercise it without a custom
profile — and do not let a sentry for it pass vacuously.

### The trap: the machine where the traverse works has discrete speeds

This is the one that will bite. The traverse helps on `ConstantPower`, and the
`ConstantPower` preset is the Makita, which is `SpindleConfig::Discrete` with
speeds 10 000 / 12 000 / 17 000 / 22 000 / 27 000 / 30 000.

`clamp_rpm` (`machine.rs:243`) snaps a discrete spindle to the NEAREST speed.
**Nearest rounds up about half the time**, and a power-limited traverse that
rounds up raises the power it was called to reduce:

| Traverse asks for | `clamp_rpm` returns | |
|---|---|---|
| 21 000 | 22 000 | **up** |
| 19 000 | 17 000 | down |
| 16 000 | 17 000 | **up** |
| 11 500 | 12 000 | **up** |
| 9 000 | 10 000 | **up** |

So a downward traverse must round DOWN to the next available speed, not to the
nearest. Calling `clamp_rpm` directly here is a defect, not a shortcut.

Two further consequences on a discrete spindle:

- **The floor is 10 000 rpm.** The traverse cannot go below the lowest listed
  speed, so it will often be unable to clear the limit on its own. The
  remainder has to fall through to the existing clamp-and-warn path rather
  than silently stopping part way.
- **The traverse is quantised.** It will overshoot or undershoot the exact
  ratio. Whatever the traverse does not deliver must be booked honestly, per
  Phase 2's rule below.

Add `MachineProfile::next_rpm_at_or_below(rpm)` beside `clamp_rpm` rather than
open-coding the search at the call site. Step 2b will want the upward twin
eventually, for the same reason.

**The refusal path already exists and is already correct.** When
`feed_for_kw` returns `None`, the current code leaves the feed alone and warns,
and the comment at `feeds/mod.rs:1806` already says why:

> The real fix is less DOC, less stepover or a lower RPM, none of which a feed
> derate can reach for.

Phase 1 makes "a lower RPM" reachable for the two cases where it helps, and
makes the VFD case take the existing refusal path instead of a feed cut that
does nothing.

**Acceptance:** `tests/literature_matrix` cell `bull_12mm_pocket_oak` returns
to `moderate`. **Do not re-pin it.** It is the acceptance test.

---


---

## The engagement ladder — REVISED 2026-09-16 after the DOC survey

The first version of this plan said the VFD branch should "recommend a
shallower depth". **Two read-only surveys killed that as written.** See
`DOC_SURVEY_MODEL.md` and `DOC_SURVEY_RUNTIME.md`, and register items T-11
and T-12.

### What the surveys found

**1. Depth is not a free parameter for most operations.** For 14 of the 24
operation types the calculator's depth recommendation is silently discarded
(T-12). Worse, for many of those the depth is not the engine's to choose at
all:

| Operation | Why the depth is not free |
|---|---|
| V-carve | depth is `dist / tan(half_angle)` per point (`vcarve.rs:87-92`). Capping it flat-bottoms the widest strokes. |
| Chamfer | depth IS the chamfer width the user asked for (`chamfer.rs:41-43`). |
| Inlay | depth sets the male/female fit (`inlay.rs:96`). A unilateral cut breaks the joint. |
| 7 surface-finishing ops | no axial parameter exists. The model surface sets the depth. |
| Drill | the depth is the hole. |

**2. Where depth IS free, it is quantised.** Every 2.5D operation uses
`DepthDistribution::Even` (`depth.rs:16-19`), so the realised depth is
`total / ceil(total / dpp)`. On a 12 mm pocket only 4.00, 3.00, 2.40, 2.00,
1.71, 1.50 and 1.33 are reachable. **"Reduce the depth by 20 percent" is not
expressible.** A proposal must be snapped to a realisable `total / n` or the
engine reasons about a depth the machine will not cut.

**3. The loop is closed only by regeneration.** The runtime power verdict
reads a MEASURED depth from the dexel (`tool_load/power.rs:422`), not the
scalar the recommendation changes (`feeds/mod.rs:1782`). They meet only when
the toolpath is regenerated. The default `ApplyScope` is `Speeds`, which never
writes the geometry.

### The generic shape, and it is not "reduce the depth"

**Ask what this operation can give up, in order, and stop at the first thing
it can actually accept.**

```
1. RPM traverse        — only when PowerModel says it helps
2. Axial depth         — only when set_depth_per_pass returns true,
                         and snapped to a realisable total/n
3. Radial width        — the wider-reaching lever; more operations expose
                         a stepover than expose a depth per pass
4. Refuse, and NAME the lever the user holds
```

Rung 4 is not a failure state. For a chamfer it is the only correct answer:
the depth is the feature the user asked for, and the engine must say "this cut
is over power at the chamfer width you specified" rather than quietly cutting
a smaller chamfer.

**The enabling change is to stop discarding the refusal signal.**
`set_depth_per_pass` already returns a `bool` for exactly this purpose and
`feeds/suggest.rs:877` throws it away. Honour it, and the ladder can ask each
rung "can this operation do it?" instead of guessing. That is T-12, and it is
now a prerequisite rather than a nice-to-have.

### What this means for sequencing

**T-12 moves ahead of Phase 1.** Without it, a depth proposal is silently
dropped for most operations and the user sees a warning with no fix and no
statement that the fix was discarded.

**T-11 also moves ahead of Phase 1 IF the modulator is in the path.** The
modulator's effective depth is proportional to the square of the nominal depth
because of the unit defect, so its response to a depth change is roughly
quadratic. Any reasoning about depth that passes through it is unsound until
it is fixed. Establish first whether the modulator sits on this path at all —
`DOC_SURVEY_RUNTIME.md` could not resolve whether its output reaches the power
verdict. If it does not, T-11 is still a real defect but it does not block.

### Revised order

```
T-12  honour the set_depth_per_pass refusal            <- prerequisite
T-11  fix the modulator's depth units, IF in path      <- check first
Phase 1  power derate: the engagement ladder           <- the red cell
Phase 2  feed cap traverses
Phase 3  deflection guard (sentry only)
Phase 4  gantry force limit                            <- blocked on data
Phase 5  helix wrap (T-7)                              <- must follow Phase 1
```

### One unverified oddity, flagged not fixed

On the `ApplyScope` path the V-carve `max_depth` envelope clamp runs on the
scratch clone and is then discarded, because the copy-back never copies
`max_depth`. It survives only through `resolve_operation_invariants`
(`suggest.rs:1406-1414`). No test pins it. The survey could not determine
whether this is deliberate. **Do not change it as part of this work** —
establish intent first.

---

## Phase 2 — the feed cap traverses instead of truncating

**Where:** `feeds/mod.rs`, Step 7, ~1836.

**Now:** `feed = machine_cut_ceiling` and `feed_clamp_factor =
machine_cut_ceiling / feed`, which thins the chip.

**Change:** reduce the RPM by the same ratio so the chipload holds, and book
it on `spindle_speedup`. `feed_clamp` then stays at 1.0, because the chipload
did not change.

**The traverse will often be partial**, for the two reasons Phase 1 sets out:
the RPM floor (6 000 on the VFD, 10 000 on the Makita) and, on a discrete
spindle, the quantisation. Book the split honestly:

- the part the traverse delivered goes on `spindle_scale`;
- the remainder falls through to the existing truncation, and `feed_clamp`
  carries **only that remainder**.

A traverse that stops part way and reports a full one is the same defect as
the kW column in the bench: a reading that hides a moving denominator.

---

## Phase 3 — confirm the deflection derate is untouched

**No code change. This phase is a guard.**

Deflection is `compliance × ap × (Ks·fz·sin θ + F_edge)`. It has no RPM term
and no feed term. Thinning the chip is correct here, and a blanket "hold the
chipload" rule applied to this path would disable the protection while
appearing to act.

Add the sentry. Change nothing.

---

## Phase 4 — the gantry feed-force limit

**Still blocked, but now specifiable.** See T-10.

**4a. `MachineProfile` gains the rating.** Follow the `kinematics` precedent
at `machine.rs:99`, where the doc says "the absence of kinematics IS the
feature flag":

```rust
/// Rated axis thrust (N) before the drive loses position. `None` on
/// every built-in preset — absence IS the feature flag, as with
/// `kinematics`. No manufacturer publishes this figure; see
/// planning/load_model_2026-09-16/THRUST_RESEARCH.md.
#[serde(default)]
pub axis_thrust_n: Option<f64>,
```

**Do not fill this in for the built-in presets.** The research spans 20:1
across machine classes (a measured 85 N for a belt gantry, a derived 1074 N
for a ballscrew, a derived 1537 N for rack and pinion) and every non-belt
figure is unverified arithmetic. A fabricated constant is worse than the
present honest silence.

**4b. The feed-force ratio goes in `feeds/force.rs`**, beside
`immersion_angle` and `affine_coefficients_for_kc`, because it is the same
model:

```rust
/// Peak feed-direction force over peak tangential force, across the
/// engagement arc. Geometry, not a fitted constant.
pub fn peak_feed_force_ratio(psi: f64, fz: f64, ks: f64, f_edge: f64,
                             kr: f64, climb: bool) -> f64
```

Three things this must carry in its docs, all measured:

- **Use the peak, not the mean.** A stepper skips on the worst tooth.
- **The ratio runs 0.53 to 1.0** and never approaches the 0.5 that an earlier
  draft of the spec assumed. In the adaptive band it is 0.79 to 0.97.
- **The ratio is independent of the species.** Both forces scale by
  `Kc / 35.1`, so the scale cancels — checked identical to four decimal places
  across a 4.7x material span. Only the absolute force needs the material.

`kr` is 0.15 for softwood (Caceres 2018). Metal uses 0.3 to 0.5, and the LOW
wood value pushes the ratio toward 1.0, so wood is the demanding case.

**4c. The lever, when it binds: reduce the depth, then the width.** Not the
chip, and never the feed rate. Cut each dial to a third and the push moves by
−67 % (depth), −45 % (width), −6 % (chipload). At a chipload of zero, 93 % of
the push remains, because the edge term carries the load and the edge term has
no chipload in it.

**4d. Refuse when the rating is absent.** Follow the `MaterialUnvalidated`
convention: no rating, no gate, no fabricated reading. A `DeflectionCapRefusal`
-shaped enum is the local precedent.

---

## The honesty problem this creates, and it is not cosmetic

`spindle_speedup` going below 1.0 makes two existing statements false.

**1. The UI sentence.** `rs_cam_viz/src/ui/feeds/why.rs:128` reads the field
and prints:

> Spindle policy MaxSpeed lifted it ×{speedup:.3} toward the spindle ceiling

After Phase 1 a power-derated cut would render "MaxSpeed lifted it ×0.500".
That is the same defect family this programme has been clearing all along —
a reading that names the wrong cause.

**2. The field name.** A "speedup" of 0.5 is a lie in the type system.

**Fix both together, in Phase 1, not afterwards.** The field carries a factor
and needs to carry a reason:

```rust
pub spindle_scale: f64,          // renamed; may now be < 1.0
pub spindle_scale_reason: SpindleScaleReason,
// Unchanged | MaxSpeedPolicy | PowerLimit | FeedCap
```

Then `why.rs` selects its sentence from the reason instead of assuming the
policy. `combined_factor` still excludes it, for the unchanged reason.

Cost: mechanical. One field rename across `feeds/mod.rs`, `why.rs:128` and the
two identity tests named above.

---

## Sentries

| Sentry | Pins |
|---|---|
| `bull_12mm_pocket_oak` (exists, RED) | Returns to `moderate` by Phase 1. **The acceptance test. Do not re-pin it.** |
| `a_vfd_power_limit_is_not_solved_by_rpm` | On `VfdConstantTorque` below rated RPM, utilisation is invariant under a constant-chipload traverse, so a power derate that only lowers RPM is rejected. The arm that must fail if someone collapses the branch back to one case. |
| `a_router_power_limit_is_solved_by_rpm` | Its non-vacuity partner. On `ConstantPower` the same traverse DOES reduce utilisation. Without this the sentry above passes on a build that never traverses at all. |
| `the_feed_cap_holds_the_chipload` | At the cap, the effective chipload is unchanged rather than truncated toward the rubbing floor. |
| `deflection_still_thins_the_chip` | Phase 3's guard. Plus: at constant chipload, predicted deflection does not move across RPM. |
| `a_traverse_is_booked_to_the_speed_axis` | After a traverse, `combined_factor()` is unchanged and `spindle_scale` carries it. Stops the identity in the two existing tests being satisfied by accident. |
| `the_spindle_scale_states_its_reason` | A scale below 1.0 never renders as the MaxSpeed sentence. |
| `the_gantry_gate_refuses_without_a_rating` | Phase 4. No `axis_thrust_n`, no reading. |

---

## Order, and what blocks what

```
Phase 1  power derate branches on PowerModel        <- fixes the red cell
   + the spindle_scale rename and reason, together
Phase 2  feed cap traverses
Phase 3  deflection guard (sentry only)
─────  the above is the whole behavioural fix
Phase 4  gantry force limit                         <- BLOCKED on a thrust rating
Phase 5  helix wrap, T-7                            <- must follow Phase 1
```

**Phase 5 must not come first.** Correcting the helix wrap raises the edge
term, which makes the power branch bind harder and more often. Before Phase 1
that drives the feed derate — the wrong lever — harder still. The two compound
rather than cancel.

**Phase 4 is not scheduled.** It needs a number nobody publishes. The useful
next step is not code: it is one afternoon with a fish scale on the actual
machine, which would turn the weakest row in `THRUST_RESEARCH.md` into the
only measured one for a non-belt drive in existence.

---

## Risks

**This moves recommended spindle speeds.** Same class of change as R1, which
needed its own authorisation for the same reason. Expect literature-matrix
movement beyond the one red cell, and read every moved cell before re-pinning
any of them.

**The rubbing floor at Step 9b interacts.** A traverse holds the chipload, so
it should REDUCE how often Step 9b's `ChiploadClampedToFloor` fires. If it
fires more after Phase 1, the traverse is being booked to the chipload axis.
That is the failure mode to watch for, and the two identity tests are the
detector.

**`modulation_raises_cutting_chipload_toward_band` is already red** and is
unrelated (pre-existing at `7a5fdad4`, still unowned). Do not let it be
absorbed into this work's result, and do not let it mask a new failure.

---

## What is already verified, so nobody re-derives it

`derate_levers.py` checks all of it and passes. The model in it reproduces
538 W and 28.0 N on the reference cut, which is the guard that its copied
constants have not drifted from the Rust.

- VFD utilisation is flat at 120 % across 9 000 / 6 000 / 4 500 / 3 000 rpm.
- The same traverse on a router: 95 % → 63 % → 47 % → 32 %.
- Depth works on both: 120 % → 85 % → 57 % → 36 % at ap 8.4 → 6.0 → 4.0 → 2.5.
- Side force is identical at every RPM from 3 000 to 18 000 at fixed chipload.
- At a chipload of zero the side force is still 44.5 N of 69.5 N.
- The feed-force ratio never drops to 0.5 at any immersion.
- Two independent wood datasets put the shipped `Ks` within about 10 %, once
  the material scaling is applied. See `THRUST_RESEARCH.md` 2.5.1.
