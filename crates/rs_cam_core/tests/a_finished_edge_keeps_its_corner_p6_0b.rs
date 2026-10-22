//! P6.0b (geometry selection, SAFETY): a finished edge keeps its corner
//! after the default dressups.
//!
//! # The defect
//!
//! P6.0 (`an_offset_never_cuts_inside_its_corner_p6_0.rs`) holds the op
//! output with every dressup OFF: the tool centre keeps `r - 0.010 mm` from
//! the part. With the default Profile dressups the cut still went into the
//! corner band. Tool radius 3, the P6.0 fixtures, measured on HEAD 6dd89feb
//! (`per_dressup_intrusion`, one dressup off at a time):
//!
//! | arm | defaults | arc fit off | merge off | both off |
//! |---|---|---|---|---|
//! | Profile Outside, rectangle | 0.0365 | 0.2284 | 0.0092 | 0.0092 |
//! | Profile Inside, L (reflex) | 0.0000 | 0.2284 | 0.0000 | 0.0092 |
//! | Pocket wall, L (reflex) | 0.0000 | 0.2284 | 0.0000 | 0.0092 |
//!
//! Entry, link, rapid order and dogbone moved nothing. The cause is the
//! segment merge (`dressup::condition::merge_linear_runs`), at its roughing
//! default of 0.3 mm. RDP keeps every dropped point within the tolerance of
//! the new chord, on EITHER side. At a corner the arc join is centred on
//! the part corner, so a chord over it moves toward the part. Arc fitting
//! runs first and converts most arc joins to G2/G3, which hides the merge
//! on the L. On the rectangle the ring starts on an arc, and the merge
//! folds the last facet into the long closing straight: the chord
//! (20, -13) -> (-20.469, -12.963) passes 0.0365 mm inside the corner arc.
//! The arm with arc fit off is a configuration the operator reaches with
//! one dressup field, so it is a sentry arm too.
//!
//! # The rule under test
//!
//! An op whose cut can be a finished edge
//! (`OperationTransformCapabilities::finished_edge`) runs the dressup
//! segment merge at most at `FlattenPolicy::UNTOLERANCED_MM`. The merge does
//! not know the material side, so the cap is on both sides. The `for_role`
//! premise "roughing leaves >= 0.5 mm stock" is false for Pocket, Profile
//! and Zigzag: they have no stock-to-leave and cut the finished wall.
//!
//! The bound: the P6.0 flatten deviation (`UNTOLERANCED_MM`), plus the
//! merge cap (`UNTOLERANCED_MM` again: RDP keeps a dropped point within the
//! tolerance of the chord, and the polyline between two points is straight),
//! plus float slack. Arc fitting stays at its own 0.05 mm dial: it fits the
//! P6.0 arc joins, whose points lie on the arc, and measured 0.0092 mm (the
//! flatten itself) in every arm.
//!
//! # A second defect, fixed in P6.0c
//!
//! Trace Left with its default dressups cut 1.8369 mm into the band. Only
//! the lead-in/out dressup moved it (lead off: 0.0000). The lead arc curled
//! to the LEFT of travel (`dressup::apply_lead_in_out`). Trace Left offset
//! OUTWARD then and ran the ring CCW, so the part was on the left, and both
//! lead arcs went into the part. P6.0c turns the arc away from the material
//! the op states (`a_lead_curls_away_from_the_part_p6_0c.rs`), and
//! [`trace_right_keeps_each_corner_with_its_lead`] holds the bound.
//!
//! O-1 (P6.2, operator ruling 2026-10-21): Trace `Left` is now the left of
//! travel, inward on a CCW loop. The outward cut round the part is Trace
//! `Right`, so the Trace arms here moved from `Left` to `Right`; the
//! toolpath they measure is the same.
//!
//! # Red proof
//!
//! With the cap reverted in `dressup_apply.rs` (the merge at
//! `merge.tolerance`), the profile outside default arm reads 0.0365 mm and
//! each "merge without arc fit" arm 0.2284 mm. The P6.0b report holds the
//! run.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
// The two instruments print their tables.
#![allow(clippy::print_stderr)]

mod common;

use std::sync::atomic::AtomicBool;

