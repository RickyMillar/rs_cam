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
//! The GUI chart reads the samples of a full simulation, and the deepest
//! sample can lift the top above the no-sample top (P29-10 review item 2:
//! on the operator file the Back rest 6 mm samples reach 20.7 mm against a
//! no-sample top of 7.8 mm). The instrument runs no simulation. When
//! `RS_CAM_P29_10_DEEPEST_SAMPLE` names a JSON file
//! `{"<project>": {"<toolpath index>": <deepest sample depth, mm>}}`, the
//! top of that op is `field_top` at that deepest sample, which is the GUI
//! chart top. The values come from the "Depth a to b mm" line of the cut
//! force card of the GUI after a simulation of every operation (the card
//! bins the samples of the chart), so they carry the print precision of
//! the card. The 2026-11-14 values (`data/gui_deepest_sample.json` of the
//! explainer) read the simulation that `generate_all` left.
//! Every op writes its rule in `top_rule`.
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
use rs_cam_core::tool_load::cut_force::CutPoint3;
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
    let deepest: serde_json::Value = std::env::var("RS_CAM_P29_10_DEEPEST_SAMPLE")
        .ok()
        .map(|path| {
            let text = std::fs::read_to_string(path).expect("the deepest-sample file reads");
            serde_json::from_str(&text).expect("the deepest-sample file is JSON")
        })
        .unwrap_or(serde_json::Value::Null);
    for (key, rel) in PROJECTS {
        let session = ProjectSession::load(&repo_root().join(rel)).expect("the project loads");
        let mut ops = Vec::new();
        for (index, tc) in session.toolpath_configs().iter().enumerate() {
            let env = session
                .safe_envelope(tc, rs_cam_viz::state::simulation::run_modulation())
                .and_then(Result::ok);
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
            let deepest_mm = deepest
                .get(key)
                .and_then(|ops| ops.get(index.to_string()))
                .and_then(serde_json::Value::as_f64);
            // One sample at the deepest depth: `field_top` reads only the
            // depth of the deepest sample.
            let samples: Vec<CutPoint3> = deepest_mm
                .map(|ap_mm| CutPoint3 {
                    ae_mm: 0.0,
                    ap_mm,
                    force_n: None,
                    weight_s: 0.0,
                    move_index: 0,
                    sample_index: 0,
                })
                .into_iter()
                .collect();
            let top = env.as_ref().map(|e| field_top(e, &samples));
            let top_rule = match deepest_mm {
                Some(d) => format!(
                    "field_top(envelope, deepest sample {d} mm): the GUI chart top after a \
                     simulation of every operation (the deepest sample read from the GUI cut \
                     force card)"
                ),
                None => "field_top(envelope, no samples): the GUI chart top where the stated \
                         depth is at least the deepest sample"
                    .to_owned(),
            };
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
                "top_rule": top_rule,
                "deepest_sample_mm": deepest_mm,
                "cell_mm": plane.as_ref().map(|p| p.field.cell_mm()),
                "plane": plane,
            }));
        }
        let doc = serde_json::json!({ "project": key, "file": rel, "ops": ops });
        let path = out.join(format!("field_{key}.json"));
        std::fs::write(&path, serde_json::to_string(&doc).expect("json")).expect("write");
    }
}
