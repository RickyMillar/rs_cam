//! G-BOTTOMPIN, rewritten for geometry selection P3.2 (ruling D9): the floor
//! of a Pocket, Profile, Zigzag or Adaptive 2D is ONE field, and it reaches
//! the emitted motion.
//!
//! Before P3.2 the op `depth` and the Heights Bottom Z were two fields. Only
//! the depth reached the cut: a pinned Bottom Z 14 mm below the floor moved
//! nothing (the old in-crate `pinned_bottom_z_reaches_motion_g_bottompin`).
//! Now the op holds a `Floor` that names its reference, the session resolves
//! it into `ResolvedHeights::bottom_z`, and the ladder runs to that bottom.
//!
//! 1. `Floor::Z { z: -14 }` moves the floor of a 6 mm Pocket to Z -14 in the
//!    emitted motion (the default floor cuts to -6).
//! 2. A pinned Bottom Z on a Pocket refuses at `set_toolpath_heights`, at
//!    `replace_toolpath_config` and at generation.
//! 3. The ladder of each `OpFloor` op ends at the resolved `bottom_z`, for
//!    each floor reference.
//! 4. The `HeightsBottom` ops are three by name, and the `OpFloor` ops are
//!    four by name.
//! 5. L-16: a floor at or above the top refuses before any ladder runs.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test the_floor_reaches_the_motion_g_bottompin
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::session::{polygon_model, single_op_session, single_op_session_with, square_polygon};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::{FloorOwner, OperationConfig, OperationType};
use rs_cam_core::compute::config::{HeightMode, HeightsConfig};
use rs_cam_core::compute::floor::Floor;
use rs_cam_core::compute::operation_configs::{
    AdaptiveConfig, PocketConfig, ProfileConfig, ZigzagConfig,
};
use rs_cam_core::session::{
    Command, ProjectSession, ReplaceToolpathConfigArgs, SessionError, SetToolpathHeightsArgs,
};
use rs_cam_core::toolpath::MoveType;

/// The stock top (mm). The stock hangs below Z 0.
const STOCK_TOP_Z: f64 = 0.0;
/// The stock thickness (mm).
const STOCK_THICKNESS: f64 = 18.0;
/// The depth of the default floor (mm).
const POCKET_DEPTH_MM: f64 = 6.0;
/// The Z floor of arm 1: 8 mm below the default floor, above the stock
/// bottom.
const FLOOR_Z: f64 = -14.0;
/// The half side of the square pocket (mm).
const HALF: f64 = 20.0;
/// The tool diameter (mm).
const TOOL_DIAMETER: f64 = 6.0;
/// The float noise of an emitted Z (mm).
const EPS: f64 = 1e-6;

fn stock() -> StockConfig {
    common::session::stock_under(HALF, STOCK_THICKNESS)
}

fn pocket(floor: Floor) -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        floor,
        depth_per_pass: 2.0,
        ..PocketConfig::default()
    })
}

fn session(op: OperationConfig) -> ProjectSession {
    single_op_session(
        stock(),
        endmill_tool_config(TOOL_DIAMETER),
        polygon_model(vec![square_polygon(HALF)], "g_bottompin"),
        "g_bottompin",
        op,
    )
}

/// The deepest Z of a fed move in the generated toolpath.
fn emitted_floor_z(mut s: ProjectSession) -> f64 {
    let cancel = AtomicBool::new(false);
    let result = s
        .generate_toolpath(0, &cancel)
        .expect("a 40 mm square pocket with a 6 mm end mill generates");
    let floor = result
        .toolpath()
        .moves
        .iter()
        .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
        .map(|m| m.target.z)
        .fold(f64::INFINITY, f64::min);
    assert!(floor.is_finite(), "the pocket emitted no fed move");
    floor
}

fn pinned_bottom() -> HeightsConfig {
    HeightsConfig {
        bottom_z: HeightMode::Manual(-20.0),
        ..HeightsConfig::default()
    }
}

