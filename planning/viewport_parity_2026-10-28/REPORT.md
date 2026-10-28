# 3D viewport and selection parity: research report (2026-10-28)

> Implementation status (2026-10-29): phases 0-3 of `design/DESIGN.md` landed on master; see its "Landed" table and `FOLLOW_UPS.md`. The gap matrix below is the 2026-10-28 reading and is not updated row by row.

Research only. No product code changed. Nothing committed. Brief:
`BRIEF.md`. Sources: `SOURCES.md`. Code read at HEAD b5aa2a45
(2026-10-28). The earlier selection programme
(`planning/geometry_selection_2026-10-16/`) decided WHAT a user selects.
This report covers HOW the selection and the camera feel in the 3D view
[geomsel].

## Summary (one screen)

**The question.** What does rs_cam need for parity with the web CAD tools
in 3D interaction, selection first, and how do we get it?

**The lead's eight claims all verify** at the cited lines (A.1-A.4), with
three corrections: the hover rebuild hits STEP faces only (drawing items
already have their own hover buffer); edge and vertex picks partly exist
(horizontal STEP edges and corners are items in pick mode); and the
brute-force ray is a second-order cost (B.1), the per-hover mesh rebuild is
the first.

**Top five gaps by value per cost (B.2):**

1. **Hover and selection colours live in the vertex buffer** (SEL-1): one
   hovered-face change rebuilds and re-uploads 120 bytes per triangle of
   the whole part (15.8 MB on a 132 k-triangle part). Fix: a per-face state
   buffer and a `face_id` vertex attribute.
2. **The camera orbits a fixed target and zooms to the screen centre**
   (CAM-1, CAM-2, CAM-3): no pivot under the cursor, no zoom to the cursor,
   a pan rule that is exact at one viewport height only. Fix: three
   formulas in `camera.rs`, proven by the page model (claims C1-C3, C8).
3. **No box select, and the left button orbits** (SEL-4, CAM-8): every
   CAD tool selects with the left button and orbits with the right or the
   middle; the convergent box rule is left-to-right = enclose,
   right-to-left = crossing. Fix: adopt the Onshape scheme, add a batch
   `SelectionEdit`.
4. **No face edges and no anti-aliasing** (REN-1, REN-4): Onshape's
   default is "shaded with edges"; our offscreen target is single-sample.
   The ordered edge polylines exist (`FaceGroup.boundary_loops`); the
   line pipeline exists. Fix: one upload slot, one `sample_count` change.
5. **No camera control over MCP** (CAM-9): an agent cannot set a preset or
   the projection, so every screenshot is from whatever view the GUI holds.
   Fix: four fields on `set_ui_view`.

**Framework verdict (C.1).** A full engine is too much. Bevy is the one
engine on wgpu 30 and egui 0.36 today, and that is a three-way version
lock; the realistic embed makes Bevy the host, which replaces the eframe
loop, the window and the screenshot path, for a picking backend that is
the same brute-force raycast we have. three-d and Fyrox are OpenGL, rend3
is archived, kiss3d owns the entry point. The lead's opinion holds, with
one addition: egui is not the limit either, because the viewport is our
own wgpu pass and every gap above is a feature we did not build, not a
framework ceiling. The useful borrowings are ideas (a pivot camera, a
jump-flood outline, an ID buffer) and small MIT or Apache crates (C.2).

**Measured (B.1).** Brute-force pick: 1.8 ms mean per ray on the 132 k
terrain, 5.3 ms at 1.28 M triangles, 0.045 ms at 9 800; a BVH cast: about
2.6 µs, build 2 ms to 451 ms once per mesh. The four disagreements in 2000
rays are rays exactly on a shared edge, where parry misses and the product
test reports a tie. The hover path runs the pick twice per frame.

**Recommended path (D.1), first three steps:** (1) a frame-time and
pick-time instrument plus cursor and tooltip feedback; (2) a BVH per mesh
model, one pick per frame; (3) the camera: pivot under the cursor, zoom to
the cursor, derived pan, the Onshape mouse scheme with a preference for the
SolidWorks scheme. Then MCP camera fields, MSAA, eased presets, face edges
with per-face smooth shading, the per-face state buffer, the box select,
the ID pass with the selection outline, select-similar rules, and last the
view cube. Each step is S or M, lands alone with a sentry and an MCP
screenshot pair, and never stops the GUI.

**Must not:** move onto an engine, replace egui, start with GPU picking,
bake state into vertices, animate the camera for an MCP call, or change
the mouse scheme without a preference (D.2).

**Deliverables:** this report; `SOURCES.md`; `viewport_parity.html` with
`check_claims.mjs` [claims] (all claims hold, run 2026-10-28); `probe/` with the
pick-latency probe source and logs.


## A. Requirements

Each row names a requirement, what reference tools do, and our state.
`V` = `crates/rs_cam_viz/src/`, `C` = `crates/rs_cam_core/src/`.

### A.1 Selection

Our state at HEAD, with the file and line. The reference-tool evidence is
in table A.1b. Rows SEL-2, SEL-3, SEL-7 and SEL-12 carry corrections from
the design dive of the same day (`design/`); the design files hold the
evidence.

| ID | Requirement | Our state | Where |
|---|---|---|---|
| SEL-1 | Hover pre-highlight, instant | A hovered STEP face takes a hover colour. The colour is a vertex attribute of the whole part, so a change of the hovered face rebuilds and re-uploads the full part buffer (3 unindexed vertices of 36 bytes and 3 indices of 4 bytes per triangle). A hovered drawing item (loop, path, circle, point) draws a ring from its own small buffer, so that hover is instant. An STL flat face has no hover colour. | `V/render/mesh_render.rs:258-305`; `V/app/viewport.rs:706-770`; `V/app/gpu_upload/meshes.rs:6-8`; `V/app/gpu_upload.rs:142-175`; `FEATURE_CATALOG.md` row "Face selection scope"; the defect record [perf-v13] |
| SEL-2 | Click select with a low latency | One CPU ray tests every triangle of every visible mesh model. The hover path runs that pick twice per frame outside pick mode (the face hover and the toolpath tooltip) and up to three times in pick mode (`pick_mode_item` runs `pick_items` up to twice, then the face hover runs `pick()`; design B §0). Probe (section B.1): 1.8 ms mean per ray on a 132 k-triangle terrain, 5.3 ms on 1.28 M triangles, 0.045 ms on 9 800. | `C/mesh.rs:373-392`; `V/interaction/picking.rs:233-306`; `V/app/viewport.rs:727-766` and `:777-815` |
| SEL-3 | Add and remove with a modifier | In pick mode a plain click toggles the item (`TogglePick`), shift-click adds one item (`AddPick`), right-click adds the layer rule (`PickLayer`). Outside pick mode Shift reaches nothing: the tree variants `Selection::Face/Faces` have no producer (design F-1, verified). Ctrl does nothing. (Corrected 2026-10-28 from the design dive; the first draft said shift-click adds the layer rule.) | `V/ui/pick_bar.rs:73-91`, `:401-402`; `V/app/viewport.rs:356-371`; `C/selection/rule.rs:371-391` |
| SEL-4 | Box and lasso select | None. A primary drag orbits the camera. | `V/app/viewport.rs:848-851` |
| SEL-5 | Select-through vs visible-only | A face pick takes the nearest hit only. An item pick takes the nearest drawn line on the screen; a tie within 2 px takes the item nearest the surface (UX-7). No way to reach an occluded face. | `V/interaction/picking.rs:233-306`, `:459-490` |
| SEL-6 | Chain, tangent and loop select | A STEP ring is one `Loop` item, a horizontal edge run is one `Path`, a round rim is one `Circle`: one click takes the whole chain. A DXF or SVG chain is one item after the import join. No tangent chain across items. | `C/io/brep_items.rs:1-45`; `C/io/CLAUDE.md` |
| SEL-7 | Select similar (same radius, plane, depth) | The rule kinds exist: `Filter { layers, kinds, diameter, face: FaceFilter { facing, z }, z }` (`Rule` is a struct, not an enum), and GT-3 `rule_offers` already derives "same diameter", "up faces at this Z" and "items of this kind at this Z" from one picked item (tolerance `CAD_ROUNDING_MM`). The gap is the entry point: the viewport has no route to the offers and MCP has no offers query (design E §0, §2). | `C/selection/rule.rs:130-196`, `:284`; `C/session/rule_offers.rs:59-205`; `V/ui/properties/selection_section.rs:729` |
| SEL-8 | Selection filters (face, edge, vertex, loop) | The pick bar has five kind chips: closed contours, open contours, circles, faces, points. A sloped or vertical STEP edge is skipped at import, so no chip reaches it. | `V/state/pick.rs:33-39`; `C/io/brep_items.rs:22-24` |
| SEL-9 | Pick list for stacked candidates | None. The pick returns one hit. | `V/interaction/picking.rs:99` |
| SEL-10 | Edge and vertex picks | Horizontal STEP edges, rims and the corners of open runs are items at their own Z, pickable in pick mode of the selected op within 15 px (12 px for a point). Not pickable outside pick mode. No sloped or vertical edge. | `C/io/brep_items.rs`; `V/interaction/picking.rs:16-20`, `:309-396` |
| SEL-11 | Selection synced with the model tree | The Geometry tab lists the items; a hovered row lights its item in the view (GT-2); `frame_items` frames and flashes picked items. A tree face selection shows in the BREP panel. | `V/ui/properties/geometry_tab.rs:50`; `V/app.rs:454`; `V/state/selection_highlight.rs` |
| SEL-12 | Undo of a selection | An op selection edit undoes through `ToolpathParamChange` (D11), but not one step per act: `apply_selection_edit` pushes no step, and the inspector snapshot flushes one step when the GUI selection leaves the toolpath, so N clicks undo as one (designs E §0.2, F-8). The tree `Selection::Face/Faces` is dead code (F-1). | `V/controller/events/mod.rs:109-143`; `V/ui/properties/panel_apply.rs:488-518`; `V/state/history.rs:20-33` |
| SEL-13 | Keyboard use | Escape ends the pick mode or the tab editor. Delete removes the selected toolpath or tool. 1-4 set view presets. No select-all, no cycle through candidates, no arrow nudge. | `V/app/input.rs:425-520` |



