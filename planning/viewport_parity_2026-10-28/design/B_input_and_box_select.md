# B. Viewport input layer and box select (design, 2026-10-28)

Status: design only. No product file changes. HEAD b5aa2a45.
Scope: the GUI input layer, the mouse scheme, box select, one hover pick per
frame. The core batch edit is in `E_selection_edits.md`; this file names the
edits it needs.

## Files read

- `V/app/viewport.rs` :10-22, :46-73, :151-188, :192-350 (tab drag), :356-418 (click chain), :610-662, :664-815 (hover), :820-881 (orbit, pan, wheel). `V` = `crates/rs_cam_viz/src`.
- `V/state/pick.rs` :1-60, :105-130, :160-325 (`TabDrag`, `TabEditMode`, `ViewportMode`, `SelectionUi`); `V/state/tab_edit.rs` :25-42, :75-105.
- `V/ui/pick_bar.rs` :36-91 (`PickClick`, `click_event`), :391-419 (`hover_text`); `V/ui/mod.rs` :375-410; `V/controller/events/mod.rs` :518-600.
- `V/interaction/picking.rs` :1-100, :233-490 (`pick_model_face`, `pick_items`, `pick_mode_item`, `choose_item`).
- Core: `session/selection_status.rs` :188-215, :1403-1488, :1507-1531 (O-PICK), :1741-1765; `selection/rule.rs` :360-400; `selection/item.rs` :99-132; `selection/near.rs` :27-39; `settings/mod.rs` :95-132, :230-251, :614, :885-897; `settings/CLAUDE.md`.
- `V/app/input.rs` :419-435; `V/ui/shortcuts_window.rs` :1-81 (call `V/app.rs:1206`); `V/state/app_settings.rs` :1-120; `V/ui/preferences.rs` :866-880; `V/ui/tokens.rs` :150-191; `V/render/colors.rs` :1-20; `V/render/camera.rs` :20-26, :103-137.
- Tests: `the_tab_editor_writes_one_step_per_act_p4_6.rs` :1-50, :248-290; `a_click_picks_the_drawn_item_p2_6.rs` :203-250; `the_dock_states_read_live_state_g_vpstate.rs` :760-800, :950-965.
- egui 0.36.2 (pinned in `Cargo.lock`): `input_state/mod.rs` :63-65, :105-118, :555-570, :630-634; `response.rs` :400-412; `data/input/event.rs` :150-162.
- `planning/viewport_parity_2026-10-28/REPORT.md` :86-187, :309-325, :500-567; `planning/geometry_selection_2026-10-16/PLAN.md` :835-845; `P4_DESIGN.md` :388-400, :565-580, :1036-1045.

Two facts in the code differ from the REPORT. Prefer the code (planning/CLAUDE.md):

- REPORT SEL-3 says a shift-click adds the layer rule. `pick_bar.rs:73-91`
  sends `AddPick` for Shift (one item) and `PickLayer` for a right-click.
- The hover runs up to three picks in pick mode, not two. `pick_mode_key`
  (`viewport.rs:668-673`) calls `pick_mode_item`, which calls `pick_items` up
  to twice. Then the face hover calls `pick()` (`:752`). Outside pick mode
  the face hover and the tooltip each call `pick()` with the same arguments
  (`:752`, `:801`; both pass `drill_pick: None`).

## 1. The input state machine

### 1.1 Ownership

`DragState` is a field of `SelectionUi` (on `GuiState`), with its type in a
new file `state/viewport_input.rs`. The transition logic is a pure function
in a new file `interaction/input.rs`.
The reasons:

1. `TabEditMode.drag` already lives in `GuiState` (`state/pick.rs:177`).
2. `RsCamApp::new` needs an `eframe::CreationContext` (`app.rs:209`, comment
   in `tests/add_toolpath_via_gui_g_guiadd.rs:27`). No test can construct
   it, so state on `RsCamApp` has no test.
3. Escape (`app/input.rs:425`) must read the box. `GuiState` is in reach there
   and in every test fixture.

A drag has no content, so the state/CLAUDE.md draft rule does not force
it into `GuiState`; the testability reason decides. The camera stays on
`RsCamApp`. `step` returns actions; the frame applies them, and a test applies
them with `apply_camera` (1.3).

### 1.2 Types

