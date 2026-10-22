//! WP28 part 2, the viz half — no viz source names the deleted second
//! staleness model.
//!
//! W14a (F171) moved the viz root of
//! `rs_cam_core/tests/stale_set_has_one_answer_wp28.rs` arm 1 here, so the
//! viz gate runs it. `Effects::stale` is the one staleness answer; the
//! deleted producer published `compute_stale_set` and `MutationKind`. The
//! scan covers the in-`src` test modules too, as the core arm does.

#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::{Path, PathBuf};

/// The two names the second staleness producer published.
const FORBIDDEN: &[&str] = &["compute_stale_set", "MutationKind"];

/// The bars of the core arm: the lowest number of files the walk must
/// visit, a name that IS in the corpus, and its lowest number of lines.
const MIN_FILES_WALKED: usize = 100;
const POSITIVE_CONTROL: &str = "ProjectSession";
const MIN_CONTROL_HITS: usize = 50;

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries =
        std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()));
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The line with every `//` comment removed (line-wise, as the core arm).
fn strip_comment(line: &str) -> &str {
    match line.find("//") {
        Some(at) => &line[..at],
        None => line,
    }
}

#[test]
fn no_viz_source_names_the_second_staleness_model_wp28() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_rs(&root, &mut files);
    files.sort();
    assert!(
        files.len() >= MIN_FILES_WALKED,
        "the walk must visit at least {MIN_FILES_WALKED} files, or it reads \
         a tree that is not there. I read {}",
        files.len()
    );

    let mut control_hits = 0_usize;
    let mut found: Vec<String> = Vec::new();
    for path in &files {
        let text = std::fs::read_to_string(path).unwrap();
        for (number, line) in text.lines().enumerate() {
            let code = strip_comment(line);
            if code.contains(POSITIVE_CONTROL) {
                control_hits += 1;
            }
            for name in FORBIDDEN {
                if code.contains(name) {
                    found.push(format!(
                        "{}:{}: {}",
                        path.display(),
                        number + 1,
                        line.trim()
                    ));
                }
            }
        }
    }
    assert!(
        control_hits >= MIN_CONTROL_HITS,
        "the reader found `{POSITIVE_CONTROL}` on {control_hits} lines. Under \
         {MIN_CONTROL_HITS} means the reader is not reading the corpus"
    );
    assert!(
        found.is_empty(),
        "WP28 part 2: `Effects::stale` is the one staleness answer. A surface \
         reports the set the command's own setter dropped. I read {} naming \
         line(s):\n  {}",
        found.len(),
        found.join("\n  ")
    );
}
