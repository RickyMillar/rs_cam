//! Compute and mutation methods on [`ProjectSession`].

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tracing::instrument;

use crate::compute::cutter::build_cutter;
use crate::compute::operation_configs::ClearingStrategy;
use crate::compute::simulate::{
    SimGroupEntry, SimToolpathEntry, SimulationRequest, run_simulation,
};
use crate::compute::tool_config::ToolConfig;
use crate::geo::BoundingBox3;
use crate::ids::ToolpathId;
use crate::mesh::TriangleMesh;
use crate::stock::simulation_cut::{SimulationCutTrace, SimulationMetricOptions};
use crate::tool::MillingCutter;
use crate::trace::debug_trace::ToolpathDebugRecorder;
use crate::trace::semantic_trace::{ToolpathSemanticKind, ToolpathSemanticRecorder, enrich_traces};

// `SetupEvalContext`, `SimulationResult`, `ToolpathConfig`, `multitool` and
// `mutation` are imported for the children: a moved body still writes
// `super::ToolpathConfig`, and from a child `super` is this module.
use super::{
    AdoptResultArgs, Command, GenerateToolpathArgs, Job, JobHandle, ProjectSession, SessionError,
    SetupEvalContext, SimulationOptions, SimulationResult, ToolpathComputeResult, ToolpathConfig,
    multitool, mutation,
};

mod diagnostics;
mod export;
mod generation;
mod params;
mod simulation;

pub(crate) use params::strip_outer_quotes;

/// Fully-owned, per-generation inputs resolved from session state by
/// [`ProjectSession::resolve_generation_inputs`]. Owning everything (mesh /
/// polygons via `Arc`, an owned [`SpatialIndex`](crate::mesh::SpatialIndex))
/// lets a caller generate one *or many* toolpaths off a single resolution
/// without re-deriving any frame-sensitive value: a
/// [`GenerateToolpathHandle`] owns one; the strategy advisor reuses it
/// across candidate strategies (only the `operation`'s clearing strategy
/// varies per candidate).
///
/// WP11a publishes the type so another crate names it. The fields stay
/// private and the type derives no `Default`, so a caller outside this module
/// builds no value of it. One public function produces it:
/// [`ProjectSession::resolve_generation_inputs`]. A second assembly of the
/// same inputs is therefore unnameable, which is the point of the publication
/// (`planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md` §1).
/// The sentry is `tests/resolved_gen_inputs_has_one_producer.rs`.
pub struct ResolvedGenInputs {
    tool: ToolConfig,
    mesh: Option<Arc<TriangleMesh>>,
    polygons: Option<Arc<Vec<crate::polygon::Polygon2>>>,
    /// G-DRILLCENTROID: the target model's drill targets (DXF points and
    /// circle/arc centres), the `Drill` family's hole source when nothing
    /// is picked. Shared with the model, never transformed here — like
    /// `selected_holes`, the generator maps them into the emission frame.
    drill_targets: Arc<Vec<crate::io::dxf_input::DrillTarget>>,
    keep_out_footprints: Vec<crate::polygon::Polygon2>,
    boundary_config: crate::compute::config::BoundaryConfig,
    emission_stock_bbox: BoundingBox3,
    heights: crate::compute::config::ResolvedHeights,
    tool_def: crate::tool::ToolDefinition,
    /// G8: shared with the per-mesh memo in [`crate::maps::geom_cache`] rather than
    /// owned, so repeated toolpath resolution over one model reuses one grid.
    ///
    /// WP11b: a [`LazyIndex`](crate::maps::geom_cache::LazyIndex), not a built
    /// index. [`ProjectSession::start`] runs on the GUI frame loop and the
    /// grid build over a 661 k-triangle terrain is the most expensive step
    /// generation makes, so the build waits for
    /// [`force`](crate::maps::geom_cache::LazyIndex::force) on the worker thread.
    /// Read it through [`Self::spatial_index`], never by hand.
    spatial_index: Option<Arc<crate::maps::geom_cache::LazyIndex>>,
    cutting_levels: Vec<f64>,
    prev_tool_radius: Option<f64>,
    /// R1 (pencil): the resolved real reference tool config when the Pencil op's
    /// `reference_tool_id` names a library tool. Resolved here (the context has
    /// no tool list) exactly like `prev_tool_radius`.
    reference_tool_cfg: Option<ToolConfig>,
    operation: crate::compute::OperationConfig,
    pre_boundary: Option<crate::polygon::Polygon2>,
    /// P2.3: the per-region processed polygon set for a `DerivedRestRegions`
    /// boundary — the same set [`ProjectSession::apply_boundary_clip_multi`]
    /// re-derives for its post-generation clip (keep-outs subtracted, user
    /// offset applied, but NOT yet tool-radius inset). Resolved once here
    /// alongside `pre_boundary`'s union attempt and shared with the
    /// mesh-finish family's pre-clip via `ExecutionContext::boundary_regions`
    /// — never re-derived just for this field. `None` for every other
    /// boundary source (or when the boundary is disabled).
    pre_boundary_regions: Option<Vec<crate::polygon::Polygon2>>,
    /// G-DRILLPICK-FRAME: the setup's local<->global transform, hoisted off
    /// the `SetupEvalContext` alongside `emission_stock_bbox` above because
    /// `generate_toolpath` needs it and the context is scoped to this
    /// resolver. `None` == identity setup == no-op. Drill picks are the one
    /// input class the geometry pipeline cannot pre-transform: meshes and
    /// polygons are transformed on the way in, but `selected_holes` arrive
    /// as raw model coordinates on the operation config.
    setup_transform: Option<crate::compute::transform::SetupTransformInfo>,
    /// N12 items 1 and 2: the XY outline of the picked BREP faces, in the
    /// emission frame.
    ///
    /// A STEP model carries a mesh and an enriched mesh, never polygons, so
    /// a picked-face 2D operation gets its geometry from here. The same
    /// outline answers a [`BoundarySource::FaceSelection`] boundary
    /// (`resolve_containment_polygon`).
    ///
    /// `None` means the toolpath picks no face, or the model exposes no
    /// enriched mesh, or the picked faces are not horizontal planes — never
    /// "the outline was empty".
    ///
    /// [`BoundarySource::FaceSelection`]: crate::compute::config::BoundarySource::FaceSelection
    face_boundary: Option<crate::polygon::Polygon2>,
}

impl ResolvedGenInputs {
    /// The spatial index over this generation's mesh, built on first read.
    ///
    /// The bundle stores a [`LazyIndex`](crate::maps::geom_cache::LazyIndex), so
    /// this is where the grid build happens. Call it off the frame loop.
    /// `None` means the operation has no mesh.
    fn spatial_index(&self) -> Option<&crate::mesh::SpatialIndex> {
        self.spatial_index
            .as_ref()
            .map(|lazy| lazy.force().as_ref())
    }
}

/// The session reads the generation TAIL makes, captured at submit time.
///
/// Step (i) of a `Job` holds `&mut ProjectSession`. Step (ii) holds no
/// session at all. Every read the tail of a generation used to make off
/// `self` therefore moves here, and [`execute_job`] reads it from the
/// handle.
///
/// The fields stay private and the type derives no `Default`, so a
/// caller outside this module builds no value of it — the rule
/// [`ResolvedGenInputs`] carries, for the same reason.
///
/// **[`ResolvedGenInputs`] does not grow to hold these.** That bundle is
/// the executor's input set, and WP11b narrows the executor onto it. A
/// recorder label, a dressup configuration and a post-clip region set
/// are not executor inputs.
///
/// **Five fields read the RAW operation, not the patched one.**
/// `op_label`, `entry_probe_leave`, `feed_rate`, `plunge_rate` and
/// `transform_capabilities` come from `ToolpathConfig::operation`, while
/// the executor, the drill-op builder and the empty-generation gate read
/// [`ResolvedGenInputs`]'s compute-time-patched copy. The tail made that
/// split before WP10 split the function; reading these off the patched
/// copy would change the answer.
pub struct GenContext {
    /// `ToolpathConfig::name`. The recorder labels and the
    /// empty-generation refusal name the toolpath the operator sees.
    toolpath_name: String,
    /// The RAW operation's label.
    op_label: &'static str,
    /// `ToolpathConfig::stock_source`. It gates the machined-stock seed.
    stock_source: crate::session::StockSource,
    /// `ToolpathConfig::dressups`.
    dressups: crate::compute::config::DressupConfig,
    /// `ToolpathConfig::rest_analysis`.
    rest_analysis: crate::compute::config::RestAnalysisConfig,
    /// `ToolpathConfig::debug_options`.
    ///
    /// WP11b: the generation door used to reach past the command surface
    /// and write this flag through `toolpath_configs_mut()` (§16 ruling 7).
    /// It is a `Command` row now, `SetToolpathDebugOptions`, so the flag
    /// arrives here the way every other dial does — off the config the
    /// submit step reads.
    ///
    /// It gates the per-dressup ITEM contexts, which are what make a
    /// recorded trace expensive. The recorders themselves stay
    /// unconditional, because the CLI reads both traces off every result.
    debug_options: crate::trace::debug_trace::ToolpathDebugOptions,
    /// The RAW operation's entry-probe stock-to-leave. `None` names a
    /// prism operation, which gets no surface probe (G-RAMPTERRAIN).
    entry_probe_leave: Option<f64>,
    /// The RAW operation's cutting feed, in mm/min.
    feed_rate: f64,
    /// The RAW operation's plunge rate, in mm/min.
    plunge_rate: f64,
    /// The RAW operation's transform capabilities.
    transform_capabilities: crate::compute::catalog::OperationTransformCapabilities,
    /// The simulated machined-stock snapshot this toolpath's id names,
    /// read from `self.simulation.prior_stocks`.
    ///
    /// S-4 (G-BYTE): this is the snapshot the generation CONSUMES in the
    /// broadest true sense. `stock_source` gates the generator seed and
    /// the entry-descent split, but the dressup air-cut filter reads the
    /// same snapshot ungated. `None` means no machined stock at all.
    prior_stock: Option<Arc<crate::dexel_stock::TriDexelStock>>,
    /// The machine envelope the pencil family's emit-time link decision
    /// costs candidates against, built from `self.machine`.
    link_kinematics: Option<crate::machine::kinematics::LinkKinematics>,
    /// `self.stock.material`. The drill gates read the workpiece's own
    /// hardness (F-016).
    material: crate::material::Material,
    /// The RAW region set the POST-generation boundary clip walks, for
    /// the two sources that resolve to a set of disjoint regions.
    ///
    /// `None` means the boundary is disabled or names another source,
    /// and the single-polygon clip answers instead. It never means "the
    /// set was empty": an empty resolution is `Some` of an empty list.
    ///
    /// This is NOT [`ResolvedGenInputs`]'s `pre_boundary_regions`, which
    /// is the PROCESSED set — keep-outs subtracted and the user offset
    /// applied. The clip processes the raw set itself.
    post_clip_regions: Option<Arc<Vec<crate::polygon::Polygon2>>>,
}

impl GenContext {
    /// The machined stock the GENERATOR is seeded with.
    ///
    /// `stock_source` gates it: a `Fresh` operation starts from the board
    /// whatever snapshot the simulation left. That is a narrower question
    /// than [`Self::prior_stock`], which the dressup air-cut filter reads
    /// ungated.
    fn generator_seed_stock(&self) -> Option<&crate::dexel_stock::TriDexelStock> {
        match self.stock_source {
            crate::session::StockSource::FromRemainingStock => self.prior_stock.as_deref(),
            crate::session::StockSource::Fresh => None,
        }
    }
}