#### A.1b What the reference tools do (selection)

Source keys are in `SOURCES.md`. "UNCONFIRMED" means the research agent
found no page that states the claim. Evidence comes from public help
pages, not from product code.

| ID | Reference tools |
|---|---|
| SEL-1 | Onshape highlights a face, edge or sketch curve under the pointer and shows edge midpoints [onshape-sel]. Fusion highlights the canvas item that the pointer is over in its Select Other list [fusion-blog-other]. No page gives a hover colour or a delay. |
| SEL-2 | Every tool selects on a plain click. Onshape: a click toggles; a second click on another item adds [onshape-sel]. Blender and SketchUp: a click replaces the selection [blender-sel] [sketchup-sel]. |
| SEL-3 | Fusion: Ctrl (Cmd on macOS) adds or removes; Shift adds only [fusion-sel]. SketchUp: Ctrl adds, Shift toggles, Shift+Ctrl removes [sketchup-sel]. Blender: Shift toggles [blender-sel]. Onshape: no modifier; the click toggles [onshape-sel]. The modifier does not converge; the toggle on a repeated click does. |
| SEL-4 | The direction rule converges: a left-to-right drag selects what the box encloses; a right-to-left drag selects what the box touches (Fusion [fusion-sel], Onshape [onshape-sel], SketchUp [sketchup-sel], Plasticity [plasticity-sel]). Blender has no direction rule [blender-sel]. Onshape draws a solid blue box for enclose and a dotted yellow box for crossing [onshape-sel]. Lasso: Fusion "Freeform" (clockwise = window, counter-clockwise = crossing) [fusion-sel]; SketchUp lasso with the same rule [sketchup-sel]; Blender Ctrl+right drag [blender-sel]. |
| SEL-5 | Fusion: a "Select Through" filter lets a window or freeform selection reach hidden objects; off, only unobscured objects select [fusion-sel]. Blender: X-ray select [blender-mesh-sel]. SolidWorks: Select Other reaches hidden items [sw-sel-2013]. |
| SEL-6 | SolidWorks: Select Loop, Select Tangency (stops at a non-tangent face or edge), Select Chain [sw-tangent-2017] [goengineer]. Plasticity: Alt+click an edge = the edge loop; Ctrl+Alt+click = the edge ring [plasticity-sel]. SketchUp: double-click a face = face with its edges; triple-click = all connected [sketchup-sel]. Fusion and Onshape: UNCONFIRMED. |
| SEL-7 | Plasticity: Ctrl+Alt+click a face = all parallel faces; on a cylinder = all parallel cylindrical faces; Alt+click a curve = all curves on the same plane [plasticity-sel]. SketchUp: "All with same tag / material" [sketchup-sel]. Blender: Select Similar [blender-mesh-sel]. Fusion and Onshape: UNCONFIRMED. |
| SEL-8 | Fusion: a Selection Filters menu (bodies, body edges, body faces, body vertices, components, more) and a Selection Priority (one of body, face, edge, component at a time) [fusion-sel]. Plasticity: keys 1-4 = control point, edge, face, solid; Tab or 5 = all [plasticity-mode]. |
| SEL-9 | Fusion: hold the left button about one second for Select Other; the list runs front to back, the wheel scrolls it, Depth and Parent tabs pick the level [fusion-blog-other2]. SolidWorks: Select Other lists the hidden entities [sw-sel-2013]. Plasticity: Ctrl+Shift+click, or click plus wheel, opens a candidate menu [plasticity-sel]. Onshape: a forum names a cycle key (grave accent) [onshape-forum-other]; UNCONFIRMED in help. |
| SEL-10 | Fusion filters "Body Edges" and "Body Vertices" [fusion-sel]. Onshape: edges and their midpoints pick on hover [onshape-sel]. SolidWorks: a middle-click on a vertex, edge or face sets the orbit anchor [sw-mmb-2014]. |
| SEL-11 | Onshape: range select in the feature list with Shift+click [onshape-sel]. Fusion: the Select Other "Parents" tab walks the tree [fusion-blog-other]. Other tools: UNCONFIRMED. |
| SEL-12 | Plasticity: Ctrl+T re-selects the source objects of the last operation [plasticity-sel]. Blender, Fusion, Onshape: UNCONFIRMED. No tool documents an undo step for a plain click. |
| SEL-13 | Onshape: Space or a click on empty space clears; Escape exits a tool [onshape-sel]. Plasticity: Escape = deselect all, A = select all, Alt+A = invert [plasticity-sel]. SketchUp: Ctrl+A all, Ctrl+T none [sketchup-sel]. Blender: Ctrl+I invert [blender-sel]. Tab as a candidate cycle: no tool documents it. Onshape prints the selection count on the cursor, "5+" past five [onshape-sel]. |

### A.2 Camera

| ID | Requirement | Our state | Where |
|---|---|---|---|
| CAM-1 | Orbit about the point under the cursor | The orbit turns yaw and pitch about a fixed `target`. | `V/render/camera.rs:103-109` |
| CAM-2 | Zoom toward the cursor | The zoom scales `distance` only; the target stays, so the view zooms to the screen centre. | `V/render/camera.rs:134-137`; `V/app/viewport.rs:874-881` |
| CAM-3 | Pan that keeps the point under the cursor | The pan moves the target by `distance × 0.001` per pixel. The projection-derived rule is `2 × distance × tan(fov/2) / viewport_height`; the two agree only at a viewport height of 828 px (45° field of view), so at other heights the point under the cursor drifts (claim C8, `check_claims.mjs`). | `V/render/camera.rs:112-131` |
| CAM-4 | Eased presets and Fit | `fit_to_bounds` and `set_preset` write the pose at once; no interpolation. The MCP `set_ui_view` reply says the view renders on the next frame, and `screenshot_gui` settles two frames. | `V/render/camera.rs:140-148`, `:217-236`; `V/app/mcp/view.rs:313-319`, `:708` |
| CAM-5 | A view cube | Four preset buttons in the dock popover (Top, Front, Right, Iso, keys 1-4). The `orientation_gizmo` is a camera-axis triad painted by egui; it is not clickable. | `V/ui/viewport_overlay.rs:617-636`; `V/state/viewport.rs:108`; `V/ui/overlays/registry.rs:802-809` |
| CAM-6 | Fit to selection | `frame_items` fits picked items; "Reset view" fits the first model. No fit to the selected faces or the selected toolpath from a key. | `V/app.rs:440-502`; `V/app/input.rs:303` |
| CAM-7 | Perspective and orthographic | Both; the toggle keeps the visible half-height at the target. | `V/render/camera.rs:45-84` |
| CAM-8 | A named mouse and touchpad scheme | Left drag orbits, right or middle drag pans, wheel zooms, no modifier. Touchpad pinch (`zoom_delta`) is not read. The scheme matches no named CAD scheme (A.2b). | `V/app/viewport.rs:848-881` |
| CAM-9 | Camera control over MCP | `set_ui_view` has no preset, projection or pose field. An agent can frame items only. | `crates/rs_cam_mcp/src/server.rs:934-990` |



#### A.2b What the reference tools do (camera)

