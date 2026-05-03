# Optimizer UX — unified speeds/feeds/geometry tuning

## Status

**U1, U2, U3, U4 shipped 2026-05-03.** The full optimizer UX
end-to-end is in place: per-toolpath modal, project rollup with
bottleneck callout, batch Apply, post-Apply reconciliation. The
worker-thread integration uses a third compute lane that takes
ownership of the session for the duration of the run; the GUI
renders an "Optimize running" placeholder during. Remaining
backlog: end-to-end smoke test (`crates/rs_cam_core/tests/optimize_smoke.rs`),
MCP migration of `suggest_feeds_speeds` -> `optimize_toolpath`,
and final retirement of `tool_load/suggest.rs` (only RefuseReason +
explanation_for_optimize need preserving).

Drafted 2026-05-03 after a live MCP run on `wanaka_full.toml`
exposed three failure modes in the per-toolpath Suggest flow. Replaces
the never-shipped "Tier 2 deep suggest" sketch with a single unified
verb spanning feed/RPM and geometry knobs.

Reviewed and updated 2026-05-03 (same day). Three big design calls
baked in since the first draft (full details in the **Resolutions**
section near the bottom):

1. **Full sim per candidate, no synthetic gate.** The simulator is the
   single source of truth for both predicted and measured values.
   Optimize is allowed to be slow if that's what gets the best data.
2. **RPM/feed proportional scaling is Stage 0** of the search. At
   constant chipload, RPM and feed scale linearly until a machine,
   tool, or LUT-row limit binds. Closed-form, instant, no sim needed.
   This is the headline "optimise for speed" win.
3. **Standalone "fix suggest" phase dropped.** Routing was already
   unified between suggest and the gate; the actual divergence
   (theoretical vs measured chipload) only resolves once candidates
   are simmed — which the new U1 does. The dedup of suggest's old
   logic is folded into U1, not a phase of its own.

## Why now

The tool-load suggest module shipped in late April (commits `5aa016e`
through `5157f7f`) with a per-toolpath **Suggest** button that recommends
feed + RPM only. A live wanaka run (53-minute job, 7 toolpaths,
documented in `memory/feedback_perf_optimization.md` adjacent notes)
exposed three concrete failure modes that no amount of polish on the
existing modal can fix:

1. **Suggest disagrees with the gate.** Recommended 1841 mm/min for
   project_curve toolpaths; the diagnostics gate then flagged
   BreakageRisk. User had to manually pull the value back. Two sources
   of truth, one source of pain.
2. **Per-toolpath optimization misses the bottleneck.** User dialled in
   TP 3 and TP 4 (small project_curves), spent 15 minutes, ~4% impact.
   TP 6 (3D finish) dominates at 61% of cycle time and was the only one
   that mattered. No view ever showed that.
3. **Trade-offs are invisible until you re-simulate.** Pulled TP 6 from
   1899→1500 mm/min for safety, total runtime went *up* (other TPs no
   longer the limiter), had to revert. Each guess is a 2-minute sim.

Two buttons (Suggest + Deep-suggest) would not fix any of these. They
add a second flavor of "didn't help" rather than addressing the
information architecture problem.

## User journeys

Wanaka-grounded. Each one drives a specific design requirement.

### J1 — "I just imported, give me a starting point"

Hobbyist, fresh `wanaka_full.toml` import, picked tools, defined stock.
Sees 7 toolpaths with default feeds. Has no anchor for whether 1899
mm/min is right.

- *Today:* Click Suggest on TP 0 → modal with two numbers, no context →
  Apply, regen, sim. Repeat 7 times. ~20 minutes of clicking. No sense
  of whether the project is "tuned" or just "individually OK".
- *Optimize:* One click, watch a progress bar for ~5–15 min while the
  simulator chews through candidates. Final modal: *Baseline 53:02 →
  optimized 41:18 (-22%, measured). 5 toolpaths within safe envelope.
  2 skipped (drill, no chipload model). Apply all?*

**→ Optimize must be the default first-time action, not a power-user mode.**
**→ It's an "open and walk away" job, not interactive. Progress + cancel.**

### J2 — "Make this 53-minute job faster"

The exact run captured this morning.

- *Today:* Hit Suggest on TPs 0/3/4/5/6 → 5 modals → Apply. Suggest
  pushed TP 3, 4 BurnRisk → BreakageRisk; gate disagreed; manual
  pullback. Tried same trick on TP 6, total runtime increased, reverted.
  Landed at 49:36 (~6% better) with two findings filed because the loop
  was confusing.
