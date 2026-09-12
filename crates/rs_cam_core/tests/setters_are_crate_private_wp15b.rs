//! WP15b — every `ProjectSession` setter is crate-private, and no crate
//! outside `rs_cam_core` names one.
//!
//! # What this file guards
//!
//! `ProjectSession` publishes one mutation door, `ProjectSession::apply`.
//! WP15a gave every setter a `Command` row and moved every production
//! site onto the door. The setters stayed `pub`, so a surface could still
//! reach around the door and write a generation input. WP15b closes the
//! reach: each setter reads `pub(crate) fn`, and the compiler refuses any
//! caller outside this crate.
//!
//! The plan rules this at
//! `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md` §25
//! ruling 2. §27 ruling 1 orders the package last in the programme.
//!
//! # The three arms
//!
//! 1. Every `fn … (&mut self` declared inside an `impl ProjectSession`
//!    block under `crates/rs_cam_core/src/session/` reads
//!    `pub(crate) fn`. Three doors and five compute doors are exempt.
//! 2. No `.rs` file in any crate outside `crates/rs_cam_core/src` names
//!    `.<setter>(`.
//! 3. The walk is not vacuous: it visits enough files, it finds enough
//!    methods, and enough methods survive the two allowlists.
//!
//! # Why a source scan
//!
//! A `pub(crate)` declaration is a compile-time fact, and the compiler
//! reports it only where a caller stands. A source scan answers the
//! question the type system cannot put on one surface: which
//! declarations are still public, and which files still name them. WP7's
//! `hatches_are_crate_private_wp7.rs` reads the same evidence the same
//! way, and this file reuses its walk.
//!
//! # The population, measured at `61c16b75`
//!
//! The scan reads **86** `&mut self` methods inside `impl ProjectSession`
//! blocks. Two doors match, `apply` and `start`; `query` takes `&self`
//! and never matches. Five compute doors match. **79** methods remain to
//! check. Of those, **20** already read `pub(crate) fn` — the raw `_impl`
//! halves, the `with_effects` pair, the `drop_*` helpers and
//! `rename_setup`. **59** read `pub fn`, and those 59 are what WP15b
//! flips.
//!
//! An earlier brief put the checked population at 60. That figure counted
//! `rename_setup` alone among the crate-private methods. The scan reads
//! nineteen more, and they pass arm 1 already.
//!
//! # Red before the fix
//!
//! **Arm 1 fails on its assertion.** It reads 59 declarations that say
//! `pub fn` and names each one with its path and line. The binary
//! compiles, because the sentry commit flips no visibility.
//!
//! **Arm 2 fails on its assertion.** It reads **609** matches in **118**
//! files, over **530** files walked: 483 in `crates/rs_cam_core/tests`, 4
//! in `crates/rs_cam_core/benches`, 54 in `crates/rs_cam_viz/src` and 68
//! in `crates/rs_cam_viz/tests`. The CLI crate and the MCP crate read
//! **0**, and the arm pins that zero. The receiver rule skips 4 matches.
//!
//! **Arm 3 passes at every commit.** It fails only on a truncated tree.
//!
//! The core migration lands first, so arm 2 reads **122** matches after
//! it — the viz half alone. Arm 2 goes green when the viz migration
//! lands. Arm 1 goes green when the visibility flips.
//!
//! # The receiver rule
//!
//! A bare `.<setter>(` pattern raises three classes of false positive.
//! [`is_excluded_receiver`] skips each one, and each skip is a claim a
//! reader can check.
//!
//! - `self` — the viz `Controller` declares its own
//!   `adopt_model_geometry` (`crates/rs_cam_viz/src/controller/io.rs`).
//!   Three lines there read `self.adopt_model_geometry(…)`, and that
//!   receiver is a `Controller`, not a session.
//! - `self.controller` — `crates/rs_cam_viz/src/app/input.rs` reads
//!   `self.controller.set_setup_pause_message(…)`, the same collision one
//!   layer out.
//! - a receiver whose trailing identifier ends in `builder` —
//!   [`rs_cam_core::session::ProjectSessionBuilder`] publishes four
//!   ALLOCATING methods that carry the setter names on purpose:
//!   `add_tool`, `add_model`, `add_setup` and `add_toolpath`. A migrated
//!   fixture calls them, and the call is not a session call.
//!
//! **The third skip is a naming convention, and every migrated fixture
//! follows it.** Name the builder `builder`, or name it with a `_builder`
//! suffix. A session variable named `builder` would slip past arm 2; the
//! visibility flip catches it, because such a call fails to compile.
//!
//! # What this file does NOT measure
//!
//! It reads source text. It does not prove the migration is complete: a
//! caller the walk never visited is invisible here. The compile of the
//! visibility flip is the completeness proof, and this scan is the
//! surface that says what is left to do before it.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

