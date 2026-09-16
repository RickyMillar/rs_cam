//! WP28 parts 2 and 3 — one staleness answer, and no registry row that
//! no surface reaches.
//!
//! Programme:
//! `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md` §34,
//! parts 2 and 3. Residuals:
//! `planning/arch_consolidation_2026-09-09/REVIEW_COMPLETENESS_2026-09-13.md`
//! §7 (the `MutationKind` / `compute_stale_set` holdout) and §21.8 (the
//! all-Skip `Command::RemoveSetup` row).
//!
//! # Arm 1 — one staleness answer
//!
//! The session carried TWO producers of a stale set. `Effects::stale`
//! reads the revision map around the mutation, so it reports every index
//! whose result the chain dropped. `compute_stale_set` read a
//! `MutationKind` tag and answered from the tag alone, with no chain
//! walk. WP1 measured the divergence: on a fixture whose second toolpath
//! reads the stock the first one leaves, the setter dropped `{0, 1}` and
//! `compute_stale_set` reported `{0}` (N15).
//!
//! WP28 part 2 deletes the second producer. This arm keeps it deleted: no
//! `.rs` file under any `crates/*/src` names `compute_stale_set` or
//! `MutationKind` outside a comment.
//!
//! The scan covers the in-`src` test modules too. That is deliberate: a
//! `#[cfg(test)]` module inside `session/compute.rs` is what held the
//! last three callers of the deleted function.
//!
//! # Arm 2 — no all-Skip row outside the allow-list
//!
//! A `Surfaces` literal declares, per row, whether the GUI, the MCP
//! server and the batch CLI reach the command. A row that declares
//! `Reach::Skip` on all three is reached by no surface, so nothing in the
//! product runs it. `Command::RemoveSetup` was such a row, and its setter
//! `remove_setup` had no caller at all. WP28 part 3 deletes both.
//!
//! ## What this arm does NOT claim
//!
//! It does not claim the registry holds no dead row. TEN rows remain
//! all-Skip, and I measured that NONE of them is constructed by
//! `rs_cam_viz/src`, `rs_cam_cli/src` or `rs_cam_mcp/src` outside an
//! in-`src` test module. They are on [`ALL_SKIP_ALLOW_LIST`] with that
//! reading as their reason, not with an exoneration. They stand because
//! §34 part 3 names one row, and because WP15a
//! (`setters_have_rows_wp15a.rs`) requires a row per setter — deleting a
//! row without deleting its setter turns that test red.
//!
//! The arm is BIDIRECTIONAL: the measured all-Skip set must EQUAL the
//! allow-list. So a new all-Skip row fails here, `remove_setup` coming
//! back fails here, and a listed row that later gains a `Reach::Reached`
//! or leaves the registry fails here too. The list cannot rot into a
//! graveyard.
//!
//! # Red before the fix
//!
//! Both arms fail on their assertion. The file compiles at the sentry
//! commit: it names no symbol the fix introduces, and it reads
//! `CommandId::ALL` and `Surfaces`, which both already ship.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use rs_cam_core::session::{CommandId, Reach};

// ── arm 1: the source scan ──────────────────────────────────────────────

/// The two names the second staleness producer published.
const FORBIDDEN: &[&str] = &["compute_stale_set", "MutationKind"];

/// The production source roots. Every crate in the workspace.
const SRC_ROOTS: &[&str] = &[
    "src",
    "../rs_cam_viz/src",
    "../rs_cam_cli/src",
    "../rs_cam_mcp/src",
];

/// The lowest number of `.rs` files the walk must visit.
///
/// A truncated tree fails here rather than passing green.
const MIN_FILES_WALKED: usize = 100;

/// A name that IS in the scanned corpus. The reader must find it, or the
/// reader is not reading the text it claims to read.
const POSITIVE_CONTROL: &str = "ProjectSession";

/// The lowest number of lines the positive control must match.
const MIN_CONTROL_HITS: usize = 50;

