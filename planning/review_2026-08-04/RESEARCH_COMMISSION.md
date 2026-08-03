# Research commission — the second tech-debt programme

You are a research agent. Your deliverable is a plan of attack modelled on
`planning/review_2026-07-29/TECH_DEBT_RESEARCH_AND_FIX_PLAN.md` — the plan
that drove the just-completed radius/instrument programme (16 waves, 5 human
checkpoints, ~90 commits, closed 2026-08-04). Study that document's SHAPE
before writing anything: per-item Problem / Research questions / Research
deliverables / Preferred fix shape / Acceptance gates / Scope and risk,
followed by an orchestration plan with explicit human checkpoints, a merge
order, and non-negotiable rules. Your plan will be executed the same way:
one orchestrator, sequential implementation waves, research-then-checkpoint-
then-fix for anything that changes machining behavior.

## Required reading before any code exploration

1. `planning/review_2026-07-29/TECH_DEBT_RESEARCH_AND_FIX_PLAN.md` — the
   template, including Addendum C (structural-safety items) and its
   C-sequencing pattern.
2. `planning/review_2026-07-29/ORCHESTRATION_LOG.md` — the full wave record.
   Every wave entry ends with findings and errata; the defects your plan
   inherits are recorded there, not in tribal memory.
3. `planning/review_2026-07-29/SUPERSEDED_CONCLUSIONS.md` — §6.B is the open
   ledger your plan must absorb (B1–B8 plus D-16.1, D-16.2, D-LV.1); §6.3's
   ruled items must not be re-litigated.
4. `planning/review_2026-07-29/ANTIPATTERNS_BACKLOG.md` — P1–P13 with
   dispositions. P7 (mega-harness rot), P11 (stale rationales), P12 (convex
   fixtures), P13 (deferral names its decider) are live practice rules.
5. `CLAUDE.md` + `FEATURE_CATALOG.md` + `AI_MACHINIST_ANALYSIS_REFERENCE.md`
   — current, reconciled 2026-08-04; treat claims there as accurate today.

## Binding measurement disciplines (violating these voided an entire
campaign once — they are rules, not suggestions)

- Quality verdicts come from pointwise instruments (`column_deviations` /
  the scallop oracle), never cell-count or area aggregates. §A.0: area is
  not a safe invariant.
- Gouge/deviation is measured SURFACE-NORMAL, never vertical.
- Render the surface and READ it before writing any verdict; a warning
  nobody sees is not a warning; an aggregate without a render has been
  wrong four separate times.
- Test populations are selected by geometry or intent-at-source, never by
  label (three gate corrections on one feature came from label selection).
- Two-fixture minimum for any strategy comparison — one fixture gave a
  confident wrong answer in both directions during H4.
- Convex/smooth fixtures cannot adjudicate concave-feature defects (P12).
- Pre-register tolerances before running comparisons.
- Never clear collisions across mismatched sim resolutions; tip-matched is
  authoritative.
- A negative result is re-earned, not assumed; a re-measurement that can't
  run gets an honest NOT RE-RUN row with reasons.

## Research areas, in the order I judge them by expected defect density

### R1. The feeds/Suggest subsystem (highest priority — census first)

Symptoms accumulated across the last programme, all fixed at the surface
only: four chipload numbers for one operation that do not agree
(narration-nominal 0.0714 / clamped 0.0044→0.0250 / gate-observed 0.0007 /
band 0.00458–0.00916 — ledger B3); a tapered-ball width formula whose
growth term was dead code for its entire life with a unit test that
exercised an impossible binding; vendor_lut vs vendor_lut_extrapolated
gating semantics verified only at the disclosure level; Suggest-vs-gate
divergence that C3 closed for width but nobody has audited for RPM, power,
or deflection. Research questions: enumerate every number the feeds stack
produces for "what load is this tool under" (an H1-style census with file:
line provenance — count the implementations); which are derived from which;
where LUT rows are matched, clamped, extrapolated, and rejected (note the
standing preference: hardness should dial parameters, not hard-reject
rows); whether the post-sim gates and pre-sim Suggest share every physical
model or only chipload width; what the DOC-derating mirror
(`feeds::geometry`) covers and misses. Deliverable: FEEDS_CENSUS.md
mirroring TOOL_SCALE_SEMANTICS.md, a reconciliation table for B3's four
numbers, and a fix sequence with a human checkpoint before any number an
operator acts on moves. Note the operator has ruled once already: sim
chipload is the arbiter over static Suggest — the fix shape should make
that structural (one shared model, Suggest as its pre-sim projection), not
aspirational.

