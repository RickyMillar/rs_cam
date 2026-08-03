# v3 process proof — campaign map

> **SUPERSEDED IN PART — H4, 2026-08-04.** Strategy verdicts in this file
> were measured through four instrument defects that are now fixed
> (classification grid 6x too coarse; finish-planner dials 6x/36x too large;
> rest-routing radius = shaft not tip; `claims_reference: self_probe`).
> Those verdicts are **void, not falsified** — the comparison could not have
> come out any other way. Which specific claims, and what replaced them:
> `planning/review_2026-07-29/SUPERSEDED_CONCLUSIONS.md`.


Navigation for `unified_v3_design.md` §7–§11a, which is long and
chronological. This is the decision tree: what we were trying to prove,
what we believed at each point, and what killed each belief.

## The goal (§0.a, set by the user 2026-07-13)

On a ×2-scaled wanaka terrain, prove that a **cascade** —

- **Op A**: ball all-over finish (Ø3 ball nose)
- **Op B**: ONE unified rest-clear (`unified_finish`, Ø1 tapered ball)

— beats **D**, a single all-over pass with the tip tool, on **wall-clock
time** at **equal COLUMNS quality**, with Region spans proving the
strategy mix.

Two gates. Both must pass. They have been chased almost entirely
separately.

---

## GATE 1 — TIME. **PASSED** (2026-07-27)

Started at **+22.9% SLOWER** than D. Now **−2.1% faster**. Four
hypotheses, three of them wrong.

```
Op B spends 44–58% of its time in air. Why?
│
├─ H1 (§7): S3's cross-region router orders regions badly.
│   └─ REFUTED by measurement: 100% of Op B's 540 m of rapid travel is
│      INSIDE regions, 0% between them. A fused router recovers nothing.
│
├─ H2 (§8): `tsp::optimize_rapid_order` never fires for surface ops —
│   it is gated on rapid-order barriers, and only depth sections and
│   adaptive3d emit any. Sized offline at 89.6% of hop distance.
│   └─ SHIPPED (region-node barriers + capability flip).
│      PARTIALLY WRONG: recovered 67.6% of hop DISTANCE but only
│      4.6% of TIME. `recoverable_air` measured surface-point to
│      surface-point and ignored the two ~30 mm safe-Z legs that
│      dominate every hop. The air is COUNT-bound, not distance-bound.
│
├─ H3 (§9): join fragments with intra-region stay-down links, so there
│   are fewer round trips to pay for.
│   └─ BUILT, and worth only −3.9% alone. Its real value was its
│      TELEMETRY: the generator emits 1 634 fragments, the shipped
│      toolpath carries 15 373. That gap pointed at H4.
│
└─ H4 (§10): `dressup::filter_air_cuts` bridges EVERY in-air run with a
    retract → traverse → descend (~35 mm) with NO test on how much air
    the bridge saves. Skipping a 2 mm sliver cost ~17× the distance it
    avoided. Confirmed independently: scallop's own log says
    `rapid_mm=77 249`, the shipped op carries 539 017 — the filter adds
    86% of Op B's air.
    └─ CORRECT. `AirBridgePolicy::ShorterThanAirPath`: Op B −27.9%.
```

**§10a then isolated the two dials and they are NOT additive** — over-cut
columns +316 (bridges alone), +379 (links alone), **+2 850 together**.
Bridges carry the whole time win; the linker is nearly worthless alone
and is half of a destructive pair.

**Settled 2026-07-27, one run, both branches, bridges-only:**
finish stack D 39 380.3 s vs cascade **38 548.2 s (−2.1%)**, project
−2.0%, collisions 0/0. D is near dial-invariant, so the margin is real.

**Status: the time gate passes on the §10 bridge policy ALONE.** The §9
linker is not needed for it and stays off.

---

## GATE 2 — QUALITY. **STILL FAILING**