/// The three doors. A door runs a row; it is not a setter.
///
/// `query` takes `&self`, so the `&mut self` scan never matches it. The
/// list names it anyway, so the allowlist names the door set the plan
/// names.
const DOORS: &[&str] = &["apply", "query", "start"];

/// The five compute entry points.
///
/// Each one runs a generation, a simulation or a plan rather than
/// writing a field. §25 ruling 4 assigns them to the `Job` programme and
/// keeps them public. WP15a's `setters_have_rows_wp15a.rs` exempts the
/// same five.
const COMPUTE_DOORS: &[&str] = &[
    "generate_toolpath",
    "generate_all",
    "run_simulation",
    "modulate_simulation_trace",
    "plan_multitool_finishing",
];

/// Files that carry a setter name as scanner data rather than as a call.
///
/// `production_writes_go_through_apply_wp15a.rs` holds `.add_setup(`
/// inside a string literal, as its own positive control. The WP6, WP6b
/// and WP7 scanners hold their forbidden names the same way. A new entry
/// here is a claim that the file scans source rather than mutates a
/// session.
const SCANNER_FILES: &[&str] = &[
    "egui_draw_sites_write_through_commands_wp6.rs",
    "non_egui_sites_write_through_commands_wp6b.rs",
    "hatches_are_crate_private_wp7.rs",
    "production_writes_go_through_apply_wp15a.rs",
    "setters_are_crate_private_wp15b.rs",
];

/// The lowest number of files the walk must visit.
///
/// A walk that reads a handful of files proves nothing. An empty or
/// truncated tree fails here rather than passing green. WP7 uses the same
/// bar over a narrower root list.
const MIN_FILES_WALKED: usize = 100;

/// The lowest number of `&mut self` methods the declaration scan must
/// find, before any allowlist applies.
const MIN_SETTERS: usize = 50;

/// The lowest number of setters that must survive [`DOORS`] and
/// [`COMPUTE_DOORS`].
///
/// A growing allowlist cannot swallow the population under this bar.
const MIN_CHECKED_SETTERS: usize = 50;

/// The lowest number of `.rs` files the session directory must hold.
const MIN_SESSION_FILES: usize = 8;

/// This crate's root directory.
fn core_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Whether `line` is a comment. Prose names a setter to explain it.
fn is_comment(line: &str) -> bool {
    line.trim_start().starts_with("//")
}

/// Whether the file's own name puts it on the scanner allowlist.
fn is_scanner(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| SCANNER_FILES.contains(&name))
}

// ── arm 1: the declarations ─────────────────────────────────────────────

/// Every `.rs` file directly under `crates/rs_cam_core/src/session/`.
fn session_sources() -> Vec<PathBuf> {
    let dir = core_root().join("src/session");
    assert!(dir.is_dir(), "{} no longer exists", dir.display());
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("read_dir session").flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out.sort();
    assert!(
        out.len() >= MIN_SESSION_FILES,
        "the session directory must hold at least {MIN_SESSION_FILES} \
         files, or the scan reads a tree that is not there. I read {}",
        out.len()
    );
    out
}

/// The name one declaration line opens, and whether it is crate-private.
///
/// The reader accepts BOTH `pub fn ` and `pub(crate) fn `. A reader that
/// saw only `pub fn ` would stop seeing a setter the moment the setter is
/// fixed, so it could not check the very property this file names.
fn declaration(line: &str) -> Option<(&str, bool)> {
    let trimmed = line.trim_start();
    let (rest, crate_private) = match trimmed.strip_prefix("pub(crate) fn ") {
        Some(rest) => (rest, true),
        None => (trimmed.strip_prefix("pub fn ")?, false),
    };
    let name = rest.split('(').next()?;
    let plain = name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if name.is_empty() || !plain {
        return None;
    }
    Some((name, crate_private))
}

