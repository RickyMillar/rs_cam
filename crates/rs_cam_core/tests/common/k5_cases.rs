//! The K5 cases (feeds contract P7): the tools and the op 3 job of the
//! blind review, built in code. Every value is copied from the source line
//! that its comment names; no test reads a `planning/` path (F155).
//!
//! Sources:
//! - `planning/rivmap_block_job/nz_wanaka_21_operator.toml` (the tool rows,
//!   the stock and op 3), at the commit of P6 (K3 forms);
//! - `planning/feeds_blind_review_2026-11-03/rootcause/04_op3_op4_drill.md`
//!   (op 3: hole 30 mm = stock 25.5 + 4.5, flute 25, stickout 30).

#![allow(
    dead_code,
    reason = "each test binary that includes this module uses only part of it"
)]
#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::sync::atomic::AtomicBool;

use rs_cam_core::ToolpathId;
use rs_cam_core::compute::alignment_pins::AlignmentPin;
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::{AlignmentPinDrillConfig, DrillCycleType};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{CentreCutting, ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::gcode::{CoolantMode, ExportedGcode};
use rs_cam_core::session::{ProjectSession, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_core::trace::debug_trace::ToolpathDebugOptions;

/// The basis text that the P6 migration wrote on each bare number.
pub const MIGRATED: &str = "bare number before K3 (2026-11)";

/// Tool 0 of nz-wanaka-21 (operator file lines 111-130): "6 mm flat end
/// mill", diameter 6.0, `cutting_length` estimated 25.0, `shank_diameter`
/// estimated 6.0, `stickout` estimated 30.0, `flute_count` estimated 2. The
/// file has no `centre_cutting` key; ruling D12 makes it a required input,
/// and a test-built tool states its facts (lead decision), so this test
/// tool states `Yes`. That is the test case, not a claim about the real
/// tool.
pub fn flat_6(stickout_mm: f64) -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.name = "6 mm flat end mill".to_owned();
    tool.diameter = 6.0;
    tool.shaft_diameter = 6.0;
    tool.set_cutting_length(ToolFact::estimated(25.0, MIGRATED));
    tool.set_shank_diameter(ToolFact::estimated(6.0, MIGRATED));
    tool.set_stickout(ToolFact::estimated(stickout_mm, MIGRATED));
    tool.set_flute_count(ToolFact::estimated(2, MIGRATED));
    tool.set_centre_cutting(ToolFact::Stated(CentreCutting::Yes));
    tool
}

/// The stickout of tool 0 (operator file line 125).
pub const FLAT_6_STICKOUT_MM: f64 = 30.0;

/// Tool 1 of nz-wanaka-21 (operator file lines 132-151): "3.175-20deg-0.3
/// V-bit", diameter 3.175, included angle 20.0, `cutting_length` estimated
/// 8.152467616, `shank_diameter` estimated 3.175, `stickout` estimated
/// 25.0, `flute_count` estimated 1. The file has no tip key and no
/// `centre_cutting` key; the test tool states the tip that the name prints
/// ([`V20_TIP_MM`]) and centre cutting `Yes` (ruling D12, the test-data
/// rule).
pub fn v20() -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::VBit);
    tool.name = "3.175-20deg-0.3 V-bit".to_owned();
    tool.diameter = 3.175;
    tool.shaft_diameter = 3.175;
    tool.included_angle = 20.0;
    tool.set_cutting_length(ToolFact::estimated(8.152467616, MIGRATED));
    tool.set_shank_diameter(ToolFact::estimated(3.175, MIGRATED));
    tool.set_stickout(ToolFact::estimated(25.0, MIGRATED));
    tool.set_flute_count(ToolFact::estimated(1, MIGRATED));
    tool.set_centre_cutting(ToolFact::Stated(CentreCutting::Yes));
    tool.set_v_tip_diameter(ToolFact::Stated(V20_TIP_MM));
    tool
}

