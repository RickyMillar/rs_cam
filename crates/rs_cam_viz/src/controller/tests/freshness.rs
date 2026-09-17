//! G-FRESHSTATE (F2.1) — one freshness model, walked over R0.1 §2.3's
//! mutation matrix.

use super::*;

/// WP5. The projection protects the three fields the inspector entry
/// cannot supply.
///
/// `ToolpathEntry` carries sixteen of the nineteen `ToolpathConfig`
/// fields. `id`, `boundary_inherit` and `planner_origin` are not among
/// them. `Command::ReplaceToolpathConfig` writes whatever the caller
/// passes and protects none of the three — the core sentry
/// `replace_toolpath_config_gates_on_the_signature.rs` pins that. So the
/// protection has exactly one home: the projection clones the STORED
/// configuration and applies the sixteen fields onto the clone, never a
/// fresh literal. A fresh literal would clear `boundary_inherit`, which
/// reaches emitted geometry (G-BOUNDARYINHERIT), and would delete the
/// multi-tool planner's ownership stamp.
#[test]
fn the_projection_keeps_the_three_fields_the_entry_cannot_supply_wp5() {
    let mut controller = sample_controller();
    let id = controller.state.session.toolpath_configs()[0].id;
    let origin = rs_cam_core::session::PlannerOrigin {
        plan_id: 7,
        tier: 1,
        tier_count: 2,
    };
    {
        let stored = &controller.state.session.toolpath_configs()[0];
        assert!(
            stored.boundary_inherit,
            "the fixture must start with boundary_inherit true, or the \
             flip below asserts nothing"
        );
        let mut config = stored.clone();
        config.boundary_inherit = false;
        config.planner_origin = Some(origin.clone());
        let _ = controller
            .state
            .session
            .apply(Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
                index: 0,
                config: Box::new(config),
            }))
            .expect("index 0 exists");
    }

    let mut entry = crate::ui::properties::build_entry_from_session_and_gui(
        id,
        &controller.state.session,
        &controller.state.gui,
    )
    .expect("toolpath exists");
    // A widget cannot write these two, so the entry is the wrong place to
    // read them from. Set the id to a value the session does not carry,
    // and the projection must still answer with the stored one.
    entry.id = ToolpathId(4242);
    entry.name = "renamed".to_owned();

    let (_, stored) = controller
        .state
        .session
        .find_toolpath_config_by_id(id)
        .expect("toolpath exists");
    let projected = crate::ui::properties::project_entry_onto(stored, &entry);

    assert_eq!(
        projected.id, id,
        "the projection keeps the stored id. The entry's id names the row \
         to write, not a value to write."
    );
    assert!(
        !projected.boundary_inherit,
        "the projection keeps the stored boundary_inherit"
    );
    assert_eq!(
        projected.planner_origin.as_ref(),
        Some(&origin),
        "the projection keeps the stored planner_origin"
    );
    assert_eq!(
        projected.name, "renamed",
        "the sixteen supplied fields still land, or this test would pass \
         over a projection that copied the stored config and wrote nothing"
    );
}

/// WP5. An open panel with no edit drops no result.
///
/// The inspector holds no commit event: it rebuilds the entry, draws it,
/// and writes it back on every frame. WP5 makes that write-back one
/// `Command::ReplaceToolpathConfig` per frame, so the core gate — not the
/// panel — is what keeps a healthy card green while the operator only
/// looks at it. An ungated replacement would drop the geometry, and the
/// downstream chain with it, sixty times a second.
#[test]
fn an_open_panel_that_edits_nothing_drops_no_result_wp5() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let id = controller.state.session.toolpath_configs()[0].id;
    assert_eq!(
        state_of(&controller, 0),
        FreshnessState::Current,
        "the fixture must start Current, or the assertions below are \
         vacuous"
    );

    // Three frames of an open panel, each one a build-draw-write cycle
    // with no widget edit at all.
    for _ in 0..3 {
        panel_edit(&mut controller, id, |_| {});
    }

    assert!(
        controller.state.session.get_result(0).is_some(),
        "a frame that moved no generation input must keep the cached \
         result"
    );
    assert_eq!(
        state_of(&controller, 0),
        FreshnessState::Current,
        "the card must still read OK after looking at the panel"
    );
    assert!(
        !controller.state.gui.dirty,
        "looking at the panel must not dirty the project (G-HEIGHTSTAB)"
    );
    assert!(
        controller.state.gui.toolpath_rt[&id].stale_since.is_none(),
        "no edit, no stale stamp"
    );
}

