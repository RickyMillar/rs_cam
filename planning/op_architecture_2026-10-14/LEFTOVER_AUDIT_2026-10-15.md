# Operation architecture: what is not done (leftover audit, 2026-10-15)

> **CLOSED** by W6.2-W6.5, C1-C3, D1 (321615b1) and D2. `FINAL_AUDIT_2026-10-16.md` re-audited every item; `PLAN.md` holds the status.

Baseline: branch `op-arch` at `ad370323`. The working tree has no edits under
`crates/` (only `.mcp.json`, `planning/rivmap_block_job/testpiece.toml`,
`.pi/`). So no W6.3 code is in the tree yet. The W6.3 marks below come from
`w6/W6_3_PLAN.md`, not from code. Paths are under `crates/rs_cam_core/src/`
unless the row names another crate. Read-only audit: no cargo, no edits.

Mark legend: **[W6.3]** = the running item covers it (do not schedule again).
**[W6.4]** / **[W6.5]** = a planned item covers it. **[GAP]** = no item.

## 1. Every operation and its shape

Registry: `compute/catalog/registry.rs`. 25 rows (24 ops + InlayPlug).
"Layered" = the adapter calls `run_layered` (`compute/execute/layered.rs:333`).

| Op | Adapter | Shape today | Item that moves it |
|---|---|---|---|
| Face | `clearing_2d.rs:513` | Layered | done (W3.3) |
| Pocket | `clearing_2d.rs:141` | Layered | done (W3.1) |
| Profile | `clearing_2d.rs:111` | Layered | done (W3.2) |
| Zigzag | `clearing_2d.rs:57` | Layered | done (W3.2) |
| Trace | `clearing_2d.rs:78` | Layered | done (W3.2) |
| Chamfer | `curve_engrave.rs:162` | Layered | done (W3.5) |
| Waterline | `finish_raster.rs:403` | Layered | done (W6.1, c6301ee7) |
| Adaptive (2D) | `clearing_2d.rs:627` | none: own level loop with `level_masks` (`clearing_2d.rs:659-700`); not `run_layered`. AUDIT lists it as Layered | **[GAP]** no item (W6_5_PLAN §4 lists it) |
| Drill | `drilling.rs:535` | none (AUDIT "Point" shape) | **[GAP]** Point shape never scheduled |
| AlignmentPinDrill | `drilling.rs:574` | none (Point) | **[GAP]** same |
| Inlay | `curve_engrave.rs:33` | none: per-polygon loop | **[GAP]** fits no shape (AUDIT) |
| InlayPlug | `curve_engrave.rs:76` | none: whole-set call | **[GAP]** fits no shape |
| VCarve | `curve_engrave.rs:120` | none: per-polygon loop | **[W6.3]** step W6.3.4, only if ruling R-W6.3a says yes; else **[GAP]** |
| HorizontalFinish | `finish_3d.rs:673` | none (ad hoc height field) | **[W6.3]** W6.3.1 |
| RadialFinish | `finish_3d.rs:619` | none | **[W6.3]** W6.3.2 |
| SpiralFinish | `finish_3d.rs:579` | none | **[W6.3]** W6.3.3 |
| RampFinish | `finish_3d.rs:532` | none | **[W6.3]** W6.3.5 |
| DropCutter | `finish_raster.rs:246` | none | **[W6.3]** W6.3.6 |
| Scallop | `finish_raster.rs:169` | none | **[W6.3]** W6.3.7 |
| UnifiedFinish | `finish_3d.rs:278` | none (one 1187-line kernel fn) | **[W6.4]** W6.4.3a/b |
| SteepShallow | `finish_3d.rs:495` | none | **[W6.4]** W6.4.4 (rest: W6.4.6, needs ruling) |
| Pencil | `finish_3d.rs:204` | none; W6.4 gives it a detector stage only, not a shape | **[W6.4]** W6.4.1 partial; no shape by design (W6_4_PLAN §4) |
| Adaptive3d | `finish_3d.rs:40` | none (Volumetric planned) | **[W6.5]** `run_volumetric` |
| ProjectCurve | `curve_engrave.rs:293` | none (Curve planned) | **[W6.5]** `run_curve` |

