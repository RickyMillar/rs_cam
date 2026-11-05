//! Feeds contract P13b (STATUS D13 item 7, operator 2026-11-05: "Lineage:
//! store it in the project file, if it is not a memory sink. Measure the
//! size first.").
//!
//! The project file stores the step records of each toolpath
//! (`feeds_provenance.steps`). This sentry applies the envelope proposal
//! (`Command::ApplyFeeds` with `PickRequest::Proposal`, the write that sets
//! the most records: the pick, the speeds and the funnel moves) to every
//! toolpath of two operator projects, saves the project, and measures:
//!
//! - the TOML size of the records of each toolpath, nested as the project
//!   file would nest them (`[[setups.toolpaths.feeds_provenance.steps]]`),
//!   in three forms: the whole basis tree, the top node of the basis only,
//!   and the basis as one JSON string;
//! - the size of the saved file with no records.
//!
//! The card bound is 64 KiB per toolpath (asserted) and the file's own size
//! per project (printed: the whole-tree form exceeds it on both projects,
//! so P13b stopped before it persisted the records). The measured numbers
//! are in `planning/feeds_contract_2026-11-03/phases/P13.md` "Result P13b".
//! Run with `-- --ignored --nocapture`.

#![expect(
    clippy::expect_used,
    clippy::print_stderr,
    reason = "test code: a panic is how a test reports a failure; the test prints its measurement"
)]

use std::path::{Path, PathBuf};

use rs_cam_core::feeds::envelope::PickRequest;
use rs_cam_core::session::{ApplyFeedsArgs, Command, ProjectSession};
use rs_cam_core::trace::lineage::StepRecord;

/// The bound of one toolpath's records (the P13b card).
const TOOLPATH_BOUND_BYTES: usize = 64 * 1024;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The nesting of the records in the project file, so the TOML table
/// headers have their saved length.
#[derive(serde::Serialize)]
struct InFile<T: serde::Serialize> {
    setups: Vec<InSetup<T>>,
}
#[derive(serde::Serialize)]
struct InSetup<T: serde::Serialize> {
    toolpaths: Vec<InToolpath<T>>,
}
#[derive(serde::Serialize)]
struct InToolpath<T: serde::Serialize> {
    feeds_provenance: InProv<T>,
}
#[derive(serde::Serialize)]
struct InProv<T: serde::Serialize> {
    steps: T,
}

fn in_file_bytes<T: serde::Serialize>(steps: T) -> usize {
    let v = InFile {
        setups: vec![InSetup {
            toolpaths: vec![InToolpath {
                feeds_provenance: InProv { steps },
            }],
        }],
    };
    toml::to_string_pretty(&v)
        .expect("the records serialize")
        .len()
}

/// Variant A: the basis of each record keeps only its top node (the law
/// of a derived basis, no `from` tree).
fn shallow(r: &StepRecord) -> StepRecord {
    use rs_cam_core::trace::lineage::BasisRecord;
    let mut r = r.clone();
    r.basis = match r.basis {
        BasisRecord::Derived { law, .. } => BasisRecord::Derived {
            law,
            from: Vec::new(),
        },
        BasisRecord::ToolFact {
            fact, fact_kind, ..
        } => BasisRecord::ToolFact {
            fact,
            fact_kind,
            legacy_row: None,
        },
        other @ (BasisRecord::Operator { .. }
        | BasisRecord::Printed { .. }
        | BasisRecord::Claim { .. }
        | BasisRecord::Rule { .. }
        | BasisRecord::Machine { .. }) => other,
    };
    r
}

/// Variant B: one record per line, the basis as one JSON string.
#[derive(serde::Serialize)]
struct Flat {
    step: String,
    quantity: String,
    value_in: Option<f64>,
    value_out: f64,
    basis: String,
}

