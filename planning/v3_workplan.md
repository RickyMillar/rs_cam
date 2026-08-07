# v3 process proof — working plan

> **SUPERSEDED IN PART — H4, 2026-08-04.** Strategy verdicts in this file
> were measured through four instrument defects that are now fixed
> (classification grid 6x too coarse; finish-planner dials 6x/36x too large;
> rest-routing radius = shaft not tip; `claims_reference: self_probe`).
> Those verdicts are **void, not falsified** — the comparison could not have
> come out any other way. Which specific claims, and what replaced them:
> `planning/review_2026-07-29/SUPERSEDED_CONCLUSIONS.md`.


Live tracker. Companion to `v3_campaign_map.md` (what we tried and why it
died) and `unified_v3_design.md` (the full chronological log).

**Rule for this file: nothing goes on the list without a BAIL criterion.**
The campaign has twice built the obvious fix for a confirmed mechanism and
had the gate reject it, so "we understand the mechanism" is not a licence
to keep going. Each item says in advance what would make us stop.

---

## END GOAL (unchanged since 2026-07-13, user's framing)

On a ×2-scaled wanaka terrain, a **cascade** — ball all-over finish
(Op A) + ONE unified rest-clear (Op B) — must beat **D**, a single
all-over pass with the tip tool, on **wall-clock time** at **equal COLUMNS
quality**, with Region spans showing the strategy mix.

**Done means one of two things, and the second is a real outcome, not a
failure:**

- **PROVEN** — both gates green on the same run, at the same dials,
  committed with the numbers; or
- **NOT PROVABLE ON THIS FIXTURE, with a named reason** — a written
  verdict that says which gate fails, why, and what would have to change
  (different terrain, different tool pairing, a different rest strategy).

**Campaign-level bail:** if GATE 2 is still open after items 1 and 3
below, we stop hypothesis-hunting and write the NOT-PROVABLE verdict.
Rationale: the net product change so far is the shipped reorder plus two
default-off dials. Continuing to spend on diagnosis past that point buys
knowledge we already have enough of.

---

## Item 1 — §11a instrument integrity **[DONE 2026-07-28 — CLEARED, with a twist]**

**RESULT: ACCEPT (instrument clears).** Re-stamping Op B's toolpath onto
its own pre-carve snapshot reproduces the sim's final stock at all four
ladder columns to ~0.5 µm. `prior_stocks` means what the ladder assumed;
the sim is faithful to the toolpath. **§11's attribution stands** — Op B
really removed 5.5 mm from a column Op A had finished to 8 µm.

**The twist: the reach contradiction survived and MOVED.** Both the sim
and an independent re-stamp produce a 1.770 top where Op B's lowest move
within 4 mm is 2.899. Since the radial profile height `h(r) ≥ 0` always,
no tool position at tip 2.899 can leave 1.770. So the discrepancy is in
the SHARED stamping path (`stamp_linear_segment` / `RadialProfileLUT`) or
in the site probe's move enumeration — not in the simulation wrapper, and
not in `prior_stocks`.

**Follow-up logged, not chased:** 68 703 of 640 062 columns differ by
>0.01 mm between sim and re-stamp, every one in the same direction (sim
removed more), worst 0.61 mm. The two paths differ in intent handling —
the metrics variant skips `MoveIntent::Retract` feeds, the plain one
stamps them — which predicts the OPPOSITE sign, so it is unexplained. It
cannot affect the gouge sites, which agree exactly.

**Next, and it is item 3's first move:** bisect Op B's move list against
that one column to name the exact move the stamp attributes it to. Stamp
in chunks onto one evolving stock, watch the column, then go move-by-move
inside the chunk that drops it. One pass, so roughly the cost of one
re-stamp. Either the move is unreachable (the stamp is wrong) or the site
probe missed it (the probe is wrong). No third option.

<details><summary>Original statement of item 1</summary>

**Question.** The per-op ladder says Op B removed 5.5 mm from a column its
own toolpath cannot reach (lowest Z within 4 mm = 2.899, final top =
1.770). Either Op B's stamping over-removes, or `prior_stocks` doesn't
mean what the ladder assumes.

