# F-036 — Per-Segment Adaptive Feed Modulation Brief (fresh session)

**Paste this into a new Claude session to land F-036 without depending on prior session context.**

**WARNING**: F-036 is **L-XL effort** — the largest finding in the Feed Modulation workstream. Plan for 2-4 weeks of focused work, not a single session. The brief below sequences the work; consider breaking into sub-findings (F-036a/b/c) if scope grows past a single PR.

You are the implementer for **F-036 — Per-segment adaptive feed modulation** in the rs_cam Feed Modulation workstream.

## Context

F-034 (kinematics + cycle time) and F-035 (predicted-feed gates) provide the foundation. F-036 adds per-segment feed modulation: a post-pass that walks the toolpath, computes target feed per segment (constant-chipload-aimed), and writes it into the toolpath IR. G-code emission then emits `F<rate>` words at every feed change.

This is what Fusion HSM's "adaptive feed control" does. Expected user-visible win: 30-50% cycle time reduction on rough toolpaths at constant-or-better tool load.

## Working directory

`/home/ricky/personal_repos/rs_cam`

## Read in order

1. **`planning/feed_modulation_roadmap.md`** — workstream narrative + feature-flag discipline.
2. **`planning/acceptance_loop/findings/F-036-per-segment-feed-modulation.md`** — the finding with fix shape + acceptance test + files.
3. **F-034's commit and F-035's commit** (`git show <F-034 SHA>` and `<F-035 SHA>`) — read both. F-036 builds on the kinematics model and predicted-feed plumbing.
4. **`planning/acceptance_loop/implementer_contract.md`** — implementer rules.
5. **`CLAUDE.md`** — workspace lint policy.

## Preconditions

- **F-034 + F-035 + F-037 must have all landed.** Verify with `git log`.
- `MachineKinematics` exists; `predicted_achieved_feed` exists; smoke baseline exists.
- `pgrep -af cargo` returns nothing.
- `cargo test --workspace -q` clean.

**If any of these are missing — STOP and ask. F-036 is too risky to land against a moving foundation.**

## What to do

Follow F-036's four pieces:

### Piece A — verify IR carries per-segment feed

`rg "feed_rate" crates/rs_cam_core/src/toolpath/` to confirm `LinearMove` / `ArcMove` already have per-move feed fields. If they don't, add them (small task; one-line struct field addition + one constructor update).

### Piece B — modulation algorithm

Create `crates/rs_cam_core/src/machine_kinematics/modulation.rs` (or similar). Implement:

```rust
pub fn adaptive_feed_modulate(
    toolpath: &mut Toolpath,
    sim_samples: &[Sample],
    kinematics: &MachineKinematics,
    tool: &Tool,
    material: &Material,
    op_config: &OperationConfig,
    chipload_band: ChiploadBand, // from vendor LUT
) -> Result<(), ModulationError>
```

