# v3 process proof — working plan

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

## Item 2 — ship decision: `AirBridgePolicy::ShorterThanAirPath`

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

---

## Item 3 — GATE 2: the gouge mechanism

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
- **2026-07-28** — item 1 DONE: instrument cleared, §11 numbers stand.
  Contradiction moved to the shared stamping path. Item 2 unblocked (its
  bail condition no longer applies — the over-cut instrument is
  trustworthy). Item 3 proceeds via the move bisect.
