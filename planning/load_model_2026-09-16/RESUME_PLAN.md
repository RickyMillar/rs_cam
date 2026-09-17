# Resume plan — the load-model programme

Written 2026-09-18. **Read this file first.** It is self-contained: a cold
session needs nothing else to restart, and it names the documents to open only
when a step needs them.

The programme began with one question: *how do we simplify the power readings
an operator sees?* Everything below still serves that. The detour into the
load model happened because the surface would otherwise have displayed numbers
whose inputs nobody had checked.

---

## 1. Where it stands

Shipped and verified:

| Commit | What |
|---|---|
| `a7c17be7` | The power ladder — RPM, depth, width, then feed as a LAST rung |
| `809d28b9` | T-11 — feed modulation reads mm, not a fraction of another quantity |
| `ad2f749b` | T-12 — the depth recommendation reaches all 24 operations |
| `70a3db27` | A failed collision check no longer reads as a clean one (CLI) |
| `6aca2ffd` | T-16 — the false bending-diameter citation removed |
| `9405f3f6` | Three surveys, and T-18 discovered |
| `93dd145c` | **T-17 — the deflection integrator bends a flute-relieved section** |
| `6d69c25a` | T-17 closed in the register |
| `0a510215` | The fluted-section derivation the code cites |

T-17 verification: core lib 2503/0, sentry 5/5, `literature_matrix` 21/21,
`literature_parity` 24/24, clippy clean, fmt clean.

**One check is outstanding and it is real.** `the_corridor_bounds_the_band_g_corridor`
in `rs_cam_viz` was never run. It PANICS rather than drifts if the force budget
crosses the edge force, and T-17 made every tool 2.441x more compliant, so it
is a plausible red. It was skipped because a second session was rewriting
`rs_cam_viz`. **Run it first thing:**

```
scripts/cargo_lane.sh test -p rs_cam_viz -j 2 --test the_corridor_bounds_the_band_g_corridor
```

If it panics on `measured()`'s `.expect(...)`, the fixture `LONG_AND_THIN`
(Ø1.5, 90 mm stickout) now has its edge force above budget. That is the same
true finding as the `gate_honors_optimizer_feed_down_into_within` repair: some
cuts are no longer rescuable by feed. Re-derive the fixture; do not widen the
arm. The file's own doc block (lines 12-23) says so.

---

## 2. Standing rules — these bind every step

- **Run every cargo command through `scripts/cargo_lane.sh <args>`.** It takes
  a lock, waits for foreign cargo/rustc and for 12 GB free, then runs `nice`.
  Two concurrent builds have crashed this machine.
- **Never run the core full gate locally without asking.** CI owns it. Ask
  before any run over about three minutes.
- `rs_cam_viz` tests need `-j 2`. Its 46 test targets are what took the
  machine down.
- **The working tree and the git INDEX are shared with other sessions.** Stage
  by explicit path, verify the staged set immediately before committing, and
  prefer `git commit -- <pathspec>`. Never stash, reset, checkout or clean the
  whole tree. Read baselines with `git show HEAD:<path>`.
- **Never push. Never open a pull request. Never `git reset --hard`.** Never
  stage `.mcp.json` or `.pi/`.
- Prose in ASD-STE100 Simplified Technical English.
- **No bench rig.** Every constant is math-derived-and-checkable or straight
  from named literature. A fabricated constant is worse than an absent one.
  When a value cannot be sourced, leave it unmodelled AND say which direction
  the error goes.
- A sweep that does not fire is evidence about the sweep, not about the code.

---

## 3. Decisions already taken — do not reopen without new evidence

- **The bending fraction is a flat 0.80, with no flute-count term.** Kops and
  Vo, Annals of the CIRP 39(1):93-96 (1990), measured from compliance. The
  operator ruled on this on 2026-09-17 after the derivation showed the
  published per-flute table's N-trend is an artefact of one thesis's flute
  model, and that its two-flute figure is the arithmetic mean where a rotating
  tool needs the harmonic mean. See `FLUTE_SECTION_MATH.md`.