### R2. The 2D operation stack under adversarial fixtures

The 3D stack just survived a hostile fixture campaign; the 2D stack
(pocket, adaptive, profile, trace, zigzag, inlay, v-carve, drill from 2D
targets) has never faced one. The single accidental probe — a 12-vertex
cross — hung pocket for 13 minutes at 386MB until the M5 cascade killed
that cause. Assume similar findings are waiting. Research questions: build
an adversarial 2D fixture family (comb/dendrite/near-degenerate edges/
holes-in-holes/disjoint islands/self-near-touching walls — extend
`tests/common/`, the M5 offset_lab generators are the seed); drive every 2D
op through it with wall-clock and memory ceilings plus boundary-fidelity
oracles (offset-ring vs analytic, the M5 pattern); catalogue hangs,
blowups, silent early terminations (the cavalier `Shape` panic still maps
to "collapsed offset" — ledger item, reproduce and fix shape it), and
tolerance violations. Also: the arc cascade rolled out to scallop and
pocket only — assess which remaining offset consumers (rest, boundary,
profile, trace, zigzag, inlay, project-curve) should adopt it, with the
same one-flatten-policy rule. Deliverable: a findings table with
reproductions, ranked by GUI-reachability, and per-op fix items.

### R3. The three permanent adaptive3d reds — end their tenure

`peck_plunge_progresses_when_depth_per_pass_equals_retract_clearance`,
`rapid_segment_lifts_to_safe_z_before_traverse`,
`planner_sim_dexel_parity_agent_search` have been "known reds, not yours"
for every gate run of the entire programme. That state is corrosive: every
agent must be told to ignore them, and a fourth red hides behind them.
Research questions: for each — what does it assert, when did it go red
(bisect if history allows), is the assertion still the intended contract,
is the defect in the test or the code? Deliverable: per-test verdict
(fix-code / fix-test / retire-with-reason) with a reproduction analysis.
This is small but should run FIRST in any execution — it cleans the gate
baseline every later wave inherits.

### R4. Simulation emission and issue-channel hygiene

Today's fully-green wanaka run still logs ~99,000 "issues" — the emission
noise CLAUDE.md warned about years ago is intact, and it is the channel
class proven to bury real signals. Research questions: what produces
issue_count entries; which are diagnostic vs noise-by-construction (every
out-of-material sample); can the channel adopt the findings-contract shape
(typed, deduplicated, severity-carrying) the generation side now has;
what would page-one triage look like for an operator (hotspots + collisions
+ typed findings, count-bounded)? Also absorb: the instrument limit that
sim cell must be below CUT DEPTH (not just tip radius) for engagement
measurability — decide whether run_simulation should WARN when the
requested resolution cannot resolve the finest op's cut depth. Deliverable:
channel inventory + a findings-shaped redesign proposal (report-only
first, checkpoint before changing any gate consumer).

### R5. Drill subsystem re-validation

Drill got typed region roles (C4) but its physics thresholds (Janka-banded
depth-to-diameter, per-peck D/d, plunge-feed envelopes, chip-welding risk)
were calibrated before the instrument fixes and never re-examined. Research
questions: do drill_summaries' numbers survive contact with the fixed
measurement stack; are the literature-matrix sources for the thresholds
still the ones cited (run /refresh-lit-matrix discipline over them); does
the drill gate suite have the same self-contradiction pattern the chipload
gate had (verdict vs evidence)? Deliverable: a gate-vs-evidence audit table
and any needed re-calibration items, each behind a checkpoint.

### R6. Fixture quality — the campaign blocker (infrastructural)

