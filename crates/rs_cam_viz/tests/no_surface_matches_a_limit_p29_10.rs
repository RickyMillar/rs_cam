//! **P29-10 (10e): no viz surface matches a limit or a state enum.**
//!
//! Core writes the text and the tone of every limit row
//! (`limits::CheckRow`). A viz `match` arm or a `matches!` on a limit enum
//! decides on its own what to show for a limit, so a new limit compiles and
//! shows with a wrong word or not at all (`P29-10_DESIGN.md` 2.4 and 3.5).
//! This scan fails on such an arm in the product source of
//! `crates/rs_cam_viz/src`. The one `Tone` to `Role` map (`role_of_tone`)
//! is the exception; `Tone` is not a scanned enum.
//!
//! The scanned enums: `LimitId`, `LimitKind`, `CriterionKind`, `CutGroup`,
//! `CheckState` and `LoadState`.
//!
//! What counts as an arm:
//!
//! - a path `Enum::Variant` in the pattern of a `match` arm: the text from
//!   the path to the next `=>` holds only pattern characters, and the text
//!   before the path does not end with `=>` (a path in an arm body is not a
//!   pattern);
//! - a path inside `matches!( .. )`.
//!
//! A comment never matches (the viz test rule), and the test modules of a
//! source file are not product source.
//!
//! The files that 10f and 10g still own are a named allow-list. The list
//! only shrinks: an allow-listed file with no arm fails the test, so its
//! row goes in the commit that removes the last arm.

#![expect(
    clippy::indexing_slicing,
    reason = "test code: the scan slices the source at the byte offsets it found"
)]

use std::path::{Path, PathBuf};

/// The scanned enums.
const ENUMS: [&str; 6] = [
    "LimitId",
    "LimitKind",
    "CriterionKind",
    "CutGroup",
    "CheckState",
    "LoadState",
];

/// The files that a later commit of P29-10 converts: (path under `src/`,
/// the commit that removes the arms).
const ALLOW: [(&str, &str); 1] = [
    // `limit_name` and the plot marks match `LimitKind` (design 3.5 item 5).
    ("ui/feeds/envelope.rs", "10g"),
];

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // A `tests/` folder under `src/` holds test modules only.
            if path.file_name().is_some_and(|n| n == "tests") {
                continue;
            }
            collect_rs(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs")
            && !path.file_name().is_some_and(|n| n == "tests.rs")
        {
            out.push(path);
        }
    }
}

/// The code of one line with its `//` comment removed. A `//` inside a
/// string literal stays.
fn strip_line_comment(line: &str) -> String {
    let mut out = String::new();
    let mut in_str = false;
    let mut prev = '\0';
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '"' && prev != '\\' {
            in_str = !in_str;
        }
        if !in_str && c == '/' && chars.peek() == Some(&'/') {
            break;
        }
        out.push(c);
        prev = if prev == '\\' && c == '\\' { '\0' } else { c };
    }
    out
}

/// The product code of a file: comments removed, the test modules cut.
/// A test module starts at a `#[cfg(test)]` line that is followed by a
/// `mod` line; it runs to the end of the file (the crate places its test
/// modules last).
fn product_code(src: &str) -> Vec<String> {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim_start();
        if t.starts_with("#[cfg(test)]")
            && lines
                .get(i + 1)
                .is_some_and(|next| next.trim_start().starts_with("mod "))
        {
            break;
        }
        out.push(strip_line_comment(line));
    }
    out
}

/// The byte ranges of the `role_of_tone` function bodies (the exception).
fn exempt_spans(code: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut from = 0;
    while let Some(at) = code[from..].find("fn role_of_tone(") {
        let start = from + at;
        let Some(open) = code[start..].find('{').map(|o| start + o) else {
            break;
        };
        let mut depth = 0_i32;
        let mut end = open;
        for (k, c) in code[open..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = open + k;
                        break;
                    }
                }
                _ => {}
            }
        }
        spans.push((start, end));
        from = end.max(start + 1);
    }
    spans
}

/// The byte ranges inside each `matches!( .. )`.
fn matches_spans(code: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut from = 0;
    while let Some(at) = code[from..].find("matches!(") {
        let open = from + at + "matches!".len();
        let mut depth = 0_i32;
        let mut end = open;
        for (k, c) in code[open..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = open + k;
                        break;
                    }
                }
                _ => {}
            }
        }
        spans.push((open, end));
        from = end.max(open + 1);
    }
    spans
}

/// A character that a match pattern can hold between a path and its `=>`.
fn pattern_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c.is_whitespace() || "_:|(){},.&@".contains(c)
}

/// `true` when the path at `at..end` is in the pattern of a match arm.
fn in_arm_pattern(code: &str, at: usize, end: usize) -> bool {
    let before = code[..at].trim_end();
    // A path in an arm body follows the arm's `=>` (directly, or after a
    // constructor such as `Some(`).
    let before_ctor = before
        .trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == ':' || c == '(');
    if before.ends_with("=>") || before_ctor.trim_end().ends_with("=>") {
        return false;
    }
    let rest = &code[end..];
    let Some(arrow) = rest.find("=>") else {
        return false;
    };
    let between = &rest[..arrow];
    // A guard (`if ..`) may follow a pattern; it holds `==` and `.`, which
    // stop the plain pattern test, so a guard is read up to its `=>`.
    let pattern_part = match between.find(" if ") {
        Some(g) => &between[..g],
        None => between,
    };
    pattern_part.chars().all(pattern_char)
        && !between.contains(';')
        && !between.contains('"')
        && !between.contains("let ")
}

