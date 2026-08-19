//! Content-addressed keys for the viewport's GPU upload pass (V8).
//!
//! ## Why keys and not dirty bits
//!
//! `upload_gpu_data` runs whenever `AppController::take_pending_upload()`
//! fires — roughly forty call sites set that flag, from a selection click to
//! each completed toolpath during `generate_all`. Before this module every
//! such fire cleared and rebuilt **every** GPU buffer in the scene: an
//! 8-operation `generate_all` re-uploaded the model mesh, every toolpath, the
//! stock and the fixtures eight times over.
//!
//! The review (`planning/perf_review_2026-08-19/PERF_REVIEW.md`, V8)
//! prescribed per-resource dirty bits. This module implements the stronger
//! form: each expensive resource carries a key derived from **the values that
//! actually feed its buffer**, and the upload pass rebuilds a resource only
//! when its key changed. That has two properties dirty bits do not:
//!
//! - a *missed* dirty-bit setter cannot stale the viewport, because nothing
//!   depends on a setter remembering to name the right resource; and
//! - a *coarse* `set_pending_upload()` (which is what all forty sites still
//!   call, unchanged) no longer costs a full-scene rebuild — the key tells
//!   the pass that nothing this resource reads has moved.
//!
//! The cost of that strength is that every input a resource reads must appear
//! in its key. Each key below documents its inputs against the code that
//! consumes them; adding a new input to a builder means adding it here.
//!
//! Geometry that arrives behind an `Arc` (`TriangleMesh`, `EnrichedMesh`,
//! `AnnotatedToolpath`) is keyed by `Arc::as_ptr` — pointer identity, which
//! this codebase already uses as a staleness key for the simulation caches
//! (`SimulationState::cached_simulation_triage`).

use rs_cam_core::enriched_mesh::FaceGroupId;

use crate::state::job::{FaceUp, SetupId, StockConfig, ToolConfig, ZRotation};
use crate::state::toolpath::ToolpathId;
use crate::state::viewport::{SpanKindFilter, ToolpathColorMode};

use super::toolpath_render::EntryPreviewConfig;

/// The display frame every viewport resource is drawn in: the active setup's
/// orientation plus the stock it is measured against.
///
/// Consumed by `Setup::transform_point` / `Setup::effective_stock` in the
/// mesh, stock and fixture uploads.
#[derive(Debug, Clone, PartialEq)]
pub struct FrameKey {
    /// `None` when no setup is resolvable (world frame).
    pub setup: Option<(SetupId, FaceUp, ZRotation)>,
    pub stock: StockConfig,
}

/// Key for `RenderResources::mesh_data_list` — the indexed, smooth-normal
/// STL path (`MeshGpuData::from_mesh`).
///
/// Inputs: the `Arc` identity of every plain mesh in the session, in model
/// order, and the display frame (`transform_mesh` bakes the setup transform
/// into the uploaded vertices). Deliberately **not** keyed on selection: a
/// plain mesh's buffer carries no selection-dependent data.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshUploadKey {
    pub frame: FrameKey,
    /// `Arc::as_ptr` of each contributing `TriangleMesh`, in model order.
    pub meshes: Vec<usize>,
}

/// Key for `RenderResources::enriched_mesh_data_list` — the per-face-coloured
/// STEP/BREP path (`enriched_mesh_gpu_data`).
///
/// Inputs: the enriched-mesh `Arc` identities, the display frame, and the two
/// colour inputs the builder reads per triangle — the selected face set and
/// the hovered face. The hover entry is what makes V13 (BREP hover highlight
/// never re-uploading) fixable without a full-scene rebuild per pointer move.
#[derive(Debug, Clone, PartialEq)]
pub struct EnrichedUploadKey {
    pub frame: FrameKey,
    /// `Arc::as_ptr` of each contributing `EnrichedMesh`, in model order.
    pub meshes: Vec<usize>,
    pub selected_faces: Vec<FaceGroupId>,
    pub hovered_face: Option<FaceGroupId>,
}

/// Key for the collision-marker line buffer.
///
/// Holds the marker positions themselves rather than a generation counter:
/// the build is `O(n²)` (density estimation, V12) so an `O(n)` comparison is
/// cheap by construction, and the controller has no generation counter to
/// borrow.
#[derive(Debug, Clone, PartialEq)]
pub struct CollisionUploadKey {
    pub positions: Vec<[f32; 3]>,
    /// Emission → display shift applied to every marker.
    pub shift: [f64; 3],
}

