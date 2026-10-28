# Viewport parity: the implementation design (index, 2026-10-28)

Design only. No product file changed, nothing committed. Six design agents
read the code at HEAD b5aa2a45 and wrote one file each; the lead read all
six, verified the claims marked VERIFIED in `RULINGS.md`, and reconciled
the overlaps. `REPORT.md` holds the requirements and the gap matrix; this
folder holds HOW to build steps 1-11 of `REPORT.md` D.1.

## The files

| File | Owns | Lines |
|---|---|---|
| `A_camera.md` | `CameraState` on `AppState`, the pivot, zoom and pan math, `CameraMotion`, the MCP camera fields and echo | 422 |
| `B_input_and_box_select.md` | The viewport input state machine (`DragState`, pure `step`), the `MouseScheme` setting, the box select, one hover pick per frame, the cursor table | 609 |
| `C_mesh_bvh.md` | `MeshBvh` in core (`mesh/bvh.rs`), its parity proof against brute force, the pick cache, the off-frame build job | 537 |
| `D_renderer.md` | MSAA, the per-face state buffer and the face pipeline, the face edges, the ID pass and outline | 732 |
| `E_selection_edits.md` | `AddPicks` / `RemovePicks`, the key check, the rule offers as "select similar", Ctrl+A, undo per act, the MCP wire in both directions | 387 |
| `F_selection_interface_and_parity.md` | The one selection contract, the GUI / MCP / CLI parity matrix, `focus()` door, `get_ui_view`, the requirements each sibling meets | 487 |
| `RULINGS.md` | Every open question with evidence and a recommendation; the lead's answer column; the reconciliation table | — |

## What the dive corrected in `REPORT.md`

The code outranks the report (planning/CLAUDE.md). Rows SEL-2, SEL-3,
SEL-7 and SEL-12 and the B.1 hover arithmetic were corrected on
2026-10-28 from these findings:

- Shift sends one pick (`AddPick`); the right-click adds the layer rule.
  Shift outside pick mode reaches nothing; `Selection::Face/Faces` are dead
  code (F-1, verified).
- Pick mode runs up to three picks per frame, not two (B §0).
- "Select similar" needs no new rule kind: `Filter { z, face, diameter }`
  and GT-3 `rule_offers` exist (E §0.1). The gap is the entry points.
- A viewport pick is not its own undo step; the inspector snapshot
  coalesces N clicks (E §0.2, F-8).
- No door checks that a pick key names an item (E §0.3); the MCP wire
  converts one way only (E §0.4, F-9).
- The hover rebuild is 120 B per triangle, not 108 (the index triple, D §2.1).
- The Front and Right presets are mislabelled (A-Q1, verified).
- Two STEP models light the same face index; the STL flat-face hover ring
  never finds a face (D §2.5, §2.7).

## The one contract (F §4, binding on every file)

| State | Owner | The one read door | The one write door |
|---|---|---|---|
| Op selection (rules, picks, excludes; Op and Boundary) | core `ProjectSession` | `session.selection_status(tc, target)` → `Arc<SelectionReport>` | `Command::EditSelection` |
| Tree focus | `AppState` | `AppState::focus()` | `AppController::set_focus()` (the field becomes private) |
| Pick mode, row hover | `GuiState.selection_ui` | getters | setters (view only) |
| Hover | the viewport frame | one `HoverTarget` | `HoverResult` once per frame |
| Camera | `AppState.camera: CameraState` | `pose()` | `CameraState` methods; MCP through `apply_camera_request` (instant) |
| Drag gesture | `SelectionUi.drag: DragState` | — | the pure `interaction::input::step` |

A gesture that changes what an op cuts ends in `Command::EditSelection`.
A gesture that changes what the user looks at writes view state through
one named door. The CLI gets no camera, hover or pick-mode door (view
state); its job key `selection` already holds a whole `Selection` with a
`picks` list, so batch picks and rules need no CLI change. `SelectionReport`
is the one selection summary; no `SelectionSummary` type is added.

## Overlaps and how they were resolved

The reconciliation table in `RULINGS.md` holds the detail. In short: the
camera lives on `AppState` (A); the drag state machine lives on
`SelectionUi` with a pure `step` (B), and it drives A's `orbit_about` with
the pivot it holds, so A's `CameraDrag` is dropped; the mouse scheme is a
core `setting_token!` (B) and A's `gesture()` is B's `classify()`; B's
`HoverResult` writes F's one `HoverTarget`, which D's face-state buffer
reads; undo per act is E's `push_act_step`; the MCP camera fields take F's
shape (one `camera` object on `set_ui_view`) with A's validation order and
echo; Escape becomes F's `UiCommand::EndViewportMode` row.

## The merged commit order

