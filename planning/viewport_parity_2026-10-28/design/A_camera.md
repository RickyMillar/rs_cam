# Design A: the camera (CAM-1, CAM-2, CAM-3, CAM-4, CAM-6, CAM-8, CAM-9)

Date 2026-10-28. HEAD b5aa2a45 (master). Design only: no product file changes, no cargo run.
`V/` = `crates/rs_cam_viz/src/`. `T/` = `crates/rs_cam_viz/tests/`.

**Files read**

| File | Lines | What it gives |
|---|---|---|
| `V/render/camera.rs` | 1-281 | `OrbitCamera`, `orbit` :103, `pan` :112, `zoom` :134, `fit_to_bounds` :140, `project_to_screen` :152, `unproject_ray` :175, `set_preset` :217; ortho `half_h = distance·tan(fov/2)` :79 |
| `V/app.rs` | 60-80, 395-505, 930-1000, 1340-1425 | `camera` field :70, init :407, fit after load :435, `fit_camera_to_bbox` :440, `frame_selection_items` :454, `frame_model_items` :478, `fit_camera_to_first_model` :493; F12 :959; `pump_dispatch` :970; `handle_events` :1212; repaint gate :1384 |
| `V/app/input.rs` | 12-40, 215-305, 480-512, 715-725 | fit after import :22-38, Show me :221-225, view arms :265-303 (comment: "these arms run in the application because each writes the camera"), keys 1-4 :485-510, fit after open :722 |
| `V/app/viewport.rs` | 30, 386, 451, 613, 739, 782, 848-890, 1262, 1356, 1433 | every camera read and the drag and wheel block |
| `V/app/pin_marks.rs` | 133 | `project_to_screen` |
| `V/app/mcp/view.rs` | 276-341, 442-710 | `screenshot_gui` two-frame settle :313-328; `mcp_set_ui_view` :442; reply note :708 |
| `V/app/mcp.rs` | 585-608 | `SetUiView` dispatch arm |
| `V/app/mcp/project.rs` | 843 | MCP `load_project` fit |
| `V/mcp_server.rs` | 1852-1882 | `set_ui_view` tool, description, wire → `SetUiViewArgs` (`frame_items` via `Into`) |
| `V/ui_command.rs` | 187-210, 391-414, 569-576 | `SetUiViewArgs`; registry rows `SetViewPreset`, `ToggleProjection`, `PreviewOrientation`, `ResetView` (mcp `Skip("no wire tool …")`) |
| `V/controller/events/mod.rs` | 862-865, 881 | no-op arms for the view commands |
| `V/interaction/picking.rs` | 63 + 12 call sites | `PickContext::camera: &OrbitCamera` |
| `V/state/mod.rs`, `V/state/viewport.rs` | 139-160, 545, 673; 55-95 | `AppState` fields, `new`, `Default`; `ViewportState` |
| `V/ui/viewport_overlay.rs`, `V/ui/overlays/panel.rs` | 291-298; 791, 822 | `projection: ProjectionMode` passed beside `state` |
| `V/ui/tokens.rs` | 701-705 | `MOTION_FAST` 0.12, `MOTION_BASE` 0.18, `MOTION_SLOW` 0.24 |
| `crates/rs_cam_mcp/src/server.rs` | 932-990, 4150-4200 | `SetUiViewParam` (`deny_unknown_fields`), its serde tests |
| `T/mcp_wire_surface_pin.rs` | 1-70, 183, 261 | snapshot `tests/snapshots/mcp_wire_surface.json`, `RS_CAM_UPDATE_WIRE_SNAPSHOT=1` |
| `T/mcp_authoring_surface.rs` | 1-76 | reads schema and description off the router; no `set_ui_view` arm today |
| `T/command_surface_completeness.rs` | 280-300, 431-447, 520-540 | `view_constructs` needle `Ui(UiCommand::<Id>(`; `SetUiView` must not be built by a GUI control; `UI_DISPATCH` files and `SESSION_WRITES` |
| `T/the_wheel_belongs_to_the_top_layer_g_scrolllayer.rs` | 105-150 | scans 200 chars after `smooth_scroll_delta` in `app/viewport.rs` for `response.hovered()` |
| `T/overlays_registry.rs` | 900 | pins the text `viewport_overlay::draw(ui, state, projection, &lane_snapshots, events, rect)` |
| `REPORT.md` | A.2, A.2b, A.4b, B.2 CAM rows, C.2 (c)(d), D.0, D.1 steps 3/4/6, D.2 | requirements, sources, step plan |
| `viewport_parity.html` | `<script id="camera-model">` | `zoomToCursor`, `orbitAboutPivot`, `panDerived`, `tween`, `easeInOut` (perspective only) |

## 1. Where the camera state lives

### 1.1 Every reader and writer of `self.camera` today

