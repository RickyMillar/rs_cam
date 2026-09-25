//! The dressup pipeline that runs after an operation generates.
//!
//! [`apply_dressups`] drives entry moves, leads, arc fitting, conditioning and
//! feed optimisation; the generic rest analysis attaches its findings on the
//! same pass. Split out of `compute/execute.rs` (P4).

use crate::compute::catalog::OperationTransformCapabilities;
use crate::compute::config::{DressupConfig, DressupEntryStyle};
use crate::compute::cutter::build_cutter;
use crate::compute::tool_config::ToolConfig;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::tool::{MillingCutter, ToolDefinition};
use crate::trace::debug_trace::ToolpathDebugContext;
use crate::trace::semantic_trace::{
    SemanticKey, ToolpathSemanticContext, ToolpathSemanticKind, ToolpathSemanticScope,
};
use crate::trace::toolpath_spans::AnnotatedToolpath;
use crate::trace::transform_provenance::{ReconcileSet, Transformed};

use super::findings::{record_derived_stepover, record_region_cap};
use super::{GeneratedToolpath, GenerationFindings};

/// P2.5: shared rest-analysis attach for any operation family. Runs the same
/// rest-depth detector `pencil::rest_depth_arm` uses
/// (`rest_field::detect_rest_valleys`), with THIS toolpath's own tool as the
/// fine cutter, and attaches `rest_grid` / `rest_regions` to `generated` —
/// no centerline toolpath is emitted, only the analysis artifacts. Reference
/// resolution order mirrors `rest_depth_arm`: prefer the actual machined
/// stock (when its XY frame overlaps the mesh), else the configured real
/// reference tool (`reference_tool_cfg`, resolved upstream from
/// `RestAnalysisConfig::reference_tool_id` the same way pencil's own
/// `reference_tool_id` is resolved), else a self-referenced bare-surface
/// probe.
#[allow(clippy::too_many_arguments)] // post-generation attach point; every
// argument is a distinct upstream source (geometry, tool, reference, stock,
// dials, findings sink)
pub(super) fn attach_generic_rest_analysis(
    generated: &mut GeneratedToolpath,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    tool_def: &ToolDefinition,
    reference_tool_cfg: Option<&ToolConfig>,
    initial_stock: Option<&crate::dexel_stock::TriDexelStock>,
    cfg: &crate::compute::config::RestAnalysisConfig,
    findings: &std::cell::RefCell<GenerationFindings>,
) {
    let reference_tool = reference_tool_cfg.map(build_cutter);
    let probe_ball = crate::tool::BallEndmill::new(
        crate::finish::pencil::SURFACE_PROBE_BALL_DIAMETER_MM,
        crate::finish::pencil::SURFACE_PROBE_BALL_LENGTH_MM,
    );
    let reference = resolve_rest_reference(
        mesh,
        reference_tool.as_ref().map(|t| t as &dyn MillingCutter),
        initial_stock,
        &probe_ball,
    );
    let defaults = crate::surface::rest_field::RestFieldParams::default();
    // PR-7 (H2.5): the fan this pass ROUTES against is now a real one.
    //
    // Wave A left this taking `RestFieldParams::default()` — a literal
    // 0.5 mm stepover and a 0-pass cap — with the note that it is "fine for
    // a report-only pass, wrong the moment it emits paths". It was already
    // wrong before that: the regions this pass attaches are consumed as a
    // `derived_rest_regions` BOUNDARY by other operations, so a routing
    // verdict taken against a fan nobody would emit decides where a real
    // toolpath is allowed to cut. On the shipped Ø1-tip taper the literal
    // 0.5 quantises the coverage threshold `ceil(own_width/stepover) ×
    // stepover` a full third coarser than the policy value does.
    //
    // `None` on either dial = ask the policy / take the detector default;
    // no parallel formula lives here.
    let offset_stepover_mm = cfg.offset_stepover_mm.unwrap_or_else(|| {
        crate::surface::reach::suggested_offset_stepover_mm(tool_def, cfg.min_valley_depth)
    });
    let rf_params = crate::surface::rest_field::RestFieldParams {
        cell_mm: cfg.cell_mm,
        min_valley_depth: cfg.min_valley_depth,
        region_margin_mm: cfg.region_margin_mm,
        offset_stepover_mm,
        num_offset_passes_cap: cfg
            .num_offset_passes
            .unwrap_or(defaults.num_offset_passes_cap),
        min_cut_length: defaults.min_cut_length,
    };
    // Same audit trail as the `UnifiedFinish` claims pipeline (PR-6a): the
    // number steers a routing decision and appears in no dial the operator
    // set. Only when the policy sized it — an explicitly pinned stepover is
    // the operator's own number and needs no notice.
    if cfg.offset_stepover_mm.is_none() {
        record_derived_stepover(
            findings,
            crate::compute::toolpath_stats::DerivedStepoverFinding {
                site: "generic rest analysis routing",
                stepover_mm: offset_stepover_mm,
                reference_depth_mm: cfg.min_valley_depth,
                reference_depth_basis: GENERIC_REST_STEPOVER_DEPTH_BASIS,
                envelope_rule_mm: tool_def.envelope_radius_mm() * 0.5,
                slope_derate: None,
            },
        );
    }
    let rf = crate::surface::rest_field::detect_rest_valleys(
        mesh, index, tool_def, reference, &rf_params,
    );
    // F3: the regions these artifacts carry are what a `DerivedRestRegions`
    // consumer will be confined to, and the MAX_REST_REGIONS cap can have
    // silently dropped some of them. Record the pre-cap count alongside —
    // always, so `Some` with nothing truncated reads as measured-clean.
    record_region_cap(findings, rf.region_cap);
    // The region set is the tier-map / derived-boundary planning surface,
    // and no headless output carries its polygons — this line is the one
    // place the numbers are observable off the GUI (multitool Phase T).
    tracing::info!(
        regions = rf.region_polygons.len(),
        pre_cap = rf.region_cap.total_before_cap,
        total_area_mm2 = format!(
            "{:.1}",
            rf.region_polygons
                .iter()
                .map(|p| p.area().abs())
                .sum::<f64>()
        ),
        cell_mm = rf_params.cell_mm,
        min_valley_depth = rf_params.min_valley_depth,
        "generic rest analysis attached region set"
    );
    generated.rest_grid = Some(std::sync::Arc::new(rf.rest_grid));
    generated.rest_regions = Some(std::sync::Arc::new(rf.region_polygons));
}