fn core_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
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

/// Every `.rs` file under every production source root.
fn scanned_files() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for relative in SRC_ROOTS {
        let path = core_root().join(relative);
        assert!(
            path.is_dir(),
            "scanned root {} no longer exists",
            path.display()
        );
        collect_rs(&path, &mut out);
    }
    out.sort();
    out
}

/// The line with every `//` comment removed.
///
/// Prose names a deleted symbol to explain the history, and that is not a
/// call. The strip is line-wise and deliberately crude: it cannot tell a
/// `//` inside a string literal from a comment. WP15b's
/// `setters_are_crate_private_wp15b.rs` reads the same evidence the same
/// way.
fn strip_comment(line: &str) -> &str {
    match line.find("//") {
        Some(at) => &line[..at],
        None => line,
    }
}

/// ARM 1. No production source names the deleted staleness producer.
#[test]
fn no_production_source_names_the_second_staleness_model() {
    let files = scanned_files();
    assert!(
        files.len() >= MIN_FILES_WALKED,
        "the walk must visit at least {MIN_FILES_WALKED} files, or it \
         reads a tree that is not there. I read {}",
        files.len()
    );

    let mut control_hits = 0_usize;
    let mut found: Vec<String> = Vec::new();
    for path in &files {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        for (number, line) in text.lines().enumerate() {
            let code = strip_comment(line);
            if code.contains(POSITIVE_CONTROL) {
                control_hits += 1;
            }
            for name in FORBIDDEN {
                if code.contains(name) {
                    found.push(format!(
                        "{}:{}: {}",
                        path.display(),
                        number + 1,
                        line.trim()
                    ));
                }
            }
        }
    }

    assert!(
        control_hits >= MIN_CONTROL_HITS,
        "the reader found `{POSITIVE_CONTROL}` on {control_hits} lines. \
         Under {MIN_CONTROL_HITS} means the reader is not reading the \
         corpus, so a green arm would prove nothing."
    );

    assert!(
        found.is_empty(),
        "WP28 part 2: `Effects::stale` is the one staleness answer. A \
         surface reports the set the command's own setter dropped. I \
         read {} naming line(s):\n  {}",
        found.len(),
        found.join("\n  ")
    );
}

// ── arm 2: the all-Skip rows ────────────────────────────────────────────

/// Every registry row that declares `Reach::Skip` on all three surfaces
/// today, with the reading that puts it here.
///
/// Each entry is the row's WIRE NAME, so this list compiles before and
/// after the deletion.
///
/// Each entry carries TWO things and nothing else: the registry row's own
/// `gui` Skip text, quoted, and the one fact I measured. I measured no
/// mechanism, so no entry states one.
///
/// MEASURED 2026-09-14: no `Command::<row>` literal stands in
/// `crates/rs_cam_viz/src`, `crates/rs_cam_cli/src` or
/// `crates/rs_cam_mcp/src` outside an in-`src` test module, for ANY of
/// these ten. Each is therefore dead in the same sense `remove_setup`
/// was. They stay because WP15a requires a row per setter, and because
/// §34 part 3 names one row. This list is a LEDGER of an open finding.
const ALL_SKIP_ALLOW_LIST: &[(&str, &str)] = &[
    (
        "set_toolpath_operation",
        "WP15a row. The registry's own GUI reason reads: no GUI control changes an operation kind in place. \
         MEASURED 2026-09-14: no production `Command` literal.",
    ),
    (
        "invalidate_stock",
        "WP15a row. The registry's own GUI reason reads: no GUI control drops the results alone; set_stock_config writes and drops. \
         MEASURED 2026-09-14: no production `Command` literal.",
    ),
    (
        "invalidate_machine",
        "WP15a row. The registry's own GUI reason reads: no GUI control drops the simulation alone; set_machine writes and drops. \
         MEASURED 2026-09-14: no production `Command` literal.",
    ),
    (
        "invalidate_tool",
        "WP15a row. The registry's own GUI reason reads: no GUI control drops a tool's results alone; replace_tool writes and drops. \
         MEASURED 2026-09-14: no production `Command` literal.",
    ),
    (
        "invalidate_model",
        "WP15a row. The registry's own GUI reason reads: the three model refresh doors take adopt_model_geometry, which also drops. \
         MEASURED 2026-09-14: no production `Command` literal.",
    ),
    (
        "invalidate_toolpath_inputs",
        "WP15a row. The registry's own GUI reason reads: the feeds Apply funnel writes one replace_toolpath_config, which drops. \
         MEASURED 2026-09-14: no production `Command` literal.",
    ),
    (
        "update_stock_from_bbox",
        "WP15a row. The registry's own GUI reason reads: no GUI control sizes the stock from a bounding box. \
         MEASURED 2026-09-14: no production `Command` literal.",
    ),
    (
        "replace_tools",
        "WP15a row. The registry's own GUI reason reads: no GUI control replaces the whole tools list. \
         MEASURED 2026-09-14: no production `Command` literal.",
    ),
    (
        "set_feeds_provenance",
        "WP15a row. The registry's own GUI reason reads: the optimizer carries the stamp in restore_toolpath_snapshot since WP8. \
         MEASURED 2026-09-14: no production `Command` literal.",
    ),
];