**Why first.** Every gouge number in §11 depends on it. If stamping
over-removes, part of the 6 060 gouged columns is an artifact and the
quality gate has been reading it.

**Method.** Re-stamp Op B's emitted moves onto its own "before Op B"
snapshot with the same cutter; compare against the sim's final stock at
the four ladder columns. (Same three-way shape that closed P2.g Task 1.)

**ACCEPT (instrument clears):** re-stamp == sim final at all four columns
to within the dexel quantum. The COLUMNS numbers stand; the geometry
reasoning about reach was wrong; continue to item 3.

**ACCEPT (defect found):** re-stamp differs materially. Then the finding
is a **simulation defect**, which outranks this campaign — it affects
every quality verdict in the repo. Stop the cascade work, write it up,
fix it, re-baseline.

**BAIL:** if the probe itself can't be made to agree with either
interpretation in one working session, stop and record the ambiguity —
do not build a third instrument to arbitrate the first two.

**Cost:** ~1 probe + one chain run.
</details>

---

## Item 2 — ship decision **[DECIDED 2026-07-28: DO NOT SHIP YET]**

**RESULT: leave it OFF, and fix the air classifier first.**

The +316 the case rested on was measured against a baseline where reflex
arcs (§12) contributed 1 218 shallow gouges. Re-measured after that fix,
on the same fixture:

| | shipped | bridges-only |
|---|---|---|
| deep columns, total | **937** | **3 503** |
| shallow | **87** | **700** |
| very-steep | 32 | 525 |
| shallow on-size | 15.5% | 14.8% |
| finish stack | — | 38 577.8 s (−2.0% vs D) |

**+613 shallow on a baseline of 87 — 8×, not 26%.** −2.0% is not worth
that.

**And there is a mechanism, which is why this is a sequencing problem
rather than a verdict.** Vetoing a bridge means emitting the air run as a
CUTTING move, so the policy leans harder on `filter_air_cuts`'
classification — and review finding 4 shows that classifier samples only
the endpoints and the arc centre, with `_tool_radius` unused. A run whose
endpoints are in air but whose middle crosses material is precisely what
it mis-labels, and the policy then cuts through it instead of retracting
over it.

**Order inverted:** fix the classification first (sample the swept path
at dexel scale, honour the cutter radius, linearize arcs — finding 4's
own recommendation), then re-measure. If the +613 collapses, this ships
on time merit. If it does not, the policy buys speed with material and
stays off permanently.

**Lesson, and it is the same one twice now:** a quality cost measured
against a contaminated baseline is not a quality cost. The arcfit noise
made +316 look like a rounding error on 1 218.

<details><summary>Original statement of item 2</summary>

**Question.** Turn it on by default?

**What is already measured.** Cascade finish stack −2.1% vs D on one run,
both branches, collisions 0/0. Op B −27.9% standalone. Cost: +316 shallow
over-cut columns. D barely moves (−1.3%, all rapid), so it is not a
cascade-only win.

**This is a DECISION, not work.** It does not depend on item 1.

**ACCEPT:** ship it on, on time merit, noting the +316 in the commit.
**BAIL:** leave it off if item 1 shows the over-cut instrument is
untrustworthy — the +316 would then be unquantified.

**Open sub-item (§10b), separate change, own A/B:** the veto compares
LENGTH, but the air run is a feed and the bridge legs are rapids, so an
equal-length bridge is much faster. The current test is mis-signed and the
−22.8% is a FLOOR. Blocked behind the `apply_dressups` signature (review
Finding 5), which is what keeps `link_kinematics` from reaching the
filter.
</details>

---

## Item 3 — GATE 2 **[gouge branch CLOSED 2026-07-28; gate still red]**

**RESULT: the gouge was `arcfit` emitting REFLEX arcs.** Direction came
from a cross product of two chords with nothing checking the outcome; on
a shallow run that sign is rounding noise, and the wrong sign gives the
reflex arc through the same endpoints — 356.4° where 3.56° was intended,
354 mm of travel for a 3.55 mm chord. A default-ON dressup, so not
campaign-specific; a machine would have driven a 114 mm circle through
the part. Fixed by requiring a fitted arc to be about as long as the
polyline it replaces (commit a6841e1).

