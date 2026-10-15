# Operation architecture: orchestration plan (2026-10-14)

Source: `AUDIT.md` in this folder (the verdict, six kernel shapes, defects
D1–D11). This file says who does what, in which order, and how each step is
proved.

## Rules for every agent

- Agents edit and test. Agents never commit, push, stash, reset or checkout.
  The lead reviews each change by path and commits it with explicit paths.
- Work on branch `op-arch` (made from master by the lead). The lead merges to
  master at the end of each wave, after the review and the live check.
- One cargo job at a time: `timeout <s> scripts/cargo_lane.sh <args>`.
- No large gates: named sentries, `--lib <filter>`, the folder sentries, then
  the full clippy line and `cargo fmt --all -- --check`. Ask before any test
  run over 3 minutes.
- One agent edits the checkout at a time (shared checkout, one build lane).
  Research agents are read-only and can run in parallel with an editor.
  No worktrees (disk is at 87 %; a worktree target is 10–55 GB).
- Constants are derived from existing constants or a cited source.
- STE prose in comments, docs and reports. No legacy shims; state breaks.
- Each fix gets a sentry. Each sentry is proved red once with the defect
  put back, then restored.
- An agent that cannot finish an item cleanly stops and reports it open.

## Safety net first: W0 golden toolpath fingerprints

Before any refactor, one agent builds
`tests/every_op_keeps_its_toolpath_g_opgolden.rs`:

- one small fixture per `GeometryRequirement` (Stock, Polygons, Mesh, Both)
  and per tool kind the registry row requires;
- every op in `OperationType::ALL`, fresh stock and rest stock where the op
  reads it;
- a fingerprint per op (move count, cut length, bbox, a hash of the moves via
  `export/fingerprint.rs`), stored as a blessed file;
- a bless switch for an intended change, which the lead approves by path.

Every later step that claims "no change in output" proves it with this test.
A step that changes output on purpose re-blesses only the ops it names.

## Waves

### W1 — defects that can leave material or make an unsafe link

Sequential, one editor each. Order by risk.

| Item | Defect | Work | Proof |
|---|---|---|---|
| W1.1 | D1 two link ceilings disagree | Research the two rules; the lead rules (R1); one `LinkCeiling` factory that both families call | sentry: same inputs → same ceiling on both families; G-opgolden re-bless for the changed family only |
| W1.2 | D2 2D Adaptive keeps only `machinable_vec[0]` | plan every piece | sentry: two-island region → both islands cleared (sim, no material above floor + eps) |
| W1.3 | D4 Chamfer side | simulate first; fix only if confirmed | sentry: chamfer width and side measured on the sim stock |
| W1.4 | D3 Inlay plug in the workpiece | prove with a sim sentry; write the split proposal (R2); implement after the ruling | sentry: the female op cuts nothing outside the female region |
| W1.5 | D11 GUI prior-op check stops at the setup | walk core `carve_order_before` | viz test: rest op first in setup 3 names the setup-2 op |
| W1.5b | The live generate_all refusal (2026-10-14): `generation_plan.rs:~139` reads `has_snapshot` once when the plan is built; `source_is_current` skips an upstream op with no result, so a stale snapshot reads current and the plan puts no Simulate step before the rest op | the plan must schedule a Simulate before a rest op whose upstream ops are generated inside the same plan | the ignored test `a_retry_after_an_upstream_failure_resimulates_before_the_rest_op_d11` turns green |
| W1.6 | D9 red schema test | make `diameter` consistent with the schema (serde default or unexposed decision) | the red test turns green |
| W1.2b | D2 in Adaptive3d slices (same planner, still plans only `machinable_vec[0]`) | reproduce on a mesh with a split slice; fix in the planner or the 3D adapter | sentry + G-OPGOLDEN |
| W1.3b | Operator 2026-10-14: Chamfer on inside edges too (pocket walls and holes) | a side setting on Chamfer (outside / inside), or chamfer every edge of the outline including holes; same bevel rule | sentry: inside bevel width and depth on a pocket and a hole |
| W1.4b | VCarve's Finish lead-in cuts 238 cells up to 0.75 mm outside the outline (found in W1.4) | strip or contain the lead on VCarve as on Inlay | sentry: no cell outside the outline |
| W1.7 | R3: retire the Rest op | delete the op on every surface (core, GUI, MCP, CLI, catalog, docs); state the load break | G-OPGOLDEN loses its Rest cases on purpose; build + clippy clean |
| W1.1b | R1 follow-up: links are not scored against the stock | a simulation check that flags a fed link whose tool passes through remaining stock | sentry: a link forced below the stock top is flagged |

