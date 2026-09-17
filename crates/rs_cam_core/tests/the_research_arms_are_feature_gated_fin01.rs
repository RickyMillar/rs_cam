//! FIN-01 — the three research arms stay behind the `research` feature, and
//! no product file reaches them.
//!
//! # What this file guards
//!
//! `finish::conformal_spiral` (with its `conformal_spiral/` folder),
//! `finish::direction_field` and `finish::spiral_finish_compact` are measured
//! research candidates. No product path calls them. They are 7431 lines, 22 %
//! of `src/finish/`, and before FIN-01 every product build compiled and linted
//! all of them.
//!
//! FIN-01 puts the three `pub mod` lines behind `#[cfg(feature = "research")]`
//! and gives the eight evidence harnesses that read them a
//! `required-features = ["research"]` row in `Cargo.toml`.
//!
//! The gate holds only while two things stay true, so this file asserts both.
//!
//! # The two arms
//!
//! 1. `src/finish/mod.rs` declares each of the three exactly once, and the
//!    line above each declaration is `#[cfg(feature = "research")]`. All
//!    three names must be accounted for, so a module that loses its
//!    attribute — or that a rename hides from arm 2 — fails here.
//! 2. No `.rs` file under `crates/rs_cam_core/src`, outside the research set
//!    and outside `finish/mod.rs`, names one of the three in code. A comment
//!    line may name them: `metrology/floor.rs` and `metrology/monge.rs`
//!    record where an instrument came from, and prose costs no build time.
//!
//! # Non-vacuity
//!
//! Arm 1 counts the three names it accounted for and fails below three.
//! Arm 2 walks at least [`MIN_FILES_WALKED`] files, and it runs the same
//! matcher over the research set first: that scan must find hits, or the
//! matcher reads nothing and the empty result outside the set means nothing.
//!
//! # Red before the fix
//!
//! Arm 1 reads a bare `pub mod conformal_spiral;` with no attribute above it.
//! Arm 2 reads `crest_lines.rs` once a `use crate::finish::direction_field`
//! line goes back in. Both defects were injected and both arms went red.
//!
//! The teeth check ran under `--features research`, and it has to: with the
//! default features either defect stops the crate compiling before a test can
//! run. That is the division of labour. Under the default features the
//! compiler is the primary guard and these two arms are belt and braces.
//! Under `research` — the feature set the CI clippy line passes — the crate
//! still compiles with the defect in place, and this file is the only thing
//! that fires.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

/// The three research modules, as they appear in `src/finish/mod.rs`.
const RESEARCH_MODULES: &[&str] = &[
    "conformal_spiral",
    "direction_field",
    "spiral_finish_compact",
];

/// The attribute every research `pub mod` line carries.
const GATE: &str = "#[cfg(feature = \"research\")]";

/// Paths under `src/`, relative and with `/` separators, that hold research
/// code. A path that starts with one of these is inside the research set.
///
/// `finish/conformal_spiral` covers both `conformal_spiral.rs` and every file
/// in `conformal_spiral/`, so a new file in that folder needs no edit here.
const RESEARCH_PATHS: &[&str] = &[
    "finish/conformal_spiral",
    "finish/direction_field.rs",
    "finish/spiral_finish_compact.rs",
];

/// The facade that declares the three modules. Arm 1 reads it; arm 2 skips it.
const FACADE: &str = "finish/mod.rs";

/// The lowest number of files arm 2 must visit. `src/` held 314 `.rs` files on
/// 2026-09-17. A walk that reads a handful proves nothing.
const MIN_FILES_WALKED: usize = 250;

/// The lowest number of in-set hits the matcher must find. The three modules
/// name each other: `conformal_spiral.rs`, `conformal_spiral/spiral_build.rs`,
/// `conformal_spiral/tests.rs` and `spiral_finish_compact.rs` each carry at
/// least one `use` line.
const MIN_RESEARCH_HITS: usize = 4;

/// This crate's `src` directory.
fn src_root() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    assert!(root.is_dir(), "{} no longer exists", root.display());
    root
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

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

/// Whether `line` carries prose. A `//`, `///` or `//!` line explains where an
/// instrument came from; it compiles nothing.
fn is_comment(line: &str) -> bool {
    line.trim_start().starts_with("//")
}

/// Whether `haystack` names `needle` as a whole identifier.
///
/// `orient_direction_field` contains `direction_field` but is a different
/// name, so a plain `contains` would read a false hit.
fn names_identifier(haystack: &str, needle: &str) -> bool {
    let is_word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut from = 0_usize;
    while let Some(offset) = haystack[from..].find(needle) {
        let start = from + offset;
        let end = start + needle.len();
        let before_ok = haystack[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !is_word(c));
        let after_ok = haystack[end..].chars().next().is_none_or(|c| !is_word(c));
        if before_ok && after_ok {
            return true;
        }
        from = end;
    }
    false
}

