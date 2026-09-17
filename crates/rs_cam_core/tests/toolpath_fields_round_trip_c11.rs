//! C11 fix-up — every editable toolpath field survives a save and a load.
//!
//! # The gap this closes
//!
//! `crates/rs_cam_viz/src/io/project.rs` was a second reader and a second
//! writer for the project file. C11 deleted it, and three of its tests went
//! with it: `round_trip_persists_editable_2d_state`,
//! `round_trip_persists_editable_3d_state` and
//! `round_trip_persists_rest_analysis_config`. Those three pinned a
//! non-default `RestAnalysisConfig`, the `BoundarySource::DerivedRestRegions`
//! struct variant, the coolant mode, the pre and post G-code, the dressups
//! and the heights through a save and a load.
//!
//! Core's own `session::save::tests::toolpath_round_trip` pins `name`,
//! `enabled` and the operation KIND only. The fields above therefore lost
//! their sentry. This file restores it against the ONE remaining schema,
//! `rs_cam_core::session::project_file`.
//!
//! # How the session is built
//!
//! Through `ProjectSession::apply(Command)` alone. Every field below has a
//! command that owns its rule; the four that no dedicated setter covers —
//! `coolant`, `pre_gcode`, `post_gcode` and `boundary_inherit` — go through
//! `Command::ReplaceToolpathConfig`, which is the door the GUI inspector
//! uses for them.
//!
//! # Why two toolpaths
//!
//! `BoundarySource::DerivedRestRegions` carries a stable `ToolpathId`, not
//! an index. A real predecessor therefore pins more than the serde mapping:
//! it pins that the id the finish operation names still resolves to the
//! rough operation after the load.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{
    BoundaryConfig, BoundaryContainment, BoundarySource, DogboneParams, DressupConfig,
    DressupEntryStyle, HeightMode, HeightsConfig, RestAnalysisConfig, SegmentMergeParams,
    StockSource,
};
use rs_cam_core::compute::operation_configs::Adaptive3dConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::session::{
    AddToolArgs, AddToolpathArgs, Command, ProjectSession, ProjectSessionBuilder,
    ReplaceToolpathConfigArgs, SetBoundaryConfigArgs, SetDressupConfigArgs,
    SetRestAnalysisConfigArgs, SetStockSourceArgs, SetToolpathDebugOptionsArgs,
    SetToolpathEnabledArgs, SetToolpathHeightsArgs, ToolpathConfig,
};
use rs_cam_core::trace::debug_trace::ToolpathDebugOptions;

