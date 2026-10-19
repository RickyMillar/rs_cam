# Duplication of code and knowledge — findings (2026-10-19)

Evidence baseline 99b6fb41, branch `geo-select`. Read-only review. Every
claim below comes from a file read or an `rg` result. A vector pair from
the sweep is a candidate only; each row below names both sides after a read.

## Instrument

- `instruments/dup_sweep_0.88_src.md`: 15 123 chunks, 45 clusters,
  168 src pairs, 1 449 test pairs.
- `instruments/dup_sweep_0.93_all.md`: 14 683 chunks, 68 clusters,
  13 src pairs, 388 test pairs.
- Previous runs: `tech_debt_2026-09-16/evidence/dup_sweep_0.88_src.md`
  (10 808 chunks, 31 clusters, 137 src pairs, 1 017 test pairs) and
  `duplicate_sweep_2026-09-15/rerun_2026-09-16.md` (threshold 0.92,
  10 808 chunks, 47 clusters, 14 src pairs, 413 test pairs).
- The index grew by 40 % (10 808 to 15 123 chunks). The src pair count grew
  by 23 % (137 to 168). The pair density per chunk therefore went down.
- The two new runs report different chunk totals (15 123 and 14 683) on the
  same index. I did not check the cause in `scripts/duplicate_sweep.py`; the
  counts of the two runs are therefore not directly comparable.

Vector pairs that I read and dropped (false positives): `cycle_time.rs` /
`readiness.rs` (a wrapper), `generation_plan::Step` / `PlanStep` (an
exhaustive adapter), `build_sim_request` (already one shared function),
`XyTerritory` / `TerritoryXy` (a rule enum and its resolved data), CLI
`ToolpathDiagnostic` (C3 exhaustive destructure, `rs_cam_cli/src/project.rs:51-110`;
this is the proof pattern the fixes below copy), MCP `From<XParam>` impls
(the documented wire pattern of `rs_cam_mcp`), `memory_budget.rs` (delegates
to core).

---

## DUP-01 (A) The MCP project verdict omits holder collisions; the ladder is written twice

- `crates/rs_cam_viz/src/controller/events/compute.rs:2494-2507` (MCP
  `get_diagnostics`, `build_mcp_diagnostics`) has four arms: UNKNOWN (failed
  checks), WARNING (rapid collisions), WARNING (air > 40 %), OK.
- `crates/rs_cam_cli/src/project.rs:964-988` has five arms. The second arm is
  `ERROR: {n} holder/shank collisions detected` on `total_collision_count > 0`.
- The MCP comment at `compute.rs:2484-2492` says it uses "the same order and the
  same word the CLI uses". It does not.
- Failure: a project with holder collisions, no rapid collision and low air cut
  gives `"verdict": "OK"` on MCP. The same response carries
  `collision_count > 0` two keys later (`compute.rs:2518`). An agent that reads
  the verdict reads a clean bill on a safety tally.
- The same handler is a hand-built `json!{}` beside the core
  `ProjectDiagnostics` derive (cold-start task 4 silent miss). It copies the
  two `pub(crate)` reason literals of
  `crates/rs_cam_core/src/session/diagnostics_types.rs:334,345` at
  `compute.rs:2467-2473`.
- The 40 % air-cut bar is typed four times:
  `crates/rs_cam_core/src/compute/catalog/capabilities.rs:362`,
  `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:341`,
  `crates/rs_cam_cli/src/project.rs:981`, `compute.rs:2503`.
- Fix (TYPES): put `ProjectVerdict` (an enum) and `ProjectDiagnostics::verdict()`
  in core. Both surfaces render the enum. Make the reason constants `pub`, or
  move the field to an enum. Read `AIR_CUT_BAR_CLEARING_PCT` everywhere.
- Cost S. Not in flux. Known: no.

## DUP-02 (B) Two G-code export pipelines; only the GUI one rewrites rapids and runs the safety pass

- GUI and MCP export: `crates/rs_cam_viz/src/io/export.rs:597-655`. It runs
  `machine_safety_pass` (`:104-129`, the only production caller of
  `validate_machine_safety`) and, when `gui.post.high_feedrate_mode`,
  `replace_rapids_with_feed` (`:643`, `:942`, `:1028`).
