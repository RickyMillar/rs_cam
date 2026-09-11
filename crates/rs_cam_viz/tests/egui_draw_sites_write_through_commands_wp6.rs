//! WP6 — the twelve egui draw sites write through the command door.
//!
//! Programme: `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md`
//! §4 WP6 and §19.
//!
//! An immediate-mode panel used to take a `&mut` hatch on the session and
//! let the widget write the project in place. No core setter ran, so the
//! core kept the previous parameter set's results and the cards stayed
//! green (G-FRESHSTATE). WP6 replaces every such site with a scratch copy
//! plus one `Command`.
//!
//! Three scans measure that, in the source-reading idiom of
//! `crates/rs_cam_viz/tests/ui_string_hygiene.rs:78-105`:
//!
//! 1. No file under `src/ui/` names one of the four hatches. The scan
//!    strips line comments and stops at the first `#[cfg(test)]`, so a
//!    test fixture that builds a session by hand is out of scope.
//! 2. The five post-write notification events are gone from the
//!    `AppEvent` enum (§14 ruling 3). Core invalidation through the rows
//!    replaces them.
//! 3. Every registry row WP6 adopts declares `gui: Reach::Reached` and
//!    is CONSTRUCTED under `src/ui/`. A row the registry says the GUI
//!    reaches, with no `Command::<Id>(` anywhere in the panel code, is a
//!    claim the surface does not keep.
//!
//! NOT MEASURED here: what the commands do. The core rows carry their
//! own sentries, and `crates/rs_cam_viz/src/controller/tests.rs` measures
//! the staleness a stock edit through this path leaves.
//!
//! The row list is spelled with WIRE NAMES, not `CommandId` variants, so
//! this file compiles against the pre-fix tree: three of the rows do not
//! exist yet, and the scan reports each missing name as a failure rather
//! than as a build error.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use rs_cam_core::session::{CommandId, Reach};

/// The session hatches a draw site must not name.
const HATCHES: &[&str] = &[
    ".stock_mut(",
    ".machine_mut(",
    ".tools_mut(",
    ".find_setup_by_id_mut(",
];

/// The five post-write notification events §14 ruling 3 deletes.
const DELETED_EVENTS: &[&str] = &[
    "StockChanged",
    "StockMaterialChanged",
    "MachineChanged",
    "HeightPlanesChanged",
    "FixtureChanged",
];

/// The wire name of every registry row a WP6 draw site calls.
///
/// Eleven rows serve the twelve sites: two machine widgets share
/// `load_machine_from_library`.
const ADOPTED_ROWS: &[&str] = &[
    "replace_tool",
    "set_stock_config",
    "set_setup_face",
    "set_setup_rotation",
    "set_setup_datum",
    "set_setup_models",
    "replace_fixture",
    "replace_keep_out",
    "load_machine_from_library",
    "set_machine_kinematics",
    "import_machine_settings",
];

/// Every `.rs` file under this crate's `src/ui/`.
fn ui_sources() -> Vec<PathBuf> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest.join("src/ui");
    assert!(
        root.is_dir(),
        "scanned path {} no longer exists",
        root.display()
    );
    let mut out = Vec::new();
    collect_rs(&root, &mut out);
    out.sort();
    assert!(!out.is_empty(), "src/ui holds no Rust source to scan");
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

/// The production lines of one file: comments stripped, and everything
/// from the test MODULE dropped.
///
/// The cut-off reads `#[cfg(test)]` plus the item it gates, not the
/// attribute alone. Two files here gate a single ITEM with it and carry
/// production code below — `ui/properties/mod.rs:3666` gates a function
/// and the real module sits at `:5866` — so an attribute-only cut would
/// stop scanning two thousand production lines early and call the file
/// clean.
fn starts_item(trimmed: &str, kind: &str) -> bool {
    for prefix in ["", "pub ", "pub(crate) ", "pub(super) "] {
        let head = format!("{prefix}{kind}");
        if trimmed.starts_with(&head) {
            return true;
        }
    }
    false
}

fn production_lines(source: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut pending_cfg_test = false;
    for (number, raw) in source.lines().enumerate() {
        let trimmed = raw.trim_start();
        if trimmed == "#[cfg(test)]" {
            pending_cfg_test = true;
        } else if pending_cfg_test {
            if starts_item(trimmed, "mod ") {
                break;
            }
            for kind in [
                "fn ", "impl ", "struct ", "enum ", "const ", "static ", "use ",
            ] {
                if starts_item(trimmed, kind) {
                    pending_cfg_test = false;
                    break;
                }
            }
        }
        let code = match raw.find("//") {
            Some(at) => &raw[..at],
            None => raw,
        };
        out.push((number + 1, code.to_owned()));
    }
    out
}

