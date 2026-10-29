# Evidence: live screenshots

All from the release binary built from master 994d613ad on 2026-10-29,
through the MCP proxy (`screenshot_gui`, 1400x900).

| File | What it shows |
|---|---|
| `01_default_view_after_import.png` | `stepped_block.step` after import plus a Pocket: 4x MSAA on the edges, the ink face edges (D4), smooth faces (D2), the selection states. |
| `02_preset_front_via_mcp.png` | `set_ui_view {camera: {preset: "front"}}`; the echo gave yaw -90, pitch 0, `view_preset: "front"`. Another session was working in the same GUI at that moment (a second setup and a Profile 2 appeared), and the selected toolpath is in that setup, so the picture also carries that setup's display frame. Take the preset pairs again on a quiet GUI. |
| `05_phase4_review_start.png` | The phase 4 build (76fb3a2f4): the Profile's default outline as the green 2.5 px ring. |
| `06_faces_selected_edges_on.png` | Four faces and three loops selected: the green face fill and the green rings merge (operator: faces need their own colour). |
| `07_display_mode_shaded.png` / `08_shaded_no_selection.png` / `09_shaded_with_edges_no_selection.png` | Shaded against Shaded with edges: the INK_65 edges at 1.25 px are nearly invisible on the pale faces. |
| `10_low_angle_selected_face.png` | The selected middle face at pitch 3 deg turns ink grey-blue: the outline's fold test fires on every pixel of a grazing face. |
| `11_round2_build_face_selected.png` | The round-2 build (712c7bf24): the picked middle face in blue, dark face edges, the accent outline on its silhouette, the green part outline at the base. |
| `12_round3_material_headlight.png` | The round-3 build (4c80e5585): one neutral material under the camera headlight, dark face edges, the picked face blue, no height planes while the Geometry tab is open. Compare with 11. |
| `13_declutter_bar.png` | The declutter build (f8b6e4045): the six-control bar (display mode, Grid, Stock Off/Box/Solid, Cutting moves, Rapids, Collisions) and More; the legend sits above the bar at 1400 pt. |
| `14_profile_picked_face_as_ring.png` | Round 4 build (24aa5bf8f): the Profile is in pick mode at once (the compact pick bar, no Pick press; `get_ui_view` reported the pick after `set_ui_view toolpath_index`), and its picked face shows as the green ring. |
| `15_pocket_picked_face_as_fill.png` | The same face picked on a Pocket: the blue fill. The Fill/Ring rule follows the op's item use. |
| `16_round5_view_cube_empty_scene.png` | build d195a1e80, empty scene, the view as found (yaw -86°, ortho): the view cube shows Front with the Top sliver; the Persp·Ortho toggle reads Ortho (`get_ui_view` projection = orthographic) |
| `17_round5_view_cube_iso_profile.png` | same build, stepped_block.step + Profile 1, `set_ui_view camera {fit: model, preset: iso, projection: perspective}`: the cube shows Top/Front/Right, Persp active; pick mode engaged with the focused Profile (Closed contours + Faces chips); the default outline ring draws green at Z 0 |
| `18_round6_profile_bottom_face_kept.png` | Round 6 build (a8598ca32 tree): `add_pick` of the bottom face (brep 1, faces down) on Profile 1 is kept: one row by pick, `plane_z` 0, state `cut`, no refusal (before R6a: "this face faces down in this setup" and a blocked selection). The context rail and the grouped More menu are on this build too. |
| `19_round7_operator_spill_as_found.png` | Round 6 build, the operator's own state: Parallel finish 2 on the STEP with a two-face boundary (offset 3.0 = the inherited diameter offset): the raster runs past each face edge and down the walls (green strips); the reach map reads 0 % for the 3 mm end mill (FOLLOW_UPS §12). |
