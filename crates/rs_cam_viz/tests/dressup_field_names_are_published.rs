//! CMP-17 — the MCP `set_dressup_config` description names every dressup field.
//!
//! # What the finding was
//!
//! The description was a hand-written prose list. It named 17 fields of 23. It
//! omitted `dogbone_angle`, `lead_in_feed_rate`, `lead_out_feed_rate`,
//! `segment_merge`, `segment_merge_tolerance` and `air_bridge_policy`, and two
//! of those six change emitted motion on every roughing operation. In the
//! other direction it advertised `retract_strategy`, which no code read
//! (G-RETRACTDIAL, deleted by CUT-03).
//!
//! # What this test measures
//!
//! `DressupConfig::FIELD_DEFS` is now the one place the vocabulary is written
//! down. The core test of the same name holds that table against the struct.
//! This test reads the description the embedded server SERVES, through the
//! same `EmbeddedCamServer::into_tool_router()` the wire pin uses, and asserts
//! the description names every row of the table.
//!
//! `#[tool(description = "…")]` takes a string literal, so the description
//! cannot be built from the table at compile time. This test is the join.
//!
//! # NOT MEASURED
//!
//! The wording around the list, whether a described field has a reader, and
//! the `set_dressup_field` tool, which names no field at all by design.
//!
//! # The shape the description must keep
//!
//! The names sit in one run, introduced by `MARKER` and closed by a full stop,
//! separated by `", "`. The test reads that run and compares it to the table as
//! a set, so an omitted field and a stale advertised field both fail. Reword
//! the prose around the run freely; keep the run.

#![cfg(feature = "mcp")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeSet;

use rs_cam_core::compute::config::DressupConfig;
use rs_cam_viz::mcp_server::EmbeddedCamServer;

/// The text that introduces the field run in the description.
const MARKER: &str = "dressup fields: ";

/// The served description of one tool.
fn description_of(tool_name: &str) -> String {
    let router = EmbeddedCamServer::into_tool_router();
    for tool in router.list_all() {
        if tool.name == tool_name {
            return tool
                .description
                .as_ref()
                .unwrap_or_else(|| panic!("`{tool_name}` serves no description"))
                .to_string();
        }
    }
    panic!("the embedded server serves no tool named `{tool_name}`");
}

/// The field names the description advertises.
fn advertised(description: &str) -> BTreeSet<String> {
    let start = description
        .find(MARKER)
        .unwrap_or_else(|| panic!("`set_dressup_config` carries no `{MARKER}` run: {description}"))
        + MARKER.len();
    let rest = &description[start..];
    let end = rest
        .find('.')
        .unwrap_or_else(|| panic!("the field run is not closed by a full stop: {rest}"));
    rest[..end]
        .split(',')
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .collect()
}

#[test]
fn dressup_field_names_are_published() {
    let description = description_of("set_dressup_config");
    let advertised = advertised(&description);
    let published: BTreeSet<String> = DressupConfig::published_field_names()
        .into_iter()
        .map(str::to_owned)
        .collect();

    let missing: Vec<&String> = published.difference(&advertised).collect();
    assert!(
        missing.is_empty(),
        "`set_dressup_config` does not name these dressup fields: {missing:?}\n\
         Add each name to the description literal in \
         `rs_cam_viz/src/mcp_server.rs`."
    );
    let stale: Vec<&String> = advertised.difference(&published).collect();
    assert!(
        stale.is_empty(),
        "`set_dressup_config` advertises names that are not dressup fields: \
         {stale:?}\n\
         The published list lives in `DressupConfig::FIELD_DEFS`."
    );
}
