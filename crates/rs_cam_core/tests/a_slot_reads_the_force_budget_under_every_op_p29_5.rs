//! Feeds contract P29-5, commit A
//! (`planning/feeds_contract_2026-11-03/phases/P29-5_DESIGN.md`): a slot or
//! a groove reads the machine force budget under every op.
//!
//! Ruling R3 (operator, 2026-11-11): "slot is a slot! it can vary in z
//! depth due to rest and similar though". A slot or a groove has material
//! on both sides of the tool (`limits::BodySides::Both`), so its depth
//! limit is the force budget whatever its pass role. Ruling R2 stays for a
//! one-sided finish pass: no axial depth ceiling.
//!
//! - Claim 1: the registry table. Every row whose radial rule is `Slot` or
//!   `Groove` and whose family is not the drill has
//!   `OperationType::body_depth_limit() == ForceBudget`. Every row with a
//!   one-sided radial rule and a finish or semi-finish role has
//!   `NoLimit(FinishPass)`. The rows that move from R2 to R3 are exactly
//!   the six ops of audit row `P28-03` (closed in P29-5c). The scan reaches Trace and Pencil
//!   (one family, two answers).
//! - Claim 2: Trace (a finish slot), Profile (a rough slot) and Project
//!   Curve (a finish slot) with one tool, one material and one stated chip
//!   give the same `rigidity_depth_cap` at `ae = D` (bit-equal), and the
//!   same envelope `Rigidity` ceiling at `D` (bit-equal).
//! - Claim 3: the config and the kind cannot disagree:
//!   `OperationConfig::cuts().body_facts().depth_rule ==
//!   op_type().body_depth_limit()` for every op at its default.
//! - Claim 4 (P29-5c): a groove reads the slot width before the
//!   simulation. VCarve, Inlay and InlayPlug with one V-bit, one material
//!   and one stated chip give the same envelope `Rigidity` ceiling as a
//!   Trace with that V-bit (bit-equal): the groove at its own fixed axis
//!   width, the Trace at D. A Chamfer states its width, not a depth: its
//!   `Rigidity` entry is `Unmodelled` with the chamfer clause.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::collections::BTreeSet;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType, RadialRule};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::envelope::{Edge, EnvelopeAxes, EnvelopeInput, LimitKind, SafeEnvelope};
use rs_cam_core::feeds::force_budget::{
    CHAMFER_STATES_NO_DEPTH, RigidityCapOutcome, rigidity_depth_cap,
};
use rs_cam_core::feeds::suggest::SuggestContext;
use rs_cam_core::feeds::{OperationFamily, PassRole, SpindleStrategy, embedded_vendor_lut};
use rs_cam_core::limits::{DepthLimit, NoDepthLimit};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

/// The rows that R3 moves: the slot and groove ops filed as a finish pass
/// (audit row `P28-03`).
const MOVED_BY_R3: [OperationType; 6] = [
    OperationType::Trace,
    OperationType::ProjectCurve,
    OperationType::VCarve,
    OperationType::Inlay,
    OperationType::InlayPlug,
    OperationType::Chamfer,
];

const fn both_sides(radial: RadialRule) -> bool {
    match radial {
        RadialRule::Slot | RadialRule::Groove => true,
        RadialRule::Stepover
        | RadialRule::Scallop
        | RadialRule::ScallopElseStepover
        | RadialRule::OffsetStepoverElseSurface
        | RadialRule::AngularStep
        | RadialRule::SurfaceFollow
        | RadialRule::NoRadial => false,
    }
}

