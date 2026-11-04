//! Q9 (Fusion hole recognition; operator ruling 2026-10-23): "Add a Bore
//! for these" (`Command::AddBoreFromDrill`) takes the first project tool
//! that the Bore accepts and that fits the holes, not the tool of the
//! Drill. It overturns the P6.7b ruling ("the Bore takes the Drill's
//! tool").
//!
//! # The fixture
//!
//! A drawing with a Ø6 circle and a Ø10 circle. The Drill runs a Ø6
//! twist drill (a V-bit with a 118° point), so it drills the Ø6 hole and
//! skips the Ø10 hole as larger (`HoleFit::Larger`). The Bore cuts the
//! Ø10 hole with one helix, so its tool D must give `D < 10 <= 2 D`.
//!
//! # The claim
//!
//! 1. Tools `[Ø6 V-bit, Ø12 end mill, Ø8 end mill]`: the Bore takes the
//!    Ø8 end mill. The V-bit is refused by the Bore, the Ø12 end mill is
//!    larger than the hole.
//! 2. Tools `[Ø6 V-bit, Ø12 end mill]`: no tool fits, so the Bore is made
//!    with NO tool (R2). Its state names the kind and the size limit.
//!
//! Red proof: copy `drill.tool_id` into the Bore again
//! (`ProjectSession::add_bore_from_drill`). Test 1 fails (V-bit), test 2
//! fails (a tool, not `None`).
//!
//! ```text
//! cargo test -p rs_cam_core -q --test add_a_bore_takes_a_fitting_end_mill_q9
//! ```

#![expect(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::sync::Arc;

use common::session::{TestHole, drawing_items, polygon_model, stock_over, toolpath_config};
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::operation_configs::DrillConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::session::{AddBoreFromDrillArgs, Command, ProjectSession, ProjectSessionBuilder};

fn tool(id: usize, tool_type: ToolType, diameter: f64) -> ToolConfig {
    ToolConfig::builder(ToolId::from_raw(id), tool_type)
        .diameter(diameter)
        .build()
}

/// The drawing, the tools `tools` and a Drill on the first tool. The
/// session and the index of the Bore that "Add a Bore for these" adds.
fn bore_after_drill(tools: Vec<ToolConfig>) -> (ProjectSession, usize) {
    let hole = |x: f64, d: f64| TestHole {
        x,
        y: 0.0,
        layer: "HOLES".to_owned(),
        diameter: Some(d),
    };
    let mut model = polygon_model(Vec::new(), "holes");
    model.items = Arc::new(drawing_items(
        Vec::new(),
        &[hole(-10.0, 6.0), hole(10.0, 10.0)],
    ));
    let mut builder = ProjectSessionBuilder::new().stock(stock_over(30.0, 10.0));
    for t in tools {
        let _ = builder.add_tool(t);
    }
    let drill_tool = builder.tools()[0].id;
    let model_id = builder.add_model(model);
    let drill = toolpath_config(
        "Holes",
        OperationConfig::Drill(DrillConfig::default()),
        drill_tool,
        model_id,
    );
    let _ = builder.add_toolpath(0, drill).unwrap();
    let mut s = builder.build();
    let drill_id = s.toolpath_configs()[0].id;
    assert_eq!(
        s.hole_selection(&s.toolpath_configs()[0])
            .unwrap()
            .holes
            .len(),
        1,
        "non-vacuity: the Ø6 drill drills the Ø6 hole only"
    );
    let bore = s
        .apply(Command::AddBoreFromDrill(AddBoreFromDrillArgs {
            index: 0,
            drill_id,
        }))
        .unwrap()
        .created
        .expect("the command adds a Bore");
    assert_eq!(
        s.toolpath_configs()[bore].operation.op_type(),
        OperationType::Bore
    );
    (s, bore)
}

#[test]
fn add_bore_picks_a_fitting_end_mill_q9() {
    let (s, bore) = bore_after_drill(vec![
        tool(0, ToolType::VBit, 6.0),
        tool(1, ToolType::EndMill, 12.0),
        tool(2, ToolType::EndMill, 8.0),
    ]);
    let eight = s.tools()[2].id;
    assert_eq!(
        s.toolpath_configs()[bore].tool_id,
        Some(eight),
        "the Bore takes the Ø8 end mill: not the Drill's V-bit, not the Ø12"
    );
    assert_ne!(
        s.toolpath_configs()[bore].tool_id,
        s.toolpath_configs()[0].tool_id
    );
    assert_eq!(s.no_tool_state(bore), None);
    // The Bore cuts the Ø10 hole with that tool.
    assert_eq!(
        s.hole_selection(&s.toolpath_configs()[bore])
            .unwrap()
            .holes
            .iter()
            .filter_map(|h| h.diameter)
            .collect::<Vec<_>>(),
        vec![10.0]
    );
}

#[test]
fn add_bore_with_no_fitting_tool_has_no_tool_q9() {
    let (s, bore) = bore_after_drill(vec![
        tool(0, ToolType::VBit, 6.0),
        tool(1, ToolType::EndMill, 12.0),
    ]);
    assert_eq!(
        s.toolpath_configs()[bore].tool_id,
        None,
        "no tool fits the Ø10 hole, so the Bore has none (R2)"
    );
    assert_eq!(
        s.no_tool_state(bore).as_deref(),
        Some(
            "No tool fits: needs a flat end mill or a bull nose \
             smaller than Ø10.00 and at least Ø5.00"
        ),
        "the state names the kind and the size limit"
    );
}