| Site | Kind | What |
|---|---|---|
| `V/app.rs:70`, `:407` | field, init | `camera: OrbitCamera`, `OrbitCamera::new()` |
| `V/app.rs:441` | write | `fit_camera_to_bbox` → `fit_to_bounds` (callers `:465`, `:489`, `:502`, `input.rs:22,27,32,38`) |
| `V/app.rs:504` | write | `fit_camera_to_first_model` resets to `new()` (callers `app.rs:435`, `input.rs:28,303,722`, `mcp/project.rs:843`) |
| `V/app/input.rs:274-299` | write | `set_preset`, `toggle_projection`, `PreviewOrientation` writes `yaw`/`pitch` fields directly |
| `V/app/viewport.rs:850`, `:856`, `:881` | write | `orbit`, `pan`, `zoom` from the drag and wheel |
| `V/app/viewport.rs:30`, `:386`, `:739`, `:782` | read | `PickContext { camera: &self.camera }` |
| `V/app/viewport.rs:451` | read | `unproject_ray` (sim semantic pick) |
| `V/app/viewport.rs:613` | read | `projection` handed to `viewport_overlay::draw` |
| `V/app/viewport.rs:889-890` | read | `view_proj`, `eye` for the frame uniforms |
| `V/app/viewport.rs:1262` | read | `view_matrix` for the orientation gizmo |
| `V/app/viewport.rs:1356`, `:1433` | read | `project_to_screen` (area labels, box corners) |
| `V/app/pin_marks.rs:133` | read | `project_to_screen` (pin labels) |
| `V/app/mcp/view.rs:656` | write (indirect) | `frame_selection_items` |

Constructions of `OrbitCamera` outside the app (unchanged by this design): `V/app/mcp/commands/selection.rs:2133`,
`V/controller/tests/selection.rs:455`, `T/an_arc_move_draws_on_its_arc_r_arc.rs:190`, `T/a_click_picks_the_drawn_item_p2_6.rs:205`,
`T/pin_editor_and_viewport_p7.rs:420`, `T/pick_mode_picks_by_kind_p4_5.rs:268`.

### 1.2 Recommendation: `V/state/camera.rs`, held as `AppState.camera`

The decisive fact: `RsCamApp` needs an `eframe::CreationContext`, and no test in `T/` builds one. Every MCP/GUI parity
test today works because the MCP arm calls a function over `AppState` (for example `commands/selection.rs:2040`).
A camera on `RsCamApp` cannot have a parity test. `T/show_me_frames_the_items_p4_4.rs` already stops at the bounds
for this reason: it cannot reach the camera.

- `render/camera.rs` keeps `OrbitCamera` and all its math. It has no egui and no wgpu import. `PickContext` and the
  six test constructions stay as they are.
- `state/camera.rs` adds `CameraState` (the pose, the motion, the drag) and `apply_camera_request` (the MCP door).
- `AppState` gains `pub camera: CameraState`. The pose field is private, so every write goes through a method that
  cancels a running motion. The compiler holds that rule (operator preference 2026-10-14: compiler over doc).
- F096 holds: the camera is view state, like `selection` and `viewport`. It is not a `Command`, it does not touch
  `session`, and it does not stale anything. It is not saved in the project file and not in the setup sheet.
- Audit for a new GUI-state field (viz `CLAUDE.md`): `AppState::new` (`V/state/mod.rs:545`) and `Default` (`:673`)
  build `CameraState::default()`; the `split_mut` borrow struct (`:230-240`) gains the field only if a caller needs a disjoint borrow;
  project IO and the setup sheet: no change; MCP and UI initialisers: none construct `AppState` field by field.

What moves and what stays:

| Item | Before | After |
|---|---|---|
| The camera value | `RsCamApp.camera` | `AppState.camera.pose()` (read), `CameraState` methods (write) |
| `fit_camera_to_bbox`, `fit_camera_to_first_model` | `RsCamApp` methods | `CameraState::fit(bbox, how)`; `first_model_bounds(session)` as a free fn in `state/camera.rs` |
| `frame_selection_items`, `frame_model_items` | `RsCamApp` | stay on `RsCamApp` (they write `selection_flash` with egui time); they gain `how: CameraWrite` |
| View arms `input.rs:274-303` | write `self.camera` | stay in `app/input.rs` (they need `ctx` for `now`); they call `CameraState` methods with `CameraWrite::Eased` |
| Drag and wheel `viewport.rs:848-881` | inline | inline, but call `CameraState` drag methods |
| `viewport_overlay::draw(…, projection, …)` | param | keep the param in the move commit (sentry `overlays_registry.rs:900`); a later commit may drop it |

The view arms could move into the controller now (the comment at `input.rs:265-273` stops being true), but the
controller has no clock. Keep them in the app; see open question Q2.

`V/state/CLAUDE.md` is at its 40-line cap. The commit that adds `camera.rs` must merge one line (for example fold
`selection.rs` and `overlays.rs` into the `selection_highlight.rs` line) so the file stays at 40. `folder_maps_name_real_files_f140` checks the name.

## 2. The representation and the API

Keep `{target, distance, yaw, pitch}`. Every operation below is a move of `target` (and `distance`) that the turntable
form can hold, so no eye-and-target representation is needed. The pivot is an argument, not stored state, except
during a drag (§1, `CameraDrag`).

