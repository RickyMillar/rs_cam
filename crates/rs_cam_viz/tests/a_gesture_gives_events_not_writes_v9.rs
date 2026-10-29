//! F4 (design F §4.4): a viewport gesture gives events, not writes.
//!
//! # The claim
//!
//! `interaction::intent::gesture_events` is pure. For each gesture it gives
//! the events the controller runs:
//!
//! 1. A tree hit (a toolpath, the stock) gives `UiCommand::Select`; a click
//!    on nothing gives `Select(None)` (the miss before F4 wrote
//!    `Selection::None` too).
//! 2. An item hit in Toolpaths gives `TogglePick`.
//! 3. A box release in pick mode gives one `EditToolpathSelection` with
//!    `AddPicks` (or `RemovePicks`); no key gives nothing.
//! 4. Escape in a viewport mode gives `EndViewportMode`; with no mode,
//!    nothing.
//! 6. Round 4 (pick mode engages with the toolpath): in pick mode a click
//!    on nothing, an item or a model face gives nothing (the pick mode
//!    judged them); a click on another toolpath's path gives
//!    `Select(Toolpath)`, which engages its pick mode; the stock keeps its
//!    meaning.
//! 5. Source arm (anchor `gesture_events(`): `app/viewport.rs` holds no
//!    `set_focus(` call. The click path sends `UiCommand::Select` and the
//!    controller writes the focus (the F2 door).

#![expect(
    clippy::panic,
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::Path;

use rs_cam_core::selection::{ItemKind, MatchKey, SelectionEdit, SourceRef};
use rs_cam_core::session::{ProjectSession, SelectionTarget};
use rs_cam_viz::interaction::intent::{ViewContext, ViewKey, ViewportGesture, gesture_events};
use rs_cam_viz::interaction::picking::PickHit;
use rs_cam_viz::state::Workspace;
use rs_cam_viz::state::pick::{PickMode, SelectionUi};
use rs_cam_viz::state::selection::Selection;
use rs_cam_viz::state::viewport_input::BoxOp;
use rs_cam_viz::ui::AppEvent;
use rs_cam_viz::ui::pick_bar::PickClick;
use rs_cam_viz::ui_command::UiCommand;

fn key(x: f64) -> MatchKey {
    MatchKey {
        source: SourceRef::ToolpathModel,
        kind: ItemKind::Circle,
        layer: Some("H".into()),
        anchor: [x, 0.0],
        measure: 5.0,
        brep: None,
        z: None,
    }
}

fn events(g: &ViewportGesture, selection_ui: &SelectionUi) -> Vec<AppEvent> {
    let session = ProjectSession::new_empty();
    gesture_events(
        g,
        &ViewContext {
            workspace: Workspace::Toolpaths,
            focus: &Selection::None,
            selection_ui,
            session: &session,
        },
    )
}

fn click(hit: Option<PickHit>) -> ViewportGesture {
    ViewportGesture::Click {
        hit,
        click: PickClick::Plain,
    }
}

