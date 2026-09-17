//! The semantic trace: which item of the plan the playhead sits inside.
//!
//! The methods resolve the active semantic item, pin one, build a trace
//! target for a span, a hotspot, an annotation or a cut issue, and pick an
//! item with a ray from the viewport. They are an `impl SimulationState`
//! fragment, so every name keeps its path.

use std::collections::HashSet;

use rs_cam_core::geo::{BoundingBox3, P3, V3};
use rs_cam_core::session::ProjectSession;
use rs_cam_core::stock::simulation_cut::{SimulationCutHotspot, SimulationCutIssue};
use rs_cam_core::trace::debug_trace::ToolpathDebugAnnotation;
use rs_cam_core::trace::semantic_trace::ToolpathSemanticItem;

use super::{ActiveSemanticItem, SimulationState, SimulationTraceTarget};
use crate::state::runtime::GuiState;
use crate::state::toolpath::ToolpathId;

impl SimulationState {
    pub fn sync_debug_state(&mut self, gui: &GuiState) {
        let boundaries = self.boundaries().to_vec();
        self.debug.sync_semantic_indexes(gui, &boundaries);
        let boundary_ids: HashSet<_> = boundaries.iter().map(|boundary| boundary.id).collect();
        if self
            .debug
            .focused_hotspot
            .is_some_and(|(toolpath_id, _)| !boundary_ids.contains(&toolpath_id))
        {
            self.debug.focused_hotspot = None;
        }
        if self
            .debug
            .pinned_semantic_item
            .is_some_and(|(toolpath_id, _)| !boundary_ids.contains(&toolpath_id))
        {
            self.debug.pinned_semantic_item = None;
        }
    }

    /// Resolve the hotspot referenced by `debug.focused_hotspot`, if any.
    /// Returns `None` if no hotspot is focused or the index is stale (e.g.
    /// after a re-run produced a different `trace.hotspots`).
    pub fn focused_hotspot_data(&self) -> Option<&SimulationCutHotspot> {
        let (_, hotspot_index) = self.debug.focused_hotspot?;
        self.results
            .as_ref()
            .and_then(|r| r.cut_trace.as_ref())
            .and_then(|trace| trace.hotspots.get(hotspot_index))
    }

    #[allow(clippy::indexing_slicing)] // active_index from active_item_index() bounded by trace.items
    pub(crate) fn playback_semantic_item(&mut self, gui: &GuiState) -> Option<ActiveSemanticItem> {
        let (boundary_index, toolpath_id, local_move) = self.current_local_toolpath_move()?;
        self.sync_debug_state(gui);
        let rt = gui.toolpath_rt.get(&toolpath_id)?;
        let trace = rt.semantic_trace.as_ref()?;
        let index = self.debug.semantic_indexes.get(&toolpath_id)?;
        let active_index = index.active_item_index(trace, local_move)?;
        Some(ActiveSemanticItem {
            toolpath_id,
            boundary_index,
            local_move,
            item: trace.items[active_index].clone(),
            ancestry: index.ancestry(trace, active_index),
        })
    }

    pub(crate) fn semantic_item_by_id(
        &mut self,
        gui: &GuiState,
        toolpath_id: ToolpathId,
        item_id: u64,
    ) -> Option<ActiveSemanticItem> {
        self.sync_debug_state(gui);
        let rt = gui.toolpath_rt.get(&toolpath_id)?;
        let trace = rt.semantic_trace.as_ref()?;
        let index = self.debug.semantic_indexes.get(&toolpath_id)?;
        let item_index = index.item_index_by_id.get(&item_id).copied()?;
        let boundary_index = self
            .boundaries()
            .iter()
            .position(|boundary| boundary.id == toolpath_id)?;
        let item = trace.items.get(item_index)?.clone();
        let local_move = item.move_start.unwrap_or_default();
        Some(ActiveSemanticItem {
            toolpath_id,
            boundary_index,
            local_move,
            item,
            ancestry: index.ancestry(trace, item_index),
        })
    }

    pub fn active_semantic_item(&mut self, gui: &GuiState) -> Option<ActiveSemanticItem> {
        self.sync_debug_state(gui);
        if let Some((toolpath_id, item_id)) = self.debug.pinned_semantic_item
            && let Some(active) = self.semantic_item_by_id(gui, toolpath_id, item_id)
        {
            return Some(active);
        }
        self.playback_semantic_item(gui)
    }

    pub fn pin_semantic_item(&mut self, toolpath_id: ToolpathId, item_id: u64) {
        self.debug.pinned_semantic_item = Some((toolpath_id, item_id));
    }