/// Why [`attach_generic_rest_analysis`] sizes the reach policy at
/// `min_valley_depth`. Shipped in the operator-facing diagnostic.
const GENERIC_REST_STEPOVER_DEPTH_BASIS: &str = "the configured min_valley_depth — the shallowest rest this pass will \
     report, where the cutter's engaged width is narrowest";

/// Not a depth basis: the shallow-raster derate is slope-keyed, not
/// depth-keyed, and its `reference_depth_mm` is a fixed 0.0.
pub(super) const SHALLOW_SLOPE_DERATE_BASIS: &str =
    "slope-keyed — cos(theta_max) of the region's covered cells, not a depth";

/// Resolve the rest-depth reference (P2.5 chain): the actual machined
/// stock (when present and its XY bbox overlaps `mesh`) → a configured
/// real reference tool → a self-referenced bare-surface probe. Shared by
/// the generic post-generation pass above (`attach_generic_rest_analysis`)
/// and `generate_unified_finish`'s in-op claims pipeline (v3 S1) — same
/// chain, same priority order, one implementation.
fn resolve_rest_reference<'a>(
    mesh: &TriangleMesh,
    reference_tool: Option<&'a dyn MillingCutter>,
    initial_stock: Option<&'a crate::dexel_stock::TriDexelStock>,
    probe_ball: &'a crate::tool::BallEndmill,
) -> crate::surface::rest_field::RestReference<'a> {
    let stock_ref = initial_stock.filter(|stock| {
        let (sb, mb) = (&stock.stock_bbox, &mesh.bbox);
        sb.min.x <= mb.max.x && sb.max.x >= mb.min.x && sb.min.y <= mb.max.y && sb.max.y >= mb.min.y
    });
    if let Some(stock) = stock_ref {
        crate::surface::rest_field::RestReference::Stock(stock)
    } else if let Some(tool) = reference_tool {
        crate::surface::rest_field::RestReference::Cutter {
            tool,
            is_surface_probe: false,
        }
    } else {
        crate::surface::rest_field::RestReference::Cutter {
            tool: probe_ball,
            is_surface_probe: true,
        }
    }
}

// ── Dressup tracing helper (Phase 4 / #44) ────────────────────────────

/// One stage of the dressup pipeline: the four identifiers it traces under.
///
/// CMP-20 / CUT-06: each stage used to write these four strings as a literal
/// at its own call site, and they existed nowhere else. Nothing could
/// enumerate the pipeline. They are named constants now, and
/// [`DRESSUP_PIPELINE`] lists them in run order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DressupStage {
    pub debug_key: &'static str,
    pub debug_label: &'static str,
    pub kind: ToolpathSemanticKind,
    pub semantic_label: &'static str,
}

