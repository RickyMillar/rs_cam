//! The issue stack under the header of the toolpath inspector (W2b):
//! THE finding list of the panel, the validator refusals that no core
//! finding states, and the rows of the stack.

use crate::ui::AppEvent;
use crate::ui::components::{Notice, NoticeStack, Role};
use crate::ui::properties::{ToolpathPanelInputs, operations};

/// W2b: THE finding list the toolpath inspector places: the core list of
/// the op (`collect_diagnostics`; with no tool, core's no-tool list) and
/// the shoe gap finding (H2: a project verdict about this toolpath). The
/// panel and the `properties::placed_findings` sentry door both call it.
pub(in crate::ui::properties) fn panel_diagnostics(
    entry: &crate::state::toolpath::ToolpathEntry,
    preconditions: &rs_cam_core::diagnostics::diagnose::PreconditionContext,
    model_refs: &rs_cam_core::diagnostics::diagnose::ModelRefContext,
    suggest_warnings: Option<&[rs_cam_core::feeds::suggest::SuggestWarning]>,
    inputs: &ToolpathPanelInputs,
) -> Vec<rs_cam_core::diagnostics::Diagnostic> {
    let tool = inputs
        .tool_configs
        .iter()
        .find(|(id, _)| Some(*id) == entry.tool_id)
        .map(|(_, tc)| tc);
    let toolpath_heights = inputs
        .toolpath_heights
        .as_ref()
        .and_then(|answer| answer.as_ref().ok());
    let mut diagnostics = operations::collect_diagnostics(
        entry,
        tool,
        &inputs.stale_default_defects,
        toolpath_heights,
        preconditions,
        model_refs,
        suggest_warnings,
        inputs.load_verdict(),
    );
    diagnostics.extend(inputs.shoe.as_ref().and_then(|shoe| shoe.finding.clone()));
    diagnostics
}

/// The validator text of a missing model; core states it as
/// `ref.model_missing` (an issue row of its own).
const SELECTED_MODEL_MISSING: &str = "Selected model is missing";

/// W6: the face of a last-run refusal whose text is over the face limit
/// (`FACE_MAX_CHARS`). The full refusal is the hover.
pub(in crate::ui::properties) const LAST_RUN_REFUSED: &str = "The last run was refused";

/// W6: the face of a validator refusal whose text is over the face limit.
pub(in crate::ui::properties) const INPUTS_REFUSED: &str = "A check of the inputs refuses this op";

/// A Danger issue row of a text with no short face of its own: the text
/// itself when it fits the face limit (`FACE_MAX_CHARS`), else `face` with
/// the text on hover.
fn refusal_notice(text: &str, face: &str) -> Notice {
    if text.chars().count() <= rs_cam_core::diagnostics::FACE_MAX_CHARS {
        Notice::new(Role::Danger, text)
    } else {
        Notice::new(Role::Danger, face).detail(text)
    }
}

/// What an issue row of the stack does when its Fix button is clicked.
enum IssueAct {
    None,
    /// Apply the stale-default fix of `stale_default_defects[index]`.
    StaleDefault(usize),
    /// Generate the source toolpath of a rest-boundary refusal.
    Generate(crate::state::toolpath::ToolpathId),
}

/// The validator refusals that no core finding states (W2b). Each other
/// refusal has ONE owner elsewhere: the no-tool text and the tool-kind
/// refusal are the Tool row, the stepover and the drill holes are core
/// findings the placement gives a place, the selection refusal is
/// `precondition.selection_refused`, a missing model is `ref.model_missing`,
/// and the D53 refusal is the
/// Containment row on the Geometry tab. What is left (the model geometry
/// checks of `validate_toolpath`) is a core gap; these rows hold it until
/// core states it.
pub(super) fn gate_refusals(
    validation_errors: &[String],
    entry: &crate::state::toolpath::ToolpathEntry,
    inputs: &ToolpathPanelInputs,
    tool: Option<&rs_cam_core::compute::tool_config::ToolConfig>,
    cut_refusal: Option<&str>,
) -> Vec<String> {
    // With no tool the validator gives only the no-tool text.
    let Some(tool) = tool else {
        return Vec::new();
    };
    let tool_kind = entry
        .operation
        .op_type()
        .tool_refusal(tool.tool_type.cutter_kind());
    let section_refusal = inputs
        .selection_report
        .as_ref()
        .and_then(|read| read.as_ref().ok())
        .and_then(|report| report.refusal.as_ref())
        .map(ToString::to_string);
    let hole = operations::hole_refusal(&inputs.validation, entry.id);
    validation_errors
        .iter()
        .filter(|err| {
            let err = err.as_str();
            tool_kind.as_deref() != Some(err)
                && err != operations::registry::STEPOVER_EXCEEDS_DIAMETER_REFUSAL
                && hole != Some(err)
                && section_refusal.as_deref() != Some(err)
                && cut_refusal != Some(err)
                && err != SELECTED_MODEL_MISSING
        })
        .cloned()
        .collect()
}