- CLI export: `crates/rs_cam_cli/src/run.rs:216` and
  `crates/rs_cam_cli/src/project.rs:1051` call
  `crates/rs_cam_core/src/gcode/mod.rs:376` `export_gcode_checked`. That function
  has no rapid rewrite and no machine-safety pass. `rg high_feedrate
  crates/rs_cam_cli` gives zero hits.
- Failure: one project with `high_feedrate_mode = true` emits G1 traverses from
  the GUI and G0 rapids from the CLI. The CLI file is never checked for a
  rapid through the part.
- The rule `crates/rs_cam_cli/CLAUDE.md` "Generated G-code must use the core
  export path" holds for the CLI. The GUI is the surface that left the core
  path.
- Fix (TYPES): `export_gcode_checked` returns an `ExportedGcode { gcode,
  machine_safety, .. }` and applies the rewrite itself. The viz door wraps it.
- Cost M. In flux (`gcode/mod.rs`, `session/compute/export.rs`, `run.rs`).

## DUP-03 (B) GUI pre-generation rules exist only in the GUI, and twice there

- `crates/rs_cam_viz/src/ui/properties/operations/validate.rs:85-117`
  (`validate_toolpath_config`, the submit gate at
  `crates/rs_cam_viz/src/controller/events/compute.rs:233`) and `:174-245`
  (`validate_geometry_selection`) hold the same model-geometry check. The text
  already differs: `:209` adds "(use Surface selector for separate mesh)".
- Core has no such check: `crates/rs_cam_core/src/diagnostics/adapters/from_model_refs.rs`
  tests only `REF_MODEL_MISSING`.
- Stepover: GUI `registry.rs:421-433` refuses `stepover >= diameter` for Pocket
  and Adaptive only, and the refusal blocks the submit. Core
  `from_static_checks.rs:246-285` flags `stepover > diameter` for every op with
  a stepover, as a diagnostic.
- Failure: stepover equal to the diameter. The GUI and MCP refuse to generate.
  The CLI generates. The core diagnostic is silent.
- Fix (TYPES): move both rules into `diagnose_toolpath_inputs`. The GUI gate
  reads the core `Diagnostic` list at `Severity::Critical`.
- Cost M. In flux (`validate.rs`, `registry.rs`).

## DUP-04 (B) The CLI job builds two different tools from one `ToolDef`

- `crates/rs_cam_cli/src/job.rs:854-895` `build_tool`: cutting length 4 x D,
  stickout default 4 x D, shank default D, `ToolMaterial::Carbide`, no
  `corner_radius_mm`.
- `crates/rs_cam_cli/src/job.rs:1053-1083` `tool_config_from_def`: starts from
  `ToolConfig::new_default` (stickout 45.0, shank 6.35,
  `crates/rs_cam_core/src/compute/tool_config.rs:215-222`) and then core
  `build_cutter` (`crates/rs_cam_core/src/compute/cutter.rs:8-51`).
- Failure: a 3 mm tool with no `stickout` key. The session tool
  (`job.rs:1129`) has stickout 45 mm and shank 6.35 mm. The phase cutter
  (`job.rs:1008`) has stickout 12 mm and shank 3 mm. One run carries two
  cantilevers for the deflection inputs.
- Rule broken: `crates/rs_cam_cli/CLAUDE.md` "Do not add CLI-only semantics that
  drift". Cold-start task 6 found the second builder.
- Fix (TYPES): delete `build_tool`. The phase cutter is
  `build_cutter(&tool_config_from_def(..))`.
- Cost S. In flux (`job.rs`).

## DUP-05 (B) The tool-load gate context is assembled by hand on two surfaces

- Export: `crates/rs_cam_core/src/gcode/mod.rs:740-790` builds
  `ToolpathLoadContext`. GUI cut-metric cards:
  `crates/rs_cam_viz/src/state/simulation.rs:300-330` builds it again.
- `PassRole -> LutPassRole` is matched by hand four times:
  `gcode/mod.rs:752-756`, `crates/rs_cam_core/src/tool_load/mod.rs:309-313`,
  `crates/rs_cam_core/src/feeds/vendor_normalize.rs:215`,
  `crates/rs_cam_viz/src/state/simulation.rs:464-472`.