impl DressupStage {
    pub const RAPID_ORDER: Self = Self {
        debug_key: "rapid_order",
        debug_label: "Optimize rapid order",
        kind: ToolpathSemanticKind::Optimization,
        semantic_label: "Rapid ordering",
    };
    pub const RAMP_ENTRY: Self = Self {
        debug_key: "entry_style",
        debug_label: "Ramp entry",
        kind: ToolpathSemanticKind::Entry,
        semantic_label: "Ramp entry",
    };
    pub const HELIX_ENTRY: Self = Self {
        debug_key: "entry_style",
        debug_label: "Helix entry",
        kind: ToolpathSemanticKind::Entry,
        semantic_label: "Helix entry",
    };
    pub const DOGBONES: Self = Self {
        debug_key: "dogbones",
        debug_label: "Apply dogbones",
        kind: ToolpathSemanticKind::Dressup,
        semantic_label: "Dogbones",
    };
    pub const LEAD_IN_OUT: Self = Self {
        debug_key: "lead_in_out",
        debug_label: "Apply lead in/out",
        kind: ToolpathSemanticKind::Dressup,
        semantic_label: "Lead in/out",
    };
    pub const LINK_MOVES: Self = Self {
        debug_key: "link_moves",
        debug_label: "Apply link moves",
        kind: ToolpathSemanticKind::Dressup,
        semantic_label: "Link moves",
    };
    pub const ARC_FIT: Self = Self {
        debug_key: "arc_fit",
        debug_label: "Fit arcs",
        kind: ToolpathSemanticKind::Optimization,
        semantic_label: "Arc fitting",
    };
    pub const SEGMENT_MERGE: Self = Self {
        debug_key: "segment_merge",
        debug_label: "Merge short segments",
        kind: ToolpathSemanticKind::Optimization,
        semantic_label: "Segment merge",
    };
    pub const AIR_CUT_FILTER: Self = Self {
        debug_key: "air_cut_filter",
        debug_label: "Filter air cuts",
        kind: ToolpathSemanticKind::Optimization,
        semantic_label: "Air-cut filter",
    };
    pub const FEED_OPTIMIZATION: Self = Self {
        debug_key: "feed_optimization",
        debug_label: "Optimize feeds",
        kind: ToolpathSemanticKind::Optimization,
        semantic_label: "Feed optimization",
    };
}

/// The dressup pipeline, in the order [`apply_dressups`] runs it.
///
/// ORDER IS LOAD-BEARING and this list states it: segment merge runs AFTER
/// arc fitting (curves are already G2/G3 by then, so the merge only cleans
/// up residual linears), and the second rapid-order pass runs AFTER link
/// moves. `RAPID_ORDER` appears twice for that reason — the barriered arm
/// and the unbarriered fallback are the same stage under two gates, and
/// exactly one of them fires. `RAMP_ENTRY` stands for the entry slot; a
/// helix entry runs [`DressupStage::HELIX_ENTRY`] in the same position.
///
/// Every stage is GATED. A run emits a subsequence of this list, never a
/// different order and never a key that is not here.
/// `tests/dressup_span_invariants.rs` is the guard.
pub const DRESSUP_PIPELINE: &[DressupStage] = &[
    DressupStage::RAPID_ORDER,
    DressupStage::RAMP_ENTRY,
    DressupStage::DOGBONES,
    DressupStage::LEAD_IN_OUT,
    DressupStage::LINK_MOVES,
    DressupStage::ARC_FIT,
    DressupStage::SEGMENT_MERGE,
    DressupStage::RAPID_ORDER,
    DressupStage::AIR_CUT_FILTER,
    DressupStage::FEED_OPTIMIZATION,
];

/// Run one dressup step with optional debug + semantic tracing scopes.
///
/// The `transform` closure must return a [`Transformed`] — that signature
/// IS the C1 contract at this boundary. A new dressup step cannot be added
/// to the pipeline without reporting how it moved the move indices, and
/// this helper is the only thing that can turn that report back into a
/// usable toolpath, which it does by reconciling `channels`.
///
/// `channels` is independent of `semantic_ctx`: it records nothing, it
/// carries the move links of items recorded EARLIER (at generation time, or
/// by a previous dressup step) through this step's move-index changes. The
/// session path registers its generation recorder there while passing
/// `None` for `semantic_ctx` — it wants the links kept honest without
/// adding per-dressup items to the trace.
fn apply_dressup_traced(
    annotated: AnnotatedToolpath,
    debug_ctx: Option<&ToolpathDebugContext>,
    semantic_ctx: Option<&ToolpathSemanticContext>,
    channels: &mut ReconcileSet<'_>,
    info: DressupStage,
    set_params: impl FnOnce(&ToolpathSemanticScope),
    transform: impl FnOnce(AnnotatedToolpath) -> Transformed,
) -> AnnotatedToolpath {
    let debug_scope = debug_ctx.map(|ctx| ctx.start_span(info.debug_key, info.debug_label));
    let debug_span_id = debug_scope.as_ref().map(|s| s.id());
    // Started BEFORE the transform (so its params are recorded even if the
    // transform is the last thing this scope sees) but bound to a move
    // range only AFTER the reconcile — an item with no link yet carries no
    // move indices, so it is never double-remapped.
    let semantic_scope = semantic_ctx.map(|ctx| {
        let scope = ctx.start_item(info.kind, info.semantic_label);
        if let Some(span_id) = debug_span_id {
            scope.set_debug_span_id(span_id);
        }
        set_params(&scope);
        scope
    });

    let (result, provenance) = transform(annotated).reconcile(channels).into_parts();
    let n = result.toolpath.moves.len();

    if let Some(scope) = semantic_scope.as_ref() {
        // C1 item 4a: every per-dressup item used to bind `0..len`, so each
        // one claimed the whole toolpath and per-step attribution said
        // nothing. The provenance knows which moves the step actually
        // restructured; where it does not (a step that rewrote move CONTENT
        // without moving an index, i.e. feed optimisation), the whole-path
        // claim is still made but is now LABELLED as such instead of being
        // indistinguishable from a precise one.
        match provenance.touched_new_range(n) {
            Some(range) => {
                scope.set_param(SemanticKey::MoveScope, "touched_moves");
                scope.bind_to_toolpath(&result.toolpath, range.start, range.end);
            }
            None => {
                scope.set_param(SemanticKey::MoveScope, "whole_path");
                scope.bind_to_toolpath(&result.toolpath, 0, n);
            }
        }
    }
    if let Some(scope) = debug_scope.as_ref()
        && n > 0
    {
        scope.set_move_range(0, n - 1);
    }
    result
}