```rust
// crates/rs_cam_viz/src/state/viewport_input.rs
/// The pointer gesture in the viewport. One gesture at a time.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum DragState {
    #[default]
    Idle,
    /// The camera turns. `pivot` is the point under the pointer at the
    /// drag start (`HoverResult::surface_point`), or `None` (the target).
    /// Step 3 of D.1 reads it; this design only records it.
    Orbit { pivot: Option<[f32; 3]> },
    Pan,
    /// A rectangle in viewport-local points. `anchor` is the press origin.
    BoxSelect { anchor: egui::Pos2, to: egui::Pos2, op: BoxOp },
    /// The tab editor owns the drag. The content (mark, snapped point)
    /// stays in `TabEditMode::drag`; this is the marker only.
    Tab,
    /// Escape cancelled a box. The button is still down. The release
    /// returns to `Idle` and sends nothing; the rest of the drag does not
    /// orbit or start a new box.
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxMode { Enclose, Crossing }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxOp { Add, Remove }

impl DragState {
    /// Left to right encloses, right to left crosses (REPORT A.1b SEL-4).
    pub fn box_mode(anchor: egui::Pos2, to: egui::Pos2) -> BoxMode {
        if to.x >= anchor.x { BoxMode::Enclose } else { BoxMode::Crossing }
    }
}
```

`SelectionUi` gains `pub drag: DragState`. `SelectionUi::set_pick`,
`set_tab_edit` and `keep_only` set it to `Idle` when they change or end a
mode, so one type holds the mode and its gesture (L-3).

Escape: `handle_keyboard_shortcuts` (`app/input.rs:425-430`) first tests
`selection_ui.drag` for `BoxSelect`. If it matches, the handler consumes
Escape and sets `Cancelled`; the mode stays on. Only a second Escape ends
the mode.

### 1.3 The frame sample and the transition

```rust
// crates/rs_cam_viz/src/interaction/input.rs
/// What the viewport `Response` and the input state say this frame.
pub struct InputSample {
    pub started: Option<egui::PointerButton>, // drag_started_by(b), first match
    pub dragging: Option<egui::PointerButton>, // dragged_by(b)
    pub stopped: bool,                          // drag_stopped()
    pub delta: egui::Vec2,                      // drag_delta()
    pub press_origin: Option<egui::Pos2>,       // pointer.press_origin()
    pub pointer: Option<egui::Pos2>,            // interact_pointer_pos()
    pub modifiers: egui::Modifiers,
    pub clicked: bool,                          // clicked()
    pub secondary_clicked: bool,                // secondary_clicked()
    pub hovered: bool,                          // hovered()
    pub zoom_delta: f32,                        // input.zoom_delta()
    pub scroll: egui::Vec2,                     // input.smooth_scroll_delta
}

impl InputSample {
    pub fn read(ui: &egui::Ui, response: &egui::Response) -> Self;
}

/// What the frame does after the transition.
#[derive(Debug, Clone, PartialEq)]
pub enum InputAction {
    Orbit { dx: f32, dy: f32 },
    Pan { dx: f32, dy: f32 },
    Zoom { factor: f32 },
    /// Hand the click to the click chain (`handle_viewport_click`).
    Click { pos: egui::Pos2, click: crate::ui::pick_bar::PickClick },
    SecondaryClick { pos: egui::Pos2 },
    /// The release of a box: the rectangle and its rule.
    BoxRelease { rect: egui::Rect, mode: BoxMode, op: BoxOp },
    /// The tab editor handles this frame (`handle_tab_drag`, unchanged).
    TabDrag,
}

/// The facts the transition needs and cannot read from the sample.
pub struct InputEnv {
    pub scheme: MouseScheme,
    pub pick_mode_on: bool,
    /// The tab editor is on and the press origin is on a tab mark.
    pub press_on_tab: bool,
}

/// THE transition. Pure: no egui context, no session.
pub fn step(state: &mut DragState, s: &InputSample, env: &InputEnv) -> Vec<InputAction>;

/// Apply the camera actions (`Orbit`, `Pan`, `Zoom`); the frame and the
/// tests call it. Every other action is left to the caller.
pub fn apply_camera(actions: &[InputAction], camera: &mut OrbitCamera);
```

The transition table. "Intent" is `classify(env.scheme, button, modifiers)`
(section 2).

