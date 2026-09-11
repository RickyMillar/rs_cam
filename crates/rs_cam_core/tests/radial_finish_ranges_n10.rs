//! **N10 sentry.** `radial_finish` divides by two dials that declare no
//! domain, so every route can hand the divide a zero or a negative number.
//!
//! # The two divide sites
//!
//! `crates/rs_cam_core/src/radial_finish.rs:96`
//! `let num_spokes = (360.0 / params.angular_step).ceil() as usize;`
//!
//! `crates/rs_cam_core/src/radial_finish.rs:108`
//! `let num_points = (max_radius / params.point_spacing).ceil() as usize + 1;`
//!
//! A `0.0` gives `inf`, which casts to `usize::MAX`. The spoke loop then
//! runs until the operator cancels it, and `Vec::with_capacity` on the
//! point count overflows. A negative value saturates the cast to `0` and
//! emits a toolpath with no cutting motion, which reports no error of its
//! own about the dial that caused it.
//!
//! # The ruling
//!
//! The operator ruled on 2026-09-11
//! (`planning/arch_consolidation_2026-09-09/STATUS.md`, "Operator
//! rulings", row N10): both dials carry `ParamRange::greater_than(0.0)`
//! in the `peck_depth` shape. Both numbers are OPERATOR-AUTHORED on that
//! date. They are not a vendor figure and not a handbook figure.
//!
//! # What each test covers
//!
//! 1. The DR-LIVE gate in `set_toolpath_param` refuses the value BEFORE
//!    the write, and leaves the stored dial untouched. This is the MCP
//!    and CLI route.
//! 2. The published schema carries the domain, so an agent reads it
//!    before it guesses.
//! 3. The generator refuses the value a project file or the GUI
//!    inspector can carry, because the registry gate never sees those.
//!
//! # Why test 3 drives a NEGATIVE value and never a zero
//!
//! A zero at the generator is the `usize::MAX` spoke loop. The pre-fix
//! run of this sentry must FAIL, not hang, so the generator test uses
//! `-5.0` and `-0.5`. Both are fast on the pre-fix code: they emit an
//! empty toolpath. The assertion reads the refusal TEXT, because the
//! empty-generation gate (G-ENTRYEMPTY) already refuses an empty
//! toolpath and names neither dial. A bare `is_err()` would pass on the
//! pre-fix code and prove nothing.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::operation_configs::RadialFinishConfig;
use rs_cam_core::session::ProjectSession;
use serde_json::json;

use common::make_endmill_6mm;
use common::meshes::plateau;
use common::session::{mesh_model, single_op_session, stock_under};

/// Half-extent of the plateau top face, in mm.
const HALF: f64 = 20.0;
/// Plateau depth, in mm. The stock hangs below `z = 0` to match it.
const DEPTH: f64 = 10.0;

/// One RadialFinish toolpath over a plateau, wired through the real
/// `ProjectSession::add_*` entry points.
fn radial_session(cfg: RadialFinishConfig) -> ProjectSession {
    single_op_session(
        stock_under(HALF, DEPTH),
        make_endmill_6mm(),
        mesh_model(plateau(2.0 * HALF, DEPTH), "plateau"),
        "Radial Finish",
        OperationConfig::RadialFinish(cfg),
    )
}

/// The two stored dials, read back off the session.
fn dials(session: &ProjectSession) -> (f64, f64) {
    match &session.toolpath_configs()[0].operation {
        OperationConfig::RadialFinish(cfg) => (cfg.angular_step, cfg.point_spacing),
        other => panic!("expected RadialFinish, got {other:?}"),
    }
}

/// Assertion 1. The setter refuses a non-positive `angular_step` and
/// keeps the previous value. A setter that rejects and mutates anyway is
/// worse than one that accepts.
#[test]
fn set_toolpath_param_refuses_a_non_positive_angular_step() {
    let mut s = radial_session(RadialFinishConfig::default());
    let (before, _) = dials(&s);

    for bad in [0.0_f64, -5.0, -0.000_001] {
        let err = s
            .set_toolpath_param(0, "angular_step", json!(bad))
            .expect_err(
                "the generator divides 360 by this dial, so a zero or a negative \
                 value must be refused at the setter",
            );
        let msg = format!("{err}");
        assert!(
            msg.contains("angular_step") && msg.contains("outside the accepted range"),
            "the refusal must name the param and the domain; got: {msg}"
        );
        assert!(
            (dials(&s).0 - before).abs() < 1e-12,
            "a refused set must leave the dial untouched; it became {}",
            dials(&s).0
        );
    }

    // The accepting side of the same boundary.
    let _ = s
        .set_toolpath_param(0, "angular_step", json!(10.0))
        .expect("a real angular step must be accepted");
    assert!((dials(&s).0 - 10.0).abs() < 1e-12);
}