/// The whole signature of the declaration that starts at `start`.
///
/// A signature runs to the line that opens the body. Most setters put
/// `&mut self` on the line AFTER the opening paren, so a same-line
/// pattern misses two thirds of them.
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

/// One declared method: its file, its line, its name and its visibility.
struct Method {
    path: PathBuf,
    line: usize,
    name: String,
    crate_private: bool,
}

/// Every `fn … (&mut self` declared inside an `impl ProjectSession`
/// block under `src/session/`.
///
/// The scan is SCOPED to those blocks. `LoadedModel::adopt_geometry` and
/// `CycleTime::fold` are public `&mut self` methods in the same
/// directory, and an unscoped scan reads them as session setters.
///
/// The block runs from a line that reads `impl ProjectSession {` to the
/// next line that is a bare `}`. Every file there is `cargo fmt` clean,
/// so a top-level block closes at column zero.
fn declared_methods() -> Vec<Method> {
    let mut out = Vec::new();
    for path in session_sources() {
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
                if is_comment(line) {
                    continue;
                }
                let Some((name, crate_private)) = declaration(line) else {
                    continue;
                };
                if signature(&lines, inner).contains("&mut self") {
                    out.push(Method {
                        path: path.clone(),
                        line: inner + 1,
                        name: name.to_owned(),
                        crate_private,
                    });
                }
            }
            index = end;
        }
    }
    out
}

/// The declared methods that are neither a door nor a compute door.
fn setters() -> Vec<Method> {
    declared_methods()
        .into_iter()
        .filter(|method| {
            let name = method.name.as_str();
            !DOORS.contains(&name) && !COMPUTE_DOORS.contains(&name)
        })
        .collect()
}

/// ARM 1. Every setter is declared `pub(crate) fn`.
///
/// A setter that reads `pub fn` is a second write surface: a crate
/// outside `rs_cam_core` can reach it, and the registry cannot say which
/// surface writes what.
#[test]
fn every_session_setter_is_declared_crate_private() {
    let setters = setters();
    assert!(
        setters.len() >= MIN_CHECKED_SETTERS,
        "at least {MIN_CHECKED_SETTERS} setters must survive the two \
         allowlists, or arm 1 asserts nothing. I read {}",
        setters.len()
    );

    let mut public: Vec<String> = Vec::new();
    for method in &setters {
        if method.crate_private {
            continue;
        }
        public.push(format!(
            "{}:{}: pub fn {}",
            method.path.display(),
            method.line,
            method.name
        ));
    }

    assert!(
        public.is_empty(),
        "WP15b: every `ProjectSession` setter is `pub(crate) fn`. A \
         surface outside `rs_cam_core` mutates a session through \
         `ProjectSession::apply` with the matching `Command` row, or \
         builds one with `ProjectSessionBuilder`. I read {} public \
         declaration(s):\n  {}",
        public.len(),
        public.join("\n  ")
    );
}

// ── arm 2: the call sites ───────────────────────────────────────────────

/// The directories outside `crates/rs_cam_core/src` that link this crate.
///
/// Every one of them is a separate compilation unit, so every one of them
/// breaks on a `pub(crate)` setter. WP7 walks the first six.
/// `../rs_cam_cli/tests` and `../rs_cam_mcp/src` read zero today, and the
/// arm pins that zero.
fn scanned_roots() -> Vec<PathBuf> {
    let core = core_root();
    let mut out = Vec::new();
    for relative in [
        "tests",
        "benches",
        "../rs_cam_viz/src",
        "../rs_cam_viz/tests",
        "../rs_cam_cli/src",
        "../rs_cam_cli/examples",
        "../rs_cam_cli/tests",
        "../rs_cam_mcp/src",
    ] {
        let path = core.join(relative);
        assert!(
            path.is_dir(),
            "scanned root {} no longer exists",
            path.display()
        );
        out.push(path);
    }
    out
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

/// Every `.rs` file under every scanned root.
fn scanned_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    for root in scanned_roots() {
        collect_rs(&root, &mut files);
    }
    files.sort();
    files
}