/// What [`ProjectSession::start`] captured for one `generate_toolpath`
/// job.
///
/// The handle is the whole input of step (ii). It owns its inputs, so
/// [`execute_job`] reads no session and runs off the frame loop.
///
/// `index` and `revision` are public because step (iii) needs both and
/// they carry no invariant. `inputs` and `context` stay private, so no
/// caller outside this module builds a handle: a handle is the evidence
/// of one submit.
pub struct GenerateToolpathHandle {
    /// The index of the toolpath this job generates.
    pub index: usize,
    /// The generation-input revision step (i) read, AFTER it dropped the
    /// cached result.
    ///
    /// Step (iii) hands it back through
    /// [`AdoptResultArgs`](super::AdoptResultArgs). An edit between the
    /// two steps moves the toolpath's revision, and the adopt then
    /// refuses with
    /// [`SessionError::StaleCompletion`](super::SessionError::StaleCompletion).
    pub revision: u64,
    /// Every per-generation input, resolved once by
    /// [`ProjectSession::resolve_generation_inputs`].
    inputs: ResolvedGenInputs,
    /// Every session read the generation tail makes.
    context: GenContext,
}

impl GenerateToolpathHandle {
    /// The toolpath's debug options, as the submit step read them.
    ///
    /// A caller that writes a trace artifact of its own reads this to know
    /// whether the operator asked for one.
    #[must_use]
    pub fn debug_options(&self) -> crate::trace::debug_trace::ToolpathDebugOptions {
        self.context.debug_options
    }

    /// The operation's registry label.
    #[must_use]
    pub fn op_label(&self) -> &'static str {
        self.context.op_label
    }

    /// The toolpath's name, as the submit step read it.
    #[must_use]
    pub fn toolpath_name(&self) -> &str {
        &self.context.toolpath_name
    }

    /// A one-line summary of the cutter this job runs.
    #[must_use]
    pub fn tool_summary(&self) -> String {
        self.inputs.tool.summary()
    }

    /// The inputs of this job, as JSON, for a debug trace artifact.
    ///
    /// The bundle's fields are private, so a caller outside this module
    /// cannot assemble this itself — which is the point. It is a READ, not
    /// a producer: nothing here builds a [`ResolvedGenInputs`].
    ///
    /// The field set follows the GUI worker's own artifact snapshot, so an
    /// artifact written before WP11b and one written after name the same
    /// things.
    #[must_use]
    pub fn request_snapshot(&self) -> serde_json::Value {
        let bbox = &self.inputs.emission_stock_bbox;
        let heights = &self.inputs.heights;
        serde_json::json!({
            "toolpath_name": self.context.toolpath_name,
            "operation": &self.inputs.operation,
            "operation_label": self.context.op_label,
            "dressups": &self.context.dressups,
            "stock_source": &self.context.stock_source,
            "tool": &self.inputs.tool,
            "heights": {
                "clearance_z": heights.clearance_z,
                "retract_z": heights.retract_z,
                "feed_z": heights.feed_z,
                "top_z": heights.top_z,
                "bottom_z": heights.bottom_z,
            },
            "stock_bbox": {
                "min": { "x": bbox.min.x, "y": bbox.min.y, "z": bbox.min.z },
                "max": { "x": bbox.max.x, "y": bbox.max.y, "z": bbox.max.z },
            },
            "boundary_enabled": self.inputs.boundary_config.enabled,
            "boundary_containment": format!("{:?}", self.inputs.boundary_config.containment),
            "keep_out_count": self.inputs.keep_out_footprints.len(),
            "debug_options": &self.context.debug_options,
        })
    }
}

/// What one generation reports about itself while it runs.
///
/// [`execute_job`] holds no session, so every surface a generation used to
/// write to from inside the GUI worker arrives here instead. Each field is
/// optional and each default is "report nothing", so the CLI and the core
/// session pass [`GenObserver::none()`] and pay for nothing.
///
/// WP11b, `IMPLEMENTATION_PLAN.md` §22 ruling 1. Two deviations from that
/// ruling's list, both deliberate:
///
/// * the **debug-options gate** is read off the handle
///   ([`GenerateToolpathHandle::debug_options`]) rather than set here,
///   because the flag belongs to the toolpath's config and WP4 gave it a
///   `Command` row. A caller that wants the GUI's behaviour copies it in
///   with [`Self::with_debug_options`].
/// * the **artifact path** stays with the caller. Writing the file inside
///   core would need a slot on
///   [`ToolpathComputeResult`](super::ToolpathComputeResult) to hand the
///   path back, and that type is constructed in about twenty places.
///   [`GenerateToolpathHandle::request_snapshot`] gives the caller what it
///   needs to write the same artifact itself.
pub struct GenObserver<'a> {
    /// Whether the per-dressup ITEM contexts are recorded.
    debug_options: crate::trace::debug_trace::ToolpathDebugOptions,
    /// Where the generation publishes the stage it is in. The GUI lane's
    /// `generation_status` reads the string this writes.
    phase_sink: Option<Arc<dyn crate::trace::debug_trace::ToolpathPhaseSink>>,
    /// The debug span the generator records into. Filled by [`execute_job`]
    /// for the nested call; `None` on a bare observer.
    debug_ctx: Option<&'a crate::trace::debug_trace::ToolpathDebugContext>,
    /// The semantic item the generator records into. Filled by
    /// [`execute_job`] for the nested call; `None` on a bare observer.
    semantic_ctx: Option<&'a crate::trace::semantic_trace::ToolpathSemanticContext>,
}

impl GenObserver<'static> {
    /// Report nothing. The core session and the CLI pass this.
    #[must_use]
    pub fn none() -> Self {
        Self {
            debug_options: crate::trace::debug_trace::ToolpathDebugOptions::default(),
            phase_sink: None,
            debug_ctx: None,
            semantic_ctx: None,
        }
    }
}

impl<'a> GenObserver<'a> {
    /// Record the per-dressup items when `options.enabled`.
    #[must_use]
    pub fn with_debug_options(
        mut self,
        options: crate::trace::debug_trace::ToolpathDebugOptions,
    ) -> Self {
        self.debug_options = options;
        self
    }

    /// Publish the stage of the generation to `sink`.
    #[must_use]
    pub fn with_phase_sink(
        mut self,
        sink: Arc<dyn crate::trace::debug_trace::ToolpathPhaseSink>,
    ) -> Self {
        self.phase_sink = Some(sink);
        self
    }

    /// The same observer, bound to one generation's trace contexts.
    ///
    /// [`execute_job`] owns the recorders, so it is the only caller.
    fn bound<'b>(
        &self,
        debug: &'b crate::trace::debug_trace::ToolpathDebugContext,
        semantic: &'b crate::trace::semantic_trace::ToolpathSemanticContext,
    ) -> GenObserver<'b> {
        GenObserver {
            debug_options: self.debug_options,
            phase_sink: self.phase_sink.as_ref().map(Arc::clone),
            debug_ctx: Some(debug),
            semantic_ctx: Some(semantic),
        }
    }

    /// Name the stage the generation is in.
    fn set_phase(&self, phase: &str) {
        if let Some(sink) = self.phase_sink.as_ref() {
            sink.set_phase(Some(phase.to_owned()));
        }
    }

    /// Report that no stage is running.
    fn clear_phase(&self) {
        if let Some(sink) = self.phase_sink.as_ref() {
            sink.set_phase(None);
        }
    }

    /// Whether the dressup pipeline records one ITEM per dressup.
    ///
    /// This is the cost the operator opts into. The recorders run either
    /// way; what the flag buys is a per-dressup entry under them.
    fn records_dressup_items(&self) -> bool {
        self.debug_options.enabled
    }

    /// The debug span the generator records into.
    fn debug_ctx(&self) -> Option<&'a crate::trace::debug_trace::ToolpathDebugContext> {
        self.debug_ctx
    }

    /// The semantic item the generator records into.
    fn semantic_ctx(&self) -> Option<&'a crate::trace::semantic_trace::ToolpathSemanticContext> {
        self.semantic_ctx
    }
}