```rust
// V/render/camera.rs
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraPose { pub target: Point3<f32>, pub distance: f32, pub yaw: f32, pub pitch: f32 }

#[derive(Debug, Clone, Copy, PartialEq)]   // new derives on the existing struct
pub struct OrbitCamera { /* fields unchanged */ }

pub struct CameraBasis { pub eye: Point3<f32>, pub forward: Vector3<f32>, pub right: Vector3<f32>, pub up: Vector3<f32> }

pub const PITCH_LIMIT: f32 = std::f32::consts::FRAC_PI_2 - 0.01;   // the clamp at :105-108, named once
pub const DISTANCE_RANGE: (f32, f32) = (0.1, 50_000.0);           // the clamp at :136, named once
pub const ORBIT_RAD_PER_PX: f32 = 0.005;                          // :104
pub const ZOOM_PER_WHEEL: f32 = 0.001;                            // :135, k = exp(-delta·ZOOM_PER_WHEEL)

impl OrbitCamera {
    pub fn pose(&self) -> CameraPose;
    pub fn set_pose(&mut self, pose: CameraPose);                 // clamps pitch and distance
    pub fn basis(&self) -> CameraBasis;                           // right = (-sin yaw, cos yaw, 0), as pan :115
    pub fn set_projection(&mut self, mode: ProjectionMode);
    /// The point on the ray through (sx, sy) in the plane through `target` normal to `forward`.
    pub fn point_at_target_depth(&self, sx: f32, sy: f32, vw: f32, vh: f32) -> Option<Point3<f32>>;
    /// World size of one pixel at camera depth `depth` (perspective) or anywhere (orthographic).
    pub fn world_per_pixel(&self, depth: f32, vh: f32) -> f32;
    pub fn orbit_about(&mut self, pivot: Point3<f32>, dx: f32, dy: f32);
    pub fn zoom_toward(&mut self, point: Point3<f32>, k: f32);
    pub fn pan_pixels(&mut self, dx: f32, dy: f32, vh: f32, grab_depth: Option<f32>);
    pub fn preset_pose(&self, preset: ViewPreset) -> CameraPose;  // yaw/pitch of set_preset, target and distance kept
    pub fn fit_pose(&self, min: [f32; 3], max: [f32; 3]) -> CameraPose; // the math of fit_to_bounds
}
```

`orbit`, `pan`, `zoom`, `fit_to_bounds` and `set_preset` are deleted (ruling 2026-09-16: no legacy shapes). Their
callers move to the new names in the same commit.

**Zoom toward a point.** `zoom_toward(P, k)`:

```text
d'      = clamp(k · d, DISTANCE_RANGE)
kk      = d' / d                       // the clamp-honest ratio (camera-model zoomToCursor)
target' = target + (1 − kk)(P − target)
```

This is a homothety about `P` with ratio `kk`: `eye' − P = kk (eye − P)`, and the basis does not change. So
`P − eye'` keeps its direction (perspective: same pixel) and its camera-space x and y scale by `kk` while `half_h`
scales by `kk` (orthographic: same pixel). The rule holds for any `P` in both projections, not only a point at target
depth. The caller picks `P`: the model hit under the cursor when one exists, else `point_at_target_depth`.
`unproject_ray` (`:175`) already inverts the orthographic matrix (parallel rays, offset origins), so
`point_at_target_depth` has no projection branch.

**Orbit about a pivot.** `orbit_about(Q, dx, dy)` (camera-model `orbitAboutPivot`, claim C2):

```text
q        = ((Q − eye)·right, (Q − eye)·up, (Q − eye)·forward)     // old basis
yaw'     = yaw − dx·ORBIT_RAD_PER_PX
pitch'   = clamp(pitch + dy·ORBIT_RAD_PER_PX, ±PITCH_LIMIT)       // clamp first
eye'     = Q − (q.x·right' + q.y·up' + q.z·forward')               // new basis
target'  = eye' + forward'·d                                       // distance unchanged
```

`Q` keeps its three camera-space coordinates, so it keeps its pixel in both projections (orthographic: x and y
fixed, `half_h` unchanged). With `Q = target`, `q = (0, 0, d)` and the result equals today's `orbit`.

**Pan.** `world_per_pixel(depth, vh)`: perspective `2·depth·tan(fov/2)/vh`; orthographic `2·half_h/vh =
2·d·tan(fov/2)/vh`. With `depth = d` the two are the same expression. `pan_pixels(dx, dy, vh, grab_depth)` uses
`grab_depth.unwrap_or(d)`, then `target += right·(−dx·s) + up·(dy·s)`. The grabbed point (the hit under the cursor at
drag start, depth `(P − eye)·forward`) then stays under the cursor in perspective (Fornjot uses the focus depth, C.2).
In orthographic the depth does not matter. Today's `0.001·d` equals the rule only at `vh = 2·tan(22.5°)/0.001 ≈ 828`
(claim C8).

