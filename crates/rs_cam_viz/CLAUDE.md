# rs_cam_viz instructions

`rs_cam_viz` owns the egui desktop application, controller, compute workers,
rendering and the embedded MCP server. The server lives here, not in
`rs_cam_mcp`. Read root `CLAUDE.md` first.

## The directory map

Most directories carry their own `CLAUDE.md`. Read it before you change the
directory; it holds the file map, invariants, sentries and traps.

| Directory | What it holds | Directory file |
|---|---|---|
| `app/` | The application shell, the frame loop and the embedded MCP server | `app/CLAUDE.md` |
| `compute/` | The background worker lanes | `compute/CLAUDE.md` |
| `controller/` | The bridge between a UI intent and `ProjectSession` | `controller/CLAUDE.md` |
| `render/` | The wgpu viewport pipelines | `render/CLAUDE.md` |
| `state/` | GUI state: freshness, staleness, selection, viewport, jobs | `state/CLAUDE.md` |
| `ui/` | Every egui panel, modal and workspace | `ui/CLAUDE.md` |
| `ui/components/` | The shared component set | `ui/components/CLAUDE.md` |
| `ui/feeds/` | The feeds and speeds surfaces | `ui/feeds/CLAUDE.md` |
| `ui/overlays/` | The viewport dock, the All viewport options catalogue and their registry | `ui/overlays/CLAUDE.md` |
| `ui/properties/` | The inspector tabs | `ui/properties/CLAUDE.md` |
| `interaction/`, `io/` | `interaction/intent.rs` (F4): the pure `gesture_events` (a click hit, a box release, Escape → the events; no focus write). `interaction/input.rs` (B2): `InputSample`, the pure `step` on `SelectionUi::drag`, `apply_camera`, the mouse scheme table (`rows`, `classify`; B3: the core setting `settings::MouseScheme`, Alt rows for a laptop; P4: `on_gizmo` on the one `gizmo_disc` sets `InputEnv.press_on_gizmo`, and a left drag from the gizmo orbits about the target in every scheme). `interaction/picking.rs`: every viewport pick (`pick`, `pick_items`, `pick_tab`, `pick_tab_loop`, `pick_open_path`, `nearest_drawn_move`: an arc picks along its drawn arc), `surface_hit` (A C3a: the model point under the press origin), `pick_items_in_rect` (B6: the box, enclose or crossing, chip kinds, import order), `visible_keys` (SEL-5: the box keeps what the eye sees, rays through `LoadedModel::ray_pick`; `EyeTest` is the one ray test), `pick_items` (R5a: a click takes the first VISIBLE candidate: the ray's first pickable face, then the drawn items whose nearest drawn point passes `EyeTest`, then a wall's refusal), `pick_candidates` (E-Q6: every candidate under the pointer, hidden ones too, for the Tab cycle, `state::pick::step_cycle`), a tie within 2 px takes the Z nearest the surface (UX-7); the display-frame geometry (`drawing_plane_z`: one plane for draw and pick, `item_display_point`: a face or STEP edge item at its own Z, `selection_frame_bounds`) is `state/display_frame.rs`, `tab_scene`, `snap_to_loop`, `model_items_bounds`. `PickContext::visibility`: a hidden model takes no pick. `PickHit::MeshFace` (an STL) in pick mode only. `io/`: project import, export and the setup sheet; `io/app_settings.rs` only re-exports core's `settings` loader and writer | — |

## GUI contracts

- GUI state is not an alternate data model. Mutate core through
  `apply(Command)` and use `Effects.stale`.
- When GUI state gains a field, audit the setup sheet, project IO, the
  controller fixtures and the MCP and UI initialisers.
- Generated worker IR and post-simulation emitted IR can differ. A surface
  must name which evidence it reports; a planned reading is not emitted.
- Simulation presentation reads bounded triage first; `NotMeasured` abstains.

## Tests

Use focused `rs_cam_viz` integration tests first. A source-scanning test needs
a non-vacuity anchor and must not match a comment. Prefer behavioural egui
harness coverage when a contract can be rendered.
