//! G-FEEDSLABEL sentry (UX-R03-005, 2026-09-10).
//!
//! The Feeds & Speeds card used to print the calculator's recommendation
//! under labels that read as the configured operation: `Commanded
//! advance/tooth: 0.0435 mm/tooth` and `DOC: 4.20 mm` on a fresh Pocket
//! whose stored feed gives 0.025 mm/tooth and whose stored depth per pass
//! is 1.2 mm. The `Apply cut geometry` button below the card writes 1.2.
//! An operator who read 4.20 believed the operation cut 4.2 mm per pass.
//!
//! This sentry pins the row model the card and the Feeds modal share
//! (`rs_cam_viz::ui::properties::feeds_rows`):
//!
//! - no calculator row label says "Commanded", and no recommendation
//!   hides behind a bare "DOC" / "WOC" label
//! - every recommendation row carries the configured value beside it
//! - on the R03 Pocket fixture both numbers appear on each row
//! - the raw calculator DOC / WOC carry the "(calculator)" note, because
//!   the apply funnel can lower them
//!
//! The modal uses the same constants and the same note helper, so the
//! assertions on them cover both surfaces.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_viz::ui::properties::feeds_rows::{
    ADVANCE_ROW_LABEL, DOC_ROW_LABEL, FeedsCardInputs, FeedsCardRow, MODAL_ADVANCE_ROW_LABEL,
    WOC_ROW_LABEL, advance_per_tooth_mm, feeds_card_rows, with_calculator_note,
};

/// The R03 demo pocket: fresh Pocket op on a Ø6 two-flute end mill,
/// Generic Wood Router. Stored feed 750, project RPM 15000, stored depth
/// per pass 1.2 (rigidity cap 0.20 × 6), stored stepover 2.1. The
/// calculator's target chipload is 0.0435, its no-hint DOC is 0.70 × 6 =
/// 4.2 and its WOC is 0.35 × 6 = 2.1.
fn r03_pocket() -> FeedsCardInputs {
    FeedsCardInputs {
        recommended_feed_mm_min: 1305.0,
        recommended_rpm: 15000.0,
        target_chip_load_mm: 0.0435,
        recommended_axial_depth_mm: Some(4.2),
        recommended_radial_width_mm: Some(2.1),
        configured_feed_mm_min: 750.0,
        configured_rpm: 15000,
        flute_count: 2,
        configured_depth_per_pass_mm: Some(1.2),
        configured_stepover_mm: Some(2.1),
    }
}

fn row<'a>(rows: &'a [FeedsCardRow], label: &str) -> &'a FeedsCardRow {
    rows.iter()
        .find(|r| r.label == label)
        .unwrap_or_else(|| panic!("no row labelled {label:?} in {rows:#?}"))
}

#[test]
fn no_calculator_row_is_labelled_commanded_or_bare() {
    let rows = feeds_card_rows(&r03_pocket());
    assert_eq!(rows.len(), 3, "advance, DOC and WOC rows: {rows:#?}");
    for r in &rows {
        assert!(
            !r.label.contains("Commanded"),
            "a calculator value must not be labelled Commanded: {:?}",
            r.label
        );
        assert!(
            r.label.starts_with("Recommended "),
            "a calculator value is a recommendation: {:?}",
            r.label
        );
    }
    assert!(!MODAL_ADVANCE_ROW_LABEL.contains("Commanded"));
}

#[test]
fn every_recommendation_row_carries_the_configured_value() {
    let rows = feeds_card_rows(&r03_pocket());
    for r in &rows {
        let configured = r
            .configured
            .as_deref()
            .unwrap_or_else(|| panic!("row {:?} has no configured value", r.label));
        assert!(
            configured.starts_with("configured "),
            "the configured cell names its role: {configured:?}"
        );
        assert!(!r.hover.is_empty(), "row {:?} has no hover text", r.label);
    }
}

#[test]
fn r03_pocket_shows_both_numbers_on_each_row() {
    let rows = feeds_card_rows(&r03_pocket());

    let doc = row(&rows, DOC_ROW_LABEL);
    assert!(doc.recommended.contains("4.20"), "{doc:#?}");
    assert!(doc.recommended.contains("(calculator)"), "{doc:#?}");
    assert_eq!(doc.configured.as_deref(), Some("configured 1.20 mm"));

    let woc = row(&rows, WOC_ROW_LABEL);
    assert!(woc.recommended.contains("2.10"), "{woc:#?}");
    assert!(woc.recommended.contains("(calculator)"), "{woc:#?}");
    assert_eq!(woc.configured.as_deref(), Some("configured 2.10 mm"));

    // The advance row prints feed ÷ (RPM × flutes) at the recommended
    // feed — the quantity its label names — not the pre-derate target.
    let adv = row(&rows, ADVANCE_ROW_LABEL);
    assert_eq!(adv.recommended, "0.0435 mm/tooth");
    assert_eq!(
        adv.configured.as_deref(),
        Some("configured 0.0250 mm/tooth")
    );
    assert!(adv.hover.contains("0.0435"), "{adv:#?}");
}

#[test]
fn rows_follow_the_operation_capabilities() {
    let mut inputs = r03_pocket();
    inputs.recommended_axial_depth_mm = None;
    inputs.configured_depth_per_pass_mm = None;
    let rows = feeds_card_rows(&inputs);
    assert!(rows.iter().all(|r| r.label != DOC_ROW_LABEL), "{rows:#?}");
    assert!(rows.iter().any(|r| r.label == WOC_ROW_LABEL), "{rows:#?}");

    inputs.recommended_radial_width_mm = None;
    inputs.configured_stepover_mm = None;
    let rows = feeds_card_rows(&inputs);
    assert_eq!(rows.len(), 1, "{rows:#?}");
    assert_eq!(rows[0].label, ADVANCE_ROW_LABEL);
}

#[test]
fn advance_per_tooth_degrades_without_panicking() {
    assert_eq!(advance_per_tooth_mm(750.0, 15000.0, 2), Some(0.025));
    assert_eq!(advance_per_tooth_mm(750.0, 15000.0, 0), None);
    assert_eq!(advance_per_tooth_mm(750.0, 0.0, 2), None);

    let mut inputs = r03_pocket();
    inputs.flute_count = 0;
    let rows = feeds_card_rows(&inputs);
    let adv = row(&rows, ADVANCE_ROW_LABEL);
    assert_eq!(adv.recommended, "\u{2014}");
    assert_eq!(adv.configured.as_deref(), Some("configured \u{2014}"));
}

#[test]
fn calculator_note_is_one_string() {
    assert_eq!(with_calculator_note("4.20 mm"), "4.20 mm (calculator)");
}
