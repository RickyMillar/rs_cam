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
//!   value check) that names the limit, and has a gate OR is a limit of the
//!   row `S-NoSampleGate` (P29-11 section 13, ruling 1; each limit of that
//!   row has no gate) for the side after the simulation; a limit with no judged group has
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
//!   declared cut with `Presence::Always` (`SafeEnvelope::rows`, the planned rows:
//!   the force budget, the deflection and the vendor depth table), the
//!   acceptance of the change list.
//! - Claim 4: non-vacuity. A contour Pocket (EndMill 3.175 mm, hardwood)
//!   sets its force budget on the slot ring, a geodesic 3D Rough (EndMill
//!   6 mm, stepover 1.2 mm) on the planner cap, and the configs give a
//!   planned cut of each chip reason (`PlannerCapped`, an entry rule with
//!   no number, `DrillCycle`) and of each width reason (`NoStatedWidth`,
//!   `NoStatedDepth`).
//! - Claim 5 (P29-8 8d, lead ruling Q1): an entry at its own depth is a
//!   value check, never a ceiling. Every row of a cut at its own depth holds
//!   the own depth as its value; a depth-ceiling row is `Exceeds` exactly
//!   when the own depth is above the bound; no entry is the binding cut of
//!   an edge; claims 2 and 3 skip a cut at its own depth. A plunge (the
//!   axial circle) holds a number only for the spindle power and (P29-8e,
//!   the check (c)) for the vertical rate (`LimitId::PlungeRate`, mm/min).
//!   Non-vacuity:
//!   a ramp and a helix row hold a force-budget number and a plunge row a
//!   power number on a config with stated speeds.
//! - Claim 6 (P29-9 task 1): a chip check of a stored entry feed reads the
//!   interval `[f, f + step) / (rpm x Z)` that the write step states
//!   (`entry_rule::StoredFeedChip`). Through the real Suggest write, a ramp
//!   feed one step under a point band is `Within` the band minimum; half
//!   the feed `Exceeds`; a feed at the band maximum is `Within` it (a
//!   ceiling reads the stated chip). The margin follows the judged end:
//!   a row inside never holds a margin below zero.
//! - Claim 7 (P29-9 9b, `P29-9_DESIGN.md` section 16): a Project Curve is
//!   judged at the lower bound of its depth into the material
//!   (`TipDepth`, the sign rule of the kernel). From above, a positive
//!   depth gives a body force-budget row at that depth: `NoNumber(
//!   OnlyALowerBound)` inside, `Exceeds` over the ceiling, never `Within`,
//!   and with no margin inside. A tip on or outside the surface gives no
//!   depth row. From below the rule needs the setup: no setup gives no row;
//!   a setup that is not flipped judges `-depth`; a flipped setup `depth`.
//! - Claim 8 (P29-9 9c, ruling Q1 (a), lead ruling after 9c): a finish pass
//!   on one side with no cap is judged at the stock the earlier op leaves.
//!   With a leave, the body power row reads the power at the leave (at the
//!   proposal speeds); with no leave, it is `NoNumber(NoEarlierLeave)`. A
//!   finish has no force-budget edge (`NoNumber(NoBound)`, row `S-NoBound`),
//!   one row per declared cut, never a silent missing row. P29-10 10h (lead
//!   ruling L3): on its fixed depth axis the deflection is a value check at
//!   the fixed depth: the body reads the earlier leave, with the basis of
//!   the beam and the caveat `AtEarlierLeave`; no deflection row of a fixed
//!   axis on any op is `NotJudgedHere`.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::collections::BTreeSet;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType, Presence};
use rs_cam_core::compute::operation_configs::ClearingStrategy;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::SpindleStrategy;
use rs_cam_core::feeds::entry_rule::ConfiguredEntry;
use rs_cam_core::feeds::envelope::{EnvelopeInput, LimitKind, SafeEnvelope};
use rs_cam_core::feeds::planned::{
    BodyColumn, CeilingEdge, ChipEnv, ChipFloor, ChipFloorEnv, ChipMaxPlanned, ChipMinPlanned,
    DeclaredCuts, DeflectionPlanned, ForceBudgetPlanned, ForceEnv, PlaneDepth, PlaneRole,
    PlannedCell, PlannedLimit, PlannedNoNumber, PowerEnv, RubbingFloorPlanned, SpindlePowerPlanned,
    TableEnv, VendorDepthTablePlanned, judge_declared_cuts, planned_limit_of,
};
use rs_cam_core::feeds::suggest::{NoProject, StockContext, SuggestContext};
use rs_cam_core::limits::{CheckRow, CutGroup, CutKind, LimitId, NoEngagementModel, Scope, scope};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::tool_load::groups::{Gate, gate_of};

