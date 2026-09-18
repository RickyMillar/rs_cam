//! R12: a stock draft cannot say `auto_from_model` and carry manual numbers.
//!
//! # What the finding was
//!
//! The Corne case (`planning/corne_case_analysis_2026-09-18/ANALYSIS.md`
//! §1 footnote) held a project with `auto_from_model = true` beside
//! `y = 169.32`, while `StockConfig::update_from_bbox` on the model gives
//! `y = 111.19` (the fixture bbox below is rounded to 0.1 mm). The MCP `set_stock_config` door clears the flag when a
//! dimension or an origin is set explicitly. The GUI stock panel let the
//! operator drag a dimension under a ticked checkbox, so the file said
//! auto, the numbers were manual, and the loader (F-026) re-derived them
//! on the next load. 58 mm of stock in +Y held no part and nobody saw it.
//!
//! # What this test measures
//!
//! `ui::properties::stock::reconcile_auto_from_model` is the one rule the
//! panel's commit path (`apply_stock_draft`) runs. Each arm drives it with
//! a previous record, a draft and a model bbox:
//!
//! 1. a moved dimension clears the flag and keeps the numbers;
//! 2. a moved origin clears the flag and keeps the numbers;
//! 3. the flag flipped on refits the numbers from the bbox;
//! 4. a padding edit under auto refits with the new padding;
//! 5. a rigidity edit under auto refits a record whose numbers lag the
//!    model, which is what the commit path did before R12 and still does;
//! 6. no model: the flag keeps its value and nothing refits.
//!
//! # NOT MEASURED
//!
//! The live checkbox untick during a drag, which needs an egui frame, and
//! the caption under the checkbox.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_viz::ui::properties::stock::reconcile_auto_from_model;

/// The Corne model bbox as read in ANALYSIS.md §1.
fn corne_bbox() -> BoundingBox3 {
    BoundingBox3 {
        min: P3::new(5.3, -104.3, 0.0),
        max: P3::new(148.1, -3.1, 18.0),
    }
}

/// A record that `update_from_bbox` produced, so it agrees with the model.
fn auto_record() -> StockConfig {
    let mut stock = StockConfig {
        auto_from_model: true,
        padding: 5.0,
        ..StockConfig::default()
    };
    stock.update_from_bbox(&corne_bbox());
    stock
}

#[test]
fn a_moved_dimension_clears_the_flag_and_keeps_the_numbers() {
    let previous = auto_record();
    let mut draft = previous.clone();
    draft.y = 169.32;

    let reconciled = reconcile_auto_from_model(&previous, draft.clone(), Some(&corne_bbox()));

    assert!(
        !reconciled.auto_from_model,
        "a dragged dimension is a manual stock; the flag must clear"
    );
    assert_eq!(reconciled.y, 169.32, "the operator's number stands");
    assert_eq!(reconciled.x, previous.x, "an unmoved row keeps its value");
    let mut expected = draft;
    expected.auto_from_model = false;
    assert_eq!(reconciled, expected, "nothing else moves");
}

#[test]
fn a_moved_origin_clears_the_flag_and_keeps_the_numbers() {
    let previous = auto_record();
    let mut draft = previous.clone();
    draft.origin_x = 0.0;

    let reconciled = reconcile_auto_from_model(&previous, draft, Some(&corne_bbox()));

    assert!(
        !reconciled.auto_from_model,
        "a moved origin is a manual stock"
    );
    assert_eq!(reconciled.origin_x, 0.0, "the operator's origin stands");
    assert_eq!(reconciled.y, previous.y, "the dimensions do not refit");
}

#[test]
fn the_flag_flipped_on_refits_from_the_model() {
    let previous = StockConfig {
        x: 153.07,
        y: 169.32,
        origin_x: 0.0,
        origin_y: -110.0,
        auto_from_model: false,
        padding: 5.0,
        ..StockConfig::default()
    };
    let mut draft = previous.clone();
    draft.auto_from_model = true;

    let reconciled = reconcile_auto_from_model(&previous, draft, Some(&corne_bbox()));

    assert!(reconciled.auto_from_model, "the tick holds");
    let expected = auto_record();
    assert_eq!(
        reconciled, expected,
        "the numbers are the bbox refit, the same the loader and add_model produce"
    );
    assert!(
        reconciled.y < 120.0,
        "the 169 mm stock shrinks to the part: y = {}",
        reconciled.y
    );
}

#[test]
fn a_padding_edit_under_auto_refits_with_the_new_padding() {
    let previous = auto_record();
    let mut draft = previous.clone();
    draft.padding = 10.0;

    let reconciled = reconcile_auto_from_model(&previous, draft, Some(&corne_bbox()));

    assert!(
        reconciled.auto_from_model,
        "padding is an auto dial, not a manual number"
    );
    let mut expected = previous.clone();
    expected.padding = 10.0;
    expected.update_from_bbox(&corne_bbox());
    assert_eq!(reconciled, expected, "the refit uses the new padding");
    assert!(
        reconciled.x > previous.x && reconciled.origin_x < previous.origin_x,
        "more padding grows the stock outward"
    );
}

#[test]
fn a_rigidity_edit_under_auto_refits_a_stale_record() {
    // The record's numbers lag the model (the Corne file as found).
    let previous = StockConfig {
        x: 153.07,
        y: 169.32,
        origin_x: 0.0,
        auto_from_model: true,
        padding: 5.0,
        ..StockConfig::default()
    };
    let mut draft = previous.clone();
    draft.workholding_rigidity = rs_cam_core::feeds::WorkholdingRigidity::High;

    let reconciled = reconcile_auto_from_model(&previous, draft, Some(&corne_bbox()));

    assert!(reconciled.auto_from_model);
    let mut expected = previous.clone();
    expected.update_from_bbox(&corne_bbox());
    assert_eq!(
        reconciled.y, expected.y,
        "no row moved and the flag is on, so the numbers follow the model"
    );
    assert_eq!(
        reconciled.workholding_rigidity,
        rs_cam_core::feeds::WorkholdingRigidity::High
    );
}

#[test]
fn without_a_model_the_flag_keeps_its_value_and_nothing_refits() {
    let previous = StockConfig {
        auto_from_model: false,
        ..StockConfig::default()
    };
    let mut draft = previous.clone();
    draft.auto_from_model = true;

    let reconciled = reconcile_auto_from_model(&previous, draft.clone(), None);

    assert_eq!(
        reconciled, draft,
        "no bbox: the tick holds and the numbers stand"
    );

    let mut moved = previous.clone();
    moved.z = 30.0;
    let reconciled = reconcile_auto_from_model(&previous, moved.clone(), None);
    assert_eq!(
        reconciled, moved,
        "a moved row without a model is still manual"
    );
    assert!(!reconciled.auto_from_model);
}
