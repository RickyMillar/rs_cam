//! CONTRACT K2b (feeds contract P5): the envelope keeps each limit that the
//! calculator and the invariant passes compute and then discard.
//!
//! For each `LimitKind` row of the P5 card table, one cell:
//!
//! - An edge on the stated axis (flute guard, vendor axial, rigidity,
//!   power, deflection, diameter): the operator states a point past it, the
//!   proposal sits on that edge (`min(stated, cap)`), the envelope names the
//!   limit as the binding one, and `Limit.basis` names its constant.
//! - A floor (the move budget, the engagement floor): a proposal never
//!   raises a stated value (K2), so the operator's point below the floor is
//!   `Placement::Outside` with that limit, and the basis names the
//!   constant.
//! - The cutting length: the flute guard is 0.8 x the same length, so the
//!   cutting-length edge always sits above the flute-guard edge. The sentry
//!   holds that order and the basis.
//! - A per-point limit (the chip band, the rubbing floor, the machine feed
//!   cap): the speeds map judges it at each point; the envelope lists it
//!   with its basis.
//!
//! The cells were found by a probe over ops, tools, machines and stated
//! points (P5 report); each is a plain configuration here. P29-7 7c: the
//! vendor axial cell is a geodesic 3D Rough (no full slot that the plan
//! always cuts); the same cell as a ContourParallel rough binds the force
//! budget on its first slot, and the test holds that too.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::ToolFact;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::FeedsInput;
use rs_cam_core::feeds::envelope::{Edge, EnvelopeInput, LimitKind, Placement, SafeEnvelope};
use rs_cam_core::feeds::sourced::MachineField;
use rs_cam_core::feeds::suggest::{NoProject, SuggestContext};
use rs_cam_core::feeds::{Basis, SpindleStrategy, calculate, embedded_vendor_lut};
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::limits::{CutKind, FirstPassKind};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

fn softwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    }
}

fn oak() -> Material {
    Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    }
}

fn end_mill(d: f64, cutting_length: f64) -> ToolConfig {
    let mut t = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    t.diameter = d;
    t.set_cutting_length(ToolFact::Stated(cutting_length));
    t
}

struct Cell<'a> {
    op: OperationType,
    tool: ToolConfig,
    machine: MachineProfile,
    material: Material,
    dpp: f64,
    stepover: f64,
    bbox: Option<&'a BoundingBox3>,
}

fn envelope(cell: &Cell<'_>) -> SafeEnvelope {
    envelope_at_chip_share(cell, 1.0)
}

/// [`envelope`] with the stated chip at `share` of the full Suggest chip.
fn envelope_at_chip_share(cell: &Cell<'_>, share: f64) -> SafeEnvelope {
    envelope_of(cell, share, |_| {})
}

/// [`envelope_at_chip_share`] of the fixture op after `edit`.
fn envelope_of(
    cell: &Cell<'_>,
    share: f64,
    edit: impl Fn(&mut rs_cam_core::compute::catalog::OperationConfig),
) -> SafeEnvelope {
    let mut op = rs_cam_core::compute::fixture_values::fixture_op(cell.op);
    edit(&mut op);
    assert!(
        op.set_depth_per_pass(cell.dpp),
        "{:?} holds a depth",
        cell.op
    );
    assert!(
        op.set_stepover(cell.stepover),
        "{:?} holds a stepover",
        cell.op
    );
    // Feeds contract P11b (L7): the envelope is sized at the operator's
    // chip. The cell states the full Suggest chip for its cut (the band chip
    // and the RPM before the Step 6 power ladder: an operator who keeps the
    // band chip), so the force limits read that chip, as the grid did before
    // P11b.
    let r = calculate(&FeedsInput::of_operation(
        &op,
        &cell.tool,
        &cell.material,
        &cell.machine,
        embedded_vendor_lut(),
        SpindleStrategy::default(),
    ));
    // The RPM before the power ladder walked it, and the band chip lifted by
    // the chip-thinning law at that RPM.
    let rpm = (r.rpm / r.derates.spindle_scale.max(1e-9)).round();
    let z = f64::from(cell.tool.flute_count_planning());
    op.set_feed_rate(
        share * r.derates.target_chip_load_mm * r.derates.radial_chip_thinning * rpm * z,
    );
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a positive RPM below 100 000"
    )]
    op.set_spindle_rpm(Some(rpm as u32));
    SafeEnvelope::for_cut(EnvelopeInput {
        operation: &op,
        tool: &cell.tool,
        machine: &cell.machine,
        material: &cell.material,
        lut: embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::default(),
        context: SuggestContext::no_project(NoProject {
            model_bbox: cell.bbox,
            ..NoProject::NONE
        }),
    })
    .expect("the cell is backed")
}

