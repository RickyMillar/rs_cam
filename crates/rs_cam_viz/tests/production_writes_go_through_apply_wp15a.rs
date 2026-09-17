//! WP15a — no production surface writes the session outside `apply`.
//!
//! # What this file guards
//!
//! `ProjectSession` publishes one mutation door,
//! `ProjectSession::apply`. The core sentry
//! (`crates/rs_cam_core/tests/setters_have_rows_wp15a.rs`) holds the
//! registry side of that property: every public setter has a row. This
//! file holds the SURFACE side: no production file under
//! `crates/rs_cam_viz/src` or `crates/rs_cam_cli/src` calls a setter.
//! A surface that calls one writes the session without naming a row,
//! and the registry's surface table then answers for a write that did
//! not happen (IMPLEMENTATION_PLAN §25 ruling 1).
//!
//! # The three properties
//!
//! 1. No production line names `session.<setter>(`.
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
//! A method call is not a type. The registry publishes `CommandId::ALL`,
//! so a test can read every row; it publishes nothing about who calls a
//! setter. The evidence is the source text, as WP7's
//! `hatches_are_crate_private_wp7.rs` reads it.
//!
//! # Two limits a reader must know
//!
//! The needle is `session.<setter>(` after the scan joins the lines
//! rustfmt broke. A call through a receiver spelled otherwise —
//! `sess.add_tool(`, or a `&mut ProjectSession` parameter named
//! anything else — is NOT seen. The core sentry covers that gap from
//! the other side: such a call still reaches a setter that carries a
//! row. And the scan reads production files only; a test may call a
//! setter directly, which §25 ruling 2 records as a test-only residual
//! until WP15b.
//!
//! # Red before the fix
//!
//! I read 48 production call sites: 39 under `crates/rs_cam_viz/src`
//! in eight files, and 9 under `crates/rs_cam_cli/src` in three. The
//! core sentry is the package's red-first evidence; the red of THIS
//! scan is shown by putting one direct setter call back.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use rs_cam_core::session::{CommandId, CommandKind, Reach};

// ── allowlists ───────────────────────────────────────────────────────

/// The three doors. A door runs a row; it is not a setter.
///
/// `query` takes `&self` and never matches the `&mut self` scan. It is
/// listed anyway, so the allowlist names the door set the plan names.
const DOORS: &[&str] = &["apply", "query", "start"];

/// The five compute entry points.
///
/// Each one runs a generation, a simulation or a plan rather than
/// writing a field. §25 ruling 4 assigns them to the `Job` programme as
/// a second write surface, not to WP15a.
const COMPUTE_DOORS: &[&str] = &[
    "generate_toolpath",
    "generate_all",
    "run_simulation",
    "modulate_simulation_trace",
    "plan_multitool_finishing",
];

/// The rows the batch CLI reaches through a compute door, not a row.
///
/// `GenerateToolpath` is a `Job` row whose `cli` column reads `Reached`
/// because the CLI generates. It generates through
/// `ProjectSession::generate_toolpath`, which is one of
/// [`COMPUTE_DOORS`], so no CLI source builds the payload. The `Job`
/// programme closes this; WP15a records it.
const CLI_COMPUTE_DOOR_ROWS: &[CommandId] = &[CommandId::GenerateToolpath];

/// The four files that serve the MCP wire, and `controller/tests.rs`.
///
/// A property about the GUI surface must not read an MCP construction
/// as a GUI caller: WP4's delegating arm converts a `CoreRequest` into a
/// `Command` inside `app/mcp/commands.rs`, so most `gui: Skip` rows are
/// constructed there — correctly, and by the wire. The list matches
/// `command_surface_completeness.rs`.
const MCP_SOURCES: &[&str] = &[
    "src/app/mcp.rs",
    "src/app/mcp/commands.rs",
    "src/app/mcp/diagnostics.rs",
    "src/app/mcp/simulation.rs",
    "src/mcp_bridge.rs",
    "src/mcp_server.rs",
    "src/controller/tests.rs",
];

// ── non-vacuity bars ─────────────────────────────────────────────────

/// The lowest number of setter names the core scan must collect.
const MIN_SETTERS: usize = 50;

/// The lowest number of production `.rs` files the walk must read.
const MIN_FILES_WALKED: usize = 100;

/// The highest number of files the `#[cfg(test)] mod` resolver may drop.
///
/// The two crates declare nine such modules today. The bar catches a
/// resolver that starts excluding production code, not a new test.
const MAX_EXCLUDED_FILES: usize = 20;

// ── file helpers ─────────────────────────────────────────────────────

