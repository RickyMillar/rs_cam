//! WP15a — no production surface writes the session outside `apply`.
//!
//! # What this file guards
//!
//! `ProjectSession` publishes one mutation door,
//! `ProjectSession::apply`. The core sentry
//! (`crates/rs_cam_core/tests/setters_have_rows_wp15a.rs`) holds the
//! registry side of that property: every public setter has a row. This
//! file holds the SURFACE side: a row's `gui` and `cli` reach columns
//! match the constructions in the view and CLI sources. That no surface
//! calls a setter outside `apply` is now the compiler's: the setters are
//! `pub(crate)` (WP15b), so the registry's surface table cannot answer
//! for a write that did not happen (IMPLEMENTATION_PLAN §25 ruling 1).
//!
//! # The properties
//!
//! 1. (Deleted, W14a F156.) It refused a production line that names
//!    `session.<setter>(`. WP15b made every setter `pub(crate)`, so a
//!    call from viz or the CLI is a compile error, and each crate's own
//!    test build reports it. The scan re-checked what the compiler holds.
//! 2. Every row whose `gui` reach reads `Reached` is CONSTRUCTED in the
//!    view's own sources.
//! 3. Every row whose `cli` reach reads `Reached` is CONSTRUCTED in
//!    `crates/rs_cam_cli/src`, and no `cli: Skip` row is.
//!
//! Property 2 repeats `command_surface_completeness.rs` P1 by design:
//! this file is the one a reader opens after a WP15a flip, and the two
//! scans share the idiom rather than one importing the other. Its
//! converse — no `gui: Skip` row is constructed in the view — stays in
//! that file alone, and property 3 adds the CLI half of both.
//!
//! # Why a source scan
//!
//! A reach column is not a type. The registry publishes `CommandId::ALL`,
//! so a test can read every row; it publishes nothing about who builds
//! one. The evidence is the source text.

#![expect(
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::{Path, PathBuf};

use rs_cam_core::session::{CommandId, CommandKind, Reach};

// ── allowlists ───────────────────────────────────────────────────────

/// The rows the batch CLI reaches through a compute door, not a row.
///
/// `GenerateToolpath` is a `Job` row whose `cli` column reads `Reached`
/// because the CLI generates. It generates through
/// `ProjectSession::generate_toolpath`, a compute door, so no CLI
/// source builds the payload. The `Job`
/// programme closes this; WP15a records it.
const CLI_COMPUTE_DOOR_ROWS: &[CommandId] = &[CommandId::GenerateToolpath];

/// The four files that serve the MCP wire, and `controller/tests.rs`.
///
/// A property about the GUI surface must not read an MCP construction
/// as a GUI caller: WP4's delegating arm converts a `CoreRequest` into a
/// `Command` inside `app/mcp/commands.rs`, so most `gui: Skip` rows are
/// constructed there — correctly, and by the wire. The list matches
/// `command_surface_completeness.rs`.
///
/// P4 moved `app/mcp.rs`'s inline `mod tests` out to `app/mcp/tests.rs`.
/// That file constructs commands, so it joins the list; without the row
/// property 2 reads a test construction as a view construction.
const MCP_SOURCES: &[&str] = &[
    "src/app/mcp.rs",
    "src/app/mcp/commands.rs",
    // Geometry selection P2.7: the wire's selection rows (it reads the
    // `selection_status` and `list_model_items` query rows for MCP).
    "src/app/mcp/commands/selection.rs",
    "src/app/mcp/diagnostics.rs",
    "src/app/mcp/generation.rs",
    "src/app/mcp/project.rs",
    "src/app/mcp/simulation.rs",
    "src/app/mcp/tests.rs",
    "src/app/mcp/view.rs",
    "src/mcp_bridge.rs",
    "src/mcp_server.rs",
    // A DIRECTORY module since SHL-03: every theme child under it
    // is a fixture file too, so the entry names the directory and
    // the skip matches by prefix.
    "src/controller/tests",
];

