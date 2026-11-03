//! Adapter: op-precondition static checks → [`Diagnostic`] list.
//!
//! Generate-time errors lifted into static validation so the user sees
//! the problem on the Params tab *before* clicking Generate. Before
//! this adapter (F-015), these op families had preconditions that
//! only fired at generation time:
//!
//! - **Drill / AlignmentPinDrill** — needs at least one hole position
//!   (for Drill a point or circle of its selection, G-DRILLCENTROID; for
//!   AlignmentPinDrill a pin on the live stock or a hole of its
//!   selection — the two sources the generator drills). The selection
//!   verdict comes from the session (`ProjectSession::hole_selection`,
//!   the resolution generation reads); this adapter only converts it.
//! - **Every other op with a selection** — its selection refuses
//!   (geometry selection P2.4, `PRECOND_SELECTION_REFUSED`): the
//!   generator's own refusal sentence, from `selection_status`.
//! - **Chamfer / V-Carve / Inlay / Inlay Plug** (UX-1c,
//!   `PRECOND_BURIED_FACE`) — a face pick below the stock top on fresh
//!   stock: the generator's own refusal sentence, from
//!   `ToolpathHeights::buried_face_refusal`.
//! - **Every op** (S11, `PRECOND_NOT_SET`) — a value with no derivation
//!   that is not set: the floor, a depth of a drawing op, or the feeds.
//! - **Every op** (R2, `PRECOND_NO_TOOL`) — the op has no tool: the
//!   no-tool state and the fix.
//! - **ProjectCurve** — needs both a source curve (the toolpath's
//!   `model_id` must point at a polygon-bearing model) AND a target
//!   surface mesh (either the same model or any other loaded model).
//!
//! Each precondition emits a `Severity::Blocking` diagnostic so the
//! params banner surfaces it as actionable and the export gate refuses
//! to proceed. Confidence is `Static` (config-only check, no sim
//! required) and `state` is `Current`.

use crate::compute::catalog::OperationConfig;
use crate::diagnostics::{
    Category, Confidence, Diagnostic, DiagnosticId, DiagnosticState, Scope, Severity, Source, ids,
};
use crate::ids::ToolpathId;