Live check after W1: rebuild release, load `testpiece_mini`, generate all,
simulate, compare with commit 7a9f5d84 (expect no change: it uses none of the
fixed ops except links).

### W2 — shared services

| Item | Service | Removes |
|---|---|---|
| W2.1 | One rest-reference resolver (overlap guard + one fallback) | 3 copies (`finish_3d.rs`, `dressup_apply.rs`, pencil `detectors.rs`) |
| W2.2 | Kernel prologue: cancel checked FIRST, `require_mesh`/`require_index`, tool refusals from `tool_constraints` | the copies in every adapter and `from_static_checks.rs:202` |
| W2.3 | Findings through a returned report struct | hand-written offset-failure / truncated-core / rest-level records |
| W2.4 | `fn capabilities(&self) -> Capabilities` on the config, static row as default; move the membership lists into it | D6 (`predict.rs` fail-open and `== Drill`), `narrate.rs` lists, the three catalog matches |
| W2.5 | Result provenance tag `Dressed | Modulated { sim_rev }`; export and narrate state it | D10 |

Proof: G-opgolden shows no change for W2.1–W2.4. W2.5 changes only text.

### W3 — the Layered shape for the 2.5D family

- W3.0: trait `LayeredOp` and `run_layered` with `LevelInfo { z, index,
  is_final }`, `LevelStock`, a `ladder()` hook, region-first default plus
  `region_ordering` (R4), the
  prepare cache per region, spans and findings owned by the framework.
- W3.1: move Pocket onto it. G-opgolden: no change. The G-RESTLEVELS sentry
  stays green.
- W3.2: Zigzag, Trace, Profile (no output change on fresh stock).
- W3.3: Face onto the framework ladder; fixes D7.
- W3.4: `RestUse` per op: Zigzag `LevelClip`, Profile/Trace `LevelSkip`,
  Face `LevelClip` on the stock rectangle, 2D Adaptive `LevelMask` (one stock
  read shared with the clip), Drill `StockColumn` (fixes D8). Each gets a case
  in the G-RESTLEVELS sentry. (The Rest op is gone after W1.7.)
- W3.5: Chamfer and Waterline as Layered ops (Waterline: `ContourFilter`).

Live check after W3: the mini project; then Z22/Z23 as Zigzag pattern and a
rest Profile on a fixture.

### W4 — conformance suite over all ops

Generated from `for_each_op!`: cancel first, rest (gated on the instance
capability), empty (`empty_is_feature_selective`), flipped frame (where the
setup transform is read), determinism, schema round trip. The existing
cancel-flag test moves into it.

### W5 — params derive

Ruling R5 = A: a new proc-macro crate (`[lints] workspace = true`; errors through `syn::Error`). `#[derive(OpParams)]` gives field
list, type names, ranges, help, aliases, `unexposed`. The GUI `dv()` reads
the derived range (D5 gets a direct fix in W1 first); MCP validates against it. `OpUiRow::draw` stays
for layout and custom widgets. The text-parsing test
`param_defs_cover_every_config_field` is deleted.

### W6 — 3D shapes

Height field, composed surface, volumetric, curve. Plan this wave after W3
and W4, from what they measure. It is not scheduled now.

## Rulings (operator, 2026-10-14)

