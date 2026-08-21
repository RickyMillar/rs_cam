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
//! `AnnotatedToolpath`, `SimulationCutTrace`) is keyed by object identity —
//! see [`ArcId`], which pins the identity with a `Weak` rather than a raw
//! address.
//!
//! This module used to justify a bare `Arc::as_ptr` key by citing
//! `SimulationState::cached_simulation_triage` as the codebase's established
//! precedent. That citation was wrong twice over: the workspace's *argued*
//! doctrine is the opposite one (`rs_cam_core::geom_cache` module doc,
//! `rs_cam_core::compute::sim_prefix` — "**pointer keys are `Weak`, never
//! bare pointers**"), and the viz caches it named have since been converted
//! to that doctrine too (`state::simulation`, `weak_matches`). Both key
//! shapes are now the same one.

use std::sync::{Arc, Weak};

use rs_cam_core::enriched_mesh::FaceGroupId;

use crate::state::job::{FaceUp, SetupId, StockConfig, ToolConfig, ZRotation};
use crate::state::toolpath::ToolpathId;
use crate::state::viewport::{SpanKindFilter, ToolpathColorMode};

use super::toolpath_render::EntryPreviewConfig;

/// Liveness-pinned `Arc` identity — the identity half of every key below.
///
/// A bare `Arc::as_ptr as usize` is not a sound key: the `Arc` can be dropped
/// and a fresh allocation can land at the same address, so the pass would
/// reuse the previous object's GPU buffer for a different object (a model
/// unloaded and reloaded, a toolpath regenerated after its predecessor was
/// freed). Pairing the address with an element count — what the mesh keys did
/// before — narrows that window rather than closing it, and requires only
/// that the replacement have the same triangle count.
///
/// Holding a [`Weak`] closes it: the allocation stays reserved while the key
/// lives, so **no other `Arc` can be handed that address**, and address
/// equality therefore proves same-allocation. That is why [`PartialEq`] here
/// is [`Weak::ptr_eq`] and needs no upgrade: a stored key whose object has
/// been dropped can only compare equal to a `Weak` into that same reserved
/// allocation, which no live replacement can be. (Where a cache must also
/// know the subject is still *alive*, upgrade instead — see
/// `rs_cam_core::geom_cache` and `state::simulation::weak_matches`.)
///
/// Cost: a dropped object's `Arc` header (tens of bytes, its `Vec`s already
/// freed) is retained until the key is replaced on the next upload pass.
pub struct ArcId<T>(Weak<T>);

impl<T> ArcId<T> {
    pub fn new(arc: &Arc<T>) -> Self {
        Self(Arc::downgrade(arc))
    }
}

// Hand-written so the impls carry no `T: Clone` / `T: PartialEq` / `T: Debug`
// bound: an identity key never touches the value it identifies.
impl<T> Clone for ArcId<T> {
    fn clone(&self) -> Self {
        Self(Weak::clone(&self.0))
    }
}

impl<T> PartialEq for ArcId<T> {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.0, &other.0)
    }
}

impl<T> std::fmt::Debug for ArcId<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ArcId({:p})", self.0.as_ptr())
    }
}

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
    /// Pinned identity of each contributing `TriangleMesh`, in model order.
    pub meshes: Vec<ArcId<rs_cam_core::mesh::TriangleMesh>>,
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
    /// Pinned identity of each contributing `EnrichedMesh`, in model order.
    pub meshes: Vec<ArcId<rs_cam_core::enriched_mesh::EnrichedMesh>>,
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
    /// Pinned identity of the selected toolpath's `AnnotatedToolpath`.
    pub annotated: ArcId<rs_cam_core::toolpath_spans::AnnotatedToolpath>,
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
    pub annotated: ArcId<rs_cam_core::toolpath_spans::AnnotatedToolpath>,
    pub palette_index: usize,
    pub selected: bool,
    pub color_mode: ToolpathColorMode,
    pub span_filter: SpanKindFilter,
    pub shift: [f64; 3],
    pub feed_rate: f64,
    pub advance_source: Option<AdvanceSource>,
    pub entry_preview: Option<EntryPreviewConfig>,
    pub tool_profile: Option<ToolConfig>,
}

/// Everything the AdvancePerTooth colouring reads that is not the toolpath.
///
/// Split out of `(usize, u64)` when the pointer became an [`ArcId`]: the
/// old shape used `0` for "no simulation has run", which an identity type
/// cannot express — and could not distinguish from a trace that happened to
/// live at address 0.
#[derive(Debug, Clone, PartialEq)]
pub struct AdvanceSource {
    /// Identity of the cut trace the per-move advance map is derived from;
    /// `None` when there is no cut trace.
    pub trace: Option<ArcId<rs_cam_core::simulation_cut::SimulationCutTrace>>,
    /// Session edit counter — stands in for the tool/material config the
    /// vendor band is matched from.
    pub edit_counter: u64,
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
    use rs_cam_core::mesh::{TriangleMesh, make_test_flat};
    use rs_cam_core::toolpath::Toolpath;
    use rs_cam_core::toolpath_spans::AnnotatedToolpath;

