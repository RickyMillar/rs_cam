//! Feeds contract P29-7 7c
//! (`planning/feeds_contract_2026-11-03/phases/P29-7_DESIGN.md` sections 2.3,
//! 2.4 and 3): every limit before the simulation judges every declared cut,
//! through one driver, on the one scope table of both sides.
//!
//! The configs: each op of `OperationType::ALL` (`fixture_op`, and the add
//! door `default_operation_for_tool` with no stated speeds, as the FM1
//! instrument builds it), on the tools of the FM1 tool builder and the
//! slender end mill of the deflection sentry, in hardwood, on the default
//! machine.
//!
//! - Claim 1: for every declared cut and every planned limit, the driver
//!   (`feeds::planned::judge_declared_cuts`) returns exactly one cell, and
//!   the state of the cell follows `limits::scope(limit, cut.kind.group())`.
//!   The lists of a limit agree: a limit that `limits::scope` judges on a
//!   group has a planned role (`planned_limit_of`: a depth ceiling or a
//!   value check) that names the limit, and has a gate OR a named exemption
//!   row for the side after the simulation; a limit with no judged group has
//!   the role `NotACutLimit` with the reason that `limits::scope` gives on
//!   every group; `gate_of` names exactly the gates whose `LIMITS` name the
//!   limit. Non-vacuity: the force budget and the deflection have a role and
//!   a gate.
//! - Claim 2: at each width column of the envelope, the drawn edge of the
//!   force budget and of the deflection is the lowest depth over the judged
//!   cuts with `Presence::Always` (the edge holds every declared cut). The
//!   test takes the minimum itself, so it does not read
//!   `ByCut::lowest_always`. Every depth ceiling (`CeilingEdge::ALL`)
//!   draws an edge on a config.
//! - Claim 3: Suggest's proposal sits at or under the ceiling of every
//!   declared cut with `Presence::Always` (`SafeEnvelope::cut_checks`:
//!   the force budget, the deflection and the vendor depth table), the
//!   acceptance of the change list.
//! - Claim 4: non-vacuity. A contour Pocket (EndMill 3.175 mm, hardwood)
//!   sets its force budget on the slot ring, a geodesic 3D Rough (EndMill
//!   6 mm, stepover 1.2 mm) on the planner cap, and the configs give a
//!   planned cut of each chip reason (`PlannerCapped`, `EntryRule`,
//!   `DrillCycle`) and of each width reason (`NoStatedWidth`,
//!   `NoStatedDepth`).

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::collections::BTreeSet;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType, Presence};
use rs_cam_core::compute::operation_configs::ClearingStrategy;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::SpindleStrategy;
use rs_cam_core::feeds::envelope::{EnvelopeInput, LimitKind, SafeEnvelope};
use rs_cam_core::feeds::planned::{
    BodyColumn, CeilingEdge, ChipEnv, ChipMaxPlanned, ChipMinPlanned, DeclaredCuts,
    DeflectionPlanned, ForceBudgetPlanned, ForceEnv, PlaneRole, PlannedCell, PlannedLimit,
    PlannedNoNumber, PowerEnv, RubbingFloorPlanned, SpindlePowerPlanned, TableEnv,
    VendorDepthTablePlanned, judge_declared_cuts, planned_limit_of,
};
use rs_cam_core::feeds::suggest::{StockContext, SuggestContext};
use rs_cam_core::limits::{
    CutGroup, CutKind, DepthLimit, LimitId, NoEngagementModel, Scope, scope,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::tool_load::groups::{Gate, gate_of};

/// The tool of the FM1 instrument (`feeds_matrix_instrument_fm1`).
fn fm1_tool(kind: ToolType, d: f64) -> ToolConfig {
    let mut t = ToolConfig::test_default(ToolId::from_raw(1), kind);
    t.diameter = d;
    t.set_flute_count(ToolFact::Stated(2));
    t.set_cutting_length(ToolFact::Stated((d * 3.0).max(12.0)));
    t.set_shank_diameter(ToolFact::Stated(d.max(3.0)));
    t.shaft_diameter = d.max(3.0);
    t.set_stickout(ToolFact::Stated(t.cutting_length_planning_mm() + 8.0));
    if matches!(kind, ToolType::BullNose) {
        t.corner_radius = d * 0.15;
        t.corner_radius_mm = d * 0.15;
    }
    if matches!(kind, ToolType::TaperedBallNose) {
        t.set_taper_half_angle(ToolFact::Stated(7.0));
        t.set_shank_diameter(ToolFact::Stated((d + 3.0).max(6.0)));
        t.shaft_diameter = t.shank_diameter_planning_mm();
    }
    if matches!(kind, ToolType::VBit) {
        t.included_angle = 60.0;
    }
    t
}

fn hardwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    }
}

