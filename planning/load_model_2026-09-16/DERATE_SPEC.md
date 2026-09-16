# Derate by the lever the constraint responds to

**Status: specification, not implemented.** Follows R1 (`73b84d69`), which
made the power model honest and thereby exposed that the derate mechanism
cannot satisfy the constraint it exists to satisfy.

Read `ADVICE.md` for the force/power physics and `SPEC.md` for what already
shipped. This document is about **which dial to turn when a limit binds**.

---

## The one-sentence problem

The engine has one lever — reduce the feed rate — and it pulls it for every
constraint. Three of the four constraints do not respond to it.

---

## The four limits, and what each actually responds to

|  | Speed limit | Load limit |
|---|---|---|
| **CNC / gantry** | max feed rate (kinematic) | **feed force** — set by chip thickness × engagement, NOT by feed rate |
| **Spindle / motor** | rpm range | **power** — set by removal rate `ap·ae·feed`, NOT by rpm alone |

The speed limits are trivial and already enforced. The load limits are the
ones that bite, and they **cross over**: spindle power depends on feed, and
gantry force depends on rpm (through chip thickness). That is why "the CNC
owns the feed and the motor owns the rpm" is the right idea about ownership
and the wrong idea about variables.

### Measured, and it is counterintuitive

Ø12 4-flute bull nose, ap 8.4, ae 4.2, `GenericHardwood` (Kc 35.1, the
anchor wood). White oak is Kc 37.26 and moves every figure up about 6 %.
The conclusions do not change.

**Holding chip thickness at 0.0625 mm/tooth and varying rpm:**

| rpm | feed mm/min | force/tooth N | gantry ~N | spindle W |
|---|---|---|---|---|
| 4 500 | 1 125 | 69.5 | **28.0** | 269 |
| 9 000 | 2 250 | 69.5 | **28.0** | 538 |
| 18 000 | 4 500 | 69.5 | **28.0** | 1 076 |

**Feed quadruples and the gantry force does not move.** Spindle power
quadruples.

**Holding rpm at 9 000 and varying chip thickness:**

| fz | feed mm/min | force/tooth N | gantry ~N | spindle W |
|---|---|---|---|---|
| 0.0250 | 900 | 54.5 | 22.0 | 459 |
| 0.0625 | 2 250 | 69.5 | 28.0 | 538 |
| 0.1000 | 3 600 | 84.5 | 34.1 | 617 |

Chip thickness 4× raises gantry force ~55 %; spindle power moves 34 %.

**So: feed rate loads the spindle. Chip thickness loads the gantry.**

---

## The spec — one rule per constraint

### 1. Machine feed cap binds

**Lever: reduce RPM, hold chip thickness.**

You are at the gantry's velocity ceiling. Feed cannot rise. The only way to
restore chip thickness is to lower rpm. Today the engine truncates the feed
at the cap, which thins the chip toward the rubbing floor — the opposite of
what is wanted.

### 2. Spindle power binds

**Lever: reduce depth of cut. On a constant-torque VFD, nothing else works.**

Measured on the `bull_12mm_pocket_oak` cut (`ADVICE.md` and T-8):

| Holding chipload, lowering rpm | VFD utilisation | Router utilisation |
|---|---|---|
| 9 000 rpm | **120 %** | 95 % |
| 6 000 | **120 %** | 63 % |
| 4 500 | **120 %** | 47 % |
| 3 000 | **120 %** | 32 % |

On `PowerModel::VfdConstantTorque`, available power scales with rpm exactly
as required power does, so **utilisation is rpm-invariant** and traversing
the constant-chipload line achieves nothing. On `PowerModel::ConstantPower`
(the routers) available power is fixed, so traversing works.

Reducing depth works on both: 120 % → 85 % → 57 % → 36 % at ap 8.4 → 6.0 →
4.0 → 2.5 mm.

**Therefore the power derate must branch on `PowerModel`:**

- `ConstantPower` → traverse the constant-chipload line (drop rpm and feed
  together).
- `VfdConstantTorque` below rated rpm → **rpm cannot help**. Reduce `ap`.
  If the operation's depth is not the engine's to change, the honest
  outcome is a refusal that names depth as the lever, not a feed cut that
  does not work.

`feed_modulation.rs` already says this in a comment on the deflection
branch: *"Dropping DOC/stepover is the real fix (out of scope for per-move
feed)."* The same is true of power and it is not said there.

### 3. Tool deflection binds

**Lever: reduce chip thickness. Do NOT hold chipload.**

Deflection is `compliance × ap · (Ks·fz·sin θ + F_edge)`. It contains no rpm
and no feed rate. Holding chip thickness and slowing everything down leaves
the tool bending exactly as far, for a slower job. This is the one case
where the current behaviour — thin the chip — is correct, and the one where
a blanket "hold chipload" rule would silently disable the protection while
appearing to act.

### 4. Gantry feed force binds

**Lever: reduce the depth of cut, then the width. NOT the chip thickness.**

Still not modelled — see the gap below — but the research of 2026-09-16
settled which lever it would need, and the answer is not the one this
document first gave.

Measured on the reference cut, each dial cut to a third of its value:

| Dial | Gantry push | Change |
|---|---|---|
| as it stands | 38.4 N | |
| chipload 0.0625 -> 0.0208 | 35.9 N | **-6 %** |
| width ae 4.2 -> 1.4 | 21.1 N | -45 % |
| depth ap 8.4 -> 2.8 | 12.8 N | **-67 %** |

