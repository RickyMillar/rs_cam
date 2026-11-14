//! Feeds contract P30-1 ("print what runs"): the record of the emitted
//! feeds (`ProjectSession::emitted_feeds`) equals the F words of the
//! program that the export door writes, in three states:
//!
//! 1. Before a simulation, with feed optimisation off: the stated feeds.
//! 2. With feed optimisation on (the 2D Adaptive fixture): the dressed
//!    feeds, above and below the stated feed.
//! 3. After a simulation with feed modulation (the Pocket fixture): the
//!    modulated feeds, up to the op ceiling.
//!
//! The test parses the G-code: each fed line (`G1`, `G2`, `G3`) of the op
//! takes the modal F word, and the fed lines pair one to one with the fed
//! moves of the result. The intents of the moves give the classes, so the
//! test holds the class rule of the record as well as its numbers. It also
//! holds that the header of the op holds the lines of the record.
//!
//! `cargo test -p rs_cam_core -q --test the_record_states_what_the_gcode_runs_p30_1`

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::DressupEntryStyle;
use rs_cam_core::compute::operation_configs::{PocketConfig, PocketPattern};
use rs_cam_core::session::{
    EmittedFeeds, FeedStage, ProjectSession, SimulationOptions, SimulationResolution,
};
use rs_cam_core::toolpath::MoveIntent;

mod common;
use common::kind_fixtures::{FLAT_CELL, TOOL_D, adaptive_session, pocket_session};
use common::op_fixtures::{flat_stock, outline_model};
use common::session::single_op_session_with;
use common::tools::endmill_tool_config;

/// The class of a fed move, by its intent: the table of the cut kinds.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Cutting,
    Plunge,
    Ramp,
    Other,
}

const fn class_of(intent: MoveIntent) -> Class {
    match intent {
        MoveIntent::ClearingCut | MoveIntent::FinishingCut | MoveIntent::Unknown => Class::Cutting,
        MoveIntent::EntryPlunge => Class::Plunge,
        MoveIntent::EntryRamp | MoveIntent::EntryHelix => Class::Ramp,
        MoveIntent::Drilling
        | MoveIntent::EntryApproach
        | MoveIntent::Linking
        | MoveIntent::Retract
        | MoveIntent::LeadIn
        | MoveIntent::LeadOut
        | MoveIntent::ObstacleClearance => Class::Other,
    }
}

/// The value of the word `letter` on a line of code, if present.
fn word(code: &str, letter: char) -> Option<f64> {
    code.split_whitespace()
        .find_map(|token| token.strip_prefix(letter))
        .and_then(|value| value.parse().ok())
}

/// The F word of each fed line of the program after the op label.
fn fed_line_feeds(gcode: &str, label: &str) -> Vec<f64> {
    let mut lines = gcode.lines();
    let marker = format!("({label})");
    assert!(
        lines.by_ref().any(|line| line.trim() == marker),
        "the program holds the op label {marker}"
    );
    let mut modal_f: Option<f64> = None;
    let mut out = Vec::new();
    for line in lines {
        let code = rs_cam_core::gcode::strip_comments(line);
        let Some(first) = code.split_whitespace().next() else {
            continue;
        };
        if let Some(f) = word(&code, 'F') {
            modal_f = Some(f);
        }
        if matches!(first, "G1" | "G2" | "G3") {
            out.push(modal_f.expect("a fed line has a modal F word"));
        }
    }
    out
}

/// The record that the G-code of `session` states, built from the parsed
/// F words and the intents of the result.
fn from_gcode(session: &ProjectSession, record: &EmittedFeeds) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let exported = session.export_gcode_checked(None).expect("the op exports");
    let tc = &session.toolpath_configs()[0];
    let feeds = fed_line_feeds(&exported.gcode, &tc.name);
    let moves: Vec<MoveIntent> = session
        .result_at(0)
        .expect("a result")
        .toolpath()
        .moves
        .iter()
        .filter(|m| m.move_type.feed_rate().is_some())
        .map(|m| m.intent)
        .collect();
    assert_eq!(
        feeds.len(),
        moves.len(),
        "each fed move writes one fed line"
    );
    // The header of the op holds the lines of the record, in order, after
    // the label.
    let label_at = exported
        .gcode
        .lines()
        .position(|l| l.trim() == format!("({})", tc.name))
        .expect("the label");
    let header: Vec<String> = exported
        .gcode
        .lines()
        .skip(label_at + 1)
        .take(record.header_lines().len())
        .map(str::to_owned)
        .collect();
    let expected: Vec<String> = record
        .header_lines()
        .iter()
        .map(|l| format!("({l})"))
        .collect();
    assert_eq!(header, expected, "the header lines of the record");
    let mut cutting = Vec::new();
    let mut plunge = Vec::new();
    let mut ramp = Vec::new();
    for (f, intent) in feeds.into_iter().zip(moves) {
        match class_of(intent) {
            Class::Cutting => cutting.push(f),
            Class::Plunge => plunge.push(f),
            Class::Ramp => ramp.push(f),
            Class::Other => {}
        }
    }
    (cutting, plunge, ramp)
}