Schedule for the lead: five ops have no item (2D Adaptive, Drill, Pin Drill,
Inlay, InlayPlug). VCarve depends on R-W6.3a.

## 2. Old code that a shared service replaced but that still exists

### 2.1 Covered by a planned item (do not schedule twice)

- [ ] **[W6.3]** Per-adapter span tails: `finish_3d.rs:564-570` (Ramp),
  `:607-613` (Spiral), `:664-665` (Radial), `:696-697` (Horizontal),
  `finish_raster.rs:234-240` (Scallop), `:393-394` (DropCutter).
- [ ] **[W6.3]** `finishing_link_stage` and `relink_in_adapter` live in
  `finish_raster.rs:90-162`; W6.3 step 4 moves them to `height_field.rs` or
  `shared.rs`. Waterline's link hook (`finish_raster.rs:614-625`) then calls
  the moved pair.
- [ ] **[W6.3]** DropCutter grid glue in the adapter
  (`finish_raster.rs:256-328`) moves to `surface/dropcutter.rs`.
- [ ] **[W6.3]** VCarve tail `curve_engrave.rs:148-149` (under R-W6.3a).
- [ ] **[W6.4]** Three `RestReference` tier choices:
  `compute/execute/dressup_apply.rs:145-160`,
  `finish/pencil/detectors.rs:235-260`, `finish/unified_finish.rs:1609-1616`.
  W6.4.1 merges the call-and-wrap tail only; the three policies stay on
  purpose.
- [ ] **[W6.4]** Two copies of the `RegionNode` barrier list:
  `finish/unified_finish.rs:1343-1354`, `finish/steep_shallow.rs:653-667`
  (W6.4.2).
- [ ] **[W6.4]** Test-only SteepShallow wrappers:
  `finish/steep_shallow.rs:138` `generate_steep_passes`, `:373`
  `generate_shallow_passes` (`cfg_attr(not(test), allow(dead_code))`). Delete
  or fold in W6.4.4.
- [ ] **[W6.5]** Hard-coded op type in the ProjectCurve ceiling:
  `curve_engrave.rs:438` `op_link_ceiling(OperationType::ProjectCurve, ctx)`.
- [ ] **[W6.5]** Config field `ProjectCurveConfig::setup_z_flipped` and the
  session patch `session/compute/generation.rs:316-325`; reads at
  `curve_engrave.rs:342`, `:388`.
- [ ] **[W6.5]** Config-to-kernel enum maps in the Adaptive3d and
  ProjectCurve adapters (`finish_3d.rs:59-98`, `curve_engrave.rs:305-331`).

### 2.2 No item: gaps to schedule

- [ ] **[GAP]** Second compute-time patch on a config clone:
  `session/compute/generation.rs:326-339` writes `AlignmentPinDrill.holes`.
  W6.5 removes only the ProjectCurve patch beside it.
- [ ] **[GAP]** Pencil builds its own `LinkCeiling` by hand:
  `finish/pencil/emission.rs:500-517`, with `obstacles: None` and the TIP
  radius. B1 covers the radius. The `obstacles: None` is new since K1. The
  post-dressup obstacle step (K3, `2ea3166b`) may cover the safety; NOT
  MEASURED. Other hand-built ceilings are test code
  (`finish/surface_link.rs:1696`, `:1789`, `:1839`).
