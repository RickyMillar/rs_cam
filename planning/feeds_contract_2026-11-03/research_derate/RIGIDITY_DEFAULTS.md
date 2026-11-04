# The two rigidity default sets (D13 item 6)

Read-only research, 2026-11-05. HEAD `79649cc6c`. No file under `crates/` differs between
`379cdefef` (the derate audit) and `79649cc6c`. `assert_rigidity_defaults.py` (this folder,
`python3 -I`) asserts each tagged claim [Rn]. It prints "ALL RIGIDITY CLAIMS HOLD".
Line numbers are at HEAD. `feeds/mod.rs` has uncommitted edits from another session, so its
line numbers are working-tree numbers.

## Answer

1. The question has a false premise. A machine file with no rigidity block does **not** get
   0.25 / 2.0 / 0.25. The `rigidity` field has no `#[serde(default)]`, and
   `RigidityProfile` has no container default. Such a file is refused with a serde
   "missing field `rigidity`" error [R2]. Claim B03 of `REPORT.md` is wrong: its
   assert reads `machine/mod.rs:254`, and that `#[serde(default)]` belongs to
   `max_cutting_feed_mm_min` on line 255.
2. A project file with no `[job.machine]` table gets `MachineProfile::default()`, which is
   the generic set 0.20 / 1.5 / 0.20 [R3].
3. The 0.25 / 2.0 / 0.25 set (`RigidityProfile::default()`) reaches the product only
   through the two Shapeoko presets in the GUI machine dropdown, and through tests [R1, R4].
4. Both sets entered in one commit, `172d6de60` (2026-03-20). Neither set copied the
   other later. Both are ports of two profiles in the gitignored
   `reference/shapeoko_feeds_and_speeds` crate. Neither profile cites a printed source [R6, R7].
5. Neither set is "right" by a printed source. Each value is Unsourced. The operator
   statement (2026-11-05) that 0.2 is the Fusion 360 default supports only the generic set.
   No repository file prints a Fusion default for depth or load [R10].

## 1. Every definition site

`machine/mod.rs` is `crates/rs_cam_core/src/machine/mod.rs`. Columns: `doc_roughing_factor` /
`adaptive_doc_factor` / `adaptive_woc_factor` (the three in question), then the other four
fields where they matter.

| Site @79649cc6c | Values | Who reaches it |
|---|---|---|
| `machine/mod.rs:166` `impl Default for RigidityProfile` | 0.25 / 2.0 / 0.25; `doc_finishing` 0.10, `woc_roughing` 0.80, `woc_roughing_max_mm` 6.35, `woc_finishing_mm` 0.635 | Only `shapeoko_vfd` (`:725`) and `shapeoko_makita` (`:748`). No serde path calls it [R1, R2]. |
| `machine/mod.rs:675` `generic_wood_router`, block at `:687` | 0.20 / 1.5 / 0.20; `doc_finishing` 0.08, `woc_roughing` 0.70, `woc_roughing_max_mm` 5.0, `woc_finishing_mm` 0.50 | `MachineProfile::default()` (`:579-581`), so every door in section 4 [R1]. |
| `machine/mod.rs:706` `shapeoko_vfd`, `:733` `shapeoko_makita` | read `RigidityProfile::default()` | GUI machine dropdown (`rs_cam_viz/src/ui/properties/machine_panel.rs:124`, `MachineProfile::presets()`); many core tests [R4]. |
| `io::machine_library` (`io/machine_library.rs:90-99`) | none; it parses the file with `toml::from_str` | GUI library modal (`machine_library_modal.rs:154`), machine panel (`machine_panel.rs:49`), MCP `load_machine_from_library` (`mcp_server.rs:2029`). A file without `[rigidity]` is refused [R3, R4]. |
| `session/project_file.rs:781-782` `#[serde(default)] pub machine` | none; a missing `[job.machine]` gives `MachineProfile::default()` | GUI open, MCP `load_project`, CLI `project` [R3]. |
| Tracked TOML files (`git ls-files '*.toml'`) | 114 files: 0.2 / 1.5 / 0.2 (woc 0.7); 14 files: 0.25 / 2.0 / 0.2 (woc 0.4); 4 files: 0.25 / 2.0 / 0.25 (woc 0.8) | Each file states its own values. A default change moves none of them [R8]. |
| `~/.config/rs_cam/machines/shapeoko_pro_xxl_scylla.toml` (outside the repo) | 0.25 / 2.0 / 0.2 (woc 0.4); comments "Operator 2026-11-02" | The operator's machine through the library door [R11]. |

