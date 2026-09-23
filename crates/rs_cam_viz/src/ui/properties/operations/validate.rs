//! Toolpath validation and the diagnostics the inspector shows.
//!
//! The Generate button and the Heights tab both call these functions. The
//! parent re-exports every public name.

use rs_cam_core::diagnostics::adapters::from_static_checks as core_static_checks;

use crate::state::job::ToolType;
use crate::state::rest_dependency::{RestCandidate, rest_predecessors};
use crate::state::toolpath::{
    HeightContext, HeightsConfig, OperationConfig, ProfileConfig, ToolpathEntry, ToolpathId,
};

pub struct ToolpathValidationContext {
    tools: Vec<ValidationTool>,
    models: Vec<ValidationModel>,
    setups: Vec<ValidationSetup>,
}

pub(in crate::ui::properties) struct ValidationTool {
    pub(in crate::ui::properties) id: crate::state::job::ToolId,
    pub(in crate::ui::properties) tool_type: ToolType,
    pub(in crate::ui::properties) diameter: f64,
}

struct ValidationModel {
    id: crate::state::job::ModelId,
    has_polygons: bool,
    has_mesh: bool,
    has_enriched_mesh: bool,
    /// Pickable drill targets (DXF points and circle/arc centres) the model
    /// exposes. The `Drill` arm reads this, never `has_polygons`
    /// (G-DRILLCENTROID).
    drill_target_count: usize,
}

struct ValidationSetup {
    /// The setup's toolpaths in plan order, as the Rest predecessor rule
    /// reads them (`crate::state::rest_dependency`).
    toolpaths: Vec<RestCandidate>,
}

impl ToolpathValidationContext {
    /// The diameter of one tool, or `None` when the project has no such
    /// tool. The Rest arm on the registry row reads it.
    pub(in crate::ui::properties) fn tool_diameter(
        &self,
        id: crate::state::job::ToolId,
    ) -> Option<f64> {
        self.tools
            .iter()
            .find(|tool| tool.id == id)
            .map(|tool| tool.diameter)
    }

    /// Build from a `ProjectSession`.
    pub fn from_session(session: &rs_cam_core::session::ProjectSession) -> Self {
        Self {
            tools: session
                .tools()
                .iter()
                .map(|tool| ValidationTool {
                    id: tool.id,
                    tool_type: tool.tool_type,
                    diameter: tool.diameter,
                })
                .collect(),
            models: session
                .models()
                .iter()
                .map(|model| ValidationModel {
                    id: crate::state::job::ModelId(model.id),
                    has_polygons: model.polygons.is_some(),
                    has_mesh: model.mesh.is_some(),
                    has_enriched_mesh: model.enriched_mesh.is_some(),
                    drill_target_count: model.drill_targets.len(),
                })
                .collect(),
            setups: session
                .list_setups()
                .iter()
                .map(|setup| ValidationSetup {
                    toolpaths: setup
                        .toolpath_indices
                        .iter()
                        .filter_map(|&tp_idx| session.toolpath_configs().get(tp_idx))
                        .map(RestCandidate::from_config)
                        .collect(),
                })
                .collect(),
        }
    }
}