#[test]
fn claim_1_a_slot_or_a_groove_reads_the_budget_under_every_role() {
    let mut moved = BTreeSet::new();
    let mut two_sided_seen = BTreeSet::new();
    let mut one_sided_finish_seen = BTreeSet::new();
    for &op in OperationType::ALL {
        let spec = op.spec();
        let radial = op.capabilities().cut_rule.radial;
        let limit = op.body_depth_limit();
        let finish = matches!(
            spec.feeds_pass_role,
            PassRole::Finish | PassRole::SemiFinish
        );
        if spec.feeds_family == OperationFamily::Drill {
            assert_eq!(
                limit,
                DepthLimit::NoLimit(NoDepthLimit::DrillCycle),
                "{op:?}: a drill cycle has no depth limit"
            );
            continue;
        }
        if both_sides(radial) {
            assert_eq!(
                limit,
                DepthLimit::ForceBudget,
                "{op:?}: a slot or a groove reads the force budget (R3)"
            );
            two_sided_seen.insert(op.kind_str());
            if finish {
                moved.insert(op.kind_str());
            }
        } else if finish {
            assert_eq!(
                limit,
                DepthLimit::NoLimit(NoDepthLimit::FinishPass),
                "{op:?}: a one-sided finish pass has no depth limit (R2)"
            );
            one_sided_finish_seen.insert(op.kind_str());
        } else {
            assert_eq!(
                limit,
                DepthLimit::ForceBudget,
                "{op:?}: a roughing cut reads the force budget"
            );
        }
    }
    let want: BTreeSet<&str> = MOVED_BY_R3.iter().map(|op| op.kind_str()).collect();
    assert_eq!(moved, want, "R3 moves exactly the six ops of row P28-03");
    // Non-vacuity: one family (Trace) gives both answers.
    assert!(two_sided_seen.contains(OperationType::Trace.kind_str()));
    assert!(one_sided_finish_seen.contains(OperationType::Pencil.kind_str()));
    assert_eq!(
        OperationType::Trace.spec().feeds_family,
        OperationType::Pencil.spec().feeds_family,
        "Trace and Pencil share one family; the sides decide"
    );
}

fn flat_6mm() -> ToolConfig {
    let mut t = ToolConfig::test_default(ToolId::from_raw(1), ToolType::EndMill);
    t.diameter = 6.0;
    t.set_flute_count(ToolFact::Stated(2));
    t.set_cutting_length(ToolFact::Stated(18.0));
    t.set_shank_diameter(ToolFact::Stated(6.0));
    t.shaft_diameter = 6.0;
    t.set_stickout(ToolFact::Stated(26.0));
    t
}

/// The stated feed and speed: chip = 3600 / (18 000 x 2) = 0.1 mm.
const FEED_MM_MIN: f64 = 3600.0;
const RPM: u32 = 18_000;

fn stated_op(kind: OperationType) -> OperationConfig {
    let mut op = OperationConfig::new_default(kind);
    rs_cam_core::compute::fixture_values::fill_unset(&mut op);
    op.set_feed_rate(FEED_MM_MIN);
    op.set_spindle_rpm(Some(RPM));
    op
}

#[test]
fn claim_2_three_slot_ops_read_one_cap_at_full_width() {
    let machine = MachineProfile::generic_wood_router();
    let material = Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    };
    let tool = flat_6mm();
    let cutter = rs_cam_core::compute::cutter::build_cutter(&tool).expect("a cutter");
    let d = tool.diameter;
    let fz = FEED_MM_MIN / (f64::from(RPM) * 2.0);
    let mut caps = Vec::new();
    let mut ceilings = Vec::new();
    for kind in [
        OperationType::Trace,
        OperationType::Profile,
        OperationType::ProjectCurve,
    ] {
        let cap = match rigidity_depth_cap(
            &machine,
            kind.body_depth_limit(),
            &cutter,
            &material,
            d,
            fz,
        ) {
            Some(RigidityCapOutcome::Cap(c)) => c.cap_mm(),
            other => panic!("{kind:?}: a slot in hardwood has a force-budget cap: {other:?}"),
        };
        caps.push((kind, cap));
        let op = stated_op(kind);
        let env = SafeEnvelope::for_cut(EnvelopeInput {
            operation: &op,
            tool: &tool,
            machine: &machine,
            material: &material,
            lut: embedded_vendor_lut(),
            spindle_strategy: SpindleStrategy::default(),
            context: SuggestContext::default(),
        })
        .unwrap_or_else(|e| panic!("{kind:?}: the envelope builds: {e}"));
        let ceiling = env
            .judged_ap_ceiling(LimitKind::Rigidity, d)
            .unwrap_or_else(|| panic!("{kind:?}: the envelope draws a Rigidity ceiling at D"));
        ceilings.push((kind, ceiling));
    }
    let &(_, first_cap) = caps.first().expect("three caps");
    for (kind, cap) in &caps {
        assert_eq!(
            cap.to_bits(),
            first_cap.to_bits(),
            "{kind:?}: the cap {cap} differs from {first_cap}"
        );
    }
    let &(_, first_ceiling) = ceilings.first().expect("three ceilings");
    for (kind, ceiling) in &ceilings {
        assert_eq!(
            ceiling.to_bits(),
            first_ceiling.to_bits(),
            "{kind:?}: the envelope ceiling {ceiling} differs from {first_ceiling}"
        );
    }
    // The envelope solves at the stated chip, so its ceiling is the
    // producer's cap.
    assert!(
        (first_ceiling - first_cap).abs() < 1e-9,
        "the envelope ceiling {first_ceiling} is the producer's cap {first_cap}"
    );
}