- `gcode/mod.rs:742-751` is an inline copy of
  `feeds/vendor_normalize.rs:152` `op_family_to_lut`, whose doc says it exists so
  that call sites "share one mapping".
- Cold-start task 4: a new number from the load context "is silently derived
  twice, once in the GUI card and once in MCP".
- Fix (TYPES): `ProjectSession::tool_load_context(index) -> Option<OwnedLoadContext>`
  in core; `LutPassRole::from(PassRole)`.
- Cost S. In flux (`gcode/mod.rs`).

## DUP-06 (D) Two simulation-group builders, held by a doc list of seven shared decisions

- Core: `crates/rs_cam_core/src/session/compute/simulation.rs:84-230`
  `simulation_groups`. GUI: `crates/rs_cam_viz/src/controller/events/simulation.rs:158-330`
  `build_simulation_groups`. A third phantom walker:
  `crates/rs_cam_viz/src/ui/readiness.rs:142`.
- The rule is DOC: `crates/rs_cam_core/src/compute/CLAUDE.md:13` "Put an eighth
  there, not in one builder".
- Past breaks recorded in the comments: G-RESTRES parity (disabled row,
  `simulation.rs:126-130`), G-STICKYEMPTY and G-RESTLEVELS (`:105-112`),
  G-RESTSTALE (GUI `:200-204`).
- The rapid feed of the request is resolved four times:
  `crates/rs_cam_core/src/session/compute.rs:2184-2188` (no `.max(1.0)` on
  `high_feedrate`), `session/compute/diagnostics.rs:613-617`,
  `session/compute/simulation.rs:521-525`, and the GUI
  `controller/events/simulation.rs:396-400`, which reads the `gui.post` mirror
  (DUP-07).
- Fix (TYPES): core `ProjectSession::simulation_request(scope)`; the GUI passes
  a scope and the viz-only semantic trace. One `PostConfig::rapid_feed(&machine)`.
- Cost L. In flux (`session/compute.rs`, `session/compute/simulation.rs`,
  `session/compute/diagnostics.rs`, `compute/simulate.rs`). Known: D11 residue
  (G-MCPSIMMIRROR / WP28 holds the flow change).

## DUP-07 (D) `GuiState::post` is a second copy of the post config, and export reads the copy

- `crates/rs_cam_viz/src/io/export.rs:604,632,642,730,886,941` and
  `io/setup_sheet.rs:409-411` read `gui.post`. MCP reads
  `session.post_config()` (`app/mcp.rs:811`, `app/mcp/commands.rs:761,773`).
- `crates/rs_cam_viz/src/controller/events/undo.rs:20-25,92-97` write the mirror
  first and then `let _ = self.apply_quietly(..)`. A refused command leaves the
  mirror and the session different, and the GUI export then emits the mirror.
- Rule: `crates/rs_cam_viz/src/controller/CLAUDE.md:30-32` says the mirror is a
  clone of the session that `refresh_post_mirror` rebuilds.
- Fix (TYPES): delete the field. Readers call `session.post_config()`. Undo
  applies the command and refreshes nothing.
- Cost M. In flux (`undo.rs`).

## DUP-08 (D) Engine `Params::default()` and op `Config::default()` are two default tables, and they disagree

- Pencil: `crates/rs_cam_core/src/finish/pencil.rs:227-243` (`num_offset_passes 0`,
  feed 1000, plunge 500) vs `crates/rs_cam_core/src/compute/operation_configs.rs:1517-1528`
  (1, 800, 400).
- Project Curve: `crates/rs_cam_core/src/ops/project_curve.rs:125-139` (feed 1000,
  plunge 300) vs `operation_configs.rs:2704-2718` (800, 400).
- Ramp Finish: `crates/rs_cam_core/src/finish/ramp_finish.rs:228-243`
  (`max_stepdown 1.0`, `sampling 1.0`) vs `operation_configs.rs:2329-2345`
  (0.5, 0.5).
- 22 `impl Default for *Params` exist in core. `UnifiedFinishParams`
  (`unified_finish.rs:255-290`) holds "lockstep" comments; it agrees today.
- Effect: an integration test that builds `XParams::default()` measures the
  engine at values no product surface ships.
