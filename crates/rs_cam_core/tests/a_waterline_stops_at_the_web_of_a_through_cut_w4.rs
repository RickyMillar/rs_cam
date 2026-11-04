//! W4 (D69 §4, operator ruling 2026-10-28): on a through cut the Waterline
//! stops at the web the job holds, and the job ends with a tabbed Profile.
//!
//! Pre-registration:
//! `planning/adaptive_research_2026-10-23/integration/W4_PREREG.md` (lead
//! approval: per ring, a check that warns, the web from the configs). On a
//! through part (model bottom at the stock bottom) the Waterline cuts no
//! OUTER ring at or below the web Z; a hole ring keeps its levels. The web Z,
//! from the project only:
//!
//! 1. a later tabbed Profile that cuts to the stock bottom: its tab top;
//! 2. else an earlier roughing op that cuts to the model bottom: its floor
//!    plus its axial stock to leave;
//! 3. else no web (today's ladder), and the Caution
//!    `geom.waterline_part_unheld` names the missing tabbed Profile.
//!
//! The fixture: a 6 mm plate (drafted outer wall, a vertical 10 mm square
//! hole) whose bottom is the stock bottom; a helical Waterline at Z 5.5 to
//! 0.5 by 1 mm.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::op_fixtures::no_dressups;
use common::session::{
    mesh_model, pinned_heights, polygon_model, square_polygon, stock_over, toolpath_config,
};
use common::tools::endmill_tool_config;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::floor::Floor;
use rs_cam_core::compute::operation_configs::WaterlineConfig;
use rs_cam_core::compute::toolpath_stats::{WaterlineLadderFinding, WebSource};
use rs_cam_core::diagnostics::{Severity, ids};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::session::{LoadedModel, ProjectSession, ProjectSessionBuilder};
use rs_cam_core::toolpath::Toolpath;

const OUTER_BOTTOM: f64 = 20.0;
const OUTER_TOP: f64 = 18.0;
const HOLE: f64 = 5.0;
const PLATE: f64 = 6.0;
const TOOL_D: f64 = 6.0;
const TAB_HEIGHT: f64 = 2.0;

/// The plate, lifted by `lift` (0: a through part; above 0: a relief that
/// stands on stock).
fn plate(lift: f64) -> TriangleMesh {
    let mut vertices: Vec<P3> = Vec::new();
    let mut square = |half: f64, z: f64| -> [u32; 4] {
        let mut ids = [0_u32; 4];
        for (k, (sx, sy)) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
            .into_iter()
            .enumerate()
        {
            vertices.push(P3::new(sx * half, sy * half, z + lift));
            ids[k] = u32::try_from(vertices.len() - 1).expect("few vertices");
        }
        ids
    };
    let ob = square(OUTER_BOTTOM, 0.0);
    let ot = square(OUTER_TOP, PLATE);
    let hb = square(HOLE, 0.0);
    let ht = square(HOLE, PLATE);
    let mut triangles: Vec<[u32; 3]> = Vec::new();
    let mut quad = |a: u32, b: u32, c: u32, d: u32| {
        triangles.push([a, b, c]);
        triangles.push([a, c, d]);
    };
    for k in 0..4 {
        let n = (k + 1) % 4;
        quad(ob[k], ob[n], ot[n], ot[k]);
        quad(hb[n], hb[k], ht[k], ht[n]);
        quad(ot[k], ot[n], ht[n], ht[k]);
        quad(ob[k], hb[k], hb[n], ob[n]);
    }
    TriangleMesh::from_raw(vertices, triangles)
}

/// The plate mesh with the outline square as a drawing item (a Profile
/// reads it).
fn model(lift: f64) -> LoadedModel {
    let mut m = mesh_model(plate(lift), "w4_plate");
    m.items = polygon_model(vec![square_polygon(OUTER_BOTTOM)], "w4_outline").items;
    m
}

/// What the job holds around the Waterline.
#[derive(Clone, Copy, PartialEq)]
enum Job {
    /// The Waterline alone.
    Alone,
    /// A 3D Rough before the Waterline.
    RoughFirst,
    /// The Waterline, then a tabbed Profile to the stock bottom.
    TabbedProfileAfter,
}

/// The session; the index of the Waterline.
fn session(job: Job, lift: f64) -> (ProjectSession, usize) {
    let mut builder =
        ProjectSessionBuilder::new().stock(stock_over(OUTER_BOTTOM + 8.0, PLATE + lift + 2.0));
    let tool_idx = builder.add_tool(endmill_tool_config(TOOL_D));
    let tool_id = rs_cam_core::ids::ToolId::from_raw(builder.tools()[tool_idx].id.raw());
    let model_id = builder.add_model(model(lift));
    let mut index = 0;
    if job == Job::RoughFirst {
        let rough = toolpath_config(
            "rough",
            OperationConfig::new_default(OperationType::Adaptive3d),
            tool_id,
            model_id,
        );
        let _ = builder.add_toolpath(0, rough).expect("add the rough");
        index = 1;
    }
    let mut waterline = toolpath_config(
        "waterline",
        OperationConfig::Waterline(WaterlineConfig {
            z_step: Some(1.0),
            sampling: 0.5,
            ..WaterlineConfig::default()
        }),
        tool_id,
        model_id,
    );
    waterline.heights = pinned_heights(lift + 5.5, lift + 0.5);
    waterline.dressups = no_dressups();
    let _ = builder
        .add_toolpath(0, waterline)
        .expect("add the waterline");
    if job == Job::TabbedProfileAfter {
        let mut op = OperationConfig::new_default(OperationType::Profile);
        if let OperationConfig::Profile(cfg) = &mut op {
            cfg.tab_count = 4;
            cfg.tab_height = TAB_HEIGHT;
            cfg.floor = Floor::StockBottom { depth: 0.0 };
        }
        let profile = toolpath_config("profile", op, tool_id, model_id);
        let _ = builder.add_toolpath(0, profile).expect("add the profile");
    }
    (builder.build(), index)
}

