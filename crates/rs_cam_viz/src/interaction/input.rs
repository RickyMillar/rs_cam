//! The viewport input layer (design B, commit B2).
//!
//! [`InputSample::read`] reads the viewport `Response` and the input state
//! once per frame. [`step`] is THE transition: a pure function of the
//! gesture, the sample and the environment, with no egui context and no
//! session. It returns the actions; the frame applies them, and
//! [`apply_camera`] applies the camera ones. A test drives `step` and
//! `apply_camera` without a frame.
//!
//! The mouse scheme is one table ([`rows`]). [`classify`] reads it, so a
//! printed row and the behaviour cannot differ.

use crate::state::camera::CameraState;
use crate::state::viewport_input::{BoxMode, BoxOp, DragState};
use egui::PointerButton;
use rs_cam_core::settings::MouseScheme;

/// The buttons that [`InputSample::read`] tests, in order.
const BUTTONS: [PointerButton; 5] = [
    PointerButton::Primary,
    PointerButton::Secondary,
    PointerButton::Middle,
    PointerButton::Extra1,
    PointerButton::Extra2,
];

/// The raw wheel step range. A track pad and a notched wheel both give a
/// step in this range, so both zoom at a usable rate.
const WHEEL_STEP_RANGE: (f32, f32) = (1.0, 120.0);

/// What the viewport `Response` and the input state say this frame.
#[derive(Debug, Clone, PartialEq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each bool is one separate egui answer of the frame; they are not a state"
)]
pub struct InputSample {
    /// The button whose drag started this frame (the first match).
    pub started: Option<PointerButton>,
    /// The button that drags this frame (the first match).
    pub dragging: Option<PointerButton>,
    /// The drag ended this frame.
    pub stopped: bool,
    /// The pointer move of the drag this frame, in points.
    pub delta: egui::Vec2,
    /// Where the button went down.
    pub press_origin: Option<egui::Pos2>,
    /// The pointer of an interaction (`interact_pointer_pos`).
    pub pointer: Option<egui::Pos2>,
    /// The pointer over the viewport (`hover_pos`): the zoom point.
    pub hover_pos: Option<egui::Pos2>,
    pub modifiers: egui::Modifiers,
    pub clicked: bool,
    pub secondary_clicked: bool,
    /// The layer-aware hover answer: false while a window above the
    /// viewport owns the pointer (G-SCROLLLAYER).
    pub hovered: bool,
    /// The pinch factor (`InputState::zoom_delta`); 1.0 is no pinch.
    pub zoom_delta: f32,
    /// The wheel move, in points. egui removes a Ctrl + scroll from it and
    /// gives it to `zoom_delta`, so one gesture never zooms twice.
    pub scroll: egui::Vec2,
}

impl InputSample {
    /// Read the sample of this frame from `ui` and the viewport `response`.
    pub fn read(ui: &egui::Ui, response: &egui::Response) -> Self {
        let (press_origin, hover_pos, modifiers, zoom_delta, scroll) = ui.input(|i| {
            (
                i.pointer.press_origin(),
                i.pointer.hover_pos(),
                i.modifiers,
                i.zoom_delta(),
                i.smooth_scroll_delta,
            )
        });
        let hovered = response.hovered();
        // The wheel and the pinch zoom only while the viewport is hovered
        // (layer-aware). The hover read is beside the wheel read on purpose:
        // the sentry `the_wheel_belongs_to_the_top_layer_g_scrolllayer`
        // holds it.
        Self {
            started: BUTTONS.into_iter().find(|b| response.drag_started_by(*b)),
            dragging: BUTTONS.into_iter().find(|b| response.dragged_by(*b)),
            stopped: response.drag_stopped(),
            delta: response.drag_delta(),
            press_origin,
            pointer: response.interact_pointer_pos(),
            hover_pos,
            modifiers,
            clicked: response.clicked(),
            secondary_clicked: response.secondary_clicked(),
            hovered,
            zoom_delta,
            scroll,
        }
    }
}

