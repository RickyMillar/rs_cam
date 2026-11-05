//! Feeds contract P11c, D13 item 6 (`planning/feeds_contract_2026-11-03/STATUS.md`,
//! "Lead rulings for P11c"): the GUI Machine panel names the machine class
//! and how the rigidity values are known, with the text that MCP
//! `inspect_machine` (`rigidity.basis`) and the lineage basis print (core
//! `RigidityBasis::text`).
//!
//! The panel is drawn headless (two frames, the real `properties::draw`
//! with the Machine focused) and the painted text is read.
//!
//! Arms:
//!
//! 1. The default machine paints "Rigidity: hobby grade default" (the IC1
//!    short face of "hobby grade (Shapeoko port), Unsourced", which is the
//!    header hover).
//! 2. An industrial machine with its own values (`SetMachine`, the panel's
//!    command door) paints "Rigidity: industrial, stated".
//!
//! Red before P11c: the panel drew no rigidity row at HEAD f5170da03, and
//! `MachineClass` did not exist (no compile).
//!
//! ```text
//! cargo test -p rs_cam_viz -q --test the_machine_panel_names_the_rigidity_class_p11c
//! ```

#![expect(
    clippy::unwrap_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::machine::{MachineClass, MachineProfile};
use rs_cam_core::session::{Command, SetMachineArgs};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::selection::Selection;
use rs_cam_viz::ui::{properties, tokens};

#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "test: foreign enum egui Shape; the arm skips every shape that this test does not read"
)]
fn painted(state: &mut AppState) -> Vec<String> {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    ctx.memory_mut(|memory| memory.set_everything_is_visible(true));
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    let mut text = Vec::new();
    for _ in 0..2 {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let mut events = Vec::new();
            properties::draw(ui, state, &mut events);
        });
        text = out
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::epaint::Shape::Text(t) => Some(t.galley.job.text.clone()),
                _ => None,
            })
            .collect();
        out.textures_delta.clear();
    }
    text
}

fn machine_panel_text(machine: Option<MachineProfile>) -> Vec<String> {
    let mut state = AppState::new();
    if let Some(m) = machine {
        let _ = state
            .apply(Command::SetMachine(SetMachineArgs {
                machine: Box::new(m),
            }))
            .unwrap();
    }
    let _ = state.set_focus(Selection::Machine);
    painted(&mut state)
}

#[test]
fn the_machine_panel_names_the_hobby_default_p11c() {
    let text = machine_panel_text(None);
    assert!(
        text.iter().any(|t| t.contains("Machine Setup")),
        "non-vacuity: the Machine panel painted: {text:?}"
    );
    let want = format!(
        "Rigidity: {}",
        MachineProfile::default().rigidity_basis().short()
    );
    assert!(
        text.iter().any(|t| t == &want),
        "the panel names the class: want {want:?} in {text:?}"
    );
    assert_eq!(want, "Rigidity: hobby grade default");
}

#[test]
fn the_machine_panel_names_an_industrial_machine_p11c() {
    let mut m = MachineProfile {
        class: MachineClass::Industrial,
        ..MachineProfile::default()
    };
    m.rigidity.doc_roughing_factor *= 2.0;
    let text = machine_panel_text(Some(m));
    assert!(
        text.iter().any(|t| t == "Rigidity: industrial, stated"),
        "the panel names the industrial class: {text:?}"
    );
}
