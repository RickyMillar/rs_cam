# Operation architecture: final audit (2026-10-16)

> **CLOSED** by D1 (321615b1, section 5 items 1-4, 11) and D2 (items 5, 7-10, 12, 13, 15; B18). Partial: item 6 (three test-only items under `cfg(test)`; the test callers of the old 2.5D entry points and finish wrappers still need a move onto the session path, see the D2 report), item 14 (`session/mutation.rs` only; `finish/pencil/emission.rs:503` and `finish/surface_link.rs:327` keep no `SAFETY:` line). Open: B1, B2, B4, B15, G-TIERBURIAL, the Waterline fragment kinds.

Baseline: branch `op-arch`, HEAD `d508ffaa`. Read-only: no cargo, no edits.
Notation: `C(n)` is a lead ruling in the PLAN.md C row; `commit Cn` is
the commit with that tag. Paths are under `crates/rs_cam_core/src/` unless the line names another
crate. "NOT MEASURED" means that the claim comes from reading code only.

## 1. Every registry row and its shape

Registry: `compute/catalog/registry.rs` (24 rows; the Rest op is retired). Adapters are in
`compute/execute/`.

| Op | Adapter | Shape now | Per-op glue left |
|---|---|---|---|
| Face | `clearing_2d.rs:513` | `run_layered` | none |
| Pocket | `clearing_2d.rs:141` | `run_layered` (both patterns, `:150`, `:159`) | none |
| Profile | `clearing_2d.rs:111` | `run_layered` | none |
| Zigzag | `clearing_2d.rs:57` | `run_layered` | none |
| Trace | `clearing_2d.rs:78` | `run_layered` | none |
| Chamfer | `curve_engrave.rs:223` | `run_layered` | none |
| Waterline | `finish_raster.rs:297` | `run_layered` | YES, see 1.1 |
| VCarve | `curve_engrave.rs:129` | `run_height_field` (analytic field) | none |
| DropCutter | `finish_raster.rs:154` | `run_height_field` | none |
| Scallop | `finish_raster.rs:35` | `run_height_field` | iso-field link, see 1.2 |
| RampFinish | `finish_3d.rs:348` | `run_height_field` | none |
| SpiralFinish | `finish_3d.rs:417` | `run_height_field` | none |
| RadialFinish | `finish_3d.rs:480` | `run_height_field` | none |
| HorizontalFinish | `finish_3d.rs:557` | `run_height_field` | none |
| UnifiedFinish | `finish_3d.rs:97` | `run_composed` | YES, see 1.1 |
| SteepShallow | `finish_3d.rs:293` | `run_composed` | YES, see 1.1 |
| Adaptive3d | `rough_3d.rs:40` | `run_volumetric` | none (the op owns its kernel params by W6.5 design) |
| ProjectCurve | `curve_engrave.rs:351` | `run_curve` | link totals not recorded, see 1.1 |
| Adaptive (2D) | `clearing_2d.rs:627` | own loop (ruling C9) | accepted by ruling, see 1.3 |
| Drill | `drilling.rs:568` | own (ruling C9) | none |
| AlignmentPinDrill | `drilling.rs:607` | own (ruling C9) | reads no territory, see 1.3 |
| Inlay | `curve_engrave.rs:42` | own (ruling C9) | none (`ClipAfter`) |
| InlayPlug | `curve_engrave.rs:85` | own (ruling C9) | none (`ClipAfter`) |
| Pencil | `finish_3d.rs:24` | own adapter + `finish::rest_detector::detect_rest` (W6.4.1) | accepted by ruling, see 1.3 |

Result: 18 of 24 ops are on a runner. The six own-loop ops agree with
ruling C(9).

### 1.1 Glue that a runner owns in other ops

- **SteepShallow** `finish_3d.rs:309-339`. The adapter builds the
  `ContourFilter` closure (`standing_near` over
  `territory.reference_stock()`) and the link stage
  (`finishing_link_stage`). `run_height_field` owns the link stage for its
  ops through `LinkRule` (`height_field.rs:279-285`).
  `run_composed(op, ctx)` (`composed.rs:40`) takes no `Prepared`, so it
  cannot read the territory. Every composed adapter must read the
  territory, the stock and the link stage itself. This asymmetry is
  structural, not one leftover.
