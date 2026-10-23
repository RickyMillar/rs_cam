# Underived defaults inventory (2026-10-23)

Ruling S11 (operator, 2026-10-23, `PLAN.md` commit 7e0e701e): a default
value must come from a derivation. The derivation is the model, the tool,
the stock, the machine, a sourced rule or a stated safety reason. A value
with no derivation is removed. The field shows empty, and the op refuses
to generate until the operator sets it.

This file is research only. Nobody edited repo files, and nobody ran cargo.
Build state: HEAD 88d18baf.

## 0. Method and keys

Paths: `OC` = `crates/rs_cam_core/src/compute/operation_configs.rs`,
`CF` = `crates/rs_cam_core/src/compute/config.rs`,
`AP` = `crates/rs_cam_core/src/feeds/suggest/apply.rs`,
`SC` = `crates/rs_cam_core/src/compute/stock_config.rs`.

Class tags (each row has exactly one tag; the counts use them):

| Tag | Meaning |
|---|---|
| `[D]` | DERIVED: from the model, tool, stock, machine or feeds. The row says from what. |
| `[S]` | SOURCED: a cited reference, a formula in the repo, or a repo measurement that the code cites (marked "measured"). |
| `[SF]` | SAFETY: the code or a ruling states a safety reason (quoted). |
| `[C]` | CONVENTION: `REFERENCE_AUDIT.md` or `INDUSTRY_DEFAULTS.md` shows the reference tools use the same value. |
| `[U]` | UNDERIVED: a value with no reason. A removal candidate. |
| `[D/U]` | Suggest writes the value at the add when the feeds cell is backed (DERIVED from the tool, the material and the machine). When Suggest refuses `Unbacked` (ruling R1), the raw struct number ships. That raw number is UNDERIVED. |
| `[OFF]` | The value turns a feature off (`false`, `0` = off, `None` = rule). It asks for no math. It is outside the ruling. |
| `[RULED]` | A choice (enum or bool) that an operator ruling or a lead ruling names. It is not a number. |

Notes on the method:

1. The `OpParams` derive (`crates/rs_cam_derive/src/lib.rs`) sets no
   default values. It reads `help`, `range`, `min`, `gt`, `max`,
   `optional`, `frame_dependent`, `unbounded`, `skip` and `driver_set`
   only. Every default value is in a `Default` impl or a `default_*` fn.
2. Every field of every op config is published (no `skip`). The
   `driver_set` fields are `selection` (all ops), `floor` (Drill, Bore,
   Pocket, Profile, Adaptive, Zigzag) and the Pin Drill `diameter`. The
   user edits `selection` and `floor` in their own editors, so they count.
3. The value that a user meets is the value after the add pipeline, not
   `Default::default()`. Section 1 gives the pipeline.
4. An unset `Option` field (`None`) is not a default value. It is listed
   only where `None` resolves to a numeric rule.

## 1. The add pipeline (what lands at the add)

| Door | Pipeline | Where |
|---|---|---|
| GUI | `suggest_params` = `new_default` + `apply_stock_defaults` + Suggest `apply(Both)`. On `Unbacked`: `default_operation` (no Suggest). Then `DressupConfig::for_op`, `HeightsConfig::default`, `BoundaryConfig::for_3d_op(D)` on a 3D op with a mesh. Then `Command::AddToolpath` runs `each_face_floor_due` / `each_hole_floor_due`. | `crates/rs_cam_viz/src/controller/events/toolpath.rs:73-230`; `crates/rs_cam_core/src/feeds/suggest.rs:901-931`; `crates/rs_cam_core/src/session/mutation/toolpath.rs:34-69` |
| MCP `add_toolpath` | The same pipeline as the GUI. | `crates/rs_cam_viz/src/app/mcp/commands.rs:787-900` |
| CLI job | `OperationConfig::new_default` with NO stock defaults and NO Suggest. `DressupConfig::default()`, not `for_op`. `BoundaryConfig::default()`. Then the job keys go through the setters. | `crates/rs_cam_cli/src/job.rs:1200-1220` |

Finding (parity): the CLI job ships the raw struct numbers for each key
that the job file does not state. Examples: Pocket floor 3 mm (GUI: min(half
the stock, 5 mm)); Drop Cutter `min_z` -50 (GUI: the stock bottom); Profile
floor 6 mm (GUI: through the stock); no Ramp entry on a rough. The rule in
`crates/rs_cam_cli/CLAUDE.md` says "an unset key keeps the registry default",
so this is by design, but the two doors give different numbers.

### 1.1 Add-time rules