At a chipload of zero the push is still 93 % of its full value. The edge term
carries the gantry load, and the edge term contains no chipload. Depth and
width scale it; chip thickness barely touches it.

This corrects the first version of this document, which said the gantry force
is "set by chip thickness". The part that was right is that it does not
respond to the FEED RATE at all: hold the chipload and take the feed from
1 125 to 4 500 mm/min, and the push does not move off 38.4 N.

---

## The gap — gantry feed force

There is no thrust, stall or feed-force limit anywhere in `rs_cam_core`.
Verified by search: deflection, power and chipload are modelled; the
question "will the gantry push this hard without losing steps" is not asked.

It is the failure mode a lighter machine meets first, and it fails by losing
position rather than by running out of watts — so it is a **force** limit,
not a power one. The measured feed force on the reference cut is 22–34 N,
which is trivial as wattage (1.4–3.3 W, under 1 % of spindle power) and
significant against a hobby gantry's thrust.

**What it needs before it can be built:**

1. **A rated axis thrust per machine profile**, in newtons.
   `MachineProfile` carries no such field. This is still the blocker.
   `THRUST_RESEARCH.md` now gives ballpark figures, and they span 20:1
   across machine classes, so one constant for all machines is not an
   option:

   | Class | Thrust | Kind |
   |---|---|---|
   | Belt gantry (Shapeoko, X-Carve) | **85 N** | measured skip |
   | Belt gantry | 132-220 N | derived stall, not usable |
   | Ballscrew (Onefinity X-50) | 1 074 N | derived, unmeasured |
   | Rack and pinion (Avid NEMA 34) | 1 537 N | derived, unmeasured |

   A skip happens at 0.5 to 0.6 of stall. No maker publishes a thrust
   rating, so every figure above is derived or measured by a hobbyist.

2. **A feed-force ratio — RESOLVED, and 0.5 was wrong.** The ratio is not a
   fitted constant. It falls out of the engagement geometry:
   `F_feed(phi) = -(F_t*cos(phi) + K_r*F_t*sin(phi))`. The peak over the
   engagement arc runs **0.53 to 1.0**, and it never approaches 0.5. In the
   adaptive clearing band (ae/D 0.05 to 0.20) it is **0.79 to 0.97**, so the
   old placeholder understated the feed load by about a factor of two exactly
   where a hobby router removes most of its material. Use the peak, not the
   mean: a stepper skips on the worst tooth.

   `K_r` for clear softwood is below 0.2 (Caceres 2018, white spruce). Metal
   uses 0.3 to 0.5. The LOW wood value pushes the ratio toward 1.0, so wood
   is the demanding case, not the easy one.

3. **A decision on what to do when it binds** — settled above: reduce the
   depth first, then the width. Never the feed rate, and thinning the chip
   is close to useless.

Logged in `TECH_DEBT_REGISTER.md` as T-10. The research is in
`THRUST_RESEARCH.md`, and `derate_levers.py` checks every rule here.

---

## Sentries

| Sentry | Pins |
|---|---|
| `the_derate_uses_the_right_lever_g_lever` | For each constraint, the derate moves the variable that constraint responds to. Table-driven, one arm per constraint. |
| `a_vfd_power_limit_is_not_solved_by_rpm_g_lever` | On `VfdConstantTorque`, utilisation is invariant under a constant-chipload traverse — so a power derate that only lowers rpm is rejected. This is the arm that encodes the measurement above, and it must fail if someone "simplifies" the power derate back to one branch. |
| `deflection_still_thins_the_chip_g_lever` | The deflection derate is NOT converted to a traverse. Its non-vacuity partner: at constant chipload, predicted deflection is unchanged across rpm. |
| `bull_12mm_pocket_oak` (existing, currently RED) | Returns to `moderate` when the power derate uses the right lever. **Do not re-pin it to make it green — it is the acceptance test for this work.** |

---

## Order of work

```
1. Branch the power derate on PowerModel            ← fixes the red cell
2. Feed-cap derate traverses instead of truncating
3. Confirm the deflection derate is untouched       ← a guard, not a change
─────
4. Gantry force limit                               ← blocked on thrust data
5. Helix wrap in the engaged-edge count (T-7)       ← AFTER 1, see below
```

**T-7 must come after step 1.** Correcting the helix wrap raises the edge
term, which makes the power branch bind harder and more often — driving the
feed derate, the wrong lever, harder still. It compounds with this problem
rather than cancelling it, and it lands safely only once the power derate
pulls the right dial.

---

## What is already shipped, for context after a compaction

`73b84d69` R1 — power built from the affine force model, 8.61× at the
reference fixture, edge share 83 %. `1a92cb56` Match vendor chipload.
`e796a4d4` the corridor. `921aa0e3` the chipload verdict row. `eac5552a`
`feeds::efficiency`. `ad89f8be` one deflection solver. `b68c484d` the dead
sibling-row scan deleted. `806dd5c4` `ui/feeds/` layout. `41d0ee5c` the
air-cut regression.

Known red: `bull_12mm_pocket_oak` (this work fixes it) and
`modulation_raises_cutting_chipload_toward_band` (pre-existing at
`7a5fdad4`, unrelated, still unowned).