/// Per-toolpath, per-session context the precondition adapter needs.
///
/// Built by the session layer (see
/// [`crate::session::ProjectSession::diagnose_toolpath_with_trace`]) so
/// the adapter stays a pure function. The context describes the
/// neighbouring toolpaths and model geometry the current toolpath
/// depends on — anything the runtime would have looked at when it
/// raised a generate-time error.
#[derive(Debug, Clone, Default)]
pub struct PreconditionContext {
    /// Geometry summary of the model this toolpath targets (the
    /// toolpath's `model_id`). `None` if the model can't be resolved
    /// (e.g. broken reference).
    pub target_model: Option<TargetModelGeometry>,
    /// True when at least one loaded model in the session carries a
    /// triangle mesh. Used by the ProjectCurve check to decide whether a
    /// projection surface is available somewhere in the scene.
    pub any_loaded_model_has_mesh: bool,
    /// The diameter of each alignment pin of the live stock. The
    /// `AlignmentPinDrill` check reads this, never the op's `holes`
    /// snapshot: the generator replaces `holes` with the live stock pins
    /// of the op's diameter before it drills (R1), so the snapshot can be
    /// empty or stale.
    pub stock_alignment_pin_diameters: Vec<f64>,
    /// The count of the holes that a drill op drills from its own
    /// selection (geometry selection P2.3c). Zero for every other op.
    pub selection_holes: usize,
    /// The refusal that generation gives for the selection of the op,
    /// with the op name (`selection::view`, through
    /// `ProjectSession::selection_status`). `None` when the selection
    /// gives the op its items.
    pub selection_refusal: Option<String>,
    /// W3h: true when the op selection holds no rule, no pick and no
    /// exclude (a new op starts so). The selection finding of a Drill and
    /// of every other op that is not a Pin Drill is then
    /// `PRECOND_SELECTION_EMPTY`, not the refusal id.
    pub selection_empty: bool,
    /// UX-1c: the refusal that generation gives for a face pick below the
    /// stock top on a one-depth op on fresh stock
    /// (`ToolpathHeights::buried_face_refusal`). `None` when the op does
    /// not refuse.
    pub buried_face_refusal: Option<String>,
    /// S11: the refusal of every op while the stock thickness is not set
    /// (`ProjectSession::stock_thickness_refusal`). `None` when it is set.
    pub stock_thickness_refusal: Option<String>,
    /// R2: the no-tool state of the op (`ProjectSession::no_tool_state`),
    /// or `None` when the op has a tool.
    pub no_tool_state: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TargetModelGeometry {
    pub has_polygons: bool,
    pub has_mesh: bool,
}

/// Run every precondition check against a single toolpath, returning a
/// flat diagnostic list.
pub fn diagnostics_from_preconditions(
    toolpath_id: ToolpathId,
    op: &OperationConfig,
    ctx: &PreconditionContext,
) -> Vec<Diagnostic> {
    let scope = Scope::Toolpath { id: toolpath_id };
    let mut out = Vec::new();
    match op {
        OperationConfig::Drill(_) => {
            out.extend(drill_checks(&scope, ctx));
        }
        OperationConfig::AlignmentPinDrill(cfg) => {
            // The generator drills the live stock pins plus the holes of
            // the selection, so this check reads the same two inputs.
            let has_pins_of_size = ctx
                .stock_alignment_pin_diameters
                .iter()
                .any(|d| crate::compute::alignment_pins::same_pin_diameter(*d, cfg.diameter));
            let message = match &ctx.selection_refusal {
                Some(refusal) => Some(format!("{refusal} — generation will fail.")),
                None if !has_pins_of_size && ctx.selection_holes == 0 => Some(format!(
                    "Alignment pin drill has no hole positions: no stock pin is \
                     {d:.2} mm. Add alignment pins of that size to the stock.",
                    d = cfg.diameter
                )),
                None => None,
            };
            if let Some(message) = message {
                out.push(Diagnostic {
                    id: DiagnosticId::from(ids::PRECOND_ALIGNMENT_PIN_DRILL_NO_HOLES),
                    scope: scope.clone(),
                    category: Category::Geometry,
                    severity: Severity::Blocking,
                    confidence: Confidence::Static,
                    state: DiagnosticState::Current,
                    source: Source::StaticValidation,
                    face: crate::diagnostics::presentation::face(
                        ids::PRECOND_ALIGNMENT_PIN_DRILL_NO_HOLES,
                    ),
                    anchor: crate::diagnostics::presentation::anchor(
                        ids::PRECOND_ALIGNMENT_PIN_DRILL_NO_HOLES,
                    ),
                    message,
                    evidence: None,
                    fix: None,
                    supersedes: vec![],
                    suppressed_diagnostics: vec![],
                });
            }
        }
        OperationConfig::ProjectCurve(_) => {
            out.extend(project_curve_checks(&scope, ctx));
            // With no curve at all, the no-curve rule above says it once.
            if ctx.target_model.is_some_and(|m| m.has_polygons) {
                out.extend(selection_refused(&scope, ctx));
            }
        }
        OperationConfig::Face(_)
        | OperationConfig::Pocket(_)
        | OperationConfig::Profile(_)
        | OperationConfig::Adaptive(_)
        | OperationConfig::VCarve(_)
        | OperationConfig::Inlay(_)
        | OperationConfig::InlayPlug(_)
        | OperationConfig::Zigzag(_)
        | OperationConfig::Trace(_)
        | OperationConfig::Bore(_)
        | OperationConfig::Chamfer(_)
        | OperationConfig::DropCutter(_)
        | OperationConfig::Adaptive3d(_)
        | OperationConfig::Waterline(_)
        | OperationConfig::Pencil(_)
        | OperationConfig::Scallop(_)
        | OperationConfig::UnifiedFinish(_)
        | OperationConfig::SteepShallow(_)
        | OperationConfig::RampFinish(_)
        | OperationConfig::SpiralFinish(_)
        | OperationConfig::RadialFinish(_)
        | OperationConfig::HorizontalFinish(_) => {
            out.extend(selection_refused(&scope, ctx));
        }
    }
    out.extend(buried_face(&scope, ctx));
    out.extend(not_set(&scope, op, ctx));
    out.extend(no_tool(&scope, ctx));
    out
}

/// R2 (Fusion): the op has no tool. The message is the no-tool state and
/// the fix, as the generation refusal (`SessionError::NoTool`) gives them.
fn no_tool(scope: &Scope, ctx: &PreconditionContext) -> Option<Diagnostic> {
    let state = ctx.no_tool_state.as_ref()?;
    Some(Diagnostic {
        id: DiagnosticId::from(ids::PRECOND_NO_TOOL),
        scope: scope.clone(),
        category: Category::Geometry,
        severity: Severity::Blocking,
        confidence: Confidence::Static,
        state: DiagnosticState::Current,
        source: Source::StaticValidation,
        face: crate::diagnostics::presentation::face(ids::PRECOND_NO_TOOL),
        anchor: crate::diagnostics::presentation::anchor(ids::PRECOND_NO_TOOL),
        message: format!(
            "{state}. {} — generation will fail.",
            crate::session::CHOOSE_A_TOOL_FIX
        ),
        evidence: None,
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    })
}

/// S11 (operator ruling 2026-10-23): the values of the op that no rule
/// gives and that are not set. The message is the generator's own refusal
/// sentence, in the order generation refuses: the stock thickness, the
/// floor (`FloorRefusal::Unset`), else the depth or the feeds
/// (`OperationConfig::not_set`).
fn not_set(scope: &Scope, op: &OperationConfig, ctx: &PreconditionContext) -> Option<Diagnostic> {
    use crate::compute::not_set::NotSetKind;
    use crate::diagnostics::presentation::{Anchor, FieldKey};
    // W1: the face and the anchor name the value that is not set.
    let field =
        |name: &str| FieldKey::parse(name).map_or(Anchor::Operation, |key| Anchor::Field { key });
    let (message, face, anchor, category) = if let Some(text) = &ctx.stock_thickness_refusal {
        (
            text.clone(),
            "Stock thickness not set".to_owned(),
            Anchor::Project,
            Category::Geometry,
        )
    } else if op.floor().is_some_and(|floor| floor.is_unset()) {
        (
            format!(
                "{}: {}",
                op.op_type().label(),
                crate::compute::floor::FLOOR_UNSET_DETAIL
            ),
            "Floor not set".to_owned(),
            // W3 fix: the Bottom height row is THE floor editor of an
            // `OpFloor` op (D6), so the finding anchors to `bottom_z`.
            field("bottom_z"),
            Category::Geometry,
        )
    } else {
        let not_set = op.not_set()?;
        let anchor = not_set.first_field().map_or(Anchor::Operation, field);
        // The feeds are a tool-load fault, a depth a geometry fault.
        let (face, category) = match not_set.kind {
            NotSetKind::Depth => ("Depth not set", Category::Geometry),
            NotSetKind::Feeds => ("Feed and spindle not set", Category::ToolLoad),
        };
        (not_set.detail(), face.to_owned(), anchor, category)
    };
    Some(Diagnostic {
        id: DiagnosticId::from(ids::PRECOND_NOT_SET),
        scope: scope.clone(),
        category,
        severity: Severity::Blocking,
        confidence: Confidence::Static,
        state: DiagnosticState::Current,
        source: Source::StaticValidation,
        face,
        anchor,
        message: format!("{message} — generation will fail."),
        evidence: None,
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    })
}

/// UX-1c (operator ruling 2026-10-22, SAFETY): a one-depth op on fresh
/// stock whose face pick is below the stock top. The message is the
/// generator's own refusal sentence (the cause and the fix).
fn buried_face(scope: &Scope, ctx: &PreconditionContext) -> Option<Diagnostic> {
    let message = ctx.buried_face_refusal.as_ref()?;
    Some(Diagnostic {
        id: DiagnosticId::from(ids::PRECOND_BURIED_FACE),
        scope: scope.clone(),
        category: Category::Safety,
        severity: Severity::Blocking,
        confidence: Confidence::Static,
        state: DiagnosticState::Current,
        source: Source::StaticValidation,
        face: crate::diagnostics::presentation::face(ids::PRECOND_BURIED_FACE),
        anchor: crate::diagnostics::presentation::anchor(ids::PRECOND_BURIED_FACE),
        message: format!("{message} — generation will fail."),
        evidence: None,
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    })
}

/// Geometry selection P2.4: the selection of an op that is not a drill
/// refuses. The message is the generator's own refusal sentence (the
/// item, the cause and the fix), from the session's `selection_status`.
fn selection_refused(scope: &Scope, ctx: &PreconditionContext) -> Option<Diagnostic> {
    let message = ctx.selection_refusal.as_ref()?;
    if ctx.selection_empty {
        return Some(selection_empty(scope));
    }
    Some(Diagnostic {
        id: DiagnosticId::from(ids::PRECOND_SELECTION_REFUSED),
        scope: scope.clone(),
        category: Category::Geometry,
        severity: Severity::Blocking,
        confidence: Confidence::Static,
        state: DiagnosticState::Current,
        source: Source::StaticValidation,
        face: crate::diagnostics::presentation::face(ids::PRECOND_SELECTION_REFUSED),
        anchor: crate::diagnostics::presentation::anchor(ids::PRECOND_SELECTION_REFUSED),
        message: format!("{message} — generation will fail."),
        evidence: None,
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    })
}

/// W3h: the op selection holds nothing (a new op starts so). The face
/// asks for the pick; the selection list header shows it.
fn selection_empty(scope: &Scope) -> Diagnostic {
    Diagnostic {
        id: DiagnosticId::from(ids::PRECOND_SELECTION_EMPTY),
        scope: scope.clone(),
        category: Category::Geometry,
        severity: Severity::Blocking,
        confidence: Confidence::Static,
        state: DiagnosticState::Current,
        source: Source::StaticValidation,
        face: crate::diagnostics::presentation::face(ids::PRECOND_SELECTION_EMPTY),
        anchor: crate::diagnostics::presentation::anchor(ids::PRECOND_SELECTION_EMPTY),
        message: "Nothing is selected. Pick the items to cut in the viewport, or add a rule \
                  on the Geometry tab — generation will fail."
            .to_owned(),
        evidence: None,
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    }
}

fn drill_checks(scope: &Scope, ctx: &PreconditionContext) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    // Drill drills the points and circles of its selection. A polygon
    // outline is not a hole source (G-DRILLCENTROID). The session gives
    // the generator's own refusal sentence for the selection.
    if ctx.selection_refusal.is_some() && ctx.selection_empty {
        out.push(selection_empty(scope));
    } else if let Some(message) = &ctx.selection_refusal {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::PRECOND_DRILL_NO_HOLES),
            scope: scope.clone(),
            category: Category::Geometry,
            severity: Severity::Blocking,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            face: crate::diagnostics::presentation::face(ids::PRECOND_DRILL_NO_HOLES),
            anchor: crate::diagnostics::presentation::anchor(ids::PRECOND_DRILL_NO_HOLES),
            message: format!("{message} — generation will fail."),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }
    out
}

