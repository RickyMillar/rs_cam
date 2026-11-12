//! Feeds contract P11a, decision L4, defect D-B
//! (`planning/feeds_contract_2026-11-03/research_ae/FINDINGS.md`): the
//! deflection edge of the safe envelope is the solved depth where the tip
//! deflection meets its target. The search bracket is never drawn as a
//! limit.
//!
//! Before P11a, `cutter_constraints::invert_deflection` returned its upper
//! bracket (`0.95 x stickout`) when no depth in the bracket reached the
//! target, and the envelope drew that bracket as an `ap(ae)` curve: on the
//! rivmap100 end mill, a flat edge at 42.75 mm, deeper than the 25 mm flute.
//!
//! The arms:
//!
//! 1. The rivmap100 end mill (D 6, Z 2, flute 25, stickout 45) on a
//!    roughing pocket in white oak: no depth below the flute reaches the
//!    target. The `Deflection` entry states "no deflection limit below the
//!    flute", and no curve point sits at the bracket (red before: a curve
//!    at 42.75 mm).
//! 2. A slender end mill (D 3, flute 12, stickout 30) in white oak:
//!    deflection binds. Feeds contract P29-7 7c: the edge is the root of
//!    the declared cut that binds each column (`feeds::planned`: the lowest
//!    root over the cuts that the plan always cuts; on this contour Pocket,
//!    the slot ring at its unlifted chip). At each curve point the binding
//!    cut, at its own width and chip, deflects at most the target, and one
//!    convergence step deeper it deflects more (the forward model
//!    `tip_deflection_from_engagement`, not the solver). The body alone
//!    (`predict_peak_deflection_um`, the door that reads the op) is not
//!    deeper than its own root: at the edge it deflects at most the
//!    target. At the proposal width the binding cut is the cut that
//!    `Limit::binding_cut` names.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::catalog::Presence;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::cutter_constraints::BINSEARCH_CONVERGENCE_MM;
use rs_cam_core::feeds::engagement::Engagement;
use rs_cam_core::feeds::envelope::CutPoint;
use rs_cam_core::feeds::envelope::{Edge, EnvelopeInput, Limit, LimitKind, SafeEnvelope};
use rs_cam_core::feeds::planned::{
    BodyColumn, DeclaredCuts, DeflectionPlanned, ForceEnv, PlannedCell, judge_declared_cuts,
};
use rs_cam_core::feeds::predict::{predict_peak_deflection_um, tip_deflection_from_engagement};
use rs_cam_core::feeds::suggest::SuggestContext;
use rs_cam_core::feeds::{Basis, SpindleStrategy, embedded_vendor_lut};
use rs_cam_core::limits::CutKind;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

fn end_mill(d: f64, flute: f64, stickout: f64) -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(3), ToolType::EndMill);
    tool.diameter = d;
    tool.helix_deg = 30.0;
    tool.set_cutting_length(ToolFact::Stated(flute));
    tool.shaft_diameter = d;
    tool.set_shank_diameter(ToolFact::Stated(d));
    tool.set_stickout(ToolFact::Stated(stickout));
    tool.set_flute_count(ToolFact::Stated(2));
    tool
}

fn roughing_pocket() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        floor: rs_cam_core::compute::floor::Floor::top(12.0),
        depth_per_pass: Some(1.0),
        stepover: Some(1.0),
        // P11b (L7): the envelope is sized at the operator's chip, 4800 /
        // (18 000 x 2) = 0.133 mm, the chip of the suggested 6 mm row that
        // the grid read before P11b.
        feed_rate: Some(4800.0),
        plunge_rate: Some(500.0),
        spindle_rpm: Some(18_000),
        ..PocketConfig::default()
    })
}

fn oak() -> Material {
    Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    }
}

fn envelope(tool: &ToolConfig, machine: &MachineProfile) -> SafeEnvelope {
    let op = roughing_pocket();
    SafeEnvelope::for_cut(EnvelopeInput {
        operation: &op,
        tool,
        machine,
        material: &oak(),
        lut: embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::MatchChart,
        context: SuggestContext::default(),
    })
    .expect("a flat end mill on a roughing pocket has an envelope")
}

/// The declared cut that binds the deflection at the body width `ae` (the
/// lowest root over the cuts that the plan always cuts, at the operator's
/// chip), with its width and chip. A tie keeps the first cut.
fn binding_cut_at(tool: &ToolConfig, ae: f64, target_um: f64) -> (CutKind, f64, f64) {
    let op = roughing_pocket();
    let tool_def = rs_cam_core::compute::cutter::build_cutter(tool).expect("the tool builds");
    let cuts = DeclaredCuts::of_operation(
        &op,
        tool,
        &rs_cam_core::feeds::entry_rule::ConfiguredEntry::registry_default(&op),
    );
    let fz = 4800.0 / (18_000.0 * 2.0);
    let column = BodyColumn {
        ae_mm: ae,
        chip_mm: Some(fz),
    };
    let material = oak();
    let env = ForceEnv {
        tool: &tool_def,
        material: &material,
        bound: target_um,
    };
    let cells = judge_declared_cuts::<DeflectionPlanned>(&cuts, column, &env);
    let mut best: Option<(f64, CutKind)> = None;
    for e in cells.iter() {
        if e.presence != Presence::Always {
            continue;
        }
        if let PlannedCell::Judged(Some(root)) = e.value
            && best.is_none_or(|(b, _)| root < b)
        {
            best = Some((root, e.kind));
        }
    }
    let kind = best.expect("a cut has a root").1;
    let cut = cuts
        .list()
        .iter()
        .find(|c| c.kind == kind)
        .expect("a declared cut");
    let planned = cuts.planned(cut, column);
    (
        kind,
        planned.width_mm.expect("a width"),
        planned.chip_mm.expect("a chip"),
    )
}

