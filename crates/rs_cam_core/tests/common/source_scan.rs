//! A small Rust source reader for the source-scan sentries (W14a).
//!
//! It is not declared in `common/mod.rs`: a sentry includes it with
//! `#[path = "common/source_scan.rs"] mod source_scan;` (the viz sentry
//! with a path into this crate), so only the scans compile it.
//!
//! It knows three things about a file:
//!
//! - the text with every comment removed and every string literal kept;
//! - the top-level items (a function, a constant, a module) and their names;
//! - which items a test that runs (a `#[test]` with no `#[ignore`) can reach
//!   by name, inside the same file.

#![allow(
    dead_code,
    reason = "each test binary that includes this module uses only part of it"
)]

use std::path::{Path, PathBuf};

/// The text of `src` without its `//` and `/* */` comments. String and
/// char literals stay, so a `//` inside a string is not a comment.
pub fn strip_comments(src: &str) -> String {
    let c: Vec<char> = src.chars().collect();
    let n = c.len();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < n {
        if c[i] == '/' && i + 1 < n && c[i + 1] == '/' {
            while i < n && c[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c[i] == '/' && i + 1 < n && c[i + 1] == '*' {
            let mut depth = 1;
            i += 2;
            while i < n && depth > 0 {
                if c[i] == '/' && i + 1 < n && c[i + 1] == '*' {
                    depth += 1;
                    i += 2;
                } else if c[i] == '*' && i + 1 < n && c[i + 1] == '/' {
                    depth -= 1;
                    i += 2;
                } else {
                    // Keep the line count, so a line of the output is a
                    // line of the input.
                    if c[i] == '\n' {
                        out.push('\n');
                    }
                    i += 1;
                }
            }
            continue;
        }
        // A raw string: r"..", r#".."#.
        if c[i] == 'r' && i + 1 < n && (c[i + 1] == '"' || c[i + 1] == '#') {
            let prev_ident = i > 0 && (c[i - 1].is_alphanumeric() || c[i - 1] == '_');
            let mut j = i + 1;
            let mut hashes = 0;
            while j < n && c[j] == '#' {
                hashes += 1;
                j += 1;
            }
            if !prev_ident && j < n && c[j] == '"' {
                let start = i;
                j += 1;
                'raw: while j < n {
                    if c[j] == '"' {
                        let mut k = 0;
                        while k < hashes && j + 1 + k < n && c[j + 1 + k] == '#' {
                            k += 1;
                        }
                        if k == hashes {
                            j += 1 + hashes;
                            break 'raw;
                        }
                    }
                    j += 1;
                }
                out.extend(&c[start..j.min(n)]);
                i = j;
                continue;
            }
        }
        if c[i] == '"' {
            let start = i;
            let mut j = i + 1;
            while j < n && c[j] != '"' {
                j += if c[j] == '\\' { 2 } else { 1 };
            }
            let end = (j + 1).min(n);
            out.extend(&c[start..end]);
            i = end;
            continue;
        }
        // A char literal ('"', '\'', '\n'); a lifetime has no closing quote.
        if c[i] == '\'' {
            if i + 3 < n && c[i + 1] == '\\' {
                if let Some(k) = (i + 2..n.min(i + 12)).find(|&k| c[k] == '\'') {
                    out.extend(&c[i..=k]);
                    i = k + 1;
                    continue;
                }
            } else if i + 2 < n && c[i + 2] == '\'' {
                out.extend(&c[i..=i + 2]);
                i += 3;
                continue;
            }
        }
        out.push(c[i]);
        i += 1;
    }
    out
}

/// The plain string literals of comment-free text, with their quotes, and
/// their char offsets. A raw string (`r".."`, `r#".."#`) is skipped whole:
/// the scans read paths, and the crates write paths as plain strings, while
/// a raw string holds planted source in a sentry's own self-test.
pub fn string_literals(text: &str) -> Vec<(usize, String)> {
    let c: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        if c[i] == 'r'
            && (i == 0 || !(c[i - 1].is_alphanumeric() || c[i - 1] == '_'))
            && i + 1 < c.len()
            && (c[i + 1] == '"' || c[i + 1] == '#')
        {
            let mut j = i + 1;
            let mut hashes = 0;
            while j < c.len() && c[j] == '#' {
                hashes += 1;
                j += 1;
            }
            if j < c.len() && c[j] == '"' {
                j += 1;
                while j < c.len() {
                    if c[j] == '"' && (0..hashes).all(|k| c.get(j + 1 + k) == Some(&'#')) {
                        j += 1 + hashes;
                        break;
                    }
                    j += 1;
                }
                i = j;
                continue;
            }
        }
        if c[i] == '\'' && i + 2 < c.len() && c[i + 2] == '\'' {
            i += 3;
            continue;
        }
        if c[i] == '\''
            && i + 3 < c.len()
            && c[i + 1] == '\\'
            && let Some(k) = (i + 2..c.len().min(i + 12)).find(|&k| c[k] == '\'')
        {
            i = k + 1;
            continue;
        }
        if c[i] == '"' {
            let start = i;
            let mut j = i + 1;
            while j < c.len() && c[j] != '"' {
                j += if c[j] == '\\' { 2 } else { 1 };
            }
            let end = (j + 1).min(c.len());
            out.push((start, c[start..end].iter().collect()));
            i = end;
            continue;
        }
        i += 1;
    }
    out
}

/// One top-level item: its name (when it has one) and its text, attributes
/// included.
pub struct Item {
    pub name: Option<String>,
    pub text: String,
}

