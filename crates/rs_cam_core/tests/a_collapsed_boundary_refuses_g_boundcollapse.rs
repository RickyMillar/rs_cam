//! G-BOUNDCOLLAPSE (operator ruling 2026-10-28;
//! `planning/adaptive_research_2026-10-23/phase3/d67/README.md` §3): a
//! machining boundary that collapses refuses generation.
//!
//! Before the ruling, a containment set that came out empty passed the
//! toolpath through UNCLIPPED, with only the report-only
//! `boundary_clip_dropped` finding. So a cut outside a declared boundary (a
//! frame or a clamp zone) could reach the program.
//!
//! The session is one 3D Rough on a stepped pocket, with a `Stock`
//! boundary. For Contour Parallel and Geodesic Adaptive:
//! 1. `Inside` with a user offset that leaves a square smaller than the
//!    tool: the containment inset collapses. Refused.
//! 2. `Center` with a user offset larger than the stock: the user offset
//!    collapses. Refused.
//! 3. `Inside` with no offset: the op generates.
//!
//! Each refusal names the op, the boundary and the fix, and the read door
//! `ProjectSession::boundary_collapse` gives the same sentence before a
//! generate.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use common::meshes::height_field;
use common::session::{mesh_model, stock_over, toolpath_config};
use common::tools::endmill_tool_config;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{BoundaryConfig, BoundaryContainment, BoundarySource};
use rs_cam_core::compute::operation_configs::{Adaptive3dConfig, ClearingStrategy};
use rs_cam_core::ids::ToolId;
use rs_cam_core::session::{ProjectSession, ProjectSessionBuilder, SessionError};
use std::sync::atomic::AtomicBool;

const TOP_Z: f64 = 10.0;
const FLOOR_Z: f64 = 6.0;
const MESH_HALF: f64 = 20.0;
const MESH_STEP: f64 = 0.5;
const POCKET_HALF: f64 = 10.0;
const TOOL_DIAMETER: f64 = 6.0;
/// `stock_over` adds 2 mm on each side, so the stock is
/// `2 · (MESH_HALF + 2)` wide.
const STOCK_HALF: f64 = MESH_HALF + 2.0;
/// Leaves a square of side `2 · (STOCK_HALF − 20) = 4` mm, less than the
/// tool diameter, so an `Inside` inset by the tool radius removes it.
const SHRINK_UNDER_THE_TOOL: f64 = -(STOCK_HALF - 2.0);
/// Larger than the stock half-width, so the user offset alone removes it.
const SHRINK_PAST_THE_STOCK: f64 = -(STOCK_HALF + 3.0);

const OP_NAME: &str = "rough in a frame";

fn session(
    strategy: ClearingStrategy,
    containment: BoundaryContainment,
    offset: f64,
) -> ProjectSession {
    let mesh = height_field(MESH_HALF, MESH_STEP, |x, y| {
        if x.abs() <= POCKET_HALF && y.abs() <= POCKET_HALF {
            FLOOR_Z
        } else {
            TOP_Z
        }
    });
    let mut stock = stock_over(MESH_HALF, TOP_Z);
    rs_cam_core::compute::fixture_values::fill_unset_stock(&mut stock);
    let mut builder = ProjectSessionBuilder::new().stock(stock);
    let tool_idx = builder.add_tool(endmill_tool_config(TOOL_DIAMETER));
    let tool = ToolId::from_raw(builder.tools()[tool_idx].id.raw());
    let model_id = builder.add_model(mesh_model(mesh, "pocket"));
    let mut op = toolpath_config(
        OP_NAME,
        OperationConfig::Adaptive3d(Adaptive3dConfig {
            stepover: 2.0,
            depth_per_pass: 3.0,
            clearing_strategy: strategy,
            ..Adaptive3dConfig::default()
        }),
        tool,
        model_id,
    );
    op.boundary = BoundaryConfig {
        enabled: true,
        source: BoundarySource::Stock,
        containment,
        offset,
    };
    builder.add_toolpath(0, op).expect("add the rough");
    builder.build()
}

#[test]
fn a_collapsed_boundary_refuses() {
    for strategy in [
        ClearingStrategy::ContourParallel,
        ClearingStrategy::GeodesicAdaptive,
    ] {
        for (containment, offset) in [
            (BoundaryContainment::Inside, SHRINK_UNDER_THE_TOOL),
            (BoundaryContainment::Center, SHRINK_PAST_THE_STOCK),
        ] {
            let case = format!("{strategy:?}, {containment:?}, offset {offset}");
            let mut s = session(strategy, containment, offset);

            // The read door answers before a generate.
            let collapse = s
                .boundary_collapse(0)
                .expect("the inputs resolve")
                .unwrap_or_else(|| panic!("{case}: the read door finds no collapse"));
            assert_eq!(collapse.containment, containment, "{case}");
            assert_eq!(collapse.op_name, OP_NAME, "{case}");

            let cancel = AtomicBool::new(false);
            match s.generate_toolpath(0, &cancel) {
                Err(SessionError::OperationFailed(text)) => {
                    assert_eq!(
                        text,
                        collapse.message(),
                        "{case}: the refusal is the read door's sentence"
                    );
                    for needle in [
                        format!("'{OP_NAME}'"),
                        "Stock machining boundary".to_owned(),
                        format!("{containment:?}"),
                        format!("{TOOL_DIAMETER:.3} mm tool"),
                        "smaller tool".to_owned(),
                        "less boundary offset".to_owned(),
                        "another containment".to_owned(),
                    ] {
                        assert!(
                            text.contains(&needle),
                            "{case}: the refusal does not name `{needle}`: {text}"
                        );
                    }
                }
                other => panic!("{case}: a collapsed boundary was not refused: {other:?}"),
            }
            assert!(
                s.result_at(0).is_none(),
                "{case}: a refused op holds a result"
            );
        }
    }
}

#[test]
fn a_normal_boundary_generates() {
    for strategy in [
        ClearingStrategy::ContourParallel,
        ClearingStrategy::GeodesicAdaptive,
    ] {
        let mut s = session(strategy, BoundaryContainment::Inside, 0.0);
        assert_eq!(
            s.boundary_collapse(0).expect("the inputs resolve"),
            None,
            "{strategy:?}: the read door finds a collapse in a normal boundary"
        );
        let cancel = AtomicBool::new(false);
        if let Err(e) = s.generate_toolpath(0, &cancel) {
            panic!("{strategy:?}: a normal boundary did not generate: {e}");
        }
        assert!(s.result_at(0).is_some(), "{strategy:?}: no result");
    }
}
