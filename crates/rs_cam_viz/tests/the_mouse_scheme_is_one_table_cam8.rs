//! CAM-8 (design B, commit B2): the mouse scheme is one table.
//!
//! # The claim
//!
//! 1. `classify` gives the intent of the design B §2.1 table for each scheme,
//!    each button and each of {no modifier, Shift, Ctrl}. Shift does not
//!    change the intent. `LeftOrbit` is today's buttons: the left drag
//!    orbits, the right and the middle drag pan.
//! 2. Each row that `rows(scheme)` prints classifies to its own intent, so
//!    the shortcuts text and the behaviour cannot differ.
//!
//! The drag tests that need an egui `Context` come with B3.

use egui::{Modifiers, PointerButton};
use rs_cam_viz::interaction::input::{Intent, MouseScheme, classify, rows};

const SCHEMES: [MouseScheme; 3] = [
    MouseScheme::LeftOrbit,
    MouseScheme::WebCad,
    MouseScheme::DesktopCad,
];

/// The design B §2.1 table, plus today's buttons as `LeftOrbit`.
fn expected(scheme: MouseScheme, button: PointerButton, ctrl: bool) -> Intent {
    use Intent::{None, Orbit, Pan, Select};
    use PointerButton::{Middle, Primary, Secondary};
    match (scheme, button, ctrl) {
        (MouseScheme::LeftOrbit, Primary, _) => Orbit,
        (MouseScheme::LeftOrbit, Secondary | Middle, _) => Pan,
        (MouseScheme::WebCad | MouseScheme::DesktopCad, Primary, _) => Select,
        (MouseScheme::WebCad, Secondary, false) => Orbit,
        (MouseScheme::WebCad, Secondary, true) => Pan,
        (MouseScheme::WebCad, Middle, _) => Pan,
        (MouseScheme::DesktopCad, Secondary, _) => Pan,
        (MouseScheme::DesktopCad, Middle, false) => Orbit,
        (MouseScheme::DesktopCad, Middle, true) => Pan,
        (_, PointerButton::Extra1 | PointerButton::Extra2, _) => None,
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
            for (name, modifiers, ctrl) in [
                ("none", Modifiers::NONE, false),
                ("shift", Modifiers::SHIFT, false),
                ("ctrl", Modifiers::COMMAND, true),
                ("ctrl+shift", Modifiers::COMMAND | Modifiers::SHIFT, true),
            ] {
                assert_eq!(
                    classify(scheme, button, modifiers),
                    expected(scheme, button, ctrl),
                    "{scheme:?} {button:?} {name}"
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 3 * 4 * 4, "every arm ran");
}

#[test]
fn the_shortcuts_rows_are_the_classify_rows_cam8() {
    for scheme in SCHEMES {
        let table = rows(scheme);
        assert!(!table.is_empty(), "{scheme:?} prints no row");
        for row in table {
            let modifiers = if row.command {
                Modifiers::COMMAND
            } else {
                Modifiers::NONE
            };
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
