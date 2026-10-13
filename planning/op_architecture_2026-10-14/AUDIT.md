# Operation architecture audit (2026-10-14)

Status: AUDIT ONLY. No code changed. Nothing here is decided until the operator
rules on it.

## Question

Can every toolpath operation be written as a small kernel that declares what it
is, so that a shared framework owns the Z ladder, rest, cancel, spans, findings,
dressups, links and the parameter schema? Can the GUI, MCP and CLI build from
that declaration?

Trigger: op Z23 (a rest Pocket) re-cut 112 cleared levels, because each adapter
decides alone what "from remaining stock" means. G-RESTLEVELS (61c21a7f) fixed
Pocket only.

## Method

Four read-only agents tested the first design (one trait per planning shape,
derived capabilities, a params derive, a typestate pipeline, a conformance kit)
against the code. Groups: 2.5D and drilling; curve and raster; 3D; the shared
surfaces (params, capabilities, dressups, typestate, registry). The lead merged
the four reports below.

## Verdict

The idea holds, but not as "one trait". The ops need six kernel shapes, and the
largest gain is in shared services, not in the traits.

| Shape | Kernel the op writes | Ops |
|---|---|---|
| Layered | `prepare(Geometry)` once; `emit_level(prepared, LevelInfo { z, index, is_final }, LevelStock)`; a `ladder()` hook (heights / one derived Z / mesh-derived) | Pocket, Zigzag, Profile, Trace, Rest, Face, 2D Adaptive, Chamfer, Waterline |
| Height field | `plan(HeightQuery, Territory)`; the query is the mesh drop cutter or an analytic field (V-carve); territory arrives BEFORE planning | 3D Finish (drop cutter), Scallop, V-carve, Horizontal, Radial, Spiral, Ramp |
| Composed surface | classifier → kernel per region (cell mask) → one router/relink stage | Unified Finish, Steep/Shallow; Pencil adds a detector stage |
| Volumetric | planner owns the stock, ladder anchor, entries, segment merge | 3D Rough (Adaptive3d) |
| Point | resolve points ONCE; peck schedule | Drill, Pin Drill |
| Curve | `plan(curves, Option<HeightQuery>)` | Project Curve |

Inlay does not fit any shape. It is two ops in one (see defects); split it.

### What the first design got wrong

1. Rest is not one thing. `RestUse` must be declared per op INSTANCE
   (`fn capabilities(&self)`, not an associated const):
   - `LevelClip` (area ops: clip the input region per level);
   - `LevelSkip` / `PathClip` (Profile, Trace: clipping the outline makes a new wall);
   - `LevelMask` (2D Adaptive: the per-level grid seed);
   - `ContourFilter` (Waterline: filter contour runs per level);
   - `StockColumn` (Drill: hole top from the stock column);
   - `Planner` (3D Rough), `Reference` (Pencil, Unified claims), `Territory` (Unified S4);
   - `LinkSafetyOnly` (simple finishes, Project Curve today);
   - `None` (Pin Drill).
2. Some capabilities read the instance (`stock_to_leave`, Scallop `continuous`),
   so capabilities are a method with the static row as default.
3. The kernel returns more than paths: engagement, rest grid, rest regions and
   area regions go to the modulator, the GUI and other ops. Kernels return a
   typed `Produced` side channel plus per-move labels/events.
4. Cancel cannot be owned by the framework alone: kernels poll inside rayon
   chunks. The framework passes one `CancelCheck` and checks it FIRST
   (an existing sentry requires cancel before any other refusal).
5. Order must be declared. Every op today is region-first (each region to full
   depth). Level-first changes retracts, rapids, spans, and on a through-profile
   it changes when parts come free. Default: region-first.
6. Planner-owned entries and segment merge (2D Adaptive, 3D Rough) are correct
   and must stay. "The op never sees dressups" is false for stock-aware planners.
7. A typestate `Toolpath<Planned> → <Dressed> → <Fed>` is not worth it. Only
   `execute_job` writes results, so an undressed path cannot reach export. The
   real gap is the feed stage: put a provenance tag on the result slot
   (`Dressed | Modulated { sim_rev }`) and make export and narrate state it.
8. The pipeline in the design omitted the boundary clip and the entry-descent
   split (`clip_and_split_entries`, `session/compute.rs:574`).

### What holds

- One dressup pipeline (`DRESSUP_PIPELINE`), with `applies(&Capabilities,
  &DressupInputs)`: entry containment needs the mesh, polygons and tool radius.
