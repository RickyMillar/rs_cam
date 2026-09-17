//! WP6b — the non-egui viz sites and the CLI sites write through the
//! command door.
//!
//! Programme: `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md`
//! §4 WP6b, §19 and §20.
//!
//! WP6 closed the twelve egui draw sites. This file closes the rest: the
//! controller doors, the application event arms and the batch CLI. Each
//! one took a `&mut` hatch on the session and wrote the project in place,
//! so no core setter ran and the cached results stayed green
//! (G-FRESHSTATE).
//!
//! Three scans measure that, in the source-reading idiom of
//! `crates/rs_cam_viz/tests/egui_draw_sites_write_through_commands_wp6.rs`.
//!
//! 1. No production file under `crates/rs_cam_viz/src` or
//!    `crates/rs_cam_cli/src` names a session hatch. The scan strips line
//!    comments and stops at the test MODULE.
//! 2. `rs_cam_core`'s session declares no `wizard_mut`. §19 ruling 5
//!    moves `WizardState` to the view: nothing saves it and the project
//!    loader resets it on every load, so it is GUI state and not project
//!    data. The hatch is deleted, not narrowed.
//! 3. Every registry row WP6b flips to `cli: Reach::Reached` is
//!    CONSTRUCTED under `crates/rs_cam_cli/src`. A row the registry says
//!    the CLI reaches, with no `Command::<Id>(` in the batch binary, is a
//!    claim the surface does not keep.
//!
//! The `gui` half of scan 3 is NOT repeated here. The WP13 sentry
//! `crates/rs_cam_viz/tests/command_surface_completeness.rs` already
//! asserts that every `gui: Reached` row is constructed in viz `src/`.
//!
//! Scan 3 reads `Command` rows alone. `GenerateToolpath` declares
//! `cli: Reached` and is a `Job`: the CLI takes it through
//! `ProjectSession::generate_toolpath`, which runs the three steps
//! inline, so the binary constructs no `Job::GenerateToolpath(`.
//!
//! One exclusion, deliberate: `crates/rs_cam_viz/src/compute/**` and
//! `crates/rs_cam_viz/src/controller/events/compute.rs` belong to WP10
//! and WP11b (§15), not to this package.
//!
//! `app/mcp/generation.rs` held a second one. Its generate arm named
//! `toolpath_configs_mut` once, to write the debug-capture flag, and the
//! allowance was a ceiling of 1. WP11b removed the site, and the sentry
//! review of 2026-09-17 proved the ceiling was dead: a second call
//! passed. The allowance is gone, so that file scans like every other.
//!
//! Red before the fix: scan 1 reports every remaining hatch call, scan 2
//! reports `wizard_mut`, and scan 3 reports the rows the CLI does not
//! construct yet.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use rs_cam_core::session::{CommandId, CommandKind, Reach};

/// The session hatches a production site must not name.
///
/// Nine hatches survive WP6b (§20). `wizard_mut` is the tenth and scan 2
/// deletes it, so it is listed here too: a view file that still names it
/// is the same defect.
const HATCHES: &[&str] = &[
    ".stock_mut(",
    ".machine_mut(",
    ".tools_mut(",
    ".models_mut(",
    ".post_mut(",
    ".wizard_mut(",
    ".setups_mut(",
    ".find_setup_by_id_mut(",
    ".find_toolpath_config_by_id_mut(",
    ".toolpath_configs_mut(",
];

/// The wire name of every registry row WP6b flips to `cli: Reached`.
const CLI_ADOPTED_ROWS: &[&str] = &[
    "set_stock_config",
    "set_spindle_strategy",
    "set_machine_kinematics",
    "replace_toolpath_config",
    "set_toolpath_enabled",
];

/// Every `.rs` file this scan reads, with the WP10 and WP11b paths
/// dropped.
fn scanned_sources() -> Vec<PathBuf> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let viz = manifest.join("src");
    let cli = manifest
        .parent()
        .expect("the viz crate sits beside its siblings")
        .join("rs_cam_cli/src");
    let mut out = Vec::new();
    for root in [&viz, &cli] {
        assert!(
            root.is_dir(),
            "scanned path {} no longer exists",
            root.display()
        );
        collect_rs(root, &mut out);
    }
    out.retain(|path| !is_excluded(path));
    out.sort();
    assert!(!out.is_empty(), "the scan found no Rust source to read");
    out
}

