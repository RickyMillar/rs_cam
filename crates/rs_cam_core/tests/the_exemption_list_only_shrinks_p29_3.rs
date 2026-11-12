//! Feeds contract P29-3, commit A
//! (`planning/feeds_contract_2026-11-03/phases/P29-3_DESIGN.md` section
//! 4.2): every exemption from a limit is a row of one list, and the list
//! only shrinks.
//!
//! - Claim 1: the count of rows that need a ruling is exactly
//!   `MAX_NEEDS_RULING`. A closed row lowers the pin in the same commit; a
//!   new row raises it on purpose. The row count and the count of each
//!   class are pinned too.
//! - Claim 2: every physical row names a `Geometry` reason. The type holds
//!   this (`ExemptionClass::Physical(Geometry)`); the test reads the reason
//!   of each row, so the claim has no fault to inject.
//! - Claim 3: every ruled row quotes the ruling, names who ruled and gives
//!   a date `YYYY-MM-DD`.
//! - Claim 4: every id is unique, and the 34 ids of `P28_limit_audit.md`
//!   and the 12 ids M1 to M12 of `P29_parts/A_cuts_and_limits.md` section
//!   3.3 are in the list, or in `CLOSED` with the step that closed them,
//!   never both. The id lists are constants here.
//! - Claim 5: the link between the list and the code. A scope read in
//!   product code carries the marker comment `// EXEMPT(<ids>)` on the line
//!   above it. Each file of a row's `where_` holds a marker with the row
//!   id; each marker id is a row; each marker sits in a file that its row
//!   names. A scope read with no marker is not found (the weak point of
//!   design section 4.2, ruling Q8).
//! - Claim 6 (P29-6 6d): every cell of `limits::scope` that is not judged
//!   names a row of the list, the row exempts that limit, and the row names
//!   `src/limits/scope.rs`, which holds its marker. A sample gate names
//!   only limits that a sample gate judges (no `NotASampleLimit`), and the
//!   five sample limits are judged on the Body, Entry, Cleanup and Link
//!   groups (operator Q1: "I think all moves should be judged").

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use rs_cam_core::limits::{
    CutGroup, EXEMPTIONS, Exemption, ExemptionClass, Geometry, LimitId, Scope, Who, scope,
};

/// The rows that need a ruling. Exact: a closed row lowers it.
const MAX_NEEDS_RULING: usize = 37;
/// Every row.
const ROWS: usize = 48;
/// The rows with a geometry reason.
const PHYSICAL: usize = 8;
/// The rows that quote an operator ruling.
const RULED: usize = 3;
/// The rows that name the files of their scope reads. The rows of an
/// undeclared cut (no reader) name none. Commit C gave the two Q4 rows
/// their file (`src/limits/sample.rs`), and added the row `C-LeadInApproach`.
/// P29-4 closed three rows with a file (`P28-01`, `P28-34`, `M7`).
/// P29-5 closed the row `F-DepthRule` (ruling R3: the depth limit reads
/// the material sides of the body cut, not only the family and the role).
/// P29-6 6b closed the row `M1` (the chip gate reads every removing sample).
/// P29-6 6c closed the row `M5` (the depth gate judges every sample).
/// P29-6 6d closed ten rows with a file: every gate judges every cut group
/// (`P28-22`, `P28-23`, `M2`, `M3`, `M8`, `Q10-LinkingInEntrySpan`,
/// `Q10-EntryPlungeInNoSpan`, `Q4-SlotRingInferred`,
/// `Q4-CappedStepNoTrace`, `C-LeadInApproach`).
/// P29-5b closed two rows with a file (`M11`, `P28-12`: the first-pass
/// filings), and the row `U-PerimeterSweep` got its first file (the
/// marker on the geodesic arm of the 3D Adaptive filing).
/// P29-5c closed the row `P28-03` (a groove reads the slot width before
/// the simulation); the row `P28-32` gained two files (the chamfer arms
/// of the envelope and of `cap_cut_of_operation`).
const ROWS_WITH_A_FILE: usize = 41;

/// The 34 rows of `phases/P28_limit_audit.md`.
const P28_IDS: [&str; 34] = [
    "P28-01", "P28-02", "P28-03", "P28-04", "P28-05", "P28-06", "P28-07", "P28-08", "P28-09",
    "P28-10", "P28-11", "P28-12", "P28-13", "P28-14", "P28-15", "P28-16", "P28-17", "P28-18",
    "P28-19", "P28-20", "P28-21", "P28-22", "P28-23", "P28-24", "P28-25", "P28-26", "P28-27",
    "P28-28", "P28-29", "P28-30", "P28-31", "P28-32", "P28-33", "P28-34",
];