| ID | Reference tools |
|---|---|
| CAM-1 | FreeCAD "Rotation mode": Window center (default), Drag at cursor ("Rotations in 3D will use current cursor position as center for rotation"), Object center [freecad-prefs]; CAD style: a middle double-click sets the rotation centre [freecad-nav]. Blender: "Orbit Around Selection" and "Auto Depth" ("Use the depth under the mouse to improve view pan, rotate, zoom") [blender-nav]. Fusion: constrained orbit about the centre mark; "Orbit around point" with Shift+middle; a "Camera pivot" preference and a "Reset Orbit Center" menu item [fusion-nav] [fusion-blog-nav] [fusion-forum-pivot]. SolidWorks: a middle-click on a vertex, edge or face sets the anchor [sw-mmb-2014]. Plasticity: Alt+middle click centres on the cursor [plasticity-view]. SketchUp orbits about the drawing-area centre (the counter-example) [sketchup-view]. Onshape: UNCONFIRMED. |
| CAM-2 | SketchUp: the wheel zooms about the cursor [sketchup-view]. FreeCAD: "Zoom at cursor", default on [freecad-prefs]. Blender: "Zoom to Mouse Position" option [blender-nav]. Tinkercad: the wheel zooms where the cursor points (video) [tinkercad-video]. three.js OrbitControls: `zoomToCursor` exists, default off [threejs-orbit]. Fusion, Onshape, Shapr3D: UNCONFIRMED. |
| CAM-3 | Every tool pans with a drag that keeps the point under the pointer; see the schemes table. |
| CAM-4 | FreeCAD: "Animation duration" default 500 ms (100-10000) [freecad-prefs]. Blender: "Smooth View" time in ms, zero = no animation [blender-nav]. three.js: `enableDamping` default off, `dampingFactor` 0.05 [threejs-orbit]. Onshape: Alt animates to the nearest floor-down view [onshape-view]. Fusion, SketchUp: UNCONFIRMED. |
| CAM-5 | Fusion ViewCube: drag to orbit, right-click for a menu with Orthographic / Perspective / Perspective with Ortho Faces [fusion-viewcube]. Onshape view cube: a corner click = trimetric, a side click = a plane view; arrows turn 15° (90° with Shift, 5° with Ctrl); its menu has Zoom to fit [onshape-view]. SketchUp has no cube; it has standard views and Previous / Next [sketchup-view]. |
| CAM-6 | Onshape: F = zoom to fit, W = zoom to window, "Zoom to selection" when something is selected [onshape-zoom]. Fusion: F6 Fit frames the whole model; Look At turns the selection planar to the screen; a forum says Fit does not frame a selected sub-element [fusion-nav] [fusion-forum-fit]. Tinkercad: F = fit selection [tinkercad-keys]. Plasticity: / = focus on the selection [plasticity-focus]. Kiri:Moto: F = fit all, H = home [kiri-controls]. |
| CAM-7 | Onshape: perspective off by default, a toggle [onshape-view] [onshape-camera]. SketchUp: perspective default, "Parallel Projection", field of view in degrees or mm [sketchup-view]. Plasticity: Num 5 toggles [plasticity-view]. Blender: "Auto Perspective" switches to perspective on orbit [blender-nav]. |
| CAM-8 | See the schemes table below. Fusion ships presets named Fusion, SolidWorks, Inventor, Tinkercad, Alias, PowerMill [fusion-blog-nav]; FreeCAD ships Blender, CAD, Gesture, Maya-Gesture, OpenCascade, OpenInventor, OpenSCAD, Revit, TinkerCAD, Touchpad [freecad-nav]. Orbit style: FreeCAD default "Rounded Arcball", also Turntable [freecad-prefs]; Blender Turntable (horizon stays level) or Trackball [blender-nav]. |
| CAM-9 | Not a product feature; an rs_cam MCP need. |

**Mouse schemes** (one row per tool; from the help pages in `SOURCES.md`):

| Tool | Orbit | Pan | Zoom | Add to selection |
|---|---|---|---|---|
| Onshape (web) | right drag | middle drag, or Ctrl + right drag | wheel | click toggles |
| Tinkercad (web) | right drag (Ctrl + left) | middle drag (Shift + right) | wheel, + / − | — |
| Shapr3D (desktop/iPad) [shapr-presets] | right drag | Shift + right or middle (sources conflict) | wheel | — |
| Fusion (default preset) | Shift + middle drag | middle drag | wheel | Ctrl / Cmd; Shift adds only |
| SolidWorks | middle drag | Ctrl + middle drag | wheel; Shift + middle drag | — |
| FreeCAD CAD (default) | middle + left drag | middle drag | wheel | — |
| Blender | middle drag | Shift + middle drag | wheel | Shift toggles |
| Plasticity | middle drag | right drag | wheel; Ctrl + middle drag | — |
| SketchUp | O tool left drag; middle press from any tool | middle + left, or Shift in Orbit | wheel (to cursor) | Ctrl adds, Shift toggles |
| Kiri:Moto (web, three.js) | left drag | right drag | wheel, middle drag | — |
| three.js OrbitControls | left drag | right drag | wheel, middle drag | — |
| FreeCAD Gesture (touch) | left drag | right drag; two fingers | wheel; pinch | tap |
| FreeCAD Touchpad | Alt + move, or Shift + left | Shift + move | Ctrl + Shift + move | — |
| rs_cam today | **left drag** | right or middle drag | wheel (to centre) | Shift (pick mode: layer rule) |

Two families hold the desktop and the web tools: the middle-button family
(SolidWorks, Fusion, FreeCAD CAD, Blender, Plasticity, SketchUp: 6) and the
right-button family (Onshape, Tinkercad, Shapr3D: 3). Only browser viewers
and touch styles orbit on the left button (Kiri:Moto, three.js, FreeCAD
Gesture), which is the rs_cam scheme today. The left button is the select
button in every CAD tool, and a box select (SEL-4) needs a left drag, so
the orbit must leave the left button. The scheme choice is in section D.

### A.3 Rendering for readability

| ID | Requirement | Our state | Where |
|---|---|---|---|
| REN-1 | Face edges on a part | No edge pass. The data exists: each STEP face holds ordered `boundary_loops` from the boundary walk, and `brep_items.rs` already reads them. `EnrichedMesh.edges` also exists, but each `BrepEdge.vertices` is an unordered set of shared tessellation vertices, so it cannot draw as a polyline. The true edge curves (`CompressedEdge.curve`) are dropped at import. The line pipeline is `LineList`, one pixel wide. | `C/geometry/enriched_mesh.rs:69-100`; `C/io/step_input.rs:88-95`, `:136`, `:468-484`; `V/render/mod.rs:91`, `:646` |
| REN-2 | Silhouettes and a selection outline | None. Faces fill with `FACE_SELECTED`; items draw a hover ring. | `V/render/colors.rs`; `V/render/mesh_render.rs:267-268` |
| REN-3 | Smooth shading with sharp feature edges | A STEP part is flat-shaded per triangle (each triangle carries its face normal), so a curved face reads faceted. An STL draws with smooth area-weighted vertex normals. | `V/render/mesh_render.rs:37-50`, `:241-250` |
| REN-4 | MSAA or FXAA | The offscreen colour and depth textures have `sample_count: 1`; the five offscreen pipelines use `MultisampleState::default()`. The blit pipeline at `:714` draws into egui's own pass, which is single-sample too: `main.rs` and `app.rs` set no `multisampling` option, so eframe's default holds. | `V/render/mod.rs:811`, `:822`, `:358`, `:471`, `:506`, `:541`, `:650`; `:713-714` (blit) |
| REN-5 | Transparency and x-ray | Opacity exists for the simulated stock, the height planes and the heatmaps (depth-read-only alpha pipelines). No x-ray of the model. | `V/render/mod.rs:34-62`, `:476-506` |
| REN-6 | Section planes | None. | — |
| REN-7 | Selection outline (not fill) | None; see REN-2. | — |



#### A.3b What the reference tools do (rendering)

| ID | Reference tools |
|---|---|
| REN-1 | Onshape: "Shaded with edges" is the default shading mode; hidden edges removed by default; tangent edges solid by default (phantom or removed as options) [onshape-shading] [onshape-hidden] [onshape-tangent]. Onshape's performance page tells large-assembly users to switch edges off because edges add render time [onshape-perf]. Fusion visual styles: Shaded, Shaded with hidden edges, Shaded with visible edges only [fusion-visual]. |
| REN-2 / REN-7 | Box-select feedback: Onshape solid blue (enclose) or dotted yellow (crossing) outline with a fill [onshape-sel]; Plasticity purple (enclose) or green (crossing) [plasticity-sel]; Fusion tints the box translucent [fusion-sel]. Whether a selected face draws an outline or a fill: UNCONFIRMED on every page read. |
| REN-3 | No page states the shading model. UNCONFIRMED. |
| REN-4 | Onshape has no anti-aliasing setting in the graphics view (a pixel filter in Render Studio only) [onshape-aa]. Fusion anti-aliasing preference: UNCONFIRMED. FreeCAD's MSAA options are on a page the agent could not read: UNCONFIRMED. |
| REN-5 | Onshape "Translucent" shading mode [onshape-shading]. Blender X-ray [blender-mesh-sel]. |
| REN-6 | UNCONFIRMED on every page read. |

### A.4 Feedback

| ID | Requirement | Our state | Where |
|---|---|---|---|
| FB-1 | Cursor changes | Grab over a tab mark, Grabbing during a tab drag. No cursor change over a pickable face or item, none during orbit or pan. | `V/app/viewport.rs:297`, `:838` |
| FB-2 | Tooltips on hover | In pick mode: the item tooltip (kind, state, action). Outside: the toolpath span tooltip. No face tooltip (surface type, size, Z). | `V/app/viewport.rs:684-704`, `:773-815`; `V/ui/pick_bar.rs` |
| FB-3 | Frame rate and pick latency targets | No frame-time instrument. `mcp_bridge.last_frame_ms` is a liveness stamp (`frame_age`), not a frame time. The pick cost is measured in B.1. | `V/mcp_bridge.rs:120`, `:278-288` |



#### A.4b What the reference tools do (feedback) and the target numbers

| ID | Reference tools |
|---|---|
| FB-1 | Onshape prints the selection count on the cursor, exact to 5, then "5+" [onshape-sel]. Fusion: UNCONFIRMED. |
| FB-2 | Face type and size tooltips: UNCONFIRMED on every page read. Onshape shows edge midpoints on hover [onshape-sel]. |
| FB-3 | No CAD vendor publishes a frame-rate target; Onshape says only that edges add render time [onshape-perf]. The targets below come from two general interaction sources. |

**Targets (cited, not from memory):**

| Target | Value | Source |
|---|---|---|
| A response to an input feels instantaneous | within 100 ms; process the input within 50 ms | Google RAIL model [rail] |
| An animation frame | 10 ms of work per frame, in a 16 ms budget at 60 Hz | Google RAIL model [rail] |
| The limit for "the system reacts instantaneously" | 0.1 s | Nielsen, response-time limits [nielsen] |
| The limit for an unbroken flow of thought | 1.0 s | Nielsen [nielsen] |
| A view animation | FreeCAD default 500 ms; Blender "Smooth View" in ms; three.js damping factor 0.05 | [freecad-prefs] [blender-nav] [threejs-orbit] |

