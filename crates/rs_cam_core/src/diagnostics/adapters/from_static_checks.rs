//! Adapter: static config-only checks → [`Diagnostic`] list.
//!
//! These checks need *only* the toolpath config + tool + (optionally)
//! the resolved heights. They run before simulation and stay valid
//! independent of any sim trace. Geometric impossibilities (e.g.
//! stepover > diameter) live here, as do tool/op compatibility rules
//! and Z-frame ordering.
//!
//! Heuristic surface-quality / cycle-time hints are also here but
//! are emitted at [`Severity::Hint`] / [`Severity::Info`] so the
//! headline ribbon doesn't show them by default.
//!
//! Mirrors the PR-1 categorization currently living in
//! `crates/rs_cam_viz/src/ui/properties/operations/mod.rs::collect_warnings`.
//! The intent is for PR-3 to delete the GUI-side function and
//! consume this list instead.

use crate::compute::catalog::{OperationConfig, UiProcessRole};
use crate::compute::config::{HeightContext, HeightsConfig};
use crate::compute::operation_configs::{PROJECT_CURVE_POINT_CAUTION, ProjectCurveProjection};
use crate::compute::tool_config::{ToolConfig, ToolType};
use crate::diagnostics::{
    Category, Confidence, Diagnostic, DiagnosticEvidence, DiagnosticId, DiagnosticState, Scope,
    Severity, Source, ids,
};
use crate::ids::ToolpathId;

/// Run every static check against a single toolpath, returning a flat
/// diagnostic list (no supersession applied yet).
pub fn diagnostics_from_static_checks(
    toolpath_id: ToolpathId,
    op: &OperationConfig,
    tool: &ToolConfig,
    heights_resolved: Option<&ResolvedHeights>,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let scope = Scope::Toolpath { id: toolpath_id };

    let feed = op.feed_rate();
    let plunge = op.plunge_rate();
    let spec = op.op_type().spec();

    out.extend(tool_op_compat_checks(&scope, op, tool));
    out.extend(stepover_checks(&scope, op, tool, spec.ui_process_role));
    out.extend(depth_checks(&scope, op, tool));
    out.extend(feed_checks(&scope, feed, plunge));
    if let Some(h) = heights_resolved {
        out.extend(heights_checks(&scope, op, h));
    }

    out
}

/// Resolved heights snapshot. Matches the four-Z layout the GUI
/// passes through `HeightContext`. Adapter callers resolve heights
/// once and pass the result here so this module doesn't need to
/// reach into the HeightMode resolution code.
#[derive(Debug, Clone)]
pub struct ResolvedHeights {
    pub top_z: f64,
    /// UX-1b: the Z that the depth dial of an `OpDial` op measures from
    /// (`compute::config::ResolvedHeights::depth_base_z`). Under a face
    /// pick on Face or Trace it is the face top while `top_z` is the stock
    /// top. Else it is `top_z`.
    pub depth_base_z: f64,
    pub bottom_z: f64,
    pub feed_z: f64,
    pub retract_z: f64,
    pub clearance_z: f64,
    /// Top of the stock this setup presents, when the caller knows it.
    ///
    /// `None` means **NOT MEASURED**, never "the cut is inside the board".
    /// [`depth_beyond_stock`] abstains rather than guess a thickness.
    pub stock_top_z: Option<f64>,
    /// Bottom of the stock this setup presents. Same contract as
    /// [`Self::stock_top_z`].
    pub stock_bottom_z: Option<f64>,
    /// UX-1: the resolved floor of an `OpFloor` op (Pocket, Profile,
    /// Zigzag, Adaptive 2D, Bore) from THE session heights
    /// (`ToolpathHeights::floor_z`). Under a face pick it measures from the
    /// face top while `top_z` is the stock top, so `top_z - depth` is not
    /// the floor. `None` when the caller holds no session heights or the
    /// floor refuses.
    pub op_floor_z: Option<f64>,
}