| From | Sample | To | Actions |
|---|---|---|---|
| `Idle` | `started = Primary`, `press_on_tab` | `Tab` | `TabDrag` |
| `Idle` | `started = b`, intent `Orbit` | `Orbit { pivot }` | `Orbit` with the delta |
| `Idle` | `started = b`, intent `Pan` | `Pan` | `Pan` with the delta |
| `Idle` | `started = b`, intent `Select`, `pick_mode_on` | `BoxSelect` | none |
| `Idle` | `started = b`, intent `Select`, pick mode off | `Idle` | none (section 8, O-1) |
| `Idle` | `clicked` | `Idle` | `Click` (Plain, Shift or Ctrl: section 4) |
| `Idle` | `secondary_clicked` | `Idle` | `SecondaryClick` |
| any | `hovered`, `zoom_delta != 1` | same | `Zoom { factor: zoom_delta }` |
| any | `hovered`, wheel (section 2.3) | same | `Zoom` |
| `Orbit` / `Pan` | `dragging` | same | `Orbit` / `Pan` with the delta |
| `BoxSelect` | `dragging` | `BoxSelect { to }` | none (paint only) |
| `BoxSelect` | `stopped` | `Idle` | `BoxRelease` |
| `Tab` | `dragging` or `stopped` | `Tab`, then `Idle` on `stopped` | `TabDrag` |
| `Cancelled` | `stopped` | `Idle` | none |
| any but `Idle` | `stopped` | `Idle` | as above |

The order inside one frame is: drag start, drag, release, click, wheel.
egui never reports `clicked` and `drag_started` for one press, because a
click-and-drag sense starts a drag only past `max_click_dist`
(`response.rs:400-405`). So a right-click still gives `secondary_clicked`
under the web scheme, and `PickLayer` and the tab delete keep working.

### 1.4 The viewport frame after the change

```rust
// crates/rs_cam_viz/src/app/viewport.rs, draw_viewport (replaces :654-662 and :820-881)
let sample = InputSample::read(ui, &response);
let env = InputEnv {
    scheme: self.controller.state().app_settings.display.mouse_scheme,
    pick_mode_on: self.active_pick().is_some(),
    press_on_tab: self.press_on_tab(&sample), // tab_under(id, press_origin)
};
let actions = {
    let gui = &mut self.controller.state_mut().gui;
    crate::interaction::input::step(&mut gui.selection_ui.drag, &sample, &env)
};
for action in actions {
    match action {
        InputAction::Orbit { dx, dy } => self.camera.orbit(dx, dy),
        InputAction::Pan { dx, dy } => self.camera.pan(dx, dy),
        InputAction::Zoom { .. } => apply_camera(&[action], &mut self.camera),
        InputAction::Click { pos, click } if !dock.closed_popover_on_click =>
            self.handle_viewport_click(pos, click),
        InputAction::Click { .. } => {}
        InputAction::SecondaryClick { pos } => self.handle_secondary_click(pos),
        InputAction::BoxRelease { rect, mode, op } => self.handle_box_release(rect, mode, op),
        InputAction::TabDrag => { let _ = self.handle_tab_drag(ui, &response); }
    }
}
self.paint_box(ui); // section 3.2
```

`zoom_by_factor(f)` needs no change in `render/camera.rs` (the file of D.1
step 3). `apply_camera` calls the present `zoom(delta)` with
`delta = f.ln() / 0.001`; `camera.rs:135` computes `exp(-delta × 0.001)`, so
the factor is exact. The wheel keeps its present delta and feel. Step 3
replaces the zoom target; this design does not.

### 1.5 The tab drag fold

`handle_tab_drag` (`viewport.rs:271-350`) keeps its body. Two changes only:

1. The caller runs it only on `InputAction::TabDrag`. The start test
   (`drag_started_by(Primary)` plus `tab_under(press_origin)`) moves into
   `InputEnv::press_on_tab`, so the claim happens before any orbit, as
   P4_DESIGN.md:1038 asks.
2. Its `bool` return is no longer read; `DragState::Tab` holds the claim.

The content stays in `TabEditMode::drag`. Its three readers do not change:
`handle_tab_drag`, `refresh_layout` (`tab_edit.rs:38` clears it on a new
layout), and `TabMarksKey::of` (`tab_edit.rs:100`). The sentry
`the_tab_editor_writes_one_step_per_act_p4_6` drives the controller through
`AppEvent::EditTabs`, not `viewport.rs`. It stays green when the release
still sends ONE `TabEdit::Move`. The claim order on a primary drag start is
tab first, then box.

## 2. The mouse scheme

### 2.1 Types

