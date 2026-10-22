//! Tests for the `ProjectSession` mutation methods.
//!
//! Moved out of `session/mutation.rs` by P4.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use crate::geo::{BoundingBox3, P3};
use crate::ids::ToolpathId;
use crate::session::{
    AddSetupArgs, AdoptResultArgs, Command, RemoveSetupArgs, ReplaceToolpathConfigArgs,
};
use std::sync::Arc;

// The parent held these names before the split. Each one now sits in the
// child that uses it, so the test module names it directly.
use crate::compute::stock_config::StockConfig;
use crate::compute::tool_config::{ToolConfig, ToolId};
use crate::compute::transform::FaceUp;
use crate::session::{Effects, ProjectSession, SessionError, ToolpathConfig};
use std::collections::BTreeSet;

use crate::compute::catalog::OperationConfig;
use crate::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig};
use crate::compute::obstacle::{Obstacle, ObstacleGeometry};
use crate::compute::operation_configs::{PencilConfig, PocketConfig};
use crate::compute::toolpath_stats::ToolpathStats;
use crate::gcode::CoolantMode;
use crate::ids::ObstacleId;
use crate::session::ToolpathComputeResult;
use crate::trace::debug_trace::ToolpathDebugOptions;

fn make_session() -> ProjectSession {
    ProjectSession::new_empty()
}

fn make_tc(tool_id: usize, model_id: usize) -> ToolpathConfig {
    ToolpathConfig {
        shoe: Default::default(),
        id: ToolpathId::from_raw(0),
        name: "test".to_owned(),
        enabled: true,
        operation: OperationConfig::Pocket(PocketConfig::default()),
        dressups: DressupConfig::default(),
        heights: HeightsConfig::default(),
        tool_id: crate::ids::ToolId::from_raw(tool_id),
        model_id: crate::ids::ModelId::from_raw(model_id),
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: crate::session::StockSource::Fresh,
        coolant: CoolantMode::Off,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: crate::feeds::FeedsProvenance::default(),
        rest_analysis: crate::compute::config::RestAnalysisConfig::default(),
        planner_origin: None,
    }
}

fn make_tool() -> ToolConfig {
    ToolConfig::new_default(
        ToolId::from_raw(0),
        crate::compute::tool_config::ToolType::EndMill,
    )
}

fn fake_result() -> ToolpathComputeResult {
    ToolpathComputeResult {
        op_data: crate::ops::drill_op::OpData::Toolpath(Arc::new(
            crate::trace::toolpath_spans::AnnotatedToolpath::new(crate::toolpath::Toolpath::new()),
        )),
        stats: ToolpathStats::default(),
        debug_trace: None,
        semantic_trace: None,
        provenance: crate::session::ResultProvenance::Dressed,
    }
}

// ── Toolpath CRUD ────────────────────────────────────────────

#[test]
fn add_toolpath_assigns_id_and_updates_setup() {
    let mut s = make_session();
    let tool_idx = s
        .add_tool(make_tool())
        .unwrap()
        .created
        .expect("add_tool reports the new tool index");
    assert_eq!(tool_idx, 0);

    let tc = make_tc(s.tools()[0].id.raw(), 0);
    let idx = s
        .add_toolpath(0, tc)
        .unwrap()
        .created
        .expect("add_toolpath reports the new toolpath index");
    assert_eq!(idx, 0);
    assert_eq!(s.toolpath_configs()[0].id, ToolpathId::from_raw(0));
    assert_eq!(s.list_setups()[0].toolpath_indices, vec![0]);

    let tc2 = make_tc(s.tools()[0].id.raw(), 0);
    let idx2 = s
        .add_toolpath(0, tc2)
        .unwrap()
        .created
        .expect("add_toolpath reports the new toolpath index");
    assert_eq!(idx2, 1);
    assert_eq!(s.toolpath_configs()[1].id, ToolpathId::from_raw(1));
    assert_eq!(s.list_setups()[0].toolpath_indices, vec![0, 1]);
}

/// Geometry selection P6.7 (L-17): `AddBoreFromDrill` adds one Bore
/// directly after the Drill, with a copy of the Drill selection, its tool
/// and model, and the floor at the Drill depth below the stock top. It
/// refuses a toolpath that is not a Drill and a stale index, and a refusal
/// writes nothing.
///
/// Red proof: write `Selection::drawing()` in place of the copy; the
/// selection assertion fails.
#[test]
fn add_bore_from_drill_copies_the_drill_p6_7() {
    use crate::compute::floor::Floor;
    use crate::compute::operation_configs::DrillConfig;
    use crate::selection::{Filter, Rule, Selection, SourceRef};
    use crate::session::AddBoreFromDrillArgs;

    let mut s = make_session();
    let _ = s.add_tool(make_tool()).unwrap();
    let tool = s.tools()[0].id.raw();
    // A Pocket first, so the Drill is not the first op of the setup.
    let _ = s.add_toolpath(0, make_tc(tool, 0)).unwrap();
    let selection = Selection::from_rules(vec![Rule {
        source: SourceRef::ToolpathModel,
        filter: Filter {
            layers: vec!["HOLES".into()],
            ..Filter::default()
        },
    }]);
    let mut drill = make_tc(tool, 0);
    drill.name = "Holes".to_owned();
    drill.operation = OperationConfig::Drill(DrillConfig {
        depth: 7.5,
        spindle_rpm: Some(12_000),
        selection: selection.clone(),
        ..DrillConfig::default()
    });
    let drill_index = s.add_toolpath(0, drill).unwrap().created.unwrap();
    // A Pocket after the Drill: the Bore goes between the two.
    let after = s
        .add_toolpath(0, make_tc(tool, 0))
        .unwrap()
        .created
        .unwrap();
    let drill_id = s.toolpath_configs()[drill_index].id;

    let effects = s
        .apply(Command::AddBoreFromDrill(AddBoreFromDrillArgs {
            index: drill_index,
            drill_id,
        }))
        .unwrap();
    let bore_index = effects.created.expect("the command reports the Bore");
    assert_eq!(s.toolpath_configs().len(), 4);
    assert_eq!(
        s.list_setups()[0].toolpath_indices,
        vec![0, drill_index, bore_index, after],
        "the Bore sits directly after the Drill"
    );
    let bore = &s.toolpath_configs()[bore_index];
    let OperationConfig::Bore(cfg) = &bore.operation else {
        panic!("the new op is a Bore: {:?}", bore.operation.label());
    };
    assert_eq!(cfg.selection, selection, "the Bore copies the selection");
    assert_eq!(cfg.floor, Floor::StockTop { depth: 7.5 });
    assert_eq!(cfg.spindle_rpm, Some(12_000));
    assert_eq!(bore.tool_id, s.toolpath_configs()[drill_index].tool_id);
    assert_eq!(bore.model_id, s.toolpath_configs()[drill_index].model_id);
    assert_eq!(bore.name, "Holes bore");

    // A Pocket is not a Drill: the command refuses and writes nothing.
    let pocket_id = s.toolpath_configs()[0].id;
    let refused = s.apply(Command::AddBoreFromDrill(AddBoreFromDrillArgs {
        index: 0,
        drill_id: pocket_id,
    }));
    assert!(
        matches!(refused, Err(SessionError::NotADrill { index: 0, .. })),
        "{refused:?}"
    );
    // A stale index (the id of another toolpath) refuses.
    let stale = s.apply(Command::AddBoreFromDrill(AddBoreFromDrillArgs {
        index: drill_index,
        drill_id: pocket_id,
    }));
    assert!(
        matches!(stale, Err(SessionError::ToolpathIdMismatch { .. })),
        "{stale:?}"
    );
    assert_eq!(s.toolpath_configs().len(), 4, "a refusal adds nothing");
}