**Callers of `project_to_screen` / `unproject_ray` keep working.** Both read `projection_matrix · view_matrix`, which
the pose defines; neither signature changes. Callers: `V/interaction/picking.rs:135,326,366,383,525,557,821,877,928,1197,1216,1258`;
`V/app/viewport.rs:451,1356,1433`; `V/app/pin_marks.rs:133`; `V/app/mcp/commands/selection.rs:2141`;
`V/controller/tests/selection.rs:459`; the four `T/` files of §1.1. They read `state.camera.pose()` instead of `self.camera`.

**The orthographic gap in the page.** The `camera-model` script has no orthographic arm, so claims C1-C3 are proven
for perspective only. Recommendation: a planning commit (C0, §6) adds `projectOrtho`, `rayOrtho` and claims C1o, C2o,
C3o to `viewport_parity.html` and `check_claims.mjs` before the product commit, so the operator rule "page plus a
script that asserts each claim" holds for both projections. The Rust tests (§5) then mirror both.

**The state wrapper.**

```rust
// V/state/camera.rs
pub enum CameraWrite { Instant, Eased { now: f64, duration: f64 } }
pub enum CameraDrag { Orbit { pivot: Point3<f32> }, Pan { grab_depth: Option<f32> } }

#[derive(Default)]
pub struct CameraState { pose: OrbitCamera, motion: Option<CameraMotion>, drag: Option<CameraDrag> }

impl CameraState {
    pub fn pose(&self) -> &OrbitCamera;
    pub fn is_moving(&self) -> bool;
    pub fn set_pose(&mut self, to: CameraPose, how: CameraWrite);   // every pose write ends here
    pub fn preset(&mut self, p: ViewPreset, how: CameraWrite);
    pub fn orientation(&mut self, yaw: f32, pitch: f32, how: CameraWrite); // PreviewOrientation
    pub fn fit(&mut self, bbox: &BoundingBox3, how: CameraWrite);
    pub fn set_projection(&mut self, mode: ProjectionMode);          // instant, see §3
    pub fn begin_drag(&mut self, drag: CameraDrag);                  // cancels a motion
    pub fn drag_by(&mut self, dx: f32, dy: f32, vh: f32);
    pub fn end_drag(&mut self);
    pub fn zoom_at(&mut self, point: Point3<f32>, k: f32);           // cancels a motion
    pub fn advance(&mut self, now: f64) -> bool;                     // §3
    pub fn finish(&mut self);                                        // §3
}
pub fn first_model_bounds(session: &ProjectSession) -> Option<BoundingBox3>;
```

The viewport block (`viewport.rs:848-881`) becomes: on `drag_started` pick under the cursor (today's
`pick_model_face`; the step-2 BVH later), then `begin_drag(Orbit { pivot: hit.or(target_depth_point).unwrap_or(target) })`;
on `dragged` call `drag_by`; on `drag_stopped` call `end_drag`. The wheel keeps its `response.hovered()` guard inside
the 200 characters after `smooth_scroll_delta` (sentry `g_scrolllayer`), and calls `zoom_at(point, exp(−scroll·ZOOM_PER_WHEEL))`.
A pinch reads `i.zoom_delta()` as `k = 1/zoom_delta` through the same call.

**The mouse scheme (CAM-8, D.1 step 3).** A pure function, so a test can hold the table without egui:

```rust
// V/state/camera.rs
#[derive(Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum NavScheme { #[default] RightOrbit, MiddleOrbit }
pub enum Gesture { Orbit, Pan }
pub fn gesture(scheme: NavScheme, button: PointerButton, ctrl: bool) -> Option<Gesture>;
// RightOrbit:  Secondary → Orbit, Middle → Pan, Primary → None (select)
// MiddleOrbit: Middle → Orbit, Middle+Ctrl → Pan, Secondary → Pan, Primary → None
```

`PointerButton` is egui's type; the function takes it by value and has no `Context`. The preference row lives with the
other Preferences rows (`V/state/app_settings.rs`, `V/ui/preferences.rs`). The tab editor keeps the right click and a
drag that starts on a tab (`app/CLAUDE.md`), and `handle_tab_drag` still runs first.

## 3. `CameraMotion`: the eased preset and fit

```rust
// V/state/camera.rs
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraMotion { from: CameraPose, to: CameraPose, yaw_step: f32, started: f64, duration: f64 }

impl CameraMotion {
    /// `yaw_step = atan2(sin(to.yaw − from.yaw), cos(to.yaw − from.yaw))`: the shortest arc.
    pub fn new(from: CameraPose, to: CameraPose, now: f64, duration: f64) -> Self;
    pub fn progress(&self, now: f64) -> f32;          // ((now − started)/duration).clamp(0, 1)
    pub fn sample(&self, now: f64) -> CameraPose;     // `to` verbatim when progress ≥ 1
}
pub fn ease_in_out_cubic(t: f32) -> f32 { if t < 0.5 { 4.0*t*t*t } else { 1.0 - (-2.0*t + 2.0).powi(3) / 2.0 } }
```

- **Interpolation** (camera-model `tween`): `s = ease(progress)`; `target`, `distance`, `pitch` linear in `s`;
  `yaw = from.yaw + yaw_step·s`. The projection does not interpolate.