```rust
// crates/rs_cam_core/src/settings/mod.rs (beside ToolpathColourDefault)
setting_token! {
    /// The viewport mouse scheme (REPORT A.2b). The GUI maps a button and
    /// the modifiers to an intent with `interaction::input::classify`.
    pub enum MouseScheme {
        /// Right drag orbits, middle drag pans, left selects (Onshape,
        /// Tinkercad, Shapr3D).
        WebCad => ("web_cad", "Right drag orbits (Onshape)"),
        /// Middle drag orbits, Ctrl + middle drag pans, left selects
        /// (SolidWorks).
        DesktopCad => ("desktop_cad", "Middle drag orbits (SolidWorks)"),
    }
}
// DisplaySettings gains `pub mouse_scheme: MouseScheme`, default WebCad;
// reader at :614 `s.token("mouse_scheme", d.mouse_scheme)`;
// writer at :894 `w.put_token("mouse_scheme", s.mouse_scheme, d.mouse_scheme)`.
```

```rust
// crates/rs_cam_viz/src/interaction/input.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent { Orbit, Pan, Select, None }

/// One row of a scheme: the button, the modifier, the intent, the text.
pub struct SchemeRow {
    pub button: egui::PointerButton,
    pub command: bool, // Ctrl (Cmd on macOS): egui `Modifiers::command`
    pub intent: Intent,
    pub keys: &'static str,
    pub does: &'static str,
}

/// THE table. `classify` and the shortcuts window both read it, so the
/// text and the behaviour cannot differ.
pub fn rows(scheme: MouseScheme) -> &'static [SchemeRow];

/// The intent of a drag that starts with `button` and `modifiers`. The
/// first row whose button and `command` match wins; no row gives `None`.
pub fn classify(scheme: MouseScheme, button: egui::PointerButton,
                modifiers: egui::Modifiers) -> Intent;
```

| Scheme | Left | Right | Middle | Ctrl + middle | Ctrl + right |
|---|---|---|---|---|---|
| `WebCad` | Select | Orbit | Pan | Pan | Pan (Onshape) |
| `DesktopCad` | Select | Pan | Orbit | Pan (SolidWorks) | Pan |

Shift and Alt do not change the drag intent. Shift and Ctrl on a left drag
choose the box op (section 4). `DesktopCad` keeps right drag as pan, so a
two-button mouse still pans; this is an open question (O-5).

### 2.2 Where it lives and how it reaches the frame

- The record: `[display] mouse_scheme` in `settings.toml`.
- The row: `token_row::<MouseScheme>(ui, "Mouse", &mut prefs.draft.display.mouse_scheme, ..)`
  in `preferences.rs::draw_display` (after "Toolpath colour", :874-879).
- The frame reads `state.app_settings.display.mouse_scheme` each frame. No
  GUI copy exists, so Apply takes effect on the next frame. The
  `app_settings.rs` header table gains the row "mouse scheme | at once".
- The shortcuts window: `shortcuts_window::draw(ctx, &mut show, scheme)`
  (call site `app.rs:1206`). A new "Viewport mouse" section prints the scheme
  label, then one grid row per `rows(scheme)` entry, then the click rows of
  section 4 and "Wheel / pinch: zoom".

### 2.3 Touchpad

- Pinch: `InputState::zoom_delta()` (egui `input_state/mod.rs:562`). It
  reads a multi-touch gesture, or the Ctrl + scroll that a platform makes
  from a pinch. The frame sends `Zoom { factor }` when it is not 1.0. egui
  removes a Ctrl + scroll from `smooth_scroll_delta` (zoom modifier
  `Modifiers::COMMAND`, `:105-118`), so a pinch does not zoom twice.
- Two-finger scroll: today it zooms, as a wheel does (`viewport.rs:874-881`).
  egui gives `MouseWheel { unit: Point }` for a touchpad and `unit: Line` for
  a notched wheel (`data/input/event.rs:150`). A high-resolution mouse can
  also send `Point`, so the unit does not reliably tell a touchpad. This
  design keeps two-finger scroll as zoom. Two-finger pan is O-6.

## 3. Box select

### 3.1 The threshold

