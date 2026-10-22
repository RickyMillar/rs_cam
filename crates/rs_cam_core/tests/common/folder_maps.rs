//! The folder `CLAUDE.md` file-map checker (F140), shared by the core sentry
//! and its viz twin.
//!
//! It is not declared in `common/mod.rs`: a sentry includes it with
//! `#[path = "common/folder_maps.rs"] mod folder_maps;` (the viz twin with a
//! path into this crate), so only the two map sentries compile it.
//!
//! The rules, for each `CLAUDE.md` under `<crate>/src/`:
//!
//! 1. Every backticked token that ends in `.rs` resolves. A `{a,b}` group
//!    expands to each name. A name with `*` is a glob over the file names
//!    of the folder tree. A name with `/` resolves against the folder, the
//!    crate `src/`, the crate root or the repo root, or ends a path in the
//!    folder tree. A bare name is a file in the folder tree or a root file
//!    of the crate `src/`.
//! 2. Every `.rs` file directly in the folder is named by a token of the
//!    folder file or of an ancestor folder file (a child file may hand its
//!    map to its parent), or it is in the caller's exclusion list.
//!    `mod.rs` and `tests.rs` are excluded everywhere.

#![allow(
    dead_code,
    reason = "each test binary that includes this module uses only part of it"
)]

use std::path::{Path, PathBuf};

/// The file names every folder may leave out of its map.
pub const ALWAYS_EXCLUDED: [&str; 2] = ["mod.rs", "tests.rs"];

/// What one scan of a crate found.
#[derive(Debug, Default)]
pub struct MapReport {
    /// The folder files the scan read.
    pub folder_files: usize,
    /// The `.rs` tokens that resolved.
    pub resolved: usize,
    /// `<folder file>: <token>` for each token that resolves to nothing.
    pub unresolved: Vec<String>,
    /// `<folder>/<file>` for each `.rs` file that no map names.
    pub unlisted: Vec<String>,
    /// `<folder>/<file>` exclusions that matched a real unlisted file.
    pub exclusions_used: usize,
}

/// The text inside each single-backtick span of `text`.
pub fn backtick_spans(text: &str) -> Vec<&str> {
    text.split('`').skip(1).step_by(2).collect()
}

/// The path-like tokens of one span that end in `.rs`. A comma separates
/// two tokens except inside a `{..}` group.
pub fn rs_tokens(span: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0_i32;
    for ch in span.chars().chain(std::iter::once(' ')) {
        let keep = ch.is_ascii_alphanumeric()
            || matches!(ch, '_' | '.' | '/' | '*' | '-' | '{' | '}')
            || (ch == ',' && depth > 0);
        if keep {
            match ch {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
            cur.push(ch);
        } else {
            if cur.ends_with(".rs") {
                out.push(std::mem::take(&mut cur));
            }
            cur.clear();
            depth = 0;
        }
    }
    out
}

/// `a/{b,c}.rs` as `a/b.rs` and `a/c.rs`. Groups expand left to right.
pub fn expand_braces(token: &str) -> Vec<String> {
    let Some(open) = token.find('{') else {
        return vec![token.to_owned()];
    };
    let Some(close_rel) = token[open..].find('}') else {
        return vec![token.to_owned()];
    };
    let close = open + close_rel;
    let (head, alts, tail) = (&token[..open], &token[open + 1..close], &token[close + 1..]);
    alts.split(',')
        .flat_map(|alt| expand_braces(&format!("{head}{alt}{tail}")))
        .collect()
}

/// A glob with `*` only: `*` matches any run of characters.
pub fn glob_match(pattern: &str, name: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == name;
    }
    let first = parts[0];
    let last = parts[parts.len() - 1];
    if !name.starts_with(first) || !name[first.len()..].ends_with(last) {
        return false;
    }
    let mut rest = &name[first.len()..name.len() - last.len()];
    for mid in &parts[1..parts.len() - 1] {
        match rest.find(mid) {
            Some(at) => rest = &rest[at + mid.len()..],
            None => return false,
        }
    }
    true
}

/// Every `.rs` file under `dir`, as a path relative to `dir` with `/`.
fn rs_tree(dir: &Path) -> Vec<String> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(base, &path, out);
            } else if path.extension().is_some_and(|e| e == "rs")
                && let Ok(rel) = path.strip_prefix(base)
            {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

/// The `.rs` names directly in `dir`.
fn rs_children(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()))
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "rs"))
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect();
    out.sort();
    out
}

/// Every folder under `src` that holds a `CLAUDE.md`.
fn folders_with_a_map(src: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.join("CLAUDE.md").is_file() {
                    out.push(path.clone());
                }
                walk(&path, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(src, &mut out);
    out.sort();
    out
}

/// The expanded `.rs` tokens of the `CLAUDE.md` in `dir`.
fn map_tokens(dir: &Path) -> Vec<String> {
    let path = dir.join("CLAUDE.md");
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    backtick_spans(&text)
        .into_iter()
        .flat_map(rs_tokens)
        .flat_map(|t| expand_braces(&t))
        .map(|t| t.strip_prefix("./").map(str::to_owned).unwrap_or(t))
        .collect()
}

fn last_component(token: &str) -> &str {
    token.rsplit('/').next().unwrap_or(token)
}

/// Does `token`, named in the map of `dir`, resolve?
fn resolves(token: &str, dir: &Path, tree: &[String], src: &Path, crate_root: &Path) -> bool {
    let repo = crate_root.join("../..");
    if token.contains('*') {
        return if token.contains('/') {
            tree.iter()
                .any(|p| glob_match(token, p) || glob_match(&format!("*/{token}"), p))
        } else {
            tree.iter().any(|p| glob_match(token, last_component(p)))
        };
    }
    if token.contains('/') {
        let suffix = format!("/{token}");
        return [dir, src, crate_root, repo.as_path()]
            .iter()
            .any(|base| base.join(token).is_file())
            || tree.iter().any(|p| p.ends_with(&suffix));
    }
    tree.iter().any(|p| last_component(p) == token) || src.join(token).is_file()
}

/// Scan every folder map of the crate at `crate_root`.
///
/// `excluded` holds `(folder relative to src, file name)` pairs that a map
/// may leave out on purpose.
pub fn scan_crate(crate_root: &Path, excluded: &[(&str, &str)]) -> MapReport {
    let src = crate_root.join("src");
    let mut report = MapReport::default();
    for dir in folders_with_a_map(&src) {
        report.folder_files += 1;
        let rel = dir
            .strip_prefix(&src)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        let tree = rs_tree(&dir);
        let own = map_tokens(&dir);
        for token in &own {
            if resolves(token, &dir, &tree, &src, crate_root) {
                report.resolved += 1;
            } else {
                report
                    .unresolved
                    .push(format!("{rel}/CLAUDE.md: `{token}`"));
            }
        }
        // The names this folder's map and every ancestor map give.
        let mut names: Vec<String> = Vec::new();
        let mut at = dir.clone();
        while at.starts_with(&src) && at != src {
            if at.join("CLAUDE.md").is_file() {
                names.extend(map_tokens(&at).iter().map(|t| last_component(t).to_owned()));
            }
            if !at.pop() {
                break;
            }
        }
        for file in rs_children(&dir) {
            if ALWAYS_EXCLUDED.contains(&file.as_str()) {
                continue;
            }
            if names.iter().any(|n| glob_match(n, &file)) {
                continue;
            }
            if excluded.iter().any(|(f, n)| *f == rel && *n == file) {
                report.exclusions_used += 1;
                continue;
            }
            report.unlisted.push(format!("{rel}/{file}"));
        }
    }
    report
}