/// The deflection target (µm) that the limit names in its basis.
fn target_um(limit: &Limit) -> f64 {
    fn find(b: &Basis) -> Option<f64> {
        match b {
            Basis::Rule(c) if c.name == "feeds.suggest.DEFLECTION_BACKOFF_TARGET_UM" => {
                Some(c.value)
            }
            Basis::Derived { from, .. } => from.iter().find_map(find),
            Basis::Rule(_)
            | Basis::Operator { .. }
            | Basis::Printed(_)
            | Basis::Claim(_)
            | Basis::Machine(_)
            | Basis::ToolFact(_) => None,
        }
    }
    find(&limit.basis).expect("the deflection limit names its target row")
}

#[test]
fn no_root_below_the_flute_is_stated_not_drawn_p11a() {
    let stickout = 45.0;
    let tool = end_mill(6.0, 25.0, stickout);
    let env = envelope(&tool, &MachineProfile::default());
    let limit = env
        .limit(LimitKind::Deflection)
        .expect("a roughing envelope in white oak carries a Deflection entry");
    let bracket = 0.95 * stickout;
    match &limit.edge {
        Edge::Unmodelled { reason } => assert_eq!(
            *reason, "no deflection limit below the flute",
            "the absence states its reason: {limit:?}"
        ),
        Edge::ApCurve { points } => panic!(
            "D-B: no depth below the flute reaches the target, but the envelope draws a \
             curve (the bracket is {bracket} mm): {points:?}"
        ),
        Edge::ApCeiling { .. }
        | Edge::ApFloor { .. }
        | Edge::AeCeiling { .. }
        | Edge::AeFloor { .. }
        | Edge::AeCurve { .. }
        | Edge::PerPoint => panic!("the deflection limit has an unexpected edge: {limit:?}"),
    }
}

#[test]
fn a_solved_edge_sits_where_deflection_meets_its_target_p11a() {
    let tool = end_mill(3.0, 12.0, 30.0);
    let machine = MachineProfile::default();
    let env = envelope(&tool, &machine);
    let limit = env
        .limit(LimitKind::Deflection)
        .expect("the slender tool carries a Deflection entry");
    let target = target_um(limit);
    let Edge::ApCurve { points } = &limit.edge else {
        panic!("the slender tool binds on deflection: {limit:?}");
    };
    assert!(!points.is_empty(), "the curve has points");
    let tool_def = rs_cam_core::compute::cutter::build_cutter(&tool).expect("the tool builds");
    let mut kinds = std::collections::BTreeSet::new();
    let mut checked = 0;
    for &[ae, ap] in points {
        // P11b (L7): the column's chip is the operator's (the op's own feed
        // and RPM), so the predictor reads the op as it stands.
        assert!(
            env.speeds.aes.iter().any(|&w| (w - ae).abs() < 1e-12),
            "a curve point sits on a width sample"
        );
        let delta_at = |depth: f64| {
            let mut op = roughing_pocket();
            assert!(op.set_depth_per_pass(depth));
            assert!(op.set_stepover(ae.max(1.0e-3)));
            predict_peak_deflection_um(&op, &tool, &oak(), &machine)
                .expect("white oak and a stated stickout give a figure")
                .predicted_um
        };
        let at_edge = delta_at(ap);
        let top = env.speeds.aps.last().copied().expect("the grid has depths");
        if (ap - top).abs() < 1e-12 {
            // A column with no root takes the top of the span (the power
            // edge's rule): the predictor stays under the target there.
            assert!(
                at_edge <= target,
                "D-B: the column ae {ae} draws no root, but the top of the span gives \
                 {at_edge} um, above the {target} um target"
            );
            continue;
        }
        // The body alone: the edge is not deeper than its root.
        assert!(
            at_edge <= target * (1.0 + 1e-9),
            "D-B: at the edge ap {ap} (ae {ae}) the body deflects {at_edge} um, above the \
             {target} um target"
        );
        // P29-7 7c: the cut that binds this column, at its own width and
        // chip; the edge is its root.
        let (kind, width, chip) = binding_cut_at(&tool, ae, target);
        let cut_at = |depth: f64| {
            let e = Engagement::from_tool_cut(
                CutPoint {
                    ap_mm: depth,
                    ae_mm: width.max(1.0e-3),
                },
                &tool_def,
            );
            tip_deflection_from_engagement(&tool_def, &oak(), &e, chip).expect("a modelled cut")
                * 1000.0
        };
        assert!(
            cut_at(ap) <= target * (1.0 + 1e-9),
            "at the edge ap {ap} (ae {ae}) the {} deflects {} um, above the {target} um target",
            kind.label(),
            cut_at(ap)
        );
        assert!(
            cut_at(ap + BINSEARCH_CONVERGENCE_MM) > target,
            "D-B: one convergence step past the edge ap {ap} (ae {ae}) the {} deflects {} um, \
             not above the {target} um target: the edge is not its root",
            kind.label(),
            cut_at(ap + BINSEARCH_CONVERGENCE_MM)
        );
        kinds.insert(kind.label());
        checked += 1;
    }
    assert!(checked > 0);
    assert!(
        kinds.contains("slot ring"),
        "non-vacuity: the slot ring binds a column: {kinds:?}"
    );
    // At the proposal width the binding cut is `Limit::binding_cut`.
    let p = env.proposal.get();
    let (kind, _, _) = binding_cut_at(&tool, p.ae_mm, target);
    assert_eq!(
        limit.binding_cut,
        Some(kind),
        "the binding cut at the proposal"
    );
}