/// What the frame does after the transition.
#[derive(Debug, Clone, PartialEq)]
pub enum InputAction {
    /// Turn the camera by a drag of `(dx, dy)` points about `pivot`, or
    /// about the target when `pivot` is `None`.
    Orbit {
        dx: f32,
        dy: f32,
        pivot: Option<[f32; 3]>,
    },
    /// Pan the camera by a drag of `(dx, dy)` points. A point at the camera
    /// depth `grab_depth` (default: the target depth) stays under the cursor.
    Pan {
        dx: f32,
        dy: f32,
        grab_depth: Option<f32>,
    },
    /// Zoom by the distance ratio `factor` toward the point under `at`
    /// (screen points), or toward the target when `at` is `None`.
    Zoom { factor: f32, at: Option<egui::Pos2> },
    /// Hand the click to the click chain (`handle_viewport_click`) with
    /// the modifiers of the press. `ui::pick_bar::PickClick::primary` is the
    /// one translation of the modifiers to a pick click (B4).
    Click {
        pos: egui::Pos2,
        modifiers: egui::Modifiers,
    },
    /// Hand the right-click to the tab delete, then the layer pick.
    SecondaryClick { pos: egui::Pos2 },
    /// The release of a box: the rectangle and its rule.
    BoxRelease {
        rect: egui::Rect,
        mode: BoxMode,
        op: BoxOp,
    },
    /// The tab editor handles this frame (`handle_tab_drag`).
    TabDrag,
}

/// The world point under the press origin at a drag start (A C3a), and its
/// camera depth `(point - eye) · forward`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DragAnchor {
    pub point: [f32; 3],
    pub depth: f32,
}

/// The facts the transition needs and cannot read from the sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InputEnv {
    pub scheme: MouseScheme,
    pub pick_mode_on: bool,
    /// The tab editor is on and the press origin is on a tab mark.
    pub press_on_tab: bool,
    /// The press origin is on the orientation gizmo (`on_gizmo`). A left
    /// drag from there orbits about the target in every scheme.
    pub press_on_gizmo: bool,
    /// The point under the press origin. The frame computes it only on the
    /// frame a drag starts; `None` on every other frame, and when the
    /// pointer ray finds no point (the camera then uses the target).
    pub drag_anchor: Option<DragAnchor>,
}

/// The orientation gizmo disc in the viewport `rect`: the centre and the
/// radius. The gizmo drawing and the press test read this one disc.
pub fn gizmo_disc(rect: egui::Rect) -> (egui::Pos2, f32) {
    const SIZE: f32 = 50.0;
    const MARGIN: f32 = 10.0;
    let centre = egui::pos2(
        rect.max.x - MARGIN - SIZE * 0.5,
        rect.min.y + MARGIN + SIZE * 0.5,
    );
    (centre, SIZE * 0.5)
}

/// The screen point `pos` is on the orientation gizmo disc of `rect`.
pub fn on_gizmo(rect: egui::Rect, pos: egui::Pos2) -> bool {
    let (centre, radius) = gizmo_disc(rect);
    pos.distance(centre) <= radius
}