/// The F201 names of the rules that a basis reads, in tree order.
fn rule_names(b: &Basis) -> Vec<String> {
    match b {
        Basis::Rule(c) => vec![c.name.to_string()],
        Basis::Derived { from, .. } => from.iter().flat_map(rule_names).collect(),
        Basis::Operator { .. }
        | Basis::Printed(_)
        | Basis::Claim(_)
        | Basis::Machine(_)
        | Basis::ToolFact(_) => Vec::new(),
    }
}

fn machine_fields(b: &Basis) -> Vec<MachineField> {
    match b {
        Basis::Machine(f) => vec![*f],
        Basis::Derived { from, .. } => from.iter().flat_map(machine_fields).collect(),
        Basis::Operator { .. }
        | Basis::Printed(_)
        | Basis::Claim(_)
        | Basis::Rule(_)
        | Basis::ToolFact(_) => Vec::new(),
    }
}

/// The proposal sits on the edge of `kind`, which binds it, and the
/// proposal is below the operator's point.
fn assert_binds(env: &SafeEnvelope, kind: LimitKind) {
    assert_eq!(env.proposal_binding, Some(kind), "the binding limit");
    let p = env.proposal.get().point().expect("a proposed depth");
    let stated = env.stated.expect("the cell states both axes");
    let limit = env.limit(kind).expect("the envelope keeps the limit");
    match &limit.edge {
        Edge::AeCeiling { mm } => {
            assert!(
                (p.ae_mm - mm).abs() < 1e-9,
                "{kind:?}: ae {} not on {mm}",
                p.ae_mm
            );
            assert!(
                p.ae_mm < stated.ae_mm,
                "{kind:?}: the proposal did not lower ae"
            );
        }
        Edge::ApCeiling { .. } | Edge::ApCurve { .. } => {
            // P19: the drawn curve holds no sample at the proposal width;
            // the judged ceiling there is the exact one.
            let cap = env
                .judged_ap_ceiling(kind, p.ae_mm)
                .expect("an ap edge at the proposal");
            assert!(
                (p.ap_mm - cap).abs() < 1e-9,
                "{kind:?}: ap {} not on {cap}",
                p.ap_mm
            );
            assert!(
                p.ap_mm < stated.ap_mm,
                "{kind:?}: the proposal did not lower ap"
            );
        }
        Edge::ApFloor { .. }
        | Edge::AeFloor { .. }
        | Edge::AeCurve { .. }
        | Edge::PerPoint
        | Edge::Unmodelled { .. } => {
            panic!("{kind:?}: not an edge that lowers a proposal")
        }
    }
}

#[test]
fn the_flute_guard_binds_and_names_its_factor_k2b() {
    // P11b (L6): at the full Suggest chip the vendor depth table binds
    // under the 8 mm guard (measured). At half the chip the table binds no
    // width, so the guard binds.
    let env = envelope_at_chip_share(
        &Cell {
            op: OperationType::Adaptive,
            tool: end_mill(6.0, 10.0),
            machine: MachineProfile::default(),
            material: softwood(),
            dpp: 9.0,
            stepover: 0.5,
            bbox: None,
        },
        0.5,
    );
    assert_binds(&env, LimitKind::FluteGuard);
    let names = rule_names(&env.limit(LimitKind::FluteGuard).expect("kept").basis);
    assert!(
        names.contains(&"feeds.calculate.flute_guard_factor".to_owned()),
        "{names:?}"
    );
    // The cutting length is the same 10 mm; the guard is 0.8 x it, so the
    // cutting-length edge sits above the guard and never binds first.
    let guard = env
        .limit(LimitKind::FluteGuard)
        .and_then(|l| l.ap_ceiling_at(1.0));
    let length = env
        .limit(LimitKind::CuttingLength)
        .and_then(|l| l.ap_ceiling_at(1.0));
    assert!(
        length > guard,
        "cutting length {length:?} vs guard {guard:?}"
    );
    assert_eq!(
        env.limit(LimitKind::CuttingLength).map(|l| l.basis.clone()),
        Some(Basis::tool_fact(
            rs_cam_core::feeds::sourced::ToolFactName::CuttingLength,
            rs_cam_core::compute::tool_config::FactKind::Stated,
        ))
    );
}