/// The whole point of the model: an edit through the panel leaves the card
/// no longer able to say "OK". Pre-fix this read `Current`, because the
/// panel wrote `tc.operation` through `find_toolpath_config_by_id_mut` and
/// no core setter ran — the core kept the previous parameter set's result
/// and `emitted_toolpaths` exported it.
#[test]
fn freshness_state_g_freshness_op_field_edit_is_edited_since() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let id = controller.state.session.toolpath_configs()[0].id;
    assert_eq!(state_of(&controller, 0), FreshnessState::Current);

    panel_edit(&mut controller, id, |entry| {
        entry.operation.set_feed_rate(1234.0);
    });

    assert_eq!(state_of(&controller, 0), FreshnessState::EditedSince);
    assert!(
        controller.state.session.get_result(0).is_none(),
        "the core must not keep a result for inputs that moved"
    );
    assert!(
        controller.state.gui.toolpath_rt[&id].result.is_some(),
        "the GUI keeps the old geometry so the viewport can draw it"
    );
    assert!(controller.state.gui.dirty);
    assert!(controller.state.gui.toolpath_rt[&id].stale_since.is_some());
}

/// The five inspector rows R0.1 §2.3 marked "S1 none, S2 none, S3 none" —
/// each one a green card over a result generated from other inputs.
#[test]
fn freshness_state_g_freshness_every_inspector_input_stales() {
    /// One inspector row: a label and the widget edit it stands for.
    type EntryEdit = Box<dyn Fn(&mut crate::state::toolpath::ToolpathEntry)>;
    let rows: Vec<(&str, EntryEdit)> = vec![
        (
            "heights",
            Box::new(|e: &mut crate::state::toolpath::ToolpathEntry| {
                e.heights.clearance_z = rs_cam_core::compute::config::HeightMode::Manual(55.0);
            }),
        ),
        (
            "boundary",
            Box::new(|e: &mut crate::state::toolpath::ToolpathEntry| {
                e.boundary.enabled = !e.boundary.enabled;
            }),
        ),
        (
            "dressup",
            Box::new(|e: &mut crate::state::toolpath::ToolpathEntry| {
                e.dressups.arc_fitting = !e.dressups.arc_fitting;
            }),
        ),
        (
            "stock source",
            Box::new(|e: &mut crate::state::toolpath::ToolpathEntry| {
                e.stock_source = rs_cam_core::compute::config::StockSource::FromRemainingStock;
            }),
        ),
        (
            "rest analysis",
            Box::new(|e: &mut crate::state::toolpath::ToolpathEntry| {
                e.rest_analysis.enabled = !e.rest_analysis.enabled;
            }),
        ),
        (
            "face selection clear",
            Box::new(|e: &mut crate::state::toolpath::ToolpathEntry| {
                e.face_selection = Some(Vec::new());
            }),
        ),
    ];

    for (label, edit) in rows {
        let mut controller = sample_controller();
        generate_all_for_test(&mut controller);
        let id = controller.state.session.toolpath_configs()[0].id;
        panel_edit(&mut controller, id, edit);
        assert_eq!(
            state_of(&controller, 0),
            FreshnessState::EditedSince,
            "{label}: an edited input must not read Current"
        );
        assert!(
            controller.state.session.get_result(0).is_none(),
            "{label}: the core result must be gone"
        );
        assert!(controller.state.gui.dirty, "{label}: the project is dirty");
    }
}

