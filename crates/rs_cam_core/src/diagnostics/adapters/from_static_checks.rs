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
    /// [`Self::from_heights`] instead. N4 (2026-09-10) moved the
    /// session route across for exactly that reason.
    pub fn from_context(ctx: &HeightContext) -> Self {
        Self {
            top_z: ctx.stock_top_z,
            bottom_z: ctx.stock_bottom_z,
            feed_z: ctx.stock_top_z,
            retract_z: ctx.safe_z,
            clearance_z: ctx.safe_z,
            stock_top_z: Some(ctx.stock_top_z),
            stock_bottom_z: Some(ctx.stock_bottom_z),
        }
    }

    /// Build from the toolpath's own [`HeightsConfig`], resolved against
    /// `ctx` (N4, 2026-09-10).
    ///
    /// This is the constructor for every caller that holds a
    /// `HeightsConfig`. The snapshot carries the planes the operator
    /// pinned, so `heights_checks` compares the heights the machine
    /// gets and [`depth_beyond_stock`] reads a pinned Top Z.
    ///
    /// The snapshot still carries no pin FLAG: it holds five numbers and
    /// no record of which of them the operator set. That is why
    /// [`depth_beyond_stock_applies`] abstains for the three operations
    /// whose floor can be a pinned Bottom Z.
    ///
    /// The stock span is `Some` because the context always carries it.
    pub fn from_heights(heights: &HeightsConfig, ctx: &HeightContext) -> Self {
        let resolved = heights.resolve(ctx);
        Self {
            top_z: resolved.top_z,
            bottom_z: resolved.bottom_z,
            feed_z: resolved.feed_z,
            retract_z: resolved.retract_z,
            clearance_z: resolved.clearance_z,
            stock_top_z: Some(ctx.stock_top_z),
            stock_bottom_z: Some(ctx.stock_bottom_z),
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
        scope: Scope::Tool { id: tool.id.0 },
        category: Category::Geometry,
        severity: Severity::Info,
        confidence: Confidence::Static,
        state: DiagnosticState::Current,
        source: Source::StaticValidation,
        message: format!("Tool \"{}\": {caution}", tool.name),
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
        && matches!(
            op,
            OperationConfig::Pocket(_) | OperationConfig::Face(_) | OperationConfig::Zigzag(_)
        )
    {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::COMPAT_BALL_NOSE_FLAT_CLEARING),
            scope: scope.clone(),
            category: Category::Quality,
            severity: Severity::Hint,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            message:
                "Ball nose tools leave scallops on flat surfaces. Consider a flat end mill for clearing."
                    .to_owned(),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }

    // End mill on Scallop/UnifiedFinish/Pencil → required ball geometry.
    if matches!(tool_type, ToolType::EndMill)
        && matches!(
            op,
            OperationConfig::Scallop(_)
                | OperationConfig::UnifiedFinish(_)
                | OperationConfig::Pencil(_)
        )
    {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::COMPAT_END_MILL_SCALLOP_PENCIL),
            scope: scope.clone(),
            category: Category::ToolLoad,
            severity: Severity::Critical,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            message:
                "Scallop, Unified Finish, and Pencil operations require a ball nose tool for correct surface contact."
                    .to_owned(),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }

    out
}