#[test]
fn add_toolpath_invalid_setup() {
    let mut s = make_session();
    let tc = make_tc(0, 0);
    let result = s.add_toolpath(99, tc);
    assert!(matches!(result, Err(SessionError::SetupNotFound(99))));
}

#[test]
fn add_toolpath_invalidates_simulation() {
    let mut s = make_session();
    let _ = s.add_tool(make_tool());
    let tc = make_tc(s.tools()[0].id.raw(), 0);
    let _ = s.add_toolpath(0, tc).unwrap();
    // simulation is cleared (set to None) by add_toolpath
    assert!(s.simulation.is_none());
}

#[test]
fn remove_toolpath_shifts_indices() {
    let mut s = make_session();
    let _ = s.add_tool(make_tool());
    let tool_id = s.tools()[0].id.raw();

    let _ = s.add_toolpath(0, make_tc(tool_id, 0)).unwrap();
    let _ = s.add_toolpath(0, make_tc(tool_id, 0)).unwrap();
    let _ = s.add_toolpath(0, make_tc(tool_id, 0)).unwrap();
    assert_eq!(s.list_setups()[0].toolpath_indices, vec![0, 1, 2]);

    // Add cached result for index 2
    s.results.insert(2, fake_result());

    let _ = s.remove_toolpath(0).unwrap();

    // Setup indices shifted: [1, 2] → [0, 1]
    assert_eq!(s.list_setups()[0].toolpath_indices, vec![0, 1]);
    // Result for old index 2 should now be at index 1
    assert!(s.results.contains_key(&1));
    assert!(!s.results.contains_key(&2));
}

#[test]
fn remove_toolpath_not_found() {
    let mut s = make_session();
    assert!(matches!(
        s.remove_toolpath(0),
        Err(SessionError::ToolpathNotFound(0))
    ));
}

#[test]
fn reorder_toolpath_swaps() {
    let mut s = make_session();
    let _ = s.add_tool(make_tool());
    let tool_id = s.tools()[0].id.raw();

    let _ = s.add_toolpath(0, make_tc(tool_id, 0)).unwrap();
    let _ = s.add_toolpath(0, make_tc(tool_id, 0)).unwrap();

    let id_0 = s.toolpath_configs()[0].id;
    let id_1 = s.toolpath_configs()[1].id;

    let _ = s.reorder_toolpath(0, 1).unwrap();
    // Global config order is stable; only the setup's display order swaps.
    assert_eq!(s.toolpath_configs()[0].id, id_0);
    assert_eq!(s.toolpath_configs()[1].id, id_1);
    assert_eq!(s.list_setups()[0].toolpath_indices, vec![1, 0]);
}

#[test]
fn set_toolpath_enabled_invalidates_sim() {
    let mut s = make_session();
    let _ = s.add_tool(make_tool());
    let _ = s
        .add_toolpath(0, make_tc(s.tools()[0].id.raw(), 0))
        .unwrap();
    let _ = s.set_toolpath_enabled(0, false).unwrap();
    assert!(!s.toolpath_configs()[0].enabled);
    assert!(s.simulation.is_none());
}

#[test]
fn set_dressup_invalidates_result_and_sim() {
    let mut s = make_session();
    let _ = s.add_tool(make_tool());
    let _ = s
        .add_toolpath(0, make_tc(s.tools()[0].id.raw(), 0))
        .unwrap();
    s.results.insert(0, fake_result());

    let _ = s.set_dressup_config(0, DressupConfig::default()).unwrap();
    assert!(!s.results.contains_key(&0));
    assert!(s.simulation.is_none());
}

/// The two F-040 lead feeds are `Option` fields that serde skips when
/// `None`, so a fresh config's JSON object does not carry them. The setter
/// used that object as its key list and refused both as unknown; the two
/// feeds were unreachable over MCP and the CLI. The field table is the key
/// list now.
///
/// CUT-13 added the second half: a value field lives inside its dressup's
/// `Option`, so the setter refuses it while the dressup is off and names the
/// enable key. Turning the dressup on first is the whole change.
#[test]
fn an_optional_dressup_field_is_settable_on_a_fresh_config() {
    let mut s = make_session();
    let _ = s.add_tool(make_tool());
    let _ = s
        .add_toolpath(0, make_tc(s.tools()[0].id.raw(), 0))
        .unwrap();
    assert!(s.toolpath_configs()[0].dressups.lead_in_out.is_none());

    let err = s
        .set_dressup_field(0, "lead_in_feed_rate", serde_json::json!(450.0))
        .expect_err("a value field of a dressup that is off must be refused");
    assert!(err.to_string().contains("lead_in_out"), "{err}");

    let _ = s
        .set_dressup_field(0, "lead_in_out", serde_json::json!(true))
        .expect("the enable key is settable");
    for key in ["lead_in_feed_rate", "lead_out_feed_rate"] {
        let _effects = s
            .set_dressup_field(0, key, serde_json::json!(450.0))
            .unwrap_or_else(|e| panic!("{key} must be settable once the dressup is on: {e}"));
    }
    let lead = s.toolpath_configs()[0]
        .dressups
        .lead_in_out
        .expect("lead-in/out is on");
    assert_eq!(lead.in_feed_rate, Some(450.0));
    assert_eq!(lead.out_feed_rate, Some(450.0));

    let err = s
        .set_dressup_field(0, "no_such_dressup_field", serde_json::json!(1))
        .expect_err("a name outside the field table is still refused");
    assert!(err.to_string().contains("unknown dressup field"), "{err}");
}

