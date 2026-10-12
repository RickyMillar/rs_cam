//! **G-OPT5 — a deeper cell ships the calculator's feed for its own depth.**
//!
//! Pre-registered in `planning/optimize_review_2026-10-12/PLAN.md` §2
//! step 3 (review finding F5). The 2026-05 grid patched the depth per
//! pass and the stepover and kept the feed of the baseline depth, so a
//! 2.5 x D cell shipped the 1 x D feed. The rebuild sizes every cell
//! through the feeds calculator at that cell's depth and stepover
//! (`tool_load::optimize::search::depth_width_cell`).
//!
//! The arms:
//!
//! 1. A 2.5 x D cell on a 3 mm 2-flute flat end mill in the mini
//!    project's oak ships the feed the calculator gives at 2.5 x D, and
//!    the depth per pass on the written operation is the cell's depth.
//! 2. That feed is below the feed of a 1 x D cell: the depth de-rate
//!    reached the feed.
//! 3. A cell whose depth the calculator moves (the flute guard) is
//!    refused, not shipped.
//!
//! The feeds calculator is not changed by this file. The tool and the
//! stock are copies of the mini project
//! (`planning/rivmap_block_job/testpiece_mini.toml`, tool 0 and the
//! `SolidWoodByJanka` stock).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::DressupConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::SpindleStrategy;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::Material;
use rs_cam_core::tool_load::optimize::search::{
    CellInputs, DepthWidthCell, depth_width_cell, feeds_at_cell,
};

/// Tool 0 of the mini project: 3 mm, 2 flutes, 30 mm cut length and
/// stick-out.
fn mini_tool() -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 3.0;
    tool.flute_count = 2;
    tool.cutting_length = 30.0;
    tool.stickout = 30.0;
    tool.shank_diameter = 3.0;
    tool
}

/// The mini project's stock material.
fn mini_oak() -> Material {
    Material::SolidWoodByJanka {
        janka_lbf: 1290.0,
        label: "Oak (feeds_oak.toml janka_lbf)".to_owned(),
        source_id: "rivmap feeds_oak.toml".to_owned(),
    }
}

/// A pocket deep enough for every cell here.
fn pocket() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        stepover: 0.838,
        depth: 30.0,
        depth_per_pass: 0.75,
        feed_rate: 3877.0,
        plunge_rate: 1938.0,
        spindle_rpm: Some(18_000),
        ..PocketConfig::default()
    })
}

fn dressups() -> DressupConfig {
    DressupConfig {
        link_moves: None,
        ..DressupConfig::default()
    }
}

#[test]
fn a_deep_cell_ships_the_calculator_feed_for_its_depth_and_keeps_its_depth() {
    let tool = mini_tool();
    let material = mini_oak();
    let machine = MachineProfile::generic_wood_router();
    let dressups = dressups();
    let inputs = CellInputs {
        tool: &tool,
        material: &material,
        machine: &machine,
        spindle_strategy: SpindleStrategy::default(),
        dressups: &dressups,
    };
    let stepover = 0.838;
    let deep = DepthWidthCell {
        depth_per_pass_mm: 2.5 * tool.diameter,
        stepover_mm: stepover,
    };
    let shallow = DepthWidthCell {
        depth_per_pass_mm: tool.diameter,
        stepover_mm: stepover,
    };

    let deep_op = depth_width_cell(&pocket(), deep, &inputs).expect("the 2.5 x D cell ships");
    let shallow_op = depth_width_cell(&pocket(), shallow, &inputs).expect("the 1 x D cell ships");

    // 1 — the depth on the written operation is the cell's depth.
    assert_eq!(deep_op.depth_per_pass(), Some(deep.depth_per_pass_mm));
    assert_eq!(deep_op.stepover(), Some(stepover));

    // 1 — the feed is the calculator's at that depth, rounded down to
    // 1 mm/min as the Suggest funnel rounds it.
    let direct = feeds_at_cell(
        &pocket(),
        &inputs,
        Some(deep.depth_per_pass_mm),
        Some(stepover),
    )
    .expect("the calculator sizes the cell");
    assert!(
        (direct.axial_depth_mm - deep.depth_per_pass_mm).abs() < 1e-9,
        "the calculator honours the pinned depth: {}",
        direct.axial_depth_mm
    );
    let expected_feed = direct.feed_rate_mm_min.floor();
    assert!(
        (deep_op.feed_rate() - expected_feed).abs() < 1e-9,
        "the 2.5 x D cell must ship the calculator's feed for 2.5 x D \
         ({expected_feed} mm/min); it ships {} mm/min",
        deep_op.feed_rate()
    );
    assert_eq!(
        deep_op.spindle_rpm(),
        Some(direct.rpm.round() as u32),
        "the RPM is the calculator's"
    );

    // 2 — the depth de-rate reached the feed.
    assert!(
        deep_op.feed_rate() < shallow_op.feed_rate(),
        "the 2.5 x D feed ({}) must be below the 1 x D feed ({})",
        deep_op.feed_rate(),
        shallow_op.feed_rate()
    );
}

#[test]
fn a_cell_the_calculator_moves_is_refused() {
    let tool = mini_tool();
    let material = mini_oak();
    let machine = MachineProfile::generic_wood_router();
    let dressups = dressups();
    let inputs = CellInputs {
        tool: &tool,
        material: &material,
        machine: &machine,
        spindle_strategy: SpindleStrategy::default(),
        dressups: &dressups,
    };
    // The flute guard caps the depth at 0.8 x the cut length = 24 mm
    // (`feeds/mod.rs`, Step 4). A 25 mm cell is past it.
    let past_the_flutes = DepthWidthCell {
        depth_per_pass_mm: 25.0,
        stepover_mm: 0.838,
    };
    let refusal = depth_width_cell(&pocket(), past_the_flutes, &inputs)
        .expect_err("the calculator moves this depth, so the cell is refused");
    assert!(
        refusal.contains("moved the depth from 25.000 mm"),
        "the refusal names the move: {refusal}"
    );
}
