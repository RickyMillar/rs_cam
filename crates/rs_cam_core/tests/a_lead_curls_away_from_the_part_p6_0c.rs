//! P6.0c (geometry selection, SAFETY): a lead-in or lead-out arc goes to
//! the side away from the material.
//!
//! # The defect
//!
//! `dressup::apply_lead_in_out` curled every lead arc to the LEFT of
//! travel. Each case below has the op's default dressups with the lead on
//! at its default radius (2 mm), a 6 mm end mill (R = 3) or a 90 degree
//! V-bit (Chamfer), one 3 mm level. The measure is the worst intrusion of
//! any fed move below Z0 past the clearance the cut keeps from the
//! material: `R` for an end mill, the tip offset for Chamfer, `0` from the
//! line for Trace None. Measured on HEAD 52cf29db (`lead_intrusion_table`):
//!
//! | case | HEAD (mm) |
//! |---|---|
//! | Profile Outside, climb | 0.0092 |
//! | Profile Outside, conventional | 1.8369 |
//! | Profile Inside, climb | 2.0000 |
//! | Profile Inside, conventional | 2.0000 |
//! | Pocket round an island, climb | 2.0000 |
//! | Pocket round an island, conventional | 2.0000 |
//! | Trace Left | 1.8369 |
//! | Trace Right | 2.0000 |
//! | Trace None (deviation from the line) | 2.0000 |
//! | Chamfer Outside | 0.0616 |
//! | Chamfer Inside | 2.0000 |
//!
//! Two mechanisms:
//!
//! - **The side.** 1.8369 is the lead arc on the material side of a ring
//!   that starts on an arc join: Profile Outside conventional (a CCW ring
//!   round the part) and Trace Left (the same ring).
//! - **The concave start corner.** 2.0000 (the lead radius) is the tangent
//!   approach of a ring that starts at a sharp corner of an inward offset.
//!   The arc ends `r` back along the cut line, past the corner, into the
//!   wall of the previous edge. The side does not change it: both climb
//!   and conventional read 2.0000.
//!
//! # The rule under test
//!
//! The op states where the material is beside each closed pass
//! (`toolpath::RingMaterial`); the lead dressup turns each arc away from
//! it, and keeps the plunge when a lead sample is on the material side of
//! the pass ring (`dressup::lead_side`). A pass on the line (Trace None)
//! gets no lead. The bound is the P6.0b bound: two
//! `FlattenPolicy::UNTOLERANCED_MM` (the offset flatten and the merge cap)
//! plus float slack.
//!
//! # Red proof
//!
//! With `lead_side::pass_lead` made to return the HEAD rule (a left arc,
//! no air check) every arm but Profile Outside climb fails with the HEAD
//! numbers above, and `the_lead_stays_where_the_side_is_free` still passes
//! (the HEAD rule gives every case a lead). With the side rule but no air
//! check, the six concave-start arms (Profile Inside, Pocket, Trace Right,
//! Chamfer Inside) still fail at 2.0000. With no lead at all,
//! `the_lead_stays_where_the_side_is_free` fails. The P6.0c report holds
//! the runs.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
// The instruments print their tables.
#![allow(clippy::print_stderr)]

mod common;
#[path = "common/source_scan.rs"]
mod source_scan;

use std::sync::atomic::AtomicBool;

use common::session::{polygon_model, single_op_session_with};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{DressupConfig, LeadParams};
use rs_cam_core::compute::floor::Floor;
use rs_cam_core::compute::operation_configs::{
    ChamferConfig, PocketConfig, PocketPattern, ProfileConfig, TraceConfig,
};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::geo::{P2, P3};
use rs_cam_core::geometry::arc_util::linearize_arc;
use rs_cam_core::ops::profile::ProfileSide;
use rs_cam_core::ops::trace_path::TraceCompensation;
use rs_cam_core::polygon::{FlattenPolicy, Polygon2};
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};

/// Tool diameter (mm): radius 3, as P6.0 and P6.0b.
const TOOL_D: f64 = 6.0;
const R: f64 = TOOL_D / 2.0;
/// Cut depth (mm). One level.
const DEPTH: f64 = 3.0;
/// The longest step (mm) the measure lays along a move.
const ARC_SAMPLE_MM: f64 = 0.005;
/// The P6.0b bound (mm).
const BOUND: f64 = 2.0 * FlattenPolicy::UNTOLERANCED_MM + 1e-9;

