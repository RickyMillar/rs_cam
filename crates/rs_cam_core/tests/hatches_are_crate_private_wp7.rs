//! WP7 — the nine `ProjectSession` mutation hatches are crate-private, and
//! no surface outside `rs_cam_core` names one.
//!
//! # What this file guards
//!
//! `ProjectSession` publishes one mutation door, `ProjectSession::apply`.
//! Nine `*_mut()` accessors reached around it. Each one hands out a
//! `&mut` to a field, so a caller could write a generation input and
//! leave the cached result in place. That is the defect G-FRESHSTATE
//! and N13 both closed one caller at a time.
//!
//! WP7 makes the reach a compile error: each of the nine is
//! `pub(crate) fn`, or it is gone. Six had no caller left anywhere after
//! the migration — production or test, in any crate — so WP7 deleted
//! them; three keep an in-crate caller and are `pub(crate) fn`. A
//! compile error is not visible to the commanded gate,
//! because the gate never runs `cargo build -p rs_cam_viz` — a
//! dev-target build compiles viz with its dev dependencies, and a
//! feature door would unify into the same library. The
//! IMPLEMENTATION_PLAN §7 says so and names a grep as the gate-visible
//! check instead:
//!
//! ```text
//! | After | Command | Expect |
//! |---|---|---|
//! | WP7 | rg "\.(stock_mut|machine_mut|tools_mut|models_mut|post_mut|
//! wizard_mut|setups_mut|find_setup_by_id_mut|
//! find_toolpath_config_by_id_mut|toolpath_configs_mut|insert_result)\("
//! crates/rs_cam_viz/src crates/rs_cam_cli/src | 0 |
//! | WP7 | rg -c "^\s*pub fn [a-z_0-9]*_mut"
//! crates/rs_cam_core/src/session/mod.rs | 0 |
//! ```
//!
//! This file encodes that grep as a test, and goes WIDER than §7 on
//! purpose. §7 scans `src` alone. An integration test, a bench and an
//! example are each a separate crate that links `rs_cam_core`, so each
//! one breaks on the same visibility change and each one must be
//! migrated. The walk therefore covers `tests/`, `benches/` and
//! `examples/` beside `src/`.
//!
//! # The three arms
//!
//! 1. Every hatch is either declared exactly once in
//!    `crates/rs_cam_core/src/session/` as `pub(crate) fn`, or absent
//!    from `crates/rs_cam_core/src/session/` altogether. No hatch reads
//!    `pub fn`. All nine names must be accounted for, so a name that
//!    neither arm reaches fails here.
//! 2. No line outside `crates/rs_cam_core/src` names `.<hatch>(`.
//! 3. `insert_result` is not public, and `wizard_mut` is declared
//!    nowhere. WP3 closed the first; §19 ruling 5 deleted the second.
//!
//! # Red before the fix
//!
//! Arm 1 reads `pub fn` on all nine. Arm 2 reads 17 lines that name a
//! hatch: one production site in viz, one CLI example, and the rest
//! test sites. A call split over two lines counts its continuation, so
//! the line count runs above the 11 call sites the fix migrates.
//!
//! # The scanner allowlist
//!
//! Three files name the hatches as DATA, not as calls: the two WP6 /
//! WP6b source scanners and this file. A scanner's own table of
//! forbidden strings is not a call, so [`SCANNER_FILES`] skips them.
//! The list is deliberately short and explicit: a new entry is a claim
//! that the file scans source rather than mutates a session.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

/// The nine accessors WP7 closed. Three stay as `pub(crate) fn`
/// (`setups_mut`, `find_toolpath_config_by_id_mut`,
/// `toolpath_configs_mut`); WP7 deleted the other six.
const HATCHES: &[&str] = &[
    "stock_mut",
    "machine_mut",
    "tools_mut",
    "models_mut",
    "post_mut",
    "setups_mut",
    "find_setup_by_id_mut",
    "find_toolpath_config_by_id_mut",
    "toolpath_configs_mut",
];

/// Files that carry a hatch name as scanner data. See the module doc.
const SCANNER_FILES: &[&str] = &[
    "egui_draw_sites_write_through_commands_wp6.rs",
    "non_egui_sites_write_through_commands_wp6b.rs",
    "hatches_are_crate_private_wp7.rs",
];

/// The lowest number of files the walk must visit. A walk that finds a
/// handful of files proves nothing, so an empty or truncated tree fails
/// here rather than passing green.
const MIN_FILES_WALKED: usize = 100;

/// This crate's root directory.
fn core_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The directories outside `crates/rs_cam_core/src` that link this crate.
///
/// Every one of them is a separate compilation unit, so every one of
/// them breaks on a `pub(crate)` hatch.
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

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Whether the file's own name puts it on the scanner allowlist.
fn is_scanner(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| SCANNER_FILES.contains(&name))
}

/// Whether `line` is a comment. A `//`, `///` or `//!` line carries
/// prose, and prose names a hatch to explain what it replaced.
fn is_comment(line: &str) -> bool {
    line.trim_start().starts_with("//")
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
    out
}

/// Where one hatch is declared, and at what visibility.
///
/// The match reads the FIRST line of the declaration alone.
/// `find_toolpath_config_by_id_mut` opens a multi-line signature, so a
/// pattern that demanded the argument list on the same line would miss
/// it.
fn declarations(hatch: &str) -> Vec<(PathBuf, usize, String)> {
    let public = format!("pub fn {hatch}(");
    let crate_private = format!("pub(crate) fn {hatch}(");
    let mut out = Vec::new();
    for path in session_sources() {
        for (number, raw) in read(&path).lines().enumerate() {
            if is_comment(raw) {
                continue;
            }
            let trimmed = raw.trim_start();
            if trimmed.starts_with(&public) || trimmed.starts_with(&crate_private) {
                out.push((path.clone(), number + 1, trimmed.to_owned()));
            }
        }
    }
    out
}

