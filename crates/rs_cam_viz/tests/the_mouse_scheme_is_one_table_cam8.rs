//! CAM-8 (designs B §2.1 and §6, B3): the mouse scheme is one table.
//!
//! # The claim
//!
//! 1. `classify` gives the intent of the table for each scheme, each button
//!    and each of {no modifier, Shift, Ctrl, Alt}. The default scheme is
//!    `DesktopCad` (operator ruling 2026-10-29): middle orbits, Ctrl +
//!    middle and right pan. `WebCad`: right orbits, middle, Ctrl + middle
//!    and Ctrl + right pan. Both: left selects (Ctrl + left removes in a
//!    box), Alt + left orbits and Alt + right pans (the laptop rows). Shift
//!    does not change the intent.
//! 2. Each row that `rows(scheme)` prints classifies to its own intent, so
//!    the shortcuts text and the behaviour cannot differ.
//! 3. With a real egui `Context`: a drag of each button runs through
//!    `InputSample::read` (the frame's read of the response), `step` and
//!    `apply_camera`. The viewport frame is on `RsCamApp`, which a test
//!    cannot build, so the drive stops at those three calls, the ones the
//!    frame makes, and a `CameraState` takes the actions.
//! 4. A pinch (`Event::Zoom(1.25)`) changes the distance by 1 / 1.25.

use egui::{Modifiers, PointerButton};
use rs_cam_core::settings::MouseScheme;
use rs_cam_viz::interaction::input::{
    InputEnv, InputSample, Intent, apply_camera, classify, rows, step,
};
use rs_cam_viz::state::camera::CameraState;
use rs_cam_viz::state::viewport_input::DragState;

const SCHEMES: [MouseScheme; 2] = [MouseScheme::DesktopCad, MouseScheme::WebCad];

/// The table (operator ruling 2026-10-29).
fn expected(scheme: MouseScheme, button: PointerButton, ctrl: bool, alt: bool) -> Intent {
    use Intent::{None, Orbit, Pan, Select};
    use PointerButton::{Middle, Primary, Secondary};
    match (scheme, button, ctrl, alt) {
        (_, Primary, _, false) => Select,
        (_, Primary, false, true) => Orbit,
        (_, Secondary, false, true) => Pan,
        (_, _, true, true) => None,
        (MouseScheme::DesktopCad, Middle, false, false) => Orbit,
        (MouseScheme::DesktopCad, Middle, true, false) => Pan,
        (MouseScheme::DesktopCad, Secondary, false, false) => Pan,
        (MouseScheme::DesktopCad, Secondary, true, false) => None,
        (MouseScheme::WebCad, Secondary, false, false) => Orbit,
        (MouseScheme::WebCad, Secondary, true, false) => Pan,
        (MouseScheme::WebCad, Middle, _, false) => Pan,
        (_, Middle, _, true) => None,
        (_, PointerButton::Extra1 | PointerButton::Extra2, _, _) => None,
    }
}

