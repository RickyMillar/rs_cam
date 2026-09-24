//! D4 — `awaiting_prior_stock` has ONE shape on every surface.
//!
//! Package W5 of `planning/gen_sim_rest_ux_2026-09-18/`, item (a). Six sites
//! reported the same block, each from its own `serde_json::json!` literal,
//! and one of them (the `generate_toolpath` failure reply) dropped `message`.
//! An agent therefore had to know which call it made before it could read the
//! answer. The fix is to serialise
//! [`rs_cam_core::compute::config::AwaitingPriorStock`] itself, and to render
//! an ARRAY row through one `BlockedRow` type.
//!
//! # Two halves, because neither half sees the other's failure
//!
//! Serialisation alone cannot see a seventh hand-built literal appear
//! somewhere else. A source scan alone cannot see a field added to the struct
//! and then dropped by a hand-written `Serialize` impl. So:
//!
//! - the ARITY half builds the struct, serialises it and counts the keys;
//! - the NET half reads each producer and refuses the key literal;
//! - the REPLY half builds a finished `generate_all` summary and reads the
//!   row the wire renders, because that reply's carrier is the one that used
//!   to be a `(toolpath id, message)` pair.
//!
//! # NOT MEASURED
//!
//! The wording of `message`. Its own sentry owns that.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use rs_cam_core::compute::config::AwaitingPriorStock;

/// The three keys the wire carries, in the struct's own order.
const KEYS: [&str; 3] = ["blocking_toolpath_id", "blocking_toolpath_index", "message"];

#[test]
fn the_serialised_block_carries_exactly_three_keys() {
    let block = AwaitingPriorStock {
        blocking_toolpath_id: Some(rs_cam_core::ToolpathId(7)),
        blocking_toolpath_index: Some(2),
        message: "waiting on 'Rough' to generate and be simulated".to_owned(),
    };
    let value = serde_json::to_value(&block).expect("AwaitingPriorStock serialises");
    let obj = value.as_object().expect("it serialises as an object");

    for key in KEYS {
        assert!(obj.contains_key(key), "the wire lost `{key}`: {obj:?}");
    }
    // A field added to the struct reaches six surfaces at once. Count the
    // keys so the addition is a decision somebody makes here, not a silent
    // widening of four replies.
    assert_eq!(
        obj.len(),
        KEYS.len(),
        "serialize_struct arity must match the field count: {obj:?}"
    );

    // `ToolpathId` is `#[serde(transparent)]`, so the id is a bare integer
    // and not `{"0": 7}`. Every pre-W5 hand-built literal emitted it that
    // way, and this is the assertion that keeps the shape byte-identical.
    assert_eq!(obj["blocking_toolpath_id"], serde_json::json!(7));
    assert_eq!(obj["blocking_toolpath_index"], serde_json::json!(2));
}

/// Every producer of the key, and the anchor that proves the scan read a
/// file that still has something to say.
///
/// A scan that reads an empty or renamed file passes and looks healthy, so
/// each row names a literal the file must still contain.
const PRODUCERS: [(&str, &str); 6] = [
    (
        "crates/rs_cam_viz/src/app/mcp/project.rs",
        "awaiting_prior_stock",
    ),
    (
        "crates/rs_cam_viz/src/app/mcp/simulation.rs",
        "awaiting_prior_stock",
    ),
    (
        "crates/rs_cam_viz/src/controller/events/compute.rs",
        "awaiting_prior_stock",
    ),
    (
        "crates/rs_cam_viz/src/controller/generate_all.rs",
        "struct BlockedRow",
    ),
    (
        "crates/rs_cam_viz/src/mcp_bridge.rs",
        "awaiting_prior_stock",
    ),
    (
        "crates/rs_cam_cli/src/project.rs",
        "awaiting_prior_stock: Vec<",
    ),
];

/// Drop `//` line comments, so a comment may name the key in prose.
fn strip_line_comments(src: &str) -> String {
    src.lines()
        .map(|line| match line.find("//") {
            Some(at) => line.get(..at).unwrap_or(""),
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn no_surface_hand_builds_the_block() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .to_path_buf();

    for (rel, anchor) in PRODUCERS {
        let path = repo.join(rel);
        let raw = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("D4 sentry needs retargeting: {rel}: {e}"));
        assert!(
            raw.len() > 400,
            "{rel} is too small to be the producer this sentry reads"
        );
        assert!(
            raw.contains(anchor),
            "{rel} no longer contains {anchor:?}; retarget this sentry"
        );

        let src = strip_line_comments(&raw);
        // A Rust field name is not a quoted string, so this fires only on a
        // `json!` key literal.
        assert!(
            !src.contains("\"blocking_toolpath_id\""),
            "{rel} hand-builds the awaiting_prior_stock shape. Serialise \
             AwaitingPriorStock instead, so the surfaces cannot drift (D4)."
        );
    }
}

/// The `generate_all` reply's array row, measured.
///
/// W1 tail. The carrier was `Vec<(toolpath id, message)>`, so this reply
/// named the waiting operation by id alone and dropped the blocker entirely.
/// It now carries `BlockedRow`, the same type every other array site
/// renders, and the row is the three identifying keys plus the flattened
/// block.
#[test]
fn the_generate_all_reply_renders_the_one_row_shape() {
    use rs_cam_viz::mcp_bridge::{BlockedRow, GenerateAllSummary, build_generate_all_response};

    let summary = GenerateAllSummary {
        generated: 1,
        failed: 0,
        errors: Vec::new(),
        blocked: vec![BlockedRow {
            toolpath_id: rs_cam_core::ToolpathId(4),
            toolpath_index: 3,
            name: "Rest B".to_owned(),
            block: AwaitingPriorStock {
                blocking_toolpath_id: Some(rs_cam_core::ToolpathId(7)),
                blocking_toolpath_index: Some(2),
                message: "waiting on 'Rough' to generate and be simulated".to_owned(),
            },
        }],
        steps: 4,
        simulations: 1,
        resolution_report: None,
        loop_error: None,
    };

    let reply: serde_json::Value =
        serde_json::from_str(&build_generate_all_response(&summary)).expect("the reply is JSON");
    let rows = reply
        .get("awaiting_prior_stock")
        .and_then(serde_json::Value::as_array)
        .expect("awaiting_prior_stock is an array");
    assert_eq!(rows.len(), 1, "reply: {reply}");
    let row = rows.first().expect("one row").as_object().expect("object");

    for key in KEYS {
        assert!(
            row.contains_key(key),
            "the array row lost the flattened `{key}`: {row:?}"
        );
    }
    for key in ["toolpath_id", "toolpath_index", "name"] {
        assert!(
            row.contains_key(key),
            "the array row must name the WAITING operation too: {row:?}"
        );
    }
    assert_eq!(
        row.len(),
        KEYS.len() + 3,
        "one row is the block plus three identifying keys, and nothing else: {row:?}"
    );
    assert_eq!(row.get("toolpath_id"), Some(&serde_json::json!(4)));
    assert_eq!(row.get("toolpath_index"), Some(&serde_json::json!(3)));
    assert_eq!(row.get("blocking_toolpath_id"), Some(&serde_json::json!(7)));
}