- Fix (TYPES): delete the engine `Default`s. Tests build params through the
  adapter from `XConfig::default()`.
- Cost M. In flux (`operation_configs.rs`). Map lead core-finish #2.

## DUP-09 (D) Display-label tables re-typed per surface, with drift

- `ToolFamily`: `crates/rs_cam_core/src/feeds/vendor_lut.rs:147-156`
  `ToolFamily::label`; `crates/rs_cam_core/src/feeds/extrapolation/plunge.rs:335-344`
  `tool_family_label` (byte-identical copy, 8 callers);
  `crates/rs_cam_viz/src/ui/feeds/shared.rs:90-99` (Title Case, "Tapered ball").
- `MaterialFamily`: `vendor_lut.rs:160-176` ("aluminium", "fibreglass") vs
  `ui/feeds/shared.rs:101-117` ("Aluminum", "Fiberglass").
- `flutes_label` (`feeds/plunge.rs:248-254`) and `flutes_text`
  (`feeds/extrapolation/plunge.rs:347-353`) are byte-identical.
- `SpanKind`: core `crates/rs_cam_core/src/trace/toolpath_spans.rs:274` (C30 says
  "the one home") vs `crates/rs_cam_viz/src/ui/sim_op_list.rs:829-842`
  ("Depth pass", "Lead-out").
- One span has two names on screen: `sim_op_list.rs:792-812` prints
  `Pass {pass_index + 1} @ Z {:.3}`; `sim_diagnostics.rs:2252-2258` and
  `sim_timeline.rs:978-1003` print `DepthPass {pass_index} · z={:.2}`.
- `SimulationIssueKind`: `sim_op_list.rs:1249-1258` and
  `sim_diagnostics.rs:1000-1009`.
- Fix (TYPES): one `label()` / `display_label()` per enum in core; viz calls it.
  Delete `tool_family_label` and one flutes helper.
- Cost S. Not in flux.

## DUP-10 (D) Cutter geometry implemented twice, held by a parity sentry

- `crates/rs_cam_core/src/feeds/mod.rs:145-195` `ToolGeometryHint::engaged_diameter_at_doc`
  states "a second, hand-maintained implementation" of
  `MillingCutter::lookup_diameter_at` (`crates/rs_cam_core/src/tool/tapered_ball.rs:165-178`,
  `tool/vbit.rs:125`).
- The ball-cap width `sqrt(2Rh - h^2)` is typed in `tool/ball.rs`,
  `tool/tapered_ball.rs:180-200`, `tool/bullnose.rs` and
  `feeds/geometry.rs` `ball_effective_diameter`.
- Enforcement: SENTRY `feeds::tests::engaged_diameter_at_doc_matches_lookup_diameter_at_across_shapes`.
- Fix (TYPES): free functions in `tool/` (`tapered_ball_engaged_radius(tip, alpha, doc)`,
  `vbit_width_at_depth`) that both the cutter impls and the hint call.
- Cost S. Not in flux.

## DUP-11 (D) Geometric and statistical primitives copied per module, with different edge rules

- Point-to-segment distance: owner `crates/rs_cam_core/src/geo.rs:381,393`. Private
  copies: `compute/execute/standing_material.rs:764` (eps 0.0),
  `compute/alignment_pins/walls.rs:181` (0.0, tuples), `dressup/link.rs:49` (1e-12),
  `polygon.rs:1946` (f64::EPSILON), `adaptive/search.rs:843` (1e-20),
  `finish/conformal_spiral.rs:2485` (EPS_VEC 1e-12). `geo.rs` itself uses 1e-18
  and 1e-20.
- Nearest-rank quantile `round((len-1) q)`: `metrology/spacing.rs:211` (empty: NaN),
  `tool_load/distribution.rs:301` (None), `maps/reach_map.rs:1375` (0.0),
  `finish/conformal_spiral.rs:1887` (0.0).
- Median: `metrology/monge.rs:527` averages the two middle values;
  `surface/rest_field.rs:1370` and `finish/conformal_spiral.rs:1948` take the upper one.
