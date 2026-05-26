# F-035 — Predicted-effective-feed in chipload / power / deflection gates

- **Stage:** sim / tool-load gates (flag-gated feature)
- **Severity:** medium (gate verdicts can be optimistic; hobby machines mis-graded)
- **Status:** landed (2026-05-26)
- **First found in:** round-10 wanaka audit (2026-05-26); user-flagged
- **Effort:** M
- **Linked PRs:** (this PR — to be filled at merge)
- **Workstream:** Feed Modulation (see `planning/feed_modulation_roadmap.md`)
- **Depends on:** F-034 (machine kinematics model) — landed at `ae58f55`
- **Source audits:** round-10 wanaka verification + user discussion
- **Acceptance test:** `crates/rs_cam_core/tests/predicted_feed_gates_f035.rs`
  (`flag_off_byte_identical_to_pre_f035`,
  `flag_on_corner_decel_drops_chipload_below_band`,
  `flag_on_straight_line_chipload_unchanged`,
  `flag_on_extends_existing_f024_test_invariants`)

## Symptom

The chipload / power / deflection gates assume the commanded feed equals the achieved feed. On a hobby-class machine (Shapeoko XXL with stock 250 mm/s² accel), the controller's planner decelerates through corners — the cutter often never reaches the commanded feed on tight geometry.

Concrete failure mode:
- Commanded feed = 4000 mm/min
- Through a tight curve, achieved feed = 2000 mm/min (50% of commanded)
- Gate uses commanded 4000 to compute chipload → reports 0.038 mm/tooth → Within
- Reality: chipload at 2000 mm/min = 0.019 mm/tooth → **Exceeds_LOW**, rubbing, burning hardwood, dulling carbide

The chipload calibration we did in the acceptance loop (F-001 + F-003) calibrated against the LUT bands — but assumed the machine actually achieves commanded feed. For straight-line cuts that's true; for corner-heavy 3D paths on hobby machines it's not.

## Hypothesised root cause

The gate sample loop reads `move.feed_rate` (the commanded value) when computing chipload per sample. There's no model of "what feed did the machine actually achieve at this sample given accel/jerk limits."

F-034 introduces the kinematics model needed to predict achieved feed. F-035 plumbs that through the gates.

## Fix shape

**Three-step**:

### Step A — predicted-feed-per-move function

Implement in `simulation/` or `machine_kinematics/`:

```rust
pub fn predicted_achieved_feed(
    move: &Move,
    prev_move: Option<&Move>,
    next_move: Option<&Move>,
    kinematics: &MachineKinematics,
    max_feed: f64,
) -> f64 // mm/min, the realistic feed the machine reaches mid-move
```

Use F-034's integrator backwards: given the move's start velocity (from junction with prev), commanded feed, and end velocity (junction with next), find the peak velocity actually reached within the move's distance under accel/jerk limits.

For long moves (> ~10× accel distance), this == commanded. For short moves between two corners, this < commanded.

### Step B — feature flag on `SimulationOptions`

```rust
pub struct SimulationOptions {
    // ... existing fields ...
    /// When true (default false), gates use predicted achieved feed instead of commanded.
    /// Requires Machine::kinematics to be set; ignored if not.
    pub use_predicted_feed_in_gates: bool,
}
```

Default `false` so all existing acceptance tests pass byte-identically with no changes.

### Step C — gate consumption

Update the chipload / power / deflection sample loops to:

```rust
let feed = if options.use_predicted_feed_in_gates && machine.kinematics.is_some() {
    predicted_achieved_feed(move, prev, next, machine.kinematics.as_ref().unwrap(), machine.max_feed_mm_min)
} else {
    move.feed_rate  // existing path; byte-identical to pre-F-035
};

let chipload = feed / (spindle_rpm * flute_count as f64);
// rest of gate evaluation unchanged
```

Sites to touch:
- `crates/rs_cam_core/src/tool_load/chipload.rs` (or wherever the chipload gate lives)
- `crates/rs_cam_core/src/tool_load/power.rs`
- `crates/rs_cam_core/src/tool_load/deflection.rs`

All three should share the same feed-resolution helper to avoid drift.

## Acceptance test

