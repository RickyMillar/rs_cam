//! P33: the depth ladder of a Project Curve through the session door.
//!
//! The kernel and adapter tests (`ops::project_curve`, `compute::execute::
//! project_curve_chaining`) hold the ladder itself. This file holds what
//! the session adds after the runner: the registry dressups (the
//! rapid-order dressup with them), the boundary clip, the entry step, and
//! the limit rows of the stated cut.
//!
//! # The claims
//!
//! 1. With `depth_per_pass` set through `SetToolpathParam`, and a boundary
//!    that cuts the curve, the stored result cuts each XY of the curve
//!    from the top down: no deeper cut comes before a shallower cut at the
//!    same XY, and the last cut is the cut of the op with no key. No rapid
//!    ends below the stock top.
//! 2. The stated rows read the depth of one pass: a tip above the model
//!    surface with no key gives no force budget number of the body cut and
//!    no flute guard number; with the key both rows hold a number.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_project_curve_ladder_holds_through_the_session_p33
//! ```

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;

mod common;
use common::op_fixtures::{CaseSpec, Fixture, Frame, Prior, case_session};
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{
    BoundaryConfig, BoundaryContainment, BoundarySource, StockSource,
};
use rs_cam_core::limits::{CutKind, LimitId};
use rs_cam_core::session::{Command, ProjectSession, SetBoundaryConfigArgs, SetToolpathParamArgs};
use rs_cam_core::toolpath::MoveType;
use serde_json::json;

/// The half width (mm) of the boundary square about the stock centre. The
/// fixture curve reaches x = -8 and x = 8, so the boundary cuts it.
const BOUNDARY_HALF_MM: f64 = 5.0;

fn session_with(params: &[(&str, serde_json::Value)], clip: bool) -> (ProjectSession, usize) {
    let (mut session, index) = case_session(CaseSpec {
        name: "p33_ladder",
        op: OperationConfig::new_default(OperationType::ProjectCurve),
        prior: Prior::None,
        stock_source: StockSource::Fresh,
        frame: Frame::TopUp,
        fixture: Fixture::Standard,
        tool: None,
    });
    for (param, value) in params {
        let _ = session
            .apply(Command::SetToolpathParam(SetToolpathParamArgs {
                index,
                param: (*param).to_owned(),
                value: value.clone(),
            }))
            .unwrap_or_else(|e| panic!("the door takes `{param}`: {e}"));
    }
    if clip {
        let half_stock = session.stock_config().x / 2.0;
        let _ = session
            .apply(Command::SetBoundaryConfig(SetBoundaryConfigArgs {
                index,
                boundary: BoundaryConfig {
                    enabled: true,
                    source: BoundarySource::Stock,
                    containment: BoundaryContainment::Center,
                    offset: BOUNDARY_HALF_MM - half_stock,
                },
            }))
            .expect("the boundary applies");
    }
    (session, index)
}

/// The Z of each fed move at each XY (to 1e-6 mm), in the emitted order.
fn cuts_by_xy(session: &ProjectSession, index: usize) -> BTreeMap<(i64, i64), Vec<f64>> {
    let result = session.result_at(index).expect("the op holds a result");
    let mut by_xy: BTreeMap<(i64, i64), Vec<f64>> = BTreeMap::new();
    for m in &result.toolpath().moves {
        if matches!(m.move_type, MoveType::Rapid) {
            continue;
        }
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a fixture coordinate in micrometres is far inside i64"
        )]
        let key = (
            (m.target.x * 1e6).round() as i64,
            (m.target.y * 1e6).round() as i64,
        );
        by_xy.entry(key).or_default().push(m.target.z);
    }
    by_xy
}