/// Rebinding the tool or the model changes what is cut, and both used to
/// leave the core result, the regeneration request and the dirty flag
/// untouched (R0.1 §2.3, "Tool / model reassignment").
#[test]
fn freshness_state_g_freshness_tool_and_model_reassignment_stale() {
    for row in ["tool", "model"] {
        let mut controller = sample_controller();
        let mut tools = controller.state.session.tools().to_vec();
        tools.push(ToolConfig::new_default(ToolId(2), ToolType::EndMill));
        let _ = controller
            .state
            .session
            .apply(Command::ReplaceTools(ReplaceToolsArgs { tools }))
            .expect("the session takes the tool list");
        generate_all_for_test(&mut controller);
        let id = controller.state.session.toolpath_configs()[0].id;
        panel_edit(&mut controller, id, |entry| {
            if row == "tool" {
                entry.tool_id = crate::state::job::ToolId(2);
            } else {
                entry.model_id = crate::state::job::ModelId(7);
            }
        });
        assert_eq!(
            state_of(&controller, 0),
            FreshnessState::EditedSince,
            "{row} reassignment"
        );
        assert!(controller.state.session.get_result(0).is_none());
    }
}

/// The other half of the contract: an edit that changes no motion leaves
/// the toolpath current. Without this the model would be a rename away
/// from staling the whole project.
#[test]
fn name_and_gcode_edits_do_not_stale() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let id = controller.state.session.toolpath_configs()[0].id;

    panel_edit(&mut controller, id, |entry| {
        entry.name = "Renamed".to_owned();
        entry.pre_gcode = "M8".to_owned();
        entry.post_gcode = "M9".to_owned();
    });

    assert_eq!(state_of(&controller, 0), FreshnessState::Current);
    assert!(controller.state.session.get_result(0).is_some());
    assert_eq!(
        controller.state.session.toolpath_configs()[0].name,
        "Renamed",
        "the edit still reached the session"
    );
}

/// A never-generated toolpath is `NoResult`, not `EditedSince`: the two
/// are the same absent core entry, and only the GUI's retained result
/// separates them.
#[test]
fn freshness_never_generated_is_no_result() {
    let mut controller = sample_controller();
    let second = push_toolpath(&mut controller, "Second");
    controller.state.gui.toolpath_rt.remove(&second);
    assert_eq!(state_of(&controller, 1), FreshnessState::NoResult);
}

/// `enabled: false` wins over everything, exactly as `ComputeStatus`
/// already ruled for the status chip.
#[test]
fn freshness_disabled_wins() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let id = controller.state.session.toolpath_configs()[0].id;
    controller.handle_internal_event(crate::ui::AppEvent::ToggleToolpathEnabled(id));
    assert_eq!(state_of(&controller, 0), FreshnessState::Disabled);
    assert!(
        controller.state.gui.dirty,
        "toggling an operation off changes what the job cuts"
    );
}

/// R0.1 §7 Q4, operator-confirmed: swapping two `Fresh` operations does
/// not change either one's geometry, so both stay `Current`; only a
/// downstream `FromRemainingStock` op goes `EditedSince`.
#[test]
fn freshness_reorder_keeps_fresh_ops_current() {
    let mut controller = sample_controller();
    let second = push_toolpath(&mut controller, "Second");
    let third = push_toolpath(&mut controller, "Rest");
    if let Some((idx, _)) = controller.state.session.find_toolpath_config_by_id(third) {
        let _ = controller
            .state
            .session
            .apply(Command::SetStockSource(SetStockSourceArgs {
                index: idx,
                source: rs_cam_core::compute::config::StockSource::FromRemainingStock,
            }))
            .expect("index is in range");
    }
    generate_all_for_test(&mut controller);

    controller.handle_internal_event(crate::ui::AppEvent::ReorderToolpath(second, 0));

    assert_eq!(state_of(&controller, 0), FreshnessState::Current);
    assert_eq!(state_of(&controller, 1), FreshnessState::Current);
    assert_eq!(state_of(&controller, 2), FreshnessState::EditedSince);
}