```rust
// tests/predicted_feed_gates_f035.rs

#[test]
fn flag_off_byte_identical_to_pre_f035() {
    // Run AS001 with Machine::kinematics set, use_predicted_feed_in_gates = false.
    // Assert: chipload/power/deflection verdicts and observed values
    // byte-identical to pre-F-035 HEAD on the same toolpath.
}

#[test]
fn flag_on_corner_decel_drops_chipload_below_band() {
    // Synthetic toolpath: tight corner that the Shapeoko kinematics can't sustain.
    // Flag off: chipload Within (commanded feed used).
    // Flag on: chipload Exceeds_LOW (predicted feed used).
    // The difference IS the bug F-035 catches.
}

#[test]
fn flag_on_straight_line_chipload_unchanged() {
    // Single long linear move where commanded == achieved.
    // Flag off and flag on produce identical chipload.
}

#[test]
fn flag_on_extends_existing_f024_test_invariants() {
    // Re-run AS001 deflection invariant with flag on.
    // Assert: peak_axial_doc_mm still matches commanded DOC + margin.
    // (Predicted feed shouldn't break the frame correctness of F-024.)
}
```

The fourth test is the bridge to F-024 — confirms predicted-feed doesn't accidentally break frame-related invariants.

## Files

Starting points (verify before assuming):
- `crates/rs_cam_core/src/tool_load/chipload.rs`
- `crates/rs_cam_core/src/tool_load/power.rs`
- `crates/rs_cam_core/src/tool_load/deflection.rs`
- `crates/rs_cam_core/src/compute/simulate.rs` — `SimulationOptions`
- `crates/rs_cam_core/src/machine_kinematics/` — new module hosting `predicted_achieved_feed` (added in F-034)
- `crates/rs_cam_core/tests/predicted_feed_gates_f035.rs` — new

## Risk

M.

- **Risk**: the three gate files have separate sample loops. Easy to miss one. Mitigation: extract a shared feed-resolution helper, all three gates call it.
- **Risk**: the predicted-feed function depends on adjacent moves (prev / next for junction velocity). The simulator's iteration order needs to provide that context; check before assuming.
- **Risk**: existing tests assert specific observed chipload values; if flag-off doesn't reproduce byte-identically, those tests will fail. Mitigation: flag default false, gates fall through to existing code path.

## Notes

- **Wait for F-034 to land first.** The kinematics model is the prerequisite.
- **Don't conflate with F-036.** F-035 only changes gate evaluation, NOT the emitted G-code. The machine still receives the commanded feed; the gates just grade what the machine ACTUALLY does with it.
- The flag-on/flag-off duality protects the loop's calibration. The cargo `_f0{24,26,27,28,31}.rs` acceptance tests stay flag-off by default and continue to pass byte-identical.
- The eventual default flip to `true` is a separate decision, after F-036 lands and the predicted-feed signal is validated against real-machine measurements.

## Implementation log (landed 2026-05-26)

- The chipload gate consumes `predicted_feed` by **scaling**
  `s.effective_chip_thickness_mm` by `predicted / commanded`. Chip
  thickness (arc-mean of `feed_per_tooth × geometry_factor`) is
  linear in feed, so scalar scaling exactly reproduces what
  re-deriving via `MillingCutter::chip_geometry` would produce —
  without the gates needing a cutter reference at every sample.
- The power gate substitutes feed directly into
  `P = Kc · DOC · WOC · feed / 60M`.
- The **deflection** gate uses `F = Kc · DOC · WOC` (NO feed term in
  the cantilever-deflection integral). It is therefore
  feed-independent and the predicted-feed plumbing is a no-op for
  it. The four acceptance tests document this with a sanity-check
  that flag-ON deflection on AS001 still produces a `Within` verdict
  in the safe band.
- Predicted feeds are stored on a new
  `SimulationCutTrace::predicted_feeds: BTreeMap<(toolpath_id,
  move_index), f64>` field, populated once per simulation when
  `SimulationOptions::use_predicted_feed_in_gates` is on AND the
  active `MachineProfile` carries `kinematics`. The field is
  `#[serde(skip)]` — round-tripping a trace through the JSON debug
  artifact loses the predicted-feed map and the gates fall back to
  commanded feed; the map is re-derivable from the toolpath IR +
  kinematics at any time.
- The single source-of-truth helper
  `tool_load::effective_feed_for_sample(sample, &predicted_feeds)`
  lives at `crates/rs_cam_core/src/tool_load/mod.rs`. Both the
  chipload gate (`tool_load/chipload.rs`) and the power gate
  (`tool_load/power.rs`) call it — no duplication. Drift between
  the two gates would now require editing the helper in one place,
  catchable by the acceptance test.
- Viz worker `SimulationRequest` grew three fields (`kinematics`,
  `use_predicted_feed_in_gates`, `max_feed_mm_min`) and the
  controller's `run_simulation` event copies them from
  `self.state.session.machine()`. The GUI doesn't yet expose a
  toggle for the flag (defaults to `false`); a follow-up under F-036
  can add the simulation-panel checkbox + wire it up. Both
  workspaces (core + viz) now drive predicted-feed plumbing through
  the same code path.
</parameter>
</invoke>