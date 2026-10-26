# R4 — independent review of the geodesic roughing path forward (2026-10-26)

Reviewer: a fresh Fable session, read-only. No code, no planning file and
no project file was changed. No cargo ran. Python ran on the existing
`phase3/fix9/entries.json` only. Every number below has its source named.

Sources read: `PLAN.md`, `STATUS.md`, `SYNTHESIS.md`, `PROMPT_NEXT.md`,
`DEFECTS.md`, `R1`–`R3`, `P2_C_geodesic.md`, `P2_C2_geodesic_fixes.md`
(C5, C6, C24, C25, C27, C28), `P3_C12`, `P3_C22` (§3, §11), `P15_D_verify.md`
(D6), `phase3/fix9/README.md`, `fixture3d_v4.html`, the fix10 images, and
the code in the worktree at `b37cad1a4`: `adaptive/research_geodesic.rs`,
`adaptive/path.rs`, `adaptive3d/clearing.rs`, `adaptive3d/path.rs`,
`adaptive3d/mod.rs`, `adaptive3d/CLAUDE.md`.

## 0. The answer in short

1. The base is sound. Z levels, regions per level, a geodesic field from
   the seed with islands as obstacles, level sets as wraps, and the
   curvature spacing law are the right structure for this machine and
   these parts. Keep them.
2. The join design is not sound. The arm in use (`zigzag_pick`) joins
   closed wraps with `JoinRule::Seam`. Its own doc says the seam step
   "reads about 0.94 of D ... so the replay re-enters there"
   (`research_geodesic.rs:1898-1904`). So the plan asks for a re-entry at
   every closed ring by design. In 2D a re-entry was a cheap count. In 3D
   it is a vertical feed into stock with a flat end mill. The 1195 and
   3578 descents are that design choice, not a 3D bug. The per-arc plunge
   was known on 2026-10-24 (`PROMPT_NEXT.md`, problem 2); it was not missed,
   it was mis-counted on the 3D page.
3. The proposed next step, a fed step at depth at each ring change, is a
   good interim fix. It removes most of the time cost and the plunge
   abuse. It is still a patch. It leaves one full-width bite and one
   velocity stop per ring. The cause-level fix is: no seam while the
   wraps are closed (one spiral), and no re-entry inside a connected
   region at all. The team has the spiral code (`core_spiral`) and runs
   it only under `JoinRule::Morph`, a rule that was dropped for another
   reason.
4. Several "refuted" join ideas were refuted by a measurement of an
   implementation that no compiler or run had verified at the time, and
   two of them (transition C6, lead-in C8) show signs of a bug rather
   than a limit. Reopen them as bug hunts before more design.
5. The measurements that matter most are missing: a kinematic audit that
   does not trust intent labels, a load-rate (MRR over time) view, and an
   acceleration or reversal count. The operator's one positive report
   ("smoother in acceleration") is measured nowhere.

## 1. Architecture

### 1.1 What is sound

- **Z levels with regions.** Every 3-axis rougher slices. The shipped
  AgentSearch and ContourSpiral arms share this path, so the research arm
  inherits the entry planner, the stamp and the safety splits. Good.