- Also `adaptive3d/clearing.rs:1760` and `dressup/tsp.rs:482` `nearest_neighbor_order`;
  `adaptive3d/clearing.rs:1787` `polygon_signed_area` vs `polygon.rs:221`.
- Rule (DOC): `crates/rs_cam_core/src/metrology/CLAUDE.md:24-26` "A geometric
  primitive belongs to `geo.rs`".
- Fix: `geo::point_to_segment_distance_xy` and a `stats` module
  (`quantile_nearest_rank -> Option<f64>`, `median`). Delete the copies.
- Cost M. In flux (`polygon.rs`). Known: D10 (sibling pair, no action); six new
  copies since.

## DUP-12 (D) `SweptDispatch` is the third band dispatcher; STK-09 merged only two

- `crates/rs_cam_core/src/dexel_stock/band_batch.rs:1-8,117-200` holds the shared
  `BandBatch<J, P>` for `whole_path.rs:278` and `playback.rs:226`.
- `crates/rs_cam_core/src/dexel_stock/swept.rs:798-1000` keeps its own driver:
  `batch_is_due` (`:920`), `build_buckets` (`:934`), `run_batch` (`:991`) and four
  constants "sized identically" / "same dial" to `whole_path.rs:219-224`.
- `swept` is the `Auto` route (`swept.rs:819-830`), so the default simulation
  runs the unshared copy.
- Fix: extend `BandJob` with a bin count, or make `BandBatch` generic over a
  per-job bin span. One-paired A/B is mandatory (hot loop).
- Cost L. Not in flux. Known: D14 (closed for two of three copies by STK-09).

## DUP-13 (D) The 3D Rough planner re-states the emitter's descent rule by hand

- `crates/rs_cam_core/src/adaptive3d/clearing.rs:672-700` says "Mirror
  segments_to_toolpath's `descent_floor` calc exactly" and re-types
  `RAPID_DESCENT_BUFFER_MM = 0.5` and the drape call.
- The emitter: `crates/rs_cam_core/src/adaptive3d/path.rs:1500-1512`, with the
  same constant and a different `None` arm (`map_or(params.safe_z, ..)`).
- Effect: the planner's stock model stamps a descent the emitter may not cut,
  if one side moves.
- Fix (TYPES): one `fn entry_descent_start(entry, rapid_floor, safe_z) -> P3` in
  `adaptive3d/path.rs`, called by both.
- Cost S. Not in flux.

## DUP-14 (C) Provenance `tool_hashes` is written and never read; its recipe hashes no shape

- Written: `crates/rs_cam_core/src/compute/simulate.rs:983-1000`. Field:
  `crates/rs_cam_core/src/stock/simulation_cut.rs:131` (serialised with the
  trace). `rg tool_hashes` finds no reader.
- `crates/rs_cam_core/src/compute/sim_prefix.rs:119-123` records that the recipe
  hashes no cutter shape, so a 6 mm flat and a 6 mm ball hash equal.
  `sim_prefix.rs:405` `hash_tool` is the full recipe.
- Further hash copies: `hash_f64` (`simulate.rs:921`) vs `hash_f64_bits`
  (`sim_prefix.rs:380`); hand FNV-1a in `compute/source_stock.rs:149-160` and
  `compute/toolpath_stats.rs:581-590`; two move walkers `hash_toolpath`
  (`simulate.rs:948`) and `carve_digest` (`source_stock.rs:149`).
- Fix: delete `tool_hashes`, or fill it with `hash_tool` and read it in
  `sim_trace_is_fresh_for`. One `digest` module with `Fnv1a` and `hash_f64`.
- Cost S. In flux (`simulate.rs`, `toolpath_stats.rs`).

## DUP-15 (D) `UnmodeledReason` has four renderers (T-20, unchanged)

- `crates/rs_cam_core/src/diagnostics/adapters/from_tool_load.rs:998-1046`
  (10 arms), `crates/rs_cam_viz/src/ui/preflight.rs:602-619` (10 arms),
  `crates/rs_cam_viz/src/ui/sim_diagnostics.rs` (4 arms),
  `crates/rs_cam_core/src/gcode/mod.rs` (3 arms).
- The wordings differ ("run simulation to evaluate" vs "run simulation first").
- Fix (TYPES): `UnmodeledReason::operator_text()` and `::state()` in
  `tool_load/verdict.rs`.
