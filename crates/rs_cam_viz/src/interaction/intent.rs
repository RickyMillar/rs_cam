//! The viewport gesture facade (F4, design F §4.4).
//!
//! [`gesture_events`] is THE translation from a viewport gesture to the
//! events the controller runs. It is pure: no `&mut`, no egui context, no
//! session write. A test drives it with no `RsCamApp`. The viewport calls
//! it and pushes the events; the controller writes the focus and the
//! selection through its doors (`UiCommand::Select`, `EditSelection`).

use rs_cam_core::selection::{MatchKey, SelectionEdit};
use rs_cam_core::session::{ProjectSession, SelectionTarget};

use crate::interaction::picking::PickHit;
use crate::state::Workspace;
use crate::state::pick::{SelectionUi, ViewportMode};
use crate::state::selection::Selection;
use crate::state::viewport_input::BoxOp;
use crate::ui::AppEvent;
use crate::ui::pick_bar::PickClick;
use crate::ui_command::{NoArgs, UiCommand};

/// A key the viewport takes as a gesture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewKey {
    /// End the viewport mode. B6: a first Escape during a box cancels the
    /// box (`SelectionUi::cancel_box`, before this translation runs); the
    /// next one ends the mode.
    Escape,
    /// E-Q6 "select other": in pick mode, the next candidate under the
    /// pointer (`UiCommand::CyclePickCandidate`).
    Tab,
}

/// One viewport gesture.
#[derive(Debug, Clone)]
pub enum ViewportGesture {
    /// A primary click that the tab editor, the pick mode, the drawing
    /// popover and the simulation pick did not take: the scene pick `hit`.
    Click {
        hit: Option<PickHit>,
        click: PickClick,
    },
    /// The release of a box in pick mode: the keys it holds.
    BoxRelease { keys: Vec<MatchKey>, op: BoxOp },
    /// A key.
    Key { key: ViewKey },
}

/// What the translation reads. No field is mutable.
pub struct ViewContext<'a> {
    pub workspace: Workspace,
    pub focus: &'a Selection,
    pub selection_ui: &'a SelectionUi,
    pub session: &'a ProjectSession,
}

/// THE translation from a gesture to the events the controller runs.
///
/// - A tree hit (an obstacle, a pin or the stock, a toolpath) gives
///   `UiCommand::Select`; a click on nothing gives `Select(None)`. It never
///   writes the focus itself.
/// - An item hit in Toolpaths (a drill target) toggles it in the op
///   selection (`TogglePick`). A STEP face hit in Toolpaths toggles its face
///   key in the selection the click edits (the open Pick section, else
///   `SelectionTarget::for_face_pick`); a face with no key gives
///   `PickRefused`.
/// - A box release in pick mode gives ONE `EditToolpathSelection` with
///   `AddPicks` or `RemovePicks`; no key or no pick mode gives nothing.
/// - Escape gives `EndViewportMode`.
///
/// A pick-mode click on an item does not come here: it goes through
/// `pick_bar::click_event`, which also turns a chip on. Round 4: a
/// pick-mode click on no item comes here; then a click on nothing, an item
/// or a model face gives nothing (the pick mode already judged them), a
/// click on another toolpath's path moves the focus (and its pick mode) to
/// it, and the stock, a pin or an obstacle keep their meaning.
pub fn gesture_events(g: &ViewportGesture, cx: &ViewContext<'_>) -> Vec<AppEvent> {
    let select = |selection: Selection| vec![AppEvent::Ui(UiCommand::Select(selection))];
    let picking = matches!(cx.selection_ui.mode, Some(ViewportMode::Pick(_)));
    match g {
        ViewportGesture::Click { hit: None, .. } if picking => Vec::new(),
        ViewportGesture::Click { hit: None, .. } => select(Selection::None),
        ViewportGesture::Click {
            hit: Some(PickHit::Item { .. } | PickHit::ModelFace { .. } | PickHit::MeshFace { .. }),
            ..
        } if picking => Vec::new(),
        ViewportGesture::Click {
            hit: Some(hit),
            click: _,
        } => match (cx.workspace, hit) {
            (
                Workspace::Setup,
                PickHit::Obstacle {
                    setup_id,
                    obstacle_id,
                },
            ) => select(Selection::Obstacle(*setup_id, *obstacle_id)),
            (Workspace::Setup | Workspace::Toolpaths, PickHit::AlignmentPin { .. })
            | (_, PickHit::StockFace { .. }) => select(Selection::Stock),
            (_, PickHit::Toolpath { id, .. }) => select(Selection::Toolpath(*id)),
            (Workspace::Toolpaths, PickHit::Item { toolpath_id, key }) => {
                vec![AppEvent::TogglePick {
                    toolpath_id: *toolpath_id,
                    target: SelectionTarget::Op,
                    key: key.clone(),
                }]
            }
            (Workspace::Toolpaths, PickHit::ModelFace { model_id, face_id }) => {
                let Selection::Toolpath(tp_id) = *cx.focus else {
                    return Vec::new();
                };
                let Some((_, tc)) = cx.session.find_toolpath_config_by_id(tp_id) else {
                    return Vec::new();
                };
                // P4.3 (L-3): the section whose Pick button is on takes the
                // click; with none on, today's rule.
                let target = cx
                    .selection_ui
                    .pick_target(tp_id)
                    .unwrap_or_else(|| SelectionTarget::for_face_pick(tc));
                match cx.session.face_pick_key(tc, *model_id, *face_id) {
                    Ok(key) => vec![AppEvent::TogglePick {
                        toolpath_id: tp_id,
                        target,
                        key,
                    }],
                    Err(refusal) => {
                        vec![AppEvent::PickRefused(format!(
                            "Could not pick the face: {refusal}"
                        ))]
                    }
                }
            }
            _ => Vec::new(),
        },
        ViewportGesture::BoxRelease { keys, op } => {
            let Some(ViewportMode::Pick(mode)) = cx.selection_ui.mode.as_ref() else {
                return Vec::new();
            };
            if keys.is_empty() {
                return Vec::new();
            }
            let edit = match op {
                BoxOp::Add => SelectionEdit::AddPicks(keys.clone()),
                BoxOp::Remove => SelectionEdit::RemovePicks(keys.clone()),
            };
            vec![AppEvent::EditToolpathSelection {
                toolpath_id: mode.toolpath_id,
                target: mode.target,
                edit,
            }]
        }
        ViewportGesture::Key {
            key: ViewKey::Escape,
        } => {
            // B6: a box in flight was cancelled before this runs
            // (`SelectionUi::cancel_box`); this Escape ends the mode.
            if cx.selection_ui.mode.is_some() {
                vec![AppEvent::Ui(UiCommand::EndViewportMode(NoArgs))]
            } else {
                Vec::new()
            }
        }
        ViewportGesture::Key { key: ViewKey::Tab } => {
            if matches!(cx.selection_ui.mode, Some(ViewportMode::Pick(_))) {
                vec![AppEvent::Ui(UiCommand::CyclePickCandidate(NoArgs))]
            } else {
                Vec::new()
            }
        }
    }
}
