# G10 Phase 0: the entry-parameter inventory

Date: 2026-09-25. Status: read-only inventory. No number moves.

Inputs:

- The Rust source at master `40f2b744` (the second pass, after the team
  lead's correction). It includes `1e17c3a2` (a helix or ramp takes the full
  material depth) and `9887735d` (`ramp_feed_rate`, `entry_clearance_mm`,
  never helix air). The first pass read `d5b7e34d`; §3.4 keeps the facts of
  before `1e17c3a2` in short. Peer sessions have uncommitted edits in
  `feeds/mod.rs`, `feeds/geometry.rs`, `tool_load/` and the FM1 instrument
  (ruling B4 step 2). None of those edits touches an entry file. Line numbers
  can drift; the script finds each value by a regular expression and prints
  its line.
- The FM1 matrix `planning/feeds_matrix_2026-09-23/matrix_2026-09-23.csv`
  as committed (960 cells, 542 ok, 418 refused).
- The measurements in the commit messages of `1e17c3a2` and `9887735d`,
  and in `8ff81472` (`planning/entry_stock_awareness_2026-09-24/PLAN.md`
  RESULTS 3).

Script (python3, stdlib, re-runnable):
`scripts/g10_inventory.py`. It prints every table below (sections A-E of
its output) and writes `g10_inventory_cells.csv` (one row per matrix cell:
the entry class, the default entry, the plunge rule, and three derived
feeds).

Labels. "Code" means the value is in the source at the file:line given.
"CSV" means the FM1 matrix prints it. **Derived** means the script computes
it; the formula is next to it. No vendor number is in this file.

## 1. The entry parameters

Paths are relative to `crates/rs_cam_core/src/` unless they start with
`viz/` (`crates/rs_cam_viz/src/`) or `cli/` (`crates/rs_cam_cli/src/`).

### 1.1 Ramp angle

| Parameter | Where set | Default | Who writes it | Source backing | Operations |
|---|---|---|---|---|---|
| `Adaptive3dConfig::ramp_angle_deg` | `compute/operation_configs.rs:832` | 10.0 deg | Default. Operator (GUI range 0.5-45, `viz/ui/properties/operations/surface_3d.rs:156`). Suggest does not write the angle. | none | Adaptive3d, when `entry_style == Ramp` |
| `DressupConfig::ramp_angle` (to `EntryStyle::Ramp { max_angle_deg }`, `compute/config.rs:627`) | `compute/config.rs:1036` | 3.0 deg | Default. Operator (GUI range 0.5-15, `viz/ui/properties/linking_dressup.rs:320`). | none. The comments only state its effect: "19.08 mm at the shipped 3 degrees" (`dressup/entry_descent.rs:137`). | Face, Pocket, Profile, Rest, Zigzag (on by default); VCarve, Inlay, Chamfer and the 3D finishes with `ANY_DRESSUP` (off by default) |
| CLI job entry `ramp` | `cli/job.rs:1116` (2.5D), `cli/job.rs:1356` (Adaptive3d) | 3.0 deg on both | Job file | none | Pocket, Profile, Adaptive (`cli/job.rs:1109`); Adaptive3d |
| Pencil internal entry ramp `ENTRY_RAMP_MAX_ANGLE_DEG` | `finish/pencil/emission.rs:70` | 12.0 deg | Code constant | "12 degrees is the operator-set cap for the R1.0 tapered ball in white oak" (an operator setting, not a vendor figure) | Pencil (its own bite-budgeted ramp); the dressup ramp on a rest-driven pass (`dressup/entry_descent.rs:479`) |

### 1.2 Helix pitch

| Parameter | Where set | Default | Who writes it | Source backing | Operations |
|---|---|---|---|---|---|
| `Adaptive3dConfig::helix_pitch` | `compute/operation_configs.rs:840` | 2.0 mm/rev | Default. Operator (GUI 0.1-10, `surface_3d.rs:190`). | none | Adaptive3d, Helix style |
| `DressupConfig::helix_pitch` | `compute/config.rs:1038` | 1.0 mm/rev | Default. Operator (GUI 0.2-10, `linking_dressup.rs:351`). | none | Adaptive (on by default); every `ANY_DRESSUP` op when the operator picks Helix |
| CLI `helix` | `cli/job.rs:1121`, `cli/job.rs:1352` | 1.0 mm on both | Job file | none | Pocket, Profile, Adaptive; Adaptive3d |

### 1.3 Helix radius

