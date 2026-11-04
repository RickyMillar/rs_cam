//! K5 ReachFlute (feeds contract P7): a cut deeper below the top than the
//! flute is long warns, with no mesh.
//!
//! The case is op 3 of the blind review (report 04 (a)): the 6 mm 2F flat
//! end mill (flute 25 mm, estimated; stickout 30 mm, estimated) drills the
//! flip pins 30 mm deep, the stock 25.5 plus 4.5 into the spoilboard. Before
//! K5 no check fired: every flute check read a per-pass depth or a mesh, and
//! op 3 has neither. The pre-simulation row reads the emitted moves and the
//! stock box. Ruling Q6: generation and export always give output.
//!
//! The simulation measure (`SimulationResult::flute_overreach`) is commit B
//! of P7; this file holds the pre-simulation row.
//!
//! Red: make `ReachFlute` give no row (its predicate `false`). The op 3
//! case then has no warning.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use common::k5_cases::{FLAT_6_STICKOUT_MM, export, export_names, op3_generated, rule_texts};
use rs_cam_core::diagnostics::ids;

/// The hole depth of op 3 (report 04 (b)): `(stock max z - stock min z) +
/// spoilboard_penetration` = 25.5 + 4.5.
const OP3_HOLE_MM: f64 = 25.5 + 4.5;

#[test]
fn a_pin_hole_past_the_flute_warns_with_no_mesh() {
    let session = op3_generated(FLAT_6_STICKOUT_MM);
    assert!(
        !session
            .result_at(0)
            .expect("a result")
            .toolpath()
            .moves
            .is_empty(),
        "op 3 generates (Q6)"
    );
    let texts = rule_texts(&session, 0, ids::TOOL_REACH_PAST_FLUTE);
    assert_eq!(texts.len(), 1, "one ReachFlute row: {texts:?}");
    let text = texts.first().expect("one row");
    assert!(
        text.contains("flute 25 mm, estimated") && text.contains("cuts 30 mm below the top"),
        "the row names the flute, how it is known, and the depth: {text}"
    );
    let row = session
        .result_at(0)
        .and_then(|r| r.stats.feasibility.as_ref())
        .and_then(|f| {
            f.rows()
                .iter()
                .find(|r| r.id() == ids::TOOL_REACH_PAST_FLUTE)
        })
        .expect("the row");
    assert_eq!(row.measured(), OP3_HOLE_MM, "the hole depth");
    assert_eq!(row.limit(), Some(25.0), "the flute length of tool 0");

    let exported = export(&session);
    assert!(
        !exported.gcode.is_empty(),
        "the export writes a program (Q6)"
    );
    assert!(
        export_names(&exported, ids::TOOL_REACH_PAST_FLUTE),
        "the export and the header name the warning: {:?}",
        exported.warnings
    );
}
