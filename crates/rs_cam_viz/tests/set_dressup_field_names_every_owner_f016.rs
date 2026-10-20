//! F016 — the `set_dressup_field` description names every owner key.
//!
//! # What the finding was
//!
//! `set_dressup_field` refuses a value field while its dressup is off, and
//! the description tells the caller which enable key to set first. That
//! list was typed by hand beside a `&str` match with a wildcard arm, so a
//! dressup missing from either one went unseen.
//!
//! # What this test measures
//!
//! The owners now live in one column, `DressupFieldDef::owner`. This test
//! reads the description the embedded server SERVES and asserts that the
//! run of enable keys in it equals the set of owners in the table.
//! `#[tool(description = "…")]` takes a string literal, so the description
//! cannot be built from the table. This test is the join.
//!
//! # The shape the description must keep
//!
//! The keys sit in one run after `MARKER`, closed by `)`, separated by
//! `", "`.

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

/// The text that introduces the enable-key run in the description.
const MARKER: &str = "set the enable key (";

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

#[test]
fn set_dressup_field_names_every_owner_key() {
    let description = description_of("set_dressup_field");
    let start = description
        .find(MARKER)
        .unwrap_or_else(|| panic!("no `{MARKER}` run in: {description}"))
        + MARKER.len();
    let rest = &description[start..];
    let end = rest
        .find(')')
        .unwrap_or_else(|| panic!("the key run is not closed by `)`: {rest}"));
    let advertised: BTreeSet<&str> = rest[..end]
        .split(',')
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .collect();
    let owners: BTreeSet<&str> = DressupConfig::FIELD_DEFS
        .iter()
        .filter_map(|def| def.owner)
        .map(|owner| owner.as_key())
        .collect();
    // Non-vacuity: the table names owners, and the run is not empty.
    assert!(owners.len() >= 5, "the table names only {owners:?}");
    assert_eq!(
        advertised, owners,
        "the `set_dressup_field` description lists {advertised:?}, but the \
         owners in `DressupConfig::FIELD_DEFS` are {owners:?}. Edit the \
         description literal in `rs_cam_viz/src/mcp_server.rs`."
    );
}
