# Rulings the lead must give (viewport parity design, 2026-10-28)

One row per open question from the design files A-F. The lead's answer
goes in the last column. "Rec." is the design agent's recommendation after
the lead's own check.

## A. Camera (`A_camera.md` §7)

| # | Question | Evidence | Rec. | Ruling |
|---|---|---|---|---|
| A-Q1 | The Front and Right presets are mislabelled. | VERIFIED by the lead with the page camera model (the same eye formula as `camera.rs:54-59`): yaw 0 ("Front") puts the eye on +X with +Y to the right on the screen, which is the conventional Right view; yaw +π/2 ("Right") puts the eye on +Y with +X to the left, which is a Back view. Z-up CAD convention: Front = eye on −Y, +X to the right; Right = eye on +X. `PreviewOrientation` (`app/input.rs:294-298`) disagrees with the presets too. No test pins the convention. | Fix in the camera commit C4: Front = yaw −π/2, Right = yaw 0, one table shared with `PreviewOrientation`; state the break in the commit. Take one `screenshot_gui` pair on a stock with a labelled datum as evidence. | taken as recommended, 2026-10-29, 7befd4558 (Top and Isometric fixed too) |
| A-Q2 | Move the view arms (`app/input.rs:274-303`) into the controller? | The controller has no frame clock; an eased write needs `now`. | Keep them in `app/input.rs`; the parity test covers the `CameraState` call both doors make. | taken as recommended, 2026-10-29, 74f02d98b |
| A-Q3 | The tween duration token. | `tokens.rs:705` `MOTION_SLOW` = 0.24 s; the reference range is 0-500 ms (REPORT A.4b). | Reuse `MOTION_SLOW`; no second near-equal number. | taken as recommended, 2026-10-29, 7befd4558 |
| A-Q4 | F12 during a motion. | — | Wait for the end with a one-shot flag; an intermediate pose has no use. | taken as recommended, 2026-10-29, 7befd4558 |
| A-Q5 | Wire units for yaw and pitch on `set_ui_view`. | — | Degrees, unit in the key (`yaw_deg`, `pitch_deg`); the crate keeps radians. | taken as recommended, 2026-10-29, 0e8bebedf |
| A-Q6 | `fit: selection` and key F with no picked items. | — | MCP refuses with the reason; the GUI F key falls back to Reset View. | taken as recommended, 2026-10-29, 7f6a63f6e |
| A-Q7 | Distance interpolation: linear or geometric. | The page model (claim C4) is linear. | Linear; change only after a screenshot sequence shows a problem, with the page updated in the same commit. | taken as recommended, 2026-10-29, 7befd4558 |
| A-Q8 | The orbit pivot before the BVH lands. | Brute force is about 2 ms per pick on the terrain (REPORT B.1). | One brute-force pick per drag start; fall back to the point at target depth, then the target. | taken as recommended, 2026-10-29, 3748829ef |
| A-Q9 | Land the scheme change (C3) before a live look? | The left button stops orbiting. | Land C1, C2, C4, C5 first; C3 after the lead tries the scheme on screen. | deferred to the operator's live look (B3 not landed); under `LeftOrbit` a left drag in pick mode is a box since 87fae1431 |

## E. Selection edits (`E_selection_edits.md` §0 and §7)

Four code facts that correct REPORT.md (verified by the agent with file:line):
`Rule` is a struct with `Filter { z, face, diameter, .. }`, so "select similar"
needs no new rule kind (GT-3 `rule_offers` already derives the offers); a
viewport pick is not its own undo step today (the step is pushed on leaving
the inspector, `panel_apply.rs:489`); no door checks that a pick key names an
item (MCP `add_pick` of a wrong key stores a lost pick); the wire converts
one way only (`server.rs:2084`), which breaks the F166 rule.