// ── scan 1: the hatches ──────────────────────────────────────────

/// No draw site names a session hatch.
#[test]
fn no_ui_source_names_a_session_hatch() {
    let mut offences: Vec<String> = Vec::new();
    let mut scanned = 0_usize;
    for path in ui_sources() {
        let source = read(&path);
        for (number, line) in production_lines(&source) {
            scanned += 1;
            for hatch in HATCHES {
                if line.contains(hatch) {
                    offences.push(format!("{}:{number} names {hatch}", path.display()));
                }
            }
        }
    }
    assert!(
        scanned > 0,
        "the scan read no production line, so it asserts nothing"
    );
    assert!(
        offences.is_empty(),
        "a draw site still writes the session through a hatch. Each site \
         must edit a scratch copy and apply one Command:\n{}",
        offences.join("\n")
    );
}

/// The hatch scan finds a hatch when one is there.
///
/// A locator that matched nothing would pass the scan above on any tree.
/// `crates/rs_cam_core/src/session/mod.rs` publishes all four, so the
/// needles are pinned against the file that declares them.
#[test]
fn the_hatch_scan_finds_the_declarations_it_is_built_around() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../rs_cam_core/src/session/mod.rs");
    assert!(path.is_file(), "{} no longer exists", path.display());
    let source = read(&path);
    for hatch in HATCHES {
        let name = hatch.trim_start_matches('.').trim_end_matches('(');
        let declaration = format!("pub fn {name}");
        assert!(
            source.contains(&declaration),
            "the scan hunts for `{hatch}`, but core declares no `{declaration}`. \
             The hatch was renamed and this scan now measures nothing."
        );
    }
}

// ── scan 2: the five deleted events ──────────────────────────────

/// The `AppEvent` enum carries none of the five post-write events.
#[test]
fn the_app_event_enum_dropped_the_five_post_write_events() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui/mod.rs");
    assert!(path.is_file(), "{} no longer exists", path.display());
    let source = read(&path);
    let mut survivors: Vec<&str> = Vec::new();
    for (_, line) in production_lines(&source) {
        for event in DELETED_EVENTS {
            if line.contains(event) {
                survivors.push(event);
            }
        }
    }
    survivors.sort_unstable();
    survivors.dedup();
    assert!(
        survivors.is_empty(),
        "§14 ruling 3 deletes these events; ui/mod.rs still names {survivors:?}. \
         Core invalidation through the command rows replaces them."
    );
    assert!(
        source.contains("pub enum AppEvent"),
        "the scan reads the wrong file: ui/mod.rs declares no AppEvent enum"
    );
}

// ── scan 3: the adopted rows ─────────────────────────────────────

/// Resolve one wire name to its registry row.
fn row_for(wire: &str) -> CommandId {
    *CommandId::ALL
        .iter()
        .find(|id| id.wire_name() == wire)
        .unwrap_or_else(|| {
            panic!(
                "the registry declares no row with the wire name '{wire}'. \
                 WP6 needs that row to replace a draw-site hatch."
            )
        })
}

/// Every adopted row says the GUI reaches it.
#[test]
fn every_adopted_row_declares_a_gui_reach() {
    assert!(!ADOPTED_ROWS.is_empty(), "the adopted row list is empty");
    for wire in ADOPTED_ROWS {
        let id = row_for(wire);
        assert!(
            matches!(id.surfaces().gui, Reach::Reached),
            "a WP6 draw site calls '{wire}', but the registry row {id:?} \
             still declares `gui: Reach::Skip`. §19 ruling 3 flips the \
             field in the commit that adds the caller."
        );
    }
}

/// Every adopted row is constructed under `src/ui/`.
///
/// The registry's `gui: Reached` is a claim about the panel code. This
/// reads the panel code and checks it.
#[test]
fn every_adopted_row_is_constructed_in_a_draw_site() {
    let sources: Vec<String> = ui_sources().iter().map(|p| read(p)).collect();
    let mut checked = 0_usize;
    for wire in ADOPTED_ROWS {
        let id = row_for(wire);
        let needle = format!("Command::{id:?}(");
        assert!(
            sources.iter().any(|s| s.contains(&needle)),
            "the registry says the GUI reaches {id:?}, but no file under \
             src/ui/ constructs `{needle}`"
        );
        checked += 1;
    }
    assert!(
        checked > 0,
        "no adopted row was checked, so this scan asserts nothing"
    );
}
