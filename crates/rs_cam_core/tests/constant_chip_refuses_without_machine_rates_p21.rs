//! Feeds contract P21c (`planning/feeds_contract_2026-11-03/phases/P21c.md`,
//! `P21_DESIGN.md` section 4): `SpindleStrategy::ConstantChip` gives no RPM
//! when the machine states no axis rates or no axis accelerations, or when
//! the RPM ceiling is empty and the chart row prints no RPM. The refusal is
//! `FeedsError::ConstantChipUnstated` with the named gap, at the one
//! Suggest door (`feeds::validate_tool_for_operation`).
//!
//! The refusal is a Suggest refusal only. The op keeps its stored feed and
//! RPM, and it still generates and exports (the operator rule: every
//! finding warns, no job is refused).
//!
//! The arms:
//!
//! - (a) a preset (no kinematics) refuses with `AxisRates`;
//! - (b) a machine with axis rates and only the scalar acceleration refuses
//!   with `Accelerations` (ruling Q3);
//! - (c) a machine with both and an empty ceiling refuses with
//!   `RowPrintsNoRpm` on a row that prints no RPM, and serves a row that
//!   prints one;
//! - (d) `MatchChart` refuses on none of them;
//! - (e) a session on a preset under `ConstantChip`: `ApplyFeeds` refuses
//!   and writes nothing, and the op generates and exports;
//! - (f) the add door reads the project policy (lead ruling 2026-11-10): on
//!   a preset it adds the op with no feeds and gives the refusal text; on a
//!   machine with stated limits the new op holds the RPM of the card.
//!
//! Red: return `None` from `feeds::constant_chip_gap` when the machine
//! states no axis rates.

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::fixture_values::fixture_op;
use rs_cam_core::feeds::suggest::{
    NoProject, SuggestContext, SuggestForOperationInput, SuggestedParams, suggest_for_operation,
};
use rs_cam_core::feeds::{
    ConstantChipGap, FeedsError, SpindleStrategy, SpindleStrategyKind, embedded_vendor_lut,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::machine::kinematics::MachineKinematics;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::session::{
    ApplyFeedsArgs, Command, DraftRequest, ProjectSession, SessionError, SetMachineArgs,
    SetPostConfigArgs,
};

mod common;
use common::session::{generate, polygon_model, single_op_session, square_polygon, stock_under};
use common::tools::endmill_tool_config;

/// Cutter diameter (mm).
const TOOL_D: f64 = 6.0;
/// Half-extent (mm) of the pocket square, and the stock height (mm).
const HALF: f64 = 20.0;
const STOCK_Z: f64 = 6.0;
/// An operator ceiling: the spindle maximum of the presets.
const CEILING: u32 = 24_000;

fn white_oak() -> Material {
    Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    }
}

fn constant_chip(rpm_ceiling: Option<u32>) -> SpindleStrategy {
    SpindleStrategy::ConstantChip { rpm_ceiling }
}

/// The preset with each axis rate at its travel rate and no per-axis
/// acceleration (the scalar only).
fn rates_only() -> MachineProfile {
    let mut machine = MachineProfile::shapeoko_vfd();
    let rate = machine.max_feed_mm_min;
    machine.kinematics = Some(MachineKinematics {
        max_rate_xyz_mm_min: Some([rate; 3]),
        ..MachineKinematics::default()
    });
    machine
}

/// The preset with its axis rates and accelerations stated: the travel
/// rate and the scalar acceleration of the kinematics default on each axis.
fn rates_and_accelerations() -> MachineProfile {
    let machine = MachineProfile::shapeoko_vfd();
    let rate = machine.max_feed_mm_min;
    let accel = machine.effective_kinematics().acceleration_mm_s2;
    machine.with_axis_limits([rate; 3], [accel; 3])
}

fn suggest(
    op: &OperationConfig,
    machine: &MachineProfile,
    strategy: SpindleStrategy,
) -> Result<SuggestedParams, FeedsError> {
    suggest_for_operation(SuggestForOperationInput {
        operation: op,
        tool: &endmill_tool_config(TOOL_D),
        machine,
        material: &white_oak(),
        lut: embedded_vendor_lut(),
        spindle_strategy: strategy,
        context: SuggestContext::no_project(NoProject::NONE),
    })
}

fn gap(result: Result<SuggestedParams, FeedsError>) -> ConstantChipGap {
    match result {
        Err(FeedsError::ConstantChipUnstated { missing }) => missing,
        Err(other) => panic!("another refusal: {other}"),
        Ok(s) => panic!("Suggest served RPM {}", s.feeds_result.rpm),
    }
}