/// This crate's root directory.
fn viz_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The `rs_cam_core` crate root, as a sibling of this one.
fn core_root() -> PathBuf {
    viz_root().join("../rs_cam_core")
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

/// Join the lines `cargo fmt` broke inside one method chain.
///
/// Thirteen of the 48 sites WP15a routed read
/// `self\n.state\n.session\n.add_setup(`, and a line-wise
/// `session.<setter>(` pattern cannot see one. A line whose first
/// character is `.` continues the line above it, so the scan folds it
/// back before matching.
fn join_continuations(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('.') && !out.is_empty() {
            while out.ends_with(' ') {
                out.pop();
            }
            out.push_str(trimmed);
        } else {
            out.push('\n');
            out.push_str(line);
        }
    }
    out
}

// ── the `#[cfg(test)]` exclusion ─────────────────────────────────────

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

/// The file a `#[cfg(test)] mod <name>;` declaration names.
///
/// A module declared inside `foo.rs` lives in `foo/`; one declared
/// inside `mod.rs` or `lib.rs` lives beside it. Nine such files sit
/// under `crates/rs_cam_viz/src`: eight test modules and one shared
/// fixture builder, `compute/worker/test_fixture.rs`.
fn declared_module_file(declaring: &Path, name: &str) -> Option<PathBuf> {
    let stem = declaring.file_stem()?.to_str()?;
    let parent = declaring.parent()?;
    let dir = if stem == "mod" || stem == "lib" {
        parent.to_path_buf()
    } else {
        parent.join(stem)
    };
    let file = dir.join(format!("{name}.rs"));
    file.is_file().then_some(file)
}

/// Every file a `#[cfg(test)] mod <name>;` declaration excludes.
///
/// The compiler proves the gating: a module declared under
/// `#[cfg(test)]` is not in the production build. The failure the
/// resolver CAN have is naming the wrong file, so
/// [`guard_excluded_files`] reads the answer back.
fn cfg_test_module_files(sources: &[PathBuf]) -> BTreeSet<PathBuf> {
    let mut excluded = BTreeSet::new();
    for path in sources {
        let text = read(path);
        let lines: Vec<&str> = text.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            if line.trim() != "#[cfg(test)]" {
                continue;
            }
            let Some(item) = item_line_after_attributes(&lines, index) else {
                continue;
            };
            let trimmed = lines[item].trim();
            let Some(rest) = trimmed.strip_prefix("mod ") else {
                continue;
            };
            let Some(name) = rest.strip_suffix(';') else {
                continue;
            };
            let Some(file) = declared_module_file(path, name) else {
                continue;
            };
            excluded.insert(file);
        }
    }
    guard_excluded_files(&excluded);
    excluded
}

