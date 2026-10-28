//! D53 (`planning/adaptive_research_2026-10-23/phase3/d50/README.md` §2;
//! integration step 7.5b, ported from the research branch 3682d1dcc):
//! a `DerivedRestRegions` consumer whose tool is larger than the source's
//! refuses to generate when its containment keeps the tool centre on the
//! regions (`Center`, `Outside`).
//!
//! The regions are tool-centre positions of the source's tool (D48). A
//! centre there keeps a tool no larger than the source's inside the
//! source's boundary; a larger tool can cut past it by the difference of
//! the radii. `Inside` containment insets the regions by the consumer's own
//! radius, so it is safe with any tool.
//!
//! The session is the D47 channel pair: a 3D Rough with its rest analysis
//! on (Ø6), and a contour-parallel rough on its derived rest regions. The
//! consumer's tool and containment vary:
//! 1. Ø8 with `Center` or `Outside`: refused, and the refusal names both
//!    radii and both fixes;
//! 2. Ø6 with `Center`, Ø4 with `Center`: not refused by this rule, and
//!    the consumer generates;
//! 3. Ø8 with `Inside`: not refused by this rule. On the narrow channel the
//!    inset leaves no area, so G-BOUNDCOLLAPSE refuses it (operator ruling
//!    2026-10-28). On a channel twice the Ø8 tool wide, the consumer
//!    generates.

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
use rs_cam_core::compute::config::{
    BoundaryConfig, BoundaryContainment, BoundarySource, StockSource,
};
use rs_cam_core::compute::operation_configs::{Adaptive3dConfig, ClearingStrategy};
use rs_cam_core::ids::ToolId;
use rs_cam_core::session::{
    AddToolpathArgs, Command, ProjectSession, ProjectSessionBuilder, SessionError,
};
use std::sync::atomic::AtomicBool;

// The stepped pocket of the D44 and D47 sentries.
const TOP_Z: f64 = 10.0;
const FLOOR_Z: f64 = 4.0;
const MESH_HALF: f64 = 40.0;
const MESH_STEP: f64 = 0.5;
const CHANNEL_HALF_LENGTH: f64 = 30.0;
const CHANNEL_HALF_WIDTH: f64 = 4.8;
/// A channel `2 · LARGER_DIAMETER` = 16 mm wide. Its rest region keeps area
/// under an `Inside` inset by the Ø8 radius (measured: one region of about
/// 3339 mm², against about 249 mm² on the 4.8 mm half-width channel).
const WIDE_CHANNEL_HALF_WIDTH: f64 = LARGER_DIAMETER;
const WIDE_HALF_X: f64 = 34.0;
const WIDE_HALF_Y: f64 = 24.0;
const WIDE_FLOOR_Z: f64 = 7.0;
const SOURCE_DIAMETER: f64 = 6.0;
const LARGER_DIAMETER: f64 = 8.0;
const SMALLER_DIAMETER: f64 = 4.0;

fn rough(strategy: ClearingStrategy) -> OperationConfig {
    OperationConfig::Adaptive3d(Adaptive3dConfig {
        stepover: 2.0,
        depth_per_pass: 3.0,
        clearing_strategy: strategy,
        ..Adaptive3dConfig::default()
    })
}

/// Op 0 the Ø6 rough (rest analysis on), op 1 its consumer with a tool of
/// `consumer_diameter` and `containment`. Op 0 is generated.
fn session(consumer_diameter: f64, containment: BoundaryContainment) -> ProjectSession {
    session_on(CHANNEL_HALF_WIDTH, consumer_diameter, containment)
}

