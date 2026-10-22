//! The per-toolpath line buffers and the height planes.
//!
//! V8: keep the previous pass's per-toolpath buffers, keyed by toolpath id,
//! and hand each back untouched when its key still matches. During
//! `generate_all` exactly one toolpath's `Arc<AnnotatedToolpath>` changes per
//! completion, so each of the eight upload passes an 8-operation generate
//! fires rebuilds one toolpath instead of eight. A selection click flips
//! `selected` on at most two keys, so it rebuilds at most two.
//!
//! The slot is a map keyed by toolpath id, not one slot, so it keeps its own
//! compare; each buffer's key still derives from its [`ToolpathInputs`].

use std::cell::OnceCell;
use std::collections::HashMap;
use std::sync::Arc;

use rs_cam_core::feeds::{AdvancePerToothMm, VendorChiploadBand};
use rs_cam_core::geo::P3;
use rs_cam_core::session::ProjectSession;
use rs_cam_core::stock::simulation_cut::SimulationCutTrace;
use rs_cam_core::tool_load::ToolLoadReport;
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;

use crate::render::RenderResources;
use crate::render::toolpath_render::{self, EntryPreviewConfig, ToolpathGpuData};
use crate::render::upload_cache::{self, SlotInputs};
use crate::state::AppState;
use crate::state::job::ToolConfig;
use crate::state::selection::Selection;
use crate::state::viewport::{SpanKindFilter, ToolpathColorMode};

use super::{Gpu, PassFrame, has_shift, shift_key, translate_annotated};

/// The per-move achieved advance per tooth and the vendor band, per
/// toolpath. Both halves are typed (`VendorChiploadBand` /
/// `AdvancePerToothMm`) so the pairing that produced F-HEATMAP — an
/// arc-mean chip thickness handed to a band comparison — cannot be
/// reassembled here without a compile error.
type AdvanceScans = (
    HashMap<rs_cam_core::ToolpathId, VendorChiploadBand>,
    HashMap<rs_cam_core::ToolpathId, HashMap<usize, AdvancePerToothMm>>,
);

/// What the AdvancePerTooth colouring reads beside the toolpath, once per
/// pass.
pub(super) struct AdvancePass<'a> {
    /// The load report the session memo gives for this trace. Its key holds
    /// the trace identity and the tool, material and operation config the
    /// vendor band is matched from, so it moves on each of them and on no
    /// counter.
    report: Arc<ToolLoadReport>,
    session: &'a ProjectSession,
    trace: Option<&'a SimulationCutTrace>,
    /// V8: built on first use. Both halves are full cut-trace scans (V11);
    /// when every toolpath's key is unchanged there is nothing to colour,
    /// so the scans do not run at all.
    scans: OnceCell<AdvanceScans>,
}

impl AdvancePass<'_> {
    fn scans(&self) -> &AdvanceScans {
        self.scans.get_or_init(|| {
            let bands = super::build_advance_bands(self.session, self.trace);
            let per_move = rs_cam_core::tool_load::display::advance_per_tooth_per_move(self.trace);
            (bands, per_move)
        })
    }
}

/// How a toolpath's cutting moves are coloured, with what each mode reads.
#[derive(Clone, Copy)]
enum Colouring<'a> {
    Normal,
    Engagement,
    AdvancePerTooth(&'a AdvancePass<'a>),
}

/// Everything one toolpath's buffers are built from.
pub(super) struct ToolpathInputs<'a> {
    id: rs_cam_core::ToolpathId,
    annotated: &'a Arc<AnnotatedToolpath>,
    /// The enumeration index, the palette selector of the Normal colouring.
    palette_index: usize,
    selected: bool,
    colouring: Colouring<'a>,
    span_filter: SpanKindFilter,
    /// The identity-setup emission → display translation.
    shift: P3,
    /// The nominal feed of the Engagement colouring.
    feed_rate: f64,
    /// The selected toolpath's overlay inputs, `None` when the overlay is
    /// not drawn. `None` is what makes "deselect" a key change rather than
    /// a silent stale overlay.
    entry_preview: Option<EntryPreviewConfig>,
    tool_profile: Option<&'a ToolConfig>,
}