- [ ] **[GAP]** W2.4 rule (read `capabilities()`, not an op-type match).
  Production sites, test modules removed:
  - `trace/narrate.rs:1081`, `:1291` (`plans_bands` = UnifiedFinish),
    `:1236` (`ramps` = RampFinish). These are capability facts.
  - `compute/validate.rs:287` (`Adaptive | Adaptive3d` membership).
  - `session/compute.rs:1441` (`has_clearing_strategy`), `:1806-1819`
    (ContourSpiral uniform WOC).
  - `feeds/vendor_normalize.rs:81`, `:89`, `:95` and
    `feeds/explain_payload.rs:160` (LUT routing; may be real per-op feeds
    math).
  - `gcode/mod.rs:186`, `:193`, `:1275`; `rs_cam_viz/src/app/gpu_upload.rs:327-462`
    (read op-specific config fields; probably legitimate).
  The W2.4 sentry did not catch narrate; decide per site.
- [ ] **[GAP]** CLI job format is a hand-written per-op parameter map for
  five ops only, with typed default literals and "pre-T9 router" shims:
  `rs_cam_cli/src/job.rs:1125-1140` (stock top `model_top + 5.0`),
  `:1330-1466` (defaults 1000, 500, 3.0, 2.0, 0.1, 15.0, 0.5;
  `cleanup_strategy` forced to `ContourParallelHybrid` at `:1390`;
  `entry_3d` falls back to `entry` at `:1410`). The W5 derive is not read
  here. CLI parity item; some of it may be legitimate.
- [ ] **[GAP]** `DeprecatedDialFinding` has no production constructor:
  type at `compute/toolpath_stats.rs:1159`, slot at `:166` and
  `compute/execute.rs:122`, read at `compute/stats.rs:59-258` and
  `trace/narrate.rs:304-325`. Only `tests/findings_transport_join_h21.rs:140`
  sets it. Dead plumbing; delete or give it a writer.
- [ ] **[GAP]** Retired keys ignored with no word on load (no alias, no
  `deny_unknown_fields`):
  - `air_bridge_policy` (DressupConfig field deleted in `1ffbcab3`): 38 TOML
    files still hold it (2 under `crates/*/tests/fixtures`, 9 under
    `planning/fixtures/rivmap100`, the rest in planning folders).
  - `flip_axis`, `workholding_rigidity` (`compute/stock_config.rs:90-96`
    states "ignored on load").
- [ ] **[GAP]** Silent default on load: `session/project_file.rs:1120`
  `tp_section.op_type.unwrap_or(OperationType::Pocket)` when a toolpath has
  no `[operation]` table (warns to the log only).
- [ ] **[GAP]** Setter alias for a retired name:
  `compute/operation_configs.rs:1246` Waterline
  `#[param(alias = "depth_per_pass")]` on `z_step`. Keep or drop by ruling.
- [ ] **[GAP]** Generic rest analysis is a post-generation stage outside any
  shape: `compute/execute.rs:604-631` and `dressup_apply.rs:36-128`. Its
  `findings.borrow_mut().record(..)` calls (`dressup_apply.rs:94`, `:107`) DO
  use the record door, so W2.3 is clean. W6.4.1 shares only its detector
  tail.
- [ ] **[GAP]** Post-generation boundary clip and entry-descent split
  (`clip_and_split_entries`, `session/compute.rs`, AUDIT item 8) are not in a
  shape (W6_5_PLAN §4).
- [ ] **[GAP]** Each kernel adapter repeats an exhaustive 8-arm `RestUse`
  match to pick its own rule: `clearing_2d.rs:665-680`,
  `finish_raster.rs:554-563`. Not a defect; a pattern a helper on
  `RestUse` could replace.
- [ ] **[GAP]** `ExecutionContext::boundary`, `boundary_regions`,
  `boundary_centre` stay `pub` (`compute/execute.rs:362-379`). Only
  `territory.rs` reads them today (rg: no other reader). Make them private
  to the territory so a new op cannot bypass it.

### 2.3 `allow(dead_code)` in production

None of these points at this programme, but the root rule asks for a
`// SAFETY:` line on each production `allow(...)`, and several have none:

- `adaptive3d/path.rs:562`, `:565` (test-only fields on the planner output).
- `adaptive3d/clearing.rs:68`, `:364` (reason comment, no SAFETY line).
- `adaptive/path.rs:639` `adaptive_segments`, `adaptive/search.rs:1004`
  `search_direction` (test-only fns).
