# Finishing efficiency programme — track split

> Written 2026-08-28 from the thin-organic investigation (`FINDINGS.md`).
> Operator's framing: *"this sounds like many parallel bits of work... too much
> for one session. Look into how to split regions first, then how we mill them,
> and the links is a separate track."* That framing is adopted below, with three
> tracks added that fell out of the evidence.
>
> **Everything ranked by MEASURED value where a measurement exists**, and marked
> unmeasured where it does not. The single most expensive mistake available here
> is building on an unmeasured premise — this investigation already refuted its
> own top-ranked lever that way (`FINDINGS.md` §0d).

## OUT OF PROGRAMME — split to `planning/rapid_safety_2026-08-28/PLAN.md`

**Operator ruling, 2026-08-28:** *"it does sound very critical, but a separate
path. It should be a separate phase."*

The audit came back after this programme was drafted, and Track B briefly became
a bucket for everything it found. Two groups do not belong in a *finishing
efficiency* programme and now have their own phase:

- **Phase S — rapid-descent blind spot.** `adaptive3d` picks a rapid-descent
  floor from a zero-radius point probe, and `collision.rs` — sole producer of
  `rapid_collision_count` — uses one too, so the detector cannot see the class
  of event the emitter can produce. Roughing, not finishing; safety, not
  efficiency; verified by replaying emitted G-code against a fine dexel, not by
  the F-034 integrator every track here uses.
- **Phase M — load-lane engagement normalised by the shank.** Empties gates
  through the `< 0.02` filter and poisons `air_cut_pct`. It is a gate/metric
  integrity problem, and it will turn currently-green verdicts red on tapers.

Neither blocks any track below. **Phase M must land before Track F**, or Track F
validates against a poisoned metric — and note that some measurements in
`FINDINGS.md` used air-cut readings on a taper and inherit that caveat.

---

## What is already settled (do not re-litigate)

| claim | evidence |
|---|---|
| Contour cascade on WHOLE regions loses to raster | 0.91×, §0d |
| Width-based "thin region" routing fires on nothing | 0.0% of area, §0b/§0c |
| Sweep angle is worth ~1.10× searched, 1.04× via gated PCA | §0e/§0f |
| Fragmentation is already solved by relink | 757 crossings → 97 retracts, §0d |
| **Ceiling HEIGHT dominates everything measured** | **1.57× on one region, path unchanged**, §0g |

**Contour-per-CELL is NOT refuted.** §0d tested contour on undivided regions.
On a monotone cell the offset cascade's pathology (a long, wildly-varying
perimeter) does not arise. Track C/D exist to test the claim that was actually
made, not the one that was refuted.

---

## Cross-cutting requirements — apply to EVERY track

These are not a track. Anything that fails one of these is not done.

- **X1 — Profile-general, never tool-specific.** Any clearance/contact question
  answers through `MillingCutter::height_at_radius` (a required trait method,
  implemented by flat / ball / bullnose / tapered_ball / vbit), never a scalar
  radius standing in for the shape. A flat endmill must come out
  **byte-identical**: `height_at_radius(r) == Some(0.0)` inside its radius, so a
  profile-aware rule reduces exactly to today's disc. That equality is the
  safety anchor and every profile change must pin it.
- **X2 — Operation-general, never one-op.** The operator's ask: *"not just the
  path planning here but an option if possible anywhere."* If a primitive is
  useful to links, it is useful to entry descents, collision checks and
  playback. Build it where every op can call it, and name the adopters.
- **X3 — GUI-exposed where it is an operator decision.** Operator's ask, and
  the repo has a named anti-pattern: the strategy advisor shipped MCP-only with
  zero GUI surface and no apply path; `FinishPlannerParams`' merge radius and
  min-island dials sat un-exposed behind a four-line override site. A dial the
  operator cannot see is a dial that does not exist. Every track states its GUI
  surface, or states explicitly that it has none and why.