#[test]
fn claim_3_the_config_and_the_kind_read_one_depth_limit() {
    for &kind in OperationType::ALL {
        let op = OperationConfig::new_default(kind);
        assert_eq!(
            op.cuts(&rs_cam_core::feeds::entry_rule::ConfiguredEntry::registry_default(&op))
                .body_facts()
                .depth_rule,
            kind.body_depth_limit(),
            "{kind:?}: the cut list and the registry row disagree"
        );
    }
}

/// A 60 degree V-bit of 6.35 mm (the FM1 tool builder).
fn vbit_6_35() -> ToolConfig {
    let d = 6.35;
    let mut t = ToolConfig::test_default(ToolId::from_raw(2), ToolType::VBit);
    t.diameter = d;
    t.included_angle = 60.0;
    t.set_flute_count(ToolFact::Stated(2));
    t.set_cutting_length(ToolFact::Stated((d * 3.0).max(12.0)));
    t.set_shank_diameter(ToolFact::Stated(d));
    t.shaft_diameter = d;
    t.set_stickout(ToolFact::Stated(t.cutting_length_planning_mm() + 8.0));
    t
}

fn envelope_of(op: &OperationConfig, tool: &ToolConfig, material: &Material) -> SafeEnvelope {
    let machine = MachineProfile::generic_wood_router();
    SafeEnvelope::for_cut(EnvelopeInput {
        operation: op,
        tool,
        machine: &machine,
        material,
        lut: embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::default(),
        context: SuggestContext::default(),
    })
    .unwrap_or_else(|e| panic!("{:?}: the envelope builds: {e}", op.op_type()))
}

#[test]
fn claim_4_a_groove_reads_the_slot_width_before_the_simulation_p29_5c() {
    let material = Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    };
    let tool = vbit_6_35();
    let d = tool.diameter;
    let trace = envelope_of(&stated_op(OperationType::Trace), &tool, &material)
        .judged_ap_ceiling(LimitKind::Rigidity, d)
        .expect("a Trace with a V-bit draws a Rigidity ceiling at D");
    assert!(
        trace.is_finite() && trace > 0.0,
        "the Trace ceiling {trace} is a depth"
    );
    for kind in [
        OperationType::VCarve,
        OperationType::Inlay,
        OperationType::InlayPlug,
    ] {
        let env = envelope_of(&stated_op(kind), &tool, &material);
        let ae = match env.axes {
            EnvelopeAxes::AxialBar { ae_mm, .. } | EnvelopeAxes::Fixed { ae_mm, .. } => ae_mm,
            EnvelopeAxes::Plane { .. } | EnvelopeAxes::RadialBar { .. } => {
                panic!("{kind:?}: a groove has a fixed width axis: {:?}", env.axes)
            }
        };
        let ceiling = env
            .judged_ap_ceiling(LimitKind::Rigidity, ae)
            .unwrap_or_else(|| panic!("{kind:?}: the envelope draws a Rigidity ceiling"));
        assert_eq!(
            ceiling.to_bits(),
            trace.to_bits(),
            "{kind:?}: the groove ceiling {ceiling} at the axis width {ae} differs from the \
             Trace ceiling {trace}"
        );
    }
    let chamfer = envelope_of(&stated_op(OperationType::Chamfer), &tool, &material);
    let rigidity = chamfer
        .limit(LimitKind::Rigidity)
        .expect("a Chamfer states a Rigidity entry");
    let Edge::Unmodelled { reason } = &rigidity.edge else {
        panic!(
            "a Chamfer states no depth, so no Rigidity edge: {:?}",
            rigidity.edge
        );
    };
    assert_eq!(*reason, CHAMFER_STATES_NO_DEPTH);
}
