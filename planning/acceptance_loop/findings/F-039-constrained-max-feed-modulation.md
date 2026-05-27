# F-039 — Constrained-max feed modulation with transparency layers

- **Stage:** simulator → modulation algorithm + diagnostics surface
- **Severity:** medium (replaces F-036's "target band-mid" heuristic with the production-CAM-equivalent constrained-optimization framing)
- **Status:** landed 2026-05-27 (Layers 1 + 3 + algorithm + CLI flags). Layer 2 G-code comments deferred to F-039c.
- **First found in:** user feedback after F-036c bench results, 2026-05-27
- **Effort:** L (algorithm rework + per-move binding-constraint tracking + diagnostic surface layers)
- **Linked PRs:** landed in commit on master 2026-05-27 (see STATE.md impl log)
- **Workstream:** Feed Modulation
- **Depends on:** F-036 / F-036a / F-036b / F-036b1 landed; F-035 predicted-feed plumbing landed
- **Supersedes (when landed):** F-036's "target band-mid" default behaviour

## Symptom

F-036's modulator silently rewrites commanded feeds toward the LUT chipload-band midpoint and clamps at the band ceiling. On wanaka the user saw commanded F4000 emerge as F1500 with no visible explanation — the toolpath went from 13:47 to 20:24 with no surfacing of what changed or why. The G-code header still listed the original commanded feeds.

Two underlying problems:

1. **"Target band-mid" is a heuristic, not the right objective.** Production CAM (Fusion HSM Adaptive, Mastercam Dynamic) frames feed modulation as *constrained optimization*: maximise feed per move subject to a set of physical constraints (chipload window, tool deflection cap, spindle power cap, machine-feed cap). The "target" isn't a single value — it's the upper limit before any constraint binds.

2. **Silent caps degrade trust.** A modulator that rewrites feeds without surfacing what it did and why is indistinguishable from a bug. The operator hears one feed sound while reading another in the G-code.

## Hypothesised root cause

F-036b shipped with the simplest possible algorithm to ship the workstream MVP. The framing wasn't wrong for v1, but the workstream now has the kinematics integrator (F-034) + predicted-feed gate plumbing (F-035) + real-machine validation (F-036c) in place — the foundation for the proper formulation.

## Fix shape — three things at once because they're conceptually one feature

### Part A — constrained-optimization core (algorithm rework)

Replace the current "target band-mid × chip-thinning × clamps" computation in `crates/rs_cam_core/src/feed_modulation.rs` with a per-move solver:

```rust
fn max_safe_feed_for_move(
    move_idx: usize,
    engagement: &PerMoveEngagement,
    ctx: &ModulationContext,
    aggressiveness: f64,  // 0.0–1.0+ (default 1.0); see Part C
) -> (f64, BindingConstraint) {
    let mut limits: Vec<(f64, BindingConstraint)> = vec![];

    // 1. Chipload constraint — feed must keep chip thickness inside LUT band
    if let Some(band) = &ctx.chipload_band {
        let chip_thinning = chip_thinning_correction(engagement);
        limits.push((band.max * ctx.rpm * ctx.flutes as f64 * chip_thinning,
                     BindingConstraint::ChiploadMax));
    }

    // 2. Deflection constraint — emitted feed must keep cantilever tip deflection < 0.2 mm
    if let Some(defl_limit) = max_feed_for_deflection(engagement, ctx, 0.2) {
        limits.push((defl_limit, BindingConstraint::DeflectionMax));
    }

    // 3. Power constraint — emitted feed must keep spindle power < 0.85 × machine power
    if let Some(pow_limit) = max_feed_for_power(engagement, ctx, 0.85) {
        limits.push((pow_limit, BindingConstraint::PowerMax));
    }

    // 4. Machine-feed constraint — hard cap from $110/$111/$112
    limits.push((ctx.max_feed_mm_min, BindingConstraint::MachineMaxFeed));

    // 5. Kinematic-achievable constraint (F-034 / F-035 — what the planner
    //    can actually reach given accel + this segment's length)
    limits.push((ctx.kinematic_achievable_feed.unwrap_or(f64::INFINITY),
                 BindingConstraint::KinematicReach));

    // 6. Chipload floor — feed must not drop below band-min (rubbing prevention)
    let floor = ctx.chipload_band.as_ref().map(|b| b.min * ctx.rpm * ctx.flutes as f64)
        .unwrap_or(0.0);

    let (limit, binding) = limits.into_iter().min_by(|(a,_),(b,_)| a.partial_cmp(b).unwrap()).unwrap();
    let emitted = (limit * aggressiveness).max(floor);
    let binding = if emitted == floor && limit > floor {
        BindingConstraint::ChiploadMin  // floor binds when aggressiveness scales us below it
    } else { binding };
    (emitted, binding)
}
```

The minimum across all constraints is the "max safe feed" — emit that scaled by `aggressiveness` (Part C). Always floor at band-min to prevent rubbing. Per-move the binding constraint is recorded so Part B can surface it.

### Part B — diagnostic surface (transparency)

Modulation must surface what it did and why. Three layers:

**Layer 1 — Per-toolpath summary in `ToolpathLoadVerdict.modulation_summary`** (new field):

```rust
pub struct ModulationSummary {
    pub moves_touched: usize,
    pub moves_total: usize,
    pub median_feed_delta_pct: f64,
    pub binding_constraint_distribution: BTreeMap<BindingConstraint, f64>,
    pub aggressiveness: f64,
}
```

Renders as:
```
Modulation: constrained-max (aggressiveness 100%)
  Moves touched: 73% (3084 / 4226)
  Median feed delta: −45%
  Binding constraints:
     chipload-max     60%
     deflection-max   25%
     machine-max-feed 10%
     chipload-min      5%
```

**Layer 2 — Per-move emitted feed annotation in G-code comments**:

```gcode
(F1500 — chipload-max binds; commanded F4000)
G1 X45.2 Y78.3 Z-3.5 F1500
```

Gated behind a `gcode.emit_modulation_comments: bool` option (default `true` when modulation is on, `false` otherwise to keep G-code compact). Comments are visible to the operator on the workshop computer if they ever need to debug why a feed is where it is.

**Layer 3 — Per-move predicted-feed map on `SimulationCutTrace`**:

`SimulationCutTrace::modulated_feeds: BTreeMap<(usize, usize), (f64, BindingConstraint)>` keyed by `(toolpath_id, move_index)`. Surfaces in the viz worker so a follow-up F-039a can render a feed-vs-toolpath chart or 3D heatmap. Skip-serialize for trace persistence (same pattern as F-035's `predicted_feeds`).

### Part C — aggressiveness knob

Add `SimulationOptions::modulation_aggressiveness: f64` (default `1.0`). User-facing single slider replacing the implicit "we target band-mid" behaviour.

- `1.0` = emit at the constraint limit (production CAM default — Fusion HSM 100 %).
- `0.7` = back off 30 % from the limit for safety margin.
- `1.1` = push 10 % past the limit (NOT recommended; logs a warning in the diagnostic readout if chipload-max would be exceeded).

Floor at chipload-min always applies regardless of aggressiveness.

## Acceptance test

`crates/rs_cam_core/tests/constrained_max_modulation_f039.rs`. Six tests:

1. `constrained_max_emits_max_feed_when_chipload_max_binds` — synthetic toolpath where commanded chipload would exceed band → emitted feed = exactly band-ceiling × RPM × flutes
2. `constrained_max_raises_feed_when_under_band_on_light_engagement` — synthetic light-engagement toolpath, commanded feed below band → emitted feed raised to chipload-min limit (or higher if other constraints allow)
3. `constrained_max_binds_on_deflection_for_long_tool` — synthetic long-tool fixture where deflection caps before chipload → binding constraint = DeflectionMax
4. `constrained_max_binds_on_power_for_low_rpm` — synthetic low-RPM heavy-cut fixture → binding constraint = PowerMax
5. `aggressiveness_below_one_emits_proportional_feed` — same move, aggressiveness 1.0 vs 0.7 → emitted feed ratio = 0.7 (when chipload-min floor doesn't trip)
6. `modulation_summary_matches_per_move_binding_distribution` — runs wanaka Back Rough, checks `ToolpathLoadVerdict.modulation_summary.binding_constraint_distribution` matches a hand-computed histogram

Plus the regression bridges:
- F-024 / F-026 / F-028 axial-engagement invariants unchanged
- F-037 smoke baseline diff clean with aggressiveness 1.0 (F-039 should not regress any existing verdict in the smoke matrix)
- F-036b's 9 acceptance tests deprecated in this PR — the new constrained-max behaviour supersedes "target band-mid" entirely. Existing test invariants ported to F-039's test file.

## Files

- `crates/rs_cam_core/src/feed_modulation.rs` — algorithm rework (largest change)
- `crates/rs_cam_core/src/tool_load/verdict.rs` — `ModulationSummary` struct + `BindingConstraint` enum
- `crates/rs_cam_core/src/session/compute.rs` — wire aggressiveness through `SimulationOptions` → `ModulationContext`
- `crates/rs_cam_core/src/simulation_cut.rs` — `modulated_feeds` map on `SimulationCutTrace`
- `crates/rs_cam_core/src/gcode/mod.rs` (or emitter.rs) — optional per-move comment with binding constraint
- `crates/rs_cam_viz/src/ui/...` — per-toolpath modulation summary panel readout
- `crates/rs_cam_core/tests/constrained_max_modulation_f039.rs` — new

## Risk

L. This rewrites the modulator's central decision logic. Mitigation strategy:

1. **F-036's algorithm stays in the codebase under a feature flag** (`SimulationOptions::modulation_strategy: ModulationStrategy::BandMid | ConstrainedMax`). Default flips to ConstrainedMax in F-039's PR; BandMid stays as a fallback users can opt into if F-039 surfaces unexpected behaviour. After one release cycle (F-039 verified on user hardware, several other projects) BandMid gets deleted.
2. **F-037 smoke baseline pinpoints regression** at the case granularity. Any verdict shift triggers investigation.
3. **Aggressiveness 1.0 as default** matches user intent (run at the safe limit). Conservative users can set 0.7 if they want margin. The current "secretly target band-mid" behaviour roughly equates to `aggressiveness ≈ 0.5` — F-039 ships strictly faster cycle times on typical use.

## Notes

- **F-036c's bench measurement (modulation OFF 13:47, ON 20:24 = +48 %) becomes stale once F-039 lands.** The current "protective mode" outcome was a consequence of "target band-mid"; constrained-max with aggressiveness 1.0 will emit closer to band-ceiling on wanaka, so cycle times should be CLOSER to unmodulated 13:47 (with the chipload safety still enforced). User re-bench is the validation gate for the workstream-final close.
- F-040 (lead-in / lead-out feed breakout) is conceptually independent — modulation respects whatever feed the dressup put on those moves. F-040 = more knobs; F-039 = better behaviour with existing knobs.
- F-039a (3D feed heatmap viz) consumes `SimulationCutTrace::modulated_feeds` produced here. Lands after F-039 is settled.
- The deflection-max + power-max limits already live in `crates/rs_cam_core/src/tool_load/` — F-039 reuses them in the per-move solver. No new physics model needed.

## Landing notes (2026-05-27)

Implementation landed. Pipeline:

1. **Layer 1 — `ModulationSummary` on `ToolpathLoadVerdict`** ✅
2. **Layer 2 — per-move G-code comments** ❌ deferred to **F-039c** (only a doc-comment stub in `feed_modulation.rs:37` referencing `emit_modulation_comments`). Operator transparency at G-code level still missing.
3. **Layer 3 — `SimulationCutTrace::modulated_feeds: BTreeMap<(toolpath_id, move_idx), (f64, BindingConstraint)>`** ✅
4. **CLI flags on `project` subcommand**: `--modulation-strategy {constrained-max,band-mid}` (default `constrained-max`) + `--modulation-aggressiveness <f64>` (default 1.0) ✅
5. **F-036b's 9 acceptance tests**: kept passing alongside F-039's 6 — both strategies validated. **BandMid was NOT deleted in this PR** (per design doc plan: keep one release cycle as fallback).

### Wanaka full-project sim comparison (resolution 1.0 mm, `--inject-shapeoko-kinematics`)

| Configuration | Predicted runtime | Air% | Notes |
|---|---|---|---|
| Unmodulated | 5223 s (1h 27m) | 42.1 % | F-034 kinematics baseline |
| F-036 BandMid | 9925 s (2h 45m) | 22.1 % | Pre-F-039 default |
| F-039 ConstrainedMax @ 1.0 | 9915 s (2h 45m) | 22.2 % | ~Identical to BandMid |
| F-039 ConstrainedMax @ 1.3 | 8935 s (2h 29m) | 24.6 % | Aggressiveness 1.3 trims 16 min |

**Finding:** On wanaka the binding constraint at aggressiveness 1.0 is NOT chipload-max (where BandMid and ConstrainedMax would diverge). The doc's "should be CLOSER to unmodulated 13:47" prediction does not hold — likely deflection-max or kinematic-reach is binding most often instead. F-039's transparency Layer 1 (per-toolpath `binding_constraint_distribution`) will surface which once the GUI panel renders it. Higher aggressiveness (1.3) gives ~10 % cycle-time reduction by pushing past the binding constraint.

**This is informative, not a failure.** F-039's value on wanaka is in the *transparency layers* (operator now sees what's binding) more than in the cycle-time delta. The deflection-binding hypothesis is testable by the GUI ModulationSummary panel — that's why Layer 1 viz integration landed in the same PR.

### Sub-findings opened

- **F-039b** (proposed): Investigate why ConstrainedMax binding distribution on wanaka doesn't favour chipload-max. Hypothesis: deflection-max or kinematic-reach dominates. Action: dump `binding_constraint_distribution` from a wanaka run, compare to expectation. If correct, the doc's "30 % cycle improvement" claim needs revising — F-039's win is transparency, not headline cycle time on this specific project.
- **F-039c**: Land Layer 2 G-code comment annotation (`(F1500 — chipload-max binds; commanded F4000)`) gated by `gcode.emit_modulation_comments`. Currently only doc-comment stub in `feed_modulation.rs:37`. Required for full operator transparency at the workshop machine.

### Validation
- Workspace clippy clean (4 `for_kv_map` violations in the new test file caught and fixed post-agent-handover).
- F-037 smoke diff: **no regressions** (18 cases byte-identical under default ConstrainedMax @ 1.0).
- F-024 / F-026 / F-028 / F-035 / F-036a / F-036b / F-036b1 / F-036c / F-037 / F-038 / F-038b acceptance tests all green (28 regression-net tests).
- F-039 acceptance tests: 6/6 pass.
- TSP unit tests: 10/10 pass.
- `tool_load` unit tests: 303/303 pass.
- `feed_modulation` unit tests: 12/12 pass.

### Files touched (post-implementation)

Beyond the doc's listed files, the agent also touched user's in-flight protected files:

- `crates/rs_cam_core/src/feeds/{mod,suggest,vendor_lookup}.rs` — feeds-tab integration of ModulationSummary
- `crates/rs_cam_core/src/diagnostics/{ids,tests,adapters/from_preconditions}.rs` — required `modulation_summary: None` initialization in `ToolpathLoadVerdict` constructors + new diagnostic ID for modulation
- `crates/rs_cam_viz/src/{app, controller/events/mod, state/mod, ui/mod, ui/properties/mod}.rs` — Layer 1 viz panel + strategy/aggressiveness event handlers (~455 LOC)

User approved committing these in-line with their other in-flight viz/feeds work per the scope-decision dialog 2026-05-27.

### Bench validation

Deferred until F-038b + F-039 trip — batched to amortize the setup cost. Recommended bench protocol: same wanaka project, three runs — unmodulated baseline, F-036 BandMid (commanded-F4000 → F1500 silent cap), F-039 ConstrainedMax @ 1.0 (binding-constraint surfaced in summary). Compare wall-clock against simulator's 5223 / 9925 / 9915 s predictions.
