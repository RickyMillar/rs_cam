//! P4 — `SetAlignmentPins` replaces the pin set in one step, and stales
//! only what reads the pins (operator ruling 2026-10-12).
//!
//! The pins reach a generation only through an `AlignmentPinDrill`
//! operation: the generation pass copies the live pins into its holes. So
//! a pin edit drops the result of every pin-drill operation, the results
//! that depend on it through the dependency edges, and the simulation. An
//! unrelated pocket keeps its result. Before P4 every pin edit dropped
//! every result (`drop_all_results`).
//!
//! The loader half (P3, P9, C1): a project file that still carries the
//! deleted `flip_axis` key refuses and names the key; without the key it
//! loads and says the keying fact once.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeSet;

use rs_cam_core::ToolpathId;
use rs_cam_core::compute::alignment_pins::AlignmentPin;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{
    BoundaryConfig, DressupConfig, HeightsConfig, RestAnalysisConfig, StockSource,
};
use rs_cam_core::compute::operation_configs::{AlignmentPinDrillConfig, PocketConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::session::{
    AddAlignmentPinArgs, AdoptResultArgs, Command, LoadedModel, ProjectLoadWarning, ProjectSession,
    ProjectSessionBuilder, RemoveAlignmentPinArgs, SetAlignmentPinsArgs, SimulationDropCause,
    ToolpathConfig,
};
use rs_cam_core::trace::debug_trace::ToolpathDebugOptions;

fn tc(name: &str, op: OperationConfig, stock_source: StockSource) -> ToolpathConfig {
    ToolpathConfig {
        id: ToolpathId(0),
        name: name.to_owned(),
        enabled: true,
        operation: op,
        dressups: DressupConfig::default(),
        heights: HeightsConfig::default(),
        tool_id: 0,
        model_id: 0,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source,
        coolant: CoolantMode::Off,
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: RestAnalysisConfig::default(),
        planner_origin: None,
    }
}

fn empty_model(name: &str) -> LoadedModel {
    LoadedModel {
        id: 0,
        name: name.to_owned(),
        mesh: None,
        polygons: None,
        drill_targets: std::sync::Arc::new(Vec::new()),
        layers: std::sync::Arc::new(Vec::new()),
        path: std::path::PathBuf::from(name),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    }
}

fn fake_result() -> rs_cam_core::session::ToolpathComputeResult {
    rs_cam_core::session::ToolpathComputeResult {
        op_data: rs_cam_core::ops::drill_op::OpData::Toolpath(std::sync::Arc::new(
            rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(
                rs_cam_core::toolpath::Toolpath::new(),
            ),
        )),
        stats: rs_cam_core::compute::toolpath_stats::ToolpathStats::default(),
        debug_trace: None,
        semantic_trace: None,
        provenance: rs_cam_core::session::ResultProvenance::Dressed,
    }
}

fn adopt(s: &mut ProjectSession, index: usize) {
    let revision = s.toolpath_revision(index);
    let _ = s
        .apply(Command::AdoptResult(AdoptResultArgs {
            index,
            revision,
            result: Box::new(fake_result()),
        }))
        .expect("the fixture adopts at the current revision");
}

/// Build a session with the named toolpaths in one setup, each with a
/// cached result. The pins start as `pins()`.
fn session_with(ops: Vec<ToolpathConfig>) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new();
    builder.add_tool(ToolConfig::new_default(ToolId(0), ToolType::EndMill));
    builder.add_model(empty_model("part.svg"));
    let tool = builder.tools()[0].id.0;
    let model = builder.models()[0].id;
    for mut op in ops {
        op.tool_id = tool;
        op.model_id = model;
        let _ = builder.add_toolpath(0, op).unwrap();
    }
    let stock = rs_cam_core::compute::stock_config::StockConfig {
        alignment_pins: pins(),
        ..Default::default()
    };
    let mut s = builder.stock(stock).build();
    for index in 0..s.toolpath_count() {
        adopt(&mut s, index);
    }
    s
}

