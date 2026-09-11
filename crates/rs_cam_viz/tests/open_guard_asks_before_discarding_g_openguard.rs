//! G-OPENGUARD sentries (F1.12) — nothing replaces the open project
//! without saying what would be lost.
//!
//! Quitting with unsaved changes asked: an "Unsaved Changes" dialog with
//! Save / Discard / Cancel. File > Open and Ctrl+O replaced the project
//! just as completely — a fresh `GuiState`, a fresh `ProjectSession`, the
//! undo history gone — and asked nothing. MCP `load_project` did the same
//! through the same `open_job_from_path`.
//!
//! Three layers are pinned here:
//!
//! - The GUI decision, by SOURCE. `RsCamApp` needs an
//!   `eframe::CreationContext`, so the `AppEvent::OpenJob` arm cannot be
//!   driven from a test; what is checked is that the arm consults
//!   `gui.dirty` and that the file picker, the load and the camera fit
//!   have moved out of it into one routine the dialog also calls.
//! - ONE dialog. The window is constructed in exactly one place, and it
//!   has a verb for each action it guards — the alternative, a second
//!   dialog with the same three buttons, is how two surfaces drift apart.
//! - The MCP contract, off the real registered schema: `discard_unsaved`
//!   exists, is optional (so the default is refuse), and the description
//!   tells an agent what the refusal means and how to get past it.
//!
//! The decision underneath both routes —
//! `state::unsaved_project_summary` — is driven against a real
//! `AppController` in `controller/workflow_tests.rs`.

#![cfg(feature = "mcp")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_viz::mcp_server::EmbeddedCamServer;

/// The two dispatch sources under test, read as text.
const INPUT_SRC: &str = include_str!("../src/app/input.rs");
const DIALOG_SRC: &str = include_str!("../src/app/export.rs");
const MCP_SRC: &str = include_str!("../src/app/mcp.rs");

/// The body of the `AppEvent::OpenJob` arm, from its `=>` to the arm that
/// follows it.
///
/// WP13 moved the arm that used to follow this one — `ShowShortcuts` — into
/// the view registry, so the end marker is the view dispatch block that now
/// sits next.
fn open_job_arm() -> &'static str {
    let start = INPUT_SRC
        .find("AppEvent::OpenJob =>")
        .expect("the OpenJob dispatch arm still exists");
    let rest = &INPUT_SRC[start..];
    let end = rest
        .find("AppEvent::Ui(cmd) => match cmd {")
        .expect("the view dispatch block after OpenJob still exists");
    &rest[..end]
}

// ── 1. The GUI arm asks before it does anything ─────────────────────────

#[test]
fn the_open_job_arm_checks_for_unsaved_changes_before_it_loads() {
    let arm = open_job_arm();
    assert!(
        arm.contains("gui.dirty"),
        "File > Open / Ctrl+O must consult the dirty flag before replacing \
         the project; arm reads:\n{arm}"
    );
    assert!(
        arm.contains("UnsavedGuard::OpenJob"),
        "an unsaved project must raise the SAME dialog quitting raises, \
         not a new one; arm reads:\n{arm}"
    );
}

/// The refusal has to come first. A picker that opens, or a camera that
/// fits, before the operator has answered is a visible action taken on a
/// decision not yet made — so neither may appear in the arm at all.
#[test]
fn the_open_job_arm_holds_no_picker_load_or_camera_fit_of_its_own() {
    let arm = open_job_arm();
    for forbidden in [
        "rfd::FileDialog",
        "open_job_from_path",
        "fit_camera_to_first_model",
    ] {
        assert!(
            !arm.contains(forbidden),
            "`{forbidden}` must live in `open_job_interactive`, which runs \
             only after the guard is answered; arm reads:\n{arm}"
        );
    }
    assert!(
        INPUT_SRC.contains("fn open_job_interactive"),
        "the picker/load/fit routine the dialog and the arm share must exist"
    );
}

/// One routine owns the sequence, so the dialog's Discard path and the
/// clean-project path cannot diverge in what they do after the answer.
#[test]
fn the_dialog_and_the_menu_run_the_same_open_routine() {
    assert_eq!(
        INPUT_SRC.matches("fn open_job_interactive").count(),
        1,
        "exactly one definition of the open routine"
    );
    assert!(
        DIALOG_SRC.contains("UnsavedGuard::OpenJob => self.open_job_interactive()"),
        "the dialog must proceed into the same routine the menu uses"
    );
}