/// Validate a session `ToolpathConfig` using the same logic as `validate_toolpath`.
pub fn validate_toolpath_config(
    tc: &rs_cam_core::session::ToolpathConfig,
    ctx: &ToolpathValidationContext,
) -> Vec<String> {
    let mut errs = Vec::new();

    let tool_id = crate::state::job::ToolId(tc.tool_id);
    let model_id = crate::state::job::ModelId(tc.model_id);
    let tp_id = tc.id;

    let Some(tool) = ctx.tools.iter().find(|t| t.id == tool_id) else {
        errs.push("No tool selected".into());
        return errs;
    };

    // Geometry validation (inline, operating on tc fields)
    if !tc.operation.is_stock_based() {
        let model = ctx.models.iter().find(|m| m.id == model_id);
        if let Some(model) = model {
            let has_face_polygons = model.has_enriched_mesh
                && tc.face_selection.as_ref().is_some_and(|f| !f.is_empty());
            let has_polygons = model.has_polygons || has_face_polygons;
            let has_mesh = model.has_mesh;

            if tc.operation.needs_both() {
                let effective_has_mesh = if let OperationConfig::ProjectCurve(ref cfg) =
                    tc.operation
                    && let Some(surface_id) = cfg.surface_model_id
                {
                    ctx.models
                        .iter()
                        .find(|m| m.id == surface_id)
                        .is_some_and(|m| m.has_mesh)
                } else {
                    has_mesh
                };
                if !has_polygons || !effective_has_mesh {
                    errs.push("Selected model must provide both 2D geometry and a 3D mesh".into());
                }
            } else if tc.operation.is_3d() {
                if !has_mesh {
                    errs.push("Selected model has no 3D mesh".into());
                }
            } else if !has_polygons {
                errs.push("Selected model has no 2D geometry".into());
            }
        } else {
            errs.push("Selected model is missing".into());
        }
    }

    // UI-05: the per-operation checks live on the registry row, so this
    // entry point and `validate_toolpath` cannot drift. They used to carry
    // byte-identical copies of the same match, messages included.
    super::registry::op_errors(
        &tc.operation,
        &super::registry::OpValidateCtx {
            tool,
            ctx,
            tp_id,
            model_id,
        },
        &mut errs,
    );

    errs
}

/// G-DRILLCENTROID (UX-R03-004): the same predicate the generator refuses
/// with, read against the target model's drill-target count, so Generate is
/// disabled with the generator's own sentence instead of a hole appearing
/// at a polygon centroid.
pub(in crate::ui::properties) fn drill_targets_refusal(
    ctx: &ToolpathValidationContext,
    model_id: crate::state::job::ModelId,
    cfg: &crate::state::toolpath::DrillConfig,
) -> Option<&'static str> {
    let targets = ctx
        .models
        .iter()
        .find(|m| m.id == model_id)
        .map_or(0, |m| m.drill_target_count);
    rs_cam_core::compute::execute::drill_targets_refusal(cfg, targets)
}

pub fn validate_toolpath(entry: &ToolpathEntry, ctx: &ToolpathValidationContext) -> Vec<String> {
    let mut errs = Vec::new();

    let Some(tool) = ctx.tools.iter().find(|tool| tool.id == entry.tool_id) else {
        errs.push("No tool selected".into());
        return errs;
    };

    validate_geometry_selection(entry, ctx, &mut errs);

    super::registry::op_errors(
        &entry.operation,
        &super::registry::OpValidateCtx {
            tool,
            ctx,
            tp_id: entry.id,
            model_id: entry.model_id,
        },
        &mut errs,
    );

    errs
}

fn validate_geometry_selection(
    entry: &ToolpathEntry,
    ctx: &ToolpathValidationContext,
    errs: &mut Vec<String>,
) {
    if entry.operation.is_stock_based() {
        return;
    }

    let model = ctx.models.iter().find(|model| model.id == entry.model_id);
    let Some(model) = model else {
        errs.push("Selected model is missing".into());
        return;
    };

    // STEP models with face selection derive polygons at compute time
    let has_face_polygons =
        model.has_enriched_mesh && entry.face_selection.as_ref().is_some_and(|f| !f.is_empty());
    let has_polygons = model.has_polygons || has_face_polygons;
    let has_mesh = model.has_mesh;

    if entry.operation.needs_both() {
        // ProjectCurve can use a separate surface model for the 3D mesh.
        let effective_has_mesh =
            if let crate::state::toolpath::OperationConfig::ProjectCurve(ref cfg) = entry.operation
                && let Some(surface_id) = cfg.surface_model_id
            {
                ctx.models
                    .iter()
                    .find(|m| m.id == surface_id)
                    .is_some_and(|m| m.has_mesh)
            } else {
                has_mesh
            };
        if !has_polygons || !effective_has_mesh {
            errs.push("Selected model must provide both 2D geometry and a 3D mesh (use Surface selector for separate mesh)".into());
        }
    } else if entry.operation.is_3d() {
        if !has_mesh {
            errs.push("Selected model has no 3D mesh".into());
        }
    } else if !has_polygons {
        errs.push("Selected model has no 2D geometry".into());
    }
}

