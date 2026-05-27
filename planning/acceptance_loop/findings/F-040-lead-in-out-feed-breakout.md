# F-040 — Lead-in / lead-out feed rate breakout

- **Stage:** dressup config + emitter
- **Severity:** low (finishing-pass quality nice-to-have; not blocking the modulation workstream)
- **Status:** landed 2026-05-27 (lead-in / lead-out feed override via `DressupConfig` + `MoveIntent::{LeadIn,LeadOut}` + F-039 modulator skip + 4 acceptance tests). Default `None` is byte-identical to pre-F-040.
- **First found in:** user feedback after F-039 design discussion, 2026-05-27
- **Effort:** S-M (one new dressup field, two new feed-rate fields, emitter consumes them, modulator respects the new intents)
- **Linked PRs:** —
- **Workstream:** Feed Modulation (adjacent — independent landing)
- **Depends on:** none (can land independently of F-039)

## Symptom

Today, lead-in and lead-out moves inherit the operation's primary `feed_rate`. Industry standard CAM (Fusion HSM, Mastercam) breaks these out as separate knobs:

- **Lead-in feed** typically lower than cutting feed — softer entry, cleaner mark
- **Lead-out feed** typically higher than cutting feed — clears chip on exit
- **Helix entry feed** distinct from ramp feed (separately tuneable)

Surface-finish-focused operations (scallop, drop-cutter, project-curve, pencil) benefit from a slower lead-in to avoid the dwell-mark a sudden cutting-feed engagement leaves. Roughing operations benefit less but get to share the param infrastructure.

## Hypothesised root cause

`OperationConfig` exposes `feed_rate` + `plunge_rate` (and op-specific `stepover` / `depth_per_pass` etc.) but not lead-in/out feeds. `DressupConfig.feed_ramp_rate` covers ramp entries; `DressupConfig.link_feed_rate` covers inter-segment linking. The slot for lead-in/out is empty — these moves silently use cutting feed.

## Fix shape

### Part A — extend `DressupConfig`

```rust
pub struct DressupConfig {
    // ... existing fields ...
    /// Lead-in feed rate (mm/min). Applied to moves tagged with
    /// `MoveIntent::LeadIn`. When `None`, falls back to the
    /// operation's primary feed_rate. Default: None.
    pub lead_in_feed_rate: Option<f64>,
    /// Lead-out feed rate (mm/min). Applied to moves tagged with
    /// `MoveIntent::LeadOut`. Same fallback semantics. Default: None.
    pub lead_out_feed_rate: Option<f64>,
}
```

### Part B — extend `MoveIntent` (already has Entry variants — lead-in/out are similar)

```rust
pub enum MoveIntent {
    // ... existing variants ...
    /// Lead-in: the cutter enters the cut from outside the material
    /// along a tangent / arc / line.
    LeadIn,
    /// Lead-out: the cutter exits the cut tangent / arc / line.
    LeadOut,
    // ...
}
```

### Part C — generators that emit lead-in/out tag the moves with the new intent

`crates/rs_cam_core/src/dressup.rs` (`apply_lead_in_out`) and any generators that emit lead-in/out directly (profile, scallop entry/exit) should tag the emitted moves with `MoveIntent::LeadIn` / `MoveIntent::LeadOut`.

### Part D — emitter consumes the new feeds

`crates/rs_cam_core/src/gcode/program_builder.rs` already routes per-move `feed_rate` (F-036a fixed this). For lead-in/out moves, generators set `move.move_type.feed_rate` directly to the dressup's `lead_in_feed_rate.unwrap_or(operation_feed_rate)` before the move ever enters the emitter. No emitter change needed.

### Part E — F-039 modulator respects the new intents

If F-039 ships before F-040, F-039's modulator should explicitly skip `MoveIntent::{LeadIn, LeadOut}` from modulation (treat as user-tuned, like `Linking` / `Retract`). Otherwise the modulator would rewrite the user's lead-in slowdown back up to band-mid. Document in F-039 + add this to F-040's acceptance test.

## Acceptance test

`crates/rs_cam_core/tests/lead_in_out_feed_rates_f040.rs`. Three tests:

