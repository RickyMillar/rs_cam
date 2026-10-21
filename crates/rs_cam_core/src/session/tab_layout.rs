//! The tab layout of a Profile and the tab edits of the viewport editor
//! (geometry selection P4.6).
//!
//! [`ProjectSession::tab_layout`] answers the `tab_layout` query: the tabs
//! that generation cuts, as marks in the drawing frame. It runs THE
//! selection resolution, `selection::view`, `dressup::tabs::resolve_hand_tabs`
//! and `dressup::tabs::plan_profile_tabs`, and no kernel. `ProfilePass`
//! cuts from the same plan, so a mark and a lift cannot differ.
//!
//! [`apply_tab_edit`] turns one editor act (add, move, delete) into the new
//! `tab_points`. The GUI writes them with ONE `SetToolpathParam`. The rules:
//!
//! - L-5: hand tabs replace the auto tabs of their loop (P5). So an edit of
//!   a loop that has auto tabs first turns each auto tab of that loop into
//!   a hand tab at its planned point, then makes the edit.
//! - O-4: an edit never deletes the last tab of a loop. `tab_points` cannot
//!   hold "this loop has no tab", so the delete is refused with a text that
//!   names the loop.

use serde::Serialize;

use crate::compute::catalog::OperationConfig;
use crate::compute::operation_configs::CompensationType;
use crate::dressup::tabs::{ProfileTabRule, TabSource, plan_profile_tabs, resolve_hand_tabs};
use crate::geo::P2;
use crate::ops::profile::ProfileParams;
use crate::selection::{ItemRef, MatchKey};

use super::selection_status::SelectionTarget;
use super::{ProjectSession, SessionError, SetupEvalContext, ToolpathConfig};

/// One tab of a Profile, as generation cuts it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TabMark {
    /// The centre of the lift on the cut ring, in the drawing frame (the
    /// frame of the items and of `tab_points`).
    pub at: [f64; 2],
    /// The centre on the outline loop, in the drawing frame: the point a
    /// hand tab at this place stores in `tab_points`.
    pub on_loop: [f64; 2],
    /// The loop the tab holds: the item whose ring is the outline.
    pub loop_key: MatchKey,
    pub source: TabSource,
}

/// The answer of the `tab_layout` query.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TabLayoutAnswer {
    /// The tabs, loop by loop in the order of the op's regions, and in cut
    /// order on each loop.
    pub marks: Vec<TabMark>,
    /// Every loop the Profile cuts, in the order of its regions. A loop
    /// with no tab is here too, so the editor can add the first one.
    pub loops: Vec<MatchKey>,
}

impl ProjectSession {
    /// The tabs that generation cuts for the Profile `tc`, with no kernel
    /// (geometry selection P4.6).
    ///
    /// # Errors
    ///
    /// The refusals of generation, in its order: a missing tool, a lateral
    /// setup with no mesh, the selection refusal (typed, as generation gives
    /// it), the tool refusal of the op, and a hand tab far from every loop
    /// (the text names the point). An op that is not a Profile refuses.
    pub fn tab_layout(&self, tc: &ToolpathConfig) -> Result<TabLayoutAnswer, SessionError> {
        use crate::compute::execute::OperationError;
        let OperationConfig::Profile(cfg) = &tc.operation else {
            return Err(SessionError::InvalidParam(format!(
                "tab_layout reads a Profile; toolpath '{}' is a {}.",
                tc.name,
                tc.operation.op_type().label()
            )));
        };
        let tool = self
            .find_tool_by_raw_id(tc.tool_id)
            .ok_or(SessionError::ToolNotFound(
                crate::compute::tool_config::ToolId(tc.tool_id),
            ))?;
        let setup = self.find_setup_for_toolpath_id(tc.id);
        self.check_lateral_setup_support(setup, &tc.operation)?;
        let ctx = SetupEvalContext::build_for_setup(self, setup);
        let frames = self.resolve_selection_frames(
            tc,
            SelectionTarget::Op,
            &ctx,
            super::selection_status::FaceAccess::Peek,
        )?;
        let op_type = tc.operation.op_type();
        let (items, _notes) =
            crate::selection::view(&frames.setup, tc.operation.capabilities().item_use)
                .map_err(|refusal| SessionError::Selection(refusal.for_op(op_type.label())))?;
        if let Some(text) = op_type.tool_refusal(tool.tool_type.cutter_kind()) {
            return Err(SessionError::OperationFailed(
                OperationError::InvalidTool(text).to_string(),
            ));
        }
        // P6.3: side On reads the lines of the selection (`Curves`), open
        // paths too; Outside and Inside read the closed areas. A tab goes on
        // a closed ring only (L-8).
        let item_use = tc.operation.capabilities().item_use;
        let (lines, regions) = match item_use {
            crate::compute::catalog::ItemUse::Curves => {
                (items.curves().map(|c| c.as_slice()), None)
            }
            _ => {
                let regions = items.regions();
                (regions.map(|r| r.as_slice()), regions)
            }
        };
        let lines = lines
            .filter(|lines| lines.iter().any(|l| l.closed))
            .ok_or_else(|| {
                SessionError::OperationFailed(format!(
                    "{}: the selection gives no closed area.",
                    op_type.label()
                ))
            })?;
        let tool_radius = crate::tool::MillingCutter::radius(&super::session_cutter(tool)?);
        let frame = ctx.local_to_global.as_ref();
        let hand = resolve_hand_tabs(&cfg.tab_points, frame, tool_radius, lines)
            .map_err(|text| SessionError::Operation(OperationError::InvalidParam(text)))?;
        let profile = ProfileParams {
            tool_radius,
            side: cfg.side,
            cut_depth: 0.0,
            feed_rate: 0.0,
            plunge_rate: 0.0,
            safe_z: 0.0,
            climb: cfg.climb,
            compensate_in_controller: cfg.compensation == CompensationType::InControl,
        };
        let plan = plan_profile_tabs(
            lines,
            &ProfileTabRule {
                regions: lines,
                hand: &hand,
                count: cfg.tab_count,
                width: cfg.tab_width,
                profile: &profile,
            },
        );
        // The frame door keeps the order of the items, so the index of an
        // item in the setup half names the same item in the model half. The
        // key of the model half is the key a pick of the item stores.
        // A `Curves` view keeps no item index, so a closed line finds its
        // item by its ring (`curve_item`). An open path is no loop.
        let key_of = |region: usize| {
            let item = match regions {
                Some(regions) => regions.exterior_item(region),
                None => lines
                    .get(region)
                    .filter(|line| line.closed)
                    .and_then(|line| curve_item(frames.setup.items(), line)),
            };
            item.and_then(|i| frames.model.items().get(i))
                .map(|r| r.identity().clone())
        };
        let drawing = |p: P2| {
            let q = frame.map_or(p, |info| info.local_to_drawing(p));
            [q.x, q.y]
        };
        let loops = (0..lines.len()).filter_map(key_of).collect();
        let mut marks = Vec::new();
        for contour in &plan.contours {
            let Some(loop_key) = contour.region.and_then(key_of) else {
                continue;
            };
            marks.extend(contour.tabs.iter().map(|t| TabMark {
                at: drawing(t.at),
                on_loop: drawing(t.on_outline),
                loop_key: loop_key.clone(),
                source: t.source,
            }));
        }
        Ok(TabLayoutAnswer { marks, loops })
    }
}