| Op / field | Value at the add | Where | Class |
|---|---|---|---|
| Pocket `floor` | `Floor::top(min(stock_z / 2, 5))` | `AP:1145` | `[U]` The ruled first case. No record gives a reason for "half" or "5". |
| Adaptive (2D) `floor` | `Floor::top(stock_z / 2)` | `AP:1148` | `[U]` The same shape as Pocket. No reason for "half". |
| Profile `floor` | `Floor::top(stock_z)`: through the stock to its bottom | `AP:1139` | `[C]` FreeCAD Profile FinalDepth = stock bottom; audit S11 MATCH. Also `[D]` from the stock. |
| Drill `floor` | `Floor::top(stock_z)`: through the stock | `AP:1142` | `[D]` from the stock (a drawing has no hole depth). The tip stops at the stock bottom, so the full diameter does not break through. Better: add the tip cone height `r / tan(half point angle)` from the tool (ID Q2, Fusion "Drill tip through bottom"). |
| Drill `floor` (STEP) | `Floor::EachHole`: each hole to its own floor | `crates/rs_cam_core/src/session/heights.rs:756` | `[D]` from the model (hole floor). `[C]` FreeCAD/Fusion hole bottom (ID Q2). |
| Pocket, Profile, Zigzag, Adaptive `floor` (STEP face below the stock top) | `Floor::EachFace { depth: 0 }` | `crates/rs_cam_core/src/session/heights.rs:711` | `[D]` from the picked face. `[C]` audit D2 MATCH. |
| Face `depth` | `max(stock padding, 1.0)` | `AP:1136` | `[D]` from the stock padding (the auto stock adds `padding` on the top, `SC:282-296`). The `1.0` floor is `[U]`. With `auto_from_model = false` the padding is not the top skin, so the derivation fails. |
| Drop Cutter `min_z` | the stock bottom Z | `AP:1133` | `[D]` from the stock. |
| Zigzag `floor` | raw `Floor::top(3.0)`: no stock default | `OC:1324`; no arm in `AP:1150-1170` | `[U]` |
| Bore `floor` (direct add) | raw `Floor::top(3.0)`: no stock default | `OC:531`; `AP:1155` lists Bore with "no stock-aware default" | `[U]` "Add a Bore for these" copies the Drill floor (`[D]`). |
| Tool | the first project tool that the op accepts | `crates/rs_cam_viz/src/controller/events/toolpath.rs:88-150` | `[D]` from the tool list and the registry tool rule. Audit S9 proposes "the tool of the last op" (FreeCAD). |
| Model | the first model of the project | `crates/rs_cam_viz/src/controller/events/toolpath.rs:83-88` | `[U]` A choice, not a number. Ruling Q6 says "the setup models"; not built. |
| Boundary (3D op, mesh) | on, model silhouette, `Center`, offset = tool diameter | `CF:775-800` (`for_3d_op`) | `[D]` from the tool (R2, `planning/corne_case_analysis_2026-09-18/ANALYSIS.md` §5). The offset is a copy and does not follow a tool change. |
| Selection | 2D ops: every item the op can cut; Face: the stock; Pin Drill: empty | `OC` each `selection:` default | `[RULED]` D2/D13, audit S1/S2 DIFFERS-KEEP. |
| Stock source | `Fresh`; coolant `Off` | `crates/rs_cam_viz/src/controller/events/toolpath.rs:221-222` | `[OFF]` |
| Rest analysis | off; detector values from `RestFieldParams::default()` | `CF:871-887` | `[OFF]` The values matter only when the operator turns it on. |

## 2. Feeds fields (one row per op group)

Suggest writes these at the add when the cell is backed. The table gives
the raw number that ships when Suggest refuses `Unbacked`, and in the CLI
job. The Suggest write: feed, plunge and RPM on every op (`AP:140-192`);
`stepover` and `depth_per_pass` only where the op names that role
(`OC:3268-3460`); `ramp_feed_rate` by the G6/G10 rule; `peck_depth` by
`apply_drill_defaults` (`AP:1063-1097`).

| Op | feed / plunge (raw) | stepover (raw) | depth per pass (raw) | Where | Class |
|---|---|---|---|---|---|
| Face | 1500 / 500 | 5.0 | 1.0 | `OC:273-277` | `[D/U]` |
| Pocket | 1000 / 500 | 2.0 | 1.5 | `OC:791-795` | `[D/U]` |
| Zigzag | 1000 / 500 | 2.0 | 1.5 | `OC:1323-1327` | `[D/U]` |
| Adaptive (2D) | 1500 / 500 | 2.0 | 2.0 | `OC:988-992` | `[D/U]` |
| Profile | 1000 / 500 | (none) | 2.0 | `OC:897-899` | `[D/U]` |
| Trace | 800 / 400 | (none) | 0.5 | `OC:327-329` | `[D/U]` |
| Chamfer | 800 / 400 | (none) | (none) | `OC:680-681` | `[D/U]` |
| V-Carve | 800 / 400 | 0.5 | (none) | `OC:1047-1049` | `[D/U]` |
| Inlay | 800 / 400 | 1.0 | (none) | `OC:1152-1154` | `[D/U]` |
| Inlay Plug | Pocket 1000 / 500 | 1.0 | Pocket 1.5 | `OC:1264-1270` | `[D/U]` |
| Drill | 300 (feed only) | — | — | `OC:400` | `[D/U]` |
| Pin Drill | 300 | — | — | `OC:620` | `[D/U]` |
| Bore | 1000 / 500 | — | — | `OC:535-536` | `[D/U]` |
| 3D Rough | 1500 / 500 | 2.0 | 3.0 | `OC:1557-1561` | `[D/U]` |
| Drop Cutter | 1000 / 500 | 1.0 | — | `OC:1403-1405` | `[D/U]` |
| Waterline | 1000 / 500 | — | `z_step` 1.0 | `OC:1683-1686` | `[D/U]` |
| Pencil | 800 / 400 | `offset_stepover` 0.5 (Suggest writes it only when `num_offset_passes > 1`) | — | `OC:1855-1858` | `[D/U]` |
| Scallop | 1000 / 500 | (from `scallop_height`) | — | `OC:2012-2013` | `[D/U]` |
| Unified Finish | 1000 / 500 | — (see section 7) | — | `OC:2371-2372` | `[D/U]` |
| Steep and Shallow | 1000 / 500 | 1.0 | — | `OC:2610-2613` | `[D/U]` |
| Ramp Finish | 1000 / 500 | — | `max_stepdown` 0.5 | `OC:2684-2690` | `[D/U]` |
| Spiral Finish | 1000 / 500 | 1.0 | — | `OC:2746-2749` | `[D/U]` |
| Radial Finish | 1000 / 500 | — | — | `OC:2816-2817` | `[D/U]` |
| Horizontal Finish | 1000 / 500 | 1.0 | — | `OC:2871-2873` | `[D/U]` |
| Project Curve | 800 / 400 | — | — | `OC:3071-3072` | `[D/U]` |