/// Generate one toolpath from a handle — step (ii) of the
/// `generate_toolpath` job.
///
/// **This function holds no session.** It is a free function over
/// `&`[`GenerateToolpathHandle`], so it coerces to a plain `fn` pointer
/// and cannot capture a `&mut ProjectSession`. That is what lets a
/// caller run it off the frame loop while the session stays usable. The
/// sentry `tests/job_three_steps_equal_generate_toolpath.rs` pins the
/// coercion.
///
/// The function runs the generator, the dressups, the boundary clip, the
/// entry-descent split and the empty-generation gate — the tail
/// [`ProjectSession::generate_toolpath`] used to run inline. It writes
/// nothing: the answer goes back to the session through step (iii),
/// `apply(Command::AdoptResult { .. })`.
///
/// `cancel` reaches the generator, which polls it between levels.
///
/// `observer` names the surfaces this generation reports to — the phase
/// string the GUI lane shows, and whether the dressups record one item
/// each. A caller with no such surface passes [`GenObserver::none()`].
///
/// An `Err` carries no cancel variant: a generator that stops on the flag
/// reports [`SessionError::OperationFailed`] like any other refusal. A
/// caller that owns the flag reads the flag, not the message.
pub fn execute_job(
    handle: &GenerateToolpathHandle,
    observer: &GenObserver<'_>,
    cancel: &AtomicBool,
) -> Result<ToolpathComputeResult, SessionError> {
    let inputs = &handle.inputs;
    let context = &handle.context;

    // Create recorders. Unconditional, whatever `debug_options` says: the
    // CLI reads both traces off every result, and the cost the operator
    // opts into is the per-dressup ITEM set below, not the recorder.
    let debug_recorder =
        ToolpathDebugRecorder::new(context.toolpath_name.clone(), context.op_label);
    let semantic_recorder =
        ToolpathSemanticRecorder::new(context.toolpath_name.clone(), context.op_label);
    let debug_root = debug_recorder.root_context();
    let semantic_root = semantic_recorder.root_context();

    observer.set_phase(context.op_label);
    let core_scope = debug_root.start_span("core_generate", context.op_label);
    let core_ctx = core_scope.context();

    // Rest machining: when this toolpath cuts the stock previous ops left
    // (`StockSource::FromRemainingStock`), seed generation with the per-op
    // simulated snapshot so adaptive3d clears only the leftover. The same
    // snapshot is reused for dressup air-cut filtering below. The
    // "snapshot present" precondition was enforced by the submit step (a
    // FromRemainingStock op with no snapshot already returned an error), so
    // here it is guaranteed `Some` — never a fresh-stock fallback.
    let gen_initial_stock = context.generator_seed_stock();

    let op_scope = semantic_root.start_item(ToolpathSemanticKind::Operation, context.op_label);
    let child_ctx = op_scope.context();
    // WP11b: the executor reads the two bundles, never 21 loose arguments.
    let tp_result = execute_generation(
        inputs,
        context,
        &observer.bound(&core_ctx, &child_ctx),
        cancel,
    );

    match tp_result {
        Ok((annotated, findings)) => {
            let mut annotated = annotated;
            // Checkpoint C (Q2): the boundary clip below can add a
            // finding of its own, so the findings stay mutable until the
            // join rather than being moved straight into it.
            let mut findings = findings;

            if !annotated.toolpath.moves.is_empty() {
                core_scope.set_move_range(0, annotated.toolpath.moves.len().saturating_sub(1));
                op_scope.bind_to_toolpath(&annotated.toolpath, 0, annotated.toolpath.moves.len());
            }
            drop(core_scope);

            // Apply dressups. Reuse the `prior_stock` snapshot the submit
            // step captured (enables air-cut filter + rest-machining-aware
            // dressups).
            let prior_stock_ref = context.prior_stock.as_deref();
            // C1: the index-carrying channels this call site owns. The
            // session produces a semantic trace, so its recorder is
            // registered here once and every transform below reconciles
            // against it — dressups, the boundary clip and the
            // entry-descent split alike.
            let mut channels = crate::trace::transform_provenance::ReconcileSet::new(
                Some(&semantic_recorder),
                None,
            );
            observer.set_phase("Apply dressups");
            // N12 item 3: the feed-optimisation stock. The GUI worker built
            // one and this door passed `None`, so one configuration emitted
            // two sets of feed rates (the P0 parity test pinned it). The
            // predicate was always core's own; only the stock was missing.
            // Everything it reads is on the handle, so the item closes with
            // no new field.
            //
            // `feed_optimization_unavailable_reason` refuses
            // remaining-stock, Rest and 3D operations. A refusal is a
            // report, not an error: the pass is skipped and the generation
            // continues, which is what the GUI door did.
            let mut feed_opt_stock = if context.dressups.feed_optimization {
                match crate::compute::catalog::feed_optimization_unavailable_reason(
                    &inputs.operation,
                    context.stock_source,
                ) {
                    Some(reason) => {
                        tracing::warn!(
                            toolpath = %context.toolpath_name,
                            "Skipping feed optimization: {reason}"
                        );
                        None
                    }
                    None => Some(crate::dexel_stock::TriDexelStock::from_bounds(
                        &inputs.emission_stock_bbox,
                        // The GUI worker's own cell size, moved here
                        // unchanged (`worker/helpers.rs`).
                        (inputs.tool.diameter / 4.0).clamp(0.25, 2.0),
                    )),
                }
            } else {
                None
            };
            // G-RAMPTERRAIN: entry moves of SURFACE-RIDING
            // operations clip to the drop-cutter surface
            // (`entry_probe_leave` names them). Prism operations
            // get no probe: their entries legitimately descend
            // below the model surface (FINDINGS.md amendment 1).
            let entry_surface = match (
                inputs.mesh.as_deref(),
                inputs.spatial_index(),
                context.entry_probe_leave,
            ) {
                (Some(m), Some(idx), Some(leave)) => Some(crate::dressup::EntrySurfaceProbe {
                    mesh: m,
                    index: idx,
                    cutter: &inputs.tool_def,
                    stock_to_leave: leave,
                    off_mesh: crate::dressup::OffMeshEntry::PlungeFallback,
                    // G-ISOCLIPENTRY: `Some` only on a rest-driven pass —
                    // `gen_initial_stock` is `None` for `StockSource::Fresh`.
                    rest_stock: gen_initial_stock,
                }),
                _ => None,
            };
            let dressup_scope = observer
                .records_dressup_items()
                .then(|| debug_root.start_span("dressups", "Apply dressups"));
            let dressup_debug_ctx = dressup_scope.as_ref().map(|scope| scope.context());
            let dressed = crate::compute::execute::apply_dressups(
                annotated,
                crate::compute::execute::DressupContext {
                    cfg: &context.dressups,
                    nominal_feed_rate: context.feed_rate,
                    plunge_rate_mm_min: Some(context.plunge_rate),
                    tool_diameter: inputs.tool_def.diameter(),
                    safe_z: inputs.heights.retract_z,
                    stock_top: inputs.emission_stock_bbox.max.z,
                    prior_stock: prior_stock_ref,
                    feed_opt_stock: feed_opt_stock.as_mut(),
                    cutter: Some(&inputs.tool_def as &dyn crate::tool::MillingCutter),
                    entry_surface,
                    transform_capabilities: context.transform_capabilities,
                    debug_ctx: dressup_debug_ctx.as_ref(),
                    semantic_ctx: observer.records_dressup_items().then_some(&semantic_root),
                },
                &mut channels,
            );
            annotated = dressed;

            // ── Boundary clipping ─────────────────────────────────
            // After dressups, clip the toolpath to the machining boundary
            // (matching the GUI compute path). Spans are precisely
            // remapped through the clip via the input→output provenance
            // map (S83) so spans_valid stays true.
            if inputs.boundary_config.enabled {
                observer.set_phase("Clip to boundary");
                let _boundary_scope = observer
                    .records_dressup_items()
                    .then(|| debug_root.start_span("boundary_clip", "Clip to boundary"));
                let resolves_to_a_region_set = matches!(
                    inputs.boundary_config.source,
                    crate::compute::config::BoundarySource::PlannedTierRegions { .. }
                        | crate::compute::config::BoundarySource::DerivedRestRegions { .. }
                );
                annotated = if resolves_to_a_region_set {
                    // The submit step resolved this set, for exactly
                    // these two sources. It re-read the SAME memoised
                    // tier map `pre_boundary` came off, so agreement
                    // between the two clips stays structural — the P2.3
                    // rule that made `DerivedRestRegions` safe.
                    //
                    // The `None` arm cannot be reached from
                    // `ProjectSession::start`. It propagates rather than
                    // clipping against an empty set, because an
                    // un-confined fine tier runs over the whole board.
                    let regions = context.post_clip_regions.as_ref().ok_or_else(|| {
                        SessionError::OperationFailed(
                            "the boundary resolves to a region set and the submit step \
                             captured none"
                                .to_owned(),
                        )
                    })?;
                    ProjectSession::apply_boundary_clip_multi(
                        annotated,
                        &inputs.boundary_config,
                        regions.as_slice(),
                        &inputs.keep_out_footprints,
                        inputs.tool_def.diameter(),
                        inputs.heights.retract_z,
                        Some(context.plunge_rate),
                        &semantic_root,
                        &mut channels,
                        &mut findings,
                    )
                    .map_err(|e| SessionError::OperationFailed(e.to_string()))?
                } else {
                    ProjectSession::apply_boundary_clip(
                        annotated,
                        &inputs.boundary_config,
                        &inputs.emission_stock_bbox,
                        inputs.mesh.as_ref(),
                        inputs.face_boundary.as_ref(),
                        &inputs.keep_out_footprints,
                        inputs.tool_def.diameter(),
                        inputs.heights.retract_z,
                        Some(context.plunge_rate),
                        &semantic_root,
                        &mut channels,
                        &mut findings,
                    )
                    .map_err(|e| SessionError::OperationFailed(e.to_string()))?
                };
            }

            // ── Entry-descent optimization ────────────────────────
            // P1 W2 (reworked): split long safe_z-to-cut-depth plunges
            // by rapiding down to just above the INPUT STOCK's material
            // ceiling first — using the actual stock (the same snapshot
            // generation was seeded with for FromRemainingStock ops, or
            // the fresh-stock top otherwise), never a mesh height. This
            // runs on every generation (not just finish passes) since
            // the stock-derived ceiling is safe by construction — unlike
            // the mesh-derived height it replaces, which understates
            // remaining stock on rest-machining ops (the 151-collision
            // Rivers lesson — see `optimize_entry_descents`'s doc).
            //
            // Inserts moves after span construction, so the spans are
            // remapped through the same provenance-map contract the
            // boundary clip uses (`Span::remap`), rather than
            // invalidating them.
            //
            // G-ISOCLIPENTRY: on a REST-DRIVEN surface-riding pass the
            // same pass also ramps the descent instead of plunging it.
            // The dressup entry door ran before the boundary clip and
            // the clip rapids its geometry away, inventing a fresh
            // vertical descent per region re-entry that no door ever
            // sees; this is the post-clip door. `gen_initial_stock` is
            // `Some` exactly on `FromRemainingStock`, so a fresh-stock
            // pass is unchanged.
            {
                let rest_entry_ramp =
                    context
                        .entry_probe_leave
                        .map(|_| crate::dressup::RestEntryRamp {
                            contact_radius_mm: crate::finish::pencil::tip_contact_radius(
                                &inputs.tool_def,
                            ),
                            feed_rate: context.feed_rate,
                            plunge_rate: context.plunge_rate,
                        });
                let (transformed, _split_count) = crate::dressup::optimize_entry_descents_annotated(
                    annotated,
                    gen_initial_stock,
                    inputs.heights.top_z,
                    inputs.tool_def.radius(),
                    &inputs.tool_def,
                    rest_entry_ramp.as_ref(),
                );
                annotated = transformed.reconcile(&mut channels).into_inner();
            }

            // ── G-ENTRYEMPTY: the empty-generation gate ───────────
            // The pipeline is finished; `annotated` is exactly what
            // would be cached, simulated and posted. An operation that
            // reaches here with no cutting motion at all, from a region
            // that was NOT empty, is a generation failure that used to
            // be reported as `Done` — see `compute::generated_empty`'s
            // module doc for the ruling and for every case that is
            // legitimately empty (rest machining and the fixpoint
            // chains that depend on it are exempt, so this cannot wedge
            // `generate_all`).
            //
            // Placed HERE, before the result is built, so a refusal
            // returns no result at all and the submit step's removal
            // stands. That is load-bearing for G-STICKYEMPTY: a cached
            // empty result makes `PhantomPriorStockScan` treat the op as
            // "generated", which withholds its phantom prior-stock
            // snapshot on the next simulation and leaves a
            // `FromRemainingStock` op unable to regenerate even after
            // its parameters are put back — the empty generation
            // poisoning itself.
            let empty_verdict = crate::compute::generated_empty::classify(
                &annotated.toolpath,
                &crate::compute::generated_empty::EmptyGenerationInputs {
                    toolpath_name: &context.toolpath_name,
                    operation: &inputs.operation,
                    stock_source: context.stock_source,
                    seeded_machined_stock: gen_initial_stock.is_some(),
                    has_mesh: inputs.mesh.is_some(),
                    polygon_count: inputs.polygons.as_deref().map_or(0, Vec::len),
                    boundary_is_derived_rest_regions: matches!(
                        inputs.boundary_config.source,
                        crate::compute::config::BoundarySource::DerivedRestRegions { .. }
                    ),
                },
            );
            match empty_verdict {
                crate::compute::generated_empty::EmptyGenerationVerdict::NotEmpty => {}
                crate::compute::generated_empty::EmptyGenerationVerdict::Legitimate(reason) => {
                    tracing::info!(
                        toolpath = %context.toolpath_name,
                        operation = %context.op_label,
                        reason = reason.describe(),
                        "Generated an empty toolpath, and that is expected here"
                    );
                }
                crate::compute::generated_empty::EmptyGenerationVerdict::Refuse(refusal) => {
                    // No result is returned, and the entry the submit
                    // step removed stays removed — so a refused
                    // generation leaves the operation with NO cached
                    // result at all, which is what stops it poisoning
                    // the next one.
                    let _ = debug_recorder.finish();
                    let _ = semantic_recorder.finish();
                    observer.clear_phase();
                    return Err(SessionError::GeneratedEmpty(refusal.to_string()));
                }
            }

            observer.set_phase("Compute stats");
            let _stats_scope = observer
                .records_dressup_items()
                .then(|| debug_root.start_span("final_stats", "Compute stats"));

            // H2.1: ONE join, shared with the GUI compute worker. This
            // used to be a struct literal that read `findings.<field>`
            // eleven times — exhaustive on `ToolpathStats` but not on
            // `GenerationFindings`, so a new finding was dropped here in
            // silence. `stats_with_findings` destructures both sides, so
            // it cannot be.
            //
            // Byte-equivalent to the literal it replaces:
            // `Toolpath::total_cutting_distance` counts
            // `Linear | ArcCW | ArcCCW` and the helper counts everything
            // that is not `Rapid` — the same three variants, `MoveType`
            // having exactly four. The rapid distance and the
            // `compute_retract_trips` arguments were already identical.
            // S-4 (G-BYTE): stamp the machined-stock snapshot THIS
            // generation consumed. `context.prior_stock` — not
            // `gen_initial_stock` — is deliberately the subject: the
            // source-gated `gen_initial_stock` seeds the generator and
            // the entry-descent split, but the same `Arc` also reaches
            // the dressup air-cut filter ungated, so it is the snapshot
            // this generation consumed in the broadest true sense.
            // `None` therefore means no machined stock was consumed at
            // all, which is what the field documents.
            let stats = crate::compute::stats::stats_with_findings(
                &annotated.toolpath,
                annotated.spans_valid.then_some(annotated.spans.as_slice()),
                findings,
                context
                    .prior_stock
                    .as_deref()
                    .map(crate::compute::toolpath_stats::StockSnapshotStamp::of),
            );

            let mut debug_trace = debug_recorder.finish();
            let mut semantic_trace = semantic_recorder.finish();
            enrich_traces(&mut debug_trace, &mut semantic_trace);

            // Build the drill-op view atomically with the annotated
            // toolpath when this is a drill cycle (§6.E dual-rep
            // invariant). Material comes from the live stock config so
            // the chip-welding / peck-adequacy / plunge-feed gates
            // (F-016) see the workpiece's actual hardness.
            let drill_op = crate::compute::execute::build_drill_op_for_config(
                &inputs.operation,
                &inputs.drill_targets,
                &inputs.tool_def,
                &inputs.tool,
                &inputs.emission_stock_bbox,
                context.material.clone(),
                inputs.setup_transform.as_ref(),
            );
            let annotated_arc = Arc::new(annotated);
            let op_data = match drill_op {
                Some(d) => crate::ops::drill_op::OpData::DrillOp(Arc::new(d), annotated_arc),
                None => crate::ops::drill_op::OpData::Toolpath(annotated_arc),
            };
            observer.clear_phase();
            Ok(ToolpathComputeResult {
                op_data,
                stats,
                debug_trace: Some(debug_trace),
                semantic_trace: Some(semantic_trace),
            })
        }
        Err(e) => {
            drop(core_scope);
            let _ = debug_recorder.finish();
            let _ = semantic_recorder.finish();
            observer.clear_phase();
            Err(SessionError::OperationFailed(e.to_string()))
        }
    }
}

