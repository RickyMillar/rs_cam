# Rapid-descent safety + load-lane integrity — separate phase

> Split out of `planning/thin_organic_2026-08-27/PROGRAMME.md` on 2026-08-28 by
> operator ruling: *"it does sound very critical, but a separate path. It should
> be a separate phase."*
>
> Source: `planning/thin_organic_2026-08-27/RADIUS_AUDIT_2026-08-28.md`, a
> read-only sweep triggered by the link-ceiling defect.
>
> **Everything here is a CODE READ.** No claim below has been reproduced at
> runtime. Magnitudes are analytic. The first task in each phase is a
> measurement, not a fix.

## Why this is not part of the finishing programme

| | finishing programme | this phase |
|---|---|---|
| subsystem | finish ops, links, region planning | `adaptive3d` roughing, `collision`, `stamping`, `tool_load` |
| what it trades | time | **safety** (S) / **metric truth** (M) |
| verification | F-034 integrator, A/B on wanaka | emitted G-code vs fine dexel (S); sim traces + verdict diffs (M) |
| heavy binaries | `scallop_*`, `air_cut_family`, `strategy_advisor` | `adaptive3d_planner_stock_xy_f027`, `adaptive3d_interior_cell_parity_f029` |
| dependency | tracks depend on each other | **blocks nothing, blocked by nothing** |

Both phases are independent of every finishing track, so queueing them behind
six sessions of path work would be an accident of scheduling rather than a
decision. Phase M additionally has to land **before** the finishing programme's
Track F validation, or Track F grades itself against a poisoned metric.

---

# Phase S — the rapid-descent blind spot *(highest severity)*

## The defect

**S-a — the emitter.** `adaptive3d/clearing.rs:1338-1342` (`sample_stock_top_at`)
chooses a rapid-descent floor from a **zero-radius point probe**, using `ray_top`
(the cell-centre read) rather than `conservative_top`. Nine producers feed it
(`:549, :1843, :1900, :2283, :2310, :2356, :2387, :2417`) and it is emitted as a
**rapid** at `adaptive3d/path.rs:1480, :1503, :1534`, descending to
`rapid_floor_z + 0.5`.

Off-axis material is invisible to that probe. Sub-cell slivers are invisible.
The only guard is `RAPID_DESCENT_BUFFER_MM = 0.5` — which is *precisely* the
"cell-scaled pad" that `dressup.rs:318-336` describes as able only to "chase
that class" and **deleted**. This site kept the pad and never received the query
that replaced it.

Magnitude: on a flat Ø6.35 the blind radius is the full 3.175 mm. On the shipped
R1.0 taper, material 0.5 mm off-axis needs only **0.11 mm** of standing height
to strike.

**S-b — the detector shares the blind spot.**
`collision.rs:450-543` (`check_rapid_collisions_against_stock`, probe at `:526`)
is also a **zero-radius point probe**, and takes no cutter argument at all. It
is the **sole producer of `rapid_collision_count`** — the metric CLAUDE.md calls
*"the most reliable signal… the primary did-anything-bad-happen indicator"*.

> A rapid descending into off-axis material is exactly the event this counter
> cannot see. The emitter's defect and the detector's defect are the same
> defect, so they mask each other.

**S-c — rapids are exempt from the other collision checks.**
`collision.rs:222-228` and `:352-359` skip rapids entirely, so holder and
fixture strikes on a rapid are checked by nothing.

**S-d — `check_collisions*` never sees remaining stock**, only the mesh. On a
`FromRemainingStock` chain the material that actually exists is not what is
tested.

All four are **NEW** — they survived the radius programme that closed
2026-08-04.

> **S1 OUTCOME (2026-08-28): STRIKES FOUND — but in the finishing link
> descents (ops 7/8: 982 descent strikes, 634 beyond discretisation, worst
> −1.30 mm; op 7 struck on 601/601 links), NOT in adaptive3d, whose suspect
> path emitted no below-top descent on this job (untested, not exonerated).
> Setup 1 fully clean. See `S1_RESULTS.md`; instrument
> `tests/rapid_replay_shipped_gcode_s1.rs` (v2 — v1's 651 strikes were
> retract-start artefacts, exempted on a monotone-profile proof). S3 should
> target the finish link planner first; S2's detector fix must make this
> exact class visible.

## S1 — MEASURE FIRST, and not with the broken instrument