- **UnifiedFinish** `finish_3d.rs:120-256`.
  - It records two findings before the run (`:123`, `:154`).
  - It maps `RestAnalysisConfig` to `RestFieldParams` (`:166-174`). The
    comment says that the map "mirrors `attach_generic_rest_analysis`".
    W6.4.1 merged the detector, not this map, so two copies remain.
  - It records `ZeroRemovalFinding` after the run (`:268-280`). The
    `run_composed` module doc permits this ("adds any finding that reads
    the stitched toolpath").
- **Waterline** `finish_raster.rs`.
  - It records the ladder finding in the adapter (`:334-343`).
  - `emit_level` holds an 8-arm `RestUse` match (`:450-459`) on a value
    that the registry fixes to `ContourFilter`.
  - It keeps a `rest_tally` `Cell` and records it after `run_layered`
    (`:361`).
  - Its `link` hook (`:512-537`) rebuilds what
    `LinkRule::AfterPlan` does in `run_height_field`.
  - The Layered runner owns `LevelClip`, `LevelSkip` and `LevelMask`, but
    not `ContourFilter`. Waterline and SteepShallow each apply that rule
    by hand.
- **ProjectCurve** `curve_engrave.rs:423-520`. The link body builds
  `RelinkParams` by hand and calls `relink_fragments` directly. Its
  report goes to `tracing::info!` only (`:505-516`). The runner records
  nothing for it. Every other relink records `RelinkTotals`
  (G-LINKVISIBLE).

### 1.2 Safety-grade finding (NOT MEASURED)

**Scallop iso field links with no ceiling on remaining stock.**

- `ScallopPass::link_rule` returns `LinkRule::Off` when `iso_field` is
  true (`finish_raster.rs:87-95`).
- `scallop_toolpath_iso_field_with_cancel` (`finish/scallop/research.rs:76`)
  calls `scallop_toolpath_research`, which passes `None` as the stage
  (`research.rs:171-181`).
- The `None` arm (`research.rs:748-785`) uses `link_ceiling: None`.
- The gate is `intra_pass_hookup_mm > 0.0` (`research.rs:722`), and the
  default is 3.0 (`compute/operation_configs.rs:1583-1585`).
- The registry row says `LinkCeilingNeed::WithRemainingStock` and
  `rest_use: RestUse::LinkSafetyOnly`. So on remaining stock the ceiling
  is the only rest behaviour of Scallop, and the iso field has none.
- Ruling R1 says: "Every op that links calls it."
- B16 fixed the same class for the UnifiedFinish region links.

A sim sentry must measure this on a rest fixture before anyone calls it
a defect.

### 1.3 Own-loop ops (ruling C9): what each reads

- **2D Adaptive** reads the prologue (`prepared.polygons()`), the
  territory (`area_outlines`, `unguarded_stock`), the record door and
  `capabilities().rest_use`. It still owns these in the adapter:
  - the level loop
  - the inter-level retract and the annotation offset arithmetic
    (`clearing_2d.rs:721-737`)
  - the span tail
  - a literal `trochoid_cap_mult: 1.2` (`:706`)
- **Drill** reads the territory stock through `column_stock`
  (`drilling.rs:552-565`).
- **AlignmentPinDrill** does not use `_prepared` (`drilling.rs:609`) and
  reads no territory. The lead must decide: a point op gets no obstacle
  pre-clip, and the post-dressup obstacle step is the only net.
- **Inlay and InlayPlug** have `ClipAfter` and read no territory. That
  agrees with their capability.
- **Pencil** owns its record, annotate, spans and `produced` assignment
  (`finish_3d.rs:66-85`). Its ceiling comes from `op_link_ceiling`
  (`:60`), so C(8) is done.

## 2. LEFTOVER_AUDIT section 2 and the C rows

### 2.1 Items that a planned W6 item covered

| Item | State | Evidence |
|---|---|---|
| Per-adapter span tails (Ramp, Spiral, Radial, Horizontal, Scallop, DropCutter) | done | 624f42a8; `run_height_field` owns `SpanRule` |
| `finishing_link_stage` / `relink_in_adapter` move | done | 624f42a8; `shared.rs:192`, `:223` |
| DropCutter grid glue | done | 624f42a8; `surface::dropcutter::finish_raster_grid` (`finish_raster.rs:250`) |
| VCarve tail | done | 624f42a8 (R-W6.3a yes) |
| Three `RestReference` tier choices | done | d7fd161e; one `finish/rest_detector.rs:118`; callers `dressup_apply.rs:99`, `pencil/detectors.rs:234`, `unified_finish.rs:1554` |
| Two `RegionNode` barrier lists | done | d7fd161e; barriers only from `order_constraints` |
| `generate_steep_passes` / `generate_shallow_passes` | done | d7fd161e |
| ProjectCurve hard-coded ceiling op type | done | ca29d74f; `run_curve` passes `link.ceiling` |
| `ProjectCurveConfig::setup_z_flipped` + session patch | done | ca29d74f; kernel field `ops/project_curve.rs:122` is set from the frame (`curve_engrave.rs:389`) |
| Config-to-kernel enum maps | done by W6.5 design | ca29d74f: the op owns its kernel params (`.into()`, `rough_3d.rs:68-83` `entry_style()`) |

### 2.2 Gaps and the C rows

| Item | C row | State | Evidence |
|---|---|---|---|
| Pin-drill config patch in `generation.rs` | C(7) | done | ca29d74f, 56dd706d; `session/compute/generation.rs:316-318` |
| Pencil hand-built `LinkCeiling` | C(8) | done (B1 override stays) | d7fd161e; `finish/pencil/emission.rs:515-533` |
| W2.4 op-type matches (narrate, validate, session/compute) | C(4) | done | ba705014; no production match outside catalog |
| W2.4 feeds sites | (none) | **unruled** | `feeds/vendor_normalize.rs:81`, `:89`, `:95`; `feeds/explain_payload.rs:160` |
| CLI job map + pre-T9 shims | C(5) | done | ba705014 |
| `DeprecatedDialFinding` | C(3) | done | 56dd706d; no mention remains |
| Retired keys refuse to load | C(1) | done | 56dd706d, ba705014; `RETIRED_KEYS` in `session/project_file.rs`; no TOML under `planning/` or `crates/` holds them, and every remaining `type =` sits under `[[tools]]` or a test table (the foreign edit in `planning/rivmap_block_job/testpiece.toml` is outside this) |
| Silent `unwrap_or(Pocket)` | C(2) | done | 56dd706d (`SessionError::MissingOperation`) |
| Waterline `depth_per_pass` alias | C(3) | done | 56dd706d |
| Other setter aliases | (none) | **unruled** | `compute/operation_configs.rs:2222` RampFinish `max_stepdown` alias `depth_per_pass`; `:1310` Pencil `offset_stepover` alias `stepover`. The C1 message says "the setter accepts only published names". |
| Generic rest analysis outside a shape | (none) | open, needs a ruling | `compute/execute.rs:621-635` (a cross-op stage) |
| `clip_and_split_entries` outside a shape | (none) | open, needs a ruling | `session/compute.rs` (a cross-op stage) |
| 8-arm `RestUse` match in each adapter | (none) | open | `clearing_2d.rs:684`, `finish_raster.rs:459`, `drilling.rs:563`, `layered.rs:548`, `territory.rs:217` |
| `ExecutionContext::boundary*` public | commit C3 | done | d508ffaa; `TerritoryInputs` fields are private (`territory.rs:116-136`) |
| `ctx.obstacles` read beside `territory.obstacles` | (new) | open, small | `shared.rs:146` reads `ctx.obstacles`; the territory already holds the same view (`territory.rs:201`) |
| 2D Adaptive, Drill, Pin Drill, Inlay, InlayPlug keep their shapes | C(9) | done, with one question | see 1.3 (Pin Drill reads no territory) |
| Production `allow(dead_code)` without SAFETY | C(10) | done | 56dd706d; each remaining one has a `SAFETY:` line |
| B17 empty `fixtures` / `keep_out_zones` | C(1) | done | 56dd706d |
| B18 `shank_length`, CLI `holder_length` | C(6) | done | 56dd706d |
| B18 third part: snippet resets position | (none) | open | `gcode/program_builder.rs:545` `state.reset_position()` |
| B20 lifted link lands as an entry | commit C2 | done | ba705014 |

## 3. New leftovers

### 3.1 Pub items with test callers only (no `cfg(test)` gate)

The old 2.5D entry points that `run_layered` replaced (a fork measured
these; I checked some by hand):

- `ops/face.rs:242` `face_toolpath`, `:251` `_with_cancel`
- `ops/zigzag.rs:46` `zigzag_toolpath`
- `ops/profile.rs:66` `profile_toolpath`
- `ops/pocket.rs:49` `pocket_toolpath`, `:104` `pocket_toolpath_at_levels_reported_with_cancel`
- `ops/trace_path.rs:128` `trace_toolpath`, `:140` `_with_cancel`
- `ops/depth.rs:260` `roughing_steps`, `:289` `depth_stepped_toolpath`,
  `:304` `_with_cancel`, `:321` `toolpath_at_levels`, `:343`
  `toolpath_at_levels_with_cancel`
- `ops/waterline.rs:239` `waterline_toolpath`, `:298` `waterline_toolpath_at_levels`
- `ops/inlay.rs:379` `inlay_pocket_toolpath`, `:486` `inlay_plug_toolpath`
- `ops/project_curve.rs:222` `project_curve_toolpath`

Finish wrappers:

- `finish/pencil.rs:505` `pencil_toolpath`, `:612` `pencil_toolpath_structured_annotated`
- `finish/ramp_finish.rs:488` `ramp_finish_toolpath_structured_annotated`
- `finish/steep_shallow.rs:554` `steep_shallow_spans`
- `finish/finish_setup.rs:422` `build_finish_surface_with_cell_size_and_cancel`
  and `:448` `build_finish_surface_with_cancel` (its doc says
  "Production ops do not call this")
- `finish/classify_probe.rs:288`, `finish/finish_planner.rs:278`,
  `finish/scallop.rs:724` `is_shipped`, `:970` `uncut_core`

The Spiral, Radial, Horizontal and Scallop wrappers are already
`#[cfg(test)]`, so these do not follow that rule.

No pub item in `compute/execute`, `finish` or `ops` is fully dead. About
60 items have no reader outside their own file, so they are wider than
they need to be (for example, the stage constants at
`dressup_apply.rs:152-212`). This is cosmetic.

### 3.2 `allow(...)`

- Every production `allow(dead_code)` has a `SAFETY:` line.
- `session/mutation.rs:29` `#[allow(unused_imports)]` has no `SAFETY:`
  line.
- Two production `allow(clippy::...)` lines that I read have no
  `SAFETY:` token: `finish/pencil/emission.rs:503` (none) and
  `finish/surface_link.rs:327` (a reason, no `SAFETY:`). Others may
  exist. The count is NOT MEASURED.

### 3.3 Comments that name deleted items or describe old code

- `finish_3d.rs:86-96` (line 88): the UnifiedFinish doc says "Mirrors
  `generate_scallop` closely" and names "the same
  cancellable-core-call / annotate / spans tail". `run_composed` replaced
  that tail.
- `finish/steep_shallow.rs:547-548`: `steep_shallow_spans` "is called by
  the op adapter". The adapter calls `run_composed`.
- `session/compute/generation.rs:24`: "the compute-time-patched
  operation". W6.5 removed the patch.
- `clearing_2d.rs:751-759`: the tail comment names four moved test
  modules. They are not in this file.
- `finish_raster.rs:539` `// ── Public API ──` and the end of
  `dressup_apply.rs` (`// ── Internal helpers ──`): empty section
  headers.
- `rs_cam_viz/src/ui/properties/toolpath_panel.rs:931`:
  `resolve_rest_reference` (deleted in d7fd161e).
- `tests/a_helix_leaves_no_core_and_a_ramp_never_outruns_the_feed_g10.rs:39`:
  `job::tests::the_adaptive3d_entry_is_the_gui_rule_g10` (deleted in C2).
- `tests/drill_picks_resolve_to_targets_g_drillpickstale.rs:176` and
  `tests/drill_pick_emission_frame_g_drillpick.rs:42`: `holes` / `cfg.holes`
  (deleted in C1).
- `tests/setters_have_rows_wp15a.rs:53-54`: `set_alignment_pin_drill_holes`,
  `add_fixture`, `remove_fixture`, `add_keep_out`, `remove_keep_out`
  (all deleted).
- "legacy" relink arms that are still live: `finish/scallop.rs:1096`,
  `finish/scallop/research.rs:188`. These are tied to 1.2.
- The test shim `ops/vcarve.rs:200` (pre-G9) is still there. It is low
  value.

These comments are history ("used to"), so they are acceptable:
`finish/steep_shallow.rs:1423`, `finish/surface_link.rs:445`,
`compute/catalog/tests.rs:88`.

### 3.4 CLAUDE.md files

- Over 40 lines, all of them index files: root `CLAUDE.md` 111,
  `crates/rs_cam_core/CLAUDE.md` 65, `crates/rs_cam_cli/CLAUDE.md` 48.
  The 40-line rule is for per-folder files, so this is not a defect.
  Every per-folder file has 40 lines or fewer.
- Missing names, both outside this programme:
  - `crates/rs_cam_core/src/material/CLAUDE.md:32` names
    `LIBRARY_CANONICAL_HASH`.
  - `crates/rs_cam_core/src/gcode/CLAUDE.md:4` names `build_program`.
    The real names are `build_single`, `build_phased` and
    `build_multi_setup`.
- Every name in the compute, finish, ops, session, toolpath, trace and
  surface files, and in the core, cli and viz crate files, exists.

### 3.5 Planning records are behind the code

`planning/op_architecture_2026-10-14/PLAN.md` status table:

- The K2 row says "open". K2 landed in 6a013a78.
- The C row says "open". C1, C2 and C3 landed.
- The W6 row lists W6.2 to W6.5 as "next".
- The backlog C3 follow-up line still reads as open.

`LEFTOVER_AUDIT_2026-10-15.md` has no closing marks. `planning/CLAUDE.md`
requires an active plan to state its current evidence.

## 4. Backlog rows

| Row | State | In or outside the migration |
|---|---|---|
| B1 | open: `finish/pencil/emission.rs:498-524`, a stated tip-radius override | Outside the migration. This is a link-ceiling physics change (shank and holder), not a structure change. |
| B2 | open | Outside the migration. It is a feature (a second board in one project). |
| B4 | open | Outside the migration. It is planner quality (AgentSearch / ContourSpiral wall band). W6.5 §1.7 kept it out. |
| B15 | parked: `compute/catalog.rs:1316` `feed_optimization_unavailable_reason` | Outside the migration. It waits for the optimizer, which was culled on 2026-10-13. |
| B16 | fixed | d7fd161e |
| B17 | fixed | 56dd706d |
| B18 | partial | `shank_length` and `holder_length` are gone (56dd706d). The snippet position reset (`gcode/program_builder.rs:545`) is open. That part is G-code, outside the migration. |
| B19, B20 | fixed | 4f7578c4, ba705014 |
| K2, K3 | fixed | 6a013a78; 2ea3166b + ad370323 |
| G-TIERBURIAL | open (cited at `finish_raster.rs:286`, `:526`) | Outside the migration |
| Waterline fragment kinds | open (`finish_raster.rs:500-510`) | A follow-up feature |

## 5. Remaining work, ranked

### Blocks "clean implementation"

1. Measure the Scallop iso-field link on remaining stock (1.2). If it
   cuts stock, route the iso-field relink through `finishing_link_stage`
   or refuse the combination. This is a SAFETY item.
2. Give `run_composed` the `Prepared` value and move the territory, stock
   and link-stage reads of SteepShallow into it (1.1).
3. Make `ContourFilter` a runner-owned rule. Then Waterline and
   SteepShallow stop applying it by hand, and Waterline's `link` hook
   uses the runner's after-plan link (1.1).
4. Merge the UnifiedFinish `RestFieldParams` map with
   `attach_generic_rest_analysis` (1.1).
5. Rule on the two remaining setter aliases (RampFinish `depth_per_pass`,
   Pencil `stepover`), then delete them or record them as kept.
6. Delete the old 2.5D entry points and the finish wrappers that only
   tests call, or put them behind `cfg(test)` (3.1).
7. Fix the stale comments and test docs in 3.3.
8. Update the PLAN.md status rows and close LEFTOVER_AUDIT (3.5).

### Follow-up (needs a ruling or is a feature)

9. Record the ProjectCurve chain-link totals (G-LINKVISIBLE).
10. Pin Drill and the territory: rule whether a point op reads it.
11. Add a `RestUse` helper to replace the five exhaustive matches, and
    read `territory.obstacles` in `op_link_ceiling`.
12. Rule on the W2.4 feeds sites.
13. Rule whether the generic rest analysis and `clip_and_split_entries`
    stay as cross-op stages.
14. Add SAFETY lines to the older production `allow(clippy::...)` lines
    and to `session/mutation.rs:29`.
15. Fix the CLAUDE.md names in `material/` and `gcode/`.
16. B1, B2, B4, B15, the B18 snippet reset, G-TIERBURIAL and the
    Waterline fragment kinds.
