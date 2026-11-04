# `tool/` — cutter geometry

One module per cutter family. The trait and the shared radius queries live in
`mod.rs`.

## Files

- `mod.rs` — the cutter trait, `cusp_radius`, `cusp_radius_mm` and
  `valley_radius_mm`, `edge_drop_by_profile`.
- `flat.rs` (`FlatEndmill`), `ball.rs` (`BallEndmill`), `bullnose.rs` (`BullNoseEndmill`, toroidal), `tapered_ball.rs` (`TaperedBallEndmill`), `vbit.rs` (`VBitEndmill`).
- `size_units.rs` — `SizeUnits` (mm or inch), inch fractions and the
  length entry parser. Display and entry only: every length stays in mm.
- `domain.rs` — `ToolDimension::domain` (math bounds), `ui_range` (GUI drag only), `ToolGeometryError`; centre-core and blade-protrusion measurement bounds.

## Invariants

- The cusp radius is what the tool FORMS. The valley radius is what the tool
  FITS INTO. The two coincide on a taper and diverge on a bull nose. Pick the
  one the caller's question asks for.
- A holder and shank envelope is part of the tool, not of the operation; collision reads it. G-CENTRECUT: `MillingCutter::non_centre_cutting` gives the tool's limits; only `ToolDefinition` holds a value (`with_non_centre_cutting`). K3: only a `CentreCutting::No(..)` fact gives limits (`ToolConfig::non_centre_cutting_limits`); `Yes` and `Unknown` give none, so an `Unknown` tool keeps the motion of a centre-cutting tool. Every reader asks the tool; nothing copies the limits. K5 VTipFlat (P7): `build_cutter` and `ToolConfig::geometry_hint` set the V tip from `v_tip_planning_mm` (the tip `planning()` gives, else 0: sharp); a stated tip whose cone height disagrees with the flute warns `tool.v_tip_unmodelled` (a V-bit only; D12: an Unknown or estimated tip, or centre cutting, stops generation: `compute::not_set::tool_facts_not_set`).
- An edge drop is the highest point of `z - height(rho)` over the edge
  (`mod.rs::edge_drop_by_profile`); a `facet_drop` reports `found` only for
  a tangency, or `drop_cutter` skips the edges (G-TIERBURIAL).
- F029: a cutter constructor returns `Result`, never asserts; every tool door calls `ToolConfig::validate` (`domain`; P7 A2: a V-bit tip that `planning()` gives is in `0 <= tip < diameter`, `ToolDimension::VTipDiameter`); `ui_range` refuses nothing (F210).
  FC0 (D4, ruling Q6): a stick-out shorter than the flute is valid; the load and `tool.stickout_shorter_than_flute` warn.
- A person reads a tool size through `ToolConfig::size_label`
  (`compute/tool_config.rs`): `Ø` is a diameter, `R` a radius, in the
  tool's `effective_size_units`. G-code comments are the exception: their
  bytes stay as they are.

## Sentries

- `cargo test -p rs_cam_core -q --test tool_scale_semantics_pr2 --test bull_nose_cusp_radius_g_bullcusp --test tapered_cusp_radius_sentry`
- `... --test a_tool_outside_its_domain_is_refused_at_every_door_f029` (F029)
- `... --test tool_geometry_hygiene --test edge_drop_matches_the_sampled_profile_g_tierburial`
- `cargo test -p rs_cam_viz -q --test a_tool_size_reads_diameter_and_radius_g_toolsize`

## Do not

- Do not use the cusp radius as a reach answer. The reach map is in `maps/`.