/// THE transition. The order in one frame is: drag start, drag, release,
/// click, wheel.
pub fn step(state: &mut DragState, s: &InputSample, env: &InputEnv) -> Vec<InputAction> {
    let mut actions = Vec::new();
    let (dx, dy) = (s.delta.x, s.delta.y);
    if let Some(button) = s.started {
        // A start begins a new gesture. A gesture whose release the frame
        // did not see does not hold the new one back.
        *state = DragState::Idle;
        if button == PointerButton::Primary && env.press_on_tab {
            *state = DragState::Tab;
            actions.push(InputAction::TabDrag);
        } else if button == PointerButton::Primary && env.press_on_gizmo {
            // The gizmo orbits about the target, as a middle drag with no
            // point under the press origin does.
            *state = DragState::Orbit { pivot: None };
            actions.push(InputAction::Orbit {
                dx,
                dy,
                pivot: None,
            });
        } else {
            let intent = classify(env.scheme, button, s.modifiers);
            match intent {
                Intent::Orbit => {
                    let pivot = env.drag_anchor.map(|a| a.point);
                    *state = DragState::Orbit { pivot };
                    actions.push(InputAction::Orbit { dx, dy, pivot });
                }
                Intent::Pan => {
                    let grab_depth = env.drag_anchor.map(|a| a.depth);
                    *state = DragState::Pan { grab_depth };
                    actions.push(InputAction::Pan { dx, dy, grab_depth });
                }
                Intent::Select if env.pick_mode_on => {
                    if let Some(anchor) = s.press_origin {
                        let op = if s.modifiers.command {
                            BoxOp::Remove
                        } else {
                            BoxOp::Add
                        };
                        *state = DragState::BoxSelect {
                            anchor,
                            to: s.pointer.unwrap_or(anchor),
                            op,
                        };
                    }
                }
                Intent::Select | Intent::None => {}
            }
        }
    } else if s.dragging.is_some() {
        match state {
            DragState::Orbit { pivot } => actions.push(InputAction::Orbit {
                dx,
                dy,
                pivot: *pivot,
            }),
            DragState::Pan { grab_depth } => actions.push(InputAction::Pan {
                dx,
                dy,
                grab_depth: *grab_depth,
            }),
            DragState::BoxSelect { to, .. } => {
                if let Some(p) = s.pointer {
                    *to = p;
                }
            }
            DragState::Tab => actions.push(InputAction::TabDrag),
            DragState::Idle | DragState::Cancelled => {}
        }
    } else if !s.stopped && *state != DragState::Idle {
        // No drag, no release: the frame lost the release. Go back to idle.
        *state = DragState::Idle;
    }

    if s.stopped {
        match std::mem::take(state) {
            DragState::BoxSelect { anchor, to, op } => actions.push(InputAction::BoxRelease {
                rect: egui::Rect::from_two_pos(anchor, to),
                mode: DragState::box_mode(anchor, to),
                op,
            }),
            DragState::Tab => actions.push(InputAction::TabDrag),
            DragState::Idle
            | DragState::Orbit { .. }
            | DragState::Pan { .. }
            | DragState::Cancelled => {}
        }
    }

    if s.clicked
        && let Some(pos) = s.pointer
    {
        actions.push(InputAction::Click {
            pos,
            modifiers: s.modifiers,
        });
    }
    if s.secondary_clicked
        && let Some(pos) = s.pointer
    {
        actions.push(InputAction::SecondaryClick { pos });
    }

    if s.hovered {
        // A pinch apart (zoom_delta > 1) moves the camera closer.
        if s.zoom_delta != 1.0 && s.zoom_delta > 0.0 {
            actions.push(InputAction::Zoom {
                factor: 1.0 / s.zoom_delta,
                at: s.hover_pos,
            });
        }
        let raw = s.scroll.y;
        if raw != 0.0 {
            // The sign gives the direction; the magnitude is clamped so a
            // track pad and a mouse wheel both zoom at a usable rate.
            let magnitude = raw.abs().clamp(WHEEL_STEP_RANGE.0, WHEEL_STEP_RANGE.1);
            let scroll = raw.signum() * magnitude;
            actions.push(InputAction::Zoom {
                factor: (-scroll * crate::render::camera::ZOOM_PER_WHEEL).exp(),
                at: s.hover_pos,
            });
        }
    }
    actions
}