| # | Ruling | Effect on the plan |
|---|---|---|
| R1 | **D**: one `LinkCeiling` builder. Stock = remaining stock when known; fallback top = `max(stock_bbox.max.z, heights.top_z)` (the G-PECKSPLIT rule); radius = `envelope_radius_mm()`; each op states in that builder whether it needs a ceiling on fresh stock (engraving: always; finishing: only with remaining stock) | W1.1. Added W1.1b (a simulation check that scores fed links against the stock: today a link through stock is a silent plough) and backlog B1 (shank and holder in the ceiling; pencil searches only the tip radius) |
| R2 | **A**: split Inlay. "Inlay" cuts only the V walls of the pocket; the floor is a normal Pocket with a flat end mill; a new "Inlay Plug" op (mirrored, start depth) runs in its own project for the plug board | W1.4. Also fix: plug not mirrored; glue gap wrong for bits that are not 90°; default flat tool is imperial |
| R3 | **Retire the Rest op**, unless the reach map uses it. Checked: the reach map does not use `ops::rest` (only a doc mention in `surface/rest_field.rs:37`). Operator note: the reach map is a visual, not planning, and must not live with planning code | W1.7: delete the Rest op on every surface; state the load break |
| R4 | **B**: region-first default plus the `region_ordering` option (the 3D Rough enum) on every Layered op | W3.0. 3D Rough keeps its own default (Global, level-first) |
| R6 | **A** (2026-10-15): 2D Adaptive seeds its grid from the shared per-level mask (threshold z + FRESH_MATERIAL_THRESHOLD_MM, the stock lattice); Adaptive3d keeps its own path | DONE |
| R5 | **A**: a proc-macro crate for `#[derive(OpParams)]`, "clean now" | W5 uses a derive. The GUI range drift (D5) and the pin-drill schema test (D9) still get their own direct fixes in W1 |

## Per-item flow

1. Read-only research agent (only when the item needs one), in parallel with
   the current editor.
2. Editor agent: code + sentry + red proof + gates; report ≤ 500 words.
3. Lead: review the diff by path, run the item's sentry once, commit.
4. Next item.

## Status