/// The 12 rows of `phases/P29_parts/A_cuts_and_limits.md` section 3.3.
const M_IDS: [&str; 12] = [
    "M1", "M2", "M3", "M4", "M5", "M6", "M7", "M8", "M9", "M10", "M11", "M12",
];

/// The audit ids that a step closed, with the step (P29-4: ruling R1, the
/// Adaptive family reads the force budget; P29-5b: the first-pass filings
/// are true; P29-5c: a groove reads the slot width; P29-6: the chip gate reads every removing sample, and the
/// depth gate judges every sample). An audit id is
/// a row or in this list, never both; the two id lists above stay the
/// record (ruling Q6).
const CLOSED: &[(&str, &str)] = &[
    ("P28-01", "P29-4"),
    ("P28-34", "P29-4"),
    ("M7", "P29-4"),
    ("M11", "P29-5b"),
    ("P28-12", "P29-5b"),
    ("P28-03", "P29-5c"),
    ("M1", "P29-6"),
    ("M5", "P29-6"),
    ("P28-22", "P29-6"),
    ("P28-23", "P29-6"),
    ("M2", "P29-6"),
    ("M3", "P29-6"),
    ("M8", "P29-6"),
];

/// The marker text before the ids.
const MARKER: &str = "// EXEMPT(";

fn crate_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("the source folder reads") {
        let path = entry.expect("a folder entry").path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Every marker in `src/`: id -> the files that hold it. The scan reads the
/// raw source (a marker IS a comment).
fn markers() -> BTreeMap<String, BTreeSet<String>> {
    let mut files = Vec::new();
    rust_files(&crate_root().join("src"), &mut files);
    assert!(files.len() > 100, "the scan reads the source tree");
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for file in files {
        let rel = file
            .strip_prefix(crate_root())
            .expect("a file under the crate")
            .to_string_lossy()
            .replace('\\', "/");
        let text = std::fs::read_to_string(&file).expect("a source file reads");
        for line in text.lines() {
            let Some(at) = line.find(MARKER) else {
                continue;
            };
            assert!(
                line.trim_start().starts_with(MARKER),
                "{rel}: a marker stands alone on its line: `{line}`"
            );
            let rest = line
                .get(at + MARKER.len()..)
                .expect("the text after the marker");
            let ids = rest
                .split_once(')')
                .unwrap_or_else(|| panic!("{rel}: a marker closes its list: `{line}`"))
                .0;
            for id in ids.split(',') {
                let id = id.trim();
                assert!(!id.is_empty(), "{rel}: an empty id in `{line}`");
                out.entry(id.to_owned()).or_default().insert(rel.clone());
            }
        }
    }
    out
}

fn row(id: &str) -> Option<&'static Exemption> {
    EXEMPTIONS.iter().find(|e| e.id == id)
}

#[test]
fn the_count_of_rows_that_need_a_ruling_is_pinned() {
    let (mut needs, mut physical, mut ruled) = (0, 0, 0);
    for e in &EXEMPTIONS {
        match e.class {
            ExemptionClass::NeedsRuling => needs += 1,
            ExemptionClass::Physical(_) => physical += 1,
            ExemptionClass::Ruled { .. } => ruled += 1,
        }
    }
    assert_eq!(
        needs, MAX_NEEDS_RULING,
        "rows that need a ruling: {needs}, pinned {MAX_NEEDS_RULING}. A closed row lowers the \
         pin and names the operator ruling in the commit; a new row raises it on purpose"
    );
    assert_eq!(EXEMPTIONS.len(), ROWS, "the row count");
    assert_eq!(physical, PHYSICAL, "the physical rows");
    assert_eq!(ruled, RULED, "the ruled rows");
}

#[test]
fn every_row_states_its_class_and_its_limit() {
    let mut reasons = BTreeSet::new();
    for e in &EXEMPTIONS {
        assert!(!e.what.trim().is_empty(), "{}: states its limit", e.id);
        assert!(!e.closes_in.trim().is_empty(), "{}: states its close", e.id);
        match e.who {
            Who::Facts(s) | Who::Undeclared(s) | Who::Reader(s) => {
                assert!(!s.trim().is_empty(), "{}: states who", e.id);
            }
            Who::Kind(kinds) => assert!(!kinds.is_empty(), "{}: names a kind", e.id),
        }
        match e.class {
            // Claim 2: the type holds the reason.
            ExemptionClass::Physical(geometry) => {
                match geometry {
                    Geometry::NoRadialEngagement
                    | Geometry::NotAnOffsetRing
                    | Geometry::NoVerticalFeed => {}
                }
                reasons.insert(format!("{geometry:?}"));
            }
            // Claim 3.
            ExemptionClass::Ruled { by, date, text } => {
                assert!(!text.trim().is_empty(), "{}: quotes the ruling", e.id);
                assert!(!by.trim().is_empty(), "{}: names who ruled", e.id);
                let parts: Vec<&str> = date.split('-').collect();
                let shape_ok = matches!(parts.as_slice(), [y, m, d]
                    if y.len() == 4 && m.len() == 2 && d.len() == 2
                        && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit())));
                assert!(shape_ok, "{}: the date `{date}` is YYYY-MM-DD", e.id);
            }
            ExemptionClass::NeedsRuling => {}
        }
    }
    assert_eq!(
        reasons.len(),
        3,
        "the physical rows reach each reason: {reasons:?}"
    );
}