/// Every `pub fn <name>_mut(` declared under `src/session/`.
///
/// This is the IMPLEMENTATION_PLAN §7 grep, widened from `mod.rs` to the
/// whole directory. It closes the hole the "absent" state opens: a
/// deleted hatch that comes back under a NEW name is not in [`HATCHES`],
/// so nothing else here would read it.
fn public_mut_declarations() -> Vec<String> {
    let mut out = Vec::new();
    for path in session_sources() {
        for (number, raw) in read(&path).lines().enumerate() {
            if is_comment(raw) {
                continue;
            }
            let trimmed = raw.trim_start();
            let Some(rest) = trimmed.strip_prefix("pub fn ") else {
                continue;
            };
            let Some(name) = rest.split('(').next() else {
                continue;
            };
            if name.ends_with("_mut") {
                out.push(format!("{}:{}: {trimmed}", path.display(), number + 1));
            }
        }
    }
    out
}

/// ARM 1. Each hatch is declared once as `pub(crate) fn`, or it is
/// absent from `src/session/`. Neither state lets a surface outside the
/// crate reach a field.
///
/// A hatch with no caller left anywhere is deleted rather than kept
/// behind `#[allow(dead_code)]`, so the two accepted states are
/// "crate-private" and "gone". A name declared twice fails: the walk
/// could then read the crate-private one and miss a public sibling.
#[test]
fn every_mutation_hatch_is_declared_crate_private() {
    let mut accounted = 0_usize;
    let mut public: Vec<String> = Vec::new();
    for hatch in HATCHES {
        let sites = declarations(hatch);
        assert!(
            sites.len() <= 1,
            "`{hatch}` must be declared at most once under src/session/. \
             I read {} declaration(s): {:?}",
            sites.len(),
            sites
        );
        if let Some((path, number, line)) = sites.first()
            && line.starts_with("pub fn ")
        {
            public.push(format!("{}:{number}: {line}", path.display()));
        }
        accounted += 1;
    }
    public.extend(public_mut_declarations());
    assert_eq!(
        accounted,
        HATCHES.len(),
        "all {} names must be accounted for — declared crate-private or \
         absent — or arm 1 asserts nothing. I accounted for {accounted}",
        HATCHES.len()
    );
    assert!(
        public.is_empty(),
        "WP7: every mutation hatch is `pub(crate) fn` or deleted, and no \
         public `*_mut` door stands under src/session/. Every surface \
         mutates through `ProjectSession::apply`. I read {} public \
         declaration(s):\n  {}",
        public.len(),
        public.join("\n  ")
    );
}

// ── arm 2: the call sites ───────────────────────────────────────────────

/// ARM 2. No crate outside `rs_cam_core` names a hatch.
///
/// This is the §7 grep, widened to `tests/`, `benches/` and
/// `examples/`. It goes red the day a new caller appears, which is why
/// it carries the explanation.
#[test]
fn no_surface_outside_core_names_a_mutation_hatch() {
    let mut files = Vec::new();
    for root in scanned_roots() {
        collect_rs(&root, &mut files);
    }
    files.sort();
    assert!(
        files.len() >= MIN_FILES_WALKED,
        "the walk must visit at least {MIN_FILES_WALKED} files, or arm 2 \
         passes on a tree it never read. I read {}",
        files.len()
    );

    let mut hits: Vec<String> = Vec::new();
    for path in &files {
        if is_scanner(path) {
            continue;
        }
        for (number, raw) in read(path).lines().enumerate() {
            if is_comment(raw) {
                continue;
            }
            for hatch in HATCHES {
                if raw.contains(&format!(".{hatch}(")) {
                    hits.push(format!("{}:{}: {}", path.display(), number + 1, raw.trim()));
                }
            }
        }
    }

    assert!(
        hits.is_empty(),
        "WP7: the nine mutation hatches are crate-private. A surface \
         outside `rs_cam_core` mutates a session through \
         `ProjectSession::apply` with the matching `Command` row, or \
         builds one with `ProjectSessionBuilder`. I read {} call \
         site(s):\n  {}",
        hits.len(),
        hits.join("\n  ")
    );
}

// ── arm 3: the two hatches earlier packages closed ──────────────────────

/// ARM 3. `insert_result` stays crate-private and `wizard_mut` stays
/// deleted.
///
/// WP3 replaced `insert_result` with `Command::AdoptResult`. §19 ruling
/// 5 moved `WizardState` to the GUI and deleted `wizard_mut`. Neither
/// may come back as a public door.
#[test]
fn insert_result_stays_crate_private_and_wizard_mut_stays_deleted() {
    let mut public_insert: Vec<String> = Vec::new();
    let mut wizard: Vec<String> = Vec::new();
    for path in session_sources() {
        for (number, raw) in read(&path).lines().enumerate() {
            if is_comment(raw) {
                continue;
            }
            let trimmed = raw.trim_start();
            if trimmed.starts_with("pub fn insert_result(") {
                public_insert.push(format!("{}:{}", path.display(), number + 1));
            }
            if trimmed.contains("fn wizard_mut(") {
                wizard.push(format!("{}:{}", path.display(), number + 1));
            }
        }
    }
    assert!(
        public_insert.is_empty(),
        "WP3 made `insert_result` crate-private. A completion arrives as \
         `Command::AdoptResult`, which carries the revision the lane \
         started from. I read {public_insert:?}"
    );
    assert!(
        wizard.is_empty(),
        "§19 ruling 5 deleted `wizard_mut`. `WizardState` lives on the \
         GUI state. I read {wizard:?}"
    );
}