/// A 200 x 200 x 10 mm board centred on X0 Y0, its top at Z0 (P6.0).
fn board() -> StockConfig {
    StockConfig {
        x: 200.0,
        y: 200.0,
        z: 10.0,
        origin_x: -100.0,
        origin_y: -100.0,
        origin_z: -10.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// The 40 x 20 mm part of P6.0.
fn part() -> Polygon2 {
    Polygon2::rectangle(-20.0, -10.0, 20.0, 10.0)
}

/// A 60 x 40 mm pocket round a 16 x 12 mm island.
fn pocket_with_island() -> Polygon2 {
    let outer = Polygon2::rectangle(-30.0, -20.0, 30.0, 20.0);
    let island = Polygon2::rectangle(-8.0, -6.0, 8.0, 6.0);
    let mut hole = island.exterior;
    hole.reverse();
    Polygon2::with_holes(outer.exterior, vec![hole])
}

fn profile(side: ProfileSide, climb: bool) -> OperationConfig {
    OperationConfig::Profile(ProfileConfig {
        side,
        climb,
        floor: Floor::top(DEPTH),
        depth_per_pass: DEPTH,
        tab_count: 0,
        finishing_passes: 0,
        ..ProfileConfig::default()
    })
}

fn pocket(climb: bool) -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        pattern: PocketPattern::Contour,
        climb,
        stepover: 2.0,
        floor: Floor::top(DEPTH),
        depth_per_pass: DEPTH,
        finishing_passes: 0,
        ..PocketConfig::default()
    })
}

fn trace(compensation: TraceCompensation) -> OperationConfig {
    OperationConfig::Trace(TraceConfig {
        depth: DEPTH,
        depth_per_pass: DEPTH,
        compensation,
        ..TraceConfig::default()
    })
}

fn chamfer(side: ProfileSide) -> OperationConfig {
    OperationConfig::Chamfer(ChamferConfig {
        side,
        ..ChamferConfig::default()
    })
}

fn vbit() -> ToolConfig {
    ToolConfig {
        diameter: 12.7,
        included_angle: 90.0,
        ..ToolConfig::new_default(ToolId(0), ToolType::VBit)
    }
}

/// The op's default dressups with the lead on at its default radius.
fn with_lead(op: &OperationConfig) -> DressupConfig {
    let mut d = DressupConfig::for_op(op.op_type());
    d.lead_in_out = Some(LeadParams::default());
    d
}

fn generate(outline: &Polygon2, op: &OperationConfig, tool: ToolConfig) -> Toolpath {
    let dressups = with_lead(op);
    let mut session = single_op_session_with(
        board(),
        tool,
        polygon_model(vec![outline.clone()], "p6_0c_lead"),
        "p6_0c",
        op.clone(),
        |tc| tc.dressups = dressups,
    );
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("the op generates");
    session
        .get_result(0)
        .expect("a result")
        .annotated()
        .toolpath
        .clone()
}

/// The XY sample points of every fed move below Z0, each with a flag: the
/// move is a lead move (`LeadIn` or `LeadOut`).
fn cut_points(tp: &Toolpath) -> Vec<(P2, bool)> {
    let mut out = Vec::new();
    for pair in tp.moves.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        let points: Vec<P3> = match b.move_type {
            MoveType::Rapid => continue,
            MoveType::Linear { .. } => {
                let n = (((b.target - a.target).norm() / ARC_SAMPLE_MM).ceil() as usize).max(1);
                (0..=n)
                    .map(|k| a.target + (b.target - a.target) * (k as f64 / n as f64))
                    .collect()
            }
            MoveType::ArcCW { i, j, .. } => {
                linearize_arc(a.target, b.target, i, j, true, ARC_SAMPLE_MM)
            }
            MoveType::ArcCCW { i, j, .. } => {
                linearize_arc(a.target, b.target, i, j, false, ARC_SAMPLE_MM)
            }
        };
        let lead = matches!(b.intent, MoveIntent::LeadIn | MoveIntent::LeadOut);
        for p in points {
            if p.z < -1e-9 {
                out.push((P2::new(p.x, p.y), lead));
            }
        }
    }
    out
}

fn point_segment_distance(p: P2, a: P2, b: P2) -> f64 {
    let ab = b - a;
    let len_sq = ab.norm_squared();
    if len_sq <= 0.0 {
        return (p - a).norm();
    }
    let t = ((p - a).dot(&ab) / len_sq).clamp(0.0, 1.0);
    (p - (a + ab * t)).norm()
}