#[test]
fn every_audit_id_is_a_row_and_no_id_repeats() {
    let mut seen = BTreeSet::new();
    for e in &EXEMPTIONS {
        assert!(seen.insert(e.id), "the id {} repeats", e.id);
    }
    for id in P28_IDS.iter().chain(M_IDS.iter()) {
        let closed = CLOSED.iter().any(|(c, _)| c == id);
        assert!(
            row(id).is_some() != closed,
            "the audit id {id} is a row ({}) or closed ({closed}), never both and never neither",
            row(id).is_some()
        );
    }
    for (id, step) in CLOSED {
        assert!(
            P28_IDS.contains(id) || M_IDS.contains(id),
            "the closed id {id} ({step}) is an audit id"
        );
    }
}

#[test]
fn every_row_and_every_marker_name_each_other() {
    let markers = markers();
    assert!(
        markers.values().map(BTreeSet::len).sum::<usize>() > 50,
        "the scan finds the markers"
    );
    // (b) Every marker id is a row, and the row names the marker's file.
    for (id, files) in &markers {
        let e = row(id).unwrap_or_else(|| panic!("the marker id {id} ({files:?}) has no row"));
        for file in files {
            assert!(
                e.where_.contains(&file.as_str()),
                "{file} holds a marker of {id}, and the row does not name the file"
            );
        }
    }
    // (a) Every file of a row exists and holds a marker of the row.
    let mut with_a_file = 0;
    for e in &EXEMPTIONS {
        if !e.where_.is_empty() {
            with_a_file += 1;
        }
        for file in e.where_ {
            assert!(
                crate_root().join(file).is_file(),
                "{}: {file} is a file",
                e.id
            );
            assert!(
                markers.get(e.id).is_some_and(|f| f.contains(*file)),
                "{}: {file} holds no marker of the row",
                e.id
            );
        }
    }
    assert_eq!(
        with_a_file, ROWS_WITH_A_FILE,
        "the rows that name the files of their scope reads"
    );
}

#[test]
fn every_not_judged_cell_names_its_row_p29_6() {
    let mut not_judged = 0;
    for limit in LimitId::ALL {
        for group in CutGroup::ALL {
            match scope(limit, group) {
                Scope::Judged | Scope::NotASampleLimit => {}
                Scope::NotJudged(cell) => {
                    not_judged += 1;
                    let e = row(cell.row).unwrap_or_else(|| {
                        panic!("{limit:?} x {group:?}: not judged, and no row {}", cell.row)
                    });
                    assert!(
                        e.limit.contains(&limit),
                        "{limit:?} x {group:?}: the row {} does not exempt the limit",
                        cell.row
                    );
                    assert!(
                        e.where_.contains(&"src/limits/scope.rs"),
                        "{limit:?} x {group:?}: the row {} does not name src/limits/scope.rs",
                        cell.row
                    );
                    assert!(!cell.reason.trim().is_empty(), "the cell states a reason");
                }
            }
        }
    }
    assert!(not_judged > 0, "the scope holds the drill cells");
    // Operator Q1 (2026-11-12): every sample limit judges every group of a
    // milling cut.
    for (gate, limits) in rs_cam_core::tool_load::groups::GATE_LIMITS {
        assert!(!limits.is_empty(), "{gate}: names its limits");
        for &limit in limits {
            for group in [
                CutGroup::Body,
                CutGroup::Entry,
                CutGroup::Cleanup,
                CutGroup::Link,
            ] {
                assert_eq!(
                    scope(limit, group),
                    Scope::Judged,
                    "{gate}: {limit:?} on {group:?}"
                );
            }
            assert_ne!(
                scope(limit, CutGroup::Drill),
                Scope::NotASampleLimit,
                "{gate}: {limit:?} is a sample limit"
            );
        }
    }
}
