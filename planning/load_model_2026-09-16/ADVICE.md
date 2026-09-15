# Force, power and wear — what the model should be

**Status: advice, not a plan.** Nothing here is implemented. Every number
below is computed from the coefficients already in `rs_cam_core`, not from
the literature directly, so they can be re-derived by anyone.

The operator's question, 2026-09-16: *"It feels like force/power should be
something that is shown, as I think that's the essential trade-off with going
up the chipload chart. What should the proper methods be, in terms of the
relation between these variables? The aim is to guide a user on feeds and
speeds and help them optimise the trade-off of speed to wear."*

---

## 1. The engine holds two force models, and they disagree

**`feeds/force.rs` — affine, chip-thickness aware.** Force per mm of axial
engagement:

```
Fc / ap  =  Ks · h  +  F_edge
```

Fitted to a real wood-milling study (woodresearch.sk 201905/12,
`Fc1z = 49.95·h + 5.30`, R² ≈ 0.99), scaled per material by `Kc / 35.1`.
This is a **two-term** model: a shearing term proportional to chip
thickness, plus a constant **edge** term that does not care how thick the
chip is. It is consumed by `tool_load::deflection`, `feed_modulation` and
`session::compute`.

**`tool_load/power.rs` — linear in MRR, constant specific energy.**

```
P_kW  =  2.0 · Kc · (ap · ae) · feed / 60e6
```

No chip-thickness term at all. Power is a straight function of volume
removal rate.

These cannot both be right, and the divergence is not small.

### The crossover, and where wood routing sits relative to it

```
h_crossover = F_edge / Ks = 2.650 / 24.98 = 0.1061 mm
```

The same for every material, because both coefficients scale by the same
`Kc` ratio. **Below 0.106 mm of chip thickness the edge term is the larger
one.** Typical wood-routing chiploads are 0.03–0.09 mm/tooth. Every one of
them is below the crossover: routing wood is an edge-dominated,
ploughing-dominated process, and a model with no edge term cannot see that.

### Measured on the fixture now on screen

6 mm 2-flute flat, 17 000 RPM, DOC 4.20, WOC 2.10, generic softwood
(`Ks = 24.98`, `F_edge = 2.650`, immersion ψ = 1.266 rad):

| fz (mm/tooth) | MRR | shipped `P` | two-term `P` | ratio | edge share | u (J/mm³) |
|---|---|---|---|---|---|---|
| 0.0380 (running) | 11 395 | 0.0067 kW | 0.0287 kW | **4.3×** | **83 %** | **151** |
| 0.0675 (vendor mid) | 20 242 | 0.0118 kW | 0.0324 kW | 2.7× | 74 % | 96 |
| 0.0850 (vendor max) | 25 490 | 0.0149 kW | 0.0346 kW | 2.3× | 69 % | 81 |

Two things to take from that table:

1. The shipped model **understates cutting power by 2–4×** in this regime,
   and the error grows as the chip gets thinner — exactly where an operator
   most needs the warning.
2. At the chipload this operation is actually running, **83 % of the cutting
   power is edge ploughing**, not cutting. That is the physical content of
   "below the band — burn risk", and today no number on any surface says it.

There is a second, separate inconsistency worth fixing at the same time.
Power multiplies `Kc` by `GRAIN_ANISOTROPY_FACTOR = 2.0` (→ 35.1 N/mm² for
softwood); `force.rs` anchors on `Kc` **without** that factor and fits
`Ks = 49.95` for hardwood. The two models are not merely different in shape,
they do not share a base constant.

---

## 2. Power, done from the same coefficients

Power is tangential force times cutting velocity. Splitting the affine force
into its two terms gives two terms of power, and they behave completely
differently:

```
P  =  Ks · MRR                              ← shear: proportional to volume
   +  F_edge · ap · Vc · (z · ψ / 2π)       ← edge: proportional to DISTANCE
```

