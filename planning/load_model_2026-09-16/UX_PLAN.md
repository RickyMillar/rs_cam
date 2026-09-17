# Feeds and speeds as a product — the UX wrap

Companion to `ADVICE.md`, which holds the physics and the measurements.
This one answers: *given that model, what does the operator see and do?*

---

## The insight that should shape the whole feature

The physics collapses to something a product can actually say.

**Below the vendor band there is no trade-off. There is only loss.**

At the fixture's 0.038 mm/tooth against the vendor midpoint of 0.0675:

| | thin (0.038) | vendor mid (0.0675) | |
|---|---|---|---|
| Energy per mm³ | 151 J/mm³ | 96 J/mm³ | **1.6× the wear** |
| Removal rate | 11 395 mm³/min | 20 242 mm³/min | **1.8× the time** |

Running thin is **slower and harder on the tool at the same time**. It is
strictly dominated. There is nothing to weigh up and no judgement to make —
it is simply wrong, and the product should say so in those terms.

**The real trade-off lives at the top of the corridor**, between chipload
and peak tooth force: going heavier is faster *and* cheaper per mm³, until
the tooth force deflects or chips the cutter. That is the only place the
operator has a genuine decision, and it is the only place the UI should ask
them to make one.

Everything below follows from that sentence.

---

## The de-risking move

`ADVICE.md` R1 — rebuilding `power.rs` on the affine coefficients — is a
**feed-moving recalibration**. It changes recommended numbers, moves the
literature matrix, and needs an explicit authorisation and a re-baseline.
That is a slow, careful piece of work.

**The UX does not have to wait for it.** `force::affine_coefficients` is
already public and already consumed by three call sites. Specific energy,
the edge share and the corridor bounds can all be computed from it *today*,
without touching the power gate.

So:

- The **display** of efficiency reads the affine model directly. Ships now.
- The **power gate's trigger point** is the recalibration. Ships when
  authorised, and changes no pixel of what is designed below.

Ship the value; defer the risk.

---

## Phase A — one row that says where you are and what it costs

**The deliverable.** The Feeds tab's power line is replaced by a chipload
verdict. One row, always present, three states.

```
Chip   0.038 mm/tooth   ⚠ thin — 1.6× tool wear, 1.8× the time   ⓘ
Chip   0.067 mm/tooth   ✓ in vendor range · 38 % force headroom  ⓘ
Chip   0.110 mm/tooth   ⚠ heavy — 8 % force headroom             ⓘ
```

The hover carries the workings: `u` in J/mm³, the ploughing share, the
vendor range, the force ceiling and how it was derived.

**Why this row and not a gauge.** The power bar it replaces reads 1 % on
essentially every job (measured peak across the shipped matrix: 23.6 %). It
cannot distinguish a good cut from a bad one. This row changes state on
every job that matters.

**Why ratios and not J/mm³ on the face.** "1.6× tool wear" is a number a
hobby user can act on. "151 J/mm³" is one they cannot. The unit belongs on
the hover, where the person who wants it will look.

**Why "ploughing" in the hover.** At 0.038 mm/tooth, **83 % of the cutting
power is rubbing rather than cutting**. That is the most visceral sentence
this product can say, and it is the physical content of "burn risk" — which
today is asserted with no number behind it.

---

## Phase B — the chart shows the corridor

The nomogram gains two shaded regions: **below the rubbing floor** and
**above the force ceiling**. Both are iso-chipload rays, so they are two
wedges, drawn exactly like the machine walls already are.

The reading becomes immediate: a clear corridor, the vendor band sitting
inside it, your point somewhere in or out.

**No new legend rows.** The shading reads the way the red machine wall
already reads — "do not go here" — and the `Sources ⓘ` row names them for
anyone who asks.

**This is not a reversal of the 2026-09-16 deletion.** What was deleted were
the band's own edges and midpoint, drawn a second time on top of the wedge
that already showed them. These are *different* constraints bounding a
*different* region.

**Explicitly not built: a power contour.** It would be drawn two orders of
magnitude off the top of the chart and would never bite (`ADVICE.md` §4).

---

## Phase C — an action that fixes it without changing the cut

Having told the operator they are 1.6× on wear, the product has to offer
them something to do about it. Today the only write is `⚡ Apply all`, which
also overwrites DOC and WOC — so the fix for "my feed is wrong" is "let me
change your cut geometry too".

**Add: `Match vendor chipload`.** Holds DOC and WOC, moves feed (or RPM
where the feed cap binds) to land inside the band.

Note this is a **restoration, not an invention**. `⚡⚡ Apply recommended
speeds` existed and was deleted by `d323cabb` along with the SPEED section.
The product deleted the safe narrow action and kept the broad one.

Routing through `feeds::suggest::apply` with an explicit `ApplyScope` keeps
the Checkpoint I contract intact — one funnel, attribution on the button
face.

---

## Phase D — the aggressiveness dial (only if Phases A–C land well)

One slider, `Conservative ←→ Aggressive`, that moves the operating point
within the corridor and shows the two consequences live:

```
Conservative ●───────────── Aggressive
             time 6:altered  wear 1.0×      (relative to vendor mid)
```

This is the operator's stated goal — *"help them optimise the trade-off of
speed to wear"* — made into a control. It is last on purpose: it is only
honest once A has established the numbers and B has established the bounds,
and it is the piece most likely to be unnecessary once A and B exist.

---

## What is deliberately not built

| Not building | Why |
|---|---|
| Power bar / gauge | Never binds. Peak 23.6 % across the whole shipped matrix, typical 1 %. |
| Surface-speed readout | 5.3 m/s at 17 000 RPM, an order of magnitude below the wood carbide optimum, and unreachable even at the 24 000 ceiling. It cannot become the constraint on this class of machine. |
| Tool life in hours/metres | No wear model, no bench data. A fabricated Taylor constant would be worse than the honest silence the engine keeps today (`ADVICE.md` R6). |
| A power contour on the chart | Would sit ~100× off the top of the axis. |

Each of these is a thing a feeds-and-speeds product is *expected* to show.
Not showing them is the decision, and it is defensible on measurement rather
than on taste.

---

## Sequencing and dependencies

```
force::affine_coefficients  (exists, public, already used by 3 call sites)
        │
        ├─► Phase A — the chipload verdict row        ← ships first, no recalibration
        ├─► Phase B — the corridor on the chart
        └─► Phase C — Match vendor chipload

power.rs two-term rebuild (R1)  ── independent ──►  the power GATE's trigger
        (needs authorisation; changes recommended numbers; moves the lit matrix)
```

Phase A is the one to build first: smallest, highest value, and it makes the
surface honest about something it currently gets 4.3× wrong.

---

## How this would be judged

Not by screenshots. Three questions, answerable by someone who was not in
this conversation:

1. Can an operator with basic chipload knowledge look at the Feeds tab and
   say whether their cut is sensible, and why, without opening anything?
2. When it is not sensible, is there one action that fixes it without
   changing anything they did not ask to change?
3. Does every number on the surface come from a model that the rest of the
   engine agrees with?

Today the answers are no, no, and no. Phase A answers the first, Phase C the
second, and R1 the third.