/// Whether this path is out of WP6b's scope.
fn is_excluded(path: &Path) -> bool {
    let text = path.to_string_lossy().replace('\\', "/");
    text.contains("/src/compute/")
        || text.ends_with("/controller/events/compute.rs")
        || text.ends_with("/tests.rs")
        || text.ends_with("_tests.rs")
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

/// Whether `trimmed` opens an item of `kind`, at any visibility.
fn starts_item(trimmed: &str, kind: &str) -> bool {
    for prefix in ["", "pub ", "pub(crate) ", "pub(super) "] {
        let head = format!("{prefix}{kind}");
        if trimmed.starts_with(&head) {
            return true;
        }
    }
    false
}

/// The production lines of one file: comments stripped, and everything
/// from the test MODULE dropped.
///
/// The cut-off reads `#[cfg(test)]` plus the item it gates, not the
/// attribute alone, because a scanned file gates a single ITEM with it
/// and carries production code below.
///
/// A gated module DECLARATION — `mod name;`, which names another file —
/// does not end the scan either. `controller.rs` declares three of them
/// in its first twenty lines, and a cut that stopped at the first one
/// would call the remaining five hundred production lines clean.
fn production_lines(source: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut pending_cfg_test = false;
    for (number, raw) in source.lines().enumerate() {
        let trimmed = raw.trim_start();
        if trimmed == "#[cfg(test)]" {
            pending_cfg_test = true;
        } else if pending_cfg_test {
            if starts_item(trimmed, "mod ") {
                if !trimmed.ends_with(';') {
                    break;
                }
                pending_cfg_test = false;
                continue;
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

/// No production site outside the egui panels names a session hatch.
#[test]
fn no_production_site_names_a_session_hatch() {
    let mut offences: Vec<String> = Vec::new();
    let mut scanned = 0_usize;
    for path in scanned_sources() {
        let display = path.to_string_lossy().replace('\\', "/");
        let source = read(&path);
        for (number, line) in production_lines(&source) {
            scanned += 1;
            for hatch in HATCHES {
                if !line.contains(hatch) {
                    continue;
                }
                offences.push(format!("{display}:{number} names {hatch}"));
            }
        }
    }
    assert!(
        scanned > 0,
        "the scan read no production line, so it asserts nothing"
    );
    assert!(
        offences.is_empty(),
        "a production site still writes the session through a hatch. Each \
         site must build a Command and take `ProjectSession::apply`:\n{}",
        offences.join("\n")
    );
}

/// The scan's reader finds a hatch on a code line, and only there.
///
/// A locator that matched nothing would pass the scan above on any tree.
#[test]
fn the_reader_finds_a_hatch_on_a_code_line_and_not_in_a_comment() {
    let source = concat!(
        "fn a() {\n",
        "    let x = session.post_mut();\n",
        "    // session.models_mut() in a comment is not a call\n",
        "}\n",
        "#[cfg(test)]\n",
        "mod tests {\n",
        "    let y = session.setups_mut();\n",
        "}\n",
    );
    let code: String = production_lines(source)
        .iter()
        .map(|(_, line)| line.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        code.contains(".post_mut("),
        "the reader must find a hatch on a code line"
    );
    assert!(
        !code.contains(".models_mut("),
        "the reader must strip a line comment before matching"
    );
    assert!(
        !code.contains(".setups_mut("),
        "the reader must stop at the test module"
    );
}

/// A gated module DECLARATION does not end the scan.
///
/// `crates/rs_cam_viz/src/controller.rs` declares three `#[cfg(test)]`
/// modules by name in its first twenty lines. A cut that stopped there
/// would read twenty of its five hundred production lines and call the
/// file clean.
#[test]
fn a_gated_module_declaration_does_not_end_the_scan() {
    let source = concat!(
        "#[cfg(test)]\n",
        "#[allow(\n",
        "    clippy::unwrap_used,\n",
        ")]\n",
        "mod holder_clearance_scope_g_holderscope;\n",
        "pub mod generate_all;\n",
        "fn later() {\n",
        "    let x = session.models_mut();\n",
        "}\n",
        "#[cfg(test)]\n",
        "mod tests {\n",
        "    let y = session.post_mut();\n",
        "}\n",
    );
    let code: String = production_lines(source)
        .iter()
        .map(|(_, line)| line.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        code.contains(".models_mut("),
        "a gated `mod name;` declaration must not end the scan"
    );
    assert!(
        !code.contains(".post_mut("),
        "a gated `mod name {{` block must still end the scan"
    );
}

/// The exclusion list drops the WP10 and WP11b paths and nothing else.
#[test]
fn the_exclusion_list_drops_the_compute_paths_alone() {
    let dropped = [
        "/r/crates/rs_cam_viz/src/compute/mod.rs",
        "/r/crates/rs_cam_viz/src/controller/events/compute.rs",
        "/r/crates/rs_cam_viz/src/controller/tests.rs",
        "/r/crates/rs_cam_viz/src/controller/workflow_tests.rs",
    ];
    let scanned = [
        "/r/crates/rs_cam_viz/src/controller/io.rs",
        "/r/crates/rs_cam_viz/src/app/input.rs",
        "/r/crates/rs_cam_cli/src/smoke.rs",
    ];
    for path in dropped {
        assert!(is_excluded(Path::new(path)), "{path} must be dropped");
    }
    for path in scanned {
        assert!(!is_excluded(Path::new(path)), "{path} must be scanned");
    }
}

// ── scan 2: the wizard hatch is deleted ──────────────────────────

/// The core session declares no `wizard_mut`, and no `wizard` field.
///
/// §19 ruling 5. The scan reads core's source rather than calling the
/// method, because a call to a deleted method is a build error and this
/// file must compile against the pre-fix tree to report a red.
#[test]
fn the_core_session_declares_no_wizard_hatch() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the viz crate sits beside its siblings")
        .join("rs_cam_core/src/session/mod.rs");
    assert!(path.is_file(), "{} no longer exists", path.display());
    let source = read(&path);
    assert!(
        source.contains("pub struct ProjectSession"),
        "the scan reads the wrong file: it declares no ProjectSession"
    );
    let mut survivors: Vec<String> = Vec::new();
    for (number, line) in production_lines(&source) {
        if line.contains("fn wizard_mut") || line.contains("fn wizard(") {
            survivors.push(format!("session/mod.rs:{number}:{}", line.trim()));
        }
    }
    assert!(
        survivors.is_empty(),
        "§19 ruling 5 moves WizardState to the view, so the session holds \
         no wizard accessor. Still declared:\n{}",
        survivors.join("\n")
    );
}

// ── scan 3: the rows the CLI adopts ──────────────────────────────

/// Resolve one wire name to its registry row.
fn row_for(wire: &str) -> CommandId {
    *CommandId::ALL
        .iter()
        .find(|id| id.wire_name() == wire)
        .unwrap_or_else(|| {
            panic!(
                "the registry declares no row with the wire name '{wire}'. \
                 WP6b needs that row to replace a CLI hatch."
            )
        })
}

/// Every row WP6b's CLI sites take declares `cli: Reach::Reached`.
#[test]
fn every_cli_adopted_row_declares_a_cli_reach() {
    assert!(
        !CLI_ADOPTED_ROWS.is_empty(),
        "the adopted row list is empty"
    );
    for wire in CLI_ADOPTED_ROWS {
        let id = row_for(wire);
        assert!(
            matches!(id.surfaces().cli, Reach::Reached),
            "a WP6b CLI site calls '{wire}', but the registry row {id:?} \
             still declares `cli: Reach::Skip`. §19 ruling 3 flips the \
             field in the commit that adds the caller."
        );
    }
}

/// Every `Command` row the registry says the CLI reaches is constructed
/// in the batch binary.
#[test]
fn every_cli_reached_command_row_is_constructed_in_the_cli() {
    let cli_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the viz crate sits beside its siblings")
        .join("rs_cam_cli/src");
    let mut paths = Vec::new();
    collect_rs(&cli_root, &mut paths);
    let sources: Vec<String> = paths.iter().map(|p| read(p)).collect();
    assert!(!sources.is_empty(), "the CLI crate holds no source to read");

    let mut checked = 0_usize;
    let mut missing: Vec<String> = Vec::new();
    for id in CommandId::ALL {
        if id.kind() != CommandKind::Command {
            continue;
        }
        if !matches!(id.surfaces().cli, Reach::Reached) {
            continue;
        }
        checked += 1;
        let needle = format!("Command::{id:?}(");
        if !sources.iter().any(|s| s.contains(&needle)) {
            missing.push(format!("{id:?} (`{needle}`)"));
        }
    }
    assert!(
        checked >= CLI_ADOPTED_ROWS.len(),
        "the registry declares {checked} `cli: Reached` Command rows, fewer \
         than the {} rows WP6b adopts, so this scan asserts too little",
        CLI_ADOPTED_ROWS.len()
    );
    assert!(
        missing.is_empty(),
        "the registry says the CLI reaches these rows, but no file under \
         crates/rs_cam_cli/src constructs them:\n{}",
        missing.join("\n")
    );
}
