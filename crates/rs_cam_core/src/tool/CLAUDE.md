# `tool/` — cutter geometry

One module per cutter family. The trait and the shared radius queries live in
`mod.rs`.

## Files

- `mod.rs` — the cutter trait, `cusp_radius`, `cusp_radius_mm` and
  `valley_radius_mm`, `edge_drop_by_profile`, `helix_lag_rad` (one helix lag for the flutes in cut and the P11a engaged-edge force), `DEFAULT_HELIX_DEG` (P13a: the one helix of a cutter that states none; every constructor and `default_helix_deg` read it; F201 row `tool.DEFAULT_HELIX_DEG`).
- `flat.rs` (`FlatEndmill`), `ball.rs` (`BallEndmill`), `bullnose.rs` (`BullNoseEndmill`, toroidal), `tapered_ball.rs` (`TaperedBallEndmill`), `vbit.rs` (`VBitEndmill`).
- `size_units.rs` — `SizeUnits` (mm or inch), inch fractions, the length entry parser; display only (lengths stay mm).
- `body.rs` — P22-0c `ToolBody`: the axial zones the tool owns (`zones()` is THE table; one `pub(crate)` constructor, called by `ToolDefinition`); `MountedBody` = body + head bands (`ToolDefinition::mounted`, THE constructor; `zones()`, `first_above_flutes()`; `head_zone` maps a band; `ZoneKind` names a collision part, P22-0c-5: `ToolAssembly`, `AssemblySegment` are gone). P22-0c-2: `CoreZone` (`ToolBody::core`) owns every core rule (plunge, straight line, zigzag, helix, the dwell rule `clamp_descents` / `max_ring_slope`); `cut_table`, `envelope_table`, `core_of` for a `&dyn MillingCutter` (`MillingCutter::tool_body`). P22-0c-6a: `upper_diameter_mm` (free function and view) is THE rule "the shank, else the envelope"; the size label and the shank section of `tip_deflection_mm` read it (search guard `shank_diameter_planning_mm() > 0.0` in `one_body_for_every_reader_p22_0c`). `max_cut_diameter_mm` = `2 x min(width(L), envelope)`, the cutting cap (ruling 1); 6b: every feeds door of a tapered ball clamps at it (`feeds::CutBody`, `EngagedDiameter::of_tool`), and the plunge ring reads the core (`feeds::section`).
- `domain.rs` — `ToolDimension::domain` (math bounds), `ui_range` (GUI drag only), `ToolGeometryError`; centre-core and blade-protrusion measurement bounds. A shank is `> 0` (P22-0c-6 ruling 6: a stated 0 refuses; an unknown shank is `Unknown`); its drag range starts at `ToolDomain::lowest_on_grid(SHANK_DRAG_STEP_MM)` (6b).

## Invariants

- The cusp radius is what the tool FORMS. The valley radius is what the tool
  FITS INTO. The two coincide on a taper and diverge on a bull nose. Pick the
  one the caller's question asks for.
- A holder and shank envelope is part of the tool, not of the operation; collision reads it. G-CENTRECUT: the body core (`ToolBody::core`, `tool::body::core_of`) gives the tool's limits; only `ToolDefinition` holds a body (`with_non_centre_cutting`). Only `CoreZone::from_geometry` halves the core diameter (search guard in `one_body_for_every_reader_p22_0c`). K3: only a `CentreCutting::No(..)` fact gives a core (`ToolConfig::non_centre_cutting_geometry`); `Yes` and `Unknown` give none, so an `Unknown` tool keeps the motion of a centre-cutting tool. Every reader asks the tool; nothing copies the limits. K5 VTipFlat (P7): `build_cutter` and `ToolConfig::geometry_hint` set the V tip from `v_tip_planning_mm` (the tip `planning()` gives, else 0: sharp); a stated tip whose cone height disagrees with the flute warns `tool.v_tip_unmodelled` (a V-bit only; D12: an Unknown or estimated tip, or centre cutting, stops generation: `compute::not_set::tool_facts_not_set`).
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
- `... --test tool_geometry_hygiene --test edge_drop_matches_the_sampled_profile_g_tierburial --test one_body_for_every_reader_p22_0c --test a_turning_descent_holds_the_core_dwell_rule_p22_0c`
- `cargo test -p rs_cam_viz -q --test a_tool_size_reads_diameter_and_radius_g_toolsize`

## Do not

- Do not use the cusp radius as a reach answer. The reach map is in `maps/`.