| # | Question | Evidence | Rec. | Ruling |
|---|---|---|---|---|
| E-Q1 | A batch with one bad key: refuse all, or apply the rest? | Rule 4 "no silent fallback" (DESIGN.md). | Refuse all; apply the same check to the single `AddPick` (a stated break of MCP behaviour). | taken as recommended, 2026-10-29, 6034bde9e |
| E-Q2 | Add an MCP `list_rule_offers` query? | Without it the stored rule has parity but the agent derives `z ± 0.01` itself and never sees the label or the count. | Yes: one `Query` row, one wire param, one handler; number parity (operator rule 2026-09-23). | taken as recommended, 2026-10-29, 969ca2fcf |
| E-Q3 | "At Z" tolerance. | `Z_HALF_RANGE_MM = CAD_ROUNDING_MM` (`rule_offers.rs:40`, L-13). | Settled: no new constant. | no change: the existing offers stand (6034bde9e) |
| E-Q4 | A `FaceNormal` rule? | `FaceNotFlat` refuses every face under `HORIZONTAL_FACE_MIN_NORMAL_Z = 0.95`; `Facing` plus `FaceFilter::z` carry "same plane". | No; add two face offers instead ("all up faces", "down faces at Z"). | taken as recommended, 2026-10-29, 6034bde9e (no `FaceNormal`) |
| E-Q5 | Ctrl+A at all, given O-PICK? | Under the default, `Only(kinds)` changes the D2/D13 refusals. | Keep it small: a notice under the default, one `add_rules` edit after a pick. | taken as recommended, 2026-10-29, 969ca2fcf |
| E-Q6 | "Select other" cycle: core or GUI? | The chosen key goes through the ordinary `pick_edit`. | GUI only; the ID list of step 10 feeds it. | not built (phase 4) |
| E-Q7 | Make every viewport selection act one undo step (`push_act_step`)? | Today N clicks on one toolpath undo as one step at navigate-away; `FEATURE_CATALOG.md:123` claims one step per edit. | Yes; it makes the catalogue claim true and gives the box one step. | taken as recommended, 2026-10-29, 591c4740f |
| E-Q8 | A plain left drag in pick mode: add or toggle? | A toggle needs a mixed `AddPicks` + `RemovePicks` or a `Replace`. | Add; Ctrl+drag removes. | taken as recommended, 2026-10-29, 87fae1431 (a box adds) |

## B. Input state machine, scheme, box select (`B_input_and_box_select.md` §8)

| # | Question | Evidence | Rec. | Ruling |
|---|---|---|---|---|
| B-O1 | Box select outside pick mode? | A box needs a target; outside pick mode the default target is the op (O-PICK). | No in the first landing: a left drag outside pick mode does nothing, and the shortcuts window says so. | taken as recommended, 2026-10-29, 87fae1431 (pick mode only) |
| B-O2 | The default scheme. | A.2b: the right-button family is the web-tool family. | `WebCad` (right orbit, middle pan, left select). | deferred to the operator's live look (B3) |
| B-O3 | The box drag threshold. | egui `max_click_dist` = 6.0 pt (egui 0.36.2 `input_state/mod.rs:113`). | egui's own value; no rs_cam constant (no underived default). | taken as recommended, 2026-10-29, 87fae1431 (egui `max_click_dist`) |
| B-O4 | Crossing-box colour: Onshape yellow? | `tokens.rs` keeps `CAUTION` for verdicts. | No: `ACCENT`, solid for enclose, dashed for crossing. | taken as recommended, 2026-10-29, 87fae1431 (accent, dashed for crossing) |
| B-O5 | Right drag under `DesktopCad`. | SolidWorks gestures give it no action; Plasticity pans. | Pan, so a two-button mouse pans. | taken as recommended, 2026-10-29, 20b0e55c3 |
| B-O6 | Two-finger touchpad pan. | egui cannot tell a touchpad scroll from a wheel reliably; Shift+scroll is egui's horizontal scroll. | Not in this landing; needs its own ruling. | not built; two-finger scroll zooms (20b0e55c3) |
| B-O7 | A right-click under `WebCad` (layer rule, tab delete). | egui separates a right click from a right drag by `max_click_dist`. | Keep both. | open until B3 |
| B-O8 | A plain box: add or toggle? | A toggle over many items is unpredictable; Onshape and Fusion add. | Add; Ctrl box removes (agrees with E-Q8). | taken as recommended, 2026-10-29, 87fae1431 |
| B-O9 | Hover during a drag. | Each face change under an orbit rebuilds the enriched mesh today. | Clear the hover during any drag. | taken as recommended, 2026-10-29, 03e5fa752 (no pick during a drag) |
| B-O10 | Lasso. | — | Not in this landing; a later `Lasso` state with the same release path. | not built |

## F. Selection contract and surface parity (`F_selection_interface_and_parity.md` §9.1)