Each commit lands alone and green: the folder sentries, the crate `--lib`
tests, `rs_cam_mcp` tests, clippy, fmt, through `scripts/cargo_lane.sh`
with a timeout. Agents implement; the lead commits with explicit paths.
"Break" means the commit message states it (ruling 2026-09-16).

| Phase | Commit (design, its number) | Depends on | Break |
|---|---|---|---|
| 0 Cleanups and door fixes | F1 retire `Selection::Face/Faces`; `selected_face_ids` reads `highlight_report` | — | none |
| | F2 the focus door: private field, `focus()`, `set_focus()`, `wire_kind()` | — | every direct focus write (about 40, MCP `toolpath_index` included) now raises the upload flag |
| | B1 one hover pick per frame (`HoverResult`, cursor table) | — | none |
| | E4 undo per act (`push_act_step`), `FEATURE_CATALOG.md:123` | — | click undo: per act, not per visit |
| | D5 STL flat-face hover ring (option A) | — | none |
| 1 Camera and pick speed | A C0 planning: orthographic claims in the page and `check_claims.mjs` | — | — |
| | A C1 camera math (`orbit_about`, `zoom_toward`, `pan_pixels`); wheel zooms to the cursor | — | `orbit`/`pan`/`zoom` removed; pan speed at heights other than 828 px |
| | A C2 camera to `AppState.camera` (`state/camera.rs`) | A C1 | `fit_camera_to_*` removed |
| | B2 input state machine with a temporary left-orbit row (no behaviour change) | A C2 | none |
| | A C3a orbit about the pivot under the cursor, pan holds the grabbed point, pinch zoom; on the present left button (no habit change; brute-force pick per drag start until C4 lands, A-Q8) | A C2, B2 | none |
| | C1 `MeshBvh` + parity tests; C2 pick cache and `LoadedModel::ray_pick`; C3 the Job row (one commit); C4 the viz call site | — | none |
| | A C5 + F3 MCP: `set_ui_view camera {preset, projection, fit, pose}` + `get_ui_view` + `FocusSummary`; one wire re-bless | A C2, F2 | echo `selection` tokens replace `"other"`; schema grows |
| 2 Feel | A C4 `CameraMotion`: eased presets and Fit, repaint only while moving, screenshots snap or wait; fix the Front/Right presets (A-Q1) | A C2 | preset orientation |
| | D1 MSAA (one `sample_count` field, resolve target, headless frame arm) | — | none |
| | D2 face pipeline (`FaceMeshVertex`, palette in `colors.rs`); D3 `face_state` slot, `(ModelId, FaceGroupId)` inputs, zero-rebuild sentry | D1 (for the sample count field) | `enriched_mesh_gpu_data` deleted (D-Q3) |
| | D4 face edges slot, `LineUniforms` 96 B, registry row `model_edges` | D1 | none |
| 3 Selection | E1 core `AddPicks`/`RemovePicks`, `check_keys_name_items`, GT-Q8 for a list, `hole_rim_key` per key (B §3.4); E2 two face offers; E3 wire variants both ways + `build_info` token | — | MCP `add_pick` of a key that names no item refuses |
| | B3 the scheme: `MouseScheme` setting (`WebCad` default), right-drag orbit, middle-drag pan, shortcuts section | A C3a; the lead's live look (A-Q9) | the left drag no longer orbits; a settings file with no `mouse_scheme` key starts in `WebCad` |
| | B4 Ctrl-click removes (`PickClick::Ctrl`, `AppEvent::RemovePick`); F4 `EndViewportMode` row and `HoverTarget` | B2 | `click_event` returns `Option<AppEvent>` |
| | B6 box select (`pick_items_in_rect`, painter, `AddPicks`/`RemovePicks` events, Escape cancel) | B3, E1, E3 | none |
| | E5 right-click offers in pick mode, Ctrl+A, MCP `list_rule_offers` (E-Q2) | E2 | none |
| | A C6 key F fits the selection | A C2 | none |
| | F7 CLI job key `[operation.boundary_selection]` (F-Q3) | E1 | a new job key |
| 4 Later | D6 ID pass, blit outline, readback ring, `CREDITS.md` (rerun, WebGPU Fundamentals) | D2, D3, B6 | none |
| | "Select other" cycle (GUI only, E-Q6) | D6 | none |
| | A C7 `viewport_overlay::draw` reads the projection from `state` (optional) | A C2 | a signature |

Evidence per commit: an MCP screenshot pair (`set_ui_view` then
`screenshot_gui`, before and after) in
`planning/viewport_parity_2026-10-28/evidence/`, on a release binary built
from HEAD.

## Landed