/// Apply the standard dressup pipeline to a toolpath.
///
/// Steps: entry style → dogbones → lead in/out → link moves → arc fitting
/// → rapid order optimization → air-cut filter → feed rate optimization.
/// Move-order/link transforms are gated by operation capabilities.
///
/// TSP barriers are derived from the input [`AnnotatedToolpath`]'s spans
/// (`RapidOrderBarrier` + `DepthPass` span starts) — see
/// [`AnnotatedToolpath::rapid_order_barriers`].
///
/// `prior_stock` + `cutter` enable the air-cut filter step (S3: the filter
/// judges air for the whole cutter, so it refuses to run without one — see
/// [`crate::dressup::filter_air_cuts`]). `feed_opt_stock` + `cutter`
/// enable feed optimization. Both `debug_ctx` and `semantic_ctx` are optional
/// per-step tracing scopes — the GUI passes them through to populate the
/// sim-tree view; CLI / session callers pass `None` and pay zero overhead.
///
/// All dressups in this pipeline are span-aware (Phase 3 sub-tasks
/// #50–#58); spans on the input are remapped through each step.
///
/// `channels` (C1) is the registry of index-carrying channels the CALLER
/// owns — today the semantic trace's move links. Every step reconciles
/// against it, so the item ranges cannot drift away from the moves they
/// name (and cannot end up past the end of the move list, which is a
/// consumer-panic class). It is orthogonal to `semantic_ctx`, which only
/// controls whether each step records an item of its own; a caller with no
/// channels passes [`ReconcileSet::empty`] and says so.
/// The height below which THIS operation's rapids are its own internal
/// linking rather than group framing — the per-op value C1 threads into
/// [`crate::dressup::tsp::optimize_rapid_order`], where it stops
/// the reorder from taking a canned cycle apart.
///
/// Returns `None` for every family whose rapids already live at `safe_z`,
/// which keeps their emitted motion byte-identical: with `None` the
/// reorder splits at every rapid exactly as it always has.
///
/// # Why the drills, and why detected this way
///
/// Drill is the one shipped family that deliberately puts rapids *below*
/// `safe_z`: `generate_drill` sets `retract_z` to
/// `effective_safe_z(cfg.retract_z, stock_top)` — the R-plane, at least
/// `stock_top + SAFE_Z_CLEARANCE_MM` — and `drill::fed_descents` roots the
/// whole peck schedule there (Fanuc G83). Every one of those rapids was
/// discarded by the reorder and re-planted at `safe_z`, which deleted the
/// R-plane approach and turned each peck re-entry into a `G1` feed from
/// full safe-Z. Nothing reported it: `DrillToolpathSummary::feed_time_s`
/// is computed from the config, not the moves. See
/// `planning/perf_review_2026-08-19/RESEARCH_drill_intent_erasure.md`.
///
/// The predicate is the presence of a `MoveIntent::Drilling` move, which
/// is the same structural test the session's drill entry-strip uses, and
/// it is read off the toolpath **as the reorder will see it** — after the
/// dressups that run ahead of the unbarriered arm. An op config is not
/// available here (`apply_dressups` is called from three crates and takes
/// no `OperationConfig`), and inventing one would have meant a signature
/// change in files this lane may not touch; more to the point, the
/// question really is about the motion, not about the label on it.
///
/// The value returned is `safe_z` itself: "anything below the plane the
/// group is framed at is the operation's own business". It is a ceiling
/// on internal linking, never a height anything is emitted at.
fn internal_link_ceiling_z(annotated: &AnnotatedToolpath, safe_z: f64) -> Option<f64> {
    toolpath_is_drill_cycle(annotated).then_some(safe_z)
}

