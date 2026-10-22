//! W6.5 step 0: Project Curve `FromBelow` on a flipped setup.
//!
//! # The claim
//!
//! Through the session door (`generate_toolpath`), Project Curve with
//! `FromBelow` on a `FaceUp::Bottom` setup (`Frame::Flipped`) cuts the
//! top-up `FromAbove` toolpath, moved by `-origin`. Every feed move stays
//! inside the stock.
//!
//! # Why it holds
//!
//! - On a Z-flipped setup the kernel takes the frame-flipped arm of
//!   `project_curve_toolpath_with_cancel`: the setup transform already
//!   inverted the mesh, so the kernel runs the `FromAbove` body.
//! - `Frame::Flipped` mirrors every world input first, so the Bottom setup
//!   transform takes each input back onto the top-up input, moved by
//!   `-origin` (`common::op_fixtures`, module doc).
//!
//! # Why it discriminates
//!
//! A frame flag stuck at `false` takes the standalone arm. That arm flips
//! the mesh again, drops the cutter on the other face and negates Z, so
//! the toolpath is not the top-up cut. The third test pins that the
//! standalone arm gives another toolpath on this fixture.
//!
//! No other test reaches the flipped arm through the session door. The
//! W4 flipped-frame check runs the default `FromAbove`, where the frame is
//! inert.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test project_curve_from_below_on_a_flipped_setup_w6_5
//! ```

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::op_fixtures::{
    CaseSpec, Fixture, Frame, Prior, case_session, finest_decimals, mesh_stock, quantize, tool_for,
};
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::operation_configs::ProjectCurveDirection;
use rs_cam_core::geo::P3;
use rs_cam_core::toolpath::{MoveType, Toolpath};

/// Generate Project Curve with `direction` on `frame`, fresh stock.
fn generate(direction: ProjectCurveDirection, frame: Frame) -> Toolpath {
    let mut op = OperationConfig::new_default(OperationType::ProjectCurve);
    let OperationConfig::ProjectCurve(cfg) = &mut op else {
        unreachable!("the default of ProjectCurve is a ProjectCurve config")
    };
    cfg.direction = direction;
    let (mut session, index) = case_session(CaseSpec {
        name: "w6_5_frame",
        op,
        prior: Prior::None,
        stock_source: StockSource::Fresh,
        frame,
        fixture: Fixture::Standard,
        tool: None,
    });
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(index, &cancel)
        .expect("Project Curve generates")
        .toolpath()
        .clone()
}

/// The feed-move targets, quantized at the finest post precision and
/// sorted. `shift` is added to each target first.
fn feed_targets(tp: &Toolpath, shift: P3) -> Vec<[i64; 3]> {
    let d = finest_decimals().xyz;
    let mut out: Vec<[i64; 3]> = tp
        .moves
        .iter()
        .filter(|m| m.move_type != MoveType::Rapid)
        .map(|m| {
            let p = m.target + shift.coords;
            [quantize(p.x, d), quantize(p.y, d), quantize(p.z, d)]
        })
        .collect();
    out.sort_unstable();
    out
}

fn minus_origin() -> P3 {
    let stock = mesh_stock();
    P3::new(-stock.origin_x, -stock.origin_y, -stock.origin_z)
}

#[test]
fn from_below_on_a_flipped_setup_is_the_top_up_cut_moved_by_minus_origin() {
    let top = generate(ProjectCurveDirection::FromAbove, Frame::TopUp);
    let flip = generate(ProjectCurveDirection::FromBelow, Frame::Flipped);
    let expected = feed_targets(&top, minus_origin());
    let got = feed_targets(&flip, P3::origin());
    assert!(!expected.is_empty(), "the top-up cut has feed moves");
    assert_eq!(
        got.len(),
        expected.len(),
        "the flipped cut has another feed-move count"
    );
    // One quantum of slack per axis: the mirror is two float reflections.
    let off: Vec<_> = expected
        .iter()
        .zip(&got)
        .filter(|(e, g)| (0..3).any(|i| (e[i] - g[i]).abs() > 1))
        .take(5)
        .collect();
    assert!(
        off.is_empty(),
        "FromBelow on the flipped setup is not the top-up FromAbove cut moved by -origin \
         (first pairs expected, got): {off:?}"
    );
}

#[test]
fn from_below_on_a_flipped_setup_cuts_inside_the_stock() {
    let flip = generate(ProjectCurveDirection::FromBelow, Frame::Flipped);
    let stock = mesh_stock();
    let r = tool_for(OperationType::ProjectCurve).diameter / 2.0;
    let tol = 10f64.powi(-i32::try_from(finest_decimals().xyz).unwrap());
    let outside: Vec<P3> = flip
        .moves
        .iter()
        .filter(|m| m.move_type != MoveType::Rapid)
        .map(|m| m.target)
        .filter(|p| {
            !(p.x >= -r - tol
                && p.x <= stock.x + r + tol
                && p.y >= -r - tol
                && p.y <= stock.y + r + tol
                && p.z >= -r - tol)
        })
        .take(5)
        .collect();
    assert!(
        outside.is_empty(),
        "feed moves outside the stock [0, {}] x [0, {}] grown by the tool radius {r}: \
         {outside:?}",
        stock.x,
        stock.y
    );
}

/// Non-vacuity: the standalone arm (the frame flag false) gives another
/// toolpath on this fixture, so a stuck flag fails the first test.
#[test]
fn the_standalone_from_below_arm_is_not_the_top_up_cut() {
    let top = generate(ProjectCurveDirection::FromAbove, Frame::TopUp);
    let standalone = generate(ProjectCurveDirection::FromBelow, Frame::TopUp);
    assert_ne!(
        feed_targets(&top, P3::origin()),
        feed_targets(&standalone, P3::origin()),
        "FromBelow on a top-up setup cuts the FromAbove toolpath, so the sentry cannot see \
         a stuck frame flag"
    );
}