- The exhaustive registry (`for_each_op!`, `registry_entry`) and a generated
  conformance suite. One fixture per `GeometryRequirement`; tool from
  `ToolConstraintsDef`; rest check gated on the instance capability; empty check
  on `empty_is_feature_selective`; flipped-frame check where the setup transform
  is read.
- A params derive: field list, type names, ranges, help, aliases and
  `unexposed = "reason"` from the struct. `OpUiRow::draw` stays the override for
  conditional rows and custom widgets (drill targets, claims block, pin wizard,
  engagement-to-stepover). Cross-field rules (peck clamp, helix cap) stay code.

## Shared services (the main gain)

Each item below is written by hand in several adapters today:

| Service | Copies today |
|---|---|
| One link stage with ONE ceiling rule | 2 that disagree (Project Curve: `tool_def.radius()` + `heights.top_z`, always; raster: `envelope_radius_mm()` + `stock_bbox.max.z`, only with stock) |
| One rest-reference resolver (stock/mesh overlap guard + fallback) | 3 with different fallbacks (`finish_3d.rs:338`, `dressup_apply.rs:166`, `finish/pencil/detectors.rs:223`) |
| Cancel closure, `require_mesh`/`require_index` | 8 in `finish_3d.rs` alone |
| Tool refusals from `tool_constraints` | Pencil, Unified, Spiral, Scallop, V-bit ops; plus a copy in `diagnostics/adapters/from_static_checks.rs:202` |
| Cut-run / depth-run spans | 5 of 7 curve/raster ops, all 2.5D ops |
| Findings through a returned report struct | offset failures in 4 ops, truncated core, rest levels |
| Frame from the context | Project Curve keeps `setup_z_flipped` in its config |

## Defects found (none fixed; verify before fixing)

| # | Defect | Evidence | Verified? |
|---|---|---|---|
| D1 | Two link ceilings disagree | `curve_engrave.rs:304-308` vs `finish_raster.rs:91-100` | code read |
| D2 | 2D Adaptive keeps only the first piece of a multi-piece inset; material can stay | `adaptive/path.rs:692` (`machinable_vec[0]`) | code read |
| D3 | Inlay: male plug cut into the same workpiece and frame; flat clear planned for a tool the op does not have | `curve_engrave.rs:55-67`, `ops/inlay.rs:98-106` | code read |
| D4 | Chamfer may cut outside the edge, not `w` mm inward as its doc says | `ops/chamfer.rs:62-90`, `ops/profile.rs:145` | NOT simulated |
| D5 | GUI clamps silently with ~116 literal ranges; MCP accepts any finite value | `ui/properties/operations/boundary_2d.rs:72-133` | code read |
| D6 | `feeds/predict.rs:701-741` ends `_ => {}` (a new op fails open); `predict.rs:313` tests `== Drill` not `is_drill_kinematics()` | as cited | code read |
| D7 | Face passes no levels to the span builder, so its depth spans snap to no level | `clearing_2d.rs:534` | code read |
| D8 | Drill top Z is the stock bbox top, so it drills air in a cleared pocket | `drilling.rs:473` | code read |
| D9 | Red test `operation_schema_params_match_params_with_nulls_for_every_op`: `AlignmentPinDrillConfig::diameter` is serde-required but unexposed | `operation_configs.rs:264`, `catalog/tests.rs:75` | test red |
| D10 | Export under `AcceptPreviousGeometry` can emit pre-modulation output with no statement | `viz/io/export.rs:268-320` | code read |
| D11 | GUI `prior_stock_blocker` searches only the op's own setup ("no prior operation" across setups) | `viz/controller/events/compute.rs:96-127` | seen live 2026-10-14 |

## Size

The kernels (`ops/`, `finish/`, `adaptive3d/`, `surface/`) do not shrink. The
glue (~4.5 k lines of adapters, ~3.4 k catalog, ~6.2 k GUI per-op editors, 335
`OperationConfig::` sites) shrinks by an estimated 15–25 %; the framework adds
about 1 k. Net: a small reduction. The gain is consistency.

## Recommended order (for the operator to rule on)

1. Fix the verified defects that can lose material or make an unsafe link:
   D1 (one link ceiling), D2, D3, then simulate D4.
2. Shared services: the link stage, the rest resolver, cancel/require
   boilerplate, refusals from the registry.
3. The Layered shape for the 2.5D family with per-op `RestUse` (Face, Zigzag
   op, Profile skip, Trace skip, 2D Adaptive mask).
4. The conformance suite.
5. The params derive (fixes D5, D9).
6. Height-field and composed-surface shapes.
