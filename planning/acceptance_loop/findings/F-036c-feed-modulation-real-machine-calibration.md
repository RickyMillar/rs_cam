# F-036c — Real-machine cycle-time calibration (deferred from F-036)

- **Stage:** verification (user-side measurement)
- **Severity:** low (validation, not a code defect)
- **Status:** open — deferred from F-036's Piece F
- **First found in:** F-036 implementation session, 2026-05-26
- **Effort:** S code-side (unflag an existing `#[ignore]`d test); requires
  one wall-clock measurement on a real Shapeoko XXL
- **Linked PRs:** —
- **Workstream:** Feed Modulation (see `planning/feed_modulation_roadmap.md`)
- **Depends on:** F-036 algorithm + F-036a G-code emission + F-036b production wiring

## Symptom

The F-036 workstream's headline success criterion is "≥ 20% cycle-time reduction at constant-or-better tool load on a real Shapeoko XXL." Until that measurement lands, the modulator is plausible but unvalidated against ground truth — same gap F-034 left for its kinematics integrator (`#[ignore]`d `shapeoko_xxl_cycle_time_calibration` test with a `REFERENCE_MEASURED_S` placeholder).

## Hypothesised root cause

Not a bug. Real-machine measurement is the only way to close the modulation loop:

1. Generate wanaka's Back Rough operation with `adaptive_feed_modulation = false`.
2. Run on Shapeoko XXL, wall-clock the runtime.
3. Generate same operation with `adaptive_feed_modulation = true`.
4. Run on Shapeoko XXL, wall-clock the runtime.
5. Compute (1 - t_modulated / t_unmodulated).
6. Assert ≥ 0.20 (20% reduction).

## Fix shape

1. Code-side: add an `#[ignore]`d test
   `crates/rs_cam_core/tests/adaptive_feed_modulation_cycle_calibration_f036c.rs` that:
   - Computes kinematic cycle time for wanaka Back Rough unmodulated.
   - Computes kinematic cycle time for wanaka Back Rough modulated.
   - Compares the **measured** values (constants in the test) to the
     kinematic predictions.
   - Asserts ≥ 20% reduction on both predicted and measured.
2. User-side: run the two G-code programs on Shapeoko XXL, record runtimes, populate the `MEASURED_UNMODULATED_S` and `MEASURED_MODULATED_S` constants, un-`#[ignore]` the test.

## Acceptance test

```rust
// crates/rs_cam_core/tests/adaptive_feed_modulation_cycle_calibration_f036c.rs

#[test]
#[ignore = "requires real-machine measurement on Shapeoko XXL — populate \
            constants then unflag"]
fn shapeoko_xxl_modulation_reduces_cycle_time_by_20pct() {
    // const MEASURED_UNMODULATED_S: f64 = TODO;
    // const MEASURED_MODULATED_S:   f64 = TODO;
    // Assert: (1.0 - MEASURED_MODULATED_S / MEASURED_UNMODULATED_S) >= 0.20;
    // Assert: kinematic predictions are within ±15% of measurements.
    todo!("populate measured constants from real-machine wall-clock runs");
}
```

## Files

- `crates/rs_cam_core/tests/adaptive_feed_modulation_cycle_calibration_f036c.rs` — new
- (No production code changes — this finding is verification-only.)

## Risk

Low. The test is `#[ignore]`d until measurements land, so it can't fail CI. The only risk is forgetting to un-`#[ignore]` after measurement; mitigate by leaving a TODO breadcrumb in `planning/feed_modulation_roadmap.md`.

## Notes

- **User input required**: the implementer (Claude) cannot drive a CNC machine. This finding lands with the test scaffolded but `#[ignore]`d, and the workstream-roadmap notes that a wall-clock measurement on the user's Shapeoko XXL is the gating step.
- If the measured reduction is < 20%, two suspects: (1) the modulation algorithm is too conservative (chip-thinning correction too strong, predicted-feed cap too tight), or (2) the Shapeoko's GRBL planner buffer can't ingest per-move F-words at speed (controller-buffer issue). Open a follow-up finding to triage.
- F-034 has a parallel `#[ignore]`d calibration test (`shapeoko_xxl_cycle_time_calibration`); both should land measurements in the same session if possible.