with `Vc = π·D·n` and `ψ = arccos(1 − ae/r)`, which `force::immersion_angle`
already computes.

**The edge term contains no feed.** That is the whole finding. Halving the
feed at constant RPM halves the shear term and changes the edge term not at
all — so power does not fall proportionally to feed, it falls toward a
floor. The shipped model says it falls linearly to zero.

### Specific energy — the number that should be on screen

Divide through by MRR (`= ap · ae · fz · z · n`). Both `ap` and `n` cancel:

```
u  =  P / MRR  =  Ks  +  (F_edge · D · ψ) / (2 · ae · fz)        [J/mm³]
```

This is the single most useful equation for the product, for four reasons:

- **It is a wear proxy.** Energy that is not removing wood is heating the
  edge. `u` is how much you spend per mm³ of wood.
- **It is hyperbolic in chipload.** Halve `fz` and the ploughing half of the
  bill doubles. Running at 0.038 instead of 0.085 costs **86 % more energy
  per mm³** (151 vs 81) — and nearly all of the extra is heat.
- **It is independent of RPM.** `n` cancels. RPM buys rate, not efficiency.
- **It is independent of DOC.** `ap` cancels. Depth buys rate, not
  efficiency.

That is the trade-off the operator asked to see, in one number.

---

## 3. What this says about the derate direction

This settles the earlier question — *should the derate traverse the
constant-chipload line?* — with physics rather than preference. Today every
derate scales feed at constant RPM (`feeds/mod.rs:1731`), so the arrow
always drops straight down and always thins the chip.

| Derate | Straight down | Why |
|---|---|---|
| L/D overhang, depth tier, workholding | **correct** | Deflection is driven by force per tooth, `Ks·h + F_edge`. You must reduce `h`. Traversing at constant `h` keeps the force and achieves nothing. |
| Machine feed cap | **wrong** | You are at the ceiling; the only way to hold chipload is to drop RPM. Truncating feed thins the chip toward the rubbing floor — the opposite of what you want. |
| Power limit | **wrong-ish** | Only the shear term responds to feed. Thinning the chip raises `u`, so the real power saving is smaller than the linear model predicts, and you pay in wear. Traversing cuts both terms. |
| Safety factor | ambiguous | Conservative either way, but it always pushes toward the rubbing floor. |

The machinery for traversing already exists: `SpindleStrategy::MaxSpeed`
"walks the constant-chipload line up the speed axis". This is the same move,
downward.

And the corroborating symptom is already shipping: `ChiploadClampedToFloor`
fires *because* feed derates drive chipload into the rubbing floor. That
warning is the model arguing with itself.

---

## 4. What actually limits a hobby router — and what does not

`Vc` at 17 000 RPM on a 6 mm cutter is **5.3 m/s**. Even at a 24 000 RPM
ceiling it is 7.5 m/s. Published optimum surface speeds for carbide in wood
are an order of magnitude higher.

Two consequences, and they should shape the whole UI:

1. **Surface speed is never the binding constraint on this class of
   machine.** The thermal-wear regime that limits metal cutting is not
   reachable here.
2. **Power is never binding either.** Solving the two-term model for the
   feed at which 0.6 kW is reached gives a feed ceiling of order 10⁵ mm/min
   — two orders of magnitude above the machine's 4 000 mm/min cap. This
   matches the measurement already recorded in `feeds/mod.rs:1714`: across
   every shipped preset × ten species × three diameters, the power branch
   never fires and peak utilisation is 23.6 %.

**So the binding constraints are: the machine's feed and RPM caps, tool
deflection, and the rubbing floor.** An honest chart draws those and does
not pretend power is a live constraint.

This also means the guidance for these users is simple and nearly always
correct: **run the highest chipload the tool and the machine will take.**
Higher chipload is more efficient, cooler per mm³, and faster. It is limited
by tooth force, not by power or speed.

---

## 5. Recommendations