`spindle_rpm` and `ramp_feed_rate` default to `None` on every op. Suggest
fills them; `None` RPM falls back to the post `spindle_speed` 18000
(`crates/rs_cam_core/src/gcode/mod.rs:1467-1469`) `[U]`.

Drill `peck_depth`: raw 3.0 (`OC:397`, Pin Drill `OC:619`). Suggest writes
`0.5 × per-peck max × D` by material, capped at `0.75 × hole depth`
(`AP:1063-1128`, `crates/rs_cam_core/src/material/mod.rs:1481-1485`). The
code says "The peck depth is a repo rule ... No vendor prints a per-peck
depth". Class `[D/U]`: DERIVED from the tool and the material through a
repo rule with no source; the raw 3.0 is UNDERIVED.

## 3. 2.5D clearing and profile

| Op / field | Value | Where | Class |
|---|---|---|---|
| Face `stock_offset` | 5.0 mm | `OC:279` | `[U]` Better: the tool radius (full coverage needs the tool edge past the stock edge). |
| Face `direction` | Zigzag | `OC:280` | `[U]` A choice; low value to remove. |
| Pocket `climb` | true | `OC:797` | `[RULED]` H6 (one direction rule for Pocket, Profile and Bore). |
| Pocket `pattern` | Contour | `OC:798` | `[U]` A choice. |
| Pocket `angle` | 0° | `OC:799` | `[D]` The setup X axis; it is read only by the Zigzag pattern. |
| Pocket `finishing_passes` | 0 | `OC:800` | `[OFF]` |
| Pocket `region_ordering` | ByArea | `OC:130-132` | `[RULED]` R4. |
| Trace, Profile, Zigzag `region_ordering` | ByArea | `OC:333, 909, 1331` | `[RULED]` R4 (the same `default_layered_region_ordering`). |
| Pocket `open_paths` | Skip | `OC:804` | `[C]` audit O3 MATCH (ruling O-2). |
| Zigzag `angle` | 0° | `OC:1329` | `[D]` The setup X axis. |
| Adaptive `tolerance` | 0.1 mm | `OC:994` | `[U]` |
| Adaptive `slot_clearing` | false | `OC:997` | `[OFF]` (G-ADAPTPASSLOAD). |
| Adaptive `min_cutting_radius` | 0 | `OC:998` | `[OFF]` (no limit). |
| Adaptive `cleanup_strategy` | ContourParallelHybrid | `OC:1000` | `[U]` A choice. |
| Adaptive `engagement_measure` | DiskArea | `OC:1001` | `[OFF]` The 2D op does not read it (retired). |
| Adaptive `path_strategy` | Agent | `OC:1002` | `[U]` A choice. |
| Profile `side` | Outside | `OC:895` | `[C]` audit S13 MATCH. |
| Profile `climb` | true | `OC:901` | `[C]` audit S13 MATCH. |
| Profile `tab_count` | 0 | `OC:902` | `[C]` audit T1, ID Q3. |
| Profile `tab_width` | 6.0 mm | `OC:903` | `[U]` Read only when tabs are on. |
| Profile `tab_height` | 2.0 mm | `OC:904` | `[U]` Read only when tabs are on. |
| Profile `finishing_passes` | 0 | `OC:906` | `[OFF]` |
| Profile `compensation` | InComputer | `OC:907` | `[U]` A choice; InControl refuses side On (P6.1). |

## 4. Curve, engrave and inlay

| Op / field | Value | Where | Class |
|---|---|---|---|
| Trace `depth` | 1.0 mm | `OC:326` | `[U]` A drawing has no depth. |
| Trace `compensation` | None (on the line) | `OC:331` | `[C]` audit O2 MATCH. |
| Chamfer `side` | Outside | `OC:677` | `[C]` audit S14 (FreeCAD Deburr). |
| Chamfer `chamfer_width` | 1.0 mm | `OC:678` | `[C]` audit S14, weak: only FreeCAD prints 1 mm (Kiri has no op; Fusion value not found). |
| Chamfer `tip_offset` | 0.1 mm | `OC:679` | `[U]` FreeCAD ExtraDepth is 0.5 mm, so it is not a match. |
| V-Carve `max_depth` | 5.0 mm | `OC:1046` | `[U]` Better: the V-bit flute length, or the stock. |
| V-Carve `tolerance` | 0.05 mm | `OC:1051` | `[U]` |
| Inlay `pocket_depth` | 3.0 mm | `OC:1145` | `[U]` |
| Inlay `floor_overlap` | `2.0 × (1 + 1/(2 × 0.4))` = 4.5 mm | `OC:1143-1146` | `[U]` by inheritance: the formula reads the raw Pocket stepover 2.0 (`[U]`), not the bound tool. Better: the same formula on the tool diameter. |
| Inlay `floor_ridge` | `0.5 × glue gap` = 0.05 mm | `OC:1149` | `[U]` by inheritance from the glue gap. |
| Inlay `tolerance` | 0.05 mm | `OC:1156` | `[U]` |
| Inlay Plug `glue_gap` | 0.1 mm (`INLAY_GLUE_GAP_DEFAULT_MM`, `OC:1062`) | `OC:1257` | `[U]` |
| Inlay Plug `pocket_depth` | Inlay 3.0 | `OC:1256` | `[U]` Better: read the Inlay op of the female project, if one is linked. |
| Inlay Plug `proud_height` | = pocket depth (3.0) | `OC:1260` | `[U]` The code says "An operator choice, not a measured constant". |
| Inlay Plug `outer_margin` | = pocket depth (3.0) | `OC:1263` | `[U]` "An operator choice". |
| Inlay Plug `tolerance` | Inlay 0.05 | `OC:1272` | `[U]` |
| Project Curve `depth` | 1.0 mm | `OC:3069` | `[U]` |
| Project Curve `point_spacing` | 0.5 mm | `OC:3070` | `[U]` |
| Project Curve `direction`, `side`, `projection` | FromAbove, Center, Cutter | `OC:3075-3077` | `[U]` Choices. `Cutter` is the safe projection (the real cutter drop), but the code states no safety reason. |
| Project Curve `chain_distance_mm` | 0 | `OC:3062-3064` | `[OFF]` |

