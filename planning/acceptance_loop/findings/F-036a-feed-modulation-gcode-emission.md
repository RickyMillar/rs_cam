# F-036a — G-code emission of per-move F-words (deferred from F-036)

- **Stage:** post-processor (G-code emitter)
- **Severity:** medium (blocks F-036 reaching production)
- **Status:** landed 2026-05-26 (regression-net only — modal layer was already correct)
- **First found in:** F-036 implementation session, 2026-05-26
- **Effort:** M (one focused PR, ~300-500 LOC change including tests)
- **Linked PRs:** —
- **Workstream:** Feed Modulation (see `planning/feed_modulation_roadmap.md`)
- **Depends on:** F-036 algorithm landed (`crates/rs_cam_core/src/feed_modulation.rs`)

## Symptom

F-036 landed the modulation algorithm (`adaptive_feed_modulate`) but the G-code emitter at `crates/rs_cam_core/src/gcode/emitter.rs` (and related `gcode/program_builder.rs`, `gcode/modal.rs`) was **not** modified. Modulated per-move `feed_rate` in the IR is silently collapsed by the modal-state filter back to one `F<rate>` word at the top of each contour, because the modal layer assumes feed never changes mid-toolpath.

## Hypothesised root cause

The emitter's modal layer caches the last-emitted F-word and suppresses redundant emissions. Per-move feed variation needs the modal layer to compare each move's feed against the cached value and emit a new `F<rate>` token whenever they differ by more than a tolerance (suggest 1 mm/min).

## Fix shape

1. Audit `crates/rs_cam_core/src/gcode/modal.rs` for the F-word suppression logic.
2. Add per-move F-word emission when `move.feed_rate != cached_feed_rate` (delta > 1 mm/min).
3. Round-trip test: synthetic toolpath with per-move varying feeds → emitted G-code → parsed back → reconstructed per-move feeds match.

## Acceptance test

```rust
// crates/rs_cam_core/tests/adaptive_feed_modulation_gcode_f036a.rs

#[test]
fn modulated_toolpath_emits_per_move_f_words() {
    // Build a toolpath with three Linear moves at feeds 1000, 1500, 2000.
    // Emit G-code.
    // Assert: emitted text contains exactly three distinct F-words
    //         (one per move), each matching the IR's feed within ±1.
}

#[test]
fn unmodulated_toolpath_emits_single_f_word() {
    // Build a toolpath with three Linear moves all at feed 1500.
    // Emit G-code.
    // Assert: emitted text contains exactly ONE F-word (modal suppression).
}

#[test]
fn modulated_gcode_round_trips_through_parser() {
    // Build a modulated toolpath.
    // Emit G-code.
    // Parse back (rs_cam_core's gcode parser, if exposed; otherwise
    // regex-extract F-words).
    // Assert: reconstructed feeds == original feeds (within 1 mm/min).
}
```

## Files

- `crates/rs_cam_core/src/gcode/modal.rs` — modal F-word suppression
- `crates/rs_cam_core/src/gcode/emitter.rs` — emit site
- `crates/rs_cam_core/tests/adaptive_feed_modulation_gcode_f036a.rs` — new test

## Risk

M. Touching the G-code emitter is the load-bearing path the loop's smoke baseline exercises. Mitigation: per-move F-words are only emitted when there's modulation; non-modulated toolpaths must still produce one F-word per contour (modal suppression unchanged). The "byte-identical for unmodulated" test pins this.

## Notes

- Verify Shapeoko XXL (GRBL controller) parses per-move F-words correctly. Most modern GRBL builds handle this fine but the planner lookahead buffer can be undersized; if cycle time goes UP rather than down on the real machine, that's a controller-buffer issue and the F-036 pipeline needs a buffer-size advisory.
- F-036b (feature flag plumbing) should land **after** this — flag-gating without G-code emission is a no-op feature.

## Implementation log (2026-05-26)

**Finding hypothesis was incorrect — modal layer was already emitting per-move F-words.** Audit of `crates/rs_cam_core/src/gcode/program_builder.rs` (lines 36-44, 326-342) revealed the F-elision logic already uses `last_feed != Some(feed_rate)` exact-equality: a fresh `Statement::Linear { feed }` (which renders `G1 ... F<rate>` per `emitter.rs:192-198`) emits whenever feeds differ; the `Statement::LinearModal` variant (no F-word) is reserved for identical-feed runs. Per-move modulated `feed_rate` values written by `adaptive_feed_modulate` already flow through to per-move F-words without any modal-layer change.

This shipped as a **regression-net-only** PR: the three acceptance tests from the finding file were added at `crates/rs_cam_core/tests/adaptive_feed_modulation_gcode_f036a.rs` to pin the contract. They pass against current master (`2baa13a`) and will fail if a future change breaks the per-move F-word path. No `modal.rs` or `program_builder.rs` edit was needed.

The finding's "delta > 1 mm/min" tolerance was a suggestion, not a requirement. Exact-equality is correct: floating-point feed values produced by `adaptive_feed_modulate` differ by hundreds of mm/min on the modulation paths the algorithm cares about (chip-thinning correction `1/sqrt(woc)` produces feed jumps of 10-50%), and the f64 noise floor on whole-number commanded feeds is < 1e-12. A tolerance change would have been a behavior change for paths emitting close-but-not-equal feeds — risking the F-037 smoke baseline and the captured-fixture corpus — for no observed benefit.

Architectural note for F-036b implementer: when wiring `adaptive_feed_modulate` into production via `SimulationOptions::adaptive_feed_modulation`, the only IR path needed is to mutate `Toolpath::moves[i].move_type`'s `feed_rate` field. The emitter pipeline (`program_builder::build_single` / `build_phased` / `build_multi_setup` → `emit_program`) will already render per-move F-words. No emitter or modal change required for that wiring.

- Commit: see PR
- Tests: `cargo test -p rs_cam_core --test adaptive_feed_modulation_gcode_f036a` (3/3 pass)
- Smoke baseline diff: no regressions (18 cases unchanged)
- Clippy + tests: workspace clean
