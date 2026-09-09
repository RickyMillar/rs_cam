# R06 — Interpreting simulation and closing the correction loop

Read `../PROTOCOL.md`, `../TOOLING.md`, `../FIXTURES.md`. Execute only R06.
Write `../results/R06/{REPORT,trace}.md` and evidence. Source paths are repo-relative.

## Outcome

The user can answer “What matters, where is it, how sure are we, what should I
change, and did that change help?” A simulation run or a green screenshot alone
is not success. Read the protocol's diagnostic-honesty section before analysis.

## Task cards

1. **First verification:** start with a generated F1 or F3 job. Find simulation,
   decide what to capture and choose/understand resolution. Predict what will
   be checked and how long feedback might take. Include the no-toolpaths and
   not-yet-simulated states rather than preparing results through MCP first.
2. **Find the priority:** use verified V6 scratch results containing a real
   safety/load concern plus noncritical inefficiency. Ask the user what they
   would address first, why, and what they believe a clean count establishes.
   Record visible evidence before revealing backend triage.
3. **Locate it:** move from project summary to operation, region/span and the
   relevant motion. Exercise list/HUD/timeline/hotspot/viewport routes. Distinguish
   selected versus playing versus pinned scope. Is the same problem identifiable
   without knowing a move index? Is a missing overlay explained?
4. **Fix and verify:** follow an offered fix or navigate to the responsible
   toolpath/setup control. Change one thing, observe stale evidence, regenerate
   and rerun. Return to the affected location and compare before/after evidence.
   Do not stop at “Optimize” or a parameter successfully changing.
5. **Unknown is not clean:** V7. Compare absent results, metrics capture off,
   not-measurable shallow detail, a no-data load criterion and drill-native
   results. Ask what has been checked in each. Use actual observed populations
   and reasons; do not infer them from the seed's name.
6. **Efficiency versus finish:** inspect absolute air time, runtime basis,
   engagement and stock/deviation view. Ask what evidence would justify a faster
   or finer strategy. Check planned/emitted feeds and prevent reach-map or
   smoothed-colour readings from masquerading as delivered metrology.
7. **Scope:** simulate only a subset in a multi-op/multi-setup scratch job. Can
   the user tell how much of the job each summary covers and what remains untested?

## Specific questions

- Does the normal hierarchy lead with consequential issues and actionable
  guidance, rather than sample counts or unfamiliar scalar metrics?
- Is every important claim qualified by scope, freshness and measurement limits
  at the decision point? Are essential limits hidden only in hover text?
- Does a warning name an appropriate control, or require translating physics
  into an unrelated field name in another workspace?
- Can playback and diagnostic selection coexist without losing the problem?
- Are false confidence and needless alarm both avoided? Can the user explain
  why zero/not applicable/unmodeled/stale are different?

## Source anchors

- `crates/rs_cam_viz/src/ui/sim_op_list.rs:17-200`: run/capture/resolution/staleness;
  later sections include operation scope and kinematics/drill pills.
- `crates/rs_cam_viz/src/ui/sim_diagnostics.rs`: project/focused cards, status,
  NOT MEASURED strip, raw issue paths and scoped details. Trace actual rendering
  against the bounded core triage; do not assume the whole panel is identical
  merely because it consumes triage for one section.
- `crates/rs_cam_viz/src/ui/sim_timeline.rs`; `src/app/viewport.rs`.
- `crates/rs_cam_viz/src/state/simulation.rs`; core `simulation_triage` and
  `sim_measurability` implementations and relevant sentries.

## Deliverable additions and boundary

Build an **evidence → interpretation → location → corrective control → refreshed
proof** map. Include a user's teach-back of one warning and one unknown/clean
state. Record a complete correction loop with before/after images and state.

R09 owns detailed visual/interaction consistency; R04 owns optimizer proposals;
R07 owns final readiness claims. No tuning of physical models or thresholds,
no inference that passing modeled checks certifies the real cut.
