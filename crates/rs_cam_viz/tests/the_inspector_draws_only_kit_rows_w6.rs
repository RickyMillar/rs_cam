//! W6 (toolpath inspector programme, 2026-10-30): the inspector draws only
//! kit rows, and only the placement reads a finding.
//!
//! # The defect this exists to catch
//!
//! `planning/toolpath_panel_review_2026-10-30/PLAN_EXECUTION.md`, "Why the
//! last passes did not hold": the premium programme built the kit, but
//! after 2026-10-13 each new feature in `ui/properties/` drew a raw row or
//! a raw message beside it. W1-W5 moved the toolpath inspector onto the kit.
//! This sentry closes the old doors, so a new raw row fails here.
//!
//! # The claims
//!
//! 1. No inspector file draws a raw row: no `ui.label(`, `ui.horizontal(`,
//!    `egui::Grid`, `DragValue::new(`, raw checkbox, `ValueRow`,
//!    `OptionalValueRow`, `KeyValueRow`, `param_grid` or `label_cell`, in
//!    code (a comment or a string literal does not count). The kit rows are
//!    `PropRow`, `ListBox`, `DataTable`, `Details`, `SectionHeader`,
//!    `Notice`, `OneLine` and `StatusLine` (`ui/components/`).
//!    [`RAW_ALLOWED`] holds the exceptions; it may only shrink. W3h
//!    emptied it.
//! 2. Only `placement.rs` reads a finding. The placement output holds
//!    `placement::Finding`, whose `Diagnostic` is private to the module, so
//!    the compiler holds the reads. This scan holds the rest: no inspector
//!    file names the type `Diagnostic`, except the files of
//!    [`DIAGNOSTIC_ALLOWED`], which build the list that `place` reads.
//!
//! The inspector (operator ruling R5) is the toolpath panel: the files of
//! [`FILES`] and every file under `toolpath_panel/` and `operations/`. The
//! Setup-side panels (machine, tool, stock, setup, obstacles, stock
//! changes, post, head, pins) take the same doors in a later pass.
//!
//! Red proof: add `ui.label("x");` to `toolpath_panel/tabs/feeds.rs`, or
//! read `d.face` of a `Diagnostic` in `tab_badges.rs`: claim 1 or claim 2
//! fails (the second one fails to compile first).
//!
//! ```text
//! cargo test -p rs_cam_viz -q --test the_inspector_draws_only_kit_rows_w6
//! ```

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure; the scanner indexes a char slice it bounds-checks"
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The inspector files outside the two folders, each with an anchor: a
/// `fn` that the file must hold, so a renamed or emptied file fails loudly.
const FILES: &[(&str, &str)] = &[
    ("toolpath_panel.rs", "fn draw_toolpath_panel"),
    ("tab_badges.rs", "fn draw_tool_row"),
    ("placement.rs", "fn place"),
    ("geometry_tab.rs", "fn draw_geometry_tab"),
    ("selection_section.rs", "fn draw_selection_section"),
    ("linking_dressup.rs", "fn param_row"),
    ("feeds_speeds.rs", "fn draw_speed_controls"),
    ("shoe_row.rs", "fn seed_gap"),
];

/// The folders of the inspector, each with the files it must hold and
/// their anchors. The walk also scans every other `.rs` file it finds.
const FOLDERS: &[(&str, &[(&str, &str)])] = &[
    (
        "toolpath_panel",
        &[
            ("toolpath_panel/header.rs", "fn draw_generate_row"),
            ("toolpath_panel/issues.rs", "fn draw_issue_stack"),
            ("toolpath_panel/tabs/mod.rs", "fn op_report"),
            ("toolpath_panel/tabs/geometry.rs", "fn draw_geometry_tab"),
            ("toolpath_panel/tabs/passes.rs", "fn draw_passes_tab"),
            ("toolpath_panel/tabs/feeds.rs", "fn draw_feeds_tab"),
            ("toolpath_panel/tabs/heights.rs", "fn draw_heights_tab"),
            ("toolpath_panel/tabs/linking.rs", "fn draw_linking_tab"),
        ],
    ),
    (
        "operations",
        &[
            ("operations/mod.rs", "fn height_row_display"),
            ("operations/boundary_2d.rs", "fn draw_face_params"),
            ("operations/surface_3d.rs", "fn draw_dropcutter_params"),
            ("operations/height_diagram.rs", "fn floor_runs"),
            ("operations/validate.rs", "fn validate_toolpath_config"),
            ("operations/registry.rs", "pub fn row"),
        ],
    ),
];

