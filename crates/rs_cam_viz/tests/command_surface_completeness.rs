//! WP13 — the command surface is complete across BOTH registries.
//!
//! Programme: `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md`
//! §4 WP13 and §21 ruling 9.
//!
//! Two registries declare what this application runs. `for_each_command!`
//! (`crates/rs_cam_core/src/session/command.rs`) declares what the session
//! runs. `for_each_ui_command!` (`crates/rs_cam_viz/src/ui_command.rs`)
//! declares what the VIEW runs. `SurfaceId` is their union, and this file
//! measures the properties that only hold over the union.
//!
//! Five properties:
//!
//! 1. Every `CommandId` whose `gui` reach is `Reached` is CONSTRUCTED in
//!    the view's own sources, and every `gui: Skip` row is not.
//! 2. Every `UiCommandId` whose `gui` reach is `Reached` is CONSTRUCTED in
//!    a production view file, every `Skip` carries a reason, every view
//!    command declares `cli: Skip`, and no view row is orphaned.
//! 3. No view command handler writes `ProjectSession`, and the view read
//!    door holds the session by shared reference.
//!
//! Property 3 is ARM-DEEP and not transitive. An arm that reads
//! `=> self.handle_reset_simulation()` passes whatever that method does.
//! Ruling 9 asked for an arm scan, and the compile-level form the scout
//! proposed — one handler taking `&ProjectSession` beside the `&mut`
//! view state — is the follow-up that retires the scan.
//! 4. Every `mcp: Reached` view row has a tool on the wire, and every
//!    `mcp: Skip` view row has none.
//! 5. No wire name appears twice across the two registries.
//!
//! **NOT MEASURED here**, and where each one lives instead.
//! `PLAN.md:495-504` lists seven prose properties. Three are properties of
//! the pipeline and not of the command surface: one implementation per
//! generation-wide post-step, one phase-builder change per export-phase
//! property, one geometry loader per import format. This file asserts
//! NOTHING about those three. The other four already carry a sentry, and
//! this file cites rather than duplicates them:
//!
//! - a supported parameter cannot silently ignore writes —
//!   `crates/rs_cam_core/tests/set_param_refuses_absent_field_n5.rs`;
//! - mutation callers cannot forget dependency invalidation —
//!   `crates/rs_cam_core/tests/command_registry_completeness.rs`;
//! - timing aggregations cannot consume different clocks —
//!   `crates/rs_cam_core/tests/air_cut_one_time_base_g_airdenom.rs`;
//! - stale display geometry cannot become executable output —
//!   `crates/rs_cam_viz/tests/export_refuses_stale_result_g_stalexport.rs`.
//!
//! Every scan STRIPS `//` comments before matching. A doc comment naming
//! a command is prose, not a caller, and `command_registry_surfaces.rs`
//! does not strip today — which is the flaw this file does not inherit.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use rs_cam_core::session::{CommandId, CommandKind, Reach};
use rs_cam_viz::ui_command::{SurfaceId, UiCommandId};

// ── fixtures ─────────────────────────────────────────────────────────