/// The exclusion set is small, holds no crate root, and holds tests.
///
/// A resolver that hid production code would show one of three shapes:
/// an exclusion set the size of the crate, a crate root among the
/// names, or a set that names no test at all. One shared fixture
/// builder carries no `#[test]` of its own, so the test check is a
/// property of the SET rather than of every member.
fn guard_excluded_files(excluded: &BTreeSet<PathBuf>) {
    assert!(
        !excluded.is_empty() && excluded.len() <= MAX_EXCLUDED_FILES,
        "the `#[cfg(test)] mod` resolver excluded {} file(s). Zero \
         means it stopped finding the test modules these crates \
         declare. More than {MAX_EXCLUDED_FILES} means it hides \
         production code from the scan.",
        excluded.len()
    );
    for path in excluded {
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        assert!(
            !matches!(stem, "lib" | "main" | "mod"),
            "the resolver excluded {}, which is a crate or module root. \
             A root is never declared under `#[cfg(test)]`.",
            path.display()
        );
    }
    assert!(
        excluded.iter().any(|path| read(path).contains("#[test]")),
        "the resolver excluded {} file(s) and not one of them declares \
         a `#[test]`, so it is no longer finding the test modules it is \
         built to find.",
        excluded.len()
    );
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

// ── the setter list, read from core ──────────────────────────────────

/// Every `pub fn … (&mut self` declared inside an `impl ProjectSession`
/// block under `crates/rs_cam_core/src/session/`, minus the allowlists.
///
/// The scan is SCOPED to those blocks, as the core sentry scopes its
/// own: `LoadedModel::adopt_geometry` and `CycleTime::fold` are public
/// `&mut self` methods in the same directory, and an unscoped scan
/// reads them as session setters.
fn session_setters() -> Vec<String> {
    let dir = core_root().join("src/session");
    let mut names = Vec::new();
    for path in collect_rs(&dir) {
        let text = read(&path);
        let lines: Vec<&str> = text.lines().collect();
        let mut index = 0_usize;
        while index < lines.len() {
            if lines[index] != "impl ProjectSession {" {
                index += 1;
                continue;
            }
            let mut end = index + 1;
            while end < lines.len() && lines[end] != "}" {
                end += 1;
            }
            for inner in (index + 1)..end {
                let line = lines[inner];
                if line.trim_start().starts_with("//") {
                    continue;
                }
                let Some(name) = public_fn_name(line) else {
                    continue;
                };
                if !signature(&lines, inner).contains("&mut self") {
                    continue;
                }
                if DOORS.contains(&name) || COMPUTE_DOORS.contains(&name) {
                    continue;
                }
                names.push(name.to_owned());
            }
            index = end;
        }
    }
    names.sort();
    names.dedup();
    names
}

/// The name of the function one declaration line opens.
///
/// Both spellings count. WP15b flipped the setters to `pub(crate) fn`,
/// and a reader that accepts only `pub fn ` returns an empty list after
/// that flip, which takes the non-vacuity guard below with it.
fn public_fn_name(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let rest = trimmed
        .strip_prefix("pub(crate) fn ")
        .or_else(|| trimmed.strip_prefix("pub fn "))?;
    let name = rest.split('(').next()?;
    let plain = name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    (!name.is_empty() && plain).then_some(name)
}

/// The whole signature of the declaration that starts at `start`.
///
/// A signature runs to the line that opens the body. Most setters put
/// `&mut self` on the line AFTER the opening paren, so a same-line
/// pattern misses them.
fn signature(lines: &[&str], start: usize) -> String {
    let mut out = String::new();
    for line in lines.iter().skip(start) {
        out.push(' ');
        out.push_str(line);
        if line.trim_end().ends_with('{') {
            break;
        }
    }
    out
}

// ── the scanned production text ──────────────────────────────────────

/// One production file, ready to match: comments stripped, inline test
/// modules removed, broken method chains joined.
fn scannable(path: &Path) -> String {
    join_continuations(&without_inline_test_modules(&strip_comments(&read(path))))
}

/// Every production `.rs` file of both surfaces, with the nine
/// `#[cfg(test)] mod` files dropped.
fn production_sources() -> Vec<PathBuf> {
    let mut sources = collect_rs(&viz_root().join("src"));
    sources.extend(collect_rs(&cli_root().join("src")));
    let excluded = cfg_test_module_files(&sources);
    sources.retain(|p| !excluded.contains(p));
    sources
}

// ── P1 — no production site calls a setter ───────────────────────────

#[test]
fn no_production_site_calls_a_session_setter() {
    let setters = session_setters();
    assert!(
        setters.len() >= MIN_SETTERS,
        "the core scan must collect at least {MIN_SETTERS} setter \
         names, or this file asserts nothing. I read {}",
        setters.len()
    );

    let sources = production_sources();
    assert!(
        sources.len() >= MIN_FILES_WALKED,
        "the production walk must read at least {MIN_FILES_WALKED} \
         files, or it asserts nothing. I read {}",
        sources.len()
    );

    let mut found: Vec<String> = Vec::new();
    for path in &sources {
        let text = scannable(path);
        for (number, line) in text.lines().enumerate() {
            for setter in &setters {
                if line.contains(&format!("session.{setter}(")) {
                    found.push(format!("{}:~{}: {setter}", path.display(), number + 1));
                }
            }
        }
    }

    assert!(
        found.is_empty(),
        "WP15a (§25 ruling 1): a production surface writes \
         `ProjectSession` through one door, `apply`. Build the row's \
         `Command` and take that door — `apply_quietly` or \
         `apply_controller_command` in the viz controller, \
         `apply_panel_command` in a draw site, \
         `rs_cam_cli::command::apply_command` in the batch CLI — and \
         flip the row's `gui` or `cli` column to `Reached` in the same \
         commit. I read {} direct setter call(s):\n  {}",
        found.len(),
        found.join("\n  ")
    );
}

/// The matcher sees a method chain `cargo fmt` broke over four lines.
///
/// Without [`join_continuations`] the scan above reads every such site
/// as clean, and thirteen of WP15a's forty-eight were that shape. This
/// is the positive control for the whole file.
#[test]
fn the_scan_matches_a_broken_method_chain() {
    // `concat!` and not a backslash-continued literal: `cargo fmt`
    // re-indents a continued literal and bakes the new spaces into the
    // string, which would silently change what this control measures.
    let sample = concat!(
        "        let _ = self\n",
        "            .state\n",
        "            .session\n",
        "            .add_setup(name, FaceUp::default());\n",
    );
    let joined = join_continuations(&strip_comments(sample));
    assert!(
        joined.contains("session.add_setup("),
        "the continuation joiner no longer folds a broken chain, so \
         `no_production_site_calls_a_session_setter` cannot see two \
         thirds of the shapes it is built to catch. I read: {joined}"
    );
    let untouched = "        let _ = other.add_setup(name);\n";
    assert!(
        !join_continuations(untouched).contains("session.add_setup("),
        "the needle must name the session receiver, or every same-named \
         method on another type reads as a violation"
    );
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
        .filter(|p| !skip.contains(p))
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