fn edge_distance(p: P2, outline: &Polygon2) -> f64 {
    let mut rings = vec![&outline.exterior];
    rings.extend(outline.holes.iter());
    let mut best = f64::INFINITY;
    for ring in rings {
        for i in 0..ring.len() {
            let d = point_segment_distance(p, ring[i], ring[(i + 1) % ring.len()]);
            best = best.min(d);
        }
    }
    best
}

/// Where the material is, for the measure.
#[derive(Clone, Copy)]
enum Material {
    /// Inside the outline (a part).
    Inside,
    /// Outside the outline (a pocket or a hole; an island is material).
    Outside,
    /// Both sides of the line (an on-the-line groove).
    BothSides,
}

/// Signed distance (mm) from the material: positive in the air.
fn air_distance(p: P2, outline: &Polygon2, material: Material) -> f64 {
    let d = edge_distance(p, outline);
    let inside = outline.contains_point(&p);
    match material {
        Material::Inside => {
            if inside {
                -d
            } else {
                d
            }
        }
        Material::Outside => {
            if inside {
                d
            } else {
                -d
            }
        }
        Material::BothSides => -d,
    }
}

/// The worst intrusion (mm) past `clearance` of the cut points (all, and
/// the lead points only), and the lead point count.
fn intrusion(
    tp: &Toolpath,
    outline: &Polygon2,
    material: Material,
    clearance: f64,
) -> (f64, f64, usize) {
    let points = cut_points(tp);
    let mut all = f64::NEG_INFINITY;
    let mut lead = f64::NEG_INFINITY;
    let mut leads = 0;
    for (p, is_lead) in points {
        let i = clearance - air_distance(p, outline, material);
        all = all.max(i);
        if is_lead {
            leads += 1;
            lead = lead.max(i);
        }
    }
    (all, lead, leads)
}

struct Case {
    name: &'static str,
    outline: Polygon2,
    op: OperationConfig,
    tool: ToolConfig,
    material: Material,
    clearance: f64,
}

fn cases() -> Vec<Case> {
    let end = || endmill_tool_config(TOOL_D);
    let mut v = Vec::new();
    for climb in [true, false] {
        v.push(Case {
            name: if climb {
                "profile outside climb"
            } else {
                "profile outside conventional"
            },
            outline: part(),
            op: profile(ProfileSide::Outside, climb),
            tool: end(),
            material: Material::Inside,
            clearance: R,
        });
        v.push(Case {
            name: if climb {
                "profile inside climb"
            } else {
                "profile inside conventional"
            },
            outline: part(),
            op: profile(ProfileSide::Inside, climb),
            tool: end(),
            material: Material::Outside,
            clearance: R,
        });
        v.push(Case {
            name: if climb {
                "pocket island climb"
            } else {
                "pocket island conventional"
            },
            outline: pocket_with_island(),
            op: pocket(climb),
            tool: end(),
            material: Material::Outside,
            clearance: R,
        });
    }
    v.push(Case {
        name: "trace left",
        outline: part(),
        op: trace(TraceCompensation::Left),
        tool: end(),
        material: Material::Inside,
        clearance: R,
    });
    v.push(Case {
        name: "trace right",
        outline: part(),
        op: trace(TraceCompensation::Right),
        tool: end(),
        material: Material::Outside,
        clearance: R,
    });
    v.push(Case {
        name: "trace none",
        outline: part(),
        op: trace(TraceCompensation::None),
        tool: end(),
        material: Material::BothSides,
        clearance: 0.0,
    });
    v.push(Case {
        name: "chamfer outside",
        outline: part(),
        op: chamfer(ProfileSide::Outside),
        tool: vbit(),
        material: Material::Inside,
        clearance: ChamferConfig::default().tip_offset,
    });
    v.push(Case {
        name: "chamfer inside",
        outline: part(),
        op: chamfer(ProfileSide::Inside),
        tool: vbit(),
        material: Material::Outside,
        clearance: ChamferConfig::default().tip_offset,
    });
    v
}

/// The case named `name`.
fn case(name: &str) -> Case {
    cases()
        .into_iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("no case {name}"))
}

/// The worst intrusion of case `name`, and its lead sample count.
fn measure(name: &str) -> (f64, usize) {
    let case = case(name);
    let tp = generate(&case.outline, &case.op, case.tool.clone());
    let (all, _lead, leads) = intrusion(&tp, &case.outline, case.material, case.clearance);
    (all, leads)
}