// ── stepover ────────────────────────────────────────────────────────

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

    if stepover > diameter {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::GEOM_STEPOVER_EXCEEDS_DIAMETER),
            scope: scope.clone(),
            category: Category::Geometry,
            severity: Severity::Critical,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            message: format!(
                "Stepover ({stepover:.2} mm) exceeds tool diameter ({diameter:.1} mm) \
                 — will leave uncut strips."
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
            message: format!(
                "Stepover is {:.0}% of tool diameter — may leave visible scallops.",
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
            message: "Very fine stepover — cycle time will be significantly longer.".to_owned(),
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
            message: format!(
                "Stepover ({stepover:.2} mm) is >{:.0}% of tool diameter. Finish quality may suffer.",
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
                        message: format!(
                            "Scallop height {scallop_h:.3} mm at this stepover. \
                             Reduce stepover for a smoother finish."
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

    if tool.cutting_length > 0.0 && dpp > tool.cutting_length {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::GEOM_DPP_EXCEEDS_CUTTING_LENGTH),
            scope: scope.clone(),
            category: Category::Safety,
            severity: Severity::Critical,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            message: format!(
                "Depth per pass ({dpp:.1} mm) exceeds cutting length ({:.1} mm). \
                 Tool shank will contact material.",
                tool.cutting_length
            ),
            evidence: Some(DiagnosticEvidence::GeometryCompare {
                lhs_label: "depth_per_pass".to_owned(),
                lhs_value: dpp,
                rhs_label: "cutting_length".to_owned(),
                rhs_value: tool.cutting_length,
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
            message: format!(
                "Depth per pass ({dpp:.1} mm) is {:.1}× the tool diameter ({:.1} mm). \
                 The vendors print the depth de-rate to {last_ratio:.0}× diameter only. \
                 The feed and the chipload band hold the last printed point, 50 %.",
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

fn feed_checks(scope: &Scope, feed: f64, plunge: f64) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if plunge > feed && feed > 0.0 {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::GEOM_PLUNGE_EXCEEDS_FEED),
            scope: scope.clone(),
            category: Category::Safety,
            severity: Severity::Caution,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            message: format!(
                "Plunge rate ({plunge:.0}) exceeds feed rate ({feed:.0}). \
                 Unusual — plunge is typically 30–50% of feed."
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
        format!("Depth exceeds stock thickness by {:.2} mm", self.excess_mm)
    }
}

/// Whether [`depth_beyond_stock`] can answer for this operation at all.
///
/// It answers for operations whose cut floor is `resolved Top Z` minus their
/// OWN depth dial. Three groups are excluded, and each `false` here means
/// **NOT MEASURED** — never "this operation stays inside the board":
///
/// * The operations that read `ResolvedHeights::bottom_z` as their floor —
///   `Adaptive3d`, `UnifiedFinish`, `Waterline`
///   ([`crate::compute::catalog::OperationType::honors_pinned_bottom_z`]).
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
pub fn depth_beyond_stock_applies(op: &OperationConfig) -> bool {
    use crate::compute::catalog::OperationType::{AlignmentPinDrill, ProjectCurve};
    // Subsumed today — all three of Adaptive3d / UnifiedFinish / Waterline
    // declare `DepthSemantics::None`, so the `Explicit` requirement below
    // already excludes them. It is kept because it states the REASON they
    // cannot be answered for, which the `Explicit` check does not: their
    // floor can be a pinned Bottom Z, and this snapshot carries no pin flag.
    if op.op_type().honors_pinned_bottom_z() {
        return false;
    }
    if matches!(op.op_type(), ProjectCurve | AlignmentPinDrill) {
        return false;
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
/// every operation this rule applies to, a pinned bottom reaches no emitted
/// motion (F1.19), so folding it in would caution on a number the machine
/// never cuts. The GUI rule F1.6 shipped takes the deeper of the two bottoms;
/// this one takes only the one that becomes motion, and that is the single
/// behavioural difference between them.
pub fn depth_beyond_stock(op: &OperationConfig, h: &ResolvedHeights) -> Option<DepthBeyondStock> {
    if !depth_beyond_stock_applies(op) {
        return None;
    }
    let stock_top_z = h.stock_top_z?;
    let stock_bottom_z = h.stock_bottom_z?;
    let cut_floor_z = h.top_z - op.default_depth_for_heights().abs();
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

/// The Top Z the generator ANCHORS on, for the plane-order check.
///
/// R7 (Corne case, 2026-09-18): the as-found 6 mm rough carried a pinned
/// `top_z = 0.109` (an un-rounded Heights-diagram drag, R5) and a pinned
/// `bottom_z = 4.0`, and the ribbon said "Bottom Z (4.0) is above Top Z
/// (0.1). No material will be cut." The numbers were the config's; the
/// sentence was false. `Adaptive3d` deliberately ignores a pinned top and
/// anchors on the stock top (`generate_adaptive3d`, `stock_top_z:
/// ctx.stock_bbox.max.z`), so a pinned bottom on a rough must be compared
/// against the stock top the rough really starts from. Every other operation
/// reads its resolved `top_z`.
fn anchored_top_z(op: &OperationConfig, h: &ResolvedHeights) -> f64 {
    if op.op_type() == crate::compute::catalog::OperationType::Adaptive3d {
        h.stock_top_z.unwrap_or(h.top_z)
    } else {
        h.top_z
    }
}

fn heights_checks(scope: &Scope, op: &OperationConfig, h: &ResolvedHeights) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let top_z = anchored_top_z(op, h);
    if h.bottom_z > top_z {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::GEOM_BOTTOM_ABOVE_TOP_Z),
            scope: scope.clone(),
            category: Category::Geometry,
            severity: Severity::Critical,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            message: format!(
                "Bottom Z ({:.1}) is above Top Z ({:.1}). No material will be cut.",
                h.bottom_z, top_z
            ),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }
    if h.feed_z < top_z {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::GEOM_FEED_Z_BELOW_TOP_Z),
            scope: scope.clone(),
            category: Category::Geometry,
            severity: Severity::Caution,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            message: format!(
                "Feed Z ({:.1}) is below Top Z ({:.1}). Tool will plunge into material at feed rate.",
                h.feed_z, top_z
            ),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }
    if h.retract_z < h.feed_z {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::GEOM_RETRACT_Z_BELOW_FEED_Z),
            scope: scope.clone(),
            category: Category::Geometry,
            severity: Severity::Caution,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            message: "Retract Z is below Feed Z. Retract moves won't clear the approach height."
                .to_owned(),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }
    if h.clearance_z < h.retract_z {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::GEOM_CLEARANCE_Z_BELOW_RETRACT_Z),
            scope: scope.clone(),
            category: Category::Geometry,
            severity: Severity::Caution,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            message: "Clearance Z is below Retract Z. Rapid moves between operations may collide."
                .to_owned(),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }
    if let Some(found) = depth_beyond_stock(op, h) {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::GEOM_DEPTH_BEYOND_STOCK),
            scope: scope.clone(),
            category: Category::Safety,
            severity: Severity::Caution,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
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