- **Exact end.** `sample` returns `to` itself at `progress ≥ 1`, not `from.yaw + yaw_step`. The two yaws differ by a
  multiple of 2π, so the jump is not visible, and the end pose is bitwise equal to an instant write (the parity test
  of §4 needs this).
- **Duration.** The reference range is 0-500 ms (A.4b: FreeCAD "Animation duration" default 500 ms [freecad-prefs];
  Blender "Smooth View" in ms, 0 = off [blender-nav]); A.4b applies 250-500 ms to rs_cam. The design reads the
  duration from a token in `V/ui/tokens.rs`, never a literal in `state/`: the app passes it in `CameraWrite::Eased`.
  See Q3 for which token.
- **Time.** `now` is `ctx.input(|i| i.time)` (seconds, `f64`), the clock `frame_selection_items` already uses
  (`input.rs:225`). Tests pass any `f64`.
- **The frame slot.** In `RsCamApp::update`: `pump_dispatch` (`app.rs:970`, MCP writes land) → `handle_events`
  (`:1212`, a key preset starts a motion with `started = now`) → **new**: `let moving = state.camera.advance(now);`
  → the UI pass and `draw_viewport` read the sampled pose. `advance` writes `sample(now)` and returns `true` while
  `now < started + duration`; at the end it writes `to`, sets `motion = None` and returns `false`. That last frame
  draws the end pose, so no extra frame is needed.
- **Repaint only while it runs (G-LV.1).** The gate at `app.rs:1384` becomes
  `if active_lanes || playing || moving { ctx.request_repaint(); }`. Nothing else requests a repaint for the camera.
  When the motion ends, the request stops on that frame. When the viewport is not drawn (Readiness), `advance` still
  runs; the absolute clock ends the motion on the first frame after `started + duration`.
- **Cancel.** `begin_drag`, `zoom_at`, `set_pose(Instant)` and a new `set_pose(Eased)` cancel a running motion.
  A new eased write starts from the current sampled pose, so a second key press mid-motion does not jump.
- **Which writes ease.** Eased: keys 1-4, the dock preset buttons, Reset View, `PreviewOrientation`, the F key fit,
  the GUI "Show me". Instant: every MCP write, the fit after a load or an import (`app.rs:435`, `input.rs:22-38,722`,
  `mcp/project.rs:843`; the auto-screenshot run at `app.rs:1416` needs the framed pose on frame 3), and the projection
  toggle (the orthographic half-height equals the perspective half-height at the target, `camera.rs:75-81`, so it does not jump).
- **MCP `screenshot_gui`.** `mcp_screenshot_gui` (`view.rs:276`) calls `state.camera.finish()` (write `to`, drop the
  motion) when it arms the slot. Then the two-frame settle (`view.rs:313-328`) captures the end pose, even when the
  operator started a motion just before.
- **GUI F12.** F12 (`app.rs:959`) sends the screenshot command at once today. When `is_moving()`, set a one-shot
  `RsCamApp.screenshot_after_motion = true` instead; on the first frame where `advance` returns `false`, send the
  command. The operator sees the motion end (at most one duration) and the file holds the end pose. Alternative in Q4.
- **The `set_ui_view` reply note** (`view.rs:708`, "view changes render on the next frame") stays true: an MCP camera
  write is instant, lands inside `pump_dispatch`, and the next `draw_viewport` reads it.

## 4. MCP: camera fields on `set_ui_view`

**Wire types** (`crates/rs_cam_mcp/src/server.rs`, beside `SetUiViewParam`, `snake_case`, `deny_unknown_fields`):

```rust
#[derive(Deserialize, Serialize, schemars::JsonSchema, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ViewPresetParam { Top, Front, Right, Iso }
#[derive(/* same */)] #[serde(rename_all = "snake_case")]
pub enum ProjectionParam { Perspective, Orthographic }
#[derive(/* same */)] #[serde(rename_all = "snake_case")]
pub enum FitParam { Model, Selection }

#[derive(Deserialize, schemars::JsonSchema, Default, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct CameraPoseParam {
    /// Orbit target, mm, display frame.
    pub target: Option<[f64; 3]>,
    /// Eye-to-target distance, mm. Refused outside [0.1, 50000].
    pub distance: Option<f64>,
    /// Turntable yaw, degrees, about +Z from +X.
    pub yaw_deg: Option<f64>,
    /// Pitch, degrees, refused outside ±89.43 (the clamp, 90° − 0.01 rad).
    pub pitch_deg: Option<f64>,
}

// SetUiViewParam gains:
pub view_preset: Option<ViewPresetParam>,
pub projection: Option<ProjectionParam>,
pub fit: Option<FitParam>,
pub camera: Option<CameraPoseParam>,
```

**Into the crate.** `mcp_server.rs:1857` destructures the four new fields and builds one crate-typed bundle, as
`frame_items` does with `Into` (`:1877`). `SetUiViewArgs` gains `camera: Option<CameraRequest>` with no `cfg(feature = "mcp")`
gate, because `CameraRequest` is a crate type. The wire → crate matches are exhaustive (F166).