/// Read one source file, relative to this crate's manifest.
fn source(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    assert!(
        path.is_file(),
        "scanned path {} no longer exists",
        path.display()
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Strip every `//` comment from one source text.
///
/// A scan that matches a doc comment reports a caller that does not
/// exist. The strip is line-wise and deliberately crude: it cannot tell a
/// `//` inside a string literal from a comment, and no registry name
/// appears inside a string literal in the scanned files.
fn strip_comments(text: &str) -> String {
    text.lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every `.rs` file under `crates/rs_cam_viz/src`, recursively.
fn viz_sources() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = Vec::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("read_dir: {e}"));
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
    assert!(found.len() > 50, "the viz source walk found almost nothing");
    found
}

/// The four files that serve the MCP wire, and the `#[cfg(test)]` module
/// files under `src`.
///
/// A property about the GUI surface must not read an MCP construction as
/// a GUI caller. The first four files ARE the MCP surface: WP4's
/// delegating arm converts a `CoreRequest` into a `Command` inside
/// `app/mcp/commands.rs`, so most `gui: Skip` rows are constructed there
/// — correctly, and by the wire.
///
/// The rest are `#[cfg(test)]` modules their parent declares, so they are
/// fixtures and not the view either. `controller/tests.rs` was the first
/// one this list carried. WP15b migrated every fixture in the crate onto
/// `ProjectSession::apply`, so a fixture now constructs the same rows a
/// caller does: `controller/holder_clearance_scope_g_holderscope.rs`
/// builds a two-tool job with `Command::ReplaceTools`, a row whose `gui`
/// column says `Skip` and stays true — no GUI control replaces the tools
/// list. Read a construction here as a fixture, never as a caller.
///
/// P4 moved `app/mcp.rs`'s own `mod tests` out to `app/mcp/tests.rs`, so
/// that file joins the list as a test module. `app/mcp.rs` stays on it as
/// an MCP source. `ui/properties/operations/mod.rs` still carries an
/// INLINE `mod tests`; it stays in the scan, because its production half
/// is a view caller.
const MCP_SOURCES: &[&str] = &[
    "src/app/mcp.rs",
    "src/app/mcp/commands.rs",
    "src/app/mcp/diagnostics.rs",
    "src/app/mcp/generation.rs",
    "src/app/mcp/project.rs",
    "src/app/mcp/simulation.rs",
    "src/app/mcp/tests.rs",
    "src/app/mcp/view.rs",
    "src/mcp_bridge.rs",
    "src/mcp_server.rs",
    "src/controller/tests.rs",
    "src/controller/workflow_tests.rs",
    "src/controller/results_parity_tests.rs",
    "src/controller/holder_clearance_staleness_g_holderstale.rs",
    "src/controller/holder_clearance_scope_g_holderscope.rs",
    "src/controller/fixpoint_resolution_notice_g_resnotice.rs",
    "src/compute/worker/gen_parity_p0_tests.rs",
];

/// The index of the ITEM line a `#[cfg(test)]` attribute introduces.
///
/// Such an attribute is followed by an `#[allow(...)]` block that runs
/// over several lines, and the module declaration sits after it. The walk
/// consumes whole attributes by counting their brackets, then stops on
/// the first line that opens an item.
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
/// The block runs from the attribute to the line that closes it, found by
/// counting braces. The crate is `cargo fmt` clean, so the count is not
/// confused by a brace inside a string literal at this depth.
///
/// **A construction inside a test module is not a caller.** This rule
/// comes from `production_writes_go_through_apply_wp15a`, whose scan is
/// the sibling of this one. The two disagreed until C01: `app/export.rs`
/// built `Command::SetProjectName` in its own test module, and that was
/// read here as a GUI caller while wp15a read it as none. A row cannot be
/// both reached and skipped, so both scans now apply the same rule. L8
/// then deleted that row, so the example is history; the rule stands for
/// every row. The helper is duplicated because the two are separate test
/// binaries.
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

/// The view sources, comments and inline test modules stripped, excluding
/// the MCP wire files.
fn gui_source_text() -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let skip: Vec<PathBuf> = MCP_SOURCES.iter().map(|r| root.join(r)).collect();
    let mut kept = 0_usize;
    let text = viz_sources()
        .into_iter()
        .filter(|p| !skip.contains(p))
        .inspect(|_| kept += 1)
        .map(|p| {
            without_inline_test_modules(&strip_comments(
                &std::fs::read_to_string(&p).unwrap_or_default(),
            ))
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        kept > 40,
        "the GUI-only source set collapsed to {kept} files"
    );
    text
}

/// Does `text` construct the registry row `id` of kind `kind`?
///
/// The needle is the payload enum's own spelling, `Command::<Id>(`, with
/// a guard against matching `UiCommand::<Id>(` — the view registry has
/// rows whose names the core registry also carries.
fn constructs(text: &str, kind: CommandKind, id: CommandId) -> bool {
    let enum_name = match kind {
        CommandKind::Command => "Command",
        CommandKind::Query => "Query",
        CommandKind::Job => "Job",
        CommandKind::UiCommand | CommandKind::UiQuery => {
            panic!("the core registry declares no view row")
        }
    };
    // The variant IDENT, which `Debug` prints for a fieldless enum.
    let needle = format!("{enum_name}::{id:?}(");
    text.match_indices(&needle).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        !before.is_some_and(|c| c.is_alphanumeric() || c == '_')
    })
}