- *Optimize:* One click. Rollup: *Bottleneck: TP 6 "3D Finish 6" — 61%
  of cycle time. ⭐ stepover 0.5→0.8, feed 1899→2100, -7:03 (measured).*
  Plus refusals inline ("TP 3 Lakes back: no safe improvement found,
  gate-limited at chipload 0.0072") instead of a misleading recommendation.

**→ Show the bottleneck explicitly. One verdict source. Refusal as a
first-class outcome with a reason.**

### J3 — "TP 0 cut nothing, what's wrong?"

Diagnostic problem, not a speed problem. Back Rough generated 7 moves
and 0mm cutting. Why?

- *Today:* Open diagnostics. No collisions, low engagement, low MRR.
  Doesn't say "the toolpath is empty". User screenshots the toolpath,
  sees 12 dots, infers the agent search found no removable material.
  15 minutes of guessing. The `stock_to_leave_axial` × `depth_per_pass`
  interaction is non-obvious.
- *Optimize:* Click Optimize on TP 0. Modal: *Current toolpath has 0mm
  cutting distance — agent search found no removable material above
  stock_to_leave_axial=3.0mm. The flat back has only ~1mm of stock above
  the floor. Reachable improvement: lower stock_to_leave_axial to 0.5,
  depth_per_pass to 3.0 → 11,225mm cut, 6:20 simulated.*

**→ Optimize is a diagnosis tool too. Recommendations carry a narrative
when the win is geometry, not feed.**

### J4 — "Did optimization actually help, or break something?"

Trust mechanism. With full sim per candidate, every number Optimize
shows already came from the simulator — predicted *is* measured at the
candidate level. The remaining uncertainty is at the project level:
applied params can interact across toolpaths (e.g. upstream stock state
changes downstream engagement).

- *Today (naive Tier 2):* Apply → regenerate → sim 2 minutes → click
  through 7 TPs in diagnostics → compare to mental model. No diff, no
  reconciliation.
- *Optimize:* Per-row verdicts and times on the rollup are sim-backed
  already. After Apply, a project-level reconciliation sim runs end to
  end with the new params. The rollup updates any TP whose end-to-end
  verdict disagrees with its candidate-isolated sim — flagged with a
  delta. Revert just that row.

**→ Per-candidate verdicts come from the same simulator that runs
diagnostics — no second model anywhere. Project-level reconciliation
catches the cross-TP interactions.**

## Design requirements

| Requirement | Driven by |
|---|---|
| Project-level rollup is the primary surface | J1, J2 |
| Bottleneck callout (% of cycle time per TP) | J2 |
| One verdict source (every number is a sim measurement, no synthetic gate) | J2, J4 |
| Refusals carry a typed reason and a fix hint | J2, J3 |
| Project-level reconciliation after Apply (catches cross-TP interactions) | J4 |
| Optimize covers geometry (stepover/DOC) when the op exposes them | J3 |
| Long-running with progress + cancel — "open and walk away" | J1 |
| Skipped TPs explained inline (drill, no model) | J1 |

Two surfaces, not five:
1. **Project Optimize rollup** — toolbar entry point, where 80% of
   workflows live.
2. **Per-toolpath detail** — drill-down for "show me alternatives" or
   "I want to override the winner". Reachable from any rollup row.

## The unified UX

### One verb: Optimize

Replaces today's per-toolpath "Suggest". Searches across whatever knobs
the op exposes (feed/RPM always; stepover/DOC for the 5 ops where it
matters: Adaptive3d, Pocket, Adaptive, Rest, Face). Every candidate
runs through **the project simulator** with the same gate that
diagnostics already uses. Ranks by measured cycle time. The current
feed/RPM-only path becomes the cheapest candidate, not a separate button.

### Search strategy: simulator is the only oracle

All candidate evaluation goes through the project simulator. No second
chipload model, no fast-and-loose verdict shortcut. **Three** stages,
ordered cheapest-first:

#### Stage 0 — analytical RPM/feed scaling (instant, no sim)

The biggest "optimise for speed" win is often dialling RPM and feed up
together while holding chipload constant. At constant chipload:

- MRR scales linearly with RPM.
- Power scales linearly with RPM (P = Kc × MRR / 60e6).
- Per-sample chipload from `effective_chip_thickness_mm` is **invariant**
  under proportional (RPM, feed) scaling — same toolpath geometry, same
  arc engagement, just faster.

So the optimiser's first move is closed-form: pick the largest scale
factor `k` such that all four limits hold:

```
k × rpm_baseline   ≤ machine.spindle_max_rpm  AND  tool.max_rpm
k × feed_baseline  ≤ machine.max_feed_mm_min
k × power_baseline ≤ machine.max_power_w × safety_factor
k × rpm_baseline   ≤ rpm band of the LUT row (no extrapolation)
```

Solution: `k = min(four ratios)`. No sim needed — the existing baseline
trace is re-evaluated analytically for power scaling; chipload is
RPM-invariant by construction. The "headroom point" `(k × rpm_baseline,
k × feed_baseline)` becomes the anchor for Stages 1 and 2.

For wanaka specifically, the machine is at 18000 RPM max and the LUT
rows are calibrated at 18000 — `k = 1`, no headroom. But for hobbyists
with higher-RPM spindles, or LUT rows calibrated below their machine
max, this is the dominant easy win and it costs nothing to compute.

**Caveat — baseline already unsafe.** Stage 0 only helps when the
baseline verdict is `Within` (or `Approximate`). Proportional scaling
preserves per-sample chipload, so it cannot fix an `Exceeds` baseline
— that needs feed *reduction* (chipload drops; risks dropping below
LUT cl_min into burn territory) or geometry change. For `Exceeds`
baselines, skip Stage 0 and go straight to Stage 1 with feed reduction
included as a search axis alongside DOC. (Practical example: yesterday's
TP 6 finished at chipload 0.0528 → BreakageRisk; Stage 0 wouldn't move
that. Stage 1 needs feed and stepover both as axes for that case.)

#### Stage 1 — coarse sim of geometry candidates (1mm dexel)

Anchored at the headroom point from Stage 0, vary the geometry knobs
(stepover, DOC) for the 5 ops where they matter (Adaptive3d, Pocket,
Adaptive, Rest, Face). For ops with no geometry knobs (drill,
project_curve, finish), Stage 1 is skipped — the headroom point is the
recommendation.

Sim each candidate at 1mm dexel resolution. Calibrated 2026-05-03 on
wanaka: 0/8 verdict-kind flips between 0.5mm and 1.0mm; peak chipload
values can drift up to ~50% on individual TPs but verdict *kinds* are
stable. Always quote Stage-2 numbers on the rollup.

#### Stage 2 — full-resolution sim of survivors (0.5mm dexel)

Top 3 by Stage-1 cycle time, re-simmed at default resolution. The
reported verdict and cycle time on the rollup are always Stage 2
numbers.

Both Stage 1 and Stage 2 run the existing simulator with the existing
gate. The "unified system" property holds — same code, dialled between
resolutions.

#### Search budget per op

| Op family | Stage-0 cost | Stage-1 candidates | Stage-1 budget | Stage-2 (top 3) |
|---|---|---|---|---|
| Drill, project_curve, V-carve, scallop | analytical | 0 (Stage 0 wins) | 0 | 0 |
| Pocket / Adaptive | analytical | 4 × DOC variants | ~4s | ~12s |
| Adaptive3d / Rest | analytical | 3 × DOC variants | ~10s | ~30s |
| Face | analytical | 3 × DOC variants | ~3s | ~10s |

(Stage-1 grids collapsed from 2D to 1D — the feed/RPM axis is now
analytical, not sweeped. DOC is the remaining geometry axis; stepover
varies along with DOC at constant chipload-via-stepover.)

Wanaka-realistic project budget: 5 ops × Stage 0 + 3 geometry ops ×
~40s sims + project-level reconciliation sim ≈ **3–5 minutes** (down
from the 5–8 min in the pre-Stage-0 plan). The user is told this up
front and can cancel.

### Project Optimize (toolbar)

```
┌─ Optimize project ─────────────────────────────────────┐
│  Current  53:02      Optimized  41:18  (-22%, measured)│
│                                                        │
│  Bottleneck: TP 6 "3D Finish 6"  (61% of runtime)      │
│  ────────────────────────────────────────────────────  │
│  ☑ TP 0  Back Rough     +stepover 3.0→4.5  -2:47       │
│  ⚠ TP 3  Lakes back     no safe improvement found      │
│           (gate-limited at chipload 0.0072)            │
│  ⚠ TP 4  Rivers back    no safe improvement found      │
│  ☑ TP 5  3D Rough 6     +stepover 3.0→4.5  -1:54       │
│  ☑ TP 6  3D Finish 6    +stepover 0.5→0.8  -7:03  ⭐   │
│                                                        │
│  TP 1, 2  drill / project_curve (no model — skipped)   │
│                                                        │
│  [Apply selected]   [Per-toolpath details]  [Cancel]   │
└────────────────────────────────────────────────────────┘
```

After Apply, the modal stays open while a project-level reconciliation
sim runs end-to-end with the new params. Per-row verdicts may shift
(cross-TP interactions); the rollup flags any disagreement with a
delta against the candidate-isolated number.

### Per-toolpath detail

Replaces today's Suggest modal. Shows the current params row pinned at
top, the candidate table below, sorted by measured cycle time, marked
with verdict badges (same source as diagnostics — same simulator).
Hover a row → the viewport ghost-renders that candidate's toolpath
using the existing toolpath render pipeline (regenerated geometry is
already cached from the sim pass).

```
┌─ Optimize TP 6 "3D Finish 6" ──────────────────────────┐
│ Current   feed 1899  rpm 18000  stepover 0.5  DOC 2.0  │
│           cycle 18:42      verdict ✓                   │
│                                                        │
│ Candidates (ranked by measured cycle time)             │
│  ⭐ feed 2100  rpm 18000  stepover 0.8  DOC 2.0        │
│      11:39  -7:03          ✓ within bounds             │
│     feed 2100  rpm 18000  stepover 1.0  DOC 2.0        │
│      9:21   -9:21          ⚠ chipload high             │
│     feed 1841  rpm 18000  stepover 0.5  DOC 3.0        │
│     14:55   -3:47          ✓ within bounds             │
│                                                        │
│ Rationale                                              │
│ • Wider stepover wins on cycle time: shorter total     │
│   path. Feed up to chipload ceiling (0.0511 mm/tooth   │
│   at 0.8 stepover, 0.0635 envelope max).               │
│ • Power cap not binding (0.6 kW @ 18000 RPM).          │
│ • Generated 9 candidates, simmed all 9, 7 passed gate. │
│                                                        │
│ [Apply ⭐]  [Apply selected]  [Cancel]                 │
└────────────────────────────────────────────────────────┘
```

## Data shapes

```rust
// New in tool_load::optimize
pub struct OptimizeCandidate {
    pub params: OperationConfig,           // full config to apply
    pub delta: ParamDelta,                 // human-readable diff vs current
    pub cycle_time_s: f64,                 // measured directly from sim of this candidate
    pub verdict: ToolpathLoadVerdict,      // from the existing gate over this candidate's sim
    pub stage: SearchStage,                // Coarse (1mm dexel) or Refined (default)
    /// Project-level reconciliation: after Apply + end-to-end sim, the
    /// candidate's verdict may differ because upstream toolpath state
    /// shifted. Populated post-Apply only.
    pub reconciled_cycle_time_s: Option<f64>,
    pub reconciled_verdict: Option<ToolpathLoadVerdict>,
}

pub enum SearchStage { Coarse, Refined }

pub enum OptimizeOutcome {
    Ranked(Vec<OptimizeCandidate>),        // current is index 0; recommended is .first_safe()
    NoSafeImprovement {
        reason: RefuseReason,              // reuse existing typed reasons
        explanation: String,               // narrative for the modal
    },
    Skipped {                              // drill, project_curve w/o samples, etc.
        reason: RefuseReason,
    },
}

pub struct ProjectOptimizeReport {
    pub baseline_cycle_time_s: f64,
    pub bottleneck_index: Option<usize>,   // TP that dominates runtime
    pub per_toolpath: Vec<OptimizeOutcome>,
}

pub fn optimize_toolpath(
    session: &ProjectSession,
    baseline_trace: &SimulationCutTrace,   // the sim already on screen
    toolpath_index: usize,
    cancel: &CancelToken,
) -> OptimizeOutcome;

pub fn optimize_project(
    session: &ProjectSession,
    baseline_trace: &SimulationCutTrace,
    progress: &mut ProgressSink,
    cancel: &CancelToken,
) -> ProjectOptimizeReport;
```

`baseline_trace` comes from the project sim already on screen — Optimize
is gated on having one (matches J4: "could require a sim, then run more
sims to optimise"). Each candidate is regenerated via `execute_operation`,
then simulated by the existing simulator pipeline at the requested
resolution. The verdict comes from `tool_load::evaluate(...)` — the
single function diagnostics already uses.

## Implementation phases

After each phase: `cargo build -p rs_cam_core && cargo test -p rs_cam_core`
plus manual MCP smoke test on wanaka.

**Status as of 2026-05-03:** U1, U2, U3, U4 all shipped. See commits
`fc15d40..952178d` for the build-up.

| Phase | What | Why first | Files | LOC |
|---|---|---|---|---|
| **U1 — `optimize_toolpath` service** | Function: takes session + baseline trace + toolpath idx + cancel, returns `OptimizeOutcome`. **Stage 0** (analytical, instant): solve max RPM/feed scale factor `k` from machine + tool + LUT-row limits; produce the headroom point. **Stage 1** (1mm sim): for the 5 geometry ops, vary DOC per Engineering Default 9 anchored at the headroom point; for others, skip. **Stage 2** (0.5mm sim): top 3 by cycle time at full res. Cancellation between stages and between candidates. As part of this work, delete suggest's now-redundant chipload bipolar/peak logic — the gate's verdict over each candidate's sim is the single source of truth. **Restore the baseline params on every exit path** (Ok, NoSafeImprovement, Skipped, cancelled, panicked candidate) so `optimize_toolpath` is a pure read of the search; the GUI keeps showing the original `OperationConfig` until the user explicitly Applies. See Engineering Default 10. | Engine for everything below. Stage 0 alone delivers the headline "scale RPM and feed up to machine limits" win at zero sim cost. | New `crates/rs_cam_core/src/tool_load/optimize.rs`; trim `tool_load/suggest.rs` (or retire it) | ~800 |
| **U2 — Per-TP modal** | Replace today's `crates/rs_cam_viz/src/ui/suggest_modal.rs` (the actual file — modal trigger lives in operations panel) with the ranked candidates table. Current row pinned. Apply routes through the extended `apply_toolpath_param_snapshot(idx, op, dressups, face_sel, feeds_auto)` mutation (Engineering Default 7) — one transactional write that clears the right `feeds_auto.*` flags per Resolution 7 and preserves the `spindle_rpm: Option<u32>` override. Reroute the existing `AppEvent::ApplySuggestedFeed` handler in `controller/events/mod.rs:171` through the same path so the silent-LUT-overwrite bug it has today closes as a side-effect. | Ships incremental win even before project view exists; gives U3's rollup a working drill-down to compose. Closes existing Suggest's silent-overwrite bug as a side-effect. | viz: rename/replace `suggest_modal.rs` → `optimize_modal.rs`; extend `apply_toolpath_param_snapshot` in `session/mutation.rs:724`; reroute `ApplySuggestedFeed` handler | ~400 |
| **U3 — Project Optimize** | Toolbar action `Optimize project`. Calls `optimize_project` (parallel `optimize_toolpath` over independent TPs via rayon — careful: regen is parallel-safe, simulator is sequential per candidate). Rollup view with bottleneck callout. Progress bar + cancel button (estimated minutes shown up front). Per-row checkboxes default to "all safe candidates". Batch apply uses U2's mutation. | The view that actually changes how the user works. Bottleneck callout is the J2 fix. | new `crates/rs_cam_viz/src/ui/optimize_project.rs` + toolbar wiring in `app.rs`; progress sink in controller | ~500 |
| **U4 — Project-level reconciliation** | After Apply on rollup, kick a project end-to-end sim in the background. As it completes, populate `reconciled_cycle_time_s` / `reconciled_verdict` per row. Visual: candidate-isolated column dims, reconciled column populates, mismatch flagged with delta. Per-row revert button. | Cross-TP interactions catch (J4). The candidate-isolated sim doesn't see how upstream params changed downstream stock state. | optimize rollup view + simulation completion hook | ~250 |

Total: ~1900 LOC. **U1 + U2 alone (~1150 LOC) ships a coherent
per-toolpath improvement and is worth landing first.** U3 + U4 add the
project view; both are required to deliver the full vision.

### Phase ordering rationale

- **U1 first** because it's the engine — both per-TP (U2) and project
  rollup (U3) call it. Stage 0 alone is a real user win: scale RPM and
  feed analytically up to machine limits at zero sim cost. The dedup
  of suggest's old logic happens here too — the gate over each
  candidate's sim becomes the single verdict source, replacing
  suggest's separate (and previously divergent) chipload routing.
- **U2 before U3** because U2 is small and gives us a working modal
  pattern to compose into U3. The per-TP detail is *also* the
  drill-down from U3's rollup.
- **U4 last** because it requires Apply + regen + sim to already be
  wired into the rollup. Rushing it ahead of U3 would mean reconciling
  predictions against an Apply path that doesn't exist yet.

## Resolutions (2026-05-03 review)

Decisions baked into the phases above. Captured here so the rationale
isn't lost when the table gets edited.

1. **Verdict source: full project simulator on every candidate, no
   synthetic gate.** User: "I'm happy with running sims for this. It
   could require a sim, then run more sims to optimise. Whatever gets
   the best data." Two-stage search (1mm coarse → 0.5mm refined on
   top-N) keeps total cost in the 5–15 min range for a wanaka-sized
   project. The simulator stays the single source of truth.

2. **Optimize requires a baseline sim.** The toolbar button is disabled
   (or auto-runs sim first) when `simulation.results` is empty. Matches
   the user's mental model — you run sim, see what's wrong, then ask
   Optimize to fix it.

3. **Cycle time is measured, not estimated.** Removes the predicted-
   cycle-time formula concern entirely. Every number on the rollup
   came from a simulator pass.

4. **Cross-TP reconciliation is the only "predicted vs measured"
   gap.** A candidate is simmed in isolation — upstream toolpath state
   from the baseline sim. After Apply, the whole project re-sims end
   to end; downstream TPs may shift because their stock state now
   reflects the *new* upstream params. U4 catches this.

5. **Modal file path.** The Suggest modal is in
   `crates/rs_cam_viz/src/ui/suggest_modal.rs`, not
   `ui/properties/operations/mod.rs`. The operations panel has the
   trigger button; the modal is a sibling.

6. **Batch-apply API needed.** `set_toolpath_param` is one-param-at-a-
   time. U2 introduces `session.set_toolpath_params(idx,
   OperationConfig)` (or a transactional `apply_optimize_candidate`
   helper) so the rollup's "Apply selected" doesn't fire 4×N
   mutations and 4×N stale-flagging cascades.

7. **Override flag preservation — `spindle_rpm` AND `feeds_auto.*`.**
   Two parallel override mechanisms cover the four params Optimize can
   change:
   - `spindle_rpm: Option<u32>` (None = auto, Some = manual override).
     If Optimize wants to change RPM, write `Some(new_value)` (not
     `None`). If Optimize is *not* changing RPM, leave the existing
     override alone.
   - `feeds_auto: FeedsAutoMode` (struct of bools on `ToolpathConfig`,
     `session/mod.rs:379`; default all-true). When the user is on the
     toolpath properties panel's Feeds tab,
     `calculate_and_apply_feeds` (`crates/rs_cam_viz/src/ui/properties/mod.rs:957`)
     overwrites `entry.operation.{feed_rate, stepover, depth_per_pass,
     plunge_rate}` from the LUT every frame for any flag still `true`.
     The frame then syncs `entry → session`
     (`properties/mod.rs:400, 2080`), so the LUT default lands back on
     the session config.

   Apply must therefore clear the relevant `feeds_auto.*` flag for
   every field Optimize is changing — otherwise candidate values get
   silently overwritten by the LUT calculator both *during* evaluation
   (if the user has that Feeds tab open) and *after* Apply (next time
   the user visits the tab). Mapping:

   | Optimize changes | Must also flip to false |
   |---|---|
   | `feed_rate` | `feeds_auto.feed_rate` |
   | `stepover` | `feeds_auto.stepover` |
   | `depth_per_pass` | `feeds_auto.depth_per_pass` |
   | `spindle_rpm: Some(_)` | `feeds_auto.spindle_speed` |

   Side-effect: the existing per-toolpath Suggest (which calls
   `set_toolpath_param("feed_rate", ...)` / `("spindle_rpm", ...)` and
   never flips `feeds_auto`) has the same bug today — Apply lands the
   value, the LUT auto-write quietly undoes it. Routing both Suggest
   and Optimize Apply through the extended snapshot API in
   Engineering Default 7 closes both at once.

8. **Standalone "fix suggest" phase dropped.** The original draft had
   a separate U1 phase to "make suggest call the same gate" before any
   UX work, sized at ~150 LOC. The 2026-05-03 surface check found:
   - Routing is *already* unified — `suggest.rs:34` imports
     `routed_lookup_family` and `tool_family_for` from `chipload`.
   - Envelope bounds (cl_min/cl_max from the LUT row) are already
     shared.
   - The actual divergence is **theoretical vs measured** chipload:
     suggest predicts steady-slot `feed/(rpm × flutes)`; the gate
     measures per-sample `effective_chip_thickness_mm` from sim,
     which spikes on transients. A standalone-suggest phase cannot
     fix this without simming candidates — which *is* U1 (was U2).
   - Standalone phase would be ~50 LOC of pure dedup and would not
     reduce user pain (the lying recommendations would still lie).
   - Decision: fold the dedup into the new U1. Once U1 sims each
     candidate, the gate verdict becomes the single source of truth
     and suggest's separate logic goes away with the modal in U2.

9. **RPM/feed proportional scaling — Stage 0 in U1.** "Optimise for
   speed" properly understood scales RPM and feed up together at
   constant chipload until *some* limit binds. Closed-form: `k =
   min(machine_max_rpm / rpm_baseline, machine_max_feed / feed_baseline,
   machine_max_power × safety / power_baseline, lut_row_max_rpm /
   rpm_baseline)`. Power scales linearly with RPM at constant chipload;
   per-sample chipload is RPM-invariant under proportional scaling
   (same toolpath geometry → same effective chip thickness if RPM and
   feed move together). No sim needed for Stage 0. For wanaka the
   machine is already at 18000 RPM max so `k = 1`, but for users with
   higher-RPM spindles or LUT rows calibrated below their machine
   max, this is the dominant easy win.

10. **Wanaka mockup numbers are illustrative.** The "-22%" in J1 and
    "-7:03 on TP 6" in J2 were sketched before U1 lands. Once Optimize
    agrees with the gate (every number is sim-measured), more wanaka
    TPs may refuse instead of recommending. Re-baseline against a live
    wanaka run after U1 to set realistic expectations for the rollup.

## Engineering defaults

Engineering details, all with committed defaults. The next agent can
revisit any of these if implementation evidence contradicts them, but
none block starting U1.

1. **Stage-1 dexel resolution: 1.0mm.** Calibrated 2026-05-03 on
   wanaka (8 TPs at 0.5mm vs 1.0mm): 0/8 verdict-kind flips. Peak
   chipload values can drift up to 49% on individual TPs but verdict
   *kinds* — BurnRisk / Within / BreakageRisk — are stable. Always
   quote Stage-2 numbers on the rollup (Stage-1 numerics may mislead
   even when verdicts agree).
2. **Stage-1 ranking: top-3 by cycle time, ignoring verdict.** Stage 2
   re-runs them at full resolution and applies the verdict that ships.
   A marginally-over-limit candidate at 1mm might still be the winner
   at 0.5mm; let Stage 2 decide.
3. **Cancel granularity: between candidates only.** Cancel takes effect
   at the next candidate boundary (~10–30s lag). Cooperative mid-sim
   cancel is out of scope for v1. The "open and walk away" job model
   tolerates this — user clicks Cancel and the next candidate boundary
   stops the run.
4. **Refusal narratives: `RefuseReason::explanation_for_optimize()`
   accessor.** Each refusal kind composes a one-line explanation at
   display time using the verdict's typed fields (peak chipload, the
   binding limit, etc.). Examples: `SteadyStateSamplesNotPresent` →
   "drill cycle / no steady-state samples to model"; `Exceeds(BurnRisk,
   peak)` → `"gate-limited at chipload {peak}"`.
