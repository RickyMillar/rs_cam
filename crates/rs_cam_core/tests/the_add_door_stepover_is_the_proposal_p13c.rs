//! Feeds contract P13c (operator ruling O3, 2026-11-05): the add-door
//! stepover of a roughing Pocket is the envelope proposal (the
//! removal-rate pick, bounded by the corner-coverage rule), not 0.35 D, and
//! the Suggest of the op states the assumptions "chips clear" and "tool
//! life not modelled".
//!
//! The claims, through THE add door (`ProjectSession::draft_toolpath`) on
//! the FM1 flat end mills (D 3.175 and 6) and the default machine and
//! material:
//!
//! 1. The drafted stepover and depth equal the proposal of the envelope of
//!    the same op with both fields unset (one envelope gives both).
//! 2. The rationale that MCP `get_suggest_rationale` and the CLI `suggest`
//!    print (`SuggestRationale::from_warnings` of the profile warnings)
//!    holds both assumption texts. The GUI half is the viz sentry
//!    `the_card_states_the_width_assumptions_p13c`.
//!
//! 3. With no force line (plywood) the width is the geometry width and the
//!    depth stays unset (lead ruling on P13c item 1, 2026-11-05).
//!
//! The first test uses only the API of HEAD 4efcb4eaa, so it compiles there and
//! fails on claim 1 (the add door wrote 0.35 D and 1.5 / 6 x D) and claim 2.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test the_add_door_stepover_is_the_proposal_p13c
//! ```

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::geometry_default::slot_mut;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::envelope::{EnvelopeInput, SafeEnvelope};
use rs_cam_core::feeds::rationale::SuggestRationale;
use rs_cam_core::feeds::suggest::{NoProject, SuggestContext};
use rs_cam_core::feeds::{SpindleStrategy, embedded_vendor_lut};
use rs_cam_core::session::{AddToolArgs, AddToolpathArgs, Command, DraftRequest, ProjectSession};

/// The operator's words (STATUS item 4a, ruling O3).
const NOTES: [&str; 2] = ["chips clear", "tool life not modelled"];

fn tool(d: f64) -> ToolConfig {
    let mut t = ToolConfig::builder(ToolId::from_raw(0), ToolType::EndMill)
        .diameter(d)
        .shaft_mm(d.max(3.0))
        .shank_mm(d.max(3.0))
        .build();
    t.set_flute_count(ToolFact::Stated(2));
    t.set_cutting_length(ToolFact::Stated((d * 3.0).max(12.0)));
    t.set_stickout(ToolFact::Stated(t.cutting_length_planning_mm() + 8.0));
    t
}

#[test]
fn the_add_door_writes_the_proposal_and_states_its_assumptions_p13c() {
    for d in [3.175, 6.0] {
        let mut s = ProjectSession::new_empty();
        let _ = s
            .apply(Command::AddTool(AddToolArgs {
                tool: Box::new(tool(d)),
            }))
            .expect("the tool");
        let tool_id = s.tools()[0].id;
        let draft = s
            .draft_toolpath(DraftRequest::new(OperationType::Pocket, 0, "pocket").tool(tool_id))
            .expect("the draft");
        let op = draft.config.operation.clone();
        let stepover = op.stepover().expect("the add door states a stepover");
        let depth = op.depth_per_pass().expect("the add door states a depth");

        let mut probe = op.clone();
        for field in ["stepover", "depth_per_pass"] {
            if let Some(slot) = slot_mut(&mut probe, field) {
                *slot = None;
            }
        }
        probe.set_spindle_rpm(None);
        let env = SafeEnvelope::for_cut(EnvelopeInput {
            operation: &probe,
            tool: &s.tools()[0],
            machine: s.machine(),
            material: &s.stock_config().material,
            lut: embedded_vendor_lut(),
            spindle_strategy: SpindleStrategy::default(),
            context: SuggestContext::no_project(NoProject::NONE),
        })
        .expect("an envelope");
        let p = env.proposal.get().point().expect("a proposed depth");
        assert!(
            (stepover - p.ae_mm).abs() < 1e-9,
            "D {d}: the add-door stepover {stepover} is not the proposal width {}",
            p.ae_mm
        );
        assert!(
            (depth - p.ap_mm).abs() < 1e-9,
            "D {d}: the add-door depth {depth} is not the proposal depth {}",
            p.ap_mm
        );

        let _ = s
            .apply(Command::AddToolpath(AddToolpathArgs {
                setup_index: 0,
                config: Box::new(draft.config),
            }))
            .expect("add the toolpath");
        let tc = &s.toolpath_configs()[0];
        let profile = s.cutter_op_profile(tc).expect("the profile");
        let rationale = SuggestRationale::from_warnings(&profile.warnings);
        let text = serde_json::to_string(&rationale).expect("json");
        for note in NOTES {
            assert!(
                text.contains(note),
                "D {d}: the rationale lacks {note:?}: {text}"
            );
        }
    }
}