/// Does `text` construct the VIEW registry row `id`?
///
/// The needle keeps the `Ui(` of `AppEvent::Ui(...)`, which is the one
/// shape a view command travels in. The bare spelling `UiCommand::<Id>(`
/// also matches a MATCH ARM, and the two dispatch files —
/// `controller/events/mod.rs` and `app/input.rs` — stay inside the
/// GUI-only source set. With the bare needle every row that owns a
/// handler reads as constructed, which is the vacuity this census exists
/// to avoid. Do not drop the prefix.
///
/// A view READ needs no wrapper: it travels as a `UiQuery` into
/// `RsCamApp::ui_query`, so a `UiQuery` row keeps the plain needle. No
/// view read declares a GUI reach today, so that arm is a forward
/// statement and not a measured one.
fn view_constructs(text: &str, id: UiCommandId) -> bool {
    let needle = match id.kind() {
        CommandKind::UiCommand => format!("Ui(UiCommand::{id:?}("),
        CommandKind::UiQuery => format!("UiQuery::{id:?}("),
        other => panic!("{id:?} declares {other:?}, which belongs to the core registry"),
    };
    text.contains(&needle)
}

// ── P1 — a `gui` reach is a claim about a caller ──────────────────────

/// This scan carries no exemption.
///
/// `GenerateToolpath` held one until WP20. The GUI generated through
/// `ProjectSession::generate_toolpath`, which runs the job's three steps
/// inline, and the GUI worker could not name `ResolvedGenInputs`. WP11b
/// switched the worker and the drain, so
/// `crates/rs_cam_viz/src/controller/events/compute.rs` constructs
/// `Job::GenerateToolpath` and the scan reads that row like every other
/// one. An exemption here hides a live row, so add none.
#[test]
fn every_gui_reached_core_row_is_constructed_in_the_view() {
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
    assert!(
        checked > 0,
        "no core row claims a GUI reach, so this scan asserts nothing"
    );
}

#[test]
fn no_gui_skipped_core_row_is_constructed_in_the_view() {
    let text = gui_source_text();
    let mut checked = 0_usize;
    let mut present = Vec::new();
    for id in CommandId::ALL {
        if !matches!(id.surfaces().gui, Reach::Skip(_)) {
            continue;
        }
        checked += 1;
        if constructs(&text, id.kind(), *id) {
            present.push(id.wire_name());
        }
    }
    assert!(
        present.is_empty(),
        "these rows say the GUI does NOT reach them, and a view source \
         constructs one: {present:?}. The Skip reason went stale when the \
         caller landed — the package that adds a caller flips the column."
    );
    assert!(
        checked > 0,
        "no core row declares a GUI skip, so this scan asserts nothing"
    );
}

/// The locator finds the needle it is built around.
///
/// A scan whose needle stopped matching would pass both tests above on an
/// empty result set. This pins one row of each kind.
#[test]
fn the_p1_locator_finds_a_known_construction() {
    let text = gui_source_text();
    assert!(
        constructs(&text, CommandKind::Command, CommandId::AdoptResult),
        "the compute drain constructs Command::AdoptResult; if the \
         locator no longer finds it, the whole P1 idiom has drifted"
    );
    assert!(
        constructs(&text, CommandKind::Query, CommandId::ToolpathCycleTime),
        "the readiness panel constructs Query::ToolpathCycleTime"
    );
    assert!(
        !constructs(&text, CommandKind::Command, CommandId::SetStockSource),
        "SetStockSource is reached by the wire alone, so the GUI-only \
         source set must NOT contain it — if it does, the MCP exclusion \
         list has drifted and the converse test is vacuous"
    );
}

// ── P2 — the view registry's own columns ─────────────────────────────

/// WP23 — a `gui: Reached` view row names a production constructor.
///
/// P1 asks this of the CORE rows the view constructs. Nothing asked it of
/// the VIEW registry before WP23. The handler census below accepts a
/// dispatch arm as proof, and a dispatch arm outlives the last caller, so
/// a row could claim a GUI reach that no control made. Two rows sat that
/// way — `SimJumpToOpEnd` and `CancelToolpathGeneration` — each with a
/// row, a payload and a handler the GUI never reached.
#[test]
fn every_gui_reached_view_row_is_constructed_in_the_view() {
    let text = gui_source_text();
    let mut checked = 0_usize;
    let mut missing = Vec::new();
    for id in UiCommandId::ALL {
        if !matches!(id.surfaces().gui, Reach::Reached) {
            continue;
        }
        checked += 1;
        if !view_constructs(&text, *id) {
            missing.push(id.wire_name());
        }
    }
    assert!(
        missing.is_empty(),
        "these view rows claim the GUI reaches them, and no production \
         view file constructs one: {missing:?}. Delete such a row; do not \
         flip its gui column to Skip. A Skip keeps the handler arm, and \
         the arm is what hid the row from the handler census."
    );
    assert!(
        checked > 0,
        "no view row claims a GUI reach, so this scan asserts nothing"
    );
}

