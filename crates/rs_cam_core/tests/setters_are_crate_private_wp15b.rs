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
//! # The arms
//!
//! 1. Every `fn … (&mut self` declared inside an `impl ProjectSession`
//!    block under `crates/rs_cam_core/src/session/` (sub-folders
//!    included) reads
//!    `pub(crate) fn`. Three doors and four compute doors are exempt.
//! 2. (Deleted, W14a F156.) No `.rs` file in any crate outside
//!    `crates/rs_cam_core/src` named `.<setter>(`. Arm 1 makes such a
//!    call a compile error in the crate that holds it, so the scan
//!    re-checked what the compiler holds.
//! 3. The walk is not vacuous: it finds enough methods, and enough
//!    methods survive the two allowlists.
//!
//! # Why a source scan
//!
//! A `pub(crate)` declaration is a compile-time fact, and the compiler
//! reports it only where a caller stands. A source scan answers the
//! question the type system cannot put on one surface: which
//! declarations are still public. WP7's
//! `hatches_are_crate_private_wp7.rs` reads the same evidence the same
//! way.
//!
//! # The population, measured at `2c593c7d`
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
//! **Arm 2** (deleted since) read 609 call sites in 118 files before the
//! migration; the visibility flip of arm 1 then made each one a compile
//! error.
//!
//! **Arm 3 passes at every commit.** It fails only on a truncated tree.
//!
//! # What this file does NOT measure
//!
//! It reads declarations, not callers. The compile of every crate that
//! links `rs_cam_core` is the proof that no caller outside it remains.

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

/// The four compute entry points.
///
/// Each one runs a generation or a simulation rather than writing a
/// field. §25 ruling 4 assigns them to the `Job` programme and keeps
/// them public. WP15a's `setters_have_rows_wp15a.rs` exempts the same
/// four.
///
/// W9a made two former entries `pub(crate)`: `plan_multitool_finishing`
/// (F040, the `PlanMultitoolFinishing` row) and
/// `modulate_simulation_trace` (F115, the `AdoptSimulation` row).
const COMPUTE_DOORS: &[&str] = &[
    "generate_toolpath",
    "generate_all",
    "run_simulation",
    // CMP-23: the S5-memoized twin of `run_simulation`. It runs a
    // simulation with a held prefix snapshot and writes nothing else.
    // WP15a exempts it too. The flat session walk never read
    // `session/compute/simulation.rs`, so this list missed it until the
    // walk descended.
    "run_simulation_memoized",
];

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

// ── arm 1: the declarations ─────────────────────────────────────────────

/// Every `.rs` file under `crates/rs_cam_core/src/session/`, sub-folders
/// included.
///
/// The setters moved into `session/mutation/` and `session/compute/`
/// before the 6bdc0899 graft. A flat walk of `session/` then read 12
/// methods and arm 3 went red. The walk now descends, as
/// `setters_have_rows_wp15a.rs` does.
fn session_sources() -> Vec<PathBuf> {
    let dir = core_root().join("src/session");
    assert!(dir.is_dir(), "{} no longer exists", dir.display());
    let mut out = Vec::new();
    collect_rs(&dir, &mut out);
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

// ── arm 2 (deleted, W14a F156) ──────────────────────────────────────────
//
// Arm 2 refused a `.<setter>(` call in any crate outside `rs_cam_core`.
// The setters are `pub(crate)` (arm 1), so such a call is a compile error in
// the crate that holds it, and that crate's own test build reports it.

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

// ── arm 3: non-vacuity ──────────────────────────────────────────────────

/// ARM 3. The declaration scan does not read an empty tree.
///
/// Arm 1 asserts an EMPTY list, so it passes on a walk that found
/// nothing. This arm states the floors, and it fails on a truncated tree,
/// a moved directory or an allowlist that grew to cover the population.
#[test]
fn the_declaration_scan_is_not_vacuous() {
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
}