/// The flat tip of the V20 that its name states ("0.3 mm flat tip",
/// operator file line 134; report 06 (a2)).
pub const V20_TIP_MM: f64 = 0.3;

/// The stock of nz-wanaka-21 (operator file lines 6-14): 395 x 460 x 25.5,
/// origin (-18.5, -8.5, 0), with its first alignment pin (lines 19-22:
/// x 6.25, y 230.0, diameter 6.0).
pub fn wanaka_stock() -> StockConfig {
    StockConfig {
        x: 395.0,
        y: 460.0,
        z: Some(25.5),
        origin_x: -18.5,
        origin_y: -8.5,
        origin_z: 0.0,
        auto_from_model: false,
        alignment_pins: vec![AlignmentPin::new(6.25, 230.0, 6.0)],
        ..StockConfig::default()
    }
}

/// Op 3 of nz-wanaka-21 (operator file lines 825-846): "Pin drill, flip
/// pins", Pin Drill, diameter 6.0, `spoilboard_penetration` 4.5, peck cycle,
/// `peck_depth` 6.0, `feed_rate` 736.0, `retract_z` 2.0, `spindle_rpm`
/// 10610, entry None. The model is a drawing in the file; here it has no
/// model at all, so no mesh exists (report 04 (a)).
pub fn op3_pin_drill() -> ToolpathConfig {
    ToolpathConfig {
        shoe: Default::default(),
        feeds_lock: Default::default(),
        id: ToolpathId::from_raw(0),
        name: "Pin drill, flip pins".to_owned(),
        enabled: true,
        operation: OperationConfig::AlignmentPinDrill(AlignmentPinDrillConfig {
            diameter: 6.0,
            spoilboard_penetration: 4.5,
            cycle: DrillCycleType::Peck,
            peck_depth: Some(6.0),
            feed_rate: Some(736.0),
            retract_z: 2.0,
            spindle_rpm: Some(10_610),
            ..AlignmentPinDrillConfig::default()
        }),
        dressups: DressupConfig::for_op(OperationType::AlignmentPinDrill),
        heights: HeightsConfig::default(),
        tool_id: Some(rs_cam_core::ids::ToolId::from_raw(0)),
        model_id: rs_cam_core::ids::ModelId::from_raw(usize::MAX),
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: StockSource::default(),
        coolant: CoolantMode::Off,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
        planner_origin: None,
    }
}

/// The op 3 session with tool 0 at `stickout_mm`, generated.
pub fn op3_generated(stickout_mm: f64) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new()
        .stock(wanaka_stock())
        .tool(flat_6(stickout_mm));
    let _ = builder
        .add_toolpath(0, op3_pin_drill())
        .expect("add the pin drill");
    let mut session = builder.build();
    let _ = session
        .generate_toolpath(0, &AtomicBool::new(false))
        .expect("op 3 generates (ruling Q6: no rule refuses)");
    session
}

/// The K5 rows of toolpath `index` with the id `id`: the texts.
pub fn rule_texts(session: &ProjectSession, index: usize, id: &str) -> Vec<String> {
    session
        .result_at(index)
        .expect("a result")
        .stats
        .feasibility
        .as_ref()
        .expect("the feasibility stage ran")
        .rows()
        .iter()
        .filter(|row| row.id() == id)
        .map(|row| row.text().to_owned())
        .collect()
}

/// The export of the session (no simulation trace). Ruling Q6: it always
/// writes a program.
pub fn export(session: &ProjectSession) -> ExportedGcode {
    session
        .export_gcode_checked(None)
        .expect("the export writes the program (ruling Q6)")
}

/// True when the export names `id` in its warnings and in a header line.
pub fn export_names(exported: &ExportedGcode, id: &str) -> bool {
    exported.warnings.iter().any(|w| w.id == id)
        && exported
            .warnings
            .iter()
            .filter(|w| w.id == id)
            .all(|w| exported.gcode.contains(&w.header_line()))
}