#[track_caller]
fn assert_floor_refusal(result: Result<impl std::fmt::Debug, SessionError>, door: &str) {
    let err = result.expect_err(door);
    let text = err.to_string();
    assert!(
        matches!(err, SessionError::InvalidParam(_))
            && text.contains("takes its floor from its floor field")
            && text.contains("Set Bottom Z to Auto"),
        "{door}: the refusal names the floor field and the fix: {text}"
    );
}

/// Arm 1 — the floor reaches the motion. The default floor cuts to -6; a
/// Z floor at -14 cuts to -14.
#[test]
fn a_z_floor_moves_the_pocket_floor() {
    let default = emitted_floor_z(session(pocket(Floor::top(POCKET_DEPTH_MM))));
    let pinned = emitted_floor_z(session(pocket(Floor::Z { z: FLOOR_Z })));
    assert!(
        (default - (STOCK_TOP_Z - POCKET_DEPTH_MM)).abs() < EPS,
        "the default floor cuts {POCKET_DEPTH_MM} mm below the top, not at {default}"
    );
    assert!(
        (pinned - FLOOR_Z).abs() < EPS,
        "Floor::Z {{ z: {FLOOR_Z} }} must cut to {FLOOR_Z}, not to {pinned}"
    );
}

/// Arm 2 — one floor field: a pinned Bottom Z on a Pocket refuses at the
/// heights door, at the GUI door and at generation.
#[test]
fn a_pinned_bottom_z_on_a_pocket_refuses_at_every_door() {
    let mut s = session(pocket(Floor::top(POCKET_DEPTH_MM)));
    assert_floor_refusal(
        s.apply(Command::SetToolpathHeights(SetToolpathHeightsArgs {
            index: 0,
            heights: pinned_bottom(),
        })),
        "set_toolpath_heights",
    );
    assert!(
        s.toolpath_configs()[0].heights.bottom_z.is_auto(),
        "a refused heights write changes nothing"
    );

    let mut config = s.toolpath_configs()[0].clone();
    config.heights = pinned_bottom();
    assert_floor_refusal(
        s.apply(Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
            index: 0,
            config: Box::new(config),
        })),
        "replace_toolpath_config",
    );
    assert!(s.toolpath_configs()[0].heights.bottom_z.is_auto());

    // The builder is a raw door, so a pin reaches the store; generation is
    // the last guard.
    let mut pinned = single_op_session_with(
        stock(),
        endmill_tool_config(TOOL_DIAMETER),
        polygon_model(vec![square_polygon(HALF)], "g_bottompin"),
        "g_bottompin",
        pocket(Floor::top(POCKET_DEPTH_MM)),
        |tc| tc.heights = pinned_bottom(),
    );
    let cancel = AtomicBool::new(false);
    assert_floor_refusal(pinned.generate_toolpath(0, &cancel), "generation");

    // Control: the same pin on a Heights-bottom op is accepted.
    let mut rough = session(OperationConfig::new_default(OperationType::Adaptive3d));
    let _ = rough
        .apply(Command::SetToolpathHeights(SetToolpathHeightsArgs {
            index: 0,
            heights: pinned_bottom(),
        }))
        .expect("Adaptive3d reads a pinned Bottom Z");
}

/// The four `OpFloor` ops, each with its default config and `floor`.
fn floor_op(op_type: OperationType, floor: Floor) -> OperationConfig {
    match op_type {
        OperationType::Pocket => OperationConfig::Pocket(PocketConfig {
            floor,
            finishing_passes: 1,
            ..PocketConfig::default()
        }),
        OperationType::Profile => OperationConfig::Profile(ProfileConfig {
            floor,
            finishing_passes: 2,
            ..ProfileConfig::default()
        }),
        OperationType::Zigzag => OperationConfig::Zigzag(ZigzagConfig {
            floor,
            ..ZigzagConfig::default()
        }),
        OperationType::Adaptive => OperationConfig::Adaptive(AdaptiveConfig {
            floor,
            ..AdaptiveConfig::default()
        }),
        other => panic!("{other:?} is not an OpFloor op"),
    }
}