## 5. Holes

| Op / field | Value | Where | Class |
|---|---|---|---|
| Drill `floor` (raw, CLI only) | `Floor::Top { depth: 10.0 }` | `OC:395` | `[U]` The GUI and MCP replace it (section 1.1). |
| Drill `cycle` | Peck (G83) | `OC:396` | `[SF]` Ruling H7: "peck is a good default for saftey, gives time to e-stop if it looks too manic". |
| Drill `dwell_time` | 0.5 s | `OC:398` | `[U]` Read only by the Dwell cycle. |
| Drill `retract_amount` | 0.5 mm | `OC:399` | `[U]` Read only by the ChipBreak cycle. |
| Drill `retract_z` | 2.0 (absolute Z) | `OC:401` | `[U]` The R plane is `max(retract_z, top + 5)` (`CF:169-171`, `crates/rs_cam_core/src/compute/execute/drilling.rs:132`), so the 5 mm safety floor hides the 2.0 when the stock top is at or above Z -3. Better: an offset above the hole top (ID Q2: "R = feed height above hole top"). |
| Drill `size_tolerance` | 0.01 mm (`CAD_ROUNDING_MM`) | `OC:403, 410-412` | `[RULED]` P6.5 / ruling 2026-10-22; the value is the CAD rounding, not an op choice. |
| Bore `pitch` | 1.0 mm (`DRESSUP_HELIX_PITCH_MM`) | `OC:532` | `[U]` The code says "a repo rule with no source". |
| Bore `climb` | true | `OC:533` | `[RULED]` H6. |
| Bore `finish_allowance` | 0 | `OC:534` | `[OFF]` (one helix). |
| Pin Drill `diameter` | 6.0 (`NOMINAL_DOWEL_MM`) | `OC:616` | `[D]` driver-set: the pin door writes the pin size. |
| Pin Drill `spoilboard_penetration` | 2.0 mm | `OC:617` | `[U]` |
| Pin Drill `cycle` | Peck | `OC:618` | `[SF]` The H7 reason. |
| Pin Drill `retract_z` | 2.0 (absolute Z) | `OC:621` | `[U]` The same safety floor hides it. |
| Pin Drill dwell / chip-break retract | 0.5 s / 0.5 mm, hard-coded, not params | `OC:606, 608` | `[U]` The operator cannot see or set them. |

## 6. 3D rough (Adaptive3d)

| Field | Value | Where | Class |
|---|---|---|---|
| `stock_to_leave_axial` | 0.5 mm | `OC:1559` | `[U]` |
| `tolerance` | 0.1 mm | `OC:1563` | `[U]` |
| `min_cutting_radius` | 0 | `OC:1564` | `[OFF]` |
| `entry_style` | Plunge | `OC:1565` | `[U]` A choice. The dressup gives Helix to a `DefaultHelix` op (G10 D3). |
| `ramp_angle_deg` | 10° (`ADAPTIVE3D_RAMP_ANGLE_DEG`, `OC:1605`) | `OC:1566` | `[U]` The code says "Repo rule, no source (G10). No wood or router source prints a ramp angle". |
| `helix_radius_factor` | 0.3 × D (`HELIX_RADIUS_OVER_D`, `CF:941`) | `OC:1567` | `[D/U]` It scales with the tool, but the factor is "Repo rule, no source". The flat-bottom cap is `[S]` (IMCO, Sandvik, Fusion, ruling Q6). |
| `helix_pitch` | 2.0 mm (`ADAPTIVE3D_HELIX_PITCH_MM`, `OC:1612`) | `OC:1568` | `[U]` "Repo rule, no source". |
| `entry_clearance_mm` | 0.5 mm (`ENTRY_CONTACT_CLEARANCE`) | `OC:1569`; `crates/rs_cam_core/src/dressup/entry_descent.rs:413` | `[U]` The doc gives only "the same 0.5 mm the adaptive3d planner's rapid floor keeps". |
| `detect_flat_areas` | false | `OC:1570` | `[OFF]` |
| `region_ordering` | Global | `OC:1571` | `[U]` A choice. |
| `clearing_strategy` | ContourParallel | `OC:1572` | `[U]` A choice. |
| `trochoid_cap_mult` | 1.6 | `OC:1573, 1592-1594` | `[S]` (owned by adaptive_research_2026-10-23; not removed here) measured: "the balanced knee from the cap sweep" (`planning/ADAPTIVE_CLEARING_ALGO_REVIEW_2026-06-12.md`). |
| `engagement_measure` | DiskArea | `OC:1574` | `[U]` "the historical" value. |
| `z_blend` | false | `OC:1575` | `[OFF]` |
| `min_region_cut_length_mm` | 15.0 mm | `OC:1577, 1543-1552` | `[S]` measured on the Wanaka Back Rough (F-038). One fixture and one 6 mm tool; better: `2.5 × D` from the stated "2× tool diameter" heuristic. Doc defect: the field doc (`OC:~1514`) says "Default 5.0 mm"; the code is 15.0. |
| `max_stay_down_distance_mm` | None = 8 × D | `OC:1581`; `crates/rs_cam_core/src/adaptive3d/path.rs:1192` | `[D]` from the tool. The factor 8 cites "Fusion HSM's typical" value with no reference. |
| `stay_down_clearance_mm` | 0.5 mm | `OC:1582, 1535-1541` | `[SF]` "smaller values risk grazing the surface on imprecise heightfields". The value matches the dexel cell at standard settings. |