impl SlotInputs for ToolpathInputs<'_> {
    type Key = upload_cache::ToolpathUploadKey;

    fn key(&self) -> Self::Key {
        let Self {
            // The map key of the slot: the buffer carries it beside its key.
            id: _,
            annotated,
            palette_index,
            selected,
            colouring,
            span_filter,
            shift,
            feed_rate,
            entry_preview,
            tool_profile,
        } = self;
        let (color_mode, advance_source) = match colouring {
            Colouring::Normal => (ToolpathColorMode::Normal, None),
            Colouring::Engagement => (ToolpathColorMode::Engagement, None),
            Colouring::AdvancePerTooth(pass) => {
                let AdvancePass {
                    report,
                    // The report memo keys on the trace and on every session
                    // part the band reads.
                    session: _,
                    trace: _,
                    // Derived from the two above.
                    scans: _,
                } = pass;
                (
                    ToolpathColorMode::AdvancePerTooth,
                    Some(upload_cache::AdvanceSource {
                        report: upload_cache::ArcId::new(report),
                    }),
                )
            }
        };
        upload_cache::ToolpathUploadKey {
            annotated: upload_cache::ArcId::new(annotated),
            palette_index: *palette_index,
            selected: *selected,
            color_mode,
            span_filter: *span_filter,
            shift: shift_key(*shift),
            feed_rate: *feed_rate,
            advance_source,
            entry_preview: entry_preview.clone(),
            tool_profile: tool_profile.cloned(),
        }
    }
}

/// The advance pass of this upload. The caller builds it only in
/// AdvancePerTooth mode.
fn advance_pass(state: &AppState) -> AdvancePass<'_> {
    AdvancePass {
        report: state.simulation.cached_load_report(state.session()),
        session: state.session(),
        trace: state
            .simulation
            .results
            .as_ref()
            .and_then(|r| r.cut_trace.as_deref()),
        scans: OnceCell::new(),
    }
}

/// The inputs of every toolpath the viewport draws, in session order.
fn inputs<'a>(
    state: &'a AppState,
    frame: &PassFrame,
    colouring: Colouring<'a>,
) -> Vec<ToolpathInputs<'a>> {
    let session = state.session();
    let gui = &state.gui;
    let display_shift = frame.display_shift();
    let selected_tp_id = match state.selection {
        Selection::Toolpath(id) => Some(id),
        _ => None,
    };

    // WP27 — which toolpaths the viewport draws. One rule, one function,
    // shared with the click pick (`interaction::picking`): two predicates
    // drift, and the drift is a click that selects geometry the viewport
    // does not draw. The setup filter stays in the loop below, so an id
    // from another setup in this set is skipped there.
    let draw_set = crate::state::viewport::toolpaths_to_draw(
        crate::state::viewport::ToolpathDrawFilter::from_state(state),
        session.toolpath_configs().iter().map(|tc| {
            let rt = gui.toolpath_rt.get(&tc.id);
            (
                tc.id,
                rt.is_none_or(|r| r.visible),
                rt.is_some_and(|r| r.run.result().is_some()),
            )
        }),
    );

    let mut out = Vec::new();
    for (i, tc) in session.toolpath_configs().iter().enumerate() {
        // Find which setup owns this toolpath
        let tp_setup_id = session
            .setup_of_toolpath_id(tc.id)
            .and_then(|idx| session.list_setups().get(idx))
            .map(|sd| sd.id);
        if tp_setup_id != frame.setup_id() {
            continue;
        }

        // Draw only what the WP27 rule named above: the eye button, the
        // "not generated yet" gate, the selection and the show-all dial,
        // in one answer.
        let tp_id = tc.id;
        let Some(result) = gui
            .toolpath_rt
            .get(&tc.id)
            .and_then(|r| r.run.result())
            .filter(|_| draw_set.contains(&tp_id))
        else {
            continue;
        };
        let selected = selected_tp_id == Some(tp_id);

        // Overlay inputs for the selected toolpath, resolved before the
        // key so the key covers them.
        // P3.1: THE heights of the session. A refused height draws no
        // entry preview.
        let entry_config = selected
            .then(|| session.toolpath_heights(tc).ok())
            .flatten()
            .map(|resolved| {
                let resolved = resolved.heights;
                use crate::state::toolpath::DressupEntryStyle;
                let entry_style = match tc.dressups.entry_style {
                    DressupEntryStyle::None => toolpath_render::EntryStyle::None,
                    DressupEntryStyle::Ramp => toolpath_render::EntryStyle::Ramp,
                    DressupEntryStyle::Helix => toolpath_render::EntryStyle::Helix,
                };
                toolpath_render::EntryPreviewConfig {
                    entry_style,
                    ramp_angle_deg: tc.dressups.ramp_angle,
                    // G10 Q6 and D2: the radius the engine emits
                    // (the operator value or 0.3 x D, capped at the
                    // flat bottom). With no tool bound, the rule has
                    // no D and the preview draws no helix.
                    helix_radius: session
                        .tools()
                        .iter()
                        .find(|t| t.id == tc.tool_id)
                        // F029: a tool outside its domain builds no
                        // cutter, so the preview has no rule radius.
                        .and_then(|t| rs_cam_core::compute::cutter::build_cutter(t).ok())
                        .map(|cutter| tc.dressups.helix_radius_for(&cutter).emitted_mm)
                        .or(tc.dressups.helix_radius)
                        .unwrap_or(0.0),
                    helix_pitch: tc.dressups.helix_pitch,
                    lead_in_out: tc.dressups.lead_in_out.is_some(),
                    lead_radius: tc.dressups.lead_in_out.map_or(0.0, |l| l.radius),
                    // Heights resolve in the emission frame; shift
                    // alongside the toolpath the preview rides on.
                    feed_z: resolved.feed_z + display_shift.z,
                    top_z: resolved.top_z + display_shift.z,
                }
            });
        let profile_tool = (selected && state.viewport.show_tool_profile_preview)
            .then(|| session.tools().iter().find(|t| t.id == tc.tool_id))
            .flatten();

        out.push(ToolpathInputs {
            id: tc.id,
            annotated: &result.annotated,
            palette_index: i,
            selected,
            colouring,
            span_filter: state.viewport.span_kind_filter,
            shift: display_shift,
            feed_rate: tc.operation.feed_rate(),
            entry_preview: entry_config,
            tool_profile: profile_tool,
        });
    }
    out
}