/// The add-door op of `op_type` on `tool`, with no stated speeds (the FM1
/// rule): the envelope reads the grid chip.
fn add_door_op(
    op_type: OperationType,
    tool: &ToolConfig,
    machine: &MachineProfile,
) -> OperationConfig {
    let stock = StockContext {
        stock_top_z: 0.0,
        stock_bottom_z: -18.0,
        stock_z: Some(18.0),
        stock_padding: 2.0,
    };
    let material = hardwood();
    let (mut op, _) = rs_cam_core::feeds::suggest::default_operation_for_tool(
        op_type,
        &stock,
        rs_cam_core::compute::geometry_default::RuleContext {
            tool,
            material: &material,
            machine,
        },
    );
    rs_cam_core::compute::fixture_values::fill_unset(&mut op);
    op.set_spindle_rpm(None);
    op
}

fn tools() -> Vec<ToolConfig> {
    vec![
        fm1_tool(ToolType::EndMill, 3.175),
        fm1_tool(ToolType::EndMill, 6.0),
        fm1_tool(ToolType::BallNose, 6.0),
        fm1_tool(ToolType::TaperedBallNose, 6.0),
        fm1_tool(ToolType::VBit, 6.35),
        slender_tool(),
    ]
}

/// The slender end mill of `the_deflection_edge_is_a_solved_depth_p11a`
/// (3 mm, a 12 mm flute, a 30 mm stick-out): the FM1 tools have no
/// deflection root below the flute, this one has.
fn slender_tool() -> ToolConfig {
    let mut t = fm1_tool(ToolType::EndMill, 3.0);
    t.set_cutting_length(ToolFact::Stated(12.0));
    t.set_stickout(ToolFact::Stated(30.0));
    t
}

/// Every config with its tool: each op at its fixture values, each op at
/// the add door with no speeds, and both 3D Rough strategies.
fn configs(machine: &MachineProfile) -> Vec<(String, OperationConfig, ToolConfig)> {
    let mut out = Vec::new();
    for tool in tools() {
        for &op_type in OperationType::ALL {
            out.push((
                format!("{op_type:?}/fixture/{:?}{}", tool.tool_type, tool.diameter),
                rs_cam_core::compute::fixture_values::fixture_op(op_type),
                tool.clone(),
            ));
            out.push((
                format!("{op_type:?}/add/{:?}{}", tool.tool_type, tool.diameter),
                add_door_op(op_type, &tool, machine),
                tool.clone(),
            ));
        }
        for strategy in [
            ClearingStrategy::GeodesicAdaptive,
            ClearingStrategy::ContourParallel,
        ] {
            let mut op = add_door_op(OperationType::Adaptive3d, &tool, machine);
            if let OperationConfig::Adaptive3d(c) = &mut op {
                c.clearing_strategy = strategy;
            }
            out.push((
                format!(
                    "Adaptive3d/{strategy:?}/{:?}{}",
                    tool.tool_type, tool.diameter
                ),
                op,
                tool.clone(),
            ));
        }
    }
    out
}