Cascade deep columns **6 060 → 937**, shallow **1 218 → 87**, at no time
cost. TSP reassembly exonerated by `V3_REORDER=off`.

**But the gate did NOT move: shallow on-size 15.2% → 15.5% vs D's 19.2%
(needs 17.2%).** The campaign's repeated mistake in miniature — the
gouges were real and serious and were never what the gate measured.
1 218 columns is 1.8% of the band; the missing 4 pp is distribution-wide.

**Remaining hypothesis, one only:** on shallow textured ground a Ø3 ball
cannot reproduce what a Ø1 tip can, and Op B is not clearing the
difference. Either that is a territory dial (`min_rest_depth_mm`, the
rest-field keep-mask — Op B declining work it should take) or it is
inherent to the tool pairing.

**BAIL is now one step away.** Per the criteria below, this is the last
mechanism. If a territory probe shows Op B correctly has nothing to clear
there, the answer is "inherent to the pairing" and that IS the verdict —
write it up and stop.

<details><summary>Original statement of item 3</summary>

**Question.** Why do both cascade finishing passes gouge when D's does
not (6 060 columns vs 78, worst −5.57 vs −1.26)?

**State.** Seven candidates eliminated by measurement, including chord
infidelity — which is real and confirmed, and whose fix made the gate
WORSE. Flank and chord are one defect, not two.

**Next hypothesis, only after item 1 clears:** Op B is a REST clearer
cutting ground with no rest — at the worst column Op A had already
finished to 8 µm. That points at territory/claims, not at path geometry.
Territory is `min_rest_depth_mm` + the rest-field keep-mask + dilation.

**ACCEPT:** a change that takes cascade deep columns to D's order of
magnitude (<200) without losing the time gate.
**BAIL — three of them, any one triggers:**
- two more candidate mechanisms eliminated with no fix found;
- any fix that reduces its own probe's reading while worsening COLUMNS
  (this has now happened twice — treat a third as proof we are measuring
  the wrong quantity);
- the mechanism turns out to be inherent to a Ø3 ball + tip rest-clear on
  this terrain, in which case that IS the verdict — write it up.
</details>

---

## Item 4 — carried capability gaps (not blocking, small)

- Pencil, RadialFinish and Chamfer permit the barriered reorder, emit no
  barriers, and are denied the unbarriered one — so they never reorder.
- Face/Inlay/VCarve have `allows_link_moves = false` pending a
  depth-aware corridor width (link gouges 9.21 / 8.21 / 5.78 mm).

**ACCEPT:** barriers emitted + a neutrality sentry per op, on the
`swept_cut_segments` oracle.
**BAIL:** if any op's fragments do not begin with an entry plunge, stop —
review Finding 2 says `rebuild_group` will change its first cut, and that
must be fixed first.

---

## Item 5 — parked, from the 2026-07-27 review (none blocking)

Listed so they are not rediscovered, not scheduled here.

| # | finding | why parked |
|---|---|---|
| 1 | arc chord in `bridge_corridor_is_swept` | `allows_link_moves` false for every op in this project |
| 2 | TSP first-cut source loss | identity for ops whose fragments open with an entry plunge — verify before item 4 |
| 3 | 2-opt cost model wrong | inert: groups exceed the 500-segment cutoff |
| 4 | air-cut classification samples endpoints only | causes LEFTOVER, not the gouge under investigation |
| 5 | `apply_dressups` positional API | blocks §10b; also a real GUI stock-top bug worth fixing alone |
| 7 | test-vacuity gaps | no bearing on the A/B |
| 8 | `ray_blend_above` order dependence | can only manufacture leftover; propagates via `prior_stocks` |

---

## Log