/// [`session`] on a channel of half-width `channel_half_width`.
fn session_on(
    channel_half_width: f64,
    consumer_diameter: f64,
    containment: BoundaryContainment,
) -> ProjectSession {
    let mesh = height_field(MESH_HALF, MESH_STEP, |x, y| {
        if x.abs() <= CHANNEL_HALF_LENGTH && y.abs() <= channel_half_width {
            FLOOR_Z
        } else if x.abs() <= WIDE_HALF_X && y.abs() <= WIDE_HALF_Y {
            WIDE_FLOOR_Z
        } else {
            TOP_Z
        }
    });
    let mut stock = stock_over(MESH_HALF, TOP_Z);
    rs_cam_core::compute::fixture_values::fill_unset_stock(&mut stock);
    let mut builder = ProjectSessionBuilder::new().stock(stock);
    let source_idx = builder.add_tool(endmill_tool_config(SOURCE_DIAMETER));
    let source_tool = ToolId::from_raw(builder.tools()[source_idx].id.raw());
    let consumer_tool = if (consumer_diameter - SOURCE_DIAMETER).abs() < f64::EPSILON {
        source_tool
    } else {
        let idx = builder.add_tool(endmill_tool_config(consumer_diameter));
        ToolId::from_raw(builder.tools()[idx].id.raw())
    };
    let model_id = builder.add_model(mesh_model(mesh, "channel"));
    let mut op0 = toolpath_config(
        "rough",
        rough(ClearingStrategy::GeodesicAdaptive),
        source_tool,
        model_id,
    );
    op0.rest_analysis.enabled = true;
    let index0 = builder.add_toolpath(0, op0).expect("add the rough");
    let mut session = builder.build();
    let source_toolpath_id = session.toolpath_configs()[index0].id;
    let mut op1 = toolpath_config(
        "edges, CP rest",
        rough(ClearingStrategy::ContourParallel),
        consumer_tool,
        model_id,
    );
    op1.stock_source = StockSource::default();
    op1.boundary = BoundaryConfig {
        enabled: true,
        source: BoundarySource::DerivedRestRegions { source_toolpath_id },
        containment,
        offset: 0.0,
    };
    let _ = session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(op1),
        }))
        .expect("add the consumer");
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generate the rough");
    session
}

fn generate_consumer(session: &mut ProjectSession) -> Result<(), SessionError> {
    let cancel = AtomicBool::new(false);
    session.generate_toolpath(1, &cancel).map(|_| ())
}

#[test]
fn a_larger_rest_consumer_tool_refuses() {
    let (own, theirs) = (LARGER_DIAMETER / 2.0, SOURCE_DIAMETER / 2.0);
    // 1. A larger tool with its centre on the regions refuses.
    for containment in [BoundaryContainment::Center, BoundaryContainment::Outside] {
        let mut s = session(LARGER_DIAMETER, containment);
        match generate_consumer(&mut s) {
            Err(SessionError::OperationFailed(text)) => {
                for needle in [
                    format!("{own:.3} mm"),
                    format!("{theirs:.3} mm"),
                    "Inside".to_owned(),
                    "no larger than".to_owned(),
                    format!("Ø{SOURCE_DIAMETER:.3}"),
                ] {
                    assert!(
                        text.contains(&needle),
                        "{containment:?}: the refusal does not name `{needle}`: {text}"
                    );
                }
                assert!(
                    s.result_at(1).is_none(),
                    "{containment:?}: a refused consumer holds a result"
                );
            }
            other => panic!("{containment:?}: a larger tool was not refused: {other:?}"),
        }
    }

    // 2. A tool no larger than the source's: the consumer generates.
    for (diameter, containment) in [
        (SOURCE_DIAMETER, BoundaryContainment::Center),
        (SMALLER_DIAMETER, BoundaryContainment::Center),
    ] {
        let mut s = session(diameter, containment);
        if let Err(e) = generate_consumer(&mut s) {
            panic!("Ø{diameter} with {containment:?} did not generate: {e}");
        }
        assert!(
            s.result_at(1).is_some(),
            "Ø{diameter} with {containment:?}: no result"
        );
    }

    // 3. Inside with the larger tool. D53 does not refuse it. On the narrow
    // channel the inset leaves no area: G-BOUNDCOLLAPSE refuses, with the
    // read door's sentence. On the wide channel the consumer generates.
    let mut narrow = session(LARGER_DIAMETER, BoundaryContainment::Inside);
    let collapse = narrow
        .boundary_collapse(1)
        .expect("the inputs resolve")
        .expect("narrow channel: the Ø8 Inside inset leaves no area");
    match generate_consumer(&mut narrow) {
        Err(SessionError::OperationFailed(text)) => assert_eq!(
            text,
            collapse.message(),
            "narrow channel: the refusal is the G-BOUNDCOLLAPSE sentence"
        ),
        other => panic!("narrow channel: Ø8 Inside was not refused: {other:?}"),
    }
    assert!(
        narrow.result_at(1).is_none(),
        "narrow channel: a refused consumer holds a result"
    );

    let mut wide = session_on(
        WIDE_CHANNEL_HALF_WIDTH,
        LARGER_DIAMETER,
        BoundaryContainment::Inside,
    );
    assert_eq!(
        wide.boundary_collapse(1).expect("the inputs resolve"),
        None,
        "wide channel: the Ø8 Inside inset keeps area"
    );
    if let Err(e) = generate_consumer(&mut wide) {
        panic!("wide channel: Ø8 with Inside did not generate: {e}");
    }
    assert!(wide.result_at(1).is_some(), "wide channel: no result");
}