/// Generate one operation's motion from the resolved bundle — the narrowed
/// executor.
///
/// WP11b, `IMPLEMENTATION_PLAN.md` §22 ruling 4. The loose entry
/// `compute::execute::execute_operation_annotated` used to take 20 positional
/// arguments and build nothing, so every caller of it assembled the inputs
/// itself. CMP-02 made `ExecutionContext` the argument; this function takes
/// the two bundles the submit step produced, and a caller outside this module
/// can construct neither, so it cannot assemble a second answer.
///
/// `inputs` carries every per-generation input.
/// [`ResolvedGenInputs::spatial_index`] is FORCED here, so this function
/// runs off the frame loop. `context` carries the session reads: the
/// machined-stock seed, the rest-analysis dials and the machine envelope.
/// `observer` carries the trace contexts the generator records into.
///
/// The entry stays, and WP12 made it `pub(crate)` (§23 ruling 1), so this
/// function is the only door to it that a caller outside the crate can
/// reach. One in-crate caller remains beside this one: the strategy
/// advisor below. The four integration tests that called it moved in-crate.
/// The sentry is `tests/loose_executor_is_crate_private_wp12.rs`.
pub fn execute_generation(
    inputs: &ResolvedGenInputs,
    context: &GenContext,
    observer: &GenObserver<'_>,
    cancel: &AtomicBool,
) -> Result<
    (
        crate::compute::execute::GeneratedToolpath,
        crate::compute::execute::GenerationFindings,
    ),
    crate::compute::execute::OperationError,
> {
    // P2.3: `pre_boundary_regions` (resolved once, by the submit step,
    // alongside `pre_boundary`) reaches the mesh-finish family's pre-clip
    // through `ExecutionContext::boundary_regions`. The advisor's probe
    // below leaves that field at its constructor default, `None`.
    let seed_stock = context.generator_seed_stock();
    let regions = inputs
        .pre_boundary_regions
        .as_deref()
        .map(crate::geometry::region_set::RegionSet::from_slice);
    let findings = std::cell::RefCell::new(crate::compute::execute::GenerationFindings::default());
    let ctx = crate::compute::execute::ExecutionContext {
        mesh: inputs.mesh.as_deref(),
        index: inputs.spatial_index(),
        polygons: inputs.polygons.as_deref().map(|v| v.as_slice()),
        drill_targets: &inputs.drill_targets,
        setup_transform: inputs.setup_transform.as_ref(),
        prev_tool_radius: inputs.prev_tool_radius,
        reference_tool_cfg: inputs.reference_tool_cfg.clone(),
        debug_ctx: observer.debug_ctx(),
        initial_stock: seed_stock,
        semantic_ctx: observer.semantic_ctx(),
        boundary: inputs.pre_boundary.as_ref(),
        boundary_regions: regions.as_ref(),
        link_kinematics: context.link_kinematics.clone(),
        rest_analysis: Some(&context.rest_analysis),
        ..crate::compute::execute::ExecutionContext::new(
            &findings,
            &inputs.tool_def,
            &inputs.tool,
            &inputs.heights,
            &inputs.cutting_levels,
            &inputs.emission_stock_bbox,
            cancel,
        )
    };
    let generated = crate::compute::execute::execute_operation_annotated(&ctx, &inputs.operation)?;
    Ok((generated, findings.into_inner()))
}

/// Clearing strategies the advisor compares for a 3D roughing op — the two
/// endpoints of the speed/load trade-off: conventional offset clearing
/// ([`ContourParallel`](ClearingStrategy::ContourParallel)) vs
/// constant-engagement trochoidal ([`ContourSpiral`](ClearingStrategy::ContourSpiral)).
/// `recommend_clearing_strategy` times both at their load-limited params and
/// lets machine acceleration decide. Extend by adding variants here.
const ADVISOR_CANDIDATE_STRATEGIES: [ClearingStrategy; 2] = [
    ClearingStrategy::ContourParallel,
    ClearingStrategy::ContourSpiral,
];

/// Map a Suggest pass's warnings to the binding [`LoadRegime`] for the
/// advisor's *why* string. Deflection-binding warnings mean the tool is the
/// limit (tool-limited); everything else reads as unconstrained here.
///
/// Machine-limited (power-binding) detection is deliberately not inferred
/// from Suggest warnings — Suggest does not emit a power-cap warning, and the
/// regime label only colours the explanation (the *choice* is always the
/// measured wall-clock minimum), so a conservative "unconstrained" default is
/// honest until a power-gate signal is threaded in.
///
/// [`LoadRegime`]: crate::machine::strategy_advisor::LoadRegime
fn regime_from_suggest_warnings(
    warnings: &[crate::feeds::suggest::SuggestWarning],
) -> crate::machine::strategy_advisor::LoadRegime {
    use crate::feeds::suggest::SuggestWarning;
    let deflection_bound = warnings.iter().any(|w| match w {
        SuggestWarning::DppCappedByDeflection { .. } => true,
        SuggestWarning::AxialDocClampedByEnvelope { binding, .. } => *binding == "deflection",
        _ => false,
    });
    if deflection_bound {
        crate::machine::strategy_advisor::LoadRegime::ToolLimited
    } else {
        crate::machine::strategy_advisor::LoadRegime::Unconstrained
    }
}

/// Map a modulated path's per-move binding-constraint distribution to the
/// advisor's [`LoadRegime`]. The dominant (most-frequent) binding constraint
/// decides: deflection → tool-limited; power / machine-max-feed /
/// kinematic-reach → machine-limited; the chipload band (max or min) →
/// unconstrained (the comfortable regime, neither the tool nor the machine
/// stressed). This is the unified-load-model upgrade over
/// [`regime_from_suggest_warnings`]: the label now comes from the actual
/// per-move binding signal of the *optimized* path, not a Suggest-warning
/// heuristic (`planning/UNIFIED_LOAD_MODEL_2026-06-18.md` §5.4).
fn regime_from_binding(
    summary: &crate::tool_load::ModulationSummary,
) -> crate::machine::strategy_advisor::LoadRegime {
    use crate::machine::strategy_advisor::LoadRegime;
    use crate::tool_load::BindingConstraint;
    let dominant = summary
        .binding_constraint_distribution
        .iter()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(binding, _)| *binding);
    match dominant {
        // Phase 3: a dominant `PlungeRate` binding means the CUTTER's
        // centre-cutting capability held the feed down — the operation's
        // plunge rate is set for what the tool can plunge, not for what the
        // machine can drive. It joins deflection as a tool limit, not the
        // comfortable regime: `Unconstrained` would report a path that is
        // mostly capped descents as a light cut. On a real path this
        // binding is never dominant (plunges are a small minority of moves),
        // so the arm is close to unreachable.
        Some(BindingConstraint::DeflectionMax | BindingConstraint::PlungeRate) => {
            LoadRegime::ToolLimited
        }
        Some(
            BindingConstraint::PowerMax
            | BindingConstraint::MachineMaxFeed
            | BindingConstraint::KinematicReach,
        ) => LoadRegime::MachineLimited,
        Some(BindingConstraint::ChiploadMax | BindingConstraint::ChiploadMin) | None => {
            LoadRegime::Unconstrained
        }
    }
}

/// Every session read the strategy advisor makes, captured once.
///
/// The advisor reaches the session in four places beyond the resolved
/// generation inputs: the ranking's machine and material, the candidate
/// simulation's stock frame and post dials, the chipload envelope's
/// material and stored toolpath, and the modulator's machine and default
/// spindle speed. This struct is all four, so step (ii) holds none of them
/// by reference.
struct AdvisorContext {
    /// The machine the candidates are timed and modulated against.
    machine: crate::machine::MachineProfile,
    /// The post dials the candidate simulation reads: the default spindle
    /// speed and the rapid-feed pair.
    post: super::ProjectPostConfig,
    /// The stock. The ranking reads its material and its workholding
    /// rigidity; the chipload envelope reads the material again.
    stock: crate::compute::StockConfig,
    /// The simulator-side id of the toolpath the advisor advises on.
    toolpath_id: ToolpathId,
    /// The toolpath's name, for the candidate simulation entry.
    toolpath_name: String,
    /// Whether the toolpath is enabled. The chipload envelope resolver
    /// skips a disabled toolpath, so the advisor then times the RAW path.
    toolpath_enabled: bool,
    /// The toolpath's STORED operation — what the chipload envelope
    /// resolver reads.
    stored_operation: crate::compute::OperationConfig,
    /// The cutter the toolpath is bound to. `None` when the toolpath names
    /// no tool in the project; the advisor then times the raw path.
    stored_tool: Option<ToolConfig>,
    /// The world-frame stock bounding box.
    stock_bbox: BoundingBox3,
    /// Q1: the bbox of the model this toolpath machines, matched on
    /// `tc.model_id`. Every candidate re-runs Suggest, and
    /// `SuggestContext::model_bbox` gates the runtime-sanity stepover
    /// back-off and the Adaptive3d entry-style pass. Without it the
    /// advisor ranked candidates at parameters the generation path
    /// would never emit. `None` when the id names no model.
    model_bbox: Option<BoundingBox3>,
    /// The setup frame the candidate simulation runs in.
    setup_ctx: super::SetupEvalContext,
}

/// What [`ProjectSession::start`] captured for one
/// `recommend_clearing_strategy` job.
///
/// The handle is the whole input of step (ii), so
/// [`execute_recommend_clearing_strategy`] reads no session and runs off
/// the frame loop.
///
/// `index` is public because a caller labels the job with it. `inputs` and
/// `context` stay private, so no caller outside this module builds a
/// handle: a handle is the evidence of one submit.
pub struct RecommendClearingStrategyHandle {
    /// The index of the toolpath this job advises on.
    pub index: usize,
    /// Every per-generation input, resolved once by
    /// [`ProjectSession::resolve_generation_inputs`]. Every candidate
    /// plans off this ONE resolution, so only the strategy varies.
    inputs: ResolvedGenInputs,
    /// Every other session read the ranking makes.
    context: AdvisorContext,
}

