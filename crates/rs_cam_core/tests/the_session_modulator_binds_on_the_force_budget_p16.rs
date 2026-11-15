//! Feeds contract P16 item 7 (D6): the force-budget limit reaches the
//! modulator through the session.
//!
//! `the_modulator_reads_the_depth_gate_force_budget_rule_p16` pins the rule
//! and the solver on a synthetic context. This file holds the session
//! wiring: `ProjectSession` builds the modulator's force-budget inputs from
//! `RigidityProfile::force_budget_of` (`session::compute`,
//! `modulate_annotated_against_trace`), for the toolpath material and for
//! each own material.
//!
//! # P32 (operator ruling 2026-11-15, "Warn only")
//!
//! The force budget never sets a feed while it is advisory. The session
//! builds the scope through `ForceBudgetScope::of`, which reads the one
//! predicate of the advisory rows (`Basis::force_budget_bound().gates_export()`).
//! While that is `false` the scope is `Advisory` and the modulator has no
//! force-budget arm. So this file now holds the opposite claim of the one
//! it was written for: the session modulator does NOT bind on the force
//! budget. The file keeps its name (the history of the claim).
//!
//! # The cells
//!
//! 1. A deep, wide Pocket roughing slot in generic hardwood on the default
//!    (hobby) machine. The rule is the force budget and the material has a
//!    force line. Before P32 the budget bound moves of this cell (review
//!    cell P_thin_005: F2615 to F1099). After a session simulation the
//!    modulation summary of the op must not name `ForceBudget`, in the full
//!    mode and in cap only.
//! 2. The same cut in Baltic birch plywood, which has no force line. The
//!    summary must not name `ForceBudget`. It carries no note: the note
//!    "force budget: no force line" explains a missing limit, and no limit
//!    is expected while the budget is advisory.
//!
//! # Red on a defect
//!
//! With `Basis::force_budget_bound` changed to a basis that gates, cell 1
//! reads a `ForceBudget` binding again (the pre-P32 behaviour).

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::session::{polygon_model, single_op_session, square_polygon, stock_under};
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::dressup::feed_modulation::{BindingConstraint, FORCE_BUDGET_NO_FORCE_LINE_NOTE};
use rs_cam_core::machine::{DepthCapRule, MachineProfile};
use rs_cam_core::material::{Material, PlywoodGrade, WoodSpecies};
use rs_cam_core::session::{
    Command, FeedModulationSwitch, ProjectSession, SetFeedModulationArgs, SimulationOptions,
    SimulationResolution,
};
use rs_cam_core::tool_load::ModulationSummary;

/// The review cell P_thin_005: a 6 mm two-flute flat end mill.
const D_MM: f64 = 6.0;
const FLUTES: u32 = 2;
const CUTTING_LENGTH_MM: f64 = 20.0;
const STICKOUT_MM: f64 = 28.0;
/// The cut: one 6 mm pass at a 5 mm stepover (the first ring is a slot).
const DEPTH_MM: f64 = 6.0;
const STEPOVER_MM: f64 = 5.0;
/// The stated speeds of the cell.
const FEED_MM_MIN: f64 = 5999.0;
const PLUNGE_MM_MIN: f64 = 2999.0;
const RPM: u32 = 10_296;
/// The pocket outline (half of a 40 mm square) and the stock thickness.
const HALF_MM: f64 = 20.0;
const STOCK_Z_MM: f64 = 12.0;
/// The simulation cell of the review run.
const RESOLUTION_MM: f64 = 0.5;

fn tool() -> ToolConfig {
    let mut t = ToolConfig::test_default(ToolId::from_raw(1), ToolType::EndMill);
    t.diameter = D_MM;
    t.set_flute_count(ToolFact::Stated(FLUTES));
    t.set_cutting_length(ToolFact::Stated(CUTTING_LENGTH_MM));
    t.set_shank_diameter(ToolFact::Stated(D_MM));
    t.shaft_diameter = D_MM;
    t.set_stickout(ToolFact::Stated(STICKOUT_MM));
    t
}