#[test]
fn the_vendor_axial_bound_binds_and_names_its_row_k2b() {
    use rs_cam_core::compute::catalog::OperationConfig;
    use rs_cam_core::compute::operation_configs::ClearingStrategy;
    let cell = Cell {
        op: OperationType::Adaptive3d,
        tool: end_mill(6.0, 20.0),
        machine: MachineProfile::default(),
        material: softwood(),
        dpp: 9.0,
        stepover: 0.5,
        bbox: None,
    };
    let strategy = |s: ClearingStrategy| {
        move |op: &mut OperationConfig| {
            if let OperationConfig::Adaptive3d(c) = op {
                c.clearing_strategy = s;
            }
        }
    };
    // A geodesic 3D Rough declares no full slot that it always cuts (its
    // residual sweep may occur: a note, never a ceiling), so the vendor
    // DOC still binds the stated 9 mm.
    let env = envelope_of(&cell, 1.0, strategy(ClearingStrategy::GeodesicAdaptive));
    assert_binds(&env, LimitKind::VendorAxial);
    let basis = &env.limit(LimitKind::VendorAxial).expect("kept").basis;
    assert!(matches!(basis, Basis::Printed(_)), "{basis:?}");
    // P29-7 7c: the ContourParallel 3D Rough always cuts its perimeter
    // sweep, a full slot, and the force budget of that slot binds under the
    // vendor DOC (3.85 mm in this softwood cell).
    let env = envelope_of(&cell, 1.0, strategy(ClearingStrategy::ContourParallel));
    assert_binds(&env, LimitKind::Rigidity);
    assert_eq!(
        env.limit(LimitKind::Rigidity).and_then(|l| l.binding_cut),
        Some(CutKind::FirstPass(FirstPassKind::Slot)),
        "the first slot sets the force budget"
    );
}

#[test]
fn the_rigidity_cap_binds_and_names_the_machine_k2b() {
    // P13c (O1): the rigidity cap is the force-budget depth. In softwood at
    // a 0.5 mm width no depth below the flute reaches the budget, so the
    // cell is oak at a 3 mm width (the force-budget depth there is about
    // 3.5 mm) with a stated 5 mm; narrower widths reach 5 mm only below
    // 1.5 mm, which removes less, so the removal-rate pick stays at 3 mm.
    let env = envelope(&Cell {
        op: OperationType::Pocket,
        tool: end_mill(6.0, 20.0),
        machine: MachineProfile::default(),
        material: oak(),
        dpp: 5.0,
        stepover: 3.0,
        bbox: None,
    });
    assert_binds(&env, LimitKind::Rigidity);
    let basis = &env.limit(LimitKind::Rigidity).expect("kept").basis;
    assert_eq!(machine_fields(basis), vec![MachineField::Rigidity]);
}

#[test]
fn the_tool_diameter_binds_the_width_k2b() {
    // P13c (O3): a Pocket width binds on the corner-coverage rule
    // (0.854 D, under D); the tool diameter binds a width that is not
    // computed (an Adaptive).
    let pocket = envelope(&Cell {
        op: OperationType::Pocket,
        tool: end_mill(6.0, 20.0),
        machine: MachineProfile::default(),
        material: softwood(),
        dpp: 0.5,
        stepover: 8.0,
        bbox: None,
    });
    assert!(
        pocket.proposal_limits.contains(&LimitKind::CornerCoverage),
        "{:?}",
        pocket.proposal_limits
    );
    let env = envelope(&Cell {
        op: OperationType::Adaptive,
        tool: end_mill(6.0, 20.0),
        machine: MachineProfile::default(),
        material: softwood(),
        dpp: 0.5,
        stepover: 8.0,
        bbox: None,
    });
    assert_binds(&env, LimitKind::Diameter);
    assert_eq!(
        env.limit(LimitKind::Diameter).map(|l| l.basis.clone()),
        Some(Basis::tool_diameter())
    );
}

/// P29-4 (ruling R1): the Adaptive family reads the force budget. On the
/// hobby budget this cell's budget depth (1.33 mm at 8 mm width) binds
/// before power (12.46 mm), and a probe over the presets, five tools, ten
/// species and MDF found no Adaptive cell where power binds first. So the
/// cell states a budget that no cut reaches (`f64::MAX`): the force budget
/// does not bind, and the power edge binds.
fn unbounded_budget(mut machine: MachineProfile) -> MachineProfile {
    machine.rigidity.force_budget_n = f64::MAX;
    machine
}

#[test]
fn the_spindle_power_binds_and_names_the_curve_k2b() {
    let env = envelope(&Cell {
        op: OperationType::Adaptive,
        tool: end_mill(12.0, 30.0),
        machine: unbounded_budget(MachineProfile::default()),
        material: oak(),
        dpp: 15.0,
        stepover: 8.0,
        bbox: None,
    });
    assert_binds(&env, LimitKind::Power);
    let basis = &env.limit(LimitKind::Power).expect("kept").basis;
    assert_eq!(machine_fields(basis), vec![MachineField::PowerCurve]);
}

