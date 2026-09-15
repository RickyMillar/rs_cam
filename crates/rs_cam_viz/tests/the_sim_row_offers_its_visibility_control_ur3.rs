//! UR5's visibility seam on the Simulation operation list.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

const SIM_OP_LIST: &str = "ui/sim_op_list.rs";

fn source() -> String {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join(SIM_OP_LIST);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn code_only(source: &str) -> String {
    source
        .lines()
        .map(|line| line.split_once("//").map_or(line, |(code, _)| code))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_sim_row_offers_its_visibility_control_only_in_all_mode_ur5() {
    let code = code_only(&source());
    let gate = "if viewport.show_all_toolpaths {";
    let push = "UiCommand::ToggleToolpathVisibility";
    let gate_at = code
        .find(gate)
        .unwrap_or_else(|| panic!("{SIM_OP_LIST} lost the all-toolpaths gate"));
    let push_at = code
        .find(push)
        .unwrap_or_else(|| panic!("{SIM_OP_LIST} lost its visibility control"));
    assert!(
        gate_at < push_at,
        "the simulation-row eye must be gated by all-toolpaths; selected-only draws the selection without a persistent visibility route"
    );
}

#[test]
fn the_sim_row_retains_selection_and_jump_routes_ur5() {
    let code = code_only(&source());
    let select = code
        .find("UiCommand::Select(Selection::Toolpath(")
        .unwrap_or_else(|| panic!("{SIM_OP_LIST} no longer selects on row click"));
    let jump = code
        .find("UiCommand::SimJumpToOpStart(")
        .unwrap_or_else(|| panic!("{SIM_OP_LIST} no longer jumps on row click"));
    assert!(
        select < jump,
        "selection must land before the playback jump"
    );
}