- `finish/scallop.rs:197` `ring_stepover` (named statement of the shipped rule).
- `session/compute/params.rs:122` (declared wrapper exemption; reasoned).
- `rs_cam_cli/src/job.rs:171` (`holder_length`, see B18), `:1807`;
  `rs_cam_cli/src/smoke.rs:44`; `rs_cam_viz/src/mcp_server.rs:90`;
  `rs_cam_viz/src/compute/worker/execute/mod.rs:169`.
- `ops/vcarve.rs:200` "shim" is in a test module (pre-G9 tests); low value.

### 2.4 Clean (checked, no action)

- Boundary reads: no `ctx.boundary*` read outside `territory.rs`. Waterline
  reads `prepared.territory().regions()` (`finish_raster.rs:459`).
- Findings: no direct `GenerationFindings` write outside the record door.
- Prologue (W2.2): `require_mesh` exists once (`prologue.rs:145`); the static
  check reads `tool_refusal` (`diagnostics/adapters/from_static_checks.rs:198-203`).
- D6: `feeds/predict.rs` has no production `== Drill` match left (test only).
- Dead pub census: every `pub`/`pub(crate)`/`pub(super)` fn in
  `compute/execute*.rs` has at least two references. A full dead-pub
  measurement needs a warnings build after W6.3. NOT MEASURED.

## 3. Backlog rows (PLAN.md)

| Row | Status | Evidence |
|---|---|---|
| B1 | open | `finish/pencil/emission.rs:502-504` (tip radius, comment names B1) |
| B2 | open | feature; no commit |
| B3 | fixed | W5 `95ee4b3c`: `dv()` reads `schema_range` (`rs_cam_viz/src/ui/properties/linking_dressup.rs:53-61`) |
| B4 | open | deferred to W6.5 Volumetric; W6.5 §1.7 keeps it out of scope |
| B5, B6, B7 | fixed | `7f820c42` |
| B8 | fixed | `bd7cf9f4` |
| B9, B10, B11 | fixed | `b523905a` |
| B12, B14 | fixed | `ec6cf39a` |
| B13 | fixed | `b8fe0a5e` |
| B15 | open (parked) | `compute/catalog.rs:1306-1309`; waits for the optimizer ruling (optimizer culled 2026-10-13) |
| B16 | open | `finish/unified_finish/region_routing.rs:420-462` (`choose_link` reads no ceiling; comment at `:440`) |
| B17 | open | `session/project_file.rs:62-96` loads an empty list; fields `:508-513`; 57 TOML files hold `fixtures = []` or `keep_out_zones = []` |
| B18 | open | `rs_cam_cli/src/job.rs:170-173` `holder_length` with `allow(dead_code)`; `compute/tool_config.rs:182` `shank_length` still a tool param; `gcode/program_builder.rs:548` pre/post snippet path unchanged |
| B19 | fixed | `4f7578c4` |
| K2 | open | dead overlay row `rs_cam_viz/src/ui/overlays/registry.rs:628-640` ("K2 merges the two rows"); duplicate `kind` at `rs_cam_viz/src/app/mcp/commands/obstacles.rs:32` beside `geometry` |
| K3 | fixed | `2ea3166b` (K3), `ad370323` (K3b). The PLAN.md status row still says "open": update it |

## 4. Rulings that block items

- R-W6.3a (VCarve in the height-field shape), R-W6.3b (`SampleFilter` rest
  rule), R-W6.3c (golden extension bless).
- R-W6.4a / R-W6.4b (SteepShallow kernels, SteepShallow rest).
- W6.5: buried-curve threshold (A/B), `Option<&HeightQuery>`, golden format.
- New, from this audit: a shape (or "stays ad hoc") for 2D Adaptive, Drill,
  Pin Drill, Inlay, InlayPlug; refuse or strip `air_bridge_policy` and the
  other ignored keys; delete or wire `DeprecatedDialFinding`; the CLI job
  mapper onto the W5 schema.