/// The raw row calls. A pattern that starts or ends with an identifier
/// character matches only at an identifier boundary, so `ValueRow` does
/// not match inside `OptionalValueRow` (that has its own pattern).
const RAW: &[&str] = &[
    "ui.label(",
    "ui.horizontal(",
    "egui::Grid",
    "Grid::new(",
    "DragValue::new(",
    "ui.checkbox(",
    "Checkbox::new(",
    "Checkbox::without_text(",
    "ValueRow",
    "OptionalValueRow",
    "KeyValueRow",
    "param_grid",
    "label_cell",
    "label_cell_sized",
];

/// An exception to claim 1: the file, the pattern, the count and the
/// reason. W6 target: none. The list may only shrink.
const RAW_ALLOWED: &[(&str, &str, usize, &str)] = &[];

/// The files that may name `Diagnostic`, with the count and the reason.
/// Each one builds the finding list that `placement::place` reads; none
/// reads the text of a finding.
const DIAGNOSTIC_ALLOWED: &[(&str, usize, &str)] = &[
    (
        "placement.rs",
        14,
        "THE reader: `Finding` wraps the `Diagnostic` and gives the face, the detail and the role",
    ),
    (
        "toolpath_panel/issues.rs",
        1,
        "`panel_diagnostics` builds THE list of the panel (core list + shoe finding)",
    ),
    (
        "operations/validate.rs",
        1,
        "`collect_diagnostics` builds the core list of the op for `panel_diagnostics`",
    ),
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui/properties")
}

/// Every inspector file: the path under `ui/properties/` and its source.
fn inspector_sources() -> BTreeMap<String, String> {
    let root = root();
    let mut out = BTreeMap::new();
    for (file, anchor) in FILES {
        let source = std::fs::read_to_string(root.join(file))
            .unwrap_or_else(|e| panic!("read the inspector file {file}: {e}"));
        assert!(
            source.contains(anchor),
            "non-vacuity: {file} no longer holds `{anchor}`; update FILES"
        );
        let _ = out.insert((*file).to_owned(), source);
    }
    for (folder, expected) in FOLDERS {
        let dir = root.join(folder);
        assert!(
            dir.is_dir(),
            "the inspector folder {} is missing; update FOLDERS",
            dir.display()
        );
        let mut stack = vec![dir];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("read an inspector folder") {
                let path = entry.expect("a folder entry").path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    let name = path
                        .strip_prefix(&root)
                        .expect("under the root")
                        .to_string_lossy()
                        .replace('\\', "/");
                    let source = std::fs::read_to_string(&path).expect("read a source file");
                    let _ = out.insert(name, source);
                }
            }
        }
        for (file, anchor) in *expected {
            let source = out
                .get(*file)
                .unwrap_or_else(|| panic!("the walk of {folder}/ did not find {file}"));
            assert!(
                source.contains(anchor),
                "non-vacuity: {file} no longer holds `{anchor}`; update FOLDERS"
            );
        }
    }
    out
}

