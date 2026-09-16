# What limits a cut, what we can say about it, and what to build

Answers three questions asked on 2026-09-17: where the helix work would land,
whether machine rigidity matters more than power, and what the UX should be.

The short version: **stop the helix work, because it does not touch power.
Rigidity does not just matter more than power — it is currently the entire
depth model, and it is a rule of thumb that cannot tell balsa from ipe. One
afternoon with a dial indicator and a luggage scale would fix that and the
gantry gap at the same time.**

---

## 1. The measurement that reframes it

`Generic Wood Router`, pocket roughing, through the real apply path:

| Tool | The calculator chose | Rigidity cap `0.2 × D` | **What ships** |
|---|---|---|---|
| Ø3.2 | 2.22 mm | 0.64 mm | **0.64 mm** |
| Ø6.0 | 4.20 mm | 1.20 mm | **1.20 mm** |
| Ø12.0 | 8.40 mm | 2.40 mm | **2.40 mm** |

The shipped depth is the rigidity cap. Exactly, every time.

Across 54 recipes — three tools, six woods, every machine preset:

- **The rigidity cap set the depth on 100 %.**
- **The power constraint bound on 4 %.**

And it only bound on 4 % *because* rigidity had already cut the depth by
3.5×. If the calculator's depth actually shipped, power would bind
constantly.

**So all the load-model work lands on a geometry that a constant then
overwrites.** That is the fact worth absorbing before deciding anything else.

### The uncomfortable corollary

The recommendation's `power_kw` is computed inside `calculate` at the
calculator's depth — Ø12 at 8.40 mm, 0.291 kW. The apply path then clamps to
2.40 mm. The user is shown a power figure for a cut the machine will not
make, about 3.5× too high.

This is the same shape as `G-SUGGEST-STALE-GEOM`, which pinned the *feed*
against final geometry in 2026-08. Power was not pinned with it. Worth a
sentry regardless of what else is decided.

---

## 2. Is rigidity more important than power?

**Empirically yes, overwhelmingly. That is the problem, not the answer.**

`doc_roughing_factor × diameter` is a single number per machine class. It
knows nothing about:

- the **material** — the same depth in balsa and in ipe, which differ 4.7× in
  cutting force;
- the **tool** — stickout, flute count, carbide against HSS;
- the **spindle** — a 0.71 kW router and a 1.5 kW VFD get the same depth;
- the **workholding** — a part screwed to the bed and one on tape get the
  same depth.

Every one of those is already in the model. The cap ignores all of them.

It is not wrong to have such a rule. It is a stand-in for something real, and
the honest reading is that **rigidity is doing the job a machine-force model
should do, badly, because we never built the machine-force model.**

### What it is actually trying to say

Three physical things hide inside `0.2 × D`:

1. **The gantry deflects under cutting load.** A force pushes the tool off
   line; the cut goes oversize and the finish suffers. This is a stiffness in
   N/mm and we model none of it.
2. **The gantry loses position under too much push.** The stepper skips. This
   is a thrust in newtons — the open item in the register.
3. **Deep cuts chatter.** A resonance, not a static limit.

The first two are measurable in an afternoon and would replace the fudge with
physics. The third is genuinely hard and we should not pretend otherwise.

---

## 3. The helix: do not build it

Settled by measurement and already written up. A helix moves engagement in
time, not in amount: the mean engaged edge is flat across 0° to 45° and equal
to what the power model already uses.

The helix matters a great deal for chatter, noise, peak deflection and
whether the cut is continuous — the swing falls 2.6× from 0° to 30° on the
reference cut. **None of that is power.**

**Recommendation: no chatter surface, no new sim metric, nothing.** It is a
negative result that removes a variable. Note it and move on. Chatter
prediction needs a modal model of the specific machine and is a research
programme, not a feature.

---

## 4. The UX

### The principle

The operator asks one question: *can my machine do this cut, and if not, what
do I change?* Everything else is detail behind a hover.

Today the engine answers with four limits, of which one fires on every recipe
as a geometric constant and three almost never fire.

### What to show

**One headline, naming the binding limit and the lever.**

```
This cut runs at 78 % of what your machine can give.
Depth is set by machine rigidity — a rule of thumb, not a measurement.   ⓘ
```

Two things that line does which the current surface does not:

- It names **what is binding**, so the operator knows which dial matters.
- It says **how confident the limit is**. A cap from a measured spindle power
  curve and a cap from `0.2 × D` should not look alike.

### Confidence is the missing axis

The programme has spent a session establishing that some numbers are solid
and others are folklore. The UI does not distinguish them, so the operator
cannot either. Suggested wording, which maps to what we actually have:

| Limit | Backing | How it should read |
|---|---|---|
| Chip too thin | vendor rows + a fitted force model | *measured* |
| Spindle power | machine power curve + `Ks` corroborated twice | *measured* |
| Tool deflection | beam model on real tool geometry | *calculated* |
| Machine rigidity | `0.2 × D`, no source | **rule of thumb** |
| Gantry push | not modelled | *not checked* |

An operator who sees "rule of thumb" next to the only limit that ever fires
will ask the right question. One who sees a bare number will not.

### What NOT to add

- No chatter surface.
- No new simulation metric. Everything above is pre-simulation: it is about
  the recipe, not the run.
- No fifth limit until one of the existing five stops being a guess.

---

## 5. The parameters

### Already in the model and dependable

`Kc` per species (FPL Table 5-3a), the affine force model (`Ks`, `F_edge`,
corroborated against two wood datasets), tool diameter, flute count, stickout,
helix, Young's modulus, `ap`, `ae`, `fz`, rpm, the spindle power model per
machine, the machine feed cap, the safety factor, and the vendor chipload
bands.

**That set is enough to make cutting power dependable, and it already is.**

### Already in the model and NOT dependable

- `RigidityProfile` — four factors, no source, sets 100 % of depths.
- `WorkholdingRigidity` — Low/Medium/High mapped to 0.85 / 1.0 / 1.03 on the
  feed. Three words standing in for how the part is held.

### Missing, and needed

Two numbers, both per machine, neither published by anyone:

1. **Axis thrust (N)** — the push at which the drive loses position. Replaces
   "will it skip" guesswork.
2. **Gantry stiffness (N/mm)** — how far the tool moves off line per newton of
   cutting force. Replaces `0.2 × D`.

### The recommendation

**Both come from one bench test, and it is not hard.**

- Clamp a luggage scale between the gantry and a fixed point. Pull until the
  axis loses steps. That is the thrust.
- Put a dial indicator against the spindle nose. Pull with a known force —
  the same scale. Deflection per newton is the stiffness.

Half an hour on the real machine turns the two weakest numbers in the model
into measured ones, on the machine the recommendations are actually for. No
literature search will do better: nothing is published, and the numbers are
machine-specific anyway.

Until then, `0.2 × D` should stay — but it should say what it is.

---

## 6. What to do, in order

1. **Nothing on the helix.** Closed by measurement.
2. **Pin the recommendation's power against final geometry**, the way the feed
   was pinned. Small, and it stops a 3.5× over-read reaching the operator.
3. **Label the limits by confidence in the UI.** No new physics, and it is the
   highest-value change here: it tells the operator which numbers to trust.
4. **Measure thrust and stiffness on the real machine.** Half an hour.
5. **Then** replace `0.2 × D` with the stiffness, and add the gantry push
   limit. Both become honest at that point, and not before.

Steps 2 and 3 are worth doing whatever happens to 4 and 5.