| # | Question | Evidence | Rec. | Ruling |
|---|---|---|---|---|
| F-Q1 | Delete `Selection::Face` and `Selection::Faces`? | VERIFIED by the lead: no production constructor; only match arms and two W10 tests. A face click sends `TogglePick`. | Yes; `selected_face_ids` then reads `highlight_report` (one door, fixes F-4). | taken as recommended, 2026-10-29, 4b509e57b |
| F-Q2 | One undo step per selection act? | Today edits coalesce into the inspector snapshot (F-8); same finding as E-Q7. | Yes (the `EditTabs` treatment). | taken as recommended, 2026-10-29, 591c4740f |
| F-Q3 | A CLI job key for the boundary selection? | The job writes `SelectionTarget::Op` only (`job.rs:1294-1301`). | Yes: `[operation.boundary_selection]`. | taken as recommended, 2026-10-29, a2e5a4686 |
| F-Q4 | A toggle edit on the MCP wire? | Core: "No toggle edit exists here" (`rule.rs:366-368`); a toggle needs the resolved state. | No; the agent reads `selected` first. | taken as recommended, 2026-10-29, 6034bde9e (no toggle on the wire) |
| F-Q5 | An MCP door to set pick mode? | Pick mode is view state; the agent names keys. | No write door; report it in `get_ui_view`. | taken as recommended, 2026-10-29, 0e8bebedf (`get_ui_view` reports it; no write door) |
| F-Q6 | Notify agents on a selection change? | UX_REVIEW §9 Q5 (B): a state shows on the row, no toast. | No; `get_ui_view` is the read door. | taken as recommended, 2026-10-29, 0e8bebedf (no notification) |

## Reconciliation of the overlaps (lead decisions, 2026-10-28)

| Overlap | A says | B says | F says | Decision |
|---|---|---|---|---|
| Where the camera lives | `AppState.camera: CameraState` (`state/camera.rs`), pose private | the camera stays on `RsCamApp` | a camera module under `state/` with one `apply_camera` door | **A**: `AppState.camera`. B's testability argument is the same as A's; MCP parity needs the state door. F's `apply_camera(CameraRequest, animate)` is A's `apply_camera_request` plus the `CameraWrite` flag: one method, two callers. |
| The drag state | `CameraDrag` inside `CameraState` (`begin_drag`, `drag_by`, `end_drag`) | `DragState` on `SelectionUi` with a pure `interaction/input.rs::step` | a pure `gesture_events` facade | **B** holds the one state machine (it also holds the tab and the box). A drops `CameraDrag`; `step` returns `InputAction::Orbit { dx, dy }` and the frame calls A's `orbit_about(pivot, dx, dy)` with the pivot from `DragState::Orbit`. F's `gesture_events` is the click half of B's actions (B keeps camera actions out of `AppEvent`). |
| The scheme type | `NavScheme` in `state/camera.rs`, preference in `app_settings` | `MouseScheme` as a core `setting_token!` in `[display]`, `classify` + `rows` one table | — | **B**: the core settings pattern persists it; `rows(scheme)` feeds the shortcuts window. A's `gesture()` is B's `classify()`. |
| The hover slot | — | one `HoverResult` per frame | one `HoverTarget` (replaces `row_hover` as the upload input, `HoverDetector`, `last_hover_face`) | **Both**: B's `HoverResult` is the per-frame computation; it writes F's one `HoverTarget`; D's face-state buffer reads `HoverDetector::hovered()` only. |
| Undo per act | — | one `AddPicks` per box | `EditTabs` treatment | **E's `push_act_step`**; F-Q2 and E-Q7 are one ruling. |
| Batch dedup through `hole_rim_key` | — | core must map each key through `hole_rim_key` before dedup | — | **E commit 1** takes it, so MCP gives the same result (B §3.4). |
| Batch `BoundaryToSelection` | — | needed for a box on a boundary whose source is not `Selection` | the GUI click sends `SetBoundaryConfig` first (F-11) | **Gap**: until a batch form exists, the box shows the notice "Set the boundary source to Selection first" (B §3.4). Add to E's scope. |
| Escape | — | consumes Escape to cancel a box, then ends the mode | a `UiCommand::EndViewportMode` row | **F**: the row; B's cancel-then-end order holds inside its handler. |
| MCP camera fields | `view_preset`, `projection`, `fit`, `camera` flat on `set_ui_view` | — | one `camera` object on `set_ui_view` | **F's shape** (one `camera` object with `preset`, `projection`, `fit`, `pose`): one key to document, one refusal path. A's validation order and echo stay. |

## C. Mesh BVH (`C_mesh_bvh.md` §7)

Facts the agent verified: the brute-force tie rule (`mesh.rs:380-388`, strict `<`, lowest index) applies to bit-equal `t` only; the probe's "ties" differ by under 1e-9. `BoundingBox3::ray_intersect` returns the EXIT `t` for an origin inside the box, so a prune on it would skip nearer triangles (a zoomed camera sits inside the model box); the BVH needs its own slab test. Core tests cannot read `planning/` (F155); the parity fixture is `crates/rs_cam_core/tests/fixtures/terrain.stl` (214 997 triangles).

