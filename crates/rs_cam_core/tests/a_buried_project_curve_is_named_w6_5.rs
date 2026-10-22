//! W6.5: a buried projected curve is named.
//!
//! # The claim
//!
//! Project Curve projects onto the mesh. On a remaining stock, stock can
//! stand above that mesh, so the tool cuts deeper than the commanded depth.
//! The Curve runner measures the planned moves against the stock the op
//! reads and records `BuriedCurveFinding`; the diagnostics carry it as
//! `geom.buried_curve` with severity Info (lead ruling W6.5).
//!
//! # The cases (from the G-OPGOLDEN fixtures, not from intent)
//!
//! - fresh stock: no measurement, no finding (X-19);
//! - the G-OPGOLDEN rest arm: the prior is a default 3D Rough with a flat
//!   end mill, which leaves `stock_to_leave_axial` (0.5 mm) and terraces on
//!   the cap. The tool cuts at least `0.5 + depth`, so the finding fires.
//!
//! The case "the stock is the surface, so the finding stays silent" is a
//! unit test of the threshold (`compute::execute::curve`): a dexel
//! simulation of any real finish leaves cusps above the mesh.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_buried_project_curve_is_named_w6_5
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::op_fixtures::{CaseSpec, Fixture, Frame, Prior, case_session, run_prior};
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::operation_configs::Adaptive3dConfig;
use rs_cam_core::diagnostics::Severity;
use rs_cam_core::diagnostics::ids;
use rs_cam_core::session::ProjectSession;

fn generate(rest: bool) -> (ProjectSession, usize) {
    let (mut session, index) = case_session(CaseSpec {
        name: "w6_5_buried",
        op: OperationConfig::new_default(OperationType::ProjectCurve),
        prior: if rest { Prior::RestPrior } else { Prior::None },
        stock_source: if rest {
            StockSource::FromRemainingStock
        } else {
            StockSource::Fresh
        },
        frame: Frame::TopUp,
        fixture: Fixture::Standard,
        tool: None,
    });
    if rest {
        run_prior(&mut session);
    }
    let cancel = AtomicBool::new(false);
    let _ = session
        .generate_toolpath(index, &cancel)
        .expect("Project Curve generates");
    (session, index)
}

fn commanded_depth() -> f64 {
    let OperationConfig::ProjectCurve(cfg) =
        OperationConfig::new_default(OperationType::ProjectCurve)
    else {
        unreachable!("the default of ProjectCurve is a ProjectCurve config")
    };
    cfg.depth
}

#[test]
fn fresh_stock_names_nothing() {
    let (session, index) = generate(false);
    let stats = &session.result_at(index).expect("the result").stats;
    assert_eq!(
        stats.buried_curve, None,
        "fresh stock: nothing was measured, so nothing is claimed"
    );
}

#[test]
fn a_curve_under_the_rough_allowance_is_named() {
    let (session, index) = generate(true);
    let stats = &session.result_at(index).expect("the result").stats;
    let f = stats
        .buried_curve
        .expect("the rest arm cuts below the rough allowance, so the finding fires");
    let depth = commanded_depth();
    let leave = Adaptive3dConfig::default().stock_to_leave_axial;
    assert!((f.commanded_depth_mm - depth).abs() < 1e-12);
    assert!(f.sampled_positions > 0);
    assert!(
        f.deepest_engagement_mm >= leave + depth - f.floor_mm,
        "the tool cuts at least the allowance plus the depth ({leave} + {depth} mm); \
         measured {} mm",
        f.deepest_engagement_mm
    );
    let diags = session
        .diagnose_toolpath_with_trace(index, None)
        .expect("diagnose");
    let d = diags
        .iter()
        .find(|d| d.id.as_str() == ids::GEOM_BURIED_CURVE)
        .expect("the diagnostics carry the finding");
    assert_eq!(d.severity, Severity::Info, "lead ruling W6.5: Info");
    assert!(d.message.contains("not the commanded"), "{}", d.message);
}