/// The view locator finds the needles it is built around.
///
/// The census above passes on an empty result set if the needle stops
/// matching. This pins two known constructions and one known absence. The
/// absence is the load-bearing one: `SetUiView` carries a dispatch match
/// arm and no GUI caller, so a needle that matched an arm would find it.
#[test]
fn the_p2_view_locator_finds_a_known_construction() {
    let text = gui_source_text();
    assert!(
        view_constructs(&text, UiCommandId::SimJumpToOpStart),
        "the simulation operation list constructs \
         UiCommand::SimJumpToOpStart; if the locator no longer finds it, \
         the whole P2 census idiom has drifted"
    );
    assert!(
        view_constructs(&text, UiCommandId::CancelCompute),
        "the viewport overlay constructs UiCommand::CancelCompute"
    );
    assert!(
        !view_constructs(&text, UiCommandId::SetUiView),
        "set_ui_view is reached by the wire alone, so the GUI-only source \
         set must NOT construct it. A hit here means the needle matched a \
         dispatch match arm, or the MCP exclusion list has drifted, and \
         the census above is vacuous either way"
    );
}

#[test]
fn every_view_skip_names_its_reason() {
    let mut checked = 0_usize;
    for id in UiCommandId::ALL {
        let surfaces = id.surfaces();
        for (surface, reach) in [
            ("gui", surfaces.gui),
            ("mcp", surfaces.mcp),
            ("cli", surfaces.cli),
        ] {
            if let Reach::Skip(reason) = reach {
                checked += 1;
                assert!(
                    reason.len() > 10,
                    "{id:?} skips the {surface} surface with the reason \
                     '{reason}', which says nothing a reader can act on"
                );
            }
        }
    }
    assert!(checked > 0, "no view row declares a skip");
}

#[test]
fn no_view_row_reaches_the_batch_command_line() {
    for id in UiCommandId::ALL {
        assert!(
            matches!(id.surfaces().cli, Reach::Skip(_)),
            "{id:?} claims the batch CLI reaches it. The CLI draws no \
             viewport, so a view row reaching it is a classification \
             error and not a missing caller."
        );
    }
    assert!(
        !UiCommandId::ALL.is_empty(),
        "the view registry has no rows"
    );
}

#[test]
fn every_view_row_is_reached_by_some_surface() {
    for id in UiCommandId::ALL {
        let surfaces = id.surfaces();
        assert!(
            matches!(surfaces.gui, Reach::Reached) || matches!(surfaces.mcp, Reach::Reached),
            "{id:?} is reached by neither the GUI nor MCP, so nothing \
             runs it. A row no surface reaches is dead, which is the \
             defect WP13 opened with (AppEvent::RemoveSetup)."
        );
    }
}

#[test]
fn the_view_registry_declares_only_view_kinds() {
    let mut commands = 0_usize;
    let mut queries = 0_usize;
    for id in UiCommandId::ALL {
        match id.kind() {
            CommandKind::UiCommand => commands += 1,
            CommandKind::UiQuery => queries += 1,
            other => panic!("{id:?} declares {other:?}, which belongs to the core registry"),
        }
    }
    assert!(commands > 0 && queries > 0, "one view kind lost every row");
}

// ── P3 — a view row never writes the session ─────────────────────────

/// The two files that hold a `AppEvent::Ui(cmd) => match cmd` block.
const UI_DISPATCH: &[&str] = &["src/controller/events/mod.rs", "src/app/input.rs"];