/// Claim 1.
#[test]
fn the_ladder_holds_after_the_dressups_and_the_boundary_clip_p33() {
    let cancel = AtomicBool::new(false);
    let (mut laddered, index) = session_with(&[("depth_per_pass", json!(0.4))], true);
    let (mut one_pass, _) = session_with(&[], true);
    for session in [&mut laddered, &mut one_pass] {
        let _ = session
            .generate_toolpath(index, &cancel)
            .expect("the Project Curve generates");
    }
    assert_eq!(
        laddered.toolpath_configs()[index]
            .operation
            .depth_per_pass(),
        Some(0.4)
    );
    assert!(
        laddered.toolpath_configs()[index].boundary.enabled,
        "non-vacuity: the op has a boundary"
    );

    let ladder_cuts = cuts_by_xy(&laddered, index);
    let final_cuts = cuts_by_xy(&one_pass, index);
    let lowest = |zs: &[f64]| zs.iter().copied().fold(f64::INFINITY, f64::min);
    let mut laddered_points = 0;
    for (xy, zs) in &ladder_cuts {
        assert!(
            zs.windows(2).all(|w| w[1] <= w[0] + 1e-9),
            "at {xy:?} a deeper cut comes before a shallower cut: {zs:?}"
        );
        if let Some(final_zs) = final_cuts.get(xy) {
            assert!(
                (lowest(zs) - lowest(final_zs)).abs() < 1e-6,
                "at {xy:?} the last pass is not the full depth: {zs:?} against {final_zs:?}"
            );
            let mut levels = zs.clone();
            levels.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
            if levels.len() >= 3 {
                laddered_points += 1;
            }
        }
    }
    assert!(
        laddered_points > 10,
        "non-vacuity: {laddered_points} curve points are cut in 3 or more passes"
    );
    let x_limit = BOUNDARY_HALF_MM + 1e-6;
    assert!(
        final_cuts
            .keys()
            .all(|(x, _)| (*x as f64 / 1e6).abs() <= x_limit),
        "non-vacuity: the boundary clips the curve at x = +/- {BOUNDARY_HALF_MM}"
    );

    let stock_top = laddered.stock_bbox().max.z;
    let result = laddered.result_at(index).expect("the op holds a result");
    for m in &result.toolpath().moves {
        if matches!(m.move_type, MoveType::Rapid) {
            assert!(
                m.target.z >= stock_top - 1e-6,
                "a rapid ends at Z {} below the stock top {stock_top}",
                m.target.z
            );
        }
    }
}

/// The stated depth rows before a simulation: how many force budget rows
/// of the body cut hold a number, and whether the flute guard row of the
/// stated point holds a number.
fn stated_depth_rows(session: &ProjectSession, index: usize) -> (usize, bool) {
    let rows = session
        .limit_rows(index, None)
        .expect("the toolpath is in the session")
        .envelope()
        .expect("the toolpath has a tool")
        .expect("the envelope builds");
    let force = rows
        .iter()
        .filter(|row| {
            row.is_stated()
                && row.check().limit == LimitId::ForceBudget
                && row.cut() == Some(CutKind::Body)
                && row.holds_a_number()
        })
        .count();
    let flute: Vec<bool> = rows
        .iter()
        .filter(|row| row.is_stated() && row.check().limit == LimitId::FluteGuard)
        .map(|row| row.holds_a_number())
        .collect();
    assert_eq!(flute.len(), 1, "one stated flute guard row");
    (force, flute[0])
}

/// Claim 2.
#[test]
fn the_stated_rows_read_the_depth_of_one_pass_p33() {
    // A tip 1 mm above the model surface: no depth before a simulation.
    let (bare, index) = session_with(&[("depth", json!(-1.0))], false);
    assert_eq!(
        stated_depth_rows(&bare, index),
        (0, false),
        "with no key the body cut states no depth: no force budget number, no flute guard number"
    );
    let (keyed, index) = session_with(
        &[("depth", json!(-1.0)), ("depth_per_pass", json!(2.0))],
        false,
    );
    assert_eq!(
        stated_depth_rows(&keyed, index),
        (1, true),
        "with the key the force budget and the flute guard are judged at the depth of one pass"
    );
}