| Wave | State |
|---|---|
| W0 | DONE: G-OPGOLDEN, 37 cases, 24/24 ops, 2 s |
| W1 | W1.1 DONE 877c93e5; W1.2 DONE b5dc7ed4; W1.3 DONE 0c8f562a (chamfer cut outside the edge); W1.4 DONE 08458617 + FM1 275e8948; W1.5, W1.6, W1.4b DONE 3ccc05bb; W1.5b DONE b086e269; W1.7 DONE 04753b54; W1.2b DONE e10bac23; W1.1b DONE e72f4ccb; W1.3b DONE. W1 DONE; live check passed (mini: no change); merged to master |
| W2 | DONE: 6fddd518, eabeb43f, e614ee9a, 66a81d77, 7d19ea6d; live check passed (mini: no change); merged to master |
| W3 | W3.0/W3.1 86692c56, W3.2 b6c83a81, W3.3/W3.4 29183d95, W3.5 Chamfer (Waterline moved to W6: needs fallible emit_level with cancel + per-level stock, and a link hook); B5-B7 7f820c42; live check passed (mini 2675 s vs 2673 s, only Face front changed, B8); merged to master; open: 2D Adaptive shared-mask ruling |
| W4 | DONE: every_op_conforms_w4 (c7de79a0, 10 checks over ALL, ~8 s); found G-RAMPCHORD (SAFETY, entry ramps cut arc chords; fixed 981ff4ed) and the classification_sampler schema null; R6 bd7cf9f4; B8-B11 b523905a; merged to master |
| W5 | DONE: rs_cam_derive #[derive(OpParams)] + ParamEnum, all 24 ops (95ee4b3c), floors in the schema; live mini export identical; merged to master |
| K (SAFETY, before W6.3) | Fixtures and keep-out zones are ignored by Face, 3D Rough, 3D Finish (and more); rapids at the fixture top; the simulation reports 0 collisions (seen live on testpiece_mini clamps, 2026-10-15). One keep-out service every op passes through, planes above fixtures, a simulation check on every move class, a W4 check. Operator 2026-10-15: keep-out zones and fixtures are ONE concept, and its geometry may be an imported model (mesh) or an outline extrude, not only a box. | K1 92fcee38, K1b 3e030aa2, K1c d71caf61 DONE. Live: testpiece_mini 0 moves within 2 mm of a clamp (scratchpad clampdepth.py); 3338 s total vs about 2673 s before (lifts now to 36.5) |
| K2 | GUI obstacle editing (box, outline, model), renderer and overlays; merge the dead `keep_outs` overlay row into the obstacle row; remove the duplicate `kind` label in `obstacle_json`; stop the pin-wall code from rasterizing a mesh silhouette on every call. | DONE 6a013a78 |
| K3 (efficiency) | The obstacle plane is global per setup, so every lift goes to 36.5 on testpiece_mini, also far from a clamp (back channels: 1356 lifts, about +0.5 s each). Rise to the plane only when a link's XY crosses a grown footprint. Pre-clip each op's territory by the obstacles so a raster links rows on one side of a clamp and does not lift over it (front finish: 5960 lifts vs 314). | DONE 2ea3166b (+ K3b ad370323). Live: 2869 s, 0 collisions, finish coverage kept |
| C (clean-up, after W6.5) | Lead rulings 2026-10-15 (operator delegated: "all old code that is left behind is retired"), evidence in `LEFTOVER_AUDIT_2026-10-15.md`: (1) retired keys refuse to load and are stripped from the files: `air_bridge_policy`, `flip_axis`, `workholding_rigidity`, empty `fixtures`/`keep_out_zones` (B17); (2) a toolpath with no `[operation]` table refuses; (3) delete the Waterline `depth_per_pass` setter alias and `DeprecatedDialFinding`; (4) W2.4 sites that match on the op type read `capabilities()` (narrate, validate, session/compute); (5) the CLI job format reads the W5 schema, the hand-written map and pre-T9 shims go; (6) B18: delete `shank_length` as a tool parameter and the CLI `holder_length`; (7) the pin-drill config patch in `generation.rs` goes with the W6.5 ProjectCurve patch; (8) pencil's own `LinkCeiling` reads obstacles through `link_ceiling_for`; (9) 2D Adaptive, Drill, Pin Drill, Inlay, InlayPlug keep their shapes; check each reads prologue, territory, record door, capabilities only; (10) production `allow(dead_code)` without `SAFETY:` lines. | DONE: C1 56dd706d, C2 ba705014, C3 d508ffaa |
| W6 | DONE: W6.1 Waterline on the framework c6301ee7; rest efficiency 1ffbcab3; W6.2 territory resolver + side channel 9e3a9c56; W6.3 height field 624f42a8; W6.4 composed surface d7fd161e; W6.4.6 SteepShallow rest + links 8a8d166c; W6.5 volumetric/curve wrappers ca29d74f |
| D1 | DONE 321615b1: Scallop iso-field links read the ceiling (SAFETY); runners own composed inputs and `ContourFilter`; one rest-field map; `RestUse::reads_reference_stock` |
| D2 | DONE (this commit; the lead commits): exhaustive `RestUse` match in the Layered runner; RampFinish and Pencil setter aliases deleted; three test-only items under `cfg(test)`; stale comments; Project Curve records its link totals; feeds lookup reads `Capabilities::lut_route`; Pin Drill `TerritoryXy::SetupMap`; one call site for the session stages; Project Curve `relink_dial` = `chain_distance_mm`; B18 closed. Partial: FINAL_AUDIT item 6 (test callers of the old entry points still need the session path) and item 14 (two older `allow(clippy::...)` lines) |

## Backlog (not scheduled)

