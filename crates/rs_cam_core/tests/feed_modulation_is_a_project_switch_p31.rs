//! Operator ruling 2026-11-15 (P31 C): "Can we have it on by default. but
//! shown as a warn. But have option to disable it?"
//!
//! P32 (operator ruling of the same day, "Cap only"): the switch has three
//! settings, `off`, `cap_only` (the default) and `full`. This file keeps
//! the P31 claims for the setting that P31 called "on", which is `full`
//! now; each test states it. `feed_modulation_has_three_modes_p32` holds
//! the claims of cap only and of a change between any two settings.
//!
//! 1. **Full.** A simulation modulates the feeds, and an export of an op
//!    that runs a cutting move above the stated feed carries one warning,
//!    `export.feed_above_stated`, in the warning list and in the header.
//!    The text is the text of `EmittedFeeds::above_stated_warning`.
//! 2. **Off.** `Command::SetFeedModulation` with the switch off drops the
//!    simulation, puts the dressed path back in each modulated result
//!    (`Effects::modulated` names them), and the result reads
//!    `FeedStage::Planned` at once. A simulation then changes no feed: no F
//!    word of a cutting move is above the stated feed, no entry F word is
//!    above its stated feed, the header says "feeds: as planned, not
//!    modulated", and the export has no `export.feed_above_stated` warning.
//!    The limit rows read `RunModulation::Off`.
//! 3. **Back to full.** The command drops the simulation again; the result
//!    stays planned and its rows read "until modulated" until the next
//!    simulation modulates it.
//! 4. **The project file.** A file with no key reads as cap only; the
//!    writer leaves the key out at cap only and writes
//!    `feed_modulation = "off"` or `"full"`. A boolean value (the P31 form)
//!    refuses the load, and the message names the three words.
//!
//! The test parses the G-code: each fed line (`G1`, `G2`, `G3`) takes the
//! modal F word, and the fed lines pair one to one with the fed moves of the
//! result, whose intents give the class of each move.
//!
//! `cargo test -p rs_cam_core -q --test feed_modulation_is_a_project_switch_p31`

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::collections::BTreeSet;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::DressupEntryStyle;
use rs_cam_core::compute::operation_configs::{PocketConfig, PocketPattern};
use rs_cam_core::diagnostics::ids;
use rs_cam_core::dressup::feed_modulation::RunModulation;
use rs_cam_core::limits::ExportFeed;
use rs_cam_core::session::{
    Command, FeedModulationSwitch, FeedStage, ProjectSession, SetFeedModulationArgs,
    SimulationDropCause, SimulationOptions, SimulationResolution,
};
use rs_cam_core::toolpath::MoveIntent;

mod common;
use common::kind_fixtures::{FLAT_CELL, TOOL_D};
use common::op_fixtures::{flat_stock, outline_model};
use common::session::single_op_session_with;
use common::tools::endmill_tool_config;

/// The stated feeds of the fixture op (the fixture values of
/// `common::session`): the cutting feed and the plunge feed. The op states
/// no ramp feed, so the stated ramp feed is the plunge feed.
const STATED_FEED: f64 = 1000.0;
const STATED_PLUNGE: f64 = 500.0;

/// A Pocket with a ramp entry and feed optimisation off, generated and not
/// simulated: every generated feed is at or below its stated feed.
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

fn simulate(session: &mut ProjectSession) {
    session
        .run_simulation(
            &SimulationOptions {
                resolution: SimulationResolution::Fixed(FLAT_CELL),
                ..SimulationOptions::default()
            },
            &AtomicBool::new(false),
        )
        .expect("the session simulates");
}

fn set_switch(session: &mut ProjectSession, switch: FeedModulationSwitch) -> BTreeSet<usize> {
    session
        .apply(Command::SetFeedModulation(SetFeedModulationArgs { switch }))
        .expect("the command applies")
        .modulated
}

/// The value of the word `letter` on a line of code, if present.
fn word(code: &str, letter: char) -> Option<f64> {
    code.split_whitespace()
        .find_map(|token| token.strip_prefix(letter))
        .and_then(|value| value.parse().ok())
}

