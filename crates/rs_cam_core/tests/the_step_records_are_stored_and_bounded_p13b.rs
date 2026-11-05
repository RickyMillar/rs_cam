//! Feeds contract P13b (K8, ruling Q7; lead rulings of 2026-11-05 on P13b):
//! the project file stores the step records of each toolpath, the store is
//! bounded, and a typed value ends a chain with a record.
//!
//! The claims, on the pinned rivmap100 project
//! (`fixtures/rivmap100_rough_2026-11-04_7effc804`), after the envelope
//! proposal is applied to each toolpath (`Command::ApplyFeeds`,
//! `PickRequest::Proposal`):
//!
//! - (b) A save and a load keep the records of each toolpath, and the
//!   stored lineage of the loaded project (no result) shows them.
//! - Bounded per toolpath (lead ruling 1, which replaced the file-size
//!   bound): the same apply five times gives the same records and the same
//!   stored bytes: a re-apply replaces, never appends.
//! - Ruling 6 (no silent drop): a typed feed after the apply keeps the feed
//!   chain and adds one final record `stated_by_operator`, from the last
//!   recorded value to the typed value, with the basis "stated".
//!
//! Red at HEAD 255d8db90: the test does not compile
//! (`FeedsProvenance::steps`, `trace::lineage::StepRecords` do not exist).

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::{Path, PathBuf};

use rs_cam_core::feeds::envelope::PickRequest;
use rs_cam_core::session::{ApplyFeedsArgs, Command, ProjectSession, SetToolpathParamArgs};
use rs_cam_core::trace::lineage::{
    BasisRecord, STEP_STATED_BY_OPERATOR, StepQuantity, StepRecords, records_of,
};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/rivmap100_rough_2026-11-04_7effc804/rivmap100.toml")
}

fn proposal(session: &mut ProjectSession, index: usize) {
    let _ = session
        .apply(Command::ApplyFeeds(ApplyFeedsArgs {
            index,
            pick: Some(PickRequest::Proposal),
            explored: None,
        }))
        .expect("the proposal applies");
}

/// The loaded fixture with the proposal applied to every toolpath that
/// takes it.
fn applied() -> ProjectSession {
    let mut session = ProjectSession::load(&fixture()).expect("the fixture loads");
    for index in 0..session.toolpath_count() {
        let _ = session.apply(Command::ApplyFeeds(ApplyFeedsArgs {
            index,
            pick: Some(PickRequest::Proposal),
            explored: None,
        }));
    }
    session
}

fn stored_bytes(steps: &StepRecords) -> String {
    #[derive(serde::Serialize)]
    struct Wrap<'a> {
        steps: &'a StepRecords,
    }
    toml::to_string(&Wrap { steps }).expect("the records serialize")
}

#[test]
fn a_save_and_a_load_keep_the_step_records_p13b() {
    let session = applied();
    let with_records = session
        .toolpath_configs()
        .iter()
        .filter(|tc| !tc.feeds_provenance.steps.is_empty())
        .count();
    assert!(with_records > 0, "the proposal wrote no step record");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("p13b_round_trip");
    std::fs::create_dir_all(&dir).expect("a scratch folder");
    // The model path is relative: save beside a copy of the mesh.
    let src = fixture();
    let src_dir = src.parent().expect("the fixture folder");
    std::fs::copy(src_dir.join("terrain.stl"), dir.join("terrain.stl")).expect("the mesh copies");
    let path = dir.join("rivmap100.toml");
    session.save(&path).expect("the project saves");
    let text = std::fs::read_to_string(&path).expect("the saved file");
    assert!(
        text.contains("feeds_provenance.steps"),
        "the file holds the step table"
    );
    let loaded = ProjectSession::load(&path).expect("the saved project loads");
    for (index, (a, b)) in session
        .toolpath_configs()
        .iter()
        .zip(loaded.toolpath_configs())
        .enumerate()
    {
        assert_eq!(
            a.feeds_provenance.steps, b.feeds_provenance.steps,
            "{}: the records changed in the round trip",
            a.name
        );
        let before = session.lineage_measured(index, None).map(|l| l.steps);
        let after = loaded.lineage_measured(index, None).map(|l| l.steps);
        assert_eq!(before, after, "{}: the stored lineage changed", a.name);
        if !a.feeds_provenance.steps.is_empty() {
            assert!(
                after.is_some_and(|s| !s.is_empty()),
                "{}: the loaded project shows no step record",
                a.name
            );
        }
    }
}

#[test]
fn a_reapply_replaces_the_step_records_p13b() {
    let mut session = ProjectSession::load(&fixture()).expect("the fixture loads");
    let index = 0;
    proposal(&mut session, index);
    let first = session.toolpath_configs()[index]
        .feeds_provenance
        .steps
        .clone();
    let first_bytes = stored_bytes(&first);
    assert!(!first.is_empty(), "the proposal wrote no step record");
    for n in 2..=5 {
        proposal(&mut session, index);
        let steps = &session.toolpath_configs()[index].feeds_provenance.steps;
        assert_eq!(
            steps.as_slice().len(),
            first.as_slice().len(),
            "apply {n}: the record count moved"
        );
        assert_eq!(
            stored_bytes(steps),
            first_bytes,
            "apply {n}: the stored bytes moved"
        );
    }
}

#[test]
fn a_typed_value_ends_the_chain_with_a_record_p13b() {
    let mut session = ProjectSession::load(&fixture()).expect("the fixture loads");
    let index = 0;
    proposal(&mut session, index);
    let before = session
        .lineage_measured(index, None)
        .expect("a lineage")
        .steps;
    let feed_chain: Vec<_> = records_of(&before, StepQuantity::Feed).cloned().collect();
    let last = feed_chain.last().expect("a feed chain").value_out;
    let typed = (last * 0.9).round();
    let _ = session
        .apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index,
            param: "feed_rate".to_owned(),
            value: serde_json::json!(typed),
        }))
        .expect("the feed edit");
    let after = session
        .lineage_measured(index, None)
        .expect("a lineage")
        .steps;
    let chain: Vec<_> = records_of(&after, StepQuantity::Feed).cloned().collect();
    assert_eq!(
        chain.len(),
        feed_chain.len() + 1,
        "the chain stays and gains one record"
    );
    assert_eq!(&chain[..feed_chain.len()], feed_chain.as_slice());
    let end = chain.last().expect("the final record");
    assert_eq!(end.step, STEP_STATED_BY_OPERATOR);
    assert_eq!(end.value_in, Some(last));
    assert_eq!(end.value_out, typed);
    assert_eq!(
        end.basis,
        BasisRecord::Operator {
            law: "stated".to_owned()
        }
    );
    // The store is not written by the view.
    let stored = &session.toolpath_configs()[index].feeds_provenance.steps;
    assert!(
        !stored
            .as_slice()
            .iter()
            .any(|r| r.step == STEP_STATED_BY_OPERATOR),
        "the lineage view wrote to the store"
    );
}