/// G-C (§2 a): the read doors of the GUI. `larger_rest_consumer` is `Some`
/// for the larger tool whatever the containment, and `None` for an equal
/// tool. `larger_rest_consumer_refusal` is the generation refusal word for
/// word, and `None` under `Inside`. `boundary_collapse_with` answers "would
/// Inside collapse?" before any write: yes on the narrow channel, no on the
/// wide one.
#[test]
fn the_read_doors_answer_as_generation_does_g_c() {
    let (own, theirs) = (LARGER_DIAMETER / 2.0, SOURCE_DIAMETER / 2.0);
    for containment in [
        BoundaryContainment::Center,
        BoundaryContainment::Outside,
        BoundaryContainment::Inside,
    ] {
        let s = session(LARGER_DIAMETER, containment);
        let source = s.toolpath_configs()[0].id;
        let larger = s
            .larger_rest_consumer(1, source)
            .expect("a larger tool, whatever the containment");
        assert!((larger.tool_radius_mm - own).abs() < 1e-9);
        assert!((larger.source_tool_radius_mm - theirs).abs() < 1e-9);
        assert!((larger.overrun_mm() - (own - theirs)).abs() < 1e-9);
    }
    let mut center = session(LARGER_DIAMETER, BoundaryContainment::Center);
    let refusal = center
        .larger_rest_consumer_refusal(1)
        .expect("Center refuses");
    match generate_consumer(&mut center) {
        Err(SessionError::OperationFailed(text)) => assert_eq!(text, refusal),
        other => panic!("Center did not refuse: {other:?}"),
    }
    let inside = session(LARGER_DIAMETER, BoundaryContainment::Inside);
    assert_eq!(inside.larger_rest_consumer_refusal(1), None);
    let equal = session(SOURCE_DIAMETER, BoundaryContainment::Center);
    let source = equal.toolpath_configs()[0].id;
    assert_eq!(equal.larger_rest_consumer(1, source), None);

    let to_inside = |s: &ProjectSession| {
        let mut boundary = s.toolpath_configs()[1].boundary.clone();
        boundary.containment = BoundaryContainment::Inside;
        s.boundary_collapse_with(1, &boundary)
            .expect("the inputs resolve")
    };
    assert!(
        to_inside(&center).is_some(),
        "Inside collapses on the narrow channel"
    );
    let wide = session_on(
        WIDE_CHANNEL_HALF_WIDTH,
        LARGER_DIAMETER,
        BoundaryContainment::Center,
    );
    assert_eq!(
        to_inside(&wide),
        None,
        "Inside keeps area on the wide channel"
    );
}