/// The F words of the program of the session, by class: the cutting moves
/// and the entries (plunge, ramp, helix). The warning lines of the header
/// and the stage line of the op come with them.
struct Program {
    cutting: Vec<f64>,
    entries: Vec<f64>,
    warning_ids: Vec<&'static str>,
    warning_details: Vec<String>,
    gcode: String,
}

fn program(session: &ProjectSession) -> Program {
    let trace = session
        .simulation_result()
        .and_then(|s| s.cut_trace.clone());
    let exported = session
        .export_gcode_checked(trace.as_deref())
        .expect("a warning never refuses the export");
    let mut lines = exported.gcode.lines();
    assert!(
        lines.by_ref().any(|line| line.trim() == "(pocket)"),
        "the program holds the op label"
    );
    let mut modal_f: Option<f64> = None;
    let mut fed = Vec::new();
    for line in lines {
        let code = rs_cam_core::gcode::strip_comments(line);
        let Some(first) = code.split_whitespace().next() else {
            continue;
        };
        if let Some(f) = word(&code, 'F') {
            modal_f = Some(f);
        }
        if matches!(first, "G1" | "G2" | "G3") {
            fed.push(modal_f.expect("a fed line has a modal F word"));
        }
    }
    let intents: Vec<MoveIntent> = session
        .result_at(0)
        .expect("a result")
        .toolpath()
        .moves
        .iter()
        .filter(|m| m.move_type.feed_rate().is_some())
        .map(|m| m.intent)
        .collect();
    assert_eq!(
        fed.len(),
        intents.len(),
        "each fed move writes one fed line"
    );
    let mut cutting = Vec::new();
    let mut entries = Vec::new();
    for (f, intent) in fed.into_iter().zip(intents) {
        match intent {
            MoveIntent::ClearingCut | MoveIntent::FinishingCut | MoveIntent::Unknown => {
                cutting.push(f);
            }
            MoveIntent::EntryPlunge | MoveIntent::EntryRamp | MoveIntent::EntryHelix => {
                entries.push(f);
            }
            MoveIntent::Drilling
            | MoveIntent::EntryApproach
            | MoveIntent::Linking
            | MoveIntent::Retract
            | MoveIntent::LeadIn
            | MoveIntent::LeadOut
            | MoveIntent::ObstacleClearance => {}
        }
    }
    assert!(!cutting.is_empty() && !entries.is_empty(), "non-vacuity");
    let feed_warnings: Vec<_> = exported
        .warnings
        .iter()
        .filter(|w| w.id == ids::EXPORT_FEED_ABOVE_STATED)
        .collect();
    Program {
        cutting,
        entries,
        warning_ids: feed_warnings.iter().map(|w| w.id).collect(),
        warning_details: feed_warnings.iter().map(|w| w.detail.clone()).collect(),
        gcode: exported.gcode,
    }
}

fn above(feeds: &[f64], stated: f64) -> usize {
    feeds.iter().filter(|f| **f > stated).count()
}

fn header_warns(gcode: &str) -> bool {
    gcode
        .lines()
        .any(|l| l.contains("WARNING") && l.contains(ids::EXPORT_FEED_ABOVE_STATED))
}

#[test]
fn in_full_a_feed_above_the_stated_feed_warns_p31() {
    let mut session = stated_pocket();
    // P32: the default is cap only; this test states full.
    assert_eq!(session.feed_modulation(), FeedModulationSwitch::CapOnly);
    let _ = set_switch(&mut session, FeedModulationSwitch::Full);
    // Before the simulation every feed is at or below its stated feed: no
    // warning.
    let before = program(&session);
    assert_eq!(above(&before.cutting, STATED_FEED), 0);
    assert_eq!(above(&before.entries, STATED_PLUNGE), 0);
    assert!(
        before.warning_ids.is_empty(),
        "{:?}",
        before.warning_details
    );
    assert!(!header_warns(&before.gcode));

    simulate(&mut session);
    let run = session.simulation_run().expect("a run");
    assert_eq!(
        session.feed_stage(0),
        Some(FeedStage::ModulatedCurrent {
            simulation_run: run
        })
    );
    let after = program(&session);
    let cutting_above = above(&after.cutting, STATED_FEED);
    assert!(
        cutting_above > 0,
        "non-vacuity: the modulation raises a move"
    );
    let max_cutting = after.cutting.iter().copied().fold(0.0_f64, f64::max);
    let max_entry = after.entries.iter().copied().fold(0.0_f64, f64::max);
    // The text, from the parsed F words (not from the record).
    let mut expected = format!(
        "Toolpath 'pocket': runs up to {max_cutting} mm/min, the stated feed is {STATED_FEED} \
         mm/min ({cutting_above} of {} cutting moves above)",
        after.cutting.len()
    );
    if max_entry > STATED_PLUNGE {
        expected.push_str(&format!(
            "; ramp entries run up to {max_entry} mm/min, the stated ramp feed is \
             {STATED_PLUNGE} mm/min"
        ));
    }
    expected.push_str(&format!("; feed modulation of simulation run {run}"));
    assert_eq!(after.warning_details, vec![expected]);
    assert!(header_warns(&after.gcode), "the header names the warning");
    assert!(
        after
            .gcode
            .lines()
            .any(|l| l.trim() == format!("(feeds: modulated from simulation run {run})")),
        "the stage line"
    );
    // The record gives the same text to every surface.
    let record = session.emitted_feeds(0).expect("a record");
    assert_eq!(
        record
            .above_stated_warning()
            .map(|text| format!("Toolpath 'pocket': {text}")),
        after.warning_details.first().cloned()
    );
}

#[test]
fn the_switch_off_puts_the_planned_feeds_back_and_no_run_modulates_p31() {
    let mut session = stated_pocket();
    let _ = set_switch(&mut session, FeedModulationSwitch::Full);
    let planned = program(&session);
    simulate(&mut session);
    assert!(matches!(
        session.feed_stage(0),
        Some(FeedStage::ModulatedCurrent { .. })
    ));
    let epoch = session.simulation_epoch();

    // Off: the simulation drops, the modulated result takes its dressed
    // path back, and the stage is planned at once.
    let restored = set_switch(&mut session, FeedModulationSwitch::Off);
    assert_eq!(restored, BTreeSet::from([0]));
    assert_eq!(session.feed_modulation(), FeedModulationSwitch::Off);
    assert!(
        session.simulation_result().is_none(),
        "the simulation drops"
    );
    assert!(
        session
            .simulation_drop_causes_since(epoch)
            .contains(SimulationDropCause::FeedModulation),
        "the drop names its cause"
    );
    assert_eq!(session.feed_stage(0), Some(FeedStage::Planned));
    assert_eq!(session.run_modulation_of(0), RunModulation::Off);
    let off = program(&session);
    assert_eq!(off.cutting, planned.cutting, "the planned cutting feeds");
    assert_eq!(off.entries, planned.entries, "the planned entry feeds");

    // A simulation with the switch off changes no feed.
    simulate(&mut session);
    assert_eq!(session.feed_stage(0), Some(FeedStage::Planned));
    let off_run = program(&session);
    assert_eq!(off_run.cutting, planned.cutting);
    assert_eq!(off_run.entries, planned.entries);
    assert_eq!(above(&off_run.cutting, STATED_FEED), 0);
    assert_eq!(above(&off_run.entries, STATED_PLUNGE), 0);
    assert!(
        off_run.warning_ids.is_empty(),
        "{:?}",
        off_run.warning_details
    );
    assert!(!header_warns(&off_run.gcode));
    assert!(
        off_run
            .gcode
            .lines()
            .any(|l| l.trim() == "(feeds: as planned, not modulated)"),
        "the stage line"
    );
    assert_eq!(session.run_modulation_of(0), RunModulation::Off);

    // The same value again changes nothing and drops nothing.
    let again = set_switch(&mut session, FeedModulationSwitch::Off);
    assert!(again.is_empty());
    assert!(session.simulation_result().is_some());
}

#[test]
fn the_switch_back_to_full_reads_not_run_until_the_next_simulation_p31() {
    let mut session = stated_pocket();
    let _ = set_switch(&mut session, FeedModulationSwitch::Off);
    simulate(&mut session);
    assert_eq!(session.feed_stage(0), Some(FeedStage::Planned));

    let restored = set_switch(&mut session, FeedModulationSwitch::Full);
    assert!(restored.is_empty(), "no result holds a modulated path");
    assert!(
        session.simulation_result().is_none(),
        "the simulation drops"
    );
    // The stale state: the result is planned and its rows say that the next
    // simulation modulates it.
    assert_eq!(session.feed_stage(0), Some(FeedStage::Planned));
    assert_eq!(
        session.run_modulation_of(0),
        RunModulation::NotRun {
            export: ExportFeed::Stated
        }
    );
    simulate(&mut session);
    assert!(matches!(
        session.feed_stage(0),
        Some(FeedStage::ModulatedCurrent { .. })
    ));
    assert!(matches!(
        session.run_modulation_of(0),
        RunModulation::Current { .. }
    ));
    let on = program(&session);
    assert!(above(&on.cutting, STATED_FEED) > 0);
    assert_eq!(on.warning_ids, vec![ids::EXPORT_FEED_ABOVE_STATED]);
}

#[test]
fn the_project_file_keeps_the_switch_p31() {
    let dir = std::env::temp_dir().join(format!("rs_cam_p31_switch_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a temp dir");
    let path = dir.join("switch.toml");
    let mut session = ProjectSession::new_empty();
    assert_eq!(session.feed_modulation(), FeedModulationSwitch::CapOnly);

    // Cap only (the default): the writer leaves the key out, and a file
    // with no key reads cap only.
    session.save(&path).expect("the project saves");
    let text = std::fs::read_to_string(&path).expect("the file");
    assert!(!text.contains("feed_modulation"), "{text}");
    let loaded = ProjectSession::load(&path).expect("the project loads");
    assert_eq!(loaded.feed_modulation(), FeedModulationSwitch::CapOnly);

    // Off and full: the key is written as the wire word and read back.
    for (switch, word) in [
        (FeedModulationSwitch::Off, "off"),
        (FeedModulationSwitch::Full, "full"),
    ] {
        assert_eq!(switch.word(), word);
        let _ = set_switch(&mut session, switch);
        session.save(&path).expect("the project saves");
        let text = std::fs::read_to_string(&path).expect("the file");
        assert!(
            text.lines()
                .any(|l| l.trim() == format!("feed_modulation = \"{word}\"")),
            "{text}"
        );
        let loaded = ProjectSession::load(&path).expect("the project loads");
        assert_eq!(loaded.feed_modulation(), switch);

        // The boolean form of P31 refuses, and the message names the words
        // (the break of P32: no alias).
        let boolean = text.replace(
            &format!("feed_modulation = \"{word}\""),
            "feed_modulation = false",
        );
        let bool_path = dir.join("boolean.toml");
        std::fs::write(&bool_path, boolean).expect("the file writes");
        let error = match ProjectSession::load(&bool_path) {
            Ok(_) => panic!("a boolean feed_modulation must refuse the load"),
            Err(error) => error.to_string(),
        };
        assert!(
            error.contains("\"off\", \"cap_only\" or \"full\"") && error.contains("no longer read"),
            "{error}"
        );
    }
    // The word "cap_only" reads too (a hand-written file).
    let cap_only = std::fs::read_to_string(&path).expect("the file").replace(
        "feed_modulation = \"full\"",
        "feed_modulation = \"cap_only\"",
    );
    std::fs::write(&path, cap_only).expect("the file writes");
    let loaded = ProjectSession::load(&path).expect("the project loads");
    assert_eq!(loaded.feed_modulation(), FeedModulationSwitch::CapOnly);

    // Back to cap only: the key goes again.
    let _ = set_switch(&mut session, FeedModulationSwitch::CapOnly);
    session.save(&path).expect("the project saves");
    let text = std::fs::read_to_string(&path).expect("the file");
    assert!(!text.contains("feed_modulation"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}
