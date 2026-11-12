//! Generation-input resolution and boundary clipping.
//!
//! [`crate::session::ProjectSession::resolve_generation_inputs`] turns a
//! setup, a tool and an operation into the inputs an operation runs on; the
//! clip methods cut that geometry to the machining boundary. Split out of
//! `session/compute.rs` (P4).

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::geo::BoundingBox3;
use crate::session::{ProjectSession, SessionError};
use crate::trace::semantic_trace::{SemanticKey, ToolpathSemanticKind};

use super::ResolvedGenInputs;

impl ProjectSession {
    /// Resolve every per-generation input from session state for toolpath
    /// `index`: tool + cutter, geometry (mesh / polygons, setup-transformed),
    /// spatial index, resolved heights, cutting levels, emission-frame stock
    /// bbox, keep-outs, boundary + pre-clip polygon, rest-machining prev-tool
    /// radius, and the operation. A pure read of `self`
    /// returning a fully-owned [`ResolvedGenInputs`] so callers can generate
    /// one or many toolpaths off a single resolution (the strategy advisor's
    /// per-strategy candidates) without re-deriving frame-sensitive values.
    ///
    /// [`start_generate_toolpath`](Self::start_generate_toolpath) puts this
    /// on a [`GenerateToolpathHandle`](crate::session::compute::GenerateToolpathHandle), and [`execute_job`](crate::session::compute::execute_job) then owns the
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

        // R2: an op with no tool refuses with the fix (`NoTool`).
        let tool = self.tool_of(tc)?.clone();

        // P2.4: the model by its EXACT id. A toolpath whose model id names
        // no model reads no mesh and no item; its selection refuses
        // `MissingSource` with the id, and a 3D op refuses below.
        let model = self.toolpath_model(tc).ok();

        // P3.1: the toolpath model's mesh, or the surface model of a Project
        // Curve. The heights read the same mesh (`toolpath_mesh`).
        let mut mesh = self.toolpath_mesh(tc)?;

        // Validate geometry requirements. A 2D op needs no check here: the
        // prologue refuses an op whose selection gives it nothing to cut
        // (`OperationError::Selection`), with the item and the fix.
        if tc.operation.is_3d() && mesh.is_none() {
            if model.is_none() {
                return Err(SessionError::MissingGeometry(format!(
                    "Toolpath '{}' reads model id {}, but no model has that id. Set an \
                     existing model on the toolpath.",
                    tc.name, tc.model_id
                )));
            }
            return Err(SessionError::MissingGeometry(
                "Operation requires a 3D mesh (STL/STEP)".to_owned(),
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

        // K1: the obstacles of the setup, resolved ONCE in the setup frame
        // (the emission frame). A missing model refuses the generation and
        // names the obstacle; an obstacle never vanishes in silence.
        let obstacles = Arc::new(
            crate::compute::obstacle::ObstacleSet::resolve(
                setup.map(|s| s.obstacles.as_slice()).unwrap_or_default(),
                &self.models,
            )
            .map_err(SessionError::Obstacle)?,
        );
        // The grown footprints also leave the machining boundary, when one
        // is on, as the keep-outs always did.
        let keep_out_footprints: Vec<crate::polygon::Polygon2> = obstacles.footprints();

        // Clone boundary config before we lose the borrow on tc
        let boundary_config = tc.boundary.clone();

        // ── Setup transforms ──────────────────────────────────────────
        // When a setup has non-identity face_up or z_rotation, transform
        // the mesh into setup-local coordinates (matching the GUI compute
        // path). The drawing items take THE frame door of the selection.
        if ctx.needs_transform()
            && let Some(raw_mesh) = mesh.as_ref()
        {
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

        // Geometry selection (DESIGN 4.1, P2.3b): resolve the selection of
        // the op (`OperationConfig::selection`) against the items of the
        // models and the stock, then move the items into the setup frame
        // through THE frame door (`to_setup_frame`). The prologue runs
        // `view` over it. A face pick is a pick of the op selection (P2.6),
        // so it reaches the op through `resolve` and `view` as any other
        // item, and gives the auto top below. An op that holds no selection
        // (a 3D op) resolves to no item; its prologue reads no item. The
        // Face default reads only the stock, so a Face op on an STL or with
        // no model resolves with no `MissingSource` (P4.2).
        // P2.4: THE resolution (`resolve_selection_frames`) is the one the
        // `selection_status` query reads, so the two give the same refusal.
        // P4.7: generation BUILDS the flat-region faces of a mesh model that
        // the selection names a face on (`FaceAccess::Build`); the status
        // read only peeks. So a generated toolpath does not depend on
        // whether the GUI job filled the cache first.
        let selection = self
            .resolve_selection_frames(
                tc,
                super::super::SelectionTarget::Op,
                &ctx,
                super::super::selection_status::FaceAccess::Build,
            )?
            .setup;
        // DESIGN 7.2: the top of the picked faces, in the SETUP frame. An op
        // with no selection slot takes the faces of its boundary selection
        // (P4.1, ruling O-1 A: the behaviour before P2.6).
        let face_top_z = match selection.face_top_z() {
            Some(top) => Some(top),
            None => self.boundary_face_top_z(tc, &ctx)?,
        };

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
        // (G-SAFEZ-LOCAL). Provided by `SetupEvalContext::safe_z`, which the
        // height context reads.
        // P3.1: THE heights rule (`session/heights.rs`): the context of the
        // setup with the setup-frame mesh box, the resolver, then an auto top
        // follows the picked faces. The GUI and MCP read the same rule
        // through `toolpath_heights`.
        // P3.5: the lowest up face of the op selection is the deepest floor
        // under "each picked face".
        // UX-8: the face of the floor "A picked face". A key that does not
        // resolve refuses generation with its typed cause, as a lost pick.
        let floor_face =
            match self.floor_face_z(tc, &ctx, super::super::selection_status::FaceAccess::Build) {
                Some(Ok(z)) => Some(z),
                Some(Err(refusal)) => {
                    return Err(SessionError::Selection(Box::new(
                        refusal.for_op(tc.operation.op_type().label()),
                    )));
                }
                None => None,
            };
        let hole_planes = if matches!(
            tc.operation.capabilities().item_use,
            crate::compute::catalog::ItemUse::Holes { .. }
        ) {
            self.selection_status(tc, super::super::selection_status::SelectionTarget::Op)
                .map_or((None, None), |report| {
                    (report.hole_rim_bottom_z, report.hole_floor_bottom_z)
                })
        } else {
            (None, None)
        };
        // K6 (decision E1): a buried face on fresh stock is a warning; the
        // job records it (`GenerationFindings::buried_face`).
        let (heights, buried_face) = self.generation_heights(
            tc,
            &ctx,
            mesh.as_deref(),
            crate::session::heights::FacePlanes {
                top: face_top_z,
                bottom: selection.face_bottom_z(),
                floor_face,
                // R-B: the hole planes of the same report that the status
                // reads (status = generation).
                hole_rim_bottom: hole_planes.0,
                hole_floor_bottom: hole_planes.1,
            },
        )?;

        // Build tool definition
        let tool_def = crate::session::session_cutter(&tool)?;

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
        let cutting_levels = tc.operation.cutting_levels(&heights);

        // R1 (pencil): resolve the real reference tool config from the Pencil
        // op's `reference_tool_id`. `None` (unset id, or id not found) falls back to the nominal
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

        // W6.5: the resolver patches no config copy. The setup frame reaches
        // an op through `ExecutionContext::setup_transform` and the stock
        // pins through `ExecutionContext::alignment_pins`.
        let operation = tc.operation.clone();
        let alignment_pins = self.stock.alignment_pins.clone();

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
            } else if let crate::compute::config::BoundarySource::Selection { selection, holes } =
                &boundary_config.source
            {
                // A failure is PROPAGATED, as in the tier arm above. The
                // operator named the regions; a selection that resolves to
                // nothing must stop the generation, not widen it to the
                // stock.
                let regions = self.resolve_boundary_selection_polys(index, selection, *holes)?;
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
                    &keep_out_footprints,
                )?
            }
        } else {
            // K3: with the boundary off, the territory resolver gives the
            // 3D Rough planner its obstacle outline
            // (`compute::execute::territory`).
            None
        };

        // K1: the obstacles for this tool, one footprint per assembly band.
        // H1: the FIXED head only (no shoe: the shoe moves to fit the
        // toolpath), and the stock top for the shelf floor.
        let obstacle_view = Arc::new(
            obstacles
                .for_tool(
                    &tool_def.mounted(&crate::machine::head::HeadFit::fixed(&self.machine.head)),
                    Some(&emission_stock_bbox),
                )
                .map_err(SessionError::Obstacle)?,
        );

        // D50: the top of the stock the source left in its rest regions.
        let rest_top_z =
            match &boundary_config.source {
                crate::compute::config::BoundarySource::DerivedRestRegions {
                    source_toolpath_id,
                } if boundary_config.enabled => self.derived_rest_top_z(*source_toolpath_id),
                crate::compute::config::BoundarySource::Stock
                | crate::compute::config::BoundarySource::ModelSilhouette
                | crate::compute::config::BoundarySource::Selection { .. }
                | crate::compute::config::BoundarySource::DerivedRestRegions { .. }
                | crate::compute::config::BoundarySource::PlannedTierRegions { .. } => None,
            };

        // W2 (D69 §3): on an op that cuts the remaining stock, the smallest
        // axial step of the earlier enabled roughing ops of the setup (the
        // registry's `PassRole::Roughing`): the depth at which the rough cut
        // the walls. A finishing step (a Waterline `z_step`) is not a rough.
        let rest_axial_step_mm = (tc.stock_source
            == crate::compute::config::StockSource::FromRemainingStock)
            .then(|| self.earlier_rough_step(index))
            .flatten();

        // W4 (D69 §4): for a Waterline, what the job holds the part with
        // at the end of a through cut.
        let through_cut_hold = matches!(
            tc.operation,
            crate::compute::catalog::OperationConfig::Waterline(_)
        )
        .then(|| self.through_cut_hold(index));

        Ok(ResolvedGenInputs {
            setup_transform: ctx.local_to_global,
            tool,
            mesh,
            selection,
            keep_out_footprints,
            obstacles,
            obstacle_view,
            boundary_config,
            emission_stock_bbox,
            heights,
            tool_def,
            spatial_index,
            cutting_levels,
            reference_tool_cfg,
            operation,
            alignment_pins,
            pre_boundary,
            pre_boundary_regions,
            rest_top_z,
            rest_axial_step_mm,
            through_cut_hold,
            buried_face,
        })
    }

