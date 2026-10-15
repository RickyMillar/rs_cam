# `gcode/` — G-code generation and post-processors

Turns a `Toolpath` into bytes. The builders are `program_builder::build_single`,
`build_phased` and `build_multi_setup` (the last two with a `_with` variant); then
`gcode::emitter::emit_program`. B18: a snippet resets the builder position; the next rapid is
Z-first, and in a setup with an obstacle the toolpath starts at the plane (`a_snippet_join_rises_to_the_plane_b18`).

## Files

- `mod.rs` — the facade and the export configuration.
- `ir.rs` — the G-code program intermediate representation.
- `program_builder.rs` — builds a `Program` from `Toolpath` inputs.
- `modal.rs` — the modal state the builder tracks.
- `emitter.rs` — renders a `Program` to bytes with a `PostDefinition`.
- `post.rs` — the data-driven post-processor definition.
- `wizard_overlay.rs` — the per-export overrides the wizard supplies.

## Invariants

- The datum decides Z zero. `StockTop` maps to Z equal to zero. A flipped
  setup zeroes to the presented top. Export consumes the datum in the setup;
  do not re-derive it here.
- A post-processor is data, not code. Add a `PostDefinition` row, not a
  branch in the emitter.
- W2.5: every session door fills `GcodePhase::feeds` from the slot it emits
  (`FeedStage::statement`). The line follows the label. It states, never refuses.

## Sentries

- `cargo test -p rs_cam_core -q --test gcode_phase0_capture`
- `cargo test -p rs_cam_core -q --test post_format_round_trip_p1`
- `cargo test -p rs_cam_core -q --test export_honors_coolant_p0d1`
- `cargo test -p rs_cam_core -q --test export_datum_setup_frame`
- `cargo test -p rs_cam_core -q --test gcode_validator_baseline`

## Do not

- Do not emit a retract plane from a local Z frame. `origin_z` above the
  stock top puts the plane inside the stock (G-SAFEZ-LOCAL).
