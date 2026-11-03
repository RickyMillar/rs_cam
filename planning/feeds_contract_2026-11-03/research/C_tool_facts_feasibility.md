# C. Tool facts and physical feasibility (research, 2026-11-03)

Read-only research. Paths are under `crates/rs_cam_core/src/` unless they say otherwise.
Line numbers are at HEAD `40cf411a7`. The files that another session edits (`mesh.rs`,
`surface/dropcutter.rs`, `ops/project_curve.rs`, `finish/scallop/ring_generation.rs`,
`compute/simulate.rs`) were read with `git show HEAD:<path>`.

Inputs: `planning/feeds_blind_review_2026-11-03/rootcause/INDEX.md` and reports 01, 04, 05, 06.

---

## 1. `ToolConfig`: the fields, the defaults and what a default means

### 1.1 The struct (`compute/tool_config.rs:443-481`)

`ToolConfig` has no struct-level serde default. Only four fields carry a serde default. The
project file does not deserialize `ToolConfig` directly: it reads `ProjectToolSection`
(`session/project_file.rs:836-881`), and that section gives a default to every field.

| Field | Type | `ToolConfig` serde default | Project-file default (`project_file.rs`) | `new_default` (`tool_config.rs:488-529`) | Downstream meaning of the default |
|---|---|---|---|---|---|
| `id` | `ToolId` | none | position index (`:1189`) | arg | identity |
| `name` | `String` | none | `"Tool"` (`:885`) | type label | free text; the operator writes "measure" here (see 1.3) |
| `tool_number` | `u32` | none | `id + 1` (`:1190`) | `id + 1` | M6 number |
| `tool_type` | `ToolType` (5 variants, `:32-38`) | none | `"end_mill"`; an unknown token warns and becomes `EndMill` (`:1116-1132`) | arg | an unknown type becomes a flat end mill |
| `diameter` | `f64` | none | 6.35 (`:891`) | 6.35 / 12.7 / 3.175 | on a tapered ball this is the TIP diameter (`:600-612`) |
| `cutting_length` | `f64` | none | 25.0 (`:894`) | 25.0 | flute length: Suggest flute guard, DPP clamp, sim axial clamp, V cone height |
| `helix_deg` | `f64` | `30.0` (`:452`) | 30.0 | 30.0 | cutter geometry; feeds |
| `corner_radius_mm` | `f64` | `0.0` (`:454`) | 0.0 | 0.0 | end-mill corner radius |
| `corner_radius` | `f64` | none | 2.0 (`:897`) | 2.0, then `normalize_geometry` | bull-nose corner radius |
| `included_angle` | `f64` | none | 90.0 (`:901`) | 90.0, normalized | V-bit angle |
| `taper_half_angle` | `f64` | none | 15.0 (`:904`) | 15.0, normalized | tapered-ball half angle |
| `shaft_diameter` | `f64` | none | 6.35 (`:907`) | 6.35 | tapered-ball cone base (the envelope) |
| `shank_diameter` | `f64` | none | 6.35 (`:910`) | 6.35 | collision band, deflection above the flutes, `ShankTooLarge` warning |
| `stickout` | `f64` | none | 45.0 (`:913`) | 45.0 | holder face height, deflection beam length, L/D load share |
| `flute_count` | `u32` | none | 2 (`:916`) | 2 | chip per tooth = feed / (rpm x flutes) |
| `tool_material` | `ToolMaterial` (Carbide, Hss) | none | `"carbide"`; an unknown token becomes Carbide with NO warning (`:1216-1222`) | Carbide | Young's modulus for deflection (`:121-146`) |
| `cut_direction` | `BitCutDirection` | none | `"up_cut"`; an unknown token becomes UpCut with NO warning (`:1224-1231`) | UpCut | vendor series choice (`tool/mod.rs:653-657`) |
| `vendor`, `product_id` | `String` | none | `""` | `""` | display only |
| `size_units` | `Option<SizeUnits>` | `None` (`:476`) | `None` | `None` | display only |
| `non_centre_cutting` | `Option<NonCentreCuttingGeometry>` | `None` (`:479`) | `None` | `None` | **`None` = centre cutting** (doc at `:478`) |

`NonCentreCuttingGeometry` (`:318-323`) holds `centre_core_diameter_mm` and
`blade_protrusion_mm`. `NonCentreCuttingGeometry::limits` (`:340-351`) derives
`max_plunge_depth_mm` = protrusion and `max_descent_angle_deg` = atan(protrusion / core).

### 1.2 The domain check (`tool_config.rs:543-565`, `tool/domain.rs:150-188`)