const KEYWORDS: &[&str] = &[
    "fn ",
    "async fn ",
    "const ",
    "static ",
    "struct ",
    "enum ",
    "impl",
    "mod ",
    "use ",
    "type ",
    "trait ",
    "macro_rules!",
];

fn without_visibility(line: &str) -> &str {
    for vis in ["pub(crate) ", "pub(super) ", "pub "] {
        if let Some(rest) = line.strip_prefix(vis) {
            return rest;
        }
    }
    line
}

fn is_item_line(line: &str) -> bool {
    let rest = without_visibility(line);
    KEYWORDS.iter().any(|k| rest.starts_with(k))
}

fn item_name(line: &str) -> Option<String> {
    let rest = without_visibility(line);
    for k in [
        "async fn ",
        "fn ",
        "const ",
        "static ",
        "struct ",
        "enum ",
        "mod ",
        "type ",
        "trait ",
    ] {
        if let Some(after) = rest.strip_prefix(k) {
            let name: String = after
                .chars()
                .take_while(|ch| ch.is_alphanumeric() || *ch == '_')
                .collect();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    None
}

/// Split comment-free text into its top-level items. An item starts at
/// column 0 with an attribute or an item keyword; the attributes stay with
/// the item they decorate.
pub fn items(text: &str) -> Vec<Item> {
    let mut out: Vec<Item> = Vec::new();
    let mut in_attrs = false;
    for line in text.split('\n') {
        let attr = line.starts_with("#[");
        let starts = attr || is_item_line(line);
        let continues_attrs = in_attrs && (attr || is_item_line(line));
        if (starts && !continues_attrs) || out.is_empty() {
            out.push(Item {
                name: None,
                text: String::new(),
            });
        }
        let item = out.last_mut().expect("one item");
        if item.name.is_none() && !attr {
            item.name = item_name(line);
        }
        item.text.push_str(line);
        item.text.push('\n');
        if attr {
            in_attrs = true;
        } else if !line.trim().is_empty() {
            in_attrs = false;
        }
    }
    out
}

/// Whether `item` holds a test that runs: a `#[test]` whose attribute group
/// has no `#[ignore`. A `#[cfg_attr(.., ignore ..)]` test runs under its
/// feature, so it counts as a test that runs.
pub fn holds_a_running_test(item: &Item) -> bool {
    let lines: Vec<&str> = item.text.split('\n').collect();
    for (i, line) in lines.iter().enumerate() {
        if line.trim() != "#[test]" {
            continue;
        }
        let mut lo = i;
        while lo > 0 && lines[lo - 1].trim_start().starts_with("#[") {
            lo -= 1;
        }
        let mut hi = i;
        while hi + 1 < lines.len() && lines[hi + 1].trim_start().starts_with("#[") {
            hi += 1;
        }
        if !lines[lo..=hi]
            .iter()
            .any(|l| l.trim_start().starts_with("#[ignore"))
        {
            return true;
        }
    }
    false
}

fn mentions(text: &str, name: &str) -> bool {
    let bytes = text.as_bytes();
    let ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    text.match_indices(name).any(|(at, _)| {
        let before = at == 0 || !ident(bytes[at - 1]);
        let end = at + name.len();
        let after = end >= bytes.len() || !ident(bytes[end]);
        before && after
    })
}

/// The items a running test reaches by name inside the file, the tests
/// themselves included.
pub fn reachable_from_running_tests(items: &[Item]) -> Vec<bool> {
    let mut reach: Vec<bool> = items.iter().map(holds_a_running_test).collect();
    let mut frontier: Vec<usize> = (0..items.len()).filter(|&i| reach[i]).collect();
    while let Some(i) = frontier.pop() {
        for (j, other) in items.iter().enumerate() {
            if reach[j] {
                continue;
            }
            if let Some(name) = &other.name
                && mentions(&items[i].text, name)
            {
                reach[j] = true;
                frontier.push(j);
            }
        }
    }
    reach
}

/// Every `.rs` file under `dir`, sorted.
pub fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|e| e == "rs") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Whether a string literal (with its quotes) names an operator work path:
/// the `planning` folder or a home directory.
pub fn is_operator_path_literal(literal: &str) -> bool {
    let body = literal
        .trim_start_matches('r')
        .trim_matches('#')
        .trim_matches('"');
    let rel = body.trim_start_matches(['/', '.']);
    body == "planning" || rel.starts_with("planning/") || body.contains("/home/")
}

/// The operator-path literals of `src` that a running test reaches. A
/// literal right after `contains(` is a check that a text holds no such
/// path, not a read of one.
pub fn operator_paths_in_running_tests(src: &str, whole_file: bool) -> Vec<String> {
    let text = strip_comments(src);
    let items = items(&text);
    let reach = reachable_from_running_tests(&items);
    let mut out = Vec::new();
    for (item, reached) in items.iter().zip(reach) {
        if !(reached || whole_file) {
            continue;
        }
        for (at, lit) in string_literals(&item.text) {
            let before: String = item.text[..item
                .text
                .char_indices()
                .nth(at)
                .map_or(item.text.len(), |(b, _)| b)]
                .chars()
                .rev()
                .take(40)
                .collect::<Vec<char>>()
                .into_iter()
                .rev()
                .collect();
            if is_operator_path_literal(&lit) && !before.trim_end().ends_with("contains(") {
                out.push(format!("{} {lit}", item.name.as_deref().unwrap_or("?")));
            }
        }
    }
    out
}