| Parameter | Where set | Default | Who writes it | Source backing | Operations |
|---|---|---|---|---|---|
| `Adaptive3dConfig::helix_radius_factor` | `compute/operation_configs.rs:836` | 0.3 x D. `finish_3d.rs:67` multiplies it by `tool_def.diameter()`, which is the envelope diameter. | Default. Operator (GUI 0.05-0.5 x D, `surface_3d.rs:182`). Suggest reads it for the headroom check. | none | Adaptive3d, Helix style |
| `DressupConfig::helix_radius` | `compute/config.rs:1037` | 2.0 mm, absolute. It does not scale with the tool. | Default. Operator (GUI 0.5-20 mm, `linking_dressup.rs:342`). | none | Adaptive (on); `ANY_DRESSUP` ops on operator choice |
| CLI `helix` | `cli/job.rs:1120` (2.5D: 2.0 mm), `cli/job.rs:1351` (Adaptive3d: 0.4 x D, comment "Pre-T9: radius = 0.8 x tool radius") | 2.0 mm; 0.4 x D | Job file | none | as above |
| 2D adaptive helical starter | `adaptive/path.rs:1159` (`helix_r = tool_radius`), gate `adaptive/path.rs:1154` (`2.0 * tool_radius`), `adaptive/search.rs:700` (`tool_radius * 1.1`) | circle radius = R (0.5 x D); a flat circle after a Z plunge, not a helix | Code | The comment cites Autodesk US7831332, Bieterman and Sandström, Ren and Bi for the entry POINT (the medial-axis maximum). Nothing cites the radius. None of these is in `CREDITS.md`. | Adaptive (2D), non-Legacy cleanup |

### 1.4 Plunge rate

| Parameter | Where set | Default | Who writes it | Source backing | Operations |
|---|---|---|---|---|---|
| `Material::plunge_rate_base` | `material/mod.rs:1171-1187` | wood 1000 / h, plywood and sheet 900 / h, times clamp(D / 6, 0.25, 3.0); h = (Janka / 600)^0.4 (`material/mod.rs:834`) | Suggest (calculator Step 8, `feeds/mod.rs:2521-2526`) | none printed. The values are a port of `estimate_plunge_rate` in `reference/shapeoko_feeds_and_speeds/src/calcs.rs` (git-ignored, not in `CREDITS.md`). The Step 8 comment names an "audit rule-of-thumb (150-300 mm/min per mm of diameter for wood)" with no source. Derived check: 1000 / 6 = 167 mm/min per mm at Janka 600, inside that band. | every milling op |
| Ball and tapered-ball tip cap | `feeds/mod.rs:2549-2562` (`150.0 * tip_d`); the same cap as a gate in `tool_load/plunge_stress.rs:20` | 150 mm/min per mm of tip diameter | Suggest | The comment says "Published FSWizard / GWizard ranges are 100–300 mm/min for sub-2 mm tapered/ball tools in wood". Not in `CREDITS.md`; no document is stored. | Ball and tapered-ball tools, every op |
| Plunge <= feed clamp | `feeds/suggest/invariants.rs:237` | plunge = min(plunge, feed) | Suggest (pass 1, and again after pass 9 and pass 10) | none (a consistency rule) | every op |
| Op-config plunge defaults | `compute/operation_configs.rs` (`Face` :172, `Pocket` :377, `Adaptive3d` :792, ...) | 500 mm/min on the clearing and 3D ops; 400 on Trace, Chamfer, VCarve, Inlay, Pencil, ProjectCurve | Default, before Suggest | none | every op |
| Drill plunge (G6 claim) | `feeds/extrapolation/drill.rs:81` (`DRILL_RULES`); envelope ceilings `material/mod.rs:619-628` (580 / 720 per mm) | axial chip = side chip / Z, at the drill RPM cap | Suggest | Amana Spektra v24 "Ramp Down = Feed Rate IPM / # of flutes" (`CREDITS.md:747`) | Drill, AlignmentPinDrill, flat end mill 3.175-6.0 mm, 2 or 3 flutes; the drill op aliases plunge to feed (`compute/operation_configs.rs:2203`, `:2208`) |
| Card text | `feeds/rationale.rs:231` | "Plunge and ramp: material base, no factor." | Suggest (only inside the aggressiveness row) | - | every op with that row |
| Provenance label | `feeds/provenance.rs:334` (`plunge_rate: Some(chip)`) | the chipload's origin (a vendor LUT row id or `Formula`) | Suggest | - | every op |

### 1.5 Ramp and helix feed