/// The source with every comment and every string or char literal blanked
/// to spaces (line breaks stay). What is left is code.
fn code_only(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    let blank = |out: &mut String, c: char| out.push(if c == '\n' { '\n' } else { ' ' });
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        // A line comment, a doc comment included.
        if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                blank(&mut out, chars[i]);
                i += 1;
            }
            continue;
        }
        // A block comment; Rust block comments nest.
        if c == '/' && next == Some('*') {
            let mut depth = 0_usize;
            while i < chars.len() {
                if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                    depth += 1;
                    blank(&mut out, '/');
                    blank(&mut out, '*');
                    i += 2;
                } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    blank(&mut out, '*');
                    blank(&mut out, '/');
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    blank(&mut out, chars[i]);
                    i += 1;
                }
            }
            continue;
        }
        // A raw string: r"…", r#"…"#, br"…".
        let prev_ident = i > 0 && ident(chars[i - 1]);
        if c == 'r' && !prev_ident && matches!(next, Some('"' | '#')) {
            let mut j = i + 1;
            let mut hashes = 0;
            while chars.get(j) == Some(&'#') {
                hashes += 1;
                j += 1;
            }
            if chars.get(j) == Some(&'"') {
                let mut k = j + 1;
                loop {
                    if k >= chars.len() {
                        break;
                    }
                    if chars[k] == '"' && (0..hashes).all(|h| chars.get(k + 1 + h) == Some(&'#')) {
                        k += 1 + hashes;
                        break;
                    }
                    k += 1;
                }
                for &ch in &chars[i..k.min(chars.len())] {
                    blank(&mut out, ch);
                }
                i = k;
                continue;
            }
        }
        // A string literal, with escapes.
        if c == '"' {
            blank(&mut out, c);
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' {
                    blank(&mut out, chars[i]);
                    i += 1;
                }
                if i < chars.len() {
                    blank(&mut out, chars[i]);
                    i += 1;
                }
            }
            if i < chars.len() {
                blank(&mut out, chars[i]);
                i += 1;
            }
            continue;
        }
        // A char literal ('x', '\n', '\'', '\u{..}'); a lifetime is code.
        if c == '\'' {
            let end = if next == Some('\\') {
                (i + 2..chars.len().min(i + 12)).find(|&k| chars[k] == '\'')
            } else if chars.get(i + 2) == Some(&'\'') {
                Some(i + 2)
            } else {
                None
            };
            if let Some(end) = end {
                for &ch in &chars[i..=end] {
                    blank(&mut out, ch);
                }
                i = end + 1;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

/// How many times `pattern` occurs in `code` at identifier boundaries.
fn count(code: &str, pattern: &str) -> usize {
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    let starts_ident = pattern.chars().next().is_some_and(ident);
    let ends_ident = pattern.chars().last().is_some_and(ident);
    code.match_indices(pattern)
        .filter(|(at, _)| {
            let before = code[..*at].chars().next_back();
            let after = code[at + pattern.len()..].chars().next();
            !(starts_ident && before.is_some_and(ident))
                && !(ends_ident && after.is_some_and(ident))
        })
        .count()
}

/// Claim 1.
#[test]
fn the_inspector_draws_only_kit_rows_w6() {
    let sources = inspector_sources();
    // Non-vacuity: the scan reads the root files and both folders.
    assert!(
        sources.len() >= FILES.len() + 14,
        "the scan read {} files",
        sources.len()
    );
    let mut found: BTreeMap<(String, &str), usize> = BTreeMap::new();
    for (file, source) in &sources {
        let code = code_only(source);
        for pattern in RAW {
            let n = count(&code, pattern);
            if n > 0 {
                let _ = found.insert((file.clone(), *pattern), n);
            }
        }
    }
    let allowed: BTreeMap<(String, &str), usize> = RAW_ALLOWED
        .iter()
        .map(|(file, pattern, n, _)| (((*file).to_owned(), *pattern), *n))
        .collect();
    let mut failures = Vec::new();
    for ((file, pattern), n) in &found {
        match allowed.get(&(file.clone(), *pattern)) {
            Some(cap) if n <= cap => {}
            Some(cap) => failures.push(format!(
                "{file}: `{pattern}` {n} times, the allow-list says {cap}"
            )),
            None => failures.push(format!(
                "{file}: `{pattern}` {n} times; draw the row with the kit (PropRow, ListBox, \
                 DataTable, Details, SectionHeader, Notice, OneLine, StatusLine)"
            )),
        }
    }
    // The list only shrinks: an entry above its count is stale.
    for ((file, pattern), cap) in &allowed {
        let n = found.get(&(file.clone(), *pattern)).copied().unwrap_or(0);
        if n < *cap {
            failures.push(format!(
                "RAW_ALLOWED says {file} `{pattern}` {cap}, the file has {n}: lower the entry"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Claim 2.
#[test]
fn only_the_placement_reads_a_finding_w6() {
    let sources = inspector_sources();
    let mut failures = Vec::new();
    let mut seen = 0_usize;
    for (file, source) in &sources {
        let n = count(&code_only(source), "Diagnostic");
        seen += n;
        let allowed = DIAGNOSTIC_ALLOWED
            .iter()
            .find(|(f, _, _)| f == file)
            .map_or(0, |(_, cap, _)| *cap);
        if n != allowed {
            failures.push(format!(
                "{file}: names `Diagnostic` {n} times, DIAGNOSTIC_ALLOWED says {allowed}; read a \
                 finding through `placement::Finding`"
            ));
        }
    }
    assert!(seen > 0, "non-vacuity: placement.rs names `Diagnostic`");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The scan's own rules: a comment, a string literal and a char literal
/// are not code; an identifier boundary holds.
#[test]
fn the_scan_reads_code_only_w6() {
    let code = code_only(
        "let a = \"ui.label(\"; // ui.label(\n/* ui.horizontal( /* nested */ */ let q = '\"';\n\
         let r = r#\"ui.label(\"#; ui.label(\"x\"); let l: &'a str = \"\\\"ui.label(\";",
    );
    assert_eq!(count(&code, "ui.label("), 1, "{code}");
    assert_eq!(count(&code, "ui.horizontal("), 0, "{code}");
    assert_eq!(count("OptionalValueRow::new", "ValueRow"), 0);
    assert_eq!(count("ValueRow::new", "ValueRow"), 1);
    assert_eq!(count("DiagnosticFix", "Diagnostic"), 0);
    assert_eq!(count("&Diagnostic,", "Diagnostic"), 1);
}