/// Key for the rest-depth heatmap overlay.
///
/// The overlay is built from the selected toolpath's `rest_grid`, reached
/// today through a full `AnnotatedToolpath` deep clone when a display shift
/// is in play (V9). Keying on the toolpath's `Arc` identity means that clone
/// happens once per generation instead of once per upload pass.
#[derive(Debug, Clone, PartialEq)]
pub struct RestHeatmapUploadKey {
    pub toolpath: ToolpathId,
    /// `Arc::as_ptr` of the selected toolpath's `AnnotatedToolpath`.
    pub annotated: usize,
    pub shift: [f64; 3],
}

/// Key for one toolpath's line buffers (`ToolpathGpuData`).
///
/// This is the key the review's "per-toolpath GPU data keyed by result
/// generation" asks for; `annotated` *is* the result generation, since the
/// compute worker publishes a fresh `Arc<AnnotatedToolpath>` per generate.
///
/// Inputs, each against its consumer in `RsCamApp::upload_gpu_data`:
///
/// - `annotated` — the geometry, and everything derived from it.
/// - `palette_index` — the enumeration index passed to
///   `ToolpathGpuData::from_toolpath` as the palette selector.
/// - `selected` — selects the highlight colour, and gates the entry-preview
///   and tool-profile overlays.
/// - `color_mode`, `span_filter` — the two viewport dials that change which
///   builder runs and which segments it emits.
/// - `shift` — the identity-setup emission → display translation.
/// - `feed_rate` — nominal feed for the Engagement colour mode.
/// - `advance_source` — AdvancePerTooth mode only: the cut-trace identity the
///   per-move advance map is derived from, paired with the session edit
///   counter. The vendor band comes from `chipload_envelopes_for_session`,
///   which reads tool and material config; the edit counter is this
///   codebase's established staleness signal for session config and is
///   cheaper than recomputing the band just to compare it.
/// - `entry_preview` / `tool_profile` — the selected toolpath's overlay
///   inputs, `None` when the overlay is not drawn.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolpathUploadKey {
    pub annotated: usize,
    pub palette_index: usize,
    pub selected: bool,
    pub color_mode: ToolpathColorMode,
    pub span_filter: SpanKindFilter,
    pub shift: [f64; 3],
    pub feed_rate: f64,
    pub advance_source: Option<(usize, u64)>,
    pub entry_preview: Option<EntryPreviewConfig>,
    pub tool_profile: Option<ToolConfig>,
}

/// Running counts of what the upload pass actually rebuilt.
///
/// The instrument the review asked for: upload cost is not visible to
/// criterion (there is no criterion harness for the GUI loop), so the pass
/// counts its own work and logs the per-pass deltas at `debug`. `passes`
/// counts calls to `upload_gpu_data`; every other field counts buffer
/// *builds*. An 8-operation `generate_all` reads "8 passes, 8 toolpath
/// builds, 28 reuses"; before the keys landed the same run built a toolpath
/// 1+2+…+8 = 36 times (a submit clears its result, so pass *k* had *k*
/// results to rebuild) and re-transformed the model mesh on all eight.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UploadStats {
    pub passes: u64,
    pub mesh_builds: u64,
    pub enriched_builds: u64,
    pub toolpath_builds: u64,
    pub toolpath_reuses: u64,
    pub collision_builds: u64,
    pub rest_heatmap_builds: u64,
}

impl UploadStats {
    /// Field-wise difference, for logging one pass without resetting the
    /// lifetime totals.
    pub fn since(&self, earlier: &UploadStats) -> UploadStats {
        UploadStats {
            passes: self.passes.saturating_sub(earlier.passes),
            mesh_builds: self.mesh_builds.saturating_sub(earlier.mesh_builds),
            enriched_builds: self.enriched_builds.saturating_sub(earlier.enriched_builds),
            toolpath_builds: self.toolpath_builds.saturating_sub(earlier.toolpath_builds),
            toolpath_reuses: self.toolpath_reuses.saturating_sub(earlier.toolpath_reuses),
            collision_builds: self
                .collision_builds
                .saturating_sub(earlier.collision_builds),
            rest_heatmap_builds: self
                .rest_heatmap_builds
                .saturating_sub(earlier.rest_heatmap_builds),
        }
    }