| Phase | Commit | Hash | Date |
|---|---|---|---|
| 0 | F1 retire `Selection::Face/Faces` | 4b509e57b | 2026-10-29 |
| 0 | F2 the focus door (`focus()`, `set_focus()`, `wire_kind()`; `AppState::set_focus` is `pub` for the integration tests, as `replace_session` is) | d2903b42b | 2026-10-29 |
| 0 | B1 one hover pick per frame (`HoverResult`, no pick during a drag, PointingHand cursor) | 03e5fa752 | 2026-10-29 |
| 0 | E4 undo per act (`push_act_step`, `toolpath_parts_moved` in `state/history.rs`) | 591c4740f | 2026-10-29 |
| 0 | D5 STL flat-face hover ring (option A) | 7c9b90595 | 2026-10-29 |
| 1 | A C0 orthographic arm and claims C0o-C3o in the page | 5dcd157db | 2026-10-29 |
| 1 | A C1 camera math (`orbit_about`, `zoom_toward`, `pan_pixels`); the wheel zooms to the cursor | 67357f9fc | 2026-10-29 |
| 1 | A C2 the camera on `AppState.camera` (`state/camera.rs`) | 74f02d98b | 2026-10-29 |
| 1 | B2 the input state machine (`DragState`, pure `step`, `MouseScheme` table with a `LeftOrbit` row); the pinch zooms | 20b0e55c3 | 2026-10-29 |
| 1 | A C3a orbit about the point under the press; the pan holds the grabbed point | 3748829ef | 2026-10-29 |
| 1 | A C5 + F3 MCP `set_ui_view camera`, `get_ui_view`, the focus echo; display-frame geometry to `state/display_frame.rs` (layering sentry f3) | 0e8bebedf | 2026-10-29 |
| 2 | A C4 `CameraMotion`; the presets follow one yaw table (Front, Right, Top and Isometric fixed) | 7befd4558 | 2026-10-29 |
| 3 | E1-E3 `AddPicks`/`RemovePicks`, `check_keys_name_items`, two face offers, the wire both ways | 6034bde9e | 2026-10-29 |
| 2 | D1 MSAA (branch `viewport-render`) | d83da5d17 | 2026-10-29 |
| 1 | C1 `MeshBvh` with the parity proof (branch `viewport-bvh`); the prune needed a relative slack `RAY_PRUNE_REL` for a flat sheet at zero, a hole in the design's proof | e1212253f | 2026-10-29 |
| 1 | C2 the pick-index cache (`cached_bvh`, `peek_bvh`, no count cap) and `LoadedModel::ray_pick` | 8e732e624 | 2026-10-29 |
| 1 | C3 the Job row `build_model_pick_index` and the GUI pump | 8ad6fa0fe | 2026-10-29 |
| 1 | C4 the viewport picks through `ray_pick` | f157ec94e | 2026-10-29 |
| 2 | D2 + D3 the face pipeline (`FaceMeshVertex`, the face-state buffer, the palette); a hover rewrites no mesh; per-model face highlight | d3bf90ee3 | 2026-10-29 |
| 2 | D4 face edges (`model_edges` slot and row, the view-space offset; the margin is eight depth steps, not four: wgpu keeps clip z in [0, w]) | 181eb0878 | 2026-10-29 |
| 3 | B4 Ctrl-click removes the item | 7e62f8905 | 2026-10-29 |
| 3 | A C6 the F key fits the selection | 7f6a63f6e | 2026-10-29 |
| 3 | F7 the CLI job keys `boundary` and `boundary_selection` | a2e5a4686 | 2026-10-29 |
| 3 | E5 the right-click rule menu, Ctrl+A, MCP `list_rule_offers` | 969ca2fcf | 2026-10-29 |
| — | the two branches merged into master | 1343e37d0 | 2026-10-29 |

Phase 0 took every recommendation; the `RULINGS.md` column was still
empty. E4 also fixed a latent defect no design file names: the panel
flush (`flush_toolpath_snapshot`) pushed with no check for a change, so a
tab act followed by a move off the toolpath pushed an empty undo step.
MCP `edit_toolpath_selection` takes `controller.apply` directly and still
rides the inspector snapshot. E4 and D5 are verified by tests, not on
screen: Undo has no MCP tool and the hover has no pointer; they need the
operator's GUI look.

## What the lead rules before phase 1

`RULINGS.md` holds 47 questions, each with a recommendation. The ones that
change the build order or a wire shape: A-Q1 (preset fix), A-Q9 and B-O2
(the scheme and when it lands), C-Q1 and C-Q3 (own BVH, off-frame job),
D-Q5 (GL backend), E-Q1 (refuse a batch with one bad key), E-Q2
(`list_rule_offers`), F-Q1 (delete the dead variants), F-Q3 (CLI boundary
key). The rest can take the recommendation.