impl ResolvedHeights {
    /// Build from a [`HeightContext`] alone, for a caller that holds no
    /// [`HeightsConfig`].
    ///
    /// This constructor PROJECTS. It puts the stock top in `top_z` and
    /// `feed_z`, the stock bottom in `bottom_z`, and the safe Z in
    /// `retract_z` and `clearance_z`. It drops every pin the operator
    /// set. Three of the four plane-order checks in `heights_checks`
    /// therefore cannot fire on the result at all, the fourth
    /// (`retract_z` below `feed_z`) fires only on a bad safe Z, and
    /// [`depth_beyond_stock`] misses a Top Z pinned below the stock top.
    ///
    /// A caller that holds the toolpath's own [`HeightsConfig`] calls
    /// [`Self::from_toolpath_heights`] instead. N4 (2026-09-10) moved the
    /// session route across for exactly that reason; P3.1 put it on THE
    /// session rule. The snapshot still carries no pin FLAG, which is why
    /// [`depth_beyond_stock_applies`] abstains for the three operations
    /// whose floor can be a pinned Bottom Z.
    pub fn from_context(ctx: &HeightContext) -> Self {
        Self {
            top_z: ctx.stock_top_z,
            depth_base_z: ctx.stock_top_z,
            bottom_z: ctx.stock_bottom_z,
            feed_z: ctx.stock_top_z,
            retract_z: ctx.safe_z,
            clearance_z: ctx.safe_z,
            stock_top_z: Some(ctx.stock_top_z),
            stock_bottom_z: Some(ctx.stock_bottom_z),
            op_floor_z: None,
        }
    }

    /// Build from THE heights of the session (geometry selection P3.1):
    /// `heights` resolved by [`crate::session::ToolpathHeights::resolve`],
    /// so the snapshot holds the face top that generation cuts from.
    pub fn from_toolpath_heights(
        answer: &crate::session::ToolpathHeights,
        heights: &HeightsConfig,
    ) -> Self {
        let mut out = Self::from_resolved(&answer.resolve(heights), &answer.context);
        out.op_floor_z = answer.floor_z(heights).and_then(Result::ok);
        out
    }

    /// The five planes as the compute struct, for THE order rule
    /// ([`crate::compute::config::height_order`]). The pin flags are not
    /// read by that rule.
    fn planes(&self) -> crate::compute::config::ResolvedHeights {
        crate::compute::config::ResolvedHeights {
            clearance_z: self.clearance_z,
            retract_z: self.retract_z,
            feed_z: self.feed_z,
            top_z: self.top_z,
            depth_base_z: self.depth_base_z,
            bottom_z: self.bottom_z,
            top_pinned: false,
            bottom_pinned: false,
        }
    }

    fn from_resolved(
        resolved: &crate::compute::config::ResolvedHeights,
        ctx: &HeightContext,
    ) -> Self {
        Self {
            top_z: resolved.top_z,
            depth_base_z: resolved.depth_base_z,
            bottom_z: resolved.bottom_z,
            feed_z: resolved.feed_z,
            retract_z: resolved.retract_z,
            clearance_z: resolved.clearance_z,
            stock_top_z: Some(ctx.stock_top_z),
            stock_bottom_z: Some(ctx.stock_bottom_z),
            op_floor_z: None,
        }
    }
}

// ── tool name against tool geometry ─────────────────────────────────

/// The tool-scoped static check: does a size token in the tool NAME that
/// clearly names the tip disagree with the tool's `diameter`?
///
/// The verdict is [`ToolConfig::name_tip_size_mismatch`]; this adapter
/// only converts it. The finding is per TOOL, not per toolpath, so the
/// session raises it once per tool from `ProjectSession::diagnose_project`
/// rather than from [`diagnostics_from_static_checks`], which runs once
/// per toolpath.
pub fn diagnostics_from_tool_name(tool: &ToolConfig) -> Vec<Diagnostic> {
    let Some(mismatch) = tool.name_tip_size_mismatch() else {
        return Vec::new();
    };
    let Some(caution) = mismatch.caution_line() else {
        return Vec::new();
    };
    vec![Diagnostic {
        id: DiagnosticId::from(ids::TOOL_NAME_SIZE_MISMATCH),
        scope: Scope::Tool { id: tool.id },
        category: Category::Geometry,
        severity: Severity::Info,
        confidence: Confidence::Static,
        state: DiagnosticState::Current,
        source: Source::StaticValidation,
        face: crate::diagnostics::presentation::face(ids::TOOL_NAME_SIZE_MISMATCH),
        anchor: crate::diagnostics::presentation::anchor(ids::TOOL_NAME_SIZE_MISMATCH),
        message: format!("Tool \"{}\": {caution}", tool.name),
        evidence: None,
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    }]
}