/// Refresh every drawn toolpath's buffers.
///
/// Anything left in `previous` at the end of the loop (a toolpath that
/// became invisible, lost the selection, moved to another setup, or was
/// deleted) is dropped with the map, releasing its GPU buffers exactly as
/// the old unconditional `clear()` did.
pub(super) fn upload(
    state: &AppState,
    frame: &PassFrame,
    gpu: &Gpu<'_>,
    resources: &mut RenderResources,
) {
    let mut previous: HashMap<rs_cam_core::ToolpathId, ToolpathGpuData> = resources
        .toolpath_data
        .drain(..)
        .filter_map(|data| data.toolpath_id.map(|id| (id, data)))
        .collect();
    let mut toolpath_builds: u64 = 0;
    let mut toolpath_reuses: u64 = 0;
    let advance;
    let colouring = match state.viewport.toolpath_color_mode {
        ToolpathColorMode::Normal => Colouring::Normal,
        ToolpathColorMode::Engagement => Colouring::Engagement,
        ToolpathColorMode::AdvancePerTooth => {
            advance = advance_pass(state);
            Colouring::AdvancePerTooth(&advance)
        }
    };
    for inputs in inputs(state, frame, colouring) {
        // V8: everything above is cheap; everything below — the
        // `translate_annotated` deep clone (V9), the line buffer build, the
        // entry/profile preview sampling — is not. Reuse the previous pass's
        // buffers when nothing they were built from has moved.
        let key = inputs.key();
        if let Some(reusable) = previous.remove(&inputs.id)
            && reusable.upload_key.as_ref() == Some(&key)
        {
            toolpath_reuses += 1;
            resources.toolpath_data.push(reusable);
            continue;
        }
        toolpath_builds += 1;
        let mut gpu_data = build(inputs, gpu, resources);
        gpu_data.upload_key = Some(key);
        resources.toolpath_data.push(gpu_data);
    }
    resources.upload_stats.toolpath_builds += toolpath_builds;
    resources.upload_stats.toolpath_reuses += toolpath_reuses;
    drop(previous);
}