use common::op_fixtures::{
    CaseSpec, EACH_FACE_UPPER_Z, FLAT_X, FLAT_Y, FLAT_Z, Fixture, Frame, OUTLINE_HALF_X,
    OUTLINE_HALF_Y, Prior, case_session, each_face_model, flat_stock, mirrored_model, run_prior,
    tool_for,
};
use common::session::{polygon_model, single_op_session_with};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{DressupConfig, DressupEntryStyle, StockSource};
use rs_cam_core::compute::floor::Floor;
use rs_cam_core::compute::operation_configs::{
    PocketConfig, PocketPattern, ProfileConfig, TraceConfig,
};
use rs_cam_core::geo::{P2, P3};
use rs_cam_core::geometry::arc_util::linearize_arc;
use rs_cam_core::ops::profile::ProfileSide;
use rs_cam_core::ops::trace_path::TraceCompensation;
use rs_cam_core::polygon::{FlattenPolicy, Polygon2};
use rs_cam_core::selection::{MatchKey, Selection, SelectionEdit};
use rs_cam_core::session::{
    AddModelArgs, Command, EditSelectionArgs, ProjectSession, SelectionTarget, SetToolpathModelArgs,
};
use rs_cam_core::toolpath::{MoveType, Toolpath};

/// Tool diameter (mm): radius 3, as P6.0.
const TOOL_D: f64 = 6.0;
const R: f64 = TOOL_D / 2.0;
/// Cut depth (mm). One level.
const DEPTH: f64 = 3.0;
/// The bound (mm): the P6.0 flatten deviation plus the P6.0b merge cap,
/// both `FlattenPolicy::UNTOLERANCED_MM`, plus float slack.
const BOUND: f64 = 2.0 * FlattenPolicy::UNTOLERANCED_MM + 1e-9;
/// The longest chord (mm) the measure lays on an arc move. Its sagitta on
/// the smallest arc here (the 2 mm lead) is `0.005^2 / (8 * 2)`, under
/// 2e-6 mm, so the measure adds no visible intrusion.
const ARC_SAMPLE_MM: f64 = 0.005;

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

/// The 40 x 20 mm part of P6.0: four convex 90 degree corners.
fn part() -> Polygon2 {
    Polygon2::rectangle(-20.0, -10.0, 20.0, 10.0)
}

/// The L of P6.0, CCW, with ONE reflex 90 degree corner at (15, 15).
fn l_outline() -> Polygon2 {
    Polygon2::new(vec![
        P2::new(0.0, 0.0),
        P2::new(40.0, 0.0),
        P2::new(40.0, 15.0),
        P2::new(15.0, 15.0),
        P2::new(15.0, 40.0),
        P2::new(0.0, 40.0),
    ])
}

fn profile(side: ProfileSide) -> OperationConfig {
    OperationConfig::Profile(ProfileConfig {
        side,
        floor: Floor::top(DEPTH),
        depth_per_pass: DEPTH,
        tab_count: 0,
        finishing_passes: 0,
        ..ProfileConfig::default()
    })
}

/// The Contour pocket: its outermost ring is the wall pass.
fn pocket() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        pattern: PocketPattern::Contour,
        stepover: 2.0,
        floor: Floor::top(DEPTH),
        depth_per_pass: DEPTH,
        finishing_passes: 0,
        ..PocketConfig::default()
    })
}

fn trace_right() -> OperationConfig {
    OperationConfig::Trace(TraceConfig {
        depth: DEPTH,
        depth_per_pass: DEPTH,
        compensation: TraceCompensation::Right,
        ..TraceConfig::default()
    })
}

/// The op's default dressups (`DressupConfig::for_op`), as a new toolpath
/// gets them.
fn defaults(op: &OperationConfig) -> DressupConfig {
    DressupConfig::for_op(op.op_type())
}

/// The defaults with the segment merge on at its default and arc fitting
/// off: the merge alone, one dressup field away from the defaults.
fn merge_without_arc_fit(op: &OperationConfig) -> DressupConfig {
    let mut d = defaults(op);
    d.arc_fitting = None;
    d.segment_merge = Some(
        DressupConfig::for_op(OperationType::Profile)
            .segment_merge
            .unwrap(),
    );
    d
}

/// The op output through the session door, with `dressups`.
fn generate(outline: &Polygon2, op: &OperationConfig, dressups: DressupConfig) -> Toolpath {
    let mut session = single_op_session_with(
        board(),
        endmill_tool_config(TOOL_D),
        polygon_model(vec![outline.clone()], "p6_0b_corner"),
        "p6_0b",
        op.clone(),
        |tc| tc.dressups = dressups,
    );
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("the op generates");
    session
        .result_at(0)
        .expect("a result")
        .annotated()
        .toolpath
        .clone()
}

