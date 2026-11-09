# P22-0c design note: one tool body model, one contact check

Read-only design pass at HEAD 500f9ea6f (2026-11-09). No code changed. No cargo job ran.
P22-0b edits `dexel_stock/`, `stock/collision.rs`, `dressup/entry_descent.rs` and `adaptive/`
at the same time; line numbers in those files can move. Operator direction (2026-11-09): "A tool
must own the parts; the systems must use them." Phase row: `P22_DESIGN.md` section 9.

## 0. Summary

- The tool facts live in `ToolConfig` (`compute/tool_config.rs`). The derived solid lives in four
  places: `MillingCutter` (the cutting shape), `ToolDefinition` (shank, stickout, core limits),
  `ToolAssembly` (`stock/collision.rs`: cutter radius, flute, shank, holder face, head bands) and
  `NonCentreCuttingLimits`. Each consumer then derives its own zone from them.
- The survey finds 11 second definitions (section 1.2). One is a live defect: the simulation
  prefix memo does not hash the centre core (D1).
- The design adds `tool/body.rs`: `ToolBody`, a list of axial zones that the tool owns. It is
  built in ONE constructor, inside `build_cutter`. `ToolAssembly` becomes `MountedBody` (the body
  plus the machine head). `NonCentreCuttingLimits` becomes a view of the core zone.
- One contact kernel tests each non-cutting zone against a solid. The solid is the live
  simulation stock or the model mesh. Each zone kind keeps its finding id. No finding refuses.

## 1. Survey

### 1.1 Readers of tool geometry

`rg` counts over `crates/` (Rust files, tests included): `ToolDefinition` 350 hits / 93 files,
`build_cutter` 201 / 87, `ToolGeometryHint` 536 / 86, `RadialProfileLUT` 208 / 42,
`envelope_radius_mm` 184 / 60, `height_at_radius` 181 / 44, `effective_diameter` 83 / 34,
`cutting_length_planning_mm` 65 / 41, `tip_deflection_mm` 55 / 19, `NonCentreCuttingGeometry`
50 / 17, `RapidClearanceCheck` 45 / 14, `AssemblySegment` 44 / 13, `shank_diameter_planning_mm`
38 / 23, `ToolAssembly` 34 / 8, `check_collisions` 29 / 6, `stickout_planning_mm` 28 / 20,
`ModulationCutter` 23 / 8, `to_assembly` 17 / 11, `for_stock_stamp` 17 / 8, `HeadBand` 15 / 4,
`NonCentreCuttingLimits` 16 / 7, `non_centre_cutting()` 9 / 6, `non_fluted_body` 4 / 3.