/// Ruling 10's named exemption, kept as a record rather than a hole.
///
/// `CloseOptimizeModal` and `CloseOptimizeProject` were expected to write
/// the session: the optimizer took it by `std::mem::replace` and
/// something had to put it back. Measured at WP13: the `std::mem::replace`
/// sat in `open_optimize_modal` and the restore sat in the compute drain,
/// so NEITHER close arm wrote the session and both passed the scan
/// unaided. WP14b deleted all three `mem::replace` sites — the two Job
/// rows own a cloned or captured handle and the project rollup takes
/// `session.clone()` — so there is no lend and no restore left to
/// exempt. The names stay here because ruling 10 named them; they buy no
/// exemption today and bought none then.
const P3_NAMED_EXEMPTIONS: &[&str] = &["CloseOptimizeModal", "CloseOptimizeProject"];

/// What a view handler must never contain.
///
/// The pattern is scoped to the SESSION receiver on purpose. A bare
/// `_mut(` would match `toolpath_rt.get_mut(`, `feeds_modal.as_mut()` and
/// `controller.state_mut()`, which are view writes and exactly what a
/// view command is for.
const SESSION_WRITES: &[&str] = &["session.apply(", "session.set_", "state.session ="];

/// Locate the `UiCommand::<Name>` arms of one dispatch file.
///
/// The arms all live inside that file's one `AppEvent::Ui(cmd) => match
/// cmd` block, and an arm runs from its own `UiCommand::` line to the
/// next one at the same indent.
fn ui_arms(relative: &str) -> BTreeMap<String, String> {
    let text = strip_comments(&source(relative));
    let at = text
        .find("AppEvent::Ui(cmd) => match cmd {")
        .unwrap_or_else(|| panic!("{relative} holds no AppEvent::Ui(cmd) dispatch block"));
    let mut arms: BTreeMap<String, String> = BTreeMap::new();
    let mut indent: Option<usize> = None;
    let mut name: Option<String> = None;
    let mut body = String::new();
    for line in text[at..].lines().skip(1) {
        let trimmed = line.trim_start();
        let depth = line.len() - trimmed.len();
        if trimmed.starts_with("UiCommand::") {
            let here = *indent.get_or_insert(depth);
            if depth == here {
                if let Some(previous) = name.take() {
                    arms.insert(previous, std::mem::take(&mut body));
                }
                let ident: String = trimmed
                    .trim_start_matches("UiCommand::")
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                name = Some(ident);
            }
        }
        if indent.is_some_and(|here| depth + 4 == here && trimmed.starts_with("},")) {
            break;
        }
        body.push_str(line);
        body.push('\n');
    }
    if let Some(previous) = name {
        arms.insert(previous, body);
    }
    assert!(
        arms.len() > 10,
        "{relative} yielded only {} view arms — the locator has drifted",
        arms.len()
    );
    arms
}