/// The XY segments of every fed move whose both ends sit below the stock
/// top (Z0): the cut. An arc move is laid as chords of [`ARC_SAMPLE_MM`].
fn cut_segments(tp: &Toolpath) -> Vec<(P2, P2)> {
    cut_segments_below(tp, 0.0)
        .into_iter()
        .map(|(a, b, _)| (a, b))
        .collect()
}

/// [`cut_segments`] below `z_top`, each with the higher Z of its ends.
fn cut_segments_below(tp: &Toolpath, z_top: f64) -> Vec<(P2, P2, f64)> {
    let mut out = Vec::new();
    for pair in tp.moves.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        let points: Vec<P3> = match b.move_type {
            MoveType::Rapid => continue,
            MoveType::Linear { .. } => vec![a.target, b.target],
            MoveType::ArcCW { i, j, .. } => {
                linearize_arc(a.target, b.target, i, j, true, ARC_SAMPLE_MM)
            }
            MoveType::ArcCCW { i, j, .. } => {
                linearize_arc(a.target, b.target, i, j, false, ARC_SAMPLE_MM)
            }
        };
        for w in points.windows(2) {
            if w[0].z >= z_top - 1e-9 || w[1].z >= z_top - 1e-9 {
                continue;
            }
            let (pa, pb) = (P2::new(w[0].x, w[0].y), P2::new(w[1].x, w[1].y));
            if (pb - pa).norm() > 1e-12 {
                out.push((pa, pb, w[0].z.max(w[1].z)));
            }
        }
    }
    out
}