| Reader | file:line | Tool facts it reads | Second definition? |
|---|---|---|---|
| `build_cutter` | `compute/cutter.rs:12` | all facts; builds the shape and `ToolDefinition` | no (the one door) |
| `ToolConfig::non_centre_cutting_limits` | `compute/tool_config.rs:1095` | `centre_cutting` -> core d, protrusion, angle | no (the source) |
| `ToolConfig::geometry_hint` | `compute/tool_config.rs:1107` | type, corner, angle, tip, taper | yes: copy of `MillingCutter::geometry_hint`, pinned equal by a test |
| `ToolConfig::envelope_diameter` | `compute/tool_config.rs:1134` | type, shaft | yes: copy of `envelope_radius_mm` (tapered shaft rule) |
| `ToolConfig::upper_diameter_mm` | `compute/tool_config.rs:1302` | shank, else envelope | yes: "shank else" rule (D3) |
| `ToolDefinition::exposed_shank_length` | `tool/mod.rs:708` | stickout - flute | yes: also `ToolAssembly::exposed_shank_length` `stock/collision.rs:119` |
| `ToolDefinition::to_assembly` | `tool/mod.rs:718` | radius, flute, shank, stickout, head | builds the assembly (absorbed below) |
| `ToolDefinition::tip_deflection_mm` | `tool/mod.rs:757` | stickout, flute, shank, width profile | yes: `shank_top = l - cutter_len` is a third shank span |
| `MillingCutter::non_centre_cutting` | `tool/mod.rs:312` | core limits (only `ToolDefinition` holds a value) | no; a trait default `None` |
| `ToolAssembly::segments`, `non_fluted_body`, `shelf_offset` | `stock/collision.rs:132-170` | flute, shank, head bands | owner in the wrong folder (see 1.3) |
| `check_collisions*` (mesh) | `stock/collision.rs:367`, `423` | assembly bands, cutter radius | check 1; skip `seg_radius <= cutter_radius` |
| `run_collision_check` | `compute/collision_check.rs:103` | `to_assembly`, head | caller of check 1 and of K1 |
| `ObstacleSet::for_tool` | `compute/obstacle.rs:633`, shelf `:705` | cutter radius, bands, shelf | no (reads the assembly) |
| `tier_flute_reach` | `maps/tier_flute_reach.rs:136` | `non_fluted_body` | no (reads the assembly) |
| multitool preview | `session/multitool.rs:1574` | `to_assembly` | no |
| head fit | `session/compute/generation.rs:451`, `compute/simulate.rs:2722` | `to_assembly` | no |
| `RapidClearanceCheck::new` | `stock/collision.rs:968` | envelope LUT, core limits, flute length | holds three checks in one struct |
| `flute_candidate`, `flute_strike`, `flute_over_tip` | `stock/collision.rs:985`, `1001`, `885` | flute length, cutter radius | check 2 (wood above the flute) |
| `drill_flute_overreach` | `stock/collision.rs:902` | drill diameter, flute length | check 2 for an analytic drill |
| `core_candidate`, `core_strike` | `stock/collision.rs:1058`, `1096` | core d, protrusion, radius, `height_at_radius` | check 3/4; `core_r = d/2` (D2) |
| `check_rapid_collisions_against_stock` | `stock/collision.rs:691` | none (a point probe) | a fifth check: the drill pre-pass |
| `RadialProfileLUT::for_stock_stamp` | `stock/radial_profile.rs:98` | profile, core d | `core_r = 0.5 d` (D2) |
| `BladeRing::of` | `dexel_stock/stamping.rs:291` | the LUT core | no (reads the LUT) |
| simulation walk | `dexel_stock/simulation.rs:518`, `653` | `cutter.length()` for `axial_doc_fraction`; the checks | no |
| `PlannerLuts::of` | `adaptive3d/path.rs:1605` | `for_stock_stamp` plus `from_cutter` | a two-table rule (cut vs envelope) that the body should own |
| 3D entry style | `adaptive3d/path.rs:2072`, `2149`, `2175` | core limits | no |
| 2D planner material | `adaptive/material_grid.rs:205` `clear_circle` | radius only | yes: clears a full disc; the simulation keeps the core (D9) |
| `HelixRadius::resolve` | `compute/config.rs:1060` | flat width, core d | `core_floor = d/2` (D2) |
| `helix_core_floor_mm` | `dressup/mod.rs:243` | core d | `d/2` again (D2) |
| 2D adaptive params | `compute/execute/clearing_2d.rs:990` | core limits | no |
| entry dressup | `compute/execute/dressup_apply.rs:698`, `732`; `dressup/entry_descent.rs:447-946` | core limits | no |
| feed optimiser, own-stock replay | `dressup/feedopt.rs:170`, `dressup/mod.rs:355` | `for_stock_stamp` | no |
| feasibility ReachFlute / ReachHolder | `compute/feasibility.rs:476`, `482` | flute, stickout (from `ToolConfig`) | yes: the flute top and the holder face, again (D4) |
| feasibility CentreCut | `compute/feasibility.rs:384` | core limits | no |
| flute guard | `feeds/envelope.rs:2479`, `feeds/mod.rs:1863` | flute length | no |
| `effective_diameter`, `engaged_diameter_at_doc` | `feeds/mod.rs:2728`, `:139` | hint, shank | yes: the tapered cap is the COLLET shank; the cutter caps at the SHAFT (D5) |
| `EngagedSection::AxialCircle` | `feeds/section.rs:75` | diameter | yes: a full circle; a non-centre tool cuts a ring (D6) |
| power at a point | `feeds/operating_point.rs:568`, `654` | shank else diameter | D3 |
| deflection predictor | `feeds/predict.rs:426`, `482` | stickout, shank else diameter | D3 |
| `ModulationCutter` builder | `session/compute.rs:2300` | hint, diameter, shank else diameter, helix | D3; a parallel value type |
| load context | `session/compute/toolpath_diagnostics.rs:46`; `feeds/suggest/adaptive_entry.rs:164` | shank with no guard | D3 (no fallback here) |
| deflection gate | `tool_load/deflection.rs:90`, `309` | `ToolDefinition.stickout` | no |
| sim prefix memo | `compute/sim_prefix.rs:457` `hash_tool` | shape probes, shank, stickout | misses the core (D1) |
| tool panel drawing | `rs_cam_viz/src/ui/properties/tool.rs:568-584` | shank r, shank length, holder face, head | yes: draws its own zones (D7) |
| simulation tool mesh | `rs_cam_viz/src/render/sim_render.rs:652-655` | shank r, shank length, stickout, head | D7 |
| tool library loader | `io/tool_library.rs` (K3 forms) | every fact | no (stores facts) |
| project file tool block | `session/project_file.rs:1340` (`ToolRecord`) | every fact | no |
| MCP `set_tool_param`, `add_tool` | `rs_cam_viz/src/mcp_server.rs:1273`, `1364`; `rs_cam_mcp/src/server.rs:1469` | every fact | no |
| MCP project summary | `rs_cam_viz/src/app/mcp/project.rs:123` | shank | no |
| CLI job tool | `rs_cam_cli/src/job.rs:1222` | every fact | no |

