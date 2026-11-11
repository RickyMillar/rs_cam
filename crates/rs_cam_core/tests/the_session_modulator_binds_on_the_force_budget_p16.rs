//! Feeds contract P16 item 7 (D6): the force-budget limit reaches the
//! modulator through the session.
//!
//! `the_modulator_reads_the_depth_gate_force_budget_rule_p16` pins the rule
//! and the solver on a synthetic context. This file holds the session
//! wiring: `ProjectSession` builds the modulator's force-budget inputs from
//! `RigidityProfile::depth_cap_rule` (`session::compute`,
//! `modulate_annotated_against_trace`), for the toolpath material and for
//! each own material.
//!
//! # The cells
//!
//! 1. A deep, wide Pocket roughing slot in generic hardwood on the default
//!    (hobby) machine. The rule is the force budget, the material has a force
//!    line, and the cut is deep and wide enough that the budget feed is
//!    below the chipload cap and the machine ceiling. The cell is the review
//!    cell P_thin_005 (feeds review 2026-11-06), where the emitted feed fell
//!    from F2615 to F1099 when the limit landed. After a session simulation
//!    the modulation summary of the op must name `ForceBudget`.
//! 2. The same cut in Baltic birch plywood, which has no force line. The
//!    summary must not name `ForceBudget`, and it must carry the note
//!    "force budget: no force line".
//!
//! # Red before the fix
//!
//! With `force_budget_inputs: None` in the session context, cell 1 reads no
//! `ForceBudget` binding.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::session::{polygon_model, single_op_session, square_polygon, stock_under};
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::dressup::feed_modulation::{BindingConstraint, FORCE_BUDGET_NO_FORCE_LINE_NOTE};
use rs_cam_core::feeds::{OperationFamily, PassRole};
use rs_cam_core::machine::{DepthCapRule, MachineProfile};
use rs_cam_core::material::{Material, PlywoodGrade, WoodSpecies};
use rs_cam_core::session::{ProjectSession, SimulationOptions, SimulationResolution};
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

/// Build, generate and simulate the cell in `material`; return the
/// modulation summary of the op.
fn simulated_summary(material: Material) -> ModulationSummary {
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
        .expect("the default simulation modulates the pocket and records its summary")
}

#[test]
fn the_session_modulator_binds_on_the_force_budget_p16() {
    // The rule of the cell is the force budget on the session's machine.
    let rule = MachineProfile::default()
        .rigidity
        .depth_cap_rule(OperationFamily::Pocket, PassRole::Roughing);
    assert!(
        matches!(rule, Some(DepthCapRule { .. })),
        "Pocket roughing on the default machine must carry the force-budget rule: {rule:?}"
    );

    // Cell 1: a force line, the limit binds.
    let summary = simulated_summary(Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    });
    let share = summary
        .binding_constraint_distribution
        .get(&BindingConstraint::ForceBudget)
        .copied()
        .unwrap_or(0.0);
    assert!(
        share > 0.0,
        "the force budget must bind some moves of the deep slot through the session: {:?}",
        summary.binding_constraint_distribution
    );
    assert!(
        summary.notes.is_empty(),
        "a material with a force line has no force-budget note: {:?}",
        summary.notes
    );

    // Cell 2: no force line, no limit, and the note.
    let summary = simulated_summary(Material::Plywood {
        grade: PlywoodGrade::BalticBirch,
    });
    assert!(
        !summary
            .binding_constraint_distribution
            .contains_key(&BindingConstraint::ForceBudget),
        "no force line, no force-budget limit: {:?}",
        summary.binding_constraint_distribution
    );
    assert_eq!(
        summary.notes,
        vec![FORCE_BUDGET_NO_FORCE_LINE_NOTE.to_owned()],
        "the summary states the missing force line"
    );
}