- **X4 — Measure before building.** Every track opens with the cheapest
  experiment that could falsify it. §0c/§0d are the precedent: a measurement
  killed a 300–400 line change before it was written, and a second measurement
  inverted the conclusion of the first.
- **X5 — Default-off / byte-identical until switched.** New behaviour ships
  inert, with a golden proving the old path is unmoved.
- **X6 — Report-only findings follow the `ToolpathStats` contract.** `None` =
  not measured, `Some(0.0)` = measured clean. Never coerce absent to zero.

---

## Track A — Tool-profile clearance *(A1 LANDED 994996b7)*

**The measured prize, and the only one that is not confined to one band.**

Links, entry descents and collision all ask the same question and all currently
answer it with a cylinder. `LinkCeiling` read stock over a flat disc of the
ENVELOPE radius; on the R1.0 taper only material within ~1.5 mm can physically
touch the cutter while the code reached 3.0 mm — a 2× over-reach that lifted
every link to the height of ridges it could never hit. Those lifted links are
the "huge walls" the operator sees; they are not retracts.

**A1 — DONE 2026-08-28** (`994996b7`, gate 237 binaries / 3453 passed) — profile-aware ceiling: `max over r of [material_top(r) −
height_at_radius(r)]`, replacing the flat disc. Contained: one production
caller.
**A2** — promote it to the shared primitive X2 demands: *given a heightfield, a
point and a cutter, the lowest tip Z that clears everything the cutter can
touch.* Name and convert the other adopters.
**A3** — re-baseline `FINDINGS.md` §0d–§0f. **Every number there used
`link_ceiling: None`**, the fresh-stock arm; the live tier is a rest op. The
ceiling regime moves times by more than any path-topology effect measured, so
those margins are provisional until re-run against a realistic ceiling.

*GUI (X3):* none directly — this is a correctness fix, not a dial. But the
viewport should stop drawing walls, which is the operator-visible acceptance.

---

## Track B — Profile adoption in the finishing path *(B1 DONE)*

Track A found one instance. The question is how many more there are.

Context that raises the stakes: this repo **already ran a radius tech-debt
programme** (`planning/review_2026-07-29/RADIUS_AUDIT.md`, ~16 waves, declared
complete 2026-08-04) and it still carries open item **R-12** — drill gates
dividing by envelope radius with `ToolProfile::Flat` hardcoded, overstating
diameter up to 14× on a tapered ball, where two of the gates block export so the
failure mode is a **silent pass**. Track A's find is the same class, live, after
that programme closed.

**B1 — DONE 2026-08-28.** `RADIUS_AUDIT_2026-08-28.md`. It found more than this
track should own: the two heaviest groups are now **Phases S and M** in
`planning/rapid_safety_2026-08-28/PLAN.md`.
What follows is the finishing-scoped remainder.

**B2 — adopt the profile primitive where it is finishing work.**
`optimize_entry_descents` (S1) is the direct sibling of the ceiling fix already
landed — same flat-disc-at-envelope defect, and the surplus is spent as *fed
plunge*, up to ~9 mm per entry on this board. Note it is currently ledgered
"DO NOT TOUCH" (`TOOL_SCALE_SEMANTICS.md` §8 item 9, assigned to A/M10); that
ruling predates the primitive and should be revisited rather than obeyed by
reflex. Then S5 (pencil's search bound under-reaches at the tip — widening to
the envelope is now strictly better and free) and S3 (`scallop.rs:1826`,
`steep_shallow.rs:559`, the two consumers never migrated off
`legacy_envelope_quarter`).

