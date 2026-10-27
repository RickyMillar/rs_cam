//! Integration step 7.4c (D58 found that no output showed `Move::feed_cap`):
//! the cut trace lists every move that carries a feed cap, once per move
//! (`SimulationCutTrace::capped_moves`), so `simulation.json` shows each
//! cap beside the samples of its move. Lead ruling 2026-10-28: the cap
//! belongs to the move, so the sample keeps its pinned size (G-SIMMEM).
//!
//! 1. A run with a capped and an uncapped cut lists exactly the capped
//!    move, with its toolpath id, move index and cap.
//! 2. The audit join: every sample of the capped move, found by the key
//!    `(toolpath_id, move_index)`, has a feed at or under the cap; no sample
//!    of the uncapped move finds a row.
//! 3. The wire: the table serialises when it holds a row, and a trace with
//!    no capped move writes no `capped_moves` key.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::simulate::{
    SimGroupEntry, SimToolpathEntry, SimulationRequest, run_simulation,
};
use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::dexel_stock::StockCutDirection;
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::stock::simulation_cut::{CappedMove, SimulationCutTrace};
use rs_cam_core::tool::{FlatEndmill, ToolDefinition};
use rs_cam_core::toolpath::{MoveIntent, Toolpath};
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;

/// The cap of the capped move (mm/min): under the commanded feed of the
/// uncapped cut, as a load rule writes it.
const CAP: f64 = 400.0;
const ID: usize = 3;
/// Move 2 cuts with no cap; move 3 cuts with the cap.
const UNCAPPED: usize = 2;
const CAPPED: usize = 3;

fn toolpath(capped: bool) -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(P3::new(2.0, 2.0, 5.0), MoveIntent::Linking);
    tp.feed_to_with_intent(P3::new(2.0, 2.0, -1.0), 200.0, MoveIntent::EntryPlunge);
    tp.feed_to_with_intent(P3::new(12.0, 2.0, -1.0), 1000.0, MoveIntent::ClearingCut);
    tp.feed_to_with_intent(P3::new(22.0, 2.0, -1.0), CAP, MoveIntent::ClearingCut);
    if capped {
        tp.moves[CAPPED].feed_cap = Some(CAP);
    }
    tp.feed_to_with_intent(P3::new(22.0, 2.0, 5.0), 300.0, MoveIntent::Retract);
    tp
}

fn trace(capped: bool) -> Arc<SimulationCutTrace> {
    let entry = SimToolpathEntry {
        head: Default::default(),
        id: ToolpathId::from_raw(ID),
        name: "cut".to_owned(),
        annotated: Arc::new(AnnotatedToolpath::new(toolpath(capped))),
        tool: Arc::new(ToolDefinition::new(
            Box::new(FlatEndmill::new(6.0, 25.0)),
            6.0,
            45.0,
            2,
            ToolMaterial::Carbide,
        )),
        tool_id: rs_cam_core::compute::tool_config::ToolId::from_raw(0),
        flute_count: 2,
        tool_summary: "6mm Flat".to_owned(),
        semantic_trace: None,
        spindle_rpm: None,
        metrics_not_applicable: false,
        drill_op: None,
        operation_config_hash: 1,
    };
    let request = SimulationRequest {
        groups: vec![SimGroupEntry {
            toolpaths: vec![entry],
            direction: StockCutDirection::FromTop,
            local_stock_bbox: None,
            local_to_global: None,
            phantom_prior_stock: None,
            stock_changes: Vec::new(),
            obstacles: Ok(Default::default()),
        }],
        stock_bbox: BoundingBox3 {
            min: P3::new(0.0, 0.0, -6.0),
            max: P3::new(30.0, 10.0, 0.0),
        },
        stock_top_z: 0.0,
        resolution: 0.25,
        spindle_rpm: 18_000,
        rapid_feed_mm_min: 5000.0,
        model_mesh: None,
        kinematics: None,
        display_stride: 1,
    };
    run_simulation(&request, &AtomicBool::new(false))
        .expect("the simulation runs")
        .cut_trace
        .expect("the run records a cut trace")
}

#[test]
fn a_trace_lists_each_capped_move() {
    let t = trace(true);
    // 1. Exactly the capped move.
    assert_eq!(
        t.capped_moves,
        vec![CappedMove {
            toolpath_id: ToolpathId::from_raw(ID),
            move_index: CAPPED,
            feed_cap_mm_min: CAP,
        }]
    );

    // 2. The audit join, by (toolpath_id, move_index).
    let cap_of = |id: ToolpathId, move_index: usize| {
        t.capped_moves
            .iter()
            .find(|c| c.toolpath_id == id && c.move_index == move_index)
            .map(|c| c.feed_cap_mm_min)
    };
    let mut joined = 0;
    for s in &t.samples {
        match cap_of(s.toolpath_id, s.move_index) {
            Some(cap) => {
                joined += 1;
                assert_eq!(s.move_index, CAPPED);
                assert!(
                    s.feed_rate_mm_min <= cap,
                    "sample {} feeds {} over its move's cap {cap}",
                    s.sample_index,
                    s.feed_rate_mm_min
                );
            }
            None => assert_ne!(s.move_index, CAPPED, "a capped sample found no row"),
        }
    }
    assert!(
        joined > 0,
        "no sample of the capped move; the join proves nothing"
    );
    assert!(
        t.samples.iter().any(|s| s.move_index == UNCAPPED),
        "no sample of the uncapped move; the fixture proves nothing"
    );

    // 3. The wire.
    let json = serde_json::to_value(t.as_ref()).expect("serialise the trace");
    assert_eq!(
        json["capped_moves"],
        serde_json::json!([{
            "toolpath_id": ID,
            "move_index": CAPPED,
            "feed_cap_mm_min": CAP,
        }])
    );
    let plain = serde_json::to_value(trace(false).as_ref()).expect("serialise the trace");
    assert!(
        plain.get("capped_moves").is_none(),
        "a trace with no capped move writes the key"
    );
    // A trace written before the field reads back with an empty table.
    let mut legacy = json;
    legacy
        .as_object_mut()
        .expect("a trace is an object")
        .remove("capped_moves");
    let read: SimulationCutTrace = serde_json::from_value(legacy).expect("read a legacy trace");
    assert!(read.capped_moves.is_empty());
}