fn ring_segments(ring: &[P2]) -> Vec<(P2, P2)> {
    (0..ring.len())
        .map(|i| (ring[i], ring[(i + 1) % ring.len()]))
        .collect()
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

fn cross(o: P2, a: P2, b: P2) -> f64 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

/// Exact distance between two segments: zero when they cross, else the
/// least of the four end-point-to-segment distances.
fn segment_segment_distance(a: P2, b: P2, c: P2, d: P2) -> f64 {
    let (d1, d2) = (cross(c, d, a), cross(c, d, b));
    let (d3, d4) = (cross(a, b, c), cross(a, b, d));
    if d1 * d2 < 0.0 && d3 * d4 < 0.0 {
        return 0.0;
    }
    point_segment_distance(a, c, d)
        .min(point_segment_distance(b, c, d))
        .min(point_segment_distance(c, a, b))
        .min(point_segment_distance(d, a, b))
}

/// The worst intrusion (mm) of the cut into the band of radius `r` round
/// the outline edges, and the cut point nearest to an edge.
fn worst_intrusion(cut: &[(P2, P2)], outline: &Polygon2, r: f64) -> (f64, P2) {
    let mut edges = ring_segments(&outline.exterior);
    for hole in &outline.holes {
        edges.extend(ring_segments(hole));
    }
    let mut worst = (f64::NEG_INFINITY, P2::new(f64::NAN, f64::NAN));
    for &(a, b) in cut {
        for &(c, d) in &edges {
            let intrusion = r - segment_segment_distance(a, b, c, d);
            if intrusion > worst.0 {
                let near = if point_segment_distance(a, c, d) <= point_segment_distance(b, c, d) {
                    a
                } else {
                    b
                };
                worst = (intrusion, near);
            }
        }
    }
    worst
}

/// The intrusion of `op` on `outline` with `dressups`.
fn intrusion(outline: &Polygon2, op: &OperationConfig, dressups: DressupConfig) -> (f64, P2) {
    let tp = generate(outline, op, dressups);
    let cut = cut_segments(&tp);
    assert!(!cut.is_empty(), "the op cut nothing");
    worst_intrusion(&cut, outline, R)
}

fn assert_within_bound(arm: &str, outline: &Polygon2, op: &OperationConfig, d: DressupConfig) {
    let (worst, near) = intrusion(outline, op, d);
    assert!(
        worst <= BOUND,
        "{arm}: the tool centre cuts {worst:.4} mm inside the corner band near ({:.3}, \
         {:.3}); the bound is {BOUND:.4} mm (2 x FlattenPolicy::UNTOLERANCED_MM). On HEAD \
         before P6.0b: 0.0365 mm (Profile Outside, defaults) and 0.2284 mm (merge without \
         arc fit).",
        near.x,
        near.y,
    );
}

// ── The default dressups ───────────────────────────────────────────────

#[test]
fn profile_outside_keeps_each_convex_corner() {
    let op = profile(ProfileSide::Outside);
    assert_within_bound("profile outside, defaults", &part(), &op, defaults(&op));
}

#[test]
fn profile_inside_keeps_the_reflex_corner() {
    let op = profile(ProfileSide::Inside);
    assert_within_bound(
        "profile inside L, defaults",
        &l_outline(),
        &op,
        defaults(&op),
    );
}

#[test]
fn pocket_wall_keeps_the_reflex_corner() {
    let op = pocket();
    assert_within_bound("pocket wall L, defaults", &l_outline(), &op, defaults(&op));
}

/// Trace Right (Trace Left before O-1) with its default dressups less the
/// lead (the lead arm is below): entry, link, arc fit and rapid order.
#[test]
fn trace_right_keeps_each_corner_without_its_lead() {
    let op = trace_right();
    let mut d = defaults(&op);
    d.lead_in_out = None;
    assert_within_bound("trace right, defaults less the lead", &part(), &op, d);
}

// ── The merge alone (arc fitting off) ───────────────────────────────────

#[test]
fn the_merge_alone_keeps_the_profile_outside_corners() {
    let op = profile(ProfileSide::Outside);
    assert_within_bound(
        "profile outside, merge without arc fit",
        &part(),
        &op,
        merge_without_arc_fit(&op),
    );
}

#[test]
fn the_merge_alone_keeps_the_profile_inside_reflex_corner() {
    let op = profile(ProfileSide::Inside);
    assert_within_bound(
        "profile inside L, merge without arc fit",
        &l_outline(),
        &op,
        merge_without_arc_fit(&op),
    );
}

#[test]
fn the_merge_alone_keeps_the_pocket_wall_reflex_corner() {
    let op = pocket();
    assert_within_bound(
        "pocket wall L, merge without arc fit",
        &l_outline(),
        &op,
        merge_without_arc_fit(&op),
    );
}

/// A finishing op has no merge by default. An operator who turns it on
/// gets the cap too.
#[test]
fn the_merge_alone_keeps_the_trace_right_corners() {
    let op = trace_right();
    let mut d = merge_without_arc_fit(&op);
    d.lead_in_out = None;
    assert_within_bound("trace right, merge without arc fit", &part(), &op, d);
}

// ── The rule reads the row ──────────────────────────────────────────────

/// The cap reads `finished_edge`: an op that cuts a finished wall gets
/// `UNTOLERANCED_MM`; Face (no wall) and 3D Rough (a rough with a leave;
/// its planner merges) keep the operator's tolerance. The frozen W2.4
/// table holds the bit of every op.
#[test]
fn the_cap_reads_the_finished_edge_row() {
    let requested = 0.3;
    for op in [
        OperationType::Pocket,
        OperationType::Profile,
        OperationType::Zigzag,
        OperationType::Adaptive,
        OperationType::Trace,
        OperationType::Chamfer,
    ] {
        let t = op.capabilities().transform;
        assert!(t.finished_edge, "{op:?} states a finished edge");
        assert_eq!(
            t.segment_merge_tolerance(requested),
            FlattenPolicy::UNTOLERANCED_MM,
            "{op:?}: the merge is capped"
        );
    }
    for op in [OperationType::Face, OperationType::Adaptive3d] {
        let t = op.capabilities().transform;
        assert!(!t.finished_edge, "{op:?} states no finished edge");
        assert_eq!(
            t.segment_merge_tolerance(requested),
            requested,
            "{op:?}: the merge keeps the operator's tolerance"
        );
    }
}

// ── The lead side (P6.0c) ───────────────────────────────────────────────

/// Trace Right (Trace Left before O-1) with every default dressup, the
/// lead included. On HEAD 6dd89feb the lead arc curled into the part: 1.8369 mm into the band
/// (the lead-out ended at (-17.849, -11.163), 1.163 mm from the edge
/// y = -10). P6.0c turns it away from the part.
#[test]
fn trace_right_keeps_each_corner_with_its_lead() {
    let op = trace_right();
    assert_within_bound("trace right, defaults", &part(), &op, defaults(&op));
}

// ── Instruments ─────────────────────────────────────────────────────────

/// One change to a dressup config, by name in [`per_dressup_intrusion`].
type DressupEdit = fn(&mut DressupConfig);

/// One dressup off at a time, on each P6.0 arm: the table in the module
/// doc.
#[test]
#[ignore = "instrument: the corner intrusion per dressup"]
fn per_dressup_intrusion() {
    let arms: [(&str, Polygon2, OperationConfig); 4] = [
        ("profile outside", part(), profile(ProfileSide::Outside)),
        (
            "profile inside L",
            l_outline(),
            profile(ProfileSide::Inside),
        ),
        ("pocket wall L", l_outline(), pocket()),
        ("trace right", part(), trace_right()),
    ];
    for (arm, outline, op) in arms {
        let base = defaults(&op);
        let off: [(&str, DressupEdit); 9] = [
            ("defaults", |_| {}),
            ("entry off", |d| d.entry_style = DressupEntryStyle::None),
            ("arc fit off", |d| d.arc_fitting = None),
            ("merge off", |d| d.segment_merge = None),
            ("arc fit + merge off", |d| {
                d.arc_fitting = None;
                d.segment_merge = None;
            }),
            ("link off", |d| d.link_moves = None),
            ("lead off", |d| d.lead_in_out = None),
            ("rapid order off", |d| d.optimize_rapid_order = false),
            ("dogbone off", |d| d.dogbone = None),
        ];
        for (name, edit) in off {
            let mut d = base.clone();
            edit(&mut d);
            let (worst, near) = intrusion(&outline, &op, d);
            eprintln!(
                "{arm:18} {name:20} {worst:+.4} mm near ({:.3}, {:.3})",
                near.x, near.y
            );
        }
    }
}

/// The corner intrusion of the G-OPGOLDEN flat cases (the fixture outline
/// and the registry tool), for the before and after record.
#[test]
#[ignore = "instrument: the corner intrusion of the G-OPGOLDEN flat cases"]
fn golden_flat_case_intrusion() {
    let outline = Polygon2::rectangle(
        -OUTLINE_HALF_X,
        -OUTLINE_HALF_Y,
        OUTLINE_HALF_X,
        OUTLINE_HALF_Y,
    );
    let ops = [
        OperationType::Pocket,
        OperationType::Profile,
        OperationType::Zigzag,
        OperationType::Adaptive,
        OperationType::Trace,
        OperationType::Face,
    ];
    for op_type in ops {
        let r = tool_for(op_type).diameter / 2.0;
        let mut arms: Vec<(&str, OperationConfig, bool)> = vec![
            ("fresh", OperationConfig::new_default(op_type), false),
            ("rest", OperationConfig::new_default(op_type), true),
        ];
        match op_type {
            OperationType::Pocket => {
                let mut op = OperationConfig::new_default(op_type);
                if let OperationConfig::Pocket(c) = &mut op {
                    c.pattern = PocketPattern::Zigzag;
                }
                arms.push(("zigzag", op, false));
            }
            OperationType::Profile => {
                arms.push(("inside", profile_side(ProfileSide::Inside), false));
            }
            _ => {}
        }
        for (arm, op, rest) in arms {
            let (mut session, index) = case_session(CaseSpec {
                name: arm,
                op,
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
            let Ok(result) = session.generate_toolpath(index, &cancel) else {
                eprintln!("{}/{arm}: refused", op_type.kind_str());
                continue;
            };
            let cut = cut_segments(result.toolpath());
            let (worst, near) = worst_intrusion(&cut, &outline, r);
            eprintln!(
                "{}/{arm}: r {r:.3}, corner intrusion {worst:+.4} mm near ({:.3}, {:.3})",
                op_type.kind_str(),
                near.x,
                near.y
            );
        }
    }
    // The P3.5 arms: both faces of `each_face_model`, each its own floor:
    // the upper face (the outline less the 16 x 12 mm hole, at
    // `EACH_FACE_UPPER_Z`) and the lower face (the hole). A segment is
    // measured against the face it cuts. Profile (outside): outside the
    // outline against the outline, else against the hole. Pocket and
    // Zigzag: below the upper face against the hole, else against the
    // outline: on the top-up setup the upper level crosses the hole, which
    // goes deeper. On the flipped setup the upper level keeps the hole as
    // an island (measured: the wall ring at x = 38 + r), so there it is
    // measured against the outline and the hole. The flipped frame emits
    // in setup coordinates: the board centre at
    // (FLAT_X / 2, FLAT_Y / 2) and its top at FLAT_Z; both rectangles are
    // symmetric about the board mid-lines, so the mirror keeps them.
    for (op_type, arm, frame) in [
        (OperationType::Pocket, "each_face", Frame::TopUp),
        (OperationType::Pocket, "each_face_flipped", Frame::Flipped),
        (OperationType::Profile, "each_face", Frame::TopUp),
        (OperationType::Zigzag, "each_face", Frame::TopUp),
    ] {
        let r = tool_for(op_type).diameter / 2.0;
        let (dx, dy, z_top) = match frame {
            Frame::TopUp => (0.0, 0.0, 0.0),
            Frame::Flipped => (FLAT_X / 2.0, FLAT_Y / 2.0, FLAT_Z),
        };
        let at = |x0: f64, y0: f64, x1: f64, y1: f64| {
            Polygon2::rectangle(x0 + dx, y0 + dy, x1 + dx, y1 + dy)
        };
        let outer = at(
            -OUTLINE_HALF_X,
            -OUTLINE_HALF_Y,
            OUTLINE_HALF_X,
            OUTLINE_HALF_Y,
        );
        let hole = at(-8.0, -6.0, 8.0, 6.0);
        let upper = match frame {
            Frame::TopUp => outer.clone(),
            Frame::Flipped => {
                Polygon2::with_holes(outer.exterior.clone(), vec![hole.exterior.clone()])
            }
        };
        let mut op = OperationConfig::new_default(op_type);
        let floor = Floor::EachFace { depth: 0.0 };
        match &mut op {
            OperationConfig::Pocket(c) => c.floor = floor,
            OperationConfig::Profile(c) => c.floor = floor,
            OperationConfig::Zigzag(c) => c.floor = floor,
            _ => unreachable!("an each_face arm on another op"),
        }
        let (mut session, index) = case_session(CaseSpec {
            name: arm,
            op,
            prior: Prior::None,
            stock_source: StockSource::Fresh,
            frame,
            fixture: Fixture::Standard,
            tool: None,
        });
        use_faces(&mut session, index, frame);
        let cancel = AtomicBool::new(false);
        let Ok(result) = session.generate_toolpath(index, &cancel) else {
            eprintln!("{}/{arm}: refused", op_type.kind_str());
            continue;
        };
        let mut worst = (f64::NEG_INFINITY, P2::new(f64::NAN, f64::NAN));
        for (pa, pb, z) in cut_segments_below(result.toolpath(), z_top) {
            let mid = P2::new((pa.x + pb.x) / 2.0, (pa.y + pb.y) / 2.0);
            let face = if op_type == OperationType::Profile {
                if outer.contains_point(&mid) {
                    &hole
                } else {
                    &outer
                }
            } else if z < z_top + EACH_FACE_UPPER_Z - 1e-6 {
                &hole
            } else {
                &upper
            };
            let w = worst_intrusion(&[(pa, pb)], face, r);
            if w.0 > worst.0 {
                worst = w;
            }
        }
        eprintln!(
            "{}/{arm}: r {r:.3}, corner intrusion {:+.4} mm near ({:.3}, {:.3})",
            op_type.kind_str(),
            worst.0,
            worst.1.x,
            worst.1.y
        );
    }
}

/// Bind `each_face_model` (mirrored on a flipped setup) to toolpath
/// `index` and pick both faces, as G-OPGOLDEN `use_faces` does.
fn use_faces(session: &mut ProjectSession, index: usize, frame: Frame) {
    let model = match frame {
        Frame::TopUp => each_face_model(),
        Frame::Flipped => mirrored_model(each_face_model(), &flat_stock()),
    };
    let picks: Vec<MatchKey> = model.items.iter().map(|item| item.key().clone()).collect();
    let model_id = session
        .apply(Command::AddModel(AddModelArgs {
            model: Box::new(model),
        }))
        .expect("add the face model")
        .created
        .expect("the add reports the model id");
    let _ = session
        .apply(Command::SetToolpathModel(SetToolpathModelArgs {
            index,
            model_id: rs_cam_core::ids::ModelId::from_raw(model_id),
        }))
        .expect("bind the face model");
    let _ = session
        .apply(Command::EditSelection(EditSelectionArgs {
            index,
            target: SelectionTarget::Op,
            edit: SelectionEdit::Replace(Box::new(Selection::from_picks(picks))),
        }))
        .expect("pick both faces");
}

/// The default Profile with `side`.
fn profile_side(side: ProfileSide) -> OperationConfig {
    let mut op = OperationConfig::new_default(OperationType::Profile);
    if let OperationConfig::Profile(c) = &mut op {
        c.side = side;
    }
    op
}
