//! WP19 — every `Effects` answer reaches the view, or says why not.
//!
//! # What this file guards
//!
//! A core mutation answers with an [`rs_cam_core::session::Effects`].
//! The answer carries two things the view must mirror: `stale`, the
//! toolpath indices whose generation-input revision moved, and
//! `simulation_cleared`, which says the session dropped its simulation.
//! The session and the viewport hold ONE simulation (WP11b, N12 item
//! 10), so a session that drops it must not leave the viewport showing
//! one. `controller/io.rs`'s `adopt_post_effects` is the model.
//!
//! A production site that DISCARDS the answer mirrors neither. This
//! file finds every such site and demands a stated reason.
//!
//! # The two shapes of a discard
//!
//! 1. `let _ = <producer>(…);` — the answer is thrown away by name.
//! 2. `if let Err(…) = <producer>(…) { … }` — the shape binds the error
//!    alone, so the `Ok` answer has no binding at all. The shape itself
//!    proves the discard; no block scan is needed.
//!
//! `crates/rs_cam_viz/src/app/input.rs` held shape 2 at `10263b4d`, and
//! that site is the red this file was written against.
//!
//! # Needle 1 is a RATCHET, not a defect finder
//!
//! Every `let _ =` site the census found is justified and allowlisted,
//! so shape 1 is green on its own. It holds the line: the next such
//! site must state its reason in [`JUSTIFIED`] or fail here. Its own
//! red is shown the way `production_writes_go_through_apply_wp15a.rs`
//! shows its — by putting one unjustified discard back, or by deleting
//! an allowlist entry, which fires
//! [`every_justified_discard_still_names_a_real_site`].
//!
//! # Why a source scan
//!
//! `Effects` is `#[must_use]`, and its own doc accepts a stated
//! `let _ =`. Twenty-one production sites take it for a real reason, so
//! the compiler cannot be the instrument (IMPLEMENTATION_PLAN §19). The
//! evidence is the source text.
//!
//! # Two limits a reader must know
//!
//! The needle is `.<producer>(` after the scan joins the lines
//! `cargo fmt` broke, plus the CLI's free function `apply_command(`. A
//! call through a receiver this scan cannot name is not seen. And the
//! scan reads production files only: the nine `#[cfg(test)] mod` files
//! are dropped, and an inline `#[cfg(test)] mod … { … }` is removed
//! from every file it reads.
//!
//! The reader is the WP15a scan's, copied rather than imported: there
//! is no shared module under `crates/rs_cam_viz/tests/`, and that file
//! says the two scans share the idiom. `join_continuations` is NOT
//! copied — it folds a line whose first character is `.`, and three of
//! the seven viz sites wrap after `let _ =` instead.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

// ── allowlists ───────────────────────────────────────────────────────

/// A production site that discards an `Effects` for a stated reason.
struct Justified {
    /// The file, as a suffix of its path.
    path: &'static str,
    /// The row that makes the site unique, as the payload's own
    /// spelling. It is matched over the statement AND the
    /// [`LOOKBACK`] lines above it, because two of these sites build
    /// the payload into a local first.
    needle: &'static str,
    /// Why the answer has nothing to reach.
    reason: &'static str,
}

/// The six viz sites that discard an answer for a stated reason.
const JUSTIFIED: &[Justified] = &[
    Justified {
        path: "src/app/mcp.rs",
        needle: "Command::SetToolpathDebugOptions(",
        reason: "a debug capture is an output of a generation, never an input to one, \
                 so the row moves no revision and `stale` is empty",
    },
    Justified {
        path: "src/controller/events/compute.rs",
        needle: "Command::SetToolpathDebugOptions(",
        reason: "the same row from the controller, with the same empty `stale`",
    },
    Justified {
        path: "src/controller/events/compute.rs",
        needle: "Command::ForgetResult(",
        reason: "`forget_core_result` is a free function holding no `AppState`, \
                 so the caller owns the runtime row it is already writing",
    },
    Justified {
        path: "src/controller/events/compute.rs",
        needle: "Command::AdoptSimulation(",
        reason: "storing a simulation leaves the session's `Some`, so `stale` is empty \
                 and `simulation_cleared` is false (`session/command.rs`)",
    },
    Justified {
        path: "src/controller/io.rs",
        needle: "Command::SetProjectName(",
        reason: "a builder over a session no surface has adopted; the caller rebuilds \
                 `GuiState` from scratch, so no runtime row exists to stamp",
    },
    Justified {
        path: "src/controller/io.rs",
        needle: "Command::ReplaceSetupsAndToolpaths(",
        reason: "the same builder, over the same unadopted session",
    },
];