#[test]
fn set_heights_invalidates_result_and_sim() {
    let mut s = make_session();
    let _ = s.add_tool(make_tool());
    let _ = s
        .add_toolpath(0, make_tc(s.tools()[0].id.raw(), 0))
        .unwrap();
    s.results.insert(0, fake_result());

    let _ = s.set_heights_config(0, HeightsConfig::default()).unwrap();
    assert!(!s.results.contains_key(&0));
    assert!(s.simulation.is_none());
}

// ── Staleness-chain invalidation sentries ────────────────────
//
// 2026-07-09 live-v2 collision class: a FromRemainingStock finish op was
// generated while an upstream finish op was enabled; the user disabled
// the upstream op and the KEPT downstream result — whose
// optimize_entry_descents rapids were lowered against the old, deeper
// prior stock — grazed the now-taller stock at rapid feed (20 rapid
// collisions at z = old_ceiling + 2 mm). Chain edits must kill dependent
// cached results (`invalidate_result_chain`).

#[test]
fn toggle_enabled_invalidates_downstream_rest_results() {
    let mut s = make_session();
    let _ = s.add_tool(make_tool());
    let tool_id = s.tools()[0].id.raw();
    let _ = s.add_toolpath(0, make_tc(tool_id, 0)).unwrap();
    let mut rest = make_tc(tool_id, 0);
    rest.stock_source = crate::session::StockSource::FromRemainingStock;
    let _ = s.add_toolpath(0, rest).unwrap();
    let _ = s.add_toolpath(0, make_tc(tool_id, 0)).unwrap();
    s.results.insert(0, fake_result());
    s.results.insert(1, fake_result());
    s.results.insert(2, fake_result());

    let _ = s.set_toolpath_enabled(0, false).unwrap();
    // The toggled op keeps its own result (still valid on re-enable)…
    assert!(s.results.contains_key(&0));
    // …the downstream FromRemainingStock result dies (generated against
    // a stock chain that no longer exists)…
    assert!(!s.results.contains_key(&1));
    // …and a downstream Fresh-stock op is untouched.
    assert!(s.results.contains_key(&2));

    // Toggling back is ALSO a chain change (cuts reappear upstream).
    s.results.insert(1, fake_result());
    let _ = s.set_toolpath_enabled(0, true).unwrap();
    assert!(!s.results.contains_key(&1));
}

#[test]
fn content_edit_invalidates_rest_dependents_transitively() {
    let mut s = make_session();
    let _ = s.add_tool(make_tool());
    let tool_id = s.tools()[0].id.raw();
    let _ = s.add_toolpath(0, make_tc(tool_id, 0)).unwrap();
    let mut rest = make_tc(tool_id, 0);
    rest.stock_source = crate::session::StockSource::FromRemainingStock;
    let _ = s.add_toolpath(0, rest).unwrap();
    // Consumer of op1's derived rest regions (Fresh stock — reached only
    // through the DerivedRestRegions edge, not the stock chain).
    let mut consumer = make_tc(tool_id, 0);
    consumer.boundary = BoundaryConfig {
        enabled: true,
        source: crate::compute::config::BoundarySource::DerivedRestRegions {
            source_toolpath_id: ToolpathId::from_raw(1),
        },
        ..BoundaryConfig::default()
    };
    let _ = s.add_toolpath(0, consumer).unwrap();
    s.results.insert(0, fake_result());
    s.results.insert(1, fake_result());
    s.results.insert(2, fake_result());

    let _ = s.set_heights_config(0, HeightsConfig::default()).unwrap();
    assert!(!s.results.contains_key(&0), "edited op invalidated");
    assert!(
        !s.results.contains_key(&1),
        "downstream rest op invalidated via the stock chain"
    );
    assert!(
        !s.results.contains_key(&2),
        "rest-region consumer invalidated transitively"
    );
}

#[test]
fn disabled_op_edit_leaves_downstream_alone() {
    let mut s = make_session();
    let _ = s.add_tool(make_tool());
    let tool_id = s.tools()[0].id.raw();
    let mut off = make_tc(tool_id, 0);
    off.enabled = false;
    let _ = s.add_toolpath(0, off).unwrap();
    let mut rest = make_tc(tool_id, 0);
    rest.stock_source = crate::session::StockSource::FromRemainingStock;
    let _ = s.add_toolpath(0, rest).unwrap();
    s.results.insert(1, fake_result());

    // Editing an op that is (and stays) disabled doesn't change the
    // material-removal chain — downstream results survive.
    let _ = s.set_heights_config(0, HeightsConfig::default()).unwrap();
    assert!(s.results.contains_key(&1));
}

#[test]
fn set_boundary_invalidates_result_and_sim() {
    let mut s = make_session();
    let _ = s.add_tool(make_tool());
    let _ = s
        .add_toolpath(0, make_tc(s.tools()[0].id.raw(), 0))
        .unwrap();
    s.results.insert(0, fake_result());

    let _ = s.set_boundary_config(0, BoundaryConfig::default()).unwrap();
    assert!(!s.results.contains_key(&0));
    assert!(s.simulation.is_none());
}