/// The tool-scoped check of FC0 item 7 (decision D4 = warn): the
/// stick-out is shorter than the flute. The verdict is
/// [`ToolConfig::stickout_shorter_than_flute`]; this adapter only converts
/// it. One finding per tool, raised from `ProjectSession::diagnose_project`.
pub fn diagnostics_from_tool_stickout(tool: &ToolConfig) -> Vec<Diagnostic> {
    if !tool.stickout_shorter_than_flute() {
        return Vec::new();
    }
    vec![Diagnostic {
        id: DiagnosticId::from(ids::TOOL_STICKOUT_SHORTER_THAN_FLUTE),
        scope: Scope::Tool { id: tool.id },
        category: Category::Geometry,
        severity: Severity::Caution,
        confidence: Confidence::Static,
        state: DiagnosticState::Current,
        source: Source::StaticValidation,
        face: crate::diagnostics::presentation::face(ids::TOOL_STICKOUT_SHORTER_THAN_FLUTE),
        anchor: crate::diagnostics::presentation::anchor(ids::TOOL_STICKOUT_SHORTER_THAN_FLUTE),
        message: format!(
            "Tool \"{}\": {}",
            tool.name,
            tool.stickout_shorter_than_flute_text()
        ),
        evidence: None,
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    }]
}

// ── tool/operation compatibility ────────────────────────────────────

fn tool_op_compat_checks(
    scope: &Scope,
    op: &OperationConfig,
    tool: &ToolConfig,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let tool_type = tool.tool_type;

    // Ball-nose on 2D clearing → demoted quality hint (PR-1).
    if matches!(tool_type, ToolType::BallNose | ToolType::TaperedBallNose)
        && op.capabilities().flat_floor_clearing
    {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::COMPAT_BALL_NOSE_FLAT_CLEARING),
            scope: scope.clone(),
            category: Category::Quality,
            severity: Severity::Hint,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            face: crate::diagnostics::presentation::face(ids::COMPAT_BALL_NOSE_FLAT_CLEARING),
            anchor: crate::diagnostics::presentation::anchor(ids::COMPAT_BALL_NOSE_FLAT_CLEARING),
            message:
                "A ball nose leaves ridges on flat floors. Use a flat end mill to clear flat areas."
                    .to_owned(),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }

    // W2.2: a tool the registry row refuses → the refusal text. The row's
    // `tool_constraints` is the one source. It replaced a hand list (end
    // mill on Scallop, Unified Finish or Pencil) that missed Spiral Finish,
    // the bull nose, the V-bit and the four V-bit operations. K6 (decision
    // E1): `Blocking` only when the generator cannot compute with the tool
    // (`OperationType::tool_cannot_compute`, the prologue's refusal); else a
    // `Critical` warning and the op generates.
    if let Some(text) = op.op_type().tool_refusal(tool_type.cutter_kind()) {
        let severity = if op
            .op_type()
            .tool_cannot_compute(tool_type.cutter_kind())
            .is_some()
        {
            Severity::Blocking
        } else {
            Severity::Critical
        };
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::COMPAT_TOOL_KIND_REFUSED),
            scope: scope.clone(),
            // The wrong kind of tool is a fit fault, not a load fault: the
            // same category as the other refusals of the op.
            category: Category::Geometry,
            severity,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            // W1: the face names the tool kind the op needs.
            face: crate::diagnostics::presentation::face_or_default(
                ids::COMPAT_TOOL_KIND_REFUSED,
                crate::diagnostics::presentation::sentence_case(&op.op_type().needs_text()),
            ),
            anchor: crate::diagnostics::presentation::anchor(ids::COMPAT_TOOL_KIND_REFUSED),
            message: text,
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }

    // Point projection on Project Curve → a caution for any tool. The
    // point rule (tip Z = surface Z + depth) is general: the tip is the
    // lowest point of every tool shape, and the far-surface risk does not
    // depend on the shape. So no tool is refused; the operator reads this.
    if let OperationConfig::ProjectCurve(cfg) = op
        && cfg.projection == ProjectCurveProjection::Point
    {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::COMPAT_PROJECT_CURVE_POINT_PROJECTION),
            scope: scope.clone(),
            category: Category::Geometry,
            severity: Severity::Caution,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            face: crate::diagnostics::presentation::face(
                ids::COMPAT_PROJECT_CURVE_POINT_PROJECTION,
            ),
            anchor: crate::diagnostics::presentation::anchor(
                ids::COMPAT_PROJECT_CURVE_POINT_PROJECTION,
            ),
            message: PROJECT_CURVE_POINT_CAUTION.to_owned(),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }

    out
}

