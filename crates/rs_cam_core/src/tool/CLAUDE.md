# `tool/` — cutter geometry

One module per cutter family. The trait and the shared radius queries live in
`mod.rs`.

## Files

- `mod.rs` — the cutter trait, `cusp_radius`, `cusp_radius_mm` and
  `valley_radius_mm`.
- `flat.rs` — the flat end mill (`CylCutter`).
- `ball.rs` — the ball end mill (`BallCutter`).
- `bullnose.rs` — the bull nose, that is the toroidal cutter (`BullCutter`).
- `tapered_ball.rs` — the tapered ball (`BallConeCutter`).
- `vbit.rs` — the V-bit, that is the cone cutter.

## Invariants

- The cusp radius is what the tool FORMS. The valley radius is what the tool
  FITS INTO. The two coincide on a taper and diverge on a bull nose. Pick the
  one the caller's question asks for.
- A holder and shank envelope is part of the tool, not of the operation.
  Collision reads the envelope.
- A person reads a tool size through `ToolConfig::size_label`
  (`compute/tool_config.rs`): `Ø` is a diameter, `R` a radius. G-code
  comments are the exception: their bytes stay as they are.

## Sentries

- `cargo test -p rs_cam_core -q --test tool_scale_semantics_pr2`
- `cargo test -p rs_cam_core -q --test bull_nose_cusp_radius_g_bullcusp`
- `cargo test -p rs_cam_core -q --test tapered_cusp_radius_sentry`
- `cargo test -p rs_cam_core -q --test tool_geometry_hygiene`
- `cargo test -p rs_cam_viz -q --test a_tool_size_reads_diameter_and_radius_g_toolsize`

## Do not

- Do not use the cusp radius as a reach answer. The reach map is in `maps/`.