The whole phase hinges on one question: *has this ever put a rapid into
material on a real project?*

**`rapid_collision_count` cannot answer it.** That is S-b. Any plan that opens
by consulting it is circular.

The honest measurement is the repo's own standing rule — *measure emitted
motion, not the plan*: take shipped G-code (the wanaka `.nc` files in
`planning/airrun_2026-08-19/` are already in the tree), replay it against a
fine-grid dexel of the same job, and ask whether any **rapid** segment ever
intersects material. Grid must be fine enough to resolve the sliver class —
sub-tool-radius, so ≲ 0.1 mm, which OOMs a full board and therefore wants a
windowed replay around rapid segments rather than a whole-board sim.

Outcome decides everything downstream:
- **Strikes found** → this is a live safety defect; fix immediately, and the
  `.nc` files already shipped need review.
- **No strikes, but near-misses within the blind radius** → latent; fix on
  merit, no alarm.
- **Nothing close** → the geometry has been protecting us; fix still correct but
  drops to ordinary priority. Say so plainly rather than inflating it.

> **S2 MECHANISM RESOLVED (2026-08-28, see S1_RESULTS.md §3):** the checker's
> timing, frame and F3 logic are healthy (proven by
> `tests/rapid_check_wanaka_link_shape.rs` + a traced pipeline run); the
> silence is S-b proper — the strikes are 0.5–2.9 mm off-axis (inter-pass
> crests at rough swath edges) and the zero-radius point probe cannot see
> them. Fix shape: evaluate rapids against the LIVE stock inside the replay
> walk with `max_clearance_tip_z_for_profile` (a disc upgrade on the frozen
> snapshot would over-flag the op's own already-cut rows). Drill entries
> (analytic path) keep the pre-pass.
>
> **S2 LANDED (2026-08-28, see S2_RESULTS.md):** live-walk profile-aware
> rapid check; falsification PASSED — the wanaka run went 0 → 202
> rapid-through-stock collisions (op 7: 179, op 8: 22, drills 0), dev loop
> green. Known limits ledgered: kerf-rim √-over-read on vertical flanks,
> no traverse early-out, S5 prefix restores pre-fix counts.

## S2 — fix the detector before the emitter

Deliberate ordering. If the emitter is fixed first, the detector still cannot
prove it worked. Give `check_rapid_collisions_against_stock` a cutter and walk
the profile — the primitive already exists
(`dexel_stock::max_clearance_tip_z_for_profile`, landed 994996b7) and
`compute/simulate.rs:1022` has both `entry.tool` and the LUT in scope.

Then re-run S1's replay: a fixed detector on unfixed emission should *find* the
strikes if there are any. **That is the phase's own falsification test.**

## S3 — fix the emitter

> **S3 FALSIFIED CLEAN (2026-08-28, see S3_RESULTS.md): pipeline 202 → 0
> with the live detector watching; independent S1 replay on the fresh
> emission 982 strikes → 2, both sub-noise, zero beyond discretisation;
> S2 crest sentries stay green. Cost: +9.6% cycle estimate (fed plunges
> returned). Below, "IMPLEMENTATION LANDED" was this note's pre-run state.**
>
> **S3 IMPLEMENTATION LANDED (2026-08-28) — FALSIFICATION PENDING.** The
> emitter S1 measured is **not** `sample_stock_top_at`: it is
> `dressup::filter_air_cuts`. The generators emit a safe fed `EntryPlunge` at
> every raster link (`toolpath.rs::raster_toolpath_from_grid`); the filter
> then classified that plunge "all air" with a **zero-radius centerline
> probe** — its `tool_radius: f64` parameter was documented "reserved for
> future per-cell radius checks" and never read — dropped it, and emitted the
> retract/hop/**rapid-descend-to-resume-Z** triple S1 read out of the shipped
> `.nc`. The filter runs only when `prior_stock` is present
> (`compute/execute.rs` step 7), which is exactly the four
> `FromRemainingStock` ops where every measured strike lives. Emitter and
> pre-S2 detector shared one blindness, which is how they masked each other.
>
> The fix: each sample is judged for the whole cutter by
> `dressup::sample_is_air_for_tool` — cheap centerline test first, then
> `max_clearance_tip_z_for_profile` over the envelope disc to CONFIRM any air
> verdict (stage-1 material implies stage-2 material, so the short-circuit is
> exact; the argument is at the function). `filter_air_cuts`,
> `filter_air_cuts_with_provenance` and `swept_path_is_all_air` now take
> `&dyn MillingCutter` in the dead radius's place; `apply_dressups` gates step
> 7 on `prior_stock` **and** `cutter`, and both production callers
> (`session::compute`, the viz worker's `helpers::apply_dressups`) now pass
> one unconditionally. The all-or-nothing whole-move rule then restores the
> fed plunge entire, and `optimize_entry_descents` re-splits its airborne top
> as before — that pass is untouched.
>
> Sentries: `tests/air_filter_tool_aware_s3.rs` — the measured class stays
> fed, a link the whole tool clears still converts, a taper and a flat endmill
> of the same envelope radius give opposite verdicts on one ridge (profile,
> not envelope), and the filtered link replays through S2's detector with zero
> collisions while the hand-rebuilt pre-S3 emission flags.
>
> **Still to run (orchestrator):** the CLI falsification (202 →
> ~0 rapid-through-stock collisions on wanaka200 at 0.3 mm) and S1's replay
> against regenerated G-code. Expect air-cut % and rapid/cutting distance
> splits to move on `FromRemainingStock` fixtures — plunges that return to fed
> are the fix working. `tests/perf_golden_*` build every op with
> `StockSource::default()` (= `Fresh`), so no prior stock reaches the filter
> and the goldens should not move; if one does, that is a finding, not a
> re-baseline.
>
> Deferred, unchanged by this work: `sample_stock_top_at` and
> `RAPID_DESCENT_BUFFER_MM` below — S1 did not exercise that path, so it is
> still a code-read finding.

`sample_stock_top_at` reads the disc with the profile, and switches
`ray_top` → `conservative_top`. `ctx.lut` and `ctx.tool_radius` are already
threaded for stamping, so this is the natural first adopter of the LUT variant
of the primitive.

Then delete `RAPID_DESCENT_BUFFER_MM`, or justify it in writing as something
other than the pad `dressup.rs` already rejected. Keeping an unexplained pad
next to a correct query is how the next reader concludes the query is untrusted.

> **S4 STATUS (2026-08-28): OPEN, deliberately deferred behind Phase M.**
> S1–S3 are done and falsified; the phase's severity driver (silent rapid
> strikes) is closed from both sides — the live profile-aware detector
> (S2) would now catch any emitter in this class, including the unexercised
> adaptive3d floor (S-a). S-c (rapids exempt from holder/fixture checks)
> and S-d (mesh-only `check_collisions*`) remain real but are rarer
> geometry classes, and this plan already calls them "scope decisions, not
> obvious wins" needing cost measurement first. Phase M outranks them: its
> poisoned metric is being quoted today (S3's own air-cut% figures carry
> the caveat) and it blocks the finishing programme's Track F. The S5
> sliver case is likely already caught by the live check
> (`conservative_top` errs high); pin it with a green sentry when S4 opens.

## S4 — close S-c and S-d

Rapids get the holder/fixture check; `check_collisions*` gets an
against-stock arm. Both are scope decisions, not obvious wins — a stock-aware
collision check may be expensive, and its cost should be measured before it is
made unconditional.

## S5 — sentries

- A rapid descending toward a **sliver** narrower than one cell must be caught.
  Red first against today's code.
- A rapid passing beside a ridge at lateral offset `r` where
  `height_at_radius(r)` clears it must **not** be flagged — the fix must not
  trade a false negative for a false positive.
- Flat endmill: byte-identical, same anchor discipline as
  `flat_endmill_profile_ceiling_is_byte_identical`.

*Heavy binaries for this phase:* `adaptive3d_planner_stock_xy_f027`,
`adaptive3d_interior_cell_parity_f029`. Full gate once before commit.

---

# Phase M — load-lane engagement is normalised by the shank

## The defect

`dexel_stock/stamping.rs:1119` computes
`(perp_max − perp_min) / (2.0 * radius)` where `radius` arrives from
`compute/simulate.rs:1032` as `envelope_radius_mm()`. One scalar answers two
different questions in the same accumulator: the stamp bbox (envelope —
correct) and the **engagement denominator** (wrong; wants
`engagement_radius_mm(axial_doc)`).

**3.46× too large** at 0.5 mm DOC; arc `acos(1−2w)` **1.94× under-read**.

Consequences:

- **M-a, silent population loss.** The `radial_woc_fraction < 0.02` filter
  (`chipload.rs:315`, `power.rs:192`, `deflection.rs:87,:219`) discards a real
  **0.12 mm side bite** as air. A finishing pass at 0.1 mm stepover empties
  every gate → `Unmodeled{AllSamplesAirCutOrRapid}`, or `Within` with
  `peak_idx: None`. This is CLAUDE.md's own *"a gate handed an empty population
  passes and looks healthy"*, reached structurally rather than by accident.
- **M-b**, `tool_load/power.rs:205-206` multiplies a correct
  `engagement_radius(axial_doc)` by the envelope-normalised arc — self-
  inconsistent inside one formula, ~1.9× power under-read.
- **M-c**, `simulation_cut.rs:919` → `air_cut_pct_of_total_runtime`: the same
  threshold labels genuine cutting `AirCut`. **Air-cut % on a tapered tool has a
  zero set by the shank** — and it is the metric the whole finishing campaign,
  including this session's measurements, has been steering by.

Related low-reading gates in the same family: U4 (`optimize/context.rs:107`,
Ø6 vendor row for a Ø2 cutter), U5 (`cutter_constraints.rs:206`, vendor axial
cap 3× permissive), U6 (immersion angle — **and its ledger entry is wrong**,
see M4), U7 (= R-12, drill gates, already ledgered open), U8 (`apply_drill_op`
carves a **Ø6 hole where a Ø2 tip cuts** — 9× area, and downstream
`FromRemainingStock` ops then believe material is gone that is physically
there).

## M1 — measure the exposure before changing anything

On a real wanaka trace: what fraction of samples falls under the 0.02 filter,
and how much `air_cut_pct` is misattributed? Analytic ratios are known; the
population is not.

## M2 — expect green to turn red, and plan for it

Fixing this **raises** engagement, arc and power across the board on tapered
tools. Some currently-`Within` verdicts will become `Exceeds`. That is the fix
working, not a regression — but it means:

- capture before/after verdicts for every shipped fixture **as evidence**, not
  as a gate to be made green again;
- treat several existing verdicts on tapered tools as **provisional** until this
  lands, and say so wherever they are quoted;
- some of this session's own measurements used air-cut readings on a taper and
  inherit the same caveat.

## M3 — fix locally, not through the clearance primitive

U3 is a **local** correction at `stamping.rs:1119`, where the axial DOC is
already present in the same `StampPartial`. It needs
`engagement_radius_mm(doc)`, not a stock query. Do **not** route it through
`max_clearance_tip_z_for_profile` — different question, different primitive.

## M4 — ledger corrections

`TOOL_SCALE_SEMANTICS.md` §8 item 7 lists U6's two sites under *"must NOT touch
— keep the envelope"* (Rule 4, feeds/force/deflection). **Rule 4 is wrong for
those two:** they are immersion-angle computations, i.e. width-at-depth
questions wearing a force-lane badge. The rule was written to protect
physical-extent sites and over-reached. Correct the ledger in the same commit
that fixes them, or the next reader will revert the fix on the ledger's
authority.

Separately, CLAUDE.md is **stale** where it says the GUI chipload heat-map
"still carries the same mismatch on a visible surface" — F-HEATMAP is closed
(`render/toolpath_render.rs:590` now uses distinct newtypes).

---

## Ordering

```
S1 (replay measurement)  ──►  S2 (fix detector)  ──►  S3 (fix emitter)  ──►  S4, S5
                                    │
                                    └── S2 then re-run S1: a fixed detector on
                                        unfixed emission is the falsification test

M1 (exposure)  ──►  M2/M3 (fix + verdict diff)  ──►  M4 (ledger)
                          │
                          └── must precede the finishing programme's Track F
```

S and M are independent of each other and of every finishing track. S first on
severity; M before any Track F validation.

## Honest caveats

- **Nothing here is reproduced.** Every magnitude is analytic, from a code read.
  S1 and M1 exist because of that.
- **Severity is asserted, not demonstrated.** S may turn out to be latent — the
  geometry may have been protecting us all along. That is a real possible
  outcome and should be reported plainly if it happens, not talked up.
- **M invalidates instruments this repo relies on**, including some used earlier
  in the thin-organic investigation. That is a cost of fixing it, not a reason
  not to.