`ToolConfig::validate` checks each number against its own math domain. It has no cross-field
rule:

- `stickout` domain is `[0, inf)` (`tool/domain.rs:161-162`). A stickout of 0 passes, and a
  stickout less than `cutting_length` passes.
- `ToolDefinition::tip_deflection_mm` returns 0 when `stickout <= 0` (`tool/mod.rs:755-758`). Thus a
  zero stickout makes the deflection gate read zero.
- `flute_count` domain is `[1, inf)` (`tool/domain.rs:154`).
- On a tapered ball, `taper_half_angle`, `shaft_diameter` and `cutting_length` are three free
  numbers. Nothing checks that the cone reaches the shaft at the flute length.

`validate` runs at every door (`tool_config.rs:531-542` lists them): the session commands, the
project loader (`project_file.rs:1514-1516`), the tool-library loader
(`io/tool_library.rs:109-133`), the MCP `add_tool` builder and `compute/cutter.rs:13`.

### 1.3 Measured or estimated: no provenance exists

No field carries provenance or confidence. `rg "provenance|confidence|estimated"` in
`compute/tool_config.rs`, `io/tool_library.rs` and `tool/` finds only the doc comment
"Measured centre core and blade protrusion" (`tool_config.rs:478`).

The operator states the estimate in the tool NAME, as free text that no code reads
(`planning/rivmap_block_job/nz_wanaka_21_operator.toml`):

| Line | Tool name | Estimated facts |
|---|---|---|
| 134 | `3.175-20deg-0.3 V-bit (0.3 mm flat tip; rs_cam models a sharp tip), 25 mm stick-out (measure)` | stickout; the tip has no field |
| 155 | `1/4 in 60 deg V-bit (the day strip slit), 30 mm stick-out (set and measure)` | stickout |
| 197 | `R1 x 6 x 20 mm 2F tapered ball (taper from the size: measure)` | taper half angle (derived from 6, 2, 20) |
| 218 | `DMM DSP1f-D2.0-L08, 2 mm 1F, 8 mm flute (shank 3.175 assumed: measure)` | shank; centre cutting unknown |
| 239 | `18 mm 2F, 20 mm flute, 6 mm shank (measure stick-out)` | stickout |
| 264 | `6 mm ball (R3), 6 mm shank (measure flute and stick-out)` | flute length, stickout |
| 113 | `6 mm flat end mill ..., 30 mm stick-out` | centre cutting unknown (no table) |

The MCP schema admits the same thing in prose. `AddToolParam` (`crates/rs_cam_mcp/src/server.rs:1664-1712`):
`cutting_length` "Default 25.0 — CHECK IT against your real tool"; `flute_count` "Default 2";
`stickout` "Default 45.0"; `non_centre_cutting` "Omit for a centre-cutting tool". The same
struct already refuses an omitted V angle, taper angle or corner radius: "There is no honest
default: the angle IS the tool, so an omitted angle is refused rather than guessed"
(`server.rs:1672-1685`). That is the S11 rule applied to three fields only.

A V-bit example shows how an estimate hides: the V20 record has `cutting_length = 8.152467616`
(`nz_wanaka_21_operator.toml:137`). That is (1.5875 - 0.15) / tan 10 deg, the cone height of a
0.3 mm flat tip. `build_cutter` builds a sharp cone, whose height is 1.5875 / tan 10 deg = 9.003 mm.
The operator put the tip into the flute length; the model reads it as a longer flute and a
sharp tip, 0.85 mm lower (report 06 a2).

### 1.4 How tools load