/// Each hit: (line number, line text).
fn hits_in(src: &str) -> Vec<(usize, String)> {
    let lines = product_code(src);
    let code = lines.join("\n");
    let exempt = exempt_spans(&code);
    let in_matches = matches_spans(&code);
    let mut hits = Vec::new();
    for name in ENUMS {
        let needle = format!("{name}::");
        let mut from = 0;
        while let Some(rel) = code[from..].find(&needle) {
            let at = from + rel;
            from = at + needle.len();
            // A whole word: `XLimitId::` is a different name.
            if code[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                continue;
            }
            let tail = &code[from..];
            let variant_len = tail
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(tail.len());
            if !tail.starts_with(|c: char| c.is_ascii_uppercase()) {
                continue;
            }
            let end = from + variant_len;
            if exempt.iter().any(|(s, e)| at >= *s && at <= *e) {
                continue;
            }
            let in_macro = in_matches.iter().any(|(s, e)| at > *s && at < *e);
            if in_macro || in_arm_pattern(&code, at, end) {
                let line_no = code[..at].matches('\n').count() + 1;
                hits.push((line_no, lines[line_no - 1].trim().to_owned()));
            }
        }
    }
    hits.sort();
    hits
}

fn rel(path: &Path) -> String {
    path.strip_prefix(src_root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[test]
fn no_surface_matches_a_limit_p29_10() {
    let mut files = Vec::new();
    collect_rs(&src_root(), &mut files);
    assert!(files.len() > 50, "non-vacuity: the scan reads the crate");
    let mut offenders = Vec::new();
    let mut allowed_with_hits = Vec::new();
    for path in &files {
        let src = std::fs::read_to_string(path).unwrap_or_default();
        let hits = hits_in(&src);
        let name = rel(path);
        if ALLOW.iter().any(|(f, _)| *f == name) {
            if !hits.is_empty() {
                allowed_with_hits.push(name);
            }
            continue;
        }
        for (line, text) in hits {
            offenders.push(format!("{name}:{line}: {text}"));
        }
    }
    assert!(
        offenders.is_empty(),
        "a viz surface matches a limit or a state enum; read the core row \
         (`CheckRow::face`, `detail`, `tone`, `LimitId::text`) instead:\n{}",
        offenders.join("\n")
    );
    for (file, commit) in ALLOW {
        assert!(
            allowed_with_hits.iter().any(|f| f == file),
            "{file} has no arm left: remove its allow-list row (owned by \
             {commit}); the list only shrinks"
        );
    }
}

/// The scan finds an arm, a guard, a `matches!` and a multi-line pattern,
/// and it does not find a path in an arm body, a comparison or a comment.
#[test]
fn the_scan_finds_an_arm_and_skips_a_body_p29_10() {
    let arm = "fn f(k: K) -> u8 {\n    match k {\n        CriterionKind::Power => 1,\n        _ => 0,\n    }\n}\n";
    assert_eq!(hits_in(arm).len(), 1, "an arm pattern");
    let multi = "match k {\n    LoadState::Within\n    | LoadState::Exceeds => 1,\n}\n";
    assert_eq!(hits_in(multi).len(), 2, "a multi-line pattern");
    let guard = "match d {\n    Some(d) if d.state == Some(LoadState::Exceeds) => 1,\n}\n";
    assert_eq!(hits_in(guard).len(), 1, "a guard");
    let mac = "let a = matches!(\n    s,\n    CheckState::Within\n);\n";
    assert_eq!(hits_in(mac).len(), 1, "a matches! over lines");
    let body = "match m {\n    A => CriterionKind::Power,\n    B => Some(CutGroup::Body),\n}\n";
    assert!(hits_in(body).is_empty(), "an arm body is not a pattern");
    let cmp =
        "let a = k == CriterionKind::Power && b;\nlet x = [CutGroup::Body, CutGroup::Entry];\n";
    assert!(
        hits_in(cmp).is_empty(),
        "a comparison and a list are not arms"
    );
    let comment = "// match k { CriterionKind::Power => 1 }\nlet a = 1;\n";
    assert!(hits_in(comment).is_empty(), "a comment never matches");
    let tests = "fn a() {}\n#[cfg(test)]\nmod tests {\n    fn b(k: K) { match k { CutGroup::Body => {} } }\n}\n";
    assert!(
        hits_in(tests).is_empty(),
        "a test module is not product source"
    );
    let exempt = "fn role_of_tone(t: Tone) -> Role {\n    match t {\n        LoadState::Within => Role::Ok,\n    }\n}\n";
    assert!(hits_in(exempt).is_empty(), "role_of_tone is the exception");
}

/// Non-vacuity: the one tone map exists, and the simulation surfaces read
/// the core row.
#[test]
fn the_tone_map_and_a_row_reader_exist_p29_10() {
    let kit = std::fs::read_to_string(src_root().join("ui/components/chip.rs")).unwrap_or_default();
    assert!(
        kit.contains("pub fn role_of_tone("),
        "the one `Tone` to `Role` map lives in the kit (`ui/components/chip.rs`)"
    );
    for file in [
        "ui/sim_diagnostics.rs",
        "ui/readiness_panel.rs",
        "ui/sim_op_list.rs",
    ] {
        let src = std::fs::read_to_string(src_root().join(file)).unwrap_or_default();
        let code = product_code(&src).join("\n");
        assert!(code.contains("CheckRow"), "{file} reads `CheckRow`");
    }
    let diagnostics =
        std::fs::read_to_string(src_root().join("ui/sim_diagnostics.rs")).unwrap_or_default();
    let code = product_code(&diagnostics).join("\n");
    assert!(
        code.contains("row.face()") && code.contains("role_of_tone("),
        "the inspector paints the core face and colours the core tone"
    );
}