/// R0.1 §7 Q1, operator ruling 2026-09-10: ANY stock edit — dimensions,
/// pins, or the material alone — stales every toolpath, on both routes.
/// The GUI route used to stale none of them.
///
/// WP6 moved the GUI route onto the command door. The panel edits a
/// scratch copy and calls `ui::properties::apply_stock_draft`, which
/// applies `Command::SetStockConfig` and stamps every index
/// `Effects::stale` names. This drives that function, not the deleted
/// `AppEvent::StockChanged`.
#[test]
fn freshness_stock_edit_stales_every_toolpath() {
    let mut controller = sample_controller();
    push_toolpath(&mut controller, "Second");
    generate_all_for_test(&mut controller);

    let mut draft = controller.state.session.stock_config().clone();
    // The fixture's stock carries `auto_from_model`, and the apply funnel
    // re-sizes such a draft around the first model BEFORE it compares. A
    // typed dimension alone would therefore arrive back at the stored
    // record and the funnel would apply nothing. The operator clears the
    // checkbox to type a dimension, so the draft does the same.
    draft.auto_from_model = false;
    draft.x = 321.0;
    crate::ui::properties::apply_stock_draft(&mut controller.state, draft);

    for index in 0..2 {
        assert_eq!(
            state_of(&controller, index),
            FreshnessState::EditedSince,
            "toolpath {index} after a stock edit"
        );
        assert!(controller.state.session.get_result(index).is_none());
    }
    assert!(controller.state.gui.dirty);
    assert!(
        controller.state.panel_side_effects.upload,
        "the stock box the viewport draws moved, so the panel owes an upload"
    );
    assert!(
        controller.state.panel_side_effects.pin_drill_sync,
        "the deleted StockChanged handler always ran the pin-drill sync"
    );
}

/// R0.1 §7 Q3, operator ruling: a machine kinematics edit stales the
/// SIMULATION only. Timing and feed modulation move; geometry does not.
#[test]
fn freshness_machine_kinematics_leaves_toolpaths_current() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let kinematics = controller.state.session.machine().effective_kinematics();
    let _ = controller
        .state
        .session
        .apply(Command::SetMachineKinematics(
            rs_cam_core::session::SetMachineKinematicsArgs {
                kinematics: Box::new(kinematics),
            },
        ));
    assert_eq!(state_of(&controller, 0), FreshnessState::Current);
    assert!(controller.state.session.get_result(0).is_some());
    assert!(controller.state.session.simulation_result().is_none());
}

/// A setup orientation flip regenerates every toolpath in that setup in a
/// new frame. The panel wrote `face_up` / `z_rotation` straight into
/// `SetupData`, so no core setter ran and every result survived.
#[test]
fn freshness_setup_orientation_stales_the_setup() {
    for row in ["face", "rotation"] {
        let mut controller = sample_controller();
        push_toolpath(&mut controller, "Second");
        generate_all_for_test(&mut controller);

        if row == "face" {
            let _ = controller
                .state
                .session
                .apply(Command::SetSetupFace(SetSetupFaceArgs {
                    setup_index: 0,
                    face_up: rs_cam_core::compute::transform::FaceUp::Bottom,
                }))
                .expect("setup 0 exists");
        } else {
            let _ = controller
                .state
                .session
                .apply(Command::SetSetupRotation(SetSetupRotationArgs {
                    setup_index: 0,
                    z_rotation: rs_cam_core::compute::transform::ZRotation::Deg90,
                }))
                .expect("setup 0 exists");
        }

        for index in 0..2 {
            assert_eq!(
                state_of(&controller, index),
                FreshnessState::EditedSince,
                "{row}: toolpath {index}"
            );
        }
    }
}