fn flat(r: &StepRecord) -> Flat {
    Flat {
        step: r.step.clone(),
        quantity: format!("{:?}", r.quantity),
        value_in: r.value_in,
        value_out: r.value_out,
        basis: serde_json::to_string(&r.basis).expect("json"),
    }
}

fn saved_size(session: &ProjectSession, dir: &Path, name: &str) -> usize {
    let path = dir.join(name);
    session.save(&path).expect("the project saves");
    let n = std::fs::metadata(&path).expect("the saved file").len();
    usize::try_from(n).expect("a file size fits usize")
}

/// The sizes of one toolpath's records (B): the whole tree, the top node,
/// the JSON string.
struct Sizes {
    name: String,
    records: usize,
    /// Form C, the stored form: the basis table and the records.
    table: usize,
    /// The basis nodes of form C.
    nodes: usize,
    full: usize,
    shallow: usize,
    json: usize,
}

/// Apply the proposal to every toolpath; return the saved file size and
/// the sizes per toolpath.
fn measure(project: &str) -> (usize, usize, Vec<Sizes>) {
    let path = repo_root().join(project);
    let mut session = ProjectSession::load(&path).expect("the project loads");
    let dir =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("rs_cam_p13b_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch folder");
    let file = saved_size(&session, &dir, "before.toml");
    for index in 0..session.toolpath_count() {
        // A refused pairing writes nothing; the measurement counts it as 0.
        let _ = session.apply(Command::ApplyFeeds(ApplyFeedsArgs {
            index,
            pick: Some(PickRequest::Proposal),
            explored: None,
        }));
    }
    let saved = saved_size(&session, &dir, "after.toml");
    let _ = std::fs::remove_dir_all(&dir);
    let rows = session
        .toolpath_configs()
        .iter()
        .map(|tc| {
            let stored = &tc.feeds_provenance.steps;
            let steps = stored.as_slice();
            let a: Vec<StepRecord> = steps.iter().map(shallow).collect();
            let b: Vec<Flat> = steps.iter().map(flat).collect();
            Sizes {
                name: tc.name.clone(),
                records: steps.len(),
                table: in_file_bytes(stored),
                nodes: stored.to_table().nodes.len(),
                full: in_file_bytes(steps),
                shallow: in_file_bytes(&a),
                json: in_file_bytes(&b),
            }
        })
        .collect();
    (file, saved, rows)
}

fn check(project: &str) {
    let (file, saved, rows) = measure(project);
    let sum = |f: fn(&Sizes) -> usize| rows.iter().map(f).sum::<usize>();
    eprintln!(
        "{project}: file {file} B, saved with records {saved} B; records {}, nodes {}; added: table (C) {} B, full {} B, shallow {} B, json {} B",
        sum(|r| r.records),
        sum(|r| r.nodes),
        sum(|r| r.table),
        sum(|r| r.full),
        sum(|r| r.shallow),
        sum(|r| r.json),
    );
    for r in &rows {
        eprintln!(
            "  {}: {} records, {} nodes, table (C) {} B, full {} B, shallow {} B, json {} B",
            r.name, r.records, r.nodes, r.table, r.full, r.shallow, r.json
        );
    }
    assert!(
        sum(|r| r.records) > 0,
        "{project}: the proposal wrote no step record"
    );
    for r in &rows {
        assert!(
            r.table <= TOOLPATH_BOUND_BYTES,
            "{project} / {}: {} B of records, bound {TOOLPATH_BOUND_BYTES} B",
            r.name,
            r.table
        );
    }
}

#[test]
#[ignore = "evidence: reads the operator projects under planning/ (P13b size measurement)"]
fn rivmap100_step_records_stay_small_p13b() {
    check("planning/fixtures/rivmap100/rivmap100.toml");
}

#[test]
#[ignore = "evidence: reads the operator projects under planning/ (P13b size measurement)"]
fn nz_wanaka_21_step_records_stay_small_p13b() {
    check("planning/rivmap_block_job/nz_wanaka_21.toml");
}