// ── stepover ────────────────────────────────────────────────────────

/// F117: the one stepover rule. `Some(stepover)` when `op` has a stepover
/// wider than a tool of `diameter`.
///
/// Two adjacent passes leave an uncut strip of width
/// `stepover − diameter`. The strip has a width only when
/// `stepover > diameter`; at equality the passes meet. Suggest clamps a
/// wider stepover to exactly the diameter
/// (`feeds::suggest::invariants::clamp_stepover_to_diameter`), so equality
/// is a value core writes itself. The GUI Generate gate refused it
/// (`stepover >= diameter`, Pocket and Adaptive only) while core flagged
/// `>` on every op and the CLI generated. The static check, the GUI gate
/// and the Suggest clamp read this one function.
///
/// `None` for an op with no stepover, or a diameter that is not positive.
#[must_use]
pub fn stepover_exceeds_diameter(op: &OperationConfig, diameter: f64) -> Option<f64> {
    let stepover = op.stepover()?;
    (diameter > 0.0 && stepover > diameter).then_some(stepover)
}

fn stepover_checks(
    scope: &Scope,
    op: &OperationConfig,
    tool: &ToolConfig,
    role: UiProcessRole,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let Some(stepover) = op.stepover() else {
        return out;
    };
    let diameter = tool.diameter;
    if diameter <= 0.0 {
        return out;
    }

    if stepover_exceeds_diameter(op, diameter).is_some() {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::GEOM_STEPOVER_EXCEEDS_DIAMETER),
            scope: scope.clone(),
            category: Category::Geometry,
            severity: Severity::Critical,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            face: crate::diagnostics::presentation::face(ids::GEOM_STEPOVER_EXCEEDS_DIAMETER),
            anchor: crate::diagnostics::presentation::anchor(ids::GEOM_STEPOVER_EXCEEDS_DIAMETER),
            message: format!(
                "The stepover ({stepover:.2} mm) is wider than the tool ({diameter:.1} mm). \
                 The toolpath leaves strips of uncut material."
            ),
            evidence: Some(DiagnosticEvidence::GeometryCompare {
                lhs_label: "stepover".to_owned(),
                lhs_value: stepover,
                rhs_label: "tool_diameter".to_owned(),
                rhs_value: diameter,
                unit: "mm".to_owned(),
            }),
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
        return out;
    }

    if stepover > diameter * 0.8 {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::QUALITY_STEPOVER_OVER_80_PCT),
            scope: scope.clone(),
            category: Category::Quality,
            severity: Severity::Hint,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            face: crate::diagnostics::presentation::face(ids::QUALITY_STEPOVER_OVER_80_PCT),
            anchor: crate::diagnostics::presentation::anchor(ids::QUALITY_STEPOVER_OVER_80_PCT),
            message: format!(
                "The stepover is {:.0} % of the tool diameter. It can leave visible ridges.",
                (stepover / diameter) * 100.0
            ),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }

    if stepover < diameter * 0.05 {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::EFFICIENCY_VERY_FINE_STEPOVER),
            scope: scope.clone(),
            category: Category::Efficiency,
            severity: Severity::Info,
            confidence: Confidence::Heuristic,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            face: crate::diagnostics::presentation::face(ids::EFFICIENCY_VERY_FINE_STEPOVER),
            anchor: crate::diagnostics::presentation::anchor(ids::EFFICIENCY_VERY_FINE_STEPOVER),
            message: "The stepover is very fine. The cycle time is much longer.".to_owned(),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }

    if matches!(role, UiProcessRole::Finish) && stepover > diameter * 0.5 {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::QUALITY_FINISH_STEPOVER_OVER_50_PCT),
            scope: scope.clone(),
            category: Category::Quality,
            severity: Severity::Hint,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            face: crate::diagnostics::presentation::face(ids::QUALITY_FINISH_STEPOVER_OVER_50_PCT),
            anchor: crate::diagnostics::presentation::anchor(
                ids::QUALITY_FINISH_STEPOVER_OVER_50_PCT,
            ),
            message: format!(
                "The stepover ({stepover:.2} mm) is more than {:.0} % of the tool diameter. The \
                 finish can be rough.",
                (stepover / diameter) * 100.0
            ),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }

    if matches!(
        tool.tool_type,
        ToolType::BallNose | ToolType::TaperedBallNose
    ) && matches!(role, UiProcessRole::Finish)
    {
        let ball_r = diameter / 2.0;
        if ball_r > 0.0 && stepover < diameter {
            let half_so = stepover / 2.0;
            let inner = ball_r * ball_r - half_so * half_so;
            if inner > 0.0 {
                let scallop_h = ball_r - inner.sqrt();
                if scallop_h > 0.1 {
                    out.push(Diagnostic {
                        id: DiagnosticId::from(ids::QUALITY_BALL_SCALLOP_HEIGHT),
                        scope: scope.clone(),
                        category: Category::Quality,
                        severity: Severity::Hint,
                        confidence: Confidence::Static,
                        state: DiagnosticState::Current,
                        source: Source::StaticValidation,
                        face: crate::diagnostics::presentation::face(
                            ids::QUALITY_BALL_SCALLOP_HEIGHT,
                        ),
                        anchor: crate::diagnostics::presentation::anchor(
                            ids::QUALITY_BALL_SCALLOP_HEIGHT,
                        ),
                        message: format!(
                            "At this stepover the ball nose leaves ridges {scallop_h:.3} mm \
                             high. Make the stepover smaller for a smoother finish."
                        ),
                        evidence: None,
                        fix: None,
                        supersedes: vec![],
                        suppressed_diagnostics: vec![],
                    });
                }
            }
        }
    }

    out
}