**B3 — display correctness (X3).** U11: the engagement diagram draws a taper as
a **cylinder** under a "Show the math" label, and the vendor-LUT viewer
green-highlights a different row than the recommendation used. S6: the adaptive3d
"Optimal load" slider maps on the tip while the engine uses
`engagement_radius_mm(dpp)`, so the displayed % is over-stated (20% low at
DPP 3, 90% at DPP 10). S7: `PlannerToolRow` carries no envelope, so
`rim_erosion_mm` can never be seeded from the dialog its own tooltip points at.
These are operator-trusted surfaces showing the wrong number — cheap, and
squarely X3.

**B4 — a sentry class that makes the defect hard to reintroduce:** for a
non-cylindrical cutter, assert profile-derived and radius-derived answers
DIFFER where they should, so a future scalar substitution fails loudly instead
of reading plausibly.

**B5 — two ledger corrections** the audit established:
`TOOL_SCALE_SEMANTICS.md` §8 item 7's "keep the envelope" ruling is **wrong** for
two immersion-angle sites (width-at-depth questions wearing a force-lane badge);
and CLAUDE.md is **stale** where it says the chipload heat-map still carries the
F-HEATMAP mismatch — that is closed.

*GUI (X3):* none — invisible correctness. Unsafe findings may need an operator
warning surface if any currently passes silently.

---

## Track C — Region splitting (cell decomposition)

**The operator's own model, and the textbook answer agrees with it.**

Take the `PlannedRegion` polygons the decomposer already produces and split each
further at *critical points* — where a sweep line's intersection with the region
splits or merges. Each resulting cell is **monotone**: one sweep crosses it
exactly once, so a raster inside it cannot fragment.

The operator's horseshoe is the canonical worked example: swept vertically it
splits into **left leg / arch / right leg**, three cells — which is what they
drew unprompted.

**C1** — measure first (X4): how many cells does the wanaka shallow band
actually decompose into, and what is each cell's elongation and monotone
direction? Cheap: pure 2D on polygons already in hand, no generation.
**C2** — implement boustrophedon/Morse decomposition on `Polygon2`.
**C3** — cell adjacency graph + visit order (a TSP over cells, not over
fragments).

*Prior art, verified in `FINDINGS.md` §5:* Choset & Pignon 1997 (boustrophedon
cellular decomposition); Acar & Choset 2002 (Morse decompositions). The general
covering problem is NP-hard — Arkin, Fekete & Mitchell 2000, the milling problem
— so this is an approximation by construction, and that is fine and standard.

*GUI (X3):* cell overlay in the preview, reusing the tier-map/rest-heatmap slot
the multitool planner already renders through. The operator must be able to SEE
the split before generation — same veto shape as the tier-map preview.

---

## Track D — Per-cell milling strategy *(depends on C)*

Once cells exist, each gets its own decision. This is where the operator's
"bent parallel" intuition lives.

**D1** — per-cell sweep DIRECTION. Cheapest form of Track C's payoff and
partially measured already: gated PCA gives 1.04× at region scale (§0f), and a
monotone cell is exactly the shape where a single axis is meaningful, so it
should do better per cell than per region.
**D2** — per-cell PATTERN: raster vs contour vs spiral. §0d refuted contour on
undivided regions; on a monotone cell its perimeter is short and well-behaved,
so the refutation does not carry over. **Re-test, do not assume either way.**
**D3** — "bent parallel", the operator's actual words: passes that follow the
cell's curvature by interpolating between its two bounding curves, rather than
straight passes at one angle. Strictly better on a curved arch, strictly more
work. Prior art: Held & Spielberger's medial-axis spiral / morphed paths.
Do only if D1/D2 leave measurable room.

*GUI (X3):* per-cell strategy must be visible and overridable — at minimum
shown in the preview, ideally an operator override per cell.

---

## Track E — Measurement rig *(underpins C and D)*

`tests/thin_organic_island_widths.rs` grew Stages A–H tonight and is now doing
work no throwaway should: generating two variants over one region and costing
both through the F-034 integrator. Every track above needs exactly that.