#[test]
fn set_boundary_derived_rest_regions_auto_enables_source_rest_analysis() {
    let mut s = make_session();
    let _ = s.add_tool(make_tool());
    let tool_id = s.tools()[0].id.raw();
    // Source toolpath (a plain pocket): rest analysis starts disabled.
    let source_idx = s
        .add_toolpath(0, make_tc(tool_id, 0))
        .unwrap()
        .created
        .expect("add_toolpath reports the new toolpath index");
    let source_id = s.toolpath_configs()[source_idx].id;
    // Consumer toolpath.
    let consumer_idx = s
        .add_toolpath(0, make_tc(tool_id, 0))
        .unwrap()
        .created
        .expect("add_toolpath reports the new toolpath index");
    s.results.insert(source_idx, fake_result());

    let boundary = BoundaryConfig {
        enabled: true,
        source: crate::compute::config::BoundarySource::DerivedRestRegions {
            source_toolpath_id: source_id,
        },
        ..BoundaryConfig::default()
    };
    let _ = s.set_boundary_config(consumer_idx, boundary).unwrap();

    assert!(s.toolpath_configs()[source_idx].rest_analysis.enabled);
    // The source's own cached result must be invalidated — it needs to
    // regenerate to actually attach the rest regions.
    assert!(!s.results.contains_key(&source_idx));
}

#[test]
fn set_boundary_derived_rest_regions_skips_rest_depth_pencil_source() {
    let mut s = make_session();
    let _ = s.add_tool(make_tool());
    let tool_id = s.tools()[0].id.raw();

    let source_tc = ToolpathConfig {
        shoe: Default::default(),
        operation: OperationConfig::Pencil(PencilConfig {
            detector: crate::finish::pencil::PencilDetector::RestDepth,
            ..PencilConfig::default()
        }),
        ..make_tc(tool_id, 0)
    };
    let source_idx = s
        .add_toolpath(0, source_tc)
        .unwrap()
        .created
        .expect("add_toolpath reports the new toolpath index");
    let source_id = s.toolpath_configs()[source_idx].id;
    let consumer_idx = s
        .add_toolpath(0, make_tc(tool_id, 0))
        .unwrap()
        .created
        .expect("add_toolpath reports the new toolpath index");

    let boundary = BoundaryConfig {
        enabled: true,
        source: crate::compute::config::BoundarySource::DerivedRestRegions {
            source_toolpath_id: source_id,
        },
        ..BoundaryConfig::default()
    };
    let _ = s.set_boundary_config(consumer_idx, boundary).unwrap();

    // A rest_depth pencil already attaches its own rest artifacts —
    // forcing the generic flag on would be redundant, so it stays off.
    assert!(!s.toolpath_configs()[source_idx].rest_analysis.enabled);
}

#[test]
fn rest_region_consumers_finds_enabled_consumers_only() {
    let mut s = make_session();
    let _ = s.add_tool(make_tool());
    let tool_id = s.tools()[0].id.raw();

    let source_idx = s
        .add_toolpath(0, make_tc(tool_id, 0))
        .unwrap()
        .created
        .expect("add_toolpath reports the new toolpath index");
    let source_id = s.toolpath_configs()[source_idx].id;
    let enabled_consumer_idx = s
        .add_toolpath(0, make_tc(tool_id, 0))
        .unwrap()
        .created
        .expect("add_toolpath reports the new toolpath index");
    let disabled_consumer_idx = s
        .add_toolpath(0, make_tc(tool_id, 0))
        .unwrap()
        .created
        .expect("add_toolpath reports the new toolpath index");
    let enabled_consumer_id = s.toolpath_configs()[enabled_consumer_idx].id;

    let enabled_boundary = BoundaryConfig {
        enabled: true,
        source: crate::compute::config::BoundarySource::DerivedRestRegions {
            source_toolpath_id: source_id,
        },
        ..BoundaryConfig::default()
    };
    let _ = s
        .set_boundary_config(enabled_consumer_idx, enabled_boundary.clone())
        .unwrap();
    let disabled_boundary = BoundaryConfig {
        enabled: false,
        ..enabled_boundary
    };
    let _ = s
        .set_boundary_config(disabled_consumer_idx, disabled_boundary)
        .unwrap();

    assert_eq!(
        s.rest_region_consumers(source_id),
        vec![enabled_consumer_id]
    );
}

// ── Tool CRUD ────────────────────────────────────────────────

#[test]
fn add_tool_returns_index_and_assigns_id() {
    let mut s = make_session();
    let idx = s
        .add_tool(make_tool())
        .unwrap()
        .created
        .expect("add_tool reports the new tool index");
    assert_eq!(idx, 0);
    let idx2 = s
        .add_tool(make_tool())
        .unwrap()
        .created
        .expect("add_tool reports the new tool index");
    assert_eq!(idx2, 1);
    assert_ne!(s.tools()[0].id, s.tools()[1].id);
}

#[test]
fn remove_tool_in_use_errors() {
    let mut s = make_session();
    let _ = s.add_tool(make_tool());
    let tool_id = s.tools()[0].id.raw();
    let _ = s.add_toolpath(0, make_tc(tool_id, 0)).unwrap();

    let result = s.remove_tool(0);
    assert!(matches!(result, Err(SessionError::ToolInUse(_))));
}

#[test]
fn remove_tool_not_in_use_succeeds() {
    let mut s = make_session();
    let _ = s.add_tool(make_tool());
    let _ = s.add_tool(make_tool());
    assert_eq!(s.tools().len(), 2);

    let _ = s.remove_tool(1).unwrap();
    assert_eq!(s.tools().len(), 1);
}

// ── Setup CRUD ───────────────────────────────────────────────

#[test]
fn add_setup_returns_index() {
    let mut s = make_session();
    // new_empty already creates setup 0
    assert_eq!(s.list_setups().len(), 1);

    let idx = s
        .add_setup("Setup 2".to_owned(), FaceUp::Bottom)
        .created
        .expect("add_setup reports the new setup index");
    assert_eq!(idx, 1);
    assert_eq!(s.list_setups().len(), 2);
    assert_eq!(s.list_setups()[1].name, "Setup 2");
    assert_eq!(s.list_setups()[1].face_up, FaceUp::Bottom);
}