/// Does the Rest op `rest_id` have a predecessor under the one rule in
/// [`crate::state::rest_dependency`]? The Operations card connector reads
/// the stored form of the same rule, the core `PrevTool` edge (G-RESTBADGE).
pub(in crate::ui::properties) fn has_prior_rest_source(
    ctx: &ToolpathValidationContext,
    rest_id: ToolpathId,
    rest_model_id: crate::state::job::ModelId,
    prev_tool_id: crate::state::job::ToolId,
) -> bool {
    ctx.setups.iter().any(|setup| {
        !rest_predecessors(&setup.toolpaths, rest_id, rest_model_id, prev_tool_id).is_empty()
    })
}

// ── Depth vs stock thickness (G-DEPTHSTOCK / G-DEPTHSTOCKCORE) ──────────

/// The depth-beyond-stock finding, re-exported from core.
///
/// F1.6 shipped this rule GUI-side. F1.18 moved the predicate to
/// [`core_static_checks::depth_beyond_stock`] and F1.18's GUI half deleted
/// the copy, so the Operations card row, the Safety header and MCP
/// `get_toolpath_diagnostics` all answer from ONE predicate. The name stays
/// here so the per-operation forms need no edit; the fields are core's, and
/// the old `bottom_z` field is now `cut_floor_z`.
pub use rs_cam_core::diagnostics::adapters::from_static_checks::DepthBeyondStock;

/// The equality epsilon the through-cut line shares with the depth rule.
/// One definition, in core: excesses at or under this are arithmetic, not a
/// cut into the bed, and a through cut EXACTLY at the stock thickness must
/// not trigger the caution (UX-R03-006 owns that case).
const DEPTH_BEYOND_STOCK_EPSILON_MM: f64 = core_static_checks::DEPTH_BEYOND_STOCK_EPS_MM;

/// The resolved-heights snapshot every GUI diagnostic surface hands to core.
///
/// One line, because the derivation belongs to core. N4 (2026-09-10) hoisted
/// the body into `ResolvedHeights::from_heights`, and the session route —
/// which MCP `get_toolpath_diagnostics` calls — now uses the same
/// constructor. Before that this GUI copy was the only caller that read the
/// entry's OWN `HeightsConfig`; the session route called
/// `ResolvedHeights::from_context`, which projects the stock top and the safe
/// Z into the five slots and drops every pin.
///
/// This matters for the depth rule: the generators cut `top_z - depth`, so a
/// Top Z pinned below the stock top deepens the emitted floor.
/// [`profile_through_cut`] in the same form already reads that pin
/// (`a_top_z_pinned_below_the_stock_top_counts_towards_the_through_cut_g_throughcut`),
/// and the two lines must not disagree at the boundary they share.
///
/// The stock span is `Some` because the context always carries it. `None`
/// there means NOT MEASURED and makes core's rule abstain.
fn diagnostics_heights(
    heights: &HeightsConfig,
    ctx: &HeightContext,
) -> core_static_checks::ResolvedHeights {
    core_static_checks::ResolvedHeights::from_heights(heights, ctx)
}

/// UX-R03-007 / G-DEPTHSTOCK: does this cut go below the stock bottom?
///
/// **This function carries no rule.** It is an input adapter: it resolves the
/// entry's heights and hands them to [`core_static_checks::depth_beyond_stock`].
/// Which operations the rule answers for, the comparison, the epsilon and the
/// sentence all live in core.
///
/// Two things a reader must not infer wrongly.
///
/// - The pinned BOTTOM Z is deliberately not read. F1.19 measured that a
///   pinned bottom reaches no emitted motion on any operation this rule
///   answers for, so cautioning on it described a cut the machine does not
///   make. The Heights tab says that beside the field instead
///   ([`bottom_z_pin_note`]).
/// - `None` covers three states and none of them is "clean on a measured
///   value": the rule does not answer for this operation, the caller supplied
///   no stock span, or the floor is at or above the stock bottom.
pub fn depth_beyond_stock(
    operation: &OperationConfig,
    heights: &HeightsConfig,
    ctx: &HeightContext,
) -> Option<DepthBeyondStock> {
    core_static_checks::depth_beyond_stock(operation, &diagnostics_heights(heights, ctx))
}