impl RecommendClearingStrategyHandle {
    /// The toolpath's name, as the submit step read it.
    #[must_use]
    pub fn toolpath_name(&self) -> &str {
        &self.context.toolpath_name
    }

    /// Whether the operation carries a clearing strategy at all.
    ///
    /// Only `Adaptive3d` does. A caller that wants to know before it pays
    /// for the ranking reads this; [`execute_recommend_clearing_strategy`]
    /// answers `Ok(None)` for every other operation.
    #[must_use]
    pub(crate) fn has_clearing_strategy(&self) -> bool {
        matches!(
            self.inputs.operation,
            crate::compute::OperationConfig::Adaptive3d(_)
        )
    }
}

/// Rank the clearing strategies from a handle — step (ii) of the
/// `recommend_clearing_strategy` job.
///
/// **This function holds no session.** It is a free function over
/// `&`[`RecommendClearingStrategyHandle`], so it coerces to a plain `fn`
/// pointer and cannot capture a `&ProjectSession`. That is what lets a
/// caller run it off the frame loop while the session stays usable.
///
/// For each candidate [`ClearingStrategy`] it (1) runs Suggest to back the
/// params off to the deflection / power limits, (2) plans the clearing
/// toolpath off the handle's single shared resolution, (3) simulates and
/// modulates that candidate, and (4) ranks them through
/// [`crate::machine::strategy_advisor::recommend_strategy`], which times each path
/// through the accel-aware integrator.
///
/// Returns `Ok(None)` when the operation is not an `Adaptive3d` op or no
/// candidate plans a usable path. The job writes nothing; there is no step
/// (iii).
///
/// `cancel` stops the loop between candidates and reaches the generator
/// and the candidate simulation.
///
/// # Errors
///
/// None today. The result type is a `Result` because the row's answer
/// column names it and the ranking may grow a refusal.
pub fn execute_recommend_clearing_strategy(
    handle: &RecommendClearingStrategyHandle,
    cancel: &AtomicBool,
) -> Result<Option<crate::machine::strategy_advisor::StrategyRecommendation>, SessionError> {
    use crate::machine::strategy_advisor::{StrategyCandidate, recommend_strategy};

    let resolved = &handle.inputs;
    let context = &handle.context;
    // Only Adaptive3d carries a clearing strategy.
    if !handle.has_clearing_strategy() {
        return Ok(None);
    }

    let machine = &context.machine;
    let material = &context.stock.material;
    let workholding = context.stock.workholding_rigidity;

    // Plan each candidate at its load-limited params. Collect OWNED
    // toolpaths so the `StrategyCandidate` borrows outlive the ranking.
    let mut planned: Vec<(
        ClearingStrategy,
        crate::toolpath::Toolpath,
        crate::machine::strategy_advisor::LoadRegime,
    )> = Vec::new();
    for &strategy in ADVISOR_CANDIDATE_STRATEGIES.iter() {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        // Override the clearing strategy on a clone of the resolved op.
        let mut op = resolved.operation.clone();
        if let crate::compute::OperationConfig::Adaptive3d(ref mut cfg) = op {
            cfg.clearing_strategy = strategy;
        }
        // Back the params off to the load limit via Suggest.
        let suggested = crate::feeds::suggest::suggest_for_operation(
            crate::feeds::suggest::SuggestForOperationInput {
                operation: &op,
                tool: &resolved.tool,
                machine,
                material,
                workholding,
                lut: crate::feeds::embedded_vendor_lut(),
                spindle_strategy: crate::feeds::SpindleStrategy::default(),
                // Q1: the bbox the runtime-sanity back-off and the
                // Adaptive3d entry-style pass read. The stock reaches
                // the ranking through `context.stock` below, so
                // `SuggestContext::stock` stays empty rather than
                // carrying the same value twice.
                // `upstream_leftover_stock_mm` stays `None`: no lookup
                // here gives it, and v1 does not read it.
                context: crate::feeds::suggest::SuggestContext {
                    model_bbox: context.model_bbox.as_ref(),
                    ..crate::feeds::suggest::SuggestContext::default()
                },
            },
        );
        let (op_loadlimited, regime) = match suggested {
            Ok(s) => {
                let regime = regime_from_suggest_warnings(&s.warnings);
                (s.operation, regime)
            }
            // Suggest refused (e.g. a material without primary-source Kc).
            // Still worth timing at the raw params; regime is unknown.
            Err(_) => (
                op,
                crate::machine::strategy_advisor::LoadRegime::Unconstrained,
            ),
        };
        // Plan the clearing toolpath — no recorders / dressups / persist;
        // the raw path is what we time.
        //
        // WP12 / §23 ruling 3: this is the named §5 residual. It plans a
        // DIFFERENT operation (`op_loadlimited`) than the bundle carries,
        // so `execute_generation` cannot serve it, and the loose entry
        // stays `pub(crate)` for it. WP14a made the advisor a `Job` row and
        // the residual MOVED here, into a free function; it did not close.
        let findings =
            std::cell::RefCell::new(crate::compute::execute::GenerationFindings::default());
        let ctx = crate::compute::execute::ExecutionContext {
            mesh: resolved.mesh.as_deref(),
            index: resolved.spatial_index(),
            polygons: resolved.polygons.as_deref().map(|v| v.as_slice()),
            prev_tool_radius: resolved.prev_tool_radius,
            boundary: resolved.pre_boundary.as_ref(),
            // Everything else stays at the constructor default. The probe
            // records no trace, seeds no stock, resolves no reference tool
            // (it plans clearing ops only, never pencil) and discards its
            // findings — `findings` is the sink it throws away.
            ..crate::compute::execute::ExecutionContext::new(
                &findings,
                &resolved.tool_def,
                &resolved.tool,
                &resolved.heights,
                &resolved.cutting_levels,
                &resolved.emission_stock_bbox,
                cancel,
            )
        };
        let result = crate::compute::execute::execute_operation_annotated(&ctx, &op_loadlimited);
        if let Ok(annotated) = result {
            let annotated_arc = Arc::new(annotated);
            // Compare OPTIMIZED candidates: simulate the path, run F-039
            // modulation, and time the MODULATED toolpath so the spiral's
            // flatter, lighter engagement (which modulation can exploit
            // harder than the parallel path's corner spikes) shows up in
            // wall-clock. The regime label falls out of the per-move
            // binding constraint of the optimized path. Falls back to the
            // raw path + Suggest-warning regime when the machine carries
            // no kinematics or the candidate can't be simulated/modulated.
            let (toolpath, regime) = match optimized_candidate(
                context,
                &annotated_arc,
                &resolved.tool,
                &op_loadlimited,
                cancel,
            ) {
                Some(opt) => opt,
                None => (annotated_arc.toolpath.clone(), regime),
            };
            planned.push((strategy, toolpath, regime));
        }
    }

    let candidates: Vec<StrategyCandidate<'_>> = planned
        .iter()
        .map(|(strategy, toolpath, regime)| StrategyCandidate {
            strategy: *strategy,
            toolpath,
            regime: *regime,
            geometry_forced: false,
        })
        .collect();

    Ok(recommend_strategy(&candidates, machine))
}

/// What [`ProjectSession::start`] captured for one `optimize_toolpath`
/// job.
///
/// The handle owns a PRIVATE CLONE of the session. The candidate loop
/// regenerates and re-simulates per candidate, so this row cannot reduce
/// its inputs to a capture list the way the two read rows do (§24 ruling
/// 2). The clone is the job's own, so step (ii) holds no reference to the
/// caller's session and the live session stays usable.
///
/// `index` is public because a caller labels the job with it. `session`
/// and `trace` stay private, so no caller outside this module builds a
/// handle: a handle is the evidence of one submit.
pub struct OptimizeToolpathHandle {
    /// The index of the toolpath this job optimizes.
    pub index: usize,
    /// The session the candidate loop mutates. A private clone.
    session: ProjectSession,
    /// The baseline trace the candidates are scored against.
    ///
    /// Held BESIDE the clone, not read out of it.
    /// [`crate::tool_load::optimize::optimize_toolpath`] takes
    /// `&mut ProjectSession` and `&SimulationCutTrace` at once, and a
    /// trace borrowed from `session.simulation` cannot live across the
    /// mutable borrow of `session`.
    trace: Arc<SimulationCutTrace>,
    /// Where the search publishes its rung and its candidate count
    /// (WP29).
    ///
    /// The progress rides the HANDLE and not a fifth parameter on
    /// [`execute_optimize_toolpath`], because that signature is pinned by
    /// `crates/rs_cam_core/tests/optimize_toolpath_is_a_job_wp14b.rs`:
    /// the arm coerces the function to
    /// `fn(&mut OptimizeToolpathHandle, &AtomicBool) -> OptimizeOutcome`,
    /// and a fifth parameter breaks that coercion.
    ///
    /// It starts SILENT. One door attaches a readable one, so the field
    /// stays private and a handle is still the evidence of one submit.
    progress: Arc<crate::tool_load::optimize::OptimizeProgress>,
}

impl OptimizeToolpathHandle {
    /// Attach the progress sink the caller reads while the search runs
    /// (WP29).
    ///
    /// The GUI builds one `Arc` per submit, attaches it here and clones it
    /// into `AppState::optimize_run`, so the frame loop reads what the
    /// worker thread writes. A caller that never calls this keeps the
    /// silent default and changes nothing.
    pub fn with_progress(&mut self, progress: Arc<crate::tool_load::optimize::OptimizeProgress>) {
        self.progress = progress;
    }
}

/// Run the optimizer's candidate search from a handle — step (ii) of the
/// `optimize_toolpath` job.
///
/// **This function holds no reference to the caller's session.** It is a
/// free function over `&mut`[`OptimizeToolpathHandle`], so it coerces to
/// a plain `fn` pointer and cannot capture a `&ProjectSession`. The
/// session it mutates is the handle's own clone.
///
/// It takes the handle by `&mut` rather than by `&` because
/// [`crate::tool_load::optimize::optimize_toolpath`] takes
/// `&mut ProjectSession`: a shared handle would make this function clone
/// the session a SECOND time (§28 ruling 4).
///
/// There is no step (iii). The outcome goes to the caller; the live
/// session is untouched, and the GUI applies a chosen candidate through
/// `Command::RestoreToolpathSnapshot` on an operator click.
///
/// The answer is the bare [`OptimizeOutcome`](crate::tool_load::optimize::OptimizeOutcome)
/// and not a `Result`. The optimizer reports every refusal as an
/// `OutcomeKind::Skipped` value and never as an `Err`.
///
/// `cancel` is polled between candidates and between search stages, and
/// it reaches the generator and the candidate simulation.
///
/// The search publishes its rung and its candidate count on the handle's
/// own progress sink (WP29). The sink rides the HANDLE, so this
/// signature does not move.
pub fn execute_optimize_toolpath(
    handle: &mut OptimizeToolpathHandle,
    cancel: &AtomicBool,
) -> crate::tool_load::optimize::OptimizeOutcome {
    let index = handle.index;
    let trace = Arc::clone(&handle.trace);
    // WP29 — clone the progress OUT before the `&mut handle.session`
    // borrow, the way the trace above already is.
    let progress = Arc::clone(&handle.progress);
    crate::tool_load::optimize::optimize_toolpath_observed(
        &mut handle.session,
        &trace,
        index,
        cancel,
        &progress,
    )
}