### 1.2 The second definitions

- D1 (defect, verify first): `hash_tool` (`compute/sim_prefix.rs:457`) hashes no core fact.
  Since P22-0a the stamp depends on the core. Two tools that differ only in `centre_cutting` can
  share a memo key, so the memo can reuse a stock stamped with the wrong table.
- D2: `core_r = core_d / 2` at four sites: `radial_profile.rs:101`, `collision.rs:1103`,
  `config.rs:1061`, `dressup/mod.rs:244`.
- D3: "shank if above 0, else the cutter diameter" at five sites: `session/compute.rs:2303`,
  `feeds/predict.rs:482`, `feeds/operating_point.rs:568` and `:654`, `tool_config.rs:1303`
  (there: else the envelope). Two more sites read the shank with no guard.
- D4: the flute top and the holder face are re-derived in `feasibility.rs`, `tip_deflection_mm`,
  `ToolAssembly`, `ToolDefinition` and the viz drawings.
- D5: a tapered ball's engaged width stops at `shank_diameter` in the hint path and at
  `shaft_diameter` in the cutter. The values differ when the shank and the shaft differ.
- D6: the plunge section of a non-centre-cutting tool is a full circle; the cut is a ring.
- D7: the GUI draws the shank and the holder from raw fields.
- D8: `envelope_diameter` and `geometry_hint` on `ToolConfig` copy the cutter (kept, pinned).
- D9: the 2D adaptive material grid clears a full disc for a non-centre-cutting tool.

### 1.3 Why the assembly lives in `stock/`

`ToolAssembly` started as the input of the MVP holder check in `stock/collision.rs`, and it
stayed there. `machine/head.rs:37` imports `HeadBand` from `stock/`, so the machine depends on the
stock model. The tool folder file says "A holder and shank envelope is part of the tool". The
code does not follow that rule today.