Ordered by value per unit of effort. Confidence is stated because some of
this is derivation and some is measurement.

### R1 — power consumes the same coefficients as deflection
**Confidence: high. Effort: small.** `force::affine_coefficients` is already
public and already consumed by three call sites. Give `predicted_power_kw`
the two-term form. This removes the 2–4× understatement, removes the
`GRAIN_ANISOTROPY_FACTOR` divergence, and makes deflection and power agree
about what a thin chip costs.

**This is a feed-moving recalibration** — it changes the power branch's
trigger point and therefore the recommended feed for any cut that trips it.
`feeds/mod.rs:1697` records that such a change must not be re-pinned
silently, and the literature matrix will move. It needs an explicit decision
and a re-baseline, not a quiet commit.

### R2 — publish specific energy as a first-class figure
**Confidence: high. Effort: small.** `u = P/MRR` in J/mm³, with the edge
share beside it. It is RPM- and DOC-independent, so it is the honest
efficiency number, and "83 % of your cutting power is ploughing, not
cutting" is the most actionable sentence this product could say to a hobby
user. It replaces the power line that currently reads 1 % on every job.

### R3 — draw the feasible corridor, not more lines
**Confidence: medium-high. Effort: medium.** The rubbing floor and the
deflection ceiling are both **iso-chipload rays**, and between them is the
region where the cut is neither burnishing nor breaking tools. Shade outside
them, the way the machine walls are already shaded, so the eye reads a
corridor rather than a set of lines. The vendor band then sits visibly
*inside* the corridor and the relationship becomes obvious.

Do **not** add a power contour. It would be drawn two orders of magnitude
off the top of the chart and would never bite (§4).

Note this is not a reversal of today's deletion. What was deleted were the
band's own edges and midpoint, drawn a second time. These are *different*
constraints that bound a different region.

### R4 — make the derate direction per-derate
**Confidence: medium. Effort: medium.** Deflection-driven derates reduce
chipload; feed-cap and power derates traverse the iso-chipload line by
reducing RPM. Same authorisation problem as R1 — it moves recommended
numbers.

### R5 — a wear/rate readout, once R1 and R2 land
**Confidence: medium. Effort: small once the above exist.** Two numbers
answer the operator's actual question: `u` (J/mm³, cost per unit wood) and
MRR (mm³/min, rate). Moving up the chipload axis improves both. Moving right
along the RPM axis improves rate only. That asymmetry *is* the advice, and
it can be stated in one line.

### R6 — a real tool-life model
**Confidence: low. Effort: large. Recommend deferring.** There is no wear
model in the engine today and no bench data to calibrate one. Taylor's
`V·T^n = C` is the standard, but its exponents for carbide in wood vary by
an order of magnitude across species and grind, and a fabricated constant
here would be worse than the honest silence the codebase currently keeps.
`u` (R2) is the right proxy until someone measures real tool life on this
machine.

---

## 6. What I am not confident about

- **The duty-cycle factor `z·ψ/2π` in the edge term** is the standard
  average-teeth-engaged expression. It is a derivation, not something I
  measured against the sim trace. Before R1 lands it should be checked
  against per-move engagement in an actual cut trace — the simulation
  already computes engagement, so the check is cheap.
- **Absolute magnitudes.** `force.rs` already flags its own numbers as
  "approximate / verify on a test cut". R1 makes power *consistent* with
  deflection; it does not make either one bench-validated. The honest claim
  after R1 is "these two models now agree", not "this is your spindle
  load".
- **The 2.0 grain-anisotropy factor.** Removing it from power is required
  for consistency with `force.rs`, but I have not traced why it was applied
  to power and not to the force fit. That history should be read before the
  factor is dropped — `planning/UNIFIED_LOAD_MODEL_2026-06-18.md` §6 and
  `KC_MILLING_CALIBRATION_2026-06-17.md` are the two documents to start
  from.