fn max(v: &[f64]) -> Option<f64> {
    v.iter().copied().reduce(f64::max)
}

fn min(v: &[f64]) -> Option<f64> {
    v.iter().copied().reduce(f64::min)
}

/// The record equals the F words of the program.
fn assert_record_is_the_gcode(session: &ProjectSession, state: &str) -> EmittedFeeds {
    let record = session.emitted_feeds(0).expect("a record");
    let (cutting, plunge, ramp) = from_gcode(session, &record);
    let c = record.cutting.expect("the op has cutting moves");
    assert_eq!(c.moves, cutting.len(), "{state}: cutting move count");
    assert_eq!(Some(c.max_mm_min), max(&cutting), "{state}: largest feed");
    assert_eq!(Some(c.min_mm_min), min(&cutting), "{state}: smallest feed");
    let stated = record.stated_mm_min.expect("a stated feed");
    assert_eq!(
        c.above_stated,
        cutting.iter().filter(|f| **f > stated).count(),
        "{state}: moves above the stated feed"
    );
    assert_eq!(record.plunge.map(|e| e.max_mm_min), max(&plunge), "{state}");
    assert_eq!(record.ramp.map(|e| e.max_mm_min), max(&ramp), "{state}");
    record
}

/// A Pocket with a ramp entry and feed optimisation off, generated and not
/// simulated.
fn stated_pocket() -> ProjectSession {
    let op = OperationConfig::Pocket(PocketConfig {
        pattern: PocketPattern::Contour,
        ..PocketConfig::default()
    });
    let mut session = single_op_session_with(
        flat_stock(),
        endmill_tool_config(TOOL_D),
        outline_model(),
        "pocket",
        op,
        |tc| {
            tc.dressups.entry_style = DressupEntryStyle::Ramp;
            tc.dressups.feed_optimization = false;
        },
    );
    session
        .generate_toolpath(0, &AtomicBool::new(false))
        .expect("the op generates");
    session
}

#[test]
fn before_a_simulation_the_record_is_the_stated_feed_p30_1() {
    let session = stated_pocket();
    assert_eq!(session.simulation_run(), None);
    let record = assert_record_is_the_gcode(&session, "stated");
    assert_eq!(record.stage, FeedStage::Planned);
    assert!(!record.feed_optimised);
    assert!(!record.runs_above_stated(), "{record:?}");
    assert_eq!(record.stage_words(), "as stated");
    // The generator can lower a move under the stated feed (a feed cap,
    // the slot ring); with no feed stage after it, no move runs above it.
    let c = record.cutting.expect("cutting moves");
    assert_eq!(c.above_stated, 0);
    assert_eq!(Some(c.max_mm_min), record.stated_mm_min);
}

#[test]
fn feed_optimisation_feeds_are_the_gcode_feeds_p30_1() {
    let session = adaptive_session();
    let record = assert_record_is_the_gcode(&session, "feed optimisation");
    assert_eq!(record.stage, FeedStage::Planned);
    assert!(record.feed_optimised, "the step runs on a flat 2D op");
    assert_eq!(record.stage_words(), "feed optimisation");
    assert!(record.runs_above_stated(), "{record:?}");
    assert!(
        record.header_lines()[1].ends_with(", set by feed optimisation"),
        "{:?}",
        record.header_lines()
    );
}

#[test]
fn modulated_feeds_are_the_gcode_feeds_p30_1() {
    let mut session = pocket_session(false);
    let before = assert_record_is_the_gcode(&session, "before modulation");
    session
        .run_simulation(
            &SimulationOptions {
                resolution: SimulationResolution::Fixed(FLAT_CELL),
                ..SimulationOptions::default()
            },
            &AtomicBool::new(false),
        )
        .expect("the session simulates");
    let after = assert_record_is_the_gcode(&session, "after modulation");
    let run = session.simulation_run().expect("a run");
    assert_eq!(
        after.stage,
        FeedStage::ModulatedCurrent {
            simulation_run: run
        }
    );
    assert_eq!(after.stage_words(), format!("modulated, run {run}"));
    let (b, a) = (
        before.cutting.expect("cutting"),
        after.cutting.expect("cutting"),
    );
    assert!(
        a.above_stated > b.above_stated && a.max_mm_min > b.max_mm_min,
        "the modulation raises light moves: {b:?} -> {a:?}"
    );
    assert!(
        after.line().starts_with("runs ") && after.line().ends_with(&after.stage_words()),
        "{}",
        after.line()
    );
}