- Cost S. In flux (`gcode/mod.rs`). Known: T-20 (open; no change since 2026-09-18).

## DUP-16 (D) Project-file sections duplicate model structs and their defaults

- `crates/rs_cam_core/src/session/project_file.rs:466-513` `ProjectStockConfig`
  repeats `crates/rs_cam_core/src/compute/stock_config.rs:68-115` `StockConfig`
  field by field, with three default fns (`:515-523`).
- `project_file.rs:533-610` `ProjectToolSection` keeps `tool_type: String` and nine
  default fns that repeat `ToolConfig::new_default` numbers
  (`compute/tool_config.rs:201-230`); `default_helix_deg` exists in both files.
- `crates/rs_cam_core/src/session/save.rs:16-25` `tool_type_to_key` repeats
  `ToolType::serde_token` (`tool_config.rs:63-72`).
- Cold-start task 5: a new `StockConfig` field needs its `serde(default)` in the
  second struct, and no test iterates fields.
- Fix (TYPES): serialise `StockConfig` and `ToolConfig` directly (the operator
  ruling allows the wire break). Delete the mirrors.
- Cost M. In flux (`project_file.rs`, `save.rs`). Known: D1 (inherited
  2026-09-16).

## DUP-17 (D) Deflection-cap inputs are assembled twice; the doc says once

- `crates/rs_cam_core/src/feeds/efficiency.rs:315-356` says "One assembly, so
  the inspector and the optimizer cannot disagree". It builds the compliance at
  `:344`.
- The optimizer builds its own at `crates/rs_cam_core/src/session/compute.rs:2000-2013`.
- Fix (TYPES): `DeflectionLimitInputs::for_cut(tool_def, line, axial_doc)` in
  `feeds/force.rs`; both call it.
- Cost S. In flux (`session/compute.rs`). Owner: power-calcs.

## DUP-18 (E) Hand-typed token vocabularies beside the serde derive

- MCP `crates/rs_cam_mcp/src/server.rs:1971-1974` types the 24 op tokens in the
  error text. The owner is `crates/rs_cam_core/src/compute/catalog.rs:305-334`.
- Op-token parsers: `crates/rs_cam_cli/src/run.rs:271`,
  `crates/rs_cam_cli/src/smoke.rs:1074` (Option), and the MCP one (serde).
- `ToolContainment` (`crates/rs_cam_core/src/geometry/boundary.rs:15-22`) is a
  twin of `BoundaryContainment` (`compute/config.rs:402-411`, whose doc says
  "re-export from core" and is not one). Hand maps at `session/compute.rs:131-135`,
  `session/compute/generation.rs:889-891,1051-1053`; hand token maps at
  `generation.rs:931-933,1106-1108`.
- Fix (TYPES): `OperationType::from_kind_str` in core; the error lists
  `OperationType::ALL`. Delete `ToolContainment`. `serde_token()` for the token.
- Cost S. In flux (`server.rs`, `run.rs`, `smoke.rs`, `generation.rs`,
  `session/compute.rs`, `compute/config.rs`).

## DUP-19 (F) Map-cache scaffolding and same-named epsilons

- `cache_len` / `clear` / `get` / `put` are the same code in
  `crates/rs_cam_core/src/maps/tier_map_cache.rs:180-267`,
  `maps/reach_map_cache.rs:180-254`, `maps/finish_surface_cache.rs:300-318`,
  `maps/geom_cache.rs:252-262`; `t.len()` vs `t.entry_count()` stays (D8 note).
- `CLAMP_EPS = 0.001` four times (`toolpath.rs:628,801`,
  `geometry/monotone_cells.rs:325,425`); `XY_EPS_MM` twice in `dressup/mod.rs:801,2621`
  and once in `compute/obstacle.rs:828`; `EPS_VEC`, `EPS_DENOM`,
  `MIN_TRIANGLE_AREA_MM2` in `finish/conformal_spiral.rs:311,314`,
  `finish/direction_field.rs:114,117,120`, `metrology/monge.rs:56,58`.
- Fix: a `memo_cache!` macro or a generic over the table; one module const per
  tolerance.
- Cost S. Not in flux. Known: D8 residue.