Shallow on-size 15.2% against D's 19.2%; the bar is 2 pp. Critically,
**it fails at SHIPPED dials too**, so it was never the air work's to fix.

```
Why is the cascade's surface worse than D's?
│
├─ Assumed for months (§7): texture/stepover — the cascade "trails by
│   3–4 pp" in shallow and very-steep. Never chased.
│   └─ WRONG FRAME. It is not texture. The cascade GOUGES.
│      `deep_overcut_locator` (new): cascade 6 060 columns cut >0.5 mm
│      BELOW the model, worst −5.57 mm. D has 78, worst −1.26.
│      4 179 of them land OFF-REGION, where the band gates never look —
│      which is why it hid inside a number the harness already printed.
│
├─ WHO? — `V3_STAGE`, scoring partial chains:
│      Rough alone        37   worst −1.256
│      Rough + D          78   worst −1.256  ← the SAME column
│      Rough + Op A      238   worst −2.243
│      full cascade    6 060   worst −5.565
│   └─ D's finishing pass introduces NO gouge. Both cascade passes do.
│
└─ WHAT MECHANISM? — seven candidates, all eliminated by measurement:
    ├─ the Rough                    → 37 columns, zero in shallow
    ├─ the §9 relink pass           → off at shipped dials
    ├─ crease node's 5 mm links     → worst column byte-identical with
    │                                 `V3_CREASE_HOOKUP=0`
    ├─ crease path Z                → `lift_to_surface` IS a drop-cutter
    ├─ the COLUMNS instrument       → frames verified; sites reproduce
    ├─ `min_z` non-contact fallback → mesh bbox min −4.0653, Op A's
    │                                 deepest move −3.918. Never equal.
    └─ CHORD INFIDELITY             → real, confirmed, and STILL not it:
         `v3_chord_gouge_probe` found Op A's chords passing 1.98 mm below
         the legal surface, and `refine_chord` genuinely sizes its probe
         count from the chord's XY length, so a 0.18 mm step that falls
         3.1 mm is never probed at all. Fixing it cut the chord reading
         36% — AND MADE THE GATE WORSE (shallow deep columns
         1 218 → 1 615). Reverted.
         `v3_flank_gouge_probe` then showed flank and chord are ONE
         defect seen from two sides, not two populations.
```

**Status: mechanism OPEN, and currently BLOCKED on a contradiction.**

---

## THE BLOCKER (§11a) — resolve this before anything else

`v3_column_ladder_probe` reads a column out of successive `prior_stocks`
snapshots, naming the op that removed the material directly:

| site | model_z | before Op A | before Op B | FINAL |
|---|---|---|---|---|
| (198.75, 52.75) | 7.335 | +0.255 | **−0.008** | **−5.565** |
| (35.50, 149.75) | 2.192 | +1.608 | **+0.091** | **−3.027** |
| (99.00, 129.75) | 5.673 | +0.767 | +0.767 | **−2.326** |
| (51.25, 123.75) | 0.003 | +1.217 | −2.243 | −2.243 |

Op B owns three of the four. At the worst, **Op A had already finished
the column to 8 µm and the REST clearer then removed 5.5 mm from it** —
a rest pass cutting where there is demonstrably no rest.

**But Op B's toolpath cannot do that.** At r = 4.0 mm (past the tapered
ball's full 3.0 radius) its lowest Z near that column is **2.899**,
against a final top of **1.770**. The tapered profile at 1.6 mm offset
sits ~9 mm above the tip, so the flank cannot reach either.

So one of these is false:

1. Op B's **stamping** removes more than its cutter geometry allows — in
   which case part of the 6 060 is an artifact the gate has been reading.
2. **`prior_stocks[Op B]`** is not "the stock before Op B carved" in the
   sense the ladder assumes.
3. Something removes material that is not any enabled op's toolpath.