/// Every row whose three surfaces all read `Reach::Skip`.
fn measured_all_skip() -> BTreeSet<&'static str> {
    CommandId::ALL
        .iter()
        .filter(|id| {
            let surfaces = id.surfaces();
            matches!(surfaces.gui, Reach::Skip(_))
                && matches!(surfaces.mcp, Reach::Skip(_))
                && matches!(surfaces.cli, Reach::Skip(_))
        })
        .map(|id| id.wire_name())
        .collect()
}

/// ARM 2. The all-Skip set equals the allow-list, in both directions.
#[test]
fn no_registry_row_declares_skip_on_every_surface() {
    assert!(
        !CommandId::ALL.is_empty(),
        "the command registry has no rows"
    );

    let measured = measured_all_skip();
    let allowed: BTreeSet<&str> = ALL_SKIP_ALLOW_LIST.iter().map(|(name, _)| *name).collect();

    let unlisted: Vec<&str> = measured.difference(&allowed).copied().collect();
    assert!(
        unlisted.is_empty(),
        "WP28 part 3: a row that declares `Reach::Skip` on the GUI, the \
         MCP server and the batch CLI is reached by no surface, so \
         nothing in the product runs it. Delete the row and its setter, \
         or put it on ALL_SKIP_ALLOW_LIST with the reading that keeps \
         it. I read {} unlisted row(s): {}",
        unlisted.len(),
        unlisted.join(", ")
    );

    let stale: Vec<&str> = allowed.difference(&measured).copied().collect();
    assert!(
        stale.is_empty(),
        "ALL_SKIP_ALLOW_LIST names {} row(s) that are no longer \
         all-Skip, or no longer in the registry: {}. Remove the \
         entry — the list is a ledger of today's finding, not a \
         graveyard.",
        stale.len(),
        stale.join(", ")
    );
}

/// ARM 3, the non-vacuity guard. Every allow-list entry carries a reason,
/// and the registry is big enough to be the registry.
#[test]
fn the_allow_list_is_not_vacuous() {
    assert!(
        CommandId::ALL.len() >= 50,
        "the registry reads {} rows. Under 50 means the walk reads a \
         list that is not the command registry.",
        CommandId::ALL.len()
    );
    for (name, reason) in ALL_SKIP_ALLOW_LIST {
        assert!(!name.is_empty(), "an allow-list entry has no wire name");
        assert!(
            reason.len() >= 40,
            "the allow-list entry `{name}` carries no reason a reader \
             can check"
        );
    }
}
