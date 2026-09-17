//! WP15a — every public `ProjectSession` setter has a `Command` row.
//!
//! # What this file guards
//!
//! `ProjectSession` publishes one mutation door,
//! `ProjectSession::apply`. A public setter that no registry row calls
//! is a SECOND write path: a surface reaches it, the registry never
//! names it, and the surface table cannot say which surface writes
//! what. The IMPLEMENTATION_PLAN §25 ruling 1 closes the gap from the
//! registry side. The setter keeps the invalidation rule; a row
//! delegates to it, so the two routes cannot carry two staleness
//! models. §12 ruling 2 states that rule: one construction site, and no
//! arm re-derives a stale set of its own. WP15a left the setters `pub`;
//! WP15b made them `pub(crate)`, and the property below is unchanged.
//!
//! # The property
//!
//! For every `pub fn <name>(&mut self …)` or
//! `pub(crate) fn <name>(&mut self …)` declared inside an
//! `impl ProjectSession` block under `crates/rs_cam_core/src/session/`,
//! the body of `ProjectSession::apply` names `.<name>(`. Four
//! allowlists carry the exceptions, and each one says what it exempts:
//!
//! - [`DOORS`] — the three doors themselves. A door is not a setter.
//! - [`COMPUTE_DOORS`] — the five compute entry points. They are the
//!   `Job` programme's second write surface (§25 ruling 4), not
//!   WP15a's.
//! - [`WRAPPERS_OVER_APPLY`] — a setter whose own body calls
//!   `self.apply(`. It is already on the door, so a row that delegated
//!   to it would recurse. A second test asserts the body really does
//!   call `self.apply(`, so the exemption cannot go stale.
//! - [`CRATE_PRIVATE_HELPERS`] — the crate-private `&mut self` methods
//!   that are not the write surface: the raw halves, the one `Effects`
//!   construction site, the result-cache internals and the `*_mut`
//!   hatches. WP15b's flip is what brought them within reach of the
//!   reader, and none of them ever took a row.
//!
//! # Why a source scan
//!
//! The registry publishes `CommandId::ALL`, so a test can read every
//! row. It publishes no list of setters, and a method that no row calls
//! is exactly the thing the type system cannot see. The evidence is
//! therefore the source text, as WP7's
//! `hatches_are_crate_private_wp7.rs` reads it. This file reuses that
//! file's `collect`-and-read idiom.
//!
//! # Red before the fix
//!
//! I read 24 setters with no row: `set_machine_ref`, `set_name`,
//! `reorder_toolpath`, `set_toolpath_operation`,
//! `auto_enable_rest_analysis_for_source`, `remove_model`,
//! `remove_setup`, `set_face_selection`,
//! `set_alignment_pin_drill_holes`, `set_drill_selected_holes`,
//! `add_fixture`, `remove_fixture`, `add_keep_out`, `remove_keep_out`,
//! `invalidate_stock`, `invalidate_machine`, `invalidate_tool`,
//! `invalidate_model`, `update_stock_from_bbox`, `replace_tools`,
//! `set_feeds_provenance`, `remove_result`,
//! `invalidate_toolpath_inputs` and `replace_setups_and_toolpaths`.
//! Thirteen of them carry a production caller in `rs_cam_viz/src` or
//! `rs_cam_cli/src`; eleven carry none. WP15a gives all 24 a row, so
//! the property is total and no allowlist has to grow.
//!
//! # What this file does NOT measure
//!
//! It reads declarations and one function body. It does not read the
//! production surfaces: whether a viz or CLI site still calls a setter
//! instead of `apply` is the viz/CLI sentry's question, and a green
//! reading here is not evidence about that.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

/// The three doors. A door runs a row; it is not a setter.
///
/// `query` takes `&self` and never matches the `&mut self` scan. It is
/// listed anyway, so the allowlist names the door set the plan names.
const DOORS: &[&str] = &["apply", "query", "start"];

/// The five compute entry points.
///
/// Each one runs a generation, a simulation or a plan rather than
/// writing a field. They carry roughly 230 external callers and §25
/// ruling 4 assigns them to the `Job` programme, not to WP15a.
const COMPUTE_DOORS: &[&str] = &[
    "generate_toolpath",
    "generate_all",
    "run_simulation",
    "modulate_simulation_trace",
    "plan_multitool_finishing",
];

/// Setters that are thin wrappers over [`ProjectSession::apply`].
///
/// Such a setter already writes through the door. A row that delegated
/// to it would call the door from inside the door. The test
/// `every_wrapper_exemption_calls_the_door` reads each body and asserts
/// the wrapper claim, so a name parked here without the call fails.
const WRAPPERS_OVER_APPLY: &[&str] = &["set_toolpath_param"];