## 7. 3D finish

| Op / field | Value | Where | Class |
|---|---|---|---|
| Drop Cutter `slope_from` / `slope_to` | 0° / 90° | `OC:1408-1409` | `[D]` The whole slope domain (no filter). |
| Drop Cutter `scallop_height` | None | `OC:1411` | `[OFF]` |
| Drop Cutter `hookup_mm` | 0 | `OC:1414` | `[OFF]` |
| Waterline `sampling` | 0.5 mm | `OC:1684` | `[U]` Better: from the tool radius or the simulation cell. |
| Waterline `continuous` | false | `OC:1688` | `[U]` A choice. |
| Waterline `hookup_mm` | 0 | `OC:1691` | `[OFF]` |
| Pencil `bitangency_angle` | 160° | `OC:1851` | `[U]` The help says "140-170 deg typical" with no source. |
| Pencil `min_cut_length` | 2.0 mm | `OC:1852` | `[U]` |
| Pencil `hookup_distance` | 5.0 mm | `OC:1853` | `[U]` |
| Pencil `num_offset_passes` | 1 | `OC:1854` | `[U]` |
| Pencil `sampling` | 0.5 mm | `OC:1856` | `[U]` |
| Pencil `stock_to_leave` | 0 | `OC:1860` | `[D]` A finish op cuts to the surface. |
| Pencil `min_valley_depth` | 0.05 mm | `OC:1861`; `crates/rs_cam_core/src/finish/pencil/chain_paths.rs:191` | `[U]` The code says "Tunable; calibrate against the real mesh harness". |
| Pencil `bisector_strength` | 1.0 | `OC:1862`; `crates/rs_cam_core/src/finish/pencil.rs:331` | `[D]` "1.0 = the geometrically-correct offset". |
| Pencil `reference_tool_diameter` | 6.0 mm | `OC:1863`; `crates/rs_cam_core/src/finish/pencil.rs:337` | `[U]` "A typical 1/4" finish ball". Better: the tool of the finish op before it (`reference_tool_id`), as `claims_reference: Auto` does. |
| Pencil `detector` | Dihedral | `OC:1864` | `[U]` "The historical default". |
| Pencil `valley_saliency` | 0.15 /mm | `OC:1865`; `pencil.rs:347` | `[S]` measured on the Wanaka mesh (0.05 / 0.2 / 0.8 stated). |
| Pencil `curvature_smoothing` | 4 | `OC:1866`; `pencil.rs:354` | `[U]` No measurement stated. |
| Pencil `rest_cell_mm` | 0.5 mm | `OC:1867`; `pencil.rs:360` | `[U]` The doc gives a run time on Wanaka, not a derivation. |
| Pencil `link_hop_distance_mm` | None = `hookup_distance` | `OC:1870` | `[OFF]` |
| Scallop `scallop_height` | 0.1 mm | `OC:2006` | `[U]` The finish target. Suggest derives the stepover from it, so the target itself must come from the operator. |
| Scallop `tolerance` | 0.05 mm | `OC:2007` | `[U]` |
| Scallop `direction` | OutsideIn | `OC:2008` | `[U]` A choice. |
| Scallop `continuous` | false | `OC:2009` | `[U]` A choice. |
| Scallop `slope_from` / `slope_to` | 0° / 90° | `OC:2010-2011` | `[D]` The whole domain. |
| Scallop `stock_to_leave` | 0 | `OC:2015` | `[D]` A finish op. |
| Scallop `intra_pass_hookup_mm` | 3.0 mm | `OC:2017, 1999-2001` | `[S]` measured (wave 12: retracts 24 → 3, zero new collisions). The size is "a little over one Ø3-ball diameter"; better: from the bound tool D. |
| Scallop `iso_field` | false | `OC:2018` | `[OFF]` |
| Unified Finish `steep_threshold_deg` | 45° | `OC:2362` | `[S]` measured, P2.e sweep 2026-07-08 (`OC:2356-2359`). |
| Unified Finish `waterline_threshold_deg` | 75° | `OC:2363` | `[S]` measured, the same sweep ("65→55 measured +27% finish time"). |
| Unified Finish `overlap_mm` | 2.0 mm | `OC:2364` | `[U]` |
| Unified Finish `scallop_height` | 0.1 mm | `OC:2365` | `[U]` The finish target. |
| Unified Finish `tolerance` | 0.05 mm | `OC:2366` | `[U]` |
| Unified Finish `raster_stepover` | 1.0 mm | `OC:2367` | `[U]` Suggest does NOT write it (stepover role `none`, `OC:~3445`). Better: from `scallop_height` and the tool, as Scallop does. |
| Unified Finish `z_step` | 1.0 mm | `OC:2368` | `[U]` Suggest does not write it. Better: from `scallop_height` and the tool. |
| Unified Finish `sampling` | 0.5 mm | `OC:2369` | `[U]` |
| Unified Finish `stock_to_leave` | 0 | `OC:2370` | `[D]` A finish op. |
| Unified Finish `pencil_claims` | false | `OC:2466-2468` | `[OFF]` |
| Unified Finish `min_rest_depth_mm` | 0.02 mm | `OC:2470-2472` | `[U]` Read only with `pencil_claims` on. |
| Unified Finish `claims_reference` | Auto | `OC:2491-2493` | `[D]` derived from the cascade (A/M6). |
| Unified Finish `territory_clip` | false | `OC:2495-2497` | `[OFF]` |
| Unified Finish `monotone_cell_decomposition` | true | `OC:2511-2513` | `[S]` the C4 rendered-surface review (`planning/thin_organic_2026-08-27/FINDINGS.md` §7). |
| Unified Finish `intra_region_hookup_mm` | 6.0 mm | `OC:2544-2546` | `[S]` measured (wave 12 table) plus an operator ruling 2026-08-03. One fixture; better: from the tool D. |
| Unified Finish `crease_hookup_mm` | 5.0 mm | `OC:2550-2552` | `[U]` "Historical `PencilParams::default().hookup_distance`". |
| Unified Finish `min_region_area_mm2`, `close_radius_mm`, `hysteresis_deg`, `classification_sampler` | None = from the tool | `OC:2385-2388` | `[D]` from the tool (F2). |
| Steep and Shallow `threshold_angle` | 45° | `OC:2606` | `[U]` The Unified Finish sweep does not cover this op. |
| Steep and Shallow `overlap_distance` | 1.0 mm | `OC:2607` | `[U]` |
| Steep and Shallow `wall_clearance` | 0.5 mm | `OC:2608` | `[U]` The help says only "Extra clearance from vertical walls". |
| Steep and Shallow `steep_first` | true | `OC:2609` | `[U]` A choice. |
| Steep and Shallow `z_step` | 1.0 mm | `OC:2611` | `[U]` Suggest does not write it (depth role `none`). |
| Steep and Shallow `sampling` | 0.5 mm | `OC:2615` | `[U]` |
| Steep and Shallow `stock_to_leave` | 0 | `OC:2616` | `[D]` A finish op. |
| Steep and Shallow `tolerance` | 0.05 mm | `OC:2617` | `[U]` |
| Steep and Shallow `hookup_mm` | 0 | `OC:2620` | `[OFF]` |
| Ramp Finish `slope_from` | 30° | `OC:2685` | `[U]` |
| Ramp Finish `slope_to` | 90° | `OC:2686` | `[D]` The top of the domain. |
| Ramp Finish `direction` | Climb | `OC:2687` | `[U]` A choice (H6 names Pocket, Profile and Bore only). |
| Ramp Finish `order_bottom_up` | false | `OC:2688` | `[U]` A choice. |
| Ramp Finish `sampling` | 0.5 mm | `OC:2692` | `[U]` |
| Ramp Finish `stock_to_leave` | 0 | `OC:2693` | `[D]` A finish op. |
| Ramp Finish `tolerance` | 0.05 mm | `OC:2694` | `[U]` |
| Spiral Finish `direction` | InsideOut | `OC:2747` | `[U]` A choice. |
| Spiral Finish `stock_to_leave` | 0 | `OC:2751` | `[D]` A finish op. |
| Radial Finish `angular_step` | 5° | `OC:2814` | `[U]` Better: from the stepover at the outer radius and the tool. |
| Radial Finish `point_spacing` | 0.5 mm | `OC:2815` | `[U]` |
| Radial Finish `stock_to_leave` | 0 | `OC:2819` | `[D]` A finish op. |
| Horizontal Finish `angle_threshold` | 5° | `OC:2870` | `[U]` |
| Horizontal Finish `stock_to_leave` | 0 | `OC:2875` | `[D]` A finish op. |

