//! Feeds contract P16 item 7 (D3): every post-simulation gate reads one
//! feed, the emitted feed of the sample.
//!
//! # The defect
//!
//! The chip, power and deflection gates read
//! `tool_load::effective_feed_for_sample` (the modulated feed when the
//! trace holds one). The depth gate read `SimulationCutSample::feed_rate_mm_min`,
//! the feed of the simulation run. After one simulation the depth gate
//! graded a feed that the machine never runs (P_thin_005: 5999 against an
//! emitted F2615), and a second simulation of the modulated toolpath gave
//! the same project a second depth verdict.
//!
//! # Why a source scan, not a type
//!
//! The approved design asks for a typed hold first: a private field, so
//! that only the door can read the sample feed. A source scan on
//! 2026-11-06 counted about 150 uses of `feed_rate_mm_min` in the 41 files
//! that name `SimulationCutSample` (plus 4 in two files that read
//! `s.feed_rate_mm_min` without the name). After the simulator and the
//! stock module, about 100 uses in about 37 files stay: test fixtures
//! that build samples, the trace narration, the drill metrics and the
//! gates. That is far past the limit of about 30 sites in the design, so
//! this scan is the hold (the design's fallback).
//!
//! # The claim
//!
//! 1. No production line under `src/tool_load/` reads `.feed_rate_mm_min`
//!    except inside `effective_feed_for_sample` (the door) or at an entry
//!    of [`ALLOWED`]. Each entry names its file, its function, the text of
//!    the read, the count, and one reason. An entry that the scan does not
//!    find exactly `count` times fails, so the list cannot go stale.
//! 2. Non-vacuity: the door itself reads the sample field (two reads), and
//!    the depth gate (`depth.rs`, `evaluate`) calls the door.
//! 3. No production line under `src/tool_load/` names the struct pattern
//!    `SimulationCutSample {`: a destructure would read the field without a
//!    dot.
//!
//! The scan skips comments and each column-0 `#[cfg(test)]` item (the test
//! modules and fixtures).

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure; the shared source scan indexes its own char buffer"
)]

#[path = "common/source_scan.rs"]
mod source_scan;

use std::path::PathBuf;

/// The one door: the gate feed of a sample.
const DOOR_FILE: &str = "mod.rs";
const DOOR_FN: &str = "effective_feed_for_sample";
/// The reads of the sample field inside the door: the early return with an
/// empty map, and the fallback when the map has no entry for the move.
const DOOR_READS: usize = 2;

/// One allowed read of `.feed_rate_mm_min` that is not a gate feed.
struct Allowed {
    file: &'static str,
    func: &'static str,
    text: &'static str,
    count: usize,
    reason: &'static str,
}

/// The allowed reads. Each one is NOT the feed a gate grades.
const ALLOWED: &[Allowed] = &[
    Allowed {
        file: "chipload.rs",
        func: "steady_state_samples_for_toolpath",
        text: "s.feed_rate_mm_min >= feed_threshold",
        count: 1,
        reason: "the steady-state population filter: it compares the commanded feed of \
                 the sample with a fraction of the op's commanded feed to drop entry and \
                 ramp moves. It selects samples; it grades no feed. A modulated feed \
                 here would drop moves that the modulator fed down.",
    },
    Allowed {
        file: "chipload.rs",
        func: "evaluate_inner",
        text: "s.feed_rate_mm_min.max(1e-9)",
        count: 1,
        reason: "the denominator of `AchievedFeedStage::median_ratio`, the ratio of the \
                 gate feed (the door, the numerator) to the commanded feed. The \
                 commanded feed is the quantity by definition.",
    },
    Allowed {
        file: "drill_gates.rs",
        func: "evaluate_plunge_feed",
        text: "drill_op.feed_rate_mm_min",
        count: 1,
        reason: "a `DrillOp` field (the programmed drill plunge feed), not a \
                 `SimulationCutSample` field.",
    },
];

fn tool_load_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/tool_load")
}

/// One production line of a file: its number, the enclosing function and
/// the code with the comment removed.
struct CodeLine {
    number: usize,
    func: Option<String>,
    code: String,
}

