//! The keyed overlay slots of the selected toolpath and the planners: the
//! rest-depth heatmap, the multi-tool tier preview, the By Area regions and
//! the per-tool reach map.

use std::sync::Arc;

use rs_cam_core::geo::P3;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::session::MultitoolPreview;
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;

use crate::render::RenderResources;
use crate::render::sim_render::SimMeshGpuData;
use crate::render::upload_cache::{self, Refresh, SlotInputs};
use crate::state::AppState;
use crate::state::job::transform_mesh;
use crate::state::selection::Selection;
use crate::state::toolpath::ToolpathId;

use super::{Gpu, PassFrame, has_shift, shift_key, shift_stock_mesh, translate_annotated};

/// The selected toolpath and its result, the source of the two
/// per-toolpath overlays.
fn selected_result(state: &AppState) -> Option<(ToolpathId, &Arc<AnnotatedToolpath>)> {
    let Selection::Toolpath(tp_id) = state.selection else {
        return None;
    };
    state
        .gui
        .toolpath_rt
        .get(&tp_id)
        .and_then(|rt| rt.result.as_ref())
        .map(|result| (tp_id, &result.annotated))
}

/// Everything the rest-depth heatmap is built from: the selected
/// toolpath's result, whose `Arc` identity changes exactly when a new
/// generation lands, and the display shift.
pub(super) struct RestInputs<'a> {
    toolpath: ToolpathId,
    annotated: &'a Arc<AnnotatedToolpath>,
    shift: P3,
}

impl SlotInputs for RestInputs<'_> {
    type Key = upload_cache::RestHeatmapUploadKey;

    fn key(&self) -> Self::Key {
        let Self {
            toolpath,
            annotated,
            shift,
        } = self;
        upload_cache::RestHeatmapUploadKey {
            toolpath: *toolpath,
            annotated: upload_cache::ArcId::new(annotated),
            shift: shift_key(*shift),
        }
    }
}

/// Upload the rest-depth heatmap overlay (pencil detector #4) whenever the
/// selected toolpath carries a populated `rest_grid`. It mirrors the
/// height-plane upload gate and rebuilds on the same pending-upload cycle,
/// not every frame.
///
/// V8: keyed on the selected toolpath's result generation. The build reaches
/// `.rest_grid` through a full `AnnotatedToolpath` deep clone whenever a
/// display shift is in play (V9) — the NORMAL config for an identity setup
/// with a non-zero stock origin — so skipping it when nothing moved is the
/// single largest saving of the pass.
pub(super) fn upload_rest_heatmap(
    state: &AppState,
    frame: &PassFrame,
    gpu: &Gpu<'_>,
    resources: &mut RenderResources,
) {
    let inputs = selected_result(state).map(|(toolpath, annotated)| RestInputs {
        toolpath,
        annotated,
        shift: frame.display_shift(),
    });
    let Refresh::Build(inputs) =
        upload_cache::refresh(&mut resources.rest_heatmap_upload_key, inputs)
    else {
        return;
    };
    resources.upload_stats.rest_heatmap_builds += 1;
    let rest_grid = inputs.and_then(|inputs| {
        let RestInputs {
            toolpath: _,
            annotated,
            shift,
        } = inputs;
        // Rest grids arrive in the emission frame alongside the toolpath
        // they came from; re-frame in lockstep with the same display shift
        // applied to the toolpath lines (identity-setup world→local shift).
        if has_shift(shift) {
            translate_annotated(annotated, shift).produced.rest_grid
        } else {
            annotated.produced.rest_grid.clone()
        }
    });
    resources.rest_heatmap_data = rest_grid.and_then(|grid| {
        rs_cam_core::maps::rest_heatmap_mesh::rest_grid_to_heatmap_mesh(&grid).and_then(|hm| {
            SimMeshGpuData::from_heightmap_mesh(gpu.device, &resources.gpu_limits, &hm)
        })
    });
}

/// Everything the tier-map preview is built from.
pub(super) struct TierInputs<'a> {
    generation: u64,
    preview: &'a MultitoolPreview,
    shift: P3,
}