#[test]
fn remove_setup_cascades_owned_toolpaths_and_refuses_the_last() {
    let mut s = make_session();
    let first_setup_id = s.list_setups()[0].id;
    let tool = s
        .add_tool(make_tool())
        .unwrap()
        .created
        .expect("tool index");
    let tool_id = s.tools().get(tool).expect("added tool").id.raw();
    let removed_setup = s
        .apply(Command::AddSetup(AddSetupArgs {
            name: Some("Removed".to_owned()),
            face_up: FaceUp::Top,
        }))
        .expect("removed setup")
        .created
        .expect("removed setup index");
    let after_setup = s
        .apply(Command::AddSetup(AddSetupArgs {
            name: Some("After".to_owned()),
            face_up: FaceUp::Bottom,
        }))
        .expect("after setup")
        .created
        .expect("after setup index");
    let after_setup_id = s.list_setups()[after_setup].id;

    let before = s
        .add_toolpath(0, make_tc(tool_id, 0))
        .expect("toolpath before removed setup")
        .created
        .expect("before index");
    let owned = s
        .add_toolpath(removed_setup, make_tc(tool_id, 0))
        .expect("owned toolpath")
        .created
        .expect("owned index");
    let owned_id = s.toolpath_configs()[owned].id;
    let mut dependent_config = make_tc(tool_id, 0);
    dependent_config.boundary = BoundaryConfig {
        enabled: true,
        source: crate::compute::config::BoundarySource::DerivedRestRegions {
            source_toolpath_id: owned_id,
        },
        ..BoundaryConfig::default()
    };
    let dependent = s
        .add_toolpath(after_setup, dependent_config)
        .expect("dependent toolpath after removed setup")
        .created
        .expect("dependent index");
    let independent = s
        .add_toolpath(after_setup, make_tc(tool_id, 0))
        .expect("independent toolpath after removed setup")
        .created
        .expect("independent index");

    let before_id = s.toolpath_configs()[before].id;
    let dependent_id = s.toolpath_configs()[dependent].id;
    let independent_id = s.toolpath_configs()[independent].id;
    for (index, moves) in [
        (before, 10),
        (owned, 20),
        (dependent, 30),
        (independent, 40),
    ] {
        let mut result = fake_result();
        result.stats.move_count = moves;
        s.results.insert(index, result);
    }
    let revisions_before: Vec<_> = (0..s.toolpath_count())
        .map(|index| s.toolpath_revision_at(index))
        .collect();
    let epoch = s.simulation_epoch();

    let effects = s
        .apply(Command::RemoveSetup(RemoveSetupArgs {
            setup_index: removed_setup,
        }))
        .expect("remove setup");

    assert_eq!(
        s.toolpath_configs()
            .iter()
            .map(|config| config.id)
            .collect::<Vec<_>>(),
        vec![before_id, dependent_id, independent_id],
        "only the removed setup's owned toolpath leaves"
    );
    assert_eq!(s.list_setups().len(), 2);
    assert_eq!(s.list_setups()[0].id, first_setup_id);
    assert_eq!(s.list_setups()[1].id, after_setup_id);
    assert_eq!(s.list_setups()[0].toolpath_indices, vec![0]);
    assert_eq!(s.list_setups()[1].toolpath_indices, vec![1, 2]);
    assert_eq!(
        s.result_at(0).map(|result| result.stats.move_count),
        Some(10),
        "the independent result before the removed setup is preserved"
    );
    assert!(
        s.result_at(1).is_none(),
        "the removed toolpath's dependent result is invalidated"
    );
    assert_eq!(
        s.result_at(2).map(|result| result.stats.move_count),
        Some(40),
        "an independent result after the removed setup is rekeyed"
    );
    assert_eq!(effects.stale, BTreeSet::from([0, 1, 2]));
    assert!(effects.revision.is_none());
    for (new_index, old_index) in [(0, before), (1, dependent), (2, independent)] {
        assert!(
            s.toolpath_revision_at(new_index) > revisions_before[old_index],
            "the surviving toolpath at index {new_index} must reject an in-flight old-index result"
        );
    }
    assert_eq!(s.simulation_epoch(), epoch + 1);

    let mut empty = make_session();
    let empty_setup = empty
        .apply(Command::AddSetup(AddSetupArgs {
            name: Some("Empty".to_owned()),
            face_up: FaceUp::Top,
        }))
        .expect("empty setup")
        .created
        .expect("empty setup index");
    let empty_epoch = empty.simulation_epoch();
    let empty_effects = empty
        .apply(Command::RemoveSetup(RemoveSetupArgs {
            setup_index: empty_setup,
        }))
        .expect("remove empty setup");
    assert!(empty_effects.stale.is_empty());
    assert_eq!(empty.simulation_epoch(), empty_epoch + 1);

    let final_epoch = empty.simulation_epoch();
    assert!(matches!(
        empty.apply(Command::RemoveSetup(RemoveSetupArgs { setup_index: 0 })),
        Err(SessionError::InvalidParam(_))
    ));
    assert_eq!(empty.simulation_epoch(), final_epoch);
}

// ── Cross-setup move ─────────────────────────────────────────

#[test]
fn move_toolpath_to_setup_moves_and_invalidates() {
    let mut s = make_session();
    let _ = s.add_tool(make_tool());
    let _ = s.add_setup("Setup 2".to_owned(), FaceUp::Bottom);

    let tool_id = s.tools()[0].id.raw();
    let _ = s.add_toolpath(0, make_tc(tool_id, 0)).unwrap();
    s.results.insert(0, fake_result());

    assert_eq!(s.list_setups()[0].toolpath_indices, vec![0]);
    assert!(s.list_setups()[1].toolpath_indices.is_empty());

    let _ = s.move_toolpath_to_setup(0, 1, None).unwrap();

    assert!(s.list_setups()[0].toolpath_indices.is_empty());
    assert_eq!(s.list_setups()[1].toolpath_indices, vec![0]);
    assert!(!s.results.contains_key(&0));
    assert!(s.simulation.is_none());
}

#[test]
fn move_toolpath_invalid_target() {
    let mut s = make_session();
    let _ = s.add_tool(make_tool());
    let _ = s
        .add_toolpath(0, make_tc(s.tools()[0].id.raw(), 0))
        .unwrap();

    assert!(matches!(
        s.move_toolpath_to_setup(0, 99, None),
        Err(SessionError::SetupNotFound(99))
    ));
}

// ── Selection edit ───────────────────────────────────────────