| Parameter | Where set | Default | Who writes it | Source backing | Operations |
|---|---|---|---|---|---|
| `FeedsResult::ramp_feed_mm_min` | `feeds/mod.rs:2529` | clamp(0.5 x feed, plunge, 1.5 x plunge) | Calculator. **No production code reads it** (rg: only `feeds/mod.rs`, test fixtures and `provenance.rs:395`, a test). | "reference calcs.rs convention" (`feeds/mod.rs:2528`). The reference itself uses a different rule (0.68 / 0.78 x feed, cap 1.5 / 2.0 x plunge). | none |
| `ramp_feed_rate: Option<f64>` op field (`9887735d`) | `compute/operation_configs.rs` (22 op configs, first at :158; every default `None`); accessor `compute/catalog.rs:1130`; one registry row per op (`compute/catalog/registry.rs:37` and on); CLI key `cli/job.rs:187`, `:1270` | `None` | Operator through MCP `set_toolpath_param` or the CLI job key. **No Suggest or feeds code writes it** (rg in `feeds/`); the feeds session owns the value. No GUI widget (rg in `crates/rs_cam_viz/src` finds only a test fixture). | none yet | every op with `plunge_rate` (all but Drill and AlignmentPinDrill) |
| Ramp feed read by the emitters | `dressup/entry_descent.rs:433` (`emit_ramp`), `:674` (`emit_helix`): `safety.ramp_feed.unwrap_or(feed_rate)`; wired through `EntrySafety::ramp_feed` (`dressup/mod.rs:186`) from `DressupContext::ramp_feed_rate_mm_min` (`compute/execute/dressup_apply.rs:456`, `:589`) and from `Adaptive3dParams::ramp_feed_rate` (`adaptive3d/path.rs:1411`) | the op's `ramp_feed_rate`, or the caller feed when `None` | - | - | helix and ramp moves through material. A straight feed through air and a straight peck keep the caller feed. |
| Dressup caller feed (used while `ramp_feed_rate` is `None`) | `compute/execute/dressup_apply.rs:524-532`, then `dressup/mod.rs:412`, `:424` | min(the plunge move's own feed, 0.5 x the FIRST fed move of the toolpath; 500 if none) | Code heuristic | none | every dressup ramp or helix, and its straight air feed |
| Adaptive3d caller feed (used while `ramp_feed_rate` is `None`) | `adaptive3d/path.rs:1525`, `:1536`; peck `:180`, `:190` | `params.plunge_rate` (the op plunge) | Suggest writes the plunge | as for the plunge | Adaptive3d |
| Lead-in and lead-out feed | `dressup/mod.rs:1370-1376`, `:1507` | lead-in = the plunge move's feed (500 if none); lead-out = the cut feed (1000 if none) | Default; operator (`lead_in_feed_rate`, `lead_out_feed_rate`) | none | ops with `lead_in_out` on |

### 1.6 Entry style, and who picks it

| Place | Where | What it does | Writer |
|---|---|---|---|
| `Adaptive3dConfig::entry_style` default | `compute/operation_configs.rs:796` | Plunge (the peck ladder `emit_peck_plunge`, `adaptive3d/path.rs:160`) | Default |
| `pick_adaptive3d_entry_style` | `feeds/suggest/adaptive_entry.rs:62`, called at `feeds/suggest/invariants.rs:177` | Rewrites Plunge to Helix or Ramp when the op is Adaptive3d, Roughing, depth/pass > 0.5 x D (`adaptive_entry.rs:27`) and the style is still the default. Helix when the bbox classifier says mixed or shallow terrain and `helix_feasible_in_bbox` passes; else Ramp. | **Suggest** |
| Suggest scope | `feeds/suggest.rs:82` | `StrategyAndFeeds` is `#[default]`. No production caller sets `FeedsWithGates` (rg), so the rewrite always runs. | Default |
| Where the rewrite lands | `viz/controller/events/toolpath.rs:111`, `viz/app/mcp/commands.rs:804`, `cli/smoke.rs:526`, `crates/rs_cam_cli/examples/apply_suggest_save.rs:99` | These callers take `s.operation` whole, so a NEW toolpath carries the rewritten style. The later apply funnel (`feeds/suggest/apply.rs:217`) writes feed, plunge, RPM, stepover and depth only; it does not write the style. | Suggest, at creation |
| `DressupConfig::for_role` | `compute/config.rs:1089`, `:1099` | Roughing role gets Ramp; SemiFinish and Finish get None (`:1119`, `:1125`); Finish also gets lead-in/out on (`:1126`) | Default |
| `normalize_for_op`, `PreferHelix` | `compute/config.rs:1162`, `:1193`; registry `compute/catalog/registry.rs:825` | On the 2D Adaptive op, turns Ramp into Helix. It runs at construction (`for_op`, `compute/config.rs:1152`), on every dressup write (`session/mutation/config.rs:33`, `:149`), on an operation change (`session/mutation/toolpath.rs:465`) and on project load (`session/project_file.rs:857`). The operator cannot keep a Ramp on 2D Adaptive. | Default (it overwrites an operator Ramp) |
| `normalize_for_op`, `ForceNone` and `strip_all` | `compute/config.rs:1172-1191`; registry rows | Sets None on Trace, Drill, AlignmentPinDrill, Adaptive3d (dressup layer) and DropCutter, UnifiedFinish, ProjectCurve | Default (a refusal, not a pick) |
| Drill-cycle strip | `compute/execute/dressup_apply.rs:572-582` | Drops the entry dressup on any toolpath with `MoveIntent::Drilling` | Code |
| CLI job `entry` | `cli/job.rs:1104-1124`, `:1337-1358` | Maps `plunge` / `ramp` / `helix` to the values in 1.1-1.3 | Job file |
| `check_plunge_entry_stability` | `feeds/suggest/adaptive_entry.rs:230` | Warns only (Plunge style, depth/pass > 0.5 x D) | Suggest |

### 1.7 Clearance above the material, and other entry numbers

The helix start rule (`rapid_to_entry_top`, `dressup/entry_descent.rs:375`,
shared by `emit_ramp` :420 and `emit_helix` :662):

1. The tool rapids to the material top + 0.5 mm when the top is a stock read
   (`ENTRY_CONTACT_CLEARANCE`), or to the top + 2.0 mm when it is the
   nominal stock top (`ENTRY_CLEARANCE`, "real stock can be thicker than
   nominal").
2. A straight feed (caller feed) goes down to the material top +
   `entry_clearance_mm` (`:392`).
3. The helix or ramp starts there and takes the full depth to the target at
   the ramp feed.
4. With no material above the target, the straight moves go to the target
   and no helix or ramp runs.

| Parameter | Where set | Value | Source backing | Effect today (master `40f2b744`) |
|---|---|---|---|---|
| `entry_clearance_mm` (the helix start clearance setting) | `Adaptive3dConfig` `compute/operation_configs.rs:717` (default :800); `DressupConfig` `compute/config.rs:764` (default :1039, wire :808, field list :944); default fn `compute/config.rs:640` | 0.5 mm, from `ENTRY_CONTACT_CLEARANCE` | operator ruling 2026-09-25 (the value); the code says it is "the 0.5 mm the adaptive3d rapid floor keeps over its stock read" | Operator (GUI 0.0-5.0 mm: `viz/ui/properties/linking_dressup.rs:329`, and the Adaptive3d rows in `viz/ui/properties/operations/surface_3d.rs`; MCP `set_dressup_config`; `set_toolpath_param` on Adaptive3d). Suggest does not write it. Read at `compute/execute/dressup_apply.rs:591` and `adaptive3d/path.rs:1394`. |
| `ENTRY_CONTACT_CLEARANCE` | `dressup/entry_descent.rs:356` | 0.5 mm | none (it equals `RAPID_DESCENT_BUFFER_MM`) | the rapid floor over a MEASURED material top |
| `ENTRY_CLEARANCE` | `dressup/entry_descent.rs:131` | 2.0 mm | none | the rapid floor over the NOMINAL stock top; the ramp leg cap (`:538`, `ENTRY_CLEARANCE / tan / 2`); the peck ladder rapid floor (`adaptive3d/path.rs:169`). It is no longer the helix start height. |
| Measured or nominal top | dressup door: `stock_top_measured: false` (`compute/execute/dressup_apply.rs:590`), then `true` where the own-stock replay has a read (`dressup/mod.rs:380`); Adaptive3d: `rapid_floor_z.is_some()` (`adaptive3d/path.rs:1526`) | - | - | decides 0.5 or 2.0 for the rapid floor |
| `RAPID_DESCENT_BUFFER_MM` (Adaptive3d) | `adaptive3d/path.rs:1456` | 0.5 mm | none | the Adaptive3d rapid stops at the planner stock floor + 0.5 |
| Peck ladder rapid floor and retract | `adaptive3d/path.rs:169` (`stock_top_z + ENTRY_CLEARANCE`), `:162` (`PECK_CLEARANCE_MM` 0.5) | 2.0 mm; 0.5 mm | none | Plunge style on Adaptive3d |
| `PLUNGE_CLEARANCE_MM` | `toolpath.rs:26` | 2.0 mm | none | the descent post-pass and the rest-pass plunge fallback |
| `SAFE_Z_CLEARANCE_MM` | `compute/config.rs:144` | 5.0 mm | none | safe Z floor |
| Lead arc radius `LeadParams::radius` | `compute/config.rs:682` | 2.0 mm, absolute | none | on by default for the Finish role |
| Ramp fold minimum run | `dressup/mod.rs:403` | max(tool radius, 1.0 mm) | none | a shorter run plunges |
| `RAMP_FOLD_MAX_LAPS` | `dressup/entry_descent.rs:201`; roles `compute/catalog.rs:515` | 3 laps, Finish and SemiFinish only | operator ruling R10 (2026-09-18) | a longer fold plunges |
| Fold walk budget | `dressup/entry_descent.rs:65-69` | `ENTRY_CLEARANCE / tan(angle) / 2` | none | 19.08 mm at 3 deg (derived: 2 / tan 3 deg / 2) |
| Helix headroom | `feeds/geometry_class.rs:153` | short bbox side >= 2 x f x D + 2 x D | none | 15.6 mm for a 6 mm tool at f = 0.3 (derived) |
| Adaptive3d rapid floor radius | `adaptive3d/path.rs:1132` | tool radius + helix radius | geometry | Helix style |
| Pencil bite budget | `finish/pencil/emission.rs:47-79` | 0.5 x tip radius, clamp 0.10-0.50 mm; window >= 0.5 mm; <= 64 laps | a measured case (wanaka200 pencil) | Pencil and the rest-driven dressup ramp |
| Plunge-entry DPP threshold | `feeds/suggest/adaptive_entry.rs:27` | 0.5 x D | one Wanaka measurement (362 um transient at DPP 3.69 on 6 mm) | the Suggest rewrite and warning |
| Keep-down link clearance | `compute/operation_configs.rs:766` (`stay_down_clearance_mm`) | 0.5 mm | "matches dexel cell-height resolution" | Adaptive3d links, not entries |

### 1.8 Tool rules

- **No tool kind is refused a plunge, a ramp or a helix.** The source has no
  centre-cutting attribute on `ToolConfig` and no rule that refuses an
  entry style for a V-bit, a ball or a tapered tool (rg for `center_cut`,
  `centre_cut`, `can_plunge`, `non_center` finds none). The only entry rules
  per tool are:
  - the ball and tapered-ball plunge cap, 150 mm/min per mm of tip diameter
    (`feeds/mod.rs:2549`; `tool_load/plunge_stress.rs:28-40` returns `None`
    for flat, bull and V-bit);
  - the tip contact radius that the rest-pass ramp and the pencil ramp read
    (`finish/pencil/emission.rs` `tip_contact_radius`).
- The V-bit plunge scales on the tool's `diameter` (12.7 mm in FM1), which
  is the widest diameter of the cone, not the tip.
- The registry tool rules (`compute/catalog/registry.rs`: VCarve, Inlay,
  Chamfer need a V-bit; Pencil, Scallop, UnifiedFinish, SpiralFinish need a
  ball or tapered ball) refuse the OPERATION, not an entry.
- FM1 flute count: 2 for every tool kind (`FLUTES`,
  `crates/rs_cam_core/tests/feeds_matrix_instrument_fm1.rs:91`). The V-bit
  is 60 deg, the bull corner radius is 0.15 x D, the tapered ball has a 7 deg
  half angle and its `diameter` is the tip.

## 2. The matrix reach

### 2.1 The entry classes

The script derives each operation's default entry from the registry rows
(role, dressup policy) with the `DressupConfig::for_op` logic.

| Class | Operations | Default entry | Entry parameters it carries |
|---|---|---|---|
| E1 | Face, Pocket, Profile, Rest, Zigzag | dressup Ramp, 3.0 deg | ramp angle; dressup entry feed (0.5 x plunge, derived), which also feeds the straight descent above the ramp; plunge |
| E2 | Adaptive | dressup Helix, r 2.0 mm, pitch 1.0 | helix radius, pitch; dressup entry feed; plunge; the 2D starter circle |
| E3 | Adaptive3d | planner Plunge (peck); Suggest rewrites it | style, ramp angle 10, helix 0.3 x D / 2.0; entry feed = plunge |
| E4a | VCarve, Inlay, Chamfer | None (lead-in/out on) | plunge; lead radius; ramp or helix only on operator choice |
| E4b | Waterline, Pencil, Scallop, SteepShallow, RampFinish, SpiralFinish, RadialFinish, HorizontalFinish | None (lead-in/out on, except Waterline) | plunge; lead radius; ramp or helix only on operator choice (fold lap cap 3); Pencil's own 12 deg ramp |
| E5 | Trace | None, forced | plunge; lead radius |
| E6 | DropCutter, UnifiedFinish, ProjectCurve | None, all stripped | plunge only |
| E7 | Drill, AlignmentPinDrill | None, forced | plunge = drill feed (G6) |

### 2.2 Cells per class

| Class | Cells | Ok cells |
|---|---|---|
| E1 | 200 | 168 |
| E2 | 40 | 30 |
| E3 | 40 | 30 |
| E4a | 120 | 18 |
| E4b | 320 | 176 |
| E5 | 40 | 32 |
| E6 | 120 | 72 |
| E7 | 80 | 16 |
| total | 960 | 542 |

Ok cells whose default entry is a ramp or a helix (E1 + E2 + E3): **228**.
Every one of the 542 ok cells ships a plunge.

### 2.3 Per tool kind, size and class (ok cells)

What the CSV prints: `plunge_mm_min` only. The CSV has no column for the
entry style, the ramp angle, the helix radius or pitch, or any ramp feed.
The Suggest rewrite shows only as `StrategyRewrote` in `suggest_warnings`.
The last two columns are **derived** (formulas under the table).

| Tool | D (mm) | Class | Ok / cells | Default entry today | Plunge, CSV (mm/min) | Plunge rule | Dressup entry feed (derived) | Approved ramp feed (derived) |
|---|---|---|---|---|---|---|---|---|
| EndMill | 3.175 | E1 | 20 / 20 | ramp 3 deg | 360-529 | material base | 180-264 | 954-4000, G6 chip |
| EndMill | 3.175 | E2 | 4 / 4 | helix r 2.0, p 1.0 | 360-529 | material base | 180-264 | 3658-4000, G6 chip |
| EndMill | 3.175 | E3 | 4 / 4 | ramp 10 deg (Suggest) | 360-529 | material base | = plunge | 3658-4000, G6 chip |
| EndMill | 6 | E1 | 20 / 20 | ramp 3 deg | 682-1000 | material base | 341-500 | 3165-4000, G6 chip |
| EndMill | 6 | E2 | 4 / 4 | helix r 2.0, p 1.0 | 682-1000 | material base | 341-500 | 4000, G6 chip |
| EndMill | 6 | E3 | 4 / 4 | ramp 10 deg (Suggest) | 682-1000 | material base | = plunge | 4000, G6 chip |
| EndMill | both | E4-E6 | 64 / 120 | none | 360-1000 | material base | - | - |
| EndMill | 3.175 / 6 | E7 | 16 / 16 | none | 1422-2133 | G6 drill claim | - | - |
| BullNose | 3.175 | E1 / E2 / E3 | 28 / 28 | ramp 3 / helix / ramp 10 | 360-529 | material base | 180-264 (E1, E2) | plunge fallback |
| BullNose | 6 | E1 / E2 / E3 | 28 / 28 | as above | 682-1000 | material base | 341-500 | plunge fallback |
| BullNose | both | E4-E6 | 52 / 120 | none | 360-1000 | material base | - | - |
| BallNose | 3.175 | E1 / E2 / E3 | 20 / 28 | as above | 371-476 | ball tip cap (softwood), material base | 186-238 | plunge fallback |
| BallNose | 6 | E1 / E2 / E3 | 20 / 28 | as above | 702-900 | ball tip cap (softwood), material base | 351-450 | plunge fallback |
| BallNose | both | E4-E6 | 58 / 120 | none | 371-900 | as above | - | - |
| TaperedBallNose | 3.175 | E1 / E2 / E3 | 28 / 28 | as above | 360-476 | ball tip cap, material base | 180-238 | plunge fallback |
| TaperedBallNose | 6 | E1 / E2 / E3 | 28 / 28 | as above | 682-900 | ball tip cap, material base | 341-450 | plunge fallback |
| TaperedBallNose | both | E4-E6 | 96 / 120 | none | 360-900 | as above | - | - |
| VBit | 6.35 | E1 | 10 / 20 | ramp 3 deg | 743-1058 | material base | 372-529 | plunge fallback |
| VBit | 12.7 | E1 | 10 / 20 | ramp 3 deg | 1160-1809 | clamped to feed | 580-904 | plunge fallback |
| VBit | both | E2 / E3 | 0 / 16 | (refused, G3) | - | - | - | - |
| VBit | both | E4a-E6 | 28 / 120 | none | 743-2116 | material base, clamped to feed | - | - |

The full per-size table is section C of the script output.

Formulas (derived):

- Dressup entry feed (while `ramp_feed_rate` is `None`, its default on
  every op) = min(plunge move feed, 0.5 x first fed move feed).
  In the generators the first fed move is the first plunge
  (`toolpath.rs:267-268`: rapid, then `feed_to(path[0], plunge_rate)`), so
  it is 0.5 x the plunge. This is a reading of the code; a replay of a
  generated pocket would confirm it.
- Approved ramp feed = min(cutting feed, axial chip x RPM x Z / tan(angle)),
  with the angle = the ramp angle, or atan(pitch / (2 pi r)) for a helix.
  The axial chip is the G6 claim of the same tool, size and material,
  read back from the CSV Drill cell as feed / (RPM x Z): 0.0508 mm
  (3.175 mm, wood and plywood), 0.0635 mm (3.175 MDF, 6.0 wood and plywood),
  0.0762 mm (6.0 MDF). Where no G6 chip exists the approved rule falls back
  to the plunge rate.
- Plunge rule: the script recomputes min(material base, ball tip cap,
  feed) from the source constants. It matches the CSV on all 526 ok
  non-drill cells (no mismatch over 1 mm/min).

Counts on the ok cells: material base 432, ball tip cap 76, clamped to
feed 18, G6 drill claim 16. The approved ramp feed has a printed axial chip
on **56** cells (flat end mill, E1-E3) and falls back to the plunge on
**172**.

`FeedsResult::ramp_feed_mm_min` today (derived, with the shipped feed in
place of the calculator feed, which the CSV does not carry) is 1.00-1.67 x
the CSV plunge; it sits at the 1.5 x base cap on 389 of 526 cells. Nothing
ships it, and nothing copies it into `ramp_feed_rate`.

### 2.4 The per-operation values are not per tool

Every ramp angle, helix radius, helix pitch, clearance and lead radius above
is a per-operation (or per-code-path) constant. None reads the tool kind,
the flute count or the material. Two of them do not even scale with the
tool size: the dressup helix radius (2.0 mm) and the lead radius (2.0 mm).
The same 2.0 mm helix is 0.63 x D on a 3.175 mm tool and 0.16 x D on a
12.7 mm V-bit (derived: 2.0 / D). The plunge reads the material, the
diameter and (for ball and tapered ball) the tip, but not the flute count
or the tool family otherwise.

## 3. Findings

### 3.1 Generic today

1. Ramp angle: 3 deg (dressup) or 10 deg (Adaptive3d), the same for every
   tool, size and wood. No source.
2. Helix: 2.0 mm / 1.0 mm pitch (dressup) or 0.3 x D / 2.0 mm (Adaptive3d);
   the CLI writes a third set (0.4 x D / 1.0 mm, 3 deg). No source.
3. Plunge: one material curve times a linear diameter scale, the same for
   flat, bull and V-bit. The ball and tapered-ball cap cites FSWizard /
   GWizard in a comment only.
4. Entry feed: the field `ramp_feed_rate` exists on 22 op configs
   (`9887735d`), but it is `None` by default and no Suggest code writes it.
   So by default the dressup still runs ramps and helixes at about 0.5 x the
   plunge (derived), and Adaptive3d runs them at the plunge. Only an
   operator (MCP or CLI; there is no GUI widget) sets a ramp feed today.
5. The G6 claim prints a plunge for the flat end mill that is 2.5x the
   milling plunge on the same tool: 6 mm flat in hardwood, 1778 mm/min on
   Drill against 702 mm/min on Pocket (CSV). The milling plunge does not
   read the G6 row.

### 3.2 Where Suggest or a default picks the entry style

1. `pick_adaptive3d_entry_style` (`feeds/suggest/adaptive_entry.rs:62`).
   It fires on all 30 ok Adaptive3d cells in FM1 (every one has depth/pass
   > 0.5 x D). FM1 passes no model bbox, so every rewrite is to Ramp. With a
   bbox (GUI, MCP `add_toolpath`, CLI smoke) it can pick Helix.
2. `DressupConfig::for_role` Roughing -> Ramp (`compute/config.rs:1099`), on
   Face, Pocket, Profile, Rest, Zigzag, and (before policy) Adaptive.
3. `PreferHelix` on Adaptive (`compute/config.rs:1193`). It runs on every
   dressup write and on project load, so it turns any operator Ramp into
   Helix.
4. `Adaptive3dConfig` default Plunge, and the CLI `entry` mapping.

The `ForceNone` and `strip_all` rows and the drill-cycle strip refuse a
style for a stated geometry reason. They do not pick one.

### 3.3 Conflicts with the operator rules

- **"The entry style is the operator's setting."** Items 1-3 of 3.2 set or
  change the style without an operator act. Item 1 overwrites only a default value. Item 3 overwrites an
  operator value, at once and on every load.
- **"Never helix through air."** Met in the code at `40f2b744`: the helix
  or ramp starts at the material top + `entry_clearance_mm` (0.5 mm) and
  takes the full depth; all air above is a rapid or a straight feed
  (`rapid_to_entry_top`, §1.7). The sentry `adaptive3d_entry_stock_aware`
  asserts no helix or ramp move starts more than the clearance (+ 0.3 mm
  replay tolerance) above the material, on a plate and a pocket (commit
  text; not run here). Two limits stay:
  - The 0.5 mm is itself helix through air, by design of the setting.
  - Where the dressup door has no stock read, the "material top" is the
    nominal stock top (`stock_top_measured: false`). Stock above or below
    nominal moves the real start.
  `8ff81472` records one OPEN item: a 2.5D helix circle at a ring start can
  reach past the tool into the part wall; the helix has no containment test
  (the ramp has G-RAMPCONTAIN).
- **"The helix start clearance becomes a setting, default 0.5 mm."** Met:
  `entry_clearance_mm`, default 0.5, on `Adaptive3dConfig` and
  `DressupConfig` (§1.7). It is per operation, not per tool, and nothing
  sources the 0.5 beyond the ruling. The other fixed clearances stay: 2.0
  (`ENTRY_CLEARANCE`, now the rapid floor over a nominal top), 0.5
  (`ENTRY_CONTACT_CLEARANCE`, `RAPID_DESCENT_BUFFER_MM`, `PECK_CLEARANCE_MM`)
  and 2.0 (`PLUNGE_CLEARANCE_MM`).
- **The plunge provenance names the chipload's row** (`feeds/provenance.rs:334`)
  although the plunge is the material base. A vendor LUT id on the plunge
  can read as "this plunge is printed". It is not, except on the 16 G6 drill
  cells.

### 3.4 Before `1e17c3a2` (the first pass, `d5b7e34d`)

- The dressup helix and ramp took only the last `ENTRY_CLEARANCE` (2.0 mm)
  above the cut depth. Above that the tool fed a straight plunge through
  material. The Adaptive3d helix did the same below the planner floor + 0.5.
- No `ramp_feed_rate` field and no `entry_clearance_mm` existed.
- `1e17c3a2` alone (full depth, start at material top + 2.0) measured on
  rivmap100 (helix r 0.3 x 6, pitch 1, plunge 500): dpp 2 983 -> 1773 s,
  dpp 4 619 -> 1255 s, dpp 8 431 -> 1114 s (the commit's measurement).
  That is the "2-2.6x slower" of the work item (derived: 1.80, 2.03, 2.58).

### 3.5 The context number

**The commit's measurement** (`9887735d` message; `8ff81472`,
`planning/entry_stock_awareness_2026-09-24/PLAN.md` RESULTS 3). rough-score,
rivmap100 demo, By Area, helix r 0.3 x D, pitch 1, plunge 500, feed 2400,
dpp 8, at master with `entry_clearance_mm` 0.5:

| Arm | Total (s) | Entry (s) | Total / 431 s (derived) |
|---|---|---|---|
| (a) `ramp_feed_rate` None | 1090 | 744 | 2.53 |
| (b) `ramp_feed_rate` 2400 | 734 | 387 | 1.70 |
| (c) (b) + `helix_pitch` 2 | 580 | 242 | 1.35 |
| (d) (b) + `helix_radius_factor` 0.45 | 869 | 517 | 2.02 |

431 s is the dpp 8 helix total before `1e17c3a2` (the 2 mm helix). The
commit also records dpp 2 and dpp 4 for the four arms (1676 / 1281 / 1126 /
1561 s and 1200 / 895 / 730 / 1088 s). It notes that a 2400 mm/min helix is
"only about twice as fast as 500": the 10-degree steps (0.31 mm) are
accel-bound in the time integrator. A larger helix radius is slower.

**Derived from the CSV** (the commanded feed only; the integrator's
acceleration limit is not in it, so the real gain is smaller, see above).
6 mm flat end mill, hardwood, Adaptive3d (CSV: feed 4000, plunge 702,
RPM 17997, depth/pass 4.685, Z 2; G6 axial chip 0.0635 mm).

| Helix | r (mm) | p (mm) | Helix angle (deg) | Path per mm of depth (mm) | s/mm at plunge | Approved feed (mm/min) | s/mm at approved | s per entry, plunge | s per entry, approved |
|---|---|---|---|---|---|---|---|---|---|
| op default | 1.80 | 2.0 | 10.03 | 5.74 | 0.49 | 4000 | 0.086 | 2.30 | 0.40 |
| rivmap100 arm, CSV plunge | 1.80 | 1.0 | 5.05 | 11.35 | 0.97 | 4000 | 0.17 | 4.55 | 0.80 |
| rivmap100 arm, plunge 500 | 1.80 | 1.0 | 5.05 | 11.35 | 1.36 | 4000 | 0.17 | 6.38 | 0.80 |

Formulas: path per mm = sqrt((2 pi r)^2 + p^2) / p; s/mm = 60 x path / F;
approved = min(feed, chip x RPM x Z / tan(angle)); here chip x RPM x Z /
tan(angle) is 12 900-25 900 mm/min, so the cutting feed binds. The 1.36 s/mm
row reproduces the commit's "about 1.4 s per mm". With the approved feed
the helix descent is 5.7x faster at the CSV plunge (8.0x at plunge 500).

The whole-rough ratio (helix descent time against cutting time) needs the
entry count and the cut length of a generated toolpath. The CSV has
neither, so this inventory does not compute it. The rivmap100 figures above
are the only measured ones.

## 4. What Phase 1 must fetch

Per parameter and tool family. "Wood" means a statement for wood or wood
composites; a metal chart is a second witness only.

| Parameter | Flat end mill (up / down / compression, 1-3 F) | Bull nose | Ball nose | Tapered ball | V-bit |
|---|---|---|---|---|---|
| Maximum ramp angle | vendor ramp angle vs flute count and centre cut (Onsrud, Amana, Whiteside, Harvey, Helical, Garr, Kennametal) | corner-radius ramp guidance (Harvey, Helical) | ball ramp angle (Harvey, Amana ZrN, PreciseBits) | tapered-ball ramp angle (Onsrud 77-100, Amana, PreciseBits) | whether a vendor allows ramp or plunge on a V-bit, and at what angle (Onsrud, Amana, Whiteside) |
| Helix diameter | helix bore or helix diameter as a fraction of D (Harvey, Helical, Kennametal; find a wood statement) | same | same, and whether a ball may helix at all | same | same |
| Helix pitch | pitch or axial step per rev, per D | same | same | same | same |
| Plunge feed | plunge vs side feed (the G6 Amana rule covers 3.175-6 mm, 2/3 F; widen to 1/4 in and up; Onsrud, Whiteside) | PreciseBits (a feed with no RPM, G6) | ball plunge per tip D (the FSWizard / GWizard claim in `feeds/mod.rs:2550`: find and store it, or drop it) | same; Onsrud tapered | V-bit plunge (CMT, Amana V-groove charts) |
| Ramp feed | vendor ramp feed rule (for example "ramp feed = x % of feed", or an axial chip per tooth) | same | same | same | same |
| Clearance above the material | CAM defaults for helix start height (Fusion, Carbide Create, VCarve) as a second witness; no vendor is likely to print one | same | same | same | same |
| Centre cutting | which tool lines are centre cutting and may plunge (the engine has no attribute for it) | same | always centre cutting: confirm | confirm | confirm the tip geometry |

Also fetch, for the operator's rulings: any wood-specific statement on
ramp or helix entry (burning at low feed, chip packing in a helix bore), and
the source of the milling plunge base (the Shapeoko reference calculator's
own citation, if it has one). "Not published anywhere I could reach" is a
valid row.

## 5. Not settled here

- The whole-rough time ratio for the 6 mm flat in hardwood (needs a
  generated toolpath; see 3.5).
- The dressup entry feed of 0.5 x plunge is a reading of the code path, not
  a replay. A generator whose first fed move is not the plunge would get
  0.5 x that move's feed instead.
- The value of `ramp_feed_rate` per tool and material (the feeds session
  owns it; G10 Phases 1-2 feed it). The derived "approved ramp feed" in §2.3
  is the commanded value; the rivmap100 arms show the time gain is
  accel-bound.
- Whether 0.5 mm clearance is right per tool family (no source; §4).
- The FM1 instrument and CSV are in flux (ruling B4 step 2 adds a
  `lut_key` column and may move V-bit numbers). The script reads columns by
  name; re-run it after the CSV is re-written.