/// The index in `items` of the item whose ring is the exterior of the
/// closed line `line` of a `Curves` view (P6.3): a loop, a circle ring or
/// the outer ring of a face. The view can reverse a ring to its winding
/// (`Polygon2::ensure_winding`), so a reversed ring matches too.
fn curve_item(
    items: &[crate::selection::ResolvedItem],
    line: &crate::polygon::Polygon2,
) -> Option<usize> {
    let same = |ring: &[P2]| {
        ring == line.exterior.as_slice() || ring.iter().rev().eq(line.exterior.iter())
    };
    items.iter().position(|r| match r.item().shape() {
        crate::selection::Shape::Face(face) => same(&face.rings.exterior),
        shape => shape
            .drawing_ring()
            .is_some_and(|ring| ring.closed && same(&ring.exterior)),
    })
}

/// One act of the viewport tab editor. Every point is in the drawing frame.
#[derive(Debug, Clone, PartialEq)]
pub enum TabEdit {
    /// A new hand tab on the loop `loop_key`, at `at` (a point of that
    /// loop).
    Add { loop_key: MatchKey, at: [f64; 2] },
    /// Move the tab `mark` (an index in `TabLayoutAnswer::marks`) to `to`.
    Move { mark: usize, to: [f64; 2] },
    /// Delete the tab `mark`.
    Delete { mark: usize },
}

/// The `tab_points` after the edit `edit` of the layout `layout`, whose
/// hand tabs are `tab_points` (geometry selection P4.6, rulings L-5 and
/// O-4).
///
/// # Errors
///
/// A mark index that the layout does not hold, or the delete of the last
/// tab of a loop (O-4). The text names the loop.
pub fn apply_tab_edit(
    layout: &TabLayoutAnswer,
    tab_points: &[[f64; 2]],
    edit: &TabEdit,
) -> Result<Vec<[f64; 2]>, String> {
    let mark = |index: usize| {
        layout
            .marks
            .get(index)
            .ok_or_else(|| format!("The layout has no tab {index}."))
    };
    match edit {
        TabEdit::Add { loop_key, at } => {
            let mut points = with_loop_as_hand(layout, tab_points, loop_key, None);
            points.push(*at);
            Ok(points)
        }
        TabEdit::Move { mark: index, to } => {
            let moved = mark(*index)?;
            match moved.source {
                TabSource::Hand(i) => {
                    let mut points = tab_points.to_vec();
                    let slot = points
                        .get_mut(i)
                        .ok_or_else(|| format!("tab_points has no point {i}."))?;
                    *slot = *to;
                    Ok(points)
                }
                TabSource::Auto => Ok(with_loop_as_hand(
                    layout,
                    tab_points,
                    &moved.loop_key,
                    Some((*index, Some(*to))),
                )),
            }
        }
        TabEdit::Delete { mark: index } => {
            let deleted = mark(*index)?;
            let on_loop = layout
                .marks
                .iter()
                .filter(|m| m.loop_key == deleted.loop_key)
                .count();
            if on_loop <= 1 {
                return Err(format!(
                    "The {} keeps its last tab. A loop with no tab cannot be stored, so the \
                     editor does not delete the last tab of a loop. To go back to the automatic \
                     tabs, use Clear on the Hand tabs row.",
                    ItemRef::new(deleted.loop_key.clone())
                ));
            }
            match deleted.source {
                TabSource::Hand(i) => {
                    if i >= tab_points.len() {
                        return Err(format!("tab_points has no point {i}."));
                    }
                    let mut points = tab_points.to_vec();
                    points.remove(i);
                    Ok(points)
                }
                TabSource::Auto => Ok(with_loop_as_hand(
                    layout,
                    tab_points,
                    &deleted.loop_key,
                    Some((*index, None)),
                )),
            }
        }
    }
}