impl SlotInputs for TierInputs<'_> {
    type Key = upload_cache::TierPreviewUploadKey;

    fn key(&self) -> Self::Key {
        let Self {
            generation,
            // The planner bumps `generation` on every landed preview, and
            // nothing else changes the preview (`TierPreviewUploadKey`).
            preview: _,
            shift,
        } = self;
        upload_cache::TierPreviewUploadKey {
            generation: *generation,
            shift: shift_key(*shift),
        }
    }
}

/// Upload the multi-tool tier-map preview overlay (Phase U). Its own slot
/// beside the rest heatmap, keyed on the planner's preview GENERATION rather
/// than on any dial: the mesh changes exactly when a new preview lands, so
/// dragging the coarseness slider rebuilds once per landed preview and
/// toggling the visibility checkbox rebuilds nothing at all.
///
/// Gated on the previewed setup being the ACTIVE one, same predicate as the
/// `show_tier_preview` callback gate: the map is in its own setup's emission
/// frame, and the pass shift belongs to the active setup — baking the wrong
/// frame's shift into the mesh is exactly the operator-observed offset
/// defect. On any other setup the inputs go `None`, the data drops, and
/// returning to the planned setup rebuilds once.
pub(super) fn upload_tier_preview(
    state: &AppState,
    frame: &PassFrame,
    gpu: &Gpu<'_>,
    resources: &mut RenderResources,
) {
    let active_setup = state.active_setup_index();
    let inputs = state
        .multitool_planner
        .as_ref()
        .filter(|planner| active_setup == Some(planner.setup_index))
        .and_then(|planner| {
            planner.ready_preview().map(|preview| TierInputs {
                generation: planner.preview_generation,
                preview,
                shift: frame.display_shift(),
            })
        });
    let Refresh::Build(inputs) =
        upload_cache::refresh(&mut resources.tier_preview_upload_key, inputs)
    else {
        return;
    };
    resources.tier_preview_data = inputs.and_then(|inputs| {
        let TierInputs {
            generation: _,
            preview,
            shift,
        } = inputs;
        rs_cam_core::maps::rest_heatmap_mesh::tier_map_to_heatmap_mesh(
            &preview.map,
            &preview.islands,
        )
        .map(|hm| shift_stock_mesh(hm, shift))
        .and_then(|hm| SimMeshGpuData::from_heightmap_mesh(gpu.device, &resources.gpu_limits, &hm))
    });
}

/// Everything the By Area regions overlay is built from. The same three
/// inputs as the rest heatmap: the map rides the selected toolpath's
/// `AnnotatedToolpath`.
pub(super) struct AreaInputs<'a> {
    toolpath: ToolpathId,
    annotated: &'a Arc<AnnotatedToolpath>,
    shift: P3,
}

impl SlotInputs for AreaInputs<'_> {
    type Key = upload_cache::AreaRegionsUploadKey;

    fn key(&self) -> Self::Key {
        let Self {
            toolpath,
            annotated,
            shift,
        } = self;
        upload_cache::AreaRegionsUploadKey {
            toolpath: *toolpath,
            annotated: upload_cache::ArcId::new(annotated),
            shift: shift_key(*shift),
        }
    }
}

/// Upload the By Area regions overlay of the selected toolpath. Keyed like
/// the rest heatmap on the toolpath's `Arc` identity and the display frame,
/// so a checkbox toggle rebuilds nothing. The map is in the emission frame;
/// the mesh is shifted, not the map, so no `AnnotatedToolpath` clone is
/// needed.
pub(super) fn upload_area_regions(
    state: &AppState,
    frame: &PassFrame,
    gpu: &Gpu<'_>,
    resources: &mut RenderResources,
) {
    let inputs = selected_result(state)
        .filter(|(_, annotated)| annotated.produced.area_regions.is_some())
        .map(|(toolpath, annotated)| AreaInputs {
            toolpath,
            annotated,
            shift: frame.display_shift(),
        });
    let Refresh::Build(inputs) =
        upload_cache::refresh(&mut resources.area_regions_upload_key, inputs)
    else {
        return;
    };
    resources.area_regions_data = inputs.and_then(|inputs| {
        let AreaInputs {
            toolpath: _,
            annotated,
            shift,
        } = inputs;
        annotated
            .produced
            .area_regions
            .as_ref()
            .and_then(|map| rs_cam_core::maps::rest_heatmap_mesh::area_regions_to_mesh(map))
            .map(|hm| shift_stock_mesh(hm, shift))
            .and_then(|hm| {
                SimMeshGpuData::from_heightmap_mesh(gpu.device, &resources.gpu_limits, &hm)
            })
    });
}