    pub fn clear_pinned_semantic_item(&mut self) {
        self.debug.pinned_semantic_item = None;
    }

    pub fn active_debug_span(
        &mut self,
        gui: &GuiState,
    ) -> Option<(
        ToolpathId,
        rs_cam_core::trace::debug_trace::ToolpathDebugSpan,
    )> {
        let active = self.active_semantic_item(gui)?;
        let rt = gui.toolpath_rt.get(&active.toolpath_id)?;
        let trace = rt.debug_trace.as_ref()?;
        let span_id = active
            .ancestry
            .iter()
            .rev()
            .find_map(|item| item.debug_span_id)?;
        trace
            .spans
            .iter()
            .find(|span| span.id == span_id)
            .cloned()
            .map(|span| (active.toolpath_id, span))
    }

    pub fn trace_target_for_item(
        &mut self,
        gui: &GuiState,
        toolpath_id: ToolpathId,
        item_id: u64,
        prefer_end: bool,
    ) -> Option<SimulationTraceTarget> {
        let active = self.semantic_item_by_id(gui, toolpath_id, item_id)?;
        let local_move = if prefer_end {
            active.item.move_end.or(active.item.move_start)?
        } else {
            active.item.move_start.or(active.item.move_end)?
        };
        Some(SimulationTraceTarget {
            toolpath_id,
            move_index: self.global_move_for_local(toolpath_id, local_move)?,
            semantic_item_id: Some(item_id),
            debug_span_id: active
                .ancestry
                .iter()
                .rev()
                .find_map(|item| item.debug_span_id),
        })
    }

    pub(crate) fn trace_target_for_span(
        &mut self,
        gui: &GuiState,
        toolpath_id: ToolpathId,
        span_id: u64,
        prefer_end: bool,
    ) -> Option<SimulationTraceTarget> {
        let rt = gui.toolpath_rt.get(&toolpath_id)?;
        let debug_trace = rt.debug_trace.as_ref()?;
        let span = debug_trace.spans.iter().find(|span| span.id == span_id)?;
        if let (Some(move_start), Some(move_end)) = (span.move_start, span.move_end) {
            return Some(SimulationTraceTarget {
                toolpath_id,
                move_index: self.global_move_for_local(
                    toolpath_id,
                    if prefer_end { move_end } else { move_start },
                )?,
                semantic_item_id: rt.semantic_trace.as_ref().and_then(|trace| {
                    trace
                        .items
                        .iter()
                        .find(|item| item.debug_span_id == Some(span_id))
                        .map(|item| item.id)
                }),
                debug_span_id: Some(span_id),
            });
        }

        let semantic_item_id = rt.semantic_trace.as_ref().and_then(|trace| {
            trace
                .items
                .iter()
                .find(|item| item.debug_span_id == Some(span_id))
                .map(|item| item.id)
        })?;
        self.trace_target_for_item(gui, toolpath_id, semantic_item_id, prefer_end)
    }

    pub fn trace_target_for_hotspot(
        &mut self,
        gui: &GuiState,
        toolpath_id: ToolpathId,
        hotspot_index: usize,
    ) -> Option<SimulationTraceTarget> {
        let rt = gui.toolpath_rt.get(&toolpath_id)?;
        let debug_trace = rt.debug_trace.as_ref()?;
        let hotspot = debug_trace.hotspots.get(hotspot_index)?.clone();
        if let Some(item_id) = hotspot.semantic_item_id {
            return self.trace_target_for_item(gui, toolpath_id, item_id, false);
        }
        if let (Some(move_start), Some(_)) = (hotspot.move_start, hotspot.move_end) {
            return Some(SimulationTraceTarget {
                toolpath_id,
                move_index: self.global_move_for_local(toolpath_id, move_start)?,
                semantic_item_id: None,
                debug_span_id: hotspot.representative_span_id,
            });
        }
        hotspot
            .representative_span_id
            .and_then(|span_id| self.trace_target_for_span(gui, toolpath_id, span_id, false))
    }

    pub fn trace_target_for_annotation(
        &self,
        toolpath_id: ToolpathId,
        annotation: &ToolpathDebugAnnotation,
    ) -> Option<SimulationTraceTarget> {
        Some(SimulationTraceTarget {
            toolpath_id,
            move_index: self.global_move_for_local(toolpath_id, annotation.move_index)?,
            semantic_item_id: None,
            debug_span_id: None,
        })
    }