**E1** — promote it into a reusable harness: *given a region, a tool, a machine
and N candidate strategies, return integrated time / cutting distance / retracts
/ links for each.*
**E2** — keep it `#[ignore]`d and mesh-optional. It is an evidence instrument,
never a gate.
**E3** — the standing lesson it encodes: **relink BOTH arms before comparing.**
The first Stage D compared a bare raster against a natively-chained cascade and
reported the cascade winning 1.05×; giving the raster the relink production
actually applies inverted it to 0.91×. An unfair comparison is worse than none.

*GUI (X3):* none — developer instrument.

---

## Track G — Link ROUTING: the missing detour

**Operator observation, 2026-08-28:** *"we retract up when I visually see a path
to traverse around which looks smaller."*

Confirmed by reading, not inferred. `build_surface_link` (`surface_link.rs:24-52`)
interpolates a **straight line** from `from` to `to` and returns `None` on the
first sample that loses contact. So the candidate set a junction is ever offered
is exactly:

```
{ straight line A→B ,  full retract to safe_z }
```

There is **no path-finding**. A lateral detour around a hole, a gap or a
standing ridge — the path the operator can see — is never generated, never
costed, never considered. The operator's hypothesis (that the retract exists to
preserve a plunge into stock) is not the cause: the retract is simply the only
fallback when the straight line is refused.

Two aggravating details:

- **One sample decides it.** A single non-contacted point returns `None` for the
  whole link. Not "mostly clear", not "clear if it bulges 3 mm sideways".
- **The reorder is equally blind.** `reorder: true` picks the next fragment by
  straight-line XY distance (`NearestPicker`), so it can select a neighbour that
  is close in XY but separated by something no straight link can cross, and then
  pay the retract anyway. Ordering and linking use different notions of
  "reachable".

**G1 — MEASURE FIRST (X4).** `RelinkReport` already counts declines by reason
(`too_far`, `off_surface`, `slower_than_retract`, `outside_boundary`,
`ceiling_above_safe_z`) and `narrate_toolpath` prints the attribution. Print the
breakdown per region on the wanaka shallow band. **The whole track is
conditional on this**: if `off_surface` dominates, the straight-line assumption
is the cause and a detour pays. If `slower_than_retract` dominates, the links
are being costed away and routing changes nothing. If `ceiling_above_safe_z`
dominates, this is Track A and not a routing problem at all. Do not build until
this number exists.

**G2 — a third candidate: the detoured stay-down link.** Cheapest useful form is
a small fixed set of offset arcs/waypoints rather than a general planner —
propose a handful of detours, drop-cutter each, keep the cheapest that clears,
and let the existing F-034 cost gate choose between it and the retract. The
comparison machinery is already there; only candidate GENERATION is missing.

**G3 — make ordering and linking agree.** If a detour becomes available, the
reorder should rank by *link-reachable* cost, not raw XY distance.

*Relationship to Track A:* independent but adjacent — A lowers how high a link
must fly, G decides whether it may go around instead of over. Both are "the link
geometry is naive"; neither subsumes the other. Sequence G after A1 so the
decline breakdown is read against a corrected ceiling, or the attribution will
blame routing for what was ceiling height.

*GUI (X3):* none directly. The operator-visible acceptance is the same as A —
fewer walls in the viewport.

---

## Track F — Validation and operator surface

**F1** — the real wanaka A/B, sim at tool-appropriate resolution (operator rule:
≈ tip diameter ÷ 10, bound by the smallest tool in the run — which is why the
R0.5 pencil forced 0.1 mm).
**F2** — operator eyeball. Under the C4 rule the eye is the accepting gate for
surface quality, and any pattern change alters the cusp pattern.
**F3** — GUI sweep (X3): every dial these tracks add, actually exposed. Audit
against the strategy-advisor anti-pattern before declaring any track done.

---

## Ordering and dependencies

