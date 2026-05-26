# F-036 — Per-segment adaptive feed modulation

- **Stage:** toolpath generation + G-code emission (large flag-gated feature)
- **Severity:** medium (cycle-time / tool-life feature; not a correctness gap)
- **Status:** open
- **First found in:** round-10 user discussion (2026-05-26)
- **Effort:** **L-XL** (multi-file feature touching IR, gates, post-processor, optimizer)
- **Linked PRs:** —
- **Workstream:** Feed Modulation (see `planning/feed_modulation_roadmap.md`)
- **Depends on:** F-034 + F-035 land first
- **Source audits:** round-10 user discussion of Fusion HSM equivalent

## Symptom

rs_cam emits a single constant `feed_rate` per toolpath. Industrial CAM (Fusion HSM "adaptive feed control," Mastercam Dynamic, BobCAD HSR) modulates feed per segment based on engagement: slower through high-engagement corners, faster through low-engagement straight runs. Result on hobby machines: 30-50% cycle-time reduction at constant-or-better tool load.

Without modulation, every toolpath is "as fast as the worst engagement point" — corners cap the entire toolpath's feed. With modulation, corners get slower-feed local windows while straights get higher feed.

## Hypothesised root cause

This is missing feature work, not a bug. The toolpath IR currently has `feed_rate` on each move (`LinearMove { from, to, feed_rate }`) but the planner sets every move's `feed_rate` to the same op-level value. No per-segment differentiation.

## Fix shape

**Four pieces, sequenced**:

### Piece A — verify IR carries per-segment feed

Verify the toolpath IR types in `crates/rs_cam_core/src/toolpath/` already have per-move `feed_rate`. If they do (likely; F-018 era confirmed individual move feeds exist), no IR change needed. If they don't, add it.

### Piece B — modulation algorithm

Post-pass on the generated toolpath. For each move:
1. Get the move's commanded base feed (from the op config).
2. Get the move's predicted achievable feed from F-034's kinematics integrator.
3. Get the move's expected engagement from the simulator's per-sample data.
4. Compute target feed:
   - Target chipload = mid-LUT-band for the tool/material
   - Target feed = target_chipload × RPM × flutes
   - Adjust for engagement: at heavy engagement, target feed = base * (max_chipload / observed_chipload)
   - Adjust for kinematics: cap at machine max_feed and predicted achievable
   - Floor at chipload-min × RPM × flutes (no rubbing)
5. Set `move.feed_rate = target_feed`.

The algorithm should be implementable as a single pure function:

```rust
pub fn adaptive_feed_modulate(
    toolpath: &mut Toolpath,
    sim_samples: &[Sample],
    kinematics: &MachineKinematics,
    tool: &Tool,
    material: &Material,
    op_config: &OperationConfig,
) -> Result<(), ModulationError>
```

### Piece C — G-code emission

Update the post-processor to emit `F<rate>` words on every move whose feed differs from the prior. Most GRBL controllers handle this fine. Verify Shapeoko XXL parses it correctly.

Add a roundtrip test: generated G-code parses correctly, when interpreted by a G-code-aware simulator produces feed values matching the IR's per-move feed.

### Piece D — feature flag

```rust
pub struct OperationConfig {
    // ... existing fields ...
    /// When true (default false), apply adaptive feed modulation post-pass to the toolpath.
    /// Requires Machine::kinematics. Falls through (no modulation) if not set.
    pub adaptive_feed_modulation: bool,
}
```

Default false. When false, every move's feed is set to op-level `feed_rate` (existing behavior, byte-identical G-code).

## Acceptance test

```rust
// tests/adaptive_feed_modulation_f036.rs

#[test]
fn flag_off_emits_identical_gcode_to_pre_f036() {
    // Generate AS001 with adaptive_feed_modulation = false.
    // Assert: emitted G-code byte-identical to pre-F-036 HEAD.
}

#[test]
fn modulated_path_has_per_segment_feed_variation() {
    // Generate AS013 (corner-heavy adaptive3d) with modulation on.
    // Assert: at least 30% of moves have feed != op_config.feed_rate.
}

#[test]
fn modulated_gates_within_constant_chipload_band() {
    // Generate + simulate modulated AS013.
    // Assert: chipload Within. Variance of chipload across samples
    //         < pre-modulation variance.
}

#[test]
fn modulated_cycle_time_lower_than_unmodulated() {
    // Same toolpath, modulation off vs on.
    // Assert: kinematic cycle time (from F-034) drops by ≥ 20%.
}

#[test]
fn modulated_path_never_emits_below_min_chipload() {
    // Safety: feed must never drop below chipload_min × RPM × flutes.
    // Assert: min feed_rate across all moves ≥ chipload_min_x_rpm_x_flutes.
}

#[test]
fn modulated_path_preserves_f024_axial_engagement_invariant() {
    // AS001 modulated.
    // Assert: peak_axial_doc_mm still ≤ commanded DOC + margin.
    // Modulation must not break frame correctness.
}

#[test]
fn modulated_path_preserves_zero_rapid_collision_invariant() {
    // AS013 modulated.
    // Assert: rapid_collision_count == 0.
    // Modulation must not introduce collision regressions.
}
```

The last three tests are bridges to the loop's existing invariants — they assert that modulation doesn't break what the loop calibrated.

## Files (likely; verify before assuming)

- `crates/rs_cam_core/src/toolpath/` — IR types (`LinearMove`, `ArcMove`, etc); confirm per-move feed_rate exists
- `crates/rs_cam_core/src/compute/operation_configs.rs` — `OperationConfig::adaptive_feed_modulation` field
- `crates/rs_cam_core/src/machine_kinematics/modulation.rs` — new module, hosts `adaptive_feed_modulate`
- `crates/rs_cam_core/src/post_processor/` (or wherever G-code emission lives) — emit `F<rate>` words on feed change
- `crates/rs_cam_core/tests/adaptive_feed_modulation_f036.rs` — new

## Risk

L-XL.

- **Risk**: scope creep. The seven acceptance tests cover most failure modes but the feature touches IR + post-pass + gates + emission. Mitigation: tight scope. Don't bundle with optimizer integration. Don't bundle with HSM trochoidal toolpaths.
- **Risk**: regression on existing acceptance tests. Mitigation: flag default off, all existing tests run with flag off and assert byte-identical G-code.
- **Risk**: machine-specific quirks in G-code emission (GRBL planner lookahead size, F-word handling on direction changes). Mitigation: test on real Shapeoko XXL before declaring done; defer support for non-GRBL controllers.
- **Risk**: the per-sample simulator data may not have the right resolution to drive per-move modulation cleanly. Mitigation: prototype on a single op kind first (adaptive3d roughing), expand once it works.

## Notes

- **Land F-034 + F-035 first.** The kinematics model and predicted-feed plumbing are prerequisites.
- **Default flag behavior is critical.** Without `adaptive_feed_modulation = false` being byte-identical to pre-F-036, the loop's regression net would catch this as a regression on every test.
- **The optimizer's Stage 1/2 infrastructure could host this** as a candidate generator. NOT in scope for F-036. Open a follow-up (F-038?) if you want optimizer integration.
- **HSM trochoidal toolpaths are different.** Modulation adjusts feed on existing geometry; trochoidal CHANGES geometry. Don't conflate.
- **Validation on real machine.** Before declaring F-036 done, generate a modulated wanaka Back Rough, run it on the user's Shapeoko XXL, measure cycle time. Assert: ≥ 20% reduction vs unmodulated, deflection still Within, surface finish acceptable.
</parameter>
</invoke>