    fn annotated() -> Arc<AnnotatedToolpath> {
        Arc::new(AnnotatedToolpath::new(Toolpath::new()))
    }

    fn mesh() -> Arc<TriangleMesh> {
        Arc::new(make_test_flat(10.0))
    }

    fn key_for(annotated: &Arc<AnnotatedToolpath>) -> ToolpathUploadKey {
        ToolpathUploadKey {
            annotated: ArcId::new(annotated),
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
        let first = annotated();
        let before = key_for(&first);
        let regenerated = annotated();
        let mut after = before.clone();
        after.annotated = ArcId::new(&regenerated);
        assert_ne!(before, after);

        // A peer toolpath, untouched by the generate, keeps its key.
        let peer_annotated = annotated();
        let peer = ToolpathUploadKey {
            annotated: ArcId::new(&peer_annotated),
            palette_index: 4,
            ..before
        };
        assert_eq!(peer, peer.clone());
        assert_ne!(peer, after);
    }

    /// The ABA property, for the identity half of every key in this module:
    /// while a stored key holds its `Weak`, the freed allocation stays
    /// reserved, so a replacement cannot be handed that address and the
    /// stored key cannot answer for it. The `assert_ne!` on the raw address
    /// is precisely the comparison the old `usize` key made — and nothing
    /// guaranteed it then.
    #[test]
    fn a_freed_toolpath_cannot_be_impersonated_by_its_replacement() {
        let first = annotated();
        let freed_addr = Arc::as_ptr(&first) as usize;
        let stale = key_for(&first);
        drop(first);

        let mut replacements = Vec::new();
        for _ in 0..64 {
            let replacement = annotated();
            assert_ne!(
                Arc::as_ptr(&replacement) as usize,
                freed_addr,
                "the stored key's Weak must reserve the freed allocation"
            );
            assert_ne!(stale, key_for(&replacement));
            replacements.push(replacement);
        }
    }

    /// A selection click flips `selected` on at most two toolpaths (the one
    /// losing selection and the one gaining it); every other key is
    /// unchanged, so the pass re-uploads nothing for them.
    #[test]
    fn selection_change_moves_only_the_selected_flag() {
        let tp = annotated();
        let unselected = key_for(&tp);
        let mut selected = unselected.clone();
        selected.selected = true;
        assert_ne!(unselected, selected);
        // Same toolpath, same generation, selection unchanged → reuse.
        assert_eq!(unselected, key_for(&tp));
    }

    /// Viewport dials that change what the builder emits must be in the key.
    #[test]
    fn colour_mode_and_span_filter_are_keyed() {
        let tp = annotated();
        let base = key_for(&tp);
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
        let tp = annotated();
        let mut a = key_for(&tp);
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
        let tp = annotated();
        let trace = Arc::new(
            rs_cam_core::simulation_cut::SimulationCutTrace::from_samples(0.5, Vec::new()),
        );
        let mut a = key_for(&tp);
        a.color_mode = ToolpathColorMode::AdvancePerTooth;
        a.advance_source = Some(AdvanceSource {
            trace: Some(ArcId::new(&trace)),
            edit_counter: 7,
        });

        let resimulated = Arc::new(
            rs_cam_core::simulation_cut::SimulationCutTrace::from_samples(0.5, Vec::new()),
        );
        let mut new_trace = a.clone();
        new_trace.advance_source = Some(AdvanceSource {
            trace: Some(ArcId::new(&resimulated)),
            edit_counter: 7,
        });
        assert_ne!(a, new_trace);

        let mut edited_session = a.clone();
        edited_session.advance_source = Some(AdvanceSource {
            trace: Some(ArcId::new(&trace)),
            edit_counter: 8,
        });
        assert_ne!(a, edited_session);

        // "No simulation has run" is its own state, not address 0.
        let mut unsimulated = a.clone();
        unsimulated.advance_source = Some(AdvanceSource {
            trace: None,
            edit_counter: 7,
        });
        assert_ne!(a, unsimulated);
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
        let plain = mesh();
        let enriched_mesh = Arc::new(rs_cam_core::enriched_mesh::EnrichedMesh {
            mesh: mesh(),
            face_groups: Vec::new(),
            triangle_to_face: Vec::new(),
            adjacency: Vec::new(),
            edges: Vec::new(),
        });
        let mesh = MeshUploadKey {
            frame: frame.clone(),
            meshes: vec![ArcId::new(&plain)],
        };
        let enriched = EnrichedUploadKey {
            frame,
            meshes: vec![ArcId::new(&enriched_mesh)],
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