5. **LUT row RPM tolerance: ±20% of calibrated RPM.** Stage 0's
   `k × rpm_baseline ≤ lut_row_max_rpm` constraint uses
   `rpm_calibrated × 1.2` as the upper bound (and ×0.8 as the lower)
   when the LUT row carries a single RPM. If a row later carries an
   explicit band, prefer the band.
6. **`Exceeds`-baseline TPs: skip Stage 0, run Stage 1 with feed
   *and* stepover (if available) as axes.** Proportional scaling
   preserves chipload, so it can't fix an `Exceeds` baseline. Search
   has to vary the params that change chipload.
7. **Extend `apply_toolpath_param_snapshot` with `feeds_auto` —
   single transactional Apply.** `session/mutation.rs:724` already has
   `apply_toolpath_param_snapshot(idx, op, dressups, face_selection)`
   that does one write + one stale-flag + one sim-invalidate. Add a
   `feeds_auto: FeedsAutoMode` parameter so Apply can clear the four
   override flags transactionally with the operation write. Existing
   callers (undo/redo) pass `tc.feeds_auto.clone()` to keep current
   behavior; Optimize passes a `feeds_auto` with the relevant flags
   flipped to `false` per the Resolution 7 table. Replaces the 4×N
   `set_toolpath_param` cascade Apply currently uses, and reroutes the
   existing Suggest's `ApplySuggestedFeed` handler through the same
   path so the feeds_auto silent-overwrite bug closes there too.

   `ToolpathConfig` is not `Clone` (`session/mod.rs:355`); the
   snapshot API takes only the changed fields, so no clone needed.

   `spindle_rpm` override: if Optimize is changing RPM, write
   `Some(new_value)`; if not, leave the existing `Option<u32>` alone.
   `feeds_auto.spindle_speed` flips to false in any case where Optimize
   wrote `Some(_)`.