// ── depth ───────────────────────────────────────────────────────────

fn depth_checks(scope: &Scope, op: &OperationConfig, tool: &ToolConfig) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let Some(dpp) = op.depth_per_pass() else {
        return out;
    };

    if tool.cutting_length_planning_mm() > 0.0 && dpp > tool.cutting_length_planning_mm() {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::GEOM_DPP_EXCEEDS_CUTTING_LENGTH),
            scope: scope.clone(),
            category: Category::Safety,
            severity: Severity::Critical,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            face: crate::diagnostics::presentation::face(ids::GEOM_DPP_EXCEEDS_CUTTING_LENGTH),
            anchor: crate::diagnostics::presentation::anchor(ids::GEOM_DPP_EXCEEDS_CUTTING_LENGTH),
            message: format!(
                "The depth per pass ({dpp:.1} mm) is more than the cutting length ({:.1} mm). \
                 The tool shank touches the material.",
                tool.cutting_length_planning_mm()
            ),
            evidence: Some(DiagnosticEvidence::GeometryCompare {
                lhs_label: "depth_per_pass".to_owned(),
                lhs_value: dpp,
                rhs_label: "cutting_length".to_owned(),
                rhs_value: tool.cutting_length_planning_mm(),
                unit: "mm".to_owned(),
            }),
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }

    // Past the published de-rate table (feeds matrix R3). The vendors
    // print the depth de-rate to 3 x D only; above it the feed and the
    // chipload band hold the last printed point, 0.50. The source layer,
    // `feeds::geometry`, owns the table edge.
    if tool.diameter > 0.0
        && crate::feeds::geometry::depth_beyond_published_table(dpp / tool.diameter)
    {
        let last_ratio = crate::feeds::geometry::DOC_DERATE_LAST_PRINTED_RATIO;
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::FEEDS_DEPTH_BEYOND_PUBLISHED_TABLE),
            scope: scope.clone(),
            category: Category::ToolLoad,
            severity: Severity::Caution,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            face: crate::diagnostics::presentation::face(ids::FEEDS_DEPTH_BEYOND_PUBLISHED_TABLE),
            anchor: crate::diagnostics::presentation::anchor(
                ids::FEEDS_DEPTH_BEYOND_PUBLISHED_TABLE,
            ),
            message: format!(
                "The depth per pass ({dpp:.1} mm) is {:.1} \u{00D7} the tool diameter ({:.1} mm). \
                 Vendor tables go to {last_ratio:.0} \u{00D7} the diameter only. The feed and \
                 the chipload range use the last table value, 50 %.",
                dpp / tool.diameter,
                tool.diameter
            ),
            evidence: Some(DiagnosticEvidence::GeometryCompare {
                lhs_label: "depth_per_pass".to_owned(),
                lhs_value: dpp,
                rhs_label: "last_printed_depth (3 x diameter)".to_owned(),
                rhs_value: tool.diameter * last_ratio,
                unit: "mm".to_owned(),
            }),
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }

    out
}