#[test]
fn the_deflection_target_binds_and_names_its_constant_k2b() {
    // P11a (L5, D-A): the engaged-edge force bends the 50 mm Ø12 less,
    // and the grid sizes power at the requested depth, so power binds
    // there first. A longer stickout (80 mm) and a narrower cut keep the
    // deflection edge the binding one. P29-4: the hobby budget depth
    // (1.45 mm) binds before deflection (2.92 mm), so the cell states a
    // budget that no cut reaches (`unbounded_budget`).
    let mut tool = end_mill(12.0, 40.0);
    tool.set_stickout(ToolFact::Stated(80.0));
    let env = envelope(&Cell {
        op: OperationType::Adaptive,
        tool,
        machine: unbounded_budget(MachineProfile::shapeoko_vfd()),
        material: oak(),
        dpp: 15.0,
        stepover: 2.0,
        bbox: None,
    });
    assert_binds(&env, LimitKind::Deflection);
    let names = rule_names(&env.limit(LimitKind::Deflection).expect("kept").basis);
    assert_eq!(
        names,
        vec!["feeds.suggest.DEFLECTION_BACKOFF_TARGET_UM".to_owned()]
    );
}

#[test]
fn the_move_budget_is_a_floor_that_places_a_fine_stepover_outside_k2b() {
    let bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, -20.0),
        max: P3::new(140.0, 150.0, 0.0),
    };
    let env = envelope(&Cell {
        op: OperationType::Adaptive,
        tool: end_mill(6.0, 20.0),
        machine: MachineProfile::default(),
        material: softwood(),
        dpp: 0.5,
        stepover: 0.02,
        bbox: Some(&bbox),
    });
    let limit = env
        .limit(LimitKind::Runtime)
        .expect("the move budget is kept");
    let Edge::AeFloor { mm } = limit.edge else {
        panic!("the move budget is an ae floor: {:?}", limit.edge)
    };
    assert!(mm > 0.02, "floor {mm}");
    assert!(
        matches!(
            env.operator,
            Some(Placement::Outside {
                binding: LimitKind::Runtime,
                ..
            })
        ),
        "{:?}",
        env.operator
    );
    // K2: the proposal never raises the stated stepover to the floor.
    assert!(env.proposal.get().ae_mm <= 0.02 + 1e-12);
    let names = rule_names(&limit.basis);
    assert!(
        names.contains(&"feeds.suggest.STEPOVER_BACKOFF_TARGET_MOVES".to_owned()),
        "{names:?}"
    );
}

#[test]
fn the_engagement_floor_places_a_tiny_depth_outside_k2b() {
    let env = envelope(&Cell {
        op: OperationType::Pocket,
        tool: end_mill(6.0, 20.0),
        machine: MachineProfile::default(),
        material: softwood(),
        dpp: 0.01,
        stepover: 1.0,
        bbox: None,
    });
    assert!(
        matches!(
            env.operator,
            Some(Placement::Outside {
                binding: LimitKind::EngagementFloor,
                ..
            })
        ),
        "{:?}",
        env.operator
    );
    assert!(
        env.proposal.get().ap_mm().expect("a proposed depth") <= 0.01 + 1e-12,
        "no raise (K2)"
    );
}

#[test]
fn the_per_point_limits_are_kept_with_their_basis_k2b() {
    // A machine whose cutting-feed cap is far under the chip's feed: every
    // grid point is held at the cap, and the chip falls under the rubbing
    // floor.
    let machine = MachineProfile {
        max_cutting_feed_mm_min: Some(100.0),
        ..MachineProfile::default()
    };
    let env = envelope(&Cell {
        op: OperationType::Pocket,
        tool: end_mill(6.0, 20.0),
        machine,
        material: softwood(),
        dpp: 0.5,
        stepover: 1.0,
        bbox: None,
    });
    let band = env
        .limit(LimitKind::ChipBand)
        .expect("the chip band is kept");
    assert!(matches!(band.basis, Basis::Printed(_)), "{:?}", band.basis);
    let cap = env.limit(LimitKind::FeedCap).expect("the feed cap is kept");
    assert_eq!(
        machine_fields(&cap.basis),
        vec![MachineField::CuttingFeedCap]
    );
    let rubbing = env
        .limit(LimitKind::RubbingFloor)
        .expect("the rubbing floor is kept");
    assert!(
        matches!(rubbing.basis, Basis::Printed(_) | Basis::Rule(_)),
        "{:?}",
        rubbing.basis
    );
    for l in [band, cap, rubbing] {
        assert_eq!(l.edge, Edge::PerPoint, "{:?}", l.kind);
    }
    assert!(env.speeds.points.iter().all(|p| p.feed_capped));
}