## 8. Dressups (`DressupConfig::for_op` at the add)

`for_op` = `for_role(role)` (`CF:1537-1580`), then Helix for a
`DefaultHelix` op (`CF:1590`), then `normalize_for_op`. `normalize_for_op`
strips dressups by the registry `dressup_policy` (for example Project Curve
is `strip_all`; Drill and Trace lose the entry), so "on" below is not
universal. The CLI job uses `DressupConfig::default()` instead.

| Field | Value | Where | Class |
|---|---|---|---|
| Entry style, Roughing | Ramp | `CF:1544` | `[U]` A choice (Roadmap B.5). |
| Entry style, `DefaultHelix` op (Adaptive) | Helix | `CF:1590` | `[RULED]` G10 D3. |
| Entry style, SemiFinish / Finish | None | `CF:1562-1572` | `[SF]` R10: "a plunge there is a 0.5 mm bite. A 3 degree ramp ... sawed a 3 mm wall to pins". |
| `ramp_angle` | 3° (`DRESSUP_RAMP_ANGLE_DEG`, `CF:928`) | `CF:1471` | `[U]` The code says "Repo rule, no source (G10)". |
| `helix_radius` | None = 0.3 × D, capped at the flat bottom | `CF:941, 1503-1513` | `[D/U]` The tool scales it; the 0.3 factor is "Repo rule, no source". |
| `helix_pitch` | 1.0 mm (`DRESSUP_HELIX_PITCH_MM`, `CF:934`) | `CF:1473` | `[U]` "Repo rule, no source". |
| `entry_clearance_mm` | 0.5 mm | `CF:1009-1011` | `[U]` (the same constant as section 6). |
| Dogbone | off; angle 90° when on | `CF:1028` | `[OFF]`; the 90° is `[D]` (a square corner). |
| Lead-in / lead-out, Finish role | on, radius 2.0 mm, feeds None | `CF:1573, 1048-1056` | `[U]` The radius does not scale with the tool. Better: a fraction of D. |
| Link moves | on; max 10 mm, feed 500 mm/min | `CF:1477, 1067-1073` | `[U]` both numbers. Better: the max from the tool D (3D Rough uses 8 × D) and the feed from the op feed. |
| Arc fitting | on for every role; tolerance 0.05 mm | `CF:1083-1087` | `[U]` |
| Segment merge, Roughing | on; tolerance 0.3 mm | `CF:1014-1016, 1552` | `[U]` Capped on an op with a finished edge (P6.0b). |
| Feed optimisation | on; max 3000 mm/min, ramp 200 | `CF:1480-1482` | `[U]` Better: the max from the machine profile max feed. |
| Rapid order optimisation | on | `CF:1483` | `[OFF]` (a pure win, no number). |