// ── Profile through cut (G-THROUGHCUT, UX-R03-006) ──────────────────────

/// A Profile whose cut bottom reaches the bottom of the board.
///
/// Informational, never a caution: the last pass frees the part, and the
/// operator decides how it is held. Zero tabs is a valid answer (vacuum,
/// double-sided tape), so the line names the holding and does not judge it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThroughCut {
    /// The depth the rule compared, measured from the STOCK top, in mm. Equal
    /// to the Depth field when Top Z is `Auto`; deeper when Top Z is pinned
    /// below the stock top.
    pub depth_mm: f64,
    /// The stock thickness of the setup, in mm.
    pub stock_thickness_mm: f64,
    /// The Profile's tab count at the time the rule was read.
    pub tab_count: usize,
}

impl ThroughCut {
    /// The one line the Profile form prints beside Depth.
    pub fn message(&self) -> String {
        through_cut_message(self.stock_thickness_mm, self.tab_count)
    }
}

/// The board thickness as an operator writes it: `12 mm`, `18.5 mm`. One
/// decimal only when the thickness has one.
fn board_thickness_text(stock_thickness_mm: f64) -> String {
    if (stock_thickness_mm - stock_thickness_mm.round()).abs() < 0.05 {
        format!("{stock_thickness_mm:.0}")
    } else {
        format!("{stock_thickness_mm:.1}")
    }
}

fn through_cut_message(stock_thickness_mm: f64, tab_count: usize) -> String {
    let board = board_thickness_text(stock_thickness_mm);
    let holding = match tab_count {
        0 => "no tabs configured".to_owned(),
        1 => "1 tab".to_owned(),
        k => format!("{k} tabs"),
    };
    format!("Through cut of a {board} mm board · Holding: {holding}")
}

/// UX-R03-006 / G-THROUGHCUT: the through-cut line for a Profile.
///
/// `None` when `depth_mm` is under `stock_thickness_mm` (a partial-depth
/// profile). Otherwise the line names the board and the holding:
///
/// - `Through cut of a 12 mm board · Holding: no tabs configured`
/// - `Through cut of a 12 mm board · Holding: 4 tabs`
///
/// The comparison is `depth >= thickness` within
/// [`DEPTH_BEYOND_STOCK_EPSILON_MM`], so a depth EXACTLY at the thickness is
/// a through cut (the case G-DEPTHSTOCK deliberately leaves alone), and a
/// depth beyond it shows this line AND the depth caution. A non-finite or
/// non-positive thickness gives `None`: there is no board to cut through.
pub fn profile_through_cut_line(
    depth_mm: f64,
    stock_thickness_mm: f64,
    tab_count: usize,
) -> Option<String> {
    if !depth_mm.is_finite()
        || !stock_thickness_mm.is_finite()
        || stock_thickness_mm <= 0.0
        || depth_mm + DEPTH_BEYOND_STOCK_EPSILON_MM < stock_thickness_mm
    {
        return None;
    }
    Some(through_cut_message(stock_thickness_mm, tab_count))
}

/// The form of [`profile_through_cut_line`] the inspector reads: the depth is
/// the Profile's cut bottom measured from the stock top, so a Top Z pinned
/// below the stock top counts towards the through cut the same way it does
/// in [`depth_beyond_stock`]. The generator cuts `top_z - cfg.depth`
/// (`OperationConfig::cutting_levels`), so that is the bottom read here; a
/// pinned Bottom Z does not reach the profile generator and is not read.
pub fn profile_through_cut(
    cfg: &ProfileConfig,
    heights: &HeightsConfig,
    ctx: &HeightContext,
) -> Option<ThroughCut> {
    let resolved = heights.resolve(ctx);
    let bottom_z = resolved.top_z - cfg.depth;
    let depth_mm = ctx.stock_top_z - bottom_z;
    let stock_thickness_mm = ctx.stock_top_z - ctx.stock_bottom_z;
    profile_through_cut_line(depth_mm, stock_thickness_mm, cfg.tab_count).map(|_| ThroughCut {
        depth_mm,
        stock_thickness_mm,
        tab_count: cfg.tab_count,
    })
}