// ── file helpers ─────────────────────────────────────────────────────

/// This crate's root directory.
fn viz_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The `rs_cam_cli` crate root, as a sibling of this one.
fn cli_root() -> PathBuf {
    viz_root().join("../rs_cam_cli")
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Every `.rs` file under `root`, recursively, in a stable order.
fn collect_rs(root: &Path) -> Vec<PathBuf> {
    assert!(root.is_dir(), "{} no longer exists", root.display());
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries =
            std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()));
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// Strip every `//` comment from one source text.
///
/// Prose names a setter to explain it. The strip is line-wise and
/// deliberately crude: it cannot tell a `//` inside a string literal
/// from a comment, and no setter name appears inside a string literal
/// in the scanned files.
fn strip_comments(text: &str) -> String {
    text.lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ── the inline `#[cfg(test)]` module removal ─────────────────────────

/// The index of the ITEM line a `#[cfg(test)]` attribute introduces.
///
/// Every such attribute in these two crates is followed by an
/// `#[allow(...)]` block that runs over several lines, and the module
/// declaration sits after it. The walk consumes whole attributes by
/// counting their brackets, then stops on the first line that opens an
/// item.
fn item_line_after_attributes(lines: &[&str], attribute: usize) -> Option<usize> {
    let mut index = attribute + 1;
    let mut depth = 0_i64;
    while index < lines.len() {
        let text = lines[index].trim();
        if depth == 0 && !text.is_empty() && !text.starts_with("#[") {
            return Some(index);
        }
        let opens = text.matches('(').count() + text.matches('[').count();
        let closes = text.matches(')').count() + text.matches(']').count();
        depth += opens as i64;
        depth -= closes as i64;
        index += 1;
    }
    None
}

/// One source text with every inline `#[cfg(test)] mod … { … }` removed.
///
/// The block runs from the attribute to the line that closes it, found
/// by counting braces. Both crates are `cargo fmt` clean, so the count
/// is not confused by a brace inside a string literal at this depth.
fn without_inline_test_modules(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut kept: Vec<&str> = Vec::with_capacity(lines.len());
    let mut index = 0_usize;
    while index < lines.len() {
        let line = lines[index];
        let opens_a_test_module = line.trim() == "#[cfg(test)]"
            && item_line_after_attributes(&lines, index).is_some_and(|item| {
                let item_text = lines[item].trim();
                item_text.starts_with("mod ") && item_text.ends_with('{')
            });
        if !opens_a_test_module {
            kept.push(line);
            index += 1;
            continue;
        }
        let mut depth = 0_i64;
        let mut opened = false;
        while index < lines.len() {
            let current = lines[index];
            depth += current.matches('{').count() as i64;
            if current.contains('{') {
                opened = true;
            }
            depth -= current.matches('}').count() as i64;
            index += 1;
            if opened && depth <= 0 {
                break;
            }
        }
    }
    kept.join("\n")
}

// ── P2 and P3 — a reach column is a claim about a caller ─────────────

/// Does `text` construct the registry row `id` of kind `kind`?
///
/// The needle is the payload enum's own spelling, `Command::<Id>(`,
/// with a guard against matching a longer identifier that ends in the
/// same characters. The idiom is `command_surface_completeness.rs`'s.
fn constructs(text: &str, kind: CommandKind, id: CommandId) -> bool {
    let enum_name = match kind {
        CommandKind::Command => "Command",
        CommandKind::Query => "Query",
        CommandKind::Job => "Job",
        CommandKind::UiCommand | CommandKind::UiQuery => {
            panic!("the core registry declares no view row")
        }
    };
    let needle = format!("{enum_name}::{id:?}(");
    text.match_indices(&needle).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        !before.is_some_and(|c| c.is_alphanumeric() || c == '_')
    })
}

