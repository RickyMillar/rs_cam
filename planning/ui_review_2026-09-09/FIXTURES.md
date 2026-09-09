# Fixture and scenario manifest

Static verification on 2026-09-09: parsed the listed TOMLs and checked referenced
model paths exist. **Not loaded/generated/simulated during planning.** Existing
files are candidates, not promises of healthy defaults or specific verdicts.

Use raw import for R01; use zero-op projects to isolate later authoring tasks.
Do not mistake a prepared project for evidence that setup was easy.

## Reusable seeds

| ID | Seed | Coverage / verified facts |
|---|---|---|
| F0 | Fresh app session, no preloaded job | Blank-start and help/navigation. Do not assume there is a New command; observe how a user starts over |
| F1 | `fixtures/demo_pocket.svg`; `test_data/ux_2d_pocket.toml` | Simple 2D pocket/profile. Project is format 3 with **zero operations**, asset exists |
| F1m | `test_data/ux_2d_pocket_mdf.toml`, `test_data/ux_2d_pocket_softwood.toml` | Same input with different declared material for feeds/default scope checks; zero operations |
| F2 | `fixtures/demo_star.svg`; `test_data/ux_2d_star.toml` | Corners, trace/v-carve and detail tool choice; zero operations |
| F3 | `fixtures/terrain_small.stl`; `test_data/ux_3d_terrain.toml` | Small rough/finish and mixed geometry. Project has zero operations and includes **both terrain STL and demo_star SVG**; isolate STL for the simplest spine |
| F4 | `fixtures/gui_step/plate_100x60x10.step`; `test_data/ux_step_plate.toml` | STEP face selection and single-setup authoring; zero operations. MDF variant also exists |
| F5 | `test_data/ux_step_block.toml`, `ux_step_stepped.toml`, `ux_step_lbracket.toml` under `test_data/` | STEP blocks, multiple heights and ambiguous/unsupported face targets; all zero-op format-3 seeds, referenced assets exist |
| F6 | `crates/rs_cam_viz/tests/fixtures/sample_2d_project.toml`, `sample_3d_project.toml` | Minimal load/scope cases using local square.svg and flat_plate.stl; **zero operations**, not ready-made simulation results |
| F7 | `crates/rs_cam_viz/tests/fixtures/missing_model_project.toml` | Deliberate missing_model.stl reference; zero operations. Use for warnings/recovery, not a healthy-load baseline |
| F8 | `planning/deep_doc_modulation_2026-09-08/Q2_r20_s15.toml` | Mature Wanaka variant: **14 operations, 1 enabled** at inspection. Four external STL/DXF paths exist on this machine. Not a minimal one-op file |
| F9 | `planning/multitool_2026-08-23/wanaka200_iso_scallop.toml` | Mature multi-setup Wanaka: **10 operations, 3 enabled** in the current working copy. Four external paths exist. This file was already user-modified; never overwrite or silently enable everything |
| F10 | `crates/rs_cam_core/tests/fixtures/rivers_aligned.dxf` | Checked-in DXF asset for curve import exploration. Confirm entity/layer content before choosing a task; not guaranteed to contain drill points |

F8/F9 reference terrain.stl, rivers_aligned.dxf, lakes.dxf and holes.dxf under
Ricky's local Downloads. They are not portable fixtures. Do not copy/publish
external assets without permission; report unavailable dependencies on other
machines and use checked-in seeds first.

### Working-copy rule

Copy a seed to `results/Rxx/scratch/`, preserving path meaning: relative model
references must be rebased or the necessary directory structure preserved.
Record original seed hash, working copy hash and every intentional change.
Never simply move a TOML and assume its relative references still resolve.
Record enabled operation IDs as well as indices; additions/reordering can change
indices. Use one frozen enabled set per comparison.

## Controlled variants to prepare in Wave 0 / relevant review

These are **not created yet**. Prepare only the variants required for the current
package; store the recipe as well as the result. Prefer small analytic geometry.

| ID | Variant and expected question | Preparation / verification |
|---|---|---|
| V1 | Clearly known inch-versus-mm size; off-origin geometry | Use a scratch SVG/DXF or scaled mesh with known dimensions. Record expected bbox in mm. Do not assume importer unit conventions |
| V2 | Layered DXF with points, circles, open curves and a closed contour | Reuse test-builder data or create a tiny documented fixture. Verify entity inventory, units and selected hole count before UI review |
| V3 | Two models and two tools, then reverse their import/library order | Preserve geometry/tool values. Test active-versus-first-item defaults and explicit target selection |
| V4 | Minimal top/bottom job with pins; separate lateral setup | Build from a small plate. Use named asymmetric landmarks so wrong face/flip cannot look correct. Record physical datum and intended faces |
| V5 | Generated and simulated simple job, then one edit at a time | Tool geometry; stock/material; heights; feed; boundary; enabled/order changes. Capture invalidation before recompute |
| V6 | Real measured warning/collision and an independently clean comparison | Seed only in scratch, verify actual current-build output. Label local coordinates, scope, collision type and cell. Never assume an old sentry still fails |
| V7 | No simulation, capture off, shallow/unmeasurable and drill-native result | Use small fixtures and verify the actual reason/population. Distinguish absent metrics from a measured zero |
| V8 | Rough + two dependent remaining-stock ops; one disabled predecessor | Small stock, pinned cell. Record expected dependency order and actual waiting/fixpoint transitions |
| V9 | Reopened/missing/replaced source; unsaved changes | Use copies and separate scratch assets. Deliberately break only scratch references; do not rename/delete user models |
| V10 | Two competing finish or feed candidates | Same input/stock/tool/material/setup/cell/capture settings except declared variable. Match delivered-quality evidence before claiming a speed win |

## Coverage strategy

Do full end-to-end spines for **F1** and a small **F3** case. Use F4/F5, V2 and V4
for format/setup branches, not ten more copies of the same full export exercise.
Use F8/F9 for density, complex dependencies and realistic output only after
small cases work and compute cost is agreed.

For operations, R03 samples stock-based/2D clearing, contour/detail/drill,
3D rough/finish and mixed curve projection. Then do a shallow **all-23-operation
capability census**: availability reason, intended input/tool, primary parameters,
advanced location, guidance and any advertised control without a reachable effect.
Deeply exercise every distinct interaction pattern, not every numeric parameter.
Use `OperationType`/catalog and current UI as the inventory, not old surface counts.

## Regression anchors for preparing evidence

Useful existing tests to read, not a command to run the entire suite:

- `crates/rs_cam_viz/src/controller/{tests,workflow_tests}.rs`: import, selection,
  undo and state transitions; scripted/renderless boundaries apply.
- `crates/rs_cam_viz/tests/{generate_all_fixpoint_parity,apply_contract_a3,wizard_e2e,mcp_escape_hatches,overlays_registry}.rs`.
- `crates/rs_cam_core/tests/model_units_survive_reload_g_unitsreload.rs`.
- `crates/rs_cam_core/tests/{export_datum_setup_frame,setup_datum_round_trip_p2,lateral_setup_end_to_end}.rs`.
- `crates/rs_cam_core/tests/air_cut_one_time_base_g_airdenom.rs` and relevant
  measurability/drill/load sentries discovered for the selected scenario.

Extract scenario recipes without confusing programmatically built test state
with a GUI-loadable project. Record the distinction in the report.