    /// True when the pass rebuilt nothing — the case a selection click on a
    /// project with no BREP faces should reach.
    pub fn is_idle(&self) -> bool {
        self.mesh_builds == 0
            && self.enriched_builds == 0
            && self.toolpath_builds == 0
            && self.collision_builds == 0
            && self.rest_heatmap_builds == 0
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::render::toolpath_render::EntryStyle;

    fn key() -> ToolpathUploadKey {
        ToolpathUploadKey {
            annotated: 0x1000,
            palette_index: 3,
            selected: false,
            color_mode: ToolpathColorMode::Normal,
            span_filter: SpanKindFilter::default(),
            shift: [0.0, 0.0, -19.0],
            feed_rate: 1200.0,
            advance_source: None,
            entry_preview: None,
            tool_profile: None,
        }
    }

    /// The generate_all case: one toolpath finishing publishes a new
    /// `Arc<AnnotatedToolpath>`, which must invalidate that toolpath's key
    /// and *only* that toolpath's key.
    #[test]
    fn new_result_generation_invalidates_only_its_own_key() {
        let before = key();
        let mut after = before.clone();
        after.annotated = 0x2000;
        assert_ne!(before, after);

        // A peer toolpath, untouched by the generate, keeps its key.
        let peer = ToolpathUploadKey {
            annotated: 0x9000,
            palette_index: 4,
            ..before
        };
        assert_eq!(peer, peer.clone());
        assert_ne!(peer, after);
    }

    /// A selection click flips `selected` on at most two toolpaths (the one
    /// losing selection and the one gaining it); every other key is
    /// unchanged, so the pass re-uploads nothing for them.
    #[test]
    fn selection_change_moves_only_the_selected_flag() {
        let unselected = key();
        let mut selected = unselected.clone();
        selected.selected = true;
        assert_ne!(unselected, selected);
        // Same toolpath, same generation, selection unchanged → reuse.
        assert_eq!(unselected, key());
    }

    /// Viewport dials that change what the builder emits must be in the key.
    #[test]
    fn colour_mode_and_span_filter_are_keyed() {
        let base = key();
        let mut recoloured = base.clone();
        recoloured.color_mode = ToolpathColorMode::Engagement;
        assert_ne!(base, recoloured);

        let mut refiltered = base.clone();
        refiltered.span_filter.show_dressup = !refiltered.span_filter.show_dressup;
        assert_ne!(base, refiltered);
    }

    /// The entry-preview overlay is rebuilt when its resolved heights or
    /// dressup dials move, not merely when the toolpath is reselected.
    #[test]
    fn entry_preview_params_are_keyed() {
        let mut a = key();
        a.selected = true;
        a.entry_preview = Some(EntryPreviewConfig {
            entry_style: EntryStyle::Ramp,
            ramp_angle_deg: 3.0,
            helix_radius: 2.0,
            helix_pitch: 1.0,
            lead_in_out: false,
            lead_radius: 0.0,
            feed_z: 1.0,
            top_z: 0.0,
        });
        let mut b = a.clone();
        assert_eq!(a, b);
        if let Some(cfg) = b.entry_preview.as_mut() {
            cfg.ramp_angle_deg = 5.0;
        }
        assert_ne!(a, b);
    }

    /// AdvancePerTooth colouring reads the cut trace and the session's tool /
    /// material config; both must move the key.
    #[test]
    fn advance_per_tooth_sources_are_keyed() {
        let mut a = key();
        a.color_mode = ToolpathColorMode::AdvancePerTooth;
        a.advance_source = Some((0x4000, 7));
        let mut new_trace = a.clone();
        new_trace.advance_source = Some((0x5000, 7));
        assert_ne!(a, new_trace);
        let mut edited_session = a.clone();
        edited_session.advance_source = Some((0x4000, 8));
        assert_ne!(a, edited_session);
    }

    /// V13's hover input: the enriched key moves with the hovered face, and
    /// the plain-mesh key has no hover input at all — so a pointer move over
    /// a STEP model cannot drag the STL upload with it.
    #[test]
    fn hover_moves_the_enriched_key_only() {
        let frame = FrameKey {
            setup: None,
            stock: StockConfig::default(),
        };
        let mesh = MeshUploadKey {
            frame: frame.clone(),
            meshes: vec![0x10],
        };
        let enriched = EnrichedUploadKey {
            frame,
            meshes: vec![0x20],
            selected_faces: Vec::new(),
            hovered_face: None,
        };
        let hovered = EnrichedUploadKey {
            hovered_face: Some(FaceGroupId(4)),
            ..enriched.clone()
        };
        assert_ne!(enriched, hovered);
        // The plain-mesh key is untouched by hover: it has no such field, so
        // rebuilding it is structurally impossible from a pointer move.
        assert_eq!(mesh, mesh.clone());
    }

    #[test]
    fn stats_deltas_and_idle() {
        let before = UploadStats {
            passes: 3,
            toolpath_builds: 8,
            ..UploadStats::default()
        };
        let after = UploadStats {
            passes: 4,
            toolpath_builds: 9,
            toolpath_reuses: 7,
            ..UploadStats::default()
        };
        let delta = after.since(&before);
        assert_eq!(delta.passes, 1);
        assert_eq!(delta.toolpath_builds, 1);
        assert_eq!(delta.toolpath_reuses, 7);
        assert!(!delta.is_idle());

        let idle = UploadStats {
            passes: 1,
            toolpath_reuses: 8,
            ..UploadStats::default()
        };
        assert!(idle.is_idle());
    }
}