/// Strategy-advisor companion to
/// [`execute_recommend_clearing_strategy`]: turn a raw candidate path into
/// the *optimized* path the user would actually run, plus its binding
/// [`LoadRegime`](crate::machine::strategy_advisor::LoadRegime). It simulates the
/// candidate in isolation to capture per-move engagement, then routes it
/// through the shared F-039 core
/// (`modulate_annotated_against_trace`) so the timed path carries
/// modulated feeds. Returns `None` (the caller times the raw path with the
/// Suggest-warning regime) when the machine has no kinematics block, the
/// candidate can't be simulated, no chipload band is available, or
/// modulation refuses.
///
/// A free function over the captured [`AdvisorContext`]. It holds no
/// session, and it reaches the chipload envelope through
/// [`crate::tool_load::chipload_envelope_for_toolpath`] — the per-toolpath
/// half of the session-wide resolver, which the gate and the viewport read
/// through the same function.
fn optimized_candidate(
    context: &AdvisorContext,
    annotated: &Arc<crate::trace::toolpath_spans::AnnotatedToolpath>,
    tool_cfg: &ToolConfig,
    operation: &crate::compute::OperationConfig,
    cancel: &AtomicBool,
) -> Option<(
    crate::toolpath::Toolpath,
    crate::machine::strategy_advisor::LoadRegime,
)> {
    // Modulate against the SAME kinematics + feed envelope
    // [`recommend_strategy`](crate::machine::strategy_advisor::recommend_strategy)
    // times the candidate with, so the optimized feeds are clamped to the
    // exact ceilings they're then timed against. `effective_kinematics`
    // (never `None` — falls back to the generic-wood-router profile) is
    // also why the advisor can optimize machines that carry no explicit
    // kinematics block, unlike the production post-sim pass.
    let kinematics = context.machine.effective_kinematics();
    let max_feed = context.machine.max_feed_mm_min.max(1.0);
    let rapid_feed = max_feed;

    let cut_trace =
        simulate_candidate_isolated(context, Arc::clone(annotated), tool_cfg, operation, cancel)?;
    let toolpath_id = context.toolpath_id;
    // The envelope resolver skips a disabled toolpath, and it reads the
    // STORED operation and the STORED cutter — never the load-limited clone
    // this candidate plans. Both facts ride on the handle, so the band the
    // advisor modulates against is the band the post-sim gate reads.
    if !context.toolpath_enabled {
        return None;
    }
    let band_range = crate::tool_load::chipload_envelope_for_toolpath(
        &context.stock.material,
        context.stored_tool.as_ref()?,
        &context.stored_operation,
        toolpath_id,
        Some(&cut_trace),
    )?;
    let band =
        crate::dressup::feed_modulation::ChiploadBand::new(band_range.start, band_range.end)?;
    // ConstrainedMax @ aggressiveness 1.0 — the "bomber feeds" operating
    // point and the `SimulationOptions` default, so the advisor times the
    // same path the user gets after a default sim.
    let strategy = crate::dressup::feed_modulation::ModulationStrategy::ConstrainedMax;
    let aggressiveness = 1.0;

    let (modulated, outcome) = modulate_annotated_against_trace(
        &FeedContext {
            material: &context.stock.material,
            machine: &context.machine,
            default_spindle_rpm: context.post.spindle_speed,
        },
        annotated.as_ref(),
        operation,
        tool_cfg,
        toolpath_id,
        &cut_trace,
        band,
        kinematics,
        max_feed,
        rapid_feed,
        strategy,
        aggressiveness,
    )?;
    let regime = outcome
        .build_summary(operation.feed_rate(), aggressiveness, strategy)
        .map(|s| regime_from_binding(&s))
        .unwrap_or(crate::machine::strategy_advisor::LoadRegime::Unconstrained);
    Some((modulated, regime))
}

/// The three session records the F-039 modulation core reads.
///
/// A struct rather than three positional arguments: the caller list is
/// already at the `too_many_arguments` ceiling, and two of the three are
/// borrowed records a positional signature lets a caller swap.
struct FeedContext<'a> {
    /// The stock material. It supplies Kc and the affine force
    /// coefficients.
    material: &'a crate::material::Material,
    /// The machine. It supplies the available power at the spindle speed.
    machine: &'a crate::machine::MachineProfile,
    /// The post's spindle speed, used when the operation carries none.
    default_spindle_rpm: u32,
}

/// Modulate ONE toolpath's per-move feeds against a simulation cut
/// trace, returning the modulated [`Toolpath`] and the raw
/// [`ModulationOutcome`] (per-move binding map + summary inputs).
///
/// This is the shared F-039 core consumed by two callers:
/// [`ProjectSession::apply_adaptive_feed_modulation`] (the production
/// post-sim pass, which stamps the result back onto the session's results)
/// and [`optimized_candidate`] (the strategy advisor, which times the
/// *modulated* path so it compares optimized candidates rather than raw
/// Suggest-feed ones). The session-side caller takes the method of the
/// same name, which reads the three records off `self` and delegates
/// here; the advisor reads them off its handle. One body, two doors.
/// Keeping the engagement aggregation + `ModulationContext` build in
/// one place is the anti-drift discipline of the unified load model
/// (`planning/UNIFIED_LOAD_MODEL_2026-06-18.md` §5) — the deflection
/// cap, power cap, and chipload band are derived here once.
///
/// Returns `None` when the op carries no usable RPM, has no moves, or
/// the modulator refuses (e.g. an empty engagement vector).
#[allow(clippy::too_many_arguments)]
fn modulate_annotated_against_trace(
    context: &FeedContext<'_>,
    annotated: &crate::trace::toolpath_spans::AnnotatedToolpath,
    operation: &crate::compute::OperationConfig,
    tool_cfg: &ToolConfig,
    toolpath_id: ToolpathId,
    cut_trace: &crate::stock::simulation_cut::SimulationCutTrace,
    band: crate::dressup::feed_modulation::ChiploadBand,
    kinematics: crate::machine::kinematics::MachineKinematics,
    max_feed: f64,
    rapid_feed: f64,
    strategy: crate::dressup::feed_modulation::ModulationStrategy,
    aggressiveness: f64,
) -> Option<(
    crate::toolpath::Toolpath,
    crate::dressup::feed_modulation::ModulationOutcome,
)> {
    use crate::dressup::feed_modulation::{
        DeflectionLimitInputs, ModulationContext, PerMoveEngagement, PowerLimitInputs,
        adaptive_feed_modulate,
    };

    let flute_count = tool_cfg.flute_count.max(1);
    let spindle_rpm = operation
        .spindle_rpm()
        .unwrap_or(context.default_spindle_rpm);
    if spindle_rpm == 0 {
        return None;
    }
    let move_count = annotated.toolpath.moves.len();
    if move_count == 0 {
        return None;
    }

    // Stage 4 — planner-predicted engagement for the constructive
    // contour-spiral, in two layers:
    //
    //  (a) Per-move: the spiral's own leading-arc engagement (α/2π)
    //      computed on its clean 2D material grid, carried
    //      positionally on the AnnotatedToolpath and looked up by
    //      cut-move target. RDP simplification keeps a subset of the
    //      emitted points verbatim, so kept cut moves hit exactly.
    //  (b) Uniform fallback: the op's target engagement
    //      (stepover/diameter via the F1 leading-arc → radial-WOC
    //      bridge), used for cut moves whose position isn't in the
    //      sampler (arc-fit / lead-in points) and for the 2D
    //      Adaptive spiral op, which carries no 3D sampler.
    //
    // The dexel simulator's cylinder-side `radial_woc_fraction`
    // reads ~10× low for adaptive ops (CLAUDE.md), so modulation on
    // the sim scalar alone never lets the flat-load spiral run
    // faster. Per move we take `max(sim, planner)` so any genuine
    // spike the simulator *does* resolve still wins — never feeding
    // above the higher of the two estimates. Gated strictly to the
    // ContourSpiral strategy: the Agent / AgentSearch path has real
    // ~2.5× target engagement spikes that a planner floor would
    // dangerously over-feed. See
    // planning/ADAPTIVE_CLEARING_ALGO_REVIEW_2026-06-12.md §"Stage 4".
    let is_contour_spiral = matches!(
        operation,
        crate::compute::OperationConfig::Adaptive3d(c)
            if matches!(
                c.clearing_strategy,
                crate::compute::operation_configs::ClearingStrategy::ContourSpiral
            )
    ) || matches!(
        operation,
        crate::compute::OperationConfig::Adaptive(c)
            if matches!(c.path_strategy, crate::adaptive::PathStrategy2d::ContourSpiral)
    );
    let planner_uniform_woc: Option<f64> = if is_contour_spiral {
        let stepover = match operation {
            crate::compute::OperationConfig::Adaptive3d(c) => Some(c.stepover),
            crate::compute::OperationConfig::Adaptive(c) => Some(c.stepover),
            _ => None,
        };
        stepover.and_then(|s| {
            let r = tool_cfg.diameter * 0.5;
            (r > 0.0 && s > 0.0).then(|| {
                let f = crate::ops::adaptive_shared::target_engagement_fraction(s, r);
                crate::ops::adaptive_shared::radial_woc_fraction_from_leading_arc(f)
            })
        })
    } else {
        None
    };
    // Position key for the per-move planner-engagement lookup
    // (0.001 mm grid — far finer than the cut-point spacing).
    let pos_key = |p: &crate::geo::P3| -> (i64, i64, i64) {
        (
            (p.x * 1000.0).round() as i64,
            (p.y * 1000.0).round() as i64,
            (p.z * 1000.0).round() as i64,
        )
    };
    let planner_map: std::collections::HashMap<(i64, i64, i64), f64> =
        if is_contour_spiral && !annotated.planner_engagement.is_empty() {
            annotated
                .planner_engagement
                .iter()
                .map(|(p, f)| (pos_key(p), *f))
                .collect()
        } else {
            std::collections::HashMap::new()
        };

    // Aggregate per-move engagement (time-weighted mean over the
    // move's samples). Samples filter on `is_cutting` so air-cut
    // and rapid moves stay at default `(0.0, 0.0)` engagement —
    // the modulator skips them via its own `should_skip` /
    // zero-engagement short-circuits.
    let mut radial_num = vec![0.0_f64; move_count];
    let mut axial_num = vec![0.0_f64; move_count];
    // T-11: the absolute axial engagement in mm, carried alongside the
    // fraction. `axial_num` holds a fraction of the tool's FLUTE LENGTH, so
    // it cannot be turned back into a depth without the flute length. The
    // dexel already measures the millimetres; carry them rather than
    // re-derive them.
    let mut axial_mm_num = vec![0.0_f64; move_count];
    let mut weight_sum = vec![0.0_f64; move_count];
    for sample in &cut_trace.samples {
        if sample.toolpath_id != toolpath_id {
            continue;
        }
        if !sample.is_cutting {
            continue;
        }
        if sample.move_index >= move_count {
            continue;
        }
        let w = sample.segment_time_s.max(0.0);
        if w <= 0.0 {
            continue;
        }
        #[allow(clippy::indexing_slicing)]
        // SAFETY: move_index < move_count checked above.
        {
            radial_num[sample.move_index] += sample.engagement.radial_woc_fraction.max(0.0) * w;
            // C2: an unmeasured axial fraction contributes nothing but
            // still carries its time weight — byte-identical to the
            // pre-C2 `0.0` sentinel, and now visibly a choice. The
            // modulator's own `PerMoveEngagement` keeps a plain f64:
            // there, `0.0` legitimately means "air" (see its doc).
            axial_num[sample.move_index] +=
                sample.engagement.axial_doc_fraction.unwrap_or(0.0).max(0.0) * w;
            // T-11: the absolute reading, same time weighting.
            axial_mm_num[sample.move_index] += sample.axial_doc_mm.max(0.0) * w;
            weight_sum[sample.move_index] += w;
        }
    }
    let engagements: Vec<PerMoveEngagement> = (0..move_count)
        .map(|i| {
            #[allow(clippy::indexing_slicing)]
            // SAFETY: i < move_count by construction.
            let w = weight_sum[i];
            if w <= 0.0 {
                return PerMoveEngagement::default();
            }
            #[allow(clippy::indexing_slicing)]
            // SAFETY: i < move_count by construction.
            let sim_radial = radial_num[i] / w;
            #[allow(clippy::indexing_slicing)]
            // SAFETY: i < move_count by construction.
            let axial = axial_num[i] / w;
            #[allow(clippy::indexing_slicing)]
            // SAFETY: i < move_count by construction.
            let axial_mm = axial_mm_num[i] / w;
            // Apply the planner engagement on lateral clearing /
            // finishing cuts only — entry helix, ramp, and linking
            // moves are not the spiral's flat-load wraps, so they
            // keep the sim-measured reading. Per-move sampler first,
            // uniform target floor as fallback; `max` with sim keeps
            // any genuine spike the simulator resolves.
            let m = annotated.toolpath.moves.get(i);
            let radial = if matches!(
                m.map(|m| m.intent),
                Some(crate::toolpath::MoveIntent::ClearingCut)
                    | Some(crate::toolpath::MoveIntent::FinishingCut)
            ) {
                let planner_woc = m
                    .and_then(|m| planner_map.get(&pos_key(&m.target)).copied())
                    .map(crate::ops::adaptive_shared::radial_woc_fraction_from_leading_arc)
                    .or(planner_uniform_woc);
                match planner_woc {
                    Some(pw) => sim_radial.max(pw),
                    None => sim_radial,
                }
            } else {
                sim_radial
            };
            PerMoveEngagement {
                radial_woc_fraction: radial,
                axial_doc_fraction: axial,
                axial_doc_mm: axial_mm,
            }
        })
        .collect();

    // F-039 — wire optional deflection + power constraint
    // inputs. Material + tool data is enough to recover Kc,
    // stickout, engagement diameter, and Young's modulus; the
    // machine's `power_at_rpm × safety_factor` gives the
    // available power.
    let material = context.material;
    // Materials without a primary-source Kc disable both the
    // deflection and power constraints in the constrained-max
    // solver; the solver falls through to chipload + machine +
    // kinematics caps. See `Material::kc_n_per_mm2`.
    let kc_opt = material.kc_n_per_mm2();
    let tool_def = crate::compute::cutter::build_cutter(tool_cfg);
    // Use the per-toolpath max axial DOC from the cut trace
    // as the deflection / power reference; falls back to
    // diameter when unavailable (no cutting samples → no
    // constraint active).
    let max_axial = cut_trace
        .samples
        .iter()
        .filter(|s| s.toolpath_id == toolpath_id && s.is_cutting)
        .map(|s| s.axial_engagement_mm.max(0.0))
        .fold(0.0_f64, f64::max);
    let nominal_axial = if max_axial > 0.0 { max_axial } else { 0.0 };
    let engagement_dia = tool_def.lookup_diameter_at(max_axial.max(0.0));
    let stickout = tool_def.stickout.max(0.0);
    let youngs = tool_def.tool_material.youngs_modulus_n_per_mm2();
    // Feed-aware deflection cap: the optimizer solves its feed cap
    // from the SAME affine force model (Ks/F_edge) and integrated
    // beam compliance the post-sim deflection gate uses, so the two
    // agree on a cut. Compliance is δ-per-newton at the toolpath's
    // peak axial DOC; deflection is linear in force so one scalar
    // suffices.
    let deflection_inputs = match crate::feeds::force::affine_coefficients(material) {
        Some((ks, f_edge)) if stickout > 0.0 && youngs > 0.0 => {
            let compliance = tool_def.tip_deflection_mm(1.0, max_axial.max(0.0), youngs);
            if compliance.is_finite() && compliance > 0.0 {
                Some(DeflectionLimitInputs {
                    ks_n_per_mm2: ks,
                    f_edge_n_per_mm: f_edge,
                    compliance_mm_per_n: compliance,
                    max_tip_deflection_mm: crate::tool_load::deflection::EXCEEDS_BOUND_MM,
                })
            } else {
                None
            }
        }
        _ => None,
    };
    let machine_profile = context.machine;
    let available_kw =
        machine_profile.power_at_rpm(spindle_rpm as f64) * machine_profile.safety_factor;
    let power_inputs = match kc_opt {
        Some(kc) if available_kw > 0.0 => Some(PowerLimitInputs {
            // S2-9 (2026-05-31): pass raw Kc; the solver applies
            // GRAIN_ANISOTROPY_FACTOR internally so this site
            // doesn't re-encode the multiplier literal.
            kc_n_per_mm2: kc,
            engagement_diameter_mm: engagement_dia,
            available_kw,
        }),
        _ => None,
    };

    let ctx = ModulationContext {
        spindle_rpm: spindle_rpm as f64,
        flute_count,
        max_feed_mm_min: max_feed,
        rapid_feed_mm_min: rapid_feed,
        chipload_band: band,
        kinematics: &kinematics,
        strategy,
        aggressiveness,
        deflection_inputs,
        power_inputs,
        nominal_axial_doc_mm: nominal_axial,
        // Phase 3 (2026-09-07) — the operation's OWN plunge rate, the
        // ceiling the geometric guard applies to a vertical-dominant
        // move whose generator emitted it without a plunge tag.
        plunge_rate_mm_min: operation.plunge_rate(),
    };

    let mut modulated_toolpath = annotated.toolpath.clone();
    let outcome = adaptive_feed_modulate(&mut modulated_toolpath, &engagements, &ctx).ok()?;
    Some((modulated_toolpath, outcome))
}