/// A tool edit drops the results of every op that tool machines. It used
/// to drop them in the core and mark nothing in the GUI, which is the one
/// case the export fallback to the GUI's own copy was written for.
#[test]
fn freshness_tool_edit_stales_its_users_and_requests_regeneration() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let id = controller.state.session.toolpath_configs()[0].id;

    let mut draft = controller.state.session.tools()[0].clone();
    draft.diameter += 1.0;
    crate::ui::properties::commit_tool_draft(&mut controller.state, ToolId(1), draft);

    assert_eq!(state_of(&controller, 0), FreshnessState::EditedSince);
    assert!(
        controller.state.gui.toolpath_rt[&id].stale_since.is_some(),
        "the operator gets a regeneration request, not a silently stale card"
    );
    assert!(controller.state.gui.dirty);
}

/// The revision counter R0.1 §4.2 asks for: bumped at the one drop site,
/// never by recording an answer. F2.4's late-result guard reads it.
#[test]
fn toolpath_revision_bumps_on_every_input_drop() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let before = controller.state.session.toolpath_revision(0);

    let _ = controller
        .state
        .session
        .apply(Command::AdoptResult(AdoptResultArgs {
            index: 0,
            revision: before,
            result: Box::new(core_result()),
        }))
        .expect("index is in range");
    assert_eq!(
        controller.state.session.toolpath_revision(0),
        before,
        "recording an answer is not a change of inputs"
    );

    let id = controller.state.session.toolpath_configs()[0].id;
    panel_edit(&mut controller, id, |entry| {
        entry.operation.set_feed_rate(999.0);
    });
    assert!(
        controller.state.session.toolpath_revision(0) > before,
        "an input edit must move the revision"
    );
}

// The F2.1 pre-fix reproduction stood here. It wrote
// `tc.operation.set_feed_rate` through `find_toolpath_config_by_id_mut`
// and asserted the core result SURVIVED, which is what made the card
// read OK over geometry from other inputs.
//
// WP7 deleted it. The nine mutation hatches are `pub(crate)`, so an
// un-commanded write from viz is a compile error and no test in this
// crate can reproduce one. The guarantee moved from a reproduction to
// the type system, and `crates/rs_cam_core/tests/
// hatches_are_crate_private_wp7.rs` is the sentry that holds it. The
// core half of the same reproduction survives in-crate, in
// `session/mutation.rs`, where the hatch is still in reach.

// ── F2.2 / G-FRESHRENDER — the surfaces draw the state F2.1 derived ──────
//
// F2.1 built one `FreshnessState` and proved every mutation lands on the
// right one. Nothing read it: the card chip, the inspector header, the two
// workspace chips and Readiness each still asked their own question, and on
// an edited operation every one of them answered "fine". The card asked
// `ComputeStatus`, which is `Done` — the generation really did finish.
// Readiness and the Readiness chip asked `gui.toolpath_rt[..].result`, the
// GUI's retained copy, which an edit deliberately KEEPS so the viewport can
// still draw something. So a project where no operation could be reproduced
// read `OK`, `2/2 computed` and no chip at all.
//
// These tests drive the real surface functions, not a source read: the chip
// vocabulary, the two badges, `operations_check` and the shared counter are
// all pure over `&AppState`. The three surfaces that cannot be driven from a
// test — the egui header, the card body, the wgpu draw — are asserted in
// `tests/freshness_surfaces_g_freshrender.rs` by reading their source, which
// says so in its own doc.
//
// NOT asserted anywhere, and deliberately: that an operation reading STALE
// cannot be exported. It still can. F2.3 owns the export gate; until it
// lands, `emitted_toolpaths` falls back to the GUI's retained result and
// will emit the old geometry. Nothing in this task's UI text says otherwise.

