# F-036a — G-code emission of per-move F-words (deferred from F-036)

- **Stage:** post-processor (G-code emitter)
- **Severity:** medium (blocks F-036 reaching production)
- **Status:** open — deferred from F-036's Piece C
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