#[test]
fn a_face_pick_edit_invalidates() {
    let mut s = make_session();
    let _ = s.add_tool(make_tool());
    let _ = s
        .add_toolpath(0, make_tc(s.tools()[0].id.raw(), 0))
        .unwrap();
    s.results.insert(0, fake_result());

    // P2.6: a face pick is a `MatchKey` pick of the op selection.
    let key = crate::selection::MatchKey {
        source: crate::selection::SourceRef::ToolpathModel,
        kind: crate::selection::ItemKind::Face,
        layer: None,
        anchor: [10.0, 20.0],
        measure: 5.0,
        brep: Some(3),
    };
    let _ = s
        .edit_toolpath_selection(
            0,
            crate::session::SelectionTarget::Op,
            crate::selection::SelectionEdit::AddPick(key.clone()),
        )
        .unwrap();

    let picks = s.toolpath_configs()[0]
        .operation
        .selection()
        .map(|sel| sel.picks().to_vec());
    assert_eq!(picks, Some(vec![key]));
    assert!(!s.results.contains_key(&0));
    assert!(s.simulation.is_none());
}

// ── Rename setup ─────────────────────────────────────────────

#[test]
fn rename_setup_changes_name() {
    let mut s = make_session();
    let _ = s.rename_setup(0, "New Name".to_owned()).unwrap();
    assert_eq!(s.list_setups()[0].name, "New Name");
}

#[test]
fn rename_setup_not_found() {
    let mut s = make_session();
    assert!(matches!(
        s.rename_setup(99, "x".to_owned()),
        Err(SessionError::SetupNotFound(99))
    ));
}

// ── Obstacle CRUD (K1) ───────────────────────────────────────

fn clamp(id: usize) -> Obstacle {
    Obstacle {
        id: ObstacleId::from_raw(id),
        name: format!("Clamp {id}"),
        enabled: true,
        clearance: 3.0,
        geometry: ObstacleGeometry::Box {
            min: [0.0, 0.0, 0.0],
            max: [30.0, 15.0, 20.0],
        },
    }
}

#[test]
fn add_obstacle_invalidates_setup_toolpaths() {
    let mut s = make_session();
    let _ = s.add_tool(make_tool());
    let _ = s
        .add_toolpath(0, make_tc(s.tools()[0].id.raw(), 0))
        .unwrap();
    s.results.insert(0, fake_result());

    let _ = s.add_obstacle(0, clamp(0)).unwrap();

    assert_eq!(s.list_setups()[0].obstacles.len(), 1);
    assert!(!s.results.contains_key(&0));
    assert!(s.simulation.is_none());
}

#[test]
fn a_rename_keeps_the_results_and_a_move_drops_them() {
    let mut s = make_session();
    let _ = s.add_tool(make_tool());
    let _ = s
        .add_toolpath(0, make_tc(s.tools()[0].id.raw(), 0))
        .unwrap();
    let _ = s.add_obstacle(0, clamp(0)).unwrap();
    s.results.insert(0, fake_result());

    let mut renamed = clamp(0);
    renamed.name = "Left clamp".to_owned();
    let _ = s
        .replace_obstacle(0, ObstacleId::from_raw(0), renamed.clone())
        .unwrap();
    assert!(s.results.contains_key(&0), "a rename drops nothing");

    let mut moved = renamed;
    moved.geometry = ObstacleGeometry::Box {
        min: [1.0, 0.0, 0.0],
        max: [31.0, 15.0, 20.0],
    };
    let _ = s
        .replace_obstacle(0, ObstacleId::from_raw(0), moved)
        .unwrap();
    assert!(!s.results.contains_key(&0), "a move drops the result");
}

#[test]
fn remove_obstacle_by_id_and_refuse_a_bad_box() {
    let mut s = make_session();
    let _ = s.add_obstacle(0, clamp(42)).unwrap();
    assert_eq!(s.list_setups()[0].obstacles.len(), 1);
    assert!(
        s.add_obstacle(0, clamp(42)).is_err(),
        "a duplicate id refuses"
    );

    let mut empty = clamp(7);
    empty.geometry = ObstacleGeometry::Box {
        min: [0.0, 0.0, 5.0],
        max: [10.0, 10.0, 5.0],
    };
    assert!(s.add_obstacle(0, empty).is_err(), "an empty box refuses");

    let _ = s.remove_obstacle(0, ObstacleId::from_raw(42)).unwrap();
    assert!(s.list_setups()[0].obstacles.is_empty());
}

// ── Invalidation helpers ─────────────────────────────────────

#[test]
fn invalidate_stock_clears_simulation() {
    let mut s = make_session();
    // simulation starts as None; invalidate should keep it None
    let _ = s.invalidate_stock();
    assert!(s.simulation.is_none());
}

#[test]
fn invalidate_machine_clears_simulation() {
    let mut s = make_session();
    let _ = s.invalidate_machine();
    assert!(s.simulation.is_none());
}

#[test]
fn invalidate_tool_clears_matching_results() {
    let mut s = make_session();
    let _ = s.add_tool(make_tool());
    let _ = s.add_tool(make_tool());
    let tool_id_0 = s.tools()[0].id.raw();
    let tool_id_1 = s.tools()[1].id.raw();

    let _ = s.add_toolpath(0, make_tc(tool_id_0, 0)).unwrap(); // idx 0
    let _ = s.add_toolpath(0, make_tc(tool_id_1, 0)).unwrap(); // idx 1
    let _ = s.add_toolpath(0, make_tc(tool_id_0, 0)).unwrap(); // idx 2

    s.results.insert(0, fake_result());
    s.results.insert(1, fake_result());
    s.results.insert(2, fake_result());

    let _ = s.invalidate_tool(crate::ids::ToolId::from_raw(tool_id_0));

    // Results for toolpaths using tool_id_0 (idx 0, 2) should be cleared
    assert!(!s.results.contains_key(&0));
    assert!(s.results.contains_key(&1)); // uses tool_id_1, unaffected
    assert!(!s.results.contains_key(&2));
}

// ── Global config ────────────────────────────────────────────

#[test]
fn set_stock_config_runs() {
    let mut s = make_session();
    let _ = s.set_stock_config(StockConfig::default());
    assert!(s.simulation.is_none());
}

#[test]
fn replace_tools_updates_id_counter() {
    let mut s = make_session();
    let mut t1 = make_tool();
    t1.id = ToolId::from_raw(5);
    let mut t2 = make_tool();
    t2.id = ToolId::from_raw(10);
    let _ = s.replace_tools(vec![t1, t2]);

    // next_tool_id should be max(5, 10) + 1 = 11
    let new_idx = s
        .add_tool(make_tool())
        .unwrap()
        .created
        .expect("add_tool reports the new tool index");
    assert_eq!(s.tools()[new_idx].id, ToolId::from_raw(11));
}

