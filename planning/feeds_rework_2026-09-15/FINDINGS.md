# Feeds and Speeds — findings, 2026-09-15

The operator reported four things after the declutter landed (`9b778d3c`).
This document records what I measured for each. It is evidence, not a plan;
the plan is `PLAN.md`.

Captures in this directory:

| File | What it shows |
|---|---|
| `evidence_modal_top.png` | the Explore window at a 1100 x 620 viewport |
| `evidence_modal_bottom.png` | the same window scrolled to charts A and B |
| `evidence_chart_a.png` | chart A, cropped and enlarged |
| `evidence_chart_b.png` | chart B, cropped and enlarged |

---

## F-1. The GUI has no direct feed, plunge or RPM control

**Severity: regression. This is the most serious finding here.**

The operator asked "there is no direct f/s control?". Correct. There is none.

`ui/properties/operations/mod.rs:37` records the history in a comment:

```
// Feed / plunge editing moved off the per-op Geometry panel into the
// Feeds & Speeds tab's SPEED section (W3.2) — the `draw_feed_params` helper
// (and its `FeedsResult` / `dv_pill` deps) was retired with the recharter.
//
// The per-operation spindle RPM override moved to the Feeds-card SPEED section
// (W3.1) as a `PrecedenceField` ...
```

So both controls were deliberately concentrated in one place: the Feeds tab's
`SPEED — how fast` section.

Commit `d323cabb` ("UR4 — the feeds modal is Explore only", today 14:13) then
deleted that section. Its diff removes:

```
-                ui.named_section("SPEED \u{2014} how fast (manual)", |ui| {
-                            let mut feed = entry.operation.feed_rate();
-                                entry.operation.set_feed_rate(feed);
-                            let mut plunge = entry.operation.plunge_rate();
-                                entry.operation.set_plunge_rate(plunge);
-    ui.named_section("SPEED \u{2014} how fast", |ui| {
-use crate::ui::components::{PrecedenceField, ...};
```

`PrecedenceField` now has **no call site anywhere in the crate**.

The Feeds & Speeds tab body (`ui/properties/mod.rs:4970`) is today: an
`Explore…` button, then `calculate_and_apply_feeds`, which draws the
read-only comparison card. The only write is `⚡ Apply all — changes the
cut`, which overwrites RPM, feed, plunge, DOC and WOC together.

**An operator cannot set a feed rate.** They can only accept the whole
recommendation or keep the whole default.

This was not a declutter decision that went too far. UR4's stated scope was
the *modal*; the SPEED section was inspector furniture that the same commit
removed on the way past, and nothing replaced it.

---

## F-2. The power gauge is arithmetically right and informationally dead

The operator asked whether ~1 % power is right. It is. The presentation is
the problem.

Measured on the Pocket demo fixture (6 mm 2-flute flat, generic softwood,
default machine):

```
power_kw        = 0.00666
available_kw    = 0.6000
mrr             = 11386 mm³/min
```

The chain, all of it defensible:

- `Material::SolidWood { GenericSoftwood }` → FPL shear 6.5 N/mm²,
  `MILLING_KC_FACTOR = 2.7` → Kc 17.55.
- `power::predicted_power_kw` applies `GRAIN_ANISOTROPY_FACTOR = 2.0` →
  Kc_eff 35.1 N/mm².
- `35.1 × (4.20 × 2.10) × 1291 / 60e6 = 0.0067 kW`.

Seven watts of cutting power. That is what a 6 mm cutter in softwood costs.
Wood is not metal.

The denominator is `machine.power_at_rpm(rpm) × safety_factor`, and the
default profile is `ConstantPower { power_kw: 0.8 }` (`machine.rs:160`), so
`0.8 × 0.75 = 0.60`. Both numbers are honest.

**The defect is that it is drawn as a gauge.** `feeds/mod.rs:1714` already
records the measurement that condemns it:

> across all three shipped presets × ten species × Ø3/Ø6/Ø12 slots the power
> branch never fires at all — rigidity and the machine cutting ceiling bind
> first, peak utilisation 23.6 %

A bar whose *maximum observed* value across the entire shipped matrix is
23.6 %, and whose typical value is 1 %, spends its whole life in the left
quarter. It cannot distinguish a safe cut from a safer one, and an operator
who learns to read it learns nothing. It also occupies a full row of a
240-point rail.

Two things are true and should not be confused: the *number* is worth
keeping (it is the one honest answer to "will my spindle stall"), and the
*bar* is not.

Secondary: the 0.8 kW default almost certainly is not the operator's
spindle. A 2.2 kW router makes the reading 0.3 %. Whatever replaces the bar
must make the denominator's provenance visible, because a headroom figure
quoted against a guessed spindle is worse than no figure.

---

## F-3. The Explore window's legends wrap one word — sometimes one letter —
per line

This is the "cooked format" in the operator's report, and it is the single
most visible defect. In `evidence_modal_bottom.png` the legends read:

```
ven          rec
dor          om
ban          men
d            ded
```