// ── 2. One dialog, two things it guards ─────────────────────────────────

#[test]
fn there_is_exactly_one_unsaved_changes_dialog() {
    let constructions = DIALOG_SRC
        .matches("egui::Window::new(\"Unsaved Changes\")")
        .count();
    assert_eq!(
        constructions, 1,
        "a second dialog with the same three buttons is how surfaces drift \
         apart; found {constructions} constructions"
    );
    for verb in ["Save & {}", "Discard & {}", "Cancel"] {
        assert!(
            DIALOG_SRC.contains(verb),
            "the dialog must keep all three answers, `{verb}` is missing"
        );
    }
}

/// Cancelling a save from inside the dialog must not proceed. The save
/// helper reports whether it saved, and the Save button is gated on it.
#[test]
fn a_cancelled_save_does_not_proceed_past_the_guard() {
    assert!(
        DIALOG_SRC.contains("&& self.save_for_unsaved_guard()"),
        "the Save button must proceed only when the save actually happened"
    );
    assert!(
        DIALOG_SRC.contains("fn save_for_unsaved_guard(&mut self) -> bool"),
        "the save helper must report whether it saved"
    );
}

// ── 3. The MCP contract ─────────────────────────────────────────────────

struct ToolFacts {
    description: String,
    schema: serde_json::Value,
}

fn tool_facts(name: &str) -> ToolFacts {
    let router = EmbeddedCamServer::into_tool_router();
    let tool = router
        .list_all()
        .into_iter()
        .find(|t| t.name == name)
        .unwrap_or_else(|| panic!("the embedded server no longer registers `{name}`"));
    ToolFacts {
        description: tool.description.map(|d| d.into_owned()).unwrap_or_default(),
        schema: serde_json::Value::Object((*tool.input_schema).clone()),
    }
}

#[test]
fn load_project_declares_an_optional_discard_unsaved_flag() {
    let facts = tool_facts("load_project");
    let flag = facts
        .schema
        .pointer("/properties/discard_unsaved")
        .unwrap_or_else(|| panic!("missing `discard_unsaved` in {}", facts.schema));
    assert_eq!(
        flag.pointer("/type").and_then(|t| t.as_str()),
        Some("boolean"),
        "`discard_unsaved` must be a boolean: {flag}"
    );

    let required: Vec<&str> = facts
        .schema
        .pointer("/required")
        .and_then(|r| r.as_array())
        .map(|r| r.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    assert!(
        !required.contains(&"discard_unsaved"),
        "it must be optional, so the DEFAULT is to refuse rather than to \
         discard; required = {required:?}"
    );
    assert!(
        required.contains(&"path"),
        "`path` is still the one required argument; required = {required:?}"
    );
}

/// An agent cannot see a dialog, so the description is the only place it
/// learns that a load can be refused and what to do about it.
#[test]
fn the_load_project_description_names_the_refusal_and_the_way_past_it() {
    let facts = tool_facts("load_project");
    for phrase in ["unsaved", "refused", "save_project", "discard_unsaved"] {
        assert!(
            facts.description.to_lowercase().contains(phrase),
            "the description must mention `{phrase}`; got: {}",
            facts.description
        );
    }
}

/// The refusal is decided BEFORE the load, so a refused call leaves the
/// project, the camera and the file dialog untouched.
#[test]
fn the_mcp_refusal_is_decided_before_the_load() {
    let start = MCP_SRC
        .find("fn mcp_load_project")
        .expect("the handler still exists");
    let rest = &MCP_SRC[start..];
    let guard = rest
        .find("unsaved_project_refusal")
        .expect("the handler must consult the guard");
    let load = rest
        .find("open_job_from_path")
        .expect("the handler must still load");
    assert!(
        guard < load,
        "the guard must be checked before `open_job_from_path` runs, or a \
         refusal arrives after the project has already been replaced"
    );
    assert!(
        rest[..load].contains("discard_unsaved"),
        "the override must be read on the way in, not after the load"
    );
}