## 2. Ownership: the body model

New module `tool/body.rs`. The tool owns its zones; the machine owns the head parts.

```rust
/// One axial span, in mm above the tool tip.
pub struct AxialSpan { pub bottom_mm: f64, pub top: SpanTop }
pub enum SpanTop { At(f64), Open }            // Open: a head shelf

pub enum Zone { Cutting(CuttingZone), Contact(ContactZone) }

pub struct CuttingZone {
    pub span: AxialSpan,
    pub inner_mm: f64,                        // 0, or the core radius (blade ring)
    pub kind: CuttingKind,                    // the outer edge is the MillingCutter profile
}
pub enum CuttingKind { Flutes }

pub struct ContactZone {
    pub span: AxialSpan,
    pub inner_mm: f64,
    pub outer: ContactRadius,
    pub kind: ContactKind,
    pub name: Option<String>,                 // a head part name, for the finding text
}
pub enum ContactRadius { Mm(f64), Shelf }     // Shelf: no width limit
pub enum ContactKind { Core, FluteTop, Shank, Holder, Shoe }

pub struct ToolBody { zones: Vec<Zone> }      // tool-owned zones only
pub struct MountedBody { body: ToolBody, head: Vec<ContactZone> }  // plus the head at the face
```

Rules the types hold:

- No `Default` on any type. No `..` in a struct literal. `ToolBody` has no public field and one
  constructor, so no surface builds a zone list by hand (tests use the same constructor).
- Every consumer matches `Zone`, `CuttingKind`, `ContactKind` and `ContactRadius` with no
  wildcard arm (`clippy::wildcard_enum_match_arm` is denied). A new kind (Bearing, Neck, Insert)
  is a compile error at the finding table, the drawing, the obstacle footprint and the kernel.
- `FluteTop` is the face where the flutes end: bottom = flute length, radius = the cutter
  envelope radius. It is today's "wood above the flute inside the cutter radius" (check 2).
- `Core` is the centre core of a non-centre-cutting tool: bottom = the blade protrusion,
  radius = `core_d / 2`, top = the flute length. It is today's check 3/4.
- `Holder` and `Shoe` come from `HeadBand` (`AssemblySegment::Holder`, `Shoe`). A band with
  `radius_mm: None` is `ContactRadius::Shelf`.

What it replaces and what stays:

| Type today | After P22-0c |
|---|---|
| `ToolAssembly`, `BodyBand`, `segments`, `non_fluted_body`, `shelf_offset` | `MountedBody` and its views; deleted |
| `HeadBand` (`stock/collision.rs`) | moves to `machine/head.rs`; `MountedBody` takes `&[HeadBand]` |
| `HolderSegment`, `segments_from_profile` (`#[cfg(test)]`, S29) | deleted |
| `ToolDefinition.shank_diameter`, `.stickout`, `.non_centre_cutting` | one field `body: ToolBody` |
| `MillingCutter::non_centre_cutting` | deleted from the trait; readers take the body view |
| `NonCentreCuttingLimits` | stays as the entry view, built only by `CoreZone::limits()` |
| `AssemblySegment` | stays as the wire name; `From<ContactKind>` with no wildcard (`Cutter` stays for K1) |
| `MillingCutter`, the five shapes | stay: the cutting edge and the drop-cutter math |
| `ToolGeometryHint`, `ModulationCutter`, `EngagedSection` | stay as feeds views (536 hits); D3/D5/D6 read the body |
| `RadialProfileLUT` | stays; built from a body view, not from the trait |

## 3. Derivation: one constructor

`ToolBody::from_config(tool: &ToolConfig, cutter: &dyn MillingCutter) -> ToolBody`, called once
in `build_cutter` after `ToolConfig::validate`. It cannot fail, because `validate` already holds
the domains (F029). The zones, from the tip up:

1. `Cutting(Flutes)`: span `[0, L]`, `inner_mm` = core radius for `CentreCutting::No`, else 0.
2. `Contact(Core)` (only for `No`): span `[protrusion, L]`, radius `0..core_d/2`.
3. `Contact(FluteTop)`: span `[L, L]`, radius `cutter.envelope_radius_mm()`.
4. `Contact(Shank)`: span `[L, stickout]`, radius `shank_d / 2`, only when `stickout > L` (the
   rule of `exposed_shank_length` today).

`L`, `shank_d` and `stickout` come from the `*_planning_mm()` readers. Those readers hold the one
default per fact (`LEGACY_DEFAULT_*`, F201 rows), and the lineage already warns
`tool.fact_unknown` for each `Unknown` fact. The body adds no default and no new refusal.
`CentreCutting::Unknown` stops generation (D12, S11) before any body reader runs; a body built for
it (the simulation of an old result) is a centre-cutting body, as today.

`ToolBody::mounted(&self, fit: &HeadFit) -> MountedBody` appends `MachineHead::bands(stickout,
shoe)`. A stickout shorter than the flute (FC0, D4) keeps both zones as stated. The holder zone
then starts below the flute top. The kernel reports each zone, so nothing hides.

## 4. Consumers: one view each

| System | View | Replaces |
|---|---|---|
| simulation stamp, planner cut stamps, feedopt, own-stock replay | `body.cut_table()` -> `RadialProfileLUT` with the ring | `for_stock_stamp(cutter)` reading the trait |
| rapid probe, planner entry stamps | `body.envelope_table()` -> the full-disc LUT | `from_cutter` at those sites; `PlannerLuts` keeps two tables from the body |
| contact kernel | `mounted.contact_zones()` | the three checks in `RapidClearanceCheck`, `check_collisions` |
| entry emitters, feasibility CentreCut, HelixCore | `body.core()` -> `CoreZone { radius_mm, protrusion_mm }`; `core.limits()` | four `d/2` sites (D2), `non_centre_cutting()` |
| `HelixRadius::resolve`, `helix_core_floor_mm` | `body.core().map(|c| c.radius_mm)` | D2 |
| feasibility ReachFlute, flute guard, sim `axial_doc_fraction` | `body.flute_top_mm()` | D4 |
| feasibility ReachHolder, `tip_deflection_mm`, deflection gate | `body.holder_face_mm()`, `body.shank()` | D4 |
| engagement and power section | `body.cut_width_at(z)`, `body.upper_diameter_mm()` | D3, D5, D6 |
| obstacle footprints (K1), tier flute reach | `mounted.contact_zones()`, `mounted.first_above_flutes()` | `ToolAssembly` |
| GUI tool panel, simulation tool mesh | `mounted.zones()` | D7 |
| sim prefix memo | `body.digest(hasher)` (destructures every field) | D1 |

No consumer computes a span or a radius. A sentry (`one_body_for_every_reader_p22_0c`) builds
every shipped tool type, with and without a core, and asserts that each view equals the zone
table. An `rg` guard in the same test fails on `centre_core_diameter_mm / 2` or on
`shank_diameter_planning_mm() > 0.0` outside `tool/body.rs`.

## 5. The one contact check

### 5.1 The kernel

`stock/contact.rs` (new; `collision.rs` keeps the result types). One function:

```rust
pub trait ContactSolid {          // the live dexel stock, the model mesh
    fn for_each_top_in_ring(&self, x: f64, y: f64, r_in: f64, r_out: f64, f: impl FnMut(f64, f64, f64));
}
pub fn zone_contact(zone: &ContactZone, tip: P3, solid: &impl ContactSolid, sweep: Option<&MoveSweep>) -> Option<f64>;
```