/// Everything the reach overlay is built from (P5).
pub(super) struct ReachInputs<'a> {
    /// The overlay state's counter, bumped only when a map arrives that is
    /// not the one already held.
    generation: u64,
    /// The MODEL mesh the overlay is drawn over.
    mesh: Arc<TriangleMesh>,
    /// The per-vertex colours the Reach lane built beside the map.
    colors: Arc<Vec<[f32; 3]>>,
    frame: &'a PassFrame,
}

impl SlotInputs for ReachInputs<'_> {
    type Key = upload_cache::ReachOverlayUploadKey;

    fn key(&self) -> Self::Key {
        let Self {
            generation,
            mesh,
            // The Reach lane builds them with the map, so `generation`
            // moves with them.
            colors: _,
            frame,
        } = self;
        upload_cache::ReachOverlayUploadKey {
            generation: *generation,
            mesh: upload_cache::ArcId::new(mesh),
            frame: frame.key(),
        }
    }
}

/// Upload the per-tool reach-map overlay (P5). Keyed on the overlay's own
/// generation, the model mesh identity and the display frame — so a
/// re-selection that hits the reach memo rebuilds nothing, while a face flip
/// or a stock resize rebuilds once.
///
/// The colours are NOT computed here. `ReachMap::vertex_gaps` runs one
/// spatial-index query per mesh vertex — hundreds of thousands on a
/// board-sized terrain — so the Reach lane builds them beside the map and
/// this pass only interleaves them into the vertex buffer.
///
/// FRAMES. The map is measured on the setup-transformed mesh (or on the raw
/// mesh for an identity setup), while the viewport draws that same mesh
/// through `transform_mesh`. Both are `SetupTransformInfo::apply_to_mesh`,
/// which maps vertices one to one and clones the triangle list, so vertex
/// ORDER is preserved: colour `i` belongs to displayed vertex `i`. The two
/// frames' COORDINATES differ on an identity setup with a non-zero stock
/// origin; the ordering does not, and the ordering is all the colours need.
/// The length check below is the tripwire if that ever stops holding.
pub(super) fn upload_reach(
    state: &AppState,
    frame: &PassFrame,
    gpu: &Gpu<'_>,
    resources: &mut RenderResources,
) {
    let overlay = &state.gui.reach_overlay;
    let inputs = overlay.ready_map().and_then(|map| {
        let model_id = map.model_id?;
        let mesh = state
            .session
            .models()
            .iter()
            .find(|model| model.id == model_id)
            .and_then(|model| model.mesh.clone())?;
        let colors = overlay.colors.as_ref().map(Arc::clone)?;
        Some(ReachInputs {
            generation: overlay.generation,
            mesh,
            colors,
            frame,
        })
    });
    let Refresh::Build(inputs) =
        upload_cache::refresh(&mut resources.reach_overlay_upload_key, inputs)
    else {
        return;
    };
    resources.reach_overlay_data = inputs.and_then(|inputs| {
        let ReachInputs {
            generation: _,
            mesh,
            colors,
            frame,
        } = inputs;
        let displayed = match frame.frame() {
            Some(setup) => std::borrow::Cow::Owned(transform_mesh(&mesh, setup, frame.stock())),
            None => std::borrow::Cow::Borrowed(mesh.as_ref()),
        };
        if colors.len() != displayed.vertices.len() {
            tracing::warn!(
                colors = colors.len(),
                vertices = displayed.vertices.len(),
                "reach overlay colours do not match the drawn mesh — overlay skipped"
            );
            return None;
        }
        crate::render::mesh_render::colored_mesh_gpu_data(
            gpu.device,
            &resources.gpu_limits,
            &displayed,
            &colors,
        )
    });
}
