//! P29-10 10g (`P29-10_DESIGN.md` section 8): the instrument that writes
//! the force-field grids of the explainer page.
//!
//! For every toolpath of the four project files of the design (the
//! operator file, the work file, rivmap100 and the live file) it builds the
//! force plane through the one door the GUI reads
//! (`ProjectSession::force_plane`), with the width D and the depth top that
//! the GUI chart uses (`CutMetricSet::diameter_mm`, `field_top`), and
//! writes one JSON file per project into the folder that
//! `RS_CAM_P29_10_EXPLAINER_DATA` names. With no simulation the depth top
//! reads no sample: it equals the top of the GUI chart where the stated
//! depth is at least the deepest sample (`field_top`).
//!
//! It reads operator files and writes files, so it is an instrument:
//! `cargo test -p rs_cam_viz -q --test p29_10_explainer_field_instrument -- --ignored`
//! with `RS_CAM_P29_10_EXPLAINER_DATA` set.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::{Path, PathBuf};

use rs_cam_core::feeds::envelope::{Edge, LimitKind};
use rs_cam_core::session::ProjectSession;
use rs_cam_viz::state::simulation::force_field::field_top;

const PROJECTS: [(&str, &str); 4] = [
    (
        "operator",
        "planning/rivmap_block_job/nz_wanaka_21_operator.toml",
    ),
    (
        "work",
        "planning/rivmap_block_job/nz_wanaka_21_6mm_rough_work.toml",
    ),
    ("rivmap100", "planning/fixtures/rivmap100/rivmap100.toml"),
    (
        "live",
        "planning/fixtures/rivmap100/rivmap100_live_0925.toml",
    ),
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
#[ignore = "instrument: writes the P29-10 explainer field grids from the operator project files"]
fn write_the_explainer_field_grids_p29_10() {
    let out = PathBuf::from(
        std::env::var("RS_CAM_P29_10_EXPLAINER_DATA")
            .expect("RS_CAM_P29_10_EXPLAINER_DATA names the output folder"),
    );
    std::fs::create_dir_all(&out).expect("the output folder");
    for (key, rel) in PROJECTS {
        let session = ProjectSession::load(&repo_root().join(rel)).expect("the project loads");
        let mut ops = Vec::new();
        for (index, tc) in session.toolpath_configs().iter().enumerate() {
            let env = session.safe_envelope(tc).and_then(Result::ok);
            let tool_d = session
                .load_context_parts(index)
                .map(|p| rs_cam_core::tool::MillingCutter::diameter(&p.tool));
            let env_d = env
                .as_ref()
                .and_then(|e| match e.limit(LimitKind::Diameter) {
                    Some(l) => match l.edge {
                        Edge::AeCeiling { mm } if mm.is_finite() && mm > 0.0 => Some(mm),
                        Edge::AeCeiling { .. }
                        | Edge::ApCeiling { .. }
                        | Edge::ApCurve { .. }
                        | Edge::ApFloor { .. }
                        | Edge::AeFloor { .. }
                        | Edge::AeCurve { .. }
                        | Edge::PerPoint
                        | Edge::Unmodelled { .. } => None,
                    },
                    None => None,
                });
            let diameter = env_d.or(tool_d);
            let top = env.as_ref().map(|e| field_top(e, &[]));
            let plane = match (diameter, top) {
                (Some(d), Some(t)) => session.force_plane(index, d, t),
                (Some(_) | None, Some(_) | None) => None,
            };
            ops.push(serde_json::json!({
                "index": index,
                "name": tc.name,
                "op_type": format!("{:?}", tc.operation.op_type()),
                "diameter_mm": diameter,
                "top_mm": top,
                "top_rule": "field_top(envelope, no samples): the GUI chart top where the stated depth is at least the deepest sample",
                "cell_mm": plane.as_ref().map(|p| p.field.cell_mm()),
                "plane": plane,
            }));
        }
        let doc = serde_json::json!({ "project": key, "file": rel, "ops": ops });
        let path = out.join(format!("field_{key}.json"));
        std::fs::write(&path, serde_json::to_string(&doc).expect("json")).expect("write");
    }
}