- **Tool library.** `io/tool_library.rs:1-19`: catalog TOML files under `$RS_CAM_TOOL_DIR`,
  `[paths] tool_library`, else `~/.config/rs_cam/tools/` (this machine: `ball_nose.toml`,
  `end_mills.toml`, `facing.toml`, `my_roughing.toml`, `tapered_ball.toml`, `v_bits.toml`).
  `ToolCatalog { tools: Vec<ToolConfig> }` (`:77-81`) deserializes `ToolConfig` directly, so a
  catalog row MUST state every field except the four serde defaults. `load_from` (`:109-121`)
  refuses the whole catalog when one tool fails `validate`. Import copies the tool into the
  project (a snapshot, `:5-9`). The dedupe key (`:325-384`) puts every cutter dimension,
  `shank_diameter`, `flute_count`, material and cut direction IN, and `stickout` OUT ("how the
  tool is mounted, not what it is").
- **Project file.** `ProjectToolSection` (`session/project_file.rs:836-881`, `deny_unknown_fields`)
  defaults every field (`:885-923`). `tool_from_project_section` (`:1183-1214`) copies the
  fields. Then `validate_tool` (`session/mod.rs:481-483`) runs per tool (`project_file.rs:1514-1516`).
  A project file that omits `flute_count`, `stickout` or `cutting_length` loads with 2, 45 and 25
  and no warning.
- **Save.** `session/save.rs:176-177` writes every field. `non_centre_cutting: None` is skipped
  (`skip_serializing_if`), so "centre cutting" and "not stated" are the same bytes on disk.

### 1.5 The setters

- `ToolParamKey` (`tool_config.rs:179-197`): one variant per settable field, 15 variants,
  `NonCentreCutting` included. `ToolParamKey::write` (`:250-315`) checks the JSON type only.
- `ProjectSession::set_tool_param` (`session/compute/params.rs:576-611`) writes a draft, then
  `write_checked_tool` (`:616-627`) runs `validate_tool`, then drops the tool results.
- MCP: `set_tool_param` (`crates/rs_cam_viz/src/mcp_server.rs:1247`, `SetToolParamInput`
  `crates/rs_cam_mcp/src/server.rs:1471-1482`) and `add_tool` (`AddToolParam`, above).
- CLI: `--tool-set` (`crates/rs_cam_cli/src/run.rs:148`).
- GUI: `crates/rs_cam_viz/src/ui/properties/tool.rs`. The checkbox "Does not cut at centre"
  (`:377-386`) sets core 0 and protrusion 0; the operator then types them (`:387-400`).
  An unticked box is `None`, which the engine reads as centre cutting.

---

## 2. The cutter model

### 2.1 `build_cutter` (`compute/cutter.rs:12-58`)

`build_cutter` runs `validate`, builds one `MillingCutter` per type, then wraps it in
`ToolDefinition::new(cutter, shank_diameter, stickout, flute_count, tool_material)` with
`with_cut_direction` and `with_non_centre_cutting(tool.non_centre_cutting_limits())`.

| Type | Constructor | Fields read | Fields NOT read |
|---|---|---|---|
| EndMill | `FlatEndmill::new(d, cutting_length)` + helix + `corner_radius_mm` (`:15-20`) | d, flute, helix, corner | — |
| BallNose | `BallEndmill::new(d, cutting_length)` + helix (`:21-25`) | d, flute, helix | — |
| BullNose | `BullNoseEndmill::new(d, corner_radius, cutting_length)?` (`:26-31`) | | |
| VBit | `VBitEndmill::new(d, included_angle, cutting_length)?` (`:32-37`) | d, angle | **tip diameter: `VBitEndmill::new` sets 0.0 (`tool/vbit.rs:62`) and `build_cutter` never sets it** |
| TaperedBallNose | `TaperedBallEndmill::new(d, taper_half_angle, shaft_diameter, cutting_length)?` (`:38-47`) | tip, taper, shaft | cross-check of the three |

`ToolDefinition` (`tool/mod.rs:644-663`) holds the cutter and the assembly facts:
`shank_diameter`, `stickout`, `flute_count`, `tool_material`, `cut_direction`,
`non_centre_cutting`.

### 2.2 Geometry hints

- `ToolGeometryHint` (`feeds/mod.rs:78-92`): `Flat`, `Ball`, `Bull { corner_radius }`,
  `VBit { included_angle, tip_diameter }`, `TaperedBall { tip_radius, taper_angle_deg }`.
- `ToolConfig::geometry_hint` (`tool_config.rs:580-598`) builds the hint from the record without a
  cutter; it writes `tip_diameter: 0.0` for every V-bit (`:589-592`). A test pins it equal to
  `build_cutter(..).to_geometry_hint()`.
- `ToolGeometryHint::engaged_diameter_at_doc` (`feeds/mod.rs:159-196`): the tapered ball gives the
  ball-cone width clamped at the shank (`:180-184`); the V-bit gives the cone width
  `2 x tan(half) x ap` clamped at d, and ignores `tip_diameter` (`:186-191`); flat, ball, bull give d.
- `MillingCutter::flat_tip_diameter` (`tool/mod.rs:524-526`) default 0.0; `VBitEndmill` returns its
  field (`tool/vbit.rs:123-125`). The readers exist (curve engrave, tool shape key, report 06),
  but no door fills the field.

### 2.3 Geometry the model loses

| Loss | Where | Effect |
|---|---|---|
| V-bit flat tip | no `ToolConfig` field (`tool_config.rs:443-481`); `vbit.rs:62`; `cutter.rs:32-37` | the cone tip sits lower than the real tip (V20: 0.85 mm); V plunge tip cap is `None` (`tool_load/plunge_stress.rs:42-47`) |
| Tapered-ball consistency | `tapered_ball.rs:76-97` checks each number alone | taper, shaft and flute length can disagree; the operator derives the taper by hand |
| Tapered-ball "diameter" | `tapered_ball.rs:130-132`: `diameter()` = shaft | the cutter radius is the envelope, so `to_assembly` gives `cutter_radius` = shaft / 2 |
| Neck / reduced shank | `ToolDefinition` has one `shank_diameter` | no relieved neck, no shank step |
| Holder per tool | H1: the holder is the machine head (`machine/head.rs`); the tool gives only `stickout` and `shank_diameter` (`tool/mod.rs:703-725`) | no collet nut per tool; the project without a `head` table reads one 25 mm band at the holder face (`machine/head.rs:145, 267-277`) |
| Centre cutting unknown | `Option` with `None` = centre cutting (`tool_config.rs:478-480`) | an unknown tool plunges as a safe plunger (report 05) |
| Drill point | no drill type; a twist drill is a 118 deg `VBit` (`session/pins.rs:561-565`) | an end mill used as a drill is the only backed drill case (report 04 d) |

---

## 3. G-CENTRECUT (commit `c0f5d2567`): the model to copy

The commit enforces one physical rule end to end. It has four parts, and each part has ONE
site.

### 3.1 The fact and the one check (on the tool)

- The fact: `ToolConfig::non_centre_cutting: Option<NonCentreCuttingGeometry>` (`tool_config.rs:479-480`).
- The derivation: `NonCentreCuttingGeometry::limits` (`:340-351`).
- The ONE check, as methods on `NonCentreCuttingLimits` (`:353-410`): `max_descent_slope`,
  `clamp_descent_angle_deg`, `allows_plunge`, `is_steeper_than_limit`, `plunge_refusal`.
  Tolerance `NON_CENTRE_CUTTING_TOL_MM = 0.02` (`:440`).
- The typed refusal: `NonCentreCuttingRefusal { at, depth_mm, max_plunge_depth_mm }` (`:417-436`)
  with one `Display` text.
- Ownership: `build_cutter` puts the limits on `ToolDefinition` (`cutter.rs:57`,
  `tool/mod.rs:685-692`). `MillingCutter::non_centre_cutting` (`tool/mod.rs:294-296`) gives them
  to every reader. A bare cutter shape returns `None`. `tool/CLAUDE.md`: "Every reader asks the
  tool; nothing copies the limits."

### 3.2 Generation: shape the motion, then refuse (pre-export)

1. `EntrySafety::centre_limits` (`dressup/mod.rs:229`) carries the limits into the shared entry
   builders. `dressup_apply.rs:701` fills it from `ctx.cutter`. 3D Rough fills it from the cutter
   too (`adaptive3d/path.rs:2066, 2092, 2170`).
2. The builders shape the move to the limit:
   - `emit_ramp` clamps the ramp angle (`dressup/entry_descent.rs:532`);
   - `emit_helix` clamps the pitch per 10 deg chord (`:840`);
   - a non-centre-cutting tool trusts only the conservative stock top (`:447`).
3. The builders return `Option<NonCentreCuttingRefusal>` (`#[must_use]`) when only a plunge deeper
   than the protrusion is possible (`entry_descent.rs:511` and the `plunge_refusal(..)` returns).
4. Entry style `None` becomes `Ramp` for such a tool when the planner made no entries
   (`dressup_apply.rs:665-679`).
5. `apply_entry` keeps the first refusal in `Produced::non_centre_cutting_refusal`
   (`trace/produced.rs`; the field moves with a frame shift).
6. ONE refusal site: `JobPhases::run_stages` (`session/compute.rs:738-743`) returns
   `OperationError::NonCentreCutting` (`compute/execute.rs:81, 331`). The toolpath has no result.

### 3.3 Simulation: measure against the live stock (post-sim)

- `RapidClearanceCheck::new(entry.tool)` (`compute/simulate.rs:1828` at HEAD) reads the limits.
- The dexel walk asks `core_descent_candidate` (`stock/collision.rs:770`) per fed move. Only a
  candidate pays a stamp flush; then `core_strike` (`:790`) reads the wood over the tip inside the
  core before the move's own stamp, and `record_core` keeps one `NonCentreCuttingDescent`
  (`:726`) per move.
- The result carries `SimulationResult::non_centre_cutting_descents` (`simulate.rs:828`).

### 3.4 Export: refuse with no override

- The eighth shared decision `non_centre_cutting_refusal_rows` (`simulate.rs:845`) turns the
  descents of the emitted toolpaths into one row per toolpath.
- `ProjectSession::export_program` refuses `ExportRefusal::NonCentreCuttingDescent`
  (`session/compute/export.rs:106-121`). The `ExportRefusal` doc says "No override"
  (`gcode/mod.rs:147-151`). Every surface (GUI, MCP, CLI) goes through this door.

### 3.5 The shape, in one table

| Layer | Site | Action |
|---|---|---|
| Fact | `ToolConfig` field, `validate` domain | stored, checked in domain |
| Rule | `NonCentreCuttingLimits` methods | the one predicate |
| Owner | `ToolDefinition` / `MillingCutter` | readers ask the tool |
| Plan | shared entry builders | clamp to the limit |
| Generation | `run_stages` | refuse (typed error, no result) |
| Simulation | `RapidClearanceCheck` in the dexel walk | measure every fed descent |
| Export | `export_program` + shared row builder | refuse, no override |
| Sentry | `--test a_non_centre_cutting_tool_never_plunges_into_stock_g_centrecut`, `--lib a_non_centre_cutting_tool_ramps_at_its_limit_g_centrecut` | red with the defect injected |

### 3.6 What G-CENTRECUT does not do

- It acts only when the fact is `Some`. `None` (unknown) acts as "centre cutting"; nothing warns
  (report 05 b).
- The export half needs `self.simulation` (`export.rs:106`). With no simulation, only the
  generation half holds. The load gate refuses a missing simulation as `LoadUnmodeled`, but
  `accept_unmodeled` waives that (`gcode/mod.rs:30-33, 1054-1072`).
- 3D Rough cut descents and link descents are not planned to the limit; the export gate alone holds
  them (commit message "Open"; INDEX section 4).
- `feeds/` reads no centre-cutting fact (report 06 c: only `predict.rs:875`). The Suggest plunge
  feed is the same for both kinds.

---

## 4. Every existing feasibility check

| # | Check | File:line | What it compares | When it runs | On failure | Gap (op 3 and others) |
|---|---|---|---|---|---|---|
| 1 | Tool domain | `tool_config.rs:543-565`, `tool/domain.rs:150-188` | each number in its math range | every tool door | refuse the edit / the load | no cross-field rule (stickout vs flute, taper vs shaft) |
| 2 | Op domain W6b | `compute/domain.rs` (`validate_operation`) | each op number in its param range | AddToolpath, edit, load | refuse | does not read the tool |
| 3 | Tool kind | `OperationType::tool_refusal` in the prologue (`compute/CLAUDE.md`), static mirror `from_static_checks.rs:202-300` | tool type vs op | pre-generation | refuse | kind only, no size |
| 4 | S11 not set | `compute/not_set.rs:1-80` | depth, feeds with no derivation | pre-generation | refuse `OperationError::NotSet` | op values only; no tool fact is ever "not set" |
| 5 | DPP vs flute | `diagnostics/adapters/from_static_checks.rs:474-509` | `depth_per_pass > cutting_length` | diagnose (static) | Critical diagnostic; export does not read diagnostics | a drill has no `depth_per_pass` (`compute/catalog.rs:845-847`); one pass, not the total depth |
| 6 | Plunge vs feed, stepover vs D, depth beyond stock | `from_static_checks.rs:314-470, 554-600, 647-735` | op numbers | diagnose | diagnostic | — |
| 7 | Suggest pass 5 DPP clamp | `feeds/suggest/invariants.rs:557-577` | DPP vs `cutting_length` | Suggest | clamp + `SuggestWarning` | no DPP on a drill |
| 8 | Calculator flute guard | `feeds/mod.rs:2110-2122`; factors `:1539-1549` (`FormulaConstant::unsourced` 0.8 and 2.0 D) | `ap` vs 0.8 x flute | Suggest | clamp + `DocExceedsFlute` | a drill sends no `ap` (MIN_AP 0.05); the 0.8 has no source |
| 9 | Shank vs collet | `feeds/mod.rs:2141-2149` | `shank_diameter > machine.max_shank_mm` | Suggest | warning | warning only |
| 10 | Long tool L/D | `feeds/mod.rs:2236-2255`, `suggest/aggressiveness.rs:160` | stickout / D | Suggest | lowers the dial load target; not the feed | no effect on finish ops (report 06) |
| 11 | Heights order F033 | `compute/config.rs` `height_order`; export `export.rs:83-92` | the four planes | edit, generation, export | refuse | — |
| 12 | G-CENTRECUT | section 3 | descent vs core limits | generation + sim + export | refuse | `None` = centre cutting |
| 13 | Model gouge UX-7b | `session/compute.rs:766-800`; export `export.rs:93-102` | reduced tool vs model mesh | after generation | Blocking finding; export refuses `CutsIntoModel` | no mesh (a DXF) = no check (`compute.rs:787-790`) |
| 14 | Holder/shank vs mesh | `stock/collision.rs:331, 410-460`; `compute/collision_check.rs:95-140` | head and shank bands vs the model mesh | on demand (GUI action, MCP `collision_check`) | count in a report; no export refusal | no mesh on a DXF; a band whose radius is not above the cutter radius is skipped (`collision.rs:441`); a strike needs > 0.01 mm (`:331`), and op 3 meets the stock top at 0 |
| 15 | K1 obstacles + H1 head | `compute/obstacle.rs:600-725`; sim `simulate.rs:2415` | bands vs clamps; Shoe band vs stock box (`:682`); shelf Z floor | generation step, sim, collision check | lift a move / collision count | the Holder band is never tested against the stock box (`:682` takes `Shoe` only); the unmodelled head has no shelf |
| 16 | Rapid through stock | `stock/collision.rs:706`, `simulate.rs:1828, 1925-1939` | rapids vs stock | sim | collision list | rapids only |
| 17 | Load gates (chip, power, deflection, depth, plunge stress) | `tool_load/` | sim samples vs bounds with `BoundSource` | post-sim; export reads via `emit_checked` (`gcode/mod.rs:1098-1109`) | refuse `LoadExceeded` / `LoadUnmodeled`, waivable (`accept_exceeded`, `accept_unmodeled`) | a weak bound (`RigidityRuleOfThumb`) never refuses (`tool_load/verdict.rs:1142-1144`); the depth gate is `NotApplicableForOp` on drills (`tool_load/depth.rs:125-132`); deflection reads `stickout`, so a 0 or a guess passes |
| 18 | Drill gates | `tool_load/drill_gates.rs:151-176`; `ops/drill_op.rs:133-149` | chip welding, peck/D, plunge feed | post-sim | verdict | `DrillOp` has no flute length and no stickout |
| 19 | Pin drill template | `session/pins.rs:616-619` | `cutting_length >= stock + penetration`, `stickout >= cutting_length` | only when a new pin tool is added | sets the values | allows `stickout == depth` (op 3) |
| 20 | G-code machine safety | `export.rs:154-176` (`validate_machine_safety`) | clearance, feed vs `max_feed_mm_min` (travel 15000) | export | warn, advisory | feed cap is the travel rate, not the 6000 cutting cap |
| 21 | Bad feed, inch, compensation | `gcode/mod.rs:1106-1108` | program shape | export | refuse | — |

### 4.1 How op 3 passed every check

Op 3: 6 mm end mill, flute 25, stickout 30, hole 30 deep (stock 25.5 + 4.5), DXF model, no
`head` table (report 04 a). Checks 5, 7 and 8 read a per-pass depth that a drill does not have.
Check 13 needs a mesh. Check 14 needs a mesh, skips a 6 mm shank on a 6 mm cutter, and does not
count a 0 mm contact. Check 15 tests the Holder band against clamps and the shelf only. Checks 17
and 18 do not read the hole depth against the flute or the stickout. Check 19 runs only when the
operator adds a pin tool, and it permits the equality. No single check owns "the deepest point
the tool reaches against the flute and the holder face".

### 4.2 The shape of the checks today

- The checks live in at least seven places: `ToolConfig::validate`, the prologue,
  `from_static_checks`, `feeds/mod.rs` steps 4 and 4c, `feeds/suggest/invariants.rs`,
  `stock/collision.rs` + `compute/obstacle.rs`, `tool_load/`, `export.rs`.
- Three outcomes exist with no common type: refuse (`OperationError`, `ExportRefusal`), report
  (`Diagnostic`), and silent clamp (`SuggestWarning`, `FeedsWarning`).
- Only four things refuse export: height order, `CutsIntoModel`, G-CENTRECUT, and the load gate
  (`export.rs:83-121`, `gcode/mod.rs:1106-1109`). Export never reads a `Diagnostic` (`rg diagnos`
  in `export.rs` finds only the builder of the list).

---

## 5. Machine facts

`MachineProfile` (`machine/mod.rs:233-301`); this machine:
`~/.config/rs_cam/machines/shapeoko_pro_xxl_scylla.toml`, copied into the project
(`nz_wanaka_21_operator.toml:53-107`).

| Fact | Field | Value (Scylla) | Source of the value | Code default |
|---|---|---|---|---|
| Spindle range | `spindle.Variable { min_rpm, max_rpm }` | 6000-24000 | operator; the H100 VFD range is not in the `$$` (the board reads `$30=1000`, `machine/mod.rs:374-390`) | preset |
| Power curve | `PowerModel::VfdConstantTorque { rated_power_kw, rated_rpm }` (`:30-35`) | 1.5 kW at 24000 | nameplate; the curve shape (linear below rated rpm) is a model, `power_at_rpm` (`:871-884`) | — |
| Travel rate | `max_feed_mm_min` | 15000 | `$110` class (machine name says `$$ 2026-10-15`) | — |
| Cutting cap | `max_cutting_feed_mm_min: Option<f64>` (`:254-255`) | absent, so 6000 | `DEFAULT_CUTTING_FEED_CAP_MM_MIN = 6000.0` (`:622-628`), a comment claim ("sits above every vendor wood-routing recommendation"), no cited file | 6000 |
| Per-axis rate / accel | `kinematics` (`machine/kinematics.rs:93-120`) | XY 15000 / Z 2500 mm/min; XY 1200 / Z 550 mm/s^2; scalar 983.3 | `$110-112`, `$120-122` firmware settings (configured limits, not measured motion) | `None` for every preset |
| Junction deviation | `kinematics.junction_deviation_mm` | 0.1 | `$11` | — |
| Rigidity | `RigidityProfile` (`:61-69`) | doc 0.25 / 0.1, woc 0.4, max 6.35, adaptive doc 2.0, woc 0.2 | operator 2026-11-02 "industry standard, a bit softer" (TOML comments); `RigidityDepthCap` doc: "a RULE OF THUMB with no published source" (`:71-80`) | preset |
| Chip formula | `ChipLoadFormula { k0, p, q }` (`:37-56`) | 0.024 / 0.61 / 1.26 | the `Default` impl: "Soft wood baseline from Shapeoko empirical data", no cited file | same |
| Aggressiveness | `aggressiveness` | 0.85 | operator ruling Q1 (`:258-280`) | 0.85 |
| Collet max | `max_shank_mm` | 7.0 | operator | — |
| Head | `head: MachineHead` (`:295-300`) | project has no `head` table: `MachineHead::unmodelled` (one hard 25 mm part, no shelf, no shoe; `machine/head.rs:145, 263-277`) | — | unmodelled |

The 6000 vs 15000 split: `cutting_ceiling()` (`machine/mod.rs:653-670`) caps Suggest, the optimizer
and modulation at 6000. Suggest step 7 lowers the rpm to stay under it (`feeds/mod.rs:2603-2672`,
INDEX 2). The export safety pass checks feeds against `max_feed_mm_min` = 15000
(`export.rs:160`). On a constant-torque VFD a lower rpm is lower power (report 01 b).

Machine provenance has the same shape as tool provenance: a note in the profile name
(`$$ 2026-10-15`) and TOML comments, no typed field. The project copy of the machine is a
snapshot; it does not track the library file.

---

## 6. Existing provenance precedents in the code

The codebase already has four typed "how do we know" patterns. None of them reaches a tool fact.

| Pattern | Site | Meaning |
|---|---|---|
| `ConstantSource::{Cited { path, printed }, Unsourced { claim }}` | `material/mod.rs:677-713` | a formula constant states its source; about 100 uses of `FormulaConstant::` |
| `BoundSource` + `gates_export` | `tool_load/verdict.rs:1019-1144` | the bound's provenance decides if it may refuse export ("the reason a bound may not refuse an export is the bound's provenance") |
| `UnmodeledReason` | `tool_load/verdict.rs:150-200` | typed absence: `MaterialUnvalidated`, `NoVendorData`, ...; refuses export unless waived |
| `NotSet` (S11) | `compute/not_set.rs` | "a value with no derivation is not set": generation refuses and names the field |
| Diagnostics `None` vs `Some(0.0)` | `diagnostics/CLAUDE.md` | "None means not measured" |

---

## 7. Suggestions

These are proposals, not findings.

### 7.1 A typed tool fact

Replace the bare numbers of the facts that a person measures with one wrapper:

```rust
pub enum ToolFact<T> {
    Measured { value: T, on: Date },          // the operator measured this tool
    Catalogue { value: T, source: String },    // vendor sheet, product id
    Estimated { value: T, basis: String },     // "from the size", "assumed"
    Unknown,                                   // no value
}
```

- Apply it to `cutting_length`, `stickout`, `flute_count`, `shank_diameter`, the tapered taper,
  the V tip diameter (a new field), and centre cutting.
- Centre cutting becomes `ToolFact<CentreCutting>` with `CentreCutting::{Yes, No(NonCentreCuttingGeometry)}`.
  `Unknown` is the default for a new tool. This removes "`None` = centre cutting".
- The project TOML writes `stickout = { measured = 30.0 }` or `stickout = { estimated = 25.0, basis = "..." }`.
  A bare number loads as `Estimated { basis: "file without provenance" }`. That keeps old files loadable and never promotes them to measured.
- Remove the `ProjectToolSection` defaults for the facts (`project_file.rs:885-923`). An absent key is
  `Unknown`, not 25 / 45 / 2. The MCP `add_tool` defaults go the same way.
- The code cannot read `.value` directly. It must call one of two methods:
  - `known() -> Option<T>`, which gives `Measured` and `Catalogue` only;
  - `planning_value() -> (T, Confidence)`, which a planner may use when it also carries the confidence.
  The compiler then forces each reader to decide.

### 7.2 One feasibility stage

Add one stage, `compute::feasibility`, beside `model_gouge` in `JobPhases` (after
`run_stages`, `session/compute.rs:738`). It reads the emitted toolpath, the resolved tool facts
and the stock, and gives a typed `FeasibilityReport`:

- deepest material contact below the stock top (from the toolpath and the stock box; the drill
  hole depth from `drill_depth_bases`) against the flute length and against the holder face
  (stickout less a clearance). This closes op 3 for every op, not only drills, and needs no mesh.
- the Holder band against the stock box (the `Shoe` test at `obstacle.rs:682` with the Holder too,
  and a clearance instead of a 0.01 mm penetration).
- every plunge or steep descent with a centre-cutting fact of `Unknown`.
- the V tip and the cone height against `cutting_length`.

Each row has a severity rule taken from the fact provenance, as `BoundSource::gates_export` does:

| Fact | Margin exceeded | Outcome |
|---|---|---|
| Measured / Catalogue | yes | refuse generation (typed `OperationError::Infeasible`) |
| Estimated | yes, or inside the estimate's band | refuse export until the fact is measured, or an explicit waiver names the fact |
| Unknown and the check needs it | — | refuse export: "measure X" (the S11 text style) |

Export reads the same report through one shared decision in `compute/simulate.rs` (a ninth, as
the compute CLAUDE.md asks) and refuses, as G-CENTRECUT does. Suggest reads the same report
before it ships a number, so Suggest and export cannot disagree.

### 7.3 Copy the G-CENTRECUT shape for each new rule

For each physical rule: one fact on the tool, one predicate type with methods, ownership on
`ToolDefinition`, planners clamp to it, one refusal in `run_stages`, one sim measurement, one
export row builder, and a sentry that is red with the defect injected.

### 7.4 Machine facts

Apply the same `Fact` wrapper to the machine values that a person states: power curve,
rpm range, rigidity factors, chip formula, cutting cap. Mark `$$` values as `Catalogue { source:
"$$ <date>" }`, because they are configured limits, not measurements. Make the 6000 default
`Estimated` so that a reader sees it as a guess. Make the absent `head` table `Unknown` so that
the holder-face check of 7.2 names it.

### 7.5 Small items found on the way

- `parse_tool_material` and `parse_cut_direction` (`project_file.rs:1216-1231`) map an unknown
  token with no warning; `parse_tool_type` warns. Make them warn (or refuse).
- `stickout` 0 passes `validate` and zeroes the deflection gate (`tool/mod.rs:755-758`). Add a
  cross-field rule `stickout >= cutting_length` or treat 0 as `Unknown`.
- The GUI checkbox writes core 0, protrusion 0 (`tool.rs:382-385`); with a typed fact the box
  would become a three-way choice (yes / no / unknown).
- The centre-cut export refusal reads `self.simulation` with no staleness check of its own
  (`export.rs:106`). Confirm that a stale simulation cannot hide a strike when
  `accept_unmodeled` is set.

### 7.6 Blast radius to measure first

`rg -c non_centre_cutting` gives 47 files across the crates. `ToolConfig` field reads of
`stickout`, `cutting_length`, `flute_count` are spread over `feeds/`, `tool_load/`, `session/`,
`compute/` (for example `feeds/suggest.rs:1002-1021`, `feeds/predict.rs:416-422`,
`feeds/cutter_constraints.rs:275-290`). Measure with `rg` before a plan, as the root
`CLAUDE.md` says.