/// The two session records a [`SimulationRequest`] assembly reads.
///
/// A struct rather than two positional arguments: both are borrowed
/// records and a positional signature is how two adjacent references get
/// swapped.
struct SimRequestContext<'a> {
    machine: &'a crate::machine::MachineProfile,
    post: &'a super::ProjectPostConfig,
}

/// Build one [`SimulationRequest`] from an already-assembled `groups` +
/// `resolution` pair.
///
/// Both callers — [`ProjectSession::run_simulation`] and
/// `simulate_candidate_isolated` — go through the identical stock-frame /
/// rapid-feed-ternary / kinematics-map shape (S.12 dedup); only these knobs
/// differ:
///
/// - `metric_options`: `run_simulation` mirrors
///   `SimulationOptions::metrics_enabled` into both fields (a single toggle
///   the production path exposes). `simulate_candidate_isolated`
///   force-enables both unconditionally — the strategy advisor's modulator
///   needs per-move engagement on every candidate regardless of the
///   session's default sim options.
/// - `model_mesh`: `run_simulation` supplies the translated model mesh so
///   the simulator can compute sim-vs-model deviation;
///   `simulate_candidate_isolated` passes `None` — a throwaway candidate
///   path is scored on engagement and feed, not surface deviation.
/// - `use_predicted_feed_in_gates`: `run_simulation` mirrors
///   `SimulationOptions::use_predicted_feed_in_gates`; the isolated path
///   force-disables it, since it evaluates candidates *before* any
///   feed-modulation pass exists to populate a predicted-feed map.
fn build_sim_request(
    context: &SimRequestContext<'_>,
    groups: Vec<SimGroupEntry>,
    stock_bbox: BoundingBox3,
    resolution: f64,
    metric_options: SimulationMetricOptions,
    model_mesh: Option<Arc<TriangleMesh>>,
    use_predicted_feed_in_gates: bool,
) -> SimulationRequest {
    let machine = context.machine;
    let post = context.post;
    SimulationRequest {
        groups,
        stock_bbox,
        stock_top_z: stock_bbox.max.z,
        resolution,
        metric_options,
        spindle_rpm: post.spindle_speed,
        rapid_feed_mm_min: if post.high_feedrate_mode {
            post.high_feedrate
        } else {
            machine.max_feed_mm_min.max(1.0)
        },
        model_mesh,
        kinematics: machine
            .kinematics
            .map(|kin| crate::compute::simulate::KinematicsContext {
                kinematics: kin,
                max_feed_mm_min: machine.max_feed_mm_min.max(1.0),
                use_predicted_feed_in_gates,
            }),
    }
}

/// Simulate a single throwaway toolpath in isolation (one setup group, one
/// entry) and return its cut trace, with arc-engagement capture on so the
/// per-move engagement the modulator needs is present.
///
/// The strategy advisor evaluates candidate strategies that are not (yet)
/// persisted in `ProjectSession::results`. It holds no session, so every
/// read rides on the captured [`AdvisorContext`]: the toolpath's own id and
/// name, the stock frame, the setup frame and the post dials.
fn simulate_candidate_isolated(
    context: &AdvisorContext,
    annotated: Arc<crate::trace::toolpath_spans::AnnotatedToolpath>,
    tool_cfg: &ToolConfig,
    operation: &crate::compute::OperationConfig,
    cancel: &AtomicBool,
) -> Option<Arc<crate::stock::simulation_cut::SimulationCutTrace>> {
    if annotated.toolpath.moves.len() < 2 {
        return None;
    }
    let stock_bbox = context.stock_bbox;
    let setup_ctx = &context.setup_ctx;
    let direction = simulation::group_stock_cut_direction(setup_ctx.face_up);

    let entry = SimToolpathEntry {
        id: context.toolpath_id,
        name: context.toolpath_name.clone(),
        annotated,
        tool: build_cutter(tool_cfg),
        flute_count: tool_cfg.flute_count,
        tool_summary: tool_cfg.summary(),
        semantic_trace: None,
        spindle_rpm: operation.spindle_rpm(),
        metrics_not_applicable: false,
        drill_op: None,
        operation_config_hash: crate::compute::simulate::hash_operation_config(operation),
    };
    let local_stock_bbox = setup_ctx.sim_local_stock_bbox();
    let groups = vec![SimGroupEntry {
        toolpaths: vec![entry],
        direction,
        local_stock_bbox,
        // The handle OWNS its setup context and the job reads it by
        // reference, so the transform is cloned rather than moved.
        // `SetupTransformInfo` is `Clone` and not `Copy`.
        local_to_global: setup_ctx.local_to_global.clone(),
        phantom_prior_stock: None,
    }];
    let resolution = auto_resolution_for_groups(&groups, &stock_bbox);
    // Deviation (model_mesh) is not needed for engagement capture; both
    // metrics flags force-on (modulator needs arc engagement regardless
    // of session defaults); predicted-feed gates force-off (no modulation
    // pass has run yet to populate a predicted-feed map). See
    // `build_sim_request`'s doc comment for the full rationale.
    let request = build_sim_request(
        &SimRequestContext {
            machine: &context.machine,
            post: &context.post,
        },
        groups,
        stock_bbox,
        resolution,
        SimulationMetricOptions {
            enabled: true,
            capture_arc_engagement: true,
        },
        None,
        false,
    );
    run_simulation(&request, cancel).ok()?.cut_trace
}

