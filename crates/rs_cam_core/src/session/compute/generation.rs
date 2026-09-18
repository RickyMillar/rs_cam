//! Generation-input resolution and boundary clipping.
//!
//! [`crate::session::ProjectSession::resolve_generation_inputs`] turns a
//! setup, a tool and an operation into the inputs an operation runs on; the
//! clip methods cut that geometry to the machining boundary. Split out of
//! `session/compute.rs` (P4).

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::compute::config::HeightContext;
use crate::compute::cutter::build_cutter;
use crate::compute::tool_config::ToolId;
use crate::geo::BoundingBox3;
use crate::session::{ProjectSession, SessionError};
use crate::trace::semantic_trace::{SemanticKey, ToolpathSemanticKind};

use super::ResolvedGenInputs;

impl ProjectSession {
    /// Resolve every per-generation input from session state for toolpath
    /// `index`: tool + cutter, geometry (mesh / polygons, setup-transformed),
    /// spatial index, resolved heights, cutting levels, emission-frame stock
    /// bbox, keep-outs, boundary + pre-clip polygon, rest-machining prev-tool
    /// radius, and the compute-time-patched operation. A pure read of `self`
    /// returning a fully-owned [`ResolvedGenInputs`] so callers can generate
    /// one or many toolpaths off a single resolution (the strategy advisor's
    /// per-strategy candidates) without re-deriving frame-sensitive values.
    ///
    /// [`start_generate_toolpath`](Self::start_generate_toolpath) puts this
    /// on a [`GenerateToolpathHandle`], and [`execute_job`] then owns the
    /// per-generation recorders + dressup/persist tail; the extraction keeps
    /// that pipeline byte-identical (the recorders simply move after the
    /// resolution, which never depended on them).
    ///
    /// `cancel` is threaded because one boundary source does real geometric
    /// work HERE rather than inside the generator:
    /// [`BoundarySource::PlannedTierRegions`](crate::compute::config::BoundarySource::PlannedTierRegions)
    /// walks a full-grid drop-cutter tier map (seconds to tens of seconds on
    /// a real board), and a resolution that could not be interrupted would
    /// make Cancel a lie for the whole of it. Every other source ignores it.
    ///
    /// WP11a: this is the ONLY producer of [`ResolvedGenInputs`]. The bundle
    /// has private fields and no `Default`, so every caller — this crate, the
    /// GUI worker door, a future one — reads the inputs from here or builds
    /// no bundle at all. Add no second producer; extend this one.
    pub fn resolve_generation_inputs(
        &self,
        index: usize,
        cancel: &AtomicBool,
    ) -> Result<ResolvedGenInputs, SessionError> {
        let tc = self
            .toolpath_configs
            .get(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;

        let tool = self
            .find_tool_by_raw_id(tc.tool_id)
            .ok_or(SessionError::ToolNotFound(ToolId(tc.tool_id)))?
            .clone();

        let model = self.find_model_by_raw_id(tc.model_id);

        let mut mesh = model.and_then(|m| m.mesh.clone());
        let mut polygons = model.and_then(|m| m.polygons.clone());

        // ProjectCurve optionally references a separate surface model for
        // the mesh (so polygons can come from a DXF while the projection
        // target is a terrain STL). Match the GUI compute path.
        if let crate::compute::OperationConfig::ProjectCurve(ref cfg) = tc.operation
            && let Some(surface_id) = cfg.surface_model_id
            && let Some(surface) = self.find_model_by_raw_id(surface_id.0)
            && surface.mesh.is_some()
        {
            mesh = surface.mesh.clone();
        }

        // N12 items 1 and 2: the picked BREP faces.
        //
        // A STEP model carries a mesh and an enriched mesh, never polygons,
        // so a 2D operation over picked faces takes its geometry from the
        // face outline. The GUI controller derived this and the resolver
        // did not, so the two doors disagreed about what geometry a face
        // pick even has, and the geometry check below refused on this door.
        //
        // Derived BEFORE the setup transform, so the outline rides into the
        // emission frame with every other polygon.
        let mut face_boundary: Option<crate::polygon::Polygon2> = None;
        let mut face_top_z: Option<f64> = None;
        if let (Some(face_ids), Some(enriched)) = (
            tc.face_selection.as_ref(),
            model.and_then(|m| m.enriched_mesh.as_ref()),
        ) && !face_ids.is_empty()
        {
            match enriched.faces_boundary_as_polygon(face_ids) {
                Some(poly) => {
                    // The face tops, in the model's own frame — the frame
                    // the GUI controller read them in. A non-identity setup
                    // moves the geometry and not this number; that is the
                    // behaviour being moved, not a new one.
                    let top = face_ids
                        .iter()
                        .filter_map(|fid| enriched.face_group(*fid))
                        .map(|group| group.bbox.max.z)
                        .fold(f64::NEG_INFINITY, f64::max);
                    if top.is_finite() {
                        face_top_z = Some(top);
                    }
                    if polygons.is_none() {
                        polygons = Some(Arc::new(vec![poly.clone()]));
                    }
                    face_boundary = Some(poly);
                }
                None => {
                    tracing::warn!(
                        toolpath = %tc.name,
                        "Selected faces produced no boundary polygon (they are \
                         not horizontal planes); ignoring the face pick"
                    );
                }
            }
        }

        // Validate geometry requirements
        if tc.operation.is_3d() && mesh.is_none() {
            return Err(SessionError::MissingGeometry(
                "Operation requires a 3D mesh (STL/STEP)".to_owned(),
            ));
        }
        if !tc.operation.is_3d() && !tc.operation.is_stock_based() && polygons.is_none() {
            return Err(SessionError::MissingGeometry(
                "Operation requires 2D geometry (SVG/DXF)".to_owned(),
            ));
        }

        // Find the setup for orientation and keep-out info.
        // F-030: route every frame-derived value (stock bbox, transform,
        // safe_z, heights bbox) through a single `SetupEvalContext`. The
        // 5 historical sites that re-derived these ad hoc are now thin
        // wrappers over the same builder.
        let setup = self.find_setup_for_toolpath_index(index);
        // The two lateral-setup preconditions (no mesh to register against;
        // keep-outs that a vertical work plane cannot express). Checked
        // BEFORE any geometry work so the refusal names the setup rather
        // than whatever the collapsed geometry happened to break first.
        // Single owner — the GUI controller calls the same method before it
        // submits to the worker.
        self.check_lateral_setup_support(setup, &tc.operation)?;
        let ctx = super::SetupEvalContext::build_for_setup(self, setup);
        let face_up = ctx.face_up;
        let z_rotation = ctx.z_rotation;

        // Collect keep-out footprints from setup fixtures and keep-out zones
        let mut keep_out_footprints: Vec<crate::polygon::Polygon2> = Vec::new();
        if let Some(s) = setup {
            for fixture in &s.fixtures {
                if fixture.enabled {
                    keep_out_footprints.push(fixture.footprint());
                }
            }
            for keep_out in &s.keep_out_zones {
                if keep_out.enabled {
                    keep_out_footprints.push(keep_out.footprint());
                }
            }
        }

        // Clone boundary config before we lose the borrow on tc
        let boundary_config = tc.boundary.clone();

        // ── Setup transforms ──────────────────────────────────────────
        // When a setup has non-identity face_up or z_rotation, transform
        // mesh and polygons into setup-local coordinates (matching the GUI
        // compute path).
        if ctx.needs_transform() {
            if let Some(raw_mesh) = mesh.as_ref() {
                // G8: memoised on (source mesh identity, full transform).
                // This used to deep-copy the mesh per toolpath — ~111 MB on
                // the reference terrain, ~95 MB of it the re-derived `faces`
                // array. Returning a *shared* Arc is also what lets the
                // spatial-index memo below hit on a non-identity setup: a
                // fresh Arc per toolpath would miss however it was keyed.
                mesh = Some(crate::maps::geom_cache::cached_transform(
                    raw_mesh,
                    &self.setup_transform_info(face_up, z_rotation),
                ));
            }
            if let Some(raw_polygons) = polygons.as_ref() {
                // The model's DRAWING — consumed in the work plane of the
                // setup that uses it. Distinct door from the footprints
                // below; they differ only on lateral setups.
                polygons = Some(Arc::new(self.transform_drawing_polygons_to_setup(
                    raw_polygons,
                    face_up,
                    z_rotation,
                )));
            }
            // The face outline is drawing geometry too, and it is read
            // again by the boundary clip after generation, so it takes the
            // same door.
            if let Some(face) = face_boundary.as_ref() {
                face_boundary = self
                    .transform_drawing_polygons_to_setup(
                        std::slice::from_ref(face),
                        face_up,
                        z_rotation,
                    )
                    .into_iter()
                    .next();
            }
            // Keep-out footprints are world-anchored hardware, not drawings,
            // so they keep the orthographic world→local projection.
            // (Unreachable with a non-empty list on a lateral setup —
            // `check_lateral_setup_support` refuses that above.)
            if !keep_out_footprints.is_empty() {
                keep_out_footprints =
                    self.transform_footprints_to_setup(&keep_out_footprints, face_up, z_rotation);
            }
        }

        // Stock bbox in the frame the toolpath is emitted in (world for
        // identity setups, zero-rooted local for non-identity). Ops read
        // `OpContext::stock_bbox` for depth anchoring (adaptive3d's
        // `stock_top_z`, face/drill tops), boundary resolution, and dressup
        // stock-top clamps — all of which must live in the emission frame.
        // The GUI controller already forwards this frame
        // (`controller/events/compute.rs` passes `ctx.heights_stock_bbox`
        // as the request bbox); pre-fix this path passed the zero-rooted
        // `local_stock_bbox` even for identity setups, so CLI/session
        // generation diverged from the GUI by `-origin` whenever
        // `stock.origin != 0` (heights/setup-frame audit 2026-06-12,
        // finding 4).
        let emission_stock_bbox = ctx.heights_stock_bbox;

        // F-028 (2026-05-25) established this frame for the height context
        // (heights.top_z anchors 2.5D depth stepping and must match where
        // the toolpath actually emits); the 2026-06-12 audit extended it to
        // the op/boundary/dressup bbox above, which had been left on the
        // zero-rooted local bbox.

        // Resolve heights. `effective_safe_z` floors the user-configured
        // `post.safe_z` at `stock_top + clearance` so rapids clear the stock.
        // That floor reads the same emission-frame bbox as `stock_top_z`
        // below; it used to read the zero-rooted local bbox, which for
        // `origin_z > SAFE_Z_CLEARANCE_MM` put the retract plane inside the
        // material and for `origin_z < 0` lifted it well above the stock
        // (G-SAFEZ-LOCAL). Provided by `SetupEvalContext::safe_z`.
        let safe_z = ctx.safe_z;

        let model_bbox = mesh.as_ref().map(|m| &m.bbox);
        let height_ctx = HeightContext {
            safe_z,
            op_depth: tc.operation.default_depth_for_heights(),
            stock_top_z: emission_stock_bbox.max.z,
            stock_bottom_z: emission_stock_bbox.min.z,
            model_top_z: model_bbox.map(|b| b.max.z),
            model_bottom_z: model_bbox.map(|b| b.min.z),
        };
        let mut heights = tc.heights.resolve(&height_ctx);
        // N12 item 1: an AUTO top follows the picked faces. The GUI
        // controller applied this override and the resolver did not. A
        // PINNED top is the operator's own answer and stays.
        if let Some(top) = face_top_z
            && tc.heights.top_z.is_auto()
        {
            heights.top_z = top;
            if tc.heights.bottom_z.is_auto() {
                heights.bottom_z = top - tc.operation.default_depth_for_heights().abs();
            }
        }

        // Build tool definition
        let tool_def = build_cutter(&tool);

        // Spatial index for 3D ops. G8: memoised per mesh identity — this
        // ran once per toolpath, so an 8-op `generate_all` rebuilt the
        // 661 k-triangle grid eight times, multiplied again by every
        // fixpoint round. `build_auto`'s cell size is a pure function of the
        // mesh, so mesh identity is the whole key; see `geom_cache`'s module
        // doc for why identity is keyed on a `Weak` and not a raw pointer.
        //
        // WP11b: this resolver runs on the GUI frame loop now, so it hands
        // out a LAZY index and builds nothing. The executor forces it on
        // the worker thread. The one arm below that needs a built index —
        // a `PlannedTierRegions` boundary, which walks the tier map here —
        // forces it explicitly and says so.
        let spatial_index = mesh.as_ref().map(crate::maps::geom_cache::lazy_auto_index);

        // Compute cutting levels from the operation config (empty for 3D ops,
        // actual depth levels for 2D ops like Profile, Pocket, Adaptive, etc.)
        let cutting_levels = tc.operation.cutting_levels(heights.top_z);

        // For Rest machining, resolve prev_tool_radius from the RestConfig's
        // prev_tool_id, matching the GUI compute path.
        let prev_tool_radius = if let crate::compute::OperationConfig::Rest(ref cfg) = tc.operation
        {
            cfg.prev_tool_id.and_then(|prev_id| {
                self.tools
                    .iter()
                    .find(|t| t.id == prev_id)
                    .map(|t| t.diameter / 2.0)
            })
        } else {
            None
        };

        // R1 (pencil): resolve the real reference tool config from the Pencil
        // op's `reference_tool_id`, mirroring the prev_tool_radius resolution
        // above. `None` (unset id, or id not found) falls back to the nominal
        // `reference_tool_diameter` ball downstream — never an error.
        //
        // P2.5: non-Pencil ops with `rest_analysis` enabled resolve their
        // reference tool the same way, from `RestAnalysisConfig::reference_tool_id`
        // — same slot, same fallback semantics (`None` = self-referenced probe
        // downstream in `attach_generic_rest_analysis`, never an error).
        let reference_tool_cfg =
            if let crate::compute::OperationConfig::Pencil(ref cfg) = tc.operation {
                cfg.reference_tool_id.and_then(|ref_id| {
                    let found = self.tools.iter().find(|t| t.id == ref_id).cloned();
                    if found.is_none() {
                        tracing::warn!(
                            ?ref_id,
                            "Pencil reference_tool_id not found in tool list; \
                             falling back to nominal reference diameter"
                        );
                    }
                    found
                })
            } else if tc.rest_analysis.enabled {
                tc.rest_analysis.reference_tool_id.and_then(|ref_id| {
                    let found = self.tools.iter().find(|t| t.id == ref_id).cloned();
                    if found.is_none() {
                        tracing::warn!(
                            ?ref_id,
                            "RestAnalysis reference_tool_id not found in tool list; \
                             falling back to self-referenced probe"
                        );
                    }
                    found
                })
            } else {
                None
            };

        // Clone operation so we can patch `setup_z_flipped` on ProjectCurve.
        // This flag is #[serde(skip)] and set at compute time — single source of
        // truth is the setup transform's `is_z_flipped()`.
        let mut operation = tc.operation.clone();
        if let crate::compute::OperationConfig::ProjectCurve(ref mut cfg) = operation {
            // F-030: `SetupEvalContext::is_z_flipped()` returns false for
            // identity setups and `xform.is_z_flipped()` otherwise — same
            // combined predicate as the previous `needs_transform && xform.is_z_flipped()`.
            cfg.setup_z_flipped = ctx.is_z_flipped();
        }
        // N12 item 9: the alignment-pin drill's holes come from the LIVE
        // stock, never from the stored config. The GUI controller
        // refreshed them at submit and this resolver did not, so the two
        // doors drilled different holes after a pin edit. Patched on the
        // compute-time copy beside `setup_z_flipped`, for the same reason:
        // the stored config records the operator's intent and this is a
        // derived value.
        if let crate::compute::OperationConfig::AlignmentPinDrill(ref mut cfg) = operation {
            cfg.holes = self
                .stock
                .alignment_pins
                .iter()
                .map(|pin| [pin.x, pin.y])
                .collect();
        }

        // Pre-resolve the effective boundary polygon so adaptive3d can
        // pre-clip its internal stock. `apply_boundary_clip` (below) resolves
        // its source polygon through this same `resolve_containment_polygon`
        // call (S.9 dedup — the two used to carry independent copies of this
        // computation, which is why they could drift). Doing this before
        // generation rather than after avoids the "cut moves outside
        // boundary become rapids" failure mode that left dexel cells
        // unstamped in deep passes.
        // `DerivedRestRegions` resolves to a *set* of disjoint polygons, but
        // adaptive3d's internal-stock pre-clip wants a single containment
        // polygon. v1 limitation: union the per-region polygons (keep-outs +
        // offset already applied) and use the result only if it collapses to
        // exactly one polygon; otherwise skip the pre-clip entirely and rely
        // on `apply_boundary_clip_multi`'s post-generation clip to enforce
        // the real boundary (this only costs adaptive3d some discarded
        // pre-clearing, not correctness).
        let mut pre_boundary_regions: Option<Vec<crate::polygon::Polygon2>> = None;
        let pre_boundary: Option<crate::polygon::Polygon2> = if boundary_config.enabled {
            if let crate::compute::config::BoundarySource::PlannedTierRegions {
                tool_ids,
                tier,
                cell_mm,
                tolerance_mm,
                margin_mm,
                treatment,
                islands,
            } = &boundary_config.source
            {
                // Unlike the `DerivedRestRegions` arm below, a failure here
                // is PROPAGATED rather than logged-and-skipped. That arm can
                // afford to fall back because its post-generation clip
                // re-resolves the same regions and refuses there; this one
                // resolves the boundary the mesh-finish family DECOMPOSES
                // against (`unified_finish`'s pre-decompose seam), so
                // continuing without it would silently run the fine tier
                // over the whole board — the exact un-confinement the
                // variant exists to prevent.
                // The tier map reads a BUILT index, so this arm forces
                // the lazy one. It is the only production caller that
                // builds on the frame loop, and it already walks a grid
                // there.
                let tier_index = spatial_index.as_ref().map(|lazy| Arc::clone(lazy.force()));
                let regions = self.resolve_planned_tier_region_polys(
                    &tc.name,
                    mesh.as_ref(),
                    tier_index.as_ref(),
                    &super::multitool::PlannedTierRecipe {
                        tool_ids,
                        tier: *tier,
                        cell_mm: *cell_mm,
                        tolerance_mm: *tolerance_mm,
                        margin_mm: *margin_mm,
                        treatment: *treatment,
                        islands: *islands,
                    },
                    cancel,
                )?;
                // Same processed pipeline as `DerivedRestRegions`, so the
                // keep-out subtraction and the user offset are applied once,
                // here, and the post-generation clip re-derives the same set
                // off the same memoised map.
                let processed_set = crate::geometry::region_set::RegionSet::from_slice(&regions)
                    .processed(&keep_out_footprints, boundary_config.offset);
                let single = processed_set.single_union();
                pre_boundary_regions = Some(processed_set.as_slice().to_vec());
                single
            } else if let crate::compute::config::BoundarySource::DerivedRestRegions {
                source_toolpath_id,
            } = &boundary_config.source
            {
                match self.resolve_derived_rest_region_polys(index, *source_toolpath_id) {
                    Ok(regions) => {
                        let processed_set =
                            crate::geometry::region_set::RegionSet::from_slice(&regions)
                                .processed(&keep_out_footprints, boundary_config.offset);
                        let single = processed_set.single_union();
                        let region_count = processed_set.len();
                        // P2.3: share this exact `processed` set with the
                        // mesh-finish family's pre-clip — it's the same set
                        // `apply_boundary_clip_multi` re-derives for the
                        // post-generation clip, resolved here once rather
                        // than a third time just for this field.
                        pre_boundary_regions = Some(processed_set.as_slice().to_vec());
                        if single.is_some() {
                            single
                        } else {
                            tracing::debug!(
                                region_count = region_count,
                                "DerivedRestRegions pre-boundary union did not collapse to a \
                                 single polygon; skipping adaptive3d pre-clip (the \
                                 post-generation boundary clip still enforces the real \
                                 boundary)"
                            );
                            None
                        }
                    }
                    Err(e) => {
                        tracing::debug!(
                            error = %e,
                            "DerivedRestRegions source unavailable while resolving \
                             pre-boundary; skipping adaptive3d pre-clip"
                        );
                        None
                    }
                }
            } else {
                // D-3b: a containment whose user offset FAILED refuses here
                // too. The adaptive3d pre-clip is an optimisation, but it
                // shares its source polygon with the real clip below (see
                // this function's doc), so letting the two disagree about
                // whether the boundary is computable is how a pre-clip and
                // an enforcement clip end up bounding different regions.
                Self::resolve_containment_polygon(
                    &boundary_config,
                    &emission_stock_bbox,
                    mesh.as_ref(),
                    face_boundary.as_ref(),
                    &keep_out_footprints,
                )
                .map_err(|e| SessionError::OperationFailed(e.to_string()))?
            }
        } else {
            None
        };

        Ok(ResolvedGenInputs {
            setup_transform: ctx.local_to_global,
            tool,
            mesh,
            polygons,
            drill_targets: model
                .map(|m| Arc::clone(&m.drill_targets))
                .unwrap_or_default(),
            keep_out_footprints,
            boundary_config,
            emission_stock_bbox,
            heights,
            tool_def,
            spatial_index,
            cutting_levels,
            prev_tool_radius,
            reference_tool_cfg,
            operation,
            pre_boundary,
            pre_boundary_regions,
            face_boundary,
        })
    }

    /// Resolve the polygon set for `BoundarySource::DerivedRestRegions`,
    /// or a `SessionError::OperationFailed` naming exactly what's missing.
    ///
    /// Shared by the fail-hard precondition in [`Self::generate_toolpath`]
    /// (checked before any geometry work) and the boundary resolution in
    /// [`Self::resolve_generation_inputs`] / the post-dressup clip — all
    /// three call sites must agree on what "the derived regions" are, so
    /// this is the only place that reads `self.results` for it.
    ///
    /// `this_index` is the index of the toolpath *being generated* (whose
    /// boundary references `source_toolpath_id`); it is only used to reject
    /// a toolpath referencing its own regions as its boundary.
    pub(crate) fn resolve_derived_rest_region_polys(
        &self,
        this_index: usize,
        source_toolpath_id: crate::ids::ToolpathId,
    ) -> Result<Arc<Vec<crate::polygon::Polygon2>>, SessionError> {
        let Some(source_index) = self
            .toolpath_configs
            .iter()
            .position(|tc| tc.id == source_toolpath_id)
        else {
            return Err(SessionError::OperationFailed(format!(
                "Boundary references toolpath id {source_toolpath_id} for its rest regions, \
                 but no toolpath with that id exists anymore. Pick a different source toolpath \
                 for the boundary, or disable the boundary.",
            )));
        };

        if source_index == this_index {
            return Err(SessionError::OperationFailed(
                "Boundary references this toolpath's own rest regions — a toolpath cannot use \
                 itself as the source for a derived-rest-regions boundary. Pick a different \
                 source toolpath."
                    .to_owned(),
            ));
        }

        let Some(source_tc) = self.toolpath_configs.get(source_index) else {
            // Unreachable in practice: `source_index` came from `position()`
            // on this same Vec a few lines above.
            return Err(SessionError::ToolpathNotFound(source_index));
        };
        let source_name = &source_tc.name;

        let Some(result) = self.results.get(&source_index) else {
            return Err(SessionError::OperationFailed(format!(
                "'{source_name}' has no generated result yet — generate '{source_name}' first; \
                 its rest analysis produces the regions this boundary needs.",
            )));
        };

        match result.annotated().rest_regions.as_ref() {
            Some(regions) if !regions.is_empty() => Ok(Arc::clone(regions)),
            _ => Err(SessionError::OperationFailed(format!(
                "'{source_name}' produced no rest regions. Its rest analysis found no material \
                 above the threshold, or it is switched off. Check the rest analysis settings \
                 on '{source_name}' and regenerate it.",
            ))),
        }
    }

    /// Resolve the boundary "containment polygon" — the polygon the cutter's
    /// footprint must stay inside (Containment=Inside) or outside (Outside).
    /// For ModelSilhouette source this returns the OUTER LOOP of the
    /// silhouette (after keep-outs and user offset). The holes are dropped
    /// here, in `silhouette_machining_outline`, before the polygon reaches
    /// adaptive3d's pre-clip and the post-generation clip: a through-hole
    /// is material for a machining boundary, and a keep-out is the
    /// mechanism for "do not cut here" (R1,
    /// `planning/corne_case_analysis_2026-09-18/ANALYSIS.md` §3, §5).
    /// The downstream toolpath clip
    /// (`clip_toolpath_to_boundary`) does its own tool-radius inset to gate
    /// CUTTER CENTER positions — but for adaptive3d's internal-stock
    /// pre-clip we want the silhouette itself, since the cutter footprint
    /// (when its center is at silhouette - tool_radius) reaches the
    /// silhouette boundary and validly stamps cells in that band.
    ///
    /// Not used for `BoundarySource::DerivedRestRegions` — that source can
    /// resolve to multiple disjoint polygons, which this single-polygon
    /// signature can't represent. See
    /// [`Self::resolve_derived_rest_region_polys`] +
    /// [`crate::geometry::region_set::RegionSet::processed`] for that source's path,
    /// wired in by the two call sites below (`resolve_generation_inputs`'s
    /// `pre_boundary` and `generate_toolpath`'s post-dressup clip).
    pub(crate) fn resolve_containment_polygon(
        boundary_config: &crate::compute::config::BoundaryConfig,
        stock_bbox: &BoundingBox3,
        mesh: Option<&Arc<crate::mesh::TriangleMesh>>,
        // N12 item 2: the XY outline of the picked BREP faces, resolved
        // once by `resolve_generation_inputs`. `None` means the toolpath
        // picks no face, so a `FaceSelection` boundary takes the stock
        // rectangle — the same fallback every other unavailable source
        // takes.
        //
        // The GUI worker used to test the PICK and not the declared
        // source, so a face pick overrode `ModelSilhouette` and `Stock`
        // alike. The declared source decides here.
        face_boundary: Option<&crate::polygon::Polygon2>,
        keep_out_footprints: &[crate::polygon::Polygon2],
    ) -> Result<Option<crate::polygon::Polygon2>, crate::compute::execute::OperationError> {
        use crate::compute::config::BoundarySource;
        use crate::geometry::boundary::{
            UserOffsetOutcome, apply_user_boundary_offset, subtract_keepouts,
        };

        let mut stock_poly = match (&boundary_config.source, mesh, face_boundary) {
            (BoundarySource::FaceSelection, _, Some(face)) => face.clone(),
            (BoundarySource::ModelSilhouette, Some(m), _) => {
                // G8: memoised per mesh identity. This ran twice per toolpath
                // (pre-boundary resolution + the post-generation enforcement
                // clip), each time rasterising every face of the mesh.
                let silhouettes = crate::maps::geom_cache::cached_silhouette(m);
                // R1: the outer loop only. The raw silhouette keeps a
                // through-hole as a hole; the boundary treats it as material.
                crate::geometry::boundary::silhouette_machining_outline(&silhouettes)
                    .unwrap_or_else(|| {
                        crate::polygon::Polygon2::rectangle(
                            stock_bbox.min.x,
                            stock_bbox.min.y,
                            stock_bbox.max.x,
                            stock_bbox.max.y,
                        )
                    })
            }
            _ => crate::polygon::Polygon2::rectangle(
                stock_bbox.min.x,
                stock_bbox.min.y,
                stock_bbox.max.x,
                stock_bbox.max.y,
            ),
        };
        if !keep_out_footprints.is_empty() {
            stock_poly = subtract_keepouts(&stock_poly, keep_out_footprints);
        }
        if boundary_config.offset.abs() > 1e-9 {
            // Checkpoint C, D-3b (F-8). This used to be
            // `if let Some(largest) = ... { stock_poly = largest }` with no
            // `else` — on an empty result the requested offset silently did
            // not happen and `stock_poly` kept its UN-OFFSET value. For a
            // negative offset that is an over-cut: the path ends up clipped
            // to a larger region than the operator asked for.
            match apply_user_boundary_offset(&stock_poly, boundary_config.offset) {
                UserOffsetOutcome::Resolved(p) => stock_poly = p,
                // D-3c: dropped, not un-offset — the multi-region path's
                // semantics (`RegionSet::processed`), so the two agree.
                UserOffsetOutcome::Collapsed => return Ok(None),
                UserOffsetOutcome::Failed(failure) => {
                    return Err(crate::compute::execute::OperationError::MissingGeometry(
                        format!(
                            "the machining boundary's {offset:+.3} mm offset could \
                             not be computed: {reason}. Refusing rather than \
                             continuing with the UN-OFFSET boundary, which would \
                             clip this toolpath to a larger region than was asked \
                             for. Repair the boundary geometry, or set the offset \
                             to zero.",
                            offset = boundary_config.offset,
                            reason = failure.describe(),
                        ),
                    ));
                }
            }
        }
        Ok(Some(stock_poly))
    }

    /// Apply boundary clipping to a toolpath, subtracting keep-out footprints.
    ///
    /// Takes/returns an [`AnnotatedToolpath`]. Spans are precisely remapped
    /// through the clip via the provenance map returned from
    /// [`crate::geometry::boundary::clip_toolpath_to_boundary_with_provenance`]; the
    /// clipper never drops input moves, only inserts retract/rapid pairs
    /// between them, so a Region span that originally covered "the moves
    /// doing the cut for region X" still covers them post-clip plus any
    /// retracts inserted into the middle. `spans_valid` stays `true`.
    ///
    /// `plunge_rate_mm_min` is the operation's own plunge rate, used for the
    /// re-entry descent the clipper emits (G-BOUNDARYPLUNGE) — see
    /// [`crate::geometry::boundary::clip_toolpath_to_boundary_set_with_provenance`].
    #[allow(clippy::too_many_arguments)]
    pub fn apply_boundary_clip(
        annotated: crate::trace::toolpath_spans::AnnotatedToolpath,
        boundary_config: &crate::compute::config::BoundaryConfig,
        stock_bbox: &BoundingBox3,
        // G8: `&Arc` rather than `&TriangleMesh` so a `ModelSilhouette`
        // boundary can reuse the per-mesh silhouette memo instead of
        // re-rasterising every face on every toolpath. The mesh is an `Arc`
        // at every production call site already; identity is the memo key.
        mesh: Option<&Arc<crate::mesh::TriangleMesh>>,
        // N12 item 2: the picked BREP faces' XY outline, for a
        // `BoundarySource::FaceSelection` boundary. `None` means no face is
        // picked; see `resolve_containment_polygon`.
        face_boundary: Option<&crate::polygon::Polygon2>,
        keep_out_footprints: &[crate::polygon::Polygon2],
        tool_diameter: f64,
        safe_z: f64,
        plunge_rate_mm_min: Option<f64>,
        semantic_ctx: &crate::trace::semantic_trace::ToolpathSemanticContext,
        channels: &mut crate::trace::transform_provenance::ReconcileSet<'_>,
        findings: &mut crate::compute::execute::GenerationFindings,
    ) -> Result<
        crate::trace::toolpath_spans::AnnotatedToolpath,
        crate::compute::execute::OperationError,
    > {
        use crate::geometry::boundary::{
            ToolContainment, clip_annotated_to_boundary_set, effective_boundary_reported,
        };

        // Resolve the source polygon for the boundary (ModelSilhouette /
        // FaceSelection fall back to the stock rectangle when the required
        // geometry isn't available), subtract keep-outs, and apply the
        // user-configured offset. Shared with the adaptive3d pre-clip path
        // in `resolve_generation_inputs` — see that function's doc comment
        // for why the two must agree on the source polygon. `stock_bbox` is
        // always provided here, so the rectangle fallback inside
        // `resolve_containment_polygon` is unreachable in practice; kept for
        // parity with that function's `Option` signature.
        // Checkpoint C, D-3b: `None` now means the user offset COLLAPSED the
        // containment, and the old `.unwrap_or_else(|| stock rectangle)`
        // would have resurrected the very un-offset boundary the collapse
        // says is wrong. A collapsed containment is a collapsed containment
        // wherever it happens, so it takes the same ruled decision as an
        // empty `effective_boundary`.
        let Some(stock_poly) = Self::resolve_containment_polygon(
            boundary_config,
            stock_bbox,
            mesh,
            face_boundary,
            keep_out_footprints,
        )?
        else {
            Self::resolve_collapsed_containment(
                None,
                boundary_config.containment,
                tool_diameter,
                1,
                findings,
            )?;
            return Ok(
                clip_annotated_to_boundary_set(annotated, &[], safe_z, plunge_rate_mm_min)
                    .reconcile(channels)
                    .into_inner(),
            );
        };

        // Map BoundaryContainment -> ToolContainment.
        let containment = match boundary_config.containment {
            crate::compute::config::BoundaryContainment::Center => ToolContainment::Center,
            crate::compute::config::BoundaryContainment::Inside => ToolContainment::Inside,
            crate::compute::config::BoundaryContainment::Outside => ToolContainment::Outside,
        };

        let tool_radius = tool_diameter / 2.0;
        let (boundaries, offset_failure) =
            effective_boundary_reported(&stock_poly, containment, tool_radius);
        // Checkpoint C, Q2 (F-1). An empty `boundaries` means the set clipper
        // passes the toolpath through with an identity mapping — i.e. the
        // containment the operator asked for is NOT APPLIED. That is correct
        // for one cause and an unbounded over-cut for the other, and until
        // Checkpoint C nothing here could tell them apart.
        if boundaries.is_empty() {
            Self::resolve_collapsed_containment(
                offset_failure,
                boundary_config.containment,
                tool_diameter,
                1,
                findings,
            )?;
        }
        // Checkpoint C, D-3c: the WHOLE set, not `boundaries.first()`. A
        // containment offset that splits its source into several polygons
        // used to keep piece 1 and clip everything outside it away — an
        // under-cut nobody chose, and the multi-region path at
        // `apply_boundary_clip_multi` already disagreed by keeping them all.
        // The multi-region semantics win: membership downstream is "inside
        // ANY", which is what a split containment means.
        let clipped =
            clip_annotated_to_boundary_set(annotated, &boundaries, safe_z, plunge_rate_mm_min)
                .reconcile(channels)
                .into_inner();

        // Recorded AFTER the reconcile so this item's own link is bound to
        // post-clip indices and is not then remapped a second time.
        if !boundaries.is_empty() {
            let clip_scope =
                semantic_ctx.start_item(ToolpathSemanticKind::BoundaryClip, "Boundary clip");
            clip_scope.set_param(
                SemanticKey::Containment,
                match boundary_config.containment {
                    crate::compute::config::BoundaryContainment::Center => "center",
                    crate::compute::config::BoundaryContainment::Inside => "inside",
                    crate::compute::config::BoundaryContainment::Outside => "outside",
                },
            );
            clip_scope.set_param(SemanticKey::KeepOutCount, keep_out_footprints.len());
            if !clipped.toolpath.moves.is_empty() {
                clip_scope.bind_to_toolpath(&clipped.toolpath, 0, clipped.toolpath.moves.len());
            }
        }

        Ok(clipped)
    }

    /// The Checkpoint C (Q2) decision, in one place because both boundary
    /// clip paths must make it identically.
    ///
    /// An empty effective boundary is either a **genuine collapse** — the
    /// pass-through case `boundary::clip_annotated_to_boundary_set`'s
    /// contract was written for, where the tool is larger than the region and
    /// nothing there is machinable — or the residue of an offset that
    /// **failed**. Option (b) of D-3a: pass through on the first WITH a typed
    /// finding naming the containment that was dropped, refuse on the second.
    ///
    /// `Ok(())` means "pass through; the finding is recorded". `Err` stops the
    /// generate. Deliberately not a `bool`: the refusal has to be
    /// unignorable at the call site.
    pub fn resolve_collapsed_containment(
        offset_failure: Option<crate::polygon::OffsetFailure>,
        containment: crate::compute::config::BoundaryContainment,
        tool_diameter: f64,
        source_region_count: usize,
        findings: &mut crate::compute::execute::GenerationFindings,
    ) -> Result<(), crate::compute::execute::OperationError> {
        if let Some(failure) = offset_failure {
            // NOT a pass-through. The safety argument for emitting an
            // unclipped path — "nothing here is machinable anyway" — rests
            // entirely on the boundary having genuinely run out of geometry,
            // and a failure establishes exactly nothing about that.
            return Err(crate::compute::execute::OperationError::MissingGeometry(
                format!(
                    "boundary containment `{containment:?}` could not be \
                     computed: {reason}. Refusing to emit this toolpath: an \
                     empty containment is passed through UNCLIPPED, which is \
                     safe only when the boundary genuinely collapsed (tool \
                     larger than the region), and this one did not — it \
                     failed. Repair the boundary geometry (self-intersecting \
                     or pinched rings, repeated vertices, non-finite \
                     coordinates) or set the containment to `Center`.",
                    reason = failure.describe(),
                ),
            ));
        }
        crate::compute::execute::record_boundary_clip_dropped(
            findings,
            crate::compute::toolpath_stats::BoundaryClipDroppedFinding {
                containment,
                tool_diameter_mm: tool_diameter,
                source_region_count,
            },
        );
        tracing::warn!(
            ?containment,
            tool_diameter,
            source_region_count,
            "boundary containment collapsed — toolpath emitted with NO \
             boundary clip (genuine collapse, recorded as a finding)"
        );
        Ok(())
    }

    /// Multi-region variant of [`Self::apply_boundary_clip`] for
    /// `BoundarySource::DerivedRestRegions`, whose source resolves to a *set*
    /// of disjoint polygons rather than one containment polygon.
    ///
    /// `regions` are the raw rest regions from
    /// [`Self::resolve_derived_rest_region_polys`]; keep-out subtraction and
    /// the user offset are applied per-region here (via
    /// [`crate::geometry::region_set::RegionSet::processed`]), then each region runs
    /// through `effective_boundary` independently for the containment /
    /// tool-radius handling — a region that collapses under the inset is
    /// dropped from the set. If EVERY region collapses the boundary is
    /// treated as collapsed, same as the single-polygon path's empty
    /// `effective_boundary` case: the original toolpath is returned
    /// unchanged (identity span mapping) with a `tracing::warn!`.
    ///
    /// Span remapping contract is identical to [`Self::apply_boundary_clip`]
    /// — the set clipper never drops input moves, so `spans_valid` stays
    /// `true`.
    ///
    /// `plunge_rate_mm_min` is the operation's own plunge rate, used for the
    /// re-entry descent the clipper emits (G-BOUNDARYPLUNGE) — see
    /// [`crate::geometry::boundary::clip_toolpath_to_boundary_set_with_provenance`].
    #[allow(clippy::too_many_arguments)]
    pub fn apply_boundary_clip_multi(
        annotated: crate::trace::toolpath_spans::AnnotatedToolpath,
        boundary_config: &crate::compute::config::BoundaryConfig,
        regions: &[crate::polygon::Polygon2],
        keep_out_footprints: &[crate::polygon::Polygon2],
        tool_diameter: f64,
        safe_z: f64,
        plunge_rate_mm_min: Option<f64>,
        semantic_ctx: &crate::trace::semantic_trace::ToolpathSemanticContext,
        channels: &mut crate::trace::transform_provenance::ReconcileSet<'_>,
        findings: &mut crate::compute::execute::GenerationFindings,
    ) -> Result<
        crate::trace::toolpath_spans::AnnotatedToolpath,
        crate::compute::execute::OperationError,
    > {
        use crate::geometry::boundary::{
            ToolContainment, clip_annotated_to_boundary_set, effective_boundary_reported,
        };

        // Per-region keep-out subtraction + user offset (regions that
        // collapse under the offset are dropped), mirroring what
        // `resolve_containment_polygon` does to its single polygon.
        let processed = crate::geometry::region_set::RegionSet::from_slice(regions)
            .processed(keep_out_footprints, boundary_config.offset);

        // Map BoundaryContainment -> ToolContainment.
        let containment = match boundary_config.containment {
            crate::compute::config::BoundaryContainment::Center => ToolContainment::Center,
            crate::compute::config::BoundaryContainment::Inside => ToolContainment::Inside,
            crate::compute::config::BoundaryContainment::Outside => ToolContainment::Outside,
        };

        // Containment / tool-radius handling per region. `effective_boundary`
        // may split one region into several (or collapse it to none) — flatten
        // everything into one set; membership downstream is "inside ANY".
        let tool_radius = tool_diameter / 2.0;
        let mut boundaries: Vec<crate::polygon::Polygon2> = Vec::new();
        // Checkpoint C, Q2: the failure channel is aggregated across regions
        // the same way `offset_polygon_reported` aggregates across repaired
        // pieces — a library failure outranks a rejected input — so one bad
        // region cannot be hidden by a dozen clean ones.
        let mut offset_failure: Option<crate::polygon::OffsetFailure> = None;
        for region in processed.as_slice() {
            let (out, failure) = effective_boundary_reported(region, containment, tool_radius);
            boundaries.extend(out);
            if failure.is_some()
                && (offset_failure.is_none()
                    || failure
                        .as_ref()
                        .is_some_and(crate::polygon::OffsetFailure::is_library_failure))
            {
                offset_failure = failure;
            }
        }

        if boundaries.is_empty() {
            // Every region collapsed (offset/inset ate them all) — same
            // "boundary collapsed" semantics as the single-polygon path, and
            // now the same Checkpoint C decision: pass through with a typed
            // finding on a genuine collapse, refuse when an offset failed.
            Self::resolve_collapsed_containment(
                offset_failure,
                boundary_config.containment,
                tool_diameter,
                regions.len(),
                findings,
            )?;
        }

        let clipped =
            clip_annotated_to_boundary_set(annotated, &boundaries, safe_z, plunge_rate_mm_min)
                .reconcile(channels)
                .into_inner();

        // Recorded AFTER the reconcile so this item's own link is bound to
        // post-clip indices and is not then remapped a second time.
        if !boundaries.is_empty() {
            let clip_scope =
                semantic_ctx.start_item(ToolpathSemanticKind::BoundaryClip, "Boundary clip");
            clip_scope.set_param(
                SemanticKey::Containment,
                match boundary_config.containment {
                    crate::compute::config::BoundaryContainment::Center => "center",
                    crate::compute::config::BoundaryContainment::Inside => "inside",
                    crate::compute::config::BoundaryContainment::Outside => "outside",
                },
            );
            clip_scope.set_param(SemanticKey::KeepOutCount, keep_out_footprints.len());
            clip_scope.set_param(SemanticKey::RegionCount, boundaries.len());
            if !clipped.toolpath.moves.is_empty() {
                clip_scope.bind_to_toolpath(&clipped.toolpath, 0, clipped.toolpath.moves.len());
            }
        }

        Ok(clipped)
    }
}
