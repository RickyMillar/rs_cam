//! F095 (tech-debt review 2026-10-19, W7): the GUI does not re-derive the
//! drill family. Core owns the predicate:
//! `OperationType::is_drill_kinematics` (a registry row field), and the
//! hole-target pick reads the row's `ItemUse::Holes`. A hand list such as
//! `OperationConfig::Drill(_) | OperationConfig::AlignmentPinDrill(_)` fails
//! open: a third drill op would join the false side with no compiler word.
//!
//! This source scan finds such a list anywhere under `src/`. The anchors
//! prove the scan read the three sites that once held one.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

fn rs_files(root: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(root).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            rs_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// The code of a source file with its `//` comments and all whitespace
/// removed, so a list split over lines still matches and a comment that
/// names the old list does not.
fn squashed(text: &str) -> String {
    text.lines()
        .map(|line| line.split("//").next().unwrap_or(""))
        .flat_map(str::chars)
        .filter(|c| !c.is_whitespace())
        .collect()
}

#[test]
fn the_gui_reads_the_core_drill_predicate() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rs_files(&root, &mut files);

    // The hand lists of the drill family, in both orders and both enums.
    let forbidden = [
        "OperationConfig::Drill(_)|OperationConfig::AlignmentPinDrill(_)",
        "OperationConfig::AlignmentPinDrill(_)|OperationConfig::Drill(_)",
        "OperationType::Drill|OperationType::AlignmentPinDrill",
        "OperationType::AlignmentPinDrill|OperationType::Drill",
    ];

    let mut hits = Vec::new();
    let mut drill_predicate_calls = 0usize;
    let mut anchors_seen = 0usize;
    for path in &files {
        let text = std::fs::read_to_string(path).unwrap();
        let flat = squashed(&text);
        for pattern in forbidden {
            if flat.contains(pattern) {
                hits.push(format!("{}: {pattern}", path.display()));
            }
        }
        drill_predicate_calls += flat.matches(".is_drill_kinematics()").count();
        if path.ends_with("app/viewport.rs") {
            assert!(
                flat.contains("ItemUse::Holes{..}"),
                "app/viewport.rs: the drill-target pick no longer reads ItemUse::Holes"
            );
            anchors_seen += 1;
        }
        if path.ends_with("ui/properties/feeds_speeds.rs") || path.ends_with("ui/properties/mod.rs")
        {
            assert!(
                flat.contains(".op_type().is_drill_kinematics()"),
                "{}: the drill test no longer reads is_drill_kinematics",
                path.display()
            );
            anchors_seen += 1;
        }
    }

    // Non-vacuity: the scan read the three former sites and found the calls.
    assert_eq!(
        anchors_seen, 3,
        "the scan did not read the three F095 sites"
    );
    assert!(
        drill_predicate_calls >= 2,
        "found {drill_predicate_calls} is_drill_kinematics calls; expected the two F095 sites"
    );
    assert!(
        hits.is_empty(),
        "a hand list of the drill family; use `op_type().is_drill_kinematics()` \
         or the row's `ItemUse`: {hits:#?}"
    );
}