## 9. Heights (`HeightsConfig::default()`: all Auto)

| Height | Auto value | Where | Class |
|---|---|---|---|
| Retract | post `safe_z`, raised to stock top + 5 mm | `CF:342, 169-171` | The 5 mm floor: `[SF]` "Rapids at `safe_z` must clear the uncut stock surface" (`CF:155-162`). The size 5 has no derivation, but the reason is safety. |
| Post `safe_z` | 10.0 | `crates/rs_cam_core/src/gcode/mod.rs:1471-1473` | `[U]` The code calls it "a legacy 2D-friendly default". On most stock the 5 mm floor overrides it. |
| Clearance | retract + 10 mm | `CF:344` | `[U]` |
| Feed | retract - 2 mm | `CF:346` | `[U]` |
| Top | the stock top; the face top under a pick (UX-1) | `CF:343`; `crates/rs_cam_core/src/session/heights.rs` | `[D]` from the stock or the face. `[C]` audit D3. |
| Bottom | the floor; else top - op depth; else the model bottom, floored at the stock bottom | `CF:365-377` | `[D]` from the floor, the model and the stock. |

## 10. Stock (`StockConfig::default()` and the auto stock)

| Field | Value | Where | Class |
|---|---|---|---|
| `auto_from_model` | true | `SC:108` | `[D]` The stock follows the models. |
| `padding` | 5.0 mm on X, Y and the top; the bottom is flush | `SC:109, 282-296` | `[U]` The offset box is a convention (audit S12 MATCH), but the number is not: Kiri 5 mm, FreeCAD 1 mm, Fusion not found. Only Kiri prints 5. |
| `x`, `y` | 100 × 100 mm | `SC:102-103` | `[U]` Overwritten by the auto stock when a model exists. |
| `z` | 25.0 mm | `SC:104` | `[U]` HIGH IMPACT. For a 2D drawing (`bbox` Z range 0) the auto stock keeps `z` unchanged (`SC:289-293`). So a DXF/SVG project cuts 25 mm stock, and the Profile and Drill "through the stock" floors and the Pocket half-stock floor all read this 25. |
| `origin_x/y/z` | 0 | `SC:105-107` | `[D]` Set by the auto stock. |
| `material` | Solid wood, GenericSoftwood | `crates/rs_cam_core/src/material/mod.rs:1115-1121` | `[U]` Every Suggest value reads it. |

## 11. Not in the scope, seen on the way

- MCP `add_tool` fills unset tool fields: flute count 2, cutting length
  25 mm, shaft 6.35 mm, shank 6.35 mm, stickout 45 mm
  (`crates/rs_cam_viz/src/mcp_server.rs` `add_tool` description). The reply
  lists them as `defaulted` and calls them "generic, not your tool". They
  are `[U]` by the ruling; the GUI tool form is a separate inventory.
- The machine dial `aggressiveness` 0.85 (`feeds/CLAUDE.md`) and the post
  `high_feedrate` 5000 are machine-profile values, not op values.

## 12. Removal candidates

Sorted by the risk that an empty field blocks a common flow. "High" means
that the common flow (add an op, Generate) stops until the operator types a
number. "Better" names a derivation that can replace the value instead of
a removal.

### High risk (every add of a common op stops)