/// Lead ruling on P13c item 1 (2026-11-05): with no force line (plywood)
/// the force budget cannot set a depth. The add door writes the geometry
/// width (the corner-coverage bound; no force enters it; P16 deleted the
/// slot threshold) and leaves the depth unset (no underived default).
#[test]
fn with_no_force_line_the_width_is_written_and_the_depth_is_not_p13c() {
    use rs_cam_core::session::SetStockConfigArgs;
    let d = 6.0;
    let mut s = ProjectSession::new_empty();
    let mut stock = s.stock_config().clone();
    stock.material = rs_cam_core::material::Material::Plywood {
        grade: rs_cam_core::material::PlywoodGrade::BalticBirch,
    };
    let _ = s
        .apply(Command::SetStockConfig(SetStockConfigArgs {
            stock: Box::new(stock),
        }))
        .expect("the stock door");
    let _ = s
        .apply(Command::AddTool(AddToolArgs {
            tool: Box::new(tool(d)),
        }))
        .expect("the tool");
    let tool_id = s.tools()[0].id;
    let op = s
        .draft_toolpath(DraftRequest::new(OperationType::Pocket, 0, "pocket").tool(tool_id))
        .expect("the draft")
        .config
        .operation;
    assert_eq!(
        op.depth_per_pass(),
        None,
        "no force line: the depth stays unset"
    );
    let stepover = op.stepover().expect("the geometry width is written");
    // The bound from the geometry: an inner offset corner at (r + s, r + s)
    // covers the outer loop's uncut corner (2r, 2r) when (s - r) sqrt 2 <= r.
    let r = d / 2.0;
    let corner = r + r / std::f64::consts::SQRT_2;
    // The envelope of the op (P16, D12: no slot threshold stops the width).
    let env = SafeEnvelope::for_cut(EnvelopeInput {
        operation: &op,
        tool: &s.tools()[0],
        machine: s.machine(),
        material: &s.stock_config().material,
        lut: embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::default(),
        context: SuggestContext::no_project(NoProject::NONE),
    })
    .expect("a backed plywood pocket");
    let want = corner;
    assert!(
        (stepover - want).abs() < 1e-9,
        "the width {stepover} is not the geometry width {want}"
    );
    // P16 (review D5): the envelope follows the same rule. It proposes the
    // geometry width and no depth, so an apply writes no depth.
    let p = env.proposal.get();
    assert_eq!(
        p.no_depth(),
        Some(rs_cam_core::feeds::envelope::NoDepthProposal::NoForceLine),
        "no force line: the envelope proposes no depth: {p:?}"
    );
    assert!(
        (p.ae_mm - want).abs() < 1e-9,
        "the envelope width {} is not the geometry width {want}",
        p.ae_mm
    );
    assert!(
        !env.pick(rs_cam_core::feeds::envelope::PickRequest::Proposal)
            .writes_depth(),
        "a proposal pick writes no depth"
    );
}