fn pins() -> Vec<AlignmentPin> {
    vec![
        AlignmentPin::new(10.0, 50.0, 6.0),
        AlignmentPin::new(90.0, 30.0, 6.0),
        AlignmentPin::new(90.0, 70.0, 6.0),
    ]
}

fn moved_pins() -> Vec<AlignmentPin> {
    rs_cam_core::compute::alignment_pins::translate(&pins(), 1.0, 0.0)
}

fn pocket() -> ToolpathConfig {
    tc(
        "pocket",
        OperationConfig::Pocket(PocketConfig::default()),
        StockSource::Fresh,
    )
}

fn pin_drill() -> ToolpathConfig {
    tc(
        "pin drill",
        OperationConfig::AlignmentPinDrill(AlignmentPinDrillConfig::default()),
        StockSource::Fresh,
    )
}

fn set(indices: &[usize]) -> BTreeSet<usize> {
    indices.iter().copied().collect()
}

/// The command replaces the whole set in one step, stales the pin drill,
/// and leaves the pocket current.
#[test]
fn set_alignment_pins_stales_the_pin_drill_and_not_the_pocket() {
    let mut s = session_with(vec![pocket(), pin_drill()]);
    let epoch = s.simulation_epoch();

    let effects = s
        .apply(Command::SetAlignmentPins(SetAlignmentPinsArgs {
            pins: moved_pins(),
        }))
        .expect("a pin set is always accepted");

    assert_eq!(s.stock_config().alignment_pins, moved_pins());
    assert_eq!(
        effects.stale,
        set(&[1]),
        "only the pin drill reads the pins"
    );
    assert!(s.get_result(0).is_some(), "the pocket must stay current");
    assert!(s.get_result(1).is_none(), "the pin drill must go stale");
    assert!(
        s.simulation_epoch() > epoch
            && s.simulation_drop_causes_since(epoch)
                .contains(SimulationDropCause::Operations),
        "the simulation carved the old holes, so it goes stale"
    );
}

/// The walk reaches what depends on the pin drill: a later rest operation
/// that starts from the remaining stock.
#[test]
fn set_alignment_pins_stales_what_depends_on_the_pin_drill() {
    let rest = tc(
        "rest",
        OperationConfig::Pocket(PocketConfig::default()),
        StockSource::FromRemainingStock,
    );
    let mut s = session_with(vec![pocket(), pin_drill(), rest]);

    let effects = s
        .apply(Command::SetAlignmentPins(SetAlignmentPinsArgs {
            pins: moved_pins(),
        }))
        .unwrap();

    assert_eq!(effects.stale, set(&[1, 2]));
    assert!(s.get_result(0).is_some());
}

/// The same set again changes nothing. With no pin-drill operation no
/// result reads the pins, so nothing drops and the simulation stays.
#[test]
fn set_alignment_pins_stales_nothing_when_nothing_reads_the_pins() {
    let mut s = session_with(vec![pocket(), pin_drill()]);
    let epoch = s.simulation_epoch();
    let effects = s
        .apply(Command::SetAlignmentPins(SetAlignmentPinsArgs {
            pins: pins(),
        }))
        .unwrap();
    assert!(effects.stale.is_empty(), "{:?}", effects.stale);
    assert_eq!(s.simulation_epoch(), epoch);

    let mut s = session_with(vec![pocket()]);
    let epoch = s.simulation_epoch();
    let effects = s
        .apply(Command::SetAlignmentPins(SetAlignmentPinsArgs {
            pins: moved_pins(),
        }))
        .unwrap();
    assert_eq!(s.stock_config().alignment_pins, moved_pins());
    assert!(effects.stale.is_empty(), "{:?}", effects.stale);
    assert!(s.get_result(0).is_some());
    assert_eq!(s.simulation_epoch(), epoch);
}