fn envelope(
    op: &OperationConfig,
    tool: &ToolConfig,
    machine: &MachineProfile,
) -> Option<SafeEnvelope> {
    SafeEnvelope::for_cut(EnvelopeInput {
        operation: op,
        tool,
        machine,
        material: &hardwood(),
        lut: rs_cam_core::feeds::embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::default(),
        context: SuggestContext::default(),
    })
    .ok()
}

fn budget_n(machine: &MachineProfile) -> f64 {
    machine
        .rigidity
        .depth_cap_of(DepthLimit::ForceBudget)
        .expect("the default machine has a force budget")
        .budget_n
}

/// The state of a cell, for claim 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Judged,
    NotJudged(&'static str),
    NotACutLimit,
}

fn state<A>(cell: &PlannedCell<A>) -> State {
    match cell {
        PlannedCell::Judged(_) => State::Judged,
        PlannedCell::NotJudged(n) => State::NotJudged(n.row),
        PlannedCell::NotACutLimit(_) => State::NotACutLimit,
    }
}

fn want(limit: LimitId, kind: CutKind) -> State {
    match scope(limit, kind.group()) {
        Scope::Judged => State::Judged,
        Scope::NotJudged(n) => State::NotJudged(n.row),
        Scope::NotACutLimit(_) => State::NotACutLimit,
    }
}

/// The cells of one limit: `(kind, state)` per declared cut.
fn cells<L: PlannedLimit>(
    cuts: &DeclaredCuts,
    column: BodyColumn,
    env: &L::Env<'_>,
) -> Vec<(CutKind, State)> {
    judge_declared_cuts::<L>(cuts, column, env)
        .iter()
        .map(|e| (e.kind, state(&e.value)))
        .collect()
}

