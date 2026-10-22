//! Geometry selection P6.7: the Selection section of a Drill and of a Bore
//! shows one row per hole size, with the count and the fit.
//!
//! # The claim
//!
//! The fit of each hole is THE fit of generation: the `hole_fits` of the
//! `selection_status` report (P6.5, `selection::hole_fit`). The section
//! groups the held holes by diameter and names the fit in the status slot
//! of each row, in the role of design section 7:
//!
//! | Size (6 mm tool) | Drill                      | Bore                   |
//! |---|---|---|
//! | 6 mm (x 2)       | drilled, Ok                | the tool size, Caution |
//! | 10 mm            | larger than the tool, Caution | helix r 2.000, Ok   |
//! | 4 mm             | smaller than the tool, Caution | (not held)         |
//! | 14 mm            | leaves a core, Caution     | leaves a core, Danger  |
//! | points           | drilled at the tool diameter, Ok | (not held)       |
//!
//! The Bore helix radius is core `ops::bore::centre_radius(10, 6)` = 2.
//! The colour arm is the discriminator: a word match alone passes with
//! every role swapped.
//!
//! Red proof (P6.7): swap `Role::Danger` and `Role::Caution` for
//! `LeavesCore` in `HoleSize::status`; the Bore colour arm fails.
//!
//! ```text
//! cargo test -p rs_cam_viz -q --test the_hole_rows_show_each_fit_p6_7
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod inspector_harness_p6_7;

use inspector_harness_p6_7 as h;
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::geo::P2;
use rs_cam_core::selection::{Item, ItemSet, Shape, SourceRef};
use rs_cam_viz::state::AppState;
use rs_cam_viz::ui::tokens;

/// The tool diameter (mm).
const TOOL_D: f64 = 6.0;

/// Two 6 mm circles, one each of 10, 4 and 14 mm, and one point.
fn holes() -> ItemSet {
    let circle = |x: f64, d: f64| Shape::circle(P2::new(x, 0.0), d);
    let shapes = vec![
        circle(0.0, 6.0),
        circle(20.0, 6.0),
        circle(40.0, 10.0),
        circle(60.0, 4.0),
        circle(80.0, 14.0),
        Shape::Point(P2::new(-20.0, 0.0)),
    ];
    ItemSet::new(
        shapes
            .into_iter()
            .map(|shape| Item::new(SourceRef::ToolpathModel, None, shape).expect("an item"))
            .collect(),
    )
}

fn stock() -> StockConfig {
    StockConfig {
        x: 160.0,
        y: 60.0,
        z: 18.0,
        origin_x: -40.0,
        origin_y: -30.0,
        origin_z: -18.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

fn state(op: OperationType) -> AppState {
    h::state(
        h::drawing_model(holes()),
        TOOL_D,
        stock(),
        OperationConfig::new_default(op),
    )
}

/// The status slot of the row whose word is `word`: it paints once, in
/// `color`.
fn assert_status(painted: &[h::Painted], word: &str, color: egui::Color32) {
    let hits = h::containing(painted, word);
    assert_eq!(
        hits.len(),
        1,
        "one status '{word}'; painted: {:?}",
        painted.iter().map(|p| &p.text).collect::<Vec<_>>()
    );
    assert_eq!(
        hits[0].color,
        Some(color),
        "the status '{word}' paints in the wrong role colour"
    );
}

#[test]
fn the_hole_rows_show_each_fit_p6_7() {
    let ctx = h::ctx();
    let mut drill = state(OperationType::Drill);
    let painted = h::render(&ctx, &mut drill, h::RAIL_WIDTH);
    for name in [
        "\u{00D8} 4.000",
        "\u{00D8} 6.000",
        "\u{00D8} 10.000",
        "\u{00D8} 14.000",
        "points",
    ] {
        assert!(
            !h::containing(&painted, name).is_empty(),
            "the Drill must paint the row '{name}'"
        );
    }
    assert!(
        !h::find(&painted, "\u{00D7} 2").is_empty(),
        "the two 6 mm circles are one row of 2"
    );
    assert_status(&painted, "larger than the tool", tokens::CAUTION);
    assert_status(&painted, "smaller than the tool", tokens::CAUTION);
    assert_status(&painted, "leaves a core", tokens::CAUTION);
    assert_status(&painted, "drilled at \u{00D8} 6.000", tokens::OK);
    let wrapped = h::wrapped(&painted);
    assert!(wrapped.is_empty(), "IC1: no text wraps: {wrapped:?}");
    let wide = h::past_the_rail(&painted);
    assert!(
        wide.is_empty(),
        "every row fits the 240-point rail: {wide:?}"
    );

    let ctx = h::ctx();
    let mut bore = state(OperationType::Bore);
    let painted = h::render(&ctx, &mut bore, h::RAIL_WIDTH);
    let helix = rs_cam_core::ops::bore::centre_radius(10.0, TOOL_D);
    assert!(
        (helix - 2.0).abs() < 1e-12,
        "the helix radius of 10 mm is 2"
    );
    assert_status(&painted, &format!("helix r {helix:.3}"), tokens::OK);
    assert_status(&painted, "leaves a core", tokens::DANGER);
    assert_status(&painted, "the tool size", tokens::CAUTION);
    let wrapped = h::wrapped(&painted);
    assert!(wrapped.is_empty(), "IC1: no text wraps: {wrapped:?}");
    let wide = h::past_the_rail(&painted);
    assert!(
        wide.is_empty(),
        "every row fits the 240-point rail: {wide:?}"
    );
}