8. **Skipped vs NoSafeImprovement vs Ranked outcome split.**
   - `Skipped`: drill cycles, project_curve with no steady-state
     samples — the gate can't model it, so neither can the optimiser.
     Reason carries the typed `RefuseReason`.
   - `NoSafeImprovement`: every candidate failed the gate, OR every
     candidate was slower than baseline. Surfaced as a row with the
     binding-limit narrative.
   - `Ranked`: at least one safe candidate found. Index 0 is the
     current params (always present); recommended is `.first_safe()`.

9. **Stage-1 DOC variant grid.** Anchored to the matched LUT row's
   axial bounds where available, falling back to a multiplier grid
   when not:

   - **LUT-anchored (preferred).** The row's `ap_min_mm` and `ap_max_mm`
     fields exist on every Amana / vendor calibration row that survives
     the must-match filter. Vendor envelopes typical 2.7×–3.7× wide
     (mean ~3.66× across 61 observations in
     `reference/shapeoko_feeds_and_speeds/data/vendor_lut/observations/amana_flat_end.json`).
     For 3-variant ops (Adaptive3d, Rest, Face): pick the grid as
     `[max(ap_min, 0.7×base), base, min(ap_max, 1.4×base)]`.
     For 4-variant ops (Pocket, Adaptive): add a midpoint between
     `base` and the upper bound — `[max(ap_min, 0.7×base), base,
     mid(base, hi), hi]`.
   - **Fallback grid (LUT row missing or no ap_min/ap_max).**
     `[0.7×, 1.0×, 1.3×]` for 3-variant; `[0.7×, 1.0×, 1.2×, 1.4×]`
     for 4-variant.
   - **Always include baseline (`1.0×`)** as the control candidate so
     the rollup can show "no change" honestly when no improvement
     beats it.
   - **Clip every variant** to the operation config's own min/max if
     the type carries one (some ops have a `min_doc`/`max_doc` field;
     most don't — use `0.05 mm` as a hard floor).

   Why upward-biased clipping but symmetric search: faster cycle time
   usually wants larger DOC, but Stage 1 is also our chance to find
   *non-monotone* wins (e.g. a smaller DOC + larger stepover combo
   shortens the path even if it spends more time per pass). Symmetric
   exploration is cheap (3–4 candidates × ~10s) and catches both.

10. **Baseline restoration is mandatory on every exit path.** A
    candidate evaluation mutates `session.toolpath_configs[idx].operation`
    (via `apply_toolpath_param_snapshot`) so the simulator can run
    against it. Same flavour of silent state leak as the `feeds_auto`
    issue: if the function returns or panics without restoring, the
    GUI now shows whichever candidate was last evaluated, not the
    baseline. Implementation:

    - At the top of `optimize_toolpath`, snapshot
      `(operation, dressups, face_selection, feeds_auto)` from
      `session.toolpath_configs[idx]`.
    - Wrap the candidate-evaluation loop in a `scopeguard::defer`-style
      RAII helper (or an explicit `let _restore = OnDropRestore { … };`)
      that re-applies the snapshot when the helper drops, regardless
      of how the loop exits — Ok, NoSafeImprovement, Skipped, cancel
      via `&AtomicBool`, or a panic in `execute_operation`.
    - The restoration is also a `apply_toolpath_param_snapshot` call,
      so it goes through the single mutation path and clears the
      stale-flag/sim cache one last time, leaving the session
      internally consistent.

    Apply on the modal is a *separate* mutation initiated by the user.
    Optimize's own internal candidate writes never persist past
    `optimize_toolpath` returning.

## Out of scope

- **Per-sample power model.** Currently no `power_kw` per
  `SimulationCutSample`. Optimize uses the same `power.rs` MRR-based
  estimate the existing gate uses.
- **Optimizing across operations** ("would replacing this rough+finish
  pair with adaptive+scallop be faster?"). That's the `FUTURE_PLANS.md`
  benchmark mode, an order of magnitude bigger.
- **Tool selection.** Optimize tunes parameters of the configured tool;
  it doesn't recommend "use a 4mm ball nose instead of 6mm".
- **Persisting candidate-search settings across runs.** The optimizer's
  search budget and ranking weights are constants in this version.
- **Real-time re-optimization on parameter change.** User edits, hits
  Optimize when they want it. No background dial-in.

## Verification

End-to-end on wanaka after U3:

1. Load `wanaka_full.toml`, generate all, run sim. Note baseline
   runtime.
2. Click toolbar **Optimize project**. Modal opens with progress strip
   + estimated time, completes in 5–15 min, can be cancelled mid-run.
3. Bottleneck callout shows TP 6 (verified separately to be 61% of
   runtime). Recommended delta on TP 6 is the largest.
4. Refusal rows for TP 1 (drill) and TP 2 (project_curve no samples)
   present with reason text.
5. Refusal rows for any project_curves where the gate disallows
   recommendation present with `gate-limited at chipload X` narrative.
6. Apply all → reconciliation sim → measured column populates. Mismatch
   between candidate-isolated and reconciled values on any single TP
   flagged with delta.
7. Total cycle time delta in header reflects the reconciled measurement.

If the bottleneck on a different project is power-bound rather than
chipload-bound, the rollup should still call out the right TP with the
correct `ExceedsReason` text.