/// Assertion 1, the second dial. `point_spacing` divides the spoke
/// radius, so it carries the same domain.
#[test]
fn set_toolpath_param_refuses_a_non_positive_point_spacing() {
    let mut s = radial_session(RadialFinishConfig::default());
    let (_, before) = dials(&s);

    for bad in [0.0_f64, -0.5, -0.000_001] {
        let err = s
            .set_toolpath_param(0, "point_spacing", json!(bad))
            .expect_err(
                "the generator divides the spoke radius by this dial, so a zero or \
                 a negative value must be refused at the setter",
            );
        let msg = format!("{err}");
        assert!(
            msg.contains("point_spacing") && msg.contains("outside the accepted range"),
            "the refusal must name the param and the domain; got: {msg}"
        );
        assert!(
            (dials(&s).1 - before).abs() < 1e-12,
            "a refused set must leave the dial untouched; it became {}",
            dials(&s).1
        );
    }

    let _ = s
        .set_toolpath_param(0, "point_spacing", json!(0.25))
        .expect("a real point spacing must be accepted");
    assert!((dials(&s).1 - 0.25).abs() < 1e-12);
}

/// Assertion 2. The domain is published, so an agent reads it before it
/// guesses. `get_operation_schema` serves this record.
#[test]
fn the_radial_finish_schema_publishes_both_domains() {
    let schema = ProjectSession::operation_schema("radial_finish")
        .expect("radial_finish must be a known operation type");
    let op = OperationType::RadialFinish;

    for name in ["angular_step", "point_spacing"] {
        let entry = schema
            .params
            .iter()
            .find(|p| p.name == name)
            .unwrap_or_else(|| panic!("the radial_finish schema must list {name}"));
        let range = entry
            .range
            .as_ref()
            .unwrap_or_else(|| panic!("{name} must publish its range"));
        assert_eq!(range["min"], json!(0.0));
        assert_eq!(range["min_exclusive"], json!(true));
        assert_eq!(range["finite"], json!(true));

        // The same domain, read off the registry the generator reads.
        let declared = OperationConfig::param_range_for_type(op, name)
            .unwrap_or_else(|| panic!("the registry must declare a range for {name}"));
        assert!(!declared.accepts(0.0), "{name} must refuse 0.0");
        assert!(!declared.accepts(-1.0), "{name} must refuse a negative");
        assert!(!declared.accepts(f64::NAN), "{name} must refuse NaN");
        assert!(declared.accepts(0.5), "{name} must accept a real length");
    }
}

/// Assertion 3. The registry gate guards `set_toolpath_param` only. A
/// project file and the GUI inspector write the config directly, so the
/// generator refuses the same numbers on its own.
///
/// The control generation comes FIRST and must succeed. Without it a
/// missing-geometry error would satisfy the refusal assertions below and
/// the test would measure nothing.
#[test]
fn the_generator_refuses_a_non_positive_dial_a_project_file_can_carry() {
    let cancel = AtomicBool::new(false);

    let mut control = radial_session(RadialFinishConfig::default());
    if let Err(e) = control.generate_toolpath(0, &cancel) {
        panic!("the control fixture must generate, else the refusals prove nothing: {e}");
    }

    let cases = [
        (
            "angular_step",
            RadialFinishConfig {
                angular_step: -5.0,
                ..RadialFinishConfig::default()
            },
        ),
        (
            "point_spacing",
            RadialFinishConfig {
                point_spacing: -0.5,
                ..RadialFinishConfig::default()
            },
        ),
    ];

    for (name, cfg) in cases {
        let mut s = radial_session(cfg);
        let Err(err) = s.generate_toolpath(0, &cancel) else {
            panic!("the generator must refuse a non-positive '{name}'");
        };
        let msg = format!("{err}");
        assert!(
            msg.contains(name),
            "the refusal must name the dial the generator divides by; got: {msg}"
        );
    }
}
