//! D11 — the GUI prior-op check walks core's carve order, across setups.
//!
//! `planning/op_architecture_2026-10-14/AUDIT.md` D11, seen live on
//! 2026-10-14. Since S0 the stock carries from one setup to the next, but
//! `prior_stock_blocker` searched only the rest operation's own setup. A
//! rest operation first in setup 3 therefore read "the first enabled
//! operation in its setup — there is no prior operation", although setup 2
//! held a done operation.
//!
//! The claims:
//!
//! - the blocker is the nearest ENABLED row in
//!   `ProjectSession::carve_order_before`, so it can sit in an earlier setup;
//! - it skips a disabled row, and it is `None` only when no earlier row in
//!   the project is enabled;
//! - Generate All gives the setup-3 rest operation its snapshot, so no
//!   blocker exists at all.

use super::*;

use crate::state::toolpath::ComputeStatus;
use rs_cam_core::compute::transform::FaceUp;

/// Setup 1 = `Scallop` (fresh); setup 2 = `Middle` (fresh); setup 3 =
/// `Front rest`, the first and only row of setup 3, from the remaining stock.
///
/// `face_up_3` is the face of setup 3. `stock_change_on_2` adds one enabled
/// stock change to setup 2, as on the live test piece.
fn three_setups(
    face_up_3: FaceUp,
    stock_change_on_2: bool,
) -> (AppController<RestChainBackend>, [ToolpathId; 3]) {
    let mut controller = AppController::with_backend(RestChainBackend::new());
    controller.compute.result_moves = true;
    sample_project_into(&mut controller);
    let template = controller
        .state
        .session()
        .toolpath_configs()
        .first()
        .cloned()
        .expect("the fixture has a first toolpath");

    for (id, name, setup_name, face_up, source) in [
        (
            ToolpathId(9),
            "Middle",
            "2 Middle",
            FaceUp::Top,
            crate::state::toolpath::StockSource::Fresh,
        ),
        (
            ToolpathId(10),
            "Front rest",
            "3 Front",
            face_up_3,
            crate::state::toolpath::StockSource::FromRemainingStock,
        ),
    ] {
        let setup_index = controller
            .state
            .apply(Command::AddSetup(AddSetupArgs {
                name: Some(setup_name.to_owned()),
                face_up,
            }))
            .expect("the session accepts a setup")
            .created
            .expect("the AddSetup row reports the new setup index");
        let mut config = template.clone();
        config.id = id;
        config.name = name.to_owned();
        config.stock_source = source;
        let _ = controller
            .state
            .apply(Command::AddToolpath(AddToolpathArgs {
                setup_index,
                config: Box::new(config),
            }))
            .expect("the setup accepts a toolpath");
    }

    if stock_change_on_2 {
        let change = rs_cam_core::compute::stock_change::StockChange {
            id: rs_cam_core::ids::StockChangeId(0),
            name: "pour".to_owned(),
            enabled: true,
            op: rs_cam_core::compute::stock_change::StockChangeOp::Add,
            geometry: rs_cam_core::compute::stock_change::StockGeometry::Model {
                model_id: rs_cam_core::ids::ModelId(0),
            },
            material: Default::default(),
            display_colour: None,
            cut_as: Default::default(),
        };
        let _ = controller
            .state
            .apply(Command::AddStockChange(
                rs_cam_core::session::AddStockChangeArgs {
                    setup_index: 1,
                    change: Box::new(change),
                },
            ))
            .expect("setup 2 accepts a stock change");
    }

    // The session gives each added row its own id; read them back by name.
    let id_of = |name: &str| {
        controller
            .state
            .session()
            .toolpath_configs()
            .iter()
            .find(|tc| tc.name == name)
            .map(|tc| tc.id)
            .expect("the fixture added that row")
    };
    let ids = [template.id, id_of("Middle"), id_of("Front rest")];
    for (index, id) in ids.iter().enumerate() {
        let rt = controller.state.gui.toolpath_rt_or_default(*id);
        rt.run = crate::state::runtime::ToolpathRun::Pending;
        let _ = controller.state.apply(Command::ForgetResult(
            rs_cam_core::session::ForgetResultArgs { index },
        ));
    }
    (controller, ids)
}

/// The block the last submit of `id` recorded.
fn block_of(
    controller: &AppController<RestChainBackend>,
    id: ToolpathId,
) -> rs_cam_core::compute::AwaitingPriorStock {
    controller
        .state
        .gui
        .toolpath_rt
        .get(&id)
        .and_then(|rt| rt.run.status().blocked_on().cloned())
        .expect("a rest op with no snapshot records AwaitingPriorStock")
}

fn set_enabled(controller: &mut AppController<RestChainBackend>, id: ToolpathId, enabled: bool) {
    let index = index_of(controller, id);
    let _ = controller
        .state
        .apply(Command::SetToolpathEnabled(SetToolpathEnabledArgs {
            index,
            enabled,
        }))
        .expect("the index comes from the session");
}

