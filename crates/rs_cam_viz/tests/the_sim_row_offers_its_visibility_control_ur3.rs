//! UR3's visibility seam, as a contract on the Simulation operation list.
//!
//! The ratified visibility model (UR5 ruling, 2026-09-14): what is drawn is
//! the SELECTION by default; the eye is the persistent hide/show control and
//! it must be reachable in BOTH draw modes, because `visible` bites in
//! selected-only mode too — a hidden toolpath draws nothing even when it is
//! the one selected. A row that hides its eye when show-all is off leaves a
//! hidden toolpath with no control on it, which is how this defect shipped:
//! the eye appeared only in the mode where it mattered least.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

const SIM_OP_LIST: &str = "ui/sim_op_list.rs";

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn read(rel: &str) -> String {
    let path = src_root().join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// `line` with any `//` comment removed, so a comment that NAMES a deleted
/// construct cannot resurrect it in a scan.
fn code_only(src: &str) -> String {
    src.lines()
        .map(|line| match line.find("//") {
            Some(i) => &line[..i],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Pure check: `visible` bites in selected-only mode. The Simulation eye is
/// reachable in both modes BECAUSE this rule holds — a hidden toolpath draws
/// nothing even when it is the one selected.
#[test]
fn hidden_bites_in_selected_only_mode_ur3() {
    // show_all off, one row selected, that row HIDDEN → nothing draws.
    let drawn = toolpaths_to_draw(
        ToolpathDrawFilter {
            selected: Some(ToolpathId(0)),
            isolate: None,
            show_all: false,
        },
        vec![(ToolpathId(0), false, true)],
    );
    assert!(
        drawn.is_empty(),
        "a hidden toolpath must draw nothing even when it is the selected one"
    );

    // Same shape, row VISIBLE → it draws. Non-vacuity for the arm above.
    let drawn = toolpaths_to_draw(
        ToolpathDrawFilter {
            selected: Some(ToolpathId(0)),
            isolate: None,
            show_all: false,
        },
        vec![(ToolpathId(0), true, true)],
    );
    assert_eq!(drawn, vec![ToolpathId(0)]);
}

/// The eye renders in both draw modes: the `ToggleToolpathVisibility` push
/// must not sit inside an `if viewport.show_all_toolpaths` block.
#[test]
fn the_sim_row_offers_its_eye_in_both_draw_modes_ur3() {
    let code = code_only(&read(SIM_OP_LIST));
    let push = "UiCommand::ToggleToolpathVisibility";
    let Some(at) = code.find(push) else {
        panic!("{SIM_OP_LIST} lost the eye's visibility push entirely");
    };
    // Walk backwards from the push to the nearest block opener at the same
    // nesting level. The eye block must not be gated on show_all.
    let before = &code[..at];
    let gate = before.rfind("if viewport.show_all_toolpaths");
    if let Some(gate_at) = gate {
        // Is the gate still OPEN at the push? A `}` between the gate and
        // the push closes it.
        let between = &code[gate_at..at];
        assert!(
            between.contains('}'),
            "the eye's ToggleToolpathVisibility push is inside an \\\n             `if viewport.show_all_toolpaths` block again. `visible` \\\n             bites in selected-only mode too, so a hidden row needs its \\\n             control there — that is the defect this sentry pins."
        );
    }
}

/// Clicking a simulation row selects the toolpath AND jumps playback. The
/// selection must be pushed first: the viewport's draw set reads the
/// selection on the same frame, and a jump without a selection leaves the
/// camera on one operation while the picture shows another.
#[test]
fn a_sim_row_click_selects_before_it_jumps_ur3() {
    let code = code_only(&read(SIM_OP_LIST));
    let select_at = code
        .find("UiCommand::Select(Selection::Toolpath(")
        .unwrap_or_else(|| panic!("{SIM_OP_LIST} no longer selects on row click"));
    let jump_at = code
        .find("UiCommand::SimJumpToOpStart(")
        .unwrap_or_else(|| panic!("{SIM_OP_LIST} no longer jumps on row click"));
    assert!(
        select_at < jump_at,
        "the row click pushes SimJumpToOpStart before Select; the selection \\\n         must land first so the same frame's draw set agrees with the jump"
    );
}