1. `lead_in_feed_rate_applied_when_set` — profile op with `lead_in_feed_rate = Some(500.0)` and `feed_rate = 1500.0`. Generate. Walk the toolpath; assert lead-in-tagged moves carry F500 and cutting moves carry F1500.
2. `lead_in_falls_back_to_cutting_feed_when_none` — same op with `lead_in_feed_rate = None`. Assert lead-in moves carry F1500 (matches cutting feed; pre-F-040 behaviour).
3. `modulation_does_not_rewrite_lead_in_moves` — F-039 (or F-036) with modulation on. Lead-in moves keep their separate feed; modulator skips them.

## Files

- `crates/rs_cam_core/src/compute/config.rs` — add two `Option<f64>` to `DressupConfig`
- `crates/rs_cam_core/src/toolpath.rs` — add `LeadIn` / `LeadOut` variants to `MoveIntent`
- `crates/rs_cam_core/src/dressup.rs` — emit with new intents + use new feed
- `crates/rs_cam_core/src/feed_modulation.rs` (post-F-039) — exclude `LeadIn` / `LeadOut` from modulation
- `crates/rs_cam_viz/src/ui/...` — expose `lead_in_feed_rate` / `lead_out_feed_rate` in the dressup panel
- `crates/rs_cam_core/tests/lead_in_out_feed_rates_f040.rs` — new

## Risk

S. Purely additive. Existing toolpaths with default `None` continue using primary feed. Smoke baseline byte-identical.

## Notes

- Helix entry is a separate concern (currently inherits ramp_feed via `DressupConfig.feed_ramp_rate`; if a future need surfaces, add a `helix_entry_feed_rate` the same way).
- Arc feed-rate breakout NOT included — the kinematics integrator (F-034) handles arc-deceleration based on radius. A separate arc-feed knob is rarely useful in practice; revisit only if a real complaint surfaces.
- F-040 unblocks better lead-in tuning for the scallop / drop-cutter quality story but isn't on the modulation workstream's critical path. Pick up when finishing-pass quality is the focus.

## Landing notes (2026-05-27)

Landed as opt-in alongside F-038b + F-039 bench-prep batch.

### Behaviour clarification vs design doc

Doc said: lead-in falls back to "operation's primary feed_rate" when `None`. **Implementation:** falls back to the *plunge* feed of the move it replaces (the descent-to-cut-depth feed), preserving pre-F-040 byte-identical behaviour. Operator who wants lead-in at cut feed sets `lead_in_feed_rate = Some(<cut_feed>)` explicitly. This keeps the F-037 smoke baseline byte-identical and avoids surprising existing finishing ops that have `lead_in_out = true` set.

### Files

- `crates/rs_cam_core/src/toolpath.rs` — `MoveIntent::{LeadIn, LeadOut}` variants
- `crates/rs_cam_core/src/compute/config.rs` — `DressupConfig.lead_in_feed_rate` + `lead_out_feed_rate` (both `Option<f64>`, default `None`)
- `crates/rs_cam_core/src/dressup.rs` — new public `apply_lead_in_out_with_feeds`; legacy `apply_lead_in_out` delegates with `None`/`None`
- `crates/rs_cam_core/src/compute/execute.rs` — pass dressup's overrides into the new variant
- `crates/rs_cam_core/src/feed_modulation.rs` — `should_skip_modulation` skips `MoveIntent::{LeadIn, LeadOut}`
- `crates/rs_cam_viz/src/state/history.rs` — `#[allow(clippy::large_enum_variant)]` with SAFETY note (F-040's 16-byte addition to `DressupConfig` tipped the variant-size diff lint; pragmatic allow since `UndoAction` is stored in a bounded history `Vec`)
- `crates/rs_cam_core/tests/lead_in_out_feed_rates_f040.rs` — 4 acceptance tests

### Validation
- 4/4 F-040 acceptance tests pass (apply when set, byte-identical fallback when None, lead-out, F-039 modulator skip).
- F-037 smoke diff: **no regressions** (18 cases byte-identical).
- Workspace clippy clean.
- F-024 / F-028 / F-036b / F-038b / F-039 regression nets all green.

### Variant 6 bench fixture

Generated alongside this PR for the bench-batch USB bundle: a small profile fixture with `lead_in_feed_rate = 500.0`, `lead_out_feed_rate = 4500.0` — audible feed transition at entry / exit on the bench machine.