/// The file's path under `src/`, with `/` separators.
fn relative(path: &Path) -> String {
    path.strip_prefix(src_root())
        .unwrap_or_else(|_| panic!("{} is not under src/", path.display()))
        .to_string_lossy()
        .replace('\\', "/")
}

/// Every code line in `path` that names one of the three modules.
fn hits(path: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for (number, raw) in read(path).lines().enumerate() {
        if is_comment(raw) {
            continue;
        }
        for module in RESEARCH_MODULES {
            if names_identifier(raw, module) {
                out.push(format!("{}:{}: {}", relative(path), number + 1, raw.trim()));
                break;
            }
        }
    }
    out
}

// ── arm 1: the gate on the declarations ─────────────────────────────────

/// ARM 1. Each research module is declared once in `src/finish/mod.rs`, under
/// `#[cfg(feature = "research")]`.
///
/// A declaration without the attribute puts 7.4k lines back into every
/// product build, and nothing else here would read it: arm 2 skips the facade
/// on purpose, because the facade must name the modules to declare them.
#[test]
fn the_facade_gates_every_research_module() {
    let facade = src_root().join(FACADE);
    let text = read(&facade);
    let lines: Vec<&str> = text.lines().collect();

    let mut accounted = 0_usize;
    let mut ungated: Vec<String> = Vec::new();
    for module in RESEARCH_MODULES {
        let declaration = format!("pub mod {module};");
        let sites: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.trim() == declaration)
            .map(|(index, _)| index)
            .collect();
        assert_eq!(
            sites.len(),
            1,
            "`{module}` must be declared exactly once in {FACADE}. I read {} \
             declaration(s)",
            sites.len()
        );
        let index = sites[0];
        let above = index.checked_sub(1).map(|i| lines[i].trim()).unwrap_or("");
        if above != GATE {
            ungated.push(format!("{FACADE}:{}: {declaration}", index + 1));
        }
        accounted += 1;
    }

    assert_eq!(
        accounted,
        RESEARCH_MODULES.len(),
        "all {} research modules must be accounted for, or this arm asserts \
         nothing. I accounted for {accounted}",
        RESEARCH_MODULES.len()
    );
    assert!(
        ungated.is_empty(),
        "FIN-01: every research module sits behind `{GATE}`. No product path \
         calls these three, and they are 22 % of `src/finish/`. The eight \
         evidence harnesses that read them carry \
         `required-features = [\"research\"]` in `Cargo.toml`. I read {} \
         ungated declaration(s):\n  {}",
        ungated.len(),
        ungated.join("\n  ")
    );
}

// ── arm 2: no product reader ────────────────────────────────────────────

/// ARM 2. No product file under `src/` names a research module in code.
///
/// The gate only saves build time while this holds: one `use` line from a
/// module that is always compiled pulls the whole arm back in, and the
/// default build then fails to compile instead of shrinking.
#[test]
fn no_product_file_names_a_research_module() {
    let mut files = Vec::new();
    collect_rs(&src_root(), &mut files);
    files.sort();
    assert!(
        files.len() >= MIN_FILES_WALKED,
        "the walk must visit at least {MIN_FILES_WALKED} files, or this arm \
         passes on a tree it never read. I read {}",
        files.len()
    );

    let mut in_set = 0_usize;
    let mut outside: Vec<String> = Vec::new();
    for path in &files {
        let rel = relative(path);
        if RESEARCH_PATHS.iter().any(|p| rel.starts_with(p)) {
            in_set += hits(path).len();
        } else if rel != FACADE {
            outside.extend(hits(path));
        }
    }

    assert!(
        in_set >= MIN_RESEARCH_HITS,
        "the matcher must read the research set itself, or an empty result \
         outside it proves nothing. I read {in_set} in-set hit(s), and at \
         least {MIN_RESEARCH_HITS} are there: the three modules name each \
         other"
    );
    assert!(
        outside.is_empty(),
        "FIN-01: `conformal_spiral`, `direction_field` and \
         `spiral_finish_compact` are research arms behind the `research` \
         feature. A product file that names one in code either breaks the \
         default build or drags the arm back into it. Name it in a comment, \
         or move the code you need out of the research set. I read {} \
         line(s):\n  {}",
        outside.len(),
        outside.join("\n  ")
    );
}