/// The add and remove rows are thin forms of the set row: the same stale
/// set, not every result.
#[test]
fn add_and_remove_take_the_set_rule() {
    let mut s = session_with(vec![pocket(), pin_drill()]);
    let effects = s
        .apply(Command::AddAlignmentPin(AddAlignmentPinArgs {
            x: 50.0,
            y: 10.0,
            diameter: 6.0,
        }))
        .unwrap();
    assert_eq!(effects.stale, set(&[1]));
    assert_eq!(s.stock_config().alignment_pins.len(), 4);
    assert!(s.get_result(0).is_some());

    adopt(&mut s, 1);
    let effects = s
        .apply(Command::RemoveAlignmentPin(RemoveAlignmentPinArgs {
            index: 3,
        }))
        .unwrap();
    assert_eq!(effects.stale, set(&[1]));
    assert_eq!(s.stock_config().alignment_pins, pins());
    assert!(s.get_result(0).is_some());

    // A duplicate add changes nothing.
    adopt(&mut s, 1);
    let effects = s
        .apply(Command::AddAlignmentPin(AddAlignmentPinArgs {
            x: 10.0,
            y: 50.0,
            diameter: 6.0,
        }))
        .unwrap();
    assert!(effects.stale.is_empty());
}

/// The mini test piece as it is stored on 2026-10-12: two pins on the flip
/// axis, and the deleted `flip_axis` key.
const MINI_PIECE_WITH_OLD_KEY: &str = r#"format_version = 3

[job]
name = "mini test piece with the old axis key"

[job.stock]
x = 65.0
y = 65.0
z = 24.5
origin_x = 0.0
origin_y = 0.0
origin_z = 0.0
padding = 0.0
auto_from_model = false
flip_axis = "horizontal"

[[job.stock.alignment_pins]]
x = 59.0
y = 32.5
diameter = 6.0

[[job.stock.alignment_pins]]
x = 6.0
y = 32.5
diameter = 6.0

[[setups]]
id = 0
name = "Top"
face_up = "top"

[[setups]]
id = 1
name = "Bottom"
face_up = "bottom"
"#;

/// C1: an old file with `flip_axis` refuses to load, and the message names
/// the key. The same file without the key loads, says the keying fact
/// exactly once, and a save writes no `flip_axis`.
#[test]
fn an_old_file_with_flip_axis_refuses_and_names_the_key() {
    let dir = std::env::temp_dir().join(format!("rs_cam_p3_flip_axis_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("mini.toml");
    std::fs::write(&path, MINI_PIECE_WITH_OLD_KEY).unwrap();
    let message = ProjectSession::load_with_warnings(&path)
        .err()
        .expect("the retired key refuses the file")
        .to_string();
    assert!(message.contains("'job.stock.flip_axis'"), "{message}");

    let without_key = MINI_PIECE_WITH_OLD_KEY.replace("flip_axis = \"horizontal\"\n", "");
    assert_ne!(
        without_key, MINI_PIECE_WITH_OLD_KEY,
        "non-vacuity: the key was there"
    );
    std::fs::write(&path, without_key).unwrap();
    let (session, warnings) =
        ProjectSession::load_with_warnings(&path).expect("the file without the key loads");
    let pin_lines: Vec<String> = warnings
        .iter()
        .filter(|w| matches!(w, ProjectLoadWarning::AlignmentPinsUnkeyed { .. }))
        .map(ProjectLoadWarning::message)
        .collect();
    assert_eq!(pin_lines.len(), 1, "{pin_lines:?}");
    assert!(pin_lines[0].contains("do not key"), "{pin_lines:?}");
    assert_eq!(session.stock_config().alignment_pins.len(), 2);

    let saved = dir.join("saved.toml");
    session.save(&saved).unwrap();
    let text = std::fs::read_to_string(&saved).unwrap();
    assert!(
        text.contains("alignment_pins"),
        "non-vacuity: the pins are saved"
    );
    assert!(
        !text.contains("flip_axis"),
        "the save wrote the deleted key"
    );
    std::fs::remove_dir_all(&dir).ok();
}