/// (a), (b), (d): the machine gaps.
#[test]
fn a_machine_that_states_no_rates_refuses_with_the_named_gap_p21() {
    let op = fixture_op(OperationType::Pocket);
    let preset = MachineProfile::shapeoko_vfd();
    assert!(
        preset.kinematics.is_none(),
        "the preset states no kinematics"
    );

    // (d) MatchChart serves each machine.
    for machine in [&preset, &rates_only(), &rates_and_accelerations()] {
        let served = suggest(&op, machine, SpindleStrategy::MatchChart)
            .unwrap_or_else(|e| panic!("MatchChart serves {}: {e}", machine.name));
        assert!(served.feeds_result.rpm > 0.0);
    }

    // (a) A preset, with and without a ceiling.
    for ceiling in [Some(CEILING), None] {
        let refusal = suggest(&op, &preset, constant_chip(ceiling));
        let text = refusal
            .as_ref()
            .err()
            .map(ToString::to_string)
            .unwrap_or_default();
        assert_eq!(gap(refusal), ConstantChipGap::AxisRates);
        assert!(
            text.contains("axis rates") && text.contains("$110"),
            "the refusal names the gap: {text}"
        );
    }

    // (b) Rates and the scalar acceleration only.
    let refusal = suggest(&op, &rates_only(), constant_chip(Some(CEILING)));
    let text = refusal
        .as_ref()
        .err()
        .map(ToString::to_string)
        .unwrap_or_default();
    assert_eq!(gap(refusal), ConstantChipGap::Accelerations);
    assert!(
        text.contains("accelerations") && text.contains("$120"),
        "the refusal names the gap: {text}"
    );

    // Both stated, with a ceiling: served.
    let served = suggest(
        &op,
        &rates_and_accelerations(),
        constant_chip(Some(CEILING)),
    )
    .unwrap_or_else(|e| panic!("a machine with stated limits is served: {e}"));
    assert!(served.feeds_result.rpm_choice.is_some());
}

/// (c) An empty ceiling refuses a row that prints no RPM and serves a row
/// that prints one.
#[test]
fn an_empty_ceiling_refuses_a_row_that_prints_no_rpm_p21() {
    let machine = rates_and_accelerations();
    let ops: Vec<OperationConfig> = [
        OperationType::Profile,
        OperationType::Pocket,
        OperationType::Adaptive,
        OperationType::Trace,
    ]
    .into_iter()
    .map(fixture_op)
    .collect();
    let (mut refused, mut served) = (0, 0);
    for op in &ops {
        let chart = suggest(op, &machine, SpindleStrategy::MatchChart)
            .unwrap_or_else(|e| panic!("MatchChart serves {:?}: {e}", op.op_type()));
        let prints_rpm = chart
            .feeds_result
            .matched_lut_row
            .as_ref()
            .is_some_and(|row| row.rpm_nominal.is_some());
        let result = suggest(op, &machine, constant_chip(None));
        if prints_rpm {
            let s = result.unwrap_or_else(|e| panic!("{:?} is served: {e}", op.op_type()));
            assert_eq!(s.feeds_result.rpm, chart.feeds_result.rpm);
            served += 1;
        } else {
            let text = result
                .as_ref()
                .err()
                .map(ToString::to_string)
                .unwrap_or_default();
            assert_eq!(gap(result), ConstantChipGap::RowPrintsNoRpm);
            assert!(
                text.contains("prints no RPM") && text.contains("RPM ceiling"),
                "the refusal names the gap: {text}"
            );
            // The ceiling gives the row an RPM basis.
            let with_ceiling = suggest(op, &machine, constant_chip(Some(CEILING)))
                .unwrap_or_else(|e| panic!("{:?} with a ceiling: {e}", op.op_type()));
            assert!(with_ceiling.feeds_result.rpm_choice.is_some());
            refused += 1;
        }
    }
    assert!(
        refused > 0 && served > 0,
        "non-vacuity: the ops hold a row with an RPM and a row with none \
         (refused {refused}, served {served})"
    );
}