/// Does this toolpath's emitted motion make it a drill cycle?
///
/// ONE construction site for the predicate two independent decisions in
/// this pipeline now depend on — the entry-dressup strip
/// (G-WANAKA-DRILL-RAMP, whose full rationale is at its call site below)
/// and the C1 internal-link ceiling above. They were written days apart
/// and would otherwise be two identical `any(...)` scans free to drift;
/// the same predicate also lives in `session::compute`.
///
/// It reads emitted motion rather than the op type on purpose: both
/// decisions are about what the moves DO. Only `drill.rs` tags
/// `MoveIntent::Drilling`, and it tags every fed descent of all four
/// cycles, so no drill-family plunge escapes and no milling op is caught.
fn toolpath_is_drill_cycle(annotated: &AnnotatedToolpath) -> bool {
    annotated
        .toolpath
        .moves
        .iter()
        .any(|m| matches!(m.intent, crate::toolpath::MoveIntent::Drilling))
}

/// Everything one generation's dressup pipeline needs besides the toolpath
/// and the reconcile channels.
///
/// CMP-20 / CUT-11: `apply_dressups` took 15 positional arguments and
/// carried its own `#[allow(clippy::too_many_arguments)]`. Nine of them are
/// per-generation constants, so a new dressup that needed one more input
/// widened a 15-argument signature in three crates. They travel together
/// now.
///
/// `channels` stays OUTSIDE this struct. It is the one `&mut` the pipeline
/// reconciles against at every step, and folding it in would hold the whole
/// context mutably borrowed while each stage reads its constants.
pub struct DressupContext<'a> {
    pub cfg: &'a DressupConfig,
    pub nominal_feed_rate: f64,
    /// WP22 (G-FEEDOPTPLUNGE): the OPERATION's own plunge rate (mm/min).
    /// The feed-optimisation pass caps a geometric plunge at it. `None`
    /// names a caller with no operation in scope, and the cap does not
    /// apply. This is NOT the `plunge_rate` local inside the pipeline,
    /// which is a heuristic for the entry and link dressups.
    pub plunge_rate_mm_min: Option<f64>,
    /// The operation's helix and ramp entry feed through material
    /// (mm/min). `None` keeps the entry dressup's own feed (the plunge
    /// feed it derives), which was the behaviour before the field.
    pub ramp_feed_rate_mm_min: Option<f64>,
    pub tool_diameter: f64,
    pub safe_z: f64,
    pub stock_top: f64,
    pub prior_stock: Option<&'a crate::dexel_stock::TriDexelStock>,
    pub feed_opt_stock: Option<&'a mut crate::dexel_stock::TriDexelStock>,
    pub cutter: Option<&'a dyn MillingCutter>,
    /// G-RAMPTERRAIN: drop-cutter surface probe for stock-aware entry
    /// moves. `None` only for operations with no mesh surface.
    pub entry_surface: Option<crate::dressup::EntrySurfaceProbe<'a>>,
    pub transform_capabilities: OperationTransformCapabilities,
    pub debug_ctx: Option<&'a ToolpathDebugContext>,
    pub semantic_ctx: Option<&'a ToolpathSemanticContext>,
}