/// One source text as logical lines, each with the number of its first
/// physical line.
///
/// The reader does two things in one pass. It strips every `//` comment,
/// because prose names a setter to explain it. It then folds a line whose
/// first character is `.` back onto the line above it, because
/// `cargo fmt` breaks a method chain over several lines and a line-wise
/// pattern cannot see `state\n.session\n.add_toolpath(`.
///
/// The strip is line-wise and deliberately crude. It cannot tell a `//`
/// inside a string literal from a comment. [`SCANNER_FILES`] carries the
/// files where that matters.
fn logical_lines(text: &str) -> Vec<(usize, String)> {
    let mut out: Vec<(usize, String)> = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = match raw.find("//") {
            Some(at) => &raw[..at],
            None => raw,
        };
        let trimmed = line.trim_start();
        if trimmed.starts_with('.')
            && let Some(last) = out.last_mut()
        {
            while last.1.ends_with(' ') {
                last.1.pop();
            }
            last.1.push_str(trimmed);
        } else {
            out.push((index + 1, line.to_owned()));
        }
    }
    out
}

/// The identifier `text` ends with, or an empty string.
fn trailing_identifier(text: &str) -> &str {
    let start = text
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_ascii_alphanumeric() || *c == '_')
        .last()
        .map_or(text.len(), |(index, _)| index);
    &text[start..]
}

/// Whether a receiver names something other than a `ProjectSession`.
///
/// The module doc states all three cases and what each one costs.
fn is_excluded_receiver(receiver: &str) -> bool {
    let tail = trailing_identifier(receiver);
    if tail == "self" || receiver.ends_with("self.controller") {
        return true;
    }
    tail.ends_with("builder")
}

/// ARM 2. No crate outside `rs_cam_core` names a setter.
///
/// This arm reports what the visibility flip would break. A hit is a call
/// site to migrate, not a defect in the code under test.
#[test]
fn no_crate_outside_core_names_a_session_setter() {
    let files = scanned_files();
    assert!(
        files.len() >= MIN_FILES_WALKED,
        "the walk must visit at least {MIN_FILES_WALKED} files, or arm 2 \
         passes on a tree it never read. I read {}",
        files.len()
    );

    let names: Vec<String> = setters()
        .into_iter()
        .map(|method| format!(".{}(", method.name))
        .collect();

    let mut hits: Vec<String> = Vec::new();
    for path in &files {
        if is_scanner(path) {
            continue;
        }
        for (number, line) in logical_lines(&read(path)) {
            for pattern in &names {
                let mut from = 0_usize;
                while let Some(offset) = line[from..].find(pattern.as_str()) {
                    let at = from + offset;
                    from = at + 1;
                    if is_excluded_receiver(&line[..at]) {
                        continue;
                    }
                    hits.push(format!("{}:{number}: {}", path.display(), line.trim()));
                }
            }
        }
    }

    assert!(
        hits.is_empty(),
        "WP15b: the `ProjectSession` setters are crate-private. A fixture \
         outside `rs_cam_core` builds a session with \
         `ProjectSessionBuilder` and mutates one with \
         `ProjectSession::apply`. I read {} call site(s) in {} file(s):\n  \
         {}",
        hits.len(),
        files.len(),
        hits.join("\n  ")
    );
}

// ── arm 3: non-vacuity ──────────────────────────────────────────────────

/// ARM 3. Neither scan reads an empty tree.
///
/// Arm 1 and arm 2 both assert an EMPTY list, so both pass on a walk that
/// found nothing. This arm states the three floors in one place, and it
/// fails on a truncated tree, a moved directory or an allowlist that grew
/// to cover the population.
#[test]
fn neither_scan_is_vacuous() {
    let methods = declared_methods();
    assert!(
        methods.len() >= MIN_SETTERS,
        "the declaration scan must find at least {MIN_SETTERS} \
         `&mut self` methods inside `impl ProjectSession` blocks. I read \
         {}",
        methods.len()
    );

    let checked = setters().len();
    assert!(
        checked >= MIN_CHECKED_SETTERS,
        "at least {MIN_CHECKED_SETTERS} of those must survive `DOORS` and \
         `COMPUTE_DOORS`. I read {checked}"
    );

    let files = scanned_files().len();
    assert!(
        files >= MIN_FILES_WALKED,
        "the call-site walk must visit at least {MIN_FILES_WALKED} files. \
         I read {files}"
    );
}