/// Arm 3 — the ladder of each `OpFloor` op ends at the resolved `bottom_z`,
/// for each floor reference, and the resolved bottom is the floor.
#[test]
fn each_op_floor_ladder_ends_at_the_bottom_z() {
    let stock_bottom = STOCK_TOP_Z - STOCK_THICKNESS;
    let cases = [
        (Floor::top(5.0), STOCK_TOP_Z - 5.0),
        (Floor::StockTop { depth: 7.5 }, STOCK_TOP_Z - 7.5),
        (Floor::StockBottom { depth: 0.5 }, stock_bottom - 0.5),
        (Floor::StockBottom { depth: -2.0 }, stock_bottom + 2.0),
        (Floor::Z { z: -11.0 }, -11.0),
    ];
    let mut checked = 0;
    for op_type in [
        OperationType::Pocket,
        OperationType::Profile,
        OperationType::Zigzag,
        OperationType::Adaptive,
    ] {
        for (floor, want) in cases {
            let s = session(floor_op(op_type, floor));
            let tc = &s.toolpath_configs()[0];
            let answer = s.toolpath_heights(tc).expect("the heights resolve");
            assert!(
                answer.floor_refusal(&tc.heights).is_none(),
                "{op_type:?} {floor:?}"
            );
            let heights = answer.heights;
            assert!(
                (heights.bottom_z - want).abs() < EPS,
                "{op_type:?} {floor:?}: bottom_z {} != the floor {want}",
                heights.bottom_z
            );
            let ladder = tc.operation.cutting_levels(&heights);
            let last = ladder.last().copied().unwrap_or(f64::NAN);
            // `z_ladder` steps `top - (total / n) * i`, so the last level is
            // the floor within float noise.
            assert!(
                (last - heights.bottom_z).abs() < 1e-9,
                "{op_type:?} {floor:?}: the ladder {ladder:?} must end at bottom_z {}",
                heights.bottom_z
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 20);
}

/// Arm 4 — the floor owners, by name.
#[test]
fn the_floor_owners_are_named() {
    let named = |owner: FloorOwner| {
        let mut names: Vec<String> = OperationType::ALL
            .iter()
            .filter(|t| t.capabilities().floor_owner == owner)
            .map(|t| format!("{t:?}"))
            .collect();
        names.sort();
        names
    };
    assert_eq!(
        named(FloorOwner::HeightsBottom),
        ["Adaptive3d", "UnifiedFinish", "Waterline"],
        "the three ops that read `heights.bottom_z`; change this list only \
         with the new site named"
    );
    assert_eq!(
        named(FloorOwner::OpFloor),
        ["Adaptive", "Pocket", "Profile", "Zigzag"],
        "ruling D9 names four ops"
    );
    for &op_type in OperationType::ALL {
        let has_floor = OperationConfig::new_default(op_type).floor().is_some();
        assert_eq!(
            has_floor,
            op_type.capabilities().floor_owner == FloorOwner::OpFloor,
            "{op_type:?}: an op has a floor field iff its floor owner is OpFloor"
        );
    }
}

/// Arm 5 — L-16: a floor at or above the top refuses before any ladder
/// runs. Before P3.2 the empty ladder fell back to `top - |top - bottom|`,
/// so a floor 5 mm ABOVE the top cut 5 mm BELOW it.
#[test]
fn a_floor_above_the_top_refuses_before_the_ladder() {
    for floor in [
        Floor::Z {
            z: STOCK_TOP_Z + 5.0,
        },
        Floor::top(-5.0),
        Floor::top(0.0),
        Floor::StockBottom {
            depth: -STOCK_THICKNESS - 1.0,
        },
    ] {
        let mut s = session(pocket(floor));
        let tc = s.toolpath_configs()[0].clone();
        let answer = s.toolpath_heights(&tc).expect("the heights resolve");
        assert!(
            answer.floor_refusal(&tc.heights).is_some(),
            "{floor:?}: the session answer names the refusal"
        );
        let cancel = AtomicBool::new(false);
        let err = s
            .generate_toolpath(0, &cancel)
            .expect_err("a floor at or above the top must refuse");
        let text = err.to_string();
        assert!(
            matches!(err, SessionError::InvalidParam(_)) && text.contains("at or above the Top Z"),
            "{floor:?}: the refusal names the floor and the top: {text}"
        );
    }
}