A zone meets the solid at a tip position when some top inside its ring stands above
`tip.z + zone.span.bottom_mm` by more than the float slack (`low_channel_epsilon_mm`, and
`NON_CENTRE_CUTTING_TOL_MM` for the core, as today). `MoveSweep` is P22-0b's exact rule: a cell
takes the lower of its top before the move and the lowest cut of this move's cutting zone before
the sample. The walk samples each move as today (at most 1 mm, at least one cell, the end
included). It records one contact per move per zone kind (the first sample).

### 5.2 Findings: ids, texts and severity

The ids and texts stay. The map is one exhaustive function `contact_finding(kind, motion,
source)`:

| Motion | Zone kind | Solid | Id (unchanged) | Tier |
|---|---|---|---|---|
| rapid | every zone, cutting included | live stock | `project.rapid_collision` | `warn(Danger::Machine)` |
| fed | `Core` | live stock, `MoveSweep` | `safety.non_centre_cutting_descent` | `warn(Danger::Tool)` |
| fed | `FluteTop` | live stock (drill: analytic stock) | `tool.flute_overreach`; clean = `tool.flute_reach_within` (Info) | `warn(Danger::Tool)` |
| fed | `Shank`, `Holder`, `Shoe` | model mesh | `project.holder_collision`; failed = `project.holder_check_failed` | `warn(Danger::Machine)` |
| fed | `Shank`, `Holder`, `Shoe` | live stock | `project.holder_collision` (new source, P22-0c-4) | `warn(Danger::Machine)` |
| any | every zone | setup obstacles (K1) | `project.obstacle_collision` | `warn(Danger::Machine)` |

The generation-stage rows stay as they are, and read the body views: `tool.reach_past_flute`,
`tool.reach_to_holder`, `safety.non_centre_cutting_plunge`, `safety.helix_inside_core`. A rapid
into stock is Machine danger, a fed rub of the core or the flute top is Tool danger, a holder
contact is Machine danger. Every row warns; generation and export always give output (K6, Q6).

### 5.3 No case is lost

| Old check | Its cases | Kernel case | Sentries that prove it |
|---|---|---|---|
| `check_collisions` (mesh) | shank, holder, head bands per sample; the skip of a band no wider than the cutter; `min_safe_stickout` | `Shank`/`Holder`/`Shoe` on `ModelMesh`; the skip stays a rule of the mesh solid (the drop proof holds only for the finished surface) | `the_collision_walk_is_the_same_in_parallel_perf7`, `a_failed_collision_check_is_not_a_clean_one_core_g_colfail`, `the_machine_head_guards_the_checks_h1`, `tier_band_overlap_g_overlapfill`, `kinematic_surfacing_p4`, `perf_golden_sim_metrics` |
| flute overreach | wood over the tip above `L` inside `R`, fed moves; the drill per hole | `FluteTop` without `MoveSweep` (the pre-move stock, as today) | `a_hole_deeper_than_the_flute_warns_k5_reachflute`, `a_job_with_each_finding_exports_and_names_it_k6` |
| `core_strike` + P22-0b | the plunge, the steep ramp, a split descent, a helix inside the core | `Core` with `MoveSweep` | `a_split_descent_strikes_like_one_move_g_simcore`, `a_non_centre_cutting_tool_never_plunges_into_stock_g_centrecut`, `a_rough_plans_its_cut_and_link_descents_to_the_core_limit_k5_centrecut`, `a_helix_inside_the_core_leaves_a_post_g_simcore` |
| rapid strike | the two channels (low, high less the rim tolerance) on the envelope | rapid arm keeps `clearance_bounds_for_profile` on `body.envelope_table()` unchanged | `rapid_check_catches_real_side_strikes_g_rapid6497`, `rapid_check_catches_shallow_plunges_g_rapidplungetol`, `rapid_collision_detector_population` |
| drill pre-pass | a point probe on the frozen snapshot, the F3 carve-out | stays as is in P22-0c; folding it in is open question Q5 | `drill_peck_reentry_rapid_does_not_collide` (lib) |

## 6. Fit with the architecture

