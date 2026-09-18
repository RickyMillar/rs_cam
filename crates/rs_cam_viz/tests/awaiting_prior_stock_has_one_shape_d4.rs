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
//! - the NET half reads each producer and refuses the key literal.
//!
//! # NOT MEASURED
//!
//! The wording of `message` (its own sentry owns that), and the shape of the
//! `generate_all` reply's `awaiting_prior_stock` rows. That reply's carrier
//! (`controller::generate_all::GenerateAllSummary::blocked`) is still a
//! `(toolpath id, message)` pair; widening it is a handoff, recorded in the
//! W5 report.

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
const PRODUCERS: [(&str, &str); 4] = [
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
    ("crates/rs_cam_viz/src/mcp_bridge.rs", "struct BlockedRow"),
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