/// Crate-private `&mut self` methods that are NOT setters.
///
/// WP15b made every setter `pub(crate) fn`, so [`public_fn_name`] now
/// accepts that spelling. The spelling alone cannot tell a setter from
/// a helper, and `src/session/` holds nineteen crate-private `&mut self`
/// methods the reader can see that were never part of the write
/// surface:
///
/// - the raw halves a setter wraps (`*_impl`). The setter calls its own
///   half inside the one [`Effects`] construction site.
/// - `with_effects`, that construction site itself. `try_with_effects`
///   stands beside it here, although only its generic parameter keeps
///   the reader from seeing it today.
/// - the result-cache internals: `insert_result`, the four `drop_*`
///   methods, `bump_all_revisions`, `invalidate_result_chain` and
///   `invalidate_output_dependents`.
/// - the three `*_mut` hatches, which `hatches_are_crate_private_wp7`
///   owns.
/// - `start_generate_toolpath`, the raw half of the `start` door.
/// - `apply_toolpath_param_snapshot_narrow`, the undo path's narrow
///   write.
///
/// No row delegates to any of them. Four match a row's text by accident
/// — `with_effects`, `insert_result`, `invalidate_result_chain` and
/// `set_toolpath_param_impl` all appear inside an `apply` arm — so
/// leaving them in the population would report a pass nobody wrote.
///
/// `rename_setup` is deliberately NOT here. It is a setter, it carries
/// the row `SetSetupName`, and it is the precedent WP15b generalised.
const CRATE_PRIVATE_HELPERS: &[&str] = &[
    "add_model_impl",
    "add_setup_impl",
    "add_tool_impl",
    "add_toolpath_impl",
    "apply_toolpath_param_snapshot_narrow",
    "bump_all_revisions",
    "drop_all_results",
    "drop_result",
    "drop_results_and_their_dependents",
    "drop_setup_results",
    "drop_tool_results",
    "find_toolpath_config_by_id_mut",
    "insert_result",
    "invalidate_output_dependents",
    "invalidate_output_dependents_of_set",
    "invalidate_result_chain",
    "set_toolpath_param_impl",
    "setups_mut",
    "start_generate_toolpath",
    "toolpath_configs_mut",
    "try_with_effects",
    "walk_output_dependents",
    "with_effects",
];

/// The lowest number of setters the scan must find.
///
/// A scan that reads a handful of names proves nothing. An empty or
/// truncated walk fails here rather than passing green.
const MIN_SETTERS: usize = 40;

/// The lowest number of `.rs` files the session directory must hold.
const MIN_SESSION_FILES: usize = 20;

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

/// Every `.rs` file directly under `crates/rs_cam_core/src/session/`.
fn session_sources() -> Vec<PathBuf> {
    let dir = core_root().join("src/session");
    assert!(dir.is_dir(), "{} no longer exists", dir.display());
    // The setters moved into `mutation/` and `compute/` on 2026-09-17, so
    // the scan walks the folder tree, not one flat directory.
    fn walk(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("read_dir session").flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(&dir, &mut out);
    out.sort();
    assert!(
        out.len() >= MIN_SESSION_FILES,
        "the session directory must hold at least {MIN_SESSION_FILES} \
         files, or the scan reads a tree that is not there. I read {}",
        out.len()
    );
    out
}

/// The name of the function one declaration line opens, when the line
/// opens a `pub fn` or a `pub(crate) fn`.
///
/// WP15b flipped the setters to `pub(crate) fn`. A reader that accepts
/// only `pub fn ` finds none of them after that flip, and every check
/// below reads an empty population and passes. The reader must see both
/// spellings, or it cannot see the thing it checks.
fn public_fn_name(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let rest = trimmed
        .strip_prefix("pub(crate) fn ")
        .or_else(|| trimmed.strip_prefix("pub fn "))?;
    let name = rest.split('(').next()?;
    let plain = name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if name.is_empty() || !plain {
        return None;
    }
    Some(name)
}

/// The whole signature of the declaration that starts at `start`.
///
/// A signature runs to the line that opens the body. Forty of the
/// setters put `&mut self` on the line AFTER the opening paren, so a
/// same-line pattern misses two thirds of them.
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

/// Every `pub fn` or `pub(crate) fn` `… (&mut self` declared inside an
/// `impl ProjectSession` block under `src/session/`, less the helpers
/// [`CRATE_PRIVATE_HELPERS`] names.
///
/// The scan is SCOPED to those blocks. `LoadedModel::adopt_geometry`
/// and `CycleTime::fold` are public `&mut self` methods in the same
/// directory, and an unscoped scan reads them as session setters.
///
/// The block runs from a line that reads `impl ProjectSession {` to the
/// next line that is a bare `}`. Both files are `cargo fmt` clean, so a
/// top-level block closes at column zero; a brace count would instead
/// have to reason about braces inside string literals.
fn setters() -> Vec<(PathBuf, usize, String)> {
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
                let Some(name) = public_fn_name(line) else {
                    continue;
                };
                if CRATE_PRIVATE_HELPERS.contains(&name) {
                    continue;
                }
                if signature(&lines, inner).contains("&mut self") {
                    out.push((path.clone(), inner + 1, name.to_owned()));
                }
            }
            index = end;
        }
    }
    out
}

