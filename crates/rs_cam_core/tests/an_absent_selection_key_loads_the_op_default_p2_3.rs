//! Geometry selection P2.3b: a project file with no `selection` key loads
//! the default of THAT op (DESIGN 3.3, decision D6).
//!
//! # The claim
//!
//! `Selection` has no `Default`. Each op config names its own serde
//! default, so:
//!
//! - a Pocket and a Drill with no `selection` key load
//!   `Selection::drawing()`;
//! - a Pin Drill with no key loads `Selection::empty()`, never the drawing
//!   default (that would drill every circle of the drawing in silence);
//! - a saved default selection writes no `selection` key, so a project that
//!   uses only defaults saves as before P2.3b;
//! - a selection that is not the default (a layer rule) saves its key and
//!   loads back equal.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test an_absent_selection_key_loads_the_op_default_p2_3
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::path::PathBuf;

use common::session::toolpath_config;
use common::tools::endmill_tool_config;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{AlignmentPinDrillConfig, DrillConfig, PocketConfig};
use rs_cam_core::selection::{Filter, Rule, Selection, SourceRef};
use rs_cam_core::session::{ProjectSession, ProjectSessionBuilder};

/// One directory for this test file, removed at the end.
fn temp_dir() -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!("rs_cam_selection_key_p2_3_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn layer_rule() -> Selection {
    Selection::from_rules(vec![Rule {
        source: SourceRef::ToolpathModel,
        filter: Filter {
            layers: vec!["CUT".to_owned()],
            ..Filter::default()
        },
    }])
}

/// Four toolpaths: a default Pocket, a default Drill, a default Pin Drill
/// and a Pocket with a layer rule.
fn ops() -> Vec<OperationConfig> {
    vec![
        OperationConfig::Pocket(PocketConfig::default()),
        OperationConfig::Drill(DrillConfig::default()),
        OperationConfig::AlignmentPinDrill(AlignmentPinDrillConfig::default()),
        OperationConfig::Pocket(PocketConfig {
            selection: layer_rule(),
            ..PocketConfig::default()
        }),
    ]
}

fn session(ops: Vec<OperationConfig>) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new();
    let tool_idx = builder.add_tool(endmill_tool_config(3.0));
    let tool_id = builder.tools()[tool_idx].id.0;
    for (i, op) in ops.into_iter().enumerate() {
        let _ = builder
            .add_toolpath(0, toolpath_config(&format!("op {i}"), op, tool_id, 0))
            .unwrap();
    }
    builder.build()
}

/// The lines of a saved project that name a selection.
fn selection_lines(text: &str) -> Vec<&str> {
    text.lines().filter(|l| l.contains("selection")).collect()
}

#[test]
fn an_absent_selection_key_loads_the_op_default() {
    let dir = temp_dir();

    // The three defaults write no key, so they load from a file with no
    // `selection` key.
    let defaults = dir.join("p2_3_defaults.toml");
    let mut ops = ops();
    let rule_op = ops.pop().unwrap();
    session(ops).save(&defaults).unwrap();
    let text = std::fs::read_to_string(&defaults).unwrap();
    assert!(
        selection_lines(&text).is_empty(),
        "a default selection wrote a key: {:?}",
        selection_lines(&text)
    );
    let loaded = ProjectSession::load(&defaults).unwrap();
    let want = [
        Selection::drawing(),
        Selection::drawing(),
        Selection::empty(),
    ];
    assert_eq!(loaded.toolpath_configs().len(), want.len());
    for (cfg, want) in loaded.toolpath_configs().iter().zip(want.iter()) {
        assert_eq!(
            cfg.operation.selection(),
            Some(want),
            "{} ({}): a file with no key loaded the wrong selection",
            cfg.name,
            cfg.operation.label()
        );
    }
    // The Pin Drill default is not the drawing default (decision D6).
    assert_ne!(Selection::empty(), Selection::drawing());

    // A selection that is not the default saves its key and loads back.
    let rule = dir.join("p2_3_rule.toml");
    session(vec![rule_op]).save(&rule).unwrap();
    let text = std::fs::read_to_string(&rule).unwrap();
    assert!(
        !selection_lines(&text).is_empty() && text.contains("CUT"),
        "the layer rule was not saved:\n{text}"
    );
    let loaded = ProjectSession::load(&rule).unwrap();
    assert_eq!(
        loaded.toolpath_configs()[0].operation.selection(),
        Some(&layer_rule())
    );

    std::fs::remove_dir_all(&dir).unwrap();
}