1. Pocket `floor` at the add: min(half the stock, 5 mm) (`AP:1145`). The
   ruled first case. A STEP face pick already gives `EachFace` (`[D]`). A
   drawing has no depth, so every reference tool also needs a typed value
   (audit S11). Better for a drawing: none; remove. Design trap:
   `each_face_floor_due` fires only while `floor_is_default` ("registry OR
   stock default", `session/CLAUDE.md`). An empty floor must count as the
   default, or a STEP face pick stops setting `EachFace` (UX-7b).
2. Adaptive (2D) `floor` at the add: half the stock (`AP:1148`). The same
   shape as item 1. Remove with it.
3. Zigzag `floor` 3.0 and Bore `floor` 3.0 at a direct add (`OC:1324`,
   `OC:531`). Better for the Bore: the Drill floor (the copy path already
   does it) or `EachHole` on a STEP model.
4. Stock `z` 25 mm on a 2D drawing (`SC:104, 289-293`). Each "through the
   stock" floor (Profile, Drill) and item 1 read it. Better: no stock
   thickness until the operator states it; the drawing import dialog can ask.
5. Trace `depth` 1.0, V-Carve `max_depth` 5.0, Inlay `pocket_depth` 3.0,
   Project Curve `depth` 1.0 (`OC:326, 1046, 1145, 3069`). A drawing has no
   depth. Better for V-Carve: the V-bit geometry (the flute length).
6. The raw feeds fallback (section 2, `[D/U]` rows): feed, plunge, stepover
   and depth per pass ship as raw numbers when Suggest refuses `Unbacked`
   and in the CLI job. Removal makes an op on an unbacked tool refuse until
   the operator types the feeds. That is the ruling, but it touches every
   op on an unbacked cell, so it has the largest blast radius.
7. Stock `material` GenericSoftwood (`material/mod.rs:1115`). Every Suggest
   value reads it. An empty material blocks Suggest on every op.

### Medium risk (a common op loses a value that it reads on every run)

8. Face `stock_offset` 5.0 (`OC:279`). Better: the tool radius.
9. Face `depth` floor 1.0 in `max(padding, 1.0)` (`AP:1136`); the padding
   part is `[D]`.
10. Drill and Pin Drill `retract_z` 2.0 absolute (`OC:401, 621`). The
    safety floor hides it in most frames. Better: an offset above the hole
    top (ID Q2).
11. Chamfer `tip_offset` 0.1 (`OC:679`). Not a FreeCAD match (0.5).
12. Unified Finish `raster_stepover` 1.0 and `z_step` 1.0; Steep and
    Shallow `z_step` 1.0 (`OC:2367-2368, 2611`). Suggest does not write
    them. Better: derive from `scallop_height` and the tool.
13. `scallop_height` 0.1 on Scallop and Unified Finish (`OC:2006, 2365`).
    The finish target; only the operator knows it.
14. Inlay `floor_overlap` (inherits the Pocket stepover 2.0), `floor_ridge`
    and Inlay Plug `glue_gap` 0.1, `proud_height`, `outer_margin`
    (`OC:1146-1149, 1257-1263`). Better for `floor_overlap`: the same
    formula on the bound tool diameter.
15. Pencil `reference_tool_diameter` 6.0 (`OC:1863`). Better: the tool of
    the finish op before it.
16. Pin Drill `spoilboard_penetration` 2.0 (`OC:617`).
17. Heights: post `safe_z` 10, clearance = retract + 10, feed = retract - 2
    (`gcode/mod.rs:1471`, `CF:344, 346`). The 5 mm safety floor stays.
18. Dressup numbers on by default: link moves 10 mm / 500 mm/min, lead
    radius 2.0, arc tolerance 0.05, segment merge 0.3, feed optimisation
    max 3000 / ramp 200, Roughing ramp angle 3°, helix pitch 1.0, helix
    radius 0.3 × D, entry clearance 0.5 (section 8). Better: link max and
    lead radius from the tool D, link feed from the op feed, feed max from
    the machine.
19. 3D Rough `stock_to_leave_axial` 0.5, `ramp_angle_deg` 10, `helix_pitch`
    2.0, `helix_radius_factor` 0.3, `entry_clearance_mm` 0.5
    (`OC:1559, 1566-1569`).
20. Bore `pitch` 1.0 (`OC:532`).
21a. Stock `padding` 5.0 (`SC:109`). Only Kiri prints 5. Removal blocks
    the auto stock on every import, so keep it until the operator rules;
    the Face depth reads it.

### Low risk (a tuning value; most operators never change it)

21. Every `tolerance` (0.05 or 0.1) and `sampling` / `point_spacing` (0.5):
    Adaptive, V-Carve, Inlay, 3D Rough, Waterline, Pencil, Scallop, Unified
    Finish, Steep and Shallow, Ramp, Radial, Project Curve. Better: one
    rule from the machine resolution or the simulation cell.
22. Pencil `bitangency_angle` 160, `min_cut_length` 2, `hookup_distance` 5,
    `num_offset_passes` 1, `min_valley_depth` 0.05, `curvature_smoothing`
    4, `rest_cell_mm` 0.5 (`OC:1851-1867`).
23. Unified Finish `overlap_mm` 2.0, `min_rest_depth_mm` 0.02,
    `crease_hookup_mm` 5.0; Steep and Shallow `threshold_angle` 45,
    `overlap_distance` 1.0, `wall_clearance` 0.5.
24. Ramp `slope_from` 30, Radial `angular_step` 5, Horizontal
    `angle_threshold` 5.
25. Profile `tab_width` 6.0, `tab_height` 2.0 (read only when tabs are
    on); Drill `dwell_time` 0.5, `retract_amount` 0.5 (read only by their
    cycles); Pin Drill hard-coded dwell 0.5 s and chip-break 0.5 mm
    (`OC:606, 608`; not visible to the operator).
26. Choices with no reason (enums and bools; not numbers, so the ruling
    text "math" does not clearly apply): Face direction, Pocket pattern,
    Adaptive cleanup and path strategy, Profile compensation, Project Curve
    direction/side/projection, 3D Rough entry style, region order, clearing
    strategy, engagement measure, Waterline/Scallop continuous, Scallop and
    Spiral direction, Ramp direction and order, Steep first, Pencil detector,
    Roughing entry Ramp, the first model of the project. An empty enum needs
    a new "not set" state in the GUI; that cost is larger than for a number.

### Keep, with a note

- The `[S]` measured values (3D Rough `trochoid_cap_mult` 1.6,
  `min_region_cut_length_mm` 15, Scallop `intra_pass_hookup_mm` 3.0, Unified
  Finish 45/75 thresholds, `intra_region_hookup_mm` 6.0,
  `monotone_cell_decomposition`, Pencil `valley_saliency` 0.15) each rest on
  one fixture and, for the lengths, one tool. Three of them are lengths
  fixed in mm for a 3 to 6 mm tool; a tool-scaled form is the better
  derivation.
- Doc defect: the `min_region_cut_length_mm` field doc says "Default 5.0
  mm"; the code value is 15.0 (`OC:1543-1552`).

## 13. Counts

Counts are rows by tag in sections 1.1 to 10 (a row is one field or one
group of fields; section 2 counts one row per op). Section 11 is not
counted.

| Class | Rows |
|---|---|
| `[D]` DERIVED | 29 |
| `[D/U]` DERIVED by Suggest, UNDERIVED raw fallback | 27 (25 feeds rows in section 2, plus the helix radius 0.3 × D in sections 6 and 8; the Drill peck rule is in the section 2 text, not a row) |
| `[S]` SOURCED (all 8 are repo measurements or a review; none cites an outside reference) | 8 |
| `[SF]` SAFETY | 5 |
| `[C]` CONVENTION | 8 |
| `[RULED]` choice named by a ruling | 7 |
| `[OFF]` feature off | 22 |
| `[U]` UNDERIVED | 103 (83 numbers, 20 enum or bool choices) |
| Total | 209 |

The `[U]` count excludes the `1.0` floor inside the Face depth row (its
first tag is `[D]`).
