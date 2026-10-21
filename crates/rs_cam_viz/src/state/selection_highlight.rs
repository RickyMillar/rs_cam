//! The selection highlight of the viewport (geometry selection P4.4).
//!
//! The viewport colours each drawing item of the selected toolpath by its
//! state in the `selection_status` report (the memo that generation also
//! reads). This module answers which report the viewport draws and when
//! the upload must run again. It resolves nothing itself.
//!
//! Lead ruling L-9: only the selected toolpath is highlighted (WP27), with
//! the foreign items of its selection.

use std::sync::Arc;

use rs_cam_core::compute::config::BoundarySource;
use rs_cam_core::session::{SelectionReport, SelectionTarget, ToolpathConfig};

use super::AppState;
use super::toolpath::ToolpathId;

/// The toolpath whose drawing the viewport draws: the vector source of the
/// polygon upload (`viewport::vector_source_toolpath`).
pub fn highlight_toolpath(state: &AppState) -> Option<&ToolpathConfig> {
    let id = super::viewport::vector_source_toolpath(state)?;
    state
        .session()
        .find_toolpath_config_by_id(id)
        .map(|(_, tc)| tc)
}

/// The selection the highlight shows for `tc`: the op selection when the op
/// holds one, else the boundary selection (an op with no selection slot,
/// P4.1). The same target a face click edits
/// (`SelectionTarget::for_face_pick`).
pub fn highlight_target(tc: &ToolpathConfig) -> SelectionTarget {
    SelectionTarget::for_face_pick(tc)
}

/// The report the highlight draws: the toolpath, the target and the memo's
/// `Arc`. `None` when no toolpath is drawn or the target does not resolve
/// (a boundary whose source is not a selection).
pub fn highlight_report(
    state: &AppState,
) -> Option<(ToolpathId, SelectionTarget, Arc<SelectionReport>)> {
    let tc = highlight_toolpath(state)?;
    let target = highlight_target(tc);
    let report = state.session().selection_status(tc, target).ok()?;
    Some((tc.id, target, report))
}

/// The boundary selection report of the drawn toolpath, for the
/// `boundary_outline` overlay. `None` unless its boundary is on and its
/// source is a selection.
pub fn boundary_report(state: &AppState) -> Option<Arc<SelectionReport>> {
    let tc = highlight_toolpath(state)?;
    if !tc.boundary.enabled || !matches!(tc.boundary.source, BoundarySource::Selection { .. }) {
        return None;
    }
    state
        .session()
        .selection_status(tc, SelectionTarget::Boundary)
        .ok()
}

/// Why the selected toolpath has no highlight, or `None` when it has one.
/// The `selection_states` row prints it.
pub fn highlight_refusal(state: &AppState) -> Option<&'static str> {
    let Some(tc) = highlight_toolpath(state) else {
        return Some("select a toolpath that reads a drawing");
    };
    let has_drawing = state
        .session()
        .models()
        .iter()
        .find(|m| m.id == tc.model_id)
        .is_some_and(|m| m.items.has_drawing());
    if !has_drawing {
        return Some("the selected toolpath reads no drawing");
    }
    if highlight_report(state).is_none() {
        return Some("the selected toolpath holds no selection");
    }
    None
}

/// The upload trigger of the highlight: the last report `Arc` that the
/// upload drew. A report is immutable and the memo gives a new `Arc` for
/// each build, so a change of pointer is a change of the states. The
/// detector holds the `Arc` itself, so the address cannot be reused by a
/// later report (no ABA).
#[derive(Debug, Default)]
pub struct ReportDetector {
    last: Option<Arc<SelectionReport>>,
    boundary: Option<Arc<SelectionReport>>,
}

impl ReportDetector {
    /// Compare the reports of this frame with the last ones. `true` (once)
    /// when either changed; the caller then asks for one upload.
    pub fn changed(
        &mut self,
        now: Option<Arc<SelectionReport>>,
        boundary: Option<Arc<SelectionReport>>,
    ) -> bool {
        let same = |a: &Option<Arc<SelectionReport>>, b: &Option<Arc<SelectionReport>>| match (a, b)
        {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            (Some(_), None) | (None, Some(_)) => false,
        };
        if same(&self.last, &now) && same(&self.boundary, &boundary) {
            return false;
        }
        self.last = now;
        self.boundary = boundary;
        true
    }

    /// Read this frame's reports from `state` and compare them
    /// ([`Self::changed`]).
    pub fn changed_in(&mut self, state: &AppState) -> bool {
        let now = highlight_report(state).map(|(_, _, report)| report);
        let boundary = boundary_report(state);
        self.changed(now, boundary)
    }
}

/// The upload trigger of the pick hover (geometry selection P4.5): the item
/// under the pointer that the hover buffer last drew. The `last_hover_face`
/// pattern (V13): the viewport asks for one hover upload on a CHANGE of the
/// hovered item only, never per frame, and the hover buffer is its own
/// small buffer, so a pointer move never rebuilds the drawing buffer.
#[derive(Debug, Default)]
pub struct HoverDetector {
    last: Option<rs_cam_core::selection::MatchKey>,
}

impl HoverDetector {
    /// Compare the hovered item of this frame with the last one. `true`
    /// (once) when it changed; the caller then uploads the hover buffer.
    pub fn changed(&mut self, now: Option<rs_cam_core::selection::MatchKey>) -> bool {
        if self.last == now {
            return false;
        }
        self.last = now;
        true
    }

    /// The item the hover buffer draws.
    pub fn hovered(&self) -> Option<&rs_cam_core::selection::MatchKey> {
        self.last.as_ref()
    }
}

/// For each note of `report`, the keys of the items that "Show me" frames.
///
/// - A note of a kind the op does not cut (`NoteCause::KindNotServed`): the
///   `Skipped` items of the note's kind and layer, from `item_states` (the
///   toolpath model `model`) and from `foreign` (P4.4).
/// - A face at the top (`NoteCause::AtTheTop`, P3.7): the one face that the
///   note names (`note.first`). Each such note names one face, so "Show me"
///   frames that face only, not every skipped face of its layer (P3.8).
/// - A circle that is not of the tool size (`NoteCause::HoleFit`, P6.5):
///   the one circle that the note names.
///
/// The keys carry the source of the item.
pub fn note_item_keys(
    model: Option<&rs_cam_core::session::LoadedModel>,
    report: &SelectionReport,
) -> Vec<Vec<rs_cam_core::selection::MatchKey>> {
    use rs_cam_core::selection::NoteCause;
    use rs_cam_core::session::ItemState;
    report
        .notes
        .iter()
        .map(|note| match note.cause {
            NoteCause::AtTheTop { .. } | NoteCause::HoleFit { .. } => {
                vec![note.first.key.clone()]
            }
            NoteCause::KindNotServed => {
                let own = model.into_iter().flat_map(|m| {
                    m.items
                        .iter()
                        .zip(&report.item_states)
                        .filter(|(item, state)| {
                            **state == ItemState::Skipped
                                && item.kind() == note.kind
                                && item.layer() == note.layer.as_deref()
                        })
                        .map(|(item, _)| item.key().clone())
                });
                let foreign = report
                    .foreign
                    .iter()
                    .filter(|(key, state)| {
                        *state == ItemState::Skipped
                            && key.kind == note.kind
                            && key.layer == note.layer
                    })
                    .map(|(key, _)| key.clone());
                own.chain(foreign).collect()
            }
        })
        .collect()
}
