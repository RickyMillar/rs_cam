//! K5 ReachHolder (feeds contract P7): a cut as deep below the top as the
//! stickout warns: the holder face reaches the stock.
//!
//! The case is op 3 of the blind review (report 04 (a)): the 6 mm flat end
//! mill with stickout 30 mm drills the pins 30 mm deep, so at the hole
//! bottom the holder face is at the stock top. Equality warns: no clearance
//! constant has a source. The holder walk of the simulation misses this
//! case (the shank is not wider than the cutter, and the strike needs a
//! penetration), so the pre-simulation row is the one that catches it. A
//! stickout of 31 mm gives no row. The row reads the stock box, so it can
//! over-read where an earlier op removed the material: it is a Tool warning
//! (no confirm click) and a Caution (lead decision). Ruling Q6: generation
//! and export always give output.
//!
//! Red: make `ReachHolder` give no row (its predicate `false`). The op 3
//! case then has no warning.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use common::k5_cases::{FLAT_6_STICKOUT_MM, export, export_names, op3_generated, rule_texts};
use rs_cam_core::diagnostics::{Danger, Severity, ids};

#[test]
fn the_holder_face_at_the_stock_top_warns() {
    let session = op3_generated(FLAT_6_STICKOUT_MM);
    let texts = rule_texts(&session, 0, ids::TOOL_REACH_TO_HOLDER);
    assert_eq!(texts.len(), 1, "one ReachHolder row at equality: {texts:?}");
    let text = texts.first().expect("one row");
    assert!(
        text.contains("stickout 30 mm, estimated")
            && text.contains("cuts 30 mm below the top")
            && text.contains("by the stock box"),
        "the row names the stickout, how it is known, the depth and the measure: {text}"
    );
    let exported = export(&session);
    assert!(
        !exported.gcode.is_empty(),
        "the export writes a program (Q6)"
    );
    assert!(
        export_names(&exported, ids::TOOL_REACH_TO_HOLDER),
        "the export and the header name the warning: {:?}",
        exported.warnings
    );
    // Lead decision: a stock-box reading is a Tool warning (no confirm
    // click) and a Caution in the diagnostics; the simulation measures
    // what touched.
    assert!(
        exported
            .warnings
            .iter()
            .filter(|w| w.id == ids::TOOL_REACH_TO_HOLDER)
            .all(|w| w.danger == Danger::Tool),
        "the pre-simulation ReachHolder row is a Tool warning"
    );
    let diagnostics = session.diagnose_toolpath(0).expect("the diagnostics");
    let row = diagnostics
        .iter()
        .find(|d| d.id.0 == ids::TOOL_REACH_TO_HOLDER)
        .expect("the ReachHolder diagnostic");
    assert_eq!(row.severity, Severity::Caution, "not Critical");
}

#[test]
fn one_more_mm_of_stickout_gives_no_row() {
    let session = op3_generated(FLAT_6_STICKOUT_MM + 1.0);
    assert!(
        rule_texts(&session, 0, ids::TOOL_REACH_TO_HOLDER).is_empty(),
        "stickout 31 mm clears the 30 mm hole"
    );
    // The flute row stands: the flute is still 25 mm.
    assert_eq!(rule_texts(&session, 0, ids::TOOL_REACH_PAST_FLUTE).len(), 1);
}