/// The blocker of a rest op first in setup 3 is the setup-2 op; with that op
/// off it is the setup-1 op; with both off there is none.
#[test]
fn a_rest_op_first_in_setup_3_names_the_setup_2_op_d11() {
    let (mut controller, [first, middle, rest]) = three_setups(FaceUp::Top, false);
    let middle_index = index_of(&controller, middle);

    controller.submit_toolpath_compute(rest);
    let block = block_of(&controller, rest);
    assert_eq!(
        block.blocking_toolpath_id,
        Some(middle),
        "the stock carries across setups (S0): the blocker of the first rest op \
         of setup 3 is the last enabled op of setup 2. Got: {}",
        block.message
    );
    assert_eq!(block.blocking_toolpath_index, Some(middle_index));
    assert!(
        block.message.contains("Middle"),
        "the message names the setup-2 op, got: {}",
        block.message
    );
    assert!(
        !block.message.contains("first enabled operation"),
        "an earlier setup holds an op, so the message must not say there is none: {}",
        block.message
    );

    // A disabled row is skipped: the walk goes on into setup 1.
    set_enabled(&mut controller, middle, false);
    controller.submit_toolpath_compute(rest);
    let block = block_of(&controller, rest);
    assert_eq!(block.blocking_toolpath_id, Some(first));

    // No enabled row before it anywhere: no blocker, and the message says so.
    set_enabled(&mut controller, first, false);
    controller.submit_toolpath_compute(rest);
    let block = block_of(&controller, rest);
    assert_eq!(block.blocking_toolpath_id, None);
    assert!(
        block
            .message
            .contains("first enabled operation in the project"),
        "got: {}",
        block.message
    );
}

/// Generate All gives the setup-3 rest op a current snapshot, so it
/// generates and no blocker exists. Run on the live test piece's shape
/// (setup 3 flipped, a stock change in setup 2) and on the plain shape.
#[test]
fn generate_all_gives_the_setup_3_rest_op_its_snapshot_d11() {
    for (face_up_3, stock_change_on_2) in [
        (FaceUp::Top, false),
        (FaceUp::Bottom, false),
        (FaceUp::Bottom, true),
    ] {
        let (mut controller, [_, _, rest]) = three_setups(face_up_3, stock_change_on_2);
        controller.handle_internal_event(crate::ui::AppEvent::GenerateAll);
        pump_until_idle(&mut controller);
        let status = controller
            .state
            .gui
            .toolpath_rt
            .get(&rest)
            .map(|rt| rt.run.status());
        assert!(
            matches!(status, Some(ComputeStatus::Done)),
            "{face_up_3:?}, stock change {stock_change_on_2}: the plan simulates \
             setups 1..=3 before the rest op, so it must generate. Got {status:?}"
        );
    }
}

/// The live cause of 2026-10-14, reproduced (W1.5b). Red with the old plan
/// rule, which read `has_snapshot` alone.
///
/// 1. Run 1: `Middle` (setup 2) fails. The plan simulates before
///    `Front rest`; its snapshot and its result record only `Scallop`.
/// 2. Run 2: `Middle` generates. At plan time the snapshot of `Front rest`
///    still reads current, because rule (3) of `source_is_current` skips an
///    enabled row with no result. So the plan puts no Simulate step before
///    `Front rest`.
/// 3. `Middle`'s new result makes the rest result out of date; core drops it.
///    The Generate step of `Front rest` then submits against a snapshot that
///    is no longer current, and the op blocks on `Middle`.
#[test]
fn a_retry_after_an_upstream_failure_resimulates_before_the_rest_op_d11() {
    let (mut controller, [_, middle, rest]) = three_setups(FaceUp::Top, false);
    controller.compute.poison = Some(middle);
    controller.handle_internal_event(crate::ui::AppEvent::GenerateAll);
    pump_until_idle(&mut controller);
    assert!(matches!(
        controller
            .state
            .gui
            .toolpath_rt
            .get(&rest)
            .map(|rt| rt.run.status()),
        Some(ComputeStatus::Done)
    ));

    // The fake lane writes no source record; a real rest result carries the
    // record of the snapshot it read (`start` stamps it). Stamp it here.
    let rest_index = index_of(&controller, rest);
    let mut result = controller
        .state
        .session()
        .get_result(rest_index)
        .cloned()
        .expect("the rest op generated in the first run");
    result.stats.source_stock = controller
        .state
        .session()
        .simulation_result()
        .and_then(|sim| sim.prior_stock_sources.get(&rest).cloned());
    assert!(result.stats.source_stock.is_some());
    let revision = controller.state.session().toolpath_revision(rest_index);
    let _ = controller
        .state
        .apply(Command::AdoptResult(AdoptResultArgs {
            index: rest_index,
            revision,
            result: Box::new(result),
        }))
        .expect("the revision is current");
    assert!(
        controller.state.session().snapshot_is_current(rest).is_ok(),
        "precondition: the snapshot reads current while 'Middle' holds no result"
    );

    controller.compute.poison = None;
    controller.handle_internal_event(crate::ui::AppEvent::GenerateAll);
    pump_until_idle(&mut controller);
    let status = controller
        .state
        .gui
        .toolpath_rt
        .get(&rest)
        .map(|rt| rt.run.status());
    assert!(
        matches!(status, Some(ComputeStatus::Done)),
        "the plan generates 'Middle' before 'Front rest', so it must simulate \
         between them. Got {status:?}"
    );
}