/// Replace the configuration at index 0 through the command door.
fn replace(s: &mut ProjectSession, config: ToolpathConfig) -> Effects {
    s.apply(Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
        index: 0,
        config: Box::new(config),
    }))
    .expect("index 0 exists")
}

/// WP5. A replacement that moves no generation input keeps the
/// cached result, and the four fields outside the signature land.
///
/// The GUI inspector applies this command on every frame the panel is
/// open, because it holds no commit event. An ungated replacement —
/// which is what `replace_toolpath_config` was before WP5 — would
/// therefore drop the geometry on every such frame.
#[test]
fn replace_toolpath_config_outside_the_signature_keeps_the_result() {
    let mut s = make_session();
    let _ = s.add_tool(make_tool());
    let _ = s
        .add_toolpath(0, make_tc(s.tools()[0].id.raw(), 0))
        .unwrap();
    s.results.insert(0, fake_result());

    let mut edited = s.toolpath_configs()[0].clone();
    edited.name = "renamed".to_owned();
    edited.coolant = CoolantMode::Flood;
    edited.pre_gcode = Some("M3 S18000".to_owned());
    edited.post_gcode = Some("M5".to_owned());

    let effects = replace(&mut s, edited);

    assert!(
        effects.stale.is_empty(),
        "name, coolant and the pre and post G-code change no motion"
    );
    assert!(
        s.results.contains_key(&0),
        "the cached geometry still answers every input that decides it"
    );
    let stored = &s.toolpath_configs()[0];
    assert_eq!(stored.name, "renamed");
    assert_eq!(stored.coolant, CoolantMode::Flood);
    assert_eq!(stored.pre_gcode.as_deref(), Some("M3 S18000"));
    assert_eq!(stored.post_gcode.as_deref(), Some("M5"));
}

/// WP5. A replacement that moves a generation input drops the result.
#[test]
fn replace_toolpath_config_on_a_moved_signature_invalidates() {
    let mut s = make_session();
    let _ = s.add_tool(make_tool());
    let _ = s
        .add_toolpath(0, make_tc(s.tools()[0].id.raw(), 0))
        .unwrap();
    s.results.insert(0, fake_result());

    let mut edited = s.toolpath_configs()[0].clone();
    edited.operation.set_feed_rate(4321.0);

    let effects = replace(&mut s, edited);

    assert_eq!(effects.stale, BTreeSet::from([0_usize]));
    assert!(!s.results.contains_key(&0));
}

#[test]
fn update_stock_from_bbox_invalidates_sim() {
    let mut s = make_session();
    let pad = s.stock_config().padding;
    let bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(120.0, 80.0, 25.0),
    };
    let _ = s.update_stock_from_bbox(&bbox);

    // Stock grew to fit bbox + padding (XY) and exact bbox + padding (Z).
    assert!((s.stock_config().x - (120.0 + 2.0 * pad)).abs() < 1e-6);
    assert!((s.stock_config().y - (80.0 + 2.0 * pad)).abs() < 1e-6);
    assert!((s.stock_config().z - (25.0 + pad)).abs() < 1e-6);
    // Simulation cache is cleared (mirrors the pattern in
    // set_toolpath_enabled_invalidates_sim).
    assert!(s.simulation.is_none());
}

/// Deliver a completion the way the compute lane does.
///
/// `insert_result` is no longer a public door. A completion arrives
/// as `Command::AdoptResult`, which carries the revision the lane
/// started from.
fn adopt(s: &mut ProjectSession, index: usize) {
    let revision = s.toolpath_revision_at(index);
    let _ = s
        .apply(Command::AdoptResult(AdoptResultArgs {
            index,
            revision,
            result: Box::new(fake_result()),
        }))
        .expect("the fixture adopts at the current revision");
}

#[test]
fn adopt_result_populates_cache() {
    let mut s = make_session();
    let _ = s.add_tool(make_tool());
    let _ = s
        .add_toolpath(0, make_tc(s.tools()[0].id.raw(), 0))
        .unwrap();

    assert!(s.result_at(0).is_none());
    adopt(&mut s, 0);
    assert!(s.result_at(0).is_some());
}

#[test]
fn adopt_result_after_apply_snapshot_restores_cache() {
    // Roadmap F.1: the snapshot path clears results[idx].
    // The viz-side regen used to leave that empty
    // because writes only landed in gui.toolpath_rt. The adoption
    // door is the symmetric write that closes the cache gap.
    let mut s = make_session();
    let _ = s.add_tool(make_tool());
    let _ = s
        .add_toolpath(0, make_tc(s.tools()[0].id.raw(), 0))
        .unwrap();
    s.results.insert(0, fake_result());

    let snapshot = s.apply(Command::RestoreToolpathSnapshot(
        crate::session::RestoreToolpathSnapshotArgs {
            index: 0,
            operation: Box::new(OperationConfig::Pocket(PocketConfig::default())),
            dressups: Box::new(DressupConfig::default()),
            boundary: Box::new(crate::compute::config::BoundaryConfig::default()),
            feeds_provenance: None,
        },
    ));
    assert!(snapshot.is_ok(), "index 0 exists");
    assert!(s.result_at(0).is_none());

    adopt(&mut s, 0);
    assert!(s.result_at(0).is_some());
}

#[test]
fn adopt_result_rejects_out_of_range_index() {
    let mut s = make_session();
    let err = s
        .apply(Command::AdoptResult(AdoptResultArgs {
            index: 99,
            revision: 0,
            result: Box::new(fake_result()),
        }))
        .unwrap_err();
    assert!(matches!(err, SessionError::ToolpathNotFound(99)));
}

// ── Phase 0 arm 3 — the un-commanded write ───────────────────
//
// WP7 moved this arm in from
// `crates/rs_cam_core/tests/mutation_paths_invalidate_alike_p0.rs`.
// The arm's SUBJECT is a direct field write followed by the public
// door `ProjectSession::invalidate_toolpath_inputs` — the feeds
// Apply funnel's path since N13. No `Command` row expresses it,
// because a row is exactly what it does NOT take, and the nine
// mutation hatches are `pub(crate)` since WP7, so an integration
// test cannot reach one. In-crate keeps both the reach and the
// meaning. The assertions are the ones the P0 file carried.
//
// The P0 file keeps arms 1, 2 and 4, its own non-vacuity guard and
// every other contract.