// ── Contextual diagnostics ──────────────────────────────────────────────

/// Collect the unified [`rs_cam_core::diagnostics::Diagnostic`]
/// list for a single toolpath via the core orchestrator. All GUI
/// surfaces that need per-toolpath diagnostics route through this
/// function — it delegates to
/// [`rs_cam_core::diagnostics::diagnose_toolpath_inputs`] so the
/// params panel and MCP `get_toolpath_diagnostics` produce identical
/// findings for the same project state. Since F1.18 that includes
/// `geom.depth_beyond_stock`: the GUI carries no rule of its own any
/// more (see [`depth_beyond_stock`]).
///
/// The heights snapshot is [`diagnostics_heights`], built from the
/// entry's own `HeightsConfig`. Since N4 (2026-09-10) the session
/// route builds its snapshot with the same core constructor, so a
/// pinned Top Z reaches both surfaces. The caller supplies the required
/// precondition and model-reference contexts from the same owned panel
/// snapshot as the editable entry, so these static checks cannot be omitted
/// from this GUI path.
///
/// Load-gate (chipload / power / deflection / drill) diagnostics are
/// included when a `load_verdict` is supplied — they render in the
/// same ribbon as the other findings.
///
/// `suggest_warnings` is the record of the Suggest run behind the recipe,
/// from `ProjectSession::suggest_warnings_for_toolpath` (the call the MCP
/// surface makes too). Ruling R4 (2026-09-24): the aggressiveness record
/// becomes `feeds.aggressiveness_engagement` here, so no Suggest calculation
/// is invisible in the ribbon.
// SAFETY: the ribbon reads eight independent inputs of one toolpath. The
// snapshot owns three of them, and the panel borrows its entry mutably
// while it reads the others, so one struct would not remove a parameter.
#[allow(clippy::too_many_arguments)]
pub fn collect_diagnostics(
    entry: &ToolpathEntry,
    tool: Option<&rs_cam_core::compute::tool_config::ToolConfig>,
    stale_defaults: &[rs_cam_core::compute::validate::StaleDefault],
    height_ctx: Option<&HeightContext>,
    preconditions: &rs_cam_core::diagnostics::diagnose::PreconditionContext,
    model_refs: &rs_cam_core::diagnostics::diagnose::ModelRefContext,
    suggest_warnings: Option<&[rs_cam_core::feeds::suggest::SuggestWarning]>,
    load_verdict: Option<&rs_cam_core::tool_load::ToolpathLoadVerdict>,
) -> Vec<rs_cam_core::diagnostics::Diagnostic> {
    let Some(tool) = tool else {
        return Vec::new();
    };
    let heights = height_ctx.map(|ctx| diagnostics_heights(&entry.heights, ctx));
    let inputs = rs_cam_core::diagnostics::ToolpathDiagnoseInputs {
        toolpath_id: entry.id,
        operation: &entry.operation,
        tool,
        heights: heights.as_ref(),
        feeds_result: entry.feeds_result.as_ref(),
        suggest_warnings,
        load_verdict,
        stale_defaults,
        preconditions: Some(preconditions),
        model_refs: Some(model_refs),
        // A/M9: generation-time findings (standing material) ride on the
        // entry's own result. Passing `None` here was why the GUI's
        // diagnostics ribbon — the surface a router operator actually
        // reads — stayed silent about a raised island the core diagnostic
        // pipeline already knew about. `None` before generation is honest:
        // nothing has been measured yet.
        stats: entry.result.as_ref().map(|result| &result.stats),
    };
    // G-DEPTHSTOCKCORE (F1.18): the depth-beyond-stock caution used to be
    // appended here from a second, GUI-side copy of the rule. Both copies
    // fired, both stamped `geom.depth_beyond_stock`, and the operator read
    // the identical sentence twice. The copy is deleted; the caution now
    // arrives inside this list, from the one core predicate, on the snapshot
    // `diagnostics_heights` built above.
    rs_cam_core::diagnostics::diagnose_toolpath_inputs(&inputs)
}