/// Apply the camera actions (`Orbit`, `Pan`, `Zoom`) to `camera` in the
/// viewport `rect`. Every other action is left to the caller.
pub fn apply_camera(actions: &[InputAction], camera: &mut CameraState, rect: egui::Rect) {
    for action in actions {
        match *action {
            InputAction::Orbit { dx, dy, pivot } => {
                // A C3a: the pivot is the point under the press origin;
                // without one, the target.
                let pivot = pivot.map_or(camera.pose().target, |[x, y, z]| {
                    nalgebra::Point3::new(x, y, z)
                });
                camera.orbit_about(pivot, dx, dy);
            }
            InputAction::Pan { dx, dy, grab_depth } => {
                // The pan step follows the viewport height and the grab
                // depth, so the grabbed point stays under the cursor.
                camera.pan_pixels(dx, dy, rect.height(), grab_depth);
            }
            InputAction::Zoom { factor, at } => {
                // Toward the point under the cursor at the target depth;
                // toward the target without a pointer position.
                let point = at
                    .and_then(|pos| {
                        camera.pose().point_at_target_depth(
                            pos.x - rect.min.x,
                            pos.y - rect.min.y,
                            rect.width(),
                            rect.height(),
                        )
                    })
                    .unwrap_or(camera.pose().target);
                camera.zoom_toward(point, factor);
            }
            InputAction::Click { .. }
            | InputAction::SecondaryClick { .. }
            | InputAction::BoxRelease { .. }
            | InputAction::TabDrag => {}
        }
    }
}

// ── the mouse scheme ───────────────────────────────────────────────────
//
// The scheme is the core setting `settings::MouseScheme` (`[display]
// mouse_scheme`, default `DesktopCad`, operator ruling 2026-10-29). The
// frame reads it from `AppSettings` each frame.

/// What a drag does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    Orbit,
    Pan,
    Select,
    None,
}

/// One row of a scheme: the button, the modifiers, the intent, the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchemeRow {
    pub button: PointerButton,
    /// Ctrl (Cmd on macOS): egui `Modifiers::command`.
    pub command: bool,
    /// Alt (Option on macOS): egui `Modifiers::alt`. The laptop rows.
    pub alt: bool,
    pub intent: Intent,
    pub keys: &'static str,
    pub does: &'static str,
}

const fn row(
    button: PointerButton,
    command: bool,
    alt: bool,
    intent: Intent,
    keys: &'static str,
    does: &'static str,
) -> SchemeRow {
    SchemeRow {
        button,
        command,
        alt,
        intent,
        keys,
        does,
    }
}

/// The rows both schemes share: the left button selects (a box in pick
/// mode; Ctrl removes, B §4), and the laptop rows Alt + left orbits and
/// Alt + right pans (Alt never collides with the pick modifiers Shift and
/// Ctrl).
const LEFT_SELECT: SchemeRow = row(
    PointerButton::Primary,
    false,
    false,
    Intent::Select,
    "Left drag",
    "Box select (pick mode)",
);
const CTRL_LEFT_SELECT: SchemeRow = row(
    PointerButton::Primary,
    true,
    false,
    Intent::Select,
    "Ctrl + left drag",
    "Box remove (pick mode)",
);
const ALT_LEFT_ORBIT: SchemeRow = row(
    PointerButton::Primary,
    false,
    true,
    Intent::Orbit,
    "Alt + left drag",
    "Orbit (laptop)",
);
const ALT_RIGHT_PAN: SchemeRow = row(
    PointerButton::Secondary,
    false,
    true,
    Intent::Pan,
    "Alt + right drag",
    "Pan (laptop)",
);

const DESKTOP_CAD: [SchemeRow; 7] = [
    row(
        PointerButton::Middle,
        false,
        false,
        Intent::Orbit,
        "Middle drag",
        "Orbit",
    ),
    row(
        PointerButton::Middle,
        true,
        false,
        Intent::Pan,
        "Ctrl + middle drag",
        "Pan",
    ),
    row(
        PointerButton::Secondary,
        false,
        false,
        Intent::Pan,
        "Right drag",
        "Pan",
    ),
    ALT_LEFT_ORBIT,
    ALT_RIGHT_PAN,
    LEFT_SELECT,
    CTRL_LEFT_SELECT,
];

