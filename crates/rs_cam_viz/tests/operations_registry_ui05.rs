//! UI-05's sentry: every operation has ONE UI row, and a blank diagram is
//! declared rather than forgotten.
//!
//! # What the finding was
//!
//! `OperationConfig` was re-matched in five UI places beside core's
//! `for_each_op!` list. Four of those matches are exhaustive, so the
//! compiler catches a new operation. The fifth — the shape-diagram fallback
//! in `toolpath_panel.rs` — ended `_ => {}`. A new operation drew no diagram
//! and nothing said so, which the audit's add-an-operation list names as one
//! of the two steps that fail silently.
//!
//! # What this test measures
//!
//! The shape `overlays_registry` uses: the declared table is held against
//! the enum it claims to cover.
//!
//! - every `OperationType::ALL` variant has exactly one `OpUiRow`;
//! - no row names an operation the enum does not carry;
//! - a row that shows no diagram states a reason, and the reason is not
//!   empty.
//!
//! # NOT MEASURED
//!
//! Whether a diagram is the RIGHT picture, and whether an operation's
//! validation arm catches everything it should. The editor field is
//! compiler-enforced: the struct literal cannot be written without it.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_viz::ui::properties::operations::registry::{OP_UI_ROWS, OpDiagram};

#[test]
fn every_operation_has_exactly_one_ui_row_ui05() {
    let mut missing = Vec::new();
    let mut duplicated = Vec::new();

    for &op in OperationType::ALL {
        let count = OP_UI_ROWS.iter().filter(|r| r.op == op).count();
        match count {
            0 => missing.push(format!("{op:?}")),
            1 => {}
            n => duplicated.push(format!("{op:?} ×{n}")),
        }
    }

    assert!(
        missing.is_empty(),
        "these operations have no row in `OP_UI_ROWS`, so the inspector \
         cannot draw them: {}. Add the row beside the others; the row \
         carries the editor, the diagram decision and the validation arm.",
        missing.join(", ")
    );
    assert!(
        duplicated.is_empty(),
        "these operations have more than one row, so `registry::row` picks \
         an arbitrary one: {}",
        duplicated.join(", ")
    );
}

#[test]
fn no_ui_row_names_an_operation_that_is_gone_ui05() {
    for entry in OP_UI_ROWS {
        assert!(
            OperationType::ALL.contains(&entry.op),
            "`OP_UI_ROWS` holds a row for {:?}, which `OperationType::ALL` \
             does not carry.",
            entry.op
        );
    }
    assert_eq!(
        OP_UI_ROWS.len(),
        OperationType::ALL.len(),
        "the table and the enum must be the same length once both arms \
         above hold."
    );
}

#[test]
fn a_blank_diagram_states_its_reason_ui05() {
    let mut blank = Vec::new();
    for entry in OP_UI_ROWS {
        if let Some(reason) = entry.diagram.none_reason() {
            assert!(
                !reason.trim().is_empty(),
                "{:?} declares no diagram but states an EMPTY reason. An \
                 empty reason is the `_ => {{}}` fallback with extra steps.",
                entry.op
            );
            blank.push(format!("{:?}", entry.op));
        }
    }
    // Non-vacuity: if NOTHING declares a blank diagram the arm above never
    // runs, and a future `_ => {}` could return unnoticed.
    assert!(
        !blank.is_empty(),
        "no operation declares a blank diagram. UI-05 recorded one \
         (UnifiedFinish); if a diagram was added for it, delete this arm \
         deliberately rather than leaving it vacuous."
    );
}

#[test]
fn the_diagram_decision_covers_every_operation_ui05() {
    // Every row resolves to one of the three decisions. This is a `match`
    // on purpose: a fourth `OpDiagram` variant must be considered here.
    let mut stepover = 0usize;
    let mut drawn = 0usize;
    let mut blank = 0usize;
    for entry in OP_UI_ROWS {
        match entry.diagram {
            OpDiagram::Stepover => stepover += 1,
            OpDiagram::Draw(_) => drawn += 1,
            OpDiagram::None(_) => blank += 1,
        }
    }
    assert_eq!(
        stepover + drawn + blank,
        OperationType::ALL.len(),
        "every operation must reach exactly one diagram decision"
    );
    assert!(
        stepover > 0 && drawn > 0,
        "the table lost a whole decision kind: {stepover} stepover, \
         {drawn} drawn, {blank} blank"
    );
}