**Cause, identified.** `explore.rs:653` builds each legend as an
`egui::Grid` with two columns. Column one is a `ui.horizontal` holding the
swatch and a `Label::new(...).wrap()`.

A `Grid` cell has no known width on its first layout pass. `TextWrapMode::
Extend` (the Grid cell default) asks for infinite width — which is the UR1
defect this codebase already banned. Someone therefore wrote `.wrap()`, which
is the opposite failure: with no width to wrap against, the label collapses
to its narrowest possible break, which for a long token is one character.

So the two obvious choices in a Grid cell are both wrong. The fix is not a
third wrap mode; it is **not to use a Grid**. Each legend entry is one row of
`swatch · label · value`; a vertical list of `horizontal_wrapped` rows lays
that out correctly and needs no column negotiation.

All three legends are affected: chart C's "What the colours mean", chart A's,
and chart B's.

---

## F-4. Chart B renders an empty frame with a negative axis

`evidence_chart_b.png`. The plot area is blank. The y-axis runs −0.1 at the
top to −0.5 at the bottom. The x-axis reads 600–900 Janka.

The series are **not** empty. Measured directly from
`FeedsExplain::rows_by_hardness()` on the same fixture:

```
rows_by_hardness len=19
min_pts=17  max_pts=19
x range  547.7 … 932.7   (Janka)
y range  0.0280 … 0.4171 (mm/tooth)
```

So: 36 real points, every one of them at positive y, and a drawn window that
is entirely below zero. The x-axis DID range itself from the data (547–933 →
ticks 600…900). The y-axis did not.

`Plot::new("feeds_modal_chart_b")` sets no `include_x` / `include_y`. Chart C
sets four of them. `egui_plot` persists bounds per plot id in `Memory`, so a
bad auto-range computed once — for instance while the chart sat clipped
below the fold of a short window, which is exactly the state
`evidence_modal_top.png` captures — is kept for the life of the session.

I have not reproduced the first bad range from a cold start, so I record the
mechanism as *supported* rather than *proven*: bounds are persisted, this
chart pins none, and its siblings that pin bounds do not exhibit it. The fix
is the same either way — pin the data range explicitly.

**This is the repo's recurring failure shape again: absence rendered as
success.** An empty chart frame and a chart of a material range with no data
look identical. The `rows.is_empty()` guard above it prints an honest "No
matching vendor rows at this diameter" — and never fires, because the rows
are there. The failure is downstream of the only honesty check.

---

## F-5. Chart A collides with itself

`evidence_chart_a.png`:

- The heading `Advance/tooth vs Diameter` is painted **over** the rotated
  y-axis label `commanded advance/tooth (mm)`.
- Point labels overlap each other and the markers: `0.229` over `6.00 mm`,
  `0.085` over the current-value marker, `0.050` over the red dot.
- Series are drawn **outside the plot frame** on the right: a diamond and
  its `0.152` label sit past the grid's right edge.
- The vendor band polygon extends beyond the min/max lines that define it.

The underlying reason is that four calibrated diameters are plotted as if
they were a curve, with every point labelled, inside 360 x 180 points.

---

## F-6. The two mini charts answer a question the operator did not ask

Chart A is advance/tooth **vs tool diameter**. Chart B is advance/tooth **vs
material hardness**. The operator is cutting one job, with one tool, in one
material. Both charts describe how the vendor table varies across tools and
materials that are not on the machine.

That is reference data. The inspector already presents the same data
properly, as a table, under `Vendor Cutting Data` — 174 observations with
material, diameter, flutes, RPM, chipload, DOC and evidence grade.

Charts A and B are also the two that are broken (F-4, F-5), and together
they are roughly 60 % of the modal's height.

---

## F-7. The `Why is the recommendation here?` disclosure is the wrong shape

The operator's words: *"'why is recommendation here' is all just too much.
I'd rather if there is a change in recommended, then there is hover over to
see why."*

They are right, and the reason is structural rather than a matter of taste.
The disclosure explains **the recommendation as a whole**. The operator
reads the card **one row at a time** — RPM changed, feed changed, DOC
changed 3.5x — and the question that arises is always about one row: *why
is my DOC being tripled?*

The disclosure cannot answer that. It lists provenance, then rationale, then
a derate chain, then a result, and leaves the reader to work out which of
those bears on the row they were looking at. The declutter earlier today cut
it from 44 painted runs to 13, which made it shorter without making it
answer the question.

The correct home for "why" is the row whose number changed.

---

## What is NOT broken

Worth recording, so the rework does not churn them:

- The comparison card's row format (`current → recommended · Δ`) reads well
  and holds the 240-point rail.
- `⚡ Apply all — changes the cut` and its funnel are correct and are pinned
  by `apply_contract_a3.rs`. The attribution stays on the button face.
- The nomogram (chart C) is the one chart that answers an operator question:
  *if I move feed and RPM, where do I end up relative to the vendor band and
  the machine's limits?* Its annotations collide (they are drawn at fixed
  offsets), but its shape is right.
- The window now fits the screen (`g_feedsfit`). That fix stands.