fn project_curve_checks(scope: &Scope, ctx: &PreconditionContext) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let target_polys = ctx.target_model.map(|m| m.has_polygons).unwrap_or(false);
    let target_mesh = ctx.target_model.map(|m| m.has_mesh).unwrap_or(false);

    if !target_polys {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::PRECOND_PROJECT_CURVE_NO_CURVE),
            scope: scope.clone(),
            category: Category::Geometry,
            severity: Severity::Blocking,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            face: crate::diagnostics::presentation::face(ids::PRECOND_PROJECT_CURVE_NO_CURVE),
            anchor: crate::diagnostics::presentation::anchor(ids::PRECOND_PROJECT_CURVE_NO_CURVE),
            message: "Project Curve needs a curve to follow. The selected model has no 2D \
                      curves. Import an SVG or DXF file with the curve, and select it as the \
                      model of this toolpath."
                .to_owned(),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }

    // A surface mesh must exist somewhere — either the same model
    // (rare: 2D + mesh combined) or another loaded model.
    if !target_mesh && !ctx.any_loaded_model_has_mesh {
        out.push(Diagnostic {
            id: DiagnosticId::from(ids::PRECOND_PROJECT_CURVE_NO_SURFACE),
            scope: scope.clone(),
            category: Category::Geometry,
            severity: Severity::Blocking,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::StaticValidation,
            face: crate::diagnostics::presentation::face(ids::PRECOND_PROJECT_CURVE_NO_SURFACE),
            anchor: crate::diagnostics::presentation::anchor(ids::PRECOND_PROJECT_CURVE_NO_SURFACE),
            message: "Project Curve needs a 3D surface to project the curve on. The \
                      project has no 3D model. Import the surface (for example an STL file) \
                      first."
                .to_owned(),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        });
    }

    out
}