**Next action, specified and cheap:** re-stamp Op B's emitted moves onto
its own "before Op B" snapshot with the same cutter and compare against
the sim's final stock at those four columns. Equal → the sim is faithful
and the geometry reasoning is wrong. Different → Op B's stamp is the
defect. Same three-way shape that closed P2.g Task 1.

---

## What is shipped vs parked

**Shipped and on:** region-node rapid-order barriers, the capability
decoupling, the link gouge-check, `RelinkParams::boundary`.

**Shipped and OFF by default, pending the quality gate:**
`AirBridgePolicy::ShorterThanAirPath` (carries the whole time win),
`intra_region_hookup_mm` (worth little, half of a destructive pair),
`crease_hookup_mm` (isolation lever, default = old behaviour).

**Instruments, all committed and reusable:** `deep_overcut_locator`,
`v3_shallow_deficit_localize` (`V3_STAGE`), `v3_gouge_site_probe`
(`V3_SITE`), `v3_chord_gouge_probe`, `v3_flank_gouge_probe`,
`v3_column_ladder_probe` (`V3_SITES`). Dials: `V3_DIALS`, `V3_CLAIMS`,
`V3_CREASE_HOOKUP`, `V3_ARCFIT`.

**Parked (from the 2026-07-27 code review, none blocking):** arc-chord
hole in `bridge_corridor_is_swept`; TSP first-cut source loss; the 2-opt
cost model (inert here — groups exceed the 500 cutoff); air-cut
classification sampling only endpoints; the `apply_dressups` signature
(which is what blocks a time-based bridge veto); GUI stock-top wiring.

## The method lesson, repeated four times

Every correction came from building an instrument and reading it, and
each one found something its own hypothesis had not predicted. Twice now
the campaign has built the obvious fix for a CONFIRMED mechanism and had
the gate reject it (§9's linker, §11's chord fix). Confirming a mechanism
is not the same as confirming that fixing it helps.

---

# REFRAMED 2026-07-28 — and Step 1 answered

The user rejected the framing above: the time margin is not the concern,
because "we arent comparing to a good thing right now" and there are too
many free levers for any raw-time comparison to mean much. The metric is
**efficiency with bounds**. The question is whether a staggered finish
(one big ball, then smaller focused passes) beats an equivalent scallop —
and specifically whether a rest pass made of SPARSE mixed strategies is
better MERGED AND LINKED than run as separate sequential ops.

That is **Q2**, and it is not what GATE 1/GATE 2 above measured (**Q1**).
See `unified_v3_design.md` §14. Step 1 — characterise the rest before
building anything — is done, via `v3_rest_anatomy` (200 s, no measurement
sim). Three findings:

1. **The instrument was broken.** `optimize_rapid_order` (default ON)
   makes TSP drop 5 of 24 routing-node spans — the 5 carrying 87% of the
   cutting. The shipped Region-span mix table reports pencil at 74% when
   it is 5.8%, and scallop at 0.6% when it is 80.9%. `spans_valid` stays
   `true`. Telemetry defect, not a safety one. **§7's H1 refutation was
   computed on this vector** and needed re-earning; §14 re-earns it.
2. **The merge premise does not hold on this fixture.** With valid spans:
   scallop 80.9%, raster 13.3%, pencil 5.8%, **waterline 0%**. No contour
   work exists here. The prize — grouped tour minus mixed tour over the
   same nodes — is **+0.9 s on 40 767 s (0.002%)**. Per the plan's own
   instruction: the premise fails, say so, do not build Step 2.
3. **The real cost is round-trip COUNT.** Op B finishes 0.476 mm²/s
   against D's 0.938 — the rest pass is half as efficient as just doing
   the whole part with the small tool. 15 311 of its 15 363 retract round
   trips are INSIDE a single routing node. Op B pays 0.79 round trips per
   mm²; D pays 0.028.

**The board now has one lever worth pulling: intra-node round trips.**
Worth ~−30% on the finish stack if closed to Op A's ratio, against 0.002%
for every routing/merging question chased so far.