The threshold is egui's `InputOptions::max_click_dist = 6.0` points
(egui 0.36.2 `input_state/mod.rs:113`; `Cargo.lock` pins the version). A press that
moves less is a click; a press that moves more starts a drag
(`response.rs:400`: "If the widget also senses clicks, this will only become
true if the pointer has moved a bit"). rs_cam adds no constant. A future
change of the egui option moves the click and the box together.

### 3.2 The painter

The box paints with `ui.painter_at(viewport_rect)` on the viewport layer,
after the dock. The colours are egui tokens in `ui/tokens.rs`, not render
colours, so F173 (`render/colors.rs:1-8`) does not apply; the sentry
`panels_read_the_token_module_up1` does.

| Mode | Stroke | Fill |
|---|---|---|
| Enclose | `tokens::ACCENT`, 1 pt, solid | new `tokens::BOX_FILL` (ACCENT at alpha 24) |
| Crossing | `tokens::ACCENT`, 1 pt, `egui::Shape::dashed_line` on the four edges | the same fill |

Onshape draws crossing in yellow. The yellow token is `CAUTION`, and
`tokens.rs` keeps the semantic colours for verdicts and the accent for
selection. So this design uses one accent with two line styles (O-4).
Nothing uploads during the drag (F132): the box paints in egui, and the
selection changes on release only.

### 3.3 The rectangle test

```rust
// crates/rs_cam_viz/src/interaction/picking.rs
/// A screen rectangle in viewport-local points.
#[derive(Debug, Clone, Copy)]
pub struct ScreenRect { pub min: [f64; 2], pub max: [f64; 2] }

/// The keys of the items of `toolpath_id`'s model that a box selects.
/// `ctx.screen_x/y` are not read. `kinds` are the chips that are on
/// (`PickFilter::effective_kinds`); a box ignores a chip-off kind (a click
/// turns a chip on; a box does not). Import order, no duplicates.
pub fn pick_items_in_rect(
    ctx: &PickContext<'_>,
    session: &ProjectSession,
    toolpath_id: ToolpathId,
    filter: &PickFilter,
    kinds: &[ItemKind],
    rect: ScreenRect,
    mode: BoxMode,
) -> Vec<rs_cam_core::selection::MatchKey>;
```

The rule for each item that `filter.admits(kinds, item)`:

1. Project each line of `item.shape().pick_lines()` through
   `item_display_point` and `project_to_screen`, as `pick_items` does
   (`picking.rs:331-341`). A face item projects at its own Z.
2. Enclose: every vertex of every line projects and lies in the rectangle.
   A vertex behind the eye fails the item.
3. Crossing: any vertex lies in the rectangle, or any segment crosses a
   rectangle edge. For a face item, the rectangle centre inside the
   projected outer ring (and outside its holes) also counts, so a box inside
   a large face takes the face.
4. The key: an item key; a face key through `face_pick_key` rules (the
   source becomes `ToolpathModel` for the op's own model).

The brief names `FaceGroup.boundary_loops`. This design uses the face
items instead:

- The STEP import builds `FaceItem.rings` from those loops (`brep_items.rs:478`).
- `pick_lines()` already returns the rings for all five kinds (`near.rs:27-39`).
- A face that is not flat has no item, and a click on it refuses
  `FaceNotFlat`, so the box and the click admit the same faces.

An STL face comes from the flat-region cache, as `face_pick_key_at` reads it
(`selection_status.rs:1461-1464`). The function
`rs_cam_core::maps::geom_cache::peek_flat_regions` is public
(`maps/geom_cache.rs:415`, `maps/mod.rs:7`), so viz reads it with no new door. A cold cache gives
no faces and no notice; the face sweep fills it.

Hidden faces (replaced by the operator's live look, 2026-10-29: "we should
only select things visible"): the box keeps an item only when the eye sees
it. `interaction::picking::visible_keys` casts up to 9 rays per item from
the eye to samples on it (a face: its centroid and its inset outer-ring
midpoints; a ring: its inset vertices; a path or a point: its vertices)
through `LoadedModel::ray_pick` (the BVH when cached, else brute force); a
sample is visible when no model meets the ray nearer than the sample less
the STEP chord tolerance, and an item is visible when any sample is. The
remove box takes the same rule. A single click and Tab still reach a hidden
item on purpose. A hidden model selects nothing (`PickContext::visibility`).

### 3.4 The release

```rust
// crates/rs_cam_viz/src/app/viewport.rs
fn handle_box_release(&mut self, rect: egui::Rect, mode: BoxMode, op: BoxOp) {
    let Some(pick) = self.active_pick() else { return };
    let keys = /* pick_items_in_rect(.., pick.kinds, ScreenRect::from(rect - viewport origin), mode) */;
    if keys.is_empty() { return; }
    let (toolpath_id, target) = (pick.toolpath_id, pick.target);
    self.controller.events_mut().push(match op {
        BoxOp::Add => AppEvent::AddPicks { toolpath_id, target, keys },
        BoxOp::Remove => AppEvent::RemovePicks { toolpath_id, target, keys },
    });
}
```

The controller handler asks core for the batch edit and applies it with ONE
`Command::EditSelection` through `apply_selection_edit`, so one undo step
holds the whole box. The edits this design needs from `E_selection_edits.md`:

- `SelectionEdit::AddPicks(Vec<MatchKey>)` and `RemovePicks(Vec<MatchKey>)`.
- O-PICK: `Selection::apply(AddPicks)` on a default selection replaces the
  default with the whole batch, as `AddPick` does for one key.
- A key that the resolution already selects adds nothing (the `AddPick`
  handler drops a `RemovePick` answer today, `events/mod.rs:532-536`).
- Each key passes through `hole_rim_key` (`selection_status.rs:1533`; R-B
  decision 4), then the batch removes duplicates. A box over a hole of a
  Drill or Bore takes the floor circle and the rim; both name one rim pick.
  Recommendation: core does it in the batch edit, so MCP gives the same
  result.
- A batch form of `ClickEdit::BoundaryToSelection` for a boundary target
  whose source is not `Selection`. Until E gives one, the GUI sends no edit
  and gives the notice "Set the boundary source to Selection first."
- The MCP wire type beside the edit (REPORT D.0).

## 4. Modifier rules

| Act | Plain | Shift | Ctrl (Cmd) |
|---|---|---|---|
| Left click on an item, pick mode | toggle (`TogglePick`) | add (`AddPick`) | remove (`RemovePick`, new) |
| Left click on a chip-off item | add, chip on | add, chip on | nothing |
| Left box, pick mode | add (`AddPicks`) | add (`AddPicks`) | remove (`RemovePicks`) |
| Right click on an item, pick mode | add the layer rule (`PickLayer`) | same | same |
| Left click, pick mode off | the present chain | the present chain (tree faces) | the present chain |

A plain box adds; it does not toggle. A toggle over many items gives a
mixed result that the user cannot predict. Onshape and Fusion add on a box.

```rust
// crates/rs_cam_viz/src/ui/pick_bar.rs
pub enum PickClick { Plain, Shift, Ctrl, Secondary }

impl PickClick {
    /// The click of a primary press with `modifiers`. Ctrl wins over Shift.
    pub fn primary(modifiers: egui::Modifiers) -> Self {
        if modifiers.command { Self::Ctrl } else if modifiers.shift { Self::Shift } else { Self::Plain }
    }
}

// click_event gains one arm:
PickClick::Ctrl if chip_off.is_none() => AppEvent::RemovePick { toolpath_id, target, key },
PickClick::Ctrl => /* no event: a chip-off item is not selected */,
```

`click_event` then returns `Option<AppEvent>`. The new
`AppEvent::RemovePick` handler mirrors the `AddPick` handler: it resolves
through `click_edit` and applies the answer only when it is `RemovePick`.
O-PICK holds: `pick_edit` (`selection_status.rs:1517-1519`) still answers
`AddPick` on a default selection, so a first plain click replaces the
default; a Ctrl-click on a default selection sends nothing.
`handle_viewport_click(pos, shift: bool)` becomes
`handle_viewport_click(pos, click: PickClick)`. `PICK_HELP` and `hover_text`
gain "Ctrl-click: remove the item. Drag: box (left to right encloses, right
to left crosses)."

## 5. One pick per frame

```rust
// crates/rs_cam_viz/src/interaction/picking.rs
/// The pick under the pointer, computed once per frame.
#[derive(Debug, Clone, Default)]
pub struct HoverResult {
    pub pos: Option<egui::Pos2>,
    /// Pick mode: the key (or its refusal) and the chip-off kind.
    pub pick_mode: Option<(Result<MatchKey, String>, Option<ItemKind>)>,
    /// The scene pick (`pick` with `drill_pick: None`): the face hover and
    /// the toolpath tooltip read it.
    pub scene: Option<PickHit>,
    /// The ray hit on a model surface, for the orbit pivot (D.1 step 3).
    pub surface_point: Option<[f32; 3]>,
}
```

`RsCamApp::hover_result(&self, response, drag: &DragState) -> HoverResult`
runs once in `draw_viewport`, before the input step:

- `DragState` is not `Idle`: an empty result. No pick runs during an orbit,
  a pan, a box or a tab drag, and the hover clears.
- Pick mode on: `pick_mode_key` once. The scene pick runs only when a model
  has an enriched mesh (the face hover needs it).
- Pick mode off: one scene pick. The two present calls pass the same
  arguments, so one call gives the same hit to both readers.

| Reader | Reads |
|---|---|
| hover ring (`hover_detector`) | `pick_mode` key, else `row_hover` |
| face hover (`last_hover_face`) | `scene` as `ModelFace` |
| pick tooltip (`pick_item_tip`) | `pick_mode` |
| span tooltip (`toolpath_span_tip`) | `scene` as `Toolpath`, pick mode off |
| cursor | the table below |

| Condition (first match) | `CursorIcon` |
|---|---|
| `DragState::Orbit` | `Grabbing` |
| `DragState::Pan` | `Move` |
| `DragState::Tab` | `Grabbing` (present) |
| `DragState::BoxSelect` | `Crosshair` |
| tab editor on, pointer on a tab | `Grab` (present) |
| `pick_mode` is `Some((Ok(_), _))` | `PointingHand` |
| default | `Default` |

## 6. Tests

All in `crates/rs_cam_viz/tests/`. The pattern of
`the_dock_states_read_live_state_g_vpstate.rs:760-800`: `Context::default()`,
`run_ui(raw_input(w, h, events), ..)`, `PointerMoved` and `PointerButton`
over three or more frames, a `click_and_drag` rect allocated by the test.
The geometry tests use the fixture and camera helpers of
`a_click_picks_the_drawn_item_p2_6.rs:203-250`.

| Test file :: function | Proves |
|---|---|
| `the_mouse_scheme_is_one_table_cam8.rs` :: `classify_reads_the_scheme_rows_cam8` | a pure table test: every button × {none, Shift, Ctrl} × scheme against section 2.1 |
| same :: `the_shortcuts_rows_are_the_classify_rows_cam8` | each `rows(scheme)` row classifies to its own intent |
| same :: `a_right_drag_turns_the_camera_and_a_left_drag_does_not_cam8` | `WebCad`: a right drag past 6 pt changes `camera.yaw`; a left drag leaves `yaw` and `target` unchanged |
| same :: `the_scheme_preference_switches_the_mapping_cam8` | `DesktopCad`: a middle drag changes `yaw`; a right drag moves `target` only |
| same :: `a_pinch_zooms_cam8` | an `Event::Zoom(1.25)` changes `distance` by the factor |
| `a_box_picks_every_item_it_holds_sel4.rs` :: `a_left_drag_in_pick_mode_sends_one_add_picks_sel4` | two loops in the box: ONE `AddPicks` with two keys; one undo step |
| same :: `left_to_right_encloses_and_right_to_left_crosses_sel4` | one rectangle over one loop and across a second: enclose gives 1 key, crossing gives 2 |
| same :: `a_short_drag_is_a_click_sel4` | a 5 pt move gives `Click`, no box |
| same :: `ctrl_box_removes_sel4` | `RemovePicks` with the boxed keys |
| same :: `the_chips_filter_the_box_sel4` | circle chip off: no circle key |
| same :: `a_box_selects_through_a_face_sel5` | the box takes a face under another face (states the CPU rule) |
| same :: `escape_cancels_a_box_and_keeps_pick_mode_sel4` | Escape mid-box: release sends nothing; `selection_ui.mode` is still `Pick` |
| same :: `the_first_box_replaces_the_default_o_pick` | default selection, box of two: the selection holds those two picks only |
| `the_tab_editor_writes_one_step_per_act_p4_6.rs` :: new `a_drag_on_a_tab_wins_over_the_box_p4_6` | pick of the tab editor with a press on a mark: `DragState::Tab`, one `TabEdit::Move`, no `AddPicks`, no orbit |
| `a_ctrl_click_removes_the_item_sel3.rs` :: `a_ctrl_click_removes_a_picked_item_sel3` | `click_event` with `Ctrl` gives `RemovePick`; the controller removes one pick |
| `interaction/input.rs` unit tests :: `step_*` | each row of the table in 1.3 |

The `step` tests cover `RsCamApp` wiring only through `step` and the two
helpers. The live look (D.0: a screenshot pair) covers the painted box.

## 7. Commit sequence

Each commit is green alone. `state/CLAUDE.md` is at its 40-line cap; commit 2
adds `viewport_input.rs` to its map, so the implementer shortens another line. The gates: the touched folder sentries,
`cargo test -p rs_cam_viz -q`, the settings sentry, then clippy. Every commit
that touches `interaction/` updates the inline map in
`crates/rs_cam_viz/CLAUDE.md` (sentry `folder_maps_name_real_files_f140`).

| # | Commit | Files |
|---|---|---|
| 1 | One hover pick per frame (`HoverResult`, cursor table) | `interaction/picking.rs`, `app/viewport.rs` |
| 2 | Input state machine, no behaviour change: `step` reproduces left orbit with a temporary `classify` row | `state/viewport_input.rs`, `state/mod.rs`, `state/pick.rs` (`SelectionUi::drag`), `interaction/input.rs`, `interaction/mod.rs`, `app/viewport.rs`, `app/input.rs`, `rs_cam_viz/CLAUDE.md`, `state/CLAUDE.md` |
| 3 | `MouseScheme` setting and row, default `WebCad`; the left button stops orbiting (BREAK); pinch; shortcuts section | `rs_cam_core/src/settings/mod.rs`, `settings/tests.rs`, `settings/CLAUDE.md`, `state/app_settings.rs`, `ui/preferences.rs`, `ui/shortcuts_window.rs`, `app.rs`, `interaction/input.rs`, the `cam8` test |
| 4 | Ctrl-click removes (`PickClick::Ctrl`, `AppEvent::RemovePick`) | `ui/pick_bar.rs`, `ui/mod.rs`, `controller/events/mod.rs`, `app/viewport.rs`, the `sel3` test |
| 5 | (E agent) `AddPicks` / `RemovePicks` in core and the MCP wire type | per `E_selection_edits.md` |
| 6 | Box select: `pick_items_in_rect`, painter, `AppEvent::AddPicks` / `RemovePicks`, Escape cancel | `interaction/picking.rs`, `interaction/input.rs`, `app/viewport.rs`, `app/input.rs`, `ui/tokens.rs` (`BOX_FILL`), `ui/mod.rs`, `controller/events/mod.rs`, `ui/pick_bar.rs` (help text), the `sel4` tests, `FEATURE_CATALOG.md` row |

Invariants kept:

- L-3: `ViewportMode` stays one `Option`. The box runs in a pick mode only;
  `DragState::Tab` runs in the tab editor only; the end of a mode sets
  `DragState::Idle`.
- F173 and UP1: the box colours are `ui/tokens.rs` tokens; no render colour.
- F132: the design adds no upload path; the box paints in egui; the hover upload
  keeps its detectors and fires less often (no pick during a drag).
- `apply(Command)`: every selection change is an `AppEvent` that the
  controller applies as one `Command::EditSelection` (one undo step per act).
- O-PICK: kept for the click (core `pick_edit`) and required for the batch.

What breaks (operator ruling 2026-09-16: state the break, no legacy):

- The left drag no longer orbits. Under `WebCad` a right drag orbits and a
  middle drag pans. The escape is Preferences ▸ Display ▸ Mouse ▸
  `DesktopCad`. No left-orbit variant exists.
- Outside pick mode a left drag does nothing (O-1).
- A settings file with no `mouse_scheme` key starts in `WebCad`. This
  breaks the settings invariant "a missing key is the behaviour before the
  key existed" (`settings/CLAUDE.md`). Commit 3 adds the key to the
  exception line beside `save_cut_trace`.
- `click_event` returns `Option<AppEvent>`; `handle_viewport_click` takes a
  `PickClick`.

## 8. Open questions for the lead

| # | Question | Recommendation |
|---|---|---|
| O-1 | Box select outside pick mode? | No, in the first landing. Outside pick mode a left drag does nothing, and the shortcuts window says so. The state stays `Idle`, so the hover picks continue during that drag. Later: a Shift + left box over STEP faces into `Selection::Faces` (GUI state, no command), the box twin of the present shift-click. |
| O-2 | The default scheme | `WebCad`, as REPORT D.1 step 3 rules. |
| O-3 | The drag threshold | egui `max_click_dist` = 6.0 pt (section 3.1). No rs_cam constant, so no underived default. |
| O-4 | Crossing colour: Onshape yellow? | No. `CAUTION` is a verdict colour. Use `ACCENT` with a dashed line for crossing. |
| O-5 | `DesktopCad` right drag | Pan (Plasticity), so a two-button mouse pans. The alternative is no action (SolidWorks gestures). |
| O-6 | Two-finger touchpad pan | Not in this landing. egui cannot reliably tell a touchpad scroll from a wheel. A later option: Shift + scroll pans, but egui already binds Shift + scroll to horizontal scroll (`horizontal_scroll_modifier`), so that needs its own ruling. |
| O-7 | A right-click under `WebCad` | Keep: it adds the layer rule in pick mode and deletes a tab in the tab editor. egui keeps a right click apart from a right drag by `max_click_dist`. |
| O-8 | A plain box: add or toggle? | Add (section 4). |
| O-9 | Hover during a drag | Clear it (section 5). It removes the picks during the drag and the enriched-mesh rebuild on each face change under an orbit; the cost is at most two rebuilds per drag (start and end). |
| O-10 | Lasso | Not in this landing. `step` gains a `Lasso` state later with the same release path. |