Applied to rs_cam: a hover highlight and a pick must land within one frame
(under 16 ms of CPU and GPU work together, 10 ms as the aim); a camera
step must land within one frame; a preset animation of 250-500 ms is in
the range the reference tools use; a box select that resolves in under
100 ms feels instant.

## B. Gap matrix

### B.1 Measurements

**Pick-latency probe (2026-10-28).** A scratch crate
(`pick_probe`, outside the repo, own target directory, profile
opt-level 3 without LTO; the product release profile uses fat LTO, so
absolute times can differ a little, the ratios hold) timed the product's
`rs_cam_core::mesh::ray_pick_triangle` against a `parry3d-f64 0.24.0`
`TriMesh` ray cast (its BVH). 400 rays per mesh, from one eye above and
beside the part, aimed at a grid over the bounding box, as a hover sweep
sees them. Machine: 24 logical CPUs, one thread.

| Mesh | Triangles | Brute force mean / median / max (ms) | BVH build (ms) | BVH cast mean / max (ms) | Same nearest hit |
|---|---|---|---|---|---|
| `planning/fixtures/rivmap100/rivmap_export/terrain.stl` | 131 962 | 1.806 / 1.793 / 2.064 | 62.4 | 0.0026 / 0.0097 | 400 / 400 |
| synthetic hemisphere, 70 × 70 quads | 9 800 | 0.045 / 0.044 / 0.055 | 2.1 | 0.0008 / 0.0041 | 397 / 400 |
| synthetic hemisphere, 200 × 200 quads | 80 000 | 0.366 / 0.350 / 0.989 | 24.2 | 0.0016 / 0.0098 | 400 / 400 |
| synthetic hemisphere, 500 × 500 quads | 500 000 | 2.388 / 2.403 / 4.602 | 174.3 | 0.0022 / 0.0088 | 400 / 400 |
| synthetic hemisphere, 800 × 800 quads | 1 280 000 | 5.317 / 5.252 / 6.866 | 450.6 | 0.0026 / 0.0102 | 399 / 400 |

The table is the second run (`probe/run.log`); the first run gave 2.200 ms
mean on the terrain and 4.855 ms at 1.28 M triangles, so a run-to-run
spread of about 20 % is normal on this machine.

Reading: the brute-force pick is 2-5 ms per ray on a large mesh, and the
hover path runs it twice per frame (SEL-2), so a hover over the terrain
spends about 3.6 ms of a 16.7 ms frame on the CPU before any draw. On a
9 800-triangle part it is 0.045 ms and is not felt; the repo's STEP
fixtures are smaller still. A BVH cast is about 1000 × faster on the large
meshes and its build is a one-time cost per mesh (62 ms for the terrain).

The four disagreements (printed in `probe/run.log` as `DISAGREE`) are all
rays that pass exactly through an edge shared by two triangles, a case the
regular synthetic grid produces: the product's `Triangle::ray_intersect`
hits both triangles within 1e-9 of one `t` (printed as ties; design C notes
they are not bit-equal, so the lowest-index rule did not decide them), and parry's test misses
the edge-exact hit and reports a miss, or on the 1.28 M mesh the far side
of the hemisphere. A viewport pick on a real part meets this case only by
chance, but it is a reason to build the BVH around the product's own
triangle test (D.1 step 2) rather than to take a second intersection rule.
No large STEP fixture exists in the repo, so no STEP number is given.

**Hover rebuild size (arithmetic, not measured).** A STEP part uploads 3
unindexed `ColoredMeshVertex` of 36 bytes per triangle
(`V/render/mesh_render.rs:276-321`; position 12 + normal 12 + colour 12, plus
three 4-byte indices per triangle; corrected 2026-10-28 from 108 B, design D).
Each change of the hovered face rebuilds and re-uploads
`120 × triangles` bytes (108 of vertices, 12 of indices): 1.2 MB at 10 k
triangles, 15.8 MB at the terrain's 132 k. The rebuild also walks every triangle on the CPU. This is the cost
that a user feels as a hover stutter on a large part; the ray pick is
second.

### B.2 Matrix

Cost: S = under a day for one agent, one file area; M = a few days,
several files or a new buffer or pipeline; L = a week or more, or a new
subsystem. "Value / cost" marks what the operator gets the most from per
unit of cost: ★★★ first, ★ last.