/// The chip vocabulary, one state at a time. `EditedSince` is the row that
/// did not exist before: it used to fall through to `Done` → `OK` green.
#[test]
fn freshness_chip_says_stale_and_never_says_ok() {
    use crate::ui::toolpath_panel::status_chip;

    let cases = [
        (FreshnessState::Current, "OK"),
        (FreshnessState::EditedSince, "STALE"),
        (FreshnessState::Regenerating, "GEN"),
        (FreshnessState::NoResult, "PEND"),
        (FreshnessState::Disabled, "OFF"),
        (FreshnessState::Error("boom".to_owned()), "ERR"),
    ];
    for (state, expected) in &cases {
        let (text, _, _) = status_chip(state);
        assert_eq!(&text, expected, "{state:?}");
    }

    // The one that matters, in detail. Amber and not the success colour;
    // red stays reserved for ERR and collisions.
    let (text, role, hover) = status_chip(&FreshnessState::EditedSince);
    assert_eq!(text, "STALE");
    // UP4: the mapping returns a ROLE rather than a raw colour, which makes
    // this assertion stronger. Under UP1's palette several theme names
    // collapsed onto one value, so comparing colours could have passed while
    // the meaning drifted; a role cannot.
    assert_eq!(role, crate::ui::components::Role::Caution);
    assert_ne!(role, crate::ui::components::Role::Ok);
    assert_ne!(role, crate::ui::components::Role::Danger);
    let hover = hover.expect("four letters cannot carry this on their own");
    assert!(
        hover.contains("PREVIOUS generation"),
        "the hover must say whose numbers these are: {hover}"
    );
    assert!(
        hover.contains("Regenerate"),
        "and what to do about it: {hover}"
    );
    // The caution the programme is under until F2.3: this text must not
    // imply the export is gated on it, because it is not.
    assert!(
        !hover.to_lowercase().contains("export"),
        "F2.3 owns the export gate; this hover must not promise it: {hover}"
    );
}

/// THE headline row. One panel edit on a generated project and every
/// surface that can be driven from a test moves together.
#[test]
fn freshness_surfaces_agree_after_one_edit_g_freshrender() {
    use crate::ui::readiness;
    use crate::ui::workspace_bar;

    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let id = controller.state.session.toolpath_configs()[0].id;
    let total = controller.state.session.toolpath_configs().len();

    // Before: everything current, nothing to report.
    assert_eq!(state_of(&controller, 0), FreshnessState::Current);
    let (status, current, enabled) = readiness::operations_check(&controller.state);
    assert_eq!((current, enabled), (total, total));
    assert_eq!(status, readiness::CheckStatus::Pass);
    assert_eq!(readiness::freshness_counts(&controller.state), (0, 0));
    assert!(workspace_bar::toolpath_badge(&controller.state).is_none());

    panel_edit(&mut controller, id, |entry| {
        entry.operation.set_feed_rate(4321.0);
    });

    // After: one stale operation, said the same way everywhere.
    assert_eq!(state_of(&controller, 0), FreshnessState::EditedSince);
    assert_eq!(
        readiness::freshness_counts(&controller.state),
        (1, 0),
        "one stale, none merely pending"
    );

    let (status, current, enabled) = readiness::operations_check(&controller.state);
    assert_eq!(
        current,
        total - 1,
        "F1.17: Readiness counted the GUI's retained result and read {total}/{total} here"
    );
    assert_eq!(enabled, total);
    assert_eq!(status, readiness::CheckStatus::Warning);

    let (chip, role) =
        workspace_bar::toolpath_badge(&controller.state).expect("the Toolpaths tab must say so");
    assert_eq!(chip, "1 stale");
    assert_eq!(role, crate::ui::components::Role::Caution);

    let (chip, _) = workspace_bar::readiness_badge(&controller.state)
        .expect("the Readiness tab must say so too");
    assert_eq!(chip, "1 stale");

    // And the card's own chip.
    let (text, _, _) = crate::ui::toolpath_panel::status_chip(&state_of(&controller, 0));
    assert_eq!(text, "STALE");
}