#[test]
fn claim_1_one_cell_for_each_cut_and_the_scope_decides_it() {
    // The lists of each limit agree.
    let mut found_force_and_deflection = 0;
    for limit in LimitId::ALL {
        let judged_somewhere = CutGroup::ALL
            .iter()
            .any(|&g| matches!(scope(limit, g), Scope::Judged));
        let role = planned_limit_of(limit);
        match role {
            PlaneRole::DepthCeiling(_) | PlaneRole::ValueCheck(_) => {
                assert_eq!(
                    role.limit(),
                    Some(limit),
                    "{limit:?}: the role names the limit"
                );
                assert!(
                    judged_somewhere,
                    "{limit:?}: a planned role and no judged group"
                );
                // The side after the simulation: a gate, or a named row.
                let row = rs_cam_core::limits::EXEMPTIONS
                    .iter()
                    .find(|e| e.limit.contains(&limit));
                assert!(
                    !gate_of(limit).is_empty() || row.is_some(),
                    "{limit:?}: judged before the simulation, with no gate and no exemption row"
                );
            }
            PlaneRole::NotACutLimit(reason) => {
                assert!(
                    !judged_somewhere,
                    "{limit:?}: the scope judges it and the role is not a cut limit"
                );
                for group in CutGroup::ALL {
                    assert_eq!(
                        scope(limit, group),
                        Scope::NotACutLimit(reason),
                        "{limit:?} {group:?}: the role states the reason of the scope"
                    );
                }
            }
        }
        let gates: Vec<Gate> = Gate::ALL
            .iter()
            .copied()
            .filter(|g| g.limits().contains(&limit))
            .collect();
        assert_eq!(gate_of(limit), gates.as_slice(), "{limit:?}: gate_of");
        if matches!(limit, LimitId::ForceBudget | LimitId::ToolDeflection)
            && role.limit().is_some()
            && !gate_of(limit).is_empty()
        {
            found_force_and_deflection += 1;
        }
    }
    assert_eq!(
        found_force_and_deflection, 2,
        "the force budget and the deflection have a role and a gate"
    );

    let machine = MachineProfile::default();
    let material = hardwood();
    let mut cuts_seen = 0;
    let mut not_judged_seen = 0;
    for (name, op, tool) in configs(&machine) {
        let Ok(tool_def) = rs_cam_core::compute::cutter::build_cutter(&tool) else {
            continue;
        };
        let cuts = DeclaredCuts::of_operation(&op, &tool);
        let declared: Vec<CutKind> = cuts.list().iter().map(|c| c.kind).collect();
        let column = BodyColumn {
            ae_mm: 0.4 * tool.diameter,
            chip_mm: Some(0.05),
        };
        let force = ForceEnv {
            tool: &tool_def,
            material: &material,
            bound: budget_n(&machine),
        };
        let table = TableEnv {
            band_max_mm: 0.1,
            d_mm: tool.diameter,
            top_mm: 3.0 * tool.diameter,
        };
        let power_kw = |w: f64| w;
        let power = PowerEnv {
            column_ae_mm: column.ae_mm,
            power_kw: &power_kw,
        };
        let chip = ChipEnv {
            thinning: cuts.thinning(),
            d_eng_mm: tool.diameter,
            bound_mm: 0.1,
        };
        let all: [(LimitId, Vec<(CutKind, State)>); 7] = [
            (
                LimitId::ForceBudget,
                cells::<ForceBudgetPlanned>(&cuts, column, &force),
            ),
            (
                LimitId::ToolDeflection,
                cells::<DeflectionPlanned>(&cuts, column, &force),
            ),
            (
                LimitId::VendorDepthTable,
                cells::<VendorDepthTablePlanned>(&cuts, column, &table),
            ),
            (
                LimitId::SpindlePower,
                cells::<SpindlePowerPlanned>(&cuts, column, &power),
            ),
            (
                LimitId::ChipMax,
                cells::<ChipMaxPlanned>(&cuts, column, &chip),
            ),
            (
                LimitId::ChipMin,
                cells::<ChipMinPlanned>(&cuts, column, &chip),
            ),
            (
                LimitId::RubbingFloor,
                cells::<RubbingFloorPlanned>(&cuts, column, &chip),
            ),
        ];
        for (limit, got) in all {
            let kinds: Vec<CutKind> = got.iter().map(|(k, _)| *k).collect();
            assert_eq!(kinds, declared, "{name} {limit:?}: one cell for each cut");
            for (kind, s) in got {
                assert_eq!(
                    s,
                    want(limit, kind),
                    "{name} {limit:?} {kind:?}: the scope cell"
                );
                cuts_seen += 1;
                if matches!(s, State::NotJudged(_)) {
                    not_judged_seen += 1;
                }
            }
        }
    }
    assert!(cuts_seen > 1000, "the configs give cuts: {cuts_seen}");
    assert!(
        not_judged_seen > 0,
        "a drill cycle gives a cell that is not judged"
    );
}

/// The lowest depth over the judged `Always` cells, taken here (no driver
/// fold): the force-budget ceiling ranks no root at `top`.
fn lowest<A: Copy>(
    cells: &rs_cam_core::feeds::planned::ByCut<PlannedCell<A>>,
    value: impl Fn(A) -> Option<f64>,
) -> Option<f64> {
    let mut best: Option<f64> = None;
    for e in cells.iter() {
        if e.presence != Presence::Always {
            continue;
        }
        if let PlannedCell::Judged(a) = e.value
            && let Some(v) = value(a)
        {
            best = Some(best.map_or(v, |b: f64| b.min(v)));
        }
    }
    best
}