/// The batch CLI holds no runtime row to stamp.
///
/// `rg -n "stale_since" crates/rs_cam_cli/src` is empty: the crate
/// carries no `ToolpathRuntime`, and it generates every enabled
/// operation in the run, so a stale set has no consumer there. One
/// rule, not fifteen comments. The reason lives on `apply_command`'s
/// own doc in `crates/rs_cam_cli/src/command.rs`.
const CLI_HOLDS_NO_RUNTIME_ROWS: &str =
    "the batch CLI holds no `ToolpathRuntime` and no `stale_since`";

/// The CLI's own door, a free function rather than a method.
const CLI_DOOR: &str = "apply_command(";

// ── bars ─────────────────────────────────────────────────────────────

/// The lowest number of producer names the core scan must collect.
///
/// I read 56.
const MIN_PRODUCERS: usize = 50;

/// The lowest number of production `.rs` files the walk must read.
///
/// I read 127.
const MIN_FILES_WALKED: usize = 100;

/// The highest number of files the `#[cfg(test)] mod` resolver may drop.
const MAX_EXCLUDED_FILES: usize = 20;

/// The lowest number of CLI sites the one path rule must exempt.
///
/// I read 15. A rule that exempts nothing is a rule nobody reads.
const MIN_CLI_EXEMPTED: usize = 5;

/// How far above a statement the allowlist needle may look.
///
/// `forget_core_result` and the `AdoptSimulation` site both build the
/// payload into a local on an earlier line, so the statement alone does
/// not name the row.
const LOOKBACK: usize = 12;