```rust
// V/state/camera.rs
pub enum FitTarget { Model, Selection }
#[derive(Default)]
pub struct PosePatch { pub target: Option<Point3<f32>>, pub distance: Option<f32>, pub yaw: Option<f32>, pub pitch: Option<f32> } // radians
#[derive(Default)]
pub struct CameraRequest { pub preset: Option<ViewPreset>, pub projection: Option<ProjectionMode>, pub fit: Option<FitTarget>, pub pose: PosePatch }
pub struct CameraEcho { pub pose: CameraPose, pub projection: ProjectionMode, pub fov: f32, pub framed: Option<BoundingBox3> }

/// The one door MCP uses. Validates every field first, then applies, all Instant.
pub fn apply_camera_request(state: &mut AppState, req: &CameraRequest) -> Result<CameraEcho, String>;
pub fn selection_fit_bounds(state: &AppState) -> Option<BoundingBox3>;   // shared with the F key
```

**Order and refusals.**

1. `mcp_set_ui_view` (`view.rs:442`) validates the camera request before step 1 (the workspace switch), so a refusal
   changes nothing. Refused: a non-finite number; `distance` outside `DISTANCE_RANGE`; `pitch_deg` outside
   `±PITCH_LIMIT`; `fit` and `frame_items` in one call (both frame); `fit: selection` when `selection_fit_bounds` is
   `None` ("fit selection needs a selected toolpath with picked items"); `fit: model` with no model.
2. It applies the request after step 6 (`frame_items`), so a `toolpath_index` in the same call is the selection that
   `fit: selection` reads. Inside the request the order is `fit` → `view_preset` → `projection` → `camera`. They
   compose: `fit` sets target and distance, `view_preset` sets yaw and pitch, `camera` overrides single fields last.
   `fit: model` is the Reset View rule (`first_model_bounds`), so the two doors agree.
3. Every write is `CameraWrite::Instant` and cancels a running GUI motion.
4. Unknown keys inside `camera` are refused by serde (`deny_unknown_fields`), as for the outer struct.

**Reply.** The echo always carries the pose, also when the call has no camera field, so an agent can record and replay it:
`"camera": {"target": [..], "distance": .., "yaw_deg": .., "pitch_deg": .., "projection": "perspective", "fov_deg": 45.0, "view_preset": "top" | null, "fit": {"min": [..], "max": [..]} | null}`.

`mcp_set_ui_view` takes one more parameter (`camera: Option<CameraRequest>`), not four, so the
`too_many_arguments` reason at `view.rs:440` stays true.

**What else must change in the same commit.**

| Item | Change |
|---|---|
| `T/snapshots/mcp_wire_surface.json` | regenerate with `RS_CAM_UPDATE_WIRE_SNAPSHOT=1`, read the diff (only `set_ui_view` moves) |
| `T/mcp_authoring_surface.rs` | new arm `set_ui_view_moves_the_camera_cam9`: the schema has `view_preset`, `projection`, `fit`, `camera` with the enum tokens; the description names each and says the write is instant |
| `V/mcp_server.rs:1853` description | add the four fields, the order, the refusals, and "the camera moves at once (no animation)" |
| `V/ui_command.rs:391-414` | the mcp `Skip` reasons of `SetViewPreset`, `ToggleProjection`, `ResetView` become false; new text: `"set_ui_view view_preset writes the same CameraState::preset"`, `"set_ui_view projection sets the mode"`, `"set_ui_view fit: model is the same fit"` |
| `crates/rs_cam_mcp/src/server.rs` tests (:4157-4200) | the new fields deserialize; an unknown key inside `camera` is refused |
| `V/app/CLAUDE.md` `set_ui_view` line | add "`camera` and `view_preset`/`fit` call `state::camera::apply_camera_request` (instant)"; merge a line to keep 40 |

`T/command_surface_completeness.rs:447` holds: no GUI control builds `SetUiView`. The F key fit is its own row (§6, C6).

## 5. Tests

Each is an integration test in `T/`, named `<behaviour>_<tag>.rs` as the folder does. Tags: `cam1`…`cam9` (the A.2 ids).
They use `rs_cam_viz::render::camera` and `rs_cam_viz::state` only; no egui context.