#[test]
fn claim_2_each_drawn_edge_is_the_lowest_over_the_declared_cuts() {
    let machine = MachineProfile::default();
    let material = hardwood();
    let mut columns_checked = 0;
    let mut drawn_kinds = BTreeSet::new();
    for (name, op, tool) in configs(&machine) {
        // The add door states no speeds, so each drawn column reads the
        // grid chip at the shallowest depth.
        if op.spindle_rpm().is_some() {
            continue;
        }
        let Some(env) = envelope(&op, &tool, &machine) else {
            continue;
        };
        let Ok(tool_def) = rs_cam_core::compute::cutter::build_cutter(&tool) else {
            continue;
        };
        for l in &env.boundary {
            if matches!(
                l.edge,
                rs_cam_core::feeds::envelope::Edge::ApCurve { .. }
                    | rs_cam_core::feeds::envelope::Edge::ApCeiling { .. }
            ) {
                drawn_kinds.insert(format!("{:?}", l.kind));
            }
        }
        let cuts = DeclaredCuts::of_operation(&op, &tool);
        let Some(&top) = env.speeds.aps.last() else {
            continue;
        };
        for (j, &ae) in env.speeds.aes.iter().enumerate() {
            let Some(point) = env.speeds.at(0, j) else {
                continue;
            };
            let column = BodyColumn {
                ae_mm: ae,
                chip_mm: Some(point.chip.get().0),
            };
            if let Some(l) = env.limit(LimitKind::Rigidity)
                && let Some(drawn) = l.ap_ceiling_at(ae)
                && matches!(
                    l.edge,
                    rs_cam_core::feeds::envelope::Edge::ApCurve { .. }
                        | rs_cam_core::feeds::envelope::Edge::ApCeiling { .. }
                )
            {
                let force = ForceEnv {
                    tool: &tool_def,
                    material: &material,
                    bound: budget_n(&machine),
                };
                let cells = judge_declared_cuts::<ForceBudgetPlanned>(&cuts, column, &force);
                let want = lowest(&cells, |a| {
                    a.map(|d| d.ceiling_mm().unwrap_or(f64::INFINITY))
                })
                .map(|v| if v.is_finite() { v } else { top });
                if let Some(want) = want {
                    assert!(
                        (drawn - want).abs() <= 1e-9 * want.max(1.0),
                        "{name} ae {ae}: the force-budget edge {drawn}, the lowest cut {want}"
                    );
                    columns_checked += 1;
                }
            }
            if let Some(l) = env.limit(LimitKind::Deflection)
                && matches!(l.edge, rs_cam_core::feeds::envelope::Edge::ApCurve { .. })
                && let Some(drawn) = l.ap_ceiling_at(ae)
            {
                let force = ForceEnv {
                    tool: &tool_def,
                    material: &material,
                    // The finish target (um): the bound of the deflection gate.
                    bound: rs_cam_core::tool_load::deflection::EXCEEDS_BOUND_MM * 1000.0,
                };
                let cells = judge_declared_cuts::<DeflectionPlanned>(&cuts, column, &force);
                let want = lowest(&cells, |a| a).unwrap_or(top);
                assert!(
                    (drawn - want).abs() <= 1e-9 * want.max(1.0),
                    "{name} ae {ae}: the deflection edge {drawn}, the lowest cut {want}"
                );
                columns_checked += 1;
            }
        }
    }
    assert!(
        columns_checked > 100,
        "the drawn columns checked: {columns_checked}"
    );
    for edge in CeilingEdge::ALL {
        assert!(
            drawn_kinds.contains(&format!("{:?}", edge.kind())),
            "{edge:?}: no config draws its edge: {drawn_kinds:?}"
        );
    }
}