| # | Question | Evidence | Rec. | Ruling |
|---|---|---|---|---|
| C-Q1 | Own BVH or pin `parry3d-f64 0.24`? | 0.24 pulls nalgebra 0.34 beside 0.33; 0.31 is glam-based; parry misses edge-exact rays our test hits (REPORT B.1). | Own flat BVH, about 250 lines, in `src/mesh/bvh.rs` owned by `mesh.rs` (`pub mod bvh;`), with the parity proof of C §2.4 and the same `Triangle::ray_intersect`. | taken as recommended, 2026-10-29, e1212253f (own BVH; the prune gained `RAY_PRUNE_REL`) |
| C-Q2 | Leaf size. | Memory scales with 1/L; the cast is O(log n + L). | Ship 8; a sweep over {2, 4, 8, 16} on the probe meshes after commit C1 sets the final value (no number from memory). | 8 shipped (e1212253f); the leaf-size sweep instrument not run |
| C-Q3 | Build thread. | 62-451 ms in the probe; the CLI and MCP never cast a ray. | A Job row `build_model_pick_index` on the GUI Job lane (the `build_model_faces` pattern), lazy per shown mesh model; brute force until it lands (same answer, slower). Never at import, never on the frame. | taken as recommended, 2026-10-29, 8ad6fa0fe |
| C-Q4 | Move `SpatialIndex` beside the BVH? | 190 files name `mesh::SpatialIndex`. | Not in this work. | not moved |
| C-Q5 | No count cap on the pick table. | A cap of 4 thrashes with five shown models or one model plus a generation. | Accept the liveness bound (the dead-mesh sweep; at most about 21 % of live mesh bytes at L = 8); `peek_bvh` sweeps on a miss too. | taken as recommended, 2026-10-29, 8e732e624 |
| C-Q6 | f32 node boxes? | Needs outward rounding and a second pad proof. | No; keep f64. | taken as recommended, 2026-10-29, e1212253f (f64 kept) |

## D. Renderer (`D_renderer.md` §7)

Facts the agent verified in the wgpu 30 sources: `R32Uint` cannot be multisampled (`format.rs:969`), so the ID pass is its own single-sample pass; depth bias is refused for `LineList` (`resource.rs:4589`), so edges use a vertex-shader offset toward the eye, `max(chord tol, 2^-22·z²/near)`; only 4 samples are guaranteed. Defects found: two STEP models light the same face index (bare `FaceGroupId` in `gpu_upload.rs:28-57`); the STL flat-face hover ring never finds a flat face (`hover_vertices` reads `m.items` only); REPORT B.1 omitted the 12-byte index triple per triangle (corrected to 120 B).

| # | Question | Evidence | Rec. | Ruling |
|---|---|---|---|---|
| D-Q1 | Line width: 1 px `LineList` or quad-expanded lines? | MSAA smooths a 1 px line; HiDPI halves it. A quad pipeline is M and moves the hover ring, tab marks and edges together. | Keep 1 px with MSAA; decide on the screenshot pair of `stepped_block.step`. | taken as recommended, 2026-10-29, 181eb0878 (1 px) |
| D-Q2 | MSAA count: fixed 4 or adapter max? | Only 4 is guaranteed; 8 needs a feature egui does not request and doubles 285 MiB at 4K. | Fixed 4 with a fallback to 1 from `get_texture_format_features`. | taken as recommended, 2026-10-29, d83da5d17 (fixed 4) |
| D-Q3 | Keep the unindexed flat path as a fallback? | Per-face vertices already give hard creases between faces. | No; delete `enriched_mesh_gpu_data`, keep the `EnrichedMeshGpuData` type for the reach overlay. | taken as recommended, 2026-10-29, d3bf90ee3 (deleted) |
| D-Q4 | ID pass timing. | The CPU BVH answers picks. | After the BVH (C) and the box select (B); it earns its cost for the outline and "visible only". | not built (phase 4) |
| D-Q5 | GL backend support? | egui's GL adapter allows zero storage buffers per stage, so the face pipeline fails there. | If GL matters, hold `face_state` in an `R32Uint` texture; if not, state "Vulkan, Metal or DX12 only" in the render `CLAUDE.md`. | taken as recommended, 2026-10-29, d3bf90ee3 (Vulkan, Metal or DX12 stated) |
| D-Q6 | `gpu_safety` budget: log or gate? | No VRAM query exists; a gate needs a typed number. | Log only. | taken as recommended, 2026-10-29, d83da5d17 (log) |
| D-Q7 | Raise `near` from 0.1? | The edge offset grows with z²/near at far zoom. | Raise it with the camera work (A), where the near plane can follow the scene bounds. | not raised; the edge margin is derived from `near` (181eb0878) |
| D-Q8 | Do STEP faces fill with item states (Cut, Skipped)? | REN-2 fill-vs-outline is UNCONFIRMED in every reference tool. | Reserve the state bits now; fill only after the UX review rules. | taken as recommended, 2026-10-29, d3bf90ee3 (slots reserved, empty) |