```
A (clearance)  ──┬──► G (link routing / detour)   both are "link geometry is naive"
B (radius audit) ─┤    G reads its declines against A's corrected ceiling
                  │
E (rig) ─────────┼── underpins C and D
                  │
C (split) ──► D (mill per cell) ──┐
                                   ├──► F (validate + GUI)
A3 (re-baseline) ─────────────────┘
```

**Session-sized chunks**, roughly:

| session | content |
|---|---|
| 1 | A1 + A2 land and verify; B1 audit read |
| 2 | B2 fix wave (unsafe first); A3 re-baseline; **G1 decline breakdown** |
| 3 | E1 rig; C1 measurement; G2 only if G1 justified it |
| 4 | C2 + C3 decomposition |
| 5 | D1 + D2 per cell, measured on the rig |
| 6 | F validation + GUI sweep |

G1 is deliberately cheap and early: it is one print of data the code already
computes, and it decides whether Track G exists at all.

D3 and any second-order work only if 5 leaves measurable room.

## Test policy — do NOT run the full gate to iterate

Operator correction, 2026-08-28: the 2026-08-27 session ran the full
`--features heavy-tests` gate **three times** (~25 min each) while iterating.
That is a misuse. CLAUDE.md already states the split; this restates it as an
operating rule for the tracks above, because six planned sessions of this would
be hours of wasted machine time.

| when | command | cost |
|---|---|---|
| **while iterating** | `cargo test -p rs_cam_core --lib <mod>` or `--test <one>` | seconds |
| **area check** | `cargo test -p rs_cam_core -q` (dev loop) | **431 s** — does not even COMPILE the 12 heavy binaries |
| **commit gate only** | `cargo test -p rs_cam_core --features heavy-tests --no-fail-fast -- -q` | **~1,663 s serial-equiv; the 12 heavy binaries are 75% of it** |

**Run the full gate ONCE, immediately before a commit** — not after every edit,
and never to answer "did that compile" (`cargo check` does that in seconds).

### Choose the heavy binaries by blast radius

The 12 are not a unit. For a **link / ceiling / clearance** change, the ones
that can actually move are:

```
scallop_isofield_gouge_m4     scallop_candidates_m4     scallop_oracle_validation_m4
air_cut_family_calibration_w5bf4        strategy_advisor_smoke
```

(scallop relinks intra-pass; air-cut is a link-sensitive metric; the advisor
runs full generation.) These cannot:

```
feed_modulation_cycle_time_f036c   machine_kinematics_cycle_time_f034
offset_growth_m5   offset_candidates_m5
adaptive3d_planner_stock_xy_f027   adaptive3d_interior_cell_parity_f029
checkpoint_b_resolution_ab   (unless the resolution/census seam moved)
```

Naming a subset with `--test x --test y` is legitimate mid-track. The **full**
set still runs once before the commit, because "I reasoned it couldn't be
affected" is exactly how a regression ships.

### And `--ignored` stays off

`#[ignore]` in this crate means *evidence run, invoked explicitly, never by a
gate* — 272 of them (gcode-emulator tests needing an external validator, param
sweeps, WANAKA evidence runs). Heaviness is the `heavy-tests` FEATURE's job.
Never pass `--include-ignored` to a gate; conflating the two mechanisms is a
mistake this repo already made once and reverted.

---

## Standing risks

- **A3 invalidates numbers.** §0d–§0f are `link_ceiling: None`. Treat every
  margin there as provisional until re-run.
- **The prize may be small.** The shallow band is ~9,000 mm² of tier 1's
  29,148 mm² covered area — MidSteep is 27,488 mm² and already contoured. Path
  work on the shallow band is low single-digit percent of the op. **Track A is
  the exception**: it touches every relinked op on every board.
- **NP-hardness is real but not blocking.** Constant-factor approximations are
  the standard answer; the failure mode to avoid is chasing optimality rather
  than shipping a good approximation.
- **The C4 rule binds Track D.** Any pattern change needs the operator's eye,
  which cannot be automated away.