- B1 (OPEN, outside the migration: a link-ceiling physics change, not a structure change): shank and holder radius in the link ceiling (W1.1b evidence: pencil on remaining stock, 21 links cut 0.50 mm = the rough's leave at the mesh edge); pencil link search uses the
  tip radius, not the envelope (R1 research).
- B2 (OPEN, outside the migration: a new feature): a second board in one project (would join the Inlay pocket and plug).
- B3: D5 GUI ranges read `ParamDef.range` (can land before W5).
- B4 (OPEN, outside the migration: planner quality; W6.5 kept it out) (was W1.2c): AgentSearch and ContourSpiral leave ~2.4x more wall-band wood than ContourParallel (double inset in tool-centre space). Operator 2026-10-14: ContourParallel is the 3D Rough strategy in use; the others are known to be rough. Revisit with W6 (Volumetric shape).
- B5 (W2.5 follow-up, found 2026-10-15): the feed-stage caution compares the modulating simulation run with the LATEST run, so generate_all (one prefix simulation per op) makes every modulated op read 'from an earlier simulation' although nothing it depends on changed. Compare against whether the op's own carve-order prefix changed (the SourceStock / prefix digest), not the run counter.
- B6: import_machine_settings replaced the whole controller block with what the dump held (firmware grbl_hal -> unknown, the H-100 VFD plugin, the tool-change probe distance lost). An import must only overwrite the fields the dump states.
- B7: a machine kinematics change leaves every toolpath current, but the link stage reads accel (link vs retract). Decide whether linking ops go stale on a machine edit.
- B8 (W3 live check 2026-10-15): the rest Face 'Face front to T' on testpiece_mini grew from 782 to 792 moves; its rows now reach x -2.9 / 67.2 where they reached -1.0 / 66.0 (the clipped region seems to get the stock offset again inside the Face kernel). All extra motion is in air outside the stock; +2 s. Check and fix the double extension.
- B9: ZigzagRaster (Zigzag op, Pocket zigzag pattern) has the same latent rest row shift B8 fixed for Face (rows start at the clipped region edge).
- B10: on a rest level the 2D Adaptive planner starts with an entry starter at the piece centre even when that point is cleared.
- B11: for a rest op, the entry dressup turns a row that runs only in air into a rapid with intent Unknown (also seen on rest Drill pecks). Air rows should be dropped or tagged, not left Unknown.
- B12 (from B10): at the level at a prior op's own floor, the rest 2D Adaptive still places an entry helix at the piece centre 7.4 mm from material with no cuts near it (level mask reads material, air filter reads air, the filter never drops a helix).
- B13 (W5): planning/fixtures/nz_south.toml holds four ProjectCurve point_spacing = 0.05, below the W5 range 0.1..=5.0 (the old GUI literal). Operator ruling 2026-10-15: allow down to 0.01 (ProjectCurve point_spacing min 0.01).
- B14 (W5): feeds clamp_peck_to_depth bypasses the setter and can write a peck under the 0.5 mm floor on a shallow hole (0.75 x depth).
- B15 (OPEN, outside the migration: waits for an optimizer ruling; the optimizer was culled 2026-10-13): feed optimisation is refused on rest ops (catalog feed_optimization_unavailable_reason), so rest ramps run at the plunge-derived rate (250 vs 1000 mm/min on the Waterline cap). Revisit with the optimizer rebuild ruling (optimizer culled 2026-10-13).
- B16 (CLOSED d7fd161e) (W6.4 design): the UnifiedFinish link between regions reads no link ceiling when it plans; only the W1.1b simulation check can flag it, and no fixture covers this op.
- B17 (CLOSED 56dd706d) (K1): an empty `fixtures = []` / `keep_out_zones = []` still loads (57 files, hashed fixtures among them). The no-legacy ruling says refuse `Some(_)` and strip the files.
- B18 (CLOSED: 56dd706d deleted `shank_length` and `holder_length`; D2 proved the snippet reset safe, sentry `a_snippet_join_rises_to_the_plane_b18`: after a snippet the next rapid is Z-first and the K3 join rapid is at the obstacle plane) (K1b): `shank_length` is still a tool parameter but no longer moves the holder face; the CLI `ToolDef.holder_length` is parsed and never read. A `pre_gcode`/`post_gcode` snippet resets the builder position, and the join check does not see that reset.
- B19 (CLOSED 4f7578c4) (2026-10-15, live): generation depends on session history. testpiece_mini, op 0 "Face back to T1" (setup 1, first op): a fresh load + Generate All keeps the air rows outside the stock; a second Generate All in the same session (after `apply_feeds`, nothing stale) drops 203 lines of them. A fresh reload gives the first output again. Suspect: the face reads a stock snapshot from the earlier whole-project simulation. Benign here (the whole face is air) but a stale stock can drop rows over real material.
- B20 (CLOSED ba705014) (W6.4, 2026-10-16): under a link ceiling, a router link and a relink clearance hop end with a vertical `Linking` feed at cutting feed onto the next region's entry, not an `EntryPlunge`. Check whether the entry-descent dressup treats that descent; if it can enter standing stock, tag it as an entry. Decide in C.
- W6.4.6 DONE (R-W6.4b yes): SteepShallow rest is `RestUse::ContourFilter` (the Waterline rule), not `Reference`: the stock-referenced detector read near-zero rest under a flat tool over terraces and cost +49 % time. Links read `link_ceiling_for` (new `hookup_mm` dial, default off; -11.6 % to -14.6 % time with it on). R-W6.4a (emitters onto W6.3 kernels) = no.
- C3 (CLOSED d508ffaa) (follow-up): `Territory::resolve` gives stock only to `RestUse::Reference`; Waterline and SteepShallow (`ContourFilter`) read `ctx.initial_stock`. Resolve stock for `ContourFilter` too and make both read `Prepared::territory()`.

## D3 (2026-10-16)

DONE in the D3 commit after 72fb9854: the per-op entry points the shape runners replaced are deleted; tests use `op_fixtures::generated_op` or `tests/common/kernels.rs`; param aliases deleted; Pin Drill draws no boundary controls. FINAL_AUDIT item 6 closed. Left (minor): `pocket_contours` and `profile_contour` have callers only in their own file (make private); `adaptive_toolpath` in `param_sweep`; dated history docs (`architecture/orientation_refactor.md`, `toolpath_stress_test/*.md`) still name deleted functions and stay as history.

## H — machine head and dust shoe (operator 2026-10-16)

Operator facts (Scylla, H100 spindle): bare collet nut about 20 mm diameter, 30 mm tall above the holder face; above it a shelf (treat as no width limit, solid: spindle body and the collector mount). Dust shoe (adjustable on the spindle, can be raised out of the way or removed): brush ring 80 mm diameter, 25 mm long, SOFT (it may touch stock AND clamps); above it a rigid plate 80 mm wide, 5 mm tall; then a shelf. Normal finishing: brush end about 25 mm above the bit end. The shoe height can change at each tool change.

Rulings (lead + operator):
- H1 core: a general head model in the machine profile (most CNCs are similar): parts with diameter, height, hard/soft, shelf flag; a fixed mount (collet face, moves with stickout) and an optional adjustable shoe mount (travel range from the collet face, can be raised clear or removed). Presets "Bare collet" and "Dust shoe". Hard parts and shelves join the tool bands in the obstacle step, the link ceiling, the simulation and collision_check; soft parts never block a move. A shoe solver gives, per tool change (and per shoe change without a tool change), the lowest brush gap in the travel range that keeps the hard shoe parts clear of stock and obstacles; if none, "raise the shoe clear / remove it" and the check uses the bare head. Per toolpath shoe setting: Auto (default) / Gap mm / Raised / Off.
- Export: the shoe note joins the tool-change `(MSG,...)` line; a shoe change that needs the operator (move, raise, refit) also gets an `M0` pause with the message (post option, default on; revisit after the operator's sender test `~/Downloads/rs_cam_msg_test/`). The setup sheet lists the shoe setting per tool change.
- H2 GUI: machine-profile head editor with the two presets, per-toolpath shoe setting, the shoe list in the export wizard and setup sheet, tool-change markers with the note in the simulation timeline, head drawn in the tool ghost.