fn preset_session_under_constant_chip() -> ProjectSession {
    let mut session = single_op_session(
        stock_under(HALF, STOCK_Z),
        endmill_tool_config(TOOL_D),
        polygon_model(vec![square_polygon(HALF)], "square"),
        "Pocket",
        fixture_op(OperationType::Pocket),
    );
    assert!(
        !session.machine().states_axis_rates(),
        "the session machine states no axis rates"
    );
    let mut post = session.post_config().clone();
    post.spindle_strategy = SpindleStrategyKind::ConstantChip;
    post.spindle_rpm_ceiling = Some(CEILING);
    let _ = session
        .apply(Command::SetPostConfig(SetPostConfigArgs {
            post: Box::new(post),
        }))
        .expect("the post-config row takes the policy");
    assert_eq!(
        session.post_config().spindle_policy(),
        constant_chip(Some(CEILING))
    );
    session
}

/// (e) The refusal is a Suggest refusal: the apply writes nothing, and the
/// op generates and exports.
#[test]
fn a_refused_op_still_generates_and_exports_p21() {
    let mut session = preset_session_under_constant_chip();
    let before = session.toolpath_configs()[0].operation.clone();
    assert!(
        before.feed_rate().is_some() && before.spindle_rpm().is_some(),
        "the fixture states a feed and an RPM"
    );

    let refusal = session.apply(Command::ApplyFeeds(ApplyFeedsArgs {
        index: 0,
        pick: None,
        explored: None,
    }));
    match refusal {
        Err(SessionError::FeedsRefused(FeedsError::ConstantChipUnstated { missing })) => {
            assert_eq!(missing, ConstantChipGap::AxisRates);
        }
        Err(other) => panic!("another refusal: {other}"),
        Ok(_) => panic!("the apply wrote a recipe on a machine with no axis rates"),
    }
    let after = &session.toolpath_configs()[0].operation;
    assert_eq!(after.feed_rate(), before.feed_rate(), "the feed holds");
    assert_eq!(after.spindle_rpm(), before.spindle_rpm(), "the RPM holds");

    // The card door reports the same refusal.
    let tc = session.toolpath_configs()[0].clone();
    let profile = session.cutter_op_profile(&tc).expect("the op has a tool");
    assert!(
        matches!(
            profile.feasibility,
            Err(FeedsError::ConstantChipUnstated {
                missing: ConstantChipGap::AxisRates
            })
        ),
        "the profile: {:?}",
        profile.feasibility
    );

    // Generation and export run with the stored values.
    generate(&mut session, 0);
    assert!(
        !session
            .result_at(0)
            .expect("the op has a result")
            .toolpath()
            .moves
            .is_empty(),
        "the refused op generates motion"
    );
    let gcode = session
        .export_gcode_checked(None)
        .expect("the export runs")
        .gcode;
    let rpm = before.spindle_rpm().expect("stated above");
    assert!(
        gcode.contains(&format!("S{rpm}")),
        "the export commands the stored RPM {rpm}"
    );
}

/// (f) The add door reads the project policy.
#[test]
fn the_add_door_reads_the_project_policy_p21() {
    // On a preset: the op is added, with no feeds and the refusal text.
    let session = preset_session_under_constant_chip();
    let model = session.models()[0].id;
    let draft = session
        .draft_toolpath(DraftRequest::new(OperationType::Pocket, 0, "Pocket 2").model(model))
        .expect("a Suggest refusal refuses no op");
    let text = draft.feeds_refusal.expect("the draft names the refusal");
    assert!(text.contains("axis rates"), "{text}");
    assert_eq!(draft.config.operation.feed_rate(), None);
    assert_eq!(draft.config.operation.spindle_rpm(), None);

    // On a machine with stated limits: the new op holds the card's RPM.
    let mut session = preset_session_under_constant_chip();
    let _ = session
        .apply(Command::SetMachine(SetMachineArgs {
            machine: Box::new(rates_and_accelerations()),
        }))
        .expect("the machine row");
    let draft = session
        .draft_toolpath(DraftRequest::new(OperationType::Pocket, 0, "Pocket 2").model(model))
        .expect("the draft");
    assert_eq!(draft.feeds_refusal, None);
    let drafted_rpm = draft.config.operation.spindle_rpm().expect("a seeded RPM");
    let card = session
        .cutter_op_profile(&draft.config)
        .expect("the draft has a tool");
    let choice = card
        .feeds
        .as_ref()
        .and_then(|f| f.rpm_choice.as_ref())
        .expect("the card runs the constant-chip rule");
    assert_eq!(
        f64::from(drafted_rpm),
        choice.rpm,
        "the add door and the card agree"
    );
    assert!(
        choice.chart_rpm.is_some_and(|chart| choice.rpm > chart),
        "non-vacuity: the policy raised the RPM above the chart RPM: {choice:?}"
    );
}