fn build(
    inputs: ToolpathInputs<'_>,
    gpu: &Gpu<'_>,
    resources: &RenderResources,
) -> ToolpathGpuData {
    let ToolpathInputs {
        id,
        annotated,
        palette_index,
        selected,
        colouring,
        span_filter,
        shift,
        feed_rate,
        entry_preview,
        tool_profile,
    } = inputs;
    // Toolpaths arrive in their emission frame: setup-local for
    // non-identity setups, *world* for identity setups (F-028). The
    // viewport draws zero-rooted local, so identity toolpaths must shift by
    // -stock.origin to land on the mesh (heights/setup-frame audit
    // 2026-06-12, finding 1 — pre-fix they rendered "through the stock
    // floor" on any origin != 0 project).
    let shifted_annotated = has_shift(shift).then(|| translate_annotated(annotated, shift));
    let render_annotated = shifted_annotated.as_ref().unwrap_or(annotated);
    let render_tp = &render_annotated.toolpath;

    let mut gpu_data = match colouring {
        Colouring::Engagement => ToolpathGpuData::from_toolpath_engagement(
            gpu.device,
            &resources.gpu_limits,
            render_tp,
            feed_rate,
        ),
        Colouring::AdvancePerTooth(pass) => {
            let scans = pass.scans();
            let band = scans.0.get(&id).copied();
            let empty: HashMap<usize, AdvancePerToothMm> = HashMap::new();
            let per_move = scans.1.get(&id).unwrap_or(&empty);
            ToolpathGpuData::from_toolpath_advance_per_tooth(
                gpu.device,
                &resources.gpu_limits,
                render_tp,
                band.as_ref(),
                per_move,
            )
        }
        Colouring::Normal => ToolpathGpuData::from_toolpath(
            gpu.device,
            &resources.gpu_limits,
            render_annotated,
            palette_index,
            selected,
            &span_filter,
        ),
    };
    gpu_data.toolpath_id = Some(id);

    // Entry path preview — selected toolpaths only, config resolved in the
    // inputs so it is part of the key.
    if let Some(config) = entry_preview.as_ref() {
        let preview_verts = toolpath_render::entry_preview_vertices(render_tp, config);
        gpu_data.attach_entry_preview(gpu.device, &resources.gpu_limits, &preview_verts);
    }

    // Tool-profile ghost overlay (optional).
    // F029: a tool outside its domain draws no ghost; the tool panel names
    // the refusal.
    if let Some(tool) = tool_profile
        && let Ok(cutter) = rs_cam_core::compute::build_cutter(tool)
    {
        let profile_verts = toolpath_render::tool_profile_preview_vertices(render_tp, &cutter);
        gpu_data.attach_tool_profile_preview(gpu.device, &resources.gpu_limits, &profile_verts);
    }
    gpu_data
}

/// Upload the height plane overlays whenever a toolpath is selected (any
/// workspace). Unkeyed: five quads.
pub(super) fn upload_height_planes(
    state: &AppState,
    frame: &PassFrame,
    stock_bbox: &rs_cam_core::geo::BoundingBox3,
    gpu: &Gpu<'_>,
    resources: &mut RenderResources,
) {
    let Selection::Toolpath(tp_id) = state.selection else {
        resources.height_planes_data = None;
        return;
    };
    let session = state.session();
    let display_shift = frame.display_shift();
    // P3.1: THE heights of the session; a refused height draws no planes.
    resources.height_planes_data = session
        .find_toolpath_config_by_id(tp_id)
        .and_then(|(_, tc)| session.toolpath_heights(tc).ok())
        .map(|resolved| resolved.heights)
        .map(|heights| {
            // Use the same stock bbox as the rest of the viewport (local or
            // global). Heights resolve in the emission frame, so identity
            // setups shift by -origin_z to match it.
            crate::render::height_planes::HeightPlanesGpuData::from_heights(
                gpu.device,
                stock_bbox,
                heights.clearance_z + display_shift.z,
                heights.retract_z + display_shift.z,
                heights.feed_z + display_shift.z,
                heights.top_z + display_shift.z,
                heights.bottom_z + display_shift.z,
            )
        });
}