const WEB_CAD: [SchemeRow; 8] = [
    row(
        PointerButton::Secondary,
        false,
        false,
        Intent::Orbit,
        "Right drag",
        "Orbit",
    ),
    row(
        PointerButton::Middle,
        false,
        false,
        Intent::Pan,
        "Middle drag",
        "Pan",
    ),
    row(
        PointerButton::Middle,
        true,
        false,
        Intent::Pan,
        "Ctrl + middle drag",
        "Pan",
    ),
    row(
        PointerButton::Secondary,
        true,
        false,
        Intent::Pan,
        "Ctrl + right drag",
        "Pan",
    ),
    ALT_LEFT_ORBIT,
    ALT_RIGHT_PAN,
    LEFT_SELECT,
    CTRL_LEFT_SELECT,
];

/// THE table of `scheme`. `classify` and the shortcuts window both read
/// it, so the text and the behaviour cannot differ.
pub fn rows(scheme: MouseScheme) -> &'static [SchemeRow] {
    match scheme {
        MouseScheme::DesktopCad => &DESKTOP_CAD,
        MouseScheme::WebCad => &WEB_CAD,
    }
}

/// The intent of a drag that starts with `button` and `modifiers`. The
/// first row whose button, `command` and `alt` match wins; no row gives
/// `None`. Shift does not change the intent (it chooses nothing for a
/// drag).
pub fn classify(scheme: MouseScheme, button: PointerButton, modifiers: egui::Modifiers) -> Intent {
    rows(scheme)
        .iter()
        .find(|r| r.button == button && r.command == modifiers.command && r.alt == modifiers.alt)
        .map_or(Intent::None, |r| r.intent)
}