/// One directory for this test file, removed at the end.
fn temp_dir(name: &str) -> PathBuf {
    let pid = std::process::id();
    let mut dir = std::env::temp_dir();
    dir.push(format!("rs_cam_toolpath_fields_{pid}_{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Apply one command and drop the `Effects`.
///
/// The round trip reads the stored configuration, not the stale set, so
/// every call below would otherwise need its own `let _ =`.
fn run(session: &mut ProjectSession, command: Command) {
    let _ = session.apply(command).unwrap();
}

/// A toolpath configuration with every field at its default.
///
/// The test moves the fields it pins through commands afterwards, so the
/// starting record states what "default" means for each of them.
fn default_config(tool_id: usize, name: &str, operation: OperationConfig) -> ToolpathConfig {
    ToolpathConfig {
        id: ToolpathId(0),
        name: name.to_owned(),
        enabled: true,
        operation,
        dressups: DressupConfig::default(),
        heights: HeightsConfig::default(),
        tool_id,
        model_id: 0,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        rest_analysis: RestAnalysisConfig::default(),
        stock_source: StockSource::Fresh,
        coolant: CoolantMode::Off,
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        planner_origin: None,
    }
}

/// The non-default dressups the finish operation carries.
///
/// `Pocket` holds the registry's `ANY_DRESSUP` policy, so
/// `DressupConfig::normalize_for_op` — which both the setter and the loader
/// run — leaves every value below alone. An operation with a stricter
/// policy would rewrite them, and the test would then pin the policy rather
/// than the round trip.
fn finish_dressups() -> DressupConfig {
    DressupConfig {
        entry_style: DressupEntryStyle::Ramp,
        ramp_angle: 12.0,
        dogbone: Some(DogboneParams::default()),
        feed_optimization: true,
        segment_merge: Some(SegmentMergeParams::default()),
        ..DressupConfig::default()
    }
}

/// The non-default heights the finish operation carries.
fn finish_heights() -> HeightsConfig {
    HeightsConfig {
        clearance_z: HeightMode::Manual(18.0),
        bottom_z: HeightMode::Manual(-4.2),
        ..HeightsConfig::default()
    }
}

/// The non-default rest analysis the finish operation carries.
///
/// `reference_tool_id` is the `Option` most likely to round-trip wrong, so
/// it names a real tool rather than staying `None`.
fn finish_rest_analysis(reference: ToolId) -> RestAnalysisConfig {
    RestAnalysisConfig {
        enabled: true,
        reference_tool_id: Some(reference),
        cell_mm: 0.75,
        min_valley_depth: 0.12,
        region_margin_mm: 1.5,
        ..RestAnalysisConfig::default()
    }
}

#[test]
fn every_editable_toolpath_field_survives_a_save_and_a_load() {
    let dir = temp_dir("editable");
    let path = dir.join("editable.toml");

    // L8 deleted the `SetProjectName` row, which no surface reached.
    // A name reaches a session through the file it loads, or through the
    // builder every fixture already uses.
    let mut session = ProjectSessionBuilder::new()
        .name("C11 Field Round Trip".to_owned())
        .build();

    // Tool 0 cuts; tool 1 is the rest reference the finish operation names.
    run(
        &mut session,
        Command::AddTool(AddToolArgs {
            tool: Box::new(ToolConfig::new_default(ToolId(0), ToolType::EndMill)),
        }),
    );
    run(
        &mut session,
        Command::AddTool(AddToolArgs {
            tool: Box::new(ToolConfig::new_default(ToolId(0), ToolType::BallNose)),
        }),
    );
    let cutter_id = session.tools()[0].id.0;
    let reference_id = session.tools()[1].id;

    // The rough operation. Its Adaptive3d block pins that an operation's
    // own parameters reach the file. L2 deleted the inert
    // `stock_to_leave_radial` dial this arm used to pin, so the arm now
    // pins `stock_to_leave_axial` — the leave dial the planner reads.
    run(
        &mut session,
        Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(default_config(
                cutter_id,
                "Roughing",
                OperationConfig::Adaptive3d(Adaptive3dConfig {
                    stock_to_leave_axial: 0.4,
                    ..Adaptive3dConfig::default()
                }),
            )),
        }),
    );
    // The finish operation, which carries every field this test pins.
    run(
        &mut session,
        Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(default_config(
                cutter_id,
                "Pocket A",
                OperationConfig::new_default(OperationType::Pocket),
            )),
        }),
    );
    let rough_id = session.toolpath_configs()[0].id;

    run(
        &mut session,
        Command::SetToolpathEnabled(SetToolpathEnabledArgs {
            index: 1,
            enabled: false,
        }),
    );
    run(
        &mut session,
        Command::SetDressupConfig(SetDressupConfigArgs {
            index: 1,
            dressups: Box::new(finish_dressups()),
        }),
    );
    run(
        &mut session,
        Command::SetToolpathHeights(SetToolpathHeightsArgs {
            index: 1,
            heights: finish_heights(),
        }),
    );
    // The boundary command runs BEFORE the rest-analysis command: wiring a
    // `DerivedRestRegions` source also runs the producer hook, which writes
    // the SOURCE operation's rest analysis. The order keeps the two writes
    // independent.
    run(
        &mut session,
        Command::SetBoundaryConfig(SetBoundaryConfigArgs {
            index: 1,
            boundary: BoundaryConfig {
                enabled: true,
                source: BoundarySource::DerivedRestRegions {
                    source_toolpath_id: rough_id,
                },
                containment: BoundaryContainment::Inside,
                offset: 1.25,
            },
        }),
    );
    run(
        &mut session,
        Command::SetRestAnalysisConfig(SetRestAnalysisConfigArgs {
            index: 1,
            rest_analysis: finish_rest_analysis(reference_id),
        }),
    );
    run(
        &mut session,
        Command::SetStockSource(SetStockSourceArgs {
            index: 1,
            source: StockSource::FromRemainingStock,
        }),
    );
    run(
        &mut session,
        Command::SetToolpathDebugOptions(SetToolpathDebugOptionsArgs {
            index: 1,
            debug_options: ToolpathDebugOptions { enabled: true },
        }),
    );
    // `coolant`, `pre_gcode`, `post_gcode` and `boundary_inherit` have no
    // setter of their own. The whole-record command is their door.
    let mut edited = session.toolpath_configs()[1].clone();
    edited.coolant = CoolantMode::Mist;
    edited.pre_gcode = Some("M7".to_owned());
    edited.post_gcode = Some("M9".to_owned());
    edited.boundary_inherit = false;
    run(
        &mut session,
        Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
            index: 1,
            config: Box::new(edited),
        }),
    );

    session.save(&path).unwrap();
    let written = std::fs::read_to_string(&path).unwrap();
    assert!(
        written.contains("stock_to_leave_axial = 0.4"),
        "the operation block must reach the file:\n{written}"
    );

    let loaded = ProjectSession::load(&path).unwrap();

    assert_eq!(loaded.name(), "C11 Field Round Trip");
    assert_eq!(loaded.toolpath_count(), 2);
    assert!(matches!(
        loaded.toolpath_configs()[0].operation,
        OperationConfig::Adaptive3d(Adaptive3dConfig {
            stock_to_leave_axial,
            ..
        }) if (stock_to_leave_axial - 0.4).abs() < 1e-9
    ));

    let finish = &loaded.toolpath_configs()[1];
    assert_eq!(finish.name, "Pocket A");
    assert!(!finish.enabled);
    assert!(matches!(finish.operation, OperationConfig::Pocket(_)));

    // Dressups.
    let expected_dressups = finish_dressups();
    assert_eq!(finish.dressups.entry_style, expected_dressups.entry_style);
    assert!((finish.dressups.ramp_angle - expected_dressups.ramp_angle).abs() < 1e-9);
    assert_eq!(finish.dressups.dogbone, expected_dressups.dogbone);
    assert!(finish.dressups.feed_optimization);
    assert_eq!(
        finish.dressups.segment_merge,
        expected_dressups.segment_merge
    );

    // Heights.
    assert!(matches!(
        finish.heights.clearance_z,
        HeightMode::Manual(v) if (v - 18.0).abs() < 1e-9
    ));
    assert!(matches!(
        finish.heights.bottom_z,
        HeightMode::Manual(v) if (v + 4.2).abs() < 1e-9
    ));

    // Boundary — the struct variant, and the id it names.
    assert_eq!(
        finish.boundary,
        BoundaryConfig {
            enabled: true,
            source: BoundarySource::DerivedRestRegions {
                source_toolpath_id: loaded.toolpath_configs()[0].id,
            },
            containment: BoundaryContainment::Inside,
            offset: 1.25,
        }
    );
    assert!(!finish.boundary_inherit);

    // Rest analysis.
    assert_eq!(finish.rest_analysis, finish_rest_analysis(reference_id));

    // Output and stock fields.
    assert_eq!(finish.coolant, CoolantMode::Mist);
    assert_eq!(finish.pre_gcode.as_deref(), Some("M7"));
    assert_eq!(finish.post_gcode.as_deref(), Some("M9"));
    assert_eq!(finish.stock_source, StockSource::FromRemainingStock);
    assert!(finish.debug_options.enabled);

    std::fs::remove_dir_all(&dir).unwrap();
}