/// The production lines of `src`: comments removed
/// (`source_scan::strip_comments`), each column-0 `#[cfg(test)]` item
/// skipped. rustfmt closes a top-level item with a column-0 `}`; an item
/// that ends with `;` is one line.
fn production_lines(src: &str) -> Vec<CodeLine> {
    let text = source_scan::strip_comments(src);
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out = Vec::new();
    let mut func: Option<String> = None;
    let mut i = 0;
    while let Some(&line) = lines.get(i) {
        if line.starts_with("#[cfg(test)]") {
            let mut j = i + 1;
            while lines.get(j).is_some_and(|l| {
                let t = l.trim_start();
                t.is_empty() || t.starts_with('#')
            }) {
                j += 1;
            }
            if lines.get(j).is_some_and(|l| l.trim_end().ends_with(';')) {
                i = j + 1;
                continue;
            }
            while lines.get(j).is_some_and(|l| *l != "}") {
                j += 1;
            }
            i = j + 1;
            continue;
        }
        let code = line.to_owned();
        if let Some(name) = fn_name(&code) {
            func = Some(name);
        }
        out.push(CodeLine {
            number: i + 1,
            func: func.clone(),
            code,
        });
        i += 1;
    }
    out
}

/// The name after the token `fn ` on a line, if any.
fn fn_name(code: &str) -> Option<String> {
    let mut rest = code;
    while let Some(at) = rest.find("fn ") {
        let before_ok = at == 0
            || rest[..at]
                .chars()
                .last()
                .is_some_and(|c| !(c.is_alphanumeric() || c == '_'));
        let after = &rest[at + 3..];
        if before_ok {
            let name: String = after
                .trim_start()
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() {
                return Some(name);
            }
        }
        rest = after;
    }
    None
}

#[test]
fn every_gate_reads_one_feed_p16() {
    let entries = source_scan::rust_files(&tool_load_dir());
    // Non-vacuity of the walk: the gate files are there.
    for must in [
        "mod.rs",
        "depth.rs",
        "chipload.rs",
        "power.rs",
        "deflection.rs",
    ] {
        assert!(
            entries.iter().any(|p| p.ends_with(must)),
            "the walk did not find src/tool_load/{must}"
        );
    }

    let mut door_reads = 0_usize;
    let mut depth_door_calls = 0_usize;
    let mut allowed_hits = vec![0_usize; ALLOWED.len()];
    let mut offenders: Vec<String> = Vec::new();

    for path in &entries {
        let file = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("file name")
            .to_owned();
        let src = std::fs::read_to_string(path).expect("read source");
        for line in production_lines(&src) {
            let func = line.func.as_deref().unwrap_or("");
            if file == "depth.rs" && func == "evaluate" {
                depth_door_calls += line.code.matches("effective_feed_for_sample(").count();
            }
            if line.code.contains("SimulationCutSample {") {
                offenders.push(format!(
                    "{file}:{} ({func}): struct pattern `SimulationCutSample {{` in production code",
                    line.number
                ));
            }
            let reads = line.code.matches(".feed_rate_mm_min").count();
            if reads == 0 {
                continue;
            }
            if file == DOOR_FILE && func == DOOR_FN {
                door_reads += reads;
                continue;
            }
            let allowed = ALLOWED
                .iter()
                .position(|a| a.file == file && a.func == func && line.code.contains(a.text));
            match allowed {
                Some(k) => {
                    if let Some(hit) = allowed_hits.get_mut(k) {
                        *hit += reads;
                    }
                }
                None => offenders.push(format!(
                    "{file}:{} ({func}): `{}` reads the sample feed; read \
                     `tool_load::effective_feed_for_sample`",
                    line.number,
                    line.code.trim()
                )),
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "a gate reads a feed other than the one gate feed:\n{}",
        offenders.join("\n")
    );
    assert_eq!(
        door_reads, DOOR_READS,
        "non-vacuity: the door `{DOOR_FN}` in {DOOR_FILE} must read the sample field \
         {DOOR_READS} times"
    );
    assert!(
        depth_door_calls >= 2,
        "the depth gate (`depth.rs` `evaluate`) must read the door for its chip and \
         for its X-VAC fallback filter; found {depth_door_calls} calls"
    );
    for (a, hits) in ALLOWED.iter().zip(&allowed_hits) {
        assert_eq!(
            *hits, a.count,
            "allow-list entry {}:{} `{}` ({}) expected {} reads, found {hits}",
            a.file, a.func, a.text, a.reason, a.count
        );
    }
}
