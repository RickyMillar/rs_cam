//! WP2a — the MCP wire surface is pinned to a checked-in snapshot.
//!
//! Programme: `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md`
//! §4 WP2a. The goal is one sentence: nothing disappears from the wire, and
//! nothing changes shape, while the command surface migrates.
//!
//! # What the pin covers
//!
//! The embedded server answers `list_tools` through rmcp's `Router`, and that
//! router reads the tool list from the router
//! `EmbeddedCamServer::into_tool_router()` builds (`src/app.rs`). This test
//! calls the same constructor, so it reads the shipped answer and not a copy of
//! it. The snapshot holds one row per tool: the tool's `name` and its
//! `inputSchema`. The server registered 78 tools on 2026-09-11. That number is
//! context, not a bar — the count assertion compares the wire against the
//! snapshot.
//!
//! A source scan for `name = "…"` cannot see a field added, removed, renamed or
//! moved between required and `Option`. It also cannot see a tool whose literal
//! survives after its method leaves the `#[tool_router]` impl. The derived
//! schemas carry every parameter struct in `rs_cam_mcp`, so this snapshot covers
//! the structs the migration moves.
//!
//! # Why `description` is not in the snapshot
//!
//! The description is prose. A prose edit moves it, and that churn hides a real
//! change of shape. `mcp_rebind_surface_g_mcprebind.rs` and
//! `mcp_authoring_surface.rs` pin the descriptions that carry a contract.
//!
//! # Why the key order is canonical
//!
//! An input schema is a `serde_json::Map`. The `preserve_order` feature changes
//! that map's store from a `BTreeMap` to an `IndexMap`, so the key order becomes
//! the insertion order. `rs_cam_cli` turns the feature on, and cargo unifies the
//! features over a workspace build. So `cargo test -p rs_cam_viz` and
//! `cargo test --workspace` do not agree about the key order. This test sorts
//! every object key on both sides, and it compares parsed values, never text.
//!
//! # How to move the snapshot
//!
//! A rename of a parameter struct moves the snapshot ON PURPOSE. schemars writes
//! the struct name into the schema's `title`, so an agent sees the rename. Set
//! `RS_CAM_UPDATE_WIRE_SNAPSHOT=1`, run this test, read the diff, then commit the
//! file.
//!
//! # NOT MEASURED
//!
//! The description text, the output shape of a tool, and the behaviour behind a
//! tool name. The duplicate-name guard below is weak by construction: the router
//! keys its tools by name in a `HashMap`, so a duplicated `#[tool(name = "…")]`
//! replaces its twin, and only the count line sees it. The same guard on the
//! snapshot's own names is the strong one, because a person can edit that file.

#![cfg(feature = "mcp")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::{BTreeMap, BTreeSet};

use rs_cam_core::session::{CommandId, Reach};
use rs_cam_viz::mcp_server::EmbeddedCamServer;

/// The snapshot, relative to this crate's manifest.
const SNAPSHOT: &str = "tests/snapshots/mcp_wire_surface.json";

/// The variable that rewrites the snapshot.
const UPDATE_VAR: &str = "RS_CAM_UPDATE_WIRE_SNAPSHOT";

/// Rebuild one object with its keys in sorted order.
///
/// The module doc gives the reason. A nested object is sorted too.
fn canonical_object(map: &serde_json::Map<String, serde_json::Value>) -> serde_json::Value {
    let mut sorted: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    for (key, entry) in map {
        sorted.insert(key.clone(), canonical(entry));
    }
    serde_json::Value::Object(sorted.into_iter().collect())
}

/// Rebuild one value with every object key in sorted order.
///
/// An array keeps its order, because the order of an array is data.
fn canonical(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => canonical_object(map),
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(canonical).collect())
        }
        other => other.clone(),
    }
}

/// Build one snapshot row from a tool's wire name and its input schema.
fn row(name: String, schema: serde_json::Value) -> serde_json::Value {
    let mut fields: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    fields.insert("name".to_owned(), serde_json::Value::String(name));
    fields.insert("inputSchema".to_owned(), schema);
    serde_json::Value::Object(fields.into_iter().collect())
}

/// The tool list the embedded server serves, in canonical form.
///
/// The router sorts its answer by name, so the rows arrive in a stable order.
fn wire_surface() -> Vec<serde_json::Value> {
    let router = EmbeddedCamServer::into_tool_router();
    let mut rows = Vec::new();
    for tool in router.list_all() {
        let schema = canonical_object(&tool.input_schema);
        rows.push(row(tool.name.into_owned(), schema));
    }
    rows
}

/// The snapshot's path, anchored on this crate's manifest.
fn snapshot_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(SNAPSHOT)
}

/// The `name` field of one row.
fn name_of(entry: &serde_json::Value) -> String {
    entry
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| panic!("a wire row carries no `name` field: {entry}"))
        .to_owned()
}

/// The `inputSchema` field of the row that carries this name.
///
/// `None` means the row is absent, or the row carries no schema. A hand-edited
/// snapshot produces the second shape, so the report says which one it found.
fn schema_of<'a>(rows: &'a [serde_json::Value], name: &str) -> Option<&'a serde_json::Value> {
    rows.iter()
        .find(|entry| name_of(entry) == name)
        .and_then(|entry| entry.get("inputSchema"))
}