    pub fn trace_target_for_cut_issue(
        &mut self,
        issue: &SimulationCutIssue,
    ) -> Option<SimulationTraceTarget> {
        let toolpath_id = issue.toolpath_id;
        Some(SimulationTraceTarget {
            toolpath_id,
            move_index: self.global_move_for_local(toolpath_id, issue.move_index)?,
            semantic_item_id: issue.semantic_item_id,
            debug_span_id: None,
        })
    }

    pub(crate) fn current_debug_annotation_with_index(
        &self,
        gui: &GuiState,
    ) -> Option<(ToolpathId, usize, ToolpathDebugAnnotation)> {
        let (_, toolpath_id, local_move) = self.current_local_toolpath_move()?;
        let rt = gui.toolpath_rt.get(&toolpath_id)?;
        let trace = rt.debug_trace.as_ref()?;
        trace
            .annotations
            .iter()
            .enumerate()
            .rev()
            .find(|(_, annotation)| annotation.move_index <= local_move)
            .map(|(index, annotation)| (toolpath_id, index, annotation.clone()))
    }

    pub fn current_debug_annotation(
        &self,
        gui: &GuiState,
    ) -> Option<(ToolpathId, ToolpathDebugAnnotation)> {
        self.current_debug_annotation_with_index(gui)
            .map(|(toolpath_id, _, annotation)| (toolpath_id, annotation))
    }

    pub fn semantic_item_bbox_in_simulation(
        &self,
        session: &ProjectSession,
        toolpath_id: ToolpathId,
        item: &ToolpathSemanticItem,
    ) -> Option<BoundingBox3> {
        let xy = item.xy_bbox?;
        let z_min = item.z_min?;
        let z_max = item.z_max?;
        let local_corners = [
            P3::new(xy.min_x, xy.min_y, z_min),
            P3::new(xy.max_x, xy.min_y, z_min),
            P3::new(xy.max_x, xy.max_y, z_min),
            P3::new(xy.min_x, xy.max_y, z_min),
            P3::new(xy.min_x, xy.min_y, z_max),
            P3::new(xy.max_x, xy.min_y, z_max),
            P3::new(xy.max_x, xy.max_y, z_max),
            P3::new(xy.min_x, xy.max_y, z_max),
        ];
        let setup = session
            .setup_of_toolpath_id(toolpath_id)
            .and_then(|idx| session.list_setups().get(idx));
        Some(BoundingBox3::from_points(local_corners.into_iter().map(
            |corner| {
                setup.map_or(corner, |s| {
                    session.inverse_transform_point_from_setup(corner, s.face_up, s.z_rotation)
                })
            },
        )))
    }

    #[allow(clippy::indexing_slicing)] // item_index from enumerate(), bounded by trace.items
    pub fn pick_semantic_item_with_ray(
        &mut self,
        gui: &GuiState,
        session: &ProjectSession,
        origin: &P3,
        dir: &V3,
    ) -> Option<SimulationTraceTarget> {
        self.sync_debug_state(gui);
        let mut best_hit: Option<(f64, usize, usize, ToolpathId, u64)> = None;

        for boundary in self.boundaries().to_vec() {
            let Some(rt) = gui.toolpath_rt.get(&boundary.id) else {
                continue;
            };
            let Some(trace) = rt.semantic_trace.as_ref() else {
                continue;
            };
            let Some(index) = self.debug.semantic_indexes.get(&boundary.id) else {
                continue;
            };

            for (item_index, item) in trace.items.iter().enumerate() {
                if item.move_start.is_none() || item.move_end.is_none() {
                    continue;
                }
                let Some(bbox) = self.semantic_item_bbox_in_simulation(session, boundary.id, item)
                else {
                    continue;
                };
                let Some(t) = bbox.ray_intersect(origin, dir) else {
                    continue;
                };
                let depth = index.depths[item_index];
                let move_span = item
                    .move_end
                    .unwrap_or(usize::MAX)
                    .saturating_sub(item.move_start.unwrap_or(0));
                let candidate = (t, depth, move_span, boundary.id, item.id);
                let replace = match best_hit {
                    None => true,
                    Some(current) => {
                        candidate.0 < current.0 - 1e-6
                            || ((candidate.0 - current.0).abs() <= 1e-6
                                && (candidate.1 > current.1
                                    || (candidate.1 == current.1 && candidate.2 < current.2)))
                    }
                };
                if replace {
                    best_hit = Some(candidate);
                }
            }
        }

        let (_, _, _, toolpath_id, item_id) = best_hit?;
        self.trace_target_for_item(gui, toolpath_id, item_id, false)
    }
}