fn assert_within_bound(name: &str) {
    let (worst, _) = measure(name);
    assert!(
        worst <= BOUND,
        "{name}: the cut goes {worst:.4} mm past its clearance from the material; the bound \
         is {BOUND:.4} mm. The lead arc is on the material side (see the HEAD table in the \
         module doc)."
    );
}

#[test]
fn profile_outside_climb_keeps_its_lead_off_the_part() {
    assert_within_bound("profile outside climb");
}

#[test]
fn profile_outside_conventional_keeps_its_lead_off_the_part() {
    assert_within_bound("profile outside conventional");
}

#[test]
fn profile_inside_climb_keeps_its_lead_off_the_wall() {
    assert_within_bound("profile inside climb");
}

#[test]
fn profile_inside_conventional_keeps_its_lead_off_the_wall() {
    assert_within_bound("profile inside conventional");
}

#[test]
fn pocket_climb_keeps_its_leads_off_the_wall_and_the_island() {
    assert_within_bound("pocket island climb");
}

#[test]
fn pocket_conventional_keeps_its_leads_off_the_wall_and_the_island() {
    assert_within_bound("pocket island conventional");
}

#[test]
fn trace_left_keeps_its_lead_off_the_part() {
    assert_within_bound("trace left");
}

#[test]
fn trace_right_keeps_its_lead_off_the_wall() {
    assert_within_bound("trace right");
}

/// On the line both sides are material: the pass keeps its plunge and
/// retract, so the cut stays on the line.
#[test]
fn trace_none_stays_on_the_line() {
    let (worst, leads) = measure("trace none");
    assert_eq!(leads, 0, "trace none: a pass on the line gets no lead arc");
    assert!(
        worst <= BOUND,
        "trace none: the cut leaves the line by {worst:.4} mm"
    );
}

#[test]
fn chamfer_outside_keeps_its_lead_off_the_part() {
    assert_within_bound("chamfer outside");
}

#[test]
fn chamfer_inside_keeps_its_lead_off_the_wall() {
    assert_within_bound("chamfer inside");
}

/// Non-vacuity: where the start of the ring leaves room, the lead stays.
/// A fix that drops every lead passes the bound arms; it fails here.
#[test]
fn the_lead_stays_where_the_side_is_free() {
    for name in [
        "profile outside climb",
        "profile outside conventional",
        "pocket island climb",
        "pocket island conventional",
        "trace left",
        "chamfer outside",
    ] {
        let (_, leads) = measure(name);
        assert!(leads > 0, "{name}: the lead is gone");
    }
}