Algorithm:
1. For each move, look up its samples in `sim_samples` (filtered by move_idx).
2. Compute mean engagement (radial_woc_fraction, axial_doc) over the move's samples.
3. Target chipload = mid-band from the vendor LUT.
4. Compute base_feed = target_chipload × RPM × flutes.
5. Adjust for engagement: if engagement is light, can go higher; if heavy, must go lower. Use the chip-thinning correction inverse.
6. Cap at machine `max_feed_mm_min` AND at predicted-achievable-feed (from F-034's integrator).
7. Floor at `chipload_band.min × RPM × flutes` (no rubbing).
8. Set `move.feed_rate = computed_target`.

Watch out for:
- Discontinuities at move boundaries (consecutive moves with different feeds = junction velocity matters)
- Plunge moves (different chip formation; usually exclude from modulation)
- Air moves (already at full feed; exclude)

### Piece C — G-code emission

Find the G-code emitter (likely `crates/rs_cam_core/src/post_processor/` or similar; verify with `rg "Gcode\|fn write_gcode"`).

Update emission to:
- Emit `F<rate>` on the first cutting move
- Emit `F<rate>` on every subsequent move whose feed differs from the prior emitted feed
- Don't emit redundant F-words (when feed unchanged)

Test G-code output by hand for AS001 modulated:
- Should see varied F-words along the path
- F-words should round-trip through a G-code parser

### Piece D — feature flag

Add `pub adaptive_feed_modulation: bool` (default `false`) to `OperationConfig`. When true, the post-pass runs after toolpath generation, before G-code emission.

```rust
// pseudocode in operation execution:
let mut toolpath = generate_toolpath(...);
let samples = run_simulation_for_modulation_data(...);
if op_config.adaptive_feed_modulation && machine.kinematics.is_some() {
    adaptive_feed_modulate(&mut toolpath, &samples, ...)?;
}
emit_gcode(&toolpath, ...);
```

Note: modulation needs simulation samples to compute per-move engagement. This means we simulate BEFORE final G-code emission — an architectural shift. Consider: should modulation happen at generation time (using planner-side engagement estimates) or at simulation time (using simulator-side engagement)? The latter is more accurate; the former is faster. **Recommend simulator-driven for v1** (accuracy > speed).

### Piece E — acceptance tests

Write `tests/adaptive_feed_modulation_f036.rs`. Per the finding, seven tests:
- `flag_off_emits_identical_gcode_to_pre_f036` ← **critical**, protects loop calibration
- `modulated_path_has_per_segment_feed_variation`
- `modulated_gates_within_constant_chipload_band`
- `modulated_cycle_time_lower_than_unmodulated`
- `modulated_path_never_emits_below_min_chipload`
- `modulated_path_preserves_f024_axial_engagement_invariant`
- `modulated_path_preserves_zero_rapid_collision_invariant`

The last three are bridges to the loop's existing invariants.

### Piece F — verification

1. `cargo clippy --workspace --all-targets -- -D warnings` clean
2. `cargo test -q` clean (all existing acceptance tests pass byte-identical with flag-off)
3. Smoke baseline diff: zero regressions in default config
4. **Real-machine verification (load-bearing):** ask the user to run wanaka's Back Rough on their Shapeoko XXL with modulation ON, measure cycle time, compare to unmodulated. Assert: ≥ 20% cycle time reduction.

## Hard rules

- **One PR, or sequenced PRs.** If scope grows past 1 PR, split into F-036a (algorithm), F-036b (G-code emission), F-036c (feature flag plumbing) — but each must satisfy "flag-off byte-identical to pre-F-036 HEAD."
- **`adaptive_feed_modulation` defaults to `false`.** Non-negotiable.
- **Don't touch existing acceptance tests.** They run flag-off and must continue to pass byte-identical.
- **Don't bundle optimizer integration.** The optimizer's Stage 1/2 could host modulation candidates, but that's a future finding (F-038?).
- **Don't bundle HSM trochoidal toolpaths.** Different concern entirely.
- If F-036 exceeds 4 weeks of work without landing a runnable subset, stop and flag the user — likely needs sub-finding split.

## When to stop and ask

- The IR doesn't carry per-move feed — clarify with the user whether to extend it (small task, but adds scope)
- The modulation needs simulator samples, which means simulating before emission — architectural shift; confirm direction with the user
- GRBL parses the per-move F-words poorly on the user's Shapeoko XXL — different controller-specific quirks may need handling
- Smoke baseline shows ANY flag-off regression — bug in the flag plumbing
- Real-machine cycle-time reduction is < 20% — either the modulation algorithm is too conservative or the machine is already running near-optimally; investigate before declaring done

## When done

Output:
- Commit SHAs (likely multiple PRs)
- Files touched
- Before/after cycle time on wanaka Back Rough (kinematic estimate AND real-machine measured if available)
- Before/after deflection / chipload verdicts on AS001 modulated
- Smoke baseline diff (should be: zero regressions with default off, improved verdicts with flag on)
- Clippy + tests status

The user (and auditor in the next round) will verify the cycle-time win on their Shapeoko XXL.
</parameter>
</invoke>