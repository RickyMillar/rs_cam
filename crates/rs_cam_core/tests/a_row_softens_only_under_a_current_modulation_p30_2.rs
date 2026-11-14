//! Feeds contract P30-2 ("a row does not soften for a stage that did not
//! run", lead ruling 2026-11-14). A Pocket whose stated feed puts the chip
//! over the band maximum:
//!
//! 1. With a planned result (no simulation), its stated chip-maximum row of
//!    the body keeps the danger tone (`Tone::Exceeds`) and carries
//!    `Caveat::UntilModulated { export: Stated }`; the export carries one
//!    `export.stated_cut_over_limit` warning (the result list and a header
//!    line), and the program is still written (a warning, never a refusal).
//! 2. After a simulation with feed modulation, the result is modulated: the
//!    same row carries `Caveat::BeforeModulation`, takes the caution tone,
//!    and the export carries no such warning.
//! 3. A simulation with modulation off on a fresh result keeps the planned
//!    stage, so the row keeps its danger tone. Review 2026-11-15 F4: the
//!    simulation is fresh for the op, so a measured verdict judges the cut:
//!    the export names the measured chip row (`load.chipload.*`), not the
//!    stated row.
//!
//! The door (`ProjectSession::limit_rows`) takes no modulation argument: it
//! reads `ProjectSession::run_modulation_of`, so the states above come from
//! the session alone.
//!
//! `cargo test -p rs_cam_core -q --test a_row_softens_only_under_a_current_modulation_p30_2`

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::DressupEntryStyle;
use rs_cam_core::compute::operation_configs::{PocketConfig, PocketPattern};
use rs_cam_core::diagnostics::ids;
use rs_cam_core::dressup::feed_modulation::RunModulation;
use rs_cam_core::limits::{Caveat, CheckRow, CutKind, ExportFeed, LimitId, Subject, Tone};
use rs_cam_core::session::{FeedStage, ProjectSession, SimulationOptions, SimulationResolution};

mod common;
use common::kind_fixtures::{FLAT_CELL, TOOL_D};
use common::op_fixtures::{flat_stock, outline_model};
use common::session::single_op_session_with;
use common::tools::endmill_tool_config;

/// A stated feed far over the chip band of the fixture tool.
const FAST_FEED: f64 = 9000.0;

fn fast_pocket() -> ProjectSession {
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
            tc.operation.set_feed_rate(FAST_FEED);
        },
    );
    session
        .generate_toolpath(0, &AtomicBool::new(false))
        .expect("the op generates");
    session
}

fn simulate(session: &mut ProjectSession, modulate: bool) {
    session
        .run_simulation(
            &SimulationOptions {
                resolution: SimulationResolution::Fixed(FLAT_CELL),
                adaptive_feed_modulation: modulate,
                ..SimulationOptions::default()
            },
            &AtomicBool::new(false),
        )
        .expect("the session simulates");
}

/// The stated chip-maximum row of the body cut.
fn body_chip_max(session: &ProjectSession) -> CheckRow {
    session
        .limit_rows(0, None)
        .expect("the op")
        .envelope()
        .expect("a tool")
        .expect("the envelope builds")
        .into_iter()
        .find(|r| {
            r.check().limit == LimitId::ChipMax
                && matches!(
                    r.subject(),
                    Subject::StatedCut {
                        cut: CutKind::Body,
                        ..
                    }
                )
        })
        .expect("the stated body chip row")
}

/// The ids and the header of an export of the session.
fn export_names_the_warning(session: &ProjectSession) -> bool {
    let trace = session
        .simulation_result()
        .and_then(|s| s.cut_trace.clone());
    let exported = session
        .export_gcode_checked(trace.as_deref())
        .expect("a warning never refuses the export");
    let in_list = exported
        .warnings
        .iter()
        .any(|w| w.id == ids::EXPORT_STATED_CUT_OVER_LIMIT);
    let in_header = exported
        .gcode
        .lines()
        .any(|l| l.contains("WARNING") && l.contains(ids::EXPORT_STATED_CUT_OVER_LIMIT));
    assert_eq!(in_list, in_header, "the list and the header agree");
    in_list
}

#[test]
fn a_planned_result_keeps_the_danger_tone_and_warns_p30_2() {
    let session = fast_pocket();
    assert_eq!(session.feed_stage(0), Some(FeedStage::Planned));
    assert_eq!(
        session.run_modulation_of(0),
        RunModulation::NotRun {
            export: ExportFeed::Stated
        }
    );
    let row = body_chip_max(&session);
    assert_eq!(row.tone(), Tone::Exceeds, "{}", row.face());
    assert!(
        row.caveats().contains(&Caveat::UntilModulated {
            export: ExportFeed::Stated
        }),
        "{:?}",
        row.caveats()
    );
    assert!(row.face().ends_with("until modulated"), "{}", row.face());
    assert!(
        row.detail()
            .contains("an export now carries the stated feed"),
        "{}",
        row.detail()
    );
    assert!(
        session.stated_cut_over_a_limit(0).is_some(),
        "the export door reads the row"
    );
    assert!(export_names_the_warning(&session), "the export warns");
}

#[test]
fn a_current_modulated_result_softens_and_does_not_warn_p30_2() {
    let mut session = fast_pocket();
    simulate(&mut session, true);
    assert!(
        matches!(
            session.feed_stage(0),
            Some(FeedStage::ModulatedCurrent { .. })
        ),
        "{:?}",
        session.feed_stage(0)
    );
    assert!(matches!(
        session.run_modulation_of(0),
        RunModulation::Current { .. }
    ));
    let row = body_chip_max(&session);
    assert_eq!(row.tone(), Tone::ExceedsAdvisory, "{}", row.face());
    assert!(row.caveats().contains(&Caveat::BeforeModulation));
    assert!(
        !export_names_the_warning(&session),
        "no warning when modulated"
    );
}

/// Review 2026-11-15 F5 (c): a proposal row does not soften under the
/// current modulation: an apply of the proposal changes the op, so the
/// export carries the proposal feed with no modulation until a new run.
#[test]
fn a_proposal_row_does_not_soften_under_the_current_modulation_review_f5() {
    let mut session = fast_pocket();
    simulate(&mut session, true);
    assert!(matches!(
        session.run_modulation_of(0),
        RunModulation::Current { .. }
    ));
    let rows = session
        .limit_rows(0, None)
        .expect("the op")
        .envelope()
        .expect("a tool")
        .expect("the envelope builds");
    let proposal = rows
        .iter()
        .find(|r| {
            r.check().limit == LimitId::ChipMax
                && matches!(
                    r.subject(),
                    Subject::Planned {
                        cut: CutKind::Body,
                        ..
                    }
                )
        })
        .expect("the proposal body chip row");
    assert!(
        proposal.holds_a_number(),
        "non-vacuity: the row holds a number: {}",
        proposal.face()
    );
    assert!(
        !proposal.caveats().contains(&Caveat::BeforeModulation),
        "{:?}",
        proposal.caveats()
    );
    assert!(
        proposal.caveats().contains(&Caveat::UntilModulated {
            export: ExportFeed::Stated
        }),
        "{:?}",
        proposal.caveats()
    );
    // The stated row of the same cut keeps the softening caveat.
    let stated = body_chip_max(&session);
    assert!(stated.caveats().contains(&Caveat::BeforeModulation));
}

#[test]
fn a_simulation_with_modulation_off_warns_by_the_measured_row_p30_2() {
    let mut session = fast_pocket();
    simulate(&mut session, false);
    assert_eq!(session.feed_stage(0), Some(FeedStage::Planned));
    let row = body_chip_max(&session);
    assert_eq!(row.tone(), Tone::Exceeds, "{}", row.face());
    assert!(
        !export_names_the_warning(&session),
        "a current measured verdict judges the cut"
    );
    let trace = session
        .simulation_result()
        .and_then(|s| s.cut_trace.clone());
    let exported = session
        .export_gcode_checked(trace.as_deref())
        .expect("a warning never refuses the export");
    assert!(
        exported
            .warnings
            .iter()
            .any(|w| w.id.starts_with("load.chipload.")),
        "the measured chip row warns: {:?}",
        exported.warnings.iter().map(|w| w.id).collect::<Vec<_>>()
    );
}