The 4 files with the Shapeoko set are `planning/rivmap_block_job/{testpiece,testpiece_mini,nz_south}.toml`
and `crates/rs_cam_core/tests/fixtures/testpiece_mini_2026-10-20_c3808eb0/testpiece_mini.toml`.
The 14 files with 0.25 / 2.0 / 0.2 are `nz_wanaka_21*.toml` and their copies under
`planning/feeds_blind_review_2026-11-03/`. The rivmap100 benchmark is in the 114 [R8; B05 in `REPORT.md`].

A second literal: `compute/validate.rs:284` writes `0.20 * d` for the stale adaptive
stepover fix. Its comment says `adaptive_woc_factor` is "the runtime authority" [R10].

## 2. History

`git log -S` on each literal gives one first commit for all six values [R6]:

```
172d6de60 2026-03-20 Add feeds & speeds core library: material, machine, calculator
```

The message says: "machine.rs: MachineProfile with spindle config (variable/discrete RPM),
power model (VFD/constant), chip load formula, rigidity profile, and three built-in presets
(generic router, Shapeoko VFD, Shapeoko Makita)". It also says the material values are
"ported from reference shapeoko_feeds_and_speeds implementation". In that commit
`impl Default for MachineProfile` already returned `generic_wood_router()`, and both
Shapeoko presets already read `RigidityProfile::default()`. The two sets have not changed since.

The port source is `reference/shapeoko_feeds_and_speeds/src/machine_profile.rs`.
`.gitignore` excludes `/reference/`, so the committed tree does not hold it [R7]:

| rs_cam set | Port source | Match |
|---|---|---|
| `RigidityProfile::default()` | `shapeoko_vfd_1500w_balanced()` (`:131`), `DocProfile` / `WocProfile` at `:178-195` | All seven values match. |
| `generic_wood_router` rigidity | `MachineProfileBuilder::new`, comment "Start with conservative defaults" (`:426`), profiles at `:442-459` | Six of seven match. `doc_finishing_factor` is 0.05 in the source and 0.08 in rs_cam. This is the only drift. |

So the two sets have two different intents. One is the "balanced" Shapeoko VFD tier. The
other is a "conservative" builder default. The original code called the Default impl the
Shapeoko tier but made `MachineProfile::default()` the conservative one. No printed source
appears beside either profile in the reference crate.

## 3. Printed sources

| Value | Set | Printed source in the repo | Status |
|---|---|---|---|
| `doc_roughing_factor` 0.25 | Shapeoko | none | Unsourced |
| `adaptive_doc_factor` 2.0 | Shapeoko | none | Unsourced |
| `adaptive_woc_factor` 0.25 | Shapeoko | none | Unsourced |
| `doc_roughing_factor` 0.20 | generic | none | Unsourced |
| `adaptive_doc_factor` 1.5 | generic | none | Unsourced |
| `adaptive_woc_factor` 0.20 | generic | none printed; operator statement 2026-11-05: "0.2 is the Fusion 360 default" | Operator statement |

Checks made:

- `CREDITS.md` has no row for any rigidity value. It lists "Autodesk Fusion adaptive reference
  help" as a source (line 261), but no number from it.
- `crates/rs_cam_core/data/vendor_lut/source_manifest.json` holds `autodesk_adaptive_reference`
  with the note "Operation semantics for adaptive and load parameters". No stored text exists
  for it under `data/vendor_lut/sources/`.
- `planning/tech_debt_2026-10-19/F201_UNSOURCED_FEEDS_CONSTANTS.md:227-228` puts the
  `RigidityProfile` values out of the F201 registry as "Per-machine data". No source is stated.
- `research_ae/REPORT.md:14-15` found that "no repo source prints 20 % as an optimal load for
  wood". It traces the 20 % to `adaptive_woc_factor = 0.20` [R10].
- A `git grep` for a Fusion default near "stepdown", "optimal load" or a number found only
  unrelated Fusion defaults (chamfer clearance, `useM06`, WCS).
- The gitignored reference doc `docs/validation/ADAPTIVE_FEEDS_SPEEDS_RESEARCH_SHAPEOKO.md:113-114`
  gives wood adaptive "AE: 10-18%D" and "AP: 1.0-1.5D". It is not in the committed tree and
  cites no page for those ranges. It agrees with the generic ap 1.5 D, not with 2.0 D.
- The operator's own machine file uses `doc_roughing_factor` 0.25 and `adaptive_doc_factor` 2.0
  with `adaptive_woc_factor` 0.2 [R11]. The operator's own depth values match the Shapeoko set.

## 4. Which code path sees which set