/// The issue stack under the header (W2b): the NoticeStack issue form, a
/// cap of 3, every blocker shown. The rows, in order:
///
/// 1. The last run's refusal (`RunState::Failed`), ONE row. A rest-boundary
///    refusal takes its short face and the `⟳` action (G-E (3)). A D53
///    refusal takes none: the Containment row owns it.
/// 2. The findings the placement gives the stack, face on the row, detail
///    on hover, one Fix button for a stale-default fix.
/// 3. The validator refusals no core finding states ([`gate_refusals`]).
pub(super) fn draw_issue_stack(
    ui: &mut egui::Ui,
    entry: &mut crate::state::toolpath::ToolpathEntry,
    inputs: &ToolpathPanelInputs,
    placement: &crate::ui::properties::placement::Placement<'_>,
    cut_refusal: Option<&str>,
    gates: &[String],
    events: &mut Vec<AppEvent>,
) {
    use rs_cam_core::diagnostics::DiagnosticFix;
    let mut notices = Vec::new();
    let mut acts = Vec::new();
    // A D53 refusal of the last run is the Containment row's (the Geometry
    // tab and its dot own it), so it takes no issue row.
    if let Some(rs_cam_core::session::RunState::Failed { reason }) = inputs
        .run_state
        .as_ref()
        .filter(|state| !matches!(state, rs_cam_core::session::RunState::Failed { reason } if cut_refusal == Some(reason.as_str())))
    {
        let face = inputs
            .rest_boundary_refusal
            .as_ref()
            .filter(|refusal| refusal.to_string() == *reason)
            .and_then(rest_refusal_face);
        match face {
            Some(face) => {
                let mut notice = Notice::new(Role::Danger, face.text).detail(reason.as_str());
                let act = match face.generate {
                    Some((source, _)) => {
                        notice = notice.fix(GENERATE_SOURCE_GLYPH);
                        IssueAct::Generate(source)
                    }
                    None => IssueAct::None,
                };
                notices.push(notice);
                acts.push(act);
            }
            // W6: a reason over the face limit takes a short face; the
            // reason is the hover.
            None => {
                notices.push(refusal_notice(reason, LAST_RUN_REFUSED));
                acts.push(IssueAct::None);
            }
        }
    }
    for d in &placement.issues {
        let mut notice = d.notice();
        let defect = match d.fix() {
            Some(DiagnosticFix::ApplyStaleDefault { rule_id, .. }) => inputs
                .stale_default_defects
                .iter()
                .position(|defect| defect.rule_id.id() == rule_id.as_str()),
            _ => None,
        };
        let act = match defect {
            Some(index) => {
                notice = notice.fix("Fix");
                IssueAct::StaleDefault(index)
            }
            None => IssueAct::None,
        };
        notices.push(notice);
        acts.push(act);
    }
    for gate in gates {
        notices.push(refusal_notice(gate, INPUTS_REFUSED));
        acts.push(IssueAct::None);
    }
    if notices.is_empty() {
        return;
    }
    let expand_id = ui.id().with("tp_issues_all").with(entry.id.raw());
    let expanded = ui.data(|d| d.get_temp::<bool>(expand_id).unwrap_or(false));
    ui.add_space(4.0);
    let response = NoticeStack::issues(notices)
        .expanded(expanded)
        .show_issues(ui);
    if response.expand {
        ui.data_mut(|d| d.insert_temp(expand_id, true));
    }
    match response.fix.and_then(|index| acts.get(index)) {
        Some(IssueAct::StaleDefault(index)) => {
            if let Some(defect) = inputs.stale_default_defects.get(*index) {
                rs_cam_core::compute::validate::apply_stale_default_to_op(
                    &mut entry.operation,
                    &mut entry.feeds_provenance,
                    defect,
                );
                entry.stale_since = Some(std::time::Instant::now());
            }
        }
        Some(IssueAct::Generate(source)) => events.push(AppEvent::GenerateToolpath(*source)),
        Some(IssueAct::None) | None => {}
    }
}

/// G-E (3): the one-line face of a rest-boundary refusal, and the
/// toolpath its action generates.
#[derive(Debug, Clone, PartialEq)]
pub(in crate::ui::properties) struct RefusalFace {
    pub text: String,
    /// The source toolpath that the `⟳` action generates, and its name.
    pub generate: Option<(crate::state::toolpath::ToolpathId, String)>,
}

/// G-E (3), §7.7: the short face of a rest-boundary refusal. `None` for a
/// refusal with no short face (a lost source id), which keeps its text.
pub(in crate::ui::properties) fn rest_refusal_face(
    refusal: &rs_cam_core::session::RestBoundaryRefusal,
) -> Option<RefusalFace> {
    use rs_cam_core::session::RestBoundaryRefusal;
    match refusal {
        RestBoundaryRefusal::NotGenerated {
            source_toolpath_id,
            source_name,
        } => Some(RefusalFace {
            text: format!("Generate '{source_name}' first"),
            generate: Some((*source_toolpath_id, source_name.clone())),
        }),
        RestBoundaryRefusal::NoRegions {
            source_toolpath_id,
            source_name,
        } => Some(RefusalFace {
            text: format!("'{source_name}' left no regions"),
            generate: Some((*source_toolpath_id, source_name.clone())),
        }),
        RestBoundaryRefusal::OwnRegions => Some(RefusalFace {
            text: "The source is this op".to_owned(),
            generate: None,
        }),
        RestBoundaryRefusal::SourceMissing { .. } => None,
    }
}

/// The glyph of the action that generates a refusal's source.
pub(in crate::ui::properties) const GENERATE_SOURCE_GLYPH: &str = "\u{27F3}";