impl ProjectSession {
    /// Start a generation of toolpath `index` — step (i) of the
    /// `generate_toolpath` job.
    ///
    /// [`ProjectSession::start`] is the public door; this is the arm
    /// behind it. The method holds `&mut self` and is meant to run on the
    /// frame loop. It does, in order:
    ///
    /// 1. drops the cached result (G-STICKYEMPTY);
    /// 2. runs the rest-machining and boundary preconditions, so a
    ///    refusal appears at SUBMIT time and not on the worker thread;
    /// 3. resolves every generation input through
    ///    [`Self::resolve_generation_inputs`], the one producer;
    /// 4. captures into a [`GenContext`] every session read the tail of
    ///    the generation makes;
    /// 5. stamps the toolpath's revision, read AFTER the drop.
    ///
    /// The handle it answers is the whole input of step (ii), so
    /// [`execute_job`] reads no session.
    ///
    /// **The drop moves no revision.** `self.results.remove` is what this
    /// method calls, the same call the monolith made; `drop_result` is
    /// the door that bumps, and a generation is not an edit of the
    /// inputs. So `revision` here is the revision the toolpath already
    /// carried, and a generation that nothing interrupts adopts cleanly.
    pub(crate) fn start_generate_toolpath(
        &mut self,
        index: usize,
        cancel: &AtomicBool,
    ) -> Result<GenerateToolpathHandle, SessionError> {
        // G-STICKYEMPTY: drop the cached result FIRST, on every path.
        //
        // [`Self::insert_result`] already states the invariant this restores
        // — `results[idx]` is either absent (stale / never generated) or a
        // result produced from the CURRENT config — but nothing enforced it
        // for a generation that did not finish. Every failure exit below
        // (the two preconditions, `resolve_generation_inputs`'s `?`, the
        // generator's own `Err`, and the empty-generation refusal) used to
        // leave the PREVIOUS parameter set's toolpath cached and readable
        // as if it were this configuration's answer.
        //
        // That is not merely untidy: `PhantomPriorStockScan` asks only "has
        // this been generated?", so a stale result makes a pending
        // `FromRemainingStock` op look generated, which withholds the
        // phantom prior-stock snapshot that is the only thing able to
        // unblock it — the op then cannot regenerate until the project is
        // reloaded, no matter what its parameters are put back to.
        //
        // The GUI worker path already did exactly this at submit time
        // (`rt.result = None` in `submit_toolpath_compute`); this makes the
        // core path agree rather than being the odd one out.
        let _ = self.results.remove(&index);

        // Rest-machining precondition, checked BEFORE any geometry work so we fail
        // fast and NEVER fall back to fresh stock: a `FromRemainingStock` op must
        // have a simulated remaining-stock snapshot. Absent it (no prior simulation,
        // or the predecessor changed since the last sim), error out — a fine rest
        // tool seeded with fresh stock clears the whole part instead of the leftover
        // (unbounded compute + wrong result; the 6mm→1mm runaway that motivated this).
        {
            let tc = self
                .toolpath_configs
                .get(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            if tc.stock_source == crate::session::StockSource::FromRemainingStock
                && self
                    .simulation
                    .as_ref()
                    .and_then(|sim| sim.prior_stocks.get(&tc.id))
                    .is_none()
            {
                return Err(SessionError::OperationFailed(format!(
                    "'{}' is set to use remaining stock (rest machining) but no simulated \
                     remaining-stock snapshot is available. Run a simulation of the preceding \
                     operations first, then regenerate — or set the stock source to Fresh if \
                     this is the first operation. (Refusing to fall back to fresh stock: a \
                     fine tool would clear the whole part instead of the leftover.)",
                    tc.name
                )));
            }
        }

        // DerivedRestRegions boundary precondition (P2.2), same shape and
        // same reasoning as the FromRemainingStock check above: checked
        // BEFORE any geometry work so we fail fast with a message naming
        // exactly what's missing, rather than silently clipping against
        // stale/absent regions (or against nothing at all).
        {
            let tc = self
                .toolpath_configs
                .get(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            if tc.boundary.enabled
                && let crate::compute::config::BoundarySource::DerivedRestRegions {
                    source_toolpath_id,
                } = &tc.boundary.source
            {
                self.resolve_derived_rest_region_polys(index, *source_toolpath_id)?;
            }
        }

        let inputs = self.resolve_generation_inputs(index, cancel)?;

        // Re-borrow the config for the recorder labels and for every dial
        // the generation tail reads. The resolved bundle owns everything
        // else; this borrow touches only `self.toolpath_configs`.
        //
        // The five operation dials below come from the RAW `tc.operation`,
        // never from `inputs.operation`, which the resolver patches at
        // compute time. The monolith read them this way, and reading them
        // off the patched copy would change the answer.
        let tc = self
            .toolpath_configs
            .get(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;

        let prior_stock = self
            .simulation
            .as_ref()
            .and_then(|sim| sim.prior_stocks.get(&tc.id).cloned());

        // P1 W4a: the pencil family's emit-time surface-link-vs-retract
        // decision costs candidates against the real machine envelope —
        // same accessor pattern `apply_adaptive_feed_modulation` uses
        // (`effective_kinematics` never `None`; `cutting_feed_ceiling_mm_min`
        // for the cutting-feed cap, `max_feed_mm_min` for the travel rate).
        let link_kinematics = Some(crate::machine::kinematics::LinkKinematics {
            kinematics: self.machine.effective_kinematics(),
            max_feed_mm_min: self.machine.cutting_feed_ceiling_mm_min().max(1.0),
            rapid_feed_mm_min: self.machine.max_feed_mm_min.max(1.0),
        });

        // The POST-generation clip's own region set, for the two sources
        // that resolve to a set of disjoint regions. The monolith resolved
        // it after the generator ran; the handle carries it instead, so
        // the executor holds no session.
        //
        // Re-resolved, never carried over from the pre-clip: the tier map
        // is memoised on (mesh identity, ladder, params), so this is a
        // cache hit on the exact map that produced the pre-decompose
        // boundary. Agreement between the two clips is therefore
        // structural — the P2.3 rule that made `DerivedRestRegions` safe.
        // It is also the RAW set. `inputs.pre_boundary_regions` is the
        // PROCESSED set, and the clip processes the raw set itself.
        let post_clip_regions: Option<Arc<Vec<crate::polygon::Polygon2>>> =
            if inputs.boundary_config.enabled {
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
                        // The tier map reads a BUILT index; force the
                        // lazy one here, as the resolver's own tier arm
                        // does.
                        let tier_index = inputs
                            .spatial_index
                            .as_ref()
                            .map(|lazy| Arc::clone(lazy.force()));
                        let regions = self.resolve_planned_tier_region_polys(
                            &tc.name,
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
                        Some(Arc::new(regions))
                    }
                    crate::compute::config::BoundarySource::DerivedRestRegions {
                        source_toolpath_id,
                    } => {
                        // The precondition above already validated this —
                        // it can only fail here if the source toolpath's
                        // result was invalidated in between, which cannot
                        // happen under `&mut self`. Propagate defensively
                        // rather than `#[allow(clippy::unwrap_used)]`.
                        Some(self.resolve_derived_rest_region_polys(index, *source_toolpath_id)?)
                    }
                    _ => None,
                }
            } else {
                None
            };

        let context = GenContext {
            toolpath_name: tc.name.clone(),
            op_label: tc.operation.label(),
            stock_source: tc.stock_source,
            dressups: tc.dressups.clone(),
            rest_analysis: tc.rest_analysis.clone(),
            debug_options: tc.debug_options,
            entry_probe_leave: tc.operation.entry_probe_leave(),
            feed_rate: tc.operation.feed_rate(),
            plunge_rate: tc.operation.plunge_rate(),
            transform_capabilities: tc.operation.transform_capabilities(),
            prior_stock,
            link_kinematics,
            material: self.stock.material.clone(),
            post_clip_regions,
        };

        Ok(GenerateToolpathHandle {
            index,
            revision: self.toolpath_revision(index),
            inputs,
            context,
        })
    }

    /// Generate a single toolpath by index.
    ///
    /// The method runs the three steps of the `generate_toolpath` job
    /// INLINE: [`Self::start`], then [`execute_job`], then
    /// `apply(Command::AdoptResult { .. })`. The monolith this replaces
    /// is split, not wrapped — the same three functions the GUI worker
    /// lane runs, in the same order.
    ///
    /// It holds `&mut self` across all three, so nothing can move the
    /// revision between the capture and the adopt: the adopt on this
    /// door cannot report a stale completion.
    ///
    /// The CLI takes this door for every generation
    /// (`crates/rs_cam_cli/src/{run,job,project,smoke}.rs`), so the CLI
    /// runs the three steps too.
    #[instrument(skip(self, cancel))]
    pub fn generate_toolpath(
        &mut self,
        index: usize,
        cancel: &AtomicBool,
    ) -> Result<&ToolpathComputeResult, SessionError> {
        // `start` answers the handle of the row it was handed, so the
        // other arm cannot happen. It is spelled out rather than wildcarded
        // because that is what makes a new `Job` row stop this door from
        // compiling until someone reads it.
        let JobHandle::GenerateToolpath(handle) = self.start(
            Job::GenerateToolpath(GenerateToolpathArgs { index }),
            cancel,
        )?
        else {
            return Err(SessionError::OperationFailed(
                "the generate_toolpath job answered another row's handle".to_owned(),
            ));
        };
        // The core door reports to nothing and records no per-dressup
        // item, which is what it did before the observer existed.
        let result = execute_job(&handle, &GenObserver::none(), cancel)?;
        let _ = self.apply(Command::AdoptResult(AdoptResultArgs {
            index: handle.index,
            revision: handle.revision,
            result: Box::new(result),
        }))?;
        // SAFETY: the adopt above inserted at this key. It refuses and
        // returns early when the index names no toolpath or the revision
        // moved, and neither can happen under one `&mut self`.
        #[allow(clippy::indexing_slicing)]
        Ok(&self.results[&index])
    }

    /// Generate all enabled toolpaths, skipping those whose IDs are in `skip`.
    #[instrument(skip(self, skip_ids, cancel))]
    pub fn generate_all(
        &mut self,
        skip_ids: &[ToolpathId],
        cancel: &AtomicBool,
    ) -> Result<(), SessionError> {
        // Collect info needed for skip/logging before mutable borrow
        let tp_info: Vec<(usize, ToolpathId, String, bool)> = self
            .toolpath_configs
            .iter()
            .enumerate()
            .map(|(idx, tc)| (idx, tc.id, tc.name.clone(), tc.enabled))
            .collect();

        for (idx, tp_id, tp_name, enabled) in &tp_info {
            if !enabled {
                continue;
            }
            if skip_ids.contains(tp_id) {
                tracing::info!(id = tp_id.0, name = %tp_name, "Skipping toolpath (skip list)");
                continue;
            }
            match self.generate_toolpath(*idx, cancel) {
                Ok(_) => {}
                Err(SessionError::MissingGeometry(msg)) => {
                    tracing::warn!(id = tp_id.0, name = %tp_name, reason = %msg, "Skipping toolpath");
                }
                Err(e) => {
                    tracing::error!(id = tp_id.0, name = %tp_name, error = %e, "Toolpath failed");
                }
            }
        }
        Ok(())
    }

    // ── Analysis ───────────────────────────────────────────────────
}

/// Compute auto-resolution from simulation groups and stock bbox.
///
/// Mirrors the GUI's `auto_resolution_for_tools` heuristic:
/// - 5 cells across the smallest tool radius for decent curve resolution
/// - Clamped to [0.02, 0.5] mm
/// - Further limited so the grid stays under ~8M cells
fn auto_resolution_for_groups(groups: &[SimGroupEntry], stock_bbox: &BoundingBox3) -> f64 {
    use crate::tool::MillingCutter as _;

    let min_radius = groups
        .iter()
        .flat_map(|g| g.toolpaths.iter())
        .map(|entry| entry.tool.radius())
        .fold(f64::INFINITY, f64::min);

    // 5 cells across the radius gives decent curve resolution
    let from_tool = (min_radius / 5.0).clamp(0.02, 0.5);

    // Cap so grid stays under ~8M cells (reasonable memory / mesh size)
    let max_cells: f64 = 8_000_000.0;
    let sx = stock_bbox.max.x - stock_bbox.min.x;
    let sy = stock_bbox.max.y - stock_bbox.min.y;
    let from_grid = ((sx * sy) / max_cells).sqrt().max(0.02);

    from_tool.max(from_grid)
}

#[cfg(test)]
mod tests;