| Path | Machine source | Set |
|---|---|---|
| GUI new project | `ProjectSession::new_empty` (`session/mod.rs:1984`) | generic |
| GUI machine dropdown, Shapeoko entry | `MachineProfile::presets()` | Shapeoko |
| GUI or MCP library load | file values; a file without `[rigidity]` is refused | file |
| MCP new or loaded project | same session door as the GUI | generic, or file |
| CLI `job` | `ProjectSession::new_empty()` (`rs_cam_cli/src/job.rs:1241`) | generic |
| CLI `project`, file with `[job.machine]` but no `[rigidity]` | serde | refused |
| CLI `project`, file with no `[job.machine]` | `#[serde(default)]` | generic |
| FM1 (`feeds_matrix_instrument_fm1`) | `MachineProfile::default()` at `:1759` and `:1783` | **generic** [R5] |
| Core tests on `shapeoko_vfd` / `shapeoko_makita` / `presets()` | the presets | Shapeoko |

Production readers of the three fields: `feeds/mod.rs` (adaptive ap floor and ae clamp, about
lines 4096-4124), `feeds/predict.rs:374`, `feeds/suggest/invariants.rs:220` and
`tool_load/depth.rs:216` (both through `depth_cap_mm`), and the MCP machine summary
(`rs_cam_viz/src/app/mcp/project.rs:577`).

## 5. Recommendation

- **One set:** keep the generic values 0.20 / 1.5 / 0.20 (with 0.08 / 0.70 / 5.0 / 0.50).
  Reasons: every door that has no machine file uses it, FM1 is blessed on it, and the
  rivmap100 benchmark states it. The operator statement supports 0.2. This is not a
  printed-source verdict.
- **One definition site:** `impl Default for RigidityProfile` holds the values.
  `generic_wood_router`, `shapeoko_vfd` and `shapeoko_makita` read `RigidityProfile::default()`.
  Alternative: the two Shapeoko presets keep their own block, marked Unsourced with the name of
  the port source. Then they are machine data, not a second default.
- `compute/validate.rs:284` reads `RigidityProfile::default().adaptive_woc_factor`, not a literal.
- **Source tags:** all seven values are Unsourced. `adaptive_woc_factor` 0.20 also carries the
  operator statement (2026-11-05, Fusion 360). Record the port origin
  (`reference/.../machine_profile.rs:426`, gitignored) in the comment, as the F201 rows do for k0.
- **Open point for the operator:** the "no underived defaults" ruling (2026-10-23) says a default
  with no derivation is removed. Under that rule `RigidityProfile` has no default, and a new
  project asks for a machine. That change is larger; the two Shapeoko presets would also need a
  source or a removal. D13 item 1 asks for a printed source for 0.2; until one exists, the value
  stays Unsourced.

## 6. What moves

If the Shapeoko set is dropped (the recommendation):

- FM1: nothing. FM1 walks `MachineProfile::default()` [R5].
- The tracked TOML files and the operator machine file: nothing. They state their values [R8].
- A GUI user who picks a Shapeoko preset: depth caps go 0.25 D to 0.20 D (roughing) and
  2.0 D to 1.5 D (adaptive); the adaptive ae target and ceiling go 0.25 D to 0.20 D.
- Tests on the presets can move where they print or compare a number, not where they read
  `machine.rigidity.*` back. Likely candidates (estimate, not run): `feed_explanation_snapshot_b3`,
  `tapered_width_model_parity_c3`, `power_ceiling_parity_f2` and the other `presets()` loops,
  `tool_load/power.rs` and `tool_load/depth.rs:369` unit tests, `feeds/tests.rs` cases with
  literal targets. The lines `feeds/tests.rs:351, 392, 888` and
  `the_depth_that_cut_is_a_measured_load_g_s3depth.rs:399-400` read the profile, so they follow.
  `g_s3depth` asserts `rough_factor != adaptive_factor`; 0.20 and 1.5 still differ.

If the generic set is dropped instead (all doors take 0.25 / 2.0 / 0.25):

- FM1 `matrix_2026-09-23.csv`: 280 rows print a cap factor (Adaptive 1.5000 x 80,
  Contour 0.2000 x 40, Pocket 0.2000 x 160); 202 of them are `ok` rows. All 280 `cap_factor`
  and `cap_mm` cells move. No `ok` row has its depth per pass at the cap, so the depth
  clamp does not bind today [R9].
- 32 of 60 `ok` adaptive cells have stepover exactly 0.20 D. They read `adaptive_woc_factor`
  and go to 0.25 D, with their feed, force and power columns. The adaptive ap floor
  (`adaptive_doc_factor`) can also move adaptive depths [R9]. The `_sim.csv` subset moves too.
- GUI new project, CLI `job`, MCP new project and every file with no `[job.machine]` move.
- `compute/validate.rs:284` (literal 0.20) disagrees with the new default.