/// The body of `ProjectSession::apply`, as source text.
///
/// The slice runs from the `apply` declaration to the `query`
/// declaration that follows it in the same `impl` block.
fn apply_body() -> String {
    let path = core_root().join("src/session/command.rs");
    let text = read(&path);
    let start = text
        .find("    pub fn apply(")
        .expect("command.rs declares `pub fn apply(`");
    let rest = &text[start..];
    let end = rest
        .find("    pub fn query(")
        .expect("`pub fn query(` follows `pub fn apply(` in command.rs");
    rest[..end].to_owned()
}

/// Whether the body of `apply` delegates to one setter.
///
/// The match reads `.<name>(` with ANY receiver. An arm that returns
/// `()` runs inside a `with_effects` closure and reads
/// `session.set_name(`, not `self.set_name(`, so a `self.`-anchored
/// pattern would miss it.
fn delegates_to(body: &str, name: &str) -> bool {
    let needle = format!(".{name}(");
    body.lines()
        .filter(|line| !is_comment(line))
        .any(|line| line.contains(&needle))
}

/// The one property: a row delegates to every public setter.
#[test]
fn every_public_setter_has_a_command_row() {
    let declared = setters();
    assert!(
        declared.len() >= MIN_SETTERS,
        "the scan must find at least {MIN_SETTERS} public `&mut self` \
         methods on `ProjectSession`, or it asserts nothing. I read {}",
        declared.len()
    );

    let body = apply_body();
    let mut missing: Vec<String> = Vec::new();
    let mut checked = 0_usize;
    for (path, number, name) in &declared {
        let name = name.as_str();
        if DOORS.contains(&name) || COMPUTE_DOORS.contains(&name) {
            continue;
        }
        if WRAPPERS_OVER_APPLY.contains(&name) {
            continue;
        }
        checked += 1;
        if !delegates_to(&body, name) {
            missing.push(format!("{}:{number}: {name}", path.display()));
        }
    }

    assert!(
        checked >= MIN_SETTERS,
        "the allowlists must not swallow the population: at least \
         {MIN_SETTERS} setters must remain to check. I checked {checked}"
    );
    assert!(
        missing.is_empty(),
        "WP15a (§25 ruling 1): every public `ProjectSession` setter is \
         reachable through `ProjectSession::apply`. Add a registry row \
         to `for_each_command!` whose `apply` arm delegates to the \
         setter; the setter keeps the invalidation rule, and the arm \
         never re-derives a stale set. I read {} setter(s) that no row \
         calls:\n  {}",
        missing.len(),
        missing.join("\n  ")
    );
}

/// A wrapper exemption is honest: the body really calls the door.
///
/// [`WRAPPERS_OVER_APPLY`] exempts a setter because it writes through
/// `apply` already. The day such a body stops calling `self.apply(`,
/// the exemption hides a second write path. This test reads the body.
#[test]
fn every_wrapper_exemption_calls_the_door() {
    let declared = setters();
    for wrapper in WRAPPERS_OVER_APPLY {
        let site = declared.iter().find(|(_, _, name)| name == wrapper);
        let Some((path, number, _)) = site else {
            panic!(
                "`{wrapper}` is exempt as a wrapper over `apply`, but no \
                 `impl ProjectSession` block declares it. Remove the \
                 entry from WRAPPERS_OVER_APPLY."
            );
        };
        let text = read(path);
        let lines: Vec<&str> = text.lines().collect();
        let mut calls_the_door = false;
        for line in lines.iter().skip(*number).take(40) {
            if is_comment(line) {
                continue;
            }
            if line.contains("self.apply(") {
                calls_the_door = true;
                break;
            }
            // Stop at the NEXT declaration, so the scan cannot read a
            // later function's `self.apply(` as this body's. Both
            // spellings stop it: WP15b made the setters `pub(crate) fn`,
            // and a `pub fn `-only test would run past the body.
            let trimmed = line.trim_start();
            if trimmed.starts_with("pub fn ") || trimmed.starts_with("pub(crate) fn ") {
                break;
            }
        }
        assert!(
            calls_the_door,
            "`{wrapper}` is exempt as a wrapper over `apply`, but I read \
             no `self.apply(` in its body at {}:{number}. Either restore \
             the call, or give the setter a registry row and drop the \
             exemption.",
            path.display()
        );
    }
}
