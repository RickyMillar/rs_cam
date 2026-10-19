//! Sentry (B18, third part): **a `pre_gcode` snippet does not let the
//! next toolpath traverse below the obstacle plane**.
//!
//! ## The concern
//!
//! A `pre_gcode` or `post_gcode` snippet resets the builder position
//! (`program_builder::push_raw_snippet`): the builder does not know what
//! the snippet did, so the next rapid is Z-first to the rapid's own
//! height. The join check (`JoinStart`) reads the end of the previous
//! toolpath and does not see that reset.
//!
//! ## Why the reset is covered
//!
//! Since K3 the obstacle step starts every toolpath in a setup with an
//! obstacle with a `MoveIntent::ObstacleClearance` rapid at the plane. The
//! Z-first rapid after the snippet therefore rises to the plane before
//! any XY traverse.
//!
//! ## What this test measures
//!
//! Two traces on each side of a clamp, a Manual retract below the clamp
//! top, and a `pre_gcode` snippet on the second trace. In the exported
//! G-code, after the snippet:
//!
//! 1. the first line with an axis word moves Z only, to the plane or
//!    higher;
//! 2. no XY traverse comes before the tool is at the plane.
//!
//! Control: with no clamp, the first Z after the snippet is below the
//! plane, so the rise comes from the obstacle step.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::session::{stock_under, toolpath_config};
use common::tools::endmill_tool_config;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{HeightMode, HeightsConfig, SAFE_Z_CLEARANCE_MM};
use rs_cam_core::compute::operation_configs::TraceConfig;
use rs_cam_core::gcode::ToolLoadExportPolicy;
use rs_cam_core::ids::ObstacleId;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    AddObstacleArgs, Command, Obstacle, ObstacleGeometry, ProjectSession, ProjectSessionBuilder,
};

const HALF: f64 = 25.0;
const DEPTH: f64 = 6.0;
const CLAMP_TOP: f64 = 20.0;
const CLEARANCE: f64 = 1.0;
/// The operator's retract: below the clamp top.
const MANUAL_RETRACT: f64 = 10.0;
/// The snippet. A comment, so it moves nothing; the builder still resets
/// its position after it.
const SNIPPET: &str = "(B18 snippet)";

fn plane() -> f64 {
    CLAMP_TOP + CLEARANCE + SAFE_Z_CLEARANCE_MM
}

/// Two traces on each side of X = 0, the second with the snippet. With
/// `clamp`, a clamp stands between them. The stock top is at Z 0, so the
/// export frame is the toolpath frame.
fn session(clamp: bool) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stock_under(HALF, DEPTH));
    let tool_idx = builder.add_tool(endmill_tool_config(6.0));
    let tool_id = builder.tools()[tool_idx].id.0;
    for (name, x0, x1) in [("Left", -20.0, -10.0), ("Right", 10.0, 20.0)] {
        let model_id = builder.add_model(common::session::polygon_model(
            vec![Polygon2::rectangle(x0, -5.0, x1, 5.0)],
            name,
        ));
        let op = OperationConfig::Trace(TraceConfig {
            depth: 1.0,
            depth_per_pass: 1.0,
            ..TraceConfig::default()
        });
        let mut cfg = toolpath_config(name, op, tool_id, model_id);
        cfg.heights = HeightsConfig {
            retract_z: HeightMode::Manual(MANUAL_RETRACT),
            clearance_z: HeightMode::Manual(MANUAL_RETRACT + 2.0),
            ..HeightsConfig::default()
        };
        if name == "Right" {
            cfg.pre_gcode = Some(SNIPPET.to_owned());
        }
        builder.add_toolpath(0, cfg).expect("add toolpath");
    }
    let mut session = builder.build();
    if clamp {
        let _ = session
            .apply(Command::AddObstacle(AddObstacleArgs {
                setup_index: 0,
                obstacle: Box::new(Obstacle {
                    id: ObstacleId(0),
                    name: "Middle clamp".to_owned(),
                    enabled: true,
                    clearance: CLEARANCE,
                    geometry: ObstacleGeometry::Box {
                        min: [-3.0, -40.0, -DEPTH],
                        max: [3.0, 40.0, CLAMP_TOP],
                    },
                }),
            }))
            .expect("add the clamp");
    }
    let cancel = AtomicBool::new(false);
    for index in 0..session.toolpath_count() {
        session
            .generate_toolpath(index, &cancel)
            .expect("generation must succeed");
    }
    session
}

fn export(session: &ProjectSession) -> String {
    session
        .export_gcode_checked(
            None,
            ToolLoadExportPolicy {
                accept_unmodeled: true,
                accept_exceeded: true,
            },
        )
        .expect("export")
        .gcode
}

/// The X, Y and Z words of one G-code line, comments removed.
fn axis_words(line: &str) -> (Option<f64>, Option<f64>, Option<f64>) {
    let code = line.split(['(', ';']).next().unwrap_or("");
    let mut words = (None, None, None);
    for token in code.split_whitespace() {
        let (letter, value) = token.split_at(1);
        let value = value.parse::<f64>().ok();
        match letter {
            "X" | "x" => words.0 = value,
            "Y" | "y" => words.1 = value,
            "Z" | "z" => words.2 = value,
            _ => {}
        }
    }
    words
}

/// The axis-word lines after the snippet line: (has XY, Z after the line).
fn lines_after_snippet(gcode: &str) -> Vec<(bool, Option<f64>, String)> {
    let lines: Vec<&str> = gcode.lines().collect();
    let at = lines
        .iter()
        .position(|l| l.trim() == SNIPPET)
        .unwrap_or_else(|| panic!("the snippet is not in the G-code:\n{gcode}"));
    lines[at + 1..]
        .iter()
        .filter_map(|line| {
            let (x, y, z) = axis_words(line);
            (x.is_some() || y.is_some() || z.is_some())
                .then(|| (x.is_some() || y.is_some(), z, (*line).to_owned()))
        })
        .collect()
}

#[test]
fn after_a_snippet_the_join_rises_to_the_plane_before_it_traverses_b18() {
    let gcode = export(&session(true));
    let after = lines_after_snippet(&gcode);
    let (first_xy, first_z, first_line) = after.first().expect("motion after the snippet");
    assert!(
        !first_xy,
        "the first move after the snippet must be Z only: {first_line}"
    );
    let z = first_z.expect("a Z word");
    assert!(
        z >= plane() - 1e-3,
        "the first move after the snippet goes to Z {z}, below the plane {}",
        plane()
    );
    // No XY traverse before the tool is at the plane.
    let mut current_z = z;
    for (xy, z, line) in &after {
        if *xy {
            assert!(
                current_z >= plane() - 1e-3,
                "an XY traverse at Z {current_z} below the plane {} after the snippet: {line}",
                plane()
            );
            break;
        }
        if let Some(z) = z {
            current_z = *z;
        }
    }
}

/// Control: with no clamp, nothing lifts the join to the plane. The rise
/// in the test above comes from the obstacle step.
#[test]
fn without_a_clamp_the_join_stays_at_the_op_heights_b18() {
    let gcode = export(&session(false));
    let after = lines_after_snippet(&gcode);
    let z = after
        .iter()
        .find_map(|(_, z, _)| *z)
        .expect("a Z after the snippet");
    assert!(
        z < plane() - 1e-3,
        "control: with no clamp the first Z after the snippet is {z}, not below the plane"
    );
}