#[test]
fn classify_reads_the_scheme_rows_cam8() {
    let mut checked = 0;
    for scheme in SCHEMES {
        for button in [
            PointerButton::Primary,
            PointerButton::Secondary,
            PointerButton::Middle,
            PointerButton::Extra1,
        ] {
            for (name, modifiers, ctrl, alt) in [
                ("none", Modifiers::NONE, false, false),
                ("shift", Modifiers::SHIFT, false, false),
                ("ctrl", Modifiers::COMMAND, true, false),
                (
                    "ctrl+shift",
                    Modifiers::COMMAND | Modifiers::SHIFT,
                    true,
                    false,
                ),
                ("alt", Modifiers::ALT, false, true),
                ("alt+shift", Modifiers::ALT | Modifiers::SHIFT, false, true),
            ] {
                assert_eq!(
                    classify(scheme, button, modifiers),
                    expected(scheme, button, ctrl, alt),
                    "{scheme:?} {button:?} {name}"
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 2 * 4 * 6, "every arm ran");
    assert_eq!(
        rs_cam_core::settings::AppSettings::default()
            .display
            .mouse_scheme,
        MouseScheme::DesktopCad,
        "the default scheme"
    );
}

#[test]
fn the_shortcuts_rows_are_the_classify_rows_cam8() {
    for scheme in SCHEMES {
        let table = rows(scheme);
        assert!(!table.is_empty(), "{scheme:?} prints no row");
        for row in table {
            let mut modifiers = Modifiers::NONE;
            modifiers.command = row.command;
            modifiers.ctrl = row.command;
            modifiers.alt = row.alt;
            assert_eq!(
                classify(scheme, row.button, modifiers),
                row.intent,
                "{scheme:?}: the row \"{}\" ({}) does not classify to its own intent",
                row.keys,
                row.does
            );
            assert!(
                row.keys.contains("drag") && !row.does.is_empty(),
                "{scheme:?}: a row prints no text"
            );
        }
    }
}

// ── the drag drive through a real egui Context ─────────────────────────

const SCREEN: egui::Vec2 = egui::Vec2::new(800.0, 600.0);
const START: egui::Pos2 = egui::Pos2::new(400.0, 300.0);

/// The camera after a drag of `button` with `modifiers` from `START`, 60 pt
/// right and 20 pt down in six frames. Each frame reads the response of a
/// full-screen `click_and_drag` area (the viewport's sense) through
/// `InputSample::read`, runs `step` and applies the camera actions.
fn drag(scheme: MouseScheme, button: PointerButton, modifiers: Modifiers) -> CameraState {
    let ctx = egui::Context::default();
    let mut camera = CameraState::default();
    let mut drag = DragState::Idle;
    let env = InputEnv {
        scheme,
        pick_mode_on: false,
        press_on_tab: false,
        drag_anchor: None,
    };
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN);
    // egui 0.36 keeps the modifiers from `Event::ModifiersChanged`.
    let mut frames: Vec<Vec<egui::Event>> = vec![
        vec![
            egui::Event::ModifiersChanged(modifiers),
            egui::Event::PointerMoved(START),
        ],
        vec![egui::Event::PointerButton {
            pos: START,
            button,
            pressed: true,
            modifiers,
        }],
    ];
    for k in 1..=6 {
        let k = k as f32;
        frames.push(vec![egui::Event::PointerMoved(
            START + egui::vec2(10.0 * k, 10.0 * k / 3.0),
        )]);
    }
    let end = START + egui::vec2(60.0, 20.0);
    frames.push(vec![egui::Event::PointerButton {
        pos: end,
        button,
        pressed: false,
        modifiers,
    }]);
    frames.push(Vec::new());
    for events in frames {
        let input = egui::RawInput {
            screen_rect: Some(rect),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            let (_, response) = ui.allocate_exact_size(SCREEN, egui::Sense::click_and_drag());
            let sample = InputSample::read(ui, &response);
            let actions = step(&mut drag, &sample, &env);
            apply_camera(&actions, &mut camera, rect);
        });
        out.textures_delta.clear();
    }
    camera
}

fn turned(camera: &CameraState) -> bool {
    let start = CameraState::default().pose().pose();
    let now = camera.pose().pose();
    now.yaw != start.yaw || now.pitch != start.pitch
}

fn moved(camera: &CameraState) -> bool {
    CameraState::default().pose().target != camera.pose().target
}

#[test]
fn a_middle_drag_turns_the_camera_and_a_left_drag_does_not_cam8() {
    let middle = drag(
        MouseScheme::DesktopCad,
        PointerButton::Middle,
        Modifiers::NONE,
    );
    assert!(turned(&middle), "DesktopCad: a middle drag orbits");
    let left = drag(
        MouseScheme::DesktopCad,
        PointerButton::Primary,
        Modifiers::NONE,
    );
    assert!(
        !turned(&left) && !moved(&left),
        "outside pick mode a left drag leaves the camera alone"
    );
    let right = drag(
        MouseScheme::DesktopCad,
        PointerButton::Secondary,
        Modifiers::NONE,
    );
    assert!(
        !turned(&right) && moved(&right),
        "DesktopCad: a right drag pans"
    );
}

#[test]
fn the_scheme_preference_switches_the_mapping_cam8() {
    let right = drag(
        MouseScheme::WebCad,
        PointerButton::Secondary,
        Modifiers::NONE,
    );
    assert!(turned(&right), "WebCad: a right drag orbits");
    let middle = drag(MouseScheme::WebCad, PointerButton::Middle, Modifiers::NONE);
    assert!(
        !turned(&middle) && moved(&middle),
        "WebCad: a middle drag moves the target only"
    );
}

#[test]
fn alt_left_orbits_on_a_laptop_cam8() {
    for scheme in SCHEMES {
        let alt_left = drag(scheme, PointerButton::Primary, Modifiers::ALT);
        assert!(turned(&alt_left), "{scheme:?}: Alt + left drag orbits");
        let alt_right = drag(scheme, PointerButton::Secondary, Modifiers::ALT);
        assert!(
            !turned(&alt_right) && moved(&alt_right),
            "{scheme:?}: Alt + right drag pans"
        );
    }
}

#[test]
fn a_pinch_zooms_cam8() {
    let ctx = egui::Context::default();
    let mut camera = CameraState::default();
    let start = camera.pose().distance;
    let mut drag = DragState::Idle;
    let env = InputEnv {
        scheme: MouseScheme::DesktopCad,
        pick_mode_on: false,
        press_on_tab: false,
        drag_anchor: None,
    };
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN);
    let mut zoomed = false;
    for events in [
        vec![egui::Event::PointerMoved(START)],
        vec![egui::Event::Zoom(1.25)],
        Vec::new(),
    ] {
        let input = egui::RawInput {
            screen_rect: Some(rect),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            let (_, response) = ui.allocate_exact_size(SCREEN, egui::Sense::click_and_drag());
            let sample = InputSample::read(ui, &response);
            let actions = step(&mut drag, &sample, &env);
            zoomed |= !actions.is_empty();
            apply_camera(&actions, &mut camera, rect);
        });
        out.textures_delta.clear();
    }
    assert!(zoomed, "the pinch gave a zoom action");
    let ratio = camera.pose().distance / start;
    assert!(
        (ratio - 1.0 / 1.25).abs() < 1e-4,
        "the pinch changed the distance by {ratio}, not 1 / 1.25"
    );
}
