//! Feeds contract P11c, D13 item 6 (ruled 2026-11-05, operator: "let the
//! default be about shapeoko. its probably a good start.. but make it
//! explicit that this number is 'hobby grade' vs 'industrial' or similar").
//!
//! Claims:
//!
//! 1. One default set: `RigidityProfile::default()`. The generic router,
//!    both Shapeoko presets and `MachineProfile::default()` hold it, with
//!    `MachineClass::Hobby`, and their basis text is "hobby grade (Shapeoko
//!    port), Unsourced".
//! 2. The class text is on the Suggest output: the envelope of a Pocket
//!    (the JSON that MCP `get_safe_envelope` and the CLI `suggest` record
//!    print) names it in the basis of the `Rigidity` and `Slot` limits.
//! 3. A machine whose values differ from the default reads "stated by the
//!    machine file", with its class.
//!
//! Red before P11c: `MachineClass` and `RigidityProfile::slot_doc_factor` do
//! not exist at HEAD f5170da03 (no compile). At HEAD the generic router held
//! its own set (0.20 / 1.5 / 0.20), unequal to `RigidityProfile::default()`.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test the_rigidity_default_is_hobby_grade_p11c
//! ```

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::machine::{MachineClass, MachineProfile, RigidityBasis, RigidityProfile};
use rs_cam_core::session::{AddToolArgs, AddToolpathArgs, Command, DraftRequest, ProjectSession};

const HOBBY_TEXT: &str =
    "hobby grade (Shapeoko port, Unsourced; force budget from the hobby slot charts)";

#[test]
fn every_built_in_machine_holds_the_one_hobby_default_p11c() {
    let mut all = vec![
        ("default", MachineProfile::default()),
        ("generic", MachineProfile::generic_wood_router()),
    ];
    all.extend(MachineProfile::presets());
    assert!(all.len() >= 5, "non-vacuity: the presets are listed");
    for (name, m) in all {
        assert_eq!(m.rigidity, RigidityProfile::default(), "{name}");
        assert_eq!(m.class, MachineClass::Hobby, "{name}");
        assert_eq!(m.rigidity_basis(), RigidityBasis::HobbyDefault, "{name}");
        assert_eq!(m.rigidity_basis().text(), HOBBY_TEXT, "{name}");
    }
}

#[test]
fn a_stated_rigidity_names_its_class_and_the_file_p11c() {
    let mut m = MachineProfile::default();
    m.rigidity.force_budget_n *= 0.5;
    assert_eq!(m.rigidity_basis(), RigidityBasis::HobbyStated);
    assert!(m.rigidity_basis().text().contains("hobby grade"));
    m.class = MachineClass::Industrial;
    assert_eq!(m.rigidity_basis(), RigidityBasis::IndustrialStated);
    assert!(m.rigidity_basis().text().contains("industrial"));
}

#[test]
fn the_suggest_envelope_names_the_hobby_class_p11c() {
    let mut s = ProjectSession::new_empty();
    let _ = s
        .apply(Command::AddTool(AddToolArgs {
            tool: Box::new(
                ToolConfig::builder(ToolId::from_raw(0), ToolType::EndMill)
                    .diameter(6.0)
                    .shaft_mm(6.0)
                    .shank_mm(6.0)
                    .build(),
            ),
        }))
        .expect("the tool");
    let tool = s.tools()[0].id;
    let draft = s
        .draft_toolpath(DraftRequest::new(OperationType::Pocket, 0, "pocket").tool(tool))
        .expect("the draft");
    let _ = s
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(draft.config),
        }))
        .expect("the toolpath");
    let tc = s.toolpath_configs().last().expect("added").clone();
    let profile = s
        .cutter_op_profile(
            &tc,
            rs_cam_core::dressup::feed_modulation::RunModulation::Off,
        )
        .expect("a profile");
    let envelope = profile.envelope.as_ref().expect("an envelope");
    let json = serde_json::to_string(envelope).expect("json");
    assert!(
        json.contains(HOBBY_TEXT),
        "the envelope JSON names the class text: {json}"
    );
    assert!(json.contains("machine.rigidity"), "and the machine field");
}