/// The one feed value every arm writes.
const EDITED_FEED_RATE: f64 = 4321.0;

/// What one write path invalidated.
#[derive(Debug)]
struct Observation {
    /// Indices whose cached result is gone after the edit.
    dropped: BTreeSet<usize>,
    /// Indices whose `toolpath_revision` moved.
    bumped: BTreeSet<usize>,
}

fn revisions(s: &ProjectSession) -> Vec<u64> {
    (0..s.toolpath_count())
        .map(|i| s.toolpath_revision_at(i))
        .collect()
}

fn observe(s: &ProjectSession, before: &[u64]) -> Observation {
    let mut dropped = BTreeSet::new();
    let mut bumped = BTreeSet::new();
    for i in 0..s.toolpath_count() {
        if s.result_at(i).is_none() {
            dropped.insert(i);
        }
        if before.get(i).copied() != Some(s.toolpath_revision_at(i)) {
            bumped.insert(i);
        }
    }
    Observation { dropped, bumped }
}

fn set(indices: &[usize]) -> BTreeSet<usize> {
    indices.iter().copied().collect()
}

fn assert_feed_landed(s: &ProjectSession) {
    let feed = s.toolpath_configs()[0].operation.feed_rate();
    assert!(
        (feed - EDITED_FEED_RATE).abs() < 1e-9,
        "the arm must write the feed. Otherwise it invalidates nothing \
         for a reason this file does not measure. I read {feed}"
    );
}

/// One setup, one tool, one model, two enabled toolpaths in plan
/// order: index 0 `Fresh`, index 1 `FromRemainingStock`. Both carry a
/// cached result. The P0 file's fixture, rebuilt in-crate.
fn chain_fixture() -> ProjectSession {
    let mut s = make_session();
    let _ = s.add_tool(make_tool());
    let _ = s.add_model(crate::session::LoadedModel {
        id: crate::ids::ModelId::from_raw(0),
        name: "part.svg".to_owned(),
        mesh: None,
        items: Default::default(),
        path: std::path::PathBuf::from("part.svg"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    });
    let tool = s.tools()[0].id.raw();
    let model = s.models()[0].id;
    let upstream = make_tc(tool, model.raw());
    let mut downstream = make_tc(tool, model.raw());
    downstream.stock_source = crate::session::StockSource::FromRemainingStock;
    let _ = s.add_toolpath(0, upstream).unwrap();
    let _ = s.add_toolpath(0, downstream).unwrap();
    adopt(&mut s, 0);
    adopt(&mut s, 1);
    assert!(
        s.result_at(0).is_some() && s.result_at(1).is_some(),
        "both rows need a cached result, or every reading below is \
         vacuous"
    );
    s
}

/// Arm 1 — `ProjectSession::set_toolpath_param`, the MCP and CLI
/// door. Present so arm 3 has something to be equal to.
fn arm_setter() -> Observation {
    let mut s = chain_fixture();
    let before = revisions(&s);
    let _ = s
        .set_toolpath_param(0, "feed_rate", serde_json::json!(EDITED_FEED_RATE))
        .expect("feed_rate is a Pocket parameter");
    assert_feed_landed(&s);
    observe(&s, &before)
}

/// Arm 3 — a direct field write, then the public door
/// `ProjectSession::invalidate_toolpath_inputs`.
///
/// The arm stays raw on purpose. Pointing it at
/// `Command::ReplaceToolpathConfig` would make the comparison below
/// read arm 4 against arm 4.
fn arm_inspector_door() -> Observation {
    let mut s = chain_fixture();
    let before = revisions(&s);
    let configs = s.toolpath_configs_mut();
    configs[0].operation.set_feed_rate(EDITED_FEED_RATE);
    let _ = s.invalidate_toolpath_inputs(0);
    assert_feed_landed(&s);
    observe(&s, &before)
}

/// CONTRACT. The setter and the feeds funnel's door invalidate alike.
#[test]
fn the_setter_and_the_inspector_door_agree() {
    let setter = arm_setter();
    let inspector = arm_inspector_door();

    assert_eq!(
        setter.dropped,
        set(&[0, 1]),
        "set_toolpath_param walks the stock chain. The edited row and the \
         downstream FromRemainingStock row both go stale."
    );
    assert_eq!(
        inspector.dropped, setter.dropped,
        "invalidate_toolpath_inputs is the feeds funnel's door onto the \
         same chain walk. The two must not diverge."
    );
}

/// CONTRACT. On this arm too, a dropped result and a bumped revision
/// are one event.
#[test]
fn the_inspector_door_drops_and_bumps_the_same_set() {
    let name = "invalidate_toolpath_inputs";
    let obs = arm_inspector_door();
    assert_eq!(
        obs.bumped, obs.dropped,
        "{name}: a reader that compares revisions and a reader that \
         checks for a cached result must reach one answer"
    );
}

// ── Simulation drop causes ───────────────────────────────────

/// Each drop records its cause against the new epoch, so a run submitted
/// at one epoch names only the causes of the drops after it. A machine
/// edit must not read as a parameter change.
#[test]
fn a_simulation_drop_records_its_cause_against_the_epoch() {
    use crate::session::{SimulationDropCause, SimulationDropCauses};

    let mut s = make_session();
    let start = s.simulation_epoch();
    assert_eq!(
        s.simulation_drop_causes_since(start),
        SimulationDropCauses::EMPTY
    );

    let _ = s.invalidate_stock();
    let after_stock = s.simulation_epoch();
    let _ = s.invalidate_machine();
    let after_machine = s.simulation_epoch();
    assert_eq!(after_machine, start + 2, "two drops move the epoch twice");

    let since_start: Vec<_> = s.simulation_drop_causes_since(start).iter().collect();
    assert_eq!(
        since_start,
        vec![
            SimulationDropCause::Operations,
            SimulationDropCause::Machine
        ]
    );
    let since_stock: Vec<_> = s.simulation_drop_causes_since(after_stock).iter().collect();
    assert_eq!(
        since_stock,
        vec![SimulationDropCause::Machine],
        "a run submitted after the stock edit was dropped by the machine edit only"
    );
    assert!(s.simulation_drop_causes_since(after_machine).is_empty());
    assert_eq!(
        s.simulation_drop_causes_since(start)
            .short_label()
            .as_deref(),
        Some("parameters changed, machine settings changed")
    );
}
