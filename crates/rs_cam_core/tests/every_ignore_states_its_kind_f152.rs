//! F152 (W14a) — every `#[ignore]` in the core tests states what kind of
//! run it parks, and none parks a broken test.
//!
//! `tests/CLAUDE.md` says "`#[ignore]` marks an instrument or an evidence
//! run, not a broken test". Before W14a that was a doc rule only: five
//! tests sat behind `#[ignore]` as known reds or retired claims ("red until
//! ..", "known-failing", "blocked on ..", "aspirational", "retired claim"),
//! and one had no reason at all. A parked red never fails, so a regression
//! of the half that still holds goes unseen, and the fix of the half that
//! does not goes unnoticed.
//!
//! The rule now: the reason starts with one of [`KINDS`], and it holds no
//! word of [`PARKED_RED`]. A known red is an inverted assertion that runs
//! (it pins today's bad value and fails when the fix lands); a retired
//! claim is deleted.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

#[path = "common/source_scan.rs"]
mod source_scan;

use std::path::Path;

/// The kinds of run an `#[ignore]` may park.
///
/// - `instrument:` prints or writes a measurement and has no bar.
/// - `evidence:` an evidence run of a programme; it may need an operator
///   file that is not in the repository.
/// - `heavy:` a real assertion that takes minutes.
/// - `external:` needs a program outside the build (an emulator).
/// - `serial:` changes process-global state and runs alone.
const KINDS: &[&str] = &["instrument:", "evidence:", "heavy:", "external:", "serial:"];

/// Words that say a test is parked because it is red or retired.
const PARKED_RED: &[&str] = &[
    "red until",
    "known-failing",
    "known failing",
    "blocked on",
    "aspirational",
    "retired",
];

/// Every `#[ignore` attribute of the core tests, and every `ignore = ".."`
/// arm of a `cfg_attr`: (file, line, attribute line). Comments do not
/// count.
fn ignore_attributes() -> Vec<(String, usize, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut out = Vec::new();
    for path in source_scan::rust_files(&root) {
        let src = std::fs::read_to_string(&path).unwrap();
        let text = source_scan::strip_comments(&src);
        let rel = path
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        for (i, line) in text.split('\n').enumerate() {
            let t = line.trim();
            if t.starts_with("#[ignore") {
                out.push((rel.clone(), i + 1, t.to_owned()));
            } else if let Some(reason) = t.strip_prefix("ignore = \"") {
                // The `ignore = ".."` arm of a `#[cfg_attr(.., ignore = ..)]`.
                out.push((rel.clone(), i + 1, format!("#[ignore = \"{reason}")));
            }
        }
    }
    out
}

/// The verdict on one attribute line: `None` is a pass.
fn refusal(attr: &str) -> Option<String> {
    let Some(reason) = attr.strip_prefix("#[ignore = \"") else {
        return Some("has no reason string".to_owned());
    };
    if !KINDS.iter().any(|k| reason.starts_with(k)) {
        return Some(format!("does not start with one of {KINDS:?}"));
    }
    let lower = reason.to_lowercase();
    PARKED_RED
        .iter()
        .find(|w| lower.contains(*w))
        .map(|w| format!("parks a red or retired test (\"{w}\")"))
}

#[test]
fn the_reason_rule_refuses_a_parked_red_and_takes_an_instrument() {
    // The guarded defects, one each, through the same verdict the scan uses.
    assert!(refusal("#[ignore]").is_some(), "a bare ignore");
    assert!(
        refusal("#[ignore = \"red until the fix lands\"]").is_some(),
        "a reason with no kind"
    );
    assert!(
        refusal("#[ignore = \"heavy: known-failing since the fix\"]").is_some(),
        "a parked red under a kind"
    );
    assert!(
        refusal("#[ignore = \"instrument: prints the sweep; no assertion\"]").is_none(),
        "an instrument"
    );
}

#[test]
fn every_ignore_states_its_kind_f152() {
    let attrs = ignore_attributes();
    // Non-vacuity: the scan reads real attributes, and a known instrument
    // is among them.
    assert!(
        attrs
            .iter()
            .any(|(f, _, a)| f == "reach_map_p5.rs" && a.starts_with("#[ignore = \"instrument:")),
        "non-vacuity: the scan did not see the `instrument:` ignore of reach_map_p5.rs \
         ({} attributes read)",
        attrs.len()
    );
    let refused: Vec<String> = attrs
        .iter()
        .filter_map(|(f, l, a)| refusal(a).map(|why| format!("{f}:{l} {why}: {a}")))
        .collect();
    assert!(
        refused.is_empty(),
        "an `#[ignore]` marks an instrument or an evidence run, not a broken test \
         (tests/CLAUDE.md). Start the reason with one of {KINDS:?}. A known red is an \
         inverted assertion that runs; a retired claim is deleted.\n{}",
        refused.join("\n")
    );
}