#[test]
fn a_gesture_gives_events_not_writes_v9() {
    let none = SelectionUi::default();
    let tp = rs_cam_core::ToolpathId::from_raw(4);

    // 1. Tree hits and the miss.
    match events(
        &click(Some(PickHit::Toolpath {
            id: tp,
            move_index: 0,
        })),
        &none,
    )
    .as_slice()
    {
        [AppEvent::Ui(UiCommand::Select(Selection::Toolpath(id)))] => assert_eq!(*id, tp),
        other => panic!("a toolpath hit gave {other:?}"),
    }
    assert!(matches!(
        events(
            &click(Some(PickHit::StockFace {
                face_normal: [0.0, 0.0, 1.0]
            })),
            &none
        )
        .as_slice(),
        [AppEvent::Ui(UiCommand::Select(Selection::Stock))]
    ));
    assert!(matches!(
        events(&click(None), &none).as_slice(),
        [AppEvent::Ui(UiCommand::Select(Selection::None))]
    ));

    // 2. An item hit toggles it in the op selection.
    match events(
        &click(Some(PickHit::Item {
            toolpath_id: tp,
            key: key(1.0),
        })),
        &none,
    )
    .as_slice()
    {
        [
            AppEvent::TogglePick {
                toolpath_id,
                target: SelectionTarget::Op,
                key: k,
            },
        ] => {
            assert_eq!(*toolpath_id, tp);
            assert_eq!(*k, key(1.0));
        }
        other => panic!("an item hit gave {other:?}"),
    }

    // 3. A box release in pick mode.
    let mut picking = SelectionUi::default();
    picking.set_pick(Some(PickMode::new(
        tp,
        SelectionTarget::Op,
        rs_cam_core::compute::catalog::ItemUse::Regions,
    )));

    // 6. Round 4: clicks in pick mode that no item took.
    assert!(
        events(&click(None), &picking).is_empty(),
        "a miss changes nothing"
    );
    for hit in [
        PickHit::Item {
            toolpath_id: tp,
            key: key(1.0),
        },
        PickHit::ModelFace {
            model_id: rs_cam_core::ids::ModelId::from_raw(0),
            face_id: rs_cam_core::geometry::enriched_mesh::FaceGroupId(2),
        },
    ] {
        assert!(
            events(&click(Some(hit)), &picking).is_empty(),
            "the pick mode judged the item or face"
        );
    }
    let other = rs_cam_core::ToolpathId::from_raw(9);
    assert!(matches!(
        events(
            &click(Some(PickHit::Toolpath {
                id: other,
                move_index: 0,
            })),
            &picking
        )
        .as_slice(),
        [AppEvent::Ui(UiCommand::Select(Selection::Toolpath(id)))] if *id == other
    ));
    assert!(matches!(
        events(
            &click(Some(PickHit::StockFace {
                face_normal: [0.0, 0.0, 1.0]
            })),
            &picking
        )
        .as_slice(),
        [AppEvent::Ui(UiCommand::Select(Selection::Stock))]
    ));
    let boxed = |op| ViewportGesture::BoxRelease {
        keys: vec![key(1.0), key(2.0)],
        op,
    };
    match events(&boxed(BoxOp::Add), &picking).as_slice() {
        [
            AppEvent::EditToolpathSelection {
                edit: SelectionEdit::AddPicks(keys),
                ..
            },
        ] => assert_eq!(keys.len(), 2),
        other => panic!("a box gave {other:?}"),
    }
    assert!(matches!(
        events(&boxed(BoxOp::Remove), &picking).as_slice(),
        [AppEvent::EditToolpathSelection {
            edit: SelectionEdit::RemovePicks(_),
            ..
        }]
    ));
    let empty = ViewportGesture::BoxRelease {
        keys: Vec::new(),
        op: BoxOp::Add,
    };
    assert!(
        events(&empty, &picking).is_empty(),
        "an empty box sends nothing"
    );
    assert!(
        events(&boxed(BoxOp::Add), &none).is_empty(),
        "no pick mode, no box edit"
    );

    // 4. Escape.
    let escape = ViewportGesture::Key {
        key: ViewKey::Escape,
    };
    assert!(matches!(
        events(&escape, &picking).as_slice(),
        [AppEvent::Ui(UiCommand::EndViewportMode(_))]
    ));
    assert!(events(&escape, &none).is_empty(), "no mode, no event");
}

#[test]
fn the_viewport_click_path_writes_no_focus_v9() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app/viewport.rs");
    let text = std::fs::read_to_string(&path).expect("read viewport.rs");
    let code: String = text
        .lines()
        .map(|line| line.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        code.contains("gesture_events("),
        "non-vacuity: viewport.rs no longer calls gesture_events"
    );
    assert!(
        !code.contains("set_focus("),
        "viewport.rs writes the focus; the click path must send UiCommand::Select"
    );
}