fn generate(job: Job, lift: f64) -> (ProjectSession, usize) {
    let (mut s, index) = session(job, lift);
    let cancel = AtomicBool::new(false);
    let _ = s
        .generate_toolpath(index, &cancel)
        .expect("the waterline generates");
    (s, index)
}

fn finding(s: &ProjectSession, index: usize) -> WaterlineLadderFinding {
    s.result_at(index)
        .expect("a result")
        .stats
        .waterline_ladder
        .expect("the ladder finding")
}

/// The lowest cutting Z of the moves outside the hole (the outer ring) and
/// inside it (the hole ring).
fn lowest_cuts(tp: &Toolpath) -> (f64, f64) {
    let (mut outer, mut hole) = (f64::INFINITY, f64::INFINITY);
    for m in tp.moves.iter().filter(|m| m.move_type.is_cutting()) {
        let inside = m.target.x.abs() < HOLE && m.target.y.abs() < HOLE;
        let slot = if inside { &mut hole } else { &mut outer };
        *slot = slot.min(m.target.z);
    }
    (outer, hole)
}

fn unheld(s: &ProjectSession, index: usize) -> bool {
    s.diagnose_toolpath_with_trace(index, None)
        .expect("diagnose")
        .iter()
        .any(|d| {
            d.id.as_str() == ids::GEOM_WATERLINE_PART_UNHELD
                && d.severity == Severity::Caution
                && !d.message.contains('\n')
        })
}

#[test]
fn a_later_tabbed_profile_sets_the_web_at_its_tab_top() {
    let (s, index) = generate(Job::TabbedProfileAfter, 0.0);
    let f = finding(&s, index);
    let web = f.web.expect("a web");
    assert_eq!(web.source, WebSource::TabbedProfile);
    assert!((web.z_mm - TAB_HEIGHT).abs() < 1e-9, "tab top {}", web.z_mm);
    assert_eq!(
        f.outer_rings_skipped, 2,
        "levels 1.5 and 0.5 of the outer ring"
    );
    assert!(!f.part_unheld);
    let (outer, hole) = lowest_cuts(s.result_at(index).expect("r").toolpath());
    assert!(
        outer > TAB_HEIGHT,
        "the outer ring stops above the tab top: {outer}"
    );
    assert!(
        (hole - 0.5).abs() < 1e-9,
        "the hole ring keeps level 0.5: {hole}"
    );
    assert!(!unheld(&s, index));
    assert!(
        f.message().contains("stops at the web Z 2.000"),
        "{}",
        f.message()
    );
}

#[test]
fn an_earlier_rough_sets_the_web_at_its_floor() {
    let (mut s, index) = generate(Job::RoughFirst, 0.0);
    let f = finding(&s, index);
    let web = f.web.expect("a web");
    assert_eq!(web.source, WebSource::EarlierFloor);
    // The 3D Rough default: the model bottom 0 plus its axial leave.
    let OperationConfig::Adaptive3d(rough) = &s.toolpath_configs()[0].operation else {
        panic!("toolpath 0 is the rough");
    };
    let leave = rough.stock_to_leave_axial;
    assert!((web.z_mm - leave).abs() < 1e-9, "web {}", web.z_mm);
    assert_eq!(f.outer_rings_skipped, 1, "level 0.5 of the outer ring");
    assert!(f.part_unheld, "no tabbed Profile follows");
    assert!(unheld(&s, index));
    let (outer, hole) = lowest_cuts(s.result_at(index).expect("r").toolpath());
    assert!(outer > leave, "{outer}");
    assert!((hole - 0.5).abs() < 1e-9, "{hole}");

    // The lead's check: the generated rough cuts no lower than the web
    // that its config gives, so the config rule and the result agree.
    let cancel = AtomicBool::new(false);
    let _ = s
        .generate_toolpath(0, &cancel)
        .expect("the rough generates");
    let rough_low = s
        .result_at(0)
        .expect("r")
        .toolpath()
        .moves
        .iter()
        .filter(|m| m.move_type.is_cutting())
        .map(|m| m.target.z)
        .fold(f64::INFINITY, f64::min);
    assert!(
        rough_low >= web.z_mm - 1e-6,
        "the rough's lowest cut {rough_low} is under the web {}",
        web.z_mm
    );
}

#[test]
fn a_relief_keeps_every_ring() {
    let (alone, i) = generate(Job::Alone, 1.0);
    let (rough_first, j) = generate(Job::RoughFirst, 1.0);
    let a = alone.result_at(i).expect("r").toolpath();
    let b = rough_first.result_at(j).expect("r").toolpath();
    assert_eq!(a.moves.len(), b.moves.len());
    assert!(
        a.moves
            .iter()
            .zip(&b.moves)
            .all(|(x, y)| x.target == y.target)
    );
    let f = finding(&rough_first, j);
    assert!(f.web.is_none() && !f.part_unheld, "not a through part");
}

#[test]
fn a_through_part_with_no_web_keeps_its_ladder_and_warns() {
    let (s, index) = generate(Job::Alone, 0.0);
    let f = finding(&s, index);
    assert!(f.web.is_none());
    assert_eq!(f.outer_rings_skipped, 0);
    assert!(f.part_unheld);
    assert!(unheld(&s, index));
    let (outer, _) = lowest_cuts(s.result_at(index).expect("r").toolpath());
    assert!((outer - 0.5).abs() < 1e-9, "today's ladder: {outer}");
}