/// P29-10: the planned rows of an envelope (`Subject::Planned`), in order.
fn planned(env: &SafeEnvelope) -> Vec<CheckRow> {
    env.rows()
        .iter()
        .filter(|r| r.is_planned())
        .cloned()
        .collect()
}

/// The declared cut of a planned row.
fn cut(row: &CheckRow) -> CutKind {
    row.cut().expect("a planned row names its cut")
}

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
        context: SuggestContext::no_project(NoProject::NONE),
        modulation: rs_cam_core::dressup::feed_modulation::RunModulation::Off,
    })
    .ok()
}

fn budget_n(machine: &MachineProfile) -> f64 {
    // P29-11 L-5: the budget of a cut under the force budget is the
    // machine field (`RigidityProfile::force_budget_of` reads it).
    machine.rigidity.force_budget_n
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

/// P29-10 10i (lead ruling 3): the scope reads the group of the declared
/// cut (`planned::planned_group`: the drill group for a drill cycle depth).
fn want(limit: LimitId, group: CutGroup) -> State {
    match scope(limit, group) {
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
    let no_sample_gate = rs_cam_core::limits::EXEMPTIONS
        .iter()
        .find(|e| e.id == "S-NoSampleGate")
        .expect("the row S-NoSampleGate");
    // Each limit of the row has no gate: a new gate closes the row.
    for limit in no_sample_gate.limit {
        assert!(
            gate_of(*limit).is_empty(),
            "{limit:?}: a sample gate judges it; close it in the row S-NoSampleGate"
        );
    }
    let mut no_gate = 0;
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
                // The side after the simulation: a gate, or the named row of
                // the gap (P29-11 section 13, ruling 1: `S-NoSampleGate`).
                assert!(
                    !gate_of(limit).is_empty() || no_sample_gate.limit.contains(&limit),
                    "{limit:?}: judged before the simulation, with no gate and not in the row \
                     S-NoSampleGate"
                );
                if gate_of(limit).is_empty() {
                    no_gate += 1;
                }
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
    assert_eq!(
        no_gate,
        no_sample_gate.limit.len(),
        "non-vacuity: each limit of S-NoSampleGate is a planned limit with no gate"
    );

    let machine = MachineProfile::default();
    let material = hardwood();
    let mut cuts_seen = 0;
    let mut not_judged_seen = 0;
    for (name, op, tool) in configs(&machine) {
        let Ok(tool_def) = rs_cam_core::compute::cutter::build_cutter(&tool) else {
            continue;
        };
        let cuts = DeclaredCuts::of_operation(&op, &tool, &ConfiguredEntry::registry_default(&op));
        let declared: Vec<CutKind> = cuts.list().iter().map(|c| c.kind).collect();
        let groups: Vec<CutGroup> = cuts
            .list()
            .iter()
            .map(rs_cam_core::feeds::planned::planned_group)
            .collect();
        let column = BodyColumn {
            ae_mm: 0.4 * tool.diameter,
            chip_mm: Some(0.05),
            stored: None,
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
        let entry_power_kw = |_, fz: f64| Some(fz);
        let power = PowerEnv {
            column_ae_mm: column.ae_mm,
            power_kw: &power_kw,
            entry_power_kw: &entry_power_kw,
        };
        let chip = ChipEnv {
            thinning: cuts.thinning(),
            d_eng_mm: tool.diameter,
            bound_mm: 0.1,
        };
        let floor = ChipFloorEnv {
            thinning: cuts.thinning(),
            d_eng_mm: tool.diameter,
            floor: ChipFloor::PointReference { mm: 0.1 },
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
                cells::<ChipMinPlanned>(&cuts, column, &floor),
            ),
            (
                LimitId::RubbingFloor,
                cells::<RubbingFloorPlanned>(&cuts, column, &chip),
            ),
        ];
        for (limit, got) in all {
            let kinds: Vec<CutKind> = got.iter().map(|(k, _)| *k).collect();
            assert_eq!(kinds, declared, "{name} {limit:?}: one cell for each cut");
            for ((kind, s), group) in got.into_iter().zip(groups.iter().copied()) {
                assert_eq!(
                    s,
                    want(limit, group),
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

/// The lowest depth over the judged `Always` cells on the body plane, taken
/// here (no driver fold): the force-budget ceiling ranks no root at `top`.
/// P29-8 8d: a cut at its own depth (an entry) is no ceiling.
fn lowest<A: Copy>(
    cells: &rs_cam_core::feeds::planned::ByCut<PlannedCell<A>>,
    value: impl Fn(A) -> Option<f64>,
) -> Option<f64> {
    let mut best: Option<f64> = None;
    for e in cells.iter() {
        if e.presence != Presence::Always || matches!(e.depth, PlaneDepth::Own(_)) {
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
        let cuts = DeclaredCuts::of_operation(&op, &tool, &ConfiguredEntry::registry_default(&op));
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
                stored: None,
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
        for row in &planned(&env) {
            if row.may_occur_row().is_some()
                || row.own_depth_mm().is_some()
                || !matches!(
                    row.check().limit,
                    LimitId::ForceBudget | LimitId::ToolDeflection | LimitId::VendorDepthTable
                )
            {
                continue;
            }
            let Some(bound) = row.check().bound else {
                continue;
            };
            // A ceiling under the smallest depth (the first depth of the
            // grid) ships the smallest depth (the force-budget floor arm, Q6).
            let bound = bound.max(floor);
            assert!(
                ap <= bound * (1.0 + 1e-9),
                "{name}: the proposal {ap} mm is over the {:?} ceiling {bound} mm of the {}",
                row.check().limit,
                cut(row).label()
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
        let cuts = DeclaredCuts::of_operation(&op, &tool, &ConfiguredEntry::registry_default(&op));
        let column = BodyColumn {
            ae_mm: 0.4 * tool.diameter,
            chip_mm: Some(0.05),
            stored: None,
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
    for reason in [PlannedNoNumber::PlannerCapped, PlannedNoNumber::DrillCycle] {
        assert!(
            chips.contains(&format!("{reason:?}")),
            "a cut with no chip: {reason:?}"
        );
    }
    assert!(
        chips.iter().any(|c| c.starts_with("Entry(")),
        "an entry with no chip: {chips:?}"
    );
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

/// P29-8 8d: the envelope of a config with stated speeds (the fixture
/// values), so the entry rule gives a chip.
#[test]
fn claim_5_an_entry_is_a_value_check_never_a_ceiling_p29_8() {
    use rs_cam_core::limits::{CheckState, EntryKind};
    let machine = MachineProfile::default();
    let (mut rows, mut ramp_numbers, mut helix_numbers, mut plunge_power) = (0, 0, 0, 0);
    for (name, mut op, tool) in configs(&machine) {
        // The fixture values state no ramp feed: state the cut feed, so a
        // ramp and a helix entry have a chip.
        let feed = op.feed_rate();
        let _ = op.as_params_mut().set_ramp_feed_rate(feed);
        let Some(env) = envelope(&op, &tool, &machine) else {
            continue;
        };
        for l in &env.boundary {
            assert!(
                !matches!(l.binding_cut, Some(CutKind::Entry(_))),
                "{name}: an entry binds the {:?} edge",
                l.kind
            );
        }
        for row in &planned(&env) {
            // P29-9 (lead ruling): the state and the margin agree.
            assert!(
                row.check()
                    .margin
                    .map(CheckState::of_margin)
                    .is_none_or(|s| s == row.check().state),
                "{name}: the {} row: the state {:?} against the margin {:?}",
                cut(row).label(),
                row.check().state,
                row.check().margin
            );
            // A plunge is the axial circle: only the power and (P29-8e) the
            // vertical rate have a number.
            if matches!(
                cut(row),
                CutKind::Entry(EntryKind::Plunge | EntryKind::PeckLadder)
            ) && matches!(row.check().state, CheckState::Within | CheckState::Exceeds)
            {
                assert!(
                    matches!(
                        row.check().limit,
                        LimitId::SpindlePower | LimitId::PlungeRate
                    ),
                    "{name}: a {} row with a number for {:?}",
                    cut(row).label(),
                    row.check().limit
                );
                if row.check().limit == LimitId::SpindlePower {
                    plunge_power += 1;
                }
            }
            let Some(own) = row.own_depth_mm() else {
                // A ramp or a helix with no own depth has no engagement:
                // its rows hold no number.
                assert!(
                    !matches!(cut(row), CutKind::Entry(EntryKind::Ramp | EntryKind::Helix))
                        || matches!(row.check().state, CheckState::NoNumber(_))
                        || !matches!(
                            row.check().limit,
                            LimitId::ForceBudget
                                | LimitId::ToolDeflection
                                | LimitId::VendorDepthTable
                        ),
                    "{name}: a {} row on the body plane",
                    cut(row).label()
                );
                continue;
            };
            rows += 1;
            if !matches!(
                row.check().limit,
                LimitId::ForceBudget | LimitId::ToolDeflection | LimitId::VendorDepthTable
            ) {
                continue;
            }
            assert_eq!(
                row.check().value.map(f64::to_bits),
                Some(own.to_bits()),
                "{name}: the {} row holds its own depth",
                cut(row).label()
            );
            if let Some(bound) = row.check().bound {
                let over = own > bound * (1.0 + 1e-9);
                assert_eq!(
                    row.check().state == CheckState::Exceeds,
                    over,
                    "{name}: the {} row: {} mm against {bound} mm",
                    cut(row).label(),
                    own
                );
            }
            if row.check().limit == LimitId::ForceBudget
                && matches!(row.check().state, CheckState::Within | CheckState::Exceeds)
            {
                match cut(row) {
                    CutKind::Entry(EntryKind::Ramp) => ramp_numbers += 1,
                    CutKind::Entry(EntryKind::Helix) => helix_numbers += 1,
                    CutKind::Entry(EntryKind::Plunge | EntryKind::PeckLadder)
                    | CutKind::Body
                    | CutKind::FirstPass(_)
                    | CutKind::SpringPass
                    | CutKind::WallFinish
                    | CutKind::EntryApproach
                    | CutKind::Cleanup(_)
                    | CutKind::CappedStep(_)
                    | CutKind::SlotRing
                    | CutKind::LeadIn
                    | CutKind::LeadOut
                    | CutKind::Link
                    | CutKind::DressupRelief
                    | CutKind::Drill => {}
                }
            }
        }
    }
    assert!(rows > 0, "the configs give rows at an own depth");
    assert!(plunge_power > 0, "a plunge row holds a power number");
    // The worst case of the operator file (wanaka idx 2, `P29-8_DESIGN.md`
    // table `W`): a geodesic 3D Rough, EndMill 6 mm, a 6 mm depth per pass,
    // a ramp entry at the stated speeds. The ramp's root is under the
    // proposal depth: the row `Exceeds`, and the proposal does not move
    // down to it (an entry is no ceiling).
    let tool = fm1_tool(ToolType::EndMill, 6.0);
    let mut rough = add_door_op(OperationType::Adaptive3d, &tool, &machine);
    if let OperationConfig::Adaptive3d(c) = &mut rough {
        c.clearing_strategy = ClearingStrategy::GeodesicAdaptive;
        c.stepover = Some(1.2);
        c.depth_per_pass = Some(6.0);
        c.entry_style = rs_cam_core::compute::operation_configs::Adaptive3dEntryStyle::Ramp;
    }
    // The speeds of wanaka idx 2: 4571 mm/min at 18000 rpm.
    rough.set_spindle_rpm(Some(18_000));
    rough.set_feed_rate(4571.0);
    let _ = rough.as_params_mut().set_ramp_feed_rate(Some(4571.0));
    let env = envelope(&rough, &tool, &machine).expect("the 3D Rough is backed");
    let ap = env.proposal.get().ap_mm().expect("a depth");
    let ramp = env
        .rows()
        .iter()
        .filter(|r| r.is_planned())
        .find(|r| {
            cut(r) == CutKind::Entry(EntryKind::Ramp) && r.check().limit == LimitId::ForceBudget
        })
        .expect("the ramp row");
    let root = ramp.check().bound.expect("the ramp has a root");
    assert_eq!(ramp.check().state, CheckState::Exceeds, "the ramp at 6 mm");
    assert!(
        root < ap,
        "the ramp root {root} mm is under the proposal {ap} mm: the entry sets no ceiling"
    );
    assert!(
        ramp_numbers > 0 && helix_numbers > 0,
        "a ramp ({ramp_numbers}) and a helix ({helix_numbers}) row hold a force-budget number"
    );
}

/// P29-9 task 1: a chip check of a stored entry feed reads the interval
/// that the write step states (`entry_rule::StoredFeedChip`). Suggest
/// writes the ramp feed rounded down to a whole mm/min, so the stated entry
/// chip sits under the band point by less than one step: the band-minimum
/// row is `Within`, and its value is still the stated chip. A ramp feed far
/// under the band stays `Exceeds`, and a ceiling reads the stated chip.
#[test]
fn claim_6_a_stored_entry_feed_is_judged_over_its_write_step_p29_9() {
    use rs_cam_core::feeds::suggest::{
        FEED_WRITE_STEP_MM_MIN, SuggestForOperationInput, suggest_for_operation,
    };
    use rs_cam_core::limits::{BoundSide, CheckState, EntryKind};
    let machine = MachineProfile::default();
    // A Face, EndMill 6 mm, hardwood, at the add door: the band is a point
    // (0.127 mm), and the feed stops at the machine feed ceiling, so the
    // ramp feed is the stored cut feed (one write).
    let tool = fm1_tool(ToolType::EndMill, 6.0);
    let op = add_door_op(OperationType::Face, &tool, &machine);
    // The real Suggest write: the speeds, the feed and the ramp feed as the
    // funnel rounds them. A ramp entry, so the funnel writes a ramp feed.
    let dressups = rs_cam_core::compute::config::DressupConfig {
        entry_style: rs_cam_core::compute::config::DressupEntryStyle::Ramp,
        ..Default::default()
    };
    let suggested = suggest_for_operation(SuggestForOperationInput {
        operation: &op,
        tool: &tool,
        machine: &machine,
        material: &hardwood(),
        lut: rs_cam_core::feeds::embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::default(),
        context: SuggestContext::no_project(NoProject {
            dressups: Some(&dressups),
            ..NoProject::NONE
        }),
    })
    .expect("the FM1 Face is backed");
    let mut op = suggested.operation;
    let ramp = op.ramp_feed_rate().expect("Suggest writes a ramp feed");
    let rpm = f64::from(op.spindle_rpm().expect("Suggest writes the speed"));
    let per_rev = rpm * f64::from(tool.flute_count_planning());
    let entry_rows = |op: &OperationConfig, limit: LimitId| {
        let env = envelope(op, &tool, &machine).expect("the Face is backed");
        planned(&env)
            .into_iter()
            .filter(|r| {
                r.check().limit == limit
                    && matches!(cut(r), CutKind::Entry(EntryKind::Ramp | EntryKind::Helix))
                    && matches!(r.check().state, CheckState::Within | CheckState::Exceeds)
            })
            .collect::<Vec<_>>()
    };
    let rows = entry_rows(&op, LimitId::ChipMin);
    assert!(!rows.is_empty(), "the Face has a ramp or helix ChipMin row");
    for row in &rows {
        let bound = row.check().bound.expect("a band minimum");
        let value = row.check().value.expect("a chip");
        assert_eq!(row.check().side, BoundSide::Floor);
        assert_eq!(
            value.to_bits(),
            (ramp / per_rev).to_bits(),
            "the stated chip"
        );
        // Non-vacuity: the stated chip is under the band, by less than one
        // write step.
        assert!(
            value < bound && bound * per_rev - ramp < FEED_WRITE_STEP_MM_MIN,
            "the stored ramp {ramp} mm/min against the band feed {} mm/min",
            bound * per_rev
        );
        assert_eq!(
            row.check().state,
            CheckState::Within,
            "a stored feed one step under the band is inside"
        );
        // The margin follows the judged end (lead ruling): never below zero
        // on a row that reads `Within`.
        assert!(
            row.check().margin.is_some_and(|m| m >= 0.0),
            "the margin {:?} of a row inside",
            row.check().margin
        );
    }
    // A ramp feed far under the band: the whole interval is outside.
    let _ = op
        .as_params_mut()
        .set_ramp_feed_rate(Some((ramp / 2.0).floor()));
    let low = entry_rows(&op, LimitId::ChipMin);
    assert!(!low.is_empty());
    for row in &low {
        assert_eq!(row.check().state, CheckState::Exceeds, "half the band feed");
    }
    // A ceiling reads the stated chip (the low end): a stored feed at the
    // band maximum, whose interval reaches over it, is inside.
    let max = entry_rows(&op, LimitId::ChipMax)
        .first()
        .and_then(|r| r.check().bound)
        .expect("a band maximum");
    // The entry feed is the ramp feed at or below the cut feed: set both.
    let at = (max * per_rev).floor();
    op.set_feed_rate(at);
    let _ = op.as_params_mut().set_ramp_feed_rate(Some(at));
    let rows = entry_rows(&op, LimitId::ChipMax);
    assert!(!rows.is_empty());
    for row in &rows {
        assert!(row.check().value.expect("a chip") <= max);
        assert!((at + FEED_WRITE_STEP_MM_MIN) / per_rev > max, "non-vacuity");
        assert_eq!(row.check().state, CheckState::Within, "at the band maximum");
    }
}

/// P29-9 9b: the envelope of a Project Curve at `depth` from `direction`,
/// with the setup fact `flipped`.
fn project_curve_rows(
    depth: f64,
    direction: rs_cam_core::compute::operation_configs::ProjectCurveDirection,
    flipped: Option<bool>,
) -> Vec<CheckRow> {
    project_curve_rows_of(
        depth,
        direction,
        flipped,
        LimitId::ForceBudget,
        &fm1_tool(ToolType::EndMill, 6.0),
    )
}

/// The planned body rows of `limit` of a Project Curve at `depth`.
fn project_curve_rows_of(
    depth: f64,
    direction: rs_cam_core::compute::operation_configs::ProjectCurveDirection,
    flipped: Option<bool>,
    limit: LimitId,
    tool: &ToolConfig,
) -> Vec<CheckRow> {
    let machine = MachineProfile::default();
    let mut op = rs_cam_core::compute::fixture_values::fixture_op(OperationType::ProjectCurve);
    if let OperationConfig::ProjectCurve(c) = &mut op {
        c.depth = Some(depth);
        c.direction = direction;
    }
    SafeEnvelope::for_cut(EnvelopeInput {
        operation: &op,
        tool,
        machine: &machine,
        material: &hardwood(),
        lut: rs_cam_core::feeds::embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::default(),
        context: SuggestContext::no_project(NoProject {
            setup_z_flipped: flipped,
            ..NoProject::NONE
        }),
        modulation: rs_cam_core::dressup::feed_modulation::RunModulation::Off,
    })
    .expect("the Project Curve envelope builds")
    .rows()
    .iter()
    .filter(|r| r.is_planned() && cut(r) == CutKind::Body && r.check().limit == limit)
    .cloned()
    .collect()
}

/// Review 2026-11-15 F3: a row that exceeds at a lower bound exceeds for
/// certain (the real cut is at least as deep), so `Caveat::LowerBound`
/// softens nothing: a Project Curve whose lower-bound depth is over the
/// deflection root is `Exceeds` with the gating tone.
#[test]
fn claim_7b_an_exceeding_lower_bound_keeps_the_gating_tone_review_f3() {
    use rs_cam_core::compute::operation_configs::ProjectCurveDirection::FromAbove;
    use rs_cam_core::limits::{Caveat, CheckState, NoNumberReason, Tone};
    let one = |depth: f64| {
        // A 2 mm end mill with a 50 mm stickout: its deflection root lies
        // in the searched span.
        let mut tool = fm1_tool(ToolType::EndMill, 2.0);
        tool.set_stickout(ToolFact::Stated(50.0));
        let rows = project_curve_rows_of(depth, FromAbove, None, LimitId::ToolDeflection, &tool);
        assert_eq!(rows.len(), 1, "one body deflection row: {rows:?}");
        rows.into_iter().next().expect("the row")
    };
    // 0.2 mm into the material: inside the root, so the bound proves
    // nothing.
    let shallow = one(0.2);
    let root = shallow.check().bound.expect("a deflection root");
    assert!(root > 0.2, "non-vacuity: the root {root} is over 0.2 mm");
    assert_eq!(
        shallow.check().state,
        CheckState::NoNumber(NoNumberReason::OnlyALowerBound)
    );
    // Over the root: certain.
    let deep = ((root + 0.1) * 10.0).ceil() / 10.0;
    let row = one(deep);
    assert_eq!(row.check().value, Some(deep));
    assert_eq!(
        row.check().state,
        CheckState::Exceeds,
        "{deep} mm over {root}"
    );
    assert!(
        row.caveats().contains(&Caveat::LowerBound),
        "the row says the value is a lower bound: {}",
        row.face()
    );
    assert_eq!(row.tone(), Tone::Exceeds, "{}", row.face());
}

#[test]
fn claim_7_a_project_curve_is_judged_at_the_lower_bound_of_its_depth_p29_9() {
    use rs_cam_core::compute::operation_configs::ProjectCurveDirection::{FromAbove, FromBelow};
    use rs_cam_core::limits::{CheckState, NoNumberReason};
    let one = |depth, direction, flipped| {
        let rows = project_curve_rows(depth, direction, flipped);
        assert!(rows.len() <= 1, "one body force-budget row: {rows:?}");
        rows.into_iter().next()
    };
    // From above, 1 mm into the material: inside the ceiling, so the
    // bound proves nothing.
    let row = one(1.0, FromAbove, None).expect("a body row at the bound");
    assert_eq!(row.check().value, Some(1.0));
    let ceiling = row.check().bound.expect("a force-budget ceiling");
    assert!(
        ceiling > 1.0,
        "non-vacuity: the ceiling {ceiling} is over 1 mm"
    );
    assert_eq!(
        row.check().state,
        CheckState::NoNumber(NoNumberReason::OnlyALowerBound)
    );
    assert_eq!(
        row.check().margin,
        None,
        "no margin at a lower bound inside"
    );
    // Over the ceiling: the real cut is at least as deep, so it exceeds.
    let deep = (ceiling + 1.0).ceil();
    let row = one(deep, FromAbove, None).expect("a body row at the deep bound");
    assert_eq!(row.check().value, Some(deep));
    assert_eq!(
        row.check().state,
        CheckState::Exceeds,
        "{deep} mm over {ceiling}"
    );
    // A tip above the surface: no depth row.
    assert!(
        one(-1.0, FromAbove, None).is_none(),
        "a tip above the surface"
    );
    // From below, the sign reads the setup.
    assert!(one(-1.0, FromBelow, None).is_none(), "no setup, no bound");
    assert_eq!(
        one(-1.0, FromBelow, Some(false)).and_then(|r| r.check().value),
        Some(1.0),
        "a setup that is not flipped judges -depth"
    );
    assert!(
        one(-1.0, FromBelow, Some(true)).is_none(),
        "a flipped setup judges depth: -1 is outside"
    );
    assert_eq!(
        one(1.0, FromBelow, Some(true)).and_then(|r| r.check().value),
        Some(1.0),
        "a flipped setup judges depth"
    );
}

#[test]
fn claim_8_a_finish_is_judged_at_the_earlier_leave_p29_9() {
    use rs_cam_core::limits::{CheckState, NoNumberReason};
    let machine = MachineProfile::default();
    let tool = fm1_tool(ToolType::BallNose, 6.0);
    let op = rs_cam_core::compute::fixture_values::fixture_op(OperationType::Scallop);
    let rows = |leave: Option<f64>| {
        SafeEnvelope::for_cut(EnvelopeInput {
            operation: &op,
            tool: &tool,
            machine: &machine,
            material: &hardwood(),
            lut: rs_cam_core::feeds::embedded_vendor_lut(),
            spindle_strategy: SpindleStrategy::default(),
            context: SuggestContext::no_project(NoProject {
                earlier_leave_mm: leave,
                ..NoProject::NONE
            }),
            modulation: rs_cam_core::dressup::feed_modulation::RunModulation::Off,
        })
        .expect("the Scallop envelope builds")
        .rows()
        .iter()
        .filter(|r| r.is_planned())
        .cloned()
        .collect::<Vec<_>>()
    };
    let body = |rows: &[CheckRow], limit: LimitId| {
        rows.iter()
            .find(|r| cut(r) == CutKind::Body && r.check().limit == limit)
            .map(|r| r.check().clone())
            .unwrap_or_else(|| panic_row(limit))
    };
    let with = rows(Some(0.5));
    let without = rows(None);
    let power = body(&with, LimitId::SpindlePower);
    assert!(
        power.value.is_some_and(|kw| kw > 0.0),
        "the power at the leave: {power:?}"
    );
    // The power reads the leave: a deeper leave gives more power.
    let deeper = body(&rows(Some(1.0)), LimitId::SpindlePower);
    assert!(
        deeper.value > power.value,
        "the power at 1.0 mm {:?} is over the power at 0.5 mm {:?}",
        deeper.value,
        power.value
    );
    assert_eq!(
        body(&without, LimitId::SpindlePower).state,
        CheckState::NoNumber(NoNumberReason::NoEarlierLeave)
    );
    for rows in [&with, &without] {
        assert_eq!(
            body(rows, LimitId::ForceBudget).state,
            CheckState::NoNumber(NoNumberReason::NoBound)
        );
    }
    // P29-10 10h (lead ruling L3): the deflection of a fixed depth axis is a
    // value check at the fixed depth. The finish body reads the earlier
    // leave, with a basis and the caveat; with no leave it has no number
    // for that reason, never "not judged here".
    for leave in [0.5, 1.0] {
        let rows = rows(Some(leave));
        let row = rows
            .iter()
            .find(|r| cut(r) == CutKind::Body && r.check().limit == LimitId::ToolDeflection)
            .expect("the body deflection row");
        assert_eq!(
            row.check().value,
            Some(leave),
            "the deflection at the leave"
        );
        assert!(
            matches!(row.check().state, CheckState::Within | CheckState::Exceeds),
            "a judged state at the leave: {:?}",
            row.check().state
        );
        assert!(row.check().basis.is_some(), "the basis of the beam");
        assert!(
            row.caveats()
                .contains(&rs_cam_core::limits::Caveat::AtEarlierLeave),
            "the caveat of the leave: {}",
            row.face()
        );
    }
    assert_eq!(
        body(&without, LimitId::ToolDeflection).state,
        CheckState::NoNumber(NoNumberReason::NoEarlierLeave)
    );
    // No deflection row of any op says "not judged here" for a fixed axis.
    let machine = MachineProfile::default();
    let mut fixed = 0;
    for (name, op, tool) in configs(&machine) {
        let Some(env) = envelope(&op, &tool, &machine) else {
            continue;
        };
        if !matches!(
            env.axes,
            rs_cam_core::feeds::envelope::EnvelopeAxes::RadialBar { .. }
                | rs_cam_core::feeds::envelope::EnvelopeAxes::Fixed { .. }
        ) {
            continue;
        }
        for row in planned(&env)
            .iter()
            .filter(|r| r.check().limit == LimitId::ToolDeflection)
        {
            fixed += 1;
            assert_ne!(
                row.check().state,
                CheckState::NoNumber(NoNumberReason::NotJudgedHere),
                "{name}: {}",
                row.face()
            );
        }
    }
    assert!(
        fixed > 0,
        "non-vacuity: deflection rows on a fixed depth axis"
    );
}

#[expect(
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]
fn panic_row(limit: LimitId) -> rs_cam_core::limits::Check {
    panic!("no body row of {limit:?}")
}

/// P29-8e (c), `P29-8e_TAB_REPLUNGE.md` section 13: `LimitId::PlungeRate`
/// judges the vertical rate of each declared cut against
/// `VerticalRateLimit::bound` at the stated speed, as a warning row. A plunge
/// entry over every vertical bound reads `Exceeds` with the stated plunge as
/// its value and a bound under it; a plunge of 1 mm/min reads `Within`; a cut
/// with no vertical feed reads `NotApplicable(NoVerticalFeed)`. The check
/// moves no feed: the stated plunge of the config is the value.
#[test]
fn claim_9_the_vertical_rate_judges_every_plunge_p29_8e() {
    use rs_cam_core::limits::{CheckState, EntryKind, Geometry};
    let machine = MachineProfile::default();
    let (mut exceeds, mut within, mut no_vertical) = (0, 0, 0);
    for (name, mut op, tool) in configs(&machine) {
        for (plunge, high) in [(1.0e6, true), (1.0, false)] {
            op.as_params_mut().set_plunge_rate(plunge);
            let Some(env) = envelope(&op, &tool, &machine) else {
                continue;
            };
            for row in planned(&env)
                .iter()
                .filter(|r| r.check().limit == LimitId::PlungeRate)
            {
                assert_eq!(row.check().unit, "mm/min", "{name}");
                match cut(row) {
                    CutKind::Entry(EntryKind::Plunge) => match row.check().state {
                        CheckState::Exceeds => {
                            assert!(high, "{name}: a 1 mm/min plunge exceeds");
                            assert_eq!(row.check().value, Some(plunge), "{name}");
                            assert!(
                                row.check().bound.is_some_and(|b| b < plunge),
                                "{name}: {:?}",
                                row.check().bound
                            );
                            exceeds += 1;
                        }
                        CheckState::Within => {
                            assert_eq!(row.check().value, Some(plunge), "{name}");
                            if !high {
                                // Q11: the true limit, not the clamped value,
                                // with the basis of its arm.
                                if let Some(b) = row.check().bound {
                                    assert!(b > plunge, "{name}: the bound {b} is the limit");
                                    assert!(row.check().basis.is_some(), "{name}: the arm's basis");
                                }
                                within += 1;
                            }
                        }
                        CheckState::NoNumber(_) | CheckState::NotApplicable(_) => {}
                    },
                    CutKind::Body => {
                        assert_eq!(
                            row.check().state,
                            CheckState::NotApplicable(Geometry::NoVerticalFeed),
                            "{name}: a body cut has no vertical feed"
                        );
                        no_vertical += 1;
                    }
                    CutKind::FirstPass(_)
                    | CutKind::SpringPass
                    | CutKind::WallFinish
                    | CutKind::Entry(_)
                    | CutKind::EntryApproach
                    | CutKind::Cleanup(_)
                    | CutKind::CappedStep(_)
                    | CutKind::SlotRing
                    | CutKind::LeadIn
                    | CutKind::LeadOut
                    | CutKind::Link
                    | CutKind::DressupRelief
                    | CutKind::Drill => {}
                }
            }
        }
    }
    assert!(exceeds > 0, "non-vacuity: a plunge over the bound exceeds");
    assert!(within > 0, "non-vacuity: a slow plunge is within");
    assert!(no_vertical > 0, "non-vacuity: a body row");
}

/// P29-10 (10b): a cut that the plan may not cut is no note. Its rows carry
/// the caveat `MayOccur` with its row, and the row names an exemption; no
/// envelope note names the cut.
#[test]
fn a_may_occur_cut_is_a_row_not_a_note_p29_10() {
    let machine = MachineProfile::default();
    let mut may_rows = 0;
    for (name, op, tool) in configs(&machine) {
        let Some(env) = envelope(&op, &tool, &machine) else {
            continue;
        };
        for note in &env.notes {
            assert!(
                !note.text().contains("the plan may not cut it"),
                "{name}: a may-occur note: {}",
                note.text()
            );
        }
        for row in planned(&env) {
            let Some(id) = row.may_occur_row() else {
                continue;
            };
            assert!(
                row.caveats()
                    .contains(&rs_cam_core::limits::Caveat::MayOccur { row: id }),
                "{name}: the caveat"
            );
            assert!(row.exemption().is_some(), "{name}: the exemption row");
            may_rows += 1;
        }
    }
    assert!(may_rows > 0, "non-vacuity: no may-occur row");
}
