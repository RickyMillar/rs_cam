//! The hand-tab editor of a Profile (geometry selection P4.6): the layout
//! it shows and the upload trigger of its marks.
//!
//! The editor reads the `tab_layout` query (core: the tabs that generation
//! cuts). It reads the query again only when the input revision of the
//! toolpath moves (`ProjectSession::toolpath_revision_at`), never per frame.
//! The marks upload on a change of the layout or of the drag only
//! ([`TabMarksDetector`], the `HoverDetector` pattern).

use rs_cam_core::dressup::tabs::TabSource;
use rs_cam_core::session::TabLayoutAnswer;

use super::AppState;
use super::pick::TabDrag;
use super::selection::Selection;
use super::toolpath::ToolpathId;

/// Read the tab layout of the tab editor again when the input revision of
/// its toolpath moved since the last read. `true` when it read.
pub fn refresh_layout(state: &mut AppState) -> bool {
    let Selection::Toolpath(id) = state.selection else {
        return false;
    };
    let Some((index, _)) = state.session().find_toolpath_config_by_id(id) else {
        return false;
    };
    let revision = state.session().toolpath_revision_at(index);
    let Some(mode) = state.gui.selection_ui.tab_edit(id) else {
        return false;
    };
    if mode.layout.as_ref().is_some_and(|(r, _)| *r == revision) {
        return false;
    }
    let answer = read_layout(state.session(), index);
    if let Some(mode) = state.gui.selection_ui.tab_edit_mut(id) {
        // A new layout numbers its marks again, so a drag of an old mark
        // ends.
        mode.drag = None;
        mode.layout = Some((revision, answer));
    }
    true
}

/// The `tab_layout` query row for the toolpath at `index`: the answer, or
/// the refusal text. The editor and the controller read it here.
pub fn read_layout(
    session: &rs_cam_core::session::ProjectSession,
    index: usize,
) -> Result<TabLayoutAnswer, String> {
    use rs_cam_core::session::{Query, QueryAnswer, TabLayoutArgs};
    match session.query(Query::TabLayout(TabLayoutArgs { index })) {
        Ok(QueryAnswer::TabLayout(answer)) => Ok(answer),
        Ok(other) => Err(format!("the tab_layout read answered {other:?}")),
        Err(e) => Err(e.to_string()),
    }
}

/// The one-line summary of the tab editor bar: "Tabs: 4 auto, 2 hand on 2
/// loops".
#[must_use]
pub fn tab_summary(answer: &TabLayoutAnswer) -> String {
    let hand = answer
        .marks
        .iter()
        .filter(|m| matches!(m.source, TabSource::Hand(_)))
        .count();
    let auto = answer.marks.len() - hand;
    let loops = answer.loops.len();
    format!(
        "Tabs: {auto} auto, {hand} hand on {loops} {}",
        if loops == 1 { "loop" } else { "loops" }
    )
}

/// What the tab marks buffer last drew: the toolpath, the revision of the
/// layout and the drag.
#[derive(Debug, Clone, PartialEq)]
pub struct TabMarksKey {
    pub toolpath_id: ToolpathId,
    pub revision: u64,
    pub drag: Option<TabDrag>,
}

impl TabMarksKey {
    /// The key of this frame: `None` when no tab editor with a layout is
    /// on for the selected toolpath in the Toolpaths workspace.
    #[must_use]
    pub fn of(state: &AppState) -> Option<Self> {
        if state.workspace != super::Workspace::Toolpaths {
            return None;
        }
        let Selection::Toolpath(id) = state.selection else {
            return None;
        };
        let mode = state.gui.selection_ui.tab_edit(id)?;
        let (revision, _) = mode.layout.as_ref()?;
        Some(Self {
            toolpath_id: id,
            revision: *revision,
            drag: mode.drag.clone(),
        })
    }
}

/// The upload trigger of the tab marks (geometry selection P4.6): the
/// marks buffer uploads on a change of the key only, never per frame.
#[derive(Debug, Default)]
pub struct TabMarksDetector {
    last: Option<TabMarksKey>,
}

impl TabMarksDetector {
    /// Compare the key of this frame with the last one. `true` (once) when
    /// it changed; the caller then uploads the marks buffer.
    pub fn changed(&mut self, now: Option<TabMarksKey>) -> bool {
        if self.last == now {
            return false;
        }
        self.last = now;
        true
    }
}
