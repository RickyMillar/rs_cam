//! Geometry selection P6.7: the Profile Side row offers "On the line", and
//! under side On the Compensation entry "In Control" is greyed.
//!
//! # The claim
//!
//! Core refuses side On with compensation In Control at every door
//! (`domain::side_on_in_control`, L-5): a cut on the line has no side for
//! G41/G42. A draft that holds the pair is refused by
//! `ReplaceToolpathConfig`, and the panel then snaps back with no word.
//! So the inspector must not let the operator make the pair:
//!
//! 1. Positive control: under side Outside, a click on "In Control"
//!    stores `InControl`. The click path works.
//! 2. Under side On, the "In Control" entry paints greyed (the alpha of
//!    its colour is below 255; under Outside it is 255), and a click on it stores
//!    nothing. The door would refuse the pair too, so the opacity is the
//!    arm that sees the greying.
//! 3. Under In Control, a pick of "On the line" stores side On AND
//!    compensation In Computer, in ONE command (the toolpath revision
//!    moves by one).
//! 4. Under side On the inspector shows the Climb hint and the Tabs header
//!    "closed loops only", and no text wraps (IC1) in the 240-point rail.
//!
//! Red proof (P6.7): enable the "In Control" entry under side On
//! (`add_enabled_ui(true || ..)` in `boundary_2d.rs`); arm 2 fails on the
//! alpha. The stored value stays In Computer even then, because the
//! door refuses the pair: a store check alone cannot see the greying.
//!
//! ```text
//! cargo test -p rs_cam_viz -q --test side_on_greys_in_control_p6_7
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod inspector_harness_p6_7;

use inspector_harness_p6_7 as h;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{CompensationType, ProfileConfig, ProfileSide};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::selection::ItemSet;
use rs_cam_viz::state::AppState;

fn profile(side: ProfileSide, compensation: CompensationType) -> AppState {
    let model = h::drawing_model(ItemSet::from_polygons(vec![Polygon2::rectangle(
        -10.0, -10.0, 10.0, 10.0,
    )]));
    h::state(
        model,
        6.0,
        StockConfig::default(),
        OperationConfig::Profile(ProfileConfig {
            side,
            compensation,
            ..ProfileConfig::default()
        }),
    )
}

fn stored_profile(state: &AppState) -> ProfileConfig {
    match h::stored(state) {
        OperationConfig::Profile(cfg) => cfg,
        other => panic!("toolpath 0 is not a Profile: {other:?}"),
    }
}

/// Open the Advanced section and the Compensation combo; the alpha of the
/// "In Control" entry in the open list. Then click it.
fn pick_in_control(ctx: &egui::Context, state: &mut AppState) -> u8 {
    if h::find(&h::render(ctx, state, h::RAIL_WIDTH), "In Control").is_empty() {
        let _ = h::click(ctx, state, "Advanced", 0);
    }
    let painted = h::click_and_look(ctx, state, "In Computer", 0);
    let entry = h::find(&painted, "In Control");
    assert_eq!(entry.len(), 1, "the open list paints one 'In Control'");
    let alpha = entry[0].alpha;
    let _ = h::click(ctx, state, "In Control", 0);
    alpha
}

#[test]
fn side_on_greys_in_control_p6_7() {
    // 1. Positive control: under Outside the click stores In Control.
    let ctx = h::ctx();
    let mut state = profile(ProfileSide::Outside, CompensationType::InComputer);
    let alpha = pick_in_control(&ctx, &mut state);
    assert_eq!(
        alpha, 255,
        "under side Outside the In Control entry is not greyed"
    );
    assert_eq!(
        stored_profile(&state).compensation,
        CompensationType::InControl,
        "under side Outside the click on In Control must store it: the click path works"
    );

    // 2. Under On the entry is greyed: the click stores nothing.
    let ctx = h::ctx();
    let mut state = profile(ProfileSide::On, CompensationType::InComputer);
    let alpha = pick_in_control(&ctx, &mut state);
    assert!(
        alpha < 255,
        "under side On the In Control entry must be greyed (alpha {alpha})"
    );
    let cfg = stored_profile(&state);
    assert_eq!(cfg.side, ProfileSide::On);
    assert_eq!(
        cfg.compensation,
        CompensationType::InComputer,
        "under side On, In Control must be greyed: side On has no side for G41/G42"
    );
}

#[test]
fn on_the_line_sets_in_computer_in_one_command_p6_7() {
    let ctx = h::ctx();
    let mut state = profile(ProfileSide::Outside, CompensationType::InControl);
    let _ = h::render(&ctx, &mut state, h::RAIL_WIDTH);
    let before = state.session().toolpath_revision_at(0);
    let _ = h::click(&ctx, &mut state, "Outside", 0);
    let _ = h::click(&ctx, &mut state, "On the line", 0);
    let cfg = stored_profile(&state);
    assert_eq!(cfg.side, ProfileSide::On, "the pick stores side On");
    assert_eq!(
        cfg.compensation,
        CompensationType::InComputer,
        "the pick of On under In Control must set In Computer, or the door refuses the pair"
    );
    assert_eq!(
        state.session().toolpath_revision_at(0) - before,
        1,
        "the side and the compensation go in ONE command"
    );
}

#[test]
fn side_on_shows_its_hints_in_one_row_each_p6_7() {
    let ctx = h::ctx();
    let mut state = profile(ProfileSide::On, CompensationType::InComputer);
    let painted = h::render(&ctx, &mut state, h::RAIL_WIDTH);
    for text in [
        "On the line",
        "open paths cut in drawn direction",
        "Tabs \u{00B7} closed loops only",
    ] {
        assert!(
            !h::find(&painted, text).is_empty(),
            "side On must paint '{text}'; painted: {:?}",
            painted.iter().map(|p| &p.text).collect::<Vec<_>>()
        );
    }
    let wrapped = h::wrapped(&painted);
    assert!(
        wrapped.is_empty(),
        "IC1: no text wraps in the 240-point rail under side On: {wrapped:?}"
    );
    let wide = h::past_the_rail(&painted);
    assert!(
        wide.is_empty(),
        "every row fits the 240-point rail: {wide:?}"
    );
}