| ID | Gap | Cost | Why that cost | Value / cost |
|---|---|---|---|---|
| SEL-1 | Face hover rebuilds the part buffer. Fix: give each vertex a `face_id` attribute (u32) and keep one small per-face state buffer (hovered, selected, state colour index) that the fragment shader reads; a hover change writes a few bytes with `queue.write_buffer` and the mesh key no longer holds the hover. Add the STL flat-face hover the same way (a region id per triangle). | M | One vertex layout, one WGSL change, one storage or uniform buffer, the `upload_cache` slot inputs lose `hovered_face` and `selected_faces` | ★★★ |
| SEL-2 | Brute-force ray. Fix: a BVH per mesh model, built once per model revision and held beside the mesh (`parry3d-f64` `TriMesh`, or the `bvh` crate). Hover runs the pick once per frame, not twice, and the result feeds both the face hover and the tooltip. | S | A 200-line own BVH in core, or a dependency (C.2: current parry3d is glam-based; `bvh` 0.11 is the last nalgebra-0.33 release); one call site in `pick_model_face`; a cache keyed by model revision. | ★★ (★★★ on terrains) |
| SEL-3 | Ctrl does nothing; the modifiers match no named scheme. Fix: name the scheme (A.2b) and map Shift = add, Ctrl = toggle (or the scheme's rule). | S | `pick_bar::click_event` and the face arm of `handle_viewport_click` | ★★ |
| SEL-4 | No box or lasso select. Fix: a primary drag with a modifier (or in pick mode) draws a rectangle in egui; on release, CPU test of every admitted item's projected lines against the rectangle (window = all points inside, crossing = any segment crosses) for drawings, and of each face's projected `boundary_loops` for STEP faces. MCP: `edit_toolpath_selection` with `SelectionEdit::Replace`, or a new batch `AddPicks(Vec<MatchKey>)` so one undo step holds the whole box. | M | A drag state, a rectangle painter, a new `pick_items_in_rect`, a batch edit in core, a GUI test and an MCP parity test | ★★★ |
| SEL-5 / SEL-9 | Nearest hit only; no pick list. Fix: collect every hit along the ray (BVH `intersections_with_ray`) and offer a small popover on a repeated click or a key ("select other"). | M | Depends on SEL-2; a popover and a cycle key | ★ |
| SEL-6 | No tangent chain across DXF items. The import join already chains touching pieces; a tangent-continuous chain across separate items is a core rule. | M | A `Rule::TangentChain` in core and its resolver; out of this scope (cite GAP_ANALYSIS row 10) | ★ |
| SEL-7 | No "select similar" from the view. Fix: a right-click on a picked item in pick mode opens the existing `rule_offers` (same diameter, same Z, up faces at Z; two new face offers) and an MCP `list_rule_offers` query gives the labels and counts. No new rule kind and no project-file key (design E §2). | S | One context menu, one query row | ★★ |
| SEL-8 | No sloped or vertical edge. Those edges give no 2.5D item by design (UX-7). A view-only edge pick (for Chamfer of one edge, UX_REVIEW 2.F4) needs an edge item kind that the 2.5D ops do not cut. | L | A new item kind and the op capability table; a product decision, not a view fix | ★ |
| SEL-10 | Edge and vertex picks only in pick mode. Fix: run `pick_items` for the selected op outside pick mode too (hover ring and tooltip), with the click still gated by pick mode. | S | One condition in `viewport.rs` | ★★ |
| SEL-11 | Tree sync mostly exists. Fix: a click on a Geometry tab row frames the item (reuse `frame_items`); the viewport flash on a tree face click. | S | Two calls | ★★ |
| SEL-12 | A viewport pick is not its own undo step (E §0.2). Fix: `push_act_step` in the controller, so every selection act (click, box, rule, Ctrl+A) is one `ToolpathParamChange`; delete the dead tree variants `Selection::Face/Faces` (F-1). | S | `controller/events/mod.rs`, `state/selection.rs` and six match arms | ★★ |
| SEL-13 | No select-all, no cycle. Fix: Ctrl+A in pick mode = `AddRule` of every admitted kind; Tab cycles the pick list (SEL-9). | S | Two key arms | ★ |
| CAM-1 | Orbit about a fixed target. Fix: on the orbit drag start, pick under the cursor (SEL-2 BVH; fall back to the current target on a miss), set `target` to the hit and move `eye` so the view does not jump (the pose is the same; only the pivot moves). Needs an eye-and-target camera, or a `pivot` beside `target`. | S–M | `OrbitCamera` gains a pivot; `orbit` rotates eye about the pivot; the drag start in `viewport.rs` | ★★★ |
| CAM-2 | Zoom to the screen centre. Fix: unproject the cursor ray, move the target toward the point under the cursor by the zoom ratio (perspective); in orthographic scale the half-height and shift the target so the cursor point stays fixed. No pick needed (ray against the target plane). | S | `OrbitCamera::zoom` takes the cursor ray | ★★★ |
| CAM-3 | Pan drifts. Fix: pan by the world distance of one pixel at the target depth, from the projection (`2 × distance × tan(fov/2) / viewport_h` in perspective, `2 × half_h / viewport_h` in orthographic). | S | One formula | ★★ |
| CAM-4 | Presets and Fit jump. Fix: a `CameraTween { from, to, started, duration }` (about 250 ms, A.2b gives the reference); each frame sets the pose by an ease curve and calls `request_repaint` only while it runs (G-LV.1 rule). MCP: `set_ui_view` sets the pose at once (no tween) so `screenshot_gui`'s two-frame settle stays true, or the tween ends in one frame when a screenshot is pending. | S–M | A tween type, two call sites, the MCP settle rule and its test | ★★ |
| CAM-5 | No view cube. Fix: an egui-painted cube in the corner (faces, edges, corners as hit regions) that sends `SetViewPreset` and the new edge and corner presets; drag on it orbits. | M | A widget, 26 presets, tests | ★ |
| CAM-6 | No fit to selection. Fix: key F = fit the selected faces, items or toolpath (reuse `frame_items` and `fit_to_bounds`). | S | One key arm, one bounds union | ★★ |
| CAM-8 | No named scheme, no pinch. Fix: adopt one scheme (A.2b), read `zoom_delta` for pinch, keep a preference for the alternate scheme. | S | `viewport.rs` input block, a preference row | ★★ |
| CAM-9 | No camera control over MCP. Fix: `set_ui_view` gains `view_preset`, `projection`, `fit: "model" / "selection"`, and an optional `camera: {yaw, pitch, distance, target}` pose; the reply echoes the pose. Parity test: a preset set by key and by MCP give the same `view_proj`. | S | A wire struct, one arm, one parity test | ★★★ (every agent screenshot gains it) |
| REN-1 | No face edges. Fix: upload every face `boundary_loops` as a `LineList` in a near-black ink colour, drawn after the faces with a small depth bias (or `depth_compare: LessEqual` and a polygon offset in the vertex shader). Later: keep `cshell.edges` at import and polyline the curves (`PolylineableCurve`) for exact edges. | S (M for exact curves) | One upload slot, the existing line pipeline, a colour token | ★★★ |
| REN-2 / REN-7 | No silhouette, no selection outline. Fix: a screen-space post pass over the offscreen depth and an ID texture (from SEL-1's `face_id`): a pixel whose neighbour has another id or a depth step draws ink; a pixel whose neighbour is a selected id draws the selection outline. Done in the blit shader, so no new pass. | M | An R32Uint attachment on the enriched pipeline (MRT), a blit shader change, MSAA interaction (resolve the id texture by sample 0) | ★★ |
| REN-3 | Faceted curved faces. Fix: index the STEP upload per face group with smooth normals inside the group and a hard edge between groups (vertices are already per face in `FaceTessellation`). | S–M | `enriched_mesh_gpu_data` builds per-face vertex normals; one third of the VRAM | ★★ |
| REN-4 | No MSAA. Fix: a 4-sample colour and depth texture, the five offscreen pipelines `MultisampleState { count: 4 }`, the single-sample offscreen as `resolve_target`; the blit pipeline stays single-sample because it draws into egui's pass. Query `TextureFormat` sample support; fall back to 1. | S | Five `MultisampleState` sites and `ensure_offscreen` | ★★★ |
| REN-5 | No model x-ray. Fix: an opacity on the model slot (the sim stock already has one) and a draw-order rule. | S | A registry row, a uniform | ★ |
| REN-6 | No section plane. Fix: a clip plane uniform, `discard` in the fragment shader, a registry row and a drag handle. | M | A shader branch in every mesh pipeline, a UI handle | ★ |
| FB-1 | No cursor feedback. Fix: `CursorIcon::PointingHand` over a pickable face or item in pick mode, `Grabbing` during orbit, `Move` during pan, `ZoomIn` on wheel. | S | Four `set_cursor_icon` calls | ★★ |
| FB-2 | No face tooltip. Fix: on a face hover (outside pick mode) a one-line tooltip: surface type, size, Z in the setup frame; detail stays on hover (no clutter rule). | S | One tooltip from `FaceGroup` data | ★★ |
| FB-3 | No frame-time instrument. Fix: a rolling frame time and pick time in `GuiState`, shown in the status bar on hover and returned by MCP `gui_status` or `get_diagnostics`, so a regression is measurable. Targets in A.4b. | S | One timer, one row | ★★ |


## C. Frameworks and example code

### C.1 Engines: a full engine is too much

Our stack (from `Cargo.lock`): eframe and egui 0.36.2, egui-wgpu 0.36.2,
wgpu 30.0.1, winit 0.30, nalgebra 0.33.3. The viewport is our own wgpu
renderer inside an `egui_wgpu::CallbackTrait` paint callback
(`V/render/mod.rs:944`, `:1421`; `V/app/viewport.rs:1050`), drawn to an
offscreen colour and depth texture and blitted into the egui pass. That is
the pattern egui's own `custom3d_wgpu` demo documents [egui-custom3d].

| Engine | Licence | Latest release | Open issues | wgpu | egui integration | Picking | Camera | Outlines | What we would replace | Migration cost |
|---|---|---|---|---|---|---|---|---|---|---|
| Bevy 0.20.0 | MIT or Apache-2.0 | 2026-10-08 [bevy-020] | 2 905 issues, active daily | `wgpu = "30"` in bevy_render 0.20 [bevy-render-toml]; matches ours today | bevy_egui 0.43.1 (MIT, 2026-10-10) pins `egui = "0.36"` and `wgpu-types = "30.0.1"` [bevy-egui-toml]; it hosts egui INSIDE Bevy. No published example of Bevy drawn inside an eframe paint callback (UNCONFIRMED). | `bevy_picking` in core since 0.15; the mesh backend is "a naive raycast against the full mesh", opt-in `MeshPickingPlugin` [bevy-015] [bevy-picking] | 0.20 upstreamed `bevy_editor_cam` as a CAD-style `PanOrbitCamera` [bevy-020]; `bevy_panorbit_camera` 0.36.0 adds smoothing, touch, orthographic, no zoom-to-cursor field documented [bevy-panorbit] | `bevy_mod_outline` 0.13.0 (vertex extrusion and jump flood) supports Bevy 0.19 only; no 0.20 release 20 days after Bevy 0.20 [bevy-outline] | The renderer, the pipelines, the camera, the upload cache; and the eframe host, window, event loop and screenshot path, because the realistic embed is Bevy as host | L |
| Fyrox 1.0.1 | MIT | 2026-03-28 [fyrox] | 59 | None shipped: `fyrox-graphics-gl` (OpenGL) [fyrox-toml] | Its own retained `fyrox-ui`; no egui | Physics ray cast on colliders [fyrox-ray] | UNCONFIRMED | UNCONFIRMED | Everything: an OpenGL host with its own UI and loop | L (not viable) |
| three-d 0.19.0 | MIT | 2026-04-17 [three-d] | 23 | None: `glow 0.17` (OpenGL) [three-d-toml] | `egui-gui` feature pins egui 0.34 inside three-d; it cannot draw into an egui-wgpu pass | `renderer::pick` casts a pixel ray against geometries [three-d-pick] | Orbit, FreeOrbit, Fly controls; no zoom-to-cursor documented [three-d-control] | UNCONFIRMED | The graphics API itself | L (not viable) |
| rend3 0.3.0 | MIT or Apache-2.0 or Zlib | 2022-02-12; repository archived 2025-06-07 [rend3] | 38, no commits in 2026 | 2022-era | 2022-era `rend3-egui` | — | — | — | — | Not viable |
| kiss3d 0.47.0 | BSD-3-Clause | 2026-10-04 (now at dimforge) [kiss3d] | 84 | `wgpu = "30"`, `winit = "0.30"`: an exact match [kiss3d-toml] | Optional feature hosts egui 0.36 inside kiss3d; `#[kiss3d::main]` owns the entry point | Not documented | `OrbitCamera3d` arc-ball | Not documented | Our renderer and camera; our math (kiss3d uses glam) | M-L |

**Verdict.** A full engine is too much for a CAD/CAM viewport inside egui.
The facts:

- **Version lockstep.** Bevy is the one engine that matches wgpu 30 and
  egui 0.36 today, and that match is a three-way lock: we upgrade egui only
  when bevy_egui follows and wgpu only when Bevy follows. egui and
  egui-wgpu 0.37.0 are published and bevy_egui still pins 0.36
  [bevy-egui-toml]. Two wgpu versions can build in one binary, but Cargo
  treats their types as different [cargo-resolver], so a split means two
  devices and no shared texture.
- **The frame loop and the ECS.** Bevy owns the `App` loop. The embedding
  parts exist (`RenderCreation::Manual` takes a device we made;
  `RenderTarget::TextureView` takes a view "created outside of Bevy";
  `ScheduleRunnerPlugin` runs without winit) [bevy-rendercreation]
  [bevy-rendertarget] [bevy-schedulerunner], but no published example draws
  Bevy inside an eframe paint callback. The realistic path is Bevy as the
  host with egui inside it, which replaces the eframe host, the window and
  the event loop, not only the renderer. The mutation contract
  (`ProjectSession::apply(Command)`) and the worker lanes would have to
  cross an ECS boundary they do not need.
- **Build time and binary size.** Bevy's own setup page says the compile
  times "are rather long", that debug builds "take multiple minutes to
  load large 3D models", and that shipping with dynamic linking "is not
  recommended" [bevy-setup]. The only binary-size figure found is a Reddit
  anecdote (50 MB release) and is not a published number [bevy-size].
- **Screenshots.** `screenshot_gui` uses egui's `ViewportCommand::Screenshot`
  of the whole window (`V/app/mcp/view.rs:276-440`); `screenshot_toolpath`
  is a CPU rasteriser in core (`V/app/mcp/view.rs:163`,
  `export::fingerprint::render_toolpath_composite_subject`) and does not
  touch the renderer at all. With eframe as host both keep working. With
  Bevy as host the MCP server would move to Bevy's `Screenshot` observer
  [bevy-screenshot].
- **Picking parity.** bevy_picking's mesh backend is the same brute-force
  raycast we have (B.1 shows the cost); an engine does not buy the BVH.
- **What engines teach, as ideas, not crates:** a CAD-style pan-orbit
  controller with a pivot under the pointer (`bevy_editor_cam`, now
  first-party), a jump-flood outline (`bevy_mod_outline`), and a picking
  event model with hover, click and drag (bevy_picking).

The other engines fail earlier: three-d and Fyrox are OpenGL, rend3 is
archived, and kiss3d owns the entry point and documents no picking.

### C.2 Building blocks that keep our renderer

Licence rule applied: no GPL, LGPL or AGPL code is copied. FreeCAD (LGPL),
Blender (GPL), Chili3D (AGPL) and IfcOpenShell (LGPL) appear as sources of
ideas only, and the report says so at each use. MIT, Apache-2.0, BSD and
0BSD code can be a source, with a `CREDITS.md` entry.

**Crates**

| Crate | Licence | Latest | Maintenance | Math types | What it gives | Fit |
|---|---|---|---|---|---|---|
| parry3d-f64 | Apache-2.0 | 0.31.1, 2026-09-18 [parry] | active (push 2026-10-06) | **glam** since 0.26 (via `glamx`); nalgebra interop is a feature-gated conversion. The probe used the older nalgebra line, 0.24.0, which pulls nalgebra 0.34 beside our 0.33 [probe]. | `TriMesh` with a built-in BVH; a ray cast returns the hit triangle; `Bvh::refit_partial` for an edited mesh | Medium: a mature BVH, but it adds glam, glamx and simba to the tree and a conversion at every call. Fornjot pins parry3d-f64 0.28 next to nalgebra 0.35, so the mix works [fornjot]. |
| bvh | MIT | 0.13.0, 2026-10-12 [bvh] | active | nalgebra 0.35 (0.13); 0.12 = 0.34; only 0.11 (2025-02) = our 0.33 | A BVH over anything `Bounded`; the caller does the ray-triangle test | Medium-low: the version that fits our nalgebra is 20 months old; the structure is small enough to write in core with a credit. |
| obvhs | MIT or Apache-2.0 | 0.4.0, 2026-10-05 [obvhs] | active, 0 issues | glam | Fast BVH2 and CWBVH builders for ray-tracing throughput | Low: glam-only and more than one pick ray per frame needs. |
| kiddo (in our tree) | MIT or Apache-2.0 | 6.3.0 [kiddo] | active | own arrays | k-d tree for point queries | Low for picking; it serves vertex and point snapping. |
| rstar | MIT or Apache-2.0 | 0.13.0, 2026-05-24 [rstar] | active | none at runtime | R*-tree over AABBs, envelope queries, no ray query | Medium for a box select over face bounds; low for picking. |
| bevy_panorbit_camera | MIT or Apache-2.0 | 0.36.0, 2026-10-20 [bevy-panorbit] | active | Bevy, glam | Target-state plus per-channel smoothness easing (orbit 0.1, pan 0.02, zoom 0.1); no zoom-to-cursor or orbit-about-cursor field in `src/lib.rs` | Reference for the easing model only. |
| dolly | MIT or Apache-2.0 | 0.6.0, 2024-07-22 (idle) [dolly] | idle 27 months | glam (old) | A rig of composable drivers (`YawPitch`, `Arm`, `Smooth`) | Design reference only. |
| three-d `OrbitControl` | MIT | 0.19.0 [three-d] | active | own | Orbit and zoom about the target; exponential zoom step `distance × (1 − exp(−k·delta))` | Reference for the zoom step; the same shape as our `zoom` today. |
| transform-gizmo-egui | MIT or Apache-2.0 | 0.11.0, 2026-08-17 [gizmo] | active | glam (mint) | Move, rotate, scale handles drawn with the egui painter; CPU ray-vs-handle hit tests | High as a reference for egui-space hit tests over a 3D view (the view cube, step 12); direct use if a gizmo is ever wanted. |
| bevy_mod_outline | MIT or Apache-2.0 | 0.13.0 (Bevy 0.19) [bevy-outline] | active | Bevy | Vertex extrusion (inverted hull) and jump-flood outlines | Reference for both outline methods. |
| rerun `re_renderer` | MIT or Apache-2.0 | tracks rerun main [rerun-pick] [rerun-outline] [rerun-readback] | active (push 2026-10-27) | glam | **The one wgpu codebase with GPU ID picking and jump-flood outlines in production**: an `Rgba32Uint` picking target with a cropped projection, a readback belt (`copy_texture_to_buffer` with 256-byte row padding, `map_async`, results by identifier a frame later), `Depth32Float` readback, `jumpflooding_*.wgsl` with an MSAA-aware init pass; examples `picking.rs`, `outlines.rs` | High as the source for steps 8 and 10: same API, permissive licence, production code. Too large to depend on; read and credit. |
| WebGPU Fundamentals "GPU picking" | BSD-style [webgpu-pick] | — | active | JavaScript | An `r32uint` target, ids from 1, a 1×1 copy into a 4-byte map-read buffer, click-cycling through stacked objects | Medium: the WebGPU API maps one-to-one onto wgpu. |
| wgpu examples | MIT or Apache-2.0 [wgpu-examples] | trunk | active | — | `msaa_line` (a multisampled attachment with the swapchain view as `resolve_target`, `LineList`), `multiple_render_targets`, `render_to_texture` (offscreen target and readback), `shadow`, `stencil_triangles` | High: the direct patterns for steps 5 and 10. |
| egui `custom3d_wgpu` demo | MIT or Apache-2.0 [egui-custom3d] | egui main | active | — | The paint-callback pattern we use; a drag turns into an angle; no camera, no picking | Nothing new to take. |

No egui-specific orbit camera crate exists on crates.io (search 2026-10-28:
"egui camera", "egui orbit", "orbit camera"); the hits are Bevy plugins, an
idle rig, a third-person follow camera and one GPL crate (ideas only).

**Open-source viewers**

| Viewer | Licence | Orbit pivot | Zoom to cursor | Eased camera | Picking | Hover highlight | Edges | MSAA | Box select |
|---|---|---|---|---|---|---|---|---|---|
| Online3DViewer [o3dv] | MIT (JavaScript, three.js) | about the target | no (eye moves along the eye-centre line) | fit-to-sphere tweens eye, centre and up; drags are not damped | CPU `Raycaster`; `Line.threshold` 10 px for edge lines | a highlight material swapped onto the mesh | `EdgesGeometry` with a user threshold angle, drawn as `LineSegments` | WebGL `antialias: true` | none found |
| three.js addons [threejs-orbitcontrols] [threejs-edges] [threejs-outline] [threejs-selbox] [threejs-gpupick] | MIT | `OrbitControls` about the target | **yes** (`zoomToCursor`): perspective dollies along the cursor ray by the radius change, then re-places the target on the view axis; orthographic unprojects the cursor before and after and shifts the camera by the difference | `enableDamping` with `dampingFactor` 0.05 | CPU `Raycaster`; the GPU example writes an `int id` attribute to a 1×1 target with `setViewOffset` and reads one pixel | emissive change in the example; `OutlinePass` = mask and depth targets, downsample, edge detect, blur, visible and hidden edge colours | `EdgesGeometry(threshold 1°)`: keep an edge whose face normals differ by more than the threshold, or that has one face | renderer flag; render targets take `samples` | `SelectionBox`: a frustum from the two corners, bounding-sphere centres inside; `SelectionHelper` draws the rectangle as a CSS box |
| Chili3D [chili3d] | **AGPL-3.0, ideas only**; TypeScript | UNCONFIRMED (file listing blocked) | UNCONFIRMED | UNCONFIRMED | UNCONFIRMED | — | — | — | — |
| CQ-editor [cq-editor] | Apache-2.0 (Python, OCCT) | OCCT rotation centre, not the cursor | yes: `StartZoomAtPoint` / `ZoomAtPoint` on a middle drag | no | OCCT selector: `MoveTo` on move, `Select` on release | OCCT dynamic highlight | OCCT wireframe mode (B-rep edges) | an OCCT flag that is inert in raster mode (UNCONFIRMED) | none (drag select prevented) |
| truck [truck] | Apache-2.0 (Rust, wgpu) | no controller in the library; `Camera::ray(uv)` for the app | app-defined | no | CPU: `Camera::ray`; the intersection is the caller's | none | `WireFrameInstance`: B-rep edges as a separate line pass | yes: `Scene.sample_count` with a multisampled buffer | none |
| Fornjot [fornjot] | 0BSD (Rust, wgpu, nalgebra 0.35, parry3d-f64 0.28) | **yes, about the cursor**: on press a ray through the cursor against every triangle (linear scan, no BVH) sets a `FocusPoint`; fallback the AABB centre; rotation about that point with the camera right and up axes; pan uses it for depth | partial: translates along the camera Z by `delta × distance_to_focus`; the cursor point is not held | no | CPU per-triangle ray | none | `PolygonMode::Line` wireframe and a `LineList` pipeline | yes: `SAMPLE_COUNT`, a multisampled colour texture, `resolve_target` | none |
| Mayo [mayo] | BSD-2-Clause (C++, Qt, OCCT) | OCCT rotation centre | yes: `AIS_ViewController::handleZoom` at the cursor, picks a 3D point under a perspective camera | **yes**: `NCollection_Lerp<Graphic3d_Camera>` under a Qt animation with a set duration | OCCT selector | OCCT dynamic highlight | OCCT display modes | `NbMsaaSamples` set after the context exists | a rubber band exists (wiring UNCONFIRMED) |
| FreeCAD [freecad-navstyle] | **LGPL-2.1, ideas only** (C++, Coin3D) | `RotationCenterMode`: window centre, scene point at the cursor (a ray pick, falls back to the focal plane), focal point at the cursor, bounding-box centre; an indicator draws while rotating | yes ("Zoom at cursor", step 1 = factor 7.5) | yes: a fixed-duration `NavigationAnimation` | Coin3D ray pick | — | — | — | — |
| Blender [blender-navigate] | **GPL-3.0, ideas only** (C++) | `USER_ORBIT_SELECTION`, `USER_DEPTH_NAVIGATE` (Auto Depth): reads the depth under the cursor once per event, keeps it as a dynamic offset, and an orthographic correction realigns the view depth to the pivot so later zooms behave | yes (`USER_ZOOM_TO_MOUSEPOS`) | yes ("Smooth View") | depth-buffer read for navigation; GPU select for selection | — | — | — | — |
| Kiri:Moto [kiri-orbit] | MIT (JavaScript, three.js) | an old `OrbitControls` adapted: about the target | no | 500 ms `tween.js` for programmatic view changes; no drag damping | CPU `Raycaster`; `Line.threshold` scaled by distance | — | — | WebGL antialias when `devicePixelRatio ≤ 1` | none found |

No viewer in the list does GPU ID picking; three.js has it as an example,
rerun has it in production. Fornjot is the one Rust viewer that orbits about
the point under the cursor, with the brute-force ray we have. Mayo is the one
with an eased camera between two poses. Online3DViewer and three.js give the
CPU face-edge rule (`EdgesGeometry`), which we do not need because the STEP
door already holds ordered face loops.

**Techniques, in words (sources above):**

- *ID-buffer picking in wgpu.* A second draw of the scene, or a second
  colour attachment of the first draw (MRT), writes an integer id per pixel
  (`R32Uint` for one id, `Rgba32Uint` for two). Zero means no hit. Render
  only the pixels near the cursor (a cropped projection, or a 1×1 target).
  `copy_texture_to_buffer` into a `MAP_READ` buffer whose row is padded to
  256 bytes, `map_async`, read the value one frame later by a request id.
  A `Depth32Float` readback the same way gives the world point.
- *Outline.* Jump flood [bgolus]: render the selected ids into a mask; an init pass
  writes each contour pixel's own coordinate; `log2(width)` ping-pong passes
  each sample the 8 neighbours at a halving step and keep the nearest seed;
  a composite pass turns the distance into an outline of any width. The
  depth-and-id edge test in the blit shader (step 10) is the cheaper
  one-pass form for a 1-2 px line: ink where the neighbour id differs or the
  depth steps.
- *Orbit about the cursor.* On press, the ray through the cursor hits the
  mesh; the nearest hit is the pivot (fall back to the focal-plane point
  under the cursor, then the scene centre). Through the drag, the eye and
  the target rotate as one rigid body about the pivot, so the pivot keeps
  its pixel. The page model (`viewport_parity.html`, claim C2) states it as:
  keep the pivot's camera-space position, apply the yaw and pitch step, then
  re-place the eye so that position holds.
- *Zoom to the cursor.* Perspective: dolly the eye along the ray through
  the cursor by the radius change, then re-place the target on the view axis
  at the new radius (three.js). The page model's form, `target' = target +
  (1 − k)(P − target)`, is the same move written on the target (claim C1).
  Orthographic: unproject the cursor before and after the scale change and
  shift the camera by the difference.
- *MSAA offscreen.* Pipelines with `multisample.count = 4`; a colour and a
  depth texture with `sample_count = 4`; the pass sets the multisampled view
  as `view` and the single-sample texture as `resolve_target`; integer
  attachments (the ID buffer) cannot resolve, so they stay single-sample or
  take a custom resolve.
- *Box select.* CPU: a frustum from the rectangle corners; test projected
  face loops or item polylines (window = all inside, crossing = any
  segment inside). It cannot see occlusion (Fusion's "select through").
  GPU: read the id buffer under the rectangle and collect the distinct ids,
  visible faces only. The hybrid is common: CPU for "through", GPU for
  "visible only".

**Credits to add when a step lands** (`CREDITS.md`): three.js
`OrbitControls` (MIT) for the zoom-to-cursor form; Fornjot (0BSD) for the
focus-point orbit; rerun `re_renderer` (MIT or Apache-2.0) for the readback
belt and jump-flood design; wgpu `msaa_line` (MIT or Apache-2.0) for the
resolve pattern. FreeCAD and Blender are named as behaviour references only.



## D. Recommendation

One path. Keep the egui host, the eframe loop, the own wgpu renderer and
the `upload_cache` slot model. Add interaction features in the order of
value per cost. Every step is small enough to land alone, with its test,
without stopping the GUI. Costs use the B.2 scale.

### D.0 Rules that hold for every step

- A hover and a camera move are UI state. A selection that changes an op
  goes through `ProjectSession::apply(Command)` as a `SelectionEdit`, so
  MCP parity is by construction: `edit_toolpath_selection` takes the same
  edit. A new edit kind (a batch, a rule) is a wire type in `rs_cam_mcp`.
- No text in the viewport. Colour, outline, cursor and a one-line tooltip.
- Each step lands with its sentry and an MCP screenshot pair
  (`set_ui_view` then `screenshot_gui`, before and after) in
  `planning/viewport_parity_2026-10-28/evidence/`.
- Measure before and after with the frame-time instrument of step 1.

### D.1 The steps

| # | Step | What changes | Files | Cost | Risk | Proof |
|---|---|---|---|---|---|---|
| 1 | **Instrument and feedback** (FB-1, FB-2, FB-3) | A rolling frame time and the last pick time in `GuiState`; a status-bar hover shows them; MCP `gui_status` or `get_diagnostics` returns them. Cursor: `PointingHand` over a pickable face or item in pick mode, `Grabbing` while orbiting, `Move` while panning. A one-line face tooltip outside pick mode (surface type, size, Z in the setup frame). | `V/state/runtime.rs`, `V/app/viewport.rs:684-815`, `V/ui/status_bar.rs`, `V/app/mcp/diagnostics.rs` | S | Low. A tooltip must not fight the pick-mode tooltip (one `Tooltip` id each). | A viz `--lib` test that the instrument updates per frame; `get_diagnostics` returns the fields; screenshot shows the cursor and the tooltip. |
| 2 | **A BVH per mesh model** (SEL-2) | `pick_model_face` casts against a BVH built once per model revision and cached beside the mesh (a `OnceCell` in the GUI model row, keyed by the model id and revision). The hover runs ONE pick per frame; the face hover and the toolpath tooltip read the same result. | `C/mesh.rs` (a `MeshBvh` type or a `parry3d-f64` `TriMesh` wrapper), `V/interaction/picking.rs:233-306`, `V/app/viewport.rs:727-815` | S | The crate choice (C.2): current `parry3d-f64` (0.31) is glam-based and `bvh` 0.13 wants nalgebra 0.35, so the clean fit is an own flat BVH in `C/mesh.rs` (a median-split AABB tree over `TriangleMesh`, about 200 lines, f64, nalgebra 0.33, deterministic tie order) with the probe as its parity test; second choice `parry3d-f64` 0.24 (measured, nalgebra 0.34 beside 0.33). Mesh revision: a rescale or a re-import must drop the cache (the `upload_cache` keys show where). | B.1 agreement test: BVH and brute force give the same nearest triangle on the terrain for 400 rays (probe method). A `--lib` test on the cache drop. Step 1 shows pick time 2 ms → under 0.1 ms on the terrain. |
| 3 | **Camera: pivot, zoom target, pan scale, scheme** (CAM-1, CAM-2, CAM-3, CAM-8) | `OrbitCamera::zoom` takes the cursor ray and moves the target by `(1 − k)(P − target)` (P = the point under the cursor at the target depth; orthographic: scale the half-height and shift the target). `pan` uses `2 × distance × tan(fov/2) / vh` per pixel. `orbit` takes a pivot: the BVH hit under the cursor at drag start, else the current target; the target is re-placed so the pivot keeps its pixel (the page model proves the math: claims C1-C3). Scheme: **right drag orbits, middle drag pans, wheel zooms to the cursor, left is select** (the right-button family of A.2b: Onshape, Tinkercad and Shapr3D, the web-tool family the brief names as the reference; it also frees the left button for the box select). A preference offers the middle-button scheme (SolidWorks: middle orbit, Ctrl+middle pan; Fusion and FreeCAD ship it as a preset). Read `zoom_delta` for a pinch. | `V/render/camera.rs`, `V/app/viewport.rs:848-881`, `V/state/app_settings.rs` (a preference row), `V/ui/preferences.rs` | S-M | A changed left button breaks habit: the first orbit attempt selects. Mitigation: the preference, and the shortcuts window names the scheme. The orthographic zoom-to-cursor needs its own test. A tab drag still wins the left button (`handle_tab_drag`). | Rust unit tests that mirror `check_claims.mjs` C1-C3 and C8 (point under the cursor fixed after zoom, orbit and pan; both projections). An egui harness test: a right drag changes yaw, a left drag does not. |
| 4 | **Camera over MCP** (CAM-9, CAM-6) | `set_ui_view` gains `view_preset` (top, front, right, iso, and later the cube presets), `projection`, `fit` (`model` or `selection`) and an optional `camera {yaw, pitch, distance, target}`; the reply echoes the pose. Key F fits the selection (faces, items or the toolpath). | `crates/rs_cam_mcp/src/server.rs:934`, `V/app/mcp/view.rs:442`, `V/app/input.rs`, `V/app.rs:440-502` | S | Low. Keep the instant pose write for MCP (no tween; step 6). | Parity test: the same preset by key and by MCP gives the same `view_proj`. `mcp_authoring_surface` sentry for the new fields. |
| 5 | **MSAA** (REN-4) | A 4-sample colour and depth texture in `ensure_offscreen`; the five offscreen pipelines `MultisampleState { count: 4, .. }`; the single-sample offscreen colour becomes the `resolve_target`; the blit pipeline (`:714`) and the blit itself are unchanged, because they draw into egui's single-sample pass. Query the adapter's sample support and fall back to 1. | `V/render/mod.rs:358, 471, 506, 541, 650, 811, 822, 1062-1110`, `V/render/gpu_safety.rs` (the texture budget grows) | S | Memory: a multisampled attachment costs `width × height × 4 bytes × samples` for the colour and the same for the depth, so at 3840 × 2160 and 4 samples about 133 MB each, 265 MB together; `gpu_safety` must count it. The ID texture of step 10 cannot resolve: keep it single-sample in its own pass, or resolve by sample 0. | `render_pipelines_headless_g_pipesmoke` with the sample count; a screenshot pair on a STEP edge at 400 %. |
| 6 | **Eased presets and Fit** (CAM-4) | A `CameraTween { from, to, started, duration }` on `RsCamApp`; `set_preset` and `fit_to_bounds` start it from the UI; each frame samples an ease-in-out curve and calls `request_repaint` only while the tween runs (G-LV.1). MCP writes the pose at once, so the two-frame settle of `screenshot_gui` stays true. Duration 250 ms (the reference range is 0 to 500 ms, A.4b). | `V/render/camera.rs`, `V/app.rs`, `V/app/input.rs:485-510`, `V/app/mcp/view.rs` | S-M | A repaint loop that never ends (the tween must end exactly). A screenshot taken mid-tween: the MCP route bypasses the tween; the GUI F12 route waits for `tween.is_none()`. | A test that the tween ends in `duration` and that an MCP preset lands in one frame. The repaint-count sentry pattern of G-LV.1. |
| 7 | **Face edges** (REN-1, REN-3) | A new upload slot `model_edges`: every `FaceGroup.boundary_loops` of each visible STEP model as a `LineList` in an ink token (`colors.rs`), drawn after the faces with `depth_compare: LessEqual` and a small view-space offset toward the eye in the vertex shader. A registry row `model_edges` (on by default, A.3b: Onshape default) that `set_ui_view overlays` switches. In the same step, smooth normals inside a face group and a hard edge between groups (`FaceTessellation` vertices are already per face). Later (M): keep `cshell.edges` at import and polyline the curves (`PolylineableCurve`) for exact edges on a coarse tessellation. | `V/app/gpu_upload/meshes.rs`, `V/render/mod.rs` (line pipeline reuse), `V/render/colors.rs`, `V/ui/overlays/registry.rs`, `V/render/mesh_render.rs:258-305` | S (M for exact curves) | Z-fighting of a 1 px line on its face: the offset must scale with depth. A heavy part doubles the line count (Onshape warns edges cost render time); the row can switch off. | `viewport_draws_selected_only_wp27` style draw-set test; the headless smoke test with the slot; a screenshot pair of `stepped_block.step`. |
| 8 | **Per-face state buffer and hover without a rebuild** (SEL-1) | The STEP vertex gains a `face_id: u32`; a storage buffer holds one state word per face (hovered, selected, item state colour index); the fragment shader reads the colour from the state and a small palette. A hover or a selection change writes bytes with `queue.write_buffer`; the mesh key loses `hovered_face` and `selected_faces`. The STL flat-face hover gets the same path with a region id per triangle. The `face_id` attribute is also the input of the ID pass of step 10. | `V/render/mesh_render.rs`, `V/render/mod.rs` (WGSL), `V/app/gpu_upload/meshes.rs`, `V/render/upload_cache.rs` | M | Storage buffers need a bind group per model; the face count is capped (`TooManyFaces`), so the buffer is small. The item state colours must stay the `colors.rs` tokens (sentry `the_selection_colours_live_in_colors_p4_4`). | `upload_cache` stats test: a hover change adds zero enriched builds (the V13 defect becomes a sentry). Step 1 shows the frame time on hover. |
| 9 | **Box select** (SEL-4, SEL-3) | A left drag in pick mode (or Shift + left drag outside it) draws an egui rectangle: solid for left-to-right (enclose), dotted for right-to-left (crossing), the convergent rule of A.1b. On release: items whose projected lines are inside (enclose) or cross (crossing) the rectangle, and STEP faces by their projected `boundary_loops`; the pick filter chips apply. One core edit `SelectionEdit::AddPicks(Vec<MatchKey>)` (and `RemovePicks` with Ctrl) so one undo step holds the box, with the MCP wire type beside it. Modifier rule: Shift adds, Ctrl removes, a plain click toggles (Onshape), stated in the shortcuts window. | `V/app/viewport.rs`, `V/interaction/picking.rs` (`pick_items_in_rect`), `C/selection/rule.rs:371`, `C/session` command, `crates/rs_cam_mcp`, `V/ui/pick_bar.rs` | M | Occlusion: a CPU rectangle test selects hidden faces too (Fusion calls that "select through"); step 10's ID buffer gives the visible-only mode later. A drag must not start a pick on a tab mark. | An egui harness test: a box over two loops gives one `AddPicks` of two keys and one undo step. MCP parity test: `edit_toolpath_selection` with the same keys gives the same `selection_status`. |
| 10 | **ID pass, selection outline and silhouette** (REN-2, REN-7, SEL-5, SEL-9) | The enriched pipeline writes `face_id` to a second `R32Uint` attachment (MRT). The blit shader reads depth and id: a neighbour with another id or a depth step draws ink (silhouette and crease), a neighbour of a selected id draws the selection outline in the accent token. The same id texture read back under the cursor (one `copy_texture_to_buffer` of a 1 px region into a 256-byte-row buffer, mapped next frame; the rerun readback belt is the model, C.2) gives a visible-only pick and the "select other" list (every id along the ray from the BVH, the visible one first). | `V/render/mod.rs` (pipelines, blit WGSL, a readback ring), `V/render/mesh_render.rs`, `V/app/viewport.rs` | M | Readback latency is one frame (map next frame); the click uses the BVH result at once and the id for the visible-only question only. MSAA: the id attachment is single-sample in its own small pass, or the id resolves by sample 0. | Headless smoke test renders the id attachment and reads one pixel; screenshot pair of a selected face with the outline. |
| 11 | **Select similar and view-side rules** (SEL-7, SEL-10, SEL-11, SEL-13) | A right-click menu (or a key) on a picked item adds a rule derived from it: "same diameter ± tolerance" (exists), "at the same Z" (new `Rule::AtZ`), "same face normal" (new). `pick_items` runs outside pick mode for the selected op (hover ring and tooltip; the click stays gated). A Geometry tab row click frames its item. Ctrl+A in pick mode adds the rule of every admitted kind. | `C/selection/rule.rs`, `V/ui/pick_bar.rs`, `V/ui/properties/geometry_tab.rs`, `V/app/input.rs` | S-M | A new rule kind is a project-file key: state the break in the commit (operator ruling 2026-09-16). | Core rule resolver tests; MCP `edit_toolpath_selection AddRule` parity test. |
| 12 | **View cube** (CAM-5) | An egui-painted cube in the corner: 6 faces, 12 edges, 8 corners as hit regions that send a preset; a drag on it orbits. Replaces the triad only when on. | `V/ui/viewport_overlay.rs`, `V/render/camera.rs` (26 presets) | M | Low value per cost: the dock presets and keys 1-4 exist. Do it last. | Harness test: a corner click gives the trimetric yaw and pitch. |

Not in this path (ruled out or later): section planes (REN-6, M, low
value), model x-ray (REN-5, S, low value), sloped and vertical edge items
(SEL-8, L, a product decision on what the 2.5D ops cut), tangent chains
across DXF items (SEL-6, core rule, GAP_ANALYSIS row 10), undo of the
tree face selection (SEL-12).

### D.2 What we must not do

- **Do not move the viewport onto an engine.** Section C.1: a three-way
  version lock, a host swap, an ECS the mutation contract does not need,
  weeks without a GUI. The one capability an engine adds that we lack is a
  camera controller, and that is a 200-line change (step 3).
- **Do not replace egui.** The viewport is our own wgpu renderer; egui only
  hosts it. The immediate-mode cost is in overlays, and the dock already
  solves that.
- **Do not start with GPU picking.** A CPU BVH gives a click and a hover in
  under 0.1 ms (B.1) and keeps the pick identical for MCP, which has no GPU
  frame. The ID buffer earns its place later for the outline and the
  visible-only question (step 10).
- **Do not bake a selection or a hover into the vertex buffer again** (the
  V13 defect, SEL-1). State lives in a small buffer; the mesh key must not
  change on a hover.
- **Do not add an upload path outside `upload_cache`** (F132) or a colour
  outside `colors.rs` (F173).
- **Do not animate the camera for an MCP call.** `set_ui_view` writes the
  pose at once; `screenshot_gui` relies on a two-frame settle.
- **Do not change the mouse scheme without a preference and a note in the
  shortcuts window.** The left button becomes select; a user who orbits on
  the left loses the habit on day one.
- **Do not take the sloped-edge question into the viewport work.** It is a
  product rule about what the 2.5D ops cut (UX-7), not a rendering gap.