- **A geodesic field from the entry, islands as obstacles.** One Eikonal
  solve per piece, reused from the iso-scallop module. It gives V3 ("grows
  into spaces") by construction. The operator accepted the look at G1.
- **The curvature spacing law** (`P2_C2` C5): `s'(ρ) = ρ + R − √(R² + ρ² +
  2ρ(R − cD))`, closed form, only R, s and the ceiling enter. This is a
  real derivation and it is what makes a spiral hold the load near the
  seed. Keep it.
- **C31, the grown outline.** It removed the one fixed-margin slot (the
  perimeter sweep) and put the whole band under the 2D rule. Thick
  over-load fell from 4.74 % to 0.85 % on Corne (`fixture3d_v4.html`).
  That is a cause-level fix. Good.

### 1.2 What is not sound: the join contract

The 2D kernel emits wraps and lets a later stage (the replay) decide each
join. A join the replay refuses becomes a 2D `Rapid` (lift and re-enter).
The design treats that outcome as legal. Three facts show it is the root
of the defect list:

1. `JoinRule::Seam` doc, `research_geodesic.rs:1898-1904`: the radial seam
   step reads 0.94 D and re-enters. `C20_LOCAL_ZIGZAG`, `C23_ZIGZAG_V2` and
   `C28_ZIGZAG_PICK` all use `Seam` (`:2258-2290`).
2. `core_spiral` (`:3391`) is called once, under `JoinRule::Morph` only
   (`:4926`). The arm in use never runs a spiral.
3. In 3D a 2D `Rapid` goes through `plan_entry` (`clearing.rs:779`). With
   a stay-down proof it becomes `RapidWithFloor`: a rise to the stock top
   plus `RAPID_DESCENT_BUFFER_MM` (0.5), a traverse, then the project's
   entry style. Corne's entry style is a plunge at 750 mm/min
   (`phase3/fix9/README.md` §4, the G-code quote). The semantic span of
   that sequence is "keep-down link", which is how the page counted it.

So the chain is: Seam → replay refusal → 2D Rapid → 3D lift-over → plunge
at the plunge feed. Each stage did what its contract says. The contract
is wrong: a planner must not emit a step it knows will be refused and
then rely on a re-entry. FreeCAD (R1 §2, §4) and Freesteel (R2 Q3) never
re-enter inside a connected region. They route a keep-down link through
cleared stock and lead in with a growing bite. Held's spiral has no seam
at all (R3 Source 1).

On the Corne Z 17 plane (`fixture3d_v4.html`, D20): 238 descents. From
`entries.json`, 40 of them lie on the seam line y = −49.76 from x 59.6 to
103.0 (47 mm from the seed; the nearest wall is 33 mm away, so check which
wraps past 33 mm still take a seam). The other 198 sit on the region walls
where open arcs end (the plane image). Those are the zigzag turns the
replay refused: the C25 mechanism (an arc that ends head-on into a wall
meets the lens the previous arc left, `arc_end_lens`), which the C28/C29
pick tries to dodge by planning each piece twice. The pick is a
symptom-level optimiser. With a legal join design it is not needed.

### 1.3 The 3D stage re-plans what the 2D stage decided

`clearing.rs` is 3996 lines and `research_geodesic.rs` is 9938 lines with
nine option enums (`MeetingRule`, `JoinRule`, `LobeMorph`, `Spacing`,
`Chaining`, `Reseed`, `ArcLink`, `ArcEnds`, `ArcPick`). The fix history
C12 → C22 (`HeldLinks`) → C25 → C26 → C26b → C28 → C29 → C31 → C33 is a
sequence of research-only branches inside the shipped 3D stage, each one
undoing a decision the stage below made: the air classifier demoted held
links to entries (C22); the RDP after the load rule broke the load (C26);
held links crossed terrain (C26b); the turn feed was lost in the merge
(D14); the 2D pick did not predict 3D plunges (C29). Each is real. The
pattern says the layer boundary is in the wrong place. The 3D stage should
drape, demote true air runs, apply the Z safety split and stamp. It should
not decide entries for a step the 2D plan marked as a transition.

### 1.4 Alternatives the team did not take, and my view of each

| Alternative | View |
|---|---|
| One continuous spiral per connected zone (field `T/s − φ/2π` while the wraps are closed; `P2_C` "Unfinished 2"; `core_spiral`) | **Take it.** Zero seams, zero re-entries, zero reversals in the closed phase, and the spacing law already makes it hold `c·D` exactly at every ρ (C5 table). Cost: a small over-cut on the closing lap. The zigzag rule gains nothing on a closed ring: a ring closes on itself, so there is no return move to save. Check whether `ArcLink::Zigzag` reverses direction between closed rings; a reversal is a full stop, the opposite of the smooth motion the operator reported. |
| Transition join (C6: drift outward over the last fraction of a lap) | The geometry says a tangential drift meets at most `λ·s ≤ s` of stock against the ring just cut. The run read 0.9375 D on 8 of 46 transition steps (D6 table, `P15_D_verify.md:1036`) and the C6 unit test was red. That is a bug signature, not a refutation. Find the step that read 0.94 before this idea stays parked. |
| Lead-in with growing bite (C8, FreeCAD's `MakeLeadPath`) | `PROMPT_NEXT.md` records "the code never applied: 0 lead-ins, every band fell back; FIND THAT GUARD first". It was not found. This is the standard join for open arcs after a keep-down return. Reopen. |
| Medial-axis spiral (Held–de Lorenzo 2018) | Not now. No medial axis of segments exists in the repo (R3 L11); terrain slices give a noisy axis; the 2014 island method needs tuned constants (μ, ν). |
| Bieterman PDE morph | Fails V3 by construction on branched slices (R3). Correctly dropped. |
| Trochoidal ring change | Out of scope by `PLAN.md`; agreed. The spiral removes the need. |
| Routed keep-down links (FreeCAD bisection, Freesteel A*) | Needed for lobe-to-lobe and far-side moves, not for ring changes. `SYNTHESIS.md` §3.2 already says V1 is a link problem. Not built yet. Second priority after the spiral. |

## 2. The proposed next step: a fed step at depth at each ring change

### 2.1 What it costs in load (derived; Corne numbers)

- Corne first rough: s 1.2, R 3, D 6. Ceiling `c = (1 − cos(1.05·acos(1 −
  s/R)))/2 = 0.2189`, `c·D = 1.313 mm` (`P2_C2` C24; the 1.05 is
  `adaptive/mod.rs:175`).
- A radial step of `s` from a closed ring meets the lune between two discs
  offset by `s`: area `≈ 2R·s = 7.2 mm²`, swept width `≈ D` (the readout
  gives 0.94 D). Area per mm of travel `≈ 2R = 6 mm²/mm` against the
  ceiling `1.313 mm²/mm`: **4.6 times the ceiling for 1.2 mm**.
- `turn_feed_factor` (`research_geodesic.rs:7316`) scales the feed by
  `c / w = 0.2189 / 0.94 = 0.23`, so the step runs at about 930 mm/min at
  F 4000. The chip load per tooth then holds (the function's own
  derivation). The step is a side cut, so the end teeth do not cut. That
  is strictly better than the plunge it replaces, which is an end-tooth
  cut at 750 mm/min with no chip path.

### 2.2 What it saves in time (from `entries.json`, before acceleration)

| Column | Descents | Plunge length | Plunge time at 750 mm/min | Rise length (≈ 6.5 mm each at F 4000) |
|---|---|---|---|---|
| Corne D20 | 1195 | 5974.6 mm | 478 s | ≈ 120 s |
| wanaka200 D20 | 3578 | 16045.1 mm | 1284 s | ≈ 350 s |

A fed step of 1.2 mm at 930 mm/min is 0.08 s. So the step removes about
600 s from Corne D20 (2760 → about 2150 s, near contour_parallel's 2022
s) and about 1600 s from wanaka200 D20 (4832 → about 3200 s). Run C21's
kinematic model for the exact figure; these are feed-only sums.

### 2.3 Verdict on the step

Do it, because it is cheap (the turn exemption, the bound `turn_bound = s +
(1 + √2)·cell` and the feed factor exist, `adaptive/path.rs:60`) and
because it removes a tool-abuse mode now. Do not call it the fix. It keeps
one 4.6× load spike, one feed dip and one velocity stop per ring (Corne
Z 17 alone has about 40 rings on the seam line). The spiral removes all
three with no exemption and no counted turn. Build the step first, the
spiral second, and retire the step on closed wraps when the spiral lands.

One caution: the turn bound admits a step of up to `s + (1 + √2)·cell =
2.41 mm` at Corne. A ring change is exactly `s'(ρ) ≤ s`, so the bound is
not the risk. The risk is the pick: with seams admitted as turns, plan A
and plan B of C28 will both lose their re-entries and the pick's tie-break
(less keep-down length) decides. Pre-register what you expect it to pick.

## 3. Measurement

### 3.1 What the instruments measure well

- The exact swept-width bound (C18, 0.471 at s 2; the page says it
  under-counts at s 1.2) and the thick measure (C32) separate real
  over-load from sim-cell noise. Good.
- The stock rebuild from the sim trace (fix9) answered the "sparse
  annulus" question with evidence. Good.
- Hard bars as pass/fail. Good in principle (see 3.3).

### 3.2 What is missing

1. **A kinematic audit that trusts no label.** The plunge problem hid
   behind the span style "keep-down link". Rule: every count on a page
   must come from a predicate on the motion (Δz, feed, removed volume),
   never from an intent tag alone. `entries.py` is that predicate. Make
   it a permanent instrument in the triage, for every strategy. Its
   thresholds (descent ≥ 1.0 mm, removed > 0.1 mm³, plane within 0.35 mm,
   `render.py:142`, `entries.py:40-44`) have no derivation; derive them
   from the sim cell and the Z noise, or name them as screening values.
2. **Load rate, not only load width.** The spindle feels material removal
   rate. The sim trace holds removed volume per sample and the move feed,
   so an MRR-over-time histogram per strategy costs one script. It would
   have shown 1195 spikes at once.
3. **Acceleration and reversals.** The operator's reported gain is
   smoothness. Nothing measures it: count velocity stops (direction
   reversals, Z reversals), the junction-deviation events of the cycle
   model (C21's port of `kinematics.rs`), and a minimum-corner-radius
   histogram. Put them on the page beside cycle time.
4. **Cycle time split.** Report cutting at feed, keep-down travel, Z
   travel and plunge feed as four numbers. One total hides a 478 s plunge
   share.
5. **The planner lattice at s 1.2.** The planner cell is `max(R/6, tol) =
   0.5 mm`. The margin between `s` (1.2) and `c·D` (1.313) is 0.113 mm,
   under a quarter of a cell. The replay cannot tell a legal step from an
   illegal one at this stepover; D6's quantum test (refusals rise at the
   half cell) says the same (G-CELLREAD). Every "over the bound" figure
   under about 1 % is inside this noise. Derive the cell from `s` (for
   example so that one cell is under the margin), or state on the page
   that the readout at s 1.2 is a screen, not a measure.
6. **The coverage bar cannot pass.** All four columns read FLAGGED on
   Corne because the triage domain is the reachable floor, which includes
   the outer band (G-OUTERBAND). A bar that fails on every column decides
   nothing. Split it: inside the part outline (a bar) and the outer band
   (information). The 0.8 mm height threshold of the rebuild
   (`render.py:98`, `stockmap.py:89`) has no derivation.
7. **The control has safety failures that are not filed.** contour_parallel
   on wanaka200 reports "86 rapid-through-stock collisions"
   (`phase3/fix6/wanaka200/contour_parallel/stderr.txt:122`) and two gouge
   lines on Corne. Neither is a `DEFECTS.md` row (G-LINKPLOUGH covers fed
   links, not rapids). The operator runs contour_parallel daily. This
   outranks research.

### 3.3 On the look gate

`PLAN.md` says the gate is the look, with three questions. The pages now
carry eleven numeric rows per column. The operator said on 2026-10-25 that
"the 3D page is tricky for me to interpret". Put the three questions back
at the top of each column with one image each, and move the tables below.

## 4. Constants without a derivation at their definition site

Shipped (not the research arm's fault, but the arm inherits them):
`AIR_THRESHOLD_MM = 0.2` (`clearing.rs:3162`), `PLUNGE_SLOPE_LIMIT = 0.3`
and `Z_DROP_FLOOR_MM = 1.0` (`:3272-3273`, the comment gives a reason, not
a derivation), `LINK_DESCENT_NOISE_MM = 0.5` (`:880`), `SIMPLIFY_INSET` and
`SIMPLIFY_TOLERANCE = 0.3` (`:2247-2250`), `PERIMETER_INSET_MARGIN_MM =
0.25` (`:2549`), `RAPID_DESCENT_BUFFER_MM = 0.5` (`path.rs:1404`),
`PECK_CLEARANCE_MM = 0.5`, `ASSUMED_RAPID_MM_MIN = 5000` (`:1868`, a named
guess), the 20 % material-crossing rule of `is_clear_path`
(`adaptive/path.rs:103-104`), `OFFSET_OVERLAP = 0.85`, `COVERAGE_THRESHOLD
= 0.25`. The 1.05 factor inside the ceiling `c` (`adaptive/mod.rs:175`) is
the one that sets every bound in this programme; I did not find its
derivation on the way and the lead should confirm it has one.

Research-side scripts: the thresholds in §3.2 items 1 and 6.

The research kernel itself is clean on this rule: every constant I read
in `research_geodesic.rs` derives from R, s, the cell, the tolerance or
the ceiling, and the module says so at each site.

## 5. What I would do next, ranked

1. **Fed turn at every ring change (the proposed step).** Reason: cheap,
   removes the plunge abuse and about 600 s on Corne now; the mechanism
   exists. Pre-register the plunge count (expect under 100 on Corne) and
   the pick outcome.
2. **One spiral while the wraps are closed, under the curvature law.**
   Reason: removes the seam class entirely (no exemption, no reversal, no
   feed dip). `core_spiral` exists; route the zigzag presets through it
   for the closed levels and start the open-arc logic at the first open
   level. Then retire item 1 on closed wraps.
3. **Make "no re-entry inside a connected region" the kernel's contract.**
   Reason: it is the cause behind C3, C25, C27, C28, C29 and the 1195.
   For an open arc: zigzag turn when the lens fits; else keep-down return
   along the band just cut plus a lead-in with a growing bite (reopen C8;
   find the guard). A 2D `Rapid` is then allowed only between zones the
   field proves disconnected at that level. Add a sentry that counts 2D
   `Rapid`s per connected zone and fails on more than one.
4. **Re-read C6 as a bug.** Reason: a tangential drift cannot read 0.94 D
   by geometry. If the transition is sound it is a second way to item 2
   that also serves open-to-open joins.
5. **Kinematic audit, MRR histogram, reversal count, cycle split
   (§3.2 items 1-4) on the page, and the label rule.** Reason: the next
   defect of this kind should not need a week.
6. **Derive the planner cell from s, or label the s 1.2 readout a
   screen.** Reason: §3.2 item 5; the Corne numbers under 1 % are not
   evidence either way.
7. **File and fix the control's 86 rapid collisions and 2 gouge lines.**
   Reason: safety on the operator's daily strategy. It is shipped code and
   outside this arm, so it needs its own owner, but it must not stay
   unfiled.
8. **Routed keep-down links with a retract penalty (R1 §4).** Reason: the
   remaining lobe-to-lobe and far-side moves; V1 is a link problem.
9. **Freeze the option space.** Reason: nine enums and about twelve
   presets cannot all be clean. Once items 1-3 land, name ONE preset as
   the candidate, delete the columns that lost (the repo rule is delete,
   not archive), and move the kernel's 3D-facing part out of
   `research_geodesic.rs` into its own file with a 40-line `CLAUDE.md`.
   The clean implementation starts from that file, not from the 9938-line
   research module.

## 6. Where I disagree with the lead's reading

- "Vertical descents into stock are the main defect of the research
  arm" (`fixture3d_v4.html`): agreed on the fact, not on the frame. The
  defect is the join contract (§1.2). A fix at the ring change alone
  leaves the same contract to fail at the next place (the 198 wall-end
  descents on Corne Z 17 are the same contract at open arcs).
- "Iterate" at G2: agreed, with the order above. Items 1 and 2 together
  are one agent-day each by the look of the code; item 3 is the real work.
- The C28/C29 pick: I would not invest more in it. It chooses between two
  plans that both re-enter where they should not.
- The speed ruling ("don't worry about the speed") is right as a goal, but
  time is still the cheapest proxy for the plunge count: 478 s of plunge
  feed on Corne was visible in the cycle time before anyone counted it.
  Keep the split (§3.2 item 4) on the page.