- **Flat, ball, bull and tapered ball all take the fraction. A V-bit does
  not**, and that gap is NON-conservative. T-4 makes it visible.
- **T-7 is withdrawn** — the helix-wrap premise fails.
- **`MILLING_KC_FACTOR` is inert on every load path** — it cancels between the
  numerator and the anchor. Verified numerically. Do not re-derive it.
- **The "+36 %" predictor over-shoot is withdrawn**, not re-measured. It was
  written against a predictor deleted ten days later.

---

## 4. NEXT: T-4 — a refusal must not read as a measurement

`predict_peak_deflection_um` has **nine** refusal exits and every one returns
`predicted_um = 0.0`. A modelled zero is unreachable, so today every `0.0` is
an absence. Four production callers read it. The back-off treats the refusal
as a pass-through and says nothing.

**Full design, with the enum already written out: `T4_REFUSAL.md` section 4.**
Follow it. The decisions inside it are settled:

- Reuse `tool_load::verdict::UnmodeledReason` as the shared VOCABULARY so the
  pre-sim and post-sim abstentions print the same words.
- Do NOT reuse `CriterionStatus` as the container — it carries
  `sample_range`, `population`, `confidence` and `exceeded`, which a
  closed form at one operating point cannot fill.
- Do NOT reuse `GatePopulation` — X-VAC is about an empty evidence set; this
  is about an inapplicable model.
- The shape to copy is `feeds::force::DeflectionCapRefusal` (`force.rs:167`),
  in the same folder. Third time the repository has drawn this.

Work items:

1. Add `DeflectionUnmodeled` and `as_unmodeled_reason`, per T4_REFUSAL §4.
2. **Delete the stale V-bit blanket guard** at `predict.rs:219`. Its comment
   says "the post-sim integrator handles V-bits; the closed-form predictor
   refuses", but since `3b0dc487` the predictor CALLS that integrator, and
   `cutter_constraints.rs:716` already computes a V-bit deflection bound from
   it. **But do not simply let the number through**: the derivation says a
   V-bit's cone is modelled while its flute relief is not, and that error is
   non-conservative. Return the value carrying a `FluteReliefUnmodeled` mark.
   This synthesis is the point — one agent said "delete the guard", another
   said "a V-bit is unmodelled", and both are half right.
3. The two UNDOCUMENTED refusals at `predict.rs:396`, hidden in a
   `.map_or(0.0, …)`: `Material::Custom` refuses there even when
   `kc_n_per_mm2()` returned `Some`, and a zero stickout refuses there. The
   module header under-states its own refusal set.
4. `CutterOpProfile::predictions` (`feeds/profile.rs:206`) has **no reader
   anywhere**, production or test. Propose deleting it rather than migrating
   it through the signature change. Confirm with `rg` first — the graph tools
   under-report callers.
5. Make the back-off ACT on the refusal instead of passing through silently.
   The sharp case: a V-bit or an unvalidated plastic on a roughing operation
   skips the DPP back-off and says nothing.

Sentry: name it for the claim, `<claim>_g_<code>.rs`, and give it a
non-vacuity anchor. Read `crates/rs_cam_core/tests/CLAUDE.md` first.

---

## 5. THEN: T-18 — a feed lift caps against the travel rate

Full entry in `TECH_DEBT_REGISTER.md`. Three sites cap a feed lift at
`machine.max_feed_mm_min * safety_factor` — the gantry TRAVEL rate — while
Step 7 clamped at `cutting_feed_ceiling_mm_min()`:

- `feeds/mod.rs:2349` (rubbing-floor lift)
- `feeds/mod.rs:2381` (drill envelope clamp)
- `feeds/suggest/adaptive_entry.rs:470`

`adaptive_entry.rs` uses BOTH, 49 lines apart. Step 7's own comment says the
rule: *"the calculator emits CUTTING feeds — clamp at the cutting ceiling, not
the gantry travel rate."*