// ── feed / plunge ───────────────────────────────────────────────────

fn feed_checks(scope: &Scope, feed: Option<f64>, plunge: Option<f64>) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    // S11: an unset feed has no ratio; `PRECOND_NOT_SET` names it.
    let (Some(feed), Some(plunge)) = (feed, plunge) else {
        return out;
    };
    if plunge > feed && feed > 0.0 {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::GEOM_PLUNGE_EXCEEDS_FEED),
            scope: scope.clone(),
            category: Category::Safety,
            severity: Severity::Caution,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            face: crate::diagnostics::presentation::face(ids::GEOM_PLUNGE_EXCEEDS_FEED),
            anchor: crate::diagnostics::presentation::anchor(ids::GEOM_PLUNGE_EXCEEDS_FEED),
            message: format!(
                "The plunge rate ({plunge:.0} mm/min) is more than the feed rate ({feed:.0} \
                 mm/min). A plunge rate is usually 30–50 % of the feed rate."
            ),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }
    out
}

// ── heights cross-validation ────────────────────────────────────────

/// An operation whose own depth dial puts its cut floor below the stock.
///
/// F1.6 shipped this rule GUI-side, so it reached the inspector ribbon and
/// nothing else — not the Operations card row, and not MCP
/// `get_toolpath_diagnostics`, which routes through
/// [`crate::diagnostics::diagnose_toolpath_inputs`]. F1.18 moves the
/// predicate here so one rule serves every surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DepthBeyondStock {
    /// How far below the stock bottom the cut floor sits, in mm. Always
    /// above [`DEPTH_BEYOND_STOCK_EPS_MM`].
    pub excess_mm: f64,
    /// Stock top minus stock bottom, in mm.
    pub stock_thickness_mm: f64,
    /// The floor the operation emits: resolved Top Z minus its depth dial.
    pub cut_floor_z: f64,
    /// The stock bottom this compares against.
    pub stock_bottom_z: f64,
}

/// A through cut landing exactly on the stock bottom is the normal way to cut
/// a part out, so the rule needs a tolerance rather than a strict compare.
/// This is an equality epsilon, not a machining allowance.
pub const DEPTH_BEYOND_STOCK_EPS_MM: f64 = 1e-6;

impl DepthBeyondStock {
    /// The operator-facing sentence. Kept identical to the GUI rule F1.6
    /// shipped, so the switchover does not change what the operator reads.
    pub fn message(&self) -> String {
        format!(
            "The cut goes {:.2} mm below the bottom of the stock",
            self.excess_mm
        )
    }
}