/// Pretty-print one input schema, or say that the row carries none.
fn pretty(schema: Option<&serde_json::Value>) -> String {
    match schema {
        Some(value) => serde_json::to_string_pretty(value)
            .unwrap_or_else(|e| format!("<cannot serialise this schema: {e}>")),
        None => "<the row carries no `inputSchema` field>".to_owned(),
    }
}

/// The names in `left` that `right` does not carry.
fn only_in(left: &BTreeSet<String>, right: &BTreeSet<String>) -> Vec<String> {
    left.difference(right).cloned().collect()
}

/// Read the snapshot's rows, in canonical form.
///
/// The verifier writes the file. Until then, say so in words a reader can act
/// on.
fn snapshot_rows() -> Vec<serde_json::Value> {
    let path = snapshot_path();
    if !path.is_file() {
        panic!(
            "the MCP wire snapshot {} does not exist. Write it with \
             `{UPDATE_VAR}=1 cargo test -p rs_cam_viz --test \
             mcp_wire_surface_pin`, read the diff, then commit the file.",
            path.display()
        );
    }
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let parsed: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
    let serde_json::Value::Array(rows) = parsed else {
        panic!(
            "the MCP wire snapshot {} must hold a JSON array of rows",
            path.display()
        );
    };
    rows.iter().map(canonical).collect()
}

/// The wire surface equals the checked-in snapshot.
#[test]
fn wire_surface_matches_the_checked_in_snapshot() {
    let path = snapshot_path();
    let actual = wire_surface();
    let actual_names: BTreeSet<String> = actual.iter().map(name_of).collect();
    assert_eq!(
        actual_names.len(),
        actual.len(),
        "two MCP tools answer to one wire name. The router keys its tools \
         by name, so one registration replaces the other."
    );

    if std::env::var_os(UPDATE_VAR).is_some() {
        let Some(parent) = path.parent() else {
            panic!("{} has no parent directory", path.display());
        };
        std::fs::create_dir_all(parent)
            .unwrap_or_else(|e| panic!("create {}: {e}", parent.display()));
        let mut text = serde_json::to_string_pretty(&actual)
            .unwrap_or_else(|e| panic!("serialise the wire surface: {e}"));
        text.push('\n');
        std::fs::write(&path, text).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
        panic!(
            "wrote the MCP wire snapshot {}. Read the diff. Re-run without \
             {UPDATE_VAR} to check it.",
            path.display()
        );
    }

    let expected = snapshot_rows();
    let expected_names: BTreeSet<String> = expected.iter().map(name_of).collect();
    assert_eq!(
        expected_names.len(),
        expected.len(),
        "the snapshot {} lists one wire name twice, so its name set is \
         smaller than its row count. Rewrite it with {UPDATE_VAR}.",
        path.display()
    );

    if actual == expected {
        return;
    }

    let mut report = String::new();
    report.push_str("the MCP wire surface moved.\n\n");
    report.push_str(&format!(
        "tool count: {} on the wire, {} in {}\n",
        actual.len(),
        expected.len(),
        path.display()
    ));
    let added = only_in(&actual_names, &expected_names);
    let removed = only_in(&expected_names, &actual_names);
    report.push_str(&format!("added to the wire: {added:?}\n"));
    report.push_str(&format!("removed from the wire: {removed:?}\n"));

    for name in &actual_names {
        if !expected_names.contains(name) {
            continue;
        }
        let mine = schema_of(&actual, name);
        let theirs = schema_of(&expected, name);
        if mine == theirs {
            continue;
        }
        report.push_str(&format!("\nthe input schema of `{name}` differs.\n"));
        report.push_str(&format!("on the wire:\n{}\n", pretty(mine)));
        report.push_str(&format!("in the snapshot:\n{}\n", pretty(theirs)));
        break;
    }

    report.push_str(&format!(
        "\nRewrite the snapshot with {UPDATE_VAR}=1 when the move is intended.\n"
    ));
    panic!("{report}");
}

/// Every command the registry says MCP reaches carries a tool on the wire.
#[test]
fn every_mcp_reached_command_row_is_on_the_wire() {
    assert!(
        std::env::var_os(UPDATE_VAR).is_none(),
        "{UPDATE_VAR} is set, so the other arm rewrites the snapshot this \
         arm reads. Re-run without the variable."
    );
    let names: BTreeSet<String> = snapshot_rows().iter().map(name_of).collect();
    let mut checked = 0_usize;
    for id in CommandId::ALL {
        if !matches!(id.surfaces().mcp, Reach::Reached) {
            continue;
        }
        assert!(
            names.contains(id.wire_name()),
            "the registry says MCP reaches {id:?} as `{}`, but the wire \
             snapshot lists no tool with that name",
            id.wire_name()
        );
        checked += 1;
    }
    assert!(
        checked > 0,
        "no registry row declares an MCP reach, so this arm asserts \
         nothing. An empty population passes and looks healthy."
    );
}