pub fn apply_dressups(
    annotated: AnnotatedToolpath,
    ctx: DressupContext<'_>,
    channels: &mut ReconcileSet<'_>,
) -> AnnotatedToolpath {
    use crate::dressup::{
        EntryStyle, LinkMoveParams, apply_dogbones, apply_entry, apply_link_moves,
    };

    let DressupContext {
        cfg,
        nominal_feed_rate,
        plunge_rate_mm_min,
        ramp_feed_rate_mm_min,
        tool_diameter,
        safe_z,
        stock_top,
        prior_stock,
        feed_opt_stock,
        cutter,
        entry_surface,
        transform_capabilities,
        debug_ctx,
        semantic_ctx,
    } = ctx;

    // Capability gate: barriered TSP only fires when the input has barriers.
    let rapid_order_barriers = annotated.rapid_order_barriers();
    let input_valid = annotated.spans_valid;
    let mut current = annotated;

    let tool_radius = tool_diameter / 2.0;

    if cfg.optimize_rapid_order
        && !rapid_order_barriers.is_empty()
        && transform_capabilities.allows_barriered_rapid_reorder()
    {
        let barrier_count = rapid_order_barriers.len();
        let link_ceiling = internal_link_ceiling_z(&current, safe_z);
        current = apply_dressup_traced(
            current,
            debug_ctx,
            semantic_ctx,
            channels,
            DressupStage::RAPID_ORDER,
            |scope| {
                scope.set_param(SemanticKey::SafeZ, safe_z);
                scope.set_param(SemanticKey::BarrierCount, barrier_count);
            },
            |at| crate::dressup::tsp::optimize_rapid_order(at, safe_z, link_ceiling),
        );
    }

    let plunge_rate = current
        .toolpath
        .moves
        .iter()
        .find_map(|m| match m.move_type {
            crate::toolpath::MoveType::Linear { feed_rate } => Some(feed_rate * 0.5),
            _ => None,
        })
        .unwrap_or(500.0);

    // 1. Entry style
    //
    // G-WANAKA-DRILL-RAMP (2026-08-19, severity high): auto-applied
    // dressups gave both of wanaka200's drill cycles `entry_style = ramp`,
    // and `apply_entry` duly rewrote every peck descent as a ramp — the
    // emitted pin-drill motion moved 19 mm laterally on its way down
    // (`G1 X15.708 Y16.271 Z28.000` then `G1 X2.500 Y2.500 Z27.000` for a
    // pin at X2.5 Y2.5). A ramped alignment-pin hole is an oval slot,
    // which destroys the flip registration that is the op's entire
    // purpose. Nothing flagged it; it was found by reading the G-code.
    //
    // The premise `apply_entry` is built on is that a straight plunge is a
    // bad way to ENTER a cut. For a drill cycle the vertical descent IS
    // the cut — end-cutting, which is exactly what a drill bit is for — so
    // there is no ramped form of it to prefer. The dressup is dropped
    // here, at the one place it is applied, rather than corrected in the
    // config: `DressupConfig::normalize_for_op` already promises this for
    // `Drill` (`FORCE_NO_ENTRY`) and the ramp reached the machine anyway,
    // because `ProjectSession::add_toolpath` takes a fully-built
    // `ToolpathConfig` and normalizes nothing. A guarantee that only holds
    // when someone remembers to route through the right setter is not a
    // guarantee.
    //
    // **Stripped, not refused.** A refusal would fire on the product's own
    // defaults — `AlignmentPinDrill` carries `DressupPolicy::ANY_DRESSUP`
    // and the Roughing role's `Ramp`, so every freshly-added pin drill
    // would stop generating — and it would buy nothing: the dressup is a
    // request to REWRITE the emitted geometry, so declining it leaves the
    // commanded cycle whole and correct. Refusal is for the case where the
    // engine cannot produce the commanded geometry at all.
    //
    // The predicate is the toolpath's own `MoveIntent::Drilling`, the same
    // one `session::compute` uses to decide a result is a drill cycle.
    // Only `drill.rs` emits it, and it tags every fed descent of all four
    // cycles, so no drill-family plunge escapes and no milling op is
    // caught. A future generator that mixed drilling and milling in one
    // toolpath would lose entry styling for the whole op — none does, and
    // `narrate.rs` already names that hypothetical as the thing to watch.
    let is_drill_cycle = toolpath_is_drill_cycle(&current);
    if is_drill_cycle && cfg.entry_style != DressupEntryStyle::None {
        tracing::warn!(
            entry_style = ?cfg.entry_style,
            "Entry dressup dropped: a drill cycle's descent is the cut, and \
             ramping it would cut an oval slot instead of a round hole"
        );
    }
    let entry_style = if is_drill_cycle {
        DressupEntryStyle::None
    } else {
        cfg.entry_style
    };
    let entry_safety = crate::dressup::EntrySafety {
        stock_top,
        surface: entry_surface,
        fold_lap_cap: transform_capabilities.ramp_fold_lap_cap,
        ramp_feed: ramp_feed_rate_mm_min,
        stock_top_measured: false,
        contact_clearance: cfg.entry_clearance_mm,
        contact_top: None,
        // Operator ruling 2026-09-24: a helix or ramp takes the full material
        // depth, so it reads the op's own replayed stock for its start.
        own_stock: cutter.map(|cutter| crate::dressup::EntryStockReplay {
            cutter,
            prior: prior_stock,
        }),
    };
    match entry_style {
        DressupEntryStyle::Ramp => {
            let ramp_angle = cfg.ramp_angle;
            current = apply_dressup_traced(
                current,
                debug_ctx,
                semantic_ctx,
                channels,
                DressupStage::RAMP_ENTRY,
                |scope| {
                    scope.set_param(SemanticKey::Kind, "ramp");
                    scope.set_param(SemanticKey::MaxAngleDeg, ramp_angle);
                },
                |at| {
                    apply_entry(
                        at,
                        EntryStyle::Ramp {
                            max_angle_deg: ramp_angle,
                        },
                        plunge_rate,
                        entry_safety,
                        tool_radius,
                    )
                },
            );
        }
        DressupEntryStyle::Helix => {
            // G10 Q6 and D2: the radius is the operator value or the rule
            // 0.3 x D, capped at the flat bottom so the helix leaves no
            // core. The trace records the radius that is emitted. With no
            // cutter (a test context) the flat bottom and the nominal D are
            // unknown, so the request (or 0.3 x the context's D) is emitted.
            let helix_radius = match cutter {
                Some(cutter) => cfg.helix_radius_for(cutter).emitted_mm,
                None => cfg
                    .helix_radius
                    .unwrap_or(crate::compute::config::HELIX_RADIUS_OVER_D * tool_diameter),
            };
            let helix_pitch = cfg.helix_pitch;
            current = apply_dressup_traced(
                current,
                debug_ctx,
                semantic_ctx,
                channels,
                DressupStage::HELIX_ENTRY,
                |scope| {
                    scope.set_param(SemanticKey::Kind, "helix");
                    scope.set_param(SemanticKey::Radius, helix_radius);
                    scope.set_param(SemanticKey::Pitch, helix_pitch);
                },
                |at| {
                    apply_entry(
                        at,
                        EntryStyle::Helix {
                            radius: helix_radius,
                            pitch: helix_pitch,
                        },
                        plunge_rate,
                        entry_safety,
                        tool_radius,
                    )
                },
            );
        }
        DressupEntryStyle::None => {}
    }

    // 2. Dogbones
    if let Some(dogbone) = cfg.dogbone {
        let angle = dogbone.angle;
        current = apply_dressup_traced(
            current,
            debug_ctx,
            semantic_ctx,
            channels,
            DressupStage::DOGBONES,
            |scope| {
                scope.set_param(SemanticKey::AngleDeg, angle);
            },
            |at| apply_dogbones(at, tool_radius, angle),
        );
    }

    // 3. Lead in/out
    if let Some(lead) = cfg.lead_in_out {
        let radius = lead.radius;
        let li_feed = lead.in_feed_rate;
        let lo_feed = lead.out_feed_rate;
        current = apply_dressup_traced(
            current,
            debug_ctx,
            semantic_ctx,
            channels,
            DressupStage::LEAD_IN_OUT,
            |scope| {
                scope.set_param(SemanticKey::Radius, radius);
                if let Some(f) = li_feed {
                    scope.set_param(SemanticKey::LeadInFeedRate, f);
                }
                if let Some(f) = lo_feed {
                    scope.set_param(SemanticKey::LeadOutFeedRate, f);
                }
            },
            |at| {
                crate::dressup::apply_lead_in_out(
                    at,
                    radius,
                    li_feed,
                    lo_feed,
                    entry_safety.surface.as_ref(),
                    // G-ISOCLIPRAPID: the lead-in's pre-position rapid
                    // travels in XY, so it goes to the operation's retract
                    // plane, never to whatever height the move before the
                    // plunge stopped at.
                    Some(safe_z),
                )
            },
        );
    }

    // 4. Link moves
    if let Some(link) = cfg
        .link_moves
        .filter(|_| transform_capabilities.allows_link_moves())
    {
        let max_dist = link.max_distance;
        let link_feed = link.feed_rate;
        current = apply_dressup_traced(
            current,
            debug_ctx,
            semantic_ctx,
            channels,
            DressupStage::LINK_MOVES,
            |scope| {
                scope.set_param(SemanticKey::MaxLinkDistance, max_dist);
                scope.set_param(SemanticKey::LinkFeedRate, link_feed);
            },
            |at| {
                apply_link_moves(
                    at,
                    &LinkMoveParams {
                        max_link_distance: max_dist,
                        link_feed_rate: link_feed,
                        safe_z_threshold: safe_z * 0.9,
                        tool_radius,
                    },
                )
            },
        );
    }

    // 5. Arc fitting
    if let Some(arc) = cfg.arc_fitting {
        let tolerance = arc.tolerance;
        current = apply_dressup_traced(
            current,
            debug_ctx,
            semantic_ctx,
            channels,
            DressupStage::ARC_FIT,
            |scope| {
                scope.set_param(SemanticKey::Tolerance, tolerance);
            },
            |at| crate::dressup::arcfit::fit_arcs(at, tolerance, tool_radius),
        );
    }

    // 5b. Segment merge (accel-friendly) — collapse dense same-feed linear cut
    // runs so a low-acceleration controller can ramp to feed. Runs after
    // arc-fitting (curves are already G2/G3; this cleans up residual linears).
    //
    // G-PLANSIMGAP (2026-09-26): an operation whose planner applies the
    // merge (the 3D Rough) already cut its path at this tolerance before
    // it stamped each cut into its own stock. A second merge here would move
    // cuts the planner stock already holds, and the entries read that
    // stock. The stage stays in the trace with its tolerance and the kind
    // "applied_by_planner", and moves nothing.
    if let Some(merge) = cfg.segment_merge {
        let merge_tol = merge.tolerance;
        let by_planner = transform_capabilities.planner_applies_segment_merge;
        current = apply_dressup_traced(
            current,
            debug_ctx,
            semantic_ctx,
            channels,
            DressupStage::SEGMENT_MERGE,
            |scope| {
                scope.set_param(SemanticKey::Tolerance, merge_tol);
                if by_planner {
                    scope.set_param(SemanticKey::Kind, "applied_by_planner");
                }
            },
            |at| {
                if by_planner {
                    Transformed::index_preserving(at)
                } else {
                    crate::dressup::condition::merge_linear_runs(at, merge_tol)
                }
            },
        );
    }

    // 6. Rapid order optimization (unbarriered fallback)
    if cfg.optimize_rapid_order
        && rapid_order_barriers.is_empty()
        && transform_capabilities.allows_unbarriered_rapid_reorder()
    {
        let link_ceiling = internal_link_ceiling_z(&current, safe_z);
        current = apply_dressup_traced(
            current,
            debug_ctx,
            semantic_ctx,
            channels,
            DressupStage::RAPID_ORDER,
            |scope| {
                scope.set_param(SemanticKey::SafeZ, safe_z);
            },
            |at| crate::dressup::tsp::optimize_rapid_order(at, safe_z, link_ceiling),
        );
    }

    // 7. Air-cut filter. S3: the filter classifies each sample for the whole
    // cutter, so it needs the cutter — a `prior_stock` without one cannot be
    // served. Skipping is the conservative arm (nothing becomes a rapid that
    // was not one), and it is unreachable from the two production callers,
    // which both pass a cutter unconditionally.
    if let (Some(stock), Some(cut)) = (prior_stock, cutter) {
        current = apply_dressup_traced(
            current,
            debug_ctx,
            semantic_ctx,
            channels,
            DressupStage::AIR_CUT_FILTER,
            |scope| {
                scope.set_param(SemanticKey::ToolRadius, tool_radius);
                scope.set_param(SemanticKey::SafeZ, safe_z);
            },
            |at| {
                crate::dressup::filter_air_cuts(at, stock, cut, safe_z, 0.1, cfg.air_bridge_policy)
            },
        );
    } else if prior_stock.is_some() {
        tracing::warn!(
            "air-cut filter skipped: prior stock present but no cutter was \
             supplied to apply_dressups"
        );
    }

    // 8. Feed rate optimization. Rewrites feed rates only — move count/order
    // and spans pass through unchanged.
    if cfg.feed_optimization
        && let (Some(stock), Some(cut)) = (feed_opt_stock, cutter)
    {
        let nominal = nominal_feed_rate;
        let max_rate = cfg.feed_max_rate;
        let ramp_rate = cfg.feed_ramp_rate;
        let params = crate::dressup::feedopt::FeedOptParams {
            nominal_feed_rate: nominal,
            max_feed_rate: max_rate,
            // WP21: the ceiling is the operator's own dial and the floor is
            // DERIVED from a second, independent dial. A nominal feed above
            // twice the ceiling therefore puts the floor above the ceiling.
            // The ceiling is the hard limit and the floor is a preference,
            // so the floor takes the ceiling's value. Before WP21 the
            // inverted pair reached `f64::clamp`, which panics on it.
            min_feed_rate: (nominal * 0.5).min(max_rate),
            ramp_rate,
            air_cut_threshold: 0.05,
            // WP22 (G-FEEDOPTPLUNGE): the operation's own plunge rate. The
            // pass caps a geometric plunge at it, so a vertical descent no
            // longer leaves this pass at the CUTTING feed.
            plunge_rate_mm_min,
        };
        current = apply_dressup_traced(
            current,
            debug_ctx,
            semantic_ctx,
            channels,
            DressupStage::FEED_OPTIMIZATION,
            |scope| {
                scope.set_param(SemanticKey::NominalFeedRate, nominal);
                scope.set_param(SemanticKey::MaxFeedRate, max_rate);
                scope.set_param(SemanticKey::RampRate, ramp_rate);
            },
            // Feed optimisation rewrites feed rates only — move count, order and
            // spans pass through untouched. The claim is made explicitly rather
            // than by omission.
            |at| {
                Transformed::index_preserving(crate::dressup::feedopt::optimize_feed_rates(
                    at, cut, stock, &params,
                ))
            },
        );
    }

    AnnotatedToolpath {
        toolpath: current.toolpath,
        spans: current.spans,
        spans_valid: input_valid && current.spans_valid,
        planner_engagement: current.planner_engagement,
        rest_grid: current.rest_grid,
        rest_regions: current.rest_regions,
        area_regions: current.area_regions,
    }
}

// ── Internal helpers ──────────────────────────────────────────────────