- Core stays GUI-free. `tool/body.rs` depends on `machine/head.rs` (`HeadBand`, `HeadFit`), as
  `to_assembly` does today. `stock/` reads `tool/`; `machine/` no longer reads `stock/`.
- The body is derived. No new mutation path: the facts change only through
  `ProjectSession::apply(Command)` (`SetToolParam`, `ReplaceTool`, add tool). The stale rules do
  not change, because a fact edit already stales every toolpath of the tool.
- Project file: no new key, no migration, no `RETIRED_KEYS` row. The save bytes do not change.
  A future zone kind adds an optional K3 fact (for example `bearing = { stated = .. }`); the
  `ToolRecord` literal with no `..` makes each loader state it (project file, tool library, CLI
  job, MCP).
- MCP and CLI parity: no parameter changes. One read-only addition, asked as Q6: the project
  summary and `inspect_collisions` name the zone kind of each contact (the GUI shows it too).
- Folder files to update: `tool/` (body.rs, the ownership invariant, sentries), `stock/`
  (contact.rs, `ToolAssembly` gone), `dexel_stock/` (G-SIMCORE: the cut table of the body),
  `machine/` (`HeadBand`), `compute/` (the "tool owns its non-centre-cutting limits" line),
  `dressup/`, `adaptive3d/`, `feeds/` (D3, D5, D6), `diagnostics/` (the table in 5.2), and the
  viz `render/` file.

## 7. Phases (one agent each; the lead verifies)

The "byte-identical" proof of each phase: the G-code bytes and the findings of a plain end mill
do not change. Run `every_op_keeps_its_toolpath_g_opgolden` (diff against
`opgolden_known_46_rows.txt`), `perf_golden_sim_metrics`, `sim_prefix_memo_s5`,
`a_job_with_each_finding_exports_and_names_it_k6`, and the folder sentries of section 5.3.

- P22-0c-1 (types, no behaviour): add `tool/body.rs` and the constructor. Move `HeadBand` to
  `machine/head.rs`. `ToolDefinition` holds the body; `to_assembly` returns `MountedBody` with the
  old views. Fix D1 with `body.digest`. New sentries: `one_body_for_every_reader_p22_0c` (the zone
  table of every type), and a memo arm in which two tools differ only in `centre_cutting` and get
  two keys. Blast radius: `ToolAssembly` 34/8, `HeadBand` 15/4, `to_assembly` 17/11,
  `AssemblySegment` 44/13. Byte-identical for every tool; the memo key changes only by design.
- P22-0c-2 (the core and cut views): `body.core()`, `cut_table()`, `envelope_table()`.
  Delete `MillingCutter::non_centre_cutting` (9/6) and the four `d/2` sites. `PlannerLuts`,
  `HelixRadius`, the 2D params and the dressups read the body. Waits for P22-0b to land.
  Byte-identical for every tool, the non-centre-cutting tools included.
- P22-0c-3 (the kernel on the live stock): `stock/contact.rs`; the walk calls it for `Core` and
  `FluteTop`; `RapidClearanceCheck` keeps only the rapid arm. `drill_flute_overreach` reads the
  `FluteTop` zone. Byte-identical findings for every tool (the `FluteTop` arm keeps the pre-move
  stock). Blast radius: `RapidClearanceCheck` 45/14, `core_strike` 9/4, `flute_strike` 5/2.
- P22-0c-4 (new coverage, needs Q1 and Q2): `Shank`, `Holder`, `Shoe` on fed moves against the
  live stock; non-cutting zones above the flutes on rapids. Not byte-identical in findings, by
  design. New sentries: a shank wider than the cutter meets standing wood; a holder meets the
  unroughed stock in a deep pocket; a plain end mill (shank no wider than the cutter) in a cleared
  pocket gives no new finding.