- **2026-07-28** — plan written. Item 1 started.
- **2026-07-28** — item 2 DECIDED: do not ship. Re-measured post-arcfix,
  the policy costs 8x the shallow gouging, not 26%. Promotes review
  finding 4 (endpoint-only air classification) to a prerequisite.
- **2026-07-28** — item 3 gouge branch CLOSED: arc-fit reflex bug found
  and fixed (deep columns −85%), gate unmoved. One hypothesis left before
  the NOT-PROVABLE verdict.
- **2026-07-28** — item 1 DONE: instrument cleared, §11 numbers stand.
  Contradiction moved to the shared stamping path. Item 2 unblocked (its
  bail condition no longer applies — the over-cut instrument is
  trustworthy). Item 3 proceeds via the move bisect.

---

# VERDICT (2026-07-28): **NOT PROVABLE ON THIS FIXTURE**

The campaign-level bail written at the top of this file has triggered, as
pre-committed. This is the second of the two defined outcomes, not a
failure.

## The numbers, honestly

At SHIPPED dials, one run, both branches, collisions 0/0:

| | D (all-over Ø1 tip) | cascade (Ø3 ball + Ø1 rest) |
|---|---|---|
| finish stack | **39 904 s** | 49 966 s (**+25%**) |
| shallow on-size | 19.2% | 15.5% |
| deep over-cut columns | 78 | 937 |
| standing material (`>+.5`, shallow) | 2 964 | **1 301** |

The cascade reaches −2.0% only with `AirBridgePolicy`, which item 2
declined because it costs 8× the shallow gouging. So **the cascade never
wins on a configuration we are willing to ship.**

## Why the quality gate cannot adjudicate it

The ±10 µm on-size bin is not a usable acceptance criterion here:

- it is **below machine repeatability** ($11 junction deviation is
  0.020 mm);
- it is **aliased** — the 0.25 mm measurement grid undersamples both
  branches' stepovers (0.21 and 0.363 mm) and aliases them DIFFERENTLY,
  so the comparison is not like-for-like;
- it is **blind to real defects** — it moved 0.3 pp when 5 123 gouged
  columns up to 5.5 mm deep were removed, and it ranked a branch with a
  28 mm UNCUT BLOCK above one without.

## Why the fixture cannot support the claim

The model is a coarse TIN: median triangle edge 0.46 mm, and **1.8% of
triangles carry 40.8% of the surface area**, with 1 749 facets larger than
the Ø3 ball itself. You cannot demonstrate a 10 µm surface difference on
geometry whose own resolution is 460 µm. Op B's crease detector reads the
faceting as creases and scribes over-cut lines along triangle edges.

## Known defects left standing (both branches)

1. **Scallop truncates its ring cascade** and leaves ~837 mm² uncut
   (`max_rings` budgeted from the flat-ground stepover). Raising the cap
   is WORSE — Op B +92% time, deep over-cut 34× — because it unmasks
   `ring_stepover`'s min-across-ring collapse. Now WARNED on every run
   with the uncut area.
2. **`ring_stepover` takes the min across a ring**, so one steep sample
   sets the advance for the whole ring. This is the root cause of (1).
3. **`filter_air_cuts` classifies air from endpoints only** — deletes
   cuts whose middle crosses material. Biases toward leftover, scales with
   fragment count, so it penalises the cascade ~12×.

## What would have to change to revisit this

- a model whose facets are finer than the cutter, or a coarser quality
  bar honestly derived from machine repeatability;
- (2) and (3) fixed, since both distort the comparison in the cascade's
  disfavour;
- an acceptance panel — defects, coverage, texture — instead of one bin.

## What the campaign produced that stands

- **`arcfit` reflex-arc fix** (a6841e1) — a crash-class G-code defect on a
  default-ON dressup: 356° commanded where 3.6° was intended. Found here,
  not specific to this campaign.
- Ring truncation made **visible** rather than silent.
- Seven instruments, all reusable: surface + deviation renders on every
  scored branch, stage attribution, gouge-site move dump, chord-gouge,
  flank-gouge, per-op column ladder, re-stamp, per-move attribution, arc
  direction sanity.
- The reorder capability work (shipped, quality byte-identical).