Two quality campaigns (v3 closure, H4's terrain columns) died on the same
fixture: terrain.stl is a coarse TIN where 1.8% of triangles carry 40.8% of
the area, so every arm misses fine dials by ~3× in unstructured speckle.
Research questions: what does an honest reference part look like (analytic
surface tessellated to a stated chord tolerance, mixed slope classes,
concave features at tip scale, known ground truth); can it be generated
procedurally into `tests/common/` rather than stored as a blob; what gate
bin does repeatability actually support at 0.1mm cells (measure, don't
assume — the ±10µm bin was below repeatability and aliased). Deliverable:
fixture spec + generation code plan + a re-opening plan for ledger B1/B2
(the v3 question) gated on it. Pair with the P7 decision: the three
#[ignore]d mega-harnesses (`v3_cascade_ab`, `p2c_headless_ab_wanaka`,
`strategy_comparison_h4`) need a policy — split reusable loaders, archive
campaign prose as dated docs, or scheduled characterisation cadence.

### R7. Hygiene wave: B7 + arcfit (two recurring defect factories)

Not research-heavy — both mechanisms are fully diagnosed — but they belong
in the plan as an early execution wave: (a) B7, the GUI worker's
field-by-field copy of GenerationFindings onto viz ToolpathStats, which has
silently dropped new side-data FIVE times; fix shape: one shared findings
pipeline (the C1/C3 one-implementation rule applied to the worker
boundary); enumerate every hand-copied field first. (b) The arcfit
intent-inheritance defect (arcfit.rs:194 — runs grouped by feed, intent
taken from first source move, lead geometry relabelled as cutting):
fix = break arc runs at intent changes; this moves fitted geometry
repo-wide, so it needs the full fingerprint re-pin discipline with
before/after justification per pin, and it should land BEFORE any new
intent-keyed measurement work builds on the corrupted labels. Also fold in:
D-LV.1 (screenshot_toolpath exporter misclassifies new-emission moves —
scoped exporter-only, viewport fine), D-16.2 (shallow band ignores
stock_to_leave), D-16.1 (band run-off overcut at stock footprint), and the
narrate/parameterized-read holes A/M12 left open (get_toolpath_params still
queues behind the frame loop; narrate_toolpath is a ~12min GUI-thread
grind — research an off-thread or incremental path).

### R8. Untouched territory sweep (bounded scouting only)

Import/mesh layer (STL/STEP/BREP, SVG/DXF), project IO round-tripping
(the emit-side dual-key wire from the A6 rename now has a documented trap —
audit for other Serialize-only wires readers might round-trip), export/
post-processing, and GUI state consistency (the recurring "core default vs
serde default" divergence class — grep for Default impls that shadow serde
defaults; wave 14 fixed one and pinned its agreement, find the rest).
Deliverable: a one-page risk map per area — enough to rank, not to fix.
Do NOT deep-dive these; flag and size only.

## What your plan must contain

Mirror the template: per-item sections with the six headings; a priority
tier (H/M/L) with an explicit merge order; named human checkpoints for
anything that moves machining output, an operator-actionable number, or a
default (P13: a deferral names its decider — the operator decides those);
work-package lanes with parallel/no-overlap rules; and a non-negotiable
rules section carrying forward: wanaka.toml is a read-only play-file
(its `wanaka_suggest_baseline` red is permanent and environmental); one
cargo job machine-wide (check `free -g` + `pgrep -af "carg[o]"` — note the
bracket, pgrep self-matches); no workspace-wide `cargo test`; zero-warning
clippy; test-code lint exemptions per repo pattern; red-first sentries with
the red captured in the commit body; fingerprint re-pins carry before/after
+ mechanism; `adaptive3d_interior_cell_parity_f029` takes ~10min (slow not
hung); read `ps -L` not `ps` when judging rayon liveness; no --release
builds inside waves (release rebuild is live-validation prep only); commit
instruments the moment they lint clean; agents append their own
ORCHESTRATION_LOG entries; and end-of-programme live validation over the
MCP (per-behavioral-batch validation was waived by the operator last
programme in favour of one final pass — propose the same unless an item is
high-risk enough to justify its own).

Research is REPORT-ONLY: prototypes behind test hooks and strategy enums,
production untouched until a checkpoint rules. Where your research finds
the premise of an item is wrong (it happened three times last programme —
min-across-ring, the relinker, the serde alias), say so and re-scope in
the plan rather than executing a wrong brief faithfully.

Write the plan to `planning/review_2026-08-04/TECH_DEBT_2_RESEARCH_AND_FIX_PLAN.md`.