- P22-0c-5 (the other readers): the mesh holder check runs the kernel on `ModelMesh`; obstacle
  footprints, `tier_flute_reach` and the GUI drawings read `MountedBody`; delete `ToolAssembly`.
  Byte-identical. Blast radius: `check_collisions` 29/6, `non_fluted_body` 4/3, viz 2 files.
- P22-0c-6 (feeds second definitions, needs Q3): one `body.upper_diameter_mm()` (D3, 7 sites);
  the tapered cap (D5); the ring plunge section (D6: `A = pi (D^2 - d_core^2) / 4`, from the
  zone radii); `tip_deflection_mm` reads the spans (D4). Byte-identical for a plain end mill;
  the numbers change for a tapered ball whose shank and shaft differ and for a non-centre plunge.

D9 (the 2D planner clears the core) is not in a phase. A lateral pass sweeps the core path with
its leading ring, so the gap is at the entries only, and P22-0b already makes the entry ramp or
helix cut the core. Q4 asks if it needs a ring clear.

## 8. Open questions for the operator

- Q1. After a fresh simulation, does the stock holder check supersede the mesh holder check (as
  `tool.flute_reach_within` supersedes `tool.reach_past_flute`)? Recommendation: yes. Keep the
  mesh check for the answer before a simulation and for `min_safe_stickout`.
- Q2. Use P22-0b's exact in-move rule for every zone, not only the core? It removes
  `tool.flute_overreach` reports where a descending ramp's own flutes cut the wood behind the tip.
  Recommendation: yes, as a later phase, after a count on the `planning/fixtures/` projects.
- Q3. The tapered engaged width: stop at the shaft (the cutter) or at the collet shank (the hint
  path today)? Recommendation: the shaft, because the cutter is the geometry.
- Q4. Does the 2D adaptive material grid need a ring clear for a non-centre-cutting tool (D9)?
  Recommendation: no, unless P22a shows a plan that differs from the simulation.
- Q5. Fold the drill pre-pass (`check_rapid_collisions_against_stock`) into the kernel? It is a
  point probe on a frozen snapshot. Recommendation: yes, after P22-0c-4.
- Q6. Show the zone kind of each contact in the GUI, MCP and CLI (one chip, no new text block)?
  Recommendation: yes, on the existing collision marker.

## 9. Rulings (2026-11-09)

- Plan approved (operator). P22-0c-1 starts when P22-0b and the D1 hash fix are committed.
- Q1 yes, Q2 yes as a later phase after a count on `planning/fixtures/`, Q4 no unless P22a shows
  a plan/simulation difference, Q5 yes after P22-0c-4 (lead, on the recommendations).
- Q3 (operator): the engaged width of a tapered ball stops at the SHAFT. "A R2 x 6 mm x 30.5 mm
  has a cutting edge of 30.5 mm with a shank above it that does not cut." The cutting zone is the
  fluted length; the shank above it is a contact zone.
- Q6 (operator): yes, one chip on the existing collision marker names the zone kind; no new text.
- D1 (`hash_tool` without the core facts) is a live defect since P22-0a: fixed ahead of P22-0c by
  a separate small change (`P22-0c_D1.md`).
- Added from the P22-0b wanaka read (`P22-0b_wanaka.md`, lead 2026-11-09), all in P22-0c scope:
  1. `clamp_descents_to_limit` reads the core-zone dwell rule (two wanaka cut descents still run at
     7.125 deg; 0.03-0.05 mm past the protrusion).
  2. The static `safety.non_centre_cutting_plunge` (idx 14 move 21244, 1.79 mm, critical) and the
     simulation core check disagree on one move: the one contact kernel must give one answer.
  3. `safety.non_centre_cutting_descent` shows only in the export header: it must reach
     `get_toolpath_diagnostics`, the project diagnostics and the GUI list (one finding, every
     surface). Its text still states the old 7.13 deg limit.
  4. Finding positions: setup 1 reports the model frame, setup 3 the G-code frame. One stated frame
     on every contact finding.