    /// W4 (D69 §4, operator ruling 2026-10-28): what the job holds the part
    /// with at the end of a through cut, for the Waterline at `index`.
    /// Derived from the project (the ops' configs and their resolved
    /// heights), never from a generated result, so a stale or missing
    /// result cannot change the Waterline. The first rule that applies:
    ///
    /// 1. A later enabled Profile of the setup with tabs (`tab_count > 0`
    ///    or hand tabs) whose floor reaches the stock bottom (to within
    ///    `FLAT_FACE_SNAP_MM`): the web is its tab top, floor plus
    ///    `tab_height` (the highest of several). The Waterline then never
    ///    cuts below a tab.
    /// 2. The lowest floor that an earlier enabled roughing op
    ///    (`PassRole::Roughing`) of the setup leaves on the outline: an op
    ///    whose resolved bottom is the model bottom (to within
    ///    `FLAT_FACE_SNAP_MM`; it cuts down to the part bottom), at its
    ///    bottom plus its axial stock to leave
    ///    (`OperationConfig::model_gouge_allowance`), when that floor stands
    ///    above the stock bottom (an earlier op that cut through holds no
    ///    web). A pocket that stops above the model bottom leaves a floor,
    ///    not a web, and is not read.
    /// 3. No web.
    ///
    /// An op whose heights do not resolve is not read.
    fn through_cut_hold(&self, index: usize) -> crate::compute::toolpath_stats::ThroughCutHold {
        use crate::compute::catalog::OperationConfig;
        use crate::compute::toolpath_stats::{ThroughCutHold, WaterlineWeb, WebSource};
        let no_hold = ThroughCutHold {
            web: None,
            tabbed_profile_follows: false,
        };
        let Some(setup) = self.find_setup_for_toolpath_index(index) else {
            return no_hold;
        };
        let order = setup.toolpath_indices();
        let Some(at) = order.iter().position(|&i| i == index) else {
            return no_hold;
        };
        let enabled = |i: &usize| self.toolpath_configs.get(*i).filter(|tc| tc.enabled);
        let tab_top = order
            .iter()
            .skip(at + 1)
            .filter_map(enabled)
            .filter_map(|tc| {
                let OperationConfig::Profile(profile) = &tc.operation else {
                    return None;
                };
                if profile.tab_count == 0 && profile.tab_points.is_empty() {
                    return None;
                }
                let h = self.toolpath_heights(tc).ok()?;
                (h.heights.bottom_z
                    <= h.context.stock_bottom_z + crate::ops::waterline::FLAT_FACE_SNAP_MM)
                    .then_some(h.heights.bottom_z + profile.tab_height)
            })
            .max_by(f64::total_cmp);
        if let Some(z_mm) = tab_top {
            return ThroughCutHold {
                web: Some(WaterlineWeb {
                    z_mm,
                    source: WebSource::TabbedProfile,
                }),
                tabbed_profile_follows: true,
            };
        }
        let earlier_floor = order
            .get(..at)
            .unwrap_or_default()
            .iter()
            .filter_map(enabled)
            .filter(|tc| {
                tc.operation.op_type().spec().feeds_pass_role == crate::feeds::PassRole::Roughing
            })
            .filter_map(|tc| {
                let h = self.toolpath_heights(tc).ok()?;
                // Only a rough that cuts down to the part bottom leaves a web
                // on the outline. A pocket that stops above the model bottom
                // leaves a floor, not a web.
                let to_part_bottom = h.context.model_bottom_z.is_some_and(|model_bottom| {
                    h.heights.bottom_z <= model_bottom + crate::ops::waterline::FLAT_FACE_SNAP_MM
                });
                if !to_part_bottom {
                    return None;
                }
                let leave = tc
                    .operation
                    .model_gouge_allowance(None)
                    .map_or(0.0, |a| a.leave_mm);
                Some((h.heights.bottom_z + leave, h.context.stock_bottom_z))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        ThroughCutHold {
            web: earlier_floor
                .filter(|&(floor, bottom)| {
                    floor > bottom + crate::ops::waterline::STOCK_BOTTOM_FLOOR_EPS_MM
                })
                .map(|(z_mm, _)| WaterlineWeb {
                    z_mm,
                    source: WebSource::EarlierFloor,
                }),
            tabbed_profile_follows: false,
        }
    }

    /// W2 (D69 §3): the smallest `OperationConfig::depth_per_pass` of the
    /// enabled roughing ops (`PassRole::Roughing`) before toolpath `index`
    /// in its setup, or `None` when no such op states one.
    fn earlier_rough_step(&self, index: usize) -> Option<f64> {
        let setup = self.find_setup_for_toolpath_index(index)?;
        setup
            .toolpath_indices()
            .iter()
            .take_while(|&&i| i != index)
            .filter_map(|&i| self.toolpath_configs.get(i))
            .filter(|tc| {
                tc.enabled
                    && tc.operation.op_type().spec().feeds_pass_role
                        == crate::feeds::PassRole::Roughing
            })
            .filter_map(|tc| tc.operation.depth_per_pass())
            .filter(|step| step.is_finite() && *step > 0.0)
            .min_by(f64::total_cmp)
    }

    /// D50: the top Z of the stock that the toolpath `source_toolpath_id`
    /// left in its rest regions (`Produced::rest_regions_top_z`), or `None`
    /// when it has no result or stated no top. The regions themselves come
    /// from [`Self::resolve_derived_rest_region_polys`], which refuses on a
    /// missing source; this read only bounds the levels.
    fn derived_rest_top_z(&self, source_toolpath_id: crate::ids::ToolpathId) -> Option<f64> {
        let source_index = self
            .toolpath_configs
            .iter()
            .position(|tc| tc.id == source_toolpath_id)?;
        self.results
            .get(&source_index)?
            .annotated()
            .produced
            .rest_regions_top_z
    }

    /// D53: refuse a `DerivedRestRegions` consumer at `index` whose tool is
    /// larger than the tool of `source_toolpath_id`, when its containment
    /// keeps the tool CENTRE in the regions (`Center`) or outside them
    /// (`Outside`).
    ///
    /// The source publishes tool-centre positions inside its own
    /// containment (`Produced::rest_regions`, D48). A centre there keeps a
    /// tool no larger than the source's inside the source's machining
    /// boundary; a larger tool can cut past it by the difference of the
    /// radii. `Inside` containment insets the regions by this op's own
    /// radius, so it is safe with any tool. The radii are the radii the
    /// containment itself uses (`ResolvedGenInputs::boundary_centre`: the
    /// built cutter's diameter over two). A tool that does not resolve is
    /// left to its own refusal.
    ///
    /// A config refusal, so it takes the generation precondition path
    /// (`SessionError::OperationFailed`, as the `DerivedRestRegions` source
    /// checks in [`Self::resolve_derived_rest_region_polys`]).
    pub(crate) fn refuse_larger_rest_consumer(
        &self,
        index: usize,
        source_toolpath_id: crate::ids::ToolpathId,
    ) -> Result<(), SessionError> {
        if self.toolpath_configs.get(index).is_none() {
            return Err(SessionError::ToolpathNotFound(index));
        }
        match self.larger_rest_consumer_text(index, source_toolpath_id) {
            Some(text) => Err(SessionError::OperationFailed(text)),
            None => Ok(()),
        }
    }

    /// G-C (§2 a): the D53 read door. `Some` when the tool of the toolpath
    /// at `index` is larger than the tool of `source_toolpath_id`, whatever
    /// the containment; the refusal of [`Self::refuse_larger_rest_consumer`]
    /// adds the containment. The radii are the radii the containment uses
    /// (the built cutter's diameter over two). A tool or a source that does
    /// not resolve gives `None`: it is left to its own refusal.
    #[must_use]
    pub fn larger_rest_consumer(
        &self,
        index: usize,
        source_toolpath_id: crate::ids::ToolpathId,
    ) -> Option<LargerRestConsumer> {
        let tc = self.toolpath_configs.get(index)?;
        let source = self
            .toolpath_configs
            .iter()
            .find(|s| s.id == source_toolpath_id)?;
        // The one radius rule of the containment, shared with the step 7.8
        // door (`add_geodesic_body_and_edges`).
        let own = self.built_tool_radius_mm(tc.tool_id)?;
        let theirs = self.built_tool_radius_mm(source.tool_id)?;
        (own > theirs).then(|| LargerRestConsumer {
            op_name: tc.name.clone(),
            source_toolpath_id,
            source_name: source.name.clone(),
            tool_radius_mm: own,
            source_tool_radius_mm: theirs,
        })
    }

    /// G-C: the D53 refusal text of the toolpath at `index` as it stands:
    /// its `DerivedRestRegions` source, its containment and its tool.
    /// `None` when the boundary is off, is not `DerivedRestRegions`, keeps
    /// the tool inside (`Inside`), or the tool is not larger.
    #[must_use]
    pub fn larger_rest_consumer_refusal(&self, index: usize) -> Option<String> {
        let tc = self.toolpath_configs.get(index)?;
        if !tc.boundary.enabled {
            return None;
        }
        let crate::compute::config::BoundarySource::DerivedRestRegions { source_toolpath_id } =
            tc.boundary.source
        else {
            return None;
        };
        self.larger_rest_consumer_text(index, source_toolpath_id)
    }

    /// The D53 refusal text for the containment of the toolpath at
    /// `index`, or `None` when it does not refuse.
    fn larger_rest_consumer_text(
        &self,
        index: usize,
        source_toolpath_id: crate::ids::ToolpathId,
    ) -> Option<String> {
        use crate::compute::config::BoundaryContainment;
        let tc = self.toolpath_configs.get(index)?;
        let containment = match tc.boundary.containment {
            BoundaryContainment::Inside => return None,
            BoundaryContainment::Center => "Center",
            BoundaryContainment::Outside => "Outside",
        };
        let larger = self.larger_rest_consumer(index, source_toolpath_id)?;
        Some(larger.refusal(containment))
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
        self.derived_rest_regions(this_index, source_toolpath_id)
            .map_err(|refusal| match refusal {
                RestRegionsLookup::Refused(refusal) => {
                    SessionError::OperationFailed(refusal.to_string())
                }
                RestRegionsLookup::NotFound(index) => SessionError::ToolpathNotFound(index),
            })
    }

    /// G-E (3): why the `DerivedRestRegions` boundary of the toolpath at
    /// `index` refuses generation, or `None` when it resolves (or the
    /// toolpath has no such boundary on). The same rule as
    /// [`Self::resolve_derived_rest_region_polys`]: its `Display` is the
    /// text generation refuses with, so a surface can match the two.
    #[must_use]
    pub fn rest_boundary_refusal(&self, index: usize) -> Option<RestBoundaryRefusal> {
        let tc = self.toolpath_configs.get(index)?;
        if !tc.boundary.enabled {
            return None;
        }
        let crate::compute::config::BoundarySource::DerivedRestRegions { source_toolpath_id } =
            tc.boundary.source
        else {
            return None;
        };
        match self.derived_rest_regions(index, source_toolpath_id) {
            Err(RestRegionsLookup::Refused(refusal)) => Some(refusal),
            Ok(_) | Err(RestRegionsLookup::NotFound(_)) => None,
        }
    }

    /// The one rule of the derived regions: the regions, or the typed
    /// refusal ([`RestBoundaryRefusal`]).
    fn derived_rest_regions(
        &self,
        this_index: usize,
        source_toolpath_id: crate::ids::ToolpathId,
    ) -> Result<Arc<Vec<crate::polygon::Polygon2>>, RestRegionsLookup> {
        let Some(source_index) = self
            .toolpath_configs
            .iter()
            .position(|tc| tc.id == source_toolpath_id)
        else {
            return Err(RestRegionsLookup::Refused(
                RestBoundaryRefusal::SourceMissing { source_toolpath_id },
            ));
        };
        if source_index == this_index {
            return Err(RestRegionsLookup::Refused(RestBoundaryRefusal::OwnRegions));
        }
        let Some(source_tc) = self.toolpath_configs.get(source_index) else {
            // Unreachable in practice: `source_index` came from `position()`
            // on this same Vec a few lines above.
            return Err(RestRegionsLookup::NotFound(source_index));
        };
        let source_name = source_tc.name.clone();
        let Some(result) = self.results.get(&source_index) else {
            return Err(RestRegionsLookup::Refused(
                RestBoundaryRefusal::NotGenerated {
                    source_toolpath_id,
                    source_name,
                },
            ));
        };
        match result.annotated().produced.rest_regions.as_ref() {
            Some(regions) if !regions.is_empty() => Ok(Arc::clone(regions)),
            _ => Err(RestRegionsLookup::Refused(RestBoundaryRefusal::NoRegions {
                source_toolpath_id,
                source_name,
            })),
        }
    }

    /// Resolve the region set of a `BoundarySource::Selection` boundary on
    /// toolpath `this_index`, or a `SessionError::OperationFailed` that
    /// names the boundary, the models it reads and the reason.
    ///
    /// The selection takes the one door of an op selection (geometry
    /// selection P2.5): [`crate::selection::resolve`] against the models
    /// (a rule on `SourceRef::ToolpathModel` reads the model of this
    /// toolpath), THE frame door into the setup frame, then
    /// [`crate::selection::view_boundary`] (`ItemUse::Regions`; a down face
    /// gives its outline, P3.4). A Bottom setup
    /// mirrors Y here exactly as it mirrors the toolpath's drawing. Each
    /// face area then grows one tool radius past its open sides and stops
    /// at a wall (operator ruling 2026-10-30,
    /// [`Self::grow_boundary_open_sides`]). The regions are then joined
    /// into one union, holes kept.
    ///
    /// A boundary has no findings sink. The open paths that a `Served`
    /// rule matches are left out, as a closed-area op leaves them out; the
    /// `Empty` refusal names them when nothing else is left.
    ///
    /// When `holes` is true, the set is the HOLES of that union instead:
    /// one outer polygon per hole ring, minus any other region of the union
    /// inside that hole. The function refuses when the union has no hole.
    ///
    /// Shared by the precondition in [`Self::generate_toolpath`], the
    /// pre-boundary in [`Self::resolve_generation_inputs`] and the
    /// post-generation clip, so the three agree on the set.
    pub(crate) fn resolve_boundary_selection_polys(
        &self,
        this_index: usize,
        selection: &crate::selection::Selection,
        holes: bool,
    ) -> Result<Arc<Vec<crate::polygon::Polygon2>>, SessionError> {
        let tc = self
            .toolpath_configs
            .get(this_index)
            .ok_or(SessionError::ToolpathNotFound(this_index))?;
        let setup = self.find_setup_for_toolpath_index(this_index);
        let ctx = super::SetupEvalContext::build_for_setup(self, setup);
        let refuse = |reason: String| {
            SessionError::OperationFailed(format!(
                "The machining boundary of '{}' reads {}: {reason} Change the boundary \
                 selection, or disable the boundary.",
                tc.name,
                self.describe_selection_sources(tc, selection),
            ))
        };

        // P4.1: THE resolution of a selection (`resolve_spec_frames`), the
        // one the boundary report of `selection_status` reads, with the same
        // `ItemUse` and op name. So the two give the same refusal.
        let resolved = self
            .resolve_spec_frames(
                tc,
                selection,
                &ctx,
                super::super::selection_status::FaceAccess::Build,
                (
                    super::super::selection_status::BOUNDARY_ITEM_USE,
                    super::super::selection_status::BOUNDARY_OUTLINE_USE,
                    super::super::selection_status::BOUNDARY_DOWN_FACES,
                ),
            )
            .map_err(|e| refuse(e.to_string()))?
            .setup;
        let (mut items, _open_path_notes) =
            crate::selection::view_boundary(&resolved).map_err(|refusal| {
                let missing = matches!(
                    refusal.cause,
                    crate::selection::RefusalCause::MissingSource(_)
                );
                let mut reason = refusal
                    .for_op(crate::session::selection_status::BOUNDARY_LABEL)
                    .to_string();
                if missing {
                    let known: Vec<String> = self
                        .models
                        .iter()
                        .map(|m| format!("{} ('{}')", m.id, m.name))
                        .collect();
                    reason.push_str(&format!(" Project models: [{}].", known.join(", ")));
                }
                refuse(reason)
            })?;
        // Operator ruling 2026-10-30: each face grows one tool radius past
        // its open sides and stops at a wall (THE open-sides rule, the one
        // helper that the boundary report calls too). The union below
        // then links two faces that touch into one area.
        self.grow_boundary_open_sides(tc, &ctx, &resolved, &mut items)?;
        let regions = items.regions().map_or(&[][..], |r| r.as_slice());

        let union = crate::polygon::Polygon2::union_all(regions);
        if union.is_empty() {
            return Err(refuse(format!(
                "the union of its {count} closed regions has no area. Repair the drawing.",
                count = regions.len(),
            )));
        }

        // The holes option: each hole ring of the union becomes one region.
        // A ring in `holes` is CW; `ensure_winding` makes the new exterior
        // CCW. A ring with fewer than 3 points has no area and is dropped.
        // An island in a hole is a separate polygon of the union. The code
        // subtracts every OTHER union polygon from the region, so the
        // toolpath also stays off an island. The parent polygon is not
        // subtracted: it touches the region only along the hole ring.
        if !holes {
            return Ok(Arc::new(union));
        }
        let hole_count: usize = union
            .iter()
            .flat_map(|poly| poly.holes.iter())
            .filter(|ring| ring.len() >= 3)
            .count();
        let mut hole_regions: Vec<crate::polygon::Polygon2> = Vec::new();
        for (parent_index, parent) in union.iter().enumerate() {
            for ring in parent.holes.iter().filter(|ring| ring.len() >= 3) {
                let mut region = crate::polygon::Polygon2::new(ring.clone());
                region.ensure_winding();
                let mut pieces = vec![region];
                for (other_index, other) in union.iter().enumerate() {
                    if other_index == parent_index {
                        continue;
                    }
                    pieces = pieces
                        .iter()
                        .flat_map(|piece| piece.difference(other))
                        .collect();
                }
                hole_regions.extend(pieces);
            }
        }
        if hole_count == 0 {
            return Err(refuse(format!(
                "the boundary machines the holes, but the union of its {count} closed \
                 regions has no hole. Clear \"Machine the holes\" to machine inside the \
                 shapes, or select shapes with holes.",
                count = regions.len(),
            )));
        }
        if hole_regions.is_empty() {
            return Err(refuse(format!(
                "the boundary machines the holes, but other regions fill all {hole_count} \
                 holes. Clear \"Machine the holes\".",
            )));
        }
        Ok(Arc::new(hole_regions))
    }

    /// The region set of the `BoundarySource::Selection` boundary of the
    /// toolpath at `index`, as generation clips to it
    /// ([`Self::resolve_boundary_selection_polys`]), or `None` when the
    /// boundary is off or not a selection.
    ///
    /// **Test door.** The harnesses under `crates/rs_cam_core/tests` are
    /// the only callers, so it sits behind `test-support` (F085).
    #[cfg(feature = "test-support")]
    pub fn boundary_selection_regions(
        &self,
        index: usize,
    ) -> Result<Option<Arc<Vec<crate::polygon::Polygon2>>>, SessionError> {
        let tc = self
            .toolpath_configs
            .get(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        match &tc.boundary.source {
            crate::compute::config::BoundarySource::Selection { selection, holes }
                if tc.boundary.enabled =>
            {
                self.resolve_boundary_selection_polys(index, selection, *holes)
                    .map(Some)
            }
            crate::compute::config::BoundarySource::Stock
            | crate::compute::config::BoundarySource::ModelSilhouette
            | crate::compute::config::BoundarySource::Selection { .. }
            | crate::compute::config::BoundarySource::DerivedRestRegions { .. }
            | crate::compute::config::BoundarySource::PlannedTierRegions { .. } => Ok(None),
        }
    }

    /// The sources that `selection` reads, for a refusal: each model by
    /// name and id (and its load error), the stock, or a model id that
    /// names no model. A rule on `SourceRef::ToolpathModel` names the
    /// model of `tc`.
    pub(crate) fn describe_selection_sources(
        &self,
        tc: &crate::session::ToolpathConfig,
        selection: &crate::selection::Selection,
    ) -> String {
        use crate::selection::SourceRef;
        let mut names: Vec<String> = Vec::new();
        for source in selection.sources() {
            let id = match source {
                SourceRef::Stock => {
                    if !names.iter().any(|n| n == "the stock") {
                        names.push("the stock".to_owned());
                    }
                    continue;
                }
                SourceRef::ToolpathModel => tc.model_id,
                SourceRef::Model(id) => id,
            };
            let name = match self.model_by_id(id) {
                Some(model) => match &model.load_error {
                    Some(error) => {
                        format!("model '{}' (id {id}, it did not load: {error})", model.name)
                    }
                    None => format!("model '{}' (id {id})", model.name),
                },
                None => format!("model id {id} (no model has that id)"),
            };
            if !names.contains(&name) {
                names.push(name);
            }
        }
        if names.is_empty() {
            "an empty selection".to_owned()
        } else {
            names.join(", ")
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
    /// Not used for `BoundarySource::DerivedRestRegions`,
    /// `PlannedTierRegions` or `Selection` — those sources can resolve
    /// to multiple disjoint polygons, which this single-polygon signature
    /// can't represent. They refuse here with an internal error rather
    /// than take the stock rectangle. See
    /// [`Self::resolve_derived_rest_region_polys`] +
    /// [`crate::geometry::region_set::RegionSet::processed`] for that source's path,
    /// wired in by the two call sites below (`resolve_generation_inputs`'s
    /// `pre_boundary` and `generate_toolpath`'s post-dressup clip).
    pub(crate) fn resolve_containment_polygon(
        boundary_config: &crate::compute::config::BoundaryConfig,
        stock_bbox: &BoundingBox3,
        mesh: Option<&Arc<crate::mesh::TriangleMesh>>,
        keep_out_footprints: &[crate::polygon::Polygon2],
    ) -> Result<Option<crate::polygon::Polygon2>, crate::compute::execute::OperationError> {
        use crate::compute::config::BoundarySource;
        use crate::geometry::boundary::{
            UserOffsetOutcome, apply_user_boundary_offset, subtract_keepouts,
        };

        let stock_rectangle = || {
            crate::polygon::Polygon2::rectangle(
                stock_bbox.min.x,
                stock_bbox.min.y,
                stock_bbox.max.x,
                stock_bbox.max.y,
            )
        };
        // Every source is named. A new source must choose its arm here; it
        // cannot fall to the stock rectangle through a wildcard.
        let mut stock_poly = match &boundary_config.source {
            BoundarySource::Stock => stock_rectangle(),
            BoundarySource::ModelSilhouette => match mesh {
                Some(m) => {
                    // G8: memoised per mesh identity. This ran twice per
                    // toolpath (pre-boundary resolution + the post-generation
                    // enforcement clip), each time rasterising every face of
                    // the mesh.
                    let silhouettes = crate::maps::geom_cache::cached_silhouette(m);
                    // R1: the outer loop only. The raw silhouette keeps a
                    // through-hole as a hole; the boundary treats it as
                    // material.
                    crate::geometry::boundary::silhouette_machining_outline(&silhouettes)
                        .unwrap_or_else(stock_rectangle)
                }
                None => stock_rectangle(),
            },
            // These three resolve to a SET of regions through
            // `RegionSet::processed` and `apply_boundary_clip_multi`. The
            // callers route them there first, so this arm is a wiring
            // defect. It refuses: the stock rectangle would widen the
            // boundary the operator asked for.
            BoundarySource::Selection { .. }
            | BoundarySource::DerivedRestRegions { .. }
            | BoundarySource::PlannedTierRegions { .. } => {
                return Err(crate::compute::execute::OperationError::Internal(format!(
                    "internal error: the '{}' boundary resolves to a region set, but it \
                     reached the single-polygon boundary path. Refusing rather than \
                     clipping to the stock rectangle.",
                    boundary_config.source.label(),
                )));
            }
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
    /// Takes/returns an [`AnnotatedToolpath`](crate::trace::toolpath_spans::AnnotatedToolpath). Spans are precisely remapped
    /// through the clip via the provenance map returned from
    /// [`crate::geometry::boundary::clip_toolpath_to_boundary_with_provenance`]; the
    /// clipper never drops input moves, only inserts retract/rapid pairs
    /// between them, so a Region span that originally covered "the moves
    /// doing the cut for region X" still covers them post-clip plus any
    /// retracts inserted into the middle. `spans_valid` stays `true`.
    ///
    /// `plunge_feed` is the plunge feed of the op's entry rule, used for the
    /// re-entry descent the clipper emits (G-BOUNDARYPLUNGE) — see
    /// [`crate::geometry::boundary::clip_toolpath_to_boundary_set_with_provenance`].
    /// P29-8e: it takes and gives a [`crate::geometry::boundary::Reentering`],
    /// so the re-entries reach the entry step after the clips.
    #[expect(
        clippy::too_many_arguments,
        reason = "each parameter is a separate input that the callers supply"
    )]
    pub fn apply_boundary_clip(
        annotated: crate::geometry::boundary::Reentering,
        boundary_config: &crate::compute::config::BoundaryConfig,
        stock_bbox: &BoundingBox3,
        // G8: `&Arc` rather than `&TriangleMesh` so a `ModelSilhouette`
        // boundary can reuse the per-mesh silhouette memo instead of
        // re-rasterising every face on every toolpath. The mesh is an `Arc`
        // at every production call site already; identity is the memo key.
        mesh: Option<&Arc<crate::mesh::TriangleMesh>>,
        keep_out_footprints: &[crate::polygon::Polygon2],
        tool_diameter: f64,
        safe_z: f64,
        plunge_feed: Option<crate::feeds::entry_rule::EntryPlungeFeed>,
        semantic_ctx: &crate::trace::semantic_trace::ToolpathSemanticContext,
        channels: &mut crate::trace::transform_provenance::ReconcileSet<'_>,
    ) -> Result<crate::geometry::boundary::Reentering, crate::compute::execute::OperationError>
    {
        use crate::geometry::boundary::clip_annotated_to_boundary_set;

        // The source polygon (ModelSilhouette falls back to the stock
        // rectangle when the model has no mesh), less the keep-outs, with
        // the user offset and the containment offset. Shared with the
        // adaptive3d pre-clip path in `resolve_generation_inputs` and the
        // G-BOUNDCOLLAPSE precondition (`clip_containment`).
        let set = Self::clip_containment(
            boundary_config,
            ContainmentSource::Single { stock_bbox, mesh },
            keep_out_footprints,
            tool_diameter,
        )?;
        // Checkpoint C, Q2 (F-1) and G-BOUNDCOLLAPSE. The set clipper passes
        // an EMPTY set through with an identity mapping, so the containment
        // the operator asked for would not apply. A failed offset and a
        // genuine collapse both refuse.
        if set.boundaries.is_empty() {
            return Err(Self::collapsed_containment_refusal(
                set.offset_failure,
                boundary_config.containment,
                tool_diameter,
                set.source_region_count,
            ));
        }
        // Checkpoint C, D-3c: the WHOLE set, not `boundaries.first()`. A
        // containment offset that splits its source into several polygons
        // used to keep piece 1 and clip everything outside it away — an
        // under-cut nobody chose, and the multi-region path at
        // `apply_boundary_clip_multi` already disagreed by keeping them all.
        // The multi-region semantics win: membership downstream is "inside
        // ANY", which is what a split containment means.
        let clipped =
            clip_annotated_to_boundary_set(annotated, &set.boundaries, safe_z, plunge_feed)
                .reconcile(channels);

        // Recorded AFTER the reconcile so this item's own link is bound to
        // post-clip indices and is not then remapped a second time.
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
        let moves = &clipped.annotated().toolpath;
        if !moves.moves.is_empty() {
            clip_scope.bind_to_toolpath(moves, 0, moves.moves.len());
        }

        Ok(clipped)
    }

    /// G-BOUNDCOLLAPSE: the containment set that the post-generation clip
    /// clips to, from the same steps that the clip runs.
    ///
    /// - `Single`: [`Self::resolve_containment_polygon`] (keep-outs, user
    ///   offset), then the containment offset. A user offset that collapses
    ///   the polygon gives an empty set.
    /// - `Regions`: [`crate::geometry::region_set::RegionSet::processed`]
    ///   (keep-outs, user offset; a region that collapses drops), then the
    ///   containment offset per region. The set is flat: membership
    ///   downstream is "inside ANY".
    ///
    /// The clip and the generation precondition
    /// ([`Self::boundary_collapse_refusal`]) both read this, so the two
    /// agree on what a collapse is.
    pub(crate) fn clip_containment(
        boundary_config: &crate::compute::config::BoundaryConfig,
        source: ContainmentSource<'_>,
        keep_out_footprints: &[crate::polygon::Polygon2],
        tool_diameter: f64,
    ) -> Result<ClipContainment, crate::compute::execute::OperationError> {
        use crate::geometry::boundary::{ToolContainment, effective_boundary_reported};

        let containment = match boundary_config.containment {
            crate::compute::config::BoundaryContainment::Center => ToolContainment::Center,
            crate::compute::config::BoundaryContainment::Inside => ToolContainment::Inside,
            crate::compute::config::BoundaryContainment::Outside => ToolContainment::Outside,
        };
        let tool_radius = tool_diameter / 2.0;
        match source {
            ContainmentSource::Single { stock_bbox, mesh } => {
                // Checkpoint C, D-3b: `None` means the user offset COLLAPSED
                // the containment. The old `.unwrap_or_else(|| stock
                // rectangle)` would have resurrected the very un-offset
                // boundary the collapse says is wrong.
                let Some(stock_poly) = Self::resolve_containment_polygon(
                    boundary_config,
                    stock_bbox,
                    mesh,
                    keep_out_footprints,
                )?
                else {
                    return Ok(ClipContainment {
                        boundaries: Vec::new(),
                        offset_failure: None,
                        source_region_count: 1,
                    });
                };
                let (boundaries, offset_failure) =
                    effective_boundary_reported(&stock_poly, containment, tool_radius);
                Ok(ClipContainment {
                    boundaries,
                    offset_failure,
                    source_region_count: 1,
                })
            }
            ContainmentSource::Regions(regions) => {
                let processed = crate::geometry::region_set::RegionSet::from_slice(regions)
                    .processed(keep_out_footprints, boundary_config.offset);
                let mut boundaries: Vec<crate::polygon::Polygon2> = Vec::new();
                // Checkpoint C, Q2: the failure channel is aggregated across
                // regions the same way `offset_polygon_reported` aggregates
                // across repaired pieces — a library failure outranks a
                // rejected input — so one bad region cannot be hidden by a
                // dozen clean ones.
                let mut offset_failure: Option<crate::polygon::OffsetFailure> = None;
                for region in processed.as_slice() {
                    let (out, failure) =
                        effective_boundary_reported(region, containment, tool_radius);
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
                Ok(ClipContainment {
                    boundaries,
                    offset_failure,
                    source_region_count: regions.len(),
                })
            }
        }
    }

    /// The refusal for an EMPTY containment set, in one place because both
    /// boundary clip paths must make it identically.
    ///
    /// An empty set is either the residue of an offset that **failed**
    /// (Checkpoint C, Q2) or a **genuine collapse**: the tool, the user
    /// offset or the keep-outs leave no area. Both refuse. Before
    /// G-BOUNDCOLLAPSE (operator ruling 2026-10-28) a genuine collapse
    /// passed the toolpath through UNCLIPPED with a report-only finding,
    /// so cuts outside a declared boundary (a frame or a clamp zone) could
    /// reach the program.
    ///
    /// The session refuses a genuine collapse before generation, with the
    /// op named ([`Self::boundary_collapse_refusal`]). This refusal is the
    /// backstop of the clip itself.
    pub fn collapsed_containment_refusal(
        offset_failure: Option<crate::polygon::OffsetFailure>,
        containment: crate::compute::config::BoundaryContainment,
        tool_diameter: f64,
        source_region_count: usize,
    ) -> crate::compute::execute::OperationError {
        if let Some(failure) = offset_failure {
            return crate::compute::execute::OperationError::MissingGeometry(format!(
                "boundary containment `{containment:?}` could not be \
                 computed: {reason}. Refusing to emit this toolpath: an \
                 empty containment would leave it UNCLIPPED. Repair the \
                 boundary geometry (self-intersecting or pinched rings, \
                 repeated vertices, non-finite coordinates) or set the \
                 containment to `Center`.",
                reason = failure.describe(),
            ));
        }
        crate::compute::execute::OperationError::MissingGeometry(format!(
            "the machining boundary collapsed: with `{containment:?}` \
             containment and a {tool_diameter:.3} mm tool, no area of its \
             {source_region_count} region(s) is left. Refusing to emit this \
             toolpath: a collapsed boundary would leave it UNCLIPPED. Use a \
             smaller tool, less boundary offset, or another containment.",
        ))
    }

    /// G-BOUNDCOLLAPSE (operator ruling 2026-10-28): the machining boundary
    /// of the toolpath at `index` collapses, or `None`.
    ///
    /// The read door of the generation refusal. The GUI can show the
    /// collapse before a generate; generation refuses on the same answer
    /// ([`Self::capture_generate_handle`]). It resolves the generation
    /// inputs, so a toolpath that refuses for another cause gives that
    /// `Err`. A FAILED offset gives `None`: the clip refuses with the
    /// failure.
    pub fn boundary_collapse(
        &self,
        index: usize,
    ) -> Result<Option<BoundaryCollapse>, SessionError> {
        let tc = self
            .toolpath_configs
            .get(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        if !tc.boundary.enabled
            || !tc
                .operation
                .capabilities()
                .territory_xy
                .session_clip_applies()
        {
            return Ok(None);
        }
        let cancel = AtomicBool::new(false);
        let inputs = self.resolve_generation_inputs(index, &cancel)?;
        let regions = self.post_clip_regions(&tc.name, index, &inputs, &cancel)?;
        Self::boundary_collapse_of(&tc.name, &inputs, regions.as_deref().map(Vec::as_slice))
    }

    /// G-C: [`Self::boundary_collapse`] for the toolpath at `index` with
    /// `boundary` in place of its stored boundary: "would this boundary
    /// collapse?". The GUI asks it before it writes `Inside` (the
    /// `[Use Inside]` act and the D-2 pick), so it never switches to a
    /// boundary that refuses. The inputs resolve from the stored config;
    /// only the boundary of the collapse test is the candidate.
    pub fn boundary_collapse_with(
        &self,
        index: usize,
        boundary: &crate::compute::config::BoundaryConfig,
    ) -> Result<Option<BoundaryCollapse>, SessionError> {
        let tc = self
            .toolpath_configs
            .get(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        if !boundary.enabled
            || !tc
                .operation
                .capabilities()
                .territory_xy
                .session_clip_applies()
        {
            return Ok(None);
        }
        let cancel = AtomicBool::new(false);
        let mut inputs = self.resolve_generation_inputs(index, &cancel)?;
        inputs.boundary_config = boundary.clone();
        let regions = self.post_clip_regions(&tc.name, index, &inputs, &cancel)?;
        Self::boundary_collapse_of(&tc.name, &inputs, regions.as_deref().map(Vec::as_slice))
    }

    /// The collapse test of [`Self::boundary_collapse`] on resolved inputs.
    ///
    /// It reads the containment set of the post-generation clip
    /// ([`Self::clip_containment`]) from the same inputs as the clip, so it
    /// finds a collapse exactly when the clip would get an empty set.
    pub(crate) fn boundary_collapse_of(
        op_name: &str,
        inputs: &ResolvedGenInputs,
        post_clip_regions: Option<&[crate::polygon::Polygon2]>,
    ) -> Result<Option<BoundaryCollapse>, SessionError> {
        use crate::tool::MillingCutter as _;
        let boundary_config = &inputs.boundary_config;
        if !boundary_config.enabled {
            return Ok(None);
        }
        let source = match post_clip_regions {
            Some(regions) => ContainmentSource::Regions(regions),
            None => ContainmentSource::Single {
                stock_bbox: &inputs.emission_stock_bbox,
                mesh: inputs.mesh.as_ref(),
            },
        };
        let tool_diameter = inputs.tool_def.diameter();
        let set = Self::clip_containment(
            boundary_config,
            source,
            &inputs.keep_out_footprints,
            tool_diameter,
        )?;
        if !set.boundaries.is_empty() || set.offset_failure.is_some() {
            return Ok(None);
        }
        Ok(Some(BoundaryCollapse {
            op_name: op_name.to_owned(),
            source_label: boundary_config.source.label(),
            containment: boundary_config.containment,
            tool_diameter_mm: tool_diameter,
            offset_mm: boundary_config.offset,
        }))
    }

    /// The region set of the POST-generation clip, for the three sources
    /// that resolve to a set of disjoint regions; `None` for every other
    /// source, or with the boundary off.
    ///
    /// Re-resolved, never carried over from the pre-clip: the tier map
    /// is memoised on (mesh identity, ladder, params), so this is a
    /// cache hit on the exact map that produced the pre-decompose
    /// boundary. Agreement between the two clips is therefore
    /// structural — the P2.3 rule that made `DerivedRestRegions` safe.
    /// It is also the RAW set. `inputs.pre_boundary_regions` is the
    /// PROCESSED set, and the clip processes the raw set itself.
    pub(crate) fn post_clip_regions(
        &self,
        op_name: &str,
        index: usize,
        inputs: &ResolvedGenInputs,
        cancel: &AtomicBool,
    ) -> Result<Option<Arc<Vec<crate::polygon::Polygon2>>>, SessionError> {
        if !inputs.boundary_config.enabled {
            return Ok(None);
        }
        match &inputs.boundary_config.source {
            crate::compute::config::BoundarySource::PlannedTierRegions {
                tool_ids,
                tier,
                cell_mm,
                tolerance_mm,
                margin_mm,
                treatment,
                islands,
            } => {
                // The tier map reads a BUILT index; force the lazy one
                // here, as the resolver's own tier arm does.
                let tier_index = inputs
                    .spatial_index
                    .as_ref()
                    .map(|lazy| Arc::clone(lazy.force()));
                let regions = self.resolve_planned_tier_region_polys(
                    op_name,
                    inputs.mesh.as_ref(),
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
                Ok(Some(Arc::new(regions)))
            }
            crate::compute::config::BoundarySource::DerivedRestRegions { source_toolpath_id } => {
                // The precondition already validated this — it can only
                // fail here if the source toolpath's result was
                // invalidated in between, which cannot happen under
                // `&self`. Propagate defensively.
                Ok(Some(self.resolve_derived_rest_region_polys(
                    index,
                    *source_toolpath_id,
                )?))
            }
            crate::compute::config::BoundarySource::Selection { selection, holes } => {
                // The same resolution as the pre-boundary, so the two
                // clips bound the same set.
                Ok(Some(self.resolve_boundary_selection_polys(
                    index, selection, *holes,
                )?))
            }
            crate::compute::config::BoundarySource::Stock
            | crate::compute::config::BoundarySource::ModelSilhouette => Ok(None),
        }
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
    /// collapsed, as in the single-polygon path: the clip refuses
    /// (G-BOUNDCOLLAPSE, [`Self::collapsed_containment_refusal`]).
    ///
    /// Span remapping contract is identical to [`Self::apply_boundary_clip`]
    /// — the set clipper never drops input moves, so `spans_valid` stays
    /// `true`.
    ///
    /// `plunge_feed` is the plunge feed of the op's entry rule, used for the
    /// re-entry descent the clipper emits (G-BOUNDARYPLUNGE) — see
    /// [`crate::geometry::boundary::clip_toolpath_to_boundary_set_with_provenance`].
    /// P29-8e: it takes and gives a [`crate::geometry::boundary::Reentering`],
    /// so the re-entries reach the entry step after the clips.
    #[expect(
        clippy::too_many_arguments,
        reason = "each parameter is a separate input that the callers supply"
    )]
    pub fn apply_boundary_clip_multi(
        annotated: crate::geometry::boundary::Reentering,
        boundary_config: &crate::compute::config::BoundaryConfig,
        regions: &[crate::polygon::Polygon2],
        keep_out_footprints: &[crate::polygon::Polygon2],
        tool_diameter: f64,
        safe_z: f64,
        plunge_feed: Option<crate::feeds::entry_rule::EntryPlungeFeed>,
        semantic_ctx: &crate::trace::semantic_trace::ToolpathSemanticContext,
        channels: &mut crate::trace::transform_provenance::ReconcileSet<'_>,
    ) -> Result<crate::geometry::boundary::Reentering, crate::compute::execute::OperationError>
    {
        use crate::geometry::boundary::clip_annotated_to_boundary_set;

        // Per-region keep-out subtraction, user offset and containment
        // offset (`clip_containment`). A region that collapses drops.
        let set = Self::clip_containment(
            boundary_config,
            ContainmentSource::Regions(regions),
            keep_out_footprints,
            tool_diameter,
        )?;
        if set.boundaries.is_empty() {
            // Every region collapsed (offset/inset ate them all), or an
            // offset failed: the same refusal as the single-polygon path.
            return Err(Self::collapsed_containment_refusal(
                set.offset_failure,
                boundary_config.containment,
                tool_diameter,
                set.source_region_count,
            ));
        }
        let boundaries = set.boundaries;

        let clipped = clip_annotated_to_boundary_set(annotated, &boundaries, safe_z, plunge_feed)
            .reconcile(channels);

        // Recorded AFTER the reconcile so this item's own link is bound to
        // post-clip indices and is not then remapped a second time.
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
        let moves = &clipped.annotated().toolpath;
        if !moves.moves.is_empty() {
            clip_scope.bind_to_toolpath(moves, 0, moves.moves.len());
        }

        Ok(clipped)
    }
}

/// G-BOUNDCOLLAPSE: where [`ProjectSession::clip_containment`] reads its
/// source polygons.
#[derive(Clone, Copy)]
pub(crate) enum ContainmentSource<'a> {
    /// One containment polygon: the stock rectangle or the model
    /// silhouette ([`ProjectSession::resolve_containment_polygon`]).
    Single {
        stock_bbox: &'a BoundingBox3,
        mesh: Option<&'a Arc<crate::mesh::TriangleMesh>>,
    },
    /// The RAW region set of a `PlannedTierRegions`, `DerivedRestRegions`
    /// or `Selection` boundary.
    Regions(&'a [crate::polygon::Polygon2]),
}

/// G-BOUNDCOLLAPSE: the containment set the post-generation clip clips to.
pub(crate) struct ClipContainment {
    /// The polygons after every offset. Empty: the boundary collapsed, or
    /// an offset failed.
    pub(crate) boundaries: Vec<crate::polygon::Polygon2>,
    /// The offset failure, when an offset failed.
    pub(crate) offset_failure: Option<crate::polygon::OffsetFailure>,
    /// The number of source regions (1 for `Single`).
    pub(crate) source_region_count: usize,
}

/// G-C (§2 a): a `DerivedRestRegions` consumer whose tool is larger than
/// the tool of its source ([`ProjectSession::larger_rest_consumer`]). The
/// source's regions are tool-centre positions of its smaller tool, so with
/// `Center` or `Outside` containment this tool can cut past the source's
/// machining boundary by the difference of the radii (D53).
#[derive(Debug, Clone, PartialEq)]
pub struct LargerRestConsumer {
    /// The name of the consumer.
    pub op_name: String,
    pub source_toolpath_id: crate::ids::ToolpathId,
    pub source_name: String,
    /// The radius of the consumer's built cutter, mm.
    pub tool_radius_mm: f64,
    /// The radius of the source's built cutter, mm.
    pub source_tool_radius_mm: f64,
}

impl LargerRestConsumer {
    /// How far past the source's boundary the tool can cut, mm.
    #[must_use]
    pub fn overrun_mm(&self) -> f64 {
        self.tool_radius_mm - self.source_tool_radius_mm
    }

    /// The D53 refusal sentence for `containment` (the word the panel and
    /// the project file use: `Center` or `Outside`).
    #[must_use]
    pub fn refusal(&self, containment: &str) -> String {
        let (own, theirs) = (self.tool_radius_mm, self.source_tool_radius_mm);
        // C1 ruling (2026-10-28): the tool fix comes first. `Inside` can
        // collapse narrow regions, and a collapse refuses (G-BOUNDCOLLAPSE).
        format!(
            "'{}' takes its boundary from the rest regions of '{}' with {containment} \
             containment, but its tool radius {own:.3} mm is larger than the tool radius of \
             '{}', {theirs:.3} mm. The regions are tool-centre positions of that tool, so this \
             tool could cut up to {:.3} mm past the machining boundary of '{}'. Use a tool no \
             larger than the tool of '{}' (Ø{:.3}), or Inside containment if the regions are \
             wider than this tool (Ø{:.3}).",
            self.op_name,
            self.source_name,
            self.source_name,
            self.overrun_mm(),
            self.source_name,
            self.source_name,
            2.0 * theirs,
            2.0 * own,
        )
    }
}

/// G-E (3): why a `DerivedRestRegions` boundary refuses generation. Its
/// `Display` is the refusal text generation gives (unchanged by G-E), so
/// the wire and MCP text do not move; a surface reads the variant to draw
/// a short face and its action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestBoundaryRefusal {
    /// No toolpath has the source id any more.
    SourceMissing {
        source_toolpath_id: crate::ids::ToolpathId,
    },
    /// The boundary names the toolpath's own regions.
    OwnRegions,
    /// The source has no generated result.
    NotGenerated {
        source_toolpath_id: crate::ids::ToolpathId,
        source_name: String,
    },
    /// The source's result has no rest regions.
    NoRegions {
        source_toolpath_id: crate::ids::ToolpathId,
        source_name: String,
    },
}

impl std::fmt::Display for RestBoundaryRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SourceMissing { source_toolpath_id } => write!(
                f,
                "Boundary references toolpath id {source_toolpath_id} for its rest regions, \
                 but no toolpath with that id exists anymore. Pick a different source toolpath \
                 for the boundary, or disable the boundary.",
            ),
            Self::OwnRegions => f.write_str(
                "Boundary references this toolpath's own rest regions — a toolpath cannot use \
                 itself as the source for a derived-rest-regions boundary. Pick a different \
                 source toolpath.",
            ),
            Self::NotGenerated { source_name, .. } => write!(
                f,
                "'{source_name}' has no generated result yet — generate '{source_name}' first; \
                 its rest analysis produces the regions this boundary needs.",
            ),
            Self::NoRegions { source_name, .. } => write!(
                f,
                "'{source_name}' produced no rest regions. Its rest analysis found no material \
                 above the threshold, or it is switched off. Check the rest analysis settings \
                 on '{source_name}' and regenerate it.",
            ),
        }
    }
}

/// The failure of [`ProjectSession::derived_rest_regions`]: a typed
/// refusal, or an index that does not resolve.
enum RestRegionsLookup {
    Refused(RestBoundaryRefusal),
    NotFound(usize),
}
/// G-BOUNDCOLLAPSE (operator ruling 2026-10-28): a machining boundary that
/// leaves no area for the tool. [`ProjectSession::boundary_collapse`] reads
/// it; generation refuses with [`Self::message`].
#[derive(Debug, Clone, PartialEq)]
pub struct BoundaryCollapse {
    /// The name of the toolpath.
    pub op_name: String,
    /// The label of the boundary source ([`crate::compute::config::BoundarySource::label`]).
    pub source_label: &'static str,
    /// The containment of the boundary.
    pub containment: crate::compute::config::BoundaryContainment,
    /// The diameter of the built cutter, the diameter the containment uses.
    pub tool_diameter_mm: f64,
    /// The user offset of the boundary.
    pub offset_mm: f64,
}

impl BoundaryCollapse {
    /// The refusal sentence: the op, the boundary and the fix.
    pub fn message(&self) -> String {
        format!(
            "'{op}' cannot keep to its {source} machining boundary: with \
             {containment:?} containment, a {tool:.3} mm tool and a \
             {offset:+.3} mm boundary offset, no area of the boundary is left. \
             Refusing generation: with a collapsed boundary the toolpath would \
             have no boundary at all, and it could cut outside it (a frame or \
             a clamp zone). Use a smaller tool, less boundary offset, or \
             another containment.",
            op = self.op_name,
            source = self.source_label,
            containment = self.containment,
            tool = self.tool_diameter_mm,
            offset = self.offset_mm,
        )
    }
}