#[test]
fn claim_3_the_proposal_sits_inside_every_declared_cut() {
    let machine = MachineProfile::default();
    let mut rows_checked = 0;
    for (name, op, tool) in configs(&machine) {
        let Some(env) = envelope(&op, &tool, &machine) else {
            continue;
        };
        let (Some(ap), Some(&floor)) = (env.proposal.get().ap_mm(), env.speeds.aps.first()) else {
            continue;
        };
        for row in &env.cut_checks {
            if row.may_occur_row.is_some()
                || !matches!(
                    row.check.limit,
                    LimitId::ForceBudget | LimitId::ToolDeflection | LimitId::VendorDepthTable
                )
            {
                continue;
            }
            let Some(bound) = row.check.bound else {
                continue;
            };
            // A ceiling under the smallest depth (the first depth of the
            // grid) ships the smallest depth (the force-budget floor arm, Q6).
            let bound = bound.max(floor);
            assert!(
                ap <= bound * (1.0 + 1e-9),
                "{name}: the proposal {ap} mm is over the {:?} ceiling {bound} mm of the {}",
                row.check.limit,
                row.cut.label()
            );
            rows_checked += 1;
        }
    }
    assert!(rows_checked > 100, "the rows checked: {rows_checked}");
}

#[test]
fn claim_4_the_slot_ring_and_the_planner_cap_bind_and_each_reason_occurs() {
    let machine = MachineProfile::default();
    // A contour Pocket, EndMill 3.175 mm, hardwood: the slot ring sets the
    // force budget (the FM1 cell).
    let tool = fm1_tool(ToolType::EndMill, 3.175);
    let pocket = add_door_op(OperationType::Pocket, &tool, &machine);
    let env = envelope(&pocket, &tool, &machine).expect("the FM1 Pocket is backed");
    assert_eq!(
        env.limit(LimitKind::Rigidity).and_then(|l| l.binding_cut),
        Some(CutKind::SlotRing),
        "the contour Pocket sets its force budget on the slot ring"
    );
    let p = env.proposal.get();
    assert_eq!(
        Some(p.ap_mm().expect("a depth")),
        env.judged_ap_ceiling(LimitKind::Rigidity, p.ae_mm),
        "the Pocket depth stands on the force budget"
    );
    // A geodesic 3D Rough, EndMill 6 mm, stepover 1.2 mm, a deep stated
    // depth: the planner cap sets the force budget.
    let tool = fm1_tool(ToolType::EndMill, 6.0);
    let mut rough = add_door_op(OperationType::Adaptive3d, &tool, &machine);
    if let OperationConfig::Adaptive3d(c) = &mut rough {
        c.clearing_strategy = ClearingStrategy::GeodesicAdaptive;
        c.stepover = Some(1.2);
        c.depth_per_pass = Some(6.0);
    }
    let env = envelope(&rough, &tool, &machine).expect("the 3D Rough is backed");
    assert_eq!(env.proposal_binding, Some(LimitKind::Rigidity));
    assert_eq!(
        env.limit(LimitKind::Rigidity).and_then(|l| l.binding_cut),
        Some(CutKind::FirstPass(
            rs_cam_core::limits::FirstPassKind::PlannerCap
        )),
        "the geodesic 3D Rough sets its force budget on the planner cap"
    );
    // Each chip reason and each width reason occurs.
    let mut chips = BTreeSet::new();
    let mut widths = BTreeSet::new();
    for (_, op, tool) in configs(&machine) {
        let cuts = DeclaredCuts::of_operation(&op, &tool);
        let column = BodyColumn {
            ae_mm: 0.4 * tool.diameter,
            chip_mm: Some(0.05),
        };
        for cut in cuts.list().iter() {
            let planned = cuts.planned(cut, column);
            if let Err(reason) = planned.chip_mm {
                chips.insert(format!("{reason:?}"));
            }
            if let Err(reason) = planned.width_mm {
                widths.insert(format!("{reason:?}"));
            }
        }
    }
    for reason in [
        PlannedNoNumber::PlannerCapped,
        PlannedNoNumber::EntryRule,
        PlannedNoNumber::DrillCycle,
    ] {
        assert!(
            chips.contains(&format!("{reason:?}")),
            "a cut with no chip: {reason:?}"
        );
    }
    for reason in [
        NoEngagementModel::NoStatedWidth,
        NoEngagementModel::NoStatedDepth,
    ] {
        assert!(
            widths.contains(&format!("{reason:?}")),
            "a cut with no width: {reason:?}"
        );
    }
}
