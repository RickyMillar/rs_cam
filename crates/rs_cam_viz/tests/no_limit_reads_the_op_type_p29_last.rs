//! Feeds contract P29-last
//! (`planning/feeds_contract_2026-11-03/phases/P29-11_DESIGN.md` section 5,
//! lead rulings L3 and L7): no display surface reads the op type.
//!
//! The viz twin of `crates/rs_cam_core/tests/no_limit_reads_the_op_type_p29_last.rs`
//! (that file states the rule and the classes). Guarded: the feeds surfaces
//! (`ui/feeds/`), the simulation surfaces (`ui/sim_*.rs`,
//! `state/simulation.rs`) and the embedded MCP server (`app/mcp/`). The
//! simulation surfaces have zero tolerance: they show a limit and read only
//! the core row. The editor files (the registry, the 3D panels) are op
//! dispatch and are not guarded (lead ruling L3).
//!
//! `cargo test -p rs_cam_viz -q --test no_limit_reads_the_op_type_p29_last`

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure; the shared source scans index their own buffers"
)]

#[path = "../../rs_cam_core/tests/common/source_scan.rs"]
mod source_scan;

#[path = "../../rs_cam_core/tests/common/op_type_scan.rs"]
mod op_type_scan;

use std::path::{Path, PathBuf};

use op_type_scan::{Allow, Class};

/// The guarded files of viz, relative to `src/`.
fn guarded(rel: &str) -> bool {
    rel.starts_with("ui/feeds/")
        || rel.starts_with("app/mcp/")
        || rel == "state/simulation.rs"
        || is_sim_surface(rel)
}

/// The files with zero tolerance: the simulation surfaces.
fn zero_tolerance(rel: &str) -> bool {
    rel == "state/simulation.rs" || is_sim_surface(rel)
}

/// `ui/sim_<name>.rs`, a file directly in `ui/`.
fn is_sim_surface(rel: &str) -> bool {
    rel.strip_prefix("ui/sim_")
        .is_some_and(|rest| !rest.contains('/') && rest.ends_with(".rs"))
}

/// The allow-list as it stands after commits L-1 and L-2 (no X entry: no
/// display file holds a scope read). The list only shrinks.
#[rustfmt::skip]
const ALLOW: &[Allow] = &[
    // W: the op draft of the MCP add doors.
    Allow { file: "app/mcp/commands.rs", function: "core_add_toolpath", class: Class::W },
    Allow { file: "app/mcp/generation.rs", function: "mcp_add_toolpath_via_gui", class: Class::W },
    // D: the registry `Capabilities` (S8, the scallop override of the card).
    Allow { file: "ui/feeds/compare.rs", function: "current_values_for_operation", class: Class::D },
    // T: a label or a report field.
    Allow { file: "app/mcp/commands.rs", function: "core_toast_for", class: Class::T },
    Allow { file: "app/mcp/commands/selection.rs", function: "selection_status_reply", class: Class::T },
    Allow { file: "app/mcp/commands/selection.rs", function: "tab_layout_reply", class: Class::T },
    Allow { file: "app/mcp/diagnostics.rs", function: "mcp_narrate_toolpath", class: Class::T },
    Allow { file: "ui/feeds/vendor.rs", function: "row_cells", class: Class::T },
];

/// The sum of the X counts: none in viz. It only goes down.
const X_TOTAL: usize = 0;

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

#[test]
fn no_surface_reads_the_op_type_p29_last() {
    // Review 2026-11-15 F9 (3): the helpers that return an op type, a
    // family or a role live in core too.
    let core_src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../rs_cam_core/src");
    let table = op_type_scan::counts_with_helpers(&src_root(), &[&src_root(), &core_src], guarded);
    if let Ok(out) = std::env::var("P29_GUARD_TABLE") {
        let _ = std::fs::write(out, op_type_scan::table_text(&table));
    }
    let files = source_scan::rust_files(&src_root())
        .iter()
        .filter(|p| {
            p.strip_prefix(src_root())
                .is_ok_and(|r| guarded(&r.to_string_lossy().replace('\\', "/")))
        })
        .count();
    assert!(
        files > 3,
        "non-vacuity: the scan reads {files} guarded files"
    );
    let failures = op_type_scan::failures(&table, ALLOW, zero_tolerance, X_TOTAL);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Non-vacuity: a planted read in a fixture string is found.
#[test]
fn the_scan_finds_a_planted_read_p29_last() {
    let planted =
        "fn label(op: &OperationConfig) -> bool {\n    op.op_type() == OperationType::Drill\n}\n";
    let hits = op_type_scan::hits(planted);
    if hits.len() != 2 || hits.iter().any(|h| h.function != "label") {
        panic!("the scan finds two reads in `label`: {hits:?}");
    }
}