/// Whether [`depth_beyond_stock`] can answer for this operation at all.
///
/// It answers for operations whose cut floor is `resolved Top Z` minus their
/// OWN depth dial. Three groups are excluded, and each `false` here means
/// **NOT MEASURED** — never "this operation stays inside the board":
///
/// * The operations whose floor is the Heights Bottom Z —
///   `Adaptive3d`, `UnifiedFinish`, `Waterline`
///   ([`crate::compute::catalog::FloorOwner::HeightsBottom`]).
///   Their floor CAN be a pinned Bottom Z, and this adapter's snapshot
///   carries no pin FLAG: [`ResolvedHeights`] holds five numbers and no
///   record of which of them the operator set, whichever constructor built
///   it. Answering for them would mean guessing.
/// * The surface-riding finish family, where the mesh is the floor and
///   `depth_semantics()` is `DepthSemantics::None`.
/// * `ProjectCurve`, whose depth drops below the MESH surface rather than the
///   stock top, and `AlignmentPinDrill`, whose whole purpose is to reach into
///   the spoilboard.
///
/// `Drill` is included: it has a depth dial anchored at the top and no
/// spoilboard allowance of its own (`DrillConfig`).
///
/// P3.2: Pocket, Profile, Zigzag and Adaptive 2D
/// ([`crate::compute::catalog::FloorOwner::OpFloor`]) are answered for with
/// every floor reference: their `ResolvedHeights::bottom_z` IS the floor.
pub fn depth_beyond_stock_applies(op: &OperationConfig) -> bool {
    let caps = op.capabilities();
    if !caps.floor_inside_stock {
        return false;
    }
    match caps.floor_owner {
        // Subsumed today — all three of Adaptive3d / UnifiedFinish /
        // Waterline declare `DepthSemantics::None`, so the `Explicit`
        // requirement below already excludes them. The arm states the
        // REASON they cannot be answered for: their floor can be a pinned
        // Bottom Z, and this snapshot carries no pin flag.
        crate::compute::catalog::FloorOwner::HeightsBottom => return false,
        // P3.2: every floor reference of these four ops is answered for.
        crate::compute::catalog::FloorOwner::OpFloor => return true,
        crate::compute::catalog::FloorOwner::OpDial => {}
    }
    // `Explicit(_)` and nothing else. `DepthSemantics::None` is the
    // surface-riding family, where the mesh is the floor. `DerivedStockTop`
    // is `DropCutter`, whose `min_z` CLAMPS a surface-riding descent rather
    // than commanding a floor — a `min_z` below the board does not mean the
    // tool reaches it. The GUI rule F1.6 shipped requires `Explicit(_)` for
    // the same reason, so the two agree on WHICH operations they answer for
    // and differ only on the pinned Bottom Z.
    matches!(
        op.depth_semantics(),
        crate::compute::catalog::DepthSemantics::Explicit(_)
    )
}

/// Compare the operation's emitted cut floor with the bottom of the stock.
///
/// `None` covers three states and the caller must not read it as "clean":
/// the operation is outside [`depth_beyond_stock_applies`]; the caller
/// supplied no stock span (`ResolvedHeights::stock_bottom_z` is `None`); or
/// the floor is at or above the stock bottom, which is the only one of the
/// three that means clean. Call [`depth_beyond_stock_applies`] to separate
/// the first from the other two.
///
/// **The pinned Bottom Z is deliberately not part of this comparison.** For
/// every `OpDial` operation this rule applies to, a pinned bottom reaches no
/// emitted motion (F1.19), so folding it in would caution on a number the
/// machine never cuts. An `OpFloor` op refuses a pin; its bottom is its
/// floor. The GUI rule F1.6 shipped takes the deeper of the two bottoms;
/// this one takes only the one that becomes motion, and that is the single
/// behavioural difference between them.
pub fn depth_beyond_stock(op: &OperationConfig, h: &ResolvedHeights) -> Option<DepthBeyondStock> {
    if !depth_beyond_stock_applies(op) {
        return None;
    }
    let stock_top_z = h.stock_top_z?;
    let stock_bottom_z = h.stock_bottom_z?;
    // UX-1: the floor of an `OpFloor` op, when the snapshot holds THE
    // session floor. Under a face pick it measures from the face top while
    // `top_z` is the stock top, so `top - depth` would be wrong there.
    // P3.2: a floor that names another reference than the Top Z
    // (`DepthSemantics::Resolved`) is the resolved bottom of the session
    // heights. Every other depth measures from the depth base (UX-1b:
    // the face top under a face pick on Face or Trace; else the top).
    let op_floor_z = h.op_floor_z.filter(|_| {
        matches!(
            op.capabilities().floor_owner,
            crate::compute::catalog::FloorOwner::OpFloor
        )
    });
    let cut_floor_z = op_floor_z.unwrap_or_else(|| match op.depth_semantics() {
        crate::compute::catalog::DepthSemantics::Resolved => h.bottom_z,
        crate::compute::catalog::DepthSemantics::Explicit(_)
        | crate::compute::catalog::DepthSemantics::DerivedStockTop(_)
        | crate::compute::catalog::DepthSemantics::None
        | crate::compute::catalog::DepthSemantics::Unset => {
            h.depth_base_z - op.default_depth_for_heights().abs()
        }
    });
    let excess_mm = stock_bottom_z - cut_floor_z;
    if excess_mm <= DEPTH_BEYOND_STOCK_EPS_MM {
        return None;
    }
    Some(DepthBeyondStock {
        excess_mm,
        stock_thickness_mm: stock_top_z - stock_bottom_z,
        cut_floor_z,
        stock_bottom_z,
    })
}