/// Stale outranks pending on both chips, and the two counts do not merge.
/// A project with one of each must report the stale one: it is the one
/// currently showing a wrong answer rather than no answer.
#[test]
fn freshness_chip_reports_stale_ahead_of_pending() {
    use crate::ui::readiness;
    use crate::ui::workspace_bar;

    let mut controller = sample_controller();
    let second = push_toolpath(&mut controller, "Second");
    generate_all_for_test(&mut controller);
    let first = controller.state.session.toolpath_configs()[0].id;

    // Toolpath 1 never generated, toolpath 0 generated then edited.
    let _ = controller
        .state
        .session
        .apply(Command::InvalidateToolpathInputs(
            InvalidateToolpathInputsArgs { index: 1 },
        ))
        .expect("toolpath 1 exists");
    if let Some(rt) = controller.state.gui.toolpath_rt.get_mut(&second) {
        rt.result = None;
    }
    panel_edit(&mut controller, first, |entry| {
        entry.operation.set_feed_rate(999.0);
    });

    assert_eq!(state_of(&controller, 0), FreshnessState::EditedSince);
    assert_eq!(state_of(&controller, 1), FreshnessState::NoResult);
    assert_eq!(readiness::freshness_counts(&controller.state), (1, 1));

    let (chip, _) = workspace_bar::toolpath_badge(&controller.state).expect("something to report");
    assert_eq!(
        chip, "1 stale",
        "a stale operation outranks a pending one; folding them into one \
         count is what let the chip read zero on a fully edited project"
    );
}

/// A collision still outranks staleness on the Readiness chip. SHE-003's
/// order is not weakened by adding a state above "uncomputed".
#[test]
fn freshness_does_not_outrank_a_collision() {
    use crate::ui::workspace_bar;

    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let id = controller.state.session.toolpath_configs()[0].id;
    panel_edit(&mut controller, id, |entry| {
        entry.operation.set_feed_rate(4321.0);
    });
    controller.state.simulation.checks.rapid_collisions =
        vec![rs_cam_core::stock::collision::RapidCollision {
            move_index: 0,
            start: rs_cam_core::geo::P3::new(0.0, 0.0, 0.0),
            end: rs_cam_core::geo::P3::new(1.0, 0.0, 0.0),
        }];

    let (chip, role) =
        workspace_bar::readiness_badge(&controller.state).expect("a collision must be reported");
    // UR2 (e7901838): one shared safety text replaces the two per-tab
    // spellings. The count, not the word "collision", is the claim.
    assert_eq!(chip, "1 safety");
    // UP4: the badge producers hand back a ROLE now, not a colour, so the
    // `role_for` shim in the bar is gone and a safety badge cannot drift
    // onto a non-safety hue.
    assert_eq!(role, crate::ui::components::Role::Danger);
}

/// Disabled operations are not outstanding work, and an errored one is not
/// "still to do" — A/M11's rule that a block is never a failure, mirrored.
#[test]
fn freshness_counts_exclude_disabled_and_error() {
    use crate::ui::readiness;

    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let id = controller.state.session.toolpath_configs()[0].id;

    let _ = controller
        .state
        .session
        .apply(Command::SetToolpathEnabled(SetToolpathEnabledArgs {
            index: 0,
            enabled: false,
        }))
        .expect("index 0 exists");
    assert_eq!(state_of(&controller, 0), FreshnessState::Disabled);
    assert_eq!(readiness::freshness_counts(&controller.state).0, 0);

    let _ = controller
        .state
        .session
        .apply(Command::SetToolpathEnabled(SetToolpathEnabledArgs {
            index: 0,
            enabled: true,
        }))
        .expect("index 0 exists");
    if let Some(rt) = controller.state.gui.toolpath_rt.get_mut(&id) {
        rt.status = crate::state::runtime::ComputeStatus::Error("nope".to_owned());
    }
    let (stale, pending) = readiness::freshness_counts(&controller.state);
    assert_eq!(
        (stale, pending),
        (0, 0),
        "an error is neither stale nor pending; it needs a fix, not a wait"
    );
}