/// The highest number of lines one accumulated statement may run to.
const MAX_STATEMENT_LINES: usize = 20;

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
/// Prose names a row to explain it. The strip is line-wise and
/// deliberately crude: it cannot tell a `//` inside a string literal
/// from a comment, and no row name appears inside a string literal in
/// the scanned files.
fn strip_comments(text: &str) -> String {
    text.lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ── the `#[cfg(test)]` exclusion ─────────────────────────────────────

/// The index of the ITEM line a `#[cfg(test)]` attribute introduces.
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

/// Every production `.rs` file of both surfaces, with the nine
/// `#[cfg(test)] mod` files dropped.
fn production_sources() -> Vec<PathBuf> {
    let mut sources = collect_rs(&viz_root().join("src"));
    sources.extend(collect_rs(&cli_root().join("src")));
    let excluded = cfg_test_module_files(&sources);
    sources.retain(|p| !excluded.contains(p));
    sources
}

// ── the producer list, read from core ────────────────────────────────

/// Every public `&mut self` method of `ProjectSession` that answers an
/// `Effects`.
///
/// The scan is SCOPED to `impl ProjectSession` blocks under
/// `crates/rs_cam_core/src/session/`, as the WP15a scan scopes its own:
/// two `pub(crate)` helpers in the same directory build an `Effects`
/// and are not a surface's door, and a test module declares a free
/// `fn replace` whose name would match half the view.
///
/// `apply` is IN the list. It is the door every surface takes, and a
/// discarded `apply` answer is the shape this file was written for.
fn effects_producers() -> Vec<String> {
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
                let declaration = signature(&lines, inner);
                if !declaration.contains("&mut self") {
                    continue;
                }
                let answers_effects =
                    declaration.contains("-> Effects") || declaration.contains("Result<Effects");
                if !answers_effects {
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
/// Both spellings count. WP15b flipped the `ProjectSession` setters to
/// `pub(crate) fn`, and a reader that accepts only `pub fn ` then names
/// one producer instead of sixty, which takes the non-vacuity guard in
/// arm (a) with it.
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

// ── the statement accumulator ────────────────────────────────────────

/// One statement that discards its producer's answer.
struct Discard {
    /// The index of the head line, zero-based.
    first: usize,
    /// The index of the line that closes the statement.
    last: usize,
    /// The statement, with every line joined by one space.
    text: String,
}

/// Does the accumulated text close one statement?
fn closes(statement: &str) -> bool {
    let opens = statement.matches('(').count() as i64;
    let shuts = statement.matches(')').count() as i64;
    opens - shuts <= 0 && (statement.ends_with(';') || statement.ends_with('{'))
}

/// Every `let _ =` and `if let Err(` statement in `text`, accumulated.
///
/// `cargo fmt` wraps a long call over four lines, so a line-wise match
/// reads three of the seven viz sites as clean. The walk appends lines
/// until the parenthesis depth returns to zero and the line ends the
/// statement.
fn discarding_statements(text: &str) -> Vec<Discard> {
    let lines: Vec<&str> = text.lines().collect();
    let mut found = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let head = line.trim();
        if !head.starts_with("let _ =") && !head.starts_with("if let Err(") {
            continue;
        }
        let mut statement = head.to_owned();
        let mut last = index;
        while !closes(&statement) && last + 1 < lines.len() && last - index < MAX_STATEMENT_LINES {
            last += 1;
            statement.push(' ');
            statement.push_str(lines[last].trim());
        }
        let discard = Discard {
            first: index,
            last,
            text: statement,
        };
        found.push(discard);
    }
    found
}

/// The producer one statement calls, if it calls one.
fn calls_a_producer(statement: &str, producers: &[String]) -> Option<String> {
    if statement.contains(CLI_DOOR) {
        return Some(CLI_DOOR.to_owned());
    }
    producers
        .iter()
        .find(|name| statement.contains(&format!(".{name}(")))
        .cloned()
}

/// The statement plus the [`LOOKBACK`] lines above it.
fn with_lookback(lines: &[&str], discard: &Discard) -> String {
    let from = discard.first.saturating_sub(LOOKBACK);
    lines[from..=discard.last].join("\n")
}

/// Is this file the batch CLI's?
fn is_cli(path: &Path) -> bool {
    path.starts_with(cli_root().join("src"))
}

/// The allowlist entry that covers one discard, if any covers it.
fn justification(path: &Path, window: &str) -> Option<&'static Justified> {
    JUSTIFIED
        .iter()
        .find(|entry| path.ends_with(entry.path) && window.contains(entry.needle))
}

// ── arm (a) — the source scan ────────────────────────────────────────

#[test]
fn no_production_site_discards_an_effects_answer() {
    let producers = effects_producers();
    assert!(
        producers.len() >= MIN_PRODUCERS,
        "the core scan must collect at least {MIN_PRODUCERS} producer \
         names, or this file asserts nothing. I read {}",
        producers.len()
    );

    let sources = production_sources();
    assert!(
        sources.len() >= MIN_FILES_WALKED,
        "the production walk must read at least {MIN_FILES_WALKED} \
         files, or it asserts nothing. I read {}",
        sources.len()
    );

    let mut unjustified: Vec<String> = Vec::new();
    let mut exempted_cli = 0_usize;
    for path in &sources {
        let text = without_inline_test_modules(&strip_comments(&read(path)));
        let lines: Vec<&str> = text.lines().collect();
        for discard in discarding_statements(&text) {
            let Some(producer) = calls_a_producer(&discard.text, &producers) else {
                continue;
            };
            if is_cli(path) {
                exempted_cli += 1;
                continue;
            }
            let window = with_lookback(&lines, &discard);
            if justification(path, &window).is_some() {
                continue;
            }
            let at = discard.first + 1;
            unjustified.push(format!("{}:~{at}: {producer}", path.display()));
        }
    }

    assert!(
        exempted_cli >= MIN_CLI_EXEMPTED,
        "the one CLI path rule must exempt at least {MIN_CLI_EXEMPTED} \
         sites, or it is a rule about nothing ({CLI_HOLDS_NO_RUNTIME_ROWS}). \
         I read {exempted_cli}"
    );

    assert!(
        unjustified.is_empty(),
        "WP19 (H2): a core mutation answers with an `Effects`, and the \
         view mirrors both halves of it — `stale` reaches the toolpath \
         cards through `state::stale::stamp_stale`, and \
         `simulation_cleared` reaches the viewport through \
         `AppController::invalidate_simulation`. A site that discards \
         the answer mirrors neither: the cards stay green and the \
         viewport goes on drawing a simulation the session no longer \
         holds. Take a door — `apply_controller_command` or \
         `apply_quietly` in the controller, `apply_panel_command` in a \
         draw site, `adopt_post_effects` for a post write — or state \
         the reason in `JUSTIFIED`. I read {} discarded answer(s):\n  {}",
        unjustified.len(),
        unjustified.join("\n  ")
    );
}

/// The accumulator sees a call `cargo fmt` wrapped after `let _ =`.
///
/// This is the positive control for the whole file. The WP15a joiner
/// folds a line whose first character is `.` and stops on this shape,
/// which is why the reader is copied and that one function is not.
#[test]
fn the_accumulator_reads_a_wrapped_call() {
    // `concat!` and not a backslash-continued literal: `cargo fmt`
    // re-indents a continued literal and bakes the new spaces into the
    // string, which would silently change what this control measures.
    let sample = concat!(
        "            let _ =\n",
        "                self.state\n",
        "                    .session\n",
        "                    .apply(rs_cam_core::session::Command::SetToolpathDebugOptions(\n",
        "                        rs_cam_core::session::SetToolpathDebugOptionsArgs {\n",
        "                            index,\n",
        "                            debug_options,\n",
        "                        },\n",
        "                    ));\n",
    );
    let found = discarding_statements(&strip_comments(sample));
    assert_eq!(found.len(), 1, "the walk must find one statement");
    let producers = ["apply".to_owned()];
    let matched = calls_a_producer(&found[0].text, &producers);
    assert!(
        matched.as_deref() == Some("apply"),
        "the accumulator no longer folds a wrapped call, so the scan \
         cannot see three of the seven shapes it is built to catch. I \
         read: {}",
        found[0].text
    );

    let plain = "        let _ = other.forget(name);\n";
    let plain_found = discarding_statements(plain);
    assert_eq!(plain_found.len(), 1);
    assert!(
        calls_a_producer(&plain_found[0].text, &producers).is_none(),
        "the needle must name a producer, or every same-named method on \
         another type reads as a discarded answer"
    );
}

/// Every allowlist entry still names a real site.
///
/// A fixed site leaves a dead exemption, and a dead exemption hides the
/// next regression. The precedent is WP15a's
/// `every_cli_exemption_names_a_reached_row`.
#[test]
fn every_justified_discard_still_names_a_real_site() {
    let producers = effects_producers();
    let sources = production_sources();
    let mut reached = vec![0_usize; JUSTIFIED.len()];
    for path in &sources {
        let text = without_inline_test_modules(&strip_comments(&read(path)));
        let lines: Vec<&str> = text.lines().collect();
        for discard in discarding_statements(&text) {
            if calls_a_producer(&discard.text, &producers).is_none() {
                continue;
            }
            let window = with_lookback(&lines, &discard);
            for (slot, entry) in JUSTIFIED.iter().enumerate() {
                if path.ends_with(entry.path) && window.contains(entry.needle) {
                    reached[slot] += 1;
                }
            }
        }
    }
    for (slot, entry) in JUSTIFIED.iter().enumerate() {
        assert!(
            reached[slot] == 1,
            "the allowlist entry `{}` / `{}` matches {} site(s), and it \
             must match exactly one. Its reason reads: {}. Drop the \
             entry when the site goes away, and split it when a second \
             site takes the same row.",
            entry.path,
            entry.needle,
            reached[slot],
            entry.reason
        );
    }
}

// ── arm (c2) — one stale helper, not two (H3) ────────────────────────

/// The MCP route and the view route stamp through ONE helper.
///
/// `mcp_stamp_stale` skipped an index whose runtime row was absent,
/// while `state::stale::stamp_stale` creates it. So a never-drawn
/// toolpath was stamped on one route and not on the other, and the
/// stated reason for the split — that the MCP helper "reads the
/// window, not the state" — was false: it read
/// `self.controller.state()`.
#[test]
fn one_helper_stamps_stale_on_both_routes_h3() {
    let mcp = strip_comments(&read(&viz_root().join("src/app/mcp.rs")));
    let commands = strip_comments(&read(&viz_root().join("src/app/mcp/commands.rs")));

    assert!(
        !mcp.contains("mcp_stamp_stale"),
        "H3: `app/mcp.rs` still declares or calls `mcp_stamp_stale`. \
         One helper stamps `stale_since`, and it is \
         `crate::state::stale::stamp_stale`."
    );
    assert!(
        !commands.contains("mcp_stamp_stale"),
        "H3: `app/mcp/commands.rs` still calls `mcp_stamp_stale`."
    );
    assert!(
        mcp.contains("stale::stamp_stale"),
        "H3: `app/mcp.rs` stamps no staleness through the one helper, \
         so the MCP route leaves a dropped result reading green."
    );
    assert!(
        commands.contains("stale::stamp_stale"),
        "H3: `app/mcp/commands.rs` stamps no staleness through the one \
         helper."
    );

    // The doc half. This note is read, not stripped: the false claim
    // sits in a `//!` comment.
    let stale = read(&viz_root().join("src/state/stale.rs"));
    assert!(
        !stale.contains("reads the window"),
        "H3: `state/stale.rs` still says the MCP helper reads the \
         window rather than the state. The helper is gone, and the \
         claim was false while it stood."
    );
}