#[cfg(test)]
#[expect(
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;

    const ENV: InputEnv = InputEnv {
        scheme: MouseScheme::DesktopCad,
        pick_mode_on: false,
        press_on_tab: false,
        press_on_gizmo: false,
        drag_anchor: None,
    };

    fn sample() -> InputSample {
        InputSample {
            started: None,
            dragging: None,
            stopped: false,
            delta: egui::Vec2::ZERO,
            press_origin: None,
            pointer: None,
            hover_pos: None,
            modifiers: egui::Modifiers::NONE,
            clicked: false,
            secondary_clicked: false,
            hovered: false,
            zoom_delta: 1.0,
            scroll: egui::Vec2::ZERO,
        }
    }

    fn drag(button: PointerButton, start: bool) -> InputSample {
        InputSample {
            started: start.then_some(button),
            dragging: Some(button),
            delta: egui::vec2(3.0, -2.0),
            press_origin: Some(egui::pos2(100.0, 100.0)),
            pointer: Some(egui::pos2(103.0, 98.0)),
            ..sample()
        }
    }

    #[test]
    fn step_a_start_on_a_tab_is_a_tab_drag() {
        let mut state = DragState::Idle;
        let env = InputEnv {
            press_on_tab: true,
            press_on_gizmo: false,
            ..ENV
        };
        let actions = step(&mut state, &drag(PointerButton::Primary, true), &env);
        assert_eq!(state, DragState::Tab);
        assert_eq!(actions, vec![InputAction::TabDrag]);
    }

    #[test]
    fn step_an_orbit_intent_orbits_with_the_delta() {
        let mut state = DragState::Idle;
        let actions = step(&mut state, &drag(PointerButton::Middle, true), &ENV);
        assert_eq!(state, DragState::Orbit { pivot: None });
        assert_eq!(
            actions,
            vec![InputAction::Orbit {
                dx: 3.0,
                dy: -2.0,
                pivot: None
            }]
        );
    }

    #[test]
    fn step_a_pan_intent_pans_with_the_delta() {
        let button = PointerButton::Secondary;
        let mut state = DragState::Idle;
        let actions = step(&mut state, &drag(button, true), &ENV);
        assert_eq!(state, DragState::Pan { grab_depth: None });
        assert_eq!(
            actions,
            vec![InputAction::Pan {
                dx: 3.0,
                dy: -2.0,
                grab_depth: None
            }]
        );
    }

    #[test]
    fn step_a_select_intent_in_pick_mode_starts_a_box() {
        let mut state = DragState::Idle;
        let env = InputEnv {
            scheme: MouseScheme::WebCad,
            pick_mode_on: true,
            press_on_tab: false,
            press_on_gizmo: false,
            drag_anchor: None,
        };
        let actions = step(&mut state, &drag(PointerButton::Primary, true), &env);
        assert_eq!(
            state,
            DragState::BoxSelect {
                anchor: egui::pos2(100.0, 100.0),
                to: egui::pos2(103.0, 98.0),
                op: BoxOp::Add,
            }
        );
        assert!(actions.is_empty(), "{actions:?}");
    }

    #[test]
    fn step_a_select_intent_without_pick_mode_does_nothing() {
        let mut state = DragState::Idle;
        let env = InputEnv {
            scheme: MouseScheme::WebCad,
            ..ENV
        };
        let actions = step(&mut state, &drag(PointerButton::Primary, true), &env);
        assert_eq!(state, DragState::Idle);
        assert!(actions.is_empty(), "{actions:?}");
        // The rest of that drag does nothing either.
        let actions = step(&mut state, &drag(PointerButton::Primary, false), &env);
        assert_eq!(state, DragState::Idle);
        assert!(actions.is_empty(), "{actions:?}");
    }

    #[test]
    fn step_a_click_is_a_click() {
        for modifiers in [
            egui::Modifiers::NONE,
            egui::Modifiers::SHIFT,
            egui::Modifiers::COMMAND,
        ] {
            let mut state = DragState::Idle;
            let s = InputSample {
                clicked: true,
                pointer: Some(egui::pos2(5.0, 6.0)),
                modifiers,
                ..sample()
            };
            let actions = step(&mut state, &s, &ENV);
            assert_eq!(state, DragState::Idle);
            assert_eq!(
                actions,
                vec![InputAction::Click {
                    pos: egui::pos2(5.0, 6.0),
                    modifiers
                }],
                "{modifiers:?}"
            );
        }
    }

    #[test]
    fn step_a_secondary_click_is_a_secondary_click() {
        let mut state = DragState::Idle;
        let s = InputSample {
            secondary_clicked: true,
            pointer: Some(egui::pos2(5.0, 6.0)),
            ..sample()
        };
        let actions = step(&mut state, &s, &ENV);
        assert_eq!(state, DragState::Idle);
        assert_eq!(
            actions,
            vec![InputAction::SecondaryClick {
                pos: egui::pos2(5.0, 6.0)
            }]
        );
    }

    #[test]
    fn step_a_pinch_zooms_only_when_hovered() {
        let at = Some(egui::pos2(40.0, 30.0));
        let mut state = DragState::Idle;
        let s = InputSample {
            zoom_delta: 1.25,
            hover_pos: at,
            hovered: true,
            ..sample()
        };
        assert_eq!(
            step(&mut state, &s, &ENV),
            vec![InputAction::Zoom {
                factor: 1.0 / 1.25,
                at
            }]
        );
        let s = InputSample {
            hovered: false,
            ..s
        };
        assert!(step(&mut state, &s, &ENV).is_empty());
    }

    #[test]
    fn step_a_wheel_zooms_by_the_old_ratio_only_when_hovered() {
        let at = Some(egui::pos2(40.0, 30.0));
        let mut state = DragState::Idle;
        for (raw, used) in [(50.0_f32, 50.0_f32), (0.2, 1.0), (-500.0, -120.0)] {
            let s = InputSample {
                scroll: egui::vec2(0.0, raw),
                hover_pos: at,
                hovered: true,
                ..sample()
            };
            // The C1 wheel: k = exp(-scroll * 0.001), the scroll clamped.
            let want = (-used * 0.001_f32).exp();
            assert_eq!(
                step(&mut state, &s, &ENV),
                vec![InputAction::Zoom { factor: want, at }],
                "raw {raw}"
            );
            let s = InputSample {
                hovered: false,
                ..s
            };
            assert!(step(&mut state, &s, &ENV).is_empty(), "raw {raw}");
        }
    }

    #[test]
    fn step_an_orbit_or_a_pan_drag_sends_the_delta() {
        let pivot = Some([1.0, 2.0, 3.0]);
        let mut state = DragState::Orbit { pivot };
        let actions = step(&mut state, &drag(PointerButton::Primary, false), &ENV);
        assert_eq!(
            actions,
            vec![InputAction::Orbit {
                dx: 3.0,
                dy: -2.0,
                pivot
            }]
        );
        let mut state = DragState::Pan {
            grab_depth: Some(80.0),
        };
        let actions = step(&mut state, &drag(PointerButton::Secondary, false), &ENV);
        assert_eq!(
            actions,
            vec![InputAction::Pan {
                dx: 3.0,
                dy: -2.0,
                grab_depth: Some(80.0)
            }]
        );
        assert_eq!(
            state,
            DragState::Pan {
                grab_depth: Some(80.0)
            }
        );
    }

    #[test]
    fn step_a_box_drag_moves_its_corner_and_sends_nothing() {
        let anchor = egui::pos2(100.0, 100.0);
        let mut state = DragState::BoxSelect {
            anchor,
            to: anchor,
            op: BoxOp::Add,
        };
        let actions = step(&mut state, &drag(PointerButton::Primary, false), &ENV);
        assert!(actions.is_empty(), "{actions:?}");
        assert_eq!(
            state,
            DragState::BoxSelect {
                anchor,
                to: egui::pos2(103.0, 98.0),
                op: BoxOp::Add
            }
        );
    }

    #[test]
    fn step_a_box_release_sends_the_rect_and_its_mode() {
        let anchor = egui::pos2(100.0, 100.0);
        let to = egui::pos2(60.0, 140.0);
        let mut state = DragState::BoxSelect {
            anchor,
            to,
            op: BoxOp::Remove,
        };
        let s = InputSample {
            stopped: true,
            ..sample()
        };
        let actions = step(&mut state, &s, &ENV);
        assert_eq!(state, DragState::Idle);
        assert_eq!(
            actions,
            vec![InputAction::BoxRelease {
                rect: egui::Rect::from_two_pos(anchor, to),
                mode: BoxMode::Crossing,
                op: BoxOp::Remove,
            }]
        );
        assert_eq!(
            DragState::box_mode(anchor, egui::pos2(140.0, 60.0)),
            BoxMode::Enclose
        );
    }

    #[test]
    fn step_a_tab_drag_ends_on_its_release() {
        let mut state = DragState::Tab;
        let actions = step(&mut state, &drag(PointerButton::Primary, false), &ENV);
        assert_eq!(actions, vec![InputAction::TabDrag]);
        assert_eq!(state, DragState::Tab);
        let s = InputSample {
            stopped: true,
            ..sample()
        };
        let actions = step(&mut state, &s, &ENV);
        assert_eq!(actions, vec![InputAction::TabDrag]);
        assert_eq!(state, DragState::Idle);
    }

    #[test]
    fn step_a_cancelled_box_ends_with_no_action() {
        let mut state = DragState::Cancelled;
        let actions = step(&mut state, &drag(PointerButton::Primary, false), &ENV);
        assert!(
            actions.is_empty(),
            "the rest of a cancelled drag: {actions:?}"
        );
        assert_eq!(state, DragState::Cancelled);
        let s = InputSample {
            stopped: true,
            ..sample()
        };
        let actions = step(&mut state, &s, &ENV);
        assert!(actions.is_empty(), "{actions:?}");
        assert_eq!(state, DragState::Idle);
    }

    #[test]
    fn step_a_lost_release_goes_back_to_idle() {
        let mut state = DragState::Pan { grab_depth: None };
        let actions = step(&mut state, &sample(), &ENV);
        assert!(actions.is_empty(), "{actions:?}");
        assert_eq!(state, DragState::Idle);
    }

    #[test]
    fn step_an_orbit_start_records_the_anchor_as_its_pivot() {
        let anchor = DragAnchor {
            point: [4.0, -5.0, 6.0],
            depth: 120.0,
        };
        let env = InputEnv {
            drag_anchor: Some(anchor),
            ..ENV
        };
        let mut state = DragState::Idle;
        let actions = step(&mut state, &drag(PointerButton::Middle, true), &env);
        let pivot = Some(anchor.point);
        assert_eq!(state, DragState::Orbit { pivot });
        assert_eq!(
            actions,
            vec![InputAction::Orbit {
                dx: 3.0,
                dy: -2.0,
                pivot
            }]
        );
        // The later frames keep the pivot of the start, whatever the env.
        let actions = step(&mut state, &drag(PointerButton::Middle, false), &ENV);
        assert_eq!(
            actions,
            vec![InputAction::Orbit {
                dx: 3.0,
                dy: -2.0,
                pivot
            }]
        );
    }

    #[test]
    fn step_a_pan_start_records_the_grab_depth() {
        let anchor = DragAnchor {
            point: [4.0, -5.0, 6.0],
            depth: 120.0,
        };
        let env = InputEnv {
            drag_anchor: Some(anchor),
            ..ENV
        };
        let mut state = DragState::Idle;
        let actions = step(&mut state, &drag(PointerButton::Secondary, true), &env);
        assert_eq!(
            state,
            DragState::Pan {
                grab_depth: Some(120.0)
            }
        );
        assert_eq!(
            actions,
            vec![InputAction::Pan {
                dx: 3.0,
                dy: -2.0,
                grab_depth: Some(120.0)
            }]
        );
        let actions = step(&mut state, &drag(PointerButton::Secondary, false), &ENV);
        assert_eq!(
            actions,
            vec![InputAction::Pan {
                dx: 3.0,
                dy: -2.0,
                grab_depth: Some(120.0)
            }]
        );
    }

    #[test]
    fn apply_camera_with_the_target_is_the_c1_camera() {
        let rect = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(800.0, 600.0));
        let mut camera = CameraState::default();
        let mut want = *camera.pose();
        apply_camera(
            &[
                InputAction::Orbit {
                    dx: 4.0,
                    dy: 2.0,
                    pivot: None,
                },
                InputAction::Pan {
                    dx: 3.0,
                    dy: 1.0,
                    grab_depth: None,
                },
                InputAction::Zoom {
                    factor: 0.9,
                    at: None,
                },
            ],
            &mut camera,
            rect,
        );
        let target = want.target;
        want.orbit_about(target, 4.0, 2.0);
        want.pan_pixels(3.0, 1.0, 600.0, None);
        let target = want.target;
        want.zoom_toward(target, 0.9);
        if *camera.pose() != want {
            panic!(
                "apply_camera added arithmetic: {:?} != {want:?}",
                camera.pose()
            );
        }

        // A pivot off the target and a grab depth off the target depth.
        let pivot = [30.0_f32, -12.0, 5.0];
        let mut camera = CameraState::default();
        let mut want = *camera.pose();
        apply_camera(
            &[
                InputAction::Orbit {
                    dx: -7.0,
                    dy: 3.0,
                    pivot: Some(pivot),
                },
                InputAction::Pan {
                    dx: 5.0,
                    dy: -4.0,
                    grab_depth: Some(150.0),
                },
            ],
            &mut camera,
            rect,
        );
        want.orbit_about(
            nalgebra::Point3::new(pivot[0], pivot[1], pivot[2]),
            -7.0,
            3.0,
        );
        want.pan_pixels(5.0, -4.0, 600.0, Some(150.0));
        if *camera.pose() != want {
            panic!(
                "apply_camera added arithmetic to the pivot orbit: {:?} != {want:?}",
                camera.pose()
            );
        }
    }
}