/// `tab_points` with each auto tab of the loop `loop_key` added as a hand
/// tab at its planned point on the loop (ruling L-5). `change` replaces the
/// point of one auto mark (`Some(to)`) or leaves it out (`None`). A loop
/// with no auto tab adds nothing.
fn with_loop_as_hand(
    layout: &TabLayoutAnswer,
    tab_points: &[[f64; 2]],
    loop_key: &MatchKey,
    change: Option<(usize, Option<[f64; 2]>)>,
) -> Vec<[f64; 2]> {
    let mut points = tab_points.to_vec();
    for (index, m) in layout.marks.iter().enumerate() {
        if m.source != TabSource::Auto || &m.loop_key != loop_key {
            continue;
        }
        match change {
            Some((changed, to)) if changed == index => points.extend(to),
            _ => points.push(m.on_loop),
        }
    }
    points
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::selection::ItemKind;

    fn key(x: f64) -> MatchKey {
        MatchKey {
            source: crate::selection::SourceRef::ToolpathModel,
            kind: ItemKind::Loop,
            layer: None,
            anchor: [x, 0.0],
            measure: 10.0,
            brep: None,
        }
    }

    fn mark(loop_x: f64, at: f64, source: TabSource) -> TabMark {
        TabMark {
            at: [at, 1.0],
            on_loop: [at, 0.0],
            loop_key: key(loop_x),
            source,
        }
    }

    /// Loop A has two auto tabs, loop B one hand tab (point 0).
    fn layout() -> TabLayoutAnswer {
        TabLayoutAnswer {
            marks: vec![
                mark(0.0, 1.0, TabSource::Auto),
                mark(0.0, 2.0, TabSource::Auto),
                mark(50.0, 51.0, TabSource::Hand(0)),
            ],
            loops: vec![key(0.0), key(50.0)],
        }
    }

    /// L-5: a move of an auto tab turns every auto tab of its loop into a
    /// hand tab and moves the one; the other loop keeps its point.
    #[test]
    fn an_auto_tab_move_turns_its_loop_to_hand_tabs_l5() {
        let points = apply_tab_edit(
            &layout(),
            &[[51.0, 0.0]],
            &TabEdit::Move {
                mark: 1,
                to: [3.0, 0.0],
            },
        )
        .expect("a move");
        assert_eq!(points, vec![[51.0, 0.0], [1.0, 0.0], [3.0, 0.0]]);
    }

    /// O-4: the last tab of a loop is not deleted, and the text names it.
    #[test]
    fn the_last_tab_of_a_loop_is_not_deleted_o4() {
        let refusal = apply_tab_edit(&layout(), &[[51.0, 0.0]], &TabEdit::Delete { mark: 2 })
            .expect_err("the last tab of loop B");
        assert!(
            refusal.contains("centre X 50.000 Y 0.000"),
            "the text names the loop: {refusal}"
        );
        let points = apply_tab_edit(&layout(), &[[51.0, 0.0]], &TabEdit::Delete { mark: 0 })
            .expect("loop A keeps one tab");
        assert_eq!(points, vec![[51.0, 0.0], [2.0, 0.0]]);
    }

    /// An add on a loop with auto tabs keeps them (L-5); an add on a loop
    /// of hand tabs appends.
    #[test]
    fn an_add_keeps_the_auto_tabs_of_its_loop_l5() {
        let points = apply_tab_edit(
            &layout(),
            &[[51.0, 0.0]],
            &TabEdit::Add {
                loop_key: key(0.0),
                at: [4.0, 0.0],
            },
        )
        .expect("an add");
        assert_eq!(
            points,
            vec![[51.0, 0.0], [1.0, 0.0], [2.0, 0.0], [4.0, 0.0]]
        );
        let points = apply_tab_edit(
            &layout(),
            &[[51.0, 0.0]],
            &TabEdit::Add {
                loop_key: key(50.0),
                at: [55.0, 0.0],
            },
        )
        .expect("an add");
        assert_eq!(points, vec![[51.0, 0.0], [55.0, 0.0]]);
    }
}
