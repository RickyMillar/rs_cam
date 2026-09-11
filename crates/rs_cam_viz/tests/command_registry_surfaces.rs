//! WP1 — a command the registry says MCP reaches has an MCP tool.
//!
//! Programme: `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md`
//! §4 WP1.
//!
//! The registry's `Surfaces` column is a claim about the shipped
//! surfaces. `Reach::Reached` on the `mcp` field says an agent can call
//! the command. Nothing in the type system checks that claim: the MCP
//! tool list is a set of `#[tool(name = "…")]` attributes in
//! `src/mcp_server.rs`, and an attribute is not a value this test can
//! read.
//!
//! So this test reads the SOURCE, in the idiom of
//! `crates/rs_cam_viz/tests/ui_string_hygiene.rs:78-89`. For every row
//! whose `mcp` reach is `Reached`, the source must carry
//! `name = "<wire_name>"`. A non-vacuity guard fails the test when no
//! row is checked, because an empty population passes and looks healthy.
//!
//! NOT MEASURED here: whether the MCP arm behind that tool name calls
//! `ProjectSession::apply`. `crates/rs_cam_core/tests/command_registry_completeness.rs`
//! measures the door. WP4 added the `McpRequestKind::Core` wrapper that
//! makes the link typed, and `mcp_core_arm_describes_every_row.rs`
//! measures it: every `Reached` row has a `CoreRequest` variant whose
//! `id()` names that row, and one dispatch arm applies them all.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::session::{CommandId, Reach};

/// Read one source file, relative to this crate's manifest.
fn source(relative: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    assert!(
        path.is_file(),
        "scanned path {} no longer exists",
        path.display()
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Every command the registry says MCP reaches is a declared MCP tool.
#[test]
fn every_mcp_reached_command_declares_a_tool() {
    let server = source("src/mcp_server.rs");
    let mut checked = 0_usize;
    for id in CommandId::ALL {
        if !matches!(id.surfaces().mcp, Reach::Reached) {
            continue;
        }
        let needle = format!("name = \"{}\"", id.wire_name());
        assert!(
            server.contains(&needle),
            "the registry says MCP reaches {id:?}, but mcp_server.rs \
             declares no tool with `{needle}`"
        );
        checked += 1;
    }
    assert!(
        checked > 0,
        "no registry row declares an MCP reach, so this scan asserts \
         nothing. Either the registry lost its rows or the `mcp` column \
         stopped saying Reached."
    );
}

/// The scan finds the needle it is built around.
///
/// A locator that matched nothing would let the test above pass on an
/// empty string. This pins the one row WP1 ships.
#[test]
fn the_scan_finds_the_set_toolpath_param_tool() {
    let server = source("src/mcp_server.rs");
    assert!(
        server.contains("name = \"set_toolpath_param\""),
        "mcp_server.rs must declare the set_toolpath_param tool. If the \
         attribute's spelling changed, this scan's whole idiom has \
         drifted."
    );
    assert_eq!(
        CommandId::SetToolpathParam.wire_name(),
        "set_toolpath_param",
        "the registry's wire name IS the MCP tool name"
    );
}

/// A skipped surface says why, in words an operator can act on.
#[test]
fn a_skipped_surface_names_its_reason() {
    for id in CommandId::ALL {
        let surfaces = id.surfaces();
        for (surface, reach) in [
            ("gui", surfaces.gui),
            ("mcp", surfaces.mcp),
            ("cli", surfaces.cli),
        ] {
            if let Reach::Skip(reason) = reach {
                assert!(
                    reason.len() > 10,
                    "{id:?} skips the {surface} surface with the reason \
                     '{reason}', which says nothing a reader can act on"
                );
            }
        }
    }
}