#[cfg(test)]
#[expect(
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;
    use crate::compute::operation_configs::{
        AlignmentPinDrillConfig, DrillConfig, ProjectCurveConfig,
    };

    fn drill_op() -> OperationConfig {
        // S11: the fixture states the values this check does not read.
        let mut op = OperationConfig::Drill(DrillConfig::default());
        crate::compute::fixture_values::fill_unset(&mut op);
        op
    }

    fn project_curve_op() -> OperationConfig {
        // S11: the fixture states the values this check does not read.
        let mut op = OperationConfig::ProjectCurve(ProjectCurveConfig::default());
        crate::compute::fixture_values::fill_unset(&mut op);
        op
    }

    fn alignment_pin_drill_op() -> OperationConfig {
        // S11: the fixture states the values this check does not read.
        let mut op = OperationConfig::AlignmentPinDrill(AlignmentPinDrillConfig::default());
        crate::compute::fixture_values::fill_unset(&mut op);
        op
    }

    /// The ribbon prints the refusal sentence the session gives for the
    /// selection, the same one the generator gives.
    #[test]
    fn drill_no_holes_fires_on_a_selection_refusal() {
        let ctx = PreconditionContext {
            selection_refusal: Some("Drill: nothing to cut".to_owned()),
            ..Default::default()
        };
        let diags = diagnostics_from_preconditions(ToolpathId::from_raw(0), &drill_op(), &ctx);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].id.as_str(), ids::PRECOND_DRILL_NO_HOLES);
        assert_eq!(diags[0].severity, Severity::Blocking);
        assert!(
            diags[0].message.starts_with("Drill: nothing to cut"),
            "got {:?}",
            diags[0].message
        );
    }

    /// W3h: an empty selection (a new op) gives the one "Select the
    /// geometry to cut" finding on the `selection` field, for a Drill and
    /// for a Pocket; a refusal of a selection that holds items keeps its
    /// own id.
    #[test]
    fn an_empty_selection_asks_for_the_pick_w3h() {
        let empty = PreconditionContext {
            selection_refusal: Some("nothing to cut".to_owned()),
            selection_empty: true,
            ..Default::default()
        };
        let pocket =
            OperationConfig::Pocket(crate::compute::operation_configs::PocketConfig::default());
        for op in [drill_op(), pocket] {
            let diags = diagnostics_from_preconditions(ToolpathId::from_raw(0), &op, &empty);
            let selection: Vec<&Diagnostic> = diags
                .iter()
                .filter(|d| {
                    d.anchor == crate::diagnostics::presentation::Anchor::field("selection")
                })
                .collect();
            assert_eq!(selection.len(), 1, "one selection finding: {diags:?}");
            assert_eq!(selection[0].id.as_str(), ids::PRECOND_SELECTION_EMPTY);
            assert_eq!(selection[0].face, "Select the geometry to cut");
            assert_eq!(
                selection[0].anchor,
                crate::diagnostics::presentation::Anchor::field("selection")
            );
        }
        let refused = PreconditionContext {
            selection_empty: false,
            ..empty
        };
        let diags = diagnostics_from_preconditions(ToolpathId::from_raw(0), &drill_op(), &refused);
        assert_eq!(diags[0].id.as_str(), ids::PRECOND_DRILL_NO_HOLES);
    }

    #[test]
    fn drill_silent_when_the_selection_gives_holes() {
        let ctx = PreconditionContext {
            selection_holes: 2,
            ..Default::default()
        };
        let diags = diagnostics_from_preconditions(ToolpathId::from_raw(0), &drill_op(), &ctx);
        assert!(diags.is_empty());
    }

    #[test]
    fn alignment_pin_drill_no_holes_fires_without_stock_pins_or_picks() {
        let ctx = PreconditionContext::default();
        let diags = diagnostics_from_preconditions(
            ToolpathId::from_raw(0),
            &alignment_pin_drill_op(),
            &ctx,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].id.as_str(),
            ids::PRECOND_ALIGNMENT_PIN_DRILL_NO_HOLES
        );
        assert_eq!(diags[0].severity, Severity::Blocking);
    }

    /// The live case: two pins on the stock. The generator drills the two
    /// stock pins, so the check stays silent.
    #[test]
    fn alignment_pin_drill_silent_with_stock_pins() {
        let ctx = PreconditionContext {
            stock_alignment_pin_diameters: vec![6.0, 6.0],
            ..Default::default()
        };
        let diags = diagnostics_from_preconditions(
            ToolpathId::from_raw(0),
            &alignment_pin_drill_op(),
            &ctx,
        );
        assert!(diags.is_empty(), "got {diags:?}");
    }

    #[test]
    fn alignment_pin_drill_silent_with_selected_holes_and_no_stock_pins() {
        let ctx = PreconditionContext {
            selection_holes: 1,
            ..Default::default()
        };
        let diags = diagnostics_from_preconditions(
            ToolpathId::from_raw(0),
            &alignment_pin_drill_op(),
            &ctx,
        );
        assert!(diags.is_empty(), "got {diags:?}");
    }

    /// A stale extra pick blocks a Pin Drill too, even with stock pins.
    #[test]
    fn alignment_pin_drill_fires_on_a_selection_refusal() {
        let ctx = PreconditionContext {
            stock_alignment_pin_diameters: vec![6.0],
            selection_refusal: Some("Alignment Pin Drill: a pick is lost".to_owned()),
            ..Default::default()
        };
        let diags = diagnostics_from_preconditions(
            ToolpathId::from_raw(0),
            &alignment_pin_drill_op(),
            &ctx,
        );
        assert_eq!(diags.len(), 1, "got {diags:?}");
        assert!(diags[0].message.contains("a pick is lost"));
    }

    #[test]
    fn project_curve_missing_curve_fires_when_target_has_only_mesh() {
        // Project has only the surface STL (mesh, no polygons).
        let ctx = PreconditionContext {
            target_model: Some(TargetModelGeometry {
                has_polygons: false,
                has_mesh: true,
            }),
            any_loaded_model_has_mesh: true,
            ..Default::default()
        };
        let diags =
            diagnostics_from_preconditions(ToolpathId::from_raw(0), &project_curve_op(), &ctx);
        assert!(
            diags
                .iter()
                .any(|d| d.id.as_str() == ids::PRECOND_PROJECT_CURVE_NO_CURVE),
            "got {:?}",
            diags
        );
    }

    #[test]
    fn project_curve_missing_surface_fires_when_only_curve_loaded() {
        // Project has only the SVG/DXF (polygons, no mesh anywhere).
        let ctx = PreconditionContext {
            target_model: Some(TargetModelGeometry {
                has_polygons: true,
                has_mesh: false,
            }),
            any_loaded_model_has_mesh: false,
            ..Default::default()
        };
        let diags =
            diagnostics_from_preconditions(ToolpathId::from_raw(0), &project_curve_op(), &ctx);
        assert!(
            diags
                .iter()
                .any(|d| d.id.as_str() == ids::PRECOND_PROJECT_CURVE_NO_SURFACE),
            "got {:?}",
            diags
        );
    }

    #[test]
    fn project_curve_silent_when_curve_and_surface_present() {
        let ctx = PreconditionContext {
            target_model: Some(TargetModelGeometry {
                has_polygons: true,
                has_mesh: false,
            }),
            any_loaded_model_has_mesh: true,
            ..Default::default()
        };
        let diags =
            diagnostics_from_preconditions(ToolpathId::from_raw(0), &project_curve_op(), &ctx);
        assert!(diags.is_empty(), "got {:?}", diags);
    }

    #[test]
    fn non_precondition_ops_emit_nothing() {
        let ctx = PreconditionContext::default();
        let pocket = crate::compute::fixture_values::fixture_op(
            crate::compute::catalog::OperationType::Pocket,
        );
        let diags = diagnostics_from_preconditions(ToolpathId::from_raw(0), &pocket, &ctx);
        assert!(diags.is_empty());
    }
}