#[test]
fn no_view_command_handler_writes_the_session() {
    let mut checked = 0_usize;
    let mut offenders = Vec::new();
    for relative in UI_DISPATCH {
        for (name, body) in ui_arms(relative) {
            checked += 1;
            for needle in SESSION_WRITES {
                if body.contains(needle) {
                    offenders.push(format!("{relative}: {name} contains `{needle}`"));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a view command handler writes ProjectSession: {offenders:#?}. A \
         view command writes the VIEW. A row that must write the session \
         belongs in the core registry. Named exemptions: \
         {P3_NAMED_EXEMPTIONS:?}"
    );
    assert!(checked > 50, "only {checked} view arms were scanned");
}

/// Every view COMMAND row has a handler arm, or is answered on the wire.
///
/// Without this, `no_view_command_handler_writes_the_session` would pass
/// over a registry whose arms had all been deleted.
#[test]
fn every_view_command_row_has_a_handler_or_a_tool() {
    let mut located: BTreeSet<String> = BTreeSet::new();
    for relative in UI_DISPATCH {
        located.extend(ui_arms(relative).into_keys());
    }
    let mut orphans = Vec::new();
    for id in UiCommandId::ALL {
        if id.kind() != CommandKind::UiCommand {
            continue;
        }
        let name = format!("{id:?}");
        if !located.contains(&name) && !matches!(id.surfaces().mcp, Reach::Reached) {
            orphans.push(id.wire_name());
        }
    }
    assert!(
        orphans.is_empty(),
        "these view commands have no dispatch arm and no MCP tool, so \
         nothing runs them: {orphans:?}"
    );
}

/// The view READ door holds the session by shared reference.
///
/// P3's scan reaches the view COMMAND arms, which sit in the two dispatch
/// files. The eight view READS sit in `RsCamApp::ui_query` instead, and
/// their guarantee is the RECEIVER: `&self` cannot write the session and
/// cannot write the view. Nothing measured that receiver, so `&mut self`
/// could land here without a test moving.
///
/// This is a source scan because a receiver is not a value a test can
/// read.
#[test]
fn the_view_read_door_holds_the_session_by_shared_reference() {
    let mcp = strip_comments(&source("src/app/mcp.rs"));
    assert!(
        mcp.contains("fn ui_query(&self, query: UiQuery) -> UiQueryAnswer"),
        "RsCamApp::ui_query must take `&self`. That receiver is the whole \
         guarantee that a view read writes neither the session nor the \
         view, and no other test measures it."
    );
}

/// Every view READ row has an arm in the read door.
///
/// The scan above proves the receiver. This one proves the population:
/// a door with no arms would satisfy it.
#[test]
fn every_view_read_row_has_an_arm_in_the_read_door() {
    let mcp = strip_comments(&source("src/app/mcp.rs"));
    let mut checked = 0_usize;
    let mut missing = Vec::new();
    for id in UiCommandId::ALL {
        if id.kind() != CommandKind::UiQuery {
            continue;
        }
        checked += 1;
        if !mcp.contains(&format!("UiQuery::{id:?}(")) {
            missing.push(id.wire_name());
        }
    }
    assert!(
        missing.is_empty(),
        "these view reads have no arm in the read door: {missing:?}"
    );
    assert!(checked > 0, "the view registry declares no read row");
}

// ── P4 — the wire agrees with the view registry ──────────────────────

#[test]
fn every_mcp_reached_view_row_declares_a_tool() {
    let server = strip_comments(&source("src/mcp_server.rs"));
    let snapshot = source("tests/snapshots/mcp_wire_surface.json");
    let mut checked = 0_usize;
    for id in UiCommandId::ALL {
        if !matches!(id.surfaces().mcp, Reach::Reached) {
            continue;
        }
        checked += 1;
        let needle = format!("name = \"{}\"", id.wire_name());
        assert!(
            server.contains(&needle),
            "the view registry says MCP reaches {id:?}, and mcp_server.rs \
             declares no tool with `{needle}`"
        );
        assert!(
            snapshot.contains(&format!("\"{}\"", id.wire_name())),
            "{id:?} claims an MCP reach and its wire name is absent from \
             the WP2a snapshot, so the tool name moved"
        );
    }
    assert!(checked > 0, "no view row declares an MCP reach");
}

#[test]
fn no_mcp_skipped_view_row_claims_a_wire_name() {
    let server = strip_comments(&source("src/mcp_server.rs"));
    let mut checked = 0_usize;
    let mut clashes = Vec::new();
    for id in UiCommandId::ALL {
        if !matches!(id.surfaces().mcp, Reach::Skip(_)) {
            continue;
        }
        checked += 1;
        if server.contains(&format!("name = \"{}\"", id.wire_name())) {
            clashes.push(id.wire_name());
        }
    }
    assert!(
        clashes.is_empty(),
        "these view rows say MCP does not reach them, and a tool carries \
         their wire name: {clashes:?}. A row's internal name must not \
         name a live tool, or the reason column is a lie."
    );
    assert!(checked > 0, "no view row declares an MCP skip");
}

// ── P5 — one name, one row, across both registries ───────────────────

#[test]
fn no_wire_name_appears_in_both_registries() {
    let all = SurfaceId::all();
    let names: BTreeSet<&str> = all.iter().map(|id| id.wire_name()).collect();
    assert_eq!(
        names.len(),
        all.len(),
        "two rows of the command surface share one wire name. The MCP \
         router registers a tool per name, so a collision registers two \
         tools under one name and one of them never runs."
    );
    assert_eq!(
        all.len(),
        CommandId::ALL.len() + UiCommandId::ALL.len(),
        "SurfaceId::all() is not the concatenation of the two registries"
    );
    assert!(
        all.iter().any(|id| matches!(id, SurfaceId::Core(_)))
            && all.iter().any(|id| matches!(id, SurfaceId::Ui(_))),
        "one side of the union contributed no row"
    );
}