/// `RingMaterial::NotStated` is the one statement the compiler does not
/// check: an op could write it to skip the side. Only the two surface ops
/// write it (their lead is held by the surface probe). The type
/// (`toolpath.rs`) and its one reader (`dressup/lead_side.rs`) name it too.
/// Test code is free to name it. The scan reads the source with comments
/// removed, so a doc line that names it does not count.
#[test]
fn only_the_surface_ops_state_no_side_p6_0c() {
    const NEEDLE: &str = "RingMaterial::NotStated";
    const WRITERS: [&str; 2] = ["ops/waterline.rs", "finish/steep_shallow.rs"];
    const TYPE_AND_READER: [&str; 2] = ["toolpath.rs", "dressup/lead_side.rs"];
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = Vec::new();
    for path in source_scan::rust_files(&src) {
        let text = source_scan::strip_comments(&std::fs::read_to_string(&path).unwrap());
        // Test code: a `tests.rs` file or the body of a `mod tests`.
        let production = text.split("mod tests {").next().unwrap_or("");
        let is_test_file = path.file_name().is_some_and(|n| n == "tests.rs");
        if !is_test_file && production.contains(NEEDLE) {
            let rel = path.strip_prefix(&src).unwrap();
            found.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    // Non-vacuity: both writers are found by the scan.
    for writer in WRITERS {
        assert!(
            found.iter().any(|f| f == writer),
            "non-vacuity: {writer} must state `{NEEDLE}`; the scan found {found:?}"
        );
    }
    let strays: Vec<&String> = found
        .iter()
        .filter(|f| !WRITERS.contains(&f.as_str()) && !TYPE_AND_READER.contains(&f.as_str()))
        .collect();
    assert!(
        strays.is_empty(),
        "`{NEEDLE}` is written outside the surface ops: {strays:?}. A 2.5D op must state \
         `Inside`, `Outside` or `BothSides` for each closed pass (P6.0c)."
    );
}

#[test]
#[ignore = "instrument: the lead intrusion of each case"]
fn lead_intrusion_table() {
    for case in cases() {
        let tp = generate(&case.outline, &case.op, case.tool.clone());
        let (all, lead, leads) = intrusion(&tp, &case.outline, case.material, case.clearance);
        eprintln!(
            "{:30} moves {:5} lead pts {:6} worst all {:+.4} worst lead {:+.4}",
            case.name,
            tp.moves.len(),
            leads,
            all,
            lead
        );
    }
}

/// The lead move count of each fresh G-OPGOLDEN case: which ops the lead
/// rule reaches.
#[test]
#[ignore = "instrument: the lead moves of each fresh G-OPGOLDEN case"]
fn golden_fresh_lead_counts() {
    use common::op_fixtures::{CaseSpec, Fixture, Frame, Prior, case_session};
    use rs_cam_core::compute::catalog::OperationType;
    use rs_cam_core::compute::config::StockSource;
    for op_type in OperationType::ALL {
        let (mut session, index) = case_session(CaseSpec {
            name: "fresh",
            op: OperationConfig::new_default(*op_type),
            prior: Prior::None,
            stock_source: StockSource::Fresh,
            frame: Frame::TopUp,
            fixture: Fixture::Standard,
            tool: None,
        });
        let cancel = AtomicBool::new(false);
        let Ok(result) = session.generate_toolpath(index, &cancel) else {
            eprintln!("{:20} refused", op_type.kind_str());
            continue;
        };
        let tp = result.toolpath();
        let lead_in = tp
            .moves
            .iter()
            .filter(|m| m.intent == MoveIntent::LeadIn)
            .count();
        let lead_out = tp
            .moves
            .iter()
            .filter(|m| m.intent == MoveIntent::LeadOut)
            .count();
        eprintln!(
            "{:20} moves {:6} lead_in {:5} lead_out {:5}",
            op_type.kind_str(),
            tp.moves.len(),
            lead_in,
            lead_out
        );
    }
}

/// The lead intrusion of the G-OPGOLDEN flat cases that carry a lead
/// (Trace and Chamfer, fresh and rest), for the before and after record.
/// Trace (compensation None) measures the deviation from the outline;
/// Chamfer (Outside) measures past the tip offset from the part.
#[test]
#[ignore = "instrument: the lead intrusion of the G-OPGOLDEN lead cases"]
fn golden_lead_intrusion() {
    use common::op_fixtures::{
        CaseSpec, Fixture, Frame, OUTLINE_HALF_X, OUTLINE_HALF_Y, Prior, case_session, run_prior,
    };
    use rs_cam_core::compute::catalog::OperationType;
    use rs_cam_core::compute::config::StockSource;
    let outline = Polygon2::rectangle(
        -OUTLINE_HALF_X,
        -OUTLINE_HALF_Y,
        OUTLINE_HALF_X,
        OUTLINE_HALF_Y,
    );
    for (op_type, material, clearance) in [
        (OperationType::Trace, Material::BothSides, 0.0),
        (
            OperationType::Chamfer,
            Material::Inside,
            ChamferConfig::default().tip_offset,
        ),
    ] {
        for rest in [false, true] {
            let (mut session, index) = case_session(CaseSpec {
                name: if rest { "rest" } else { "fresh" },
                op: OperationConfig::new_default(op_type),
                prior: if rest { Prior::RestPrior } else { Prior::None },
                stock_source: if rest {
                    StockSource::FromRemainingStock
                } else {
                    StockSource::Fresh
                },
                frame: Frame::TopUp,
                fixture: Fixture::Standard,
                tool: None,
            });
            if rest {
                run_prior(&mut session);
            }
            let cancel = AtomicBool::new(false);
            let result = session
                .generate_toolpath(index, &cancel)
                .expect("the op generates");
            let tp = result.toolpath();
            let (all, lead, leads) = intrusion(tp, &outline, material, clearance);
            eprintln!(
                "{}/{}: moves {}, lead points {leads}, worst all {all:+.4}, worst lead {lead:+.4}",
                op_type.kind_str(),
                if rest { "rest" } else { "fresh" },
                tp.moves.len(),
            );
        }
    }
}