fn pocket() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        stepover: Some(STEPOVER_MM),
        floor: rs_cam_core::compute::floor::Floor::top(DEPTH_MM),
        depth_per_pass: Some(DEPTH_MM),
        feed_rate: Some(FEED_MM_MIN),
        plunge_rate: Some(PLUNGE_MM_MIN),
        spindle_rpm: Some(RPM),
        ..PocketConfig::default()
    })
}

/// Build, generate and simulate the cell in `material` under `switch`;
/// return the modulation summary of the op.
fn simulated_summary(material: Material, switch: FeedModulationSwitch) -> ModulationSummary {
    let stock = StockConfig {
        material,
        ..stock_under(HALF_MM, STOCK_Z_MM)
    };
    let mut session: ProjectSession = single_op_session(
        stock,
        tool(),
        polygon_model(vec![square_polygon(HALF_MM)], "p16_square"),
        "Pocket",
        pocket(),
    );
    let _ = session
        .apply(Command::SetFeedModulation(SetFeedModulationArgs { switch }))
        .expect("the switch applies");
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("the pocket generates");
    session
        .run_simulation(
            &SimulationOptions {
                resolution: SimulationResolution::Fixed(RESOLUTION_MM),
                ..SimulationOptions::default()
            },
            &cancel,
        )
        .expect("the pocket simulates");
    let report = session.tool_load_report();
    report
        .per_toolpath
        .first()
        .and_then(|v| v.modulation_summary.clone())
        .expect("the simulation modulates the pocket and records its summary")
}

#[test]
fn the_session_modulator_binds_on_the_force_budget_p16() {
    // The rule of the cell is the force budget on the session's machine.
    let rule = MachineProfile::default()
        .rigidity
        .force_budget_of({
            let op =
                rs_cam_core::compute::catalog::OperationConfig::new_default(OperationType::Pocket);
            op.cuts(&rs_cam_core::feeds::entry_rule::ConfiguredEntry::registry_default(&op))
                .body_facts()
        })
        .ok();
    assert!(
        matches!(rule, Some(DepthCapRule { .. })),
        "Pocket roughing on the default machine must carry the force-budget rule: {rule:?}"
    );

    // The one predicate: the basis of the budget does not gate an export.
    assert!(
        !rs_cam_core::feeds::sourced::Basis::force_budget_bound().gates_export(),
        "this file states the advisory budget; a gating budget caps again (the other p16 file)"
    );

    for switch in [FeedModulationSwitch::Full, FeedModulationSwitch::CapOnly] {
        // Cell 1: a force line, and still no force-budget limit.
        let summary = simulated_summary(
            Material::SolidWood {
                species: WoodSpecies::GenericHardwood,
            },
            switch,
        );
        assert!(
            !summary
                .binding_constraint_distribution
                .contains_key(&BindingConstraint::ForceBudget),
            "{switch:?}: the advisory force budget sets no feed: {:?}",
            summary.binding_constraint_distribution
        );
        assert!(
            summary
                .notes
                .iter()
                .all(|n| n != FORCE_BUDGET_NO_FORCE_LINE_NOTE),
            "{switch:?}: a material with a force line has no force-budget note: {:?}",
            summary.notes
        );

        // Cell 2: no force line, no limit, and no note while advisory.
        let summary = simulated_summary(
            Material::Plywood {
                grade: PlywoodGrade::BalticBirch,
            },
            switch,
        );
        assert!(
            !summary
                .binding_constraint_distribution
                .contains_key(&BindingConstraint::ForceBudget),
            "{switch:?}: no force line, no force-budget limit: {:?}",
            summary.binding_constraint_distribution
        );
        assert!(
            summary
                .notes
                .iter()
                .all(|n| n != FORCE_BUDGET_NO_FORCE_LINE_NOTE),
            "{switch:?}: the note explains a missing limit; none is expected: {:?}",
            summary.notes
        );
    }
}