/// The view's own sources, without the MCP wire.
///
/// Comments are stripped and inline `#[cfg(test)]` modules removed: a
/// construction inside a test module is not a caller, and reading one
/// as a caller would make property 3's converse pass on test code.
fn gui_source_text() -> String {
    let root = viz_root();
    let skip: Vec<PathBuf> = MCP_SOURCES.iter().map(|r| root.join(r)).collect();
    let mut kept = 0_usize;
    let text = collect_rs(&root.join("src"))
        .into_iter()
        .filter(|p| !skip.iter().any(|s| p == s || p.starts_with(s)))
        .inspect(|_| kept += 1)
        .map(|p| without_inline_test_modules(&strip_comments(&read(&p))))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        kept > 40,
        "the GUI-only source set collapsed to {kept} files"
    );
    text
}

/// The batch CLI's own sources, read the same way.
///
/// `crates/rs_cam_cli/src/command.rs` builds a `Command` inside its own
/// `#[cfg(test)]` module, so the removal is what keeps a test-only
/// construction out of the CLI caller set.
fn cli_source_text() -> String {
    let mut kept = 0_usize;
    let text = collect_rs(&cli_root().join("src"))
        .into_iter()
        .inspect(|_| kept += 1)
        .map(|p| without_inline_test_modules(&strip_comments(&read(&p))))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(kept > 3, "the CLI source set collapsed to {kept} files");
    text
}

#[test]
fn every_gui_reached_row_is_constructed_in_the_view() {
    let text = gui_source_text();
    let mut checked = 0_usize;
    let mut missing = Vec::new();
    for id in CommandId::ALL {
        if !matches!(id.surfaces().gui, Reach::Reached) {
            continue;
        }
        checked += 1;
        if !constructs(&text, id.kind(), *id) {
            missing.push(id.wire_name());
        }
    }
    assert!(
        missing.is_empty(),
        "these rows claim the GUI reaches them, and no view source \
         constructs one: {missing:?}. Either a caller went away, or the \
         row's gui column should say Skip and why."
    );
    assert!(checked > 0, "no core row claims a GUI reach");
}

#[test]
fn every_cli_reached_row_is_constructed_in_the_batch_command_line() {
    let text = cli_source_text();
    let mut checked = 0_usize;
    let mut missing = Vec::new();
    for id in CommandId::ALL {
        if !matches!(id.surfaces().cli, Reach::Reached) {
            continue;
        }
        if CLI_COMPUTE_DOOR_ROWS.contains(id) {
            continue;
        }
        checked += 1;
        if !constructs(&text, id.kind(), *id) {
            missing.push(id.wire_name());
        }
    }
    assert!(
        missing.is_empty(),
        "these rows claim the batch CLI reaches them, and no CLI source \
         constructs one: {missing:?}. Either a caller went away, or the \
         row's cli column should say Skip and why."
    );
    assert!(checked > 0, "no core row claims a CLI reach");
}

#[test]
fn no_cli_skipped_row_is_constructed_in_the_batch_command_line() {
    let text = cli_source_text();
    let mut checked = 0_usize;
    let mut present = Vec::new();
    for id in CommandId::ALL {
        if !matches!(id.surfaces().cli, Reach::Skip(_)) {
            continue;
        }
        checked += 1;
        if constructs(&text, id.kind(), *id) {
            present.push(id.wire_name());
        }
    }
    assert!(
        present.is_empty(),
        "these rows say the batch CLI does NOT reach them, and a CLI \
         source constructs one: {present:?}. The Skip reason went stale \
         when the caller landed — the package that adds a caller flips \
         the column."
    );
    assert!(checked > 0, "no core row declares a CLI skip");
}

/// Every compute-door exemption names a row that really claims the CLI.
///
/// An exemption parked on a row whose `cli` column reads `Skip` hides
/// nothing and misleads the next reader.
#[test]
fn every_cli_exemption_names_a_reached_row() {
    for id in CLI_COMPUTE_DOOR_ROWS {
        assert!(
            matches!(id.surfaces().cli, Reach::Reached),
            "`{}` is exempt from the CLI construction scan, and its cli \
             column no longer reads Reached. Drop the exemption.",
            id.wire_name()
        );
    }
}