/// F033: the plane-order findings. The verdict is THE core rule
/// ([`crate::compute::config::height_order`]: the R7 anchor, the bottom only
/// where it reaches the motion); this adapter converts each violation into a
/// finding with its own id and the one severity of its kind. The GUI badge
/// reads the same rule through `ToolpathHeights::height_order`.
fn height_order_findings(
    scope: &Scope,
    op: &OperationConfig,
    h: &ResolvedHeights,
) -> Vec<Diagnostic> {
    crate::compute::config::height_order(op.op_type(), &h.planes(), h.stock_top_z)
        .into_iter()
        .map(|violation| {
            let id = violation.id();
            Diagnostic {
                id: DiagnosticId::from(id),
                scope: scope.clone(),
                category: Category::Geometry,
                severity: height_order_severity(violation),
                confidence: Confidence::Static,
                state: DiagnosticState::Current,
                source: Source::StaticValidation,
                face: crate::diagnostics::presentation::face(id),
                anchor: crate::diagnostics::presentation::anchor(id),
                message: format!("{violation}."),
                evidence: None,
                fix: None,
                supersedes: vec![],
                suppressed_diagnostics: vec![],
            }
        })
        .collect()
}

/// F033: the one severity of a plane-order violation. A violation that
/// refuses the edit, generation and export is `Critical`; the other kinds
/// are `Caution`. The GUI badge maps the same split to its colours.
#[must_use]
pub fn height_order_severity(violation: crate::compute::config::HeightOrderViolation) -> Severity {
    if violation.refuses() {
        Severity::Critical
    } else {
        Severity::Caution
    }
}

fn heights_checks(scope: &Scope, op: &OperationConfig, h: &ResolvedHeights) -> Vec<Diagnostic> {
    let mut out = height_order_findings(scope, op, h);
    if let Some(found) = depth_beyond_stock(op, h) {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::GEOM_DEPTH_BEYOND_STOCK),
            scope: scope.clone(),
            category: Category::Safety,
            severity: Severity::Caution,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            face: crate::diagnostics::presentation::face(ids::GEOM_DEPTH_BEYOND_STOCK),
            anchor: crate::diagnostics::presentation::anchor(ids::GEOM_DEPTH_BEYOND_STOCK),
            message: found.message(),
            // The GUI rule F1.6 shipped carried this evidence line and the
            // inspector ribbon renders it (`ui/properties/mod.rs`, the
            // `GeometryCompare` arm). F1.18's switchover deletes that rule,
            // so the line is carried here instead of being lost. The left
            // label is `cut_floor_z`, not the old `bottom_z`: the value is
            // the floor the operation emits, never the Heights tab's Bottom
            // Z, and the old spelling is the confusion J7 removed.
            evidence: Some(DiagnosticEvidence::GeometryCompare {
                lhs_label: "cut_floor_z".to_owned(),
                lhs_value: found.cut_floor_z,
                rhs_label: "stock_bottom_z".to_owned(),
                rhs_value: found.stock_bottom_z,
                unit: "mm".to_owned(),
            }),
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }
    out
}