| File | Asserts |
|---|---|
| `the_zoom_keeps_the_point_under_the_cursor_cam2.rs` | Both projections × cursor at centre, corner, edge × `k ∈ {0.5, 0.9, 1.1, 2.0}` × `P` at target depth and off depth: `project_to_screen(P)` before and after differ by < 1e-2 px at 1600×900. A clamp arm: at `distance = 0.1`, `k = 0.5` leaves target and distance unchanged. |
| `the_orbit_keeps_its_pivot_on_its_pixel_cam1.rs` | 20 drag steps, some past the pitch clamp: the pivot pixel moves < 1e-2 px and `distance` is unchanged; with `pivot = target` the pose equals the old yaw/pitch step (regression anchor); both projections. |
| `the_pan_keeps_the_point_under_the_cursor_cam3.rs` | `vh ∈ {400, 828, 1400}`, both projections: a drag of (dx, dy) moves the grabbed point by exactly (dx, dy) px (< 1e-2). Anchor arm: at `vh = 2·tan(fov/2)/0.001` the derived step equals `0.001·d` (claim C8). |
| `a_camera_motion_eases_and_ends_exactly_cam4.rs` | `sample(started) == from`; `sample(started + duration) == to` bitwise; distance is monotone over 100 samples; 350° → 10° turns +20°, not −340°; `advance` is `true` inside, `false` at the end and after, and `is_moving()` is then `false`; `begin_drag` mid-motion cancels and keeps the sampled pose; `duration = 0` is instant. Source arm (non-vacuity anchor `fn update`): the `request_repaint` gate in `V/app.rs` reads the `advance` result and no other camera site calls `request_repaint`. |
| `a_view_preset_gives_one_view_by_key_and_by_mcp_cam9.rs` | For each preset: state A takes `camera.preset(p, Eased { now: 0, duration })` then `advance(duration + 1)`; state B takes `apply_camera_request(&mut b, &CameraRequest { preset: Some(p), .. })`. `pose().view_proj(16/9)` is bitwise equal. Same for `fit: model` against the Reset View route (`first_model_bounds` + `fit`). |
| `an_mcp_camera_pose_lands_in_one_frame_cam9.rs` | After `apply_camera_request` with a full `PosePatch`, `pose()` equals the request with no `advance` call and `is_moving()` is `false`; a running GUI motion is cancelled; refusals (NaN, `distance = 0`, `pitch_deg = 91`, `fit` with no model) change nothing (pose before == pose after). |
| `the_mouse_scheme_maps_each_button_cam8.rs` | The `gesture` table for both schemes; `Primary` is never a camera gesture. |

`render/camera.rs` keeps `unproject_round_trip` and gains an orthographic round trip in the same module.

Sentries per commit (local loop, no full gate): the new files above; `render_pipelines_headless_g_pipesmoke`,
`a_click_picks_the_drawn_item_p2_6`, `pick_mode_picks_by_kind_p4_5`, `pin_editor_and_viewport_p7`,
`an_arc_move_draws_on_its_arc_r_arc`, `the_wheel_belongs_to_the_top_layer_g_scrolllayer`, `show_me_frames_the_items_p4_4`,
`overlays_registry`, `the_dock_states_read_live_state_g_vpstate`, `the_viewport_dock_reaches_every_option_g_vpdock`,
`command_surface_completeness`, `command_registry_surfaces`, `folder_maps_name_real_files_f140`; for C5 add
`mcp_wire_surface_pin`, `mcp_authoring_surface`, `mcp_core_arm_describes_every_row`, `cargo test -p rs_cam_mcp -q`;
then `cargo test -p rs_cam_viz -q --lib` and clippy.

## 6. Commit sequence

Each commit is green alone. "Break" lines go in the commit message (ruling 2026-09-16).

| # | Commit | Files | Break |
|---|---|---|---|
| C0 | planning: orthographic arm and claims C1o-C3o in the camera model | `planning/viewport_parity_2026-10-28/viewport_parity.html`, `check_claims.mjs` | — |
| C1 | viz: camera math: `CameraPose`, `basis`, `orbit_about`, `zoom_toward`, `pan_pixels`, `point_at_target_depth`; the wheel zooms toward the point under the cursor; pan scale from the projection | `V/render/camera.rs`, `V/app/viewport.rs:848-881`, `T/the_zoom_keeps_the_point_under_the_cursor_cam2.rs`, `T/the_pan_keeps_the_point_under_the_cursor_cam3.rs` | `orbit`/`pan`/`zoom` removed; wheel zoom goes to the cursor; pan speed changes at viewport heights other than 828 px |
| C2 | viz: the camera moves to `AppState.camera` (`state/camera.rs`); no behaviour change | `V/state/camera.rs` (new), `V/state/mod.rs`, `V/state/CLAUDE.md`, `V/app.rs`, `V/app/input.rs`, `V/app/viewport.rs`, `V/app/pin_marks.rs`, `V/app/mcp/view.rs`, `V/app/mcp/project.rs` | `RsCamApp::fit_camera_to_*` removed |
| C3 | viz: orbit about the point under the cursor; pan holds the grabbed point; right-drag orbit scheme with a middle-orbit preference; pinch | `V/state/camera.rs`, `V/app/viewport.rs`, `V/state/app_settings.rs`, `V/ui/preferences.rs`, the shortcuts window, `T/the_orbit_keeps_its_pivot_on_its_pixel_cam1.rs`, `T/the_mouse_scheme_maps_each_button_cam8.rs` | the left drag no longer orbits (D.2: the preference and the shortcuts note land in this commit) |
| C4 | viz: eased presets and fit (`CameraMotion`); repaint only while it runs; screenshots wait or snap | `V/state/camera.rs`, `V/app.rs` (slot, gate, F12 flag), `V/app/input.rs`, `V/app/mcp/view.rs:276`, `V/ui/tokens.rs` (if Q3 picks a new token), `T/a_camera_motion_eases_and_ends_exactly_cam4.rs` | — |
| C5 | viz+mcp: `set_ui_view` takes `view_preset`, `projection`, `fit`, `camera`; reply echoes the pose | `crates/rs_cam_mcp/src/server.rs`, `V/mcp_server.rs`, `V/ui_command.rs`, `V/app/mcp.rs:585`, `V/app/mcp/view.rs`, `V/state/camera.rs`, `T/snapshots/mcp_wire_surface.json`, `T/mcp_authoring_surface.rs`, `V/app/CLAUDE.md`, the two cam9 tests | wire: new optional fields only (no agent call breaks) |
| C6 | viz: key F fits the selection (CAM-6) | `V/ui_command.rs` (row `FitSelection`: gui `Reached`, mcp `Skip("set_ui_view fit: selection writes the same fit")`, cli `Skip`), `V/app/input.rs`, `V/controller/events/mod.rs` (no-op arm), the shortcuts window | — |
| C7 | viz (optional): `viewport_overlay::draw` reads the projection from `state` | `V/ui/viewport_overlay.rs`, `V/ui/overlays/panel.rs`, `V/app/viewport.rs:648`, `V/controller/tests/mod.rs:387`, `T/the_dock_states_read_live_state_g_vpstate.rs`, `T/the_viewport_dock_reaches_every_option_g_vpdock.rs`, `T/overlays_registry.rs:900` (moved string) | the `draw` signature |

