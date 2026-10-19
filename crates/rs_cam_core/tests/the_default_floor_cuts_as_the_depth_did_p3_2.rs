//! Geometry selection P3.2, section 4 of `P3_DESIGN.md` (byte identity):
//! the default floor `Floor::Top { depth }` cuts as the op `depth` did.
//!
//! Before P3.2 `OperationConfig::cutting_levels(top_z)` built the ladder of
//! the four ops as `DepthStepping { start_z: top, final_z: top -
//! depth.abs(), .. }`, and the Auto bottom was `top - op_depth.abs()`. Now
//! the ladder runs from the resolved top to the resolved `bottom_z`, which
//! is the floor. For a positive depth below the Top Z the two must be the
//! same bits, so no project without a pin moves (ruling L-3: the default
//! reference is the resolved Top Z, so a pinned top keeps `top - depth`).
//!
//! Each of Pocket, Profile, Zigzag and Adaptive 2D, with an auto top and
//! with a pinned top: the session ladder (`toolpath_heights`, then
//! `cutting_levels`) and the generation bottom equal the old formula,
//! compared with `==`, no tolerance. The old formula is written here
//! independently of the code under test.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test the_default_floor_cuts_as_the_depth_did_p3_2
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::session::{polygon_model, single_op_session_with, square_polygon, stock_under};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::HeightMode;
use rs_cam_core::compute::floor::Floor;
use rs_cam_core::compute::operation_configs::{
    AdaptiveConfig, PocketConfig, ProfileConfig, ZigzagConfig,
};
use rs_cam_core::ops::depth::DepthStepping;
use rs_cam_core::session::{GenerateToolpathArgs, Job, JobHandle, ProjectSession};

/// The depth (mm): not a multiple of the step, so the ladder redistributes.
const DEPTH: f64 = 7.3;
/// The depth per pass (mm).
const STEP: f64 = 1.7;
/// The stock thickness (mm).
const STOCK_THICKNESS: f64 = 18.0;
/// The pinned Top Z of the pinned arm (mm): below the stock top, so a floor
/// from the stock top would move it.
const PINNED_TOP: f64 = -2.5;
/// The half side of the square drawing (mm).
const HALF: f64 = 20.0;

/// The op with `floor: Floor::Top { depth: DEPTH }`, and its spring passes.
fn op(op_type: OperationType) -> (OperationConfig, usize) {
    let floor = Floor::top(DEPTH);
    match op_type {
        OperationType::Pocket => (
            OperationConfig::Pocket(PocketConfig {
                floor,
                depth_per_pass: STEP,
                finishing_passes: 1,
                ..PocketConfig::default()
            }),
            1,
        ),
        OperationType::Profile => (
            OperationConfig::Profile(ProfileConfig {
                floor,
                depth_per_pass: STEP,
                finishing_passes: 2,
                ..ProfileConfig::default()
            }),
            2,
        ),
        OperationType::Zigzag => (
            OperationConfig::Zigzag(ZigzagConfig {
                floor,
                depth_per_pass: STEP,
                ..ZigzagConfig::default()
            }),
            0,
        ),
        OperationType::Adaptive => (
            OperationConfig::Adaptive(AdaptiveConfig {
                floor,
                depth_per_pass: STEP,
                ..AdaptiveConfig::default()
            }),
            0,
        ),
        other => panic!("{other:?} is not a floor op"),
    }
}

fn session(op: OperationConfig, top: HeightMode) -> ProjectSession {
    single_op_session_with(
        stock_under(HALF, STOCK_THICKNESS),
        endmill_tool_config(6.0),
        polygon_model(vec![square_polygon(HALF)], "p3_2_default_floor"),
        "p3_2_default_floor",
        op,
        |tc| tc.heights.top_z = top,
    )
}

/// The ladder before P3.2, from `catalog.rs` at 86265095: the Pocket and
/// Profile arm, and the `uniform` arm of Zigzag and Adaptive (no spring
/// pass).
fn old_ladder(top_z: f64, finishing_passes: usize) -> Vec<f64> {
    DepthStepping {
        start_z: top_z,
        final_z: top_z - DEPTH.abs(),
        max_step_down: STEP,
        finishing_passes,
    }
    .all_levels()
}

/// The (top, bottom) that generation hands the op.
fn generation_heights(session: &mut ProjectSession) -> (f64, f64) {
    let cancel = AtomicBool::new(false);
    let JobHandle::GenerateToolpath(handle) = session
        .start(
            Job::GenerateToolpath(GenerateToolpathArgs { index: 0 }),
            &cancel,
        )
        .expect("step (i) captures the generation inputs")
    else {
        panic!("the generate_toolpath row answers its own handle variant");
    };
    let snapshot = handle.request_snapshot();
    let read = |key: &str| snapshot["heights"][key].as_f64().expect("a height");
    (read("top_z"), read("bottom_z"))
}

#[test]
fn the_default_floor_cuts_as_the_depth_did_p3_2() {
    let mut checked = 0;
    for op_type in [
        OperationType::Pocket,
        OperationType::Profile,
        OperationType::Zigzag,
        OperationType::Adaptive,
    ] {
        for (arm, top) in [
            ("auto top", HeightMode::Auto),
            ("pinned top", HeightMode::Manual(PINNED_TOP)),
        ] {
            let (config, finishing_passes) = op(op_type);
            let mut s = session(config, top);
            let tc = s.toolpath_configs()[0].clone();
            let answer = s.toolpath_heights(&tc).expect("the heights resolve");
            let heights = answer.heights;
            let want_top = match top {
                HeightMode::Manual(z) => z,
                HeightMode::Auto | HeightMode::FromReference(_) => 0.0,
            };
            assert_eq!(
                heights.top_z.to_bits(),
                want_top.to_bits(),
                "{op_type:?} {arm}: the top"
            );

            // The bottom: the old Auto bottom `top - op_depth.abs()`.
            assert_eq!(
                heights.bottom_z.to_bits(),
                (want_top - DEPTH.abs()).to_bits(),
                "{op_type:?} {arm}: the bottom must be the old auto bottom"
            );

            // The ladder, bit for bit.
            let new = tc.operation.cutting_levels(&heights);
            let old = old_ladder(want_top, finishing_passes);
            let bits = |v: &[f64]| v.iter().map(|z| z.to_bits()).collect::<Vec<_>>();
            assert_eq!(
                bits(&new),
                bits(&old),
                "{op_type:?} {arm}: the ladder moved: {new:?} != {old:?}"
            );
            assert!(old.len() >= 5, "fixture: the ladder has several levels");

            // Generation reads the same rule.
            let (gen_top, gen_bottom) = generation_heights(&mut s);
            assert_eq!(
                (gen_top.to_bits(), gen_bottom.to_bits()),
                (heights.top_z.to_bits(), heights.bottom_z.to_bits()),
                "{op_type:?} {arm}: generation and the session answer differ"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 8);
}