Replace all three with `machine.cutting_feed_ceiling_mm_min()`. **One decision
to take deliberately:** Step 7 does NOT apply the safety factor to the
ceiling, and the three lift sites DO apply it to travel. Pick one, and say why
in the code.

Latent today only because the rubbing floor caps chipload at 0.025 mm/tooth on
the shipped presets. That is evidence about the presets.

Same structure as **T-15** (a late step raises a feed the power ladder
clamped). Consider one guard that re-checks every ceiling after the last lift,
rather than three patches plus a fourth for T-15.

---

## 6. THEN: T-9 — small, take it while the file is open

`feeds/suggest/apply.rs:79` rounds the feed to the NEAREST whole mm/min after
every clamp has bound it. Worst case **+0.5 mm/min**, against ADVISORY
ceilings only; no kinematic maximum is breached. Add a round-DOWN twin of
`round_suggestion_value`, the same shape as `machine::next_rpm_at_or_below`.
Detail in `T9_CEILING.md`.

Also from that survey, cheap and worth doing: `rs_cam_viz/src/io/export.rs:21`
passes `max_feed_mm_min: None` into the validator, which disables the one
check that could catch any of this in the emitted file.

---

## 7. THEN: the limits surface — the original request

**Do not start until the `rs_cam_viz` wave lands.** Design is in `PLAN.md`
(§11 recommends Reading A: depth as a post-sim criterion, not gating
initially), `SURVEY_UI.md` and `REVIEW_DESIGN.md`. The interactive prototype
is `derate_bench.html`.

The important discovery: **`CriterionStatus` and `verdict_badge` already do
most of this.** The prototype was 80 % a reimplementation, and worse on the
chipload floor percent. Extend the existing surface; do not build a parallel
one.

The operator's stated requirements, verbatim in effect:

- Treat every limit as the same kind of thing: **0 to limit**.
- Wording like "limit calculated from X" belongs in a hover `(i)`, not on the
  face, and in standardised technical English.
- It must **trace back to the setting that set it** — machine kinematics, tool
  choice, material.
- *"Treat it as a multimeter, not a verbal explanation."*
- After simulation, show the **distribution and the limits**, with the full
  graphs behind a click-through modal. At the toolpath stage, where those
  metrics do not exist, show predictions.

**Two rulings still needed from the operator before building:**

1. Drop the `≈` confidence mark, or keep it?
2. Reinstate the power bar on the Feeds card? It was removed on a 23.6 % peak
   utilisation figure that is now stale — re-measured after R1: median 17.8 %,
   p90 89.4 %, peak 100 %. The doc's own stated condition for reinstating is
   met.

---

## 8. Still open, lower priority

- **T-15** — pass 9 can raise a feed the power ladder clamped. Reachable by
  hand today. Pairs with T-18.
- **T-14** — a drop-cutter finishing pass measures 42.5 mm of axial
  engagement. The power ladder made it load-bearing.
- **T-2, T-3, T-5** — guard and structure debt, no physics.
- **T-10** (gantry thrust) and **T-13** (`F_edge` per mm of depth) are
  BLOCKED on a number that cannot be sourced. Under the no-bench-rig rule they
  stay unmodelled. Do not invent a constant to close them.

---

## 9. The three defect classes this repository keeps rediscovering

Every confirmed defect in this programme is one of these. When reviewing new
code, look for them by name:

1. **A value computed at one state, consumed at another.** (`cut_efficiency`,
   `realised_step_down`, `mrr_mm3_min`.)
2. **A quantity multiplied by a fraction of a DIFFERENT quantity.** (T-11;
   the GUI nomogram's `preview_power_kw`; the optimizer's stepover advice;
   T-17's engagement-vs-bending diameter; T-18's travel-vs-cutting feed.)
3. **An absence rendering as a reading.** (T-4; the CLI collision defect;
   `feeds::calculate` reporting `power_kw = 0.0` for a material with no `Kc`.)

Class 2 is the most common and the hardest to see, because the two quantities
share units and magnitude. Only the name differs.