Rules kept: **F096** (the camera is view state; no `Command`, no session write; `command_surface_completeness` P3
scans `app/input.rs` for session writes and finds none); **F132** (no upload change: `view_proj` is a per-frame
uniform, not an `upload_cache` slot); **G-LV.1** (one repaint gate, true only while a motion runs; tested in cam4);
**F166** (exhaustive wire matches); **F140** (the folder map names `camera.rs`); **D.2** (no animation for an MCP call;
no scheme change without a preference and a shortcuts note). Each commit lands an MCP screenshot pair in
`planning/viewport_parity_2026-10-28/evidence/` (D.0), taken on a release binary built from HEAD.

## 7. Open questions for the lead

| # | Question | Recommendation |
|---|---|---|
| Q1 | The presets may be mislabelled. With the eye formula `camera.rs:54-59`, `yaw = 0` puts the eye on +X with +Y to the right, so `ViewPreset::Front` (`:223`) looks from +X, and `Right` (`:227`, yaw +π/2) looks from +Y. The usual Z-up CAD convention is Front = eye on −Y (X to the right), Right = eye on +X. `PreviewOrientation` disagrees with the presets: `FaceUp::Right` writes yaw −π/2 (`input.rs:298`), `FaceUp::Left` +π/2 (`:294`). | Confirm with one `screenshot_gui` pair on a stock with a labelled datum. If confirmed, set Front = yaw −π/2, Right = yaw 0, and route `PreviewOrientation` through `CameraState::orientation` with one table. State the break in C4. |
| Q2 | Move the view arms (`input.rs:274-303`) into the controller now that the camera is on `AppState`? The controller has no clock for an eased write. | Keep them in `app/input.rs` (they need `ctx` for `now`). The parity test covers the `CameraState` call both doors make. Revisit only if a frame clock lands on `AppState` for another reason. |
| Q3 | The duration token. `MOTION_SLOW` is 0.24 s (`tokens.rs:705`), inside the A.4b range; the REPORT says 250 ms. | Reuse `MOTION_SLOW`: it is an existing token, inside 250-500 ms within rounding, and it avoids a second near-equal number. If the lead wants a distinct knob, add `MOTION_CAMERA = 0.25` with a doc that cites A.4b. |
| Q4 | F12 during a motion: wait for the end, or capture the frame as drawn? | Wait (a one-shot flag). A saved screenshot of an intermediate pose has no use, and the wait is at most one duration. |
| Q5 | Wire units for yaw and pitch. | Degrees, with the unit in the key (`yaw_deg`, `pitch_deg`): an agent reads 90 more reliably than 1.5708. The crate keeps radians. |
| Q6 | What `fit: selection` and key F frame when the selected toolpath has no picked items. | Refuse in MCP with the reason; in the GUI the F key falls back to Reset View. A toolpath-result bbox fit is a later extension (needs the result bounds per toolpath). |
| Q7 | Distance interpolation: linear (the page model) or geometric (`d0·(d1/d0)^s`, even in perceived scale on a large fit change)? | Linear in C4, to match the proven page. Switch only after a screenshot sequence shows a problem, and update the page and its claim in the same commit. |
| Q8 | The orbit pivot before the step-2 BVH: run today's brute-force `pick_model_face` once per drag start (about 2 ms on the terrain, B.1)? | Yes. One pick per drag start is inside the one-frame budget (A.4b: 10 ms aim). Fall back to the point at target depth under the cursor, then to `target`. |
| Q9 | Should C3 (the scheme change) wait for the operator to try it? | Land C1, C2, C4, C5 first (no habit change). Land C3 after the operator rules on the scheme in a live look, because the left button stops orbiting. |